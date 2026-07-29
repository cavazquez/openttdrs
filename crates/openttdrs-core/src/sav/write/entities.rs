//! Serialización de entidades: estaciones, ciudades, industrias.

use super::super::SavError;
use super::super::chunks::CH_TABLE;
use super::chunks::raw_table_chunk;
use super::codec::{write_gamma, write_str};
use crate::game_state::GameState;
use crate::industry::{Industry, IndustryKind, IndustrySpec};
use crate::map::coord_to_linear_index;
use crate::station::{Station, StopKind};

/// Bits `FACIL_*` al escribir `STNN` (alineados con el import).
const FACIL_TRAIN: u8 = 0x01;
const FACIL_TRUCK_STOP: u8 = 0x02;
const FACIL_BUS_STOP: u8 = 0x04;
const FACIL_AIRPORT: u8 = 0x08;
const FACIL_DOCK: u8 = 0x10;
const FACIL_WAYPOINT: u8 = 0x80;

/// `INVALID_TILE` en `OpenTTD`.
const INVALID_TILE: u32 = 0xFFFF_FFFF;

/// `NUM_CARGO` (`OpenTTD` moderno).
const NUM_CARGO: u32 = 64;

/// `STR_SV_STNAME` — plantilla de nombre generado.
const STR_SV_STNAME: u16 = 0x6006;

/// `VEH_INVALID`.
const VEH_INVALID: u8 = 0xFF;

fn facilities_for_stop(kind: StopKind) -> u8 {
    match kind {
        StopKind::RailStation => FACIL_TRAIN,
        StopKind::TruckStop => FACIL_TRUCK_STOP,
        StopKind::BusStop => FACIL_BUS_STOP,
        StopKind::Dock | StopKind::Buoy => FACIL_DOCK,
        StopKind::Airport => FACIL_AIRPORT,
        StopKind::RailWaypoint => FACIL_WAYPOINT | FACIL_TRAIN,
        StopKind::RoadWaypoint => FACIL_WAYPOINT | FACIL_BUS_STOP | FACIL_TRUCK_STOP,
    }
}

fn is_waypoint(facilities: u8) -> bool {
    facilities & FACIL_WAYPOINT != 0
}

fn append_field(header: &mut Vec<u8>, ftype: u8, name: &str) -> Result<(), SavError> {
    header.push(ftype);
    write_str(name, header)
}

/// Header `STNN` moderno (SLV ≥ 340): SAVEBYTE + structs anidados.
///
/// Orden alineado con `station_sl.cpp` / save 15.3 (ver fixtures `*_15_3.sav`).
fn append_stnn_header(header: &mut Vec<u8>) -> Result<(), SavError> {
    // Top: facilities (SAVEBYTE→U8), normal, waypoint, speclist*, roadstoptiledata.
    append_field(header, 2, "facilities")?; // SLE_FILE_U8
    append_field(header, 0x1B, "normal")?; // STRUCT | HAS_LENGTH
    append_field(header, 0x1B, "waypoint")?;
    append_field(header, 0x1B, "speclist")?;
    append_field(header, 0x1B, "roadstopspeclist")?;
    append_field(header, 0x1B, "roadstoptiledata")?;
    header.push(0);

    // Nest: SlStationNormal
    append_field(header, 0x1B, "base")?;
    append_field(header, 6, "train_station.tile")?;
    append_field(header, 2, "train_station.w")?;
    append_field(header, 2, "train_station.h")?;
    append_field(header, 6, "bus_stops")?; // REF → U32
    append_field(header, 6, "truck_stops")?;
    append_field(header, 6, "ship_station.tile")?;
    append_field(header, 2, "ship_station.w")?;
    append_field(header, 2, "ship_station.h")?;
    append_field(header, 6, "docking_station.tile")?;
    append_field(header, 2, "docking_station.w")?;
    append_field(header, 2, "docking_station.h")?;
    append_field(header, 6, "airport.tile")?;
    append_field(header, 2, "airport.w")?;
    append_field(header, 2, "airport.h")?;
    append_field(header, 2, "airport.type")?;
    append_field(header, 2, "airport.layout")?;
    append_field(header, 8, "airport.flags")?;
    append_field(header, 2, "airport.rotation")?;
    append_field(header, 6, "airport.psa")?;
    append_field(header, 2, "indtype")?;
    append_field(header, 2, "time_since_load")?;
    append_field(header, 2, "time_since_unload")?;
    append_field(header, 2, "last_vehicle_type")?;
    append_field(header, 2, "had_vehicle_of_type")?;
    append_field(header, 0x16, "loading_vehicles")?; // REF U32 | HAS_LENGTH
    append_field(header, 8, "always_accepted")?;
    append_field(header, 0x1B, "goods")?;
    header.push(0);

    // Nest: SlStationBase (normal.base)
    append_stnn_base_header(header)?;

    // Nest: SlStationGoods
    append_field(header, 2, "status")?;
    append_field(header, 2, "time_since_pickup")?;
    append_field(header, 2, "rating")?;
    append_field(header, 2, "last_speed")?;
    append_field(header, 2, "last_age")?;
    append_field(header, 2, "amount_fract")?;
    append_field(header, 6, "cargo.reserved_count")?;
    append_field(header, 4, "link_graph")?;
    append_field(header, 4, "node")?;
    append_field(header, 6, "max_waiting_cargo")?;
    append_field(header, 0x1B, "flow")?;
    append_field(header, 0x1B, "cargo")?;
    header.push(0);

    // Nest: SlStationFlow
    append_field(header, 4, "source")?;
    append_field(header, 4, "via")?;
    append_field(header, 6, "share")?;
    append_field(header, 1, "restricted")?; // SLE_FILE_I8 / bool
    header.push(0);

    // Nest: SlStationCargo
    append_field(header, 4, "first")?;
    append_field(header, 0x16, "second")?; // REFLIST
    header.push(0);

    // Nest: SlStationWaypoint
    append_field(header, 0x1B, "base")?;
    append_field(header, 4, "town_cn")?;
    append_field(header, 6, "train_station.tile")?;
    append_field(header, 2, "train_station.w")?;
    append_field(header, 2, "train_station.h")?;
    append_field(header, 4, "waypoint_flags")?;
    append_field(header, 6, "road_waypoint_area.tile")?;
    append_field(header, 2, "road_waypoint_area.w")?;
    append_field(header, 2, "road_waypoint_area.h")?;
    header.push(0);

    // Nest: waypoint.base
    append_stnn_base_header(header)?;

    // Nest: speclist (StationSpec)
    append_field(header, 6, "grfid")?;
    append_field(header, 4, "localidx")?;
    header.push(0);

    // Nest: roadstopspeclist
    append_field(header, 6, "grfid")?;
    append_field(header, 4, "localidx")?;
    header.push(0);

    // Nest: roadstoptiledata
    append_field(header, 6, "tile")?;
    append_field(header, 2, "random_bits")?;
    append_field(header, 2, "animation_frame")?;
    header.push(0);

    Ok(())
}

fn append_stnn_base_header(header: &mut Vec<u8>) -> Result<(), SavError> {
    append_field(header, 6, "xy")?;
    append_field(header, 6, "town")?;
    append_field(header, 9, "string_id")?; // SLE_FILE_STRINGID
    append_field(header, 0x1A, "name")?; // STRING | HAS_LENGTH
    append_field(header, 2, "delete_ctr")?;
    append_field(header, 2, "owner")?;
    append_field(header, 2, "facilities")?;
    append_field(header, 5, "build_date")?; // I32
    append_field(header, 4, "random_bits")?;
    append_field(header, 2, "waiting_triggers")?;
    header.push(0);
    Ok(())
}

fn write_empty_goods_entry(buf: &mut Vec<u8>) -> Result<(), SavError> {
    // Defaults típicos de GoodsEntry vacía en saves 15.3.
    buf.push(0); // status
    buf.push(255); // time_since_pickup
    buf.push(175); // rating
    buf.push(0); // last_speed
    buf.push(255); // last_age
    buf.push(0); // amount_fract
    buf.extend_from_slice(&0u32.to_be_bytes()); // cargo.reserved_count
    buf.extend_from_slice(&u16::MAX.to_be_bytes()); // link_graph
    buf.extend_from_slice(&u16::MAX.to_be_bytes()); // node
    buf.extend_from_slice(&0u32.to_be_bytes()); // max_waiting_cargo
    write_gamma(0, buf)?; // flow
    write_gamma(0, buf)?; // cargo
    Ok(())
}

fn write_stnn_base(
    buf: &mut Vec<u8>,
    tile_idx: u32,
    name: &str,
    facilities: u8,
    town_ref: u32,
) -> Result<(), SavError> {
    buf.extend_from_slice(&tile_idx.to_be_bytes());
    buf.extend_from_slice(&town_ref.to_be_bytes());
    buf.extend_from_slice(&STR_SV_STNAME.to_be_bytes());
    write_str(name, buf)?;
    buf.push(0); // delete_ctr
    buf.push(0); // owner (compañía 0)
    buf.push(facilities);
    buf.extend_from_slice(&0i32.to_be_bytes()); // build_date
    buf.extend_from_slice(&0u16.to_be_bytes()); // random_bits
    buf.push(0); // waiting_triggers
    Ok(())
}

fn write_stnn_normal(
    buf: &mut Vec<u8>,
    st: &Station,
    tile_idx: u32,
    facilities: u8,
    town_ref: u32,
) -> Result<(), SavError> {
    let name = st.name.as_deref().unwrap_or("");
    buf.push(1); // base presente
    write_stnn_base(buf, tile_idx, name, facilities, town_ref)?;

    let (train_tile, train_w, train_h) =
        if facilities & FACIL_TRAIN != 0 && !is_waypoint(facilities) {
            (tile_idx, 1u8, 1u8)
        } else {
            (INVALID_TILE, 0u8, 0u8)
        };
    buf.extend_from_slice(&train_tile.to_be_bytes());
    buf.push(train_w);
    buf.push(train_h);

    // bus_stops / truck_stops: sin chunk ROAD → null.
    buf.extend_from_slice(&0u32.to_be_bytes());
    buf.extend_from_slice(&0u32.to_be_bytes());

    buf.extend_from_slice(&INVALID_TILE.to_be_bytes()); // ship_station.tile
    buf.push(0);
    buf.push(0);
    buf.extend_from_slice(&INVALID_TILE.to_be_bytes()); // docking_station.tile
    buf.push(0);
    buf.push(0);

    let (air_tile, air_w, air_h, air_type) = if facilities & FACIL_AIRPORT != 0 {
        (tile_idx, 1u8, 1u8, 0u8)
    } else {
        (INVALID_TILE, 0u8, 0u8, 0u8)
    };
    buf.extend_from_slice(&air_tile.to_be_bytes());
    buf.push(air_w);
    buf.push(air_h);
    buf.push(air_type);
    buf.push(0); // layout
    buf.extend_from_slice(&0u64.to_be_bytes()); // airport.flags
    buf.push(0); // rotation
    buf.extend_from_slice(&0u32.to_be_bytes()); // psa null

    buf.push(0); // indtype
    buf.push(0); // time_since_load
    buf.push(0); // time_since_unload
    buf.push(VEH_INVALID); // last_vehicle_type
    buf.push(0); // had_vehicle_of_type
    write_gamma(0, buf)?; // loading_vehicles
    buf.extend_from_slice(&0u64.to_be_bytes()); // always_accepted

    write_gamma(NUM_CARGO, buf)?;
    for _ in 0..NUM_CARGO {
        write_empty_goods_entry(buf)?;
    }
    Ok(())
}

fn write_stnn_waypoint(
    buf: &mut Vec<u8>,
    st: &Station,
    tile_idx: u32,
    facilities: u8,
    town_ref: u32,
) -> Result<(), SavError> {
    let name = st.name.as_deref().unwrap_or("");
    buf.push(1); // base presente
    write_stnn_base(buf, tile_idx, name, facilities, town_ref)?;
    buf.extend_from_slice(&0u16.to_be_bytes()); // town_cn
    let (train_tile, w, h) = if facilities & FACIL_TRAIN != 0 {
        (tile_idx, 1u8, 1u8)
    } else {
        (INVALID_TILE, 0u8, 0u8)
    };
    buf.extend_from_slice(&train_tile.to_be_bytes());
    buf.push(w);
    buf.push(h);
    buf.extend_from_slice(&0u16.to_be_bytes()); // waypoint_flags
    let (road_tile, rw, rh) = if facilities & (FACIL_BUS_STOP | FACIL_TRUCK_STOP) != 0 {
        (tile_idx, 1u8, 1u8)
    } else {
        (INVALID_TILE, 0u8, 0u8)
    };
    buf.extend_from_slice(&road_tile.to_be_bytes());
    buf.push(rw);
    buf.push(rh);
    Ok(())
}

/// Construye records STNN modernos (SAVEBYTE + structs) desde estaciones del estado.
///
/// # Errors
///
/// Falla si algún nombre de estación es demasiado largo.
pub(super) fn stnn_records(state: &GameState, map_w: u32) -> Result<Vec<Vec<u8>>, SavError> {
    // CITY sintético o real: el primer municipio es índice 0 → ref 1.
    let town_ref = 1u32;
    let mut out = Vec::with_capacity(state.stations.len());
    for st in &state.stations {
        if st.pos.x < 0 || st.pos.y < 0 {
            continue;
        }
        let ux = st.pos.x.cast_unsigned();
        let uy = st.pos.y.cast_unsigned();
        let tile_idx = uy.saturating_mul(map_w).saturating_add(ux);
        let facilities = facilities_for_stop(st.stop_kind);
        let mut rec = Vec::new();
        // SAVEBYTE: leído a mano por OpenTTD antes de SlObject.
        rec.push(facilities);
        if is_waypoint(facilities) {
            rec.push(0); // normal ausente
            rec.push(1); // waypoint presente
            write_stnn_waypoint(&mut rec, st, tile_idx, facilities, town_ref)?;
        } else {
            rec.push(1); // normal presente
            write_stnn_normal(&mut rec, st, tile_idx, facilities, town_ref)?;
            rec.push(0); // waypoint ausente
        }
        write_gamma(0, &mut rec)?; // speclist
        write_gamma(0, &mut rec)?; // roadstopspeclist
        write_gamma(0, &mut rec)?; // roadstoptiledata
        out.push(rec);
    }
    Ok(out)
}

/// Chunk `STNN` `CH_TABLE` con schema moderno.
///
/// # Errors
///
/// Falla si el header o algún gamma está fuera de rango.
pub(super) fn stnn_chunk(records: &[Vec<u8>]) -> Result<Vec<u8>, SavError> {
    let mut header = Vec::new();
    append_stnn_header(&mut header)?;
    raw_table_chunk(*b"STNN", &header, records, CH_TABLE)
}

/// Record CITY mínimo (`OpenTTD` exige ≥1 municipio: `STR_ERROR_NO_TOWN_IN_SCENARIO`).
pub(super) fn default_city_record(map_w: u32, map_h: u32) -> Result<Vec<u8>, SavError> {
    let x = map_w / 2;
    let y = map_h / 2;
    let tile_idx = y.saturating_mul(map_w).saturating_add(x);
    let mut rec = Vec::new();
    rec.extend_from_slice(&tile_idx.to_be_bytes());
    write_str("Town", &mut rec)?;
    rec.extend_from_slice(&500u32.to_be_bytes()); // cache.population
    rec.extend_from_slice(&0u32.to_be_bytes()); // townnamegrfid
    rec.extend_from_slice(&0x20C0u16.to_be_bytes()); // townnametype (inglés)
    rec.extend_from_slice(&0u32.to_be_bytes()); // townnameparts
    Ok(rec)
}

/// Construye records CITY desde ciudades del estado.
///
/// Si no hay towns, emite un municipio sintético (requerido por `OpenTTD` al load).
///
/// # Errors
///
/// Falla si algún nombre de ciudad es demasiado largo.
pub(super) fn city_records(state: &GameState, map_w: u32) -> Result<Vec<Vec<u8>>, SavError> {
    let mut out = Vec::with_capacity(state.towns.len().max(1));
    for town in &state.towns {
        let Some(tile_idx) = coord_to_linear_index(town.pos, map_w) else {
            continue;
        };
        let mut rec = Vec::new();
        rec.extend_from_slice(&tile_idx.to_be_bytes());
        write_str(&town.name, &mut rec)?;
        // cache.population: el import la pone en 0 y rebuild_town_populations la recalcula;
        // igual la escribimos para roundtrip de lectura best-effort / fixtures.
        rec.extend_from_slice(&town.population.to_be_bytes());
        rec.extend_from_slice(&0u32.to_be_bytes()); // townnamegrfid
        rec.extend_from_slice(&0x20C0u16.to_be_bytes()); // townnametype (inglés)
        rec.extend_from_slice(&0u32.to_be_bytes()); // townnameparts
        out.push(rec);
    }
    if out.is_empty() {
        let (_, h) = state.map.dimensions();
        out.push(default_city_record(map_w, h)?);
    }
    Ok(out)
}

fn industry_ottd_type(ind: &Industry) -> u8 {
    // Índices temperate OpenTTD (`table/industry.h`); best-effort.
    let spec = ind.spec.unwrap_or(match ind.kind {
        IndustryKind::CoalMine => IndustrySpec::CoalMine,
        IndustryKind::Forest => IndustrySpec::Forest,
        IndustryKind::OilWell => IndustrySpec::OilWells,
        IndustryKind::Factory => IndustrySpec::Factory,
    });
    match spec {
        IndustrySpec::CoalMine => 0,
        IndustrySpec::PowerStation => 1,
        IndustrySpec::Sawmill => 2,
        IndustrySpec::Forest => 3,
        IndustrySpec::OilRefinery => 4,
        IndustrySpec::OilWells => 5,
        IndustrySpec::Farm => 6,
        IndustrySpec::Factory => 7,
        IndustrySpec::IronOreMine => 8,
        IndustrySpec::GoldMine => 18,
        IndustrySpec::CopperOreMine => 24,
        other => {
            let _ = other;
            0
        }
    }
}

fn industry_footprint(ind: &Industry) -> (u8, u8) {
    if ind.tiles.is_empty() {
        return (1, 1);
    }
    let min_x = ind.tiles.iter().map(|t| t.x).min().unwrap_or(ind.pos.x);
    let max_x = ind.tiles.iter().map(|t| t.x).max().unwrap_or(ind.pos.x);
    let min_y = ind.tiles.iter().map(|t| t.y).min().unwrap_or(ind.pos.y);
    let max_y = ind.tiles.iter().map(|t| t.y).max().unwrap_or(ind.pos.y);
    let w = u8::try_from((max_x - min_x + 1).clamp(1, 255)).unwrap_or(1);
    let h = u8::try_from((max_y - min_y + 1).clamp(1, 255)).unwrap_or(1);
    (w, h)
}

pub(super) fn indy_records(state: &GameState, map_w: u32) -> Vec<Vec<u8>> {
    let mut out = Vec::with_capacity(state.industries.len());
    for ind in &state.industries {
        let Some(tile_idx) = coord_to_linear_index(ind.pos, map_w) else {
            continue;
        };
        let (w, h) = industry_footprint(ind);
        let mut rec = Vec::new();
        rec.extend_from_slice(&tile_idx.to_be_bytes());
        rec.push(w);
        rec.push(h);
        rec.push(industry_ottd_type(ind));
        out.push(rec);
    }
    out
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::map::TileCoord;
    use crate::station::Station;

    #[test]
    fn stnn_header_starts_with_savebyte_facilities() {
        let mut header = Vec::new();
        append_stnn_header(&mut header).unwrap();
        assert_eq!(header[0], 2); // U8
        assert_eq!(&header[2..12], b"facilities");
        assert!(header.windows(6).any(|w| w == b"normal"));
        assert!(header.windows(5).any(|w| w == b"goods"));
    }

    #[test]
    fn stnn_rail_record_has_savebyte_and_goods64() {
        let mut state = GameState::new(64, 64);
        let mut rail = Station::new_with_kind(TileCoord::new(28, 39), StopKind::RailStation);
        rail.name = Some("Central".into());
        state.stations = vec![rail];
        let recs = stnn_records(&state, 64).unwrap();
        assert_eq!(recs.len(), 1);
        let r = &recs[0];
        assert_eq!(r[0], FACIL_TRAIN);
        assert_eq!(r[1], 1); // normal
        // goods count 64 aparece como gamma 64 = 0x40 tras always_accepted.
        assert!(r.windows(1).any(|w| w[0] == 64) || r.contains(&0x40));
        let chunk = stnn_chunk(&recs).unwrap();
        assert!(chunk.starts_with(b"STNN"));
        assert_eq!(chunk[4], CH_TABLE);
    }
}
