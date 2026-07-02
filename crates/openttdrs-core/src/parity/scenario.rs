//! Escenarios determinísticos para reproducir casos de paridad en headless.
//!
//! `truck_bay` reproduce el caso de los videos `openttd.webm` / `opentddrs.webm`:
//! un camión con órdenes de carga/descarga recorre una ruta con dos curvas de
//! 90° y entra a una playa de carga (`StopKind::TruckStop`, bahía no drive-through).
//!
//! `train_line` es el escenario ferroviario mínimo de la Fase Rail 1: un tren
//! sale de un depósito, recorre una L con una señal de bloque y una curva, y
//! cicla entre dos estaciones (carga en A, viaja a B).

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

/// Id del tren del escenario `train_line`.
pub const TRAIN_LINE_VEHICLE_ID: u32 = 1;
/// Plataforma de la estación A (carga; al oeste de la línea).
pub const TRAIN_LINE_STATION_A: TileCoord = TileCoord::new(1, 6);
/// Plataforma de la estación B (al final de la rama sur).
pub const TRAIN_LINE_STATION_B: TileCoord = TileCoord::new(13, 10);
/// Depósito ferroviario (boca hacia la línea, al sur).
pub const TRAIN_LINE_DEPOT: TileCoord = TileCoord::new(4, 5);
/// Tesela con la señal de bloque sobre la recta.
pub const TRAIN_LINE_SIGNAL: TileCoord = TileCoord::new(7, 6);
/// Esquina de la L (curva este→sur).
pub const TRAIN_LINE_CORNER: TileCoord = TileCoord::new(12, 6);

/// Construye un escenario determinístico por nombre.
#[must_use]
pub fn build_scenario(name: &str) -> Option<GameState> {
    match name {
        "truck_bay" => Some(build_truck_bay()),
        "train_line" => Some(build_train_line()),
        _ => None,
    }
}

/// Nombres de escenarios disponibles.
#[must_use]
pub fn scenario_names() -> &'static [&'static str] {
    &["truck_bay", "train_line"]
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

/// Mapa chico y plano con una L ferroviaria:
/// - vía de (2,6) a (12,6) por el eje X, esquina en (12,6) y rama sur hasta (12,10);
/// - estación A (1×1) en (1,6), acceso (2,6), con goods en stock para cargar;
/// - estación B (1×1) en (13,10), acceso (12,10);
/// - depósito en (4,5) con la boca hacia la línea (salida (4,6));
/// - señal de bloque en (7,6) sobre la recta;
/// - un tren (id 1) que arranca dentro del depósito con órdenes A ↔ B.
///
/// # Panics
///
/// Si la construcción del escenario fijo falla (bug del propio escenario).
#[must_use]
#[allow(clippy::expect_used)] // escenario fijo: un fallo de construcción es un bug del escenario
pub fn build_train_line() -> GameState {
    let mut state = GameState::new(20, 14);
    state.world_seed = 0;

    // Recta X y rama sur (autorail: los track bits se infieren de los vecinos).
    for x in 2..=12 {
        apply_command(&mut state, &Command::PlaceRail(TileCoord::new(x, 6)))
            .expect("vía recta train_line");
    }
    for y in 7..=10 {
        apply_command(&mut state, &Command::PlaceRail(TileCoord::new(12, y)))
            .expect("rama sur train_line");
    }

    // Direcciones diagonales OpenTTD: NE=0 (−x), SE=1 (+y), SW=2 (+x), NW=3 (−y).
    apply_command(
        &mut state,
        &Command::PlaceRailStation(TRAIN_LINE_STATION_A, 2),
    )
    .expect("estación A train_line");
    apply_command(
        &mut state,
        &Command::PlaceRailStation(TRAIN_LINE_STATION_B, 0),
    )
    .expect("estación B train_line");
    // Boca hacia +y: la salida del depósito empalma con la línea en (4,6).
    apply_command(&mut state, &Command::PlaceRailDepotDir(TRAIN_LINE_DEPOT, 1))
        .expect("depósito train_line");
    apply_command(
        &mut state,
        &Command::PlaceRailSignal(TRAIN_LINE_SIGNAL, 0, 128, 128),
    )
    .expect("señal train_line");

    // Stock de goods en la estación A para que el tren cargue al llegar.
    let station_a = state
        .stations
        .iter_mut()
        .find(|s| s.pos == TRAIN_LINE_STATION_A)
        .expect("estación A registrada");
    station_a.cargo_stock.goods = 40;

    let mut train = Vehicle::new(
        TRAIN_LINE_VEHICLE_ID,
        VehicleKind::Train,
        TRAIN_LINE_DEPOT,
        TRAIN_LINE_STATION_A,
    );
    train.set_vehicle_orders(vec![
        VehicleOrder::station(TRAIN_LINE_STATION_A),
        VehicleOrder::station(TRAIN_LINE_STATION_B),
    ]);
    train.sync_order_destination(&state.map);
    if let Some(path) = find_path(&state.map, train.pos, train.dest, PathNetwork::Rail) {
        train.path = VecDeque::from(path);
    }
    state.vehicles.push(train);

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
        assert!(build_scenario("train_line").is_some());
        assert_eq!(scenario_names(), &["truck_bay", "train_line"]);
    }

    #[test]
    fn train_line_layout_is_consistent() {
        use crate::station::rail_station_approach_tile;

        let state = build_train_line();
        assert_eq!(
            rail_station_approach_tile(&state.map, TRAIN_LINE_STATION_A),
            Some(TileCoord::new(2, 6)),
            "acceso a la estación A"
        );
        assert_eq!(
            rail_station_approach_tile(&state.map, TRAIN_LINE_STATION_B),
            Some(TileCoord::new(12, 10)),
            "acceso a la estación B"
        );
        assert_eq!(state.vehicles.len(), 1);
        assert_eq!(state.stations.len(), 2);
        assert_eq!(state.vehicles[0].kind, VehicleKind::Train);
        assert!(
            !state.vehicles[0].path.is_empty(),
            "el tren arranca con ruta desde el depósito"
        );
        // La línea conecta depósito → acceso A → esquina → acceso B por vía.
        let a_to_b = find_path(
            &state.map,
            TileCoord::new(2, 6),
            TileCoord::new(12, 10),
            PathNetwork::Rail,
        )
        .expect("ruta ferroviaria A → B");
        assert!(a_to_b.contains(&TRAIN_LINE_SIGNAL), "pasa por la señal");
        assert!(a_to_b.contains(&TRAIN_LINE_CORNER), "pasa por la esquina");
        assert!(
            find_path(
                &state.map,
                TRAIN_LINE_DEPOT,
                TileCoord::new(2, 6),
                PathNetwork::Rail
            )
            .is_some(),
            "el depósito conecta con la línea"
        );
        // La señal quedó colocada sobre la recta.
        let signal_tile = state.map.get(TRAIN_LINE_SIGNAL).unwrap();
        assert!(crate::rail_signals::rail_tile_is_signals(signal_tile.m5));
    }
}
