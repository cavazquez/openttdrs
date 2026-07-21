//! Estaciones (`STNN`), ciudades (`CITY`), industrias (`INDY`), vehículos
//! (`VEHS`) y empresas (`PLYR`) desde tablas autodescriptivas.

use crate::map::{TileCoord, coord_from_linear_index};
use crate::town::Town;

use super::chunks::{RawChunk, find_chunk};
use super::table::{SlRecord, SlValue, record_get};

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
    /// `BaseStation::string_id` (`STR_SV_STNAME_*`) cuando no hay nombre custom.
    pub string_id: Option<u16>,
    /// Índice de ciudad (`BaseStation::town`) para armar el nombre generado.
    pub town_id: Option<u32>,
}

/// Entrada del índice de estación (`StationID`) en `STNN`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SavStationIndex {
    pub pos: TileCoord,
    pub is_waypoint: bool,
    pub facilities: u8,
    pub name: Option<String>,
    pub string_id: Option<u16>,
    pub town_id: Option<u32>,
}

/// Primer (y único) registro de un campo struct de tabla.
fn nested_struct<'a>(record: &'a SlRecord, name: &str) -> Option<&'a SlRecord> {
    match record_get(record, name)? {
        SlValue::Structs(items) => items.first(),
        _ => None,
    }
}

/// Base de estación: top-level en saves legacy, anidada en `normal.base` o
/// `waypoint.base` en las tablas `STNN` modernas.
fn station_base_record(record: &SlRecord) -> &SlRecord {
    for station_kind in ["normal", "waypoint"] {
        if let Some(kind) = nested_struct(record, station_kind)
            && let Some(base) = nested_struct(kind, "base")
        {
            return base;
        }
    }
    record
}

/// Mapa `StationID` → tesela para resolver destinos de órdenes.
#[must_use]
pub(crate) fn station_index_from_chunks(
    chunks: &[RawChunk],
    map_w: u32,
    save_version: u16,
) -> std::collections::HashMap<u32, SavStationIndex> {
    let Some(stnn) = find_chunk(chunks, "STNN") else {
        return std::collections::HashMap::new();
    };
    let mut out = std::collections::HashMap::new();
    for (idx, record) in table_rows(stnn, save_version) {
        let base = station_base_record(&record);
        let Some(xy) = record_get(base, "xy").and_then(SlValue::as_u64) else {
            continue;
        };
        let Some(pos) = coord_from_linear_index(xy, map_w) else {
            continue;
        };
        let facilities = record_get(base, "facilities")
            .and_then(SlValue::as_u64)
            .or_else(|| record_get(&record, "facilities").and_then(SlValue::as_u64))
            .unwrap_or(0);
        let is_waypoint = facilities & FACIL_WAYPOINT != 0;
        let name = record_get(base, "name")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(str::to_string);
        let string_id = record_get(base, "string_id")
            .and_then(SlValue::as_u64)
            .and_then(|v| u16::try_from(v).ok());
        // `SLE_REF(..., REF_TOWN)`: 0 = null, resto = `Town::index + 1`.
        #[allow(clippy::cast_possible_truncation)]
        let town_id = record_get(base, "town")
            .and_then(SlValue::as_u64)
            .and_then(|v| (v > 0).then_some((v - 1) as u32));
        #[allow(clippy::cast_possible_truncation)]
        out.insert(
            idx,
            SavStationIndex {
                pos,
                is_waypoint,
                facilities: facilities as u8,
                name,
                string_id,
                town_id,
            },
        );
    }
    out
}

fn table_rows(chunk: &RawChunk, save_version: u16) -> Vec<(u32, super::table::SlRecord)> {
    super::array_legacy::chunk_rows(chunk, save_version)
}

/// Estaciones del chunk `STNN`; best-effort (tabla o array legacy).
#[must_use]
pub(crate) fn stations_from_chunks(
    chunks: &[RawChunk],
    map_w: u32,
    save_version: u16,
) -> Vec<SavStation> {
    station_index_from_chunks(chunks, map_w, save_version)
        .into_values()
        .filter(|st| !st.is_waypoint)
        .map(|st| SavStation {
            pos: st.pos,
            name: st.name,
            facilities: st.facilities,
            string_id: st.string_id,
            town_id: st.town_id,
        })
        .collect()
}

/// `STR_SV_STNAME` … `STR_SV_STNAME_FALLBACK` (`table/strings.h` de `OpenTTD`).
const STR_SV_STNAME: u16 = 0x6006;
const STR_SV_STNAME_FALLBACK: u16 = 0x6027;

/// Nombre de estación generado (plantillas españolas `STR_SV_STNAME_*`).
#[must_use]
pub fn format_generated_station_name(string_id: u16, town_name: &str) -> Option<String> {
    if !(STR_SV_STNAME..=STR_SV_STNAME_FALLBACK).contains(&string_id) {
        return None;
    }
    let t = town_name;
    let formatted = match string_id {
        0x6006 | 0x6017 | 0x6018 => t.to_string(), // BASE / BUOY / WAYPOINT
        0x6007 => format!("{t} Norte"),            // NORTH
        0x6008 => format!("{t} Sur"),              // SOUTH
        0x6009 => format!("{t} Este"),             // EAST
        0x600A => format!("{t} Oeste"),            // WEST
        0x600B => format!("{t} Central"),          // CENTRAL
        0x600C => format!("{t} Transferencia"),    // TRANSFER
        0x600D => format!("{t} Parada"),           // HALT
        0x600E => format!("Valle de {t}"),         // VALLEY
        0x600F => format!("Cumbres de {t}"),       // HEIGHTS
        0x6010 => format!("Arboleda de {t}"),      // WOODS
        0x6011 => format!("Lago de {t}"),          // LAKESIDE
        0x6012 => format!("{t} Intercambio"),      // EXCHANGE
        0x6013 => format!("Aeropuerto de {t}"),    // AIRPORT
        0x6014 => format!("Campo petrolífero de {t}"), // OILFIELD
        0x6015 => format!("Minas de {t}"),         // MINES
        0x6016 => format!("Muelles de {t}"),       // DOCKS
        0x6020 => format!("{t} Anexo"),            // ANNEXE
        0x6021 => format!("Proximidades de {t}"),  // SIDINGS
        0x6022 => format!("Ramal de {t}"),         // BRANCH
        0x6023 => format!("{t} Alto"),             // UPPER
        0x6024 => format!("{t} Bajo"),             // LOWER
        0x6025 => format!("Helipuerto de {t}"),    // HELIPORT
        0x6026 => format!("Bosque de {t}"),        // FOREST
        0x6027 => format!("{t} Estación"),         // FALLBACK (sin #{NUM})
        _ => return None,
    };
    Some(formatted)
}

/// Resuelve el nombre visible: custom, o plantilla `string_id` + ciudad.
#[must_use]
pub fn resolve_sav_station_name(station: &SavStation, towns: &[Town]) -> Option<String> {
    if let Some(name) = station.name.as_ref().filter(|n| !n.is_empty()) {
        return Some(name.clone());
    }
    let string_id = station.string_id?;
    let town_name = station
        .town_id
        .and_then(|id| towns.iter().find(|t| t.id == id))
        .map(|t| t.name.as_str())
        .filter(|n| !n.is_empty())?;
    format_generated_station_name(string_id, town_name)
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
///
/// La población **no** viene en el save: `OpenTTD` la reconstruye al cargar
/// (`RebuildTownCaches`); ver `sav::rebuild_town_populations`.
#[must_use]
pub(crate) fn towns_from_chunks(chunks: &[RawChunk], map_w: u32, save_version: u16) -> Vec<Town> {
    let Some(city) = find_chunk(chunks, "CITY") else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (idx, record) in table_rows(city, save_version) {
        let Some(xy) = record_get(&record, "xy").and_then(SlValue::as_u64) else {
            continue;
        };
        let Some(pos) = coord_from_linear_index(xy, map_w) else {
            continue;
        };
        let name = record_get(&record, "name")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .or_else(|| generated_town_name(&record))
            .unwrap_or_else(|| format!("Ciudad {}", idx + 1));
        out.push(Town {
            id: idx,
            pos,
            name,
            population: 0,
            local_authority_rating: 0,
            passengers_served: 0,
            mail_served: 0,
            growth_funded: 0,
            ..Default::default()
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
    /// `Industry.random_colour` (`Colours`, 0–15) para `PALETTE_MODIFIER_COLOUR`.
    pub random_colour: u8,
}

/// Industrias del chunk `INDY` (solo saves con tablas); best-effort.
#[must_use]
pub(crate) fn industries_from_chunks(
    chunks: &[RawChunk],
    map_w: u32,
    save_version: u16,
) -> Vec<SavIndustry> {
    let Some(indy) = find_chunk(chunks, "INDY") else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (_, record) in table_rows(indy, save_version) {
        if let Some(ind) = sav_industry_from_record(&record, map_w) {
            out.push(ind);
        }
    }
    if out.is_empty() {
        for &(index, industry_type) in &super::build::indy_pairs(chunks) {
            if let Some(pos) = coord_from_linear_index(u64::from(index), map_w) {
                out.push(SavIndustry {
                    pos,
                    width: 1,
                    height: 1,
                    industry_type,
                    random_colour: 0,
                });
            }
        }
    }
    out
}

fn sav_industry_from_record(record: &SlRecord, map_w: u32) -> Option<SavIndustry> {
    let tile = record_get(record, "location.tile").and_then(SlValue::as_u64)?;
    let pos = coord_from_linear_index(tile, map_w)?;
    let width = record_get(record, "location.w")
        .and_then(SlValue::as_u64)
        .unwrap_or(1);
    let height = record_get(record, "location.h")
        .and_then(SlValue::as_u64)
        .unwrap_or(1);
    let industry_type = record_get(record, "type")
        .and_then(SlValue::as_u64)
        .unwrap_or(0);
    let random_colour = record_get(record, "random_colour")
        .and_then(SlValue::as_u64)
        .unwrap_or(0);
    #[allow(clippy::cast_possible_truncation)]
    Some(SavIndustry {
        pos,
        width: width.min(255) as u8,
        height: height.min(255) as u8,
        industry_type: industry_type.min(255) as u8,
        random_colour: (random_colour % 16) as u8,
    })
}

/// Dinero de la primera empresa del chunk `PLYR` (la del jugador en partidas locales).
#[must_use]
pub(crate) fn company_money_from_chunks(chunks: &[RawChunk], save_version: u16) -> Option<i64> {
    let record = first_company_record(chunks, save_version)?;
    match record_get(&record, "money")? {
        SlValue::Int(v) => Some(*v),
        SlValue::Uint(v) => i64::try_from(*v).ok(),
        _ => None,
    }
}

/// Color de compañía (`Colours`) de la primera empresa en `PLYR`.
#[must_use]
pub(crate) fn company_colour_from_chunks(chunks: &[RawChunk], save_version: u16) -> Option<u8> {
    let record = first_company_record(chunks, save_version)?;
    let colour = record_get(&record, "colour")?.as_u64()?;
    Some((colour % 16) as u8)
}

fn first_company_record(chunks: &[RawChunk], save_version: u16) -> Option<SlRecord> {
    let plyr = find_chunk(chunks, "PLYR")?;
    let rows = table_rows(plyr, save_version);
    let (_, record) = rows.into_iter().min_by_key(|(idx, _)| *idx)?;
    Some(record)
}

/// Tipo de vehículo de `OpenTTD` (`VehicleType`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SavVehicleKind {
    Train,
    RoadVehicle,
}

/// Vehículo decodificado del chunk `VEHS`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavVehicle {
    pub kind: SavVehicleKind,
    pub pos: TileCoord,
    /// Progreso sub-tesela (`Vehicle::progress`, 0…255) al guardar.
    pub progress: u8,
    /// Coordenada píxel absoluta (`Vehicle::x_pos` / `y_pos`).
    pub x_pos: i32,
    pub y_pos: i32,
    /// Velocidad y fracción interna al guardar (`cur_speed` / `subspeed`).
    pub cur_speed: u16,
    pub subspeed: u8,
    /// Dirección visual/de movimiento (`Vehicle::direction`) al guardar.
    pub direction: u8,
    /// ID de motor vanilla de `OpenTTD` (`Vehicle::engine_type`).
    pub engine_type: u16,
    /// `CargoType` de `OpenTTD` (0 = pasajeros).
    pub cargo_type: u8,
    /// Órdenes de la lista referenciada (`ORDL`).
    pub orders: Vec<super::orders::SavOrder>,
    /// Índice de orden actual (`cur_real_order_index`).
    pub current_order: usize,
    /// `false` si el jugador detuvo el vehículo (`VehState::Stopped`).
    pub running: bool,
    /// Tren: unidad sin `GVSF_FRONT` (vagón del consist anterior).
    pub is_wagon: bool,
}

/// Bit `GVSF_FRONT` de `Vehicle::subtype` (cabeza de convoy en tren/camión).
const GVSF_FRONT: u64 = 0x01;

/// Vehículos del chunk `VEHS` (sparse table): trenes (cabeza + vagones) y
/// vehículos de carretera cabeza de convoy; barcos/aviones se omiten.
#[must_use]
#[allow(clippy::too_many_lines)]
pub(crate) fn vehicles_from_chunks(
    chunks: &[RawChunk],
    map_w: u32,
    order_import: &super::orders::SavOrderImport,
    save_version: u16,
) -> Vec<SavVehicle> {
    let Some(vehs) = find_chunk(chunks, "VEHS") else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (_, record) in table_rows(vehs, save_version) {
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
        let is_front = subtype & GVSF_FRONT != 0;
        // Carretera: solo cabezas. Tren: cabeza y vagones.
        if kind == SavVehicleKind::RoadVehicle && !is_front {
            continue;
        }
        let is_wagon = kind == SavVehicleKind::Train && !is_front;
        let Some(tile) = record_get(common, "tile").and_then(SlValue::as_u64) else {
            continue;
        };
        let Some(pos) = coord_from_linear_index(tile, map_w) else {
            continue;
        };
        let progress = record_get(common, "progress")
            .and_then(SlValue::as_u64)
            .unwrap_or(0)
            .try_into()
            .unwrap_or(u8::MAX);
        let x_pos = record_get(common, "x_pos")
            .and_then(SlValue::as_i64)
            .unwrap_or(i64::from(pos.x) * 16);
        let y_pos = record_get(common, "y_pos")
            .and_then(SlValue::as_i64)
            .unwrap_or(i64::from(pos.y) * 16);
        let cur_speed = record_get(common, "cur_speed")
            .and_then(SlValue::as_u64)
            .unwrap_or(0)
            .try_into()
            .unwrap_or(u16::MAX);
        let subspeed = record_get(common, "subspeed")
            .and_then(SlValue::as_u64)
            .unwrap_or(0)
            .try_into()
            .unwrap_or(u8::MAX);
        let direction = record_get(common, "direction")
            .and_then(SlValue::as_u64)
            .unwrap_or(0)
            .try_into()
            .unwrap_or(u8::MAX);
        let engine_type = record_get(common, "engine_type")
            .and_then(SlValue::as_u64)
            .unwrap_or(0)
            .try_into()
            .unwrap_or(u16::MAX);
        let cargo_type = record_get(common, "cargo_type")
            .and_then(SlValue::as_u64)
            .unwrap_or(0xFF);
        let order_list_ref = record_get(common, "orders")
            .and_then(SlValue::as_u64)
            .unwrap_or(0);
        let orders = if is_wagon {
            Vec::new()
        } else {
            order_import.orders_for_vehicle_ref(order_list_ref)
        };
        let current_order = record_get(common, "cur_real_order_index")
            .and_then(SlValue::as_u64)
            .and_then(|v| usize::try_from(v).ok())
            .unwrap_or(0);
        let vehstatus = record_get(common, "vehstatus")
            .and_then(SlValue::as_u64)
            .unwrap_or(0);
        let running = vehstatus & 1 == 0;
        #[allow(clippy::cast_possible_truncation)]
        out.push(SavVehicle {
            kind,
            pos,
            progress,
            x_pos: i32::try_from(x_pos).unwrap_or(0),
            y_pos: i32::try_from(y_pos).unwrap_or(0),
            cur_speed,
            subspeed,
            direction,
            engine_type,
            cargo_type: cargo_type.min(255) as u8,
            orders,
            current_order,
            running,
            is_wagon,
        });
    }
    out
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::cast_possible_truncation)]
mod tests {
    use super::super::chunks::{CH_SPARSE_TABLE, CH_TABLE, RawChunk, find_chunk, parse_chunks};
    use super::super::container;
    use super::super::orders::SavOrderImport;
    use super::super::table::{SlValue, record_get};
    use super::super::table::tests::{build_table_body, write_str};
    use super::*;

    /// Smoke del fixture oráculo FTA Helidepot (2 pads + 1 Tricario A↔B).
    #[test]
    fn helidepot_fta_cycle_fixture_shape() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/helidepot_fta_cycle_15_3.sav");
        let raw = std::fs::read(&path).expect("fixture helidepot_fta_cycle_15_3.sav");
        let (data, version) = container::decompress(&raw).expect("decompress");
        let chunks = parse_chunks(&data).expect("chunks");
        let map_w = 64u32;

        let stnn = find_chunk(&chunks, "STNN").expect("STNN");
        let mut helidepots = 0usize;
        for (_, record) in table_rows(stnn, version) {
            let Some(normal) = nested_struct(&record, "normal") else {
                continue;
            };
            let atype = record_get(normal, "airport.type").and_then(SlValue::as_u64);
            let aw = record_get(normal, "airport.w").and_then(SlValue::as_u64);
            let ah = record_get(normal, "airport.h").and_then(SlValue::as_u64);
            // OpenTTD `AT_HELIDEPOT = 6`.
            if atype == Some(6) && aw == Some(2) && ah == Some(2) {
                helidepots += 1;
            }
        }
        assert_eq!(helidepots, 2);

        let vehs = find_chunk(&chunks, "VEHS").expect("VEHS");
        let order_import = SavOrderImport::from_chunks(&chunks, version);
        let mut primary = 0usize;
        for (_, record) in table_rows(vehs, version) {
            if record_get(&record, "type").and_then(SlValue::as_u64) != Some(3) {
                continue;
            }
            let Some(sub) = nested_struct(&record, "aircraft") else {
                continue;
            };
            let Some(common) = nested_struct(sub, "common") else {
                continue;
            };
            let orders_ref = record_get(common, "orders").and_then(SlValue::as_u64);
            let orders = orders_ref
                .map(|r| order_import.orders_for_vehicle_ref(r))
                .unwrap_or_default();
            if orders.len() == 2 {
                primary += 1;
                let fta_pos = record_get(sub, "pos").and_then(SlValue::as_u64);
                let prev = record_get(sub, "previous_pos").and_then(SlValue::as_u64);
                assert_eq!(fta_pos, Some(11), "heli en raise takeoff Helidepot");
                assert_eq!(prev, Some(17));
                let _ = map_w;
            }
        }
        assert_eq!(primary, 1, "un helicóptero con órdenes A↔B");
    }

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
        let stations = stations_from_chunks(&chunks, 64, 300);
        assert_eq!(stations.len(), 1);
        assert_eq!(stations[0].pos, TileCoord::new(5, 2));
        assert_eq!(stations[0].name.as_deref(), Some("Mi Estación"));
        assert_eq!(stations[0].facilities, 1);
    }

    #[test]
    fn decodes_towns_with_fallback_name() {
        let mut t1 = Vec::new();
        t1.extend_from_slice(&(3u32 * 64 + 3).to_be_bytes());
        write_str("Rosario", &mut t1);

        let mut t2 = Vec::new();
        t2.extend_from_slice(&(7u32 * 64 + 1).to_be_bytes());
        write_str("", &mut t2);

        let chunk = RawChunk {
            name: *b"CITY",
            ch_type: CH_TABLE,
            body: build_table_body(&[(6, "xy"), (0x0A | 0x10, "name")], &[t1, t2]),
        };
        let towns = towns_from_chunks(&[chunk], 64, 300);
        assert_eq!(towns.len(), 2);
        assert_eq!(towns[0].name, "Rosario");
        assert_eq!(towns[0].population, 0, "la población se reconstruye aparte");
        assert_eq!(towns[0].pos, TileCoord::new(3, 3));
        assert_eq!(towns[1].name, "Ciudad 2");
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
        let towns = towns_from_chunks(&[chunk], 64, 300);
        assert_eq!(towns.len(), 2);
        assert_eq!(towns[0].name, "Invenville");
        assert_eq!(towns[1].name, "Ciudad 2");
    }

    #[test]
    fn missing_chunks_yield_empty() {
        assert!(stations_from_chunks(&[], 64, 300).is_empty());
        assert!(towns_from_chunks(&[], 64, 300).is_empty());
        assert!(industries_from_chunks(&[], 64, 300).is_empty());
        assert!(
            vehicles_from_chunks(
                &[],
                64,
                &super::super::orders::SavOrderImport::from_chunks(&[], 300),
                300,
            )
            .is_empty()
        );
        assert!(company_money_from_chunks(&[], 300).is_none());
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
        let industries = industries_from_chunks(&[chunk], 64, 300);
        assert_eq!(industries.len(), 1);
        assert_eq!(industries[0].pos, TileCoord::new(10, 5));
        assert_eq!((industries[0].width, industries[0].height), (2, 3));
        assert_eq!(industries[0].industry_type, 7);
    }

    #[test]
    fn reads_colour_from_first_company() {
        let mut c0 = Vec::new();
        c0.extend_from_slice(&500_000i64.to_be_bytes());
        c0.push(6);
        let chunk = RawChunk {
            name: *b"PLYR",
            ch_type: CH_TABLE,
            body: build_table_body(&[(7, "money"), (2, "colour")], &[c0]),
        };
        assert_eq!(company_colour_from_chunks(&[chunk], 300), Some(6));
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
        assert_eq!(company_money_from_chunks(&[chunk], 300), Some(500_000));
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
    fn decodes_front_vehicles_and_train_wagons() {
        let vehicles = vehicles_from_chunks(
            &[vehs_chunk()],
            64,
            &super::super::orders::SavOrderImport::from_chunks(&[], 300),
            300,
        );
        assert_eq!(vehicles.len(), 3);
        assert_eq!(vehicles[0].kind, SavVehicleKind::Train);
        assert!(!vehicles[0].is_wagon);
        assert_eq!(vehicles[0].pos, TileCoord::new(3, 1));
        assert_eq!(vehicles[0].cargo_type, 9);
        assert_eq!(vehicles[1].kind, SavVehicleKind::Train);
        assert!(vehicles[1].is_wagon);
        assert_eq!(vehicles[1].pos, TileCoord::new(4, 1));
        assert_eq!(vehicles[2].kind, SavVehicleKind::RoadVehicle);
        assert!(!vehicles[2].is_wagon);
        assert_eq!(vehicles[2].pos, TileCoord::new(7, 2));
        assert_eq!(vehicles[2].cargo_type, 0);
    }
}

#[cfg(test)]
mod generated_station_name_tests {
    use super::*;
    use crate::GameState;
    use crate::sav::{chunks, container};

    #[test]
    fn formats_valley_and_north_spanish_templates() {
        assert_eq!(
            format_generated_station_name(0x600E, "Sarnpool Bridge").as_deref(),
            Some("Valle de Sarnpool Bridge")
        );
        assert_eq!(
            format_generated_station_name(0x6007, "Sarnpool Bridge").as_deref(),
            Some("Sarnpool Bridge Norte")
        );
    }

    #[test]
    fn dual_fixture_imports_openttd_generated_station_names()
    -> Result<(), Box<dyn std::error::Error>> {
        let raw = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/train_dual_pbs_curve_15_3.sav"
        ))?;
        let (data, version) = container::decompress(&raw)?;
        let chunk_list = chunks::parse_chunks(&data)?;
        let map_w = 64;
        let stations = stations_from_chunks(&chunk_list, map_w, version);
        let towns = towns_from_chunks(&chunk_list, map_w, version);
        let names: Vec<_> = stations
            .iter()
            .filter_map(|s| resolve_sav_station_name(s, &towns))
            .collect();
        assert!(
            names.iter().any(|n| n == "Valle de Sarnpool Bridge"),
            "nombres={names:?}"
        );
        assert!(
            names.iter().any(|n| n == "Sarnpool Bridge Norte"),
            "nombres={names:?}"
        );

        let state = GameState::from_sav_game(crate::sav::load(&raw)?);
        let state_names: Vec<_> = state
            .stations
            .iter()
            .filter_map(|s| s.name.clone())
            .collect();
        assert!(state_names.iter().any(|n| n == "Valle de Sarnpool Bridge"));
        assert!(state_names.iter().any(|n| n == "Sarnpool Bridge Norte"));
        Ok(())
    }
}

#[cfg(test)]
mod oil_refinery_colour_tests {
    use super::*;
    use crate::sav::{chunks, container};

    #[test]
    fn dual_fixture_oil_refinery_imports_grey_random_colour()
    -> Result<(), Box<dyn std::error::Error>> {
        let raw = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/train_dual_pbs_curve_15_3.sav"
        ))?;
        let (data, version) = container::decompress(&raw)?;
        let chunk_list = chunks::parse_chunks(&data)?;
        let industries = industries_from_chunks(&chunk_list, 64, version);
        let refinery = industries
            .iter()
            .find(|i| i.industry_type == 4)
            .ok_or("oil refinery type 4")?;
        assert_eq!(refinery.random_colour, 14, "OpenTTD Grey");
        Ok(())
    }
}
