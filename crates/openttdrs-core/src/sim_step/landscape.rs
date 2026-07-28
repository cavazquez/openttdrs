//! `CallLandscapeTick` — orden `OpenTTD`: town → trees → station → industry → companies → linkgraph.

use crate::flow_stat::{DistributionType, StationFlows};
use crate::linkgraph_parity::{build_jobs_from_game, run_full_pipeline, to_station_flows_helper};
use crate::{GameState, station};

/// Tick de economía en el que se spawnean/unen jobs del linkgraph (`SPAWN_JOIN_TICK`).
pub const LINKGRAPH_SPAWN_JOIN_TICK: u16 = 21;

/// `CallLandscapeTick` (`landscape.cpp:1727-1740`).
pub(super) fn call_landscape_tick(state: &mut GameState, t: u64) {
    on_tick_town(state, t);
    on_tick_trees(state);
    on_tick_water(state);
    on_tick_station(state, t);
    on_tick_industry(state, t);
    on_tick_companies(state, t);
    on_tick_link_graph(state);
}

/// `OnTick_Town`: demanda urbana y crecimiento.
fn on_tick_town(state: &mut GameState, t: u64) {
    super::economy::produce_town_demand(state, t);
    super::economy::grow_towns(state, t);
    // Renovación en visitas del tile loop (P3.6).
    let visit_coords: Vec<_> = state
        .runtime
        .tile_loop_visited
        .iter()
        .map(|(c, _)| *c)
        .collect();
    let dirty = crate::town::tile_loop_town_house_renovation(
        &mut state.map,
        &mut state.towns,
        &visit_coords,
        state.climate,
        state.calendar.year,
        &state.house_spec_catalog,
        &state.house_overrides,
        &mut state.random,
    );
    state.runtime.landscape_tile_dirty.extend(dirty);
}

/// `OnTick_Trees`: ciclo de vegetación sobre las visitas del tile loop.
fn on_tick_trees(state: &mut GameState) {
    crate::map::tree_tile_loop::tick_tree_tile_loop(state);
}

/// Inundación desde agua (`TileLoop_Water` / P3.2) sobre las visitas del tile loop.
fn on_tick_water(state: &mut GameState) {
    crate::map::water_flood::tick_water_flood(state);
}

/// `OnTick_Station`: rating (ciclo 185 ticks ≈ `STATION_ACCEPTANCE_TICKS` del port).
fn on_tick_station(state: &mut GameState, t: u64) {
    if t > 0 && t.is_multiple_of(u64::from(crate::economy::STATION_RATING_TICKS)) {
        station::update_station_ratings(
            &mut state.stations,
            state.order.selectgoods,
            &mut state.random,
        );
    }
}

/// `OnTick_Industry`: producción y cambio diario de nivel.
fn on_tick_industry(state: &mut GameState, t: u64) {
    super::economy::produce_industries(state, t);
    if state.runtime.calendar_triggers.new_day {
        super::economy::maybe_change_industry_production(state);
    }
}

/// `OnTick_Companies`: rivales / `GameScript` (hooks del port).
fn on_tick_companies(state: &mut GameState, t: u64) {
    crate::ai::tick_ai_companies(state, t);
    crate::gs::tick_gs(state);
    crate::subsidy::tick_subsidies(state);
    crate::disaster::tick_disasters(state);
}

/// `OnTick_LinkGraph` (P2.21) — jobs síncronos sobre copia del grafo cuando
/// `economy_timer.date_fract == 21`, con cadencia `recalc_interval_days`.
fn on_tick_link_graph(state: &mut GameState) {
    if state.economy_timer.date_fract != LINKGRAPH_SPAWN_JOIN_TICK {
        return;
    }
    let interval = state.cargo_dist.recalc_interval_days.max(1);
    let offset = state.economy_timer.date % interval;
    // OpenTTD: offset==0 → SpawnNext; offset==interval/2 → JoinNext.
    // Aquí ambos ejecutan el MCF síncrono sobre una copia del grafo.
    if offset == 0 || offset == interval / 2 {
        // Copia observacional: el pipeline no muta estaciones ni el grafo en vivo.
        let stations = state.stations.clone();
        let link_graph = state.link_graph.clone();
        let distribution = state.cargo_dist.distribution;
        let (map_w, map_h) = state.map.dimensions();
        if matches!(distribution, DistributionType::Manual) {
            state.runtime.station_flows = StationFlows::default();
            return;
        }
        let jobs = build_jobs_from_game(&stations, &link_graph, distribution, map_w, map_h);
        let mut merged = StationFlows::default();
        for (cargo, mut job) in jobs {
            run_full_pipeline(&mut job);
            let part = to_station_flows_helper(&job, cargo);
            for (station_tile, table) in part.by_station {
                let dest = merged.by_station.entry(station_tile).or_default();
                for (c, map) in table.by_cargo {
                    let dest_map = dest.by_cargo.entry(c).or_default();
                    for (origin, fs) in map.by_origin {
                        for (via, amount) in fs.shares {
                            dest_map.add_flow(origin, via, amount);
                        }
                    }
                }
            }
        }
        state.runtime.station_flows = merged;
    }
}
