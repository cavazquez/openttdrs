//! Escenarios determinísticos para reproducir casos de paridad en headless.
//!
//! `truck_bay` reproduce el caso de los videos `openttd.webm` / `opentddrs.webm`:
//! un camión con órdenes de carga/descarga recorre una ruta con dos curvas de
//! 90° y entra a una playa de carga (`StopKind::TruckStop`, bahía no drive-through).
//!
//! `train_line` es el escenario ferroviario mínimo de la Fase Rail 1: un tren
//! sale de un depósito, recorre una L con una señal de bloque y una curva, y
//! cicla entre dos estaciones (carga en A, viaja a B).
//!
//! `train_signal` (Fase Rail 3D): dos trenes y una señal de bloque en línea
//! recta; el tren líder espera hasta que el bloque se libera.
//!
//! `train_supply`: mina → estación A → señales → estación B → fábrica.
//!
//! `train_supply_signal`: igual con un tren bloqueador para demostrar espera en señal.

use std::collections::VecDeque;

use crate::cargo::CargoType;
use crate::command::{Command, apply_command};
use crate::industry::{Industry, IndustryKind};
use crate::map::TileCoord;
use crate::rail_signals::{
    SIGTYPE_BLOCK, SIGTYPE_COMBO, SIGTYPE_ENTRY, SIGTYPE_EXIT, SIGTYPE_PATH, SIGTYPE_PATH_ONEWAY,
};
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
/// Plataforma de la estación B (al final de la rama sur; boca al norte).
pub const TRAIN_LINE_STATION_B: TileCoord = TileCoord::new(12, 10);
/// Depósito ferroviario (boca hacia la línea, al sur).
pub const TRAIN_LINE_DEPOT: TileCoord = TileCoord::new(4, 5);
/// Tesela con la señal de bloque sobre la recta.
pub const TRAIN_LINE_SIGNAL: TileCoord = TileCoord::new(7, 6);
/// Esquina de la L (curva este→sur).
pub const TRAIN_LINE_CORNER: TileCoord = TileCoord::new(12, 6);

/// Id del tren líder del escenario `train_signal`.
pub const TRAIN_SIGNAL_LEAD_ID: u32 = 1;
/// Id del tren que ocupa el bloque en `train_signal`.
pub const TRAIN_SIGNAL_BLOCKER_ID: u32 = 2;
/// Señal de bloque bidireccional en `train_signal`.
pub const TRAIN_SIGNAL_TILE: TileCoord = TileCoord::new(2, 0);
/// Tesela ocupada por el tren bloqueador.
pub const TRAIN_SIGNAL_BLOCK_TILE: TileCoord = TileCoord::new(4, 0);

/// Id del tren del escenario `train_supply`.
pub const TRAIN_SUPPLY_VEHICLE_ID: u32 = 1;
/// Mina de carbón en cobertura de la estación A.
pub const TRAIN_SUPPLY_MINE: TileCoord = TileCoord::new(0, 4);
/// Fábrica en cobertura de la estación B (consume carbón entregado en la estación).
pub const TRAIN_SUPPLY_FACTORY: TileCoord = TileCoord::new(14, 8);
/// Señales sobre la recta este (oeste → este).
pub const TRAIN_SUPPLY_SIGNAL_WEST: TileCoord = TileCoord::new(5, 6);
pub const TRAIN_SUPPLY_SIGNAL_EAST: TileCoord = TileCoord::new(10, 6);
/// Señal en la rama sur hacia la estación B.
pub const TRAIN_SUPPLY_SIGNAL_SOUTH: TileCoord = TileCoord::new(12, 8);
/// Señal donde el tren debe detenerse si el bloque este está ocupado.
pub const TRAIN_SUPPLY_WAIT_SIGNAL: TileCoord = TRAIN_LINE_SIGNAL;
/// Tren estacionado que bloquea el tramo entre las señales central y este.
pub const TRAIN_SUPPLY_BLOCKER_ID: u32 = 2;
pub const TRAIN_SUPPLY_BLOCK_TILE: TileCoord = TileCoord::new(8, 6);

/// Dos vías paralelas: ida y=6 (→este), vuelta y=4 (←oeste), cada una con un solo sentido.
pub const TRAIN_DUAL_TRACK_OUT_Y: i32 = 6;
pub const TRAIN_DUAL_TRACK_RET_Y: i32 = 4;
pub const TRAIN_DUAL_STATION_A: TileCoord = TileCoord::new(1, 6);
pub const TRAIN_DUAL_STATION_B: TileCoord = TileCoord::new(13, 6);
/// Depósito al sur de la vía de ida; boca hacia el norte (empalme en (4,6)).
pub const TRAIN_DUAL_DEPOT: TileCoord = TileCoord::new(4, 7);
/// Boca del depósito hacia la vía de ida (y=6).
pub const TRAIN_DUAL_DEPOT_EXIT: TileCoord = TileCoord::new(4, 6);
/// Mina de carbón visible al noroeste de la estación A.
pub const TRAIN_DUAL_COAL_MINE: TileCoord = TileCoord::new(0, 1);
/// Fábrica visible al noreste de la estación B.
pub const TRAIN_DUAL_FACTORY: TileCoord = TileCoord::new(14, 4);
pub const TRAIN_DUAL_VEHICLE_ID: u32 = 1;
pub const TRAIN_DUAL_VEHICLE_2_ID: u32 = 2;
/// Alias del tren líder del escenario dual (sonda `DevBot`).
pub const TRAIN_DUAL_VEHICLE_OUT_ID: u32 = TRAIN_DUAL_VEHICLE_ID;

/// Construye un escenario determinístico por nombre.
#[must_use]
pub fn build_scenario(name: &str) -> Option<GameState> {
    match name {
        "truck_bay" => Some(build_truck_bay()),
        "train_line" => Some(build_train_line()),
        "train_supply" => Some(build_train_supply()),
        "train_supply_dual" => Some(build_train_supply_dual()),
        "train_supply_signal" => Some(build_train_supply_signal_snapshot()),
        "train_signal" => Some(build_train_signal()),
        "rail_signals_mixed" => Some(build_rail_signals_mixed()),
        "loan_interest" => Some(build_loan_interest()),
        "town_growth" => Some(build_town_growth()),
        "breakdown" => Some(build_breakdown()),
        _ => None,
    }
}

/// Nombres de escenarios disponibles.
#[must_use]
pub fn scenario_names() -> &'static [&'static str] {
    &[
        "truck_bay",
        "train_line",
        "train_supply",
        "train_supply_dual",
        "train_supply_signal",
        "train_signal",
        "rail_signals_mixed",
        "loan_interest",
        "town_growth",
        "breakdown",
    ]
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
    for y in 7..=9 {
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
        &Command::PlaceRailStation(TRAIN_LINE_STATION_B, 3),
    )
    .expect("estación B train_line");
    // Boca hacia +y: la salida del depósito empalma con la línea en (4,6).
    apply_command(&mut state, &Command::PlaceRailDepotDir(TRAIN_LINE_DEPOT, 1))
        .expect("depósito train_line");
    apply_command(
        &mut state,
        &Command::PlaceRailSignal(TRAIN_LINE_SIGNAL, 0, 128, 128, SIGTYPE_BLOCK),
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

/// Cadena productor → consumidor por ferrocarril (mina de carbón → estación A →
/// señales → estación B junto a fábrica). Reutiliza la geometría de `train_line`
/// pero **no** precarga goods: la carga sale de la mina vía `try_load_from_industry`.
///
/// # Panics
///
/// Si la construcción del escenario fijo falla (bug del propio escenario).
#[must_use]
#[allow(clippy::expect_used)] // escenario fijo: un fallo de construcción es un bug del escenario
pub fn build_train_supply() -> GameState {
    build_train_supply_core()
}

/// Instantánea para el cliente: tren cargado detenido en la señal (7,6) con un
/// bloqueador en (8,6). Sirve para verificar visualmente la espera en señal.
///
/// # Panics
///
/// Si la construcción del escenario fijo falla (bug del propio escenario).
#[must_use]
#[allow(clippy::expect_used)]
pub fn build_train_supply_signal_snapshot() -> GameState {
    let mut state = build_train_supply_core();
    let train = state
        .vehicles
        .iter_mut()
        .find(|v| v.id == TRAIN_SUPPLY_VEHICLE_ID)
        .expect("tren train_supply");
    train.pos = TRAIN_SUPPLY_WAIT_SIGNAL;
    train.dest = TRAIN_LINE_STATION_B;
    train.cargo = 20;
    train.cargo_type = Some(CargoType::Coal);
    train.set_vehicle_orders(vec![VehicleOrder::station(TRAIN_LINE_STATION_B)]);
    train.current_order = 0;
    train.progress = 255;
    train.running = false;
    train.path.clear();

    let mut blocker = Vehicle::new(
        TRAIN_SUPPLY_BLOCKER_ID,
        VehicleKind::Train,
        TRAIN_SUPPLY_BLOCK_TILE,
        TRAIN_SUPPLY_BLOCK_TILE,
    );
    blocker.running = false;
    state.vehicles.push(blocker);
    state
}

#[allow(clippy::expect_used)]
fn build_train_supply_core() -> GameState {
    let mut state = GameState::new(20, 14);
    state.world_seed = 0;

    for x in 2..=12 {
        apply_command(&mut state, &Command::PlaceRail(TileCoord::new(x, 6)))
            .expect("vía recta train_supply");
    }
    for y in 7..=9 {
        apply_command(&mut state, &Command::PlaceRail(TileCoord::new(12, y)))
            .expect("rama sur train_supply");
    }

    apply_command(
        &mut state,
        &Command::PlaceRailStation(TRAIN_LINE_STATION_A, 2),
    )
    .expect("estación A train_supply");
    apply_command(
        &mut state,
        &Command::PlaceRailStation(TRAIN_LINE_STATION_B, 3),
    )
    .expect("estación B train_supply");
    apply_command(&mut state, &Command::PlaceRailDepotDir(TRAIN_LINE_DEPOT, 1))
        .expect("depósito train_supply");
    place_train_supply_signals(&mut state);

    let mut mine = Industry::new(TRAIN_SUPPLY_MINE, IndustryKind::CoalMine);
    mine.stock = 80;
    state.industries.push(mine);

    let factory = Industry::new(TRAIN_SUPPLY_FACTORY, IndustryKind::Factory);
    state.industries.push(factory);

    let mut train = Vehicle::new(
        TRAIN_SUPPLY_VEHICLE_ID,
        VehicleKind::Train,
        TRAIN_LINE_DEPOT,
        TRAIN_LINE_STATION_A,
    );
    train.set_vehicle_orders(vec![
        VehicleOrder::station_with_flags(TRAIN_LINE_STATION_A, true, false),
        VehicleOrder::station(TRAIN_LINE_STATION_B),
    ]);
    train.sync_order_destination(&state.map);
    if let Some(path) = find_path(&state.map, train.pos, train.dest, PathNetwork::Rail) {
        train.path = VecDeque::from(path);
    }
    state.vehicles.push(train);

    state
}

/// Mina → estación A → **vía de ida** (y=6, solo →este) → estación B;
/// el mismo tren vuelve por **vía de vuelta** (y=4, solo ←oeste).
/// Dos rieles físicos separados, señales unidireccionales, mina y fábrica visibles,
/// dos locomotoras que salen del depósito en (4,7).
///
/// # Panics
///
/// Si la construcción del escenario fijo falla (bug del propio escenario).
#[must_use]
#[allow(clippy::expect_used)]
pub fn build_train_supply_dual() -> GameState {
    let mut state = GameState::new(24, 14);
    state.world_seed = 0;
    state.disasters_enabled = false;

    for x in 2..=12 {
        apply_command(
            &mut state,
            &Command::PlaceRail(TileCoord::new(x, TRAIN_DUAL_TRACK_OUT_Y)),
        )
        .expect("vía ida train_supply_dual");
        apply_command(
            &mut state,
            &Command::PlaceRail(TileCoord::new(x, TRAIN_DUAL_TRACK_RET_Y)),
        )
        .expect("vía vuelta train_supply_dual");
    }
    for &x in &[3, 10] {
        apply_command(&mut state, &Command::PlaceRail(TileCoord::new(x, 5)))
            .expect("conector entre vías train_supply_dual");
    }

    apply_command(
        &mut state,
        &Command::PlaceRailStation(TRAIN_DUAL_STATION_A, 2),
    )
    .expect("estación A");
    apply_command(
        &mut state,
        &Command::PlaceRailStation(TRAIN_DUAL_STATION_B, 0),
    )
    .expect("estación B");

    apply_command(
        &mut state,
        &Command::PlaceIndustryKind(TRAIN_DUAL_COAL_MINE, IndustryKind::CoalMine),
    )
    .expect("mina train_supply_dual");
    apply_command(
        &mut state,
        &Command::PlaceIndustryKind(TRAIN_DUAL_FACTORY, IndustryKind::Factory),
    )
    .expect("fábrica train_supply_dual");
    complete_industry_construction(&mut state, TRAIN_DUAL_COAL_MINE);
    complete_industry_construction(&mut state, TRAIN_DUAL_FACTORY);
    if let Some(mine) = state
        .industries
        .iter_mut()
        .find(|i| i.pos == TRAIN_DUAL_COAL_MINE)
    {
        mine.stock = 120;
    }

    // Boca hacia la vía de ida: salida del depósito en (4,6).
    apply_command(&mut state, &Command::PlaceRailDepotDir(TRAIN_DUAL_DEPOT, 3))
        .expect("depósito train_supply_dual");

    place_one_way_signals_on_row(&mut state, TRAIN_DUAL_TRACK_OUT_Y, &[5, 7, 9], 0);
    place_one_way_signals_on_row(&mut state, TRAIN_DUAL_TRACK_RET_Y, &[9, 7, 5], 2);

    let orders = vec![
        VehicleOrder::station_with_flags(TRAIN_DUAL_STATION_A, true, false),
        VehicleOrder::station(TRAIN_DUAL_STATION_B),
        VehicleOrder::station(TRAIN_DUAL_STATION_A),
    ];
    push_dual_train(&mut state, TRAIN_DUAL_VEHICLE_ID, orders.clone(), true);
    // El segundo arranca detenido para no bloquear la salida del depósito.
    push_dual_train(&mut state, TRAIN_DUAL_VEHICLE_2_ID, orders, false);

    state
}

/// Pone en marcha el tren 2 del escenario dual cuando el 1 ya cargó en A
/// (`current_order > 0`) y puede usar la vía de ida sin cruzarse de frente.
pub(crate) fn release_staged_depot_trains(state: &mut GameState) {
    let leader_ready = state
        .vehicles
        .iter()
        .find(|v| v.id == TRAIN_DUAL_VEHICLE_ID)
        .is_some_and(|v| v.current_order > 0);
    if !leader_ready {
        return;
    }
    let Some(train2) = state
        .vehicles
        .iter_mut()
        .find(|v| v.id == TRAIN_DUAL_VEHICLE_2_ID && !v.running && v.pos == TRAIN_DUAL_DEPOT)
    else {
        return;
    };
    train2.running = true;
    train2.sync_order_destination(&state.map);
}

fn complete_industry_construction(state: &mut GameState, origin: TileCoord) {
    let tiles: Vec<TileCoord> = state
        .industries
        .iter()
        .find(|i| i.pos == origin)
        .map(|i| i.tiles.clone())
        .unwrap_or_default();
    for c in tiles {
        let Some(mut tile) = state.map.get(c) else {
            continue;
        };
        tile.m1 |= 0x80;
        let _ = state.map.set_tile(c, tile);
    }
}

fn push_dual_train(state: &mut GameState, id: u32, orders: Vec<VehicleOrder>, running: bool) {
    let mut train = Vehicle::new(
        id,
        VehicleKind::Train,
        TRAIN_DUAL_DEPOT,
        TRAIN_DUAL_STATION_A,
    );
    train.running = running;
    train.set_vehicle_orders(orders);
    train.sync_order_destination(&state.map);
    if let Some(path) = find_path(&state.map, train.pos, train.dest, PathNetwork::Rail) {
        train.path = VecDeque::from(path);
    }
    state.vehicles.push(train);
}

/// Señales de un solo sentido en un carril recto (sin `make_signal_bidirectional_x`).
#[allow(clippy::expect_used)]
fn place_one_way_signals_on_row(state: &mut GameState, y: i32, xs: &[i32], orientation: u8) {
    for &x in xs {
        let tile = TileCoord::new(x, y);
        apply_command(
            state,
            &Command::PlaceRailSignal(tile, orientation, 128, 128, SIGTYPE_BLOCK),
        )
        .expect("señal unidireccional train_supply_dual");
    }
}

#[allow(clippy::expect_used)]
fn place_signals_on_row(state: &mut GameState, y: i32, xs: &[i32]) {
    for &x in xs {
        let tile = TileCoord::new(x, y);
        apply_command(
            state,
            &Command::PlaceRailSignal(tile, 0, 128, 128, SIGTYPE_BLOCK),
        )
        .expect("señal train_supply");
        make_signal_bidirectional_x(state, tile);
    }
}

#[allow(clippy::expect_used)]
fn place_train_supply_signals(state: &mut GameState) {
    place_signals_on_row(state, 6, &[5, 7, 10]);
    apply_command(
        state,
        &Command::PlaceRailSignal(TRAIN_SUPPLY_SIGNAL_SOUTH, 2, 128, 128, SIGTYPE_BLOCK),
    )
    .expect("señal sur train_supply");
    make_signal_bidirectional_x(state, TRAIN_SUPPLY_SIGNAL_SOUTH);
}

/// Habilita salida bidireccional en una señal sobre carril X (bits 2 y 3).
#[allow(clippy::expect_used)]
fn make_signal_bidirectional_x(state: &mut GameState, signal: TileCoord) {
    let mut tile = state.map.get(signal).expect("tesela de señal");
    tile.m3 = (tile.m3 & 0x0F) | 0xC0;
    tile.m3hi = (tile.m3hi & 0x0F) | 0xC0;
    state
        .map
        .set_tile(signal, tile)
        .expect("señal bidireccional");
}

/// Línea recta con señal en (2,0): tren 1 espera en la señal mientras el tren 2
/// ocupa el bloque en (4,0); al retirarse el bloqueador, el líder continúa.
///
/// # Panics
///
/// Si la construcción del escenario fijo falla (bug del propio escenario).
#[must_use]
#[allow(clippy::expect_used)] // escenario fijo: un fallo de construcción es un bug del escenario
pub fn build_train_signal() -> GameState {
    let mut state = GameState::new(12, 4);
    state.world_seed = 0;

    for x in 0..=6_i32 {
        apply_command(
            &mut state,
            &Command::SetRailBits(TileCoord::new(x, 0), 0x01),
        )
        .expect("vía train_signal");
    }
    apply_command(
        &mut state,
        &Command::PlaceRailSignal(TRAIN_SIGNAL_TILE, 0, 128, 128, SIGTYPE_BLOCK),
    )
    .expect("señal train_signal");
    make_signal_bidirectional_x(&mut state, TRAIN_SIGNAL_TILE);

    let goal = TileCoord::new(6, 0);
    let mut lead = Vehicle::new(
        TRAIN_SIGNAL_LEAD_ID,
        VehicleKind::Train,
        TRAIN_SIGNAL_TILE,
        goal,
    );
    lead.path = VecDeque::from((3..=6).map(|x| TileCoord::new(x, 0)).collect::<Vec<_>>());
    lead.set_cruise_speed();

    let mut blocker = Vehicle::new(
        TRAIN_SIGNAL_BLOCKER_ID,
        VehicleKind::Train,
        TRAIN_SIGNAL_BLOCK_TILE,
        TRAIN_SIGNAL_BLOCK_TILE,
    );
    blocker.running = false;

    state.vehicles.push(lead);
    state.vehicles.push(blocker);
    state
}

/// Fila Y de la tira de regresión encoding (esquina inferior del mapa demo).
pub const RAIL_SIGNALS_MIXED_Y: i32 = 18;

/// Línea principal del demo (aproximación + estación presignal).
pub const RAIL_SIGNALS_DEMO_MAIN_Y: i32 = 12;
pub const RAIL_SIGNALS_DEMO_PLAT1_Y: i32 = 10;
pub const RAIL_SIGNALS_DEMO_PLAT2_Y: i32 = 14;
pub const RAIL_SIGNALS_DEMO_ENTRY: TileCoord = TileCoord::new(21, 11);
pub const RAIL_SIGNALS_DEMO_EXIT1: TileCoord = TileCoord::new(24, 10);
pub const RAIL_SIGNALS_DEMO_EXIT2: TileCoord = TileCoord::new(24, 14);
pub const RAIL_SIGNALS_DEMO_TWO_WAY_WEST: TileCoord = TileCoord::new(6, 12);
pub const RAIL_SIGNALS_DEMO_TWO_WAY_EAST: TileCoord = TileCoord::new(9, 12);
pub const RAIL_SIGNALS_DEMO_MINE: TileCoord = TileCoord::new(2, 7);
pub const RAIL_SIGNALS_DEMO_FACTORY: TileCoord = TileCoord::new(30, 14);
pub const RAIL_SIGNALS_DEMO_LOAD_STATION: TileCoord = TileCoord::new(5, 12);
pub const RAIL_SIGNALS_DEMO_UNLOAD_STATION: TileCoord = TileCoord::new(32, 12);
pub const RAIL_SIGNALS_DEMO_DEPOT: TileCoord = TileCoord::new(2, 13);
pub const RAIL_SIGNALS_DEMO_LEAD_ID: u32 = 701;
pub const RAIL_SIGNALS_DEMO_BLOCKER2_ID: u32 = 703;

/// Teselas con señal y tipo esperado en la tira de regresión (`SignalType` en `signal_type.h`).
pub const RAIL_SIGNALS_MIXED_TYPES: &[(i32, u8)] = &[
    (1, SIGTYPE_BLOCK),
    (2, SIGTYPE_ENTRY),
    (3, SIGTYPE_EXIT),
    (4, SIGTYPE_COMBO),
    (5, SIGTYPE_PATH),
    (6, SIGTYPE_PATH_ONEWAY),
];

#[must_use]
pub fn rail_signals_mixed_coord(x: i32) -> TileCoord {
    TileCoord::new(x, RAIL_SIGNALS_MIXED_Y)
}

/// Demo jugable: mina → carga → presignals/two-way → descarga en fábrica.
///
/// - **Economía:** mina de carbón (NO) y fábrica (SE); estaciones de carga/descarga.
/// - **Two-way:** señales bidireccionales en el terminal oeste (x=6 y x=9).
/// - **Presignals:** entry en ramificación (21,11); exits en plataformas; un bloqueador en vía 2 (entry verde).
/// - **Encoding:** tira x=1..6 en y=18 para golden/regresión.
///
/// # Panics
///
/// Si la construcción del escenario fijo falla (bug del propio escenario).
#[must_use]
#[allow(clippy::expect_used, clippy::too_many_lines)]
pub fn build_rail_signals_mixed() -> GameState {
    let mut state = GameState::new(36, 22);
    state.world_seed = 0;
    state.disasters_enabled = false;
    state.economy.money = 500_000;

    build_rail_signals_demo_track(&mut state);
    build_rail_signals_encoding_strip(&mut state);
    state
}

#[allow(clippy::expect_used, clippy::too_many_lines)]
fn build_rail_signals_demo_track(state: &mut GameState) {
    apply_command(
        state,
        &Command::PlaceIndustryKind(RAIL_SIGNALS_DEMO_MINE, IndustryKind::CoalMine),
    )
    .expect("mina demo");
    apply_command(
        state,
        &Command::PlaceIndustryKind(RAIL_SIGNALS_DEMO_FACTORY, IndustryKind::Factory),
    )
    .expect("fábrica demo");
    complete_industry_construction(state, RAIL_SIGNALS_DEMO_MINE);
    complete_industry_construction(state, RAIL_SIGNALS_DEMO_FACTORY);
    if let Some(mine) = state
        .industries
        .iter_mut()
        .find(|i| i.pos == RAIL_SIGNALS_DEMO_MINE)
    {
        mine.stock = 120;
    }

    for x in 2..=34 {
        if x == 5 || x == 32 {
            continue;
        }
        apply_command(
            state,
            &Command::PlaceRail(TileCoord::new(x, RAIL_SIGNALS_DEMO_MAIN_Y)),
        )
        .expect("vía principal demo");
    }
    for c in [
        TileCoord::new(2, 9),
        TileCoord::new(3, 10),
        TileCoord::new(33, 12),
    ] {
        apply_command(state, &Command::PlaceRail(c)).expect("acceso mina");
    }

    apply_command(
        state,
        &Command::PlaceRailStation(RAIL_SIGNALS_DEMO_LOAD_STATION, 0),
    )
    .expect("estación carga");
    apply_command(
        state,
        &Command::PlaceRailStation(RAIL_SIGNALS_DEMO_UNLOAD_STATION, 0),
    )
    .expect("estación descarga");
    for x in 22..=28 {
        apply_command(
            state,
            &Command::PlaceRail(TileCoord::new(x, RAIL_SIGNALS_DEMO_PLAT1_Y)),
        )
        .expect("plataforma 1");
        apply_command(
            state,
            &Command::PlaceRail(TileCoord::new(x, RAIL_SIGNALS_DEMO_PLAT2_Y)),
        )
        .expect("plataforma 2");
    }
    for c in [
        TileCoord::new(20, 11),
        TileCoord::new(20, 13),
        TileCoord::new(21, 11),
        TileCoord::new(21, 13),
        TileCoord::new(22, 11),
        TileCoord::new(22, 13),
        TileCoord::new(23, 11),
        TileCoord::new(23, 13),
        TileCoord::new(26, 11),
        TileCoord::new(26, 13),
        // Vuelta plataforma 2 → throat (sin cruzar exit en sentido prohibido).
        TileCoord::new(27, 13),
        TileCoord::new(28, 13),
    ] {
        apply_command(state, &Command::PlaceRail(c)).expect("conector presignal");
    }

    apply_command(
        state,
        &Command::PlaceRailDepotDir(RAIL_SIGNALS_DEMO_DEPOT, 3),
    )
    .expect("depósito demo");

    place_two_way_block_signal(state, RAIL_SIGNALS_DEMO_TWO_WAY_WEST);
    place_two_way_block_signal(state, RAIL_SIGNALS_DEMO_TWO_WAY_EAST);
    place_oriented_presignal(state, RAIL_SIGNALS_DEMO_ENTRY, SIGTYPE_ENTRY, 0);
    place_oriented_presignal(state, RAIL_SIGNALS_DEMO_EXIT1, SIGTYPE_EXIT, 0);
    place_oriented_presignal(state, RAIL_SIGNALS_DEMO_EXIT2, SIGTYPE_EXIT, 0);

    let mut lead = Vehicle::new(
        RAIL_SIGNALS_DEMO_LEAD_ID,
        VehicleKind::Train,
        RAIL_SIGNALS_DEMO_DEPOT,
        RAIL_SIGNALS_DEMO_LOAD_STATION,
    );
    lead.running = true;
    lead.set_vehicle_orders(vec![
        VehicleOrder::station_with_flags(RAIL_SIGNALS_DEMO_LOAD_STATION, true, false),
        VehicleOrder::station(RAIL_SIGNALS_DEMO_UNLOAD_STATION),
    ]);
    lead.sync_order_destination(&state.map);
    lead.set_cruise_speed();
    if let Some(path) = find_path(&state.map, lead.pos, lead.dest, PathNetwork::Rail) {
        lead.path = VecDeque::from(path);
    }

    let mut blocker2 = Vehicle::new(
        RAIL_SIGNALS_DEMO_BLOCKER2_ID,
        VehicleKind::Train,
        TileCoord::new(27, RAIL_SIGNALS_DEMO_PLAT2_Y),
        TileCoord::new(27, RAIL_SIGNALS_DEMO_PLAT2_Y),
    );
    blocker2.running = false;

    state.vehicles.push(lead);
    state.vehicles.push(blocker2);
}

#[allow(clippy::expect_used)]
fn build_rail_signals_encoding_strip(state: &mut GameState) {
    for x in 0..=7_i32 {
        apply_command(
            state,
            &Command::SetRailBits(TileCoord::new(x, RAIL_SIGNALS_MIXED_Y), 0x01),
        )
        .expect("tira encoding");
    }
    for &(x, sig_type) in RAIL_SIGNALS_MIXED_TYPES {
        place_mixed_signal_on_x(state, x, sig_type);
    }
}

#[allow(clippy::expect_used)]
fn place_two_way_block_signal(state: &mut GameState, c: TileCoord) {
    apply_command(
        state,
        &Command::PlaceRailSignal(c, 0, 128, 128, SIGTYPE_BLOCK),
    )
    .expect("two-way block");
    make_signal_bidirectional_x(state, c);
}

#[allow(clippy::expect_used)]
fn place_oriented_presignal(state: &mut GameState, c: TileCoord, sig_type: u8, orientation: u8) {
    apply_command(
        state,
        &Command::PlaceRailSignal(c, orientation, 128, 128, sig_type),
    )
    .expect("colocar presignal demo");
}

#[allow(clippy::expect_used)]
fn place_mixed_signal_on_x(state: &mut GameState, x: i32, sig_type: u8) {
    let c = rail_signals_mixed_coord(x);
    apply_command(state, &Command::PlaceRailSignal(c, 0, 128, 128, sig_type))
        .expect("colocar señal rail_signals_mixed");
}

/// Compañía con préstamo para verificar interés mensual.
fn build_loan_interest() -> GameState {
    let mut state = GameState::new(8, 8);
    state.economy.loan = 100_000;
    state.economy.money = 50_000;
    state
}

/// Ciudad con parada bus y casas en cobertura.
fn build_town_growth() -> GameState {
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
fn build_breakdown() -> GameState {
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
    }
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
        assert!(build_scenario("train_supply").is_some());
        assert!(build_scenario("train_supply_signal").is_some());
        assert!(build_scenario("train_signal").is_some());
        assert!(build_scenario("train_supply_dual").is_some());
        assert!(build_scenario("rail_signals_mixed").is_some());
        assert_eq!(
            scenario_names(),
            &[
                "truck_bay",
                "train_line",
                "train_supply",
                "train_supply_dual",
                "train_supply_signal",
                "train_signal",
                "rail_signals_mixed",
                "loan_interest",
                "town_growth",
                "breakdown",
            ]
        );
    }

    #[test]
    fn train_signal_layout_is_consistent() {
        let state = build_train_signal();
        let signal_tile = state.map.get(TRAIN_SIGNAL_TILE).unwrap();
        assert!(crate::rail_signals::rail_tile_is_signals(signal_tile.m5));
        assert_eq!(state.vehicles.len(), 2);
        assert_eq!(state.vehicles[0].id, TRAIN_SIGNAL_LEAD_ID);
        assert_eq!(state.vehicles[0].pos, TRAIN_SIGNAL_TILE);
        assert_eq!(state.vehicles[1].id, TRAIN_SIGNAL_BLOCKER_ID);
        assert_eq!(state.vehicles[1].pos, TRAIN_SIGNAL_BLOCK_TILE);
        assert!(!state.vehicles[1].running);
    }

    #[test]
    fn train_line_layout_is_consistent() {
        use crate::station::{rail_station_approach_tile, rail_station_stop_tile};

        let state = build_train_line();
        assert_eq!(
            rail_station_approach_tile(&state.map, TRAIN_LINE_STATION_A),
            Some(TileCoord::new(2, 6)),
            "acceso a la estación A"
        );
        assert_eq!(
            rail_station_approach_tile(&state.map, TRAIN_LINE_STATION_B),
            Some(TileCoord::new(12, 9)),
            "acceso a la estación B (boca al norte)"
        );
        assert_eq!(state.vehicles.len(), 1);
        assert_eq!(state.stations.len(), 2);
        assert_eq!(state.vehicles[0].kind, VehicleKind::Train);
        assert!(
            !state.vehicles[0].path.is_empty(),
            "el tren arranca con ruta desde el depósito"
        );
        assert_eq!(
            rail_station_stop_tile(&state.map, TRAIN_LINE_STATION_A),
            Some(TRAIN_LINE_STATION_A),
            "destino de parada A = plataforma"
        );
        assert_eq!(
            rail_station_stop_tile(&state.map, TRAIN_LINE_STATION_B),
            Some(TRAIN_LINE_STATION_B),
            "destino de parada B = plataforma"
        );
        // La línea conecta depósito → plataforma A → esquina → plataforma B.
        let a_to_b = find_path(
            &state.map,
            TRAIN_LINE_STATION_A,
            TRAIN_LINE_STATION_B,
            PathNetwork::Rail,
        )
        .expect("ruta ferroviaria A → B");
        assert!(a_to_b.contains(&TRAIN_LINE_SIGNAL), "pasa por la señal");
        assert!(a_to_b.contains(&TRAIN_LINE_CORNER), "pasa por la esquina");
        assert!(
            find_path(
                &state.map,
                TRAIN_LINE_DEPOT,
                TRAIN_LINE_STATION_A,
                PathNetwork::Rail
            )
            .is_some(),
            "el depósito conecta con la estación A"
        );
        // La señal quedó colocada sobre la recta.
        let signal_tile = state.map.get(TRAIN_LINE_SIGNAL).unwrap();
        assert!(crate::rail_signals::rail_tile_is_signals(signal_tile.m5));
    }

    #[test]
    fn train_supply_has_mine_factory_and_signals() {
        use crate::station;

        let state = build_train_supply();
        assert_eq!(state.vehicles.len(), 1);
        assert_eq!(state.industries.len(), 2);
        assert_eq!(state.industries[0].kind, IndustryKind::CoalMine);
        assert_eq!(state.industries[1].kind, IndustryKind::Factory);
        assert!(
            station::industry_in_station_coverage(
                &state.industries[0],
                TRAIN_LINE_STATION_A,
                station::STATION_COVERAGE_RADIUS,
            ),
            "mina en cobertura de estación A"
        );
        assert!(
            station::industry_in_station_coverage(
                &state.industries[1],
                TRAIN_LINE_STATION_B,
                station::STATION_COVERAGE_RADIUS,
            ),
            "fábrica en cobertura de estación B"
        );
        for &signal in &[
            TRAIN_SUPPLY_SIGNAL_WEST,
            TRAIN_LINE_SIGNAL,
            TRAIN_SUPPLY_SIGNAL_EAST,
            TRAIN_SUPPLY_SIGNAL_SOUTH,
        ] {
            let tile = state.map.get(signal).unwrap();
            assert!(
                crate::rail_signals::rail_tile_is_signals(tile.m5),
                "señal en {signal:?}"
            );
        }
    }

    #[test]
    fn train_supply_signal_snapshot_has_blocker_on_signal() {
        let state = build_train_supply_signal_snapshot();
        assert_eq!(state.vehicles.len(), 2);
        assert_eq!(state.vehicles[0].pos, TRAIN_SUPPLY_WAIT_SIGNAL);
        assert_eq!(state.vehicles[0].cargo, 20);
        assert_eq!(state.vehicles[1].id, TRAIN_SUPPLY_BLOCKER_ID);
        assert_eq!(state.vehicles[1].pos, TRAIN_SUPPLY_BLOCK_TILE);
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn train_supply_dual_has_two_tracks_signals_and_paths() {
        use crate::map::TileKind;
        use crate::station::{self, rail_station_stop_tile};

        let state = build_train_supply_dual();
        assert_eq!(state.vehicles.len(), 2);
        assert_eq!(state.stations.len(), 2);
        assert_eq!(
            state.map.get_kind(TRAIN_DUAL_DEPOT),
            Some(TileKind::RailDepot)
        );
        assert!(
            state
                .map
                .get_kind(TRAIN_DUAL_COAL_MINE)
                .is_some_and(|k| k == TileKind::Industry),
            "mina visible en el mapa"
        );
        assert!(
            state
                .map
                .get_kind(TRAIN_DUAL_FACTORY)
                .is_some_and(|k| k == TileKind::Industry),
            "fábrica visible en el mapa"
        );
        assert!(
            state
                .vehicles
                .iter()
                .find(|v| v.id == TRAIN_DUAL_VEHICLE_ID)
                .is_some_and(|v| v.pos == TRAIN_DUAL_DEPOT),
            "tren 1 arranca en el depósito"
        );
        assert!(
            state
                .vehicles
                .iter()
                .find(|v| v.id == TRAIN_DUAL_VEHICLE_2_ID)
                .is_some_and(|v| v.pos == TRAIN_DUAL_DEPOT && !v.running),
            "tren 2 espera en el depósito hasta que el 1 libere la salida"
        );
        let mine = state
            .industries
            .iter()
            .find(|i| i.pos == TRAIN_DUAL_COAL_MINE)
            .expect("mina");
        let factory = state
            .industries
            .iter()
            .find(|i| i.pos == TRAIN_DUAL_FACTORY)
            .expect("fábrica");
        assert!(
            station::industry_in_station_coverage(
                mine,
                TRAIN_DUAL_STATION_A,
                station::STATION_COVERAGE_RADIUS,
            ),
            "mina en cobertura de estación A"
        );
        assert!(
            station::industry_in_station_coverage(
                factory,
                TRAIN_DUAL_STATION_B,
                station::STATION_COVERAGE_RADIUS,
            ),
            "fábrica en cobertura de estación B"
        );
        for &y in &[TRAIN_DUAL_TRACK_OUT_Y, TRAIN_DUAL_TRACK_RET_Y] {
            for &x in &[5, 7, 9] {
                let tile = state.map.get(TileCoord::new(x, y)).unwrap();
                assert!(
                    crate::rail_signals::rail_tile_is_signals(tile.m5),
                    "señal en ({x},{y})"
                );
                assert_eq!(
                    crate::rail_signals::rail_signal_present_mask(tile.m3).count_ones(),
                    1,
                    "una señal unidireccional en ({x},{y})"
                );
            }
        }
        let out_mask = crate::rail_signals::rail_signal_present_mask(
            state
                .map
                .get(TileCoord::new(7, TRAIN_DUAL_TRACK_OUT_Y))
                .unwrap()
                .m3,
        );
        let ret_mask = crate::rail_signals::rail_signal_present_mask(
            state
                .map
                .get(TileCoord::new(7, TRAIN_DUAL_TRACK_RET_Y))
                .unwrap()
                .m3,
        );
        assert_eq!(out_mask, 0b0100, "señales ida miran hacia +x");
        assert_eq!(ret_mask, 0b1000, "señales vuelta miran hacia -x");
        assert!(
            find_path(
                &state.map,
                TRAIN_DUAL_DEPOT,
                TRAIN_DUAL_STATION_A,
                PathNetwork::Rail,
            )
            .is_some(),
            "ruta depósito → A"
        );
        assert!(
            find_path(
                &state.map,
                TRAIN_DUAL_STATION_A,
                TRAIN_DUAL_STATION_B,
                PathNetwork::Rail,
            )
            .is_some(),
            "ruta ida A → B"
        );
        assert!(
            find_path(
                &state.map,
                TRAIN_DUAL_STATION_B,
                TRAIN_DUAL_STATION_A,
                PathNetwork::Rail,
            )
            .is_some(),
            "ruta vuelta B → A"
        );
        assert_eq!(
            rail_station_stop_tile(&state.map, TRAIN_DUAL_STATION_A),
            Some(TRAIN_DUAL_STATION_A)
        );
        assert_eq!(
            rail_station_stop_tile(&state.map, TRAIN_DUAL_STATION_B),
            Some(TRAIN_DUAL_STATION_B)
        );
    }

    #[test]
    fn train_supply_dual_signals_green_on_clear_blocks() {
        let mut state = build_train_supply_dual();
        let mut dirty = Vec::new();
        crate::rail_signals::update_rail_signal_states(
            &mut state.map,
            &state.vehicles,
            &mut dirty,
            true,
        );
        for &y in &[TRAIN_DUAL_TRACK_OUT_Y, TRAIN_DUAL_TRACK_RET_Y] {
            let tile = state.map.get(TileCoord::new(7, y)).unwrap();
            assert!(
                crate::rail_signals::signal_is_green(tile.m3hi, 2)
                    || crate::rail_signals::signal_is_green(tile.m3hi, 3),
                "señal en (7,{y}) debe estar verde con bloque libre: m3hi={:#x}",
                tile.m3hi
            );
        }
    }

    #[test]
    fn train_supply_dual_second_train_never_passes_closed_signal() {
        use crate::rail_signals::train_blocked_by_signal;

        let mut state = build_train_supply_dual();
        for _ in 0..8_000 {
            let blocked = state
                .vehicles
                .iter()
                .find(|v| v.id == TRAIN_DUAL_VEHICLE_2_ID)
                .filter(|v| v.running && v.movement_target().is_some())
                .is_some_and(|v| train_blocked_by_signal(&state.map, &state.vehicles, v));
            let pos_before = state
                .vehicles
                .iter()
                .find(|v| v.id == TRAIN_DUAL_VEHICLE_2_ID)
                .map(|v| v.pos);
            state.step();
            if blocked {
                let pos_after = state
                    .vehicles
                    .iter()
                    .find(|v| v.id == TRAIN_DUAL_VEHICLE_2_ID)
                    .map(|v| v.pos);
                assert_eq!(
                    pos_before, pos_after,
                    "tren 2 no debe avanzar con señal/bloque cerrado"
                );
            }
        }
    }

    #[test]
    fn train_supply_dual_follower_waits_before_last_signal() {
        use crate::rail_signals::train_blocked_by_signal;
        use std::collections::VecDeque;

        let mut state = build_train_supply_dual();
        let signal_pos = TileCoord::new(9, TRAIN_DUAL_TRACK_OUT_Y);
        let leader_pos = TileCoord::new(11, TRAIN_DUAL_TRACK_OUT_Y);
        let follower_pos = TileCoord::new(8, TRAIN_DUAL_TRACK_OUT_Y);

        {
            let leader = state
                .vehicles
                .iter_mut()
                .find(|v| v.id == TRAIN_DUAL_VEHICLE_ID)
                .expect("tren 1");
            leader.pos = leader_pos;
            leader.dest = TRAIN_DUAL_STATION_B;
            leader.current_order = 1;
            leader.path.clear();
            leader.running = false;
        }
        {
            let follower = state
                .vehicles
                .iter_mut()
                .find(|v| v.id == TRAIN_DUAL_VEHICLE_2_ID)
                .expect("tren 2");
            follower.pos = follower_pos;
            follower.dest = TRAIN_DUAL_STATION_B;
            follower.current_order = 1;
            follower.path = VecDeque::from([
                TileCoord::new(9, TRAIN_DUAL_TRACK_OUT_Y),
                TileCoord::new(10, TRAIN_DUAL_TRACK_OUT_Y),
                TileCoord::new(11, TRAIN_DUAL_TRACK_OUT_Y),
                TRAIN_DUAL_STATION_B,
            ]);
            follower.running = true;
            follower.set_cruise_speed();
            follower.progress = 200;
        }

        let mut dirty = Vec::new();
        crate::rail_signals::update_rail_signal_states(
            &mut state.map,
            &state.vehicles,
            &mut dirty,
            true,
        );
        let follower = state
            .vehicles
            .iter()
            .find(|v| v.id == TRAIN_DUAL_VEHICLE_2_ID)
            .expect("tren 2");
        assert!(
            train_blocked_by_signal(&state.map, &state.vehicles, follower),
            "seguidor debe frenar al completar la tesela previa a la última señal"
        );

        for _ in 0..500 {
            state.step();
            let f = state
                .vehicles
                .iter()
                .find(|v| v.id == TRAIN_DUAL_VEHICLE_2_ID)
                .expect("tren 2");
            assert!(
                f.pos.x < signal_pos.x,
                "no debe entrar al bloque tras la señal {signal_pos:?}: pos={:?}",
                f.pos
            );
        }
    }

    #[test]
    fn train_supply_dual_follower_waits_at_signal_behind_leader() {
        use crate::rail_signals::train_blocked_by_signal;
        use std::collections::VecDeque;

        let mut state = build_train_supply_dual();
        let leader_pos = TileCoord::new(8, TRAIN_DUAL_TRACK_OUT_Y);
        let signal_pos = TileCoord::new(7, TRAIN_DUAL_TRACK_OUT_Y);
        let follower_pos = TileCoord::new(6, TRAIN_DUAL_TRACK_OUT_Y);

        {
            let leader = state
                .vehicles
                .iter_mut()
                .find(|v| v.id == TRAIN_DUAL_VEHICLE_ID)
                .expect("tren 1");
            leader.pos = leader_pos;
            leader.dest = TRAIN_DUAL_STATION_B;
            leader.current_order = 1;
            leader.cargo = 20;
            leader.cargo_type = Some(crate::CargoType::Coal);
            leader.path.clear();
            leader.running = false;
        }
        {
            let follower = state
                .vehicles
                .iter_mut()
                .find(|v| v.id == TRAIN_DUAL_VEHICLE_2_ID)
                .expect("tren 2");
            follower.pos = follower_pos;
            follower.dest = TRAIN_DUAL_STATION_B;
            follower.current_order = 1;
            follower.path = VecDeque::from([
                TileCoord::new(7, TRAIN_DUAL_TRACK_OUT_Y),
                TileCoord::new(8, TRAIN_DUAL_TRACK_OUT_Y),
                TileCoord::new(9, TRAIN_DUAL_TRACK_OUT_Y),
                TRAIN_DUAL_STATION_B,
            ]);
            follower.running = true;
            follower.set_cruise_speed();
            follower.progress = 200;
        }

        let mut dirty = Vec::new();
        crate::rail_signals::update_rail_signal_states(
            &mut state.map,
            &state.vehicles,
            &mut dirty,
            true,
        );
        let follower = state
            .vehicles
            .iter()
            .find(|v| v.id == TRAIN_DUAL_VEHICLE_2_ID)
            .expect("tren 2");
        assert!(
            train_blocked_by_signal(&state.map, &state.vehicles, follower),
            "seguidor debe frenar al completar la tesela previa a la señal"
        );

        for _ in 0..500 {
            state.step();
            let f = state
                .vehicles
                .iter()
                .find(|v| v.id == TRAIN_DUAL_VEHICLE_2_ID)
                .expect("tren 2");
            assert!(
                f.pos.x <= signal_pos.x,
                "el seguidor no debe entrar al bloque protegido por {signal_pos:?}: pos={:?}",
                f.pos
            );
        }
        let f = state
            .vehicles
            .iter()
            .find(|v| v.id == TRAIN_DUAL_VEHICLE_2_ID)
            .expect("tren 2");
        assert!(
            f.pos.x <= signal_pos.x,
            "debe esperar en o antes de la señal: pos={:?}",
            f.pos
        );
    }

    #[test]
    fn train_supply_dual_round_trip_returns_to_a() {
        let mut state = build_train_supply_dual();
        let mut used_return_track = false;
        for _ in 0..12_000 {
            state.step();
            let train = state
                .vehicles
                .iter()
                .find(|v| v.id == TRAIN_DUAL_VEHICLE_ID)
                .expect("tren dual");
            if train.pos.y == TRAIN_DUAL_TRACK_RET_Y {
                used_return_track = true;
            }
        }
        let train = state
            .vehicles
            .iter()
            .find(|v| v.id == TRAIN_DUAL_VEHICLE_ID)
            .expect("tren dual");
        assert!(
            state.stats.cargo_deliveries > 0,
            "debe haber descargado carbón en B (pos={:?} order={} cargo={})",
            train.pos,
            train.current_order,
            train.cargo,
        );
        assert!(
            used_return_track,
            "debe circular por la vía de vuelta y={TRAIN_DUAL_TRACK_RET_Y}"
        );
        assert_eq!(
            train.pos, TRAIN_DUAL_STATION_A,
            "tras el ciclo debe volver a estación A: {:?}",
            train.pos
        );
    }

    #[test]
    fn train_supply_dual_tick_698_no_signal_violation() {
        use crate::rail_signals::train_blocked_by_signal;

        let mut state = build_train_supply_dual();
        for tick in 1..=698 {
            state.step();
            let vehicles = state.vehicles.clone();
            for v in &vehicles {
                if v.id != TRAIN_DUAL_VEHICLE_2_ID || !v.running {
                    continue;
                }
                if !train_blocked_by_signal(&state.map, &vehicles, v) {
                    continue;
                }
                let after = state
                    .vehicles
                    .iter()
                    .find(|t| t.id == TRAIN_DUAL_VEHICLE_2_ID)
                    .expect("tren 2");
                assert_eq!(
                    v.pos, after.pos,
                    "tick {tick}: tren 2 avanzó con señal/bloque cerrado {:?}→{:?}",
                    v.pos, after.pos
                );
            }
        }
    }

    fn signal_type_on_any_track(m2: u8) -> u8 {
        use crate::rail_signals::{SignalTrack, signal_type_for_track};
        [
            SignalTrack::X,
            SignalTrack::Y,
            SignalTrack::Upper,
            SignalTrack::Lower,
            SignalTrack::Left,
            SignalTrack::Right,
        ]
        .into_iter()
        .map(|t| signal_type_for_track(m2, t))
        .find(|&t| t != SIGTYPE_BLOCK)
        .unwrap_or(SIGTYPE_BLOCK)
    }

    #[test]
    fn rail_signals_mixed_demo_has_presignal_station_and_two_way() {
        use crate::rail_signals::{
            rail_signal_present_mask, rail_signal_state_mask, update_rail_signal_states,
        };
        use crate::station;
        let mut state = build_rail_signals_mixed();
        update_rail_signal_states(&mut state.map, &state.vehicles, &mut Vec::new(), true);
        assert_eq!(state.map.dimensions(), (36, 22));
        assert_eq!(state.stations.len(), 2);
        assert_eq!(
            state.vehicles.len(),
            2,
            "líder activo + bloqueador en plataforma 2"
        );
        assert_eq!(state.industries.len(), 2);
        assert!(
            station::industry_in_station_coverage(
                state
                    .industries
                    .iter()
                    .find(|i| i.pos == RAIL_SIGNALS_DEMO_MINE)
                    .expect("mina"),
                RAIL_SIGNALS_DEMO_LOAD_STATION,
                station::STATION_COVERAGE_RADIUS,
            ),
            "mina en cobertura de estación de carga"
        );
        assert!(
            station::industry_in_station_coverage(
                state
                    .industries
                    .iter()
                    .find(|i| i.pos == RAIL_SIGNALS_DEMO_FACTORY)
                    .expect("fábrica"),
                RAIL_SIGNALS_DEMO_UNLOAD_STATION,
                station::STATION_COVERAGE_RADIUS,
            ),
            "fábrica en cobertura de estación de descarga"
        );
        let lead = state
            .vehicles
            .iter()
            .find(|v| v.id == RAIL_SIGNALS_DEMO_LEAD_ID)
            .expect("tren líder");
        assert!(lead.running);
        assert_eq!(lead.pos, RAIL_SIGNALS_DEMO_DEPOT);
        assert_eq!(lead.orders.len(), 2);
        let load_approach =
            crate::station::rail_station_approach_tile(&state.map, RAIL_SIGNALS_DEMO_LOAD_STATION)
                .or_else(|| {
                    crate::station::rail_station_stop_tile(
                        &state.map,
                        RAIL_SIGNALS_DEMO_LOAD_STATION,
                    )
                })
                .expect("acceso estación carga");
        assert!(
            find_path(
                &state.map,
                TileCoord::new(2, RAIL_SIGNALS_DEMO_MAIN_Y),
                load_approach,
                PathNetwork::Rail,
            )
            .is_some(),
            "red ferroviaria depósito → carga"
        );

        let entry = state.map.get(RAIL_SIGNALS_DEMO_ENTRY).unwrap();
        assert_eq!(
            signal_type_on_any_track(entry.m2),
            SIGTYPE_ENTRY,
            "entry en ramificación presignal, no en línea principal"
        );
        for exit in [RAIL_SIGNALS_DEMO_EXIT1, RAIL_SIGNALS_DEMO_EXIT2] {
            let tile = state.map.get(exit).unwrap();
            assert_eq!(
                signal_type_on_any_track(tile.m2),
                SIGTYPE_EXIT,
                "exit en {exit:?}"
            );
        }
        for two_way in [
            RAIL_SIGNALS_DEMO_TWO_WAY_WEST,
            RAIL_SIGNALS_DEMO_TWO_WAY_EAST,
        ] {
            let tile = state.map.get(two_way).unwrap();
            assert_eq!(
                rail_signal_present_mask(tile.m3) & 0x0C,
                0x0C,
                "two-way en {two_way:?}"
            );
        }
        let entry_state = rail_signal_state_mask(entry.m3hi);
        let entry_present = rail_signal_present_mask(entry.m3);
        assert_eq!(
            entry_state & entry_present,
            entry_present,
            "entry verde: plataforma 1 libre aunque la 2 esté bloqueada"
        );
    }

    #[test]
    fn rail_signals_mixed_train_cycles_orders_after_delivery() {
        let mut state = build_rail_signals_mixed();
        let mut saw_delivery = false;
        for _ in 0..1200 {
            let before = state.stats.cargo_deliveries;
            state.step();
            let v = state
                .vehicles
                .iter()
                .find(|veh| veh.id == RAIL_SIGNALS_DEMO_LEAD_ID)
                .expect("tren líder");
            if state.stats.cargo_deliveries > before {
                saw_delivery = true;
            }
            assert!(
                !(saw_delivery && v.no_network_route_to_order),
                "sin ruta tras primera entrega: orden {} pos {:?} dest {:?}",
                v.current_order + 1,
                v.pos,
                v.dest
            );
        }
        let v = state
            .vehicles
            .iter()
            .find(|veh| veh.id == RAIL_SIGNALS_DEMO_LEAD_ID)
            .expect("tren líder");
        assert!(
            state.stats.cargo_deliveries >= 2,
            "debe completar al menos dos entregas (ciclos): got {} entregas, orden {}, cargo {}",
            state.stats.cargo_deliveries,
            v.current_order + 1,
            v.cargo
        );
    }

    #[test]
    fn rail_signals_mixed_path_from_plat2_to_load_station() {
        use crate::parity::scenario::{RAIL_SIGNALS_DEMO_LOAD_STATION, build_rail_signals_mixed};
        let state = build_rail_signals_mixed();
        let from = TileCoord::new(25, 14);
        let dest = crate::station::resolve_order_destination(
            &state.map,
            VehicleKind::Train,
            VehicleOrder::station(RAIL_SIGNALS_DEMO_LOAD_STATION),
        );
        let path = find_path(&state.map, from, dest, PathNetwork::Rail);
        assert!(
            path.is_some(),
            "debe haber ruta plataforma 2 → carga (conector de vuelta): from {from:?} dest {dest:?}"
        );
    }

    #[test]
    fn rail_signals_mixed_has_all_signal_types() {
        use crate::rail_signals::{
            SignalTrack, default_signal_variant, rail_tile_is_signals, signal_placement_for_track,
            signal_type_for_track,
        };
        let state = build_rail_signals_mixed();
        let variant = default_signal_variant(crate::news::CALENDAR_BASE_YEAR);
        for &(x, expected_type) in RAIL_SIGNALS_MIXED_TYPES {
            let c = rail_signals_mixed_coord(x);
            let tile = state.map.get(c).expect("tesela señal");
            assert!(
                rail_tile_is_signals(tile.m5),
                "tesela ({x},{RAIL_SIGNALS_MIXED_Y}) debe ser RAIL_TILE_SIGNALS"
            );
            assert_eq!(
                signal_type_for_track(tile.m2, SignalTrack::X),
                expected_type,
                "tipo en x={x}"
            );
            let placement = signal_placement_for_track(SignalTrack::X, 0, variant, expected_type)
                .expect("encoding");
            assert_eq!(tile.m2 & 0x0F, placement.m2 & 0x0F, "m2 tipo en x={x}");
            assert_eq!(tile.m3 & 0xF0, placement.m3 & 0xF0, "m3 presente en x={x}");
        }
    }

    #[test]
    fn rail_signals_mixed_json_roundtrip_preserves_encoding() {
        use crate::rail_signals::{SignalTrack, signal_type_for_track};
        let state = build_rail_signals_mixed();
        let json = state.save_json().expect("guardar");
        let restored = GameState::load_json(&json).expect("cargar");
        for &(x, expected_type) in RAIL_SIGNALS_MIXED_TYPES {
            let before = state.map.get(rail_signals_mixed_coord(x)).unwrap();
            let after = restored.map.get(rail_signals_mixed_coord(x)).unwrap();
            assert_eq!(before.m2, after.m2, "m2 x={x}");
            assert_eq!(before.m3, after.m3, "m3 x={x}");
            assert_eq!(before.m3hi, after.m3hi, "m3hi x={x}");
            assert_eq!(before.m5, after.m5, "m5 x={x}");
            assert_eq!(
                signal_type_for_track(after.m2, SignalTrack::X),
                expected_type
            );
        }
    }
}
