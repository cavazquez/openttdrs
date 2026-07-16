//! Escenarios de economía / pueblo / averías.

use crate::command::{Command, apply_command};
use crate::map::TileCoord;
use crate::GameState;

pub fn build_loan_interest() -> GameState {
    let mut state = GameState::new(8, 8);
    state.economy.loan = 100_000;
    state.economy.money = 50_000;
    state.ensure_companies();
    state
}

/// Fase 4 / #86: mina+fábrica + bosque + pozo (3 rutas; L en madera/petróleo).
#[must_use]
#[allow(clippy::expect_used)]

pub fn build_town_growth() -> GameState {
    use crate::map::TileKind;
    use crate::station::StopKind;
    use crate::town::Town;

    let mut state = GameState::new(32, 32);
    let town_pos = TileCoord::new(10, 10);
    state.towns.push(Town {
        id: 1,
        pos: town_pos,
        name: "Parityville".into(),
        population: 120,
        ..Default::default()
    });
    state
        .map
        .set_kind(TileCoord::new(9, 10), TileKind::House)
        .ok();
    state
        .map
        .set_kind(TileCoord::new(10, 9), TileKind::House)
        .ok();
    apply_command(
        &mut state,
        &Command::PlaceBusStop(TileCoord::new(11, 10), 0),
    )
    .ok();
    state.stations[0].stop_kind = StopKind::BusStop;
    state
}

/// Tren con fiabilidad baja para forzar avería en headless.
pub fn build_breakdown() -> GameState {
    let mut state = GameState::new(16, 16);
    apply_command(&mut state, &Command::PlaceRail(TileCoord::new(2, 2))).ok();
    apply_command(&mut state, &Command::PlaceRail(TileCoord::new(3, 2))).ok();
    apply_command(&mut state, &Command::PlaceRailDepot(TileCoord::new(1, 2))).ok();
    apply_command(
        &mut state,
        &Command::BuildVehicleAtDepot(TileCoord::new(1, 2), crate::ENGINE_TRAIN_KIRBY),
    )
    .ok();
    if let Some(v) = state.vehicles.first_mut() {
        v.reliability = 1;
        v.running = true;
        v.cur_speed = 40;
        v.depot_leave_cleared = true;
    }
    state
}
