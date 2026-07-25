//! `CallLandscapeTick` — orden `OpenTTD`: town → trees → station → industry → companies → linkgraph.

use crate::{GameState, station};

/// `CallLandscapeTick` (`landscape.cpp:1727-1740`).
pub(super) fn call_landscape_tick(state: &mut GameState, t: u64) {
    on_tick_town(state, t);
    on_tick_trees(state);
    on_tick_station(state, t);
    on_tick_industry(state, t);
    on_tick_companies(state, t);
    on_tick_link_graph(state);
}

/// `OnTick_Town`: demanda urbana y crecimiento.
fn on_tick_town(state: &mut GameState, t: u64) {
    super::economy::produce_town_demand(state, t);
    super::economy::grow_towns(state, t);
}

/// `OnTick_Trees`: ciclo de vegetación sobre las visitas del tile loop.
fn on_tick_trees(state: &mut GameState) {
    crate::map::tree_tile_loop::tick_tree_tile_loop(state);
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

/// `OnTick_LinkGraph` — disparo real en P2.21 (`date_fract == 21`).
fn on_tick_link_graph(_state: &mut GameState) {
    // Stub: el planificador asíncrono/síncrono se cablea en P2.21.
}
