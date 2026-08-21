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
//! Residual: tram, creación runtime de pagos `CAPY`, rotor heli (solo ala fija en export).

use super::super::SavError;
use super::super::chunks::{CH_SPARSE_TABLE, CH_TABLE};
use super::super::orders_codec::{append_ordl_orders_header, encode_vehicle_order};
use super::chunks::raw_table_chunk;
use super::codec::{write_gamma, write_str};
use crate::game_state::GameState;
use crate::map::{TileCoord, TileKind, coord_to_linear_index};
use crate::vehicle::{DIR_NE, DIR_NW, DIR_SE, DIR_SW, Vehicle, VehicleKind, VehicleOrder};
use std::collections::HashMap;

/// Cabeza de convoy + motor (`GVSF_FRONT | GVSF_ENGINE`).
const TRAIN_SUBTYPE_FRONT_ENGINE: u8 = 0x01 | 0x08;

/// `GroundVehicleSubtypeFlags::GVSF_WAGON` (unidad remolcada del consist).
const TRAIN_SUBTYPE_WAGON: u8 = 1 << 2;

/// `AirVehicleSubType::AIR_AIRCRAFT` (ala fija; no requiere rotor).
const AIR_AIRCRAFT: u8 = 2;
/// `AirVehicleSubType::AIR_SHADOW`.
const AIR_SHADOW: u8 = 4;

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

fn station_id_for_pos(state: &GameState, pos: TileCoord) -> Option<u16> {
    state
        .stations
        .iter()
        .position(|s| s.pos == pos)
        .and_then(|i| u16::try_from(i).ok())
}

fn cargo_ottd_byte(v: &Vehicle) -> u8 {
    if let Some(c) = v.cargo_type {
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
    match v.engine_id {
        Some(0) => 204, // MPS Oil Tanker
        Some(2) => 206, // MPS Passenger Ferry
        Some(7) => 211, // Yate Cargo ship
        _ => DEFAULT_OPENTTD_SHIP_ENGINE,
    }
}

fn openttd_aircraft_engine_type(v: &Vehicle) -> u16 {
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
    owner: u8,
    tile: u32,
    x_pos: i32,
    y_pos: i32,
    z_pos: i32,
    direction: u8,
    engine_type: u16,
    vehstatus: u8,
    cargo: u8,
    order_list_ref: u32,
    cur_order: u8,
    /// `REF_VEHICLE`: 0 = null, resto = índice sparse + 1.
    next_ref: u32,
    /// Grupo de flota (`DEFAULT_GROUP` si no está asignado).
    group_id: u16,
    /// Inicio del horario en ticks (`Vehicle::timetable_start`).
    timetable_start: u64,
    /// Tiempo transcurrido en la orden actual.
    current_order_time: u32,
    /// Retraso acumulado del horario (`lateness_counter`).
    timetable_lateness: i32,
    /// `Vehicle::vehicle_flags` (`OpenTTD` `VehicleFlags`).
    vehicle_flags: u16,
    /// Intervalo de servicio (`Vehicle::service_interval`).
    service_interval: u16,
}

fn write_vehs_common(buf: &mut Vec<u8>, c: &CommonWire) {
    buf.push(c.subtype);
    buf.push(c.owner);
    buf.extend_from_slice(&c.tile.to_be_bytes());
    buf.extend_from_slice(&u32::try_from(c.x_pos).unwrap_or(0).to_be_bytes());
    buf.extend_from_slice(&u32::try_from(c.y_pos).unwrap_or(0).to_be_bytes());
    buf.extend_from_slice(&c.z_pos.to_be_bytes());
    buf.push(c.direction);
    buf.extend_from_slice(&c.engine_type.to_be_bytes());
    buf.push(c.vehstatus);
    buf.push(c.cargo);
    buf.extend_from_slice(&c.order_list_ref.to_be_bytes());
    buf.push(c.cur_order);
    buf.extend_from_slice(&c.next_ref.to_be_bytes());
    buf.extend_from_slice(&c.group_id.to_be_bytes());
    buf.extend_from_slice(&c.timetable_start.to_be_bytes());
    buf.extend_from_slice(&c.current_order_time.to_be_bytes());
    buf.extend_from_slice(&c.timetable_lateness.to_be_bytes());
    buf.extend_from_slice(&c.vehicle_flags.to_be_bytes());
    buf.extend_from_slice(&c.service_interval.to_be_bytes());
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

fn common_wire_for(
    v: &Vehicle,
    tile_idx: u32,
    direction: u8,
    engine_type: u16,
    order_list_ref: u32,
    subtype: u8,
    next_ref: u32,
) -> CommonWire {
    let cargo = cargo_ottd_byte(v);
    let cur_order = u8::try_from(v.current_order.min(255)).unwrap_or(0);
    let vehstatus = if v.running { 0 } else { VEHSTATUS_STOPPED };
    let x_pos = v.pos.x * TILE_SIZE + i32::from(v.rail_pixel.min(15));
    let y_pos = v.pos.y * TILE_SIZE + TILE_SIZE / 2;
    let z_pos = i32::from(v.z_pos.unwrap_or(0));
    let group_id = v.group_id.unwrap_or(0xFFFE).min(u32::from(u16::MAX)) as u16;
    CommonWire {
        subtype,
        owner: v.owner.0,
        tile: tile_idx,
        x_pos,
        y_pos,
        z_pos,
        direction,
        engine_type,
        vehstatus,
        cargo,
        order_list_ref,
        cur_order,
        next_ref,
        group_id,
        timetable_start: u64::from(v.timetable_start),
        current_order_time: v.current_order_time,
        timetable_lateness: v.timetable_lateness,
        vehicle_flags: vehicle_flags_for(v),
        service_interval: v.service_interval_days,
    }
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
    train_track: Option<u8>,
) -> Result<(), SavError> {
    rec.push(veh_type);
    for t in VEH_TRAIN..=VEH_AIRCRAFT {
        if t == veh_type {
            write_gamma(1, rec)?; // struct presente
            write_gamma(1, rec)?; // common presente
            write_vehs_common(rec, common);
            if let Some(track) = train_track {
                rec.push(track);
            }
        } else {
            write_gamma(0, rec)?;
        }
    }
    Ok(())
}

/// ORDL + VEHS: tren, ROAD, ship y aircraft (ala fija + sombra).
///
/// # Errors
///
/// Falla si algún valor gamma está fuera de rango.
#[allow(clippy::too_many_lines)]
pub(super) fn ordl_and_vehs_records(
    state: &GameState,
    map_w: u32,
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
            2
        } else {
            1
        });
    }

    // GRUPOS/ORDL aún no tienen un chunk GRPS en este escritor, pero cuando
    // varios vehículos apuntan a la misma lista compartida sí debemos emitir
    // una sola ORDL y conservar su identidad en el campo `orders` de VEHS.
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
            // Primario + sombra (OpenTTD `Missing shadow for aircraft`).
            let shadow_idx = sparse_idx + 1;
            let next_ref = shadow_idx + 1; // REF = index+1
            let engine_type = openttd_aircraft_engine_type(v);

            let mut primary = Vec::new();
            write_gamma(sparse_idx, &mut primary)?;
            push_typed_vehicle(
                &mut primary,
                VEH_AIRCRAFT,
                &common_wire_for(
                    v,
                    tile_idx,
                    direction,
                    engine_type,
                    order_list_ref,
                    AIR_AIRCRAFT,
                    next_ref,
                ),
                None,
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
                    owner: v.owner.0,
                    tile: tile_idx,
                    x_pos: v.pos.x * TILE_SIZE + TILE_SIZE / 2,
                    y_pos: v.pos.y * TILE_SIZE + TILE_SIZE / 2,
                    z_pos: i32::from(v.z_pos.unwrap_or(0)),
                    direction,
                    engine_type,
                    vehstatus: VEHSTATUS_STOPPED,
                    cargo: 0,
                    order_list_ref: 0,
                    cur_order: 0,
                    next_ref: 0,
                    group_id: 0xFFFE,
                    timetable_start: 0,
                    current_order_time: 0,
                    timetable_lateness: 0,
                    vehicle_flags: 0,
                    service_interval: 0,
                },
                None,
            )?;
            vehs.push(shadow);
            sparse_idx += 1;
            continue;
        }

        let mut rec = Vec::new();
        write_gamma(sparse_idx, &mut rec)?;

        if is_train {
            let track = track_bits_for(state, v.pos);
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
            push_typed_vehicle(
                &mut rec,
                VEH_TRAIN,
                &common_wire_for(
                    v,
                    tile_idx,
                    direction,
                    engine_type,
                    order_list_ref,
                    subtype,
                    next_ref,
                ),
                Some(track),
            )?;
        } else if is_road {
            let engine_type = openttd_road_engine_type(v);
            push_typed_vehicle(
                &mut rec,
                VEH_ROAD,
                &common_wire_for(
                    v,
                    tile_idx,
                    direction,
                    engine_type,
                    order_list_ref,
                    TRAIN_SUBTYPE_FRONT_ENGINE,
                    0,
                ),
                None,
            )?;
        } else {
            // ship
            let engine_type = openttd_ship_engine_type(v);
            push_typed_vehicle(
                &mut rec,
                VEH_SHIP,
                &common_wire_for(
                    v,
                    tile_idx,
                    direction,
                    engine_type,
                    order_list_ref,
                    TRAIN_SUBTYPE_FRONT_ENGINE,
                    0,
                ),
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

    // train → common + track
    append_field(header, 0x1B, "common")?;
    append_field(header, 2, "track")?;
    header.push(0);
    append_vehs_common_fields(header)?;

    // roadveh → common
    append_field(header, 0x1B, "common")?;
    header.push(0);
    append_vehs_common_fields(header)?;

    // ship → common (state/path/rotation usan defaults)
    append_field(header, 0x1B, "common")?;
    header.push(0);
    append_vehs_common_fields(header)?;

    // aircraft → common (pos/state/targetairport usan defaults)
    append_field(header, 0x1B, "common")?;
    header.push(0);
    append_vehs_common_fields(header)?;
    Ok(())
}

fn append_vehs_common_fields(header: &mut Vec<u8>) -> Result<(), SavError> {
    append_field(header, 2, "subtype")?;
    append_field(header, 2, "owner")?;
    append_field(header, 6, "tile")?;
    append_field(header, 6, "x_pos")?;
    append_field(header, 6, "y_pos")?;
    append_field(header, 5, "z_pos")?; // SLE_FILE_I32
    append_field(header, 2, "direction")?;
    append_field(header, 4, "engine_type")?;
    append_field(header, 2, "vehstatus")?;
    append_field(header, 2, "cargo_type")?;
    append_field(header, 6, "orders")?; // REF_ORDERLIST → U32
    append_field(header, 2, "cur_real_order_index")?;
    append_field(header, 6, "next")?; // REF_VEHICLE → U32
    append_field(header, 4, "group_id")?; // Vehicle::group_id → U16
    append_field(header, 8, "timetable_start")?; // SLE_UINT64
    append_field(header, 5, "current_order_time")?; // SLE_INT32
    append_field(header, 5, "lateness_counter")?; // SLE_INT32
    append_field(header, 4, "vehicle_flags")?; // SLE_UINT16
    append_field(header, 4, "service_interval")?; // SLE_UINT16
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
    #[allow(clippy::items_after_statements)]
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
            Some(0)
        );
        assert_eq!(
            record_get(common, "owner").and_then(SlValue::as_u64),
            Some(0)
        );
        assert_eq!(
            record_get(train, "track").and_then(SlValue::as_u64),
            Some(u64::from(TRACK_BIT_X))
        );
    }

    #[test]
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
        train.current_order_time = 55;
        train.timetable_lateness = -7;
        train.timetable_started = true;
        train.timetable_autofill = true;
        train.vehicle_flags = 1 << 7;
        train.service_interval_days = 87;
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
            record_get(common, "current_order_time").and_then(SlValue::as_i64),
            Some(55)
        );
        assert_eq!(
            record_get(common, "lateness_counter").and_then(SlValue::as_i64),
            Some(-7)
        );
        assert_eq!(
            record_get(common, "vehicle_flags").and_then(SlValue::as_u64),
            Some(u64::from((1u16 << 7) | 0b1_1000))
        );
        assert_eq!(
            record_get(common, "service_interval").and_then(SlValue::as_u64),
            Some(87)
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
