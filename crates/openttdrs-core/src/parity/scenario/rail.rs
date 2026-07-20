//! Escenarios ferroviarios: line, supply, signal, PBS, `rail_signals_mixed`.

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

/// Añade un vagón de carga al tren de un escenario fijo.
#[allow(clippy::expect_used)] // enlace inválido = bug del escenario
fn attach_cargo_wagon(state: &mut GameState, head_id: u32, engine_id: u16) {
    let head = state
        .vehicles
        .iter()
        .find(|v| v.id == head_id)
        .expect("locomotora de escenario");
    let wagon_id = 10_000_u32.saturating_add(head_id);
    let mut wagon = Vehicle::new(wagon_id, VehicleKind::Train, head.pos, head.dest);
    wagon.engine_id = Some(engine_id);
    state.vehicles.push(wagon);
    crate::train_consist::attach_wagon(&mut state.vehicles, head_id, wagon_id)
        .expect("enganchar vagón de escenario");
}

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

/// Ids / geometría del escenario `train_pbs` (path signals, dos corredores).
pub const TRAIN_PBS_NORTH_ID: u32 = 1;
pub const TRAIN_PBS_SOUTH_ID: u32 = 2;
pub const TRAIN_PBS_NORTH_Y: i32 = 2;
pub const TRAIN_PBS_SOUTH_Y: i32 = 4;
pub const TRAIN_PBS_PATH_A: i32 = 3;
pub const TRAIN_PBS_PATH_B: i32 = 7;
pub const TRAIN_PBS_GOAL_X: i32 = 9;

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

#[must_use]
#[allow(clippy::expect_used, clippy::missing_panics_doc)] // escenario fijo
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
    // Escenario: ya en servicio (sin espera de 37 ticks al spawnear).
    train.depot_leave_cleared = true;
    // Escenario legado de una unidad abstracta: no representa una locomotora
    // comprable y conserva la capacidad sintética usada por las trazas.
    train.engine_id = None;
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
    train.depot_leave_cleared = true;
    state.vehicles.push(train);
    attach_cargo_wagon(
        &mut state,
        TRAIN_SUPPLY_VEHICLE_ID,
        crate::ENGINE_WAGON_COAL,
    );

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
/// (`current_order > 0`) y ya pasó la boca del depósito en la vía de ida.
pub(crate) fn release_staged_depot_trains(state: &mut GameState) {
    let leader_ready = state
        .vehicles
        .iter()
        .find(|v| v.id == TRAIN_DUAL_VEHICLE_ID)
        .is_some_and(|v| {
            v.current_order > 0
                && v.pos.y == TRAIN_DUAL_TRACK_OUT_Y
                && v.pos.x > TRAIN_DUAL_DEPOT_EXIT.x
        });
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
    train2.depot_leave_cleared = true;
    train2.wait_counter = 0;
    train2.sync_order_destination(&state.map);
}

pub(crate) fn complete_industry_construction(state: &mut GameState, origin: TileCoord) {
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
    if running {
        train.depot_leave_cleared = true;
    }
    train.set_vehicle_orders(orders);
    train.sync_order_destination(&state.map);
    if let Some(path) = find_path(&state.map, train.pos, train.dest, PathNetwork::Rail) {
        train.path = VecDeque::from(path);
    }
    state.vehicles.push(train);
    attach_cargo_wagon(state, id, crate::ENGINE_WAGON_COAL);
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

/// Dos corredores E–O con path signals: ambos trenes reservan en paralelo (Fase 3 PBS).
///
/// # Panics
///
/// Si la construcción del escenario fijo falla (bug del propio escenario).
#[must_use]
#[allow(clippy::expect_used)]
pub fn build_train_pbs() -> GameState {
    let mut state = GameState::new(16, 8);
    state.world_seed = 0;
    state.disasters_enabled = false;

    for &y in &[TRAIN_PBS_NORTH_Y, TRAIN_PBS_SOUTH_Y] {
        for x in 1..=10 {
            apply_command(&mut state, &Command::PlaceRail(TileCoord::new(x, y))).expect("vía pbs");
            let mut t = state.map.get(TileCoord::new(x, y)).expect("tile");
            t.m5 = 0x01 | (crate::rail_signals::RAIL_TILE_NORMAL << 6); // TRACK_X
            state.map.set_tile(TileCoord::new(x, y), t).expect("set");
        }
        for &x in &[TRAIN_PBS_PATH_A, TRAIN_PBS_PATH_B] {
            apply_command(
                &mut state,
                &Command::PlaceRailSignal(TileCoord::new(x, y), 0, 128, 128, SIGTYPE_PATH),
            )
            .expect("path pbs");
        }
    }

    let goal_n = TileCoord::new(TRAIN_PBS_GOAL_X, TRAIN_PBS_NORTH_Y);
    let goal_s = TileCoord::new(TRAIN_PBS_GOAL_X, TRAIN_PBS_SOUTH_Y);
    let mut north = Vehicle::new(
        TRAIN_PBS_NORTH_ID,
        VehicleKind::Train,
        TileCoord::new(2, TRAIN_PBS_NORTH_Y),
        goal_n,
    );
    north.path = VecDeque::from(
        (3..=TRAIN_PBS_GOAL_X)
            .map(|x| TileCoord::new(x, TRAIN_PBS_NORTH_Y))
            .collect::<Vec<_>>(),
    );
    north.running = true;
    north.set_cruise_speed();

    let mut south = Vehicle::new(
        TRAIN_PBS_SOUTH_ID,
        VehicleKind::Train,
        TileCoord::new(2, TRAIN_PBS_SOUTH_Y),
        goal_s,
    );
    south.path = VecDeque::from(
        (3..=TRAIN_PBS_GOAL_X)
            .map(|x| TileCoord::new(x, TRAIN_PBS_SOUTH_Y))
            .collect::<Vec<_>>(),
    );
    south.running = true;
    south.set_cruise_speed();

    state.vehicles.push(north);
    state.vehicles.push(south);
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
    lead.depot_leave_cleared = true;

    let mut blocker2 = Vehicle::new(
        RAIL_SIGNALS_DEMO_BLOCKER2_ID,
        VehicleKind::Train,
        TileCoord::new(27, RAIL_SIGNALS_DEMO_PLAT2_Y),
        TileCoord::new(27, RAIL_SIGNALS_DEMO_PLAT2_Y),
    );
    blocker2.running = false;

    state.vehicles.push(lead);
    state.vehicles.push(blocker2);
    attach_cargo_wagon(state, RAIL_SIGNALS_DEMO_LEAD_ID, crate::ENGINE_WAGON_GOODS);
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
