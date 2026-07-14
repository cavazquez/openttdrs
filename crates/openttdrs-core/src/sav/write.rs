//! Export mínimo de [`GameState`] a savegame `OpenTTD` (`.sav`).
//!
//! Contenedor por defecto: `OTTZ` (zlib). Versión de save: [`EXPORT_SAVE_VERSION`].
//! Chunks: `MAPS` + planos + `STNN` + `CITY` + `INDY` + `ORDL` + `VEHS` + `DATE` + `PLYR`.
//!
//! Limitaciones: ver `docs/ROADMAP_SAV_EXPORT.md`.

#![allow(clippy::cast_possible_truncation)]

use std::io::Write;
use std::path::Path;

use flate2::Compression;
use flate2::write::ZlibEncoder;

use crate::CargoType;
use crate::game_state::GameState;
use crate::industry::{Industry, IndustryKind, IndustrySpec};
use crate::map::{Map, Tile, TileCoord, TileKind};
use crate::news::{CALENDAR_BASE_YEAR, calendar_day_index, calendar_year_day};
use crate::station::StopKind;
use crate::vehicle::{Vehicle, VehicleKind, VehicleOrder};

use super::SavError;
use super::chunks::{CH_RIFF, CH_SPARSE_TABLE, CH_TABLE};

/// Versión SLV del export (≥ 348: `HouseID` en MAP8; ≥ 300: tick u64).
pub const EXPORT_SAVE_VERSION: u16 = 350;

/// Bits `FACIL_*` al escribir `STNN` (alineados con el import).
const FACIL_TRAIN: u8 = 0x01;
const FACIL_TRUCK_STOP: u8 = 0x02;
const FACIL_BUS_STOP: u8 = 0x04;
const FACIL_AIRPORT: u8 = 0x08;
const FACIL_DOCK: u8 = 0x10;
const FACIL_WAYPOINT: u8 = 0x80;

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

/// Contenedor exterior del `.sav`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SavContainer {
    /// Sin compresión (`OTTN`). Útil en tests y fixtures.
    Ottn,
    /// zlib (`OTTZ`). Formato habitual de `OpenTTD` moderno.
    #[default]
    Ottz,
}

/// Escribe `state` como `.sav` en `path`.
///
/// # Errors
///
/// Falla si no se puede serializar el mapa o escribir el archivo.
pub fn save(state: &GameState, path: &Path) -> Result<(), SavError> {
    save_with(state, path, SavContainer::Ottz)
}

/// Como [`save`], con contenedor explícito.
pub fn save_with(state: &GameState, path: &Path, container: SavContainer) -> Result<(), SavError> {
    let bytes = save_to_bytes_with(state, container)?;
    std::fs::write(path, bytes).map_err(|e| SavError::Io(e.to_string()))
}

/// Serializa a bytes (`OTTZ` por defecto).
pub fn save_to_bytes(state: &GameState) -> Result<Vec<u8>, SavError> {
    save_to_bytes_with(state, SavContainer::Ottz)
}

/// Serializa a bytes con contenedor explícito.
pub fn save_to_bytes_with(state: &GameState, container: SavContainer) -> Result<Vec<u8>, SavError> {
    let payload = build_chunk_stream(state)?;
    wrap_container(&payload, EXPORT_SAVE_VERSION, container)
}

/// Chunks siempre presentes en un export mínimo (mapa vacío + DATE + PLYR).
pub const REQUIRED_EXPORT_CHUNKS: &[&str] = &[
    "MAPS", "MAPT", "MAPH", "MAPO", "MAP2", "M3LO", "M3HI", "MAP5", "MAPE", "MAP7", "MAP8", "DATE",
    "PLYR",
];

/// Nombres de chunks RIFF/TABLE en el stream exportado (orden de aparición).
///
/// # Errors
///
/// Fallo al construir el stream (mapa vacío, etc.).
pub fn exported_chunk_names(state: &GameState) -> Result<Vec<String>, SavError> {
    let payload = build_chunk_stream(state)?;
    Ok(scan_chunk_names(&payload))
}

fn scan_chunk_names(payload: &[u8]) -> Vec<String> {
    let mut names = Vec::new();
    let mut i = 0usize;
    while i + 4 <= payload.len() {
        if payload[i..i + 4] == [0, 0, 0, 0] {
            break;
        }
        let name = String::from_utf8_lossy(&payload[i..i + 4]).into_owned();
        if name.len() == 4 && name.bytes().all(|b| (32..127).contains(&b)) {
            names.push(name);
        }
        // Saltar cabecera chunk: 4 (id) + 1 (type/size hi) + 3 (size) = 8, luego payload.
        // El id ya está en i..i+4; el byte de tipo está en i+4.
        if i + 8 > payload.len() {
            break;
        }
        let m = payload[i + 4];
        let size = (u32::from(m & 0xF0) << 20)
            | (u32::from(payload[i + 5]) << 16)
            | (u32::from(payload[i + 6]) << 8)
            | u32::from(payload[i + 7]);
        let chunk_type = m & 0x0F;
        i += 8;
        // CH_TABLE/SPARSE tienen tamaño 0 en el header y payload con gamma — no
        // podemos saltar de forma fiable aquí; para validación basta detectar
        // fourcc conocidos en secuencia con búsqueda lineal.
        if chunk_type == 0 {
            // CH_RIFF: size es el payload.
            i = i.saturating_add(size as usize);
        } else {
            // Para tablas: re-escanear desde aquí buscando el siguiente fourcc
            // ASCII de 4 letras conocido / alfanumérico.
            break;
        }
    }
    // Tras CH_TABLE el tamaño del header no basta: completar con búsqueda de fourcc.
    for &want in REQUIRED_EXPORT_CHUNKS
        .iter()
        .chain(["STNN", "CITY", "INDY", "ORDL", "VEHS"].iter())
    {
        if names.iter().any(|n| n == want) {
            continue;
        }
        if payload.windows(4).any(|w| w == want.as_bytes()) {
            names.push(want.to_string());
        }
    }
    names
}

fn wrap_container(
    payload: &[u8],
    version: u16,
    container: SavContainer,
) -> Result<Vec<u8>, SavError> {
    let mut out = Vec::with_capacity(8 + payload.len());
    match container {
        SavContainer::Ottn => {
            out.extend_from_slice(b"OTTN");
            out.extend_from_slice(&version.to_be_bytes());
            out.extend_from_slice(&[0, 0]);
            out.extend_from_slice(payload);
        }
        SavContainer::Ottz => {
            out.extend_from_slice(b"OTTZ");
            out.extend_from_slice(&version.to_be_bytes());
            out.extend_from_slice(&[0, 0]);
            let mut enc = ZlibEncoder::new(Vec::new(), Compression::default());
            enc.write_all(payload)
                .map_err(|e| SavError::Io(format!("zlib write: {e}")))?;
            let compressed = enc
                .finish()
                .map_err(|e| SavError::Io(format!("zlib finish: {e}")))?;
            out.extend_from_slice(&compressed);
        }
    }
    Ok(out)
}

fn build_chunk_stream(state: &GameState) -> Result<Vec<u8>, SavError> {
    let (w, h) = state.map.dimensions();
    if w == 0 || h == 0 {
        return Err(SavError::BadFormat("mapa vacío".into()));
    }
    let n = (w as usize)
        .checked_mul(h as usize)
        .ok_or_else(|| SavError::BadFormat("dimensiones de mapa overflow".into()))?;

    let planes = collect_planes(&state.map, w, h, n);

    let mut data = Vec::new();
    // MAPS RIFF: dims big-endian (como gen_demo_sav.py / saves clásicos).
    data.extend_from_slice(&riff_chunk(*b"MAPS", &{
        let mut dims = [0u8; 8];
        dims[0..4].copy_from_slice(&w.to_be_bytes());
        dims[4..8].copy_from_slice(&h.to_be_bytes());
        dims
    }));
    data.extend_from_slice(&riff_chunk(*b"MAPT", &planes.mapt));
    data.extend_from_slice(&riff_chunk(*b"MAPH", &planes.maph));
    data.extend_from_slice(&riff_chunk(*b"MAPO", &planes.mapo));
    data.extend_from_slice(&riff_chunk(*b"MAP2", &planes.map2));
    data.extend_from_slice(&riff_chunk(*b"M3LO", &planes.m3lo));
    data.extend_from_slice(&riff_chunk(*b"M3HI", &planes.m3hi));
    data.extend_from_slice(&riff_chunk(*b"MAP5", &planes.map5));
    data.extend_from_slice(&riff_chunk(*b"MAPE", &planes.mape));
    data.extend_from_slice(&riff_chunk(*b"MAP7", &planes.map7));
    data.extend_from_slice(&riff_chunk(*b"MAP8", &planes.map8));

    let stnn = stnn_records(state, w);
    if !stnn.is_empty() {
        data.extend_from_slice(&table_chunk(
            *b"STNN",
            &[(6, "xy"), (0x0A | 0x10, "name"), (2, "facilities")],
            &stnn,
        ));
    }

    let city = city_records(state, w);
    if !city.is_empty() {
        data.extend_from_slice(&table_chunk(
            *b"CITY",
            &[
                (6, "xy"),
                (0x0A | 0x10, "name"),
                (6, "cache.population"),
                (6, "townnamegrfid"),
                (4, "townnametype"),
                (6, "townnameparts"),
            ],
            &city,
        ));
    }

    let indy = indy_records(state, w);
    if !indy.is_empty() {
        data.extend_from_slice(&table_chunk(
            *b"INDY",
            &[
                (6, "location.tile"),
                (2, "location.w"),
                (2, "location.h"),
                (2, "type"),
            ],
            &indy,
        ));
    }

    let (ordl, vehs) = ordl_and_vehs_records(state, w);
    if !ordl.is_empty() {
        data.extend_from_slice(&ordl_chunk(&ordl));
    }
    if !vehs.is_empty() {
        data.extend_from_slice(&vehs_chunk(&vehs));
    }

    data.extend_from_slice(&table_chunk(
        *b"DATE",
        &[(5, "date"), (8, "tick_counter")],
        &[date_record(state)],
    ));
    data.extend_from_slice(&table_chunk(
        *b"PLYR",
        &[(7, "money"), (2, "colour")],
        &[plyr_record(state)],
    ));

    data.extend_from_slice(&[0, 0, 0, 0]);
    Ok(data)
}

struct MapPlanes {
    mapt: Vec<u8>,
    maph: Vec<u8>,
    mapo: Vec<u8>,
    map2: Vec<u8>,
    m3lo: Vec<u8>,
    m3hi: Vec<u8>,
    map5: Vec<u8>,
    mape: Vec<u8>,
    map7: Vec<u8>,
    map8: Vec<u8>,
}

fn collect_planes(map: &Map, w: u32, h: u32, n: usize) -> MapPlanes {
    let mut planes = MapPlanes {
        mapt: vec![0; n],
        maph: vec![0; n],
        mapo: vec![0; n],
        map2: vec![0; n * 2],
        m3lo: vec![0; n],
        m3hi: vec![0; n],
        map5: vec![0; n],
        mape: vec![0; n],
        map7: vec![0; n],
        map8: vec![0; n * 2],
    };
    for y in 0..h {
        for x in 0..w {
            let i = (y * w + x) as usize;
            let Some(tile) = map.get(TileCoord::new(x.cast_signed(), y.cast_signed())) else {
                continue;
            };
            planes.mapt[i] = tile_mapt(tile);
            planes.maph[i] = tile.height;
            planes.mapo[i] = tile.m1;
            // MAP2 en el save: u16 big-endian (byte alto = m2_hi, bajo = m2).
            planes.map2[i * 2] = tile.m2_hi;
            planes.map2[i * 2 + 1] = tile.m2;
            planes.m3lo[i] = tile.m3;
            planes.m3hi[i] = tile.m3hi;
            planes.map5[i] = tile.m5;
            planes.mape[i] = tile.m6;
            planes.map7[i] = tile.m7;
            // MAP8 en el save: u16 big-endian; en memoria `Tile.m8` es LE.
            let m8 = tile.m8.to_be_bytes();
            planes.map8[i * 2] = m8[0];
            planes.map8[i * 2 + 1] = m8[1];
        }
    }
    planes
}

/// Byte MAPT: conserva el del tile si está; si no, deriva del [`TileKind`].
fn tile_mapt(tile: Tile) -> u8 {
    if tile.mapt != 0 {
        return tile.mapt;
    }
    match tile.kind {
        TileKind::Grass | TileKind::CoalField => 0x00,
        TileKind::Rail | TileKind::RailDepot => 0x10,
        TileKind::Road | TileKind::RoadDepot => 0x20,
        TileKind::House => 0x30,
        TileKind::Forest => 0x40,
        TileKind::Station | TileKind::Airport => 0x50,
        TileKind::Water | TileKind::ShipDepot => 0x60,
        TileKind::Void => 0x70,
        TileKind::Industry => 0x80,
        TileKind::RailTunnel
        | TileKind::RoadTunnel
        | TileKind::RailBridge
        | TileKind::RoadBridge => 0x90,
        TileKind::Unknown(t) => (t & 0x0F) << 4,
    }
}

/// Fecha `OpenTTD` aproximada (días desde año 0) + tick monotónico.
fn date_record(state: &GameState) -> Vec<u8> {
    let day_index = calendar_day_index(state.tick);
    let (year, doy) = calendar_year_day(day_index);
    // Aproximación: 365 * year + (doy - 1). Suficiente para roundtrip interno;
    // OpenTTD usa calendario gregoriano real — ver ROADMAP_SAV_EXPORT.
    let calendar_date = i32::try_from(u64::from(year) * 365 + (doy.saturating_sub(1)))
        .unwrap_or(i32::try_from(u64::from(CALENDAR_BASE_YEAR) * 365).unwrap_or(0));
    let mut rec = Vec::with_capacity(12);
    rec.extend_from_slice(&calendar_date.to_be_bytes());
    rec.extend_from_slice(&state.tick.get().to_be_bytes());
    rec
}

fn plyr_record(state: &GameState) -> Vec<u8> {
    let mut rec = Vec::with_capacity(9);
    rec.extend_from_slice(&state.economy.money.to_be_bytes());
    rec.push(state.company_colour);
    rec
}

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

fn stnn_records(state: &GameState, map_w: u32) -> Vec<Vec<u8>> {
    let mut out = Vec::with_capacity(state.stations.len());
    for st in &state.stations {
        if st.pos.x < 0 || st.pos.y < 0 {
            continue;
        }
        let ux = st.pos.x.cast_unsigned();
        let uy = st.pos.y.cast_unsigned();
        let tile_idx = uy.saturating_mul(map_w).saturating_add(ux);
        let mut rec = Vec::new();
        rec.extend_from_slice(&tile_idx.to_be_bytes());
        let name = st.name.as_deref().unwrap_or("");
        write_str(name, &mut rec);
        rec.push(facilities_for_stop(st.stop_kind));
        out.push(rec);
    }
    out
}

fn tile_index(pos: TileCoord, map_w: u32) -> Option<u32> {
    if pos.x < 0 || pos.y < 0 {
        return None;
    }
    Some(
        pos.y
            .cast_unsigned()
            .saturating_mul(map_w)
            .saturating_add(pos.x.cast_unsigned()),
    )
}

fn city_records(state: &GameState, map_w: u32) -> Vec<Vec<u8>> {
    let mut out = Vec::with_capacity(state.towns.len());
    for town in &state.towns {
        let Some(tile_idx) = tile_index(town.pos, map_w) else {
            continue;
        };
        let mut rec = Vec::new();
        rec.extend_from_slice(&tile_idx.to_be_bytes());
        write_str(&town.name, &mut rec);
        // cache.population: el import la pone en 0 y rebuild_town_populations la recalcula;
        // igual la escribimos para roundtrip de lectura best-effort / fixtures.
        rec.extend_from_slice(&town.population.to_be_bytes());
        rec.extend_from_slice(&0u32.to_be_bytes()); // townnamegrfid
        rec.extend_from_slice(&0x20C0u16.to_be_bytes()); // townnametype (inglés)
        rec.extend_from_slice(&0u32.to_be_bytes()); // townnameparts
        out.push(rec);
    }
    out
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

fn indy_records(state: &GameState, map_w: u32) -> Vec<Vec<u8>> {
    let mut out = Vec::with_capacity(state.industries.len());
    for ind in &state.industries {
        let Some(tile_idx) = tile_index(ind.pos, map_w) else {
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
            let id = u16::try_from(tile_index(depot, map_w)?).ok()?;
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

/// Una lista ORDL por vehículo (solo órdenes goto estación/waypoint).
fn ordl_and_vehs_records(state: &GameState, map_w: u32) -> (Vec<Vec<u8>>, Vec<Vec<u8>>) {
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
        let Some(tile_idx) = tile_index(v.pos, map_w) else {
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
            write_gamma(order_bytes.len() as u32, &mut rec); // count of orders struct
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
        write_gamma(sparse_idx, &mut rec);
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
    (ordl, vehs)
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

fn ordl_chunk(records: &[Vec<u8>]) -> Vec<u8> {
    // Header con struct anidado `orders` (como gen_demo_sav.py).
    let mut header = Vec::new();
    header.push(0x1B); // STRUCT | HAS_LENGTH
    write_str("orders", &mut header);
    header.push(0); // fin lista top-level → subcampos de orders
    header.push(2);
    write_str("type", &mut header);
    header.push(2);
    write_str("flags", &mut header);
    header.push(4);
    write_str("dest", &mut header);
    header.push(2);
    write_str("refit_cargo", &mut header);
    header.push(4);
    write_str("wait_time", &mut header);
    header.push(4);
    write_str("travel_time", &mut header);
    header.push(4);
    write_str("max_speed", &mut header);
    header.push(0);
    raw_table_chunk(*b"ORDL", &header, records, CH_TABLE)
}

fn vehs_chunk(records: &[Vec<u8>]) -> Vec<u8> {
    let mut header = Vec::new();
    header.push(2);
    write_str("type", &mut header);
    header.push(0x1B); // STRUCT | HAS_LENGTH
    write_str("train", &mut header);
    header.push(0x1B);
    write_str("roadveh", &mut header);
    header.push(0);
    for _ in 0..2 {
        header.push(0x1B);
        write_str("common", &mut header);
        header.push(0);
        header.push(6);
        write_str("tile", &mut header);
        header.push(2);
        write_str("subtype", &mut header);
        header.push(2);
        write_str("cargo_type", &mut header);
        header.push(6);
        write_str("orders", &mut header);
        header.push(2);
        write_str("cur_real_order_index", &mut header);
        header.push(2);
        write_str("vehstatus", &mut header);
        header.push(0);
    }
    raw_table_chunk(*b"VEHS", &header, records, CH_SPARSE_TABLE)
}

fn raw_table_chunk(name: [u8; 4], header: &[u8], records: &[Vec<u8>], ch_type: u8) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&name);
    out.push(ch_type);
    write_gamma(header.len() as u32 + 1, &mut out);
    out.extend_from_slice(header);
    for rec in records {
        write_gamma(rec.len() as u32 + 1, &mut out);
        out.extend_from_slice(rec);
    }
    write_gamma(0, &mut out);
    out
}

fn write_gamma(v: u32, buf: &mut Vec<u8>) {
    assert!(v < (1 << 14), "export usa gammas < 2^14");
    if v < (1 << 7) {
        buf.push(v as u8);
    } else {
        buf.push(0x80 | ((v >> 8) as u8));
        buf.push((v & 0xFF) as u8);
    }
}

fn write_str(s: &str, buf: &mut Vec<u8>) {
    write_gamma(s.len() as u32, buf);
    buf.extend_from_slice(s.as_bytes());
}

fn riff_chunk(name: [u8; 4], payload: &[u8]) -> Vec<u8> {
    let size = payload.len();
    let mut out = Vec::with_capacity(8 + size);
    out.extend_from_slice(&name);
    out.push((((size >> 24) as u8) << 4) | CH_RIFF);
    out.push((size >> 16) as u8);
    out.push((size >> 8) as u8);
    out.push(size as u8);
    out.extend_from_slice(payload);
    out
}

fn table_chunk(name: [u8; 4], fields: &[(u8, &str)], records: &[Vec<u8>]) -> Vec<u8> {
    let mut header = Vec::new();
    for &(ftype, key) in fields {
        header.push(ftype);
        write_str(key, &mut header);
    }
    header.push(0);

    let mut out = Vec::new();
    out.extend_from_slice(&name);
    out.push(CH_TABLE);
    write_gamma(header.len() as u32 + 1, &mut out);
    out.extend_from_slice(&header);
    for rec in records {
        write_gamma(rec.len() as u32 + 1, &mut out);
        out.extend_from_slice(rec);
    }
    write_gamma(0, &mut out);
    out
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::cast_possible_truncation
)]
mod tests {
    use super::*;
    use crate::map::TileKind;
    use crate::sav;
    use crate::station::{Station, StopKind};
    use crate::tick::GameTick;
    use crate::town::Town;
    use crate::vehicle::{Vehicle, VehicleKind};

    fn tiny_state() -> GameState {
        let mut state = GameState::new(64, 64);
        state.economy.money = 777_000;
        state.company_colour = 3;
        state.tick = GameTick::new(12_345);
        let c = TileCoord::new(10, 20);
        let mut tile = state.map.get(c).expect("in bounds");
        tile.kind = TileKind::Rail;
        tile.mapt = 0x10;
        tile.m5 = 0x01; // TRACK_X
        tile.m2 = 0xAB;
        tile.m2_hi = 0xCD;
        tile.m3 = 0x11;
        tile.m3hi = 0x22;
        tile.m8 = 0x1234;
        tile.height = 2;
        state.map.set_tile(c, tile).expect("set");
        state
    }

    #[test]
    fn ottn_roundtrip_preserves_stations_stnn() {
        let mut state = tiny_state();
        let mut rail = Station::new_with_kind(TileCoord::new(28, 39), StopKind::RailStation);
        rail.name = Some("Central Demo".into());
        let mut bus = Station::new_with_kind(TileCoord::new(17, 15), StopKind::BusStop);
        bus.name = Some("Parada Villa".into());
        state.stations = vec![rail, bus];

        let bytes = save_to_bytes_with(&state, SavContainer::Ottn).expect("save");
        let sav_game = sav::load(&bytes).expect("load");
        assert_eq!(sav_game.stations.len(), 2);
        let names: Vec<_> = sav_game
            .stations
            .iter()
            .filter_map(|s| s.name.as_deref())
            .collect();
        assert!(names.contains(&"Central Demo"));
        assert!(names.contains(&"Parada Villa"));
        let central = sav_game
            .stations
            .iter()
            .find(|s| s.name.as_deref() == Some("Central Demo"))
            .expect("central");
        assert_eq!(central.pos, TileCoord::new(28, 39));
        assert_eq!(central.facilities & FACIL_TRAIN, FACIL_TRAIN);

        let loaded = GameState::from_sav_game(sav_game);
        assert_eq!(loaded.stations.len(), 2);
        assert!(
            loaded
                .stations
                .iter()
                .any(|s| s.name.as_deref() == Some("Central Demo")
                    && s.stop_kind == StopKind::RailStation)
        );
    }

    #[test]
    fn ottn_roundtrip_preserves_city_and_indy() {
        use crate::industry::{Industry, IndustryKind, IndustrySpec};
        use crate::town::Town;

        let mut state = tiny_state();
        state.towns = vec![Town {
            id: 0,
            pos: TileCoord::new(16, 16),
            name: "Villa Demo".into(),
            population: 1200,
            local_authority_rating: 0,
            passengers_served: 0,
            mail_served: 0,
            growth_funded: 0,
            ..Default::default()
        }];
        state.industries = vec![Industry::with_tiles_spec(
            TileCoord::new(36, 20),
            IndustryKind::CoalMine,
            IndustrySpec::CoalMine,
            vec![
                TileCoord::new(36, 20),
                TileCoord::new(37, 20),
                TileCoord::new(36, 21),
                TileCoord::new(37, 21),
            ],
            0,
        )];

        let bytes = save_to_bytes_with(&state, SavContainer::Ottn).expect("save");
        let sav_game = sav::load(&bytes).expect("load");
        assert_eq!(sav_game.towns.len(), 1);
        assert_eq!(sav_game.towns[0].name, "Villa Demo");
        assert_eq!(sav_game.towns[0].pos, TileCoord::new(16, 16));
        assert_eq!(sav_game.industries.len(), 1);
        assert_eq!(sav_game.industries[0].pos, TileCoord::new(36, 20));
        assert_eq!(sav_game.industries[0].width, 2);
        assert_eq!(sav_game.industries[0].height, 2);
        assert_eq!(sav_game.industries[0].industry_type, 0);
    }

    #[test]
    fn ottn_roundtrip_preserves_vehicles_and_orders() {
        use crate::vehicle::{Vehicle, VehicleKind, VehicleOrder};

        let mut state = tiny_state();
        let mut rail = Station::new_with_kind(TileCoord::new(28, 39), StopKind::RailStation);
        rail.name = Some("Central".into());
        let mut bus_stop = Station::new_with_kind(TileCoord::new(17, 15), StopKind::BusStop);
        bus_stop.name = Some("Parada".into());
        state.stations = vec![rail, bus_stop];

        let mut train = Vehicle::new(
            0,
            VehicleKind::Train,
            TileCoord::new(20, 40),
            TileCoord::new(20, 40),
        );
        train.running = true;
        train.set_vehicle_orders(vec![VehicleOrder::Station {
            station: TileCoord::new(28, 39),
            full_load: false,
            no_unload: false,
            wait_ticks: 0,
            travel_ticks: 0,
        }]);
        let mut bus = Vehicle::new(
            1,
            VehicleKind::Bus,
            TileCoord::new(13, 16),
            TileCoord::new(13, 16),
        );
        bus.running = true;
        bus.set_vehicle_orders(vec![VehicleOrder::Station {
            station: TileCoord::new(17, 15),
            full_load: false,
            no_unload: false,
            wait_ticks: 0,
            travel_ticks: 0,
        }]);
        state.vehicles = vec![train, bus];

        let bytes = save_to_bytes_with(&state, SavContainer::Ottn).expect("save");
        let sav_game = sav::load(&bytes).expect("load");
        assert_eq!(sav_game.vehicles.len(), 2);
        assert!(
            sav_game
                .vehicles
                .iter()
                .any(|v| v.kind == sav::SavVehicleKind::Train && !v.orders.is_empty())
        );
        assert!(
            sav_game
                .vehicles
                .iter()
                .any(|v| v.kind == sav::SavVehicleKind::RoadVehicle && !v.orders.is_empty())
        );

        let loaded = GameState::from_sav_game(sav_game);
        assert!(loaded.vehicles.len() >= 2);
        assert!(
            loaded
                .vehicles
                .iter()
                .any(|v| v.kind == VehicleKind::Train && !v.orders.is_empty())
        );
        assert!(
            loaded
                .vehicles
                .iter()
                .any(|v| v.kind == VehicleKind::Bus && !v.orders.is_empty())
        );
    }

    #[test]
    fn ottn_roundtrip_preserves_map_money_tick_colour() {
        let state = tiny_state();
        let bytes = save_to_bytes_with(&state, SavContainer::Ottn).expect("save");
        assert!(bytes.starts_with(b"OTTN"));
        let sav_game = sav::load(&bytes).expect("load");
        assert_eq!(sav_game.version, EXPORT_SAVE_VERSION);
        assert_eq!(sav_game.money, Some(777_000));
        assert_eq!(sav_game.company_colour, Some(3));
        assert_eq!(sav_game.game_time.map(|t| t.tick), Some(12_345));
        let tile = sav_game.map.get(TileCoord::new(10, 20)).expect("tile");
        assert_eq!(tile.kind, TileKind::Rail);
        assert_eq!(tile.mapt, 0x10);
        assert_eq!(tile.m5, 0x01);
        assert_eq!(tile.height, 2);
        assert_eq!(tile.m2, 0xAB);
        assert_eq!(tile.m2_hi, 0xCD);
        assert_eq!(tile.m3, 0x11);
        assert_eq!(tile.m3hi, 0x22);
        assert_eq!(tile.m8, 0x1234);
    }

    #[test]
    fn ottz_roundtrip_loads() {
        let state = tiny_state();
        let bytes = save_to_bytes(&state).expect("save ottz");
        assert!(bytes.starts_with(b"OTTZ"));
        let sav_game = sav::load(&bytes).expect("load ottz");
        assert_eq!(sav_game.money, Some(777_000));
        assert_eq!(sav_game.map.dimensions(), (64, 64));
    }

    #[test]
    fn derives_mapt_from_kind_when_zero() {
        let mut state = GameState::new(64, 64);
        let c = TileCoord::new(5, 5);
        let mut tile = state.map.get(c).expect("in bounds");
        tile.kind = TileKind::Road;
        tile.mapt = 0;
        tile.m5 = 0x0F;
        state.map.set_tile(c, tile).expect("set");
        let bytes = save_to_bytes_with(&state, SavContainer::Ottn).expect("save");
        let sav_game = sav::load(&bytes).expect("load");
        let tile = sav_game.map.get(c).expect("tile");
        assert_eq!(tile.mapt, 0x20);
        assert_eq!(tile.kind, TileKind::Road);
    }

    #[test]
    fn from_sav_game_roundtrip_via_export() {
        let state = tiny_state();
        let bytes = save_to_bytes_with(&state, SavContainer::Ottn).expect("save");
        let loaded = GameState::from_sav_game(sav::load(&bytes).expect("load"));
        assert_eq!(loaded.economy.money, 777_000);
        assert_eq!(loaded.company_colour, 3);
        assert_eq!(loaded.tick.get(), 12_345);
        let tile = loaded.map.get(TileCoord::new(10, 20)).expect("tile");
        assert_eq!(tile.kind, TileKind::Rail);
        assert_eq!(tile.m5, 0x01);
    }

    #[test]
    fn export_includes_required_chunks_for_openttd_validation() {
        let names = exported_chunk_names(&tiny_state()).expect("chunks");
        for req in REQUIRED_EXPORT_CHUNKS {
            assert!(
                names.iter().any(|n| n == *req),
                "falta chunk obligatorio {req} en {names:?}"
            );
        }

        // Escenario con entidades: opcionales presentes (#66).
        let mut state = tiny_state();
        let mut rail = Station::new_with_kind(TileCoord::new(28, 39), StopKind::RailStation);
        rail.name = Some("Central".into());
        state.stations = vec![rail];
        state.towns = vec![Town {
            id: 0,
            pos: TileCoord::new(16, 16),
            name: "Villa".into(),
            population: 500,
            ..Default::default()
        }];
        let pos = TileCoord::new(10, 20);
        state.vehicles = vec![Vehicle::new(0, VehicleKind::Train, pos, pos)];
        let names = exported_chunk_names(&state).expect("chunks");
        assert!(names.iter().any(|n| n == "STNN"), "{names:?}");
        assert!(names.iter().any(|n| n == "CITY"), "{names:?}");
        assert!(names.iter().any(|n| n == "VEHS"), "{names:?}");
    }
}
