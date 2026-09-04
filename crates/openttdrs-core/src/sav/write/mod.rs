//! Export mínimo de [`GameState`] a savegame `OpenTTD` (`.sav`).
//!
//! Contenedor por defecto: `OTTZ` (zlib). Versión de save: [`EXPORT_SAVE_VERSION`].
//! Chunks: `MAPS` (`CH_TABLE`) + planos RIFF + `STNN`/`CITY`/`INDY`/`PSAC`/`ORDL`/`VEHS`/`CAPA`/`LGRP` + `DATE` + `PLYR`.
//!
//! Subconjunto prometido (MVP #226/#267): mapa + `CITY` (≥1) + `STNN` moderno
//! (SAVEBYTE + structs) + `VEHS`/`ORDL` (tren + ROAD + ship + aircraft ala fija)
//! + `INDY` + `CAPA` + `ECMY` + `DATE`/`PLYR` cargable por `OpenTTD` ≥15.3 dedicated.
//!
//! Residual: tranvía, settings fuera del subconjunto modelado de `PATS`,
//! ejecución de `ENGN`/`SRND`/callbacks `NewGRF` y flags completos de `PLYR`.
//! La configuración activa `NGRF` (archivo, GRFID, versión y parámetros) y
//! las filas base de `OBJS` se reconstruyen cuando se modifican en el runtime;
//! `OBID` y las columnas desconocidas siguen conservándose como passthrough;
//! `ORDL`/`VEHS`/`STNN`/`CITY`/`INDY` reutilizan sus cuerpos originales cuando
//! las filas semánticas no cambiaron.
//! `PATS`/`ECMY`/`CAPY` aplican la misma regla para ajustes y pagos conocidos.
//! Los chunks nativos no modelados se conservan como passthrough al reexportar.
//! Limitaciones: `docs/PARIDAD.md` y `docs/archive/merged-2026-07/ROADMAP_SAV_EXPORT.md`.

#![allow(clippy::cast_possible_truncation)]

mod chunks;
pub(crate) mod codec;
mod entities;
mod fleet;
mod map;
mod meta;
mod newgrf;
mod object_mappings;
mod objects;
mod vehicles;

use std::io::Write;
use std::path::Path;

use flate2::Compression;
use flate2::write::ZlibEncoder;

use super::SavError;
use crate::game_state::GameState;

/// Versión SLV del export.
///
/// Se mantiene en **355** (mínimo viable actual): ≥294 `MAPS` `CH_TABLE`, ≥295
/// tablas, ≥300 tick u64, ≥348 `HouseID` en MAP8 y ≥355 `PLYR.face_style`.
/// `OpenTTD` 15.3 (`SAVEGAME_VERSION` 362) carga saves más antiguos; subir a
/// 362 no aporta al MVP de load y obligaría campos DATE/economía posteriores
/// sin ganancia.
pub const EXPORT_SAVE_VERSION: u16 = 355;

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

/// Serializa las filas semánticas de las tablas reconstruidas.
///
/// El importador usa esta representación como huella: si las filas siguen
/// iguales, el escritor puede reutilizar el cuerpo original y no perder
/// columnas que una versión más nueva de `OpenTTD` haya añadido.
pub(crate) struct SavSemanticTableRecords {
    pub(crate) ordl: Vec<Vec<u8>>,
    pub(crate) vehs: Vec<Vec<u8>>,
    pub(crate) stnn: Vec<Vec<u8>>,
    pub(crate) city: Vec<Vec<u8>>,
    pub(crate) indy: Vec<Vec<u8>>,
    pub(crate) pats: Vec<Vec<u8>>,
    pub(crate) ecmy: Vec<Vec<u8>>,
    pub(crate) capy: Vec<Vec<u8>>,
    pub(crate) plyr: Vec<Vec<u8>>,
    pub(crate) grps: Vec<Vec<u8>>,
    pub(crate) ernw: Vec<Vec<u8>>,
    pub(crate) lgrp: Vec<Vec<u8>>,
    pub(crate) ngrf: Vec<Vec<u8>>,
    pub(crate) date: Vec<Vec<u8>>,
    pub(crate) capa: Vec<Vec<u8>>,
}

pub(crate) fn semantic_table_records(
    state: &GameState,
) -> Result<SavSemanticTableRecords, SavError> {
    let (map_w, _) = state.map.dimensions();
    let cargo_export = entities::cargo_packet_export(state, map_w);
    let capa = entities::capa_records(&cargo_export);
    let (ordl, vehs) = vehicles::ordl_and_vehs_records_with_cargo(state, map_w, &cargo_export)?;
    let stnn = entities::stnn_records_with_cargo(state, map_w, &cargo_export)?;
    let city = entities::city_records(state, map_w)?;
    let indy = entities::indy_records_with_cargo(state, map_w)?;
    let autoreplace_export = fleet::autoreplace_export(state)?;
    let plyr = meta::plyr_records(state, &autoreplace_export)?;
    let grps = fleet::group_records(&state.vehicle_groups)?;
    let ernw = fleet::autoreplace_records(&autoreplace_export)?;
    let lgrp = super::linkgraph::lgrp_records(&state.link_graph, &state.stations, map_w)?;
    let ngrf = newgrf::newgrf_records(state)?;
    let date_records = vec![meta::date_record(state)];
    Ok(SavSemanticTableRecords {
        ordl,
        vehs,
        stnn,
        city,
        indy,
        pats: vec![meta::pats_record(state)],
        ecmy: vec![meta::ecmy_record(state)],
        capy: meta::capy_records(state)?,
        plyr,
        grps,
        ernw,
        lgrp,
        ngrf,
        date: date_records,
        capa,
    })
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
            "STNN", "CITY", "INDY", "ORDL", "VEHS", "CAPA", "LGRP", "LGRJ", "LGRS", "PATS", "ECMY",
            "CAPY", "GRPS", "ERNW", "ENGN", "ENGS", "EIDS", "GSET", "NGRF", "OBJS", "OBID", "SRND",
            "PSAC", "IIDS", "TIDS", "APID", "ATID", "RAIL", "ROTT", "GLOG", "GOAL", "STPE", "STPA",
            "SIGN",
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

#[allow(clippy::too_many_lines)]
fn build_chunk_stream(state: &GameState) -> Result<Vec<u8>, SavError> {
    let (w, h) = state.map.dimensions();
    if w == 0 || h == 0 {
        return Err(SavError::BadFormat("mapa vacío".into()));
    }
    let n = (w as usize)
        .checked_mul(h as usize)
        .ok_or_else(|| SavError::BadFormat("dimensiones de mapa overflow".into()))?;

    let export_map = entities::map_with_road_stop_indices(state, w)?;
    let planes = map::collect_planes(&export_map, w, h, n);
    let autoreplace_export = fleet::autoreplace_export(state)?;
    let cargo_export = entities::cargo_packet_export(state, w);
    let rebuild_objects = state.sav_objects_dirty
        || !state
            .sav_opaque_chunks
            .iter()
            .any(|chunk| chunk.name == *b"OBJS");
    let rebuild_object_mappings = state.sav_object_mappings_dirty
        || !state
            .sav_opaque_chunks
            .iter()
            .any(|chunk| chunk.name == *b"OBID");

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

    let raw_tables = state.sav_table_passthrough.as_ref();
    let stnn = entities::stnn_records_with_cargo(state, w, &cargo_export)?;
    let raw_stnn = raw_tables.and_then(|passthrough| {
        passthrough.stnn_chunk.as_ref().filter(|chunk| {
            chunk.name == *b"STNN"
                && chunk.ch_type != super::chunks::CH_RIFF
                && passthrough.stnn_semantic_records == stnn
        })
    });
    if let Some(raw) = raw_stnn {
        data.extend_from_slice(&chunks::raw_chunk(raw.name, raw.ch_type, &raw.body));
    } else if !stnn.is_empty() {
        let canonical = entities::stnn_chunk(&stnn)?;
        data.extend_from_slice(&chunks::table_chunk_with_passthrough(
            raw_tables.and_then(|tables| tables.stnn_chunk.as_ref()),
            canonical,
        )?);
    }

    // CITY siempre: OpenTTD rechaza saves sin municipios.
    let city = entities::city_records(state, w)?;
    let raw_city = raw_tables.and_then(|passthrough| {
        passthrough.city_chunk.as_ref().filter(|chunk| {
            chunk.name == *b"CITY"
                && chunk.ch_type != super::chunks::CH_RIFF
                && passthrough.city_semantic_records == city
        })
    });
    if let Some(raw) = raw_city {
        data.extend_from_slice(&chunks::raw_chunk(raw.name, raw.ch_type, &raw.body));
    } else {
        let mut city_header = Vec::new();
        entities::append_city_header(&mut city_header)?;
        let canonical =
            chunks::raw_table_chunk(*b"CITY", &city_header, &city, crate::sav::chunks::CH_TABLE)?;
        data.extend_from_slice(&chunks::table_chunk_with_passthrough(
            raw_tables.and_then(|tables| tables.city_chunk.as_ref()),
            canonical,
        )?);
    }

    let indy = entities::indy_records_with_cargo(state, w)?;
    let raw_indy = raw_tables.and_then(|passthrough| {
        passthrough.indy_chunk.as_ref().filter(|chunk| {
            chunk.name == *b"INDY"
                && chunk.ch_type != super::chunks::CH_RIFF
                && passthrough.indy_semantic_records == indy
        })
    });
    if let Some(raw) = raw_indy {
        data.extend_from_slice(&chunks::raw_chunk(raw.name, raw.ch_type, &raw.body));
    } else if !indy.is_empty() {
        let canonical = entities::indy_chunk(state, w)?;
        data.extend_from_slice(&chunks::table_chunk_with_passthrough(
            raw_tables.and_then(|tables| tables.indy_chunk.as_ref()),
            canonical,
        )?);
    }

    let (ordl, vehs) = vehicles::ordl_and_vehs_records_with_cargo(state, w, &cargo_export)?;
    let raw_ordl = raw_tables.and_then(|passthrough| {
        passthrough.ordl_chunk.as_ref().filter(|chunk| {
            chunk.name == *b"ORDL"
                && chunk.ch_type != super::chunks::CH_RIFF
                && passthrough.ordl_semantic_records == ordl
        })
    });
    if let Some(raw) = raw_ordl {
        data.extend_from_slice(&chunks::raw_chunk(raw.name, raw.ch_type, &raw.body));
    } else if !ordl.is_empty() {
        let canonical = vehicles::ordl_chunk(&ordl)?;
        data.extend_from_slice(&chunks::table_chunk_with_passthrough(
            raw_tables.and_then(|tables| tables.ordl_chunk.as_ref()),
            canonical,
        )?);
    }
    let raw_vehs = raw_tables.and_then(|passthrough| {
        passthrough.vehs_chunk.as_ref().filter(|chunk| {
            chunk.name == *b"VEHS"
                && chunk.ch_type != super::chunks::CH_RIFF
                && passthrough.vehs_semantic_records == vehs
        })
    });
    if let Some(raw) = raw_vehs {
        data.extend_from_slice(&chunks::raw_chunk(raw.name, raw.ch_type, &raw.body));
    } else if !vehs.is_empty() {
        let canonical = vehicles::vehs_chunk(&vehs)?;
        data.extend_from_slice(&chunks::table_chunk_with_passthrough(
            raw_tables.and_then(|tables| tables.vehs_chunk.as_ref()),
            canonical,
        )?);
    }
    let capa_records = entities::capa_records(&cargo_export);
    let raw_capa = raw_tables.and_then(|passthrough| {
        passthrough.capa_chunk.as_ref().filter(|chunk| {
            chunk.name == *b"CAPA"
                && chunk.ch_type != super::chunks::CH_RIFF
                && passthrough.capa_semantic_records == capa_records
        })
    });
    if let Some(raw) = raw_capa {
        data.extend_from_slice(&chunks::raw_chunk(raw.name, raw.ch_type, &raw.body));
    } else if let Some(capa) = entities::capa_chunk(&cargo_export)? {
        data.extend_from_slice(&chunks::table_chunk_with_passthrough(
            raw_tables.and_then(|tables| tables.capa_chunk.as_ref()),
            capa,
        )?);
    }

    let lgrp = super::linkgraph::lgrp_records(&state.link_graph, &state.stations, w)?;
    let raw_lgrp = raw_tables.and_then(|passthrough| {
        passthrough.lgrp_chunk.as_ref().filter(|chunk| {
            chunk.name == *b"LGRP"
                && chunk.ch_type != super::chunks::CH_RIFF
                && passthrough.lgrp_semantic_records == lgrp
        })
    });
    if let Some(raw) = raw_lgrp {
        data.extend_from_slice(&chunks::raw_chunk(raw.name, raw.ch_type, &raw.body));
    } else {
        data.extend_from_slice(&super::linkgraph::encode_lgrp_chunk(
            &state.link_graph,
            &state.stations,
            w,
        )?);
    }
    data.extend_from_slice(&super::linkgraph::encode_linkgraph_runtime_chunks(
        &state.link_graph,
    )?);

    let pats = vec![meta::pats_record(state)];
    let raw_pats = raw_tables.and_then(|passthrough| {
        passthrough.pats_chunk.as_ref().filter(|chunk| {
            chunk.name == *b"PATS"
                && chunk.ch_type != super::chunks::CH_RIFF
                && passthrough.pats_semantic_records == pats
        })
    });
    if let Some(raw) = raw_pats {
        data.extend_from_slice(&chunks::raw_chunk(raw.name, raw.ch_type, &raw.body));
    } else {
        let canonical = meta::pats_chunk(state)?;
        data.extend_from_slice(&chunks::table_chunk_with_passthrough(
            raw_tables.and_then(|tables| tables.pats_chunk.as_ref()),
            canonical,
        )?);
    }

    let ecmy = vec![meta::ecmy_record(state)];
    let raw_ecmy = raw_tables.and_then(|passthrough| {
        passthrough.ecmy_chunk.as_ref().filter(|chunk| {
            chunk.name == *b"ECMY"
                && chunk.ch_type != super::chunks::CH_RIFF
                && passthrough.ecmy_semantic_records == ecmy
        })
    });
    if let Some(raw) = raw_ecmy {
        data.extend_from_slice(&chunks::raw_chunk(raw.name, raw.ch_type, &raw.body));
    } else {
        let canonical = meta::ecmy_chunk(state)?;
        data.extend_from_slice(&chunks::table_chunk_with_passthrough(
            raw_tables.and_then(|tables| tables.ecmy_chunk.as_ref()),
            canonical,
        )?);
    }

    let capy = meta::capy_records(state)?;
    let raw_capy_payments = raw_tables.and_then(|passthrough| {
        passthrough.capy_chunk.as_ref().filter(|chunk| {
            chunk.name == *b"CAPY"
                && chunk.ch_type != super::chunks::CH_RIFF
                && passthrough.capy_semantic_records == capy
        })
    });
    if let Some(raw) = raw_capy_payments {
        data.extend_from_slice(&chunks::raw_chunk(raw.name, raw.ch_type, &raw.body));
    } else if let Some(capy) = meta::capy_chunk(state)? {
        data.extend_from_slice(&chunks::table_chunk_with_passthrough(
            raw_tables.and_then(|tables| tables.capy_chunk.as_ref()),
            capy,
        )?);
    }
    let grps = fleet::group_records(&state.vehicle_groups)?;
    let raw_grps = raw_tables.and_then(|passthrough| {
        passthrough.grps_chunk.as_ref().filter(|chunk| {
            chunk.name == *b"GRPS"
                && chunk.ch_type != super::chunks::CH_RIFF
                && passthrough.grps_semantic_records == grps
        })
    });
    if let Some(raw) = raw_grps {
        data.extend_from_slice(&chunks::raw_chunk(raw.name, raw.ch_type, &raw.body));
    } else if let Some(groups) = fleet::groups_chunk(&state.vehicle_groups)? {
        data.extend_from_slice(&chunks::table_chunk_with_passthrough(
            raw_tables.and_then(|tables| tables.grps_chunk.as_ref()),
            groups,
        )?);
    }

    let ernw = fleet::autoreplace_records(&autoreplace_export)?;
    let raw_ernw = raw_tables.and_then(|passthrough| {
        passthrough.ernw_chunk.as_ref().filter(|chunk| {
            chunk.name == *b"ERNW"
                && chunk.ch_type != super::chunks::CH_RIFF
                && passthrough.ernw_semantic_records == ernw
        })
    });
    if let Some(raw) = raw_ernw {
        data.extend_from_slice(&chunks::raw_chunk(raw.name, raw.ch_type, &raw.body));
    } else if let Some(renew) = fleet::autoreplace_chunk(&autoreplace_export)? {
        data.extend_from_slice(&chunks::table_chunk_with_passthrough(
            raw_tables.and_then(|tables| tables.ernw_chunk.as_ref()),
            renew,
        )?);
    }
    let ngrf = newgrf::newgrf_records(state)?;
    let raw_ngrf = raw_tables.and_then(|passthrough| {
        passthrough.ngrf_chunk.as_ref().filter(|chunk| {
            chunk.name == *b"NGRF"
                && chunk.ch_type != super::chunks::CH_RIFF
                && passthrough.ngrf_semantic_records == ngrf
        })
    });
    if let Some(raw) = raw_ngrf {
        data.extend_from_slice(&chunks::raw_chunk(raw.name, raw.ch_type, &raw.body));
    } else if let Some(ngrf) = newgrf::newgrf_chunk(state)? {
        data.extend_from_slice(&chunks::table_chunk_with_passthrough(
            raw_tables.and_then(|tables| tables.ngrf_chunk.as_ref()),
            ngrf,
        )?);
    }
    if rebuild_objects && let Some(objs) = objects::objects_chunk(state, w, h)? {
        data.extend_from_slice(&objs);
    }
    if rebuild_object_mappings && let Some(obid) = object_mappings::object_mappings_chunk(state)? {
        data.extend_from_slice(&obid);
    }
    // `PSAC` se reconstruye cuando el estado tiene storages decodificados o
    // una industria escribió registros `7C`. Las filas ajenas (por ejemplo de
    // estaciones/pueblos todavía sin runtime) viajan dentro del mismo pool y
    // se conservan en `GameState::sav_persistent_storages`.
    let psac = entities::persistent_storage_chunk(state)?;
    let rebuild_psac = psac.is_some();
    if let Some(psac) = psac {
        data.extend_from_slice(&psac);
    }
    for chunk in &state.sav_opaque_chunks {
        if super::REBUILT_CHUNKS.contains(&chunk.name)
            || (rebuild_objects && chunk.name == *b"OBJS")
            || (rebuild_object_mappings && chunk.name == *b"OBID")
            || (rebuild_psac && chunk.name == *b"PSAC")
        {
            continue;
        }
        data.extend_from_slice(&chunks::raw_chunk(chunk.name, chunk.ch_type, &chunk.body));
    }

    let date_records = vec![meta::date_record(state)];
    let raw_date = raw_tables.and_then(|passthrough| {
        passthrough.date_chunk.as_ref().filter(|chunk| {
            chunk.name == *b"DATE"
                && chunk.ch_type != super::chunks::CH_RIFF
                && passthrough.date_semantic_records == date_records
        })
    });
    if let Some(raw) = raw_date {
        data.extend_from_slice(&chunks::raw_chunk(raw.name, raw.ch_type, &raw.body));
    } else {
        let canonical = chunks::table_chunk(
            *b"DATE",
            &[
                (5, "date"),
                (8, "tick_counter"),
                (6, "random_state[0]"),
                (6, "random_state[1]"),
            ],
            &date_records,
        )?;
        data.extend_from_slice(&chunks::table_chunk_with_passthrough(
            raw_tables.and_then(|tables| tables.date_chunk.as_ref()),
            canonical,
        )?);
    }
    let plyr = meta::plyr_records(state, &autoreplace_export)?;
    let raw_plyr = raw_tables.and_then(|passthrough| {
        passthrough.plyr_chunk.as_ref().filter(|chunk| {
            chunk.name == *b"PLYR"
                && chunk.ch_type != super::chunks::CH_RIFF
                && passthrough.plyr_semantic_records == plyr
        })
    });
    if let Some(raw) = raw_plyr {
        data.extend_from_slice(&chunks::raw_chunk(raw.name, raw.ch_type, &raw.body));
    } else {
        let canonical = meta::plyr_chunk(state, &autoreplace_export)?;
        data.extend_from_slice(&chunks::table_chunk_with_passthrough(
            raw_tables.and_then(|tables| tables.plyr_chunk.as_ref()),
            canonical,
        )?);
    }

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
    use crate::vehicle::{Vehicle, VehicleKind, VehicleOrder};

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

    fn assert_table_field_type(body: &[u8], field_type: u8, field_name: &str) {
        let mut encoded = vec![field_type];
        codec::write_str(field_name, &mut encoded).expect("encode field name");
        assert!(
            body.windows(encoded.len()).any(|window| window == encoded),
            "header no contiene {field_name} con tipo {field_type:#04x}"
        );
    }

    #[test]
    fn ottn_roundtrip_preserves_stations_stnn() {
        let mut state = tiny_state();
        let mut rail = Station::new_with_kind(TileCoord::new(28, 39), StopKind::RailStation);
        rail.name = Some("Central Demo".into());
        rail.owner = crate::company::CompanyId::NONE;
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
        assert_eq!(central.owner, crate::company::CompanyId::NONE.0);
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
    #[allow(clippy::too_many_lines)]
    fn ottn_roundtrip_preserves_construction_settings_in_pats() {
        let mut state = tiny_state();
        state.climate = crate::Climate::SubTropical;
        state.snow_line_height = 2;
        state.construction.map_height_limit = 75;
        state.construction.road_vehicle_driving_side = crate::RoadVehicleDrivingSide::Right;
        state.construction.train_signal_side = crate::TrainSignalSide::Right;
        state.construction.freeform_edges = false;
        state.pathfinding.wait_for_pbs_path = 7;
        state.pathfinding.path_backoff_interval = 8;
        state.pathfinding.reverse_at_signals = false;
        state.pathfinding.wait_oneway_signal = 9;
        state.pathfinding.wait_twoway_signal = 10;
        state.pathfinding.reserve_paths = true;
        state.train_acceleration_model = crate::engine::TrainAccelerationModel::Original;
        state.road_vehicle_acceleration_model =
            crate::engine::RoadVehicleAccelerationModel::Original;
        state.station_noise_level = true;
        state.serve_neutral_industries = false;
        state.vehicle_breakdowns = 0;
        state.no_servicing_if_no_breakdowns = false;
        state.subsidy_duration = 5_000;
        state.subsidy_multiplier = 3;
        state.disasters_enabled = false;
        state.town_council_tolerance = crate::town::TownCouncilTolerance::Permissive;
        state.using_wallclock_units = true;
        state.global_economy.inflation_enabled = false;
        state.global_economy.recessions_enabled = true;
        state.global_economy.inflation_prices = 123_456;
        state.global_economy.inflation_payment = 234_567;
        state.global_economy.fluct = -7;
        state.global_economy.interest_rate = 13;
        state.global_economy.infl_amount = 4;
        state.global_economy.infl_amount_pr = 3;
        state.global_economy.industry_daily_change_counter = 77;
        state.cargo_payments = vec![crate::CargoPaymentState {
            id: 1,
            front_vehicle_ref: Some(7),
            front_vehicle_id: None,
            route_profit: -11,
            visual_profit: -7,
            visual_transfer: 3,
        }];

        let bytes = save_to_bytes_with(&state, SavContainer::Ottn).expect("save");
        let sav_game = sav::load(&bytes).expect("load");
        assert_eq!(sav_game.climate, state.climate);
        assert_eq!(sav_game.snow_line_height, state.snow_line_height);
        assert_eq!(sav_game.construction, state.construction);
        assert_eq!(sav_game.pathfinding, state.pathfinding);
        assert_eq!(
            sav_game.train_acceleration_model,
            state.train_acceleration_model
        );
        assert_eq!(
            sav_game.road_vehicle_acceleration_model,
            state.road_vehicle_acceleration_model
        );
        assert_eq!(sav_game.station_noise_level, state.station_noise_level);
        assert_eq!(
            sav_game.serve_neutral_industries,
            state.serve_neutral_industries
        );
        assert_eq!(sav_game.vehicle_breakdowns, state.vehicle_breakdowns);
        assert_eq!(
            sav_game.no_servicing_if_no_breakdowns,
            state.no_servicing_if_no_breakdowns
        );
        assert_eq!(sav_game.subsidy_duration, state.subsidy_duration);
        assert_eq!(sav_game.subsidy_multiplier, state.subsidy_multiplier);
        assert_eq!(sav_game.disasters_enabled, state.disasters_enabled);
        assert_eq!(
            sav_game.town_council_tolerance,
            state.town_council_tolerance
        );
        assert_eq!(sav_game.using_wallclock_units, state.using_wallclock_units);
        assert_eq!(
            sav_game.global_economy.inflation_enabled,
            state.global_economy.inflation_enabled
        );
        assert_eq!(
            sav_game.global_economy.recessions_enabled,
            state.global_economy.recessions_enabled
        );
        assert_eq!(sav_game.global_economy, state.global_economy);
        assert_eq!(sav_game.cargo_payments, state.cargo_payments);
        assert!(
            exported_chunk_names(&state)
                .expect("chunk names")
                .iter()
                .any(|name| name == "PATS")
        );
        assert!(
            exported_chunk_names(&state)
                .expect("chunk names")
                .iter()
                .any(|name| name == "ECMY")
        );
        assert!(
            exported_chunk_names(&state)
                .expect("chunk names")
                .iter()
                .any(|name| name == "CAPY")
        );
    }

    #[test]
    fn capy_runtime_front_id_is_translated_to_sparse_vehicle_ref() {
        let mut state = tiny_state();
        let vehicle_pos = TileCoord::new(10, 20);
        state.vehicles.push(Vehicle::new(
            41,
            VehicleKind::Train,
            vehicle_pos,
            vehicle_pos,
        ));
        state.cargo_payments = vec![crate::CargoPaymentState {
            id: 0,
            front_vehicle_ref: None,
            front_vehicle_id: Some(41),
            route_profit: 99,
            visual_profit: 77,
            visual_transfer: 11,
        }];

        let bytes = save_to_bytes_with(&state, SavContainer::Ottn).expect("save");
        let sav_game = sav::load(&bytes).expect("load");
        assert_eq!(sav_game.cargo_payments.len(), 1);
        assert_eq!(sav_game.cargo_payments[0].front_vehicle_ref, Some(0));
        assert_eq!(sav_game.cargo_payments[0].route_profit, 99);

        let loaded = GameState::from_sav_game(sav_game);
        assert_eq!(loaded.vehicles.len(), 1);
        assert_eq!(loaded.cargo_payments[0].front_vehicle_id, Some(0));
        assert_eq!(loaded.cargo_payments[0].front_vehicle_ref, Some(0));
        let (payload, _) = crate::sav::container::decompress(&bytes).expect("payload");
        let chunks = crate::sav::chunks::parse_chunks(&payload).expect("chunks");
        let original_capy = crate::sav::chunks::find_chunk(&chunks, "CAPY")
            .expect("CAPY original")
            .body
            .clone();
        let resaved = save_to_bytes_with(&loaded, SavContainer::Ottn).expect("resave");
        let (resaved_payload, _) = crate::sav::container::decompress(&resaved).expect("payload");
        let resaved_chunks = crate::sav::chunks::parse_chunks(&resaved_payload).expect("chunks");
        let resaved_capy =
            crate::sav::chunks::find_chunk(&resaved_chunks, "CAPY").expect("CAPY resaved");
        assert_eq!(resaved_capy.body, original_capy);
    }

    #[test]
    fn ottn_roundtrip_preserves_group_names_and_autoreplace_rules() {
        let mut state = tiny_state();
        let mut group = crate::VehicleGroup::new(7, "Carga");
        group.owner = 3;
        group.vehicle_type = 1;
        group.flags = 2;
        group.livery_in_use = 3;
        group.livery_colour1 = 4;
        group.livery_colour2 = 5;
        group.parent = Some(2);
        group.number = 11;
        state.vehicle_groups = vec![group];
        let vehicle_pos = TileCoord::new(10, 20);
        let mut grouped = Vehicle::new(42, VehicleKind::Train, vehicle_pos, vehicle_pos);
        grouped.group_id = Some(7);
        state.vehicles = vec![grouped];
        state.autoreplace_rules.push(crate::AutoReplaceRule {
            from_engine_id: 100,
            to_engine_id: 101,
            owner: Some(crate::CompanyId::PLAYER),
            enabled: true,
            only_when_old: true,
            group_id: Some(7),
            default_group_only: false,
            sav_pool_id: Some(2),
            sav_next_pool_id: None,
        });
        state.companies[0].engine_renew_list_head = Some(2);

        let bytes = save_to_bytes_with(&state, SavContainer::Ottn).expect("save");
        let (payload, _) = crate::sav::container::decompress(&bytes).expect("decompress");
        let chunks = crate::sav::chunks::parse_chunks(&payload).expect("chunks");
        let ernw = crate::sav::chunks::find_chunk(&chunks, "ERNW").expect("ERNW chunk");
        assert_table_field_type(&ernw.body, 6, "next");
        assert_table_field_type(&ernw.body, 1, "replace_when_old");
        let plyr = crate::sav::chunks::find_chunk(&chunks, "PLYR").expect("PLYR chunk");
        assert_table_field_type(&plyr.body, 6, "engine_renew_list");
        let sav_game = sav::load(&bytes).expect("load");
        assert_eq!(sav_game.vehicle_groups, state.vehicle_groups);
        assert_eq!(sav_game.autoreplace_rules, state.autoreplace_rules);
        assert_eq!(sav_game.companies[0].engine_renew_list_head, Some(2));
        assert_eq!(sav_game.vehicles.len(), 1);
        assert_eq!(sav_game.vehicles[0].group_id, Some(7));
        let loaded = GameState::from_sav_game(sav_game);
        assert_eq!(loaded.vehicles.len(), 1);
        assert_eq!(loaded.vehicles[0].group_id, Some(7));
        assert_eq!(loaded.companies[0].engine_renew_list_head, Some(2));
        let names = exported_chunk_names(&state).expect("chunk names");
        assert!(names.iter().any(|name| name == "GRPS"));
        assert!(names.iter().any(|name| name == "ERNW"));
    }

    #[test]
    fn ottn_roundtrip_preserves_ernw_chains_per_company() {
        let mut state = tiny_state();
        state.ensure_rival_transcargo();
        state.autoreplace_rules = vec![
            crate::AutoReplaceRule {
                from_engine_id: 10,
                to_engine_id: 11,
                owner: Some(crate::CompanyId::PLAYER),
                enabled: true,
                only_when_old: false,
                group_id: None,
                default_group_only: false,
                sav_pool_id: Some(2),
                sav_next_pool_id: None,
            },
            crate::AutoReplaceRule {
                from_engine_id: 20,
                to_engine_id: 21,
                owner: Some(crate::CompanyId(1)),
                enabled: true,
                only_when_old: true,
                group_id: None,
                default_group_only: false,
                sav_pool_id: Some(4),
                sav_next_pool_id: Some(7),
            },
            crate::AutoReplaceRule {
                from_engine_id: 30,
                to_engine_id: 31,
                owner: Some(crate::CompanyId(1)),
                enabled: true,
                only_when_old: false,
                group_id: None,
                default_group_only: true,
                sav_pool_id: Some(7),
                sav_next_pool_id: None,
            },
        ];
        state.companies[0].engine_renew_list_head = Some(2);
        state.companies[1].engine_renew_list_head = Some(4);

        let bytes = save_to_bytes_with(&state, SavContainer::Ottn).expect("save");
        if let Ok(path) = std::env::var("OPENTTDRS_DUMP_ERNW_SAV") {
            std::fs::write(&path, &bytes).expect("dump ERNW sav");
        }
        let sav_game = sav::load(&bytes).expect("load");
        assert_eq!(sav_game.companies[0].engine_renew_list_head, Some(2));
        assert_eq!(sav_game.companies[1].engine_renew_list_head, Some(4));
        assert_eq!(sav_game.autoreplace_rules, state.autoreplace_rules);

        let loaded = GameState::from_sav_game(sav_game);
        assert_eq!(loaded.companies[0].engine_renew_list_head, Some(2));
        assert_eq!(loaded.companies[1].engine_renew_list_head, Some(4));
        assert_eq!(loaded.autoreplace_rules, state.autoreplace_rules);
    }

    #[test]
    fn ottn_roundtrip_rehydrates_shared_order_identity() {
        let mut state = tiny_state();
        let station_pos = TileCoord::new(28, 39);
        state.stations = vec![Station::new_with_kind(station_pos, StopKind::RailStation)];
        let orders = vec![VehicleOrder::station(station_pos)];
        state.shared_order_lists = vec![crate::SharedOrderList {
            id: 77,
            orders: orders.clone(),
        }];
        let mut first = Vehicle::new(
            40,
            VehicleKind::Train,
            TileCoord::new(10, 20),
            TileCoord::new(10, 20),
        );
        first.shared_order_id = Some(77);
        first.next_shared_vehicle_id = Some(41);
        first.set_vehicle_orders(orders.clone());
        let mut second = Vehicle::new(
            41,
            VehicleKind::Train,
            TileCoord::new(11, 20),
            TileCoord::new(11, 20),
        );
        second.shared_order_id = Some(77);
        second.set_vehicle_orders(orders);
        state.vehicles = vec![first, second];

        let bytes = save_to_bytes_with(&state, SavContainer::Ottn).expect("save");
        let loaded = GameState::from_sav_game(sav::load(&bytes).expect("load"));
        assert_eq!(loaded.shared_order_lists.len(), 1);
        assert_eq!(loaded.shared_order_lists[0].id, 0);
        assert_eq!(loaded.shared_order_lists[0].orders.len(), 1);
        assert_eq!(loaded.vehicles.len(), 2);
        assert_eq!(loaded.vehicles[0].shared_order_id, Some(0));
        assert_eq!(loaded.vehicles[1].shared_order_id, Some(0));
        assert_eq!(loaded.vehicles[0].next_shared_vehicle_id, Some(1));
        assert_eq!(loaded.vehicles[1].next_shared_vehicle_id, None);
    }

    #[test]
    fn ottn_roundtrip_preserves_opaque_runtime_chunks() {
        let mut state = tiny_state();
        let body = crate::sav::table::tests::build_table_body(&[(2, "grfid")], &[vec![7]]);
        state.sav_opaque_chunks = [*b"GSET", *b"ENGN", *b"SRND"]
            .into_iter()
            .map(|name| crate::SavOpaqueChunk {
                name,
                ch_type: crate::sav::chunks::CH_TABLE,
                body: body.clone(),
            })
            .collect();

        let bytes = save_to_bytes_with(&state, SavContainer::Ottn).expect("save");
        let sav_game = sav::load(&bytes).expect("load");
        assert_eq!(sav_game.opaque_chunks, state.sav_opaque_chunks);
        let loaded = GameState::from_sav_game(sav_game);
        assert_eq!(loaded.sav_opaque_chunks, state.sav_opaque_chunks);
        let names = exported_chunk_names(&state).expect("chunk names");
        for expected in ["GSET", "ENGN", "SRND"] {
            assert!(names.iter().any(|name| name == expected), "{names:?}");
        }
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn imported_vehs_body_is_reused_until_vehicle_semantics_change() {
        let mut state = tiny_state();
        let vehicle_pos = TileCoord::new(10, 20);
        let station_pos = TileCoord::new(28, 39);
        state
            .stations
            .push(Station::new_with_kind(station_pos, StopKind::RailStation));
        state.industries.push(crate::Industry::new(
            TileCoord::new(5, 5),
            crate::IndustryKind::CoalMine,
        ));
        let mut train = Vehicle::new(41, VehicleKind::Train, vehicle_pos, vehicle_pos);
        train.set_vehicle_orders(vec![VehicleOrder::station(station_pos)]);
        state.vehicles.push(train);

        let original = save_to_bytes_with(&state, SavContainer::Ottn).expect("save original");
        let (original_payload, _) = crate::sav::container::decompress(&original).expect("payload");
        let original_chunks = crate::sav::chunks::parse_chunks(&original_payload).expect("chunks");
        let original_vehs = crate::sav::chunks::find_chunk(&original_chunks, "VEHS")
            .expect("VEHS original")
            .body
            .clone();
        let original_ordl = crate::sav::chunks::find_chunk(&original_chunks, "ORDL")
            .expect("ORDL original")
            .body
            .clone();
        let original_stnn = crate::sav::chunks::find_chunk(&original_chunks, "STNN")
            .expect("STNN original")
            .body
            .clone();
        let original_city = crate::sav::chunks::find_chunk(&original_chunks, "CITY")
            .expect("CITY original")
            .body
            .clone();
        let original_indy = crate::sav::chunks::find_chunk(&original_chunks, "INDY")
            .expect("INDY original")
            .body
            .clone();
        let original_pats = crate::sav::chunks::find_chunk(&original_chunks, "PATS")
            .expect("PATS original")
            .body
            .clone();
        let original_ecmy = crate::sav::chunks::find_chunk(&original_chunks, "ECMY")
            .expect("ECMY original")
            .body
            .clone();
        let original_plyr = crate::sav::chunks::find_chunk(&original_chunks, "PLYR")
            .expect("PLYR original")
            .body
            .clone();
        let original_date = crate::sav::chunks::find_chunk(&original_chunks, "DATE")
            .expect("DATE original")
            .body
            .clone();

        let mut loaded = GameState::from_sav_game(sav::load(&original).expect("load original"));
        let passthrough = loaded
            .sav_table_passthrough
            .as_ref()
            .expect("VEHS passthrough after import");
        assert_eq!(
            passthrough
                .vehs_chunk
                .as_ref()
                .expect("VEHS passthrough")
                .body,
            original_vehs
        );
        assert_eq!(
            passthrough
                .ordl_chunk
                .as_ref()
                .expect("ORDL passthrough")
                .body,
            original_ordl
        );
        assert_eq!(
            passthrough
                .stnn_chunk
                .as_ref()
                .expect("STNN passthrough")
                .body,
            original_stnn
        );
        assert_eq!(
            passthrough
                .city_chunk
                .as_ref()
                .expect("CITY passthrough")
                .body,
            original_city
        );
        assert_eq!(
            passthrough
                .indy_chunk
                .as_ref()
                .expect("INDY passthrough")
                .body,
            original_indy
        );
        assert_eq!(
            passthrough
                .pats_chunk
                .as_ref()
                .expect("PATS passthrough")
                .body,
            original_pats
        );
        assert_eq!(
            passthrough
                .ecmy_chunk
                .as_ref()
                .expect("ECMY passthrough")
                .body,
            original_ecmy
        );
        assert_eq!(
            passthrough
                .plyr_chunk
                .as_ref()
                .expect("PLYR passthrough")
                .body,
            original_plyr
        );
        assert_eq!(
            passthrough
                .date_chunk
                .as_ref()
                .expect("DATE passthrough")
                .body,
            original_date
        );

        let resaved = save_to_bytes_with(&loaded, SavContainer::Ottn).expect("resave");
        let (resaved_payload, _) = crate::sav::container::decompress(&resaved).expect("payload");
        let resaved_chunks = crate::sav::chunks::parse_chunks(&resaved_payload).expect("chunks");
        let resaved_vehs =
            crate::sav::chunks::find_chunk(&resaved_chunks, "VEHS").expect("VEHS resaved");
        assert_eq!(resaved_vehs.body, original_vehs);
        let resaved_ordl =
            crate::sav::chunks::find_chunk(&resaved_chunks, "ORDL").expect("ORDL resaved");
        assert_eq!(resaved_ordl.body, original_ordl);
        let resaved_stnn =
            crate::sav::chunks::find_chunk(&resaved_chunks, "STNN").expect("STNN resaved");
        assert_eq!(resaved_stnn.body, original_stnn);
        let resaved_city =
            crate::sav::chunks::find_chunk(&resaved_chunks, "CITY").expect("CITY resaved");
        assert_eq!(resaved_city.body, original_city);
        let resaved_indy =
            crate::sav::chunks::find_chunk(&resaved_chunks, "INDY").expect("INDY resaved");
        assert_eq!(resaved_indy.body, original_indy);
        let resaved_pats =
            crate::sav::chunks::find_chunk(&resaved_chunks, "PATS").expect("PATS resaved");
        assert_eq!(resaved_pats.body, original_pats);
        let resaved_ecmy =
            crate::sav::chunks::find_chunk(&resaved_chunks, "ECMY").expect("ECMY resaved");
        assert_eq!(resaved_ecmy.body, original_ecmy);
        let resaved_plyr =
            crate::sav::chunks::find_chunk(&resaved_chunks, "PLYR").expect("PLYR resaved");
        assert_eq!(resaved_plyr.body, original_plyr);
        let resaved_date =
            crate::sav::chunks::find_chunk(&resaved_chunks, "DATE").expect("DATE resaved");
        assert_eq!(resaved_date.body, original_date);

        loaded.vehicles[0].cur_speed = loaded.vehicles[0].cur_speed.saturating_add(1);
        let changed = save_to_bytes_with(&loaded, SavContainer::Ottn).expect("save changed");
        let (changed_payload, _) = crate::sav::container::decompress(&changed).expect("payload");
        let changed_chunks = crate::sav::chunks::parse_chunks(&changed_payload).expect("chunks");
        let changed_vehs =
            crate::sav::chunks::find_chunk(&changed_chunks, "VEHS").expect("VEHS changed");
        assert_ne!(changed_vehs.body, original_vehs);
    }

    #[test]
    fn ottn_roundtrip_preserves_active_newgrf_configuration() {
        let mut state = tiny_state();
        let mut active = crate::NewGrfEntry::new("active.grf", 0x4142_4301);
        active.grf_version = 8;
        active.set_param(0, 0x0102_0304);
        active.set_param(3, 0xAABB_CCDD);
        let mut disabled = crate::NewGrfEntry::new("disabled.grf", 0x4449_5301);
        disabled.enabled = false;
        let mut static_grf = crate::NewGrfEntry::new("static.grf", 0x5354_4101);
        static_grf.is_static = true;
        state.newgrf_stack = vec![active.clone(), disabled, static_grf];

        let bytes = save_to_bytes_with(&state, SavContainer::Ottn).expect("save");
        let payload = &bytes[8..];
        let chunks = crate::sav::chunks::parse_chunks(payload).expect("chunks");
        let ngrf = crate::sav::chunks::find_chunk(&chunks, "NGRF").expect("NGRF");
        assert_eq!(ngrf.ch_type, crate::sav::chunks::CH_TABLE);
        let original_ngrf = ngrf.body.clone();

        let sav_game = sav::load(&bytes).expect("load");
        assert_eq!(sav_game.newgrf_stack, vec![active.clone()]);
        let loaded = GameState::from_sav_game(sav_game);
        assert_eq!(loaded.newgrf_stack, vec![active]);
        let passthrough = loaded
            .sav_table_passthrough
            .as_ref()
            .expect("NGRF passthrough after import");
        assert_eq!(
            passthrough
                .ngrf_chunk
                .as_ref()
                .expect("NGRF passthrough")
                .body,
            original_ngrf
        );
        let resaved = save_to_bytes_with(&loaded, SavContainer::Ottn).expect("resave");
        let resaved_chunks = crate::sav::chunks::parse_chunks(&resaved[8..]).expect("chunks");
        assert_eq!(
            crate::sav::chunks::find_chunk(&resaved_chunks, "NGRF")
                .expect("NGRF resaved")
                .body,
            original_ngrf
        );
        let mut changed = loaded.clone();
        changed.newgrf_stack[0].set_param(0, 0xDEAD_BEEF);
        let changed_bytes = save_to_bytes_with(&changed, SavContainer::Ottn).expect("changed");
        let changed_chunks = crate::sav::chunks::parse_chunks(&changed_bytes[8..]).expect("chunks");
        assert_ne!(
            crate::sav::chunks::find_chunk(&changed_chunks, "NGRF")
                .expect("NGRF changed")
                .body,
            original_ngrf
        );
    }

    #[test]
    fn ottn_roundtrip_hydrates_industry_psa_storage() {
        let mut state = tiny_state();
        let pos = TileCoord::new(12, 12);
        let mut tile = state.map.get(pos).expect("industry tile");
        tile.kind = TileKind::Industry;
        state.map.set_tile(pos, tile).expect("set industry tile");
        let mut industry = crate::Industry::new(pos, crate::IndustryKind::CoalMine);
        industry.newgrf_persistent_regs.insert(7, 0xDEAD_BEEF);
        state.industries.push(industry);

        let bytes = save_to_bytes_with(&state, SavContainer::Ottn).expect("save");
        let chunks = crate::sav::chunks::parse_chunks(&bytes[8..]).expect("chunks");
        let indy = crate::sav::chunks::find_chunk(&chunks, "INDY").expect("INDY");
        let indy_rows =
            crate::sav::table::parse_table_chunk(&indy.body, false).expect("INDY table");
        assert_eq!(
            crate::sav::table::record_get(&indy_rows[0].1, "psa")
                .and_then(crate::sav::table::SlValue::as_u64),
            Some(1)
        );
        assert!(crate::sav::chunks::find_chunk(&chunks, "PSAC").is_some());

        let loaded = GameState::from_sav_game(sav::load(&bytes).expect("load"));
        assert_eq!(loaded.industries.len(), 1);
        assert_eq!(loaded.industries[0].newgrf_persistent_storage_id, Some(0));
        assert_eq!(
            loaded.industries[0].newgrf_persistent_regs.get(&7),
            Some(&0xDEAD_BEEF)
        );
        assert_eq!(loaded.sav_persistent_storages.len(), 1);
        assert_eq!(loaded.sav_persistent_storages[0].storage[7], 0xDEAD_BEEF);

        let resaved = save_to_bytes_with(&loaded, SavContainer::Ottn).expect("resave");
        let resaved_chunks = crate::sav::chunks::parse_chunks(&resaved[8..]).expect("chunks");
        let psac = crate::sav::chunks::find_chunk(&resaved_chunks, "PSAC").expect("PSAC resaved");
        let rows = crate::sav::table::parse_table_chunk(&psac.body, false).expect("PSAC table");
        let values = match crate::sav::table::record_get(&rows[0].1, "storage") {
            Some(crate::sav::table::SlValue::List(values)) => values,
            other => panic!("storage ausente: {other:?}"),
        };
        assert_eq!(values[7].as_u64(), Some(u64::from(0xDEAD_BEEF_u32)));
    }

    #[test]
    fn ottn_roundtrip_hydrates_airport_psa_storage() {
        let mut state = tiny_state();
        let pos = TileCoord::new(12, 12);
        let mut tile = state.map.get(pos).expect("airport tile");
        tile.kind = TileKind::Station;
        tile.mapt = 0x50;
        tile.m2 = 0;
        tile.m6 = 1 << 3; // StationType::Airport
        state.map.set_tile(pos, tile).expect("set airport tile");

        let mut airport = Station::new_with_kind(pos, StopKind::Airport);
        airport.ottd_station_id = Some(0);
        airport.airport_tiles = vec![pos];
        airport.airport_newgrf_spec_id = Some(10);
        airport.newgrf_persistent_regs.insert(7, 0xCAFE_BABE);
        state.stations.push(airport);

        let bytes = save_to_bytes_with(&state, SavContainer::Ottn).expect("save");
        let chunks = crate::sav::chunks::parse_chunks(&bytes[8..]).expect("chunks");
        let stnn = crate::sav::chunks::find_chunk(&chunks, "STNN").expect("STNN");
        let rows = crate::sav::table::parse_table_chunk(&stnn.body, false).expect("STNN table");
        let normal = match crate::sav::table::record_get(&rows[0].1, "normal") {
            Some(crate::sav::table::SlValue::Structs(items)) => items.first().expect("normal"),
            other => panic!("normal ausente: {other:?}"),
        };
        assert_eq!(
            crate::sav::table::record_get(normal, "airport.psa")
                .and_then(crate::sav::table::SlValue::as_u64),
            Some(1)
        );
        let psac = crate::sav::chunks::find_chunk(&chunks, "PSAC").expect("PSAC");
        let psac_rows =
            crate::sav::table::parse_table_chunk(&psac.body, false).expect("PSAC table");
        let values = match crate::sav::table::record_get(&psac_rows[0].1, "storage") {
            Some(crate::sav::table::SlValue::List(values)) => values,
            other => panic!("storage ausente: {other:?}"),
        };
        assert_eq!(values[7].as_u64(), Some(u64::from(0xCAFE_BABEu32)));

        let loaded = GameState::from_sav_game(sav::load(&bytes).expect("load"));
        assert_eq!(loaded.stations.len(), 1);
        assert_eq!(loaded.stations[0].newgrf_persistent_storage_id, Some(0));
        assert_eq!(
            loaded.stations[0].newgrf_persistent_regs.get(&7),
            Some(&0xCAFE_BABE)
        );
        assert_eq!(loaded.sav_persistent_storages.len(), 1);

        let resaved = save_to_bytes_with(&loaded, SavContainer::Ottn).expect("resave");
        let resaved_game = sav::load(&resaved).expect("reload");
        assert_eq!(
            resaved_game.stations[0].airport_persistent_storage_id,
            Some(0)
        );
        let resaved_chunks = crate::sav::chunks::parse_chunks(&resaved[8..]).expect("chunks");
        let resaved_psac =
            crate::sav::chunks::find_chunk(&resaved_chunks, "PSAC").expect("PSAC resaved");
        let resaved_rows =
            crate::sav::table::parse_table_chunk(&resaved_psac.body, false).expect("PSAC table");
        let resaved_values = match crate::sav::table::record_get(&resaved_rows[0].1, "storage") {
            Some(crate::sav::table::SlValue::List(values)) => values,
            other => panic!("storage ausente: {other:?}"),
        };
        assert_eq!(resaved_values[7].as_u64(), Some(u64::from(0xCAFE_BABEu32)));
    }

    #[test]
    fn ottn_roundtrip_preserves_town_psa_list_refs() {
        let mut state = tiny_state();
        let town = Town {
            id: 3,
            pos: TileCoord::new(12, 12),
            name: "PSA Town".into(),
            ..Default::default()
        };
        state.towns.push(town);
        state.sav_town_persistent_storage_ids.insert(3, vec![2, 5]);
        state.sav_persistent_storages = vec![
            crate::sav::SavPersistentStorage {
                storage_id: 2,
                grfid: 0x1111_2222,
                storage: {
                    let mut storage = vec![0; 256];
                    storage[7] = 0xCAFE_BABE;
                    storage
                },
            },
            crate::sav::SavPersistentStorage {
                storage_id: 5,
                grfid: 0x3333_4444,
                storage: {
                    let mut storage = vec![0; 256];
                    storage[9] = 0x1020_3040;
                    storage
                },
            },
        ];

        let bytes = save_to_bytes_with(&state, SavContainer::Ottn).expect("save");
        let chunks = crate::sav::chunks::parse_chunks(&bytes[8..]).expect("chunks");
        let city = crate::sav::chunks::find_chunk(&chunks, "CITY").expect("CITY");
        let rows = crate::sav::table::parse_table_chunk(&city.body, false).expect("CITY table");
        let refs = match crate::sav::table::record_get(&rows[0].1, "psa_list") {
            Some(crate::sav::table::SlValue::List(values)) => values
                .iter()
                .map(crate::sav::table::SlValue::as_u64)
                .collect::<Option<Vec<_>>>(),
            other => panic!("psa_list ausente: {other:?}"),
        };
        assert_eq!(refs, Some(vec![3, 6]));

        let loaded = GameState::from_sav_game(sav::load(&bytes).expect("load"));
        assert_eq!(
            loaded.sav_town_persistent_storage_ids.get(&0),
            Some(&vec![2, 5])
        );
        assert_eq!(
            loaded.towns[0]
                .newgrf_persistent_regs
                .get(&0x1111_2222)
                .and_then(|regs| regs.get(&7)),
            Some(&0xCAFE_BABE)
        );
        assert_eq!(
            loaded.towns[0]
                .newgrf_persistent_regs
                .get(&0x3333_4444)
                .and_then(|regs| regs.get(&9)),
            Some(&0x1020_3040)
        );
        let resaved = save_to_bytes_with(&loaded, SavContainer::Ottn).expect("resave");
        let resaved_chunks = crate::sav::chunks::parse_chunks(&resaved[8..]).expect("chunks");
        let resaved_city = crate::sav::chunks::find_chunk(&resaved_chunks, "CITY").expect("CITY");
        let resaved_rows =
            crate::sav::table::parse_table_chunk(&resaved_city.body, false).expect("CITY table");
        let resaved_refs = match crate::sav::table::record_get(&resaved_rows[0].1, "psa_list") {
            Some(crate::sav::table::SlValue::List(values)) => values
                .iter()
                .map(crate::sav::table::SlValue::as_u64)
                .collect::<Option<Vec<_>>>(),
            other => panic!("psa_list ausente: {other:?}"),
        };
        assert_eq!(resaved_refs, Some(vec![3, 6]));
        let resaved_psac = crate::sav::chunks::find_chunk(&resaved_chunks, "PSAC").expect("PSAC");
        let resaved_psac_rows =
            crate::sav::table::parse_table_chunk(&resaved_psac.body, false).expect("PSAC table");
        assert_eq!(resaved_psac_rows.len(), 2);
        assert_eq!(
            match crate::sav::table::record_get(&resaved_psac_rows[0].1, "storage") {
                Some(crate::sav::table::SlValue::List(values)) => values[7].as_u64(),
                other => panic!("storage ausente: {other:?}"),
            },
            Some(u64::from(0xCAFE_BABEu32))
        );
        assert_eq!(
            match crate::sav::table::record_get(&resaved_psac_rows[1].1, "storage") {
                Some(crate::sav::table::SlValue::List(values)) => values[9].as_u64(),
                other => panic!("storage ausente: {other:?}"),
            },
            Some(u64::from(0x1020_3040u32))
        );
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn ottn_roundtrip_writes_native_city_metadata_and_histories() {
        let mut state = tiny_state();
        let town = Town {
            id: 4,
            pos: TileCoord::new(12, 12),
            name: "Native Town".into(),
            townnamegrfid: 0x1122_3344,
            townnametype: 0x20C0,
            townnameparts: 0x5566_7788,
            native_flags: 0x80,
            authority_ratings: vec![-100, 250, 500],
            have_ratings: 0x0005,
            goals: [11, 22, 33, 44, 55],
            supplied_cargo: vec![crate::town::TownSuppliedCargo {
                cargo: 2,
                history: vec![crate::town::TownSuppliedHistory {
                    production: 1200,
                    transported: 900,
                }],
            }],
            received_cargo: vec![
                crate::town::TownReceivedCargo {
                    old_max: 300,
                    new_max: 450,
                    old_act: 200,
                    new_act: 350,
                },
                crate::town::TownReceivedCargo::default(),
                crate::town::TownReceivedCargo::default(),
                crate::town::TownReceivedCargo::default(),
                crate::town::TownReceivedCargo::default(),
                crate::town::TownReceivedCargo::default(),
            ],
            time_until_rebuild: 17,
            grow_counter: 1234,
            growth_rate: 4321,
            fund_buildings_months: 2,
            road_build_months: 3,
            exclusivity: Some(crate::CompanyId(7)),
            exclusive_counter: 9,
            larger_town: true,
            layout: crate::town::TownLayout::Grid3x3,
            valid_history: 0x0102_0304_0506_0708,
            native_text: "script text".into(),
            statues: 0x0021,
            unwanted: vec![4, 0],
            is_growing: true,
            has_church: true,
            has_stadium: true,
            ..Default::default()
        };
        state.towns.push(town.clone());

        let bytes = save_to_bytes_with(&state, SavContainer::Ottn).expect("save");
        let chunks = crate::sav::chunks::parse_chunks(&bytes[8..]).expect("chunks");
        let city = crate::sav::chunks::find_chunk(&chunks, "CITY").expect("CITY");
        let rows = crate::sav::table::parse_table_chunk(&city.body, false).expect("CITY table");
        let record = &rows[0].1;
        assert!(crate::sav::table::record_get(record, "cache.population").is_none());
        assert_eq!(
            crate::sav::table::record_get(record, "townnamegrfid")
                .and_then(crate::sav::table::SlValue::as_u64),
            Some(u64::from(town.townnamegrfid))
        );
        assert_eq!(
            crate::sav::table::record_get(record, "townnametype")
                .and_then(crate::sav::table::SlValue::as_u64),
            Some(u64::from(town.townnametype))
        );
        assert_eq!(
            crate::sav::table::record_get(record, "townnameparts")
                .and_then(crate::sav::table::SlValue::as_u64),
            Some(u64::from(town.townnameparts))
        );
        assert_eq!(
            crate::sav::table::record_get(record, "flags")
                .and_then(crate::sav::table::SlValue::as_u64),
            Some(0x87)
        );
        assert!(matches!(
            crate::sav::table::record_get(record, "ratings"),
            Some(crate::sav::table::SlValue::List(values))
                if values.len() == crate::town::MAX_TOWN_AUTHORITY_COMPANIES
        ));
        assert!(matches!(
            crate::sav::table::record_get(record, "supplied"),
            Some(crate::sav::table::SlValue::Structs(values))
                if values.len() == 1
        ));
        assert!(matches!(
            crate::sav::table::record_get(record, "received"),
            Some(crate::sav::table::SlValue::Structs(values))
                if values.len() == crate::town::TOWN_GROWTH_EFFECT_COUNT + 1
        ));

        let loaded = GameState::from_sav_game(sav::load(&bytes).expect("load"));
        assert_eq!(loaded.towns[0].townnamegrfid, town.townnamegrfid);
        assert_eq!(loaded.towns[0].townnameparts, town.townnameparts);
        assert_eq!(loaded.towns[0].native_flags, 0x87);
        assert_eq!(loaded.towns[0].goals, town.goals);
        assert_eq!(loaded.towns[0].supplied_cargo, town.supplied_cargo);
        assert_eq!(loaded.towns[0].received_cargo, town.received_cargo);
        assert_eq!(loaded.towns[0].native_text, town.native_text);

        // Cambiar un escalar fuerza el encoder canónico y actualiza el byte de
        // flags sin reutilizar el cuerpo CITY tomado como passthrough.
        let mut changed = loaded;
        changed.towns[0].has_stadium = false;
        let changed_bytes = save_to_bytes_with(&changed, SavContainer::Ottn).expect("changed");
        let changed_chunks = crate::sav::chunks::parse_chunks(&changed_bytes[8..]).expect("chunks");
        let changed_city = crate::sav::chunks::find_chunk(&changed_chunks, "CITY").expect("CITY");
        let changed_rows =
            crate::sav::table::parse_table_chunk(&changed_city.body, false).expect("CITY table");
        assert_eq!(
            crate::sav::table::record_get(&changed_rows[0].1, "flags")
                .and_then(crate::sav::table::SlValue::as_u64),
            Some(0x83)
        );
    }

    #[test]
    fn ottn_roundtrip_preserves_industry_nested_histories() {
        let mut state = tiny_state();
        let pos = TileCoord::new(12, 12);
        let mut tile = state.map.get(pos).expect("industry tile");
        tile.kind = TileKind::Industry;
        state.map.set_tile(pos, tile).expect("set industry tile");
        state
            .industries
            .push(crate::Industry::new(pos, crate::IndustryKind::CoalMine));
        state.sav_industry_histories.push(crate::sav::SavIndustry {
            industry_id: 0,
            pos,
            width: 1,
            height: 1,
            neutral_station_id: None,
            industry_type: 0,
            random_colour: 0,
            counter: 0,
            selected_layout: 0,
            random: 0,
            last_prod_year: 0,
            was_cargo_delivered: false,
            control_flags: 0,
            exclusive_supplier: None,
            founder: None,
            construction_date: 0,
            construction_type: crate::industry::INDUSTRY_CONSTRUCTION_UNKNOWN,
            prod_level: crate::industry::PRODLEVEL_DEFAULT,
            valid_history: 0b11,
            persistent_storage_id: None,
            produced: vec![
                crate::sav::SavIndustryProducedCargo {
                    cargo_slot: 1,
                    waiting: 7,
                    rate: 5,
                    history: vec![crate::sav::SavIndustryProducedHistory {
                        production: 31,
                        transported: 17,
                    }],
                },
                crate::sav::SavIndustryProducedCargo {
                    // Slot 42 is a custom cargo.  The local catalog is empty, so
                    // it must survive as an opaque INDY row until the GRF is
                    // installed on a later load.
                    cargo_slot: 42,
                    waiting: 19,
                    rate: 11,
                    history: vec![crate::sav::SavIndustryProducedHistory {
                        production: 23,
                        transported: 7,
                    }],
                },
            ],
            accepted: vec![crate::sav::SavIndustryAcceptedCargo {
                // Accepted custom cargo follows the same passthrough rule.
                cargo_slot: 43,
                waiting: 13,
                last_accepted: 9001,
                accumulated_waiting: 77,
                history: vec![crate::sav::SavIndustryAcceptedHistory {
                    accepted: 5,
                    waiting: 9,
                }],
            }],
        });

        let bytes = save_to_bytes_with(&state, SavContainer::Ottn).expect("save");
        let sav_game = sav::load(&bytes).expect("load");
        assert_eq!(sav_game.industries[0].valid_history, 0b11);
        assert_eq!(sav_game.industries[0].produced[0].history.len(), 1);
        assert_eq!(sav_game.industries[0].produced[0].history[0].production, 31);

        let loaded = GameState::from_sav_game(sav_game);
        assert_eq!(loaded.sav_industry_histories.len(), 1);
        let resaved = save_to_bytes_with(&loaded, SavContainer::Ottn).expect("resave");
        let resaved_game = sav::load(&resaved).expect("reload");
        assert_eq!(resaved_game.industries[0].valid_history, 0b11);
        assert_eq!(
            resaved_game.industries[0].produced[0].history[0].transported,
            17
        );
        let custom_produced = resaved_game.industries[0]
            .produced
            .iter()
            .find(|entry| entry.cargo_slot == 42)
            .expect("custom produced cargo passthrough");
        assert_eq!(custom_produced.waiting, 19);
        assert_eq!(custom_produced.rate, 11);
        assert_eq!(custom_produced.history[0].production, 23);
        let custom_accepted = resaved_game.industries[0]
            .accepted
            .iter()
            .find(|entry| entry.cargo_slot == 43)
            .expect("custom accepted cargo passthrough");
        assert_eq!(custom_accepted.waiting, 13);
        assert_eq!(custom_accepted.last_accepted, 9001);
        assert_eq!(custom_accepted.accumulated_waiting, 77);
        assert_eq!(custom_accepted.history[0].accepted, 5);
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
        let mut state = tiny_state();
        state.random.state = [0x1020_3040, 0x5060_7080];
        let bytes = save_to_bytes_with(&state, SavContainer::Ottn).expect("save");
        assert!(bytes.starts_with(b"OTTN"));
        let sav_game = sav::load(&bytes).expect("load");
        assert_eq!(sav_game.version, EXPORT_SAVE_VERSION);
        assert_eq!(sav_game.money, Some(777_000));
        assert_eq!(sav_game.company_colour, Some(3));
        assert_eq!(sav_game.game_time.map(|t| t.tick), Some(12_345));
        assert_eq!(sav_game.random_state, Some([0x1020_3040, 0x5060_7080]));
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
        let loaded = GameState::from_sav_game(sav_game);
        assert_eq!(loaded.random.state, [0x1020_3040, 0x5060_7080]);
    }

    #[test]
    fn ottn_roundtrip_preserves_company_pool_money_and_colour() {
        let mut state = tiny_state();
        state.sync_active_from_mirrors();
        state.ensure_rival_transcargo();
        let expected_rival_liveries = {
            let rival = state
                .companies
                .iter_mut()
                .find(|company| company.is_ai)
                .expect("rival company");
            rival.economy.money = 456_789;
            rival.economy.loan = 123_000;
            rival.bankruptcy_months = 4;
            rival.set_colour(11);
            rival.president_name = Some("Ada Rival".into());
            rival.manager_face = 1 << 7;
            rival.manager_face_style = Some("modern".into());
            rival.liveries[1] = crate::CompanyLivery {
                in_use: crate::COMPANY_LIVERY_FLAG_PRIMARY,
                colour1: 7,
                colour2: 11,
            };
            rival.liveries[crate::COMPANY_LIVERY_SCHEME_COUNT - 1] = crate::CompanyLivery {
                in_use: crate::COMPANY_LIVERY_FLAG_SECONDARY,
                colour1: 11,
                colour2: 14,
            };
            rival.engine_renew = false;
            rival.engine_renew_months = -3;
            rival.engine_renew_money = 765_432;
            rival.renew_keep_length = true;
            rival.servint_ispercent = true;
            rival.servint_trains = 88;
            rival.servint_roadveh = 77;
            rival.servint_aircraft = 66;
            rival.servint_ships = 55;
            rival.effective_liveries()
        };

        let expected_player_liveries = state.companies[0].effective_liveries();

        let bytes = save_to_bytes_with(&state, SavContainer::Ottn).expect("save");
        let (payload, _) = crate::sav::container::decompress(&bytes).expect("decompress");
        let chunks = crate::sav::chunks::parse_chunks(&payload).expect("chunks");
        let plyr = crate::sav::chunks::find_chunk(&chunks, "PLYR").expect("PLYR chunk");
        assert_table_field_type(&plyr.body, 0x1A, "president_name");
        assert_table_field_type(&plyr.body, 6, "face");
        assert_table_field_type(&plyr.body, 0x1A, "face_style");
        assert_table_field_type(&plyr.body, 0x1B, "liveries");
        let sav_game = sav::load(&bytes).expect("load");
        assert_eq!(sav_game.companies.len(), 2);
        assert_eq!(sav_game.companies[1].money, 456_789);
        assert_eq!(sav_game.companies[1].loan, Some(123_000));
        assert_eq!(sav_game.companies[1].bankruptcy_months, Some(4));
        assert_eq!(sav_game.companies[1].colour, 11);
        assert_eq!(sav_game.companies[1].name.as_deref(), Some("TransCargo"));
        assert_eq!(
            sav_game.companies[1].president_name.as_deref(),
            Some("Ada Rival")
        );
        assert_eq!(sav_game.companies[1].manager_face, Some(1 << 7));
        assert_eq!(
            sav_game.companies[1].manager_face_style.as_deref(),
            Some("modern")
        );
        assert_eq!(sav_game.companies[1].is_ai, Some(true));
        assert_eq!(sav_game.companies[1].engine_renew, Some(false));
        assert_eq!(sav_game.companies[1].engine_renew_months, Some(-3));
        assert_eq!(sav_game.companies[1].engine_renew_money, Some(765_432));
        assert_eq!(sav_game.companies[1].renew_keep_length, Some(true));
        assert_eq!(sav_game.companies[1].servint_ispercent, Some(true));
        assert_eq!(sav_game.companies[1].servint_trains, Some(88));
        assert_eq!(sav_game.companies[1].servint_roadveh, Some(77));
        assert_eq!(sav_game.companies[1].servint_aircraft, Some(66));
        assert_eq!(sav_game.companies[1].servint_ships, Some(55));
        assert_eq!(sav_game.companies[0].liveries, expected_player_liveries);
        assert_eq!(sav_game.companies[1].liveries, expected_rival_liveries);

        let loaded = GameState::from_sav_game(sav_game);
        let loaded_rival = loaded
            .companies
            .iter()
            .find(|company| company.id.0 == 1)
            .expect("rival after load");
        assert_eq!(loaded_rival.economy.money, 456_789);
        assert_eq!(loaded_rival.economy.loan, 123_000);
        assert_eq!(loaded_rival.bankruptcy_months, 4);
        assert_eq!(loaded_rival.colour, 11);
        assert_eq!(loaded_rival.name, "TransCargo");
        assert_eq!(loaded_rival.president_name.as_deref(), Some("Ada Rival"));
        assert_eq!(loaded_rival.manager_face, 1 << 7);
        assert_eq!(loaded_rival.manager_face_style.as_deref(), Some("modern"));
        assert!(loaded_rival.is_ai);
        assert!(!loaded_rival.engine_renew);
        assert_eq!(loaded_rival.engine_renew_months, -3);
        assert_eq!(loaded_rival.engine_renew_money, 765_432);
        assert!(loaded_rival.renew_keep_length);
        assert!(loaded_rival.servint_ispercent);
        assert_eq!(loaded_rival.servint_trains, 88);
        assert_eq!(loaded_rival.servint_roadveh, 77);
        assert_eq!(loaded_rival.servint_aircraft, 66);
        assert_eq!(loaded_rival.servint_ships, 55);
        assert_eq!(loaded.companies[0].liveries, expected_player_liveries);
        assert_eq!(loaded_rival.liveries, expected_rival_liveries);
    }

    #[test]
    fn ottn_roundtrip_preserves_company_max_loan_override_and_global_sentinel() {
        let mut state = tiny_state();
        state.sync_active_from_mirrors();
        state.ensure_rival_transcargo();
        let rival = state
            .companies
            .iter_mut()
            .find(|company| company.is_ai)
            .expect("rival company");
        rival.economy.max_loan = 455_000;
        rival.economy.max_loan_override = Some(455_000);

        let bytes = save_to_bytes_with(&state, SavContainer::Ottn).expect("save");
        let (payload, _) = crate::sav::container::decompress(&bytes).expect("decompress");
        let chunks = crate::sav::chunks::parse_chunks(&payload).expect("chunks");
        let plyr = crate::sav::chunks::find_chunk(&chunks, "PLYR").expect("PLYR chunk");
        assert_table_field_type(&plyr.body, 7, "max_loan");

        let sav_game = sav::load(&bytes).expect("load");
        assert_eq!(
            sav_game.companies[0].max_loan,
            Some(crate::company::COMPANY_MAX_LOAN_DEFAULT)
        );
        assert_eq!(sav_game.companies[1].max_loan, Some(455_000));

        let mut loaded = GameState::from_sav_game(sav_game);
        let loaded_rival = loaded
            .companies
            .iter()
            .find(|company| company.id.0 == 1)
            .expect("rival after load");
        assert_eq!(loaded_rival.economy.max_loan_override, Some(455_000));
        assert_eq!(loaded_rival.economy.max_loan, 455_000);

        // Una recomputación global (por inflación/carga JSON) no puede borrar
        // el override individual que OpenTTD conserva en `Company::max_loan`.
        loaded.sync_scaled_max_loan();
        let loaded_rival = loaded
            .companies
            .iter()
            .find(|company| company.id.0 == 1)
            .expect("rival after max-loan sync");
        assert_eq!(loaded_rival.economy.max_loan, 455_000);
    }

    #[test]
    fn ottn_roundtrip_preserves_company_quarterly_history() {
        let mut state = tiny_state();
        let history = &mut state.companies[0].quarterly_economy;
        history.cur_income = 12_345;
        history.cur_expenses = 6_789;
        history.cur_deliveries = 16;
        history.cur_delivered_cargo = vec![7, 9];
        history.cur_company_value = 500_000;
        history.cur_performance_history = 321;
        history.samples = vec![
            crate::QuarterlyEconomyEntry {
                income: 100,
                expenses: 20,
                deliveries: 4,
                delivered_cargo: vec![1, 3],
                performance_history: 111,
                company_value: 400_000,
            },
            crate::QuarterlyEconomyEntry {
                income: 200,
                expenses: 30,
                deliveries: 6,
                delivered_cargo: vec![2, 4],
                performance_history: 222,
                company_value: 450_000,
            },
        ];

        let bytes = save_to_bytes_with(&state, SavContainer::Ottn).expect("save");
        let (payload, _) = crate::sav::container::decompress(&bytes).expect("decompress");
        let chunks = crate::sav::chunks::parse_chunks(&payload).expect("chunks");
        let plyr = crate::sav::chunks::find_chunk(&chunks, "PLYR").expect("PLYR chunk");
        assert_table_field_type(&plyr.body, 0x1B, "cur_economy");
        assert_table_field_type(&plyr.body, 0x1B, "old_economy");
        crate::sav::table::parse_table_chunk(&plyr.body, false).expect("parse PLYR");

        let sav_game = sav::load(&bytes).expect("load");
        let company = sav_game.companies.first().expect("company");
        let current = company.cur_economy.as_ref().expect("current economy");
        assert_eq!(current.income, 12_345);
        assert_eq!(current.expenses, -6_789);
        assert_eq!(current.company_value, 500_000);
        assert_eq!(&current.delivered_cargo[..2], &[7, 9]);
        assert_eq!(current.performance_history, 321);
        // OpenTTD serializa el más reciente primero.
        assert_eq!(company.old_economy.len(), 2);
        assert_eq!(company.old_economy[0].income, 200);
        assert_eq!(company.old_economy[0].expenses, -30);
        assert_eq!(company.old_economy[1].income, 100);

        let loaded = GameState::from_sav_game(sav_game);
        let loaded_history = &loaded.companies[0].quarterly_economy;
        assert_eq!(loaded_history.cur_income, 12_345);
        assert_eq!(loaded_history.cur_expenses, 6_789);
        assert_eq!(loaded_history.cur_deliveries, 16);
        assert_eq!(&loaded_history.cur_delivered_cargo[..2], &[7, 9]);
        assert_eq!(loaded_history.samples.len(), 2);
        assert_eq!(loaded_history.samples[0].income, 100);
        assert_eq!(loaded_history.samples[1].income, 200);
        assert_eq!(loaded_history.samples[1].expenses, 30);
        assert_eq!(&loaded_history.samples[1].delivered_cargo[..2], &[2, 4]);
        assert_eq!(
            loaded_history.samples[1].delivered_cargo.len(),
            crate::economy_quarterly::QUARTERLY_CARGO_SLOTS
        );
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
        ship.ship_state = 16;
        ship.ship_track = crate::ship_movement::TRACK_LEFT;
        ship.ship_rotation = 7;
        ship.ship_path = vec![3, 11, 27];
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
        let imported_ship = sav_game
            .vehicles
            .iter()
            .find(|v| v.kind == sav::SavVehicleKind::Ship)
            .expect("ship importado");
        assert_eq!(imported_ship.ship_state, 16);
        assert_eq!(imported_ship.ship_rotation, 7);
        assert_eq!(imported_ship.ship_path, vec![3, 11, 27]);
        assert_eq!(
            imported_ship.ship_track,
            crate::ship_movement::TRACK_LEFT,
            "TrackBits debe conservar la proyección usada por el controlador"
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
    #[allow(clippy::too_many_lines)]
    fn export_mvp_rich_emits_indy_road_vehs_and_stations() {
        use crate::sav::chunks::{find_chunk, parse_chunks};
        use crate::sav::table::{SlValue, parse_table_chunk, record_get};

        let mut state = mvp_rich_state();
        state.sync_active_from_mirrors();
        state.companies[0].president_name = Some("Ada Lovelace".into());
        state.companies[0].manager_face = 1 << 7;
        state.companies[0].manager_face_style = Some("modern".into());
        state.companies[0].reset_liveries();
        // Ejercita el valor distinto del centinela global de `PLYR.max_loan`.
        // El smoke OpenTTD opcional re-guarda este valor para acreditar tanto
        // el wire i64 como la semántica de override por compañía.
        state.economy.max_loan = 450_000;
        state.economy.max_loan_override = Some(450_000);
        state.sync_active_from_mirrors();
        let custom_bus_livery = crate::CompanyLivery {
            in_use: crate::COMPANY_LIVERY_FLAG_PRIMARY | crate::COMPANY_LIVERY_FLAG_SECONDARY,
            colour1: 7,
            colour2: 11,
        };
        // La salida de smoke lleva una librea no trivial: el round-trip con
        // OpenTTD acredita que no se limita a escribir 23 defaults.
        state.companies[0].liveries[14] = custom_bus_livery;
        let bus = state
            .vehicles
            .iter_mut()
            .find(|vehicle| vehicle.kind == crate::VehicleKind::Bus)
            .expect("bus MVP");
        bus.unit_number = 314;
        bus.native_sprite_num = 7;
        bus.acceleration = 13;
        bus.refit_capacity = 29;
        bus.dest = TileCoord::new(14, 16);
        bus.progress = 173;
        bus.motion_counter = 0x1234_5678;
        bus.cur_speed = 41;
        bus.subspeed = 99;
        bus.economy_age_days = 777;
        bus.last_service_newgrf_day = 1_234;
        bus.depot_unbunching_last_departure = 88_000;
        bus.depot_unbunching_next_departure = 99_000;
        bus.round_trip_time = 12_345;
        bus.cargo = 17;
        bus.capacity = 31;
        bus.cargo_packets.action_counts = [3, 5, 7, 2];
        bus.road_state = 8;
        bus.frame = 6;
        bus.blocked_ctr = 19;
        bus.overtaking = crate::road_movement::rvsb::RVSB_DRIVE_SIDE;
        bus.overtaking_ctr = 7;
        bus.crashed_ctr = 23;
        bus.reverse_ctr = 3;
        bus.road_gv_flags = 0x4567;
        bus.road_path = vec![
            crate::vehicle::RoadPathEntry {
                trackdir: 9,
                tile: 1234,
            },
            crate::vehicle::RoadPathEntry {
                trackdir: 17,
                tile: 5678,
            },
        ];
        let bytes = save_to_bytes_with(&state, SavContainer::Ottn).expect("save");
        let payload = &bytes[8..];
        let chunks = parse_chunks(payload).expect("chunks");
        assert!(find_chunk(&chunks, "INDY").is_some());
        assert!(find_chunk(&chunks, "VEHS").is_some());
        assert!(find_chunk(&chunks, "STNN").is_some());

        let sav_game = sav::load(&bytes).expect("load rust");
        assert!(sav_game.stations.len() >= 2);
        assert_eq!(sav_game.industries.len(), 1);
        assert_eq!(
            sav_game.companies[0].president_name.as_deref(),
            Some("Ada Lovelace")
        );
        assert_eq!(sav_game.companies[0].manager_face, Some(1 << 7));
        assert_eq!(
            sav_game.companies[0].manager_face_style.as_deref(),
            Some("modern")
        );
        assert_eq!(sav_game.companies[0].max_loan, Some(450_000));
        assert_eq!(sav_game.vehicles.len(), 2, "tren + bus");
        assert_eq!(sav_game.companies[0].liveries[14], custom_bus_livery);
        let saved_bus = sav_game
            .vehicles
            .iter()
            .find(|v| v.kind == sav::SavVehicleKind::RoadVehicle)
            .expect("bus en VEHS");
        assert_eq!(saved_bus.progress, 173);
        assert_eq!(saved_bus.unit_number, 314);
        assert_eq!(saved_bus.sprite_num, 7);
        assert_eq!(saved_bus.acceleration, 13);
        assert_eq!(saved_bus.refit_capacity, 29);
        assert_eq!(saved_bus.dest, TileCoord::new(14, 16));
        assert_eq!(saved_bus.motion_counter, 0x1234_5678);
        assert_eq!(saved_bus.cur_speed, 41);
        assert_eq!(saved_bus.subspeed, 99);
        assert_eq!(saved_bus.economy_age_days, 777);
        assert_eq!(
            saved_bus.date_of_last_service_newgrf,
            crate::sav::write::vehicles::packed_calendar_date_from_day_index(1_234)
        );
        assert_eq!(saved_bus.depot_unbunching_last_departure, 88_000);
        assert_eq!(saved_bus.depot_unbunching_next_departure, 99_000);
        assert_eq!(saved_bus.round_trip_time, 12_345);
        assert_eq!(saved_bus.cargo, 17);
        assert_eq!(saved_bus.cargo_capacity, 31);
        assert_eq!(saved_bus.cargo_action_counts, [3, 5, 7, 2]);
        assert_eq!(saved_bus.road_state, 8);
        assert_eq!(saved_bus.road_frame, 6);
        assert_eq!(saved_bus.road_blocked_ctr, 19);
        assert_eq!(
            saved_bus.road_overtaking,
            crate::road_movement::rvsb::RVSB_DRIVE_SIDE
        );
        assert_eq!(saved_bus.road_overtaking_ctr, 7);
        assert_eq!(saved_bus.road_crashed_ctr, 23);
        assert_eq!(saved_bus.road_reverse_ctr, 3);
        let imported = GameState::from_sav_game(sav_game);
        let imported_bus = imported
            .vehicles
            .iter()
            .find(|vehicle| vehicle.kind == crate::VehicleKind::Bus)
            .expect("bus importado");
        assert_eq!(imported_bus.progress, 173);
        assert_eq!(imported_bus.unit_number, 314);
        assert_eq!(imported_bus.native_sprite_num, 7);
        assert_eq!(imported_bus.acceleration, 13);
        assert_eq!(imported_bus.refit_capacity, 29);
        // La orden de parada vuelve a proyectar el destino operativo sobre la
        // estación; el `dest_tile` crudo ya se verificó en `saved_bus`.
        assert_eq!(imported_bus.dest, TileCoord::new(17, 16));
        assert_eq!(imported_bus.motion_counter, 0x1234_5678);
        assert_eq!(imported_bus.cur_speed, 41);
        assert_eq!(imported_bus.subspeed, 99);
        assert_eq!(imported_bus.economy_age_days, 777);
        assert_eq!(imported_bus.last_service_newgrf_day, 1_234);
        assert_eq!(imported_bus.depot_unbunching_last_departure, 88_000);
        assert_eq!(imported_bus.depot_unbunching_next_departure, 99_000);
        assert_eq!(imported_bus.round_trip_time, 12_345);
        assert_eq!(imported_bus.cargo, 17);
        assert_eq!(imported_bus.capacity, 31);
        assert_eq!(imported_bus.cargo_packets.action_counts, [3, 5, 7, 2]);
        assert_eq!(imported_bus.road_state, 8);
        assert_eq!(imported_bus.frame, 6);
        assert_eq!(imported_bus.blocked_ctr, 19);
        assert_eq!(
            imported_bus.overtaking,
            crate::road_movement::rvsb::RVSB_DRIVE_SIDE
        );
        assert_eq!(imported_bus.overtaking_ctr, 7);
        assert_eq!(imported_bus.crashed_ctr, 23);
        assert_eq!(imported_bus.reverse_ctr, 3);
        assert_eq!(imported_bus.road_gv_flags, 0x4567);
        assert_eq!(
            imported_bus.road_path,
            vec![
                crate::vehicle::RoadPathEntry {
                    trackdir: 9,
                    tile: 1234,
                },
                crate::vehicle::RoadPathEntry {
                    trackdir: 17,
                    tile: 5678,
                },
            ]
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
        assert_eq!(record_get(rv, "state").and_then(SlValue::as_u64), Some(8));
        assert_eq!(record_get(rv, "frame").and_then(SlValue::as_u64), Some(6));
        assert_eq!(
            record_get(rv, "blocked_ctr").and_then(SlValue::as_u64),
            Some(19)
        );
        let action_counts = match record_get(common, "cargo.action_counts") {
            Some(SlValue::List(values)) => values,
            other => panic!("cargo.action_counts ausente: {other:?}"),
        };
        assert_eq!(
            action_counts
                .iter()
                .map(SlValue::as_u64)
                .collect::<Option<Vec<_>>>(),
            Some(vec![3, 5, 7, 2])
        );
        assert_eq!(
            record_get(rv, "gv_flags").and_then(SlValue::as_u64),
            Some(0x4567)
        );
        let path = match record_get(rv, "path") {
            Some(SlValue::Structs(items)) => items,
            other => panic!("path ausente: {other:?}"),
        };
        assert_eq!(path.len(), 2);
        assert_eq!(
            record_get(&path[0], "trackdir").and_then(SlValue::as_u64),
            Some(9)
        );
        assert_eq!(
            record_get(&path[0], "tile").and_then(SlValue::as_u64),
            Some(1234)
        );
        assert_eq!(
            record_get(&path[1], "trackdir").and_then(SlValue::as_u64),
            Some(17)
        );
        assert_eq!(
            record_get(&path[1], "tile").and_then(SlValue::as_u64),
            Some(5678)
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
        let mut state = tiny_state();
        state.economy.loan = 50_000;
        state.companies[0].bankruptcy_months = 2;
        state.companies[0].manager_face_style = Some("modern".into());
        state.companies[0].quarterly_economy.cur_income = 900;
        state.companies[0].quarterly_economy.cur_expenses = 400;
        state.companies[0].quarterly_economy.cur_deliveries = 7;
        state.companies[0].quarterly_economy.cur_delivered_cargo = vec![3, 4];
        state.companies[0]
            .quarterly_economy
            .samples
            .push(crate::QuarterlyEconomyEntry {
                income: 1_200,
                expenses: 500,
                deliveries: 9,
                delivered_cargo: vec![4, 5],
                performance_history: 456,
                company_value: 800_000,
            });
        let names = exported_chunk_names(&state).expect("chunks");
        assert!(names.iter().any(|n| n == "CITY"), "{names:?}");
        let bytes = save_to_bytes_with(&state, SavContainer::Ottn).expect("save");
        let sav_game = sav::load(&bytes).expect("load");
        assert!(
            !sav_game.towns.is_empty(),
            "OpenTTD exige ≥1 municipio; el export sintético debe roundtrippear"
        );
        assert_eq!(
            sav_game.companies[0].manager_face_style.as_deref(),
            Some("modern")
        );
        assert_eq!(sav_game.companies[0].loan, Some(50_000));
        assert_eq!(sav_game.companies[0].bankruptcy_months, Some(2));
        assert_eq!(
            sav_game.companies[0]
                .cur_economy
                .as_ref()
                .map(|entry| entry.income),
            Some(900)
        );
        assert_eq!(
            sav_game.companies[0]
                .cur_economy
                .as_ref()
                .map(|entry| entry.expenses),
            Some(-400)
        );
        assert_eq!(sav_game.companies[0].old_economy.len(), 1);
        assert_eq!(
            sav_game.companies[0].old_economy[0].performance_history,
            456
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
        let (payload, _) = crate::sav::container::decompress(&bytes).expect("payload");
        let original_chunks = crate::sav::chunks::parse_chunks(&payload).expect("chunks");
        let original_lgrp = crate::sav::chunks::find_chunk(&original_chunks, "LGRP")
            .expect("LGRP original")
            .body
            .clone();
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
        let resaved = save_to_bytes_with(&loaded, SavContainer::Ottn).expect("resave");
        let (resaved_payload, _) = crate::sav::container::decompress(&resaved).expect("payload");
        let resaved_chunks = crate::sav::chunks::parse_chunks(&resaved_payload).expect("chunks");
        let resaved_lgrp =
            crate::sav::chunks::find_chunk(&resaved_chunks, "LGRP").expect("LGRP resaved");
        assert_eq!(resaved_lgrp.body, original_lgrp);
    }

    #[test]
    fn export_roundtrip_preserves_station_and_vehicle_cargo_packets() {
        use crate::cargo::CargoType;
        use crate::cargo_packet::CargoPacket;

        let mut state = tiny_state();
        let source = TileCoord::new(28, 39);
        let destination = TileCoord::new(40, 40);
        let mut source_station = Station::new_with_kind(source, StopKind::RailStation);
        source_station.cargo_packets.push(
            CargoPacket::new(CargoType::Coal, 7, source)
                .with_first_station(source)
                .with_next_hop(Some(destination)),
        );
        let destination_station = Station::new_with_kind(destination, StopKind::RailStation);
        state.stations = vec![source_station, destination_station];

        let vehicle_pos = TileCoord::new(10, 20);
        let mut train = Vehicle::new(0, VehicleKind::Train, vehicle_pos, vehicle_pos);
        train.cargo_type = Some(CargoType::Coal);
        train.cargo_packets.push(
            CargoPacket::new(CargoType::Coal, 9, source)
                .with_first_station(source)
                .with_next_hop(Some(destination)),
        );
        state.vehicles = vec![train];

        let bytes = save_to_bytes_with(&state, SavContainer::Ottn).expect("save");
        let (payload, _) = crate::sav::container::decompress(&bytes).expect("payload");
        let original_chunks = crate::sav::chunks::parse_chunks(&payload).expect("chunks");
        let original_capa = crate::sav::chunks::find_chunk(&original_chunks, "CAPA")
            .expect("CAPA original")
            .body
            .clone();
        let sav_game = sav::load(&bytes).expect("load");
        assert_eq!(sav_game.cargo_packets.len(), 2);
        assert_eq!(sav_game.stations[0].cargo[0].packet_ids.len(), 1);
        assert_eq!(sav_game.vehicles[0].cargo_packet_ids.len(), 1);

        let loaded = GameState::from_sav_game(sav_game);
        assert_eq!(
            loaded.stations[0].cargo_packets.total_of(CargoType::Coal),
            7
        );
        assert_eq!(loaded.vehicles[0].cargo_packets.total(), 9);
        assert_eq!(loaded.vehicles[0].cargo_source, Some(source));
        assert_eq!(
            loaded.vehicles[0].cargo_packets.packets[0].next_hop,
            Some(destination)
        );
        let passthrough = loaded
            .sav_table_passthrough
            .as_ref()
            .expect("CAPA passthrough after import");
        assert_eq!(
            passthrough
                .capa_chunk
                .as_ref()
                .expect("CAPA passthrough")
                .body,
            original_capa
        );
        let resaved = save_to_bytes_with(&loaded, SavContainer::Ottn).expect("resave");
        let (resaved_payload, _) = crate::sav::container::decompress(&resaved).expect("payload");
        let resaved_chunks = crate::sav::chunks::parse_chunks(&resaved_payload).expect("chunks");
        assert_eq!(
            crate::sav::chunks::find_chunk(&resaved_chunks, "CAPA")
                .expect("CAPA resaved")
                .body,
            original_capa
        );
    }

    #[test]
    fn export_roundtrip_preserves_custom_vehicle_cargo_id() {
        use crate::cargo::CargoType;
        use crate::cargo_packet::CargoPacket;

        let mut state = mvp_train_state();
        let source = TileCoord::new(28, 39);
        let train = state.vehicles.first_mut().expect("train MVP");
        train.cargo_type = Some(CargoType::Custom(11));
        train
            .cargo_packets
            .push(CargoPacket::new(CargoType::Custom(11), 13, source));

        let bytes = save_to_bytes_with(&state, SavContainer::Ottn).expect("save");
        let sav_game = sav::load(&bytes).expect("load");
        assert_eq!(sav_game.version, EXPORT_SAVE_VERSION);
        assert_eq!(sav_game.vehicles[0].cargo_type, 42);
        assert_eq!(sav_game.vehicles[0].cargo_packet_ids.len(), 1);

        let loaded = GameState::from_sav_game(sav_game);
        let train = &loaded.vehicles[0];
        assert_eq!(train.cargo_type, Some(CargoType::Custom(11)));
        assert_eq!(train.cargo_packets.total(), 13);
        assert_eq!(
            train.cargo_packets.primary_type(),
            Some(CargoType::Custom(11))
        );
    }

    #[test]
    fn export_roundtrip_preserves_object_pool_instances() {
        let mut state = tiny_state();
        state.objects.push(crate::sav::SavObject {
            object_id: 4,
            tile: TileCoord::new(11, 12),
            width: 2,
            height: 1,
            town: 3,
            build_date: 77,
            colour: 6,
            view: 1,
            object_type: 512,
        });
        state.object_mappings.push(crate::sav::SavObjectMapping {
            object_type: 512,
            grfid: 0x4f42_0001,
            entity_id: 3,
            substitute_id: 512,
        });

        let bytes = save_to_bytes_with(&state, SavContainer::Ottn).expect("save");
        let names = exported_chunk_names(&state).expect("chunk names");
        assert!(names.iter().any(|name| name == "OBJS"), "{names:?}");
        assert!(names.iter().any(|name| name == "OBID"), "{names:?}");
        let sav_game = sav::load(&bytes).expect("load");
        assert_eq!(sav_game.objects, state.objects);
        assert_eq!(sav_game.object_mappings, state.object_mappings);

        let loaded = GameState::from_sav_game(sav_game);
        assert_eq!(loaded.objects, state.objects);
        assert_eq!(loaded.object_mappings, state.object_mappings);
        assert!(!loaded.sav_objects_dirty);
        assert!(!loaded.sav_object_mappings_dirty);
    }
}
