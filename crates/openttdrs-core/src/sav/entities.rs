//! Estaciones (`STNN`), ciudades (`CITY`), industrias (`INDY`), vehículos
//! (`VEHS`) y empresas (`PLYR`) desde tablas autodescriptivas.

use crate::map::TileCoord;
use crate::town::Town;

use super::chunks::{CH_SPARSE_TABLE, CH_TABLE, RawChunk, find_chunk};
use super::table::{SlRecord, SlValue, parse_table_chunk, record_get};

/// Flag de waypoint en `BaseStation::facilities` (no es una estación jugable).
const FACIL_WAYPOINT: u64 = 0x80;

/// Estación decodificada del save (posición + nombre custom + facilities).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavStation {
    pub pos: TileCoord,
    /// Nombre puesto por el jugador; `None` si usa nombre generado.
    pub name: Option<String>,
    /// Bits `FACIL_*` de `OpenTTD` (1 tren, 2 camión, 4 bus, 8 aeropuerto, 0x10 muelle).
    pub facilities: u8,
}

fn tile_to_coord(tile: u64, map_w: u32) -> Option<TileCoord> {
    if map_w == 0 {
        return None;
    }
    let x = i32::try_from(tile % u64::from(map_w)).ok()?;
    let y = i32::try_from(tile / u64::from(map_w)).ok()?;
    Some(TileCoord::new(x, y))
}

fn table_rows(chunk: &RawChunk) -> Vec<(u32, super::table::SlRecord)> {
    let sparse = match chunk.ch_type {
        CH_TABLE => false,
        CH_SPARSE_TABLE => true,
        _ => return Vec::new(),
    };
    parse_table_chunk(&chunk.body, sparse).unwrap_or_default()
}

/// Estaciones del chunk `STNN` (solo saves con tablas, SLV ≥ 295); best-effort.
#[must_use]
pub(crate) fn stations_from_chunks(chunks: &[RawChunk], map_w: u32) -> Vec<SavStation> {
    let Some(stnn) = find_chunk(chunks, "STNN") else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (_, record) in table_rows(stnn) {
        let Some(facilities) = record_get(&record, "facilities").and_then(SlValue::as_u64) else {
            continue;
        };
        if facilities & FACIL_WAYPOINT != 0 {
            continue;
        }
        let Some(xy) = record_get(&record, "xy").and_then(SlValue::as_u64) else {
            continue;
        };
        let Some(pos) = tile_to_coord(xy, map_w) else {
            continue;
        };
        let name = record_get(&record, "name")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(str::to_string);
        #[allow(clippy::cast_possible_truncation)]
        out.push(SavStation {
            pos,
            name,
            facilities: facilities as u8,
        });
    }
    out
}

/// Nombre generado con el generador nativo de `OpenTTD` a partir de los
/// campos `townnamegrfid`/`townnametype`/`townnameparts` del record.
fn generated_town_name(record: &super::table::SlRecord) -> Option<String> {
    let grfid = record_get(record, "townnamegrfid")
        .and_then(SlValue::as_u64)
        .unwrap_or(0);
    let name_type = record_get(record, "townnametype").and_then(SlValue::as_u64)?;
    let parts = record_get(record, "townnameparts").and_then(SlValue::as_u64)?;
    crate::townname::town_name_from_save(
        u32::try_from(grfid).ok()?,
        u16::try_from(name_type).ok()?,
        u32::try_from(parts).ok()?,
    )
}

/// Ciudades del chunk `CITY` (tabla); nombre custom, nombre generado con el
/// generador nativo de `OpenTTD`, o «Ciudad N» como último recurso.
#[must_use]
pub(crate) fn towns_from_chunks(chunks: &[RawChunk], map_w: u32) -> Vec<Town> {
    let Some(city) = find_chunk(chunks, "CITY") else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (idx, record) in table_rows(city) {
        let Some(xy) = record_get(&record, "xy").and_then(SlValue::as_u64) else {
            continue;
        };
        let Some(pos) = tile_to_coord(xy, map_w) else {
            continue;
        };
        let population = record_get(&record, "cache.population")
            .or_else(|| record_get(&record, "population"))
            .and_then(SlValue::as_u64)
            .unwrap_or(0);
        let name = record_get(&record, "name")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .or_else(|| generated_town_name(&record))
            .unwrap_or_else(|| format!("Ciudad {}", idx + 1));
        #[allow(clippy::cast_possible_truncation)]
        out.push(Town {
            id: idx,
            pos,
            name,
            population: population as u32,
        });
    }
    out
}

/// Industria decodificada del chunk `INDY` (saves con tablas).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavIndustry {
    /// Tesela de origen (`location.tile`).
    pub pos: TileCoord,
    /// Dimensiones del rectángulo (`location.w` × `location.h`).
    pub width: u8,
    pub height: u8,
    /// `IndustryType` de `OpenTTD` (índice en la tabla de specs).
    pub industry_type: u8,
}

/// Industrias del chunk `INDY` (solo saves con tablas); best-effort.
#[must_use]
pub(crate) fn industries_from_chunks(chunks: &[RawChunk], map_w: u32) -> Vec<SavIndustry> {
    let Some(indy) = find_chunk(chunks, "INDY") else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (_, record) in table_rows(indy) {
        let Some(tile) = record_get(&record, "location.tile").and_then(SlValue::as_u64) else {
            continue;
        };
        let Some(pos) = tile_to_coord(tile, map_w) else {
            continue;
        };
        let width = record_get(&record, "location.w")
            .and_then(SlValue::as_u64)
            .unwrap_or(1);
        let height = record_get(&record, "location.h")
            .and_then(SlValue::as_u64)
            .unwrap_or(1);
        let industry_type = record_get(&record, "type")
            .and_then(SlValue::as_u64)
            .unwrap_or(0);
        #[allow(clippy::cast_possible_truncation)]
        out.push(SavIndustry {
            pos,
            width: width.min(255) as u8,
            height: height.min(255) as u8,
            industry_type: industry_type.min(255) as u8,
        });
    }
    out
}

/// Dinero de la primera empresa del chunk `PLYR` (la del jugador en partidas locales).
#[must_use]
pub(crate) fn company_money_from_chunks(chunks: &[RawChunk]) -> Option<i64> {
    let plyr = find_chunk(chunks, "PLYR")?;
    let rows = table_rows(plyr);
    let (_, record) = rows.iter().min_by_key(|(idx, _)| *idx)?;
    match record_get(record, "money")? {
        SlValue::Int(v) => Some(*v),
        SlValue::Uint(v) => i64::try_from(*v).ok(),
        _ => None,
    }
}

/// Tipo de vehículo de `OpenTTD` (`VehicleType`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SavVehicleKind {
    Train,
    RoadVehicle,
}

/// Vehículo decodificado del chunk `VEHS` (solo cabezas de convoy).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavVehicle {
    pub kind: SavVehicleKind,
    pub pos: TileCoord,
    /// `CargoType` de `OpenTTD` (0 = pasajeros).
    pub cargo_type: u8,
}

/// Primer (y único) registro de un campo struct de tabla.
fn nested_struct<'a>(record: &'a SlRecord, name: &str) -> Option<&'a SlRecord> {
    match record_get(record, name)? {
        SlValue::Structs(items) => items.first(),
        _ => None,
    }
}

/// Bit `GVSF_FRONT` de `Vehicle::subtype` (cabeza de convoy en tren/camión).
const GVSF_FRONT: u64 = 0x01;

/// Vehículos del chunk `VEHS` (sparse table): trenes y vehículos de carretera
/// cabeza de convoy; barcos, aviones y efectos se omiten.
#[must_use]
pub(crate) fn vehicles_from_chunks(chunks: &[RawChunk], map_w: u32) -> Vec<SavVehicle> {
    let Some(vehs) = find_chunk(chunks, "VEHS") else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (_, record) in table_rows(vehs) {
        let Some(vtype) = record_get(&record, "type").and_then(SlValue::as_u64) else {
            continue;
        };
        let (kind, sub_name) = match vtype {
            0 => (SavVehicleKind::Train, "train"),
            1 => (SavVehicleKind::RoadVehicle, "roadveh"),
            _ => continue,
        };
        let Some(sub) = nested_struct(&record, sub_name) else {
            continue;
        };
        let Some(common) = nested_struct(sub, "common") else {
            continue;
        };
        let subtype = record_get(common, "subtype")
            .and_then(SlValue::as_u64)
            .unwrap_or(0);
        if subtype & GVSF_FRONT == 0 {
            continue;
        }
        let Some(tile) = record_get(common, "tile").and_then(SlValue::as_u64) else {
            continue;
        };
        let Some(pos) = tile_to_coord(tile, map_w) else {
            continue;
        };
        let cargo_type = record_get(common, "cargo_type")
            .and_then(SlValue::as_u64)
            .unwrap_or(0xFF);
        #[allow(clippy::cast_possible_truncation)]
        out.push(SavVehicle {
            kind,
            pos,
            cargo_type: cargo_type.min(255) as u8,
        });
    }
    out
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::cast_possible_truncation)]
mod tests {
    use super::super::chunks::RawChunk;
    use super::super::table::tests::{build_table_body, write_str};
    use super::*;

    fn station_chunk(records: &[Vec<u8>]) -> RawChunk {
        RawChunk {
            name: *b"STNN",
            ch_type: CH_TABLE,
            body: build_table_body(
                &[(6, "xy"), (0x0A | 0x10, "name"), (2, "facilities")],
                records,
            ),
        }
    }

    #[test]
    fn decodes_station_with_custom_name_and_skips_waypoints() {
        let mut st = Vec::new();
        st.extend_from_slice(&(2u32 * 64 + 5).to_be_bytes()); // xy → (5,2) con w=64
        write_str("Mi Estación", &mut st);
        st.push(1); // FACIL_TRAIN

        let mut wp = Vec::new();
        wp.extend_from_slice(&10u32.to_be_bytes());
        write_str("", &mut wp);
        wp.push(0x80); // waypoint

        let chunks = vec![station_chunk(&[st, wp])];
        let stations = stations_from_chunks(&chunks, 64);
        assert_eq!(stations.len(), 1);
        assert_eq!(stations[0].pos, TileCoord::new(5, 2));
        assert_eq!(stations[0].name.as_deref(), Some("Mi Estación"));
        assert_eq!(stations[0].facilities, 1);
    }

    #[test]
    fn decodes_towns_with_population_and_fallback_name() {
        let mut t1 = Vec::new();
        t1.extend_from_slice(&(3u32 * 64 + 3).to_be_bytes());
        write_str("Rosario", &mut t1);
        t1.extend_from_slice(&1234u32.to_be_bytes());

        let mut t2 = Vec::new();
        t2.extend_from_slice(&(7u32 * 64 + 1).to_be_bytes());
        write_str("", &mut t2);
        t2.extend_from_slice(&55u32.to_be_bytes());

        let chunk = RawChunk {
            name: *b"CITY",
            ch_type: CH_TABLE,
            body: build_table_body(
                &[(6, "xy"), (0x0A | 0x10, "name"), (6, "cache.population")],
                &[t1, t2],
            ),
        };
        let towns = towns_from_chunks(&[chunk], 64);
        assert_eq!(towns.len(), 2);
        assert_eq!(towns[0].name, "Rosario");
        assert_eq!(towns[0].population, 1234);
        assert_eq!(towns[0].pos, TileCoord::new(3, 3));
        assert_eq!(towns[1].name, "Ciudad 2");
        assert_eq!(towns[1].population, 55);
    }

    #[test]
    fn town_without_custom_name_uses_native_generator() {
        // grfid=0, townnametype=0x20C0 (inglés original), parts=0 → "Invenville".
        let mut town_gen = Vec::new();
        town_gen.extend_from_slice(&65u32.to_be_bytes()); // (1,1) con w=64
        write_str("", &mut town_gen);
        town_gen.extend_from_slice(&0u32.to_be_bytes()); // townnamegrfid
        town_gen.extend_from_slice(&0x20C0u16.to_be_bytes()); // townnametype
        town_gen.extend_from_slice(&0u32.to_be_bytes()); // townnameparts

        // NewGRF (grfid != 0): no replicable → fallback «Ciudad N».
        let mut grf = Vec::new();
        grf.extend_from_slice(&(2u32 * 64 + 2).to_be_bytes());
        write_str("", &mut grf);
        grf.extend_from_slice(&0xDEAD_BEEFu32.to_be_bytes());
        grf.extend_from_slice(&0x20C0u16.to_be_bytes());
        grf.extend_from_slice(&7u32.to_be_bytes());

        let chunk = RawChunk {
            name: *b"CITY",
            ch_type: CH_TABLE,
            body: build_table_body(
                &[
                    (6, "xy"),
                    (0x0A | 0x10, "name"),
                    (6, "townnamegrfid"),
                    (4, "townnametype"),
                    (6, "townnameparts"),
                ],
                &[town_gen, grf],
            ),
        };
        let towns = towns_from_chunks(&[chunk], 64);
        assert_eq!(towns.len(), 2);
        assert_eq!(towns[0].name, "Invenville");
        assert_eq!(towns[1].name, "Ciudad 2");
    }

    #[test]
    fn missing_chunks_yield_empty() {
        assert!(stations_from_chunks(&[], 64).is_empty());
        assert!(towns_from_chunks(&[], 64).is_empty());
        assert!(industries_from_chunks(&[], 64).is_empty());
        assert!(vehicles_from_chunks(&[], 64).is_empty());
        assert!(company_money_from_chunks(&[]).is_none());
    }

    #[test]
    fn decodes_industries_with_location_and_type() {
        let mut i1 = Vec::new();
        i1.extend_from_slice(&(5u32 * 64 + 10).to_be_bytes()); // location.tile
        i1.push(2); // location.w
        i1.push(3); // location.h
        i1.push(7); // type
        let chunk = RawChunk {
            name: *b"INDY",
            ch_type: CH_TABLE,
            body: build_table_body(
                &[
                    (6, "location.tile"),
                    (2, "location.w"),
                    (2, "location.h"),
                    (2, "type"),
                ],
                &[i1],
            ),
        };
        let industries = industries_from_chunks(&[chunk], 64);
        assert_eq!(industries.len(), 1);
        assert_eq!(industries[0].pos, TileCoord::new(10, 5));
        assert_eq!((industries[0].width, industries[0].height), (2, 3));
        assert_eq!(industries[0].industry_type, 7);
    }

    #[test]
    fn reads_money_from_first_company() {
        let mut c0 = Vec::new();
        c0.extend_from_slice(&500_000i64.to_be_bytes());
        let mut c1 = Vec::new();
        c1.extend_from_slice(&(-42i64).to_be_bytes());
        let chunk = RawChunk {
            name: *b"PLYR",
            ch_type: CH_TABLE,
            body: build_table_body(&[(7, "money")], &[c0, c1]),
        };
        assert_eq!(company_money_from_chunks(&[chunk]), Some(500_000));
    }

    /// Cuerpo VEHS (sparse) con un tren cabeza, un vagón y un bus.
    fn vehs_chunk() -> RawChunk {
        use super::super::table::tests::write_gamma;

        // Struct presente (gamma 1) + struct `common` (gamma 1) + campos.
        fn with_common(tile: u32, subtype: u8, cargo: u8, buf: &mut Vec<u8>) {
            buf.push(1); // train/roadveh: 1 registro
            buf.push(1); // common: 1 registro
            buf.extend_from_slice(&tile.to_be_bytes());
            buf.push(subtype);
            buf.push(cargo);
        }

        // Header: type u8 + structs train/roadveh (cada uno con struct common).
        let mut header = Vec::new();
        header.push(2);
        write_str("type", &mut header);
        header.push(11);
        write_str("train", &mut header);
        header.push(11);
        write_str("roadveh", &mut header);
        header.push(0);
        for _ in 0..2 {
            // Sub-lista de train/roadveh: un struct `common`…
            header.push(11);
            write_str("common", &mut header);
            header.push(0);
            // …cuyos campos son tile/subtype/cargo_type.
            header.push(6);
            write_str("tile", &mut header);
            header.push(2);
            write_str("subtype", &mut header);
            header.push(2);
            write_str("cargo_type", &mut header);
            header.push(0);
        }

        // Tren cabeza (subtype bit0), en (3,1) con w=64.
        let mut v0 = vec![1u8]; // índice sparse 1
        v0.push(0); // type 0 = tren
        with_common(64 + 3, 0x01, 9, &mut v0);
        v0.push(0); // roadveh ausente

        // Vagón del mismo tren: se omite (sin GVSF_FRONT).
        let mut v1 = vec![2u8];
        v1.push(0);
        with_common(64 + 4, 0x04, 9, &mut v1);
        v1.push(0);

        // Bus (roadveh con pasajeros) en (7,2).
        let mut v2 = vec![3u8];
        v2.push(1); // type 1 = carretera
        v2.push(0); // train ausente
        with_common(2 * 64 + 7, 0x01, 0, &mut v2);

        let mut body = Vec::new();
        write_gamma(header.len() as u32 + 1, &mut body);
        body.extend_from_slice(&header);
        for r in [&v0, &v1, &v2] {
            write_gamma(r.len() as u32 + 1, &mut body);
            body.extend_from_slice(r);
        }
        write_gamma(0, &mut body);

        RawChunk {
            name: *b"VEHS",
            ch_type: CH_SPARSE_TABLE,
            body,
        }
    }

    #[test]
    fn decodes_front_vehicles_and_skips_wagons() {
        let vehicles = vehicles_from_chunks(&[vehs_chunk()], 64);
        assert_eq!(vehicles.len(), 2);
        assert_eq!(vehicles[0].kind, SavVehicleKind::Train);
        assert_eq!(vehicles[0].pos, TileCoord::new(3, 1));
        assert_eq!(vehicles[0].cargo_type, 9);
        assert_eq!(vehicles[1].kind, SavVehicleKind::RoadVehicle);
        assert_eq!(vehicles[1].pos, TileCoord::new(7, 2));
        assert_eq!(vehicles[1].cargo_type, 0);
    }
}
