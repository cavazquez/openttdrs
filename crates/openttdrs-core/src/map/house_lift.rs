//! Ascensor de Large Office (`AnimateTile_Town` / `town_map.h`).

use crate::Climate;
use crate::cargodist::parity::Randomizer;
use crate::house_spec::{BUILDING_FLAG_IS_ANIMATED, HouseSpec, HouseSpecDef, NEW_HOUSE_OFFSET};
use crate::newgrf_callback::resolve_house_animation_callback_with_world;
use crate::newgrf_sprites::{
    CALLBACK_FAILED, CBID_HOUSE_ANIMATION_NEXT_FRAME, CBID_HOUSE_ANIMATION_SPEED,
};
use crate::town::Town;

use super::{Map, Tile, TileCoord, TileKind};

pub const LIFT_MAX_POSITION: u8 = 36;
const LIFT_DESTINATION_FLOORS: u8 = 7;
const LIFT_STEPS_PER_FLOOR: u8 = 6;
/// Bits bajos de `MAPE`/`m6` compartidos con `AnimatedTileState` de OpenTTD.
///
/// Una entrada que acaba de llegar a destino no se quita del vector `ANIT` en
/// el mismo pase: queda `Deleted` hasta el siguiente `AnimateAnimatedTiles`.
/// Si `TileLoop_Town` la reactiva entre ambos pases, `AddAnimatedTile` vuelve
/// a marcar el mismo slot como `Animated`, sin moverlo al final del vector.
const HOUSE_ANIMATION_STATE_MASK: u8 = 0x03;
const HOUSE_ANIMATION_STATE_NONE: u8 = 0;
const HOUSE_ANIMATION_STATE_DELETED: u8 = 1;
const HOUSE_ANIMATION_STATE_ACTIVE: u8 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LiftStep {
    Idle,
    Moving,
    Arrived,
}

#[must_use]
pub const fn lift_has_destination(tile: Tile) -> bool {
    tile.m7 & 1 != 0
}

#[must_use]
pub const fn lift_destination(tile: Tile) -> u8 {
    (tile.m7 >> 1) & 0x07
}

#[must_use]
pub const fn lift_position(tile: Tile) -> u8 {
    (tile.m6 >> 2) & 0x3F
}

#[must_use]
const fn house_animation_state(tile: Tile) -> u8 {
    tile.m6 & HOUSE_ANIMATION_STATE_MASK
}

#[must_use]
const fn with_house_animation_state(mut tile: Tile, state: u8) -> Tile {
    tile.m6 = (tile.m6 & !HOUSE_ANIMATION_STATE_MASK) | (state & HOUSE_ANIMATION_STATE_MASK);
    tile
}

#[must_use]
pub const fn with_lift_destination(mut tile: Tile, destination: u8) -> Tile {
    tile.m7 = (tile.m7 & !0x0F) | 1 | ((destination & 0x07) << 1);
    tile
}

#[must_use]
pub const fn with_lift_position(mut tile: Tile, position: u8) -> Tile {
    let clamped = if position > LIFT_MAX_POSITION {
        LIFT_MAX_POSITION
    } else {
        position
    };
    tile.m6 = (tile.m6 & 0x03) | ((clamped & 0x3F) << 2);
    tile
}

#[must_use]
pub const fn halt_lift(mut tile: Tile) -> Tile {
    tile.m7 &= !0x0F;
    tile
}

#[must_use]
pub fn house_tile_has_lift(tile: Tile) -> bool {
    if tile.kind != TileKind::House || tile.m3 & 0x80 == 0 {
        return false;
    }
    HouseSpec::get(tile.m8 & 0x0FFF)
        .is_some_and(|spec| spec.building_flags & BUILDING_FLAG_IS_ANIMATED != 0)
}

/// `true` si la entrada de `ANIT` pertenece a una casa `NewGRF`.
///
/// El catálogo puede no haberse rehidratado todavía al importar un SAV, por
/// eso el identificador persistido de `MAP8` es la fuente de verdad aquí.
#[must_use]
pub fn house_tile_has_newgrf_animation(tile: Tile) -> bool {
    tile.kind == TileKind::House && (tile.m8 & 0x0FFF) >= NEW_HOUSE_OFFSET
}

/// `true` si la tesela forma parte de la porción urbana que este runtime
/// conserva en la cola global `ANIT`.
#[must_use]
pub fn house_tile_has_modeled_animation(tile: Tile) -> bool {
    house_tile_has_lift(tile) || house_tile_has_newgrf_animation(tile)
}

/// Un paso de `AnimateTile_Town`; el destino ya debe estar asignado.
pub fn advance_house_lift(tile: &mut Tile) -> LiftStep {
    if !house_tile_has_lift(*tile) || !lift_has_destination(*tile) {
        return LiftStep::Idle;
    }
    let destination = lift_destination(*tile) * LIFT_STEPS_PER_FLOOR;
    let position = lift_position(*tile);
    let next = if position < destination {
        position + 1
    } else {
        position.saturating_sub(1)
    };
    *tile = with_lift_position(*tile, next);
    if next == destination {
        *tile = halt_lift(*tile);
        LiftStep::Arrived
    } else {
        LiftStep::Moving
    }
}

fn choose_lift_destination(position: u8, rng: &mut Randomizer) -> u8 {
    loop {
        let destination =
            u8::try_from(rng.random_range(u32::from(LIFT_DESTINATION_FLOORS))).unwrap_or(0);
        if destination != 1 && destination * LIFT_STEPS_PER_FLOOR != position {
            return destination;
        }
    }
}

/// Inserta una entrada de casa ya presente en `ANIT` sin alterar `MAPE`.
///
/// La hidratación de un SAV debe conservar el estado de animación original:
/// una entrada `Deleted` se elimina en el próximo pase, mientras una
/// `Animated` puede consumir `_random`. La activación en runtime usa una
/// función especializada que sí replica `AddAnimatedTile`.
pub fn add_house_animation_to_queue(active: &mut Vec<TileCoord>, coord: TileCoord) {
    if !active.contains(&coord) {
        active.push(coord);
    }
}

/// Alias de compatibilidad para importadores que sólo conocían ascensores.
pub fn add_house_lift_to_animation(active: &mut Vec<TileCoord>, coord: TileCoord) {
    add_house_animation_to_queue(active, coord);
}

/// Equivalente inmediato de `DeleteAnimatedTile(tile, true)` para una casa.
///
/// `DoClearSquare` lo invoca antes de sustituir una tesela animable. Quitar
/// con `swap_remove` replica la compactación sin orden estable de `ANIT`, que
/// decide qué ascensor/casa recibe cada palabra posterior del RNG global.
pub fn remove_house_animation_immediately(
    map: &mut Map,
    active: &mut Vec<TileCoord>,
    coord: TileCoord,
) -> bool {
    let Some(tile) = map.get(coord) else {
        return false;
    };
    if house_animation_state(tile) == HOUSE_ANIMATION_STATE_NONE {
        return false;
    }

    let _ = map.set_tile(
        coord,
        with_house_animation_state(tile, HOUSE_ANIMATION_STATE_NONE),
    );
    if let Some(index) = active.iter().position(|&entry| entry == coord) {
        active.swap_remove(index);
    }
    true
}

/// Equivalente de `AddAnimatedTile` para un ascensor de casa vanilla.
///
/// Mantiene la posición del vector si el tile estaba `Deleted`: OpenTTD sabe
/// que esa entrada todavía vive en `ANIT` y sólo cambia sus dos bits bajos de
/// `MAPE` a `Animated`. En un estado válido, `None` no tiene entrada y
/// `Deleted` sí la tiene; las comprobaciones del vector sólo protegen la
/// recuperación de un SAV corrupto sin alterar ese contrato normal.
pub fn activate_house_lift_animation(
    map: &mut Map,
    active: &mut Vec<TileCoord>,
    coord: TileCoord,
) -> bool {
    let Some(mut tile) = map.get(coord) else {
        return false;
    };
    if !house_tile_has_lift(tile) {
        return false;
    }
    if house_animation_state(tile) == HOUSE_ANIMATION_STATE_ACTIVE {
        return false;
    }

    add_house_animation_to_queue(active, coord);
    tile = with_house_animation_state(tile, HOUSE_ANIMATION_STATE_ACTIVE);
    let _ = map.set_tile(coord, tile);
    true
}

/// Equivalente de `AddAnimatedTile` para una casa `NewGRF`.
///
/// Una entrada `Deleted` ya conserva su slot dentro de `ANIT`; reactivarla no
/// debe moverla al final de la cola. El retorno sólo indica si el estado pasó
/// a `Animated`; no implica que haya cambiado el frame visible.
pub fn activate_newgrf_house_animation(
    map: &mut Map,
    active: &mut Vec<TileCoord>,
    coord: TileCoord,
) -> bool {
    let Some(mut tile) = map.get(coord) else {
        return false;
    };
    if !house_tile_has_newgrf_animation(tile)
        || house_animation_state(tile) == HOUSE_ANIMATION_STATE_ACTIVE
    {
        return false;
    }

    add_house_animation_to_queue(active, coord);
    tile = with_house_animation_state(tile, HOUSE_ANIMATION_STATE_ACTIVE);
    let _ = map.set_tile(coord, tile);
    true
}

/// Aplica el resultado de un callback de animación `NewGRF` de casa.
///
/// Modela `AnimationBase::ChangeAnimationFrame`: `0xFD` no cambia nada,
/// `0xFE` añade la tesela a `ANIT`, `0xFF` la marca para borrar en el próximo
/// pase y cualquier otro byte fija `MAP7` y la activa. Devuelve `true` sólo
/// si el frame cambió y por tanto el caller debe marcar la tesela dirty.
pub fn apply_newgrf_house_animation_callback_result(
    map: &mut Map,
    active: &mut Vec<TileCoord>,
    coord: TileCoord,
    result: u16,
) -> bool {
    // `CALLBACK_FAILED` comparte el byte bajo con la orden de borrar, pero no
    // debe tocar el estado de animación.
    if result == u16::MAX {
        return false;
    }
    let Some(mut tile) = map.get(coord) else {
        return false;
    };
    if !house_tile_has_newgrf_animation(tile) {
        return false;
    }

    match (result & 0xFF) as u8 {
        0xFD => false,
        0xFE => {
            let _ = activate_newgrf_house_animation(map, active, coord);
            false
        }
        0xFF => {
            if house_animation_state(tile) == HOUSE_ANIMATION_STATE_ACTIVE {
                tile = with_house_animation_state(tile, HOUSE_ANIMATION_STATE_DELETED);
                let _ = map.set_tile(coord, tile);
            }
            false
        }
        frame => {
            let changed = tile.m7 != frame;
            if changed {
                tile.m7 = frame;
                let _ = map.set_tile(coord, tile);
            }
            let _ = activate_newgrf_house_animation(map, active, coord);
            changed
        }
    }
}

#[derive(Debug, Default)]
struct NewgrfHouseAnimationStep {
    changed: bool,
    delete: bool,
    sound: Option<crate::NewgrfTileSound>,
}

struct NewgrfHouseAnimationContext<'a> {
    towns: &'a mut [Town],
    house_catalog: &'a [HouseSpecDef],
    climate: Climate,
}

/// Ejecuta `AnimationBase::AnimateTile` para una entrada urbana `NewGRF`.
///
/// CB20 se consulta antes del gate de cadencia, mientras que CB1A sólo toma
/// `Random()` cuando el flag correspondiente está presente y el tick llega a
/// la potencia de dos resultante. Los bits 8..14 de CB1A se devuelven como
/// una solicitud de sonido ambiental para el subsistema de audio.
fn step_newgrf_house_animation(
    map: &mut Map,
    context: &mut NewgrfHouseAnimationContext<'_>,
    coord: TileCoord,
    tile: Tile,
    tick: u64,
    rng: &mut Randomizer,
) -> NewgrfHouseAnimationStep {
    let house_id = tile.m8 & 0x0FFF;
    let Some(def) = crate::house_spec::house_spec_def(context.house_catalog, house_id) else {
        // Al importar una partida puede existir ANIT antes de rehidratar el
        // catálogo NewGRF. Igual que `AnimateNewHouseTile`, no se elimina la
        // entrada si no existe una spec viva para despacharla.
        return NewgrfHouseAnimationStep::default();
    };

    let mut speed = def.animation_speed.min(16);
    if def.has_animation_speed_callback() {
        let result = resolve_house_animation_callback_with_world(
            def,
            map,
            context.towns,
            context.house_catalog,
            context.climate,
            coord,
            CBID_HOUSE_ANIMATION_SPEED,
            0,
            0,
        );
        if result != CALLBACK_FAILED {
            speed = u8::try_from(result & 0xFF).unwrap_or(0).min(16);
        }
    }
    if !tick.is_multiple_of(1_u64 << u32::from(speed)) {
        return NewgrfHouseAnimationStep::default();
    }

    let mut frame = tile.m7;
    let mut frame_set_by_callback = false;
    let mut delete = false;
    let mut sound = None;
    if def.has_animation_next_frame_callback() {
        let random_bits = if def.animation_next_frame_uses_random_bits() {
            rng.next()
        } else {
            0
        };
        let result = resolve_house_animation_callback_with_world(
            def,
            map,
            context.towns,
            context.house_catalog,
            context.climate,
            coord,
            CBID_HOUSE_ANIMATION_NEXT_FRAME,
            random_bits,
            0,
        );
        if result != CALLBACK_FAILED {
            sound = crate::newgrf_tile_animation_sound_from_callback(def.grfid, result, coord);
            frame_set_by_callback = true;
            match result & 0xFF {
                0xFF => delete = true,
                0xFE => frame_set_by_callback = false,
                next_frame => frame = u8::try_from(next_frame).unwrap_or(0),
            }
        }
    }

    if !frame_set_by_callback {
        if frame < def.animation_frames {
            frame = frame.saturating_add(1);
        } else if frame == def.animation_frames && def.animation_loops() {
            frame = 0;
        } else {
            delete = true;
        }
    }

    let changed = tile.m7 != frame;
    if changed {
        let mut updated = tile;
        updated.m7 = frame;
        let _ = map.set_tile(coord, updated);
    }
    NewgrfHouseAnimationStep {
        changed,
        delete,
        sound,
    }
}

/// Ejecuta el recorrido compartido de `AnimateAnimatedTiles`.
///
/// El callback parametrizado permite conservar la API histórica de ascensores
/// para importadores sin catálogo NewGRF, sin inventar ahí consumos de RNG.
fn step_house_animations_with_newgrf_stepper<F>(
    map: &mut Map,
    tick: u64,
    rng: &mut Randomizer,
    active: &mut Vec<TileCoord>,
    sound_events: &mut Vec<crate::NewgrfTileSound>,
    mut step_newgrf: F,
) -> Vec<TileCoord>
where
    F: FnMut(&mut Map, TileCoord, Tile, u64, &mut Randomizer) -> NewgrfHouseAnimationStep,
{
    let mut dirty = Vec::new();
    let mut index = 0;
    while index < active.len() {
        let coord = active[index];
        let Some(mut tile) = map.get(coord) else {
            // `AnimateAnimatedTiles` elimina con el último elemento, no
            // preservando el orden del resto del vector.
            active.swap_remove(index);
            continue;
        };

        // `AnimateAnimatedTiles` limpia `Deleted` antes de llamar al draw
        // proc, incluso en ticks donde `AnimateTile_Town` retorna por la
        // cadencia de cuatro. Reemplaza el slot con el último elemento igual
        // que el vector C++ y procesa ese reemplazo en esta misma pasada.
        if house_animation_state(tile) != HOUSE_ANIMATION_STATE_ACTIVE {
            tile = with_house_animation_state(tile, HOUSE_ANIMATION_STATE_NONE);
            let _ = map.set_tile(coord, tile);
            active.swap_remove(index);
            continue;
        }

        // `AnimateTile_Town` delega NewGRF antes de la cadencia de ascensores
        // vanilla. `DeleteAnimatedTile` mantiene el slot hasta la próxima
        // pasada global, igual que la rama de ascensores más abajo.
        if house_tile_has_newgrf_animation(tile) {
            let step = step_newgrf(map, coord, tile, tick, rng);
            if step.changed {
                dirty.push(coord);
            }
            if let Some(sound) = step.sound {
                sound_events.push(sound);
            }
            if step.delete
                && let Some(current) = map.get(coord)
            {
                let deleted = with_house_animation_state(current, HOUSE_ANIMATION_STATE_DELETED);
                let _ = map.set_tile(coord, deleted);
            }
            index += 1;
            continue;
        }

        // El draw proc de una casa sólo valida la spec después de este return.
        // Una entrada vieja de un tile ya reemplazado queda viva hasta el
        // próximo tick divisible por cuatro, pero una `Deleted` sí se limpió
        // arriba en cualquier tick.
        if tick & 3 != 0 {
            index += 1;
            continue;
        }
        if !house_tile_has_lift(tile) {
            tile = with_house_animation_state(tile, HOUSE_ANIMATION_STATE_DELETED);
            let _ = map.set_tile(coord, tile);
            index += 1;
            continue;
        }

        if !lift_has_destination(tile) {
            let destination = choose_lift_destination(lift_position(tile), rng);
            tile = with_lift_destination(tile, destination);
        }
        let step = advance_house_lift(&mut tile);
        let _ = map.set_tile(coord, tile);
        dirty.push(coord);
        if step == LiftStep::Arrived {
            // `DeleteAnimatedTile` marca `Deleted`; no hace `swap_remove`
            // hasta el siguiente `AnimateAnimatedTiles`. Así un TileLoop del
            // mismo tick puede reactivar el slot sin reordenar `ANIT`.
            tile = with_house_animation_state(tile, HOUSE_ANIMATION_STATE_DELETED);
            let _ = map.set_tile(coord, tile);
        }
        index += 1;
    }
    dirty
}

/// Ejecuta la porción urbana de `AnimateAnimatedTiles` sin catálogo NewGRF.
///
/// Se conserva como API de compatibilidad para los importadores históricos:
/// sus entradas NewGRF conservan ANIT y no consumen RNG hasta que usen la ruta
/// con contexto completo.
pub fn step_house_animations(
    map: &mut Map,
    tick: u64,
    rng: &mut Randomizer,
    active: &mut Vec<TileCoord>,
) -> Vec<TileCoord> {
    let mut ignored_sounds = Vec::new();
    step_house_animations_with_newgrf_stepper(
        map,
        tick,
        rng,
        active,
        &mut ignored_sounds,
        |_, _, _, _, _| NewgrfHouseAnimationStep::default(),
    )
}

/// Ejecuta la porción urbana de `AnimateAnimatedTiles` con CB1A/CB20 NewGRF.
///
/// Comparte el vector persistido `ANIT` con ascensores vanilla; los cambios de
/// frame se devuelven para que el caller marque la tesela visualmente dirty.
pub fn step_house_animations_with_newgrf(
    map: &mut Map,
    tick: u64,
    rng: &mut Randomizer,
    active: &mut Vec<TileCoord>,
    towns: &mut [Town],
    house_catalog: &[HouseSpecDef],
    climate: Climate,
) -> Vec<TileCoord> {
    let mut ignored_sounds = Vec::new();
    step_house_animations_with_newgrf_and_sounds(
        map,
        tick,
        rng,
        active,
        towns,
        house_catalog,
        climate,
        &mut ignored_sounds,
    )
}

/// Variante que devuelve los sonidos ambientales solicitados por CB1A.
///
/// El caller de `GameState` valida cada sample contra el catálogo antes de
/// encolarlo; conservar la solicitud separada mantiene este módulo libre de
/// estado global y de dependencias de Bevy.
#[allow(clippy::too_many_arguments)]
pub fn step_house_animations_with_newgrf_and_sounds(
    map: &mut Map,
    tick: u64,
    rng: &mut Randomizer,
    active: &mut Vec<TileCoord>,
    towns: &mut [Town],
    house_catalog: &[HouseSpecDef],
    climate: Climate,
    sound_events: &mut Vec<crate::NewgrfTileSound>,
) -> Vec<TileCoord> {
    let mut context = NewgrfHouseAnimationContext {
        towns,
        house_catalog,
        climate,
    };
    step_house_animations_with_newgrf_stepper(
        map,
        tick,
        rng,
        active,
        sound_events,
        |map, coord, tile, tick, rng| {
            step_newgrf_house_animation(map, &mut context, coord, tile, tick, rng)
        },
    )
}

/// Alias de compatibilidad para la antigua API exclusiva de ascensores.
pub fn step_house_lifts(
    map: &mut Map,
    tick: u64,
    rng: &mut Randomizer,
    active: &mut Vec<TileCoord>,
) -> Vec<TileCoord> {
    step_house_animations(map, tick, rng, active)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GameState;

    fn large_office() -> Tile {
        Tile::completed_house(4, 0, 0)
    }

    fn newgrf_animation_callback_entry(
        variable: u8,
        shift: u8,
        and_mask: u32,
    ) -> crate::newgrf_sprites::Action2VarEntry {
        crate::newgrf_sprites::Action2VarEntry {
            first: crate::newgrf_sprites::Action2VarTerm {
                variable,
                param: None,
                adjust: crate::newgrf_sprites::Action2VarAdjust {
                    shift,
                    and_mask,
                    ..Default::default()
                },
            },
            ops: Vec::new(),
            ranges: Vec::new(),
            default: 0,
        }
    }

    fn newgrf_animation_house(
        id: u16,
        runtime: crate::newgrf_sprites::TrainSpriteGraphics,
    ) -> HouseSpecDef {
        HouseSpecDef {
            id,
            local_id: 0,
            subst_id: 0,
            building_flags: crate::house_spec::BUILDING_FLAG_SIZE_1X1,
            min_year: 0,
            max_year: crate::house_spec::HOUSE_YEAR_MAX,
            population: 0,
            mail_generation: 0,
            availability: crate::house_spec::DEFAULT_HOUSE_AVAILABILITY,
            probability: crate::house_spec::DEFAULT_HOUSE_PROBABILITY,
            processing_time: 0,
            extra_flags: 0,
            animation_frames: 0,
            animation_status: 0xFF,
            animation_speed: 2,
            override_id: None,
            callback_mask: 0,
            name: "animation-house".into(),
            from_newgrf: true,
            grfid: 1,
            newgrf_views: Vec::new(),
            newgrf_local_id: 0,
            newgrf_runtime: Some(Box::new(runtime)),
        }
    }

    fn cb20_and_cb1a_runtime() -> crate::newgrf_sprites::TrainSpriteGraphics {
        let mut runtime = crate::newgrf_sprites::TrainSpriteGraphics::default();
        runtime
            .assigns
            .push(crate::newgrf_sprites::TrainSpriteAssign {
                local_id: 0,
                set_id: 0,
            });
        runtime.action2_var.insert(
            0,
            crate::newgrf_sprites::Action2VarEntry {
                first: crate::newgrf_sprites::Action2VarTerm {
                    variable: 0x0C,
                    param: None,
                    adjust: crate::newgrf_sprites::Action2VarAdjust {
                        and_mask: u32::from(u16::MAX),
                        ..Default::default()
                    },
                },
                ops: Vec::new(),
                ranges: vec![
                    (
                        1,
                        u32::from(CBID_HOUSE_ANIMATION_SPEED),
                        u32::from(CBID_HOUSE_ANIMATION_SPEED),
                    ),
                    (
                        2,
                        u32::from(CBID_HOUSE_ANIMATION_NEXT_FRAME),
                        u32::from(CBID_HOUSE_ANIMATION_NEXT_FRAME),
                    ),
                ],
                default: 0,
            },
        );
        // CB20 devuelve 2, por lo que CB1A sólo llega cada cuatro ticks.
        runtime
            .action2_var
            .insert(1, newgrf_animation_callback_entry(0x0C, 4, 0xFF));
        // CB1A devuelve el byte bajo de `param1`, suministrado por Random().
        runtime
            .action2_var
            .insert(2, newgrf_animation_callback_entry(0x10, 0, 0xFF));
        runtime
    }

    #[test]
    fn lift_moves_up_one_position_towards_destination() {
        let mut tile = with_lift_destination(large_office(), 2);
        assert_eq!(advance_house_lift(&mut tile), LiftStep::Moving);
        assert_eq!(lift_position(tile), 1);
        assert_eq!(lift_destination(tile), 2);
    }

    #[test]
    fn lift_moves_down_one_position_towards_destination() {
        let mut tile = with_lift_position(large_office(), 18);
        tile = with_lift_destination(tile, 0);
        assert_eq!(advance_house_lift(&mut tile), LiftStep::Moving);
        assert_eq!(lift_position(tile), 17);
    }

    #[test]
    fn lift_halts_exactly_at_destination() {
        let mut tile = with_lift_position(large_office(), 11);
        tile = with_lift_destination(tile, 2);
        assert_eq!(advance_house_lift(&mut tile), LiftStep::Arrived);
        assert_eq!(lift_position(tile), 12);
        assert!(!lift_has_destination(tile));
        assert_eq!(advance_house_lift(&mut tile), LiftStep::Idle);
    }

    #[test]
    fn active_lift_chooses_destination_with_native_range_and_insertion_order() {
        let coord = TileCoord::new(2, 2);
        let mut map = Map::new_flat(8, 8, 0);
        map.set_tile(coord, large_office()).expect("office");
        let mut active = Vec::new();
        assert!(activate_house_lift_animation(&mut map, &mut active, coord));
        let mut rng = Randomizer::new(1);
        let mut expected = rng;
        // El primer resultado es piso 0, inválido porque ya está allí; el
        // segundo es 6. El bucle debe consumir ambas palabras.
        assert_eq!(expected.random_range(7), 0);
        assert_eq!(expected.random_range(7), 6);

        let dirty = step_house_lifts(&mut map, 4, &mut rng, &mut active);

        let tile = map.get(coord).expect("office after animation");
        assert_eq!(dirty, vec![coord]);
        assert_eq!(rng, expected);
        assert_eq!(lift_destination(tile), 6);
        assert_eq!(lift_position(tile), 1);
        assert_eq!(active, vec![coord]);
    }

    #[test]
    fn arrived_lift_stays_deleted_until_the_next_animation_pass() {
        let first = TileCoord::new(2, 2);
        let second = TileCoord::new(5, 5);
        let mut map = Map::new_flat(8, 8, 0);
        let first_tile = with_lift_destination(with_lift_position(large_office(), 5), 1);
        let second_tile = with_lift_destination(with_lift_position(large_office(), 0), 2);
        map.set_tile(first, first_tile).expect("first office");
        map.set_tile(second, second_tile).expect("second office");
        let mut active = Vec::new();
        assert!(activate_house_lift_animation(&mut map, &mut active, first));
        assert!(activate_house_lift_animation(&mut map, &mut active, second));
        let mut rng = Randomizer::new(7);

        let dirty = step_house_lifts(&mut map, 4, &mut rng, &mut active);

        assert_eq!(dirty, vec![first, second]);
        assert_eq!(active, vec![first, second]);
        let first_after = map.get(first).expect("first after arrival");
        assert_eq!(
            house_animation_state(first_after),
            HOUSE_ANIMATION_STATE_DELETED
        );
        assert!(!lift_has_destination(first_after));
        assert_eq!(lift_position(first_after), 6);
        assert_eq!(
            house_animation_state(map.get(second).expect("second still active")),
            HOUSE_ANIMATION_STATE_ACTIVE
        );

        // `AnimateAnimatedTiles` limpia `Deleted` aun cuando
        // `AnimateTile_Town` retorna por `counter & 3`.
        let dirty = step_house_lifts(&mut map, 5, &mut rng, &mut active);
        assert!(dirty.is_empty());
        assert_eq!(active, vec![second]);
        assert_eq!(
            house_animation_state(map.get(first).expect("first cleanup")),
            HOUSE_ANIMATION_STATE_NONE
        );
    }

    #[test]
    fn reactivating_deleted_lift_preserves_its_anit_slot() {
        let first = TileCoord::new(2, 2);
        let second = TileCoord::new(5, 5);
        let mut map = Map::new_flat(8, 8, 0);
        map.set_tile(first, large_office()).expect("first office");
        map.set_tile(second, large_office()).expect("second office");
        let mut active = Vec::new();
        assert!(activate_house_lift_animation(&mut map, &mut active, first));
        assert!(activate_house_lift_animation(&mut map, &mut active, second));

        let mut first_tile = map.get(first).expect("first active");
        first_tile = with_house_animation_state(first_tile, HOUSE_ANIMATION_STATE_DELETED);
        map.set_tile(first, first_tile).expect("mark first deleted");

        assert!(activate_house_lift_animation(&mut map, &mut active, first));
        assert_eq!(active, vec![first, second]);
        assert_eq!(
            house_animation_state(map.get(first).expect("first reactivated")),
            HOUSE_ANIMATION_STATE_ACTIVE
        );
        assert!(
            !activate_house_lift_animation(&mut map, &mut active, first),
            "AddAnimatedTile no duplica una entrada ya Animated"
        );
        assert_eq!(active, vec![first, second]);

        // La próxima pasada debe asignar la primera palabra RNG al slot que
        // llegó antes, no al que quedaría primero tras un `swap_remove`
        // prematuro seguido de `push`.
        let mut rng = Randomizer::new(1);
        let mut expected = rng;
        let expected_first = choose_lift_destination(0, &mut expected);
        let expected_second = choose_lift_destination(0, &mut expected);
        step_house_lifts(&mut map, 4, &mut rng, &mut active);
        assert_eq!(
            lift_destination(map.get(first).expect("first after animate")),
            expected_first
        );
        assert_eq!(
            lift_destination(map.get(second).expect("second after animate")),
            expected_second
        );
        assert_eq!(rng, expected);
    }

    #[test]
    fn adding_lift_animation_is_stable_and_deduplicated() {
        let first = TileCoord::new(3, 5);
        let second = TileCoord::new(1, 7);
        let mut active = vec![first];
        add_house_animation_to_queue(&mut active, first);
        add_house_animation_to_queue(&mut active, second);
        assert_eq!(active, vec![first, second]);
    }

    #[test]
    fn newgrf_house_entry_keeps_its_shared_anit_slot_until_cb1a_is_connected() {
        let coord = TileCoord::new(2, 2);
        let mut map = Map::new_flat(8, 8, 0);
        map.set_completed_house(coord, crate::house_spec::NEW_HOUSE_OFFSET, 0)
            .expect("NewGRF house");
        let mut tile = map.get(coord).expect("NewGRF house tile");
        tile.m6 = (tile.m6 & !HOUSE_ANIMATION_STATE_MASK) | HOUSE_ANIMATION_STATE_ACTIVE;
        map.set_tile(coord, tile).expect("active NewGRF house");
        let mut active = vec![coord];
        let mut rng = Randomizer::new(7);
        let expected = rng;

        let dirty = step_house_animations(&mut map, 4, &mut rng, &mut active);

        assert!(dirty.is_empty());
        assert_eq!(active, vec![coord]);
        assert_eq!(rng, expected);
        assert!(map.get(coord).is_some_and(house_tile_has_newgrf_animation));
    }

    #[test]
    fn newgrf_cb20_gates_cb1a_and_passes_declared_random_bits() {
        let id = NEW_HOUSE_OFFSET;
        let coord = TileCoord::new(2, 2);
        let mut map = Map::new_flat(8, 8, 0);
        map.set_completed_house(coord, id, 0)
            .expect("NewGRF house inside map");
        let mut active = Vec::new();
        assert!(activate_newgrf_house_animation(
            &mut map,
            &mut active,
            coord
        ));

        let mut def = newgrf_animation_house(id, cb20_and_cb1a_runtime());
        def.animation_frames = 8;
        def.animation_status = 1;
        // Si CB20 no se evaluara, este valor impediría CB1A hasta el tick 2^16.
        def.animation_speed = 16;
        def.extra_flags = crate::house_spec::HOUSE_EXTRA_FLAG_CALLBACK_1A_RANDOM_BITS;
        def.callback_mask = crate::house_spec::HOUSE_CALLBACK_ANIMATION_NEXT_FRAME_MASK
            | crate::house_spec::HOUSE_CALLBACK_ANIMATION_SPEED_MASK;
        let catalog = vec![def];
        let mut towns = Vec::new();
        let mut rng = Randomizer::new(42);

        let before_cb1a = rng;
        let dirty = step_house_animations_with_newgrf(
            &mut map,
            1,
            &mut rng,
            &mut active,
            &mut towns,
            &catalog,
            Climate::Temperate,
        );
        assert!(dirty.is_empty(), "CB20 corre pero la cadencia aún no vence");
        assert_eq!(rng, before_cb1a, "CB20 nunca consume Random()");
        assert_eq!(map.get(coord).expect("house after tick 1").m7, 0);

        let mut expected = rng;
        let expected_frame = u8::try_from(expected.next() & 0xFF).unwrap_or(0);
        assert_ne!(expected_frame, 0, "seed de regresión hace visible CB1A");
        let dirty = step_house_animations_with_newgrf(
            &mut map,
            4,
            &mut rng,
            &mut active,
            &mut towns,
            &catalog,
            Climate::Temperate,
        );
        assert_eq!(rng, expected, "CB1A toma exactamente una palabra RNG");
        assert_eq!(dirty, vec![coord]);
        assert_eq!(map.get(coord).expect("house after CB1A").m7, expected_frame);
        assert_eq!(active, vec![coord]);
    }

    #[test]
    fn newgrf_cb1a_emits_its_ambient_tile_sound() {
        let id = NEW_HOUSE_OFFSET;
        let coord = TileCoord::new(2, 2);
        let mut map = Map::new_flat(8, 8, 0);
        map.set_completed_house(coord, id, 0)
            .expect("NewGRF house inside map");
        let mut active = Vec::new();
        assert!(activate_newgrf_house_animation(
            &mut map,
            &mut active,
            coord
        ));

        let mut runtime = cb20_and_cb1a_runtime();
        runtime
            .action2_var
            .get_mut(&2)
            .expect("CB1A callback set")
            .first
            .adjust
            .add_val = Some(0x3100);
        let mut def = newgrf_animation_house(id, runtime);
        def.animation_frames = 8;
        def.animation_status = 1;
        def.animation_speed = 16;
        def.extra_flags = crate::house_spec::HOUSE_EXTRA_FLAG_CALLBACK_1A_RANDOM_BITS;
        def.callback_mask = crate::house_spec::HOUSE_CALLBACK_ANIMATION_NEXT_FRAME_MASK
            | crate::house_spec::HOUSE_CALLBACK_ANIMATION_SPEED_MASK;
        let catalog = vec![def];
        let mut towns = Vec::new();
        let mut rng = Randomizer::new(42);
        let mut sounds = Vec::new();

        let _ = step_house_animations_with_newgrf_and_sounds(
            &mut map,
            1,
            &mut rng,
            &mut active,
            &mut towns,
            &catalog,
            Climate::Temperate,
            &mut sounds,
        );
        assert!(sounds.is_empty(), "CB20 no reproduce sonido de animación");

        let _ = step_house_animations_with_newgrf_and_sounds(
            &mut map,
            4,
            &mut rng,
            &mut active,
            &mut towns,
            &catalog,
            Climate::Temperate,
            &mut sounds,
        );

        assert_eq!(
            sounds,
            vec![crate::NewgrfTileSound {
                grfid: 1,
                local_id: 0x31,
                at: coord,
            }]
        );
    }

    #[test]
    fn newgrf_default_non_loop_animation_marks_deleted_on_the_next_pass() {
        let id = NEW_HOUSE_OFFSET;
        let coord = TileCoord::new(2, 2);
        let mut map = Map::new_flat(8, 8, 0);
        map.set_completed_house(coord, id, 0)
            .expect("NewGRF house inside map");
        let mut tile = map.get(coord).expect("NewGRF house");
        tile.m7 = 1;
        map.set_tile(coord, tile).expect("set final frame");
        let mut active = Vec::new();
        assert!(activate_newgrf_house_animation(
            &mut map,
            &mut active,
            coord
        ));

        let mut def =
            newgrf_animation_house(id, crate::newgrf_sprites::TrainSpriteGraphics::default());
        def.animation_frames = 1;
        def.animation_status = 0;
        def.animation_speed = 0;
        let catalog = vec![def];
        let mut towns = Vec::new();
        let mut rng = Randomizer::new(7);
        let expected = rng;

        let dirty = step_house_animations_with_newgrf(
            &mut map,
            0,
            &mut rng,
            &mut active,
            &mut towns,
            &catalog,
            Climate::Temperate,
        );
        assert!(dirty.is_empty());
        assert_eq!(rng, expected);
        assert_eq!(
            active,
            vec![coord],
            "DeleteAnimatedTile no hace swap_remove"
        );
        assert_eq!(
            house_animation_state(map.get(coord).expect("deleted house")),
            HOUSE_ANIMATION_STATE_DELETED
        );

        let dirty = step_house_animations_with_newgrf(
            &mut map,
            1,
            &mut rng,
            &mut active,
            &mut towns,
            &catalog,
            Climate::Temperate,
        );
        assert!(dirty.is_empty());
        assert!(active.is_empty());
        assert_eq!(
            house_animation_state(map.get(coord).expect("cleaned house")),
            HOUSE_ANIMATION_STATE_NONE
        );
    }

    #[test]
    fn newgrf_callback_result_uses_add_and_delete_animated_tile_semantics() {
        let coord = TileCoord::new(2, 2);
        let mut map = Map::new_flat(8, 8, 0);
        map.set_completed_house(coord, crate::house_spec::NEW_HOUSE_OFFSET, 0)
            .expect("NewGRF house");
        let mut active = Vec::new();

        assert!(apply_newgrf_house_animation_callback_result(
            &mut map,
            &mut active,
            coord,
            4,
        ));
        assert_eq!(active, vec![coord]);
        assert_eq!(map.get(coord).expect("active house").m7, 4);
        assert_eq!(
            house_animation_state(map.get(coord).expect("active house")),
            HOUSE_ANIMATION_STATE_ACTIVE
        );

        assert!(!apply_newgrf_house_animation_callback_result(
            &mut map,
            &mut active,
            coord,
            0xFF,
        ));
        assert_eq!(active, vec![coord], "DeleteAnimatedTile waits one pass");
        assert_eq!(
            house_animation_state(map.get(coord).expect("deleted house")),
            HOUSE_ANIMATION_STATE_DELETED
        );

        assert!(!apply_newgrf_house_animation_callback_result(
            &mut map,
            &mut active,
            coord,
            0xFE,
        ));
        assert_eq!(active, vec![coord], "reactivation preserves the ANIT slot");
        assert_eq!(
            house_animation_state(map.get(coord).expect("reactivated house")),
            HOUSE_ANIMATION_STATE_ACTIVE
        );
        assert!(!apply_newgrf_house_animation_callback_result(
            &mut map,
            &mut active,
            coord,
            0xFD,
        ));
        assert!(!apply_newgrf_house_animation_callback_result(
            &mut map,
            &mut active,
            coord,
            u16::MAX,
        ));
        assert_eq!(map.get(coord).expect("unchanged house").m7, 4);
        assert_eq!(active, vec![coord]);
    }

    fn lift_game(order: &[TileCoord]) -> GameState {
        let mut state = GameState::new(8, 8);
        for &coord in order {
            state
                .map
                .set_tile(coord, large_office())
                .expect("office inside map");
            assert!(activate_house_lift_animation(
                &mut state.map,
                &mut state.active_house_animations,
                coord,
            ));
        }
        assert_eq!(state.active_house_animations, order);
        state.random = Randomizer::new(1);
        state
    }

    fn assert_lift_progress_matches(
        control: &GameState,
        resumed: &GameState,
        coords: &[TileCoord],
    ) {
        assert_eq!(
            control.active_house_animations,
            resumed.active_house_animations
        );
        assert_eq!(control.random, resumed.random);
        assert_eq!(control.canonical_hash(), resumed.canonical_hash());
        for &coord in coords {
            let control_tile = control.map.get(coord).expect("control office");
            let resumed_tile = resumed.map.get(coord).expect("resumed office");
            assert_eq!(control_tile.m6, resumed_tile.m6, "MAP6 at {coord:?}");
            assert_eq!(control_tile.m7, resumed_tile.m7, "MAP7 at {coord:?}");
        }
    }

    fn assert_save_resume_lift_trajectory(save_after_destination_assignment: bool) {
        let first = TileCoord::new(2, 2);
        let second = TileCoord::new(5, 5);
        let coords = [first, second];
        let mut control = lift_game(&[second, first]);
        let mut subject = lift_game(&[second, first]);

        if save_after_destination_assignment {
            control.step();
            subject.step();
            assert_lift_progress_matches(&control, &subject, &coords);
            assert!(
                coords
                    .iter()
                    .all(|&coord| { control.map.get(coord).is_some_and(lift_has_destination) }),
                "el snapshot de mitad de animación tiene destinos ya asignados"
            );
        }

        let saved = subject.save_json().expect("save JSON");
        let mut resumed = GameState::load_json(&saved).expect("load JSON");
        assert_eq!(resumed.active_house_animations, vec![second, first]);

        for _ in 0..32 {
            control.step();
            resumed.step();
            assert_lift_progress_matches(&control, &resumed, &coords);
        }
    }

    #[test]
    fn pending_house_lift_queue_survives_json_save_resume_tick_by_tick() {
        assert_save_resume_lift_trajectory(false);
    }

    #[test]
    fn assigned_house_lift_queue_survives_json_save_resume_tick_by_tick() {
        assert_save_resume_lift_trajectory(true);
    }

    #[test]
    fn inverse_house_lift_queue_order_is_persisted_and_changes_the_hash() {
        let first = TileCoord::new(2, 2);
        let second = TileCoord::new(5, 5);
        let forward = lift_game(&[first, second]);
        let reverse = lift_game(&[second, first]);
        assert_ne!(forward.canonical_hash(), reverse.canonical_hash());

        let loaded =
            GameState::load_json(&reverse.save_json().expect("save JSON")).expect("load JSON");
        assert_eq!(loaded.active_house_animations, vec![second, first]);
    }

    #[test]
    fn legacy_json_without_house_animation_queue_keeps_rng_and_uses_an_empty_queue() {
        let state = lift_game(&[TileCoord::new(2, 2), TileCoord::new(5, 5)]);
        let random_before_load = state.random;
        let mut legacy = serde_json::to_value(&state).expect("serialize legacy fixture");
        legacy
            .as_object_mut()
            .expect("GameState serializes to an object")
            .remove("active_house_animations");

        let loaded =
            GameState::load_json(&serde_json::to_string(&legacy).expect("encode legacy fixture"))
                .expect("load JSON without persistent queue");
        assert!(loaded.active_house_animations.is_empty());
        assert_eq!(loaded.random, random_before_load);
    }

    #[test]
    fn legacy_lift_queue_json_alias_preserves_order() {
        let state = lift_game(&[TileCoord::new(2, 2), TileCoord::new(5, 5)]);
        let mut legacy = serde_json::to_value(&state).expect("serialize legacy fixture");
        let legacy_object = legacy
            .as_object_mut()
            .expect("GameState serializes to an object");
        let queue = legacy_object
            .remove("active_house_animations")
            .expect("current queue field");
        legacy_object.insert("active_house_lifts".to_owned(), queue);

        let loaded =
            GameState::load_json(&serde_json::to_string(&legacy).expect("encode legacy fixture"))
                .expect("load JSON with former queue field");
        assert_eq!(
            loaded.active_house_animations,
            state.active_house_animations
        );
    }
}
