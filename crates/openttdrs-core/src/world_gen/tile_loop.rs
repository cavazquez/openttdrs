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
use crate::map::{Map, TileKind, collect_tile_loop_visits};

/// Cantidad de pasadas que `CreateRivers` ejecuta tras ensanchar ríos.
///
/// Es `TILE_UPDATE_FREQUENCY` en `landscape.cpp`; en ese intervalo cada
/// tesela recibe una visita LFSR y el agua estable queda marcada como
/// `non-flooding` en `MAP3` bit 0.
pub const LANDSCAPE_RIVER_TILE_LOOP_PASSES: u64 = 256;

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
        let tile = state.map.get(*coord).unwrap_or(*snapshot);
        match tile.kind {
            // TileLoop_Clear y TileLoop_Trees comparten el crecimiento de
            // hierba/campos y la actualización de árboles. El helper ya
            // vuelve a leer la tesela viva después de una transición desértica.
            TileKind::Forest
                if state.climate == crate::world_gen::Climate::Temperate
                    && generation_rng.is_some() =>
            {
                // `TileLoop_Trees` delega primero una orilla a
                // `TileLoop_Water`. El callback puede cambiar la tesela, por
                // lo que el procesador de árbol la vuelve a leer del mapa.
                if (tile.m2 >> 6) & 0x07 == 3 {
                    crate::map::water_flood::tile_loop_water_at(state, *coord, tile);
                }
                if let Some(rng) = generation_rng.as_deref_mut() {
                    crate::map::tree_tile_loop::process_generation_tree_growth_at(
                        &mut state.map,
                        state.climate,
                        tick,
                        rng,
                        *coord,
                    );
                }
            }
            TileKind::Grass | TileKind::Forest | TileKind::CoalField => {
                crate::map::tree_tile_loop::process_tree_and_field_growth_from_visits(
                    &mut state.map,
                    tick,
                    state.world_seed,
                    &[(*coord, tile)],
                );
            }
            // TileLoop_Water es el único callback aplicable a MP_WATER y no
            // debe tratar los bordes MP_VOID como tiles válidos al inundar.
            TileKind::Water => {
                crate::map::water_flood::tile_loop_water_at(state, *coord, tile);
            }
            // TileLoop_Industry primero deja que una industria sobre agua
            // intente inundar, luego ejecuta randomización, obra y animación.
            // Las tres operaciones consumen exactamente la visita actual y no
            // barren de nuevo el mapa.
            TileKind::Industry => {
                if crate::map::industry_terrain::industry_tile_on_water(tile) {
                    crate::map::water_flood::tile_loop_water_at(state, *coord, tile);
                }
                let one = [(*coord, tile)];
                let _ = crate::map::industry_random::
                    advance_industry_tile_randomisation_from_visits_with_catalog(
                        &mut state.map,
                        tick,
                        state.world_seed,
                        &one,
                        &state.industries,
                        &state.towns,
                        &state.industry_tile_spec_catalog,
                        &state.industry_spec_catalog,
                        state.climate,
                    );
                let _ =
                    crate::map::industry_construction::advance_industry_construction_from_visits(
                        &mut state.map,
                        &one,
                        &state.industries,
                    );
                let _ =
                    crate::map::industry_tile_anim::advance_industry_tile_loop_events_from_visits(
                        &mut state.map,
                        tick,
                        &one,
                    );
            }
            // Casas, carreteras, vías, estaciones, objetos y depósitos tienen
            // callbacks de tile loop propios en OpenTTD. Para un mundo nuevo
            // sus rutas de estado todavía no tienen una mutación equivalente;
            // dejar explícito el no-op evita ejecutar lógica de economía del
            // tick normal y mantiene el contrato de la pasada.
            TileKind::House
            | TileKind::Road
            | TileKind::Rail
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

    // El último conjunto queda disponible para herramientas de diagnóstico,
    // igual que después de `phase_tile_loop` en la simulación normal.
    state.runtime.tile_loop_visited = visits;
    visit_count
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
    // Mover en vez de clonar evita duplicar un mapa de hasta 4096² teselas
    // durante su creación. El placeholder no se observa: se reemplaza por el
    // mapa generado antes de devolver.
    let landscape = std::mem::replace(map, Map::new_flat(0, 0, 0));
    let mut state = GameState::from_map(landscape);
    state.climate = climate;
    state.world_seed = seed;
    for _ in 0..LANDSCAPE_RIVER_TILE_LOOP_PASSES {
        // `CreateRivers` no incrementa `TimerGameTick::counter` dentro de
        // este bucle. Usar siempre cero conserva tanto el callback manual de
        // tile 0 como cualquier regla dependiente del tick.
        run_generation_tile_loop_impl(&mut state, 0, Some(&mut *rng));
    }
    *map = state.map;
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
