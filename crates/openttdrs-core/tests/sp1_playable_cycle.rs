//! SP1: ciclo jugable industria → estación → vehículo → carga → pago (solo core).

#![allow(clippy::expect_used, clippy::unwrap_used)]

use openttdrs_core::{
    Command, GameState, Industry, IndustryKind, PathNetwork, TileCoord, Vehicle, VehicleKind,
    apply_command, find_path,
};

const DEMO_ROAD_Y: i32 = 6;
const DEMO_INDUSTRY: TileCoord = TileCoord::new(2, 3);
const DEMO_LOAD: TileCoord = TileCoord::new(3, DEMO_ROAD_Y - 1);
const DEMO_DELIVER: TileCoord = TileCoord::new(10, DEMO_ROAD_Y - 1);
const STATION_ENTRANCE_DIR: u8 = 1;

fn setup_demo_road_line(state: &mut GameState) {
    for x in 2..=12_i32 {
        apply_command(
            state,
            &Command::PlaceRoadBits(TileCoord::new(x, DEMO_ROAD_Y), 0x0A),
        )
        .expect("carretera horizontal demo");
    }
}

fn setup_demo_economy_via_commands(state: &mut GameState) {
    state
        .map
        .set_kind(DEMO_INDUSTRY, openttdrs_core::TileKind::Industry)
        .expect("mina");
    let mut mine = Industry::new(DEMO_INDUSTRY, IndustryKind::CoalMine);
    mine.stock = 64;
    state.industries.push(mine);

    apply_command(
        state,
        &Command::PlaceStationDir(DEMO_LOAD, STATION_ENTRANCE_DIR),
    )
    .expect("parada carga");
    apply_command(
        state,
        &Command::PlaceStationDir(DEMO_DELIVER, STATION_ENTRANCE_DIR),
    )
    .expect("parada descarga");

    let mut truck = Vehicle::new(9010, VehicleKind::Truck, DEMO_LOAD, DEMO_DELIVER);
    truck.running = true;
    truck.set_station_orders(vec![DEMO_LOAD, DEMO_DELIVER]);
    if let Some(path) = find_path(&state.map, DEMO_LOAD, DEMO_DELIVER, PathNetwork::Road) {
        truck.path = path.into();
    }
    state.vehicles.push(truck);
}

#[test]
fn sp1_truck_loads_at_mine_delivers_and_earns_income() {
    let mut state = GameState::new(24, 20);
    setup_demo_road_line(&mut state);
    setup_demo_economy_via_commands(&mut state);

    let money_before = state.economy.money;
    for _ in 0..800 {
        state.step();
    }

    assert!(
        state.stats.cargo_units_loaded > 0,
        "carga en parada junto a mina"
    );
    assert!(
        state.stats.cargo_units_delivered > 0,
        "entrega en segunda parada"
    );
    assert!(state.stats.cargo_income_earned > 0, "pago por entrega");
    assert!(
        state.economy.money > money_before - 5_000,
        "ingresos netos tras costes de explotación"
    );
}
