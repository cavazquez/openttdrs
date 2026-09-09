//! Ciclo de teselas usado durante el arranque de un mundo nuevo.
//!
//! `GenerateWorld` no ejecuta un tick económico completo después de crear el
//! mapa. `OpenTTD` ejecuta 0x500 pasadas de `RunTileLoop` y sólo despacha el
//! callback de la tesela visitada (`landscape.cpp` / `genworld.cpp`). Mantener
//! esta ruta separada de [`GameState::step`](crate::GameState::step) evita que
//! el diagnóstico de mapas aleatorios adelante calendarios, vehículos o
//! producción antes de tiempo.

use crate::GameState;
use crate::cargodist::parity::Randomizer;
use crate::house_spec::{
    BUILDING_FLAG_IS_CHURCH, BUILDING_FLAG_IS_STADIUM, BUILDING_FLAG_SIZE_1X1,
    BUILDING_FLAG_SIZE_1X2, BUILDING_FLAG_SIZE_2X1, BUILDING_FLAG_SIZE_2X2, HouseSpec,
    get_town_radius_group,
};
use crate::map::{
    Map, Tile, TileCoord, TileKind, TileLoopState, collect_tile_loop_visits, coord_to_linear_index,
    industry_instance_id,
};

/// Cantidad de pasadas que `CreateRivers` ejecuta tras ensanchar ríos.
///
/// Es `TILE_UPDATE_FREQUENCY` en `landscape.cpp`; en ese intervalo cada
/// tesela recibe una visita LFSR y el agua estable queda marcada como
/// `non-flooding` en `MAP3` bit 0.
pub const LANDSCAPE_RIVER_TILE_LOOP_PASSES: u64 = 256;

/// Pasadas de `RunTileLoop` que `GenerateWorld` ejecuta después de crear el
/// mapa y las entidades iniciales (`genworld.cpp`).
pub const STARTUP_TILE_LOOP_PASSES: u64 = 0x500;

/// Ejecuta una pasada de `RunTileLoop` para la generación de un mundo nuevo.
///
/// `tick` es el contador de `TimerGameTick` usado por los callbacks de
/// paisaje. La función no avanza `GameState::tick`: durante la generación de
/// `OpenTTD` ese contador es independiente del calendario de la partida. El
/// estado LFSR persistente sí se actualiza en cada llamada.
///
/// El retorno es el número de teselas visitadas (útil para telemetría y
/// pruebas de la secuencia LFSR).
pub fn run_generation_tile_loop(state: &mut GameState, tick: u64) -> usize {
    run_generation_tile_loop_impl(state, tick, None)
}

/// Ejecuta la cola de tile loops de una partida nueva con el RNG global de
/// generación.
///
/// `OpenTTD` llama a `RunTileLoop`, incrementa `TimerGameTick::counter` y
/// repite la operación `0x500` veces antes de entregar el mundo al jugador.
/// El parámetro `passes` permite a los oráculos y tests aislar una prefija de
/// la cola; los consumidores de Nueva partida deben usar
/// [`STARTUP_TILE_LOOP_PASSES`]. El contador económico de [`GameState`] no se
/// modifica.
pub fn run_generation_tile_loops_with_rng(
    state: &mut GameState,
    rng: &mut Randomizer,
    passes: u64,
) -> usize {
    let mut visited = 0usize;
    for tick in 0..passes {
        visited =
            visited.saturating_add(run_generation_tile_loop_impl(state, tick, Some(&mut *rng)));
    }
    visited
}

/// Ejecuta la transición que `OpenTTD` hace al entregar un mundo nuevo al
/// `StateGameLoop`.
///
/// `GenerateWorld` termina con `TimerGameTick::counter == 0x500`. En el primer
/// tick regular se llama primero a `AnimateAnimatedTiles` con ese contador y,
/// después de incrementarlo, a `RunTileLoop` con `0x501`. La diferencia es
/// observable en `MAP6/MAP7` de industrias animadas y en los campos visitados
/// por el LFSR; omitirla deja al generador un tick detrás del mapa que exporta
/// el oráculo de `OpenTTD`.
pub fn run_first_regular_game_tick_with_rng(
    state: &mut GameState,
    rng: &mut Randomizer,
    startup_tick: u64,
) -> usize {
    let (width, height) = state.map.dimensions();
    let animated_industries: Vec<TileCoord> = (0..height)
        .flat_map(|y| (0..width).map(move |x| TileCoord::new(x.cast_signed(), y.cast_signed())))
        .filter(|&coord| {
            state
                .map
                .get(coord)
                .is_some_and(|tile| tile.kind == TileKind::Industry && tile.m6 & 0x03 != 0)
        })
        .collect();
    let _ = crate::map::industry_tile_anim::advance_startup_animated_industry_tiles(
        &mut state.map,
        startup_tick,
        &animated_industries,
        rng,
    );
    let visited = run_generation_tile_loop_impl(state, startup_tick.saturating_add(1), Some(rng));
    // `StateGameLoop` continúa con `CallLandscapeTick`; en una partida nueva
    // el primer callback con efecto observable suele ser `OnTick_Trees`, que
    // puede consumir el RNG global y convertir una tesela clear en árboles.
    // Reproducirlo aquí mantiene el raw exportado después del tick alineado
    // con OpenTTD, sin adelantar economía ni calendario.
    let _ = super::trees::advance_first_regular_tree_tick(
        &mut state.map,
        state.climate,
        startup_tick.saturating_add(1),
        rng,
    );
    visited
}

/// Variante interna de [`run_generation_tile_loop`] para la cola de
/// `CreateRivers`. Sólo esa frontera tiene que conservar el stream global de
/// `Random()` mientras los árboles de humedal ejecutan `TileLoop_Trees`.
fn run_generation_tile_loop_impl(
    state: &mut GameState,
    tick: u64,
    mut generation_rng: Option<&mut Randomizer>,
) -> usize {
    let visits = collect_tile_loop_visits(&state.map, tick, &mut state.cur_tileloop_tile);
    let visit_count = visits.len();

    // El tile loop de OpenTTD despacha cada tesela en el orden LFSR. Releer el
    // tile desde el mapa antes de cada callback conserva mutaciones producidas
    // por una tesela anterior de la misma pasada (en especial inundaciones).
    for (coord, snapshot) in &visits {
        let mut tile = state.map.get(*coord).unwrap_or(*snapshot);
        // `TileLoop_Clear` despacha la transición de clima antes del
        // crecimiento genérico. Releer la tesela conserva la mutación de
        // MAP3/MAP5 y decide si la segunda parte del callback debe ejecutarse.
        match state.climate {
            crate::world_gen::Climate::SubTropical => {
                crate::map::tree_tile_loop::tile_loop_clear_desert(
                    &mut state.map,
                    *coord,
                    state.climate,
                    state.world_seed,
                );
            }
            crate::world_gen::Climate::SubArctic => {
                crate::map::tree_tile_loop::tile_loop_clear_alps_at(
                    &mut state.map,
                    *coord,
                    state.snow_line_height,
                );
                if tile.kind == TileKind::Forest
                    && let Some(rng) = generation_rng.as_deref_mut()
                {
                    crate::map::tree_tile_loop::tile_loop_trees_alps_at(
                        &mut state.map,
                        *coord,
                        state.snow_line_height,
                        rng,
                    );
                }
            }
            _ => {}
        }
        tile = state.map.get(*coord).unwrap_or(tile);
        dispatch_generation_tile_loop_tile(state, tick, *coord, tile, &mut generation_rng);
    }

    // El último conjunto queda disponible para herramientas de diagnóstico,
    // igual que después de `phase_tile_loop` en la simulación normal.
    state.runtime.tile_loop_visited = visits;
    visit_count
}

#[allow(clippy::too_many_lines)] // Agrupa el orden observable de una visita de generación.
fn dispatch_generation_tile_loop_tile(
    state: &mut GameState,
    tick: u64,
    coord: TileCoord,
    tile: Tile,
    generation_rng: &mut Option<&mut Randomizer>,
) {
    match tile.kind {
        // TileLoop_Clear y TileLoop_Trees comparten el crecimiento de
        // hierba/campos y la actualización de árboles. El helper ya vuelve a
        // leer la tesela viva después de una transición desértica.
        TileKind::Forest
            if matches!(
                state.climate,
                crate::world_gen::Climate::Temperate
                    | crate::world_gen::Climate::SubArctic
                    | crate::world_gen::Climate::SubTropical
                    | crate::world_gen::Climate::Toyland
            ) && generation_rng.is_some() =>
        {
            // `TileLoop_Trees` delega primero una orilla a `TileLoop_Water`.
            // El callback puede cambiar la tesela, por lo que el procesador
            // de árbol la vuelve a leer del mapa.
            if (tile.m2 >> 6) & 0x07 == 3 {
                crate::map::water_flood::tile_loop_water_at(state, coord, tile);
            }
            if let Some(rng) = generation_rng.as_deref_mut() {
                crate::map::tree_tile_loop::process_generation_tree_growth_at(
                    &mut state.map,
                    state.climate,
                    tick,
                    rng,
                    coord,
                );
            }
        }
        TileKind::Forest | TileKind::CoalField | TileKind::Grass => {
            if tile.kind == TileKind::Grass
                && crate::map::tree_tile_loop::clear_ground_type(tile.m5)
                    == crate::world_gen::CLEAR_GROUND_FIELDS
            {
                tile_loop_clear_field(state, coord, tile);
            } else {
                crate::map::tree_tile_loop::process_tree_and_field_growth_from_visits(
                    &mut state.map,
                    tick,
                    state.world_seed,
                    &[(coord, tile)],
                );
            }
        }
        // TileLoop_Water es el único callback aplicable a MP_WATER y no debe
        // tratar los bordes MP_VOID como tiles válidos al inundar.
        TileKind::Water => crate::map::water_flood::tile_loop_water_at(state, coord, tile),
        // TileLoop_Industry primero deja que una industria sobre agua intente
        // inundar, luego ejecuta randomización, obra y animación. Las tres
        // operaciones consumen exactamente la visita actual y no barren de
        // nuevo el mapa.
        TileKind::Industry => {
            if crate::map::industry_terrain::industry_tile_on_water(tile) {
                crate::map::water_flood::tile_loop_water_at(state, coord, tile);
            }
            let Some(live_tile) = state.map.get(coord) else {
                return;
            };
            if live_tile.kind != TileKind::Industry {
                return;
            }
            let one = [(coord, live_tile)];
            let _ = crate::map::industry_random::
                advance_industry_tile_randomisation_from_visits_with_catalog_and_cargo_catalog(
                    &mut state.map,
                    tick,
                    state.world_seed,
                    &one,
                    &state.industries,
                    &state.towns,
                    &state.industry_tile_spec_catalog,
                    &state.industry_spec_catalog,
                    state.climate,
                    &state.cargo_spec_catalog,
                );
            // `MakeIndustryTileBigger` muta sólo la tesela visitada. La
            // simulación económica puede sincronizar un footprint, pero la
            // cola de generación debe conservar el desfase LFSR de cada parte.
            let was_completed = live_tile.m1 & 0x80 != 0;
            let construction_rollover =
                crate::map::industry_construction::industry_construction_counter(live_tile.m1) == 3;
            let _ = crate::map::industry_construction::advance_industry_construction_tile_loop_at(
                &mut state.map,
                coord,
            );
            // Aunque el grupo de animación vanilla no tenga callbacks, la
            // expresión C++ `TriggerIndustryTileAnimation_ConstructionStageChanged`
            // evalúa `Random()` antes de comprobar la máscara. Esa extracción
            // ocurre en cada cuarta visita (incluida la finalización).
            if construction_rollover && let Some(rng) = generation_rng.as_deref_mut() {
                let _ = rng.next();
            }
            // Completing a vanilla power-plant chimney creates a smoke effect
            // immediately. `ChimneySmokeInit` takes one word from the same
            // global RNG for its initial sprite/progress, even though the
            // effect is not serialized in the world-raw map.
            let completed_now = !was_completed
                && state
                    .map
                    .get(coord)
                    .is_some_and(|updated| updated.m1 & 0x80 != 0);
            if completed_now
                && state.map.get(coord).is_some_and(|updated| {
                    crate::map::industry_tile_anim::industry_gfx(&updated) == 8
                })
                && let Some(rng) = generation_rng.as_deref_mut()
            {
                let _chimney_smoke_seed = rng.next();
            }
            // OpenTTD retorna inmediatamente después de `MakeIndustryTileBigger`;
            // una tesela que termina su obra en esta visita no puede animarse
            // hasta una franja posterior.
            if !was_completed {
                return;
            }
            let one = [(coord, state.map.get(coord).unwrap_or(live_tile))];
            if let Some(rng) = generation_rng.as_deref_mut() {
                let _ = crate::map::industry_tile_anim::
                    advance_industry_tile_loop_events_from_visits_with_rng(
                        &mut state.map,
                        tick,
                        &one,
                        rng,
                    );
            } else {
                let _ =
                    crate::map::industry_tile_anim::advance_industry_tile_loop_events_from_visits(
                        &mut state.map,
                        tick,
                        &one,
                    );
            }
        }
        TileKind::Road => tile_loop_road(state, coord, tile),
        // `TileLoop_Town` avanza la construcción y consume el mismo stream
        // global cuando una casa terminada produce pasajeros/correo. La
        // economía que actualiza estaciones queda fuera de esta cola: sólo
        // se reproducen aquí los bytes de MAP3/MAP5 y las extracciones de
        // `Random()` que pueden desplazar los callbacks siguientes.
        TileKind::House => tile_loop_house(state, tick, coord, tile, generation_rng),
        // Vías, estaciones, objetos y depósitos tienen callbacks de tile loop
        // propios en OpenTTD. Para un mundo nuevo sus rutas de estado todavía
        // no tienen una mutación equivalente; dejar explícito el no-op evita
        // ejecutar lógica de economía del tick normal.
        TileKind::Rail
        | TileKind::RoadDepot
        | TileKind::RailDepot
        | TileKind::ShipDepot
        | TileKind::Airport
        | TileKind::RoadTunnel
        | TileKind::RailTunnel
        | TileKind::RoadBridge
        | TileKind::RailBridge
        | TileKind::Station
        | TileKind::Void
        | TileKind::Unknown(_) => {}
    }
}

/// Ejecuta la parte de `TileLoop_Town` observable durante la cola de arranque.
///
/// `OpenTTD` guarda el contador de construcción en los cinco bits bajos de
/// `MAP5`: los tres inferiores son el contador y los bits 3–4 la etapa. Al
/// terminar una obra escribe `MAP3` bit 7 y reinicia la edad (`MAP5 = 0`). Una
/// casa multitesela se identifica por las flags de la tesela norte y avanza
/// todas sus subteselas en una sola visita; las subteselas restantes tienen
/// flags vacías y no vuelven a avanzar el conjunto.
fn tile_loop_house(
    state: &mut GameState,
    tick: u64,
    coord: TileCoord,
    tile: Tile,
    generation_rng: &mut Option<&mut Randomizer>,
) {
    let house_id = tile.m8 & 0x0FFF;
    let Some(_house) =
        crate::house_spec::vanilla_or_newgrf_house(&state.house_spec_catalog, house_id)
    else {
        return;
    };
    // `NewHouseTileLoop` se ejecuta antes de la construcción. Mientras
    // `processing_time` sea positivo sólo lo decrementa y marca la tesela
    // dirty: ese timer no consume RNG y `TileLoop_Town` continúa con la obra.
    // Al agotarse ejecuta la aleatorización/callbacks y vuelve a programar el
    // período; el rearme no depende de esos callbacks y preserva la cadencia.
    if house_id >= crate::house_spec::NEW_HOUSE_OFFSET {
        let building_flags = crate::house_spec::house_spec_def(&state.house_spec_catalog, house_id)
            .map_or(0, |def| def.building_flags);
        let processing_time = tile.m6 >> 2;
        if processing_time != 0 {
            let mut updated = tile;
            // `Get/SetHouseProcessingTime` ocupa los seis bits altos de
            // MAPE. Los dos bajos son `AnimatedTileState`, por lo que deben
            // sobrevivir al decremento tal como en el mapa nativo.
            updated.m6 = (tile.m6 & 0x03) | ((processing_time - 1) << 2);
            if state.map.set_tile(coord, updated).is_ok() {
                state.runtime.landscape_tile_dirty.push(coord);
            }
        } else {
            if let Some(rng) = generation_rng.as_deref_mut() {
                advance_newgrf_house_tile_loop_randomisation(state, coord, building_flags, rng);
                trigger_newgrf_house_tile_loop_animations(state, coord, building_flags, rng);
            }
            let next_processing_time =
                crate::house_spec::house_spec_def(&state.house_spec_catalog, house_id)
                    .map_or(0, |def| def.processing_time.min(0x3F));
            let mut updated = state.map.get(coord).unwrap_or(tile);
            updated.m6 = (updated.m6 & 0x03) | (next_processing_time << 2);
            // `NewHouseTileLoop` termina con `MarkTileDirtyByTile`, incluso
            // cuando la propiedad es cero o la randomización no cambió MAP1.
            if state.map.set_tile(coord, updated).is_ok() {
                state.runtime.landscape_tile_dirty.push(coord);
            }
        }

        // `NewHouseTileLoop` retorna al caller antes de que
        // `TileLoop_Town` decida si la casa ya terminó. Las partes secundarias
        // no llevan flags de huella, así que sólo la tesela norte avanza el
        // conjunto completo.
        let Some(live_tile) = state.map.get(coord) else {
            return;
        };
        if live_tile.m3 & 0x80 == 0 {
            if building_flags == 0 {
                return;
            }
            for (dx, dy) in crate::house_spec::house_footprint_offsets(building_flags) {
                let part = TileCoord::new(coord.x + dx, coord.y + dy);
                if !advance_house_construction_tile(&mut state.map, part) {
                    continue;
                }
                // `AdvanceSingleHouseConstruction` marca la etapa, aun sin
                // callback. CB1C toma una palabra nueva sólo si su bit de
                // máscara está publicado y usa `param2 = 0` para un rollover
                // normal (la llamada inicial de `BuildTownHouse` es otra ruta).
                state.runtime.landscape_tile_dirty.push(part);
                if let Some(rng) = generation_rng.as_deref_mut() {
                    trigger_newgrf_house_construction_stage_changed_animation(state, part, rng);
                }
            }
        }
        return;
    }

    let Some(house) = HouseSpec::get(house_id) else {
        return;
    };

    if tile.m3 & 0x80 == 0 {
        // Las subteselas de una casa multitesela llevan un `HouseID` propio
        // pero su spec tiene flags vacías. OpenTTD sólo avanza el conjunto
        // cuando visita la tesela norte (la que conserva las flags de la
        // huella); avanzar una subtesela por separado terminaría la obra
        // varias visitas antes que el original.
        if house.building_flags == 0 {
            return;
        }
        for (dx, dy) in crate::house_spec::house_footprint_offsets(house.building_flags) {
            let part = TileCoord::new(coord.x + dx, coord.y + dy);
            advance_house_construction_tile(&mut state.map, part);
        }
        return;
    }

    let Some(rng) = generation_rng.as_deref_mut() else {
        return;
    };
    // `TileLoop_Town` tests the vanilla lift before taking the unconditional
    // random value used by cargo generation. `AddAnimatedTile` actualiza la
    // lista persistida y el estado bajo de `MAPE`; tanto la decisión como el
    // orden de inserción deben seguir el stream global para que
    // `AnimateAnimatedTiles` tome luego el mismo `RandomRange(7)`.
    let can_activate_lift = house.building_flags & crate::house_spec::BUILDING_FLAG_IS_ANIMATED
        != 0
        && !crate::map::house_lift::lift_has_destination(tile);
    if can_activate_lift {
        let activated = rng.chance16(1, 2);
        if activated {
            crate::map::activate_house_lift_animation(
                &mut state.map,
                &mut state.active_house_animations,
                coord,
            );
        }
    }

    // `r = Random()` siempre se extrae antes de la producción. El perfil de
    // Nueva partida de OpenTTD usa `TCGM_BITCOUNT` por defecto: la producción
    // sólo visita las dos especificaciones (pasajeros/correo) cuando los dos
    // bits altos del contador de tick coinciden con los dos bits bajos de
    // `TileIndex`. Esto también mantiene el stream exacto en la cola 0x500.
    let house_random = rng.next();
    let tile_index = coord_to_linear_index(coord, state.map.dimensions().0).unwrap_or(0);
    if ((tick >> 8) & 0x03) == u64::from(tile_index & 0x03) {
        let _passengers = rng.next();
        let _mail = rng.next();
    }

    // La renovación no es una pasada posterior de economía: `TileLoop_Town`
    // reutiliza `r = Random()` de esta misma visita después de generar carga.
    // Ejecutarla aquí conserva tanto el intercalado LFSR como el mapa que ven
    // las teselas siguientes de la franja.
    maybe_rebuild_vanilla_town_house(state, coord, tile, house, house_random, rng);
}

/// Bits de `HouseRandomTrigger`: `TileLoop` y `TileLoopNorth`.
const HOUSE_RANDOM_TRIGGER_TILE_LOOP: u8 = 1 << 0;
const HOUSE_RANDOM_TRIGGER_TILE_LOOP_NORTH: u8 = 1 << 1;

/// Ejecuta la parte Action2 de `NewHouseTileLoop` que no depende de callbacks.
///
/// Cada visita vencida dispara primero la tesela actual y después el trigger
/// compartido de la parte norte. Este último propaga sus bits resultantes a
/// las otras partes de una huella multitile, aunque cada parte consume su
/// propia palabra de `Random()` como el `DoTriggerHouseRandomisation` nativo.
fn advance_newgrf_house_tile_loop_randomisation(
    state: &mut GameState,
    coord: TileCoord,
    building_flags: u8,
    rng: &mut Randomizer,
) {
    let _ = trigger_newgrf_house_randomisation(
        state,
        coord,
        HOUSE_RANDOM_TRIGGER_TILE_LOOP,
        None,
        false,
        rng,
    );

    if building_flags & HOUSE_FOOTPRINT_FLAGS == 0 {
        return;
    }
    let Some(shared_random_bits) = trigger_newgrf_house_randomisation(
        state,
        coord,
        HOUSE_RANDOM_TRIGGER_TILE_LOOP_NORTH,
        None,
        false,
        rng,
    ) else {
        return;
    };

    for &(dx, dy) in crate::house_spec::house_footprint_offsets(building_flags)
        .iter()
        .skip(1)
    {
        let _ = trigger_newgrf_house_randomisation(
            state,
            TileCoord::new(coord.x + dx, coord.y + dy),
            HOUSE_RANDOM_TRIGGER_TILE_LOOP_NORTH,
            Some(shared_random_bits),
            true,
            rng,
        );
    }
}

/// Ejecuta los triggers CB1B que siguen a la randomización de `NewHouseTileLoop`.
///
/// La llamada sin sincronizar sólo toma una palabra cuando la máscara y el
/// flag del spec coinciden. En cambio, toda huella válida toma primero una
/// palabra compartida para la rama sincronizada, aun si ninguna parte publica
/// CB1B; las llamadas que sí coinciden toman una palabra baja propia y reciben
/// la compartida en los 16 bits altos de `param1`.
fn trigger_newgrf_house_tile_loop_animations(
    state: &mut GameState,
    coord: TileCoord,
    building_flags: u8,
    rng: &mut Randomizer,
) {
    trigger_newgrf_house_tile_loop_animation(state, coord, false, 0, rng);

    if building_flags & HOUSE_FOOTPRINT_FLAGS == 0 {
        return;
    }
    let shared_random_bits = u16::try_from(rng.next() & u32::from(u16::MAX)).unwrap_or(0);
    trigger_newgrf_house_tile_loop_animation(state, coord, true, shared_random_bits, rng);
    for &(dx, dy) in crate::house_spec::house_footprint_offsets(building_flags)
        .iter()
        .skip(1)
    {
        trigger_newgrf_house_tile_loop_animation(
            state,
            TileCoord::new(coord.x + dx, coord.y + dy),
            true,
            shared_random_bits,
            rng,
        );
    }
}

/// Ejecuta un CB1B de una sola tesela y aplica `ChangeAnimationFrame`.
fn trigger_newgrf_house_tile_loop_animation(
    state: &mut GameState,
    coord: TileCoord,
    sync: bool,
    shared_random_bits: u16,
    rng: &mut Randomizer,
) {
    let Some(tile) = state.map.get(coord) else {
        return;
    };
    let house_id = tile.m8 & 0x0FFF;
    let matches_trigger = crate::house_spec::house_spec_def(&state.house_spec_catalog, house_id)
        .is_some_and(|def| {
            def.has_animation_tile_loop_callback()
                && def.animation_tile_loop_is_synchronized() == sync
        });
    if !matches_trigger {
        return;
    }

    let random_bits = if sync {
        (rng.next() & u32::from(u16::MAX)) | (u32::from(shared_random_bits) << 16)
    } else {
        rng.next()
    };
    let Some(def) = crate::house_spec::house_spec_def(&state.house_spec_catalog, house_id) else {
        return;
    };
    let result = crate::newgrf_callback::resolve_house_animation_callback_with_world(
        def,
        &state.map,
        &mut state.towns,
        &state.house_spec_catalog,
        state.climate,
        coord,
        crate::newgrf_sprites::CBID_HOUSE_ANIMATION_TRIGGER_TILE_LOOP,
        random_bits,
        0,
    );
    if crate::map::house_lift::apply_newgrf_house_animation_callback_result(
        &mut state.map,
        &mut state.active_house_animations,
        coord,
        result,
    ) {
        state.runtime.landscape_tile_dirty.push(coord);
    }
}

/// Ejecuta CB1C al terminar una etapa de obra de una casa `NewGRF`.
///
/// La palabra aleatoria de `TriggerHouseAnimation_ConstructionStageChanged`
/// sólo se consume después de verificar la máscara. La vía de tile loop
/// siempre representa un rollover, por lo que `param2` es cero; la llamada
/// inicial de `BuildTownHouse` usa uno y se conecta desde su propio call site.
fn trigger_newgrf_house_construction_stage_changed_animation(
    state: &mut GameState,
    coord: TileCoord,
    rng: &mut Randomizer,
) {
    let Some(tile) = state.map.get(coord) else {
        return;
    };
    let house_id = tile.m8 & 0x0FFF;
    let Some(def) = crate::house_spec::house_spec_def(&state.house_spec_catalog, house_id) else {
        return;
    };
    if !def.has_animation_construction_stage_changed_callback() {
        return;
    }

    let result = crate::newgrf_callback::resolve_house_animation_callback_with_world(
        def,
        &state.map,
        &mut state.towns,
        &state.house_spec_catalog,
        state.climate,
        coord,
        crate::newgrf_sprites::CBID_HOUSE_ANIMATION_TRIGGER_CONSTRUCTION_STAGE_CHANGED,
        rng.next(),
        0,
    );
    if crate::map::house_lift::apply_newgrf_house_animation_callback_result(
        &mut state.map,
        &mut state.active_house_animations,
        coord,
        result,
    ) {
        state.runtime.landscape_tile_dirty.push(coord);
    }
}

/// Port de una llamada a `DoTriggerHouseRandomisation` para una sola tesela.
///
/// `shared_random_bits` sólo se usa al propagar `TileLoopNorth`: la palabra
/// global todavía se consume, pero la máscara de reseed recibe los bits que
/// resolvió la parte norte. Los scopes Action2 se construyen después de dejar
/// el trigger pendiente en `MAP3`, para que `var 5F` vea el mismo estado que
/// el resolver nativo.
fn trigger_newgrf_house_randomisation(
    state: &mut GameState,
    coord: TileCoord,
    trigger: u8,
    shared_random_bits: Option<u8>,
    mark_dirty: bool,
    rng: &mut Randomizer,
) -> Option<u8> {
    let snapshot = state.map.get(coord)?;
    if snapshot.kind != TileKind::House {
        return None;
    }
    let house_id = snapshot.m8 & 0x0FFF;
    let def = crate::house_spec::house_spec_def(&state.house_spec_catalog, house_id)?;
    let grfid = def.grfid;
    let local_id = def.newgrf_local_id;
    let runtime = def.newgrf_runtime.clone();
    // `HasSpriteGroups` nativo exige un grupo default asignado por Action3;
    // un grafo variational perteneciente a otra casa del mismo GRF no debe
    // introducir `Random()` en esta tesela. Para sprites planos no retenemos
    // el grafo, de modo que la vista ya decodificada es la evidencia del grupo.
    let has_sprite_group = runtime
        .as_deref()
        .map_or(!def.newgrf_views.is_empty(), |gfx| {
            gfx.assigns.iter().any(|assign| assign.local_id == local_id)
                || gfx
                    .extended_assigns
                    .iter()
                    .any(|(assigned_id, _)| *assigned_id == u16::from(local_id))
        });
    if !has_sprite_group {
        return None;
    }

    let waiting = (snapshot.m3 & 0x1F) | trigger;
    let mut pending = snapshot;
    pending.m3 = (pending.m3 & !0x1F) | waiting;
    state.map.set_tile(coord, pending).ok()?;

    let (reseed, used) = if let Some(runtime) = runtime.as_deref() {
        let neighbor_params = requested_newgrf_house_scope_vars(runtime);
        let counts = crate::house_spec::HouseScopeCounts::from_map(&state.map, &state.towns);
        let mut ctx = crate::house_spec::action2_eval_ctx_for_house_tile_with_counts(
            &state.map,
            pending,
            coord.x,
            coord.y,
            state.climate,
            &state.towns,
            &state.house_spec_catalog,
            &counts,
            &neighbor_params,
        );
        let result = runtime.rerandomisation_for_local_id(local_id, &mut ctx, waiting);
        if let Some(town_index) = newgrf_house_town_index(state, coord, pending) {
            crate::newgrf_callback::writeback_town_persistent_registers(
                &mut state.towns[town_index],
                grfid,
                &ctx,
            );
        }
        result
    } else {
        (0, 0)
    };

    // El C++ asigna `Random()` a `uint8_t` incluso cuando la máscara resuelta
    // vale cero y aun cuando TileLoopNorth recibe los bits compartidos.
    let generated_random_bits = u8::try_from(rng.next() & u32::from(u8::MAX)).unwrap_or(0);
    let random_bits = shared_random_bits.unwrap_or(generated_random_bits);
    let mut updated = state.map.get(coord)?;
    updated.m3 = (updated.m3 & !0x1F) | (waiting & !used);
    let reseed_mask = u8::try_from(reseed & u32::from(u8::MAX)).unwrap_or(0);
    updated.m1 = (updated.m1 & !reseed_mask) | (random_bits & reseed_mask);
    state.map.set_tile(coord, updated).ok()?;
    if mark_dirty {
        state.runtime.landscape_tile_dirty.push(coord);
    }
    Some(updated.m1)
}

/// Variables parametrizadas de `HouseScopeResolver` que el grafo Action2
/// puede requerir mientras decide la máscara de reseed.
fn requested_newgrf_house_scope_vars(
    runtime: &crate::newgrf_sprites::TrainSpriteGraphics,
) -> Vec<(u8, u8)> {
    let mut requested = Vec::new();
    for entry in runtime.action2_var.values() {
        for term in std::iter::once(&entry.first).chain(entry.ops.iter().map(|op| &op.rhs)) {
            if (0x60..=0x63).contains(&term.variable)
                && let Some(parameter) = term.param
                && !requested.contains(&(term.variable, parameter))
            {
                requested.push((term.variable, parameter));
            }
        }
    }
    requested.sort_unstable();
    requested
}

/// Devuelve el town scope que seleccionó el constructor del contexto de casa:
/// primero `MAP2`, y ante una referencia legacy inexistente, el más cercano.
fn newgrf_house_town_index(state: &GameState, coord: TileCoord, tile: Tile) -> Option<usize> {
    let persisted = u32::from(tile.m2) | (u32::from(tile.m2_hi) << 8);
    state
        .towns
        .iter()
        .position(|town| town.id == persisted)
        .or_else(|| crate::town::nearest_town_index(&state.towns, coord).map(|(index, _)| index))
}

/// Máscara `BUILDING_HAS_1_TILE` de `OpenTTD`. Los bits no representan una
/// cuenta: `Size1x1` es el bit cero y los otros tamaños ocupan bits 2--4.
const HOUSE_FOOTPRINT_FLAGS: u8 = BUILDING_FLAG_SIZE_1X1
    | BUILDING_FLAG_SIZE_2X1
    | BUILDING_FLAG_SIZE_1X2
    | BUILDING_FLAG_SIZE_2X2;
const VANILLA_HOUSE_FOOTPRINT_FLAGS: u8 = HOUSE_FOOTPRINT_FLAGS;
const VANILLA_HOUSE_MULTI_TILE_FLAGS: u8 =
    BUILDING_FLAG_SIZE_2X1 | BUILDING_FLAG_SIZE_1X2 | BUILDING_FLAG_SIZE_2X2;

/// Replica la cola de reconstrucción de `TileLoop_Town` para una casa vanilla
/// completa. El contador se decrementa con wrapping, igual que el `uint16_t`
/// nativo: un valor cero no dispara una reconstrucción prematura.
fn maybe_rebuild_vanilla_town_house(
    state: &mut GameState,
    coord: TileCoord,
    tile: Tile,
    house: HouseSpec,
    house_random: u32,
    rng: &mut Randomizer,
) {
    if house.building_flags & VANILLA_HOUSE_FOOTPRINT_FLAGS == 0
        || tile.m3 & 0x20 != 0
        || tile.m5 < house.minimum_life
    {
        return;
    }

    // Validar antes de tocar el caché municipal. `ClearTownHouse` asume una
    // huella íntegra; un `.sav` corrupto no debe perder población sólo porque
    // una de sus subteselas quedó fuera del mapa.
    let footprint_is_intact = crate::house_spec::house_footprint_offsets(house.building_flags)
        .into_iter()
        .map(|(dx, dy)| TileCoord::new(coord.x + dx, coord.y + dy))
        .all(
            |part| matches!(state.map.get(part), Some(current) if current.kind == TileKind::House),
        );
    if !footprint_is_intact {
        return;
    }

    let persisted_town_id = u32::from(tile.m2) | (u32::from(tile.m2_hi) << 8);
    let Some(town_index) = state
        .towns
        .iter()
        .position(|town| town.id == persisted_town_id)
        .or_else(|| crate::town::nearest_town_index(&state.towns, coord).map(|(index, _)| index))
    else {
        return;
    };

    let rebuild_tile = {
        let town = &mut state.towns[town_index];
        if !town.is_growing {
            return;
        }

        town.time_until_rebuild = town.time_until_rebuild.wrapping_sub(1);
        if town.time_until_rebuild != 0 {
            return;
        }
        town.time_until_rebuild = u16::try_from((house_random >> 16) & 0xFF)
            .unwrap_or(0)
            .saturating_add(192);

        // `ClearTownHouse` reduce la población sólo si la construcción ya
        // había terminado, quita un edificio (no una subtesela) y actualiza
        // los flags únicos antes de que `TryBuildTownHouse` elija un reemplazo.
        town.population = town.population.saturating_sub(u32::from(house.population));
        town.num_houses = town.num_houses.saturating_sub(1);
        if house.building_flags & BUILDING_FLAG_IS_CHURCH != 0 {
            town.has_church = false;
        } else if house.building_flags & BUILDING_FLAG_IS_STADIUM != 0 {
            town.has_stadium = false;
        }
        crate::town::update_town_radius(town);

        rebuild_house_tile_closest_to_town(coord, house.building_flags, town.pos)
    };

    let Some(cleared) = clear_vanilla_town_house_footprint(&mut state.map, coord, house) else {
        // Un footprint corrupto no existe en OpenTTD; no hacemos que una
        // importación parcial borre una tesela aislada ni deje contadores
        // municipales inconsistentes.
        return;
    };
    state.runtime.landscape_tile_dirty.extend(cleared);

    // `GB(r, 24, 8) < 12` deja el solar vacío, sin otro sorteo.
    if (house_random >> 24) & 0xFF < 12 {
        return;
    }

    if let Some(base) = crate::world_gen::rebuild_vanilla_town_house_with_rng(
        &mut state.map,
        &mut state.towns[town_index],
        rebuild_tile,
        state.climate,
        state.snow_line_height,
        state.calendar.year,
        rng,
    ) {
        let flags = state
            .map
            .get(base)
            .and_then(|rebuilt| HouseSpec::get(rebuilt.m8 & 0x0FFF))
            .map_or(0, |rebuilt| rebuilt.building_flags);
        for (dx, dy) in crate::house_spec::house_footprint_offsets(flags) {
            state
                .runtime
                .landscape_tile_dirty
                .push(TileCoord::new(base.x + dx, base.y + dy));
        }
    }
}

/// Después de despejar un edificio multitile, `OpenTTD` desplaza el nuevo intento
/// hacia el centro de la ciudad para que las casas grandes no se alejen de la
/// red vial. `TileIndexToTileIndexDiffC(town, tile)` es `town - tile`.
fn rebuild_house_tile_closest_to_town(
    coord: TileCoord,
    building_flags: u8,
    town_pos: TileCoord,
) -> TileCoord {
    if building_flags & VANILLA_HOUSE_MULTI_TILE_FLAGS == 0 {
        return coord;
    }
    let x = (town_pos.x - coord.x).clamp(0, 1);
    let y = (town_pos.y - coord.y).clamp(0, 1);
    if building_flags & BUILDING_FLAG_SIZE_2X2 != 0 {
        TileCoord::new(coord.x + x, coord.y + y)
    } else if building_flags & BUILDING_FLAG_SIZE_1X2 != 0 {
        TileCoord::new(coord.x, coord.y + y)
    } else {
        TileCoord::new(coord.x + x, coord.y)
    }
}

/// `ClearTownHouse` + `DoClearSquare` para cada subtesela de una huella
/// vanilla. La comprobación previa mantiene la operación atómica ante mapas
/// corruptos; en un mapa válido coincide con el orden base, `+Y`, `+X`,
/// `+X+Y` de `OpenTTD`.
fn clear_vanilla_town_house_footprint(
    map: &mut Map,
    coord: TileCoord,
    house: HouseSpec,
) -> Option<Vec<TileCoord>> {
    let parts: Vec<_> = crate::house_spec::house_footprint_offsets(house.building_flags)
        .into_iter()
        .map(|(dx, dy)| TileCoord::new(coord.x + dx, coord.y + dy))
        .collect();
    if parts.is_empty()
        || parts
            .iter()
            .any(|&part| !matches!(map.get(part), Some(current) if current.kind == TileKind::House))
    {
        return None;
    }

    for &part in &parts {
        let mut clear = map.get(part)?;
        crate::map::clear_neighbour_non_flooding_states(map, part);
        clear.kind = TileKind::Grass;
        clear.mapt &= 0x0F;
        clear.m1 = crate::company::OWNER_NONE_M1;
        clear.m2 = 0;
        clear.m2_hi = 0;
        clear.m3 = 0;
        clear.m3hi = 0;
        clear.m5 = crate::world_gen::clear_ground_m5(crate::world_gen::CLEAR_GROUND_GRASS, 3);
        clear.m6 = 0;
        clear.m7 = 0;
        clear.m8 = 0;
        map.set_tile(part, clear).ok()?;
    }
    Some(parts)
}

/// Despacha una visita actual de `TileLoop_Town` con el stream global.
///
/// La cola de creación y el loop normal comparten el contrato de casas
/// vanilla: obra incompleta o callback `NewGRF` no consumen por este fallback;
/// una casa completa toma el `Random()` incondicional y, en la franja
/// `TCGM_BITCOUNT` correspondiente, los dos sorteos de pasajeros/correo.
/// Mantener la visita individual permite al caller conservar el intercalado
/// LFSR con `TileLoop_Industry` y futuros despachos de tesela.
pub(crate) fn advance_town_tile_loop_from_visit_with_rng(
    state: &mut GameState,
    tick: u64,
    coord: TileCoord,
    snapshot: Tile,
    rng: &mut Randomizer,
) {
    if snapshot.kind != TileKind::House {
        return;
    }
    let tile = state.map.get(coord).unwrap_or(snapshot);
    let mut shared_rng = Some(rng);
    tile_loop_house(state, tick, coord, tile, &mut shared_rng);
}

/// Avanza una tesela de obra y devuelve si acabó una etapa.
///
/// `GetHouseConstructionTick` ocupa los tres bits bajos de `MAP5`; sólo el
/// wrap a cero dispara `TriggerHouseAnimation_ConstructionStageChanged`.
fn advance_house_construction_tile(map: &mut crate::map::Map, coord: TileCoord) -> bool {
    let Some(mut tile) = map.get(coord) else {
        return false;
    };
    if tile.kind != TileKind::House || tile.m3 & 0x80 != 0 {
        return false;
    }

    let next = (tile.m5 & 0x1F).wrapping_add(1) & 0x1F;
    let stage_changed = next.is_multiple_of(8);
    tile.m5 = (tile.m5 & !0x1F) | next;
    if (tile.m5 >> 3) & 0x03 == 3 {
        tile.m3 |= 0x80;
        tile.m5 = 0;
    }
    map.set_tile(coord, tile).is_ok() && stage_changed
}

/// Ejecuta la parte de `TileLoop_Road` que es observable durante la creación
/// de un mundo nuevo.
///
/// En ese momento todavía no hay vehículos ni obras viales, pero las calles
/// municipales sí pasan por el ajuste de decoración según la zona del pueblo.
/// `SetRoadside` sólo modifica `MAP6[3..=5]`; conservar los bits inferiores es
/// importante para los tipos de carretera/tranvía importados desde un save.
fn tile_loop_road(state: &mut GameState, coord: TileCoord, tile: Tile) {
    // Los depósitos tienen su propio callback en OpenTTD y nunca llegan aquí:
    // el decodificador los clasifica como `TileKind::RoadDepot`.
    if (tile.m5 >> 6) & 0x03 == 2 {
        return;
    }

    let Some(town) = state
        .towns
        .iter()
        .min_by_key(|town| (crate::economy::manhattan_distance(town.pos, coord), town.id))
    else {
        return;
    };

    // `_town_road_types` y `_town_road_types_2` de road_cmd.cpp. El primer
    // valor es el estado estable y el segundo el estado de transición que se
    // instala cuando la calle aún está en terreno desnudo.
    let zone = usize::from(get_town_radius_group(town, coord) as u8).min(4);
    let [desired, pre] = if state.climate == crate::world_gen::Climate::Toyland {
        // Toyland usa StreetLights en las zonas exteriores y no árboles.
        [[1_u8, 1], [2, 2], [3, 2], [3, 2], [3, 2]][zone]
    } else {
        [[1_u8, 1], [2, 2], [2, 2], [5, 5], [3, 2]][zone]
    };
    let current = (tile.m6 >> 3) & 0x07;
    let next = if current == desired {
        return;
    } else if current == pre {
        desired
    } else if current == 0 {
        pre
    } else {
        0
    };
    if next == current {
        return;
    }
    let mut updated = tile;
    updated.m6 = (updated.m6 & !0x38) | (next << 3);
    let _ = state.map.set_tile(coord, updated);
}

/// Reproduce la rama `CLEAR_FIELDS` de `TileLoop_Clear`.
///
/// Los campos son `MP_CLEAR` aunque se vean como una clase semántica de
/// terreno. Cada visita actualiza primero las cercas que limitan con una
/// tesela que no es campo y después avanza `MAP5`/`MAP3` con el contador de
/// ocho estados. Cuando un campo huérfano supera el tipo 7, `OpenTTD` lo
/// convierte en hierba de densidad 2; los campos ligados a una industria
/// vuelven al tipo 0 después del tipo 8.
/// Despacha una visita regular de `TileLoop_Clear` para un campo vivo.
///
/// La cola de generación ya utiliza [`tile_loop_clear_field`], pero la
/// simulación regular mantiene su propio despacho LFSR. Validar el nibble
/// crudo además del tipo semántico evita tratar como campos a una tesela que
/// el cargador represente como `Grass` por compatibilidad.
pub(crate) fn advance_clear_field_tile_loop_from_visit(
    state: &mut GameState,
    coord: TileCoord,
) -> bool {
    let Some(tile) = state.map.get(coord) else {
        return false;
    };
    if tile.kind != TileKind::Grass
        || tile.ottd_type_nibble() != 0
        || crate::map::tree_tile_loop::clear_ground_type(tile.m5)
            != crate::world_gen::CLEAR_GROUND_FIELDS
    {
        return false;
    }

    tile_loop_clear_field(state, coord, tile);
    state.map.get(coord).is_some_and(|updated| updated != tile)
}

fn tile_loop_clear_field(state: &mut GameState, coord: TileCoord, tile: Tile) {
    if tile.m3 & 0x10 != 0 {
        return;
    }

    let mut updated = tile;
    for direction in 0_u8..4 {
        if field_fence(updated, direction) != 0 {
            continue;
        }
        let (dx, dy) = crate::map::diag_dir_offset(direction);
        let neighbour = TileCoord::new(coord.x + dx, coord.y + dy);
        let neighbour_is_field = state.map.get(neighbour).is_some_and(|candidate| {
            candidate.kind == TileKind::Grass
                && candidate.ottd_type_nibble() == 0
                && crate::map::tree_tile_loop::clear_ground_type(candidate.m5)
                    == crate::world_gen::CLEAR_GROUND_FIELDS
        });
        if !neighbour_is_field {
            set_field_fence(&mut updated, direction, 3);
        }
    }

    let counter = crate::map::tree_tile_loop::clear_counter(updated.m5);
    if counter < 7 {
        updated.m5 = crate::map::tree_tile_loop::with_clear_counter(updated.m5, counter + 1);
    } else {
        updated.m5 = crate::map::tree_tile_loop::with_clear_counter(updated.m5, 0);
        let field_type = updated.m3 & 0x0F;
        if field_type >= 7
            && !state
                .industries
                .iter()
                .any(|industry| industry.instance_id == industry_instance_id(&updated))
        {
            // `MakeClear(tile, CLEAR_GRASS, 2)` resets every auxiliary map
            // plane except the low `TropicZone` nibble of `MAPT`.
            updated.kind = TileKind::Grass;
            updated.mapt &= 0x0F;
            updated.m1 = crate::company::OWNER_NONE_M1;
            updated.m2 = 0;
            updated.m2_hi = 0;
            updated.m3 = 0;
            updated.m3hi = 0;
            updated.m5 = crate::world_gen::clear_ground_m5(crate::world_gen::CLEAR_GROUND_GRASS, 2);
            updated.m6 = 0;
            updated.m7 = 0;
            updated.m8 = 0;
        } else {
            let next_type = if field_type < 8 { field_type + 1 } else { 0 };
            updated.m3 = (updated.m3 & !0x0F) | next_type;
        }
    }

    let _ = state.map.set_tile(coord, updated);
}

fn field_fence(tile: Tile, direction: u8) -> u8 {
    match direction & 3 {
        0 => (tile.m3 >> 5) & 0x07,   // DIAGDIR_NE
        1 => (tile.m3hi >> 2) & 0x07, // DIAGDIR_SE
        2 => (tile.m3hi >> 5) & 0x07, // DIAGDIR_SW
        _ => (tile.m6 >> 2) & 0x07,   // DIAGDIR_NW
    }
}

fn set_field_fence(tile: &mut Tile, direction: u8, value: u8) {
    let value = (value & 0x07) << 5;
    match direction & 3 {
        0 => tile.m3 = (tile.m3 & !0xE0) | value,
        1 => tile.m3hi = (tile.m3hi & !0x1C) | (value >> 3),
        2 => tile.m3hi = (tile.m3hi & !0xE0) | value,
        _ => tile.m6 = (tile.m6 & !0x1C) | (value >> 3),
    }
}

/// Reproduce las pasadas `RunTileLoop` finales de `CreateRivers`.
///
/// `GenerateLandscape` las corre antes de `GenerateClearTile`. El estado
/// temporal contiene sólo paisaje, por lo que no adelanta calendario,
/// vehículos, economía ni entidades de la partida; se conserva únicamente el
/// mapa y el cursor LFSR que afectan a los bytes de la frontera de generación.
pub fn run_landscape_river_tile_loops(
    map: &mut Map,
    climate: crate::world_gen::Climate,
    seed: u64,
) {
    let mut rng = Randomizer::new(seed as u32);
    run_landscape_river_tile_loops_with_rng(map, climate, seed, &mut rng);
}

/// Igual que [`run_landscape_river_tile_loops`], pero continúa el stream de
/// generación que ya usaron terreno y ríos.
///
/// Es `pub(crate)` para que [`crate::world_gen::apply_landscape_with_rng`]
/// no reinicie el RNG entre `CreateRivers` y `GenerateClearTile`.
pub(crate) fn run_landscape_river_tile_loops_with_rng(
    map: &mut Map,
    climate: crate::world_gen::Climate,
    seed: u64,
    rng: &mut Randomizer,
) {
    let _ = run_landscape_tile_loops_with_rng_and_cursor(
        map,
        climate,
        seed,
        rng,
        LANDSCAPE_RIVER_TILE_LOOP_PASSES,
        crate::world_gen::DEF_SNOW_LINE_HEIGHT,
        TileLoopState::default().cur_tileloop_tile,
    );
}

/// Ejecuta una cantidad explícita de pasadas del tile loop de generación y
/// devuelve el cursor LFSR para que otra frontera continúe la misma secuencia.
pub(crate) fn run_landscape_tile_loops_with_rng_and_cursor(
    map: &mut Map,
    climate: crate::world_gen::Climate,
    seed: u64,
    rng: &mut Randomizer,
    passes: u64,
    snow_line_height: u8,
    start_cursor: u32,
) -> u32 {
    // Mover en vez de clonar evita duplicar un mapa de hasta 4096² teselas
    // durante su creación. El placeholder no se observa: se reemplaza por el
    // mapa generado antes de devolver.
    let landscape = std::mem::replace(map, Map::new_flat(0, 0, 0));
    let mut state = GameState::from_map(landscape);
    state.climate = climate;
    state.world_seed = seed;
    state.snow_line_height = snow_line_height;
    state.cur_tileloop_tile = start_cursor;
    for _ in 0..passes {
        // `CreateRivers` no incrementa `TimerGameTick::counter` dentro de
        // este bucle. Usar siempre cero conserva tanto el callback manual de
        // tile 0 como cualquier regla dependiente del tick.
        run_generation_tile_loop_impl(&mut state, 0, Some(&mut *rng));
    }
    let cursor = state.cur_tileloop_tile;
    *map = state.map;
    cursor
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::map::{
        Map, TileCoord, TileKind, TileLoopState, WaterClass, collect_tile_loop_visits,
        set_water_class_m1,
    };

    #[test]
    fn generation_loop_uses_lfsr_without_advancing_calendar() {
        let state_map = Map::new_flat(64, 64, 1);
        let mut expected_cur = TileLoopState::default().cur_tileloop_tile;
        let expected = collect_tile_loop_visits(&state_map, 0, &mut expected_cur);
        let mut state = GameState::from_map(state_map);
        let before_tick = state.tick.get();
        let before_calendar = state.calendar;

        let visited = run_generation_tile_loop(&mut state, 0);

        assert_eq!(visited, expected.len());
        assert_eq!(state.cur_tileloop_tile, expected_cur);
        assert_eq!(state.runtime.tile_loop_visited.len(), expected.len());
        assert_eq!(state.tick.get(), before_tick);
        assert_eq!(state.calendar, before_calendar);
    }

    #[test]
    fn generation_loop_processes_clear_tiles_but_not_simulation_entities() {
        let mut map = Map::new_flat(64, 64, 1);
        let c = TileCoord::new(0, 0); // tile 0 se visita manualmente en tick 0
        map.set_kind(c, TileKind::Grass).unwrap();
        map.set_mapt_m5(c, 0, 0x00).unwrap();
        let mut state = GameState::from_map(map);
        let before_vehicles = state.vehicles.len();
        let before_industries = state.industries.len();

        run_generation_tile_loop(&mut state, 0);

        assert_eq!(state.vehicles.len(), before_vehicles);
        assert_eq!(state.industries.len(), before_industries);
        assert_ne!(state.map.get(c).unwrap().m5, 0x00);
    }

    #[test]
    fn startup_tile_loop_prefix_keeps_calendar_separate() {
        let map = Map::new_flat(64, 64, 1);
        let mut state = GameState::from_map(map);
        let mut expected_cursor = state.cur_tileloop_tile;
        let mut expected_visits = 0usize;
        for tick in 0..3 {
            expected_visits +=
                collect_tile_loop_visits(&state.map, tick, &mut expected_cursor).len();
        }
        let mut rng = Randomizer::new(42);
        let before_tick = state.tick.get();

        let visited = run_generation_tile_loops_with_rng(&mut state, &mut rng, 3);

        assert_eq!(visited, expected_visits);
        assert_eq!(state.tick.get(), before_tick);
        assert_eq!(state.cur_tileloop_tile, expected_cursor);
        assert_eq!(state.runtime.tile_loop_visited.len(), 16);
    }

    #[test]
    fn first_regular_game_tick_runs_animation_then_next_lfsr_pass() {
        let map = Map::new_flat(64, 64, 1);
        let mut state = GameState::from_map(map);
        let mut expected_cursor = state.cur_tileloop_tile;
        let _ = collect_tile_loop_visits(&state.map, 1281, &mut expected_cursor);
        let mut rng = Randomizer::new(42);

        let visited = run_first_regular_game_tick_with_rng(&mut state, &mut rng, 1280);

        assert_eq!(visited, 16);
        assert_eq!(state.cur_tileloop_tile, expected_cursor);
        assert_eq!(state.tick.get(), 0);
        assert_eq!(state.runtime.tile_loop_visited.len(), 16);
    }

    #[test]
    fn road_tile_loop_applies_town_roadside_transition_and_preserves_low_bits() {
        let mut map = Map::new_flat(16, 16, 0);
        let road = TileCoord::new(8, 8);
        let mut tile = map.get(road).unwrap();
        tile.kind = TileKind::Road;
        tile.mapt = crate::map::OTTD_MP_ROAD << 4;
        tile.m5 = 0x05;
        tile.m6 = 0x03;
        map.set_tile(road, tile).unwrap();

        let mut town = crate::town::Town {
            id: 0,
            pos: road,
            num_houses: 40,
            squared_town_zone_radius: [100, 64, 36, 16, 4],
            ..Default::default()
        };
        crate::town::update_town_radius(&mut town);
        let mut state = GameState::from_map(map);
        state.towns.push(town);

        // TownCentre uses Paved as the transition and StreetLights as the
        // stable value (`_town_road_types[4]`).
        let current = state.map.get(road).unwrap();
        tile_loop_road(&mut state, road, current);
        assert_eq!((state.map.get(road).unwrap().m6 >> 3) & 0x07, 2);
        assert_eq!(state.map.get(road).unwrap().m6 & 0x07, 3);
        let current = state.map.get(road).unwrap();
        tile_loop_road(&mut state, road, current);
        assert_eq!((state.map.get(road).unwrap().m6 >> 3) & 0x07, 3);
        assert_eq!(state.map.get(road).unwrap().m6 & 0x07, 3);
    }

    #[test]
    fn clear_field_tile_loop_updates_fences_and_reclaims_orphans() {
        let mut map = Map::new_flat(3, 3, 0);
        let field = TileCoord::new(1, 1);
        let mut tile = map.get(field).unwrap();
        tile.m5 = crate::world_gen::clear_ground_m5(crate::world_gen::CLEAR_GROUND_FIELDS, 3);
        tile.m2 = 0;
        map.set_tile(field, tile).unwrap();
        let mut state = GameState::from_map(map);

        let current = state.map.get(field).unwrap();
        tile_loop_clear_field(&mut state, field, current);
        let updated = state.map.get(field).unwrap();
        assert_eq!(crate::map::tree_tile_loop::clear_counter(updated.m5), 1);
        assert_eq!((updated.m3 >> 5) & 0x07, 3);
        assert_eq!((updated.m3hi >> 2) & 0x07, 3);
        assert_eq!((updated.m3hi >> 5) & 0x07, 3);
        assert_eq!((updated.m6 >> 2) & 0x07, 3);

        let mut orphan = updated;
        orphan.m3 = (orphan.m3 & !0x0F) | 7;
        orphan.m5 = crate::map::tree_tile_loop::with_clear_counter(orphan.m5, 7);
        state.map.set_tile(field, orphan).unwrap();
        let current = state.map.get(field).unwrap();
        tile_loop_clear_field(&mut state, field, current);
        let reclaimed = state.map.get(field).unwrap();
        assert_eq!(reclaimed.kind, TileKind::Grass);
        assert_eq!(reclaimed.m5, crate::world_gen::clear_ground_m5(0, 2));
        assert_eq!(reclaimed.m3, 0);
    }

    #[test]
    fn town_tile_loop_advances_multitile_construction_once_per_base_visit() {
        let mut map = Map::new_flat(4, 4, 0);
        let base = TileCoord::new(1, 1);
        let spec = crate::map::TownHouseSpec {
            house_id: 20, // vanilla 2×2 stadium footprint
            town_id: 0,
            random_bits: 0,
            construction_counter: 7,
            construction_stage: 2,
            is_protected: false,
            processing_time: 0,
        };
        let offsets = [(0, 0, 20), (0, 1, 21), (1, 0, 22), (1, 1, 23)];
        for (dx, dy, house_id) in offsets {
            let tile =
                crate::map::Tile::town_house(crate::map::TownHouseSpec { house_id, ..spec }, 0, 0);
            map.set_tile(TileCoord::new(base.x + dx, base.y + dy), tile)
                .unwrap();
        }
        let mut state = GameState::from_map(map);
        let mut rng = Randomizer::new(1);
        let mut generation_rng = Some(&mut rng);
        let current = state.map.get(base).unwrap();

        tile_loop_house(&mut state, 0, base, current, &mut generation_rng);

        for (dx, dy, _) in offsets {
            let tile = state
                .map
                .get(TileCoord::new(base.x + dx, base.y + dy))
                .unwrap();
            assert_ne!(tile.m3 & 0x80, 0);
            assert_eq!(tile.m5, 0);
        }
        // Construction returns before the completed-house RNG path.
        assert_eq!(rng, Randomizer::new(1));
    }

    #[test]
    fn town_tile_loop_consumes_house_random_and_default_cargo_draws() {
        let mut map = Map::new_flat(2, 2, 0);
        let coord = TileCoord::new(0, 0);
        map.set_tile(coord, crate::map::Tile::completed_house(0, 0, 0))
            .unwrap();
        let mut state = GameState::from_map(map);
        let mut actual = Randomizer::new(42);
        let mut expected = actual;
        let _ = expected.next();
        let _ = expected.next();
        let _ = expected.next();
        let mut generation_rng = Some(&mut actual);
        let current = state.map.get(coord).unwrap();

        tile_loop_house(&mut state, 0, coord, current, &mut generation_rng);

        assert_eq!(actual, expected);
    }

    #[test]
    fn protected_animated_house_still_uses_lift_rng_but_skips_rebuild() {
        let mut map = Map::new_flat(2, 2, 0);
        let coord = TileCoord::new(1, 0);
        let house = HouseSpec::get(4).expect("Large Office vanilla");
        map.set_completed_house(coord, house.id, u8::MAX)
            .expect("office inside map");
        map.set_house_town_id(coord, 7)
            .expect("office town attribution");
        let mut protected = map.get(coord).expect("office tile");
        protected.m3 |= 0x20;
        map.set_tile(coord, protected).expect("protect office");

        let mut state = GameState::from_map(map);
        state.towns.push(crate::town::Town {
            id: 7,
            pos: coord,
            population: u32::from(house.population),
            num_houses: 1,
            is_growing: true,
            time_until_rebuild: 1,
            ..Default::default()
        });
        // La primera palabra es cero: Chance16(1, 2) activa el ascensor.
        let mut actual = Randomizer { state: [8, 0] };
        let mut expected = actual;
        assert!(expected.chance16(1, 2));
        let _ = expected.next(); // `r = Random()`; tile 1 no entra en cargo en tick 0.
        let current = state.map.get(coord).expect("protected office");

        {
            let mut generation_rng = Some(&mut actual);
            tile_loop_house(&mut state, 0, coord, current, &mut generation_rng);
        }

        assert_eq!(actual, expected);
        assert_eq!(state.active_house_animations, vec![coord]);
        assert_eq!(state.towns[0].time_until_rebuild, 1);
        assert_eq!(state.towns[0].population, u32::from(house.population));
        assert_eq!(state.towns[0].num_houses, 1);
        let after = state.map.get(coord).expect("protected office after loop");
        assert_eq!(after.kind, TileKind::House);
        assert_ne!(after.m3 & 0x20, 0, "protection survives the lift path");
    }

    #[test]
    fn completed_multitile_house_consumes_rng_per_part_but_rebuilds_once() {
        let mut map = Map::new_flat(4, 4, 0);
        let base = TileCoord::new(1, 1);
        let parts = [
            (base, 20),
            (TileCoord::new(1, 2), 21),
            (TileCoord::new(2, 1), 22),
            (TileCoord::new(2, 2), 23),
        ];
        for &(coord, house_id) in &parts {
            map.set_completed_house(coord, house_id, u8::MAX)
                .expect("stadium part inside map");
        }
        let stadium = HouseSpec::get(20).expect("stadium north tile");
        let mut state = GameState::from_map(map);
        state.towns.push(crate::town::Town {
            id: 0,
            pos: base,
            population: u32::from(stadium.population),
            num_houses: 1,
            is_growing: true,
            time_until_rebuild: 5,
            ..Default::default()
        });
        let mut actual = Randomizer::new(42);
        let mut expected = actual;
        // Tick 256 coincide con las piezas de índices 5 y 9: ambas toman
        // `r` + pasajeros + correo. Las otras dos toman sólo `r`.
        for _ in 0..8 {
            let _ = expected.next();
        }

        for &(coord, _) in &parts {
            let current = state.map.get(coord).expect("completed stadium part");
            let mut generation_rng = Some(&mut actual);
            tile_loop_house(&mut state, 256, coord, current, &mut generation_rng);
        }

        assert_eq!(actual, expected);
        assert_eq!(state.towns[0].time_until_rebuild, 4);
        assert_eq!(state.towns[0].num_houses, 1);
        for &(coord, _) in &parts {
            assert_eq!(
                state.map.get(coord).expect("stadium remains").kind,
                TileKind::House
            );
        }
    }

    #[test]
    fn newgrf_house_processing_timer_advances_without_rng_or_vanilla_fallback() {
        let id = crate::house_spec::NEW_HOUSE_OFFSET;
        let coord = TileCoord::new(1, 0);
        let mut map = Map::new_flat(2, 2, 0);
        let mut tile = crate::map::Tile::town_house(
            crate::map::TownHouseSpec {
                house_id: id,
                town_id: 0,
                random_bits: 0,
                construction_counter: 0,
                construction_stage: crate::map::TOWN_HOUSE_COMPLETED,
                is_protected: false,
                processing_time: 3,
            },
            0,
            0,
        );
        tile.m6 |= 0x03;
        map.set_tile(coord, tile).expect("NewGRF house inside map");
        let mut state = GameState::from_map(map);
        state
            .house_spec_catalog
            .push(crate::house_spec::HouseSpecDef {
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
                name: "processing-timer".into(),
                from_newgrf: true,
                grfid: 1,
                newgrf_views: Vec::new(),
                newgrf_local_id: 0,
                newgrf_runtime: None,
            });
        let mut actual = Randomizer::new(42);
        let current = state.map.get(coord).expect("NewGRF house");

        {
            let mut generation_rng = Some(&mut actual);
            tile_loop_house(&mut state, 0, coord, current, &mut generation_rng);
        }

        let updated = state.map.get(coord).expect("NewGRF house after timer");
        assert_eq!(updated.m6, 0x0B, "3 → 2 preserving AnimatedTileState");
        assert_eq!(actual, Randomizer::new(42));
        assert!(state.runtime.landscape_tile_dirty.contains(&coord));
    }

    #[test]
    fn newgrf_house_processing_timer_rearms_and_consumes_shared_cb1b_word() {
        let id = crate::house_spec::NEW_HOUSE_OFFSET;
        let coord = TileCoord::new(1, 0);
        let mut map = Map::new_flat(2, 2, 0);
        let mut tile = crate::map::Tile::town_house(
            crate::map::TownHouseSpec {
                house_id: id,
                town_id: 0,
                random_bits: 0,
                construction_counter: 0,
                construction_stage: crate::map::TOWN_HOUSE_COMPLETED,
                is_protected: false,
                processing_time: 0,
            },
            0,
            0,
        );
        tile.m6 |= 0x03;
        map.set_tile(coord, tile).expect("NewGRF house inside map");
        let mut state = GameState::from_map(map);
        state
            .house_spec_catalog
            .push(crate::house_spec::HouseSpecDef {
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
                processing_time: 3,
                extra_flags: 0,
                animation_frames: 0,
                animation_status: 0xFF,
                animation_speed: 2,
                override_id: None,
                callback_mask: 0,
                name: "periodic-timer".into(),
                from_newgrf: true,
                grfid: 1,
                newgrf_views: Vec::new(),
                newgrf_local_id: 0,
                newgrf_runtime: None,
            });
        let mut actual = Randomizer::new(42);

        for expected_m6 in [0x0F, 0x0B, 0x07, 0x03] {
            let current = state.map.get(coord).expect("NewGRF house");
            let mut generation_rng = Some(&mut actual);
            tile_loop_house(&mut state, 0, coord, current, &mut generation_rng);
            assert_eq!(
                state.map.get(coord).expect("NewGRF house after loop").m6,
                expected_m6
            );
        }

        let mut expected = Randomizer::new(42);
        let _shared_synchronised_cb1b_random = expected.next();
        assert_eq!(actual, expected);
        assert_eq!(
            state
                .runtime
                .landscape_tile_dirty
                .iter()
                .filter(|&&dirty| dirty == coord)
                .count(),
            4
        );
    }

    fn house_random_runtime(
        local_ids: &[u8],
        triggers: u8,
    ) -> crate::newgrf_sprites::TrainSpriteGraphics {
        let mut runtime = crate::newgrf_sprites::TrainSpriteGraphics::default();
        for &local_id in local_ids {
            runtime
                .assigns
                .push(crate::newgrf_sprites::TrainSpriteAssign {
                    local_id,
                    set_id: 1,
                });
        }
        runtime.action2_random.insert(
            1,
            crate::newgrf_sprites::Action2RandomEntry {
                typ: 0x80,
                consist_count: 0,
                triggers,
                randbit: 0,
                sets: vec![0, 0],
            },
        );
        runtime
    }

    fn newgrf_runtime_house(
        id: u16,
        local_id: u8,
        building_flags: u8,
        processing_time: u8,
        runtime: crate::newgrf_sprites::TrainSpriteGraphics,
    ) -> crate::house_spec::HouseSpecDef {
        crate::house_spec::HouseSpecDef {
            id,
            local_id,
            subst_id: 0,
            building_flags,
            min_year: 0,
            max_year: crate::house_spec::HOUSE_YEAR_MAX,
            population: 0,
            mail_generation: 0,
            availability: crate::house_spec::DEFAULT_HOUSE_AVAILABILITY,
            probability: crate::house_spec::DEFAULT_HOUSE_PROBABILITY,
            processing_time,
            extra_flags: 0,
            animation_frames: 0,
            animation_status: 0xFF,
            animation_speed: 2,
            override_id: None,
            callback_mask: 0,
            name: "random-house".into(),
            from_newgrf: true,
            grfid: 1,
            newgrf_views: Vec::new(),
            newgrf_local_id: local_id,
            newgrf_runtime: Some(Box::new(runtime)),
        }
    }

    fn house_animation_callback_entry(
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

    #[test]
    fn newgrf_house_construction_rollover_runs_cb1c_after_its_timer() {
        let id = crate::house_spec::NEW_HOUSE_OFFSET;
        let coord = TileCoord::new(1, 0);
        let mut map = Map::new_flat(2, 2, 0);
        map.set_tile(
            coord,
            crate::map::Tile::town_house(
                crate::map::TownHouseSpec {
                    house_id: id,
                    town_id: 0,
                    random_bits: 0,
                    construction_counter: 7,
                    construction_stage: 0,
                    is_protected: false,
                    processing_time: 1,
                },
                0,
                0,
            ),
        )
        .expect("NewGRF house inside map");

        let mut runtime = crate::newgrf_sprites::TrainSpriteGraphics::default();
        runtime
            .assigns
            .push(crate::newgrf_sprites::TrainSpriteAssign {
                local_id: 0,
                set_id: 0,
            });
        // `var 0C` expone el ID de callback: el frame observable confirma que
        // la llamada es CB1C, no CB20 ni un trigger de tile loop.
        runtime.action2_var.insert(
            0,
            house_animation_callback_entry(0x0C, 0, u32::from(u8::MAX)),
        );
        let mut def =
            newgrf_runtime_house(id, 0, crate::house_spec::BUILDING_FLAG_SIZE_1X1, 0, runtime);
        def.callback_mask =
            crate::house_spec::HOUSE_CALLBACK_ANIMATION_TRIGGER_CONSTRUCTION_STAGE_CHANGED_MASK;
        let mut state = GameState::from_map(map);
        state.house_spec_catalog.push(def);

        let mut actual = Randomizer::new(42);
        let mut expected = actual;
        let _cb1c_random = expected.next();
        let current = state.map.get(coord).expect("NewGRF house");
        let mut generation_rng = Some(&mut actual);
        tile_loop_house(&mut state, 0, coord, current, &mut generation_rng);

        let updated = state.map.get(coord).expect("NewGRF house after loop");
        assert_eq!(actual, expected, "CB1C takes exactly its fresh Random()");
        assert_eq!(updated.m5, 0x08, "counter rollover enters stage 1");
        assert_eq!(updated.m3 & 0x80, 0, "stage 1 is still under construction");
        assert_eq!(updated.m6 >> 2, 0, "timer decremented before construction");
        assert_eq!(
            updated.m7,
            u8::try_from(
                crate::newgrf_sprites::CBID_HOUSE_ANIMATION_TRIGGER_CONSTRUCTION_STAGE_CHANGED
            )
            .expect("CB1C frame")
        );
        assert_eq!(state.active_house_animations, vec![coord]);
        assert_eq!(
            state
                .runtime
                .landscape_tile_dirty
                .iter()
                .filter(|&&dirty| dirty == coord)
                .count(),
            3,
            "timer, stage rollover, and animation frame each invalidate the tile"
        );
    }

    #[test]
    fn newgrf_house_tile_loop_runs_unsynchronised_cb1b_and_adds_anit() {
        let id = crate::house_spec::NEW_HOUSE_OFFSET;
        let coord = TileCoord::new(1, 0);
        let mut map = Map::new_flat(2, 2, 0);
        map.set_tile(
            coord,
            crate::map::Tile::town_house(
                crate::map::TownHouseSpec {
                    house_id: id,
                    town_id: 0,
                    random_bits: 0,
                    construction_counter: 0,
                    construction_stage: crate::map::TOWN_HOUSE_COMPLETED,
                    is_protected: false,
                    processing_time: 0,
                },
                0,
                0,
            ),
        )
        .expect("NewGRF house inside map");
        let mut runtime = crate::newgrf_sprites::TrainSpriteGraphics::default();
        runtime
            .assigns
            .push(crate::newgrf_sprites::TrainSpriteAssign {
                local_id: 0,
                set_id: 0,
            });
        runtime
            .action2_var
            .insert(0, house_animation_callback_entry(0x1A, 0, 7));
        let mut def =
            newgrf_runtime_house(id, 0, crate::house_spec::BUILDING_FLAG_SIZE_1X1, 5, runtime);
        def.callback_mask = crate::house_spec::HOUSE_CALLBACK_ANIMATION_TRIGGER_TILE_LOOP_MASK;
        let mut state = GameState::from_map(map);
        state.house_spec_catalog.push(def);

        let mut actual = Randomizer::new(42);
        let mut expected = actual;
        let _tile_loop_random = expected.next();
        let _tile_loop_north_random = expected.next();
        let _unsynchronised_cb1b_random = expected.next();
        let _shared_synchronised_cb1b_random = expected.next();
        let current = state.map.get(coord).expect("NewGRF house");
        let mut generation_rng = Some(&mut actual);
        tile_loop_house(&mut state, 0, coord, current, &mut generation_rng);

        let updated = state.map.get(coord).expect("NewGRF house after loop");
        assert_eq!(actual, expected);
        assert_eq!(updated.m7, 7);
        assert_eq!(updated.m6, (5 << 2) | 0x03);
        assert_eq!(state.active_house_animations, vec![coord]);
    }

    #[test]
    fn synchronised_cb1b_shares_high_random_word_across_house_footprint() {
        let id = crate::house_spec::NEW_HOUSE_OFFSET;
        let north = TileCoord::new(1, 1);
        let east = TileCoord::new(2, 1);
        let mut map = Map::new_flat(4, 3, 0);
        for (coord, house_id) in [(north, id), (east, id + 1)] {
            map.set_tile(
                coord,
                crate::map::Tile::town_house(
                    crate::map::TownHouseSpec {
                        house_id,
                        town_id: 0,
                        random_bits: 0,
                        construction_counter: 0,
                        construction_stage: crate::map::TOWN_HOUSE_COMPLETED,
                        is_protected: false,
                        processing_time: 0,
                    },
                    0,
                    0,
                ),
            )
            .expect("NewGRF house inside map");
        }
        let mut runtime = crate::newgrf_sprites::TrainSpriteGraphics::default();
        runtime.assigns.extend([
            crate::newgrf_sprites::TrainSpriteAssign {
                local_id: 0,
                set_id: 0,
            },
            crate::newgrf_sprites::TrainSpriteAssign {
                local_id: 1,
                set_id: 1,
            },
        ]);
        // CB1B recibe la palabra compartida en los bits 16--31. Devolver su
        // byte bajo hace visible que ambas partes ven la misma mitad alta.
        let callback = house_animation_callback_entry(0x10, 16, 0xFF);
        runtime.action2_var.insert(0, callback.clone());
        runtime.action2_var.insert(1, callback);
        let mut north_def = newgrf_runtime_house(
            id,
            0,
            crate::house_spec::BUILDING_FLAG_SIZE_2X1,
            0,
            runtime.clone(),
        );
        north_def.callback_mask =
            crate::house_spec::HOUSE_CALLBACK_ANIMATION_TRIGGER_TILE_LOOP_MASK;
        north_def.extra_flags = crate::house_spec::HOUSE_EXTRA_FLAG_SYNCHRONIZED_CALLBACK_1B;
        let mut east_def = newgrf_runtime_house(id + 1, 1, 0, 0, runtime);
        east_def.callback_mask = crate::house_spec::HOUSE_CALLBACK_ANIMATION_TRIGGER_TILE_LOOP_MASK;
        east_def.extra_flags = crate::house_spec::HOUSE_EXTRA_FLAG_SYNCHRONIZED_CALLBACK_1B;
        let mut state = GameState::from_map(map);
        state.house_spec_catalog.extend([north_def, east_def]);

        let mut actual = Randomizer::new(42);
        let mut expected = actual;
        let _north_tile_loop_random = expected.next();
        let _north_tile_loop_north_random = expected.next();
        let _east_tile_loop_north_random = expected.next();
        let shared_random = expected.next();
        let _north_cb1b_low_random = expected.next();
        let _east_cb1b_low_random = expected.next();
        let current = state.map.get(north).expect("north NewGRF house");
        let mut generation_rng = Some(&mut actual);
        tile_loop_house(&mut state, 0, north, current, &mut generation_rng);

        let expected_frame = u8::try_from(shared_random & 0xFF).unwrap_or(0);
        assert_eq!(actual, expected);
        assert_eq!(
            state.map.get(north).expect("north after loop").m7,
            expected_frame
        );
        assert_eq!(
            state.map.get(east).expect("east after loop").m7,
            expected_frame
        );
        assert_eq!(state.active_house_animations, vec![north, east]);
    }

    #[test]
    fn newgrf_house_tile_loop_rerandomises_action2_bits_and_keeps_unmatched_trigger() {
        let id = crate::house_spec::NEW_HOUSE_OFFSET;
        let coord = TileCoord::new(1, 0);
        let mut map = Map::new_flat(2, 2, 0);
        let mut tile = crate::map::Tile::town_house(
            crate::map::TownHouseSpec {
                house_id: id,
                town_id: 0,
                random_bits: 0xA4,
                construction_counter: 0,
                construction_stage: crate::map::TOWN_HOUSE_COMPLETED,
                is_protected: false,
                processing_time: 0,
            },
            0,
            0,
        );
        tile.m6 |= 0x03;
        map.set_tile(coord, tile).expect("NewGRF house inside map");
        let mut state = GameState::from_map(map);
        state.house_spec_catalog.push(newgrf_runtime_house(
            id,
            0,
            crate::house_spec::BUILDING_FLAG_SIZE_1X1,
            5,
            house_random_runtime(&[0], HOUSE_RANDOM_TRIGGER_TILE_LOOP),
        ));

        let mut actual = Randomizer::new(42);
        let mut expected = actual;
        let tile_loop_random = expected.next();
        let _tile_loop_north_random = expected.next();
        let _shared_synchronised_cb1b_random = expected.next();
        let current = state.map.get(coord).expect("NewGRF house");
        let mut generation_rng = Some(&mut actual);
        tile_loop_house(&mut state, 0, coord, current, &mut generation_rng);

        let updated = state.map.get(coord).expect("NewGRF house after loop");
        assert_eq!(actual, expected);
        assert_eq!(
            updated.m1,
            (0xA4 & !1) | (u8::try_from(tile_loop_random & 1).unwrap_or(0))
        );
        assert_eq!(updated.m3 & 0x1F, HOUSE_RANDOM_TRIGGER_TILE_LOOP_NORTH);
        assert_eq!(updated.m6, 0x17, "timer 5 + AnimatedTileState 3");
        assert!(state.runtime.landscape_tile_dirty.contains(&coord));
    }

    #[test]
    fn newgrf_house_tile_loop_randomisation_writes_town_parent_persistent_storage() {
        let id = crate::house_spec::NEW_HOUSE_OFFSET;
        let coord = TileCoord::new(1, 0);
        let mut map = Map::new_flat(2, 2, 0);
        map.set_tile(
            coord,
            crate::map::Tile::town_house(
                crate::map::TownHouseSpec {
                    house_id: id,
                    town_id: 7,
                    random_bits: 0,
                    construction_counter: 0,
                    construction_stage: crate::map::TOWN_HOUSE_COMPLETED,
                    is_protected: false,
                    processing_time: 0,
                },
                0,
                0,
            ),
        )
        .expect("NewGRF house inside map");
        let mut state = GameState::from_map(map);
        state.towns.push(crate::town::Town {
            id: 7,
            pos: coord,
            ..Default::default()
        });

        // Bit alto de `shift`: selector interno de scope parent Action2.
        let parent_scope = 0x80;
        let parent_literal = |value: u32| crate::newgrf_sprites::Action2VarTerm {
            variable: 0x1A,
            param: None,
            adjust: crate::newgrf_sprites::Action2VarAdjust {
                shift: parent_scope,
                and_mask: value,
                ..Default::default()
            },
        };
        let mut runtime = house_random_runtime(&[0], HOUSE_RANDOM_TRIGGER_TILE_LOOP);
        runtime.action2_var.insert(
            0,
            crate::newgrf_sprites::Action2VarEntry {
                first: parent_literal(42),
                ops: vec![crate::newgrf_sprites::Action2VarOp {
                    operator: 0x10, // `\\2psto`
                    rhs: parent_literal(5),
                }],
                ranges: Vec::new(),
                default: 0,
            },
        );
        state.house_spec_catalog.push(newgrf_runtime_house(
            id,
            0,
            crate::house_spec::BUILDING_FLAG_SIZE_1X1,
            1,
            runtime,
        ));

        let mut rng = Randomizer::new(42);
        let current = state.map.get(coord).expect("NewGRF house");
        let mut generation_rng = Some(&mut rng);
        tile_loop_house(&mut state, 0, coord, current, &mut generation_rng);

        assert_eq!(
            state.towns[0]
                .newgrf_persistent_regs
                .get(&1)
                .and_then(|registers| registers.get(&5)),
            Some(&42)
        );
    }

    #[test]
    fn newgrf_house_without_action3_group_rearms_after_shared_cb1b_word() {
        let id = crate::house_spec::NEW_HOUSE_OFFSET;
        let coord = TileCoord::new(1, 0);
        let mut map = Map::new_flat(2, 2, 0);
        map.set_tile(
            coord,
            crate::map::Tile::town_house(
                crate::map::TownHouseSpec {
                    house_id: id,
                    town_id: 0,
                    random_bits: 0xA4,
                    construction_counter: 0,
                    construction_stage: crate::map::TOWN_HOUSE_COMPLETED,
                    is_protected: false,
                    processing_time: 0,
                },
                0,
                0,
            ),
        )
        .expect("NewGRF house inside map");
        let mut state = GameState::from_map(map);
        let mut runtime = house_random_runtime(&[0], HOUSE_RANDOM_TRIGGER_TILE_LOOP);
        runtime.assigns.clear();
        state.house_spec_catalog.push(newgrf_runtime_house(
            id,
            0,
            crate::house_spec::BUILDING_FLAG_SIZE_1X1,
            2,
            runtime,
        ));

        let mut actual = Randomizer::new(42);
        let current = state.map.get(coord).expect("NewGRF house");
        let mut generation_rng = Some(&mut actual);
        tile_loop_house(&mut state, 0, coord, current, &mut generation_rng);

        let updated = state.map.get(coord).expect("NewGRF house after loop");
        let mut expected = Randomizer::new(42);
        let _shared_synchronised_cb1b_random = expected.next();
        assert_eq!(actual, expected);
        assert_eq!(updated.m1, 0xA4);
        assert_eq!(updated.m3 & 0x1F, 0);
        assert_eq!(updated.m6, 2 << 2);
        assert!(state.runtime.landscape_tile_dirty.contains(&coord));
    }

    #[test]
    fn newgrf_house_tile_loop_north_propagates_shared_random_bits_to_multitile_parts() {
        let id = crate::house_spec::NEW_HOUSE_OFFSET;
        let north = TileCoord::new(1, 1);
        let east = TileCoord::new(2, 1);
        let mut map = Map::new_flat(4, 3, 0);
        let north_tile = crate::map::Tile::town_house(
            crate::map::TownHouseSpec {
                house_id: id,
                town_id: 0,
                random_bits: 0xA4,
                construction_counter: 0,
                construction_stage: crate::map::TOWN_HOUSE_COMPLETED,
                is_protected: false,
                processing_time: 0,
            },
            0,
            0,
        );
        let east_tile = crate::map::Tile::town_house(
            crate::map::TownHouseSpec {
                house_id: id + 1,
                town_id: 0,
                random_bits: 0x52,
                construction_counter: 0,
                construction_stage: crate::map::TOWN_HOUSE_COMPLETED,
                is_protected: false,
                processing_time: 0,
            },
            0,
            0,
        );
        map.set_tile(north, north_tile)
            .expect("north house inside map");
        map.set_tile(east, east_tile)
            .expect("east house inside map");
        let mut state = GameState::from_map(map);
        let runtime = house_random_runtime(&[0, 1], HOUSE_RANDOM_TRIGGER_TILE_LOOP_NORTH);
        state.house_spec_catalog.extend([
            newgrf_runtime_house(
                id,
                0,
                crate::house_spec::BUILDING_FLAG_SIZE_2X1,
                0,
                runtime.clone(),
            ),
            newgrf_runtime_house(id + 1, 1, 0, 0, runtime),
        ]);

        let mut actual = Randomizer::new(42);
        let mut expected = actual;
        let _tile_loop_random = expected.next();
        let shared_random = expected.next();
        let _part_random = expected.next();
        let _shared_synchronised_cb1b_random = expected.next();
        let current = state.map.get(north).expect("north house");
        let mut generation_rng = Some(&mut actual);
        tile_loop_house(&mut state, 0, north, current, &mut generation_rng);

        let north_updated = state.map.get(north).expect("north house after loop");
        let east_updated = state.map.get(east).expect("east house after loop");
        let shared_bit = u8::try_from(shared_random & 1).unwrap_or(0);
        assert_eq!(actual, expected);
        assert_eq!(north_updated.m1 & 1, shared_bit);
        assert_eq!(east_updated.m1 & 1, shared_bit);
        assert_eq!(north_updated.m3 & 0x1F, HOUSE_RANDOM_TRIGGER_TILE_LOOP);
        assert_eq!(east_updated.m3 & 0x1F, 0);
        assert!(state.runtime.landscape_tile_dirty.contains(&north));
        assert!(state.runtime.landscape_tile_dirty.contains(&east));
    }

    #[test]
    fn town_rebuild_uses_the_house_random_without_an_extra_chance_roll() {
        let mut map = Map::new_flat(16, 16, 0);
        let coord = TileCoord::new(8, 8);
        let house = HouseSpec::get(6).expect("vanilla house");
        map.set_completed_house(coord, house.id, u8::MAX).unwrap();
        map.set_house_town_id(coord, 7).unwrap();
        let mut state = GameState::from_map(map);
        state.towns.push(crate::town::Town {
            id: 7,
            pos: coord,
            population: u32::from(house.population),
            num_houses: 1,
            is_growing: true,
            time_until_rebuild: 1,
            ..Default::default()
        });
        let mut rng = Randomizer::new(42);
        let before_rng = rng;
        let current = state.map.get(coord).expect("house tile");

        // En `TileLoop_Town`, el byte alto menor que 12 borra la casa y deja
        // el solar vacío. El byte 16--23 programa el próximo intento. No hay
        // otro `Chance16` ni `RandomRange` después de `r = Random()`.
        maybe_rebuild_vanilla_town_house(&mut state, coord, current, house, 0x0001_0000, &mut rng);

        let cleared = state.map.get(coord).expect("cleared tile");
        assert_eq!(cleared.kind, TileKind::Grass);
        assert_eq!(cleared.m1, crate::company::OWNER_NONE_M1);
        assert_eq!(
            cleared.m5,
            crate::world_gen::clear_ground_m5(crate::world_gen::CLEAR_GROUND_GRASS, 3)
        );
        assert_eq!(state.towns[0].time_until_rebuild, 193);
        assert_eq!(state.towns[0].population, 0);
        assert_eq!(state.towns[0].num_houses, 0);
        assert_eq!(rng, before_rng, "no debe sortear una segunda chance");
    }

    #[test]
    fn landscape_river_loops_mark_stable_water() {
        let mut map = Map::new_flat(64, 64, 0);
        for y in 0..64 {
            for x in 0..64 {
                let c = TileCoord::new(x, y);
                map.set_kind(c, TileKind::Water).unwrap();
                map.set_mapt_m5(c, 0x60, 0).unwrap();
                map.set_m1(c, set_water_class_m1(0x11, WaterClass::Sea))
                    .unwrap();
            }
        }

        run_landscape_river_tile_loops(&mut map, crate::world_gen::Climate::Temperate, 42);

        let stable = map
            .tiles()
            .iter()
            .filter(|tile| tile.kind == TileKind::Water && (tile.m3 & 1) != 0)
            .count();
        // Con el contador de tick fijo en cero, tile 0 ocupa una de las 16
        // visitas de cada pasada. OpenTTD por ello alcanza 1 + 15×256
        // posiciones LFSR, no un barrido completo de 4096 teselas.
        assert_eq!(stable, 1 + 15 * 256);
    }
}
