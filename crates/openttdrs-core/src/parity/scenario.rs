//! Escenarios determinísticos para reproducir casos de paridad en headless.
//!
//! `truck_bay` reproduce el caso de los videos `openttd.webm` / `opentddrs.webm`:
//! un camión con órdenes de carga/descarga recorre una ruta con dos curvas de
//! 90° y entra a una playa de carga (`StopKind::TruckStop`, bahía no drive-through).

use std::collections::VecDeque;

use crate::command::{Command, apply_command};
use crate::industry::{Industry, IndustryKind};
use crate::map::TileCoord;
use crate::vehicle::{Vehicle, VehicleKind, VehicleOrder};
use crate::{GameState, PathNetwork, find_path};

/// Tesela de la parada de carga (bahía camión) del escenario `truck_bay`.
pub const TRUCK_BAY_LOAD_STOP: TileCoord = TileCoord::new(4, 5);
/// Carretera de acceso a la parada de carga.
pub const TRUCK_BAY_LOAD_ROAD: TileCoord = TileCoord::new(4, 6);
/// Tesela de la parada de descarga.
pub const TRUCK_BAY_DELIVER_STOP: TileCoord = TileCoord::new(16, 11);
/// Carretera de acceso a la parada de descarga.
pub const TRUCK_BAY_DELIVER_ROAD: TileCoord = TileCoord::new(16, 12);
/// Id del camión del escenario.
pub const TRUCK_BAY_VEHICLE_ID: u32 = 1;

/// Construye un escenario determinístico por nombre (`truck_bay`).
#[must_use]
pub fn build_scenario(name: &str) -> Option<GameState> {
    match name {
        "truck_bay" => Some(build_truck_bay()),
        _ => None,
    }
}

/// Nombres de escenarios disponibles.
#[must_use]
pub fn scenario_names() -> &'static [&'static str] {
    &["truck_bay"]
}

fn place_road_polyline(state: &mut GameState, waypoints: &[TileCoord]) -> Vec<TileCoord> {
    let mut tiles = Vec::new();
    for pair in waypoints.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let dx = (b.x - a.x).signum();
        let dy = (b.y - a.y).signum();
        let mut c = a;
        loop {
            if tiles.last() != Some(&c) {
                tiles.push(c);
            }
            if c == b {
                break;
            }
            c = TileCoord::new(c.x + dx, c.y + dy);
        }
    }
    for pair in tiles.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let bits_a = road_bits_toward(a, b);
        let bits_b = road_bits_toward(b, a);
        apply_command(state, &Command::PlaceRoadBits(a, bits_a)).ok();
        apply_command(state, &Command::PlaceRoadBits(b, bits_b)).ok();
    }
    tiles
}

/// `RoadBits` hacia la tesela vecina (NW=1, SW=2, SE=4, NE=8).
const fn road_bits_toward(from: TileCoord, to: TileCoord) -> u8 {
    match (to.x - from.x, to.y - from.y) {
        (-1, 0) => 0x08,
        (0, -1) => 0x01,
        (1, 0) => 0x02,
        (0, 1) => 0x04,
        _ => 0,
    }
}

/// Mapa chico y plano con:
/// - ruta de carretera con dos curvas de 90°: (4,6)→(10,6)→(10,12)→(16,12);
/// - bahía de carga `TruckStop` en (4,5) con acceso (4,6) y mina de carbón cerca;
/// - parada de descarga `TruckStop` en (16,11) con acceso (16,12);
/// - un camión (id 1) con órdenes circulares carga → descarga.
///
/// # Panics
///
/// Si la construcción del escenario fijo falla (bug del propio escenario).
#[must_use]
#[allow(clippy::expect_used)] // escenario fijo: un fallo de construcción es un bug del escenario
pub fn build_truck_bay() -> GameState {
    let mut state = GameState::new(24, 18);
    state.world_seed = 0;

    place_road_polyline(
        &mut state,
        &[
            TileCoord::new(4, 6),
            TileCoord::new(10, 6),
            TileCoord::new(10, 12),
            TileCoord::new(16, 12),
        ],
    );

    // Paradas bahía: dir 1 (SE) → la boca mira a la carretera en y+1.
    apply_command(&mut state, &Command::PlaceTruckStop(TRUCK_BAY_LOAD_STOP, 1))
        .expect("parada de carga truck_bay");
    apply_command(
        &mut state,
        &Command::PlaceTruckStop(TRUCK_BAY_DELIVER_STOP, 1),
    )
    .expect("parada de descarga truck_bay");

    // Mina de carbón en cobertura de la parada de carga, con stock inicial.
    let mut mine = Industry::new(TileCoord::new(3, 3), IndustryKind::CoalMine);
    mine.stock = 60;
    state.industries.push(mine);

    let mut truck = Vehicle::new(
        TRUCK_BAY_VEHICLE_ID,
        VehicleKind::Truck,
        TRUCK_BAY_DELIVER_ROAD,
        TRUCK_BAY_LOAD_STOP,
    );
    // Carga completa en la mina (como en el video de referencia: el camión
    // espera en la bahía hasta llenar) y descarga normal en destino.
    truck.set_vehicle_orders(vec![
        VehicleOrder::station_with_flags(TRUCK_BAY_LOAD_STOP, true, false),
        VehicleOrder::station(TRUCK_BAY_DELIVER_STOP),
    ]);
    truck.sync_order_destination(&state.map);
    if let Some(path) = find_path(&state.map, truck.pos, truck.dest, PathNetwork::Road) {
        truck.path = VecDeque::from(path);
    }
    state.vehicles.push(truck);

    state
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::station::road_stop_approach_tile;

    #[test]
    fn truck_bay_layout_is_consistent() {
        let state = build_truck_bay();
        assert_eq!(
            road_stop_approach_tile(&state.map, TRUCK_BAY_LOAD_STOP),
            Some(TRUCK_BAY_LOAD_ROAD)
        );
        assert_eq!(
            road_stop_approach_tile(&state.map, TRUCK_BAY_DELIVER_STOP),
            Some(TRUCK_BAY_DELIVER_ROAD)
        );
        assert_eq!(state.vehicles.len(), 1);
        assert_eq!(state.stations.len(), 2);
        // La ruta calculada existe y contiene las dos esquinas de la L.
        let path = find_path(
            &state.map,
            TRUCK_BAY_LOAD_ROAD,
            TRUCK_BAY_DELIVER_ROAD,
            PathNetwork::Road,
        )
        .expect("ruta por carretera");
        assert!(path.contains(&TileCoord::new(10, 6)));
        assert!(path.contains(&TileCoord::new(10, 12)));
    }

    #[test]
    fn unknown_scenario_returns_none() {
        assert!(build_scenario("nope").is_none());
        assert!(build_scenario("truck_bay").is_some());
        assert_eq!(scenario_names(), &["truck_bay"]);
    }
}
