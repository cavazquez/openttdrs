//! Serialización de vehículos y órdenes (ORDL, VEHS).
//!
//! Schema VEHS mínimo loadable por `OpenTTD` ≥15.3 (#226/#267):
//! `direction`/`owner`/`engine_type`/`x_pos`/`y_pos`/`z_pos` son
//! obligatorios — sin ellos `AfterLoad` deja `INVALID_DIR`/`INVALID_OWNER` y
//! crashea en `Train::UpdateDeltaXY` / `GetImage`.
//!
//! Tren: + `track`. ROAD (bus/camión): tesela `MP_ROAD` con roadtype válido
//! (`m4`/`M3HI`; 0 = `ROADTYPE_ROAD`). Ship: agua (`MP_WATER`). Aircraft:
//! primario + sombra encadenada (`next` REF) — `OpenTTD` exige shadow.
//!
//! Residual: tranvías, callbacks `NewGRF` de vehículos todavía no modelados y
//! parte del runtime FTA que no existe en el modelo de estado.

use super::super::SavError;
use super::super::chunks::{CH_SPARSE_TABLE, CH_TABLE};
use super::super::orders_codec::{append_ordl_orders_header, encode_vehicle_order};
use super::chunks::raw_table_chunk;
use super::codec::{write_gamma, write_str};
use super::entities::CargoPacketExport;
#[cfg(test)]
use super::entities::cargo_packet_export;
use crate::game_state::GameState;
use crate::map::{TileCoord, TileKind, coord_to_linear_index};
use crate::news::{CALENDAR_BASE_YEAR, calendar_year_day};
use crate::vehicle::{
    DIR_NE, DIR_NW, DIR_SE, DIR_SW, Vehicle, VehicleKind, VehicleOrder, VehicleOrderRuntime,
};
use std::collections::HashMap;

/// Cabeza de convoy + motor (`GVSF_FRONT | GVSF_ENGINE`).
const TRAIN_SUBTYPE_FRONT_ENGINE: u8 = 0x01 | 0x08;

/// `GroundVehicleSubtypeFlags::GVSF_WAGON` (unidad remolcada del consist).
const TRAIN_SUBTYPE_WAGON: u8 = 1 << 2;

/// `AirVehicleSubType::AIR_AIRCRAFT` (ala fija; no requiere rotor).
const AIR_AIRCRAFT: u8 = 2;
/// `AirVehicleSubType::AIR_HELICOPTER`.
const AIR_HELICOPTER: u8 = 0;
/// `AirVehicleSubType::AIR_SHADOW`.
const AIR_SHADOW: u8 = 4;
/// `AirVehicleSubType::AIR_ROTOR`.
const AIR_ROTOR: u8 = 6;

/// `VehState::Stopped` (bit 1).
const VEHSTATUS_STOPPED: u8 = 1 << 1;

/// `TRACK_BIT_X` / `TRACK_BIT_Y`.
const TRACK_BIT_X: u8 = 0x01;
const TRACK_BIT_Y: u8 = 0x02;

/// Kirby Paul Tank (primer motor rail vanilla).
const DEFAULT_OPENTTD_TRAIN_ENGINE: u16 = 0;

/// MPS Regal Bus (`engines.h` id 116).
const DEFAULT_OPENTTD_BUS_ENGINE: u16 = 116;

/// MPS Mail Truck (`engines.h` id 126).
const DEFAULT_OPENTTD_TRUCK_ENGINE: u16 = 126;

/// MPS Oil Tanker (`engines.h` id 204).
const DEFAULT_OPENTTD_SHIP_ENGINE: u16 = 204;

/// Yate Haugan (`engines.h` id 218).
const DEFAULT_OPENTTD_AIRCRAFT_ENGINE: u16 = 218;

/// `VEH_TRAIN` / `VEH_ROAD` / `VEH_SHIP` / `VEH_AIRCRAFT`.
const VEH_TRAIN: u8 = 0;
const VEH_ROAD: u8 = 1;
const VEH_SHIP: u8 = 2;
const VEH_AIRCRAFT: u8 = 3;

/// Píxeles por tesela (`TILE_SIZE`).
const TILE_SIZE: i32 = 16;

/// Convierte el índice de día del reloj Rust al `Date` que usa `OpenTTD` en
/// `Vehicle::date_of_last_service`.
pub(super) fn packed_calendar_date_from_day_index(day_index: u64) -> i32 {
    let (year, doy) = calendar_year_day(day_index);
    i32::try_from(u64::from(year) * 365 + doy.saturating_sub(1))
        .unwrap_or(i32::try_from(u64::from(CALENDAR_BASE_YEAR) * 365).unwrap_or(0))
}

fn station_id_for_pos(state: &GameState, pos: TileCoord) -> Option<u16> {
    // Las estaciones importadas conservan el `StationID` real de OpenTTD;
    // no necesariamente coincide con su posición en `GameState.stations`.
    // Sólo las estaciones creadas localmente carecen de ese campo y usan el
    // índice denso como fallback para saves sintéticos/propios.
    if let Some(station) = state.stations.iter().find(|s| s.pos == pos)
        && let Some(id) = station
            .ottd_station_id
            .and_then(|id| u16::try_from(id).ok())
    {
        return Some(id);
    }
    state
        .stations
        .iter()
        .position(|s| s.pos == pos)
        .and_then(|i| u16::try_from(i).ok())
}

fn airport_id_for_vehicle(state: &GameState, v: &Vehicle) -> u16 {
    let pos = v.airport_fta_station.unwrap_or(v.dest);
    state
        .stations
        .iter()
        .find(|station| station.pos == pos)
        .and_then(|station| station.ottd_station_id)
        .and_then(|id| u16::try_from(id).ok())
        .or_else(|| station_id_for_pos(state, pos))
        .unwrap_or(u16::MAX)
}

fn last_station_id_for_vehicle(state: &GameState, v: &Vehicle) -> u16 {
    v.last_station_visited
        .and_then(|pos| station_id_for_pos(state, pos))
        .unwrap_or(u16::MAX)
}

fn last_loading_station_id_for_vehicle(state: &GameState, v: &Vehicle) -> u16 {
    v.last_pickup_station
        .and_then(|pos| station_id_for_pos(state, pos))
        .unwrap_or(u16::MAX)
}

fn cargo_ottd_byte(v: &Vehicle) -> u8 {
    if let Some(c) = v.cargo_type.or_else(|| v.cargo_packets.primary_type()) {
        return c.temperate_id();
    }
    match v.kind {
        VehicleKind::Bus | VehicleKind::Tram | VehicleKind::Aircraft => 0, // pasajeros
        VehicleKind::Truck => 2,                                           // correo
        VehicleKind::Ship => 3,                                            // petróleo
        VehicleKind::Train => 1,                                           // carbón
    }
}

fn encode_goto_order(order: &VehicleOrder, state: &GameState, map_w: u32) -> Option<Vec<u8>> {
    encode_vehicle_order(order, |pos| station_id_for_pos(state, pos), map_w).map(|b| b.to_vec())
}

/// `TrackBits` desde el mapa, o `TRACK_BIT_X` si no hay vía legible.
fn track_bits_for(state: &GameState, pos: TileCoord) -> u8 {
    let Some(tile) = state.map.get(pos) else {
        return TRACK_BIT_X;
    };
    if !matches!(tile.kind, TileKind::Rail | TileKind::Station) {
        return TRACK_BIT_X;
    }
    let bits = tile.m5 & 0x3F;
    if bits == 0 { TRACK_BIT_X } else { bits }
}

/// Dirección diagonal coherente con el eje de vía.
fn train_direction(track: u8, dir: u8) -> u8 {
    let on_x = track & TRACK_BIT_X != 0;
    let on_y = track & TRACK_BIT_Y != 0;
    if on_x && matches!(dir, DIR_NE | DIR_SW) {
        return dir;
    }
    if on_y && matches!(dir, DIR_NW | DIR_SE) {
        return dir;
    }
    if on_y && !on_x { DIR_SE } else { DIR_NE }
}

/// `OpenTTD` exige roadtype válido en la tesela del ROAD vehicle (`AfterLoad`).
fn road_tile_ok(state: &GameState, pos: TileCoord) -> bool {
    let Some(tile) = state.map.get(pos) else {
        return false;
    };
    matches!(tile.kind, TileKind::Road | TileKind::RoadDepot)
}

fn water_tile_ok(state: &GameState, pos: TileCoord) -> bool {
    let Some(tile) = state.map.get(pos) else {
        return false;
    };
    matches!(tile.kind, TileKind::Water | TileKind::ShipDepot)
}

/// `EngineID` vanilla `OpenTTD` desde el catálogo Rust (inverso de `vanilla_train_engine_id`).
fn openttd_train_engine_type(v: &Vehicle) -> u16 {
    if let Some(native) = v.native_engine_type {
        return native;
    }
    match v.engine_id {
        Some(100) => 0,  // Kirby Paul Tank
        Some(101) => 8,  // Chaney 'Jubilee'
        Some(102) => 9,  // Ginzu 'A4'
        Some(103) => 10, // SH '8P'
        Some(104) => 11, // Manley-Morel
        Some(105) => 12, // Dash
        Some(106) => 13, // SH/Hendry '25'
        Some(107) => 14, // UU '37'
        Some(108) => 15, // Floss '47'
        Some(109) => 22, // SH '125'
        Some(110) => 23, // SH '30'
        Some(111) => 24, // SH '40'
        Some(112) => 25, // T.I.M.
        Some(113) => 26, // AsiaStar
        _ => DEFAULT_OPENTTD_TRAIN_ENGINE,
    }
}

/// `EngineID` `OpenTTD` para bus/camión (ids globales en `table/engines.h`).
fn openttd_road_engine_type(v: &Vehicle) -> u16 {
    if let Some(native) = v.native_engine_type {
        return native;
    }
    match v.kind {
        VehicleKind::Bus => match v.engine_id {
            Some(0) => 116, // MPS Regal Bus
            Some(1) => 117, // Hereford Leopard
            Some(2) => 118, // Foster Bus
            _ => DEFAULT_OPENTTD_BUS_ENGINE,
        },
        VehicleKind::Truck => match v.engine_id {
            Some(10) => 126, // MPS Mail Truck
            Some(11) => 138, // Balogh Goods
            Some(12) => 139, // Craighead Goods
            Some(13) => 140, // Goss Goods
            _ => DEFAULT_OPENTTD_TRUCK_ENGINE,
        },
        _ => DEFAULT_OPENTTD_BUS_ENGINE,
    }
}

fn openttd_ship_engine_type(v: &Vehicle) -> u16 {
    if let Some(native) = v.native_engine_type {
        return native;
    }
    match v.engine_id {
        Some(0) => 204, // MPS Oil Tanker
        Some(2) => 206, // MPS Passenger Ferry
        Some(7) => 211, // Yate Cargo ship
        _ => DEFAULT_OPENTTD_SHIP_ENGINE,
    }
}

fn openttd_aircraft_engine_type(v: &Vehicle) -> u16 {
    if let Some(native) = v.native_engine_type {
        return native;
    }
    match v.engine_id {
        Some(0) => 218,  // Yate Haugan
        Some(10) => 225, // Yate Aerospace YAC 1-11
        _ => DEFAULT_OPENTTD_AIRCRAFT_ENGINE,
    }
}

type SavRecordBytes = Vec<u8>;
type SavRecordList = Vec<SavRecordBytes>;

struct CommonWire {
    subtype: u8,
    name: Option<String>,
    owner: u8,
    tile: u32,
    x_pos: i32,
    y_pos: i32,
    z_pos: i32,
    direction: u8,
    engine_type: u16,
    cur_speed: u16,
    subspeed: u8,
    motion_counter: u32,
    progress: u8,
    vehstatus: u8,
    cargo: u8,
    cargo_subtype: u8,
    cargo_capacity: u16,
    cargo_count: u16,
    cargo_packet_refs: Vec<u32>,
    cargo_action_counts: [u32; 4],
    cargo_age_counter: u16,
    age_days: u32,
    economy_age_days: u32,
    max_age_days: u32,
    date_of_last_service: i32,
    date_of_last_service_newgrf: i32,
    order_list_ref: u32,
    cur_order: u8,
    current_order: VehicleOrderRuntime,
    /// `REF_VEHICLE`: 0 = null, resto = índice sparse + 1.
    next_ref: u32,
    /// `REF_VEHICLE` de la cadena de órdenes compartidas.
    next_shared_ref: u32,
    /// Grupo de flota (`DEFAULT_GROUP` si no está asignado).
    group_id: u16,
    /// Inicio del horario en ticks (`Vehicle::timetable_start`).
    timetable_start: u64,
    /// Tiempo transcurrido en la orden actual.
    current_order_time: u32,
    /// Retraso acumulado del horario (`lateness_counter`).
    timetable_lateness: i32,
    depot_unbunching_last_departure: u64,
    depot_unbunching_next_departure: u64,
    round_trip_time: u32,
    /// `Vehicle::vehicle_flags` (`OpenTTD` `VehicleFlags`).
    vehicle_flags: u16,
    /// Semilla de randomización de callbacks/Action2 de `NewGRF`.
    random_bits: u16,
    /// Triggers de randomización pendientes (`VehicleRandomTriggers`).
    waiting_random_triggers: u8,
    /// Última estación visitada (`StationID::Invalid()` si no existe).
    last_station_visited: u16,
    /// Última estación desde la que el vehículo pudo salir con carga.
    last_loading_station: u16,
    /// Tick absoluto de la última salida con carga.
    last_loading_tick: u64,
    /// Intervalo de servicio (`Vehicle::service_interval`).
    service_interval: u16,
    reliability: u16,
    reliability_spd_dec: u16,
    breakdown_ctr: u8,
    breakdown_delay: u8,
    breakdowns_since_last_service: u8,
    breakdown_chance: u8,
    /// Año calendario nativo de compra.
    build_year: i32,
    /// Cuenta atrás de carga/descarga.
    load_unload_ticks: u16,
    /// Campo legacy de pago de carga.
    cargo_paid_for: u16,
    profit_this_year: i64,
    profit_last_year: i64,
    /// Valor contable de la unidad (sin la fracción de 8 bits).
    value: i64,
    day_counter: u8,
    tick_counter: u8,
    running_ticks: u8,
}

/// Campos específicos de `SlVehicleAircraft` (`vehicle_sl.cpp`).
#[derive(Clone, Copy)]
struct AircraftWire {
    crashed_counter: u16,
    pos: u8,
    targetairport: u16,
    state: u8,
    previous_pos: u8,
    last_direction: u8,
    number_consecutive_turns: u8,
    turn_counter: u8,
    flags: u8,
}

/// Campos específicos de `SlVehicleShip` (`vehicle_sl.cpp`).
#[derive(Clone, Copy)]
struct ShipWire {
    state: u8,
    rotation: u8,
}

/// Campos específicos de `SlVehicleTrain` (`vehicle_sl.cpp`).
#[derive(Clone, Copy)]
struct TrainWire {
    crash_anim_pos: u16,
    force_proceed: u8,
    track: u8,
    flags: u16,
    wait_counter: u16,
    gv_flags: u16,
}

/// Campos específicos de `SlVehicleRoadVeh` que no forman parte de `common`.
struct RoadWire {
    state: u8,
    frame: u8,
    blocked_ctr: u16,
    overtaking: u8,
    overtaking_ctr: u8,
    crashed_ctr: u16,
    reverse_ctr: u8,
    gv_flags: u16,
    path: Vec<crate::vehicle::RoadPathEntry>,
}

fn road_wire_for(v: &Vehicle) -> RoadWire {
    RoadWire {
        state: v.road_state,
        frame: v.frame,
        blocked_ctr: v.blocked_ctr,
        overtaking: v.overtaking,
        overtaking_ctr: v.overtaking_ctr,
        crashed_ctr: v.crashed_ctr,
        reverse_ctr: v.reverse_ctr,
        gv_flags: v.road_gv_flags,
        path: v.road_path.clone(),
    }
}

fn train_wire_for(state: &GameState, v: &Vehicle) -> TrainWire {
    let track = if v.train_track != 0 {
        v.train_track
    } else {
        // `track_bits_for` devuelve la máscara del tile; SlVehicleTrain.track
        // guarda el índice de Track. Elegir el primer bit evita escribir la
        // máscara (que OpenTTD interpretaría como otra vía).
        let bits = track_bits_for(state, v.pos);
        match bits.trailing_zeros() {
            1 => 1,
            2 => 2,
            3 => 3,
            4 => 4,
            5 => 5,
            _ => 0,
        }
    };
    TrainWire {
        crash_anim_pos: v.train_crash_anim_pos,
        force_proceed: u8::from(v.force_proceed),
        track,
        flags: v.train_flags,
        wait_counter: u16::try_from(v.wait_counter).unwrap_or(u16::MAX),
        gv_flags: v.train_gv_flags,
    }
}

fn ship_wire_for(v: &Vehicle) -> ShipWire {
    // `Ship::state` puede conservar estados especiales (depósito/wormhole).
    // Para una unidad creada desde JSON sin snapshot raw, reconstruir sólo la
    // TrackBit correspondiente mantiene una salida SAV navegable.
    let state = if v.ship_state != 0 {
        v.ship_state
    } else {
        match v.ship_track {
            crate::ship_movement::TRACK_X => 1,
            crate::ship_movement::TRACK_Y => 2,
            crate::ship_movement::TRACK_UPPER => 4,
            crate::ship_movement::TRACK_LOWER => 8,
            crate::ship_movement::TRACK_LEFT => 16,
            crate::ship_movement::TRACK_RIGHT => 32,
            _ => 0,
        }
    };
    ShipWire {
        state,
        rotation: v.ship_rotation,
    }
}

fn aircraft_wire_for(state: &GameState, v: &Vehicle) -> AircraftWire {
    AircraftWire {
        crashed_counter: v.crashed_ctr,
        pos: v.airport_pos,
        targetairport: airport_id_for_vehicle(state, v),
        state: v.airport_heading.as_u8(),
        previous_pos: v.airport_prev_pos,
        last_direction: v.direction,
        number_consecutive_turns: v.aircraft_number_consecutive_turns,
        turn_counter: v.aircraft_turn_counter,
        flags: v.aircraft_flags,
    }
}

fn write_aircraft_fields(buf: &mut Vec<u8>, aircraft: &AircraftWire) {
    buf.extend_from_slice(&aircraft.crashed_counter.to_be_bytes());
    buf.push(aircraft.pos);
    buf.extend_from_slice(&aircraft.targetairport.to_be_bytes());
    buf.push(aircraft.state);
    buf.push(aircraft.previous_pos);
    buf.push(aircraft.last_direction);
    buf.push(aircraft.number_consecutive_turns);
    buf.push(aircraft.turn_counter);
    buf.push(aircraft.flags);
}

fn write_ship_fields(buf: &mut Vec<u8>, ship: ShipWire) {
    buf.push(ship.state);
    buf.push(ship.rotation);
}

fn write_train_fields(buf: &mut Vec<u8>, train: TrainWire) {
    buf.extend_from_slice(&train.crash_anim_pos.to_be_bytes());
    buf.push(train.force_proceed);
    buf.push(train.track);
    buf.extend_from_slice(&train.flags.to_be_bytes());
    buf.extend_from_slice(&train.wait_counter.to_be_bytes());
    buf.extend_from_slice(&train.gv_flags.to_be_bytes());
}

fn write_road_fields(buf: &mut Vec<u8>, road: &RoadWire) -> Result<(), SavError> {
    buf.push(road.state);
    buf.push(road.frame);
    buf.extend_from_slice(&road.blocked_ctr.to_be_bytes());
    buf.push(road.overtaking);
    buf.push(road.overtaking_ctr);
    buf.extend_from_slice(&road.crashed_ctr.to_be_bytes());
    buf.push(road.reverse_ctr);
    write_gamma(
        u32::try_from(road.path.len()).map_err(|_| SavError::ValueOutOfRange {
            field: "road path count",
            value: u32::MAX,
        })?,
        buf,
    )?;
    for entry in &road.path {
        buf.push(entry.trackdir);
        buf.extend_from_slice(&entry.tile.to_be_bytes());
    }
    buf.extend_from_slice(&road.gv_flags.to_be_bytes());
    Ok(())
}

fn cargo_action_counts_for(v: &Vehicle, cargo_count: u32) -> [u32; 4] {
    let counts = v.cargo_packets.action_counts;
    let total = counts.iter().copied().fold(0_u32, u32::saturating_add);
    if total == cargo_count {
        counts
    } else if cargo_count == 0 {
        [0; 4]
    } else {
        // Una partida JSON/legacy sólo tiene el contador agregado. OpenTTD
        // considera esa carga conservada hasta que una parada la clasifica.
        [0, 0, cargo_count, 0]
    }
}

fn write_vehs_common(buf: &mut Vec<u8>, c: &CommonWire) -> Result<(), SavError> {
    buf.push(c.subtype);
    write_str(c.name.as_deref().unwrap_or(""), buf)?;
    buf.push(c.owner);
    buf.extend_from_slice(&c.tile.to_be_bytes());
    buf.extend_from_slice(&u32::try_from(c.x_pos).unwrap_or(0).to_be_bytes());
    buf.extend_from_slice(&u32::try_from(c.y_pos).unwrap_or(0).to_be_bytes());
    buf.extend_from_slice(&c.z_pos.to_be_bytes());
    buf.push(c.direction);
    buf.extend_from_slice(&c.engine_type.to_be_bytes());
    buf.extend_from_slice(&c.cur_speed.to_be_bytes());
    buf.push(c.subspeed);
    buf.extend_from_slice(&c.motion_counter.to_be_bytes());
    buf.push(c.progress);
    buf.push(c.vehstatus);
    buf.push(c.cargo);
    buf.push(c.cargo_subtype);
    buf.extend_from_slice(&c.cargo_capacity.to_be_bytes());
    buf.extend_from_slice(&c.cargo_count.to_be_bytes());
    write_gamma(
        u32::try_from(c.cargo_packet_refs.len()).map_err(|_| SavError::ValueOutOfRange {
            field: "vehicle cargo packet count",
            value: u32::MAX,
        })?,
        buf,
    )?;
    for packet_id in &c.cargo_packet_refs {
        buf.extend_from_slice(&packet_id.saturating_add(1).to_be_bytes());
    }
    write_gamma(4, buf)?;
    for count in c.cargo_action_counts {
        buf.extend_from_slice(&count.to_be_bytes());
    }
    buf.extend_from_slice(&c.cargo_age_counter.to_be_bytes());
    buf.extend_from_slice(&i32::try_from(c.age_days).unwrap_or(i32::MAX).to_be_bytes());
    buf.extend_from_slice(
        &i32::try_from(c.economy_age_days)
            .unwrap_or(i32::MAX)
            .to_be_bytes(),
    );
    buf.extend_from_slice(
        &i32::try_from(c.max_age_days)
            .unwrap_or(i32::MAX)
            .to_be_bytes(),
    );
    buf.extend_from_slice(&c.date_of_last_service.to_be_bytes());
    buf.extend_from_slice(&c.date_of_last_service_newgrf.to_be_bytes());
    buf.extend_from_slice(&c.order_list_ref.to_be_bytes());
    buf.push(c.cur_order);
    buf.push(c.current_order.order_type);
    buf.push(c.current_order.flags);
    buf.extend_from_slice(&c.current_order.dest.to_be_bytes());
    buf.push(c.current_order.refit_cargo);
    buf.extend_from_slice(&c.current_order.wait_time.to_be_bytes());
    buf.extend_from_slice(&c.current_order.travel_time.to_be_bytes());
    buf.extend_from_slice(&c.current_order.max_speed.to_be_bytes());
    buf.extend_from_slice(&c.next_ref.to_be_bytes());
    buf.extend_from_slice(&c.group_id.to_be_bytes());
    buf.extend_from_slice(&c.timetable_start.to_be_bytes());
    buf.extend_from_slice(&c.current_order_time.to_be_bytes());
    buf.extend_from_slice(&c.timetable_lateness.to_be_bytes());
    buf.extend_from_slice(&c.vehicle_flags.to_be_bytes());
    buf.extend_from_slice(&c.random_bits.to_be_bytes());
    buf.push(c.waiting_random_triggers);
    buf.extend_from_slice(&c.next_shared_ref.to_be_bytes());
    buf.extend_from_slice(&c.last_station_visited.to_be_bytes());
    buf.extend_from_slice(&c.last_loading_station.to_be_bytes());
    buf.extend_from_slice(&c.service_interval.to_be_bytes());
    buf.extend_from_slice(&c.reliability.to_be_bytes());
    buf.extend_from_slice(&c.reliability_spd_dec.to_be_bytes());
    buf.push(c.breakdown_ctr);
    buf.push(c.breakdown_delay);
    buf.push(c.breakdowns_since_last_service);
    buf.push(c.breakdown_chance);
    buf.extend_from_slice(&c.build_year.to_be_bytes());
    buf.extend_from_slice(&c.load_unload_ticks.to_be_bytes());
    buf.extend_from_slice(&c.cargo_paid_for.to_be_bytes());
    buf.extend_from_slice(&c.profit_this_year.saturating_mul(256).to_be_bytes());
    buf.extend_from_slice(&c.profit_last_year.saturating_mul(256).to_be_bytes());
    buf.extend_from_slice(&c.value.saturating_mul(256).to_be_bytes());
    buf.extend_from_slice(&c.last_loading_tick.to_be_bytes());
    buf.push(c.day_counter);
    buf.push(c.tick_counter);
    buf.push(c.running_ticks);
    buf.extend_from_slice(&c.depot_unbunching_last_departure.to_be_bytes());
    buf.extend_from_slice(&c.depot_unbunching_next_departure.to_be_bytes());
    buf.extend_from_slice(
        &i32::try_from(c.round_trip_time)
            .unwrap_or(i32::MAX)
            .to_be_bytes(),
    );
    Ok(())
}

/// Bits de `VehicleFlags` que el core modela hoy.
fn vehicle_flags_for(v: &Vehicle) -> u16 {
    let mut flags = v.vehicle_flags;
    if v.timetable_started {
        flags |= 1 << 3; // VehicleFlag::TimetableStarted
    }
    if v.timetable_autofill {
        flags |= 1 << 4; // VehicleFlag::AutofillTimetable
    }
    flags
}

fn push_order_list(
    orders: &[VehicleOrder],
    state: &GameState,
    map_w: u32,
    ordl: &mut SavRecordList,
) -> Result<u32, SavError> {
    let mut order_bytes = Vec::new();
    for order in orders {
        if let Some(enc) = encode_goto_order(order, state, map_w) {
            order_bytes.push(enc);
        }
    }
    if order_bytes.is_empty() {
        return Ok(0);
    }
    let list_idx = u32::try_from(ordl.len()).unwrap_or(0);
    let mut rec = Vec::new();
    write_gamma(order_bytes.len() as u32, &mut rec)?;
    for o in &order_bytes {
        rec.extend_from_slice(o);
    }
    ordl.push(rec);
    Ok(list_idx + 1)
}

fn push_orders(
    v: &Vehicle,
    state: &GameState,
    map_w: u32,
    ordl: &mut SavRecordList,
) -> Result<u32, SavError> {
    push_order_list(&v.orders, state, map_w, ordl)
}

fn current_order_runtime_for(v: &Vehicle, state: &GameState, map_w: u32) -> VehicleOrderRuntime {
    if let Some(runtime) = v.current_order_state {
        return runtime;
    }
    let Some(order) = v.orders.get(v.current_order) else {
        return VehicleOrderRuntime {
            order_type: 0,
            flags: 0,
            dest: 0,
            refit_cargo: 0xFF,
            wait_time: 0,
            travel_time: 0,
            max_speed: u16::MAX,
        };
    };
    let Some(encoded) = encode_vehicle_order(order, |pos| station_id_for_pos(state, pos), map_w)
    else {
        return VehicleOrderRuntime {
            order_type: 0,
            flags: 0,
            dest: 0,
            refit_cargo: 0xFF,
            wait_time: 0,
            travel_time: 0,
            max_speed: u16::MAX,
        };
    };
    VehicleOrderRuntime {
        order_type: encoded[0],
        flags: encoded[1],
        dest: u16::from_be_bytes([encoded[2], encoded[3]]),
        refit_cargo: encoded[4],
        wait_time: u16::from_be_bytes([encoded[5], encoded[6]]),
        travel_time: u16::from_be_bytes([encoded[7], encoded[8]]),
        max_speed: u16::from_be_bytes([encoded[9], encoded[10]]),
    }
}

#[allow(clippy::too_many_arguments)]
fn common_wire_for(
    v: &Vehicle,
    current_tick: u64,
    tile_idx: u32,
    direction: u8,
    engine_type: u16,
    order_list_ref: u32,
    subtype: u8,
    next_ref: u32,
    next_shared_ref: u32,
    last_station_visited: u16,
    last_loading_station: u16,
    current_order: VehicleOrderRuntime,
    cargo_packet_refs: &[u32],
) -> CommonWire {
    let cargo = cargo_ottd_byte(v);
    let cargo_count = if v.cargo_packets.is_empty() {
        v.cargo
    } else {
        v.cargo_packets.total()
    };
    let cargo_action_counts = cargo_action_counts_for(v, cargo_count);
    let cur_order = u8::try_from(v.current_order.min(255)).unwrap_or(0);
    let vehstatus = if v.running { 0 } else { VEHSTATUS_STOPPED };
    let x_pos = v.pos.x * TILE_SIZE + i32::from(v.rail_pixel.min(15));
    let y_pos = v.pos.y * TILE_SIZE + TILE_SIZE / 2;
    let z_pos = i32::from(v.z_pos.unwrap_or(0));
    let age_days = v.vehicle_age_days(current_tick).min(u64::from(u32::MAX));
    let economy_age_days = if v.economy_age_days == 0 {
        age_days
    } else {
        u64::from(v.economy_age_days)
    };
    let date_of_last_service = packed_calendar_date_from_day_index(v.last_service_day);
    let date_of_last_service_newgrf = if v.last_service_newgrf_day == 0 {
        date_of_last_service
    } else {
        packed_calendar_date_from_day_index(u64::try_from(v.last_service_newgrf_day).unwrap_or(0))
    };
    let group_id = v.group_id.unwrap_or(0xFFFE).min(u32::from(u16::MAX)) as u16;
    CommonWire {
        subtype,
        name: v.name.clone(),
        owner: v.owner.0,
        tile: tile_idx,
        x_pos,
        y_pos,
        z_pos,
        direction,
        engine_type,
        cur_speed: v.cur_speed,
        subspeed: v.subspeed,
        motion_counter: v.motion_counter,
        progress: v.progress,
        vehstatus,
        cargo,
        cargo_subtype: v.cargo_subtype,
        cargo_capacity: u16::try_from(v.capacity).unwrap_or(u16::MAX),
        cargo_count: u16::try_from(cargo_count).unwrap_or(u16::MAX),
        cargo_packet_refs: cargo_packet_refs.to_vec(),
        cargo_action_counts,
        cargo_age_counter: v.cargo_age_counter,
        age_days: u32::try_from(age_days).unwrap_or(u32::MAX),
        economy_age_days: u32::try_from(economy_age_days).unwrap_or(u32::MAX),
        max_age_days: v.max_age_days,
        date_of_last_service,
        date_of_last_service_newgrf,
        order_list_ref,
        cur_order,
        current_order,
        next_ref,
        next_shared_ref,
        group_id,
        timetable_start: u64::from(v.timetable_start),
        current_order_time: v.current_order_time,
        timetable_lateness: v.timetable_lateness,
        depot_unbunching_last_departure: v.depot_unbunching_last_departure,
        depot_unbunching_next_departure: v.depot_unbunching_next_departure,
        round_trip_time: v.round_trip_time,
        vehicle_flags: vehicle_flags_for(v),
        random_bits: v.newgrf_random_bits,
        waiting_random_triggers: v.newgrf_waiting_random_triggers,
        last_station_visited,
        last_loading_station,
        last_loading_tick: v.last_depart_tick.unwrap_or(0),
        service_interval: v.service_interval_days,
        reliability: v.reliability,
        reliability_spd_dec: v.reliability_spd_dec,
        breakdown_ctr: v.breakdown_ctr,
        breakdown_delay: v.breakdown_delay,
        breakdowns_since_last_service: v.breakdowns_since_last_service,
        breakdown_chance: v.breakdown_chance,
        build_year: build_year_for_vehicle(v, current_tick),
        load_unload_ticks: v.load_unload_ticks,
        cargo_paid_for: v.cargo_paid_for,
        profit_this_year: v.profit_this_year,
        profit_last_year: v.profit_last_year,
        value: v.value,
        day_counter: v.newgrf_day_counter,
        tick_counter: v.newgrf_tick_counter,
        running_ticks: v.running_ticks,
    }
}

fn build_year_for_vehicle(v: &Vehicle, current_tick: u64) -> i32 {
    if v.build_year != 0 {
        return i32::try_from(v.build_year).unwrap_or(i32::MAX);
    }
    let age_days = v.vehicle_age_days(current_tick);
    let build_tick = current_tick
        .saturating_sub(age_days.saturating_mul(u64::from(crate::economy::TICKS_PER_DAY)));
    let (year, _) = crate::news::calendar_year_day(crate::news::calendar_day_index(
        crate::tick::GameTick::new(build_tick),
    ));
    i32::try_from(year).unwrap_or(i32::MAX)
}

fn cargo_packet_refs_for<'a>(export: &'a CargoPacketExport, v: &Vehicle) -> &'a [u32] {
    export
        .vehicle_refs
        .get(&v.id)
        .map_or(&[][..], Vec::as_slice)
}

fn sparse_vehicle_ref(sparse_by_vehicle_id: &HashMap<u32, u32>, id: Option<u32>) -> u32 {
    id.and_then(|vehicle_id| sparse_by_vehicle_id.get(&vehicle_id).copied())
        .map_or(0, |index| index.saturating_add(1))
}

fn diag_direction(v: &Vehicle) -> u8 {
    if matches!(v.direction, DIR_NE | DIR_SE | DIR_SW | DIR_NW) {
        v.direction
    } else {
        DIR_NE
    }
}

fn push_typed_vehicle(
    rec: &mut Vec<u8>,
    veh_type: u8,
    common: &CommonWire,
    train_runtime: Option<TrainWire>,
    road_runtime: Option<&RoadWire>,
    ship_runtime: Option<&ShipWire>,
    aircraft_runtime: Option<&AircraftWire>,
) -> Result<(), SavError> {
    rec.push(veh_type);
    for t in VEH_TRAIN..=VEH_AIRCRAFT {
        if t == veh_type {
            write_gamma(1, rec)?; // struct presente
            write_gamma(1, rec)?; // common presente
            write_vehs_common(rec, common)?;
            if let Some(train) = train_runtime {
                write_train_fields(rec, train);
            }
            if let Some(road) = road_runtime {
                write_road_fields(rec, road)?;
            }
            if let Some(ship) = ship_runtime {
                write_ship_fields(rec, *ship);
            }
            if let Some(aircraft) = aircraft_runtime {
                write_aircraft_fields(rec, aircraft);
            }
        } else {
            write_gamma(0, rec)?;
        }
    }
    Ok(())
}

/// ORDL + VEHS: tren, ROAD, ship y aircraft (ala fija/helicóptero + auxiliares).
///
/// # Errors
///
/// Falla si algún valor gamma está fuera de rango.
#[allow(clippy::too_many_lines)]
#[cfg(test)]
pub(super) fn ordl_and_vehs_records(
    state: &GameState,
    map_w: u32,
) -> Result<(SavRecordList, SavRecordList), SavError> {
    let cargo_export = cargo_packet_export(state, map_w);
    ordl_and_vehs_records_with_cargo(state, map_w, &cargo_export)
}

#[allow(clippy::too_many_lines)]
pub(crate) fn ordl_and_vehs_records_with_cargo(
    state: &GameState,
    map_w: u32,
    cargo_export: &CargoPacketExport,
) -> Result<(SavRecordList, SavRecordList), SavError> {
    let mut ordl = Vec::new();
    let mut vehs = Vec::new();

    // Primero construimos la tabla sparse que realmente se va a emitir. La
    // referencia `next` de VEHS no apunta al id de nuestro JSON sino al índice
    // de la tabla sparse + 1; sin esta pasada los consist cuyo orden en el
    // vector no coincide con su cadena `next_unit` quedaban truncados.
    let eligible: Vec<usize> = state
        .vehicles
        .iter()
        .enumerate()
        .filter_map(|(idx, v)| {
            let is_train = v.kind == VehicleKind::Train;
            let is_road = matches!(v.kind, VehicleKind::Bus | VehicleKind::Truck);
            let is_ship = v.kind == VehicleKind::Ship;
            let is_air = v.kind == VehicleKind::Aircraft;
            if !is_train && !is_road && !is_ship && !is_air {
                return None;
            }
            if is_road && !road_tile_ok(state, v.pos) {
                return None;
            }
            if is_ship && !water_tile_ok(state, v.pos) {
                return None;
            }
            coord_to_linear_index(v.pos, map_w).map(|_| idx)
        })
        .collect();
    let mut sparse_by_vehicle_id = HashMap::with_capacity(eligible.len());
    let mut sparse_idx = 0u32;
    for &vehicle_idx in &eligible {
        let v = &state.vehicles[vehicle_idx];
        sparse_by_vehicle_id.insert(v.id, sparse_idx);
        sparse_idx = sparse_idx.saturating_add(if v.kind == VehicleKind::Aircraft {
            if v.engine_id
                .is_some_and(crate::engine::aircraft_is_helicopter)
            {
                3
            } else {
                2
            }
        } else {
            1
        });
    }

    // `fleet::fleet_chunks` emite GRPS/ERNW por separado. Aquí sólo se asigna
    // la referencia `ORDL`: varios vehículos de una lista compartida deben
    // reutilizar una sola fila y conservar su identidad en `VEHS.common.orders`.
    let mut shared_order_refs = HashMap::<u32, u32>::new();
    sparse_idx = 0;

    for &vehicle_idx in &eligible {
        let v = &state.vehicles[vehicle_idx];
        let is_train = v.kind == VehicleKind::Train;
        let is_road = matches!(v.kind, VehicleKind::Bus | VehicleKind::Truck);
        let is_air = v.kind == VehicleKind::Aircraft;
        let Some(tile_idx) = coord_to_linear_index(v.pos, map_w) else {
            continue;
        };

        let order_list_ref = if let Some(shared_id) = v.shared_order_id {
            if let Some(existing) = shared_order_refs.get(&shared_id) {
                *existing
            } else if let Some(shared) = state.shared_order_lists.iter().find(|l| l.id == shared_id)
            {
                let list_ref = push_order_list(&shared.orders, state, map_w, &mut ordl)?;
                shared_order_refs.insert(shared_id, list_ref);
                list_ref
            } else {
                // Unlinked/malformed ids are tolerated for old JSON saves;
                // preserve the vehicle-local orders rather than dropping them.
                push_orders(v, state, map_w, &mut ordl)?
            }
        } else {
            push_orders(v, state, map_w, &mut ordl)?
        };
        let direction = if is_train {
            let track = track_bits_for(state, v.pos);
            train_direction(track, v.direction)
        } else {
            diag_direction(v)
        };

        if is_air {
            let is_helicopter = v
                .engine_id
                .is_some_and(crate::engine::aircraft_is_helicopter);
            // Primario + sombra (y rotor para helicópteros). OpenTTD exige
            // ambos auxiliares al cargar un `Aircraft` normal.
            let shadow_idx = sparse_idx + 1;
            let next_ref = shadow_idx + 1; // REF = index+1
            let engine_type = openttd_aircraft_engine_type(v);
            let aircraft_runtime = aircraft_wire_for(state, v);
            let last_station_visited = last_station_id_for_vehicle(state, v);

            let mut primary = Vec::new();
            write_gamma(sparse_idx, &mut primary)?;
            push_typed_vehicle(
                &mut primary,
                VEH_AIRCRAFT,
                &common_wire_for(
                    v,
                    state.tick.get(),
                    tile_idx,
                    direction,
                    engine_type,
                    order_list_ref,
                    if is_helicopter {
                        AIR_HELICOPTER
                    } else {
                        AIR_AIRCRAFT
                    },
                    next_ref,
                    sparse_vehicle_ref(&sparse_by_vehicle_id, v.next_shared_vehicle_id),
                    last_station_visited,
                    last_loading_station_id_for_vehicle(state, v),
                    current_order_runtime_for(v, state, map_w),
                    cargo_packet_refs_for(cargo_export, v),
                ),
                None,
                None,
                None,
                Some(&aircraft_runtime),
            )?;
            vehs.push(primary);
            sparse_idx += 1;

            let mut shadow = Vec::new();
            write_gamma(sparse_idx, &mut shadow)?;
            push_typed_vehicle(
                &mut shadow,
                VEH_AIRCRAFT,
                &CommonWire {
                    subtype: AIR_SHADOW,
                    name: None,
                    owner: v.owner.0,
                    tile: tile_idx,
                    x_pos: v.pos.x * TILE_SIZE + TILE_SIZE / 2,
                    y_pos: v.pos.y * TILE_SIZE + TILE_SIZE / 2,
                    z_pos: i32::from(v.z_pos.unwrap_or(0)),
                    direction,
                    engine_type,
                    vehstatus: VEHSTATUS_STOPPED,
                    cargo: 0,
                    cargo_subtype: 0,
                    cargo_capacity: 0,
                    cargo_count: 0,
                    cargo_packet_refs: Vec::new(),
                    cargo_action_counts: [0; 4],
                    cargo_age_counter: 0,
                    age_days: 0,
                    economy_age_days: 0,
                    max_age_days: 0,
                    date_of_last_service: 0,
                    date_of_last_service_newgrf: 0,
                    order_list_ref: 0,
                    cur_order: 0,
                    current_order: VehicleOrderRuntime {
                        order_type: 0,
                        flags: 0,
                        dest: 0,
                        refit_cargo: 0xFF,
                        wait_time: 0,
                        travel_time: 0,
                        max_speed: u16::MAX,
                    },
                    // Para helicópteros la sombra encadena al rotor. En el
                    // formato SAV `REF_VEHICLE` es sparse index + 1.
                    next_ref: if is_helicopter { sparse_idx + 2 } else { 0 },
                    next_shared_ref: 0,
                    group_id: 0xFFFE,
                    timetable_start: 0,
                    current_order_time: 0,
                    timetable_lateness: 0,
                    depot_unbunching_last_departure: 0,
                    depot_unbunching_next_departure: 0,
                    round_trip_time: 0,
                    vehicle_flags: 0,
                    random_bits: 0,
                    waiting_random_triggers: 0,
                    last_station_visited: u16::MAX,
                    last_loading_station: u16::MAX,
                    last_loading_tick: 0,
                    service_interval: 0,
                    reliability: 0,
                    reliability_spd_dec: 0,
                    breakdown_ctr: 0,
                    breakdown_delay: 0,
                    breakdowns_since_last_service: 0,
                    breakdown_chance: 0,
                    build_year: 0,
                    load_unload_ticks: 0,
                    cargo_paid_for: 0,
                    profit_this_year: 0,
                    profit_last_year: 0,
                    value: 0,
                    day_counter: 0,
                    tick_counter: 0,
                    running_ticks: 0,
                    cur_speed: 0,
                    subspeed: 0,
                    motion_counter: 0,
                    progress: 0,
                },
                None,
                None,
                None,
                Some(&aircraft_runtime),
            )?;
            vehs.push(shadow);
            sparse_idx += 1;
            if is_helicopter {
                let mut rotor = Vec::new();
                write_gamma(sparse_idx, &mut rotor)?;
                push_typed_vehicle(
                    &mut rotor,
                    VEH_AIRCRAFT,
                    &CommonWire {
                        subtype: AIR_ROTOR,
                        name: None,
                        owner: v.owner.0,
                        tile: tile_idx,
                        x_pos: v.pos.x * TILE_SIZE + TILE_SIZE / 2,
                        y_pos: v.pos.y * TILE_SIZE + TILE_SIZE / 2,
                        z_pos: i32::from(v.z_pos.unwrap_or(0)) + 5,
                        direction,
                        engine_type,
                        vehstatus: VEHSTATUS_STOPPED,
                        cargo: 0,
                        cargo_subtype: 0,
                        cargo_capacity: 0,
                        cargo_count: 0,
                        cargo_packet_refs: Vec::new(),
                        cargo_action_counts: [0; 4],
                        cargo_age_counter: 0,
                        age_days: 0,
                        economy_age_days: 0,
                        max_age_days: 0,
                        date_of_last_service: 0,
                        date_of_last_service_newgrf: 0,
                        order_list_ref: 0,
                        cur_order: 0,
                        current_order: VehicleOrderRuntime {
                            order_type: 0,
                            flags: 0,
                            dest: 0,
                            refit_cargo: 0xFF,
                            wait_time: 0,
                            travel_time: 0,
                            max_speed: u16::MAX,
                        },
                        next_ref: 0,
                        next_shared_ref: 0,
                        group_id: 0xFFFE,
                        timetable_start: 0,
                        current_order_time: 0,
                        timetable_lateness: 0,
                        depot_unbunching_last_departure: 0,
                        depot_unbunching_next_departure: 0,
                        round_trip_time: 0,
                        vehicle_flags: 0,
                        random_bits: 0,
                        waiting_random_triggers: 0,
                        last_station_visited: u16::MAX,
                        last_loading_station: u16::MAX,
                        last_loading_tick: 0,
                        service_interval: 0,
                        reliability: 0,
                        reliability_spd_dec: 0,
                        breakdown_ctr: 0,
                        breakdown_delay: 0,
                        breakdowns_since_last_service: 0,
                        breakdown_chance: 0,
                        build_year: 0,
                        load_unload_ticks: 0,
                        cargo_paid_for: 0,
                        profit_this_year: 0,
                        profit_last_year: 0,
                        value: 0,
                        day_counter: 0,
                        tick_counter: 0,
                        running_ticks: 0,
                        cur_speed: 32,
                        subspeed: 0,
                        motion_counter: 0,
                        progress: 0,
                    },
                    None,
                    None,
                    None,
                    Some(&aircraft_runtime),
                )?;
                vehs.push(rotor);
                sparse_idx += 1;
            }
            continue;
        }

        let mut rec = Vec::new();
        write_gamma(sparse_idx, &mut rec)?;

        if is_train {
            let train_runtime = train_wire_for(state, v);
            let engine_type = openttd_train_engine_type(v);
            let subtype = if v.prev_unit.is_none() {
                TRAIN_SUBTYPE_FRONT_ENGINE
            } else {
                TRAIN_SUBTYPE_WAGON
            };
            let next_ref = v
                .next_unit
                .and_then(|next_id| sparse_by_vehicle_id.get(&next_id).copied())
                .map_or(0, |idx| idx.saturating_add(1));
            let next_shared_ref =
                sparse_vehicle_ref(&sparse_by_vehicle_id, v.next_shared_vehicle_id);
            push_typed_vehicle(
                &mut rec,
                VEH_TRAIN,
                &common_wire_for(
                    v,
                    state.tick.get(),
                    tile_idx,
                    direction,
                    engine_type,
                    order_list_ref,
                    subtype,
                    next_ref,
                    next_shared_ref,
                    last_station_id_for_vehicle(state, v),
                    last_loading_station_id_for_vehicle(state, v),
                    current_order_runtime_for(v, state, map_w),
                    cargo_packet_refs_for(cargo_export, v),
                ),
                Some(train_runtime),
                None,
                None,
                None,
            )?;
        } else if is_road {
            let engine_type = openttd_road_engine_type(v);
            let road_runtime = road_wire_for(v);
            push_typed_vehicle(
                &mut rec,
                VEH_ROAD,
                &common_wire_for(
                    v,
                    state.tick.get(),
                    tile_idx,
                    direction,
                    engine_type,
                    order_list_ref,
                    TRAIN_SUBTYPE_FRONT_ENGINE,
                    0,
                    sparse_vehicle_ref(&sparse_by_vehicle_id, v.next_shared_vehicle_id),
                    last_station_id_for_vehicle(state, v),
                    last_loading_station_id_for_vehicle(state, v),
                    current_order_runtime_for(v, state, map_w),
                    cargo_packet_refs_for(cargo_export, v),
                ),
                None,
                Some(&road_runtime),
                None,
                None,
            )?;
        } else {
            // ship
            let engine_type = openttd_ship_engine_type(v);
            let ship_runtime = ship_wire_for(v);
            push_typed_vehicle(
                &mut rec,
                VEH_SHIP,
                &common_wire_for(
                    v,
                    state.tick.get(),
                    tile_idx,
                    direction,
                    engine_type,
                    order_list_ref,
                    TRAIN_SUBTYPE_FRONT_ENGINE,
                    0,
                    sparse_vehicle_ref(&sparse_by_vehicle_id, v.next_shared_vehicle_id),
                    last_station_id_for_vehicle(state, v),
                    last_loading_station_id_for_vehicle(state, v),
                    current_order_runtime_for(v, state, map_w),
                    cargo_packet_refs_for(cargo_export, v),
                ),
                None,
                None,
                Some(&ship_runtime),
                None,
            )?;
        }

        vehs.push(rec);
        sparse_idx += 1;
    }
    Ok((ordl, vehs))
}

fn append_field(header: &mut Vec<u8>, ftype: u8, name: &str) -> Result<(), SavError> {
    header.push(ftype);
    write_str(name, header)
}

/// Header VEHS mínimo (train + roadveh + ship + aircraft) alineado con `vehicle_sl.cpp`.
fn append_vehs_header(header: &mut Vec<u8>) -> Result<(), SavError> {
    append_field(header, 2, "type")?; // SAVEBYTE → U8
    append_field(header, 0x1B, "train")?;
    append_field(header, 0x1B, "roadveh")?;
    append_field(header, 0x1B, "ship")?;
    append_field(header, 0x1B, "aircraft")?;
    header.push(0);

    // train → common + estado runtime específico
    append_field(header, 0x1B, "common")?;
    append_field(header, 4, "crash_anim_pos")?;
    append_field(header, 2, "force_proceed")?;
    append_field(header, 2, "track")?;
    append_field(header, 4, "flags")?;
    append_field(header, 4, "wait_counter")?;
    append_field(header, 4, "gv_flags")?;
    header.push(0);
    append_vehs_common_fields(header)?;

    // roadveh → common + estado de conducción + caché de ruta.
    append_field(header, 0x1B, "common")?;
    append_field(header, 2, "state")?;
    append_field(header, 2, "frame")?;
    append_field(header, 4, "blocked_ctr")?;
    append_field(header, 2, "overtaking")?;
    append_field(header, 2, "overtaking_ctr")?;
    append_field(header, 4, "crashed_ctr")?;
    append_field(header, 2, "reverse_ctr")?;
    append_field(header, 0x1B, "path")?;
    append_field(header, 4, "gv_flags")?;
    header.push(0);
    append_vehs_common_fields(header)?;
    append_field(header, 2, "trackdir")?;
    append_field(header, 6, "tile")?;
    header.push(0);

    // ship → common + estado/rotación persistentes. El path cache se deja
    // fuera hasta contar con una representación de Trackdir interoperable.
    append_field(header, 0x1B, "common")?;
    append_field(header, 2, "state")?;
    append_field(header, 2, "rotation")?;
    header.push(0);
    append_vehs_common_fields(header)?;

    // aircraft → common + `SlVehicleAircraft` (FTA y destino)
    append_field(header, 0x1B, "common")?;
    append_field(header, 4, "crashed_counter")?;
    append_field(header, 2, "pos")?;
    append_field(header, 4, "targetairport")?;
    append_field(header, 2, "state")?;
    append_field(header, 2, "previous_pos")?;
    append_field(header, 2, "last_direction")?;
    append_field(header, 2, "number_consecutive_turns")?;
    append_field(header, 2, "turn_counter")?;
    append_field(header, 2, "flags")?;
    header.push(0);
    append_vehs_common_fields(header)?;
    Ok(())
}

fn append_vehs_common_fields(header: &mut Vec<u8>) -> Result<(), SavError> {
    append_field(header, 2, "subtype")?;
    append_field(header, 0x0A, "name")?;
    append_field(header, 2, "owner")?;
    append_field(header, 6, "tile")?;
    append_field(header, 6, "x_pos")?;
    append_field(header, 6, "y_pos")?;
    append_field(header, 5, "z_pos")?; // SLE_FILE_I32
    append_field(header, 2, "direction")?;
    append_field(header, 4, "engine_type")?;
    append_field(header, 4, "cur_speed")?;
    append_field(header, 2, "subspeed")?;
    append_field(header, 6, "motion_counter")?; // SLE_UINT32
    append_field(header, 2, "progress")?;
    append_field(header, 2, "vehstatus")?;
    append_field(header, 2, "cargo_type")?;
    append_field(header, 2, "cargo_subtype")?;
    append_field(header, 4, "cargo_cap")?;
    append_field(header, 4, "cargo_count")?;
    append_field(header, 0x16, "cargo.packets")?; // REF_CARGO_PACKET list
    append_field(header, 0x16, "cargo.action_counts")?; // SLE_CONDARR u32[4]
    append_field(header, 4, "cargo_age_counter")?;
    append_field(header, 5, "age")?;
    append_field(header, 5, "economy_age")?;
    append_field(header, 5, "max_age")?;
    append_field(header, 5, "date_of_last_service")?;
    append_field(header, 5, "date_of_last_service_newgrf")?;
    append_field(header, 6, "orders")?; // REF_ORDERLIST → U32
    append_field(header, 2, "cur_real_order_index")?;
    append_field(header, 2, "current_order.type")?;
    append_field(header, 2, "current_order.flags")?;
    append_field(header, 4, "current_order.dest")?;
    append_field(header, 2, "current_order.refit_cargo")?;
    append_field(header, 4, "current_order.wait_time")?;
    append_field(header, 4, "current_order.travel_time")?;
    append_field(header, 4, "current_order.max_speed")?;
    append_field(header, 6, "next")?; // REF_VEHICLE → U32
    append_field(header, 4, "group_id")?; // Vehicle::group_id → U16
    append_field(header, 8, "timetable_start")?; // SLE_UINT64
    append_field(header, 5, "current_order_time")?; // SLE_INT32
    append_field(header, 5, "lateness_counter")?; // SLE_INT32
    append_field(header, 4, "vehicle_flags")?; // SLE_UINT16
    append_field(header, 4, "random_bits")?; // SLE_UINT16
    append_field(header, 2, "waiting_triggers")?; // SLE_UINT8
    append_field(header, 6, "next_shared")?; // REF_VEHICLE → U32
    append_field(header, 4, "last_station_visited")?; // SLE_UINT16
    append_field(header, 4, "last_loading_station")?; // SLE_UINT16
    append_field(header, 4, "service_interval")?; // SLE_UINT16
    append_field(header, 4, "reliability")?; // SLE_UINT16
    append_field(header, 4, "reliability_spd_dec")?; // SLE_UINT16
    append_field(header, 2, "breakdown_ctr")?; // SLE_UINT8
    append_field(header, 2, "breakdown_delay")?; // SLE_UINT8
    append_field(header, 2, "breakdowns_since_last_service")?; // SLE_UINT8
    append_field(header, 2, "breakdown_chance")?; // SLE_UINT8
    append_field(header, 5, "build_year")?; // SLE_INT32
    append_field(header, 4, "load_unload_ticks")?; // SLE_UINT16
    append_field(header, 4, "cargo_paid_for")?; // SLE_UINT16
    append_field(header, 7, "profit_this_year")?; // SLE_INT64
    append_field(header, 7, "profit_last_year")?; // SLE_INT64
    append_field(header, 7, "value")?; // SLE_INT64
    append_field(header, 8, "last_loading_tick")?; // SLE_UINT64
    append_field(header, 2, "day_counter")?; // SLE_UINT8
    append_field(header, 2, "tick_counter")?; // SLE_UINT8
    append_field(header, 2, "running_ticks")?; // SLE_UINT8
    append_field(header, 8, "depot_unbunching_last_departure")?; // SLE_UINT64
    append_field(header, 8, "depot_unbunching_next_departure")?; // SLE_UINT64
    append_field(header, 5, "round_trip_time")?; // SLE_INT32
    header.push(0);
    Ok(())
}

/// Construye chunk ORDL con records.
///
/// # Errors
///
/// Falla si algún valor gamma está fuera de rango.
pub(super) fn ordl_chunk(records: &[Vec<u8>]) -> Result<Vec<u8>, SavError> {
    let mut header = Vec::new();
    append_ordl_orders_header(&mut header)?;
    raw_table_chunk(*b"ORDL", &header, records, CH_TABLE)
}

/// Construye chunk VEHS con records.
///
/// # Errors
///
/// Falla si algún valor gamma está fuera de rango.
pub(super) fn vehs_chunk(records: &[Vec<u8>]) -> Result<Vec<u8>, SavError> {
    let mut header = Vec::new();
    append_vehs_header(&mut header)?;
    raw_table_chunk(*b"VEHS", &header, records, CH_SPARSE_TABLE)
}

/// Asegura agua de mar bajo `pos` (fixture ship).
#[cfg(test)]
#[allow(clippy::expect_used)]
pub(crate) fn ensure_sea_tile(state: &mut GameState, pos: TileCoord) {
    use crate::map::{WaterClass, make_water_tile};
    make_water_tile(&mut state.map, pos, WaterClass::Sea).expect("water");
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::station::{Station, StopKind};
    use crate::vehicle::{Vehicle, VehicleKind, VehicleOrder};

    #[test]
    fn encode_station_order_for_existing_station() {
        let mut state = GameState::new(64, 64);
        state.stations = vec![Station::new_with_kind(
            TileCoord::new(28, 39),
            StopKind::RailStation,
        )];
        let order = VehicleOrder::station(TileCoord::new(28, 39));
        let enc = encode_goto_order(&order, &state, 64).expect("encode");
        assert_eq!(enc.len(), 11);
        // OT_GOTO_STATION | (OrderStopLocation::Middle << 4) = 0x11.
        assert_eq!(enc[0], 0x11);
        assert_eq!(&enc[2..4], &0u16.to_be_bytes());
    }

    #[test]
    fn encode_station_order_prefers_imported_ottd_station_id() {
        let mut state = GameState::new(64, 64);
        let station_pos = TileCoord::new(28, 39);
        let mut station = Station::new_with_kind(station_pos, StopKind::RailStation);
        station.ottd_station_id = Some(42);
        state.stations = vec![station];

        let order = VehicleOrder::station(station_pos);
        let enc = encode_goto_order(&order, &state, 64).expect("encode");
        assert_eq!(enc.len(), 11);
        assert_eq!(&enc[2..4], &42u16.to_be_bytes());
    }

    #[test]
    fn ordl_records_non_empty_for_train_with_orders() {
        let mut state = GameState::new(64, 64);
        state.stations = vec![Station::new_with_kind(
            TileCoord::new(28, 39),
            StopKind::RailStation,
        )];
        let mut train = Vehicle::new(
            0,
            VehicleKind::Train,
            TileCoord::new(20, 40),
            TileCoord::new(20, 40),
        );
        train.set_vehicle_orders(vec![VehicleOrder::station(TileCoord::new(28, 39))]);
        state.vehicles = vec![train];
        let (ordl, vehs) = ordl_and_vehs_records(&state, 64).unwrap();
        assert_eq!(ordl.len(), 1);
        assert_eq!(vehs.len(), 1);
        assert!(ordl[0].len() > 1);
    }

    #[test]
    fn vehs_exports_road_bus_on_road_tile() {
        let mut state = GameState::new(64, 64);
        let bus_pos = TileCoord::new(13, 16);
        let mut road = state.map.get(bus_pos).unwrap();
        road.kind = TileKind::Road;
        road.mapt = 0x20;
        road.m5 = 0x0A; // ROAD_X
        state.map.set_tile(bus_pos, road).unwrap();

        let train = Vehicle::new(
            0,
            VehicleKind::Train,
            TileCoord::new(20, 40),
            TileCoord::new(20, 40),
        );
        let bus = Vehicle::new(1, VehicleKind::Bus, bus_pos, bus_pos);
        state.vehicles = vec![train, bus];
        let (_, vehs) = ordl_and_vehs_records(&state, 64).unwrap();
        assert_eq!(vehs.len(), 2, "tren + bus");
    }

    #[test]
    fn vehs_skips_road_vehicle_off_road() {
        let mut state = GameState::new(64, 64);
        let bus = Vehicle::new(
            1,
            VehicleKind::Bus,
            TileCoord::new(13, 16),
            TileCoord::new(13, 16),
        );
        state.vehicles = vec![bus];
        let (_, vehs) = ordl_and_vehs_records(&state, 64).unwrap();
        assert!(vehs.is_empty(), "bus sobre grass omitido");
    }

    #[test]
    fn vehs_exports_ship_on_water() {
        use crate::sav::chunks::{find_chunk, parse_chunks};
        use crate::sav::table::{SlValue, parse_table_chunk, record_get};

        let mut state = GameState::new(64, 64);
        let ship_pos = TileCoord::new(30, 30);
        ensure_sea_tile(&mut state, ship_pos);
        let mut ship = Vehicle::new(0, VehicleKind::Ship, ship_pos, ship_pos);
        ship.running = false;
        ship.direction = DIR_NE;
        ship.ship_state = 16; // TRACK_BIT_LEFT, conserva el byte raw del SAV.
        ship.ship_track = crate::ship_movement::TRACK_LEFT;
        ship.ship_rotation = 7;
        state.vehicles = vec![ship];

        let (_, vehs) = ordl_and_vehs_records(&state, 64).unwrap();
        assert_eq!(vehs.len(), 1);
        let chunk = vehs_chunk(&vehs).unwrap();
        let chunks = parse_chunks(&chunk).unwrap();
        let raw = find_chunk(&chunks, "VEHS").expect("VEHS");
        let rows = parse_table_chunk(&raw.body, true).expect("parse VEHS");
        assert_eq!(rows.len(), 1);
        assert_eq!(
            record_get(&rows[0].1, "type").and_then(SlValue::as_u64),
            Some(2)
        );
        let ship = match record_get(&rows[0].1, "ship") {
            Some(SlValue::Structs(items)) => items.first().expect("ship"),
            other => panic!("ship ausente: {other:?}"),
        };
        let common = match record_get(ship, "common") {
            Some(SlValue::Structs(items)) => items.first().expect("common"),
            other => panic!("common ausente: {other:?}"),
        };
        assert_eq!(
            record_get(common, "engine_type").and_then(SlValue::as_u64),
            Some(u64::from(DEFAULT_OPENTTD_SHIP_ENGINE))
        );
        assert_eq!(
            record_get(ship, "state").and_then(SlValue::as_u64),
            Some(16)
        );
        assert_eq!(
            record_get(ship, "rotation").and_then(SlValue::as_u64),
            Some(7)
        );
    }

    #[test]
    fn vehs_skips_ship_off_water() {
        let mut state = GameState::new(64, 64);
        let ship = Vehicle::new(
            0,
            VehicleKind::Ship,
            TileCoord::new(30, 30),
            TileCoord::new(30, 30),
        );
        state.vehicles = vec![ship];
        let (_, vehs) = ordl_and_vehs_records(&state, 64).unwrap();
        assert!(vehs.is_empty());
    }

    #[test]
    fn vehs_exports_aircraft_with_shadow_chain() {
        use crate::sav::chunks::{find_chunk, parse_chunks};
        use crate::sav::table::{SlValue, parse_table_chunk, record_get};

        let mut state = GameState::new(64, 64);
        let air_pos = TileCoord::new(40, 40);
        let mut air = Vehicle::new(0, VehicleKind::Aircraft, air_pos, air_pos);
        air.running = false;
        air.direction = DIR_NE;
        state.vehicles = vec![air];

        let (_, vehs) = ordl_and_vehs_records(&state, 64).unwrap();
        assert_eq!(vehs.len(), 2, "primario + sombra");
        let chunk = vehs_chunk(&vehs).unwrap();
        let chunks = parse_chunks(&chunk).unwrap();
        let raw = find_chunk(&chunks, "VEHS").expect("VEHS");
        let rows = parse_table_chunk(&raw.body, true).expect("parse VEHS");
        assert_eq!(rows.len(), 2);
        let primary = match record_get(&rows[0].1, "aircraft") {
            Some(SlValue::Structs(items)) => items.first().expect("aircraft"),
            other => panic!("aircraft ausente: {other:?}"),
        };
        let common = match record_get(primary, "common") {
            Some(SlValue::Structs(items)) => items.first().expect("common"),
            other => panic!("common ausente: {other:?}"),
        };
        assert_eq!(
            record_get(common, "subtype").and_then(SlValue::as_u64),
            Some(u64::from(AIR_AIRCRAFT))
        );
        assert_eq!(
            record_get(common, "next").and_then(SlValue::as_u64),
            Some(2),
            "REF sombra = sparse_idx 1 + 1"
        );
        let shadow = match record_get(&rows[1].1, "aircraft") {
            Some(SlValue::Structs(items)) => items.first().expect("shadow"),
            other => panic!("shadow ausente: {other:?}"),
        };
        let shadow_common = match record_get(shadow, "common") {
            Some(SlValue::Structs(items)) => items.first().expect("common"),
            other => panic!("shadow common ausente: {other:?}"),
        };
        assert_eq!(
            record_get(shadow_common, "subtype").and_then(SlValue::as_u64),
            Some(u64::from(AIR_SHADOW))
        );
    }

    #[test]
    #[allow(clippy::items_after_statements, clippy::too_many_lines)]
    fn vehs_exports_helicopter_rotor_and_fta_state() {
        use crate::airport_fta::AirportHeading;
        use crate::sav::chunks::{find_chunk, parse_chunks};
        use crate::sav::table::{SlValue, parse_table_chunk, record_get};

        let mut state = GameState::new(64, 64);
        let air_pos = TileCoord::new(40, 40);
        let airport_pos = TileCoord::new(42, 40);
        let mut airport = Station::new_with_kind(airport_pos, StopKind::Airport);
        airport.ottd_station_id = Some(42);
        state.stations = vec![airport];

        let mut helicopter = Vehicle::new(7, VehicleKind::Aircraft, air_pos, airport_pos);
        helicopter.engine_id = Some(crate::engine::ENGINE_AIRCRAFT_TRICARIO);
        helicopter.crashed_ctr = 19;
        helicopter.aircraft_number_consecutive_turns = 6;
        helicopter.aircraft_turn_counter = 11;
        helicopter.aircraft_flags = 0xA5;
        helicopter.airport_pos = 8;
        helicopter.airport_prev_pos = 7;
        helicopter.airport_heading = AirportHeading::HeliLanding;
        helicopter.airport_fta_station = Some(airport_pos);
        helicopter.direction = DIR_SW;
        state.vehicles = vec![helicopter];

        let (_, vehs) = ordl_and_vehs_records(&state, 64).unwrap();
        assert_eq!(vehs.len(), 3, "primario + sombra + rotor");
        let chunk = vehs_chunk(&vehs).unwrap();
        let chunks = parse_chunks(&chunk).unwrap();
        let raw = find_chunk(&chunks, "VEHS").expect("VEHS");
        let rows = parse_table_chunk(&raw.body, true).expect("parse VEHS");
        assert_eq!(rows.len(), 3);
        let parsed = crate::sav::entities::vehicles_from_chunks(
            &chunks,
            64,
            &crate::sav::orders::SavOrderImport::from_chunks(&chunks, 360),
            360,
        );
        let parsed_primary = parsed.first().expect("primario parseado");
        assert_eq!(parsed_primary.aircraft_crashed_counter, 19);
        assert_eq!(parsed_primary.aircraft_number_consecutive_turns, 6);
        assert_eq!(parsed_primary.aircraft_turn_counter, 11);
        assert_eq!(parsed_primary.aircraft_flags, 0xA5);

        fn aircraft(row: &crate::sav::table::SlRecord) -> &crate::sav::table::SlRecord {
            match record_get(row, "aircraft") {
                Some(SlValue::Structs(items)) => items.first().expect("aircraft"),
                other => panic!("aircraft ausente: {other:?}"),
            }
        }
        fn common(aircraft: &crate::sav::table::SlRecord) -> &crate::sav::table::SlRecord {
            match record_get(aircraft, "common") {
                Some(SlValue::Structs(items)) => items.first().expect("common"),
                other => panic!("common ausente: {other:?}"),
            }
        }

        let primary = aircraft(&rows[0].1);
        let primary_common = common(primary);
        assert_eq!(
            record_get(primary_common, "subtype").and_then(SlValue::as_u64),
            Some(u64::from(AIR_HELICOPTER))
        );
        assert_eq!(
            record_get(primary_common, "next").and_then(SlValue::as_u64),
            Some(2),
            "REF sombra = sparse_idx 1 + 1"
        );
        assert_eq!(
            record_get(primary, "crashed_counter").and_then(SlValue::as_u64),
            Some(19)
        );
        assert_eq!(
            record_get(primary, "number_consecutive_turns").and_then(SlValue::as_u64),
            Some(6)
        );
        assert_eq!(
            record_get(primary, "turn_counter").and_then(SlValue::as_u64),
            Some(11)
        );
        assert_eq!(
            record_get(primary, "flags").and_then(SlValue::as_u64),
            Some(0xA5)
        );
        assert_eq!(
            record_get(primary, "pos").and_then(SlValue::as_u64),
            Some(8)
        );
        assert_eq!(
            record_get(primary, "targetairport").and_then(SlValue::as_u64),
            Some(42)
        );
        assert_eq!(
            record_get(primary, "state").and_then(SlValue::as_u64),
            Some(u64::from(AirportHeading::HeliLanding.as_u8()))
        );
        assert_eq!(
            record_get(primary, "previous_pos").and_then(SlValue::as_u64),
            Some(7)
        );

        let shadow_common = common(aircraft(&rows[1].1));
        assert_eq!(
            record_get(shadow_common, "subtype").and_then(SlValue::as_u64),
            Some(u64::from(AIR_SHADOW))
        );
        assert_eq!(
            record_get(shadow_common, "next").and_then(SlValue::as_u64),
            Some(3),
            "la sombra de un helicóptero debe apuntar al rotor"
        );

        let rotor_common = common(aircraft(&rows[2].1));
        assert_eq!(
            record_get(rotor_common, "subtype").and_then(SlValue::as_u64),
            Some(u64::from(AIR_ROTOR))
        );
        assert_eq!(
            record_get(rotor_common, "next").and_then(SlValue::as_u64),
            Some(0)
        );
    }

    #[test]
    #[allow(clippy::items_after_statements, clippy::too_many_lines)]
    fn vehs_preserves_train_consist_next_refs_and_wagon_subtypes() {
        use crate::sav::chunks::{find_chunk, parse_chunks};
        use crate::sav::table::{SlRecord, SlValue, parse_table_chunk, record_get};

        let mut state = GameState::new(64, 64);
        let head_pos = TileCoord::new(20, 40);
        let middle_pos = TileCoord::new(21, 40);
        let rear_pos = TileCoord::new(22, 40);

        // Deliberadamente dejamos la unidad trasera antes de la intermedia en
        // `state.vehicles`: las referencias deben seguir `next_unit`, no el
        // orden incidental del vector JSON.
        let mut head = Vehicle::new(10, VehicleKind::Train, head_pos, head_pos);
        head.next_unit = Some(30);
        head.train_crash_anim_pos = 321;
        head.force_proceed = true;
        head.train_track = 5;
        head.train_flags = 0x1234;
        head.train_gv_flags = 0x4567;
        head.wait_counter = 513;
        let mut rear = Vehicle::new(20, VehicleKind::Train, rear_pos, rear_pos);
        rear.prev_unit = Some(30);
        let mut middle = Vehicle::new(30, VehicleKind::Train, middle_pos, middle_pos);
        middle.prev_unit = Some(10);
        middle.next_unit = Some(20);
        state.vehicles = vec![head, rear, middle];

        let (_, vehs) = ordl_and_vehs_records(&state, 64).unwrap();
        let chunk = vehs_chunk(&vehs).unwrap();
        let chunks = parse_chunks(&chunk).unwrap();
        let raw = find_chunk(&chunks, "VEHS").expect("VEHS");
        let rows = parse_table_chunk(&raw.body, true).expect("parse VEHS");
        assert_eq!(rows.len(), 3);
        let parsed = crate::sav::entities::vehicles_from_chunks(
            &chunks,
            64,
            &crate::sav::orders::SavOrderImport::from_chunks(&chunks, 360),
            360,
        );
        assert_eq!(parsed[0].train_crash_anim_pos, 321);
        assert_eq!(parsed[0].train_force_proceed, 1);
        assert_eq!(parsed[0].train_track, 5);
        assert_eq!(parsed[0].train_flags, 0x1234);
        assert_eq!(parsed[0].train_wait_counter, 513);
        assert_eq!(parsed[0].train_gv_flags, 0x4567);

        fn train_common(row: &SlRecord) -> &SlRecord {
            let train = match record_get(row, "train") {
                Some(SlValue::Structs(items)) => items.first().expect("train"),
                other => panic!("train ausente: {other:?}"),
            };
            match record_get(train, "common") {
                Some(SlValue::Structs(items)) => items.first().expect("common"),
                other => panic!("common ausente: {other:?}"),
            }
        }

        // Filas: cabeza (sparse 0), trasera (sparse 1), intermedia (sparse 2).
        let head_common = train_common(&rows[0].1);
        assert_eq!(
            record_get(head_common, "next").and_then(SlValue::as_u64),
            Some(3)
        );
        assert_eq!(
            record_get(head_common, "subtype").and_then(SlValue::as_u64),
            Some(u64::from(TRAIN_SUBTYPE_FRONT_ENGINE))
        );
        let head_train = match record_get(&rows[0].1, "train") {
            Some(SlValue::Structs(items)) => items.first().expect("train"),
            other => panic!("train ausente: {other:?}"),
        };
        assert_eq!(
            record_get(head_train, "crash_anim_pos").and_then(SlValue::as_u64),
            Some(321)
        );
        assert_eq!(
            record_get(head_train, "force_proceed").and_then(SlValue::as_u64),
            Some(1)
        );
        assert_eq!(
            record_get(head_train, "track").and_then(SlValue::as_u64),
            Some(5)
        );
        assert_eq!(
            record_get(head_train, "flags").and_then(SlValue::as_u64),
            Some(0x1234)
        );
        assert_eq!(
            record_get(head_train, "wait_counter").and_then(SlValue::as_u64),
            Some(513)
        );
        assert_eq!(
            record_get(head_train, "gv_flags").and_then(SlValue::as_u64),
            Some(0x4567)
        );

        let rear_common = train_common(&rows[1].1);
        assert_eq!(
            record_get(rear_common, "next").and_then(SlValue::as_u64),
            Some(0)
        );
        assert_eq!(
            record_get(rear_common, "subtype").and_then(SlValue::as_u64),
            Some(u64::from(TRAIN_SUBTYPE_WAGON))
        );

        let middle_common = train_common(&rows[2].1);
        assert_eq!(
            record_get(middle_common, "next").and_then(SlValue::as_u64),
            Some(2)
        );
        assert_eq!(
            record_get(middle_common, "subtype").and_then(SlValue::as_u64),
            Some(u64::from(TRAIN_SUBTYPE_WAGON))
        );
    }

    #[test]
    fn vehs_reuses_ordl_ref_for_shared_order_list() {
        use crate::sav::chunks::{find_chunk, parse_chunks};
        use crate::sav::table::{SlValue, parse_table_chunk, record_get};
        use crate::shared_orders::SharedOrderList;

        let mut state = GameState::new(64, 64);
        let station_pos = TileCoord::new(28, 39);
        state.stations = vec![Station::new_with_kind(station_pos, StopKind::RailStation)];
        let shared_orders = vec![VehicleOrder::station(station_pos)];
        state.shared_order_lists = vec![SharedOrderList {
            id: 77,
            orders: shared_orders.clone(),
        }];

        let mut first = Vehicle::new(
            1,
            VehicleKind::Train,
            TileCoord::new(20, 40),
            TileCoord::new(20, 40),
        );
        first.shared_order_id = Some(77);
        first.set_vehicle_orders(shared_orders.clone());
        let mut second = Vehicle::new(
            2,
            VehicleKind::Train,
            TileCoord::new(21, 40),
            TileCoord::new(21, 40),
        );
        second.shared_order_id = Some(77);
        second.set_vehicle_orders(shared_orders);
        state.vehicles = vec![first, second];

        let (ordl, vehs) = ordl_and_vehs_records(&state, 64).unwrap();
        assert_eq!(ordl.len(), 1, "una sola lista ORDL compartida");
        let chunk = vehs_chunk(&vehs).unwrap();
        let chunks = parse_chunks(&chunk).unwrap();
        let raw = find_chunk(&chunks, "VEHS").expect("VEHS");
        let rows = parse_table_chunk(&raw.body, true).expect("parse VEHS");
        assert_eq!(rows.len(), 2);
        for (_, row) in rows {
            let train = match record_get(&row, "train") {
                Some(SlValue::Structs(items)) => items.first().expect("train"),
                other => panic!("train ausente: {other:?}"),
            };
            let common = match record_get(train, "common") {
                Some(SlValue::Structs(items)) => items.first().expect("common"),
                other => panic!("common ausente: {other:?}"),
            };
            assert_eq!(
                record_get(common, "orders").and_then(SlValue::as_u64),
                Some(1),
                "ambos vehículos deben referenciar ORDL[0]"
            );
        }
    }

    #[test]
    fn vehs_record_includes_direction_and_engine() {
        use crate::sav::chunks::{find_chunk, parse_chunks};
        use crate::sav::table::{SlValue, parse_table_chunk, record_get};

        let mut state = GameState::new(64, 64);
        let mut tile = state.map.get(TileCoord::new(20, 40)).unwrap();
        tile.kind = TileKind::Rail;
        tile.mapt = 0x10;
        tile.m5 = TRACK_BIT_X;
        state.map.set_tile(TileCoord::new(20, 40), tile).unwrap();

        let mut train = Vehicle::new(
            0,
            VehicleKind::Train,
            TileCoord::new(20, 40),
            TileCoord::new(20, 40),
        );
        train.direction = DIR_NE;
        train.running = true;
        train.owner = crate::CompanyId(3);
        train.name = Some("Expreso Norte".to_owned());
        // ID fuera del catálogo vanilla: debe sobrevivir al round-trip.
        train.native_engine_type = Some(511);
        state.vehicles = vec![train];

        let (_, vehs) = ordl_and_vehs_records(&state, 64).unwrap();
        let chunk = vehs_chunk(&vehs).unwrap();
        let chunks = parse_chunks(&chunk).unwrap();
        let raw = find_chunk(&chunks, "VEHS").expect("VEHS");
        let rows = parse_table_chunk(&raw.body, true).expect("parse VEHS");
        assert_eq!(rows.len(), 1);
        let train = match record_get(&rows[0].1, "train") {
            Some(SlValue::Structs(items)) => items.first().expect("train struct"),
            other => panic!("train ausente: {other:?}"),
        };
        let common = match record_get(train, "common") {
            Some(SlValue::Structs(items)) => items.first().expect("common"),
            other => panic!("common ausente: {other:?}"),
        };
        assert_eq!(
            record_get(common, "direction").and_then(SlValue::as_u64),
            Some(u64::from(DIR_NE))
        );
        assert_eq!(
            record_get(common, "engine_type").and_then(SlValue::as_u64),
            Some(511)
        );
        assert_eq!(
            record_get(common, "owner").and_then(SlValue::as_u64),
            Some(3)
        );
        assert_eq!(
            record_get(common, "name").and_then(SlValue::as_str),
            Some("Expreso Norte")
        );
        assert_eq!(
            record_get(train, "track").and_then(SlValue::as_u64),
            Some(u64::from(crate::ship_movement::TRACK_X))
        );
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn vehs_record_preserves_timetable_runtime_fields() {
        use crate::sav::chunks::{find_chunk, parse_chunks};
        use crate::sav::table::{SlValue, parse_table_chunk, record_get};

        let mut state = GameState::new(64, 64);
        let station_pos = TileCoord::new(28, 39);
        state.stations = vec![Station::new_with_kind(station_pos, StopKind::RailStation)];
        let order = VehicleOrder::station(station_pos)
            .with_wait_ticks(12)
            .expect("station supports timetable wait")
            .with_travel_ticks(34)
            .with_max_speed(90);
        let mut train = Vehicle::new(
            0,
            VehicleKind::Train,
            TileCoord::new(20, 40),
            TileCoord::new(20, 40),
        );
        train.set_vehicle_orders(vec![order]);
        train.timetable_start = 1_234;
        train.motion_counter = 0x1234_5678;
        train.current_order_time = 55;
        train.timetable_lateness = -7;
        train.economy_age_days = 654;
        train.last_service_newgrf_day = 321;
        train.depot_unbunching_last_departure = 77_000;
        train.depot_unbunching_next_departure = 88_000;
        train.round_trip_time = 9_876;
        train.timetable_started = true;
        train.timetable_autofill = true;
        train.vehicle_flags = 1 << 7;
        train.newgrf_random_bits = 0xCAFE;
        train.newgrf_waiting_random_triggers = 0x12;
        train.last_station_visited = Some(station_pos);
        train.last_pickup_station = Some(station_pos);
        train.last_depart_tick = Some(9_876);
        train.current_order_state = Some(VehicleOrderRuntime {
            order_type: 3,
            flags: 0x91,
            dest: 0x1234,
            refit_cargo: 0xFF,
            wait_time: 11,
            travel_time: 22,
            max_speed: 333,
        });
        train.newgrf_day_counter = 7;
        train.newgrf_tick_counter = 8;
        train.running_ticks = 9;
        train.service_interval_days = 87;
        train.cargo_subtype = 3;
        train.cargo_age_counter = 42;
        train.max_age_days = 12_345;
        train.last_service_day = 123;
        train.reliability = 7_654;
        train.reliability_spd_dec = 321;
        train.breakdown_ctr = 4;
        train.breakdown_delay = 5;
        train.breakdowns_since_last_service = 6;
        train.breakdown_chance = 7;
        train.profit_this_year = 123_456;
        train.profit_last_year = -654_321;
        train.build_year = 1987;
        train.load_unload_ticks = 23;
        train.cargo_paid_for = 17;
        train.value = 765_432;
        state.vehicles = vec![train];

        let (_, vehs) = ordl_and_vehs_records(&state, 64).unwrap();
        let chunk = vehs_chunk(&vehs).unwrap();
        let chunks = parse_chunks(&chunk).unwrap();
        let raw = find_chunk(&chunks, "VEHS").expect("VEHS");
        let rows = parse_table_chunk(&raw.body, true).expect("parse VEHS");
        let train = match record_get(&rows[0].1, "train") {
            Some(SlValue::Structs(items)) => items.first().expect("train struct"),
            other => panic!("train ausente: {other:?}"),
        };
        let common = match record_get(train, "common") {
            Some(SlValue::Structs(items)) => items.first().expect("common struct"),
            other => panic!("common ausente: {other:?}"),
        };
        assert_eq!(
            record_get(common, "timetable_start").and_then(SlValue::as_u64),
            Some(1_234)
        );
        assert_eq!(
            record_get(common, "motion_counter").and_then(SlValue::as_u64),
            Some(0x1234_5678)
        );
        assert_eq!(
            record_get(common, "current_order_time").and_then(SlValue::as_i64),
            Some(55)
        );
        assert_eq!(
            record_get(common, "lateness_counter").and_then(SlValue::as_i64),
            Some(-7)
        );
        assert_eq!(
            record_get(common, "economy_age").and_then(SlValue::as_i64),
            Some(654)
        );
        assert_eq!(
            record_get(common, "date_of_last_service_newgrf").and_then(SlValue::as_i64),
            Some(packed_calendar_date_from_day_index(321).into())
        );
        assert_eq!(
            record_get(common, "depot_unbunching_last_departure").and_then(SlValue::as_u64),
            Some(77_000)
        );
        assert_eq!(
            record_get(common, "depot_unbunching_next_departure").and_then(SlValue::as_u64),
            Some(88_000)
        );
        assert_eq!(
            record_get(common, "round_trip_time").and_then(SlValue::as_i64),
            Some(9_876)
        );
        assert_eq!(
            record_get(common, "vehicle_flags").and_then(SlValue::as_u64),
            Some(u64::from((1u16 << 7) | 0b1_1000))
        );
        assert_eq!(
            record_get(common, "random_bits").and_then(SlValue::as_u64),
            Some(0xCAFE)
        );
        assert_eq!(
            record_get(common, "waiting_triggers").and_then(SlValue::as_u64),
            Some(0x12)
        );
        assert_eq!(
            record_get(common, "last_station_visited").and_then(SlValue::as_u64),
            Some(0),
            "la estación sintética usa el índice denso 0"
        );
        assert_eq!(
            record_get(common, "last_loading_station").and_then(SlValue::as_u64),
            Some(0),
            "la estación sintética usa el índice denso 0"
        );
        assert_eq!(
            record_get(common, "last_loading_tick").and_then(SlValue::as_u64),
            Some(9_876)
        );
        assert_eq!(
            record_get(common, "current_order.type").and_then(SlValue::as_u64),
            Some(3)
        );
        assert_eq!(
            record_get(common, "current_order.flags").and_then(SlValue::as_u64),
            Some(0x91)
        );
        assert_eq!(
            record_get(common, "current_order.dest").and_then(SlValue::as_u64),
            Some(0x1234)
        );
        assert_eq!(
            record_get(common, "current_order.wait_time").and_then(SlValue::as_u64),
            Some(11)
        );
        assert_eq!(
            record_get(common, "current_order.travel_time").and_then(SlValue::as_u64),
            Some(22)
        );
        assert_eq!(
            record_get(common, "current_order.max_speed").and_then(SlValue::as_u64),
            Some(333)
        );
        assert_eq!(
            record_get(common, "service_interval").and_then(SlValue::as_u64),
            Some(87)
        );
        assert_eq!(
            record_get(common, "cargo_subtype").and_then(SlValue::as_u64),
            Some(3)
        );
        assert_eq!(
            record_get(common, "cargo_age_counter").and_then(SlValue::as_u64),
            Some(42)
        );
        assert_eq!(
            record_get(common, "max_age").and_then(SlValue::as_i64),
            Some(12_345)
        );
        assert_eq!(
            record_get(common, "reliability").and_then(SlValue::as_u64),
            Some(7_654)
        );
        assert_eq!(
            record_get(common, "reliability_spd_dec").and_then(SlValue::as_u64),
            Some(321)
        );
        assert_eq!(
            record_get(common, "breakdowns_since_last_service").and_then(SlValue::as_u64),
            Some(6)
        );
        assert_eq!(
            record_get(common, "profit_this_year").and_then(SlValue::as_i64),
            Some(123_456_i64 * 256)
        );
        assert_eq!(
            record_get(common, "profit_last_year").and_then(SlValue::as_i64),
            Some(-654_321_i64 * 256)
        );
        assert_eq!(
            record_get(common, "build_year").and_then(SlValue::as_i64),
            Some(1_987)
        );
        assert_eq!(
            record_get(common, "load_unload_ticks").and_then(SlValue::as_u64),
            Some(23)
        );
        assert_eq!(
            record_get(common, "cargo_paid_for").and_then(SlValue::as_u64),
            Some(17)
        );
        assert_eq!(
            record_get(common, "value").and_then(SlValue::as_i64),
            Some(765_432_i64 * 256)
        );
        assert_eq!(
            record_get(common, "day_counter").and_then(SlValue::as_u64),
            Some(7)
        );
        assert_eq!(
            record_get(common, "tick_counter").and_then(SlValue::as_u64),
            Some(8)
        );
        assert_eq!(
            record_get(common, "running_ticks").and_then(SlValue::as_u64),
            Some(9)
        );
    }

    #[test]
    fn exported_ordl_chunk_imports_orders() {
        use crate::sav::chunks::{find_chunk, parse_chunks};
        use crate::sav::orders::SavOrderImport;
        use crate::sav::table::{SlValue, parse_table_chunk, record_get};

        let mut state = GameState::new(64, 64);
        state.stations = vec![Station::new_with_kind(
            TileCoord::new(28, 39),
            StopKind::RailStation,
        )];
        let mut train = Vehicle::new(
            0,
            VehicleKind::Train,
            TileCoord::new(20, 40),
            TileCoord::new(20, 40),
        );
        train.set_vehicle_orders(vec![VehicleOrder::station(TileCoord::new(28, 39))]);
        state.vehicles = vec![train];
        let (ordl, _) = ordl_and_vehs_records(&state, 64).unwrap();
        let chunk_bytes = ordl_chunk(&ordl).unwrap();
        let chunks = parse_chunks(&chunk_bytes).unwrap();
        let raw = find_chunk(&chunks, "ORDL").expect("ORDL");
        let rows = parse_table_chunk(&raw.body, false).expect("parse ORDL table");
        assert_eq!(rows.len(), 1, "una lista ORDL");
        let orders = record_get(&rows[0].1, "orders").expect("campo orders");
        let SlValue::Structs(items) = orders else {
            panic!("orders no es Structs: {orders:?}");
        };
        assert_eq!(items.len(), 1);
        let import = SavOrderImport::from_chunks(&chunks, 350);
        assert_eq!(
            import.orders_for_vehicle_ref(1).len(),
            1,
            "lista ORDL vía ref 1"
        );
    }
}
