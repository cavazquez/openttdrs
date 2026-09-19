//! Escenarios y helpers de carretera (familia `truck_bay`).

use std::collections::VecDeque;

use crate::command::{Command, apply_command};
use crate::industry::{Industry, IndustryKind};
use crate::map::TileCoord;
use crate::vehicle::{Vehicle, VehicleKind, VehicleOrder};
use crate::{Climate, GameState, IndustrySpec, PathNetwork, find_path};

/// Semilla fija del fixture vial inicial.
pub const FIRST_ROUTE_WORLD_SEED: u64 = 0xC0A1_1950;
/// Año de inicio del fixture vial inicial.
pub const FIRST_ROUTE_YEAR: u32 = 1950;
/// Origen de la mina de carbón del escenario `first_route`.
pub const FIRST_ROUTE_COAL_MINE: TileCoord = TileCoord::new(8, 8);
/// Origen de la central eléctrica del escenario `first_route`.
pub const FIRST_ROUTE_POWER_STATION: TileCoord = TileCoord::new(48, 8);
/// Parada de carga prevista junto a la mina.
pub const FIRST_ROUTE_LOAD_STOP: TileCoord = TileCoord::new(12, 11);
/// Parada de descarga prevista junto a la central eléctrica.
pub const FIRST_ROUTE_DELIVER_STOP: TileCoord = TileCoord::new(52, 11);
/// Depósito previsto para comprar el primer camión.
pub const FIRST_ROUTE_DEPOT: TileCoord = TileCoord::new(5, 12);
/// Dirección del depósito: salida hacia el este, sobre la carretera principal.
pub const FIRST_ROUTE_DEPOT_DIRECTION: u8 = 2;
/// Fila de la carretera que une las dos paradas previstas.
pub const FIRST_ROUTE_ROAD_Y: i32 = 12;
/// Primer `x` de la carretera prevista.
pub const FIRST_ROUTE_ROAD_START_X: i32 = 6;
/// Último `x` de la carretera prevista.
pub const FIRST_ROUTE_ROAD_END_X: i32 = 56;
/// Id que recibe el único vehículo comprado en el escenario vacío.
pub const FIRST_ROUTE_VEHICLE_ID: u32 = 1;

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

/// Mundo inicial, plano y determinista para construir un servicio de carbón.
///
/// Incluye únicamente la mina y la central eléctrica visibles; deja al jugador
/// construir por comandos la carretera, el depósito, las dos paradas y el
/// camión. Conserva sólo la entrada estática de `OpenGFX`: no carga `NewGRF`
/// dinámicos que puedan alterar la simulación.
///
/// # Panics
///
/// Si la siembra fija de industrias falla (bug del propio escenario).
#[must_use]
#[allow(clippy::expect_used)] // fixture fijo: un fallo de construcción es un bug del escenario
pub fn build_first_route() -> GameState {
    let mut state = GameState::new(64, 64);
    state.world_seed = FIRST_ROUTE_WORLD_SEED;
    state.climate = Climate::Temperate;
    state.tick = crate::news::tick_for_calendar_year(FIRST_ROUTE_YEAR);
    state.sync_timers_from_tick();
    state.ai.enabled = false;
    state.disasters_enabled = false;
    state.vehicle_breakdowns = 0;
    state.newgrf_stack.retain(|entry| entry.is_static);

    // El fixture puede sembrar industrias; no crea transporte, carga ni rutas.
    apply_command(
        &mut state,
        &Command::PlaceIndustrySpecLayout(FIRST_ROUTE_COAL_MINE, IndustrySpec::CoalMine, 0),
    )
    .expect("mina de carbón first_route");
    apply_command(
        &mut state,
        &Command::PlaceIndustrySpecLayout(FIRST_ROUTE_POWER_STATION, IndustrySpec::PowerStation, 1),
    )
    .expect("central eléctrica first_route");

    state
}

pub(crate) fn place_road_polyline(
    state: &mut GameState,
    waypoints: &[TileCoord],
) -> Vec<TileCoord> {
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
