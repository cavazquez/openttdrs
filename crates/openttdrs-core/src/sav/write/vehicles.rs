//! Serialización de vehículos y órdenes (ORDL, VEHS).

use super::super::chunks::{CH_SPARSE_TABLE, CH_TABLE};
use super::super::SavError;
use super::chunks::raw_table_chunk;
use super::codec::{write_gamma, write_str};
use crate::CargoType;
use crate::game_state::GameState;
use crate::map::{TileCoord, coord_to_linear_index};
use crate::vehicle::{Vehicle, VehicleKind, VehicleOrder};

/// `OT_GOTO_STATION` / `OT_GOTO_DEPOT` / `OT_GOTO_WAYPOINT` / `OT_CONDITIONAL` (`order_type.h`).
const OT_GOTO_STATION: u8 = 1;
const OT_GOTO_DEPOT: u8 = 2;
const OT_GOTO_WAYPOINT: u8 = 6;
const OT_CONDITIONAL: u8 = 7;
const OTTD_DEPOT_SERVICE: u8 = 1 << 0;
const OTTD_DEPOT_PART_OF_ORDERS: u8 = 1 << 1;
const OTTD_DEPOT_HALT: u8 = 1 << 3;
/// Cabeza de convoy (`GVSF_FRONT`).
const GVSF_FRONT: u8 = 0x01;

fn station_id_for_pos(state: &GameState, pos: TileCoord) -> Option<u16> {
    state
        .stations
        .iter()
        .position(|s| s.pos == pos)
        .and_then(|i| u16::try_from(i).ok())
}

fn cargo_ottd_byte(v: &Vehicle) -> u8 {
    match v.cargo_type {
        Some(c) => c.temperate_id(),
        None => match v.kind {
            VehicleKind::Bus | VehicleKind::Aircraft => 0,
            _ => 1,
        },
    }
}

fn encode_goto_order(order: &VehicleOrder, state: &GameState, map_w: u32) -> Option<Vec<u8>> {
    let (order_type, dest, flags, refit) = match *order {
        VehicleOrder::Station {
            station,
            full_load,
            no_unload,
            ..
        } => {
            let id = station_id_for_pos(state, station)?;
            let mut flags = 0u8;
            if full_load {
                flags |= 2 << 4; // FullLoad
            }
            if no_unload {
                flags |= 4; // NoUnload
            }
            (OT_GOTO_STATION, id, flags, 0xFFu8)
        }
        VehicleOrder::Waypoint { waypoint, .. } => {
            let id = station_id_for_pos(state, waypoint)?;
            (OT_GOTO_WAYPOINT, id, 0, 0xFF)
        }
        VehicleOrder::Depot {
            depot,
            stop,
            refit_cargo,
            ..
        } => {
            let id = u16::try_from(coord_to_linear_index(depot, map_w)?).ok()?;
            let mut flags = OTTD_DEPOT_PART_OF_ORDERS;
            if stop {
                flags |= OTTD_DEPOT_HALT;
            } else {
                flags |= OTTD_DEPOT_SERVICE;
            }
            let refit = refit_cargo.map_or(0xFF, CargoType::temperate_id);
            (OT_GOTO_DEPOT, id, flags, refit)
        }
        VehicleOrder::Conditional {
            condition,
            value,
            jump_to,
        } => {
            // LoadPercentage (var 0) + MoreThan(4) / LessThan(2).
            let comparator: u8 = match condition {
                crate::vehicle::OrderConditionKind::CargoLoadAbove => 4,
                crate::vehicle::OrderConditionKind::CargoLoadBelow => 2,
            };
            let order_type = OT_CONDITIONAL | (comparator << 5);
            let flags = u8::try_from(jump_to.min(255)).unwrap_or(255);
            let dest = u16::from(value); // variable 0 in high bits
            (order_type, dest, flags, 0xFF)
        }
        VehicleOrder::Tile(_) => return None,
    };
    let mut o = Vec::with_capacity(10);
    o.push(order_type);
    o.push(flags);
    o.extend_from_slice(&dest.to_be_bytes());
    o.push(refit);
    o.extend_from_slice(&0u16.to_be_bytes()); // wait_time
    o.extend_from_slice(&0u16.to_be_bytes()); // travel_time
    o.extend_from_slice(&0u16.to_be_bytes()); // max_speed
    Some(o)
}

type SavRecordBytes = Vec<u8>;
type SavRecordList = Vec<SavRecordBytes>;

/// Una lista ORDL por vehículo (solo órdenes goto estación/waypoint).
///
/// # Errors
///
/// Falla si algún valor gamma está fuera de rango.
pub(super) fn ordl_and_vehs_records(
    state: &GameState,
    map_w: u32,
) -> Result<(SavRecordList, SavRecordList), SavError> {
    let mut ordl = Vec::new();
    let mut vehs = Vec::new();
    let mut sparse_idx = 0u32;

    for v in &state.vehicles {
        if !matches!(
            v.kind,
            VehicleKind::Train | VehicleKind::Bus | VehicleKind::Truck
        ) {
            continue;
        }
        let Some(tile_idx) = coord_to_linear_index(v.pos, map_w) else {
            continue;
        };

        let mut order_bytes = Vec::new();
        for order in &v.orders {
            if let Some(enc) = encode_goto_order(order, state, map_w) {
                order_bytes.push(enc);
            }
        }
        let order_list_ref = if order_bytes.is_empty() {
            0u32
        } else {
            let list_idx = u32::try_from(ordl.len()).unwrap_or(0);
            let mut rec = Vec::new();
            write_gamma(order_bytes.len() as u32, &mut rec)?; // count of orders struct
            for o in &order_bytes {
                rec.extend_from_slice(o);
            }
            ordl.push(rec);
            list_idx + 1
        };

        let vtype: u8 = match v.kind {
            VehicleKind::Train => 0,
            VehicleKind::Bus | VehicleKind::Truck => 1,
            _ => continue,
        };
        let cargo = cargo_ottd_byte(v);
        let cur_order = u8::try_from(v.current_order.min(255)).unwrap_or(0);
        let vehstatus = u8::from(!v.running); // bit 0 = stopped

        let mut rec = Vec::new();
        write_gamma(sparse_idx, &mut rec)?;
        rec.push(vtype);
        if vtype == 0 {
            // train presente, roadveh ausente
            write_vehs_common(
                &mut rec,
                tile_idx,
                cargo,
                order_list_ref,
                cur_order,
                vehstatus,
            );
            rec.push(0); // roadveh count = 0
        } else {
            rec.push(0); // train ausente
            write_vehs_common(
                &mut rec,
                tile_idx,
                cargo,
                order_list_ref,
                cur_order,
                vehstatus,
            );
        }
        vehs.push(rec);
        sparse_idx += 1;
    }
    Ok((ordl, vehs))
}

fn write_vehs_common(
    buf: &mut Vec<u8>,
    tile: u32,
    cargo: u8,
    order_list_ref: u32,
    cur_order: u8,
    vehstatus: u8,
) {
    buf.push(1); // train/roadveh struct count
    buf.push(1); // common struct count
    buf.extend_from_slice(&tile.to_be_bytes());
    buf.push(GVSF_FRONT);
    buf.push(cargo);
    buf.extend_from_slice(&order_list_ref.to_be_bytes());
    buf.push(cur_order);
    buf.push(vehstatus);
}

/// Construye chunk ORDL con records.
///
/// # Errors
///
/// Falla si algún valor gamma está fuera de rango.
pub(super) fn ordl_chunk(records: &[Vec<u8>]) -> Result<Vec<u8>, SavError> {
    // Header con struct anidado `orders` (como gen_demo_sav.py).
    let mut header = Vec::new();
    header.push(0x1B); // STRUCT | HAS_LENGTH
    write_str("orders", &mut header)?;
    header.push(0); // fin lista top-level → subcampos de orders
    header.push(2);
    write_str("type", &mut header)?;
    header.push(2);
    write_str("flags", &mut header)?;
    header.push(4);
    write_str("dest", &mut header)?;
    header.push(2);
    write_str("refit_cargo", &mut header)?;
    header.push(4);
    write_str("wait_time", &mut header)?;
    header.push(4);
    write_str("travel_time", &mut header)?;
    header.push(4);
    write_str("max_speed", &mut header)?;
    header.push(0);
    raw_table_chunk(*b"ORDL", &header, records, CH_TABLE)
}

/// Construye chunk VEHS con records.
///
/// # Errors
///
/// Falla si algún valor gamma está fuera de rango.
pub(super) fn vehs_chunk(records: &[Vec<u8>]) -> Result<Vec<u8>, SavError> {
    let mut header = Vec::new();
    header.push(2);
    write_str("type", &mut header)?;
    header.push(0x1B); // STRUCT | HAS_LENGTH
    write_str("train", &mut header)?;
    header.push(0x1B);
    write_str("roadveh", &mut header)?;
    header.push(0);
    for _ in 0..2 {
        header.push(0x1B);
        write_str("common", &mut header)?;
        header.push(0);
        header.push(6);
        write_str("tile", &mut header)?;
        header.push(2);
        write_str("subtype", &mut header)?;
        header.push(2);
        write_str("cargo_type", &mut header)?;
        header.push(6);
        write_str("orders", &mut header)?;
        header.push(2);
        write_str("cur_real_order_index", &mut header)?;
        header.push(2);
        write_str("vehstatus", &mut header)?;
        header.push(0);
    }
    raw_table_chunk(*b"VEHS", &header, records, CH_SPARSE_TABLE)
}
