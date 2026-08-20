//! Export mínimo de [`GameState`] a savegame `OpenTTD` (`.sav`).
//!
//! Contenedor por defecto: `OTTZ` (zlib). Versión de save: [`EXPORT_SAVE_VERSION`].
//! Chunks: `MAPS` (`CH_TABLE`) + planos RIFF + `STNN`/`CITY`/`INDY`/`ORDL`/`VEHS`/`LGRP` + `DATE` + `PLYR`.
//!
//! Subconjunto prometido (MVP #226/#267): mapa + `CITY` (≥1) + `STNN` moderno
//! (SAVEBYTE + structs) + `VEHS`/`ORDL` (tren + ROAD + ship + aircraft ala fija)
//! + `INDY` + `DATE`/`PLYR` cargable por `OpenTTD` ≥15.3 dedicated.
//!
//! Residual: tram, rotor heli, `CAPY`/`ECMY` packets, settings fuera de los
//! tres campos de `PATS`, `GSET`/`ENGN`/`SRND`/`NewGRF`/`PLYR` completo.
//! Limitaciones: `docs/PARIDAD.md` y `docs/archive/merged-2026-07/ROADMAP_SAV_EXPORT.md`.

#![allow(clippy::cast_possible_truncation)]

mod chunks;
pub(crate) mod codec;
mod entities;
mod map;
mod meta;
mod vehicles;

use std::io::Write;
use std::path::Path;

use flate2::Compression;
use flate2::write::ZlibEncoder;

use super::SavError;
use crate::game_state::GameState;

/// Versión SLV del export.
///
/// Se mantiene en **350** (mínimo viable): ≥294 `MAPS` `CH_TABLE`, ≥295 tablas,
/// ≥300 tick u64, ≥348 `HouseID` en MAP8. `OpenTTD` 15.3 (`SAVEGAME_VERSION` 362)
/// carga saves más antiguos; subir a 362 no aporta al MVP de load y obligaría
/// campos DATE/economía posteriores sin ganancia.
pub const EXPORT_SAVE_VERSION: u16 = 350;

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

/// Chunks siempre presentes en un export mínimo (mapa + CITY + DATE + PLYR).
/// `CITY` es obligatorio para `OpenTTD` (`STR_ERROR_NO_TOWN_IN_SCENARIO`).
pub const REQUIRED_EXPORT_CHUNKS: &[&str] = &[
    "MAPS", "MAPT", "MAPH", "MAPO", "MAP2", "M3LO", "M3HI", "MAP5", "MAPE", "MAP7", "MAP8", "CITY",
    "DATE", "PLYR",
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
    for &want in REQUIRED_EXPORT_CHUNKS.iter().chain(
        [
            "STNN", "CITY", "INDY", "ORDL", "VEHS", "LGRP", "LGRJ", "LGRS", "PATS",
        ]
        .iter(),
    ) {
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

    let planes = map::collect_planes(&state.map, w, h, n);

    let mut data = Vec::new();
    // MAPS CH_TABLE (SLV ≥ 294): dim_x/dim_y SLE_FILE_U32 BE — ver map_sl.cpp.
    // Planos MAPT…MAP8 siguen CH_RIFF densos.
    let mut maps_rec = Vec::with_capacity(8);
    maps_rec.extend_from_slice(&w.to_be_bytes());
    maps_rec.extend_from_slice(&h.to_be_bytes());
    data.extend_from_slice(&chunks::table_chunk(
        *b"MAPS",
        &[(6, "dim_x"), (6, "dim_y")],
        &[maps_rec],
    )?);
    data.extend_from_slice(&chunks::riff_chunk(*b"MAPT", &planes.mapt));
    data.extend_from_slice(&chunks::riff_chunk(*b"MAPH", &planes.maph));
    data.extend_from_slice(&chunks::riff_chunk(*b"MAPO", &planes.mapo));
    data.extend_from_slice(&chunks::riff_chunk(*b"MAP2", &planes.map2));
    data.extend_from_slice(&chunks::riff_chunk(*b"M3LO", &planes.m3lo));
    data.extend_from_slice(&chunks::riff_chunk(*b"M3HI", &planes.m3hi));
    data.extend_from_slice(&chunks::riff_chunk(*b"MAP5", &planes.map5));
    data.extend_from_slice(&chunks::riff_chunk(*b"MAPE", &planes.mape));
    data.extend_from_slice(&chunks::riff_chunk(*b"MAP7", &planes.map7));
    data.extend_from_slice(&chunks::riff_chunk(*b"MAP8", &planes.map8));

    let stnn = entities::stnn_records(state, w)?;
    if !stnn.is_empty() {
        data.extend_from_slice(&entities::stnn_chunk(&stnn)?);
    }

    // CITY siempre: OpenTTD rechaza saves sin municipios.
    let city = entities::city_records(state, w)?;
    data.extend_from_slice(&chunks::table_chunk(
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
    )?);

    let indy = entities::indy_records(state, w);
    if !indy.is_empty() {
        data.extend_from_slice(&chunks::table_chunk(
            *b"INDY",
            &[
                (6, "location.tile"),
                (2, "location.w"),
                (2, "location.h"),
                (2, "type"),
            ],
            &indy,
        )?);
    }

    let (ordl, vehs) = vehicles::ordl_and_vehs_records(state, w)?;
    if !ordl.is_empty() {
        data.extend_from_slice(&vehicles::ordl_chunk(&ordl)?);
    }
    if !vehs.is_empty() {
        data.extend_from_slice(&vehicles::vehs_chunk(&vehs)?);
    }

    data.extend_from_slice(&super::linkgraph::encode_linkgraph_chunks(
        &state.link_graph,
        &state.stations,
        w,
    )?);

    data.extend_from_slice(&meta::pats_chunk(state)?);

    data.extend_from_slice(&chunks::table_chunk(
        *b"DATE",
        &[(5, "date"), (8, "tick_counter")],
        &[meta::date_record(state)],
    )?);
    data.extend_from_slice(&chunks::table_chunk(
        *b"PLYR",
        &[(7, "money"), (2, "colour")],
        &[meta::plyr_record(state)],
    )?);

    data.extend_from_slice(&[0, 0, 0, 0]);
    Ok(data)
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::cast_possible_truncation
)]
mod tests {
    use super::*;
    use crate::map::{TileCoord, TileKind};
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
        assert_eq!(central.facilities & 0x01, 0x01); // FACIL_TRAIN

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
    fn ottn_roundtrip_preserves_construction_settings_in_pats() {
        let mut state = tiny_state();
        state.construction.road_vehicle_driving_side = crate::RoadVehicleDrivingSide::Right;
        state.construction.train_signal_side = crate::TrainSignalSide::Right;
        state.construction.freeform_edges = false;

        let bytes = save_to_bytes_with(&state, SavContainer::Ottn).expect("save");
        let sav_game = sav::load(&bytes).expect("load");
        assert_eq!(sav_game.construction, state.construction);
        assert!(
            exported_chunk_names(&state)
                .expect("chunk names")
                .iter()
                .any(|name| name == "PATS")
        );
    }

    #[test]
    fn ottn_roundtrip_preserves_city_and_indy() {
        use crate::industry::{Industry, IndustryKind, IndustrySpec};

        let mut state = tiny_state();
        state.towns = vec![Town {
            id: 0,
            pos: TileCoord::new(16, 16),
            name: "Villa Demo".into(),
            population: 1200,
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
        train.set_vehicle_orders(vec![VehicleOrder::station(TileCoord::new(28, 39))]);
        let bus_pos = TileCoord::new(13, 16);
        let mut road = state.map.get(bus_pos).expect("in bounds");
        road.kind = TileKind::Road;
        road.mapt = 0x20;
        road.m5 = 0x0A;
        state.map.set_tile(bus_pos, road).expect("set");
        let mut bus = Vehicle::new(1, VehicleKind::Bus, bus_pos, bus_pos);
        bus.running = true;
        bus.set_vehicle_orders(vec![VehicleOrder::station(TileCoord::new(17, 15))]);
        state.vehicles = vec![train, bus];

        let bytes = save_to_bytes_with(&state, SavContainer::Ottn).expect("save");
        let sav_game = sav::load(&bytes).expect("load");
        assert_eq!(sav_game.vehicles.len(), 2, "tren + bus en VEHS");
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
        assert_eq!(loaded.vehicles.len(), 2);
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

    /// Estado mínimo con STNN moderno cargable por `OpenTTD` 15.3.
    fn mvp_stations_state() -> GameState {
        let mut state = tiny_state();
        let rail_pos = TileCoord::new(28, 39);
        let mut rail_tile = state.map.get(rail_pos).expect("in bounds");
        rail_tile.kind = TileKind::Station;
        rail_tile.mapt = 0x50; // MP_STATION << 4
        state.map.set_tile(rail_pos, rail_tile).expect("set");

        // Vía bajo/junto a la estación (contexto visual; no requerido por saveload).
        let track = TileCoord::new(28, 40);
        let mut track_tile = state.map.get(track).expect("in bounds");
        track_tile.kind = TileKind::Rail;
        track_tile.mapt = 0x10;
        track_tile.m5 = 0x01;
        state.map.set_tile(track, track_tile).expect("set");

        let mut rail = Station::new_with_kind(rail_pos, StopKind::RailStation);
        rail.name = Some("Central Demo".into());
        state.stations = vec![rail];
        state.towns = vec![Town {
            id: 0,
            pos: TileCoord::new(16, 16),
            name: "Villa Demo".into(),
            population: 1200,
            ..Default::default()
        }];
        state
    }

    #[test]
    fn export_stnn_is_modern_savebyte_schema() {
        use crate::sav::chunks::{CH_TABLE, find_chunk, parse_chunks};
        use crate::sav::table::{SlValue, parse_table_chunk, record_get};

        let state = mvp_stations_state();
        let bytes = save_to_bytes_with(&state, SavContainer::Ottn).expect("save");
        let payload = &bytes[8..];
        let chunks = parse_chunks(payload).expect("chunks");
        let stnn = find_chunk(&chunks, "STNN").expect("STNN");
        assert_eq!(stnn.ch_type, CH_TABLE);
        let rows = parse_table_chunk(&stnn.body, false).expect("STNN table");
        assert_eq!(rows.len(), 1);
        let rec = &rows[0].1;
        // SAVEBYTE facilities en top-level.
        assert_eq!(
            record_get(rec, "facilities").and_then(SlValue::as_u64),
            Some(1)
        );
        let normal = match record_get(rec, "normal") {
            Some(SlValue::Structs(items)) => items.first().expect("normal struct"),
            other => panic!("normal ausente: {other:?}"),
        };
        let base = match record_get(normal, "base") {
            Some(SlValue::Structs(items)) => items.first().expect("base"),
            other => panic!("base ausente: {other:?}"),
        };
        assert_eq!(
            record_get(base, "name").and_then(|v| v.as_str()),
            Some("Central Demo")
        );
        assert_eq!(
            record_get(base, "xy").and_then(SlValue::as_u64),
            Some(u64::from(39u32 * 64 + 28))
        );
        let goods = match record_get(normal, "goods") {
            Some(SlValue::Structs(items)) => items,
            other => panic!("goods ausente: {other:?}"),
        };
        assert_eq!(goods.len(), 64, "NUM_CARGO goods entries");

        // Smoke OpenTTD: OPENTTDRS_DUMP_MVP_STATIONS_SAV=/ruta/absoluta.sav
        if let Ok(path) = std::env::var("OPENTTDRS_DUMP_MVP_STATIONS_SAV") {
            let path = std::path::PathBuf::from(&path);
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            std::fs::write(&path, &bytes).expect("dump stations sav");
        }
    }

    /// Mapa+CITY+STNN+VEHS(tren)+ORDL — fixture OpenTTD-loadable (#226).
    fn mvp_train_state() -> GameState {
        use crate::vehicle::VehicleOrder;

        let mut state = mvp_stations_state();
        let rail_pos = TileCoord::new(28, 39);
        // Vía bajo el tren (TRACK_BIT_X).
        let train_pos = TileCoord::new(20, 40);
        let mut track_tile = state.map.get(train_pos).expect("in bounds");
        track_tile.kind = TileKind::Rail;
        track_tile.mapt = 0x10;
        track_tile.m5 = 0x01;
        state.map.set_tile(train_pos, track_tile).expect("set");

        let mut train = Vehicle::new(0, VehicleKind::Train, train_pos, train_pos);
        train.running = true;
        train.direction = crate::vehicle::DIR_NE;
        train.set_vehicle_orders(vec![VehicleOrder::station(rail_pos)]);
        state.vehicles = vec![train];
        state
    }

    /// Fixture ship (#267): CITY+STNN dock + VEHS ship sobre agua.
    fn mvp_ship_state() -> GameState {
        use crate::map::{WaterClass, make_water_tile};
        use crate::vehicle::VehicleOrder;

        let mut state = mvp_stations_state();
        let dock_pos = TileCoord::new(32, 32);
        let mut dock = Station::new_with_kind(dock_pos, StopKind::Dock);
        dock.name = Some("Muelle Demo".into());
        state.stations.push(dock);

        let ship_pos = TileCoord::new(30, 32);
        make_water_tile(&mut state.map, ship_pos, WaterClass::Sea).expect("sea");
        // Franja de agua adyacente (navegación / AfterLoad).
        for x in 28..36 {
            let c = TileCoord::new(x, 32);
            let _ = make_water_tile(&mut state.map, c, WaterClass::Sea);
        }
        let mut dock_tile = state.map.get(dock_pos).expect("in bounds");
        dock_tile.kind = TileKind::Station;
        dock_tile.mapt = 0x50;
        // ST_DOCK en bits 3–6 de m6 (= 6 << 3).
        dock_tile.m6 = 6 << 3;
        state.map.set_tile(dock_pos, dock_tile).expect("set");

        let mut ship = Vehicle::new(0, VehicleKind::Ship, ship_pos, ship_pos);
        ship.running = false;
        ship.direction = crate::vehicle::DIR_NE;
        ship.set_vehicle_orders(vec![VehicleOrder::station(dock_pos)]);
        state.vehicles = vec![ship];
        state
    }

    /// Fixture rico: estaciones + tren + bus ROAD + industria (`#226`).
    fn mvp_rich_state() -> GameState {
        use crate::industry::{Industry, IndustryKind, IndustrySpec};
        use crate::vehicle::VehicleOrder;

        let mut state = mvp_train_state();
        let bus_stop = TileCoord::new(17, 15);
        let mut bus_st = Station::new_with_kind(bus_stop, StopKind::BusStop);
        bus_st.name = Some("Parada Villa Demo".into());
        state.stations.push(bus_st);

        // Carretera bajo el bus (ROAD_X) — AfterLoad exige roadtype válido.
        let bus_pos = TileCoord::new(13, 16);
        for x in 10..23 {
            let c = TileCoord::new(x, 16);
            let mut t = state.map.get(c).expect("in bounds");
            t.kind = TileKind::Road;
            t.mapt = 0x20;
            t.m5 = 0x0A; // ROAD_X
            t.m3hi = 0; // m4 / ROADTYPE_ROAD
            state.map.set_tile(c, t).expect("set");
        }
        let mut stop_tile = state.map.get(bus_stop).expect("in bounds");
        stop_tile.kind = TileKind::Station;
        stop_tile.mapt = 0x50;
        stop_tile.m6 = 3 << 3; // ST_BUS
        state.map.set_tile(bus_stop, stop_tile).expect("set");

        let mut bus = Vehicle::new(1, VehicleKind::Bus, bus_pos, bus_pos);
        bus.running = true;
        bus.direction = crate::vehicle::DIR_NE;
        bus.set_vehicle_orders(vec![VehicleOrder::station(bus_stop)]);
        state.vehicles.push(bus);

        // Mina de carbón 2×2 + INDY.
        let ind_tiles = [
            TileCoord::new(36, 20),
            TileCoord::new(37, 20),
            TileCoord::new(36, 21),
            TileCoord::new(37, 21),
        ];
        for (i, &c) in ind_tiles.iter().enumerate() {
            let mut t = state.map.get(c).expect("in bounds");
            t.kind = TileKind::Industry;
            t.mapt = 0x80;
            t.m5 = u8::try_from(i).unwrap_or(0);
            state.map.set_tile(c, t).expect("set");
        }
        state.industries = vec![Industry::with_tiles_spec(
            TileCoord::new(36, 20),
            IndustryKind::CoalMine,
            IndustrySpec::CoalMine,
            ind_tiles.to_vec(),
            0,
        )];
        state
    }

    #[test]
    fn export_mvp_train_emits_vehs_ordl_and_direction() {
        use crate::sav::chunks::{find_chunk, parse_chunks};
        use crate::sav::table::{SlValue, parse_table_chunk, record_get};

        let state = mvp_train_state();
        let bytes = save_to_bytes_with(&state, SavContainer::Ottn).expect("save");
        let payload = &bytes[8..];
        let chunks = parse_chunks(payload).expect("chunks");
        assert!(find_chunk(&chunks, "VEHS").is_some());
        assert!(find_chunk(&chunks, "ORDL").is_some());
        assert!(find_chunk(&chunks, "STNN").is_some());

        let vehs = find_chunk(&chunks, "VEHS").expect("VEHS");
        let rows = parse_table_chunk(&vehs.body, true).expect("VEHS table");
        assert_eq!(rows.len(), 1);
        let train = match record_get(&rows[0].1, "train") {
            Some(SlValue::Structs(items)) => items.first().expect("train"),
            other => panic!("train ausente: {other:?}"),
        };
        let common = match record_get(train, "common") {
            Some(SlValue::Structs(items)) => items.first().expect("common"),
            other => panic!("common ausente: {other:?}"),
        };
        assert_eq!(
            record_get(common, "direction").and_then(SlValue::as_u64),
            Some(1),
            "DIR_NE requerido por UpdateDeltaXY"
        );

        // Smoke OpenTTD: OPENTTDRS_DUMP_MVP_TRAIN_SAV=/ruta/mvp_openttd_train.sav
        if let Ok(path) = std::env::var("OPENTTDRS_DUMP_MVP_TRAIN_SAV") {
            let path = std::path::PathBuf::from(&path);
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            std::fs::write(&path, &bytes).expect("dump train sav");
        }
    }

    #[test]
    fn export_mvp_ship_emits_vehs_ship_and_ordl() {
        use crate::sav::chunks::{find_chunk, parse_chunks};
        use crate::sav::table::{SlValue, parse_table_chunk, record_get};

        let state = mvp_ship_state();
        let bytes = save_to_bytes_with(&state, SavContainer::Ottn).expect("save");
        let payload = &bytes[8..];
        let chunks = parse_chunks(payload).expect("chunks");
        assert!(find_chunk(&chunks, "VEHS").is_some());
        assert!(find_chunk(&chunks, "ORDL").is_some());

        let sav_game = sav::load(&bytes).expect("load rust");
        assert!(
            sav_game
                .vehicles
                .iter()
                .any(|v| v.kind == sav::SavVehicleKind::Ship),
            "ship en VEHS"
        );

        let vehs = find_chunk(&chunks, "VEHS").expect("VEHS");
        let rows = parse_table_chunk(&vehs.body, true).expect("VEHS table");
        assert_eq!(rows.len(), 1);
        assert_eq!(
            record_get(&rows[0].1, "type").and_then(SlValue::as_u64),
            Some(2)
        );

        // Smoke OpenTTD: OPENTTDRS_DUMP_MVP_SHIP_SAV=/ruta/mvp_openttd_ship.sav
        if let Ok(path) = std::env::var("OPENTTDRS_DUMP_MVP_SHIP_SAV") {
            let path = std::path::PathBuf::from(&path);
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            std::fs::write(&path, &bytes).expect("dump ship sav");
        }
    }

    #[test]
    fn export_mvp_rich_emits_indy_road_vehs_and_stations() {
        use crate::sav::chunks::{find_chunk, parse_chunks};
        use crate::sav::table::{SlValue, parse_table_chunk, record_get};

        let state = mvp_rich_state();
        let bytes = save_to_bytes_with(&state, SavContainer::Ottn).expect("save");
        let payload = &bytes[8..];
        let chunks = parse_chunks(payload).expect("chunks");
        assert!(find_chunk(&chunks, "INDY").is_some());
        assert!(find_chunk(&chunks, "VEHS").is_some());
        assert!(find_chunk(&chunks, "STNN").is_some());

        let sav_game = sav::load(&bytes).expect("load rust");
        assert!(sav_game.stations.len() >= 2);
        assert_eq!(sav_game.industries.len(), 1);
        assert_eq!(sav_game.vehicles.len(), 2, "tren + bus");
        assert!(
            sav_game
                .vehicles
                .iter()
                .any(|v| v.kind == sav::SavVehicleKind::RoadVehicle)
        );

        let vehs = find_chunk(&chunks, "VEHS").expect("VEHS");
        let rows = parse_table_chunk(&vehs.body, true).expect("VEHS table");
        assert_eq!(rows.len(), 2);
        let road = rows
            .iter()
            .find(|(_, r)| record_get(r, "type").and_then(SlValue::as_u64) == Some(1))
            .expect("roadveh row");
        let rv = match record_get(&road.1, "roadveh") {
            Some(SlValue::Structs(items)) => items.first().expect("roadveh"),
            other => panic!("roadveh ausente: {other:?}"),
        };
        let common = match record_get(rv, "common") {
            Some(SlValue::Structs(items)) => items.first().expect("common"),
            other => panic!("common ausente: {other:?}"),
        };
        assert_eq!(
            record_get(common, "engine_type").and_then(SlValue::as_u64),
            Some(116),
            "MPS Regal Bus"
        );

        // Smoke OpenTTD: OPENTTDRS_DUMP_MVP_RICH_SAV=/ruta/mvp_openttd_rich.sav
        if let Ok(path) = std::env::var("OPENTTDRS_DUMP_MVP_RICH_SAV") {
            let path = std::path::PathBuf::from(&path);
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            std::fs::write(&path, &bytes).expect("dump rich sav");
        }
    }

    #[test]
    fn export_demo_with_modern_stnn_and_vehs_for_rust_roundtrip() {
        let state = mvp_rich_state();
        let bytes = save_to_bytes_with(&state, SavContainer::Ottn).expect("save");
        let sav_game = sav::load(&bytes).expect("load rust");
        assert!(sav_game.stations.len() >= 2);
        assert_eq!(sav_game.vehicles.len(), 2, "tren + bus");
        assert_eq!(sav_game.industries.len(), 1);

        // Dump opcional (mapa mínimo). Fixture completo: gen_demo_sav.py.
        if let Ok(path) = std::env::var("OPENTTDRS_DUMP_DEMO_SAV") {
            let path = std::path::PathBuf::from(&path);
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            std::fs::write(&path, &bytes).expect("dump demo sav");
        }
    }

    #[test]
    fn export_maps_is_ch_table_with_dim_xy() {
        use crate::sav::chunks::{CH_TABLE, parse_chunks};
        use crate::sav::table::{SlValue, parse_table_chunk, record_get};

        let bytes = save_to_bytes_with(&tiny_state(), SavContainer::Ottn).expect("save");
        assert!(bytes.starts_with(b"OTTN"));
        let payload = &bytes[8..];
        let chunks = parse_chunks(payload).expect("parse chunks");
        let maps = chunks
            .iter()
            .find(|c| &c.name == b"MAPS")
            .expect("MAPS presente");
        assert_eq!(maps.ch_type, CH_TABLE, "MAPS debe ser CH_TABLE (SLV≥294)");
        let rows = parse_table_chunk(&maps.body, false).expect("MAPS table");
        assert_eq!(rows.len(), 1);
        let rec = &rows[0].1;
        assert_eq!(record_get(rec, "dim_x").and_then(SlValue::as_u64), Some(64));
        assert_eq!(record_get(rec, "dim_y").and_then(SlValue::as_u64), Some(64));
        // Planos siguen RIFF.
        let mapt_chunk = chunks.iter().find(|c| &c.name == b"MAPT").expect("MAPT");
        assert_eq!(mapt_chunk.ch_type, 0);
        assert_eq!(mapt_chunk.body.len(), 64 * 64);
    }

    #[test]
    fn export_emits_synthetic_city_when_no_towns() {
        let names = exported_chunk_names(&tiny_state()).expect("chunks");
        assert!(names.iter().any(|n| n == "CITY"), "{names:?}");
        let bytes = save_to_bytes_with(&tiny_state(), SavContainer::Ottn).expect("save");
        let sav_game = sav::load(&bytes).expect("load");
        assert!(
            !sav_game.towns.is_empty(),
            "OpenTTD exige ≥1 municipio; el export sintético debe roundtrippear"
        );
        // Dump opcional para smoke OpenTTD: OPENTTDRS_DUMP_MVP_SAV=/ruta.sav
        if let Ok(path) = std::env::var("OPENTTDRS_DUMP_MVP_SAV") {
            std::fs::write(&path, &bytes).expect("dump mvp sav");
        }
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
        assert!(names.iter().any(|n| n == "LGRP"), "{names:?}");
    }

    #[test]
    fn export_roundtrip_preserves_lgrp_edge() {
        use crate::cargo::CargoType;
        use crate::link_graph::LinkEdgeKey;

        let mut state = tiny_state();
        let a = TileCoord::new(4, 4);
        let b = TileCoord::new(8, 6);
        state.stations = vec![Station::new(a), Station::new(b)];
        state
            .link_graph
            .record_trip(a, b, CargoType::Goods, 7, 40, 120);
        let bytes = save_to_bytes_with(&state, SavContainer::Ottn).expect("save");
        let sav = sav::load(&bytes).expect("load");
        let key = LinkEdgeKey {
            from: a,
            to: b,
            cargo: CargoType::Goods,
        };
        let sample = sav.link_graph.edges.get(&key).expect("LGRP edge");
        assert_eq!(sample.units_total, 7);
        assert!(sample.capacity_total >= 40);
        assert_eq!(sample.travel_time(), 120);
        let loaded = GameState::from_sav_game(sav);
        assert_eq!(
            loaded.link_graph.edges.get(&key).map(|s| s.units_total),
            Some(7)
        );
    }
}
