//! Ascensor de Large Office (`AnimateTile_Town` / `town_map.h`).

use crate::cargodist::parity::Randomizer;
use crate::house_spec::{BUILDING_FLAG_IS_ANIMATED, HouseSpec};

use super::{Map, Tile, TileCoord, TileKind};

pub const LIFT_MAX_POSITION: u8 = 36;
const LIFT_DESTINATION_FLOORS: u8 = 7;
const LIFT_STEPS_PER_FLOOR: u8 = 6;

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

/// Equivalente urbano de `AddAnimatedTile`.
///
/// La lista conserva el orden de inserción de OpenTTD. No usar un `HashSet`:
/// cuando dos ascensores toman destinos en la misma pasada, intercambiarlos
/// puede alterar qué posición recibe cada palabra del stream global.
pub fn add_house_lift_to_animation(active: &mut Vec<TileCoord>, coord: TileCoord) {
    if !active.contains(&coord) {
        active.push(coord);
    }
}

/// Ejecuta el subconjunto de ascensores de `AnimateAnimatedTiles`.
///
/// `TileLoop_Town` ya hizo el `Chance16(1, 2)` que añade un ascensor a la
/// lista. Aquí no se vuelve a sortear esa decisión: una entrada activa que no
/// tenga destino toma `RandomRange(7)` únicamente cuando el contador global es
/// múltiplo de cuatro, igual que `AnimateTile_Town`.
pub fn step_house_lifts(
    map: &mut Map,
    tick: u64,
    rng: &mut Randomizer,
    active: &mut Vec<TileCoord>,
) -> Vec<TileCoord> {
    let mut dirty = Vec::new();
    // `AnimateTile_Town` retorna antes de validar el tipo en tres de cada
    // cuatro ticks, por lo que una entrada vieja sobrevive hasta la próxima
    // pasada divisible por cuatro.
    if tick & 3 != 0 {
        return dirty;
    }

    let mut index = 0;
    while index < active.len() {
        let coord = active[index];
        let Some(mut tile) = map.get(coord) else {
            // `AnimateAnimatedTiles` elimina con el último elemento, no
            // preservando el orden del resto del vector.
            active.swap_remove(index);
            continue;
        };
        if !house_tile_has_lift(tile) {
            active.swap_remove(index);
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
            // El original marca la entrada para borrar; quitarla ahora evita
            // una segunda animación local y conserva la semántica observable
            // de MAP6/MAP7. Una reactivación posterior vuelve a agregarla al
            // final mediante `AddAnimatedTile`.
            active.swap_remove(index);
        } else {
            index += 1;
        }
    }
    dirty
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GameState;

    fn large_office() -> Tile {
        Tile::completed_house(4, 0, 0)
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
        let mut active = vec![coord];
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
    fn adding_lift_animation_is_stable_and_deduplicated() {
        let first = TileCoord::new(3, 5);
        let second = TileCoord::new(1, 7);
        let mut active = vec![first];
        add_house_lift_to_animation(&mut active, first);
        add_house_lift_to_animation(&mut active, second);
        assert_eq!(active, vec![first, second]);
    }

    fn lift_game(order: Vec<TileCoord>) -> GameState {
        let mut state = GameState::new(8, 8);
        for &coord in &order {
            state
                .map
                .set_tile(coord, large_office())
                .expect("office inside map");
        }
        state.active_house_lifts = order;
        state.random = Randomizer::new(1);
        state
    }

    fn assert_lift_progress_matches(
        control: &GameState,
        resumed: &GameState,
        coords: &[TileCoord],
    ) {
        assert_eq!(control.active_house_lifts, resumed.active_house_lifts);
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
        let mut control = lift_game(vec![second, first]);
        let mut subject = lift_game(vec![second, first]);

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
        assert_eq!(resumed.active_house_lifts, vec![second, first]);

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
        let forward = lift_game(vec![first, second]);
        let reverse = lift_game(vec![second, first]);
        assert_ne!(forward.canonical_hash(), reverse.canonical_hash());

        let loaded =
            GameState::load_json(&reverse.save_json().expect("save JSON")).expect("load JSON");
        assert_eq!(loaded.active_house_lifts, vec![second, first]);
    }

    #[test]
    fn legacy_json_without_lift_queue_keeps_rng_and_uses_an_empty_queue() {
        let state = lift_game(vec![TileCoord::new(2, 2), TileCoord::new(5, 5)]);
        let random_before_load = state.random;
        let mut legacy = serde_json::to_value(&state).expect("serialize legacy fixture");
        legacy
            .as_object_mut()
            .expect("GameState serializes to an object")
            .remove("active_house_lifts");

        let loaded =
            GameState::load_json(&serde_json::to_string(&legacy).expect("encode legacy fixture"))
                .expect("load JSON without persistent queue");
        assert!(loaded.active_house_lifts.is_empty());
        assert_eq!(loaded.random, random_before_load);
    }
}
