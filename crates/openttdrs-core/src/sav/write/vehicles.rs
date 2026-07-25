//! Serialización de vehículos y órdenes (ORDL, VEHS).

use super::super::SavError;
use super::super::chunks::{CH_SPARSE_TABLE, CH_TABLE};
use super::super::orders_codec::{append_ordl_orders_header, encode_vehicle_order};
use super::chunks::raw_table_chunk;
use super::codec::{write_gamma, write_str};
use crate::game_state::GameState;
use crate::map::{TileCoord, coord_to_linear_index};
use crate::vehicle::{Vehicle, VehicleKind, VehicleOrder};

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
    encode_vehicle_order(order, |pos| station_id_for_pos(state, pos), map_w).map(|b| b.to_vec())
}

type SavRecordBytes = Vec<u8>;
type SavRecordList = Vec<SavRecordBytes>;

/// Una lista ORDL por vehículo (goto estación/waypoint/depósito/condicional).
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
