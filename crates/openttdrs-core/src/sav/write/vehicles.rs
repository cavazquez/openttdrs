//! Serialización de vehículos y órdenes (ORDL, VEHS).
//!
//! Schema VEHS mínimo loadable por `OpenTTD` ≥15.3 (#226):
//! `direction`/`owner`/`engine_type`/`x_pos`/`y_pos`/`z_pos` son
//! obligatorios — sin ellos `AfterLoad` deja `INVALID_DIR`/`INVALID_OWNER` y
//! crashea en `Train::UpdateDeltaXY` / `GetImage`.
//!
//! Tren: + `track`. ROAD (bus/camión): tesela `MP_ROAD` con roadtype válido
//! (`m4`/`M3HI`; 0 = `ROADTYPE_ROAD`). Ship/aircraft/tram quedan fuera.

use super::super::SavError;
use super::super::chunks::{CH_SPARSE_TABLE, CH_TABLE};
use super::super::orders_codec::{append_ordl_orders_header, encode_vehicle_order};
use super::chunks::raw_table_chunk;
use super::codec::{write_gamma, write_str};
use crate::game_state::GameState;
use crate::map::{TileCoord, TileKind, coord_to_linear_index};
use crate::vehicle::{
    DIR_NE, DIR_NW, DIR_SE, DIR_SW, Vehicle, VehicleKind, VehicleOrder,
};

/// Cabeza de convoy + motor (`GVSF_FRONT | GVSF_ENGINE`).
const TRAIN_SUBTYPE_FRONT_ENGINE: u8 = 0x01 | 0x08;

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

/// `VEH_TRAIN` / `VEH_ROAD`.
const VEH_TRAIN: u8 = 0;
const VEH_ROAD: u8 = 1;

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
        VehicleKind::Bus | VehicleKind::Tram => 0, // pasajeros
        VehicleKind::Truck => 2,                   // correo
        _ => 1,                                    // carbón (tren)
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
    if bits == 0 {
        TRACK_BIT_X
    } else {
        bits
    }
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
    if on_y && !on_x {
        DIR_SE
    } else {
        DIR_NE
    }
}

/// `OpenTTD` exige roadtype válido en la tesela del ROAD vehicle (`AfterLoad`).
fn road_tile_ok(state: &GameState, pos: TileCoord) -> bool {
    let Some(tile) = state.map.get(pos) else {
        return false;
    };
    matches!(tile.kind, TileKind::Road | TileKind::RoadDepot)
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
}

fn push_orders(
    v: &Vehicle,
    state: &GameState,
    map_w: u32,
    ordl: &mut SavRecordList,
) -> Result<u32, SavError> {
    let mut order_bytes = Vec::new();
    for order in &v.orders {
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

fn common_wire_for(
    v: &Vehicle,
    tile_idx: u32,
    direction: u8,
    engine_type: u16,
    order_list_ref: u32,
) -> CommonWire {
    let cargo = cargo_ottd_byte(v);
    let cur_order = u8::try_from(v.current_order.min(255)).unwrap_or(0);
    let vehstatus = if v.running { 0 } else { VEHSTATUS_STOPPED };
    let x_pos = v.pos.x * TILE_SIZE + i32::from(v.rail_pixel.min(15));
    let y_pos = v.pos.y * TILE_SIZE + TILE_SIZE / 2;
    let z_pos = i32::from(v.z_pos.unwrap_or(0));
    CommonWire {
        subtype: TRAIN_SUBTYPE_FRONT_ENGINE,
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
    }
}

/// ORDL + VEHS: cabezas tren y ROAD (bus/camión sobre carretera).
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
        let is_train = v.kind == VehicleKind::Train;
        let is_road = matches!(v.kind, VehicleKind::Bus | VehicleKind::Truck);
        if !is_train && !is_road {
            continue;
        }
        if is_road && !road_tile_ok(state, v.pos) {
            // Sin roadtype válido AfterLoad hace SlErrorCorrupt.
            continue;
        }
        let Some(tile_idx) = coord_to_linear_index(v.pos, map_w) else {
            continue;
        };

        let order_list_ref = push_orders(v, state, map_w, &mut ordl)?;

        let mut rec = Vec::new();
        write_gamma(sparse_idx, &mut rec)?;

        if is_train {
            let track = track_bits_for(state, v.pos);
            let direction = train_direction(track, v.direction);
            let engine_type = openttd_train_engine_type(v);
            rec.push(VEH_TRAIN);
            write_gamma(1, &mut rec)?; // train presente
            write_gamma(1, &mut rec)?; // common presente
            write_vehs_common(
                &mut rec,
                &common_wire_for(v, tile_idx, direction, engine_type, order_list_ref),
            );
            rec.push(track);
            write_gamma(0, &mut rec)?; // roadveh ausente
        } else {
            let direction = if matches!(v.direction, DIR_NE | DIR_SE | DIR_SW | DIR_NW) {
                v.direction
            } else {
                DIR_NE
            };
            let engine_type = openttd_road_engine_type(v);
            rec.push(VEH_ROAD);
            write_gamma(0, &mut rec)?; // train ausente
            write_gamma(1, &mut rec)?; // roadveh presente
            write_gamma(1, &mut rec)?; // common presente
            write_vehs_common(
                &mut rec,
                &common_wire_for(v, tile_idx, direction, engine_type, order_list_ref),
            );
            // state/frame/path/gv_flags: ausentes en header → defaults 0 (TRACKDIR_NE).
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

/// Header VEHS mínimo (train + roadveh) alineado con `vehicle_sl.cpp`.
fn append_vehs_header(header: &mut Vec<u8>) -> Result<(), SavError> {
    append_field(header, 2, "type")?; // SAVEBYTE → U8
    append_field(header, 0x1B, "train")?;
    append_field(header, 0x1B, "roadveh")?;
    header.push(0);

    // train → common + track
    append_field(header, 0x1B, "common")?;
    append_field(header, 2, "track")?;
    header.push(0);
    append_vehs_common_fields(header)?;

    // roadveh → common (state/path/gv_flags usan defaults si faltan)
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
