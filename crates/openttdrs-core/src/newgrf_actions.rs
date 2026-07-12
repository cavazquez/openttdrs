//! Inspección parse-only de acciones `NewGRF` (Action0–14) y apply mínimo.
//!
//! El walker cuenta acciones sin aplicar. Action0 features registradas:
//! - `RoadTypes` (0x12) → `GameState.road_type_catalog`
//! - `Stations` (0x04) → `station_class_catalog` / `station_spec_catalog`

use std::path::Path;

use crate::GameState;
use crate::engine::{EngineDef, next_free_engine_id, vanilla_engine_catalog};
use crate::newgrf_config::{GrfContainerVersion, GrfScanError, parse_grf_container};
use crate::road_type::{
    RoadTramType, RoadType, RoadTypeDef, next_free_road_type_id, vanilla_road_type_catalog,
};
use crate::station_class::{
    StationClassDef, StationClassId, StationSpecDef, StationSpecId, next_free_station_class_id,
    next_free_station_spec_id, vanilla_station_class_catalog, vanilla_station_spec_catalog,
};
use crate::vehicle::VehicleKind;

/// Feature Action0: `Trains` (`OpenTTD` `GSF_TRAINS`).
pub const ACTION0_FEATURE_TRAINS: u8 = 0x00;
/// Feature Action0: `Stations` (`OpenTTD` `GSF_STATIONS`).
pub const ACTION0_FEATURE_STATIONS: u8 = 0x04;
/// Feature Action0: `RoadTypes` (`OpenTTD` `GSF_ROADTYPES`).
pub const ACTION0_FEATURE_ROADTYPES: u8 = 0x12;

/// Prop etiqueta 4 chars (`RoadTypes` short / Stations class label).
const PROP_LABEL: u8 = 0x08;
/// Prop flags (`RoadTypes`: bit0 = tram).
const PROP_FLAGS: u8 = 0x09;
/// Stations: platforms disallowed bitmask.
const PROP_STATION_DISALLOWED_PLATFORMS: u8 = 0x0A;
/// Stations: lengths disallowed bitmask.
const PROP_STATION_DISALLOWED_LENGTHS: u8 = 0x0B;
/// Stations: short label 4 chars del spec.
const PROP_STATION_SPEC_SHORT: u8 = 0x0C;
/// Prop año introducción (uint16 LE).
const PROP_INTRO_YEAR: u8 = 0x16;
/// Extensión local: nombre C-string (tests / GRFs propios).
const PROP_NAME_CSTRING: u8 = 0xFE;

/// Resumen de un bloque Action5 para Inspeccionar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Action5SlotSummary {
    pub type_id: u8,
    pub num_sprites: u8,
    pub offset: u16,
    pub preview_wh: Option<(u16, u16)>,
}

/// Informe de inspección de un `.grf` (sin aplicar).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GrfInspectReport {
    pub container: Option<GrfContainerVersion>,
    pub action_counts: [u32; 16],
    pub action0_features: Vec<u8>,
    pub action5_slots: Vec<Action5SlotSummary>,
    pub pseudo_sprites: u32,
    pub real_sprites: u32,
    pub warnings: Vec<String>,
}

impl GrfInspectReport {
    #[must_use]
    pub fn format_summary(&self) -> String {
        let mut lines = Vec::new();
        match self.container {
            Some(GrfContainerVersion::V1) => lines.push("Contenedor: v1".into()),
            Some(GrfContainerVersion::V2) => lines.push("Contenedor: v2".into()),
            None => lines.push("Contenedor: ?".into()),
        }
        lines.push(format!(
            "Pseudo: {} · reales: {}",
            self.pseudo_sprites, self.real_sprites
        ));
        let mut hist = Vec::new();
        for (action, count) in self.action_counts.iter().enumerate() {
            if *count > 0 {
                hist.push(format!("A{action:X}={count}"));
            }
        }
        if hist.is_empty() {
            lines.push("Acciones: (ninguna)".into());
        } else {
            lines.push(format!("Acciones: {}", hist.join(" ")));
        }
        if !self.action0_features.is_empty() {
            let feats: Vec<_> = self
                .action0_features
                .iter()
                .map(|f| format!("0x{f:02X}"))
                .collect();
            lines.push(format!("Action0 features: {}", feats.join(", ")));
        }
        if !self.action5_slots.is_empty() {
            let slots: Vec<_> = self
                .action5_slots
                .iter()
                .map(|s| {
                    let name = crate::newgrf_sprites::action5_type_name(s.type_id);
                    let preview = s
                        .preview_wh
                        .map(|(w, h)| format!(" {w}×{h}"))
                        .unwrap_or_default();
                    format!(
                        "0x{:02X}×{} @{} ({name}){preview}",
                        s.type_id, s.num_sprites, s.offset
                    )
                })
                .collect();
            lines.push(format!("Action5: {}", slots.join("; ")));
        }
        for w in &self.warnings {
            lines.push(format!("! {w}"));
        }
        lines.join("\n")
    }
}

/// Cabecera Action0 parseada (sin props).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Action0Header {
    pub feature: u8,
    pub num_props: u8,
    pub num_ids: u8,
}

/// Metadatos `RoadTypes` leídos de un Action0 (antes de asignar ID global).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedRoadTypeMeta {
    pub class: RoadTramType,
    pub label: String,
    pub short_label: String,
    pub intro_year: u16,
}

/// Metadatos `Stations` leídos de un Action0 (antes de asignar IDs).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedStationMeta {
    pub class_short_label: String,
    pub class_label: String,
    pub short_label: String,
    pub label: String,
    pub disallowed_platforms: u8,
    pub disallowed_lengths: u8,
}

/// Metadatos `Trains` Action0 (antes de asignar ID ≥1000).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedTrainMeta {
    pub name: String,
    pub intro_year: u16,
    pub max_speed: u16,
    pub power_hp: u32,
}

/// Inspecciona bytes de un `.grf` (parse-only).
///
/// # Errors
///
/// Contenedor inválido / demasiado corto.
pub fn inspect_grf_bytes(data: &[u8]) -> Result<GrfInspectReport, GrfScanError> {
    let (container, section) = parse_grf_container(data)?;
    let mut report = GrfInspectReport {
        container: Some(container),
        ..Default::default()
    };
    walk_data_section(section, container, &mut report);
    if let Ok(blocks) = crate::newgrf_sprites::collect_action5_blocks(data) {
        report.action5_slots = blocks
            .into_iter()
            .map(|b| Action5SlotSummary {
                type_id: b.type_id,
                num_sprites: b.num_sprites,
                offset: b.offset,
                preview_wh: b.first_preview.as_ref().map(|s| (s.width, s.height)),
            })
            .collect();
    }
    Ok(report)
}

/// # Errors
///
/// E/S o contenedor inválido.
pub fn inspect_grf_file(path: &Path) -> Result<GrfInspectReport, GrfScanError> {
    let data = std::fs::read(path).map_err(|e| GrfScanError::Io(e.to_string()))?;
    inspect_grf_bytes(&data)
}

fn walk_data_section(
    data_section: &[u8],
    container: GrfContainerVersion,
    report: &mut GrfInspectReport,
) {
    let mut i = 0usize;
    while i < data_section.len() {
        let (size, header) = match container {
            GrfContainerVersion::V2 => {
                if i + 5 > data_section.len() {
                    report
                        .warnings
                        .push("sección truncada (cabecera v2)".into());
                    break;
                }
                let size = u32::from_le_bytes([
                    data_section[i],
                    data_section[i + 1],
                    data_section[i + 2],
                    data_section[i + 3],
                ]) as usize;
                if size == 0 {
                    break;
                }
                (size, 5usize)
            }
            GrfContainerVersion::V1 => {
                if i + 3 > data_section.len() {
                    report
                        .warnings
                        .push("sección truncada (cabecera v1)".into());
                    break;
                }
                let size = u16::from_le_bytes([data_section[i], data_section[i + 1]]) as usize;
                if size == 0 {
                    break;
                }
                (size, 3usize)
            }
        };
        let info = data_section[i + header - 1];
        let payload_start = i + header;
        if info == 0xFF {
            let end = payload_start + size;
            if end > data_section.len() {
                report.warnings.push("pseudo-sprite truncado".into());
                break;
            }
            let payload = &data_section[payload_start..end];
            report.pseudo_sprites = report.pseudo_sprites.saturating_add(1);
            process_pseudo_payload(payload, report);
            i = end;
            continue;
        }
        let next = match container {
            GrfContainerVersion::V1 => i + 2 + size,
            GrfContainerVersion::V2 => payload_start + size,
        };
        if next > data_section.len() {
            report.warnings.push("sprite real truncado".into());
            break;
        }
        report.real_sprites = report.real_sprites.saturating_add(1);
        i = next;
    }
}

fn process_pseudo_payload(payload: &[u8], report: &mut GrfInspectReport) {
    let Some(&action) = payload.first() else {
        report.warnings.push("pseudo vacío".into());
        return;
    };
    if action == 0xFF {
        return;
    }
    if action <= 0x0F {
        report.action_counts[usize::from(action)] =
            report.action_counts[usize::from(action)].saturating_add(1);
    }
    if action == 0x00 {
        match parse_action0_header(payload) {
            Some(h) => {
                if !report.action0_features.contains(&h.feature) {
                    report.action0_features.push(h.feature);
                }
            }
            None => report
                .warnings
                .push("Action0 con cabecera incompleta".into()),
        }
    }
}

#[must_use]
pub fn parse_action0_header(payload: &[u8]) -> Option<Action0Header> {
    if payload.len() < 4 || payload[0] != 0x00 {
        return None;
    }
    Some(Action0Header {
        feature: payload[1],
        num_props: payload[2],
        num_ids: payload[3],
    })
}

pub fn for_each_pseudo_payload(
    data: &[u8],
    mut visit: impl FnMut(&[u8]),
) -> Result<(), GrfScanError> {
    let (container, section) = parse_grf_container(data)?;
    let mut i = 0usize;
    while i < section.len() {
        let (size, header) = match container {
            GrfContainerVersion::V2 => {
                if i + 5 > section.len() {
                    break;
                }
                let size = u32::from_le_bytes([
                    section[i],
                    section[i + 1],
                    section[i + 2],
                    section[i + 3],
                ]) as usize;
                if size == 0 {
                    break;
                }
                (size, 5usize)
            }
            GrfContainerVersion::V1 => {
                if i + 3 > section.len() {
                    break;
                }
                let size = u16::from_le_bytes([section[i], section[i + 1]]) as usize;
                if size == 0 {
                    break;
                }
                (size, 3usize)
            }
        };
        let info = section[i + header - 1];
        let payload_start = i + header;
        if info == 0xFF {
            let end = payload_start + size;
            if end > section.len() {
                break;
            }
            visit(&section[payload_start..end]);
            i = end;
            continue;
        }
        let next = match container {
            GrfContainerVersion::V1 => i + 2 + size,
            GrfContainerVersion::V2 => payload_start + size,
        };
        if next > section.len() {
            break;
        }
        i = next;
    }
    Ok(())
}

#[must_use]
pub fn parse_action0_roadtype_meta(payload: &[u8]) -> Option<ParsedRoadTypeMeta> {
    let header = parse_action0_header(payload)?;
    if header.feature != ACTION0_FEATURE_ROADTYPES || header.num_ids == 0 {
        return None;
    }
    if payload.len() < 5 {
        return None;
    }
    let mut i = 5usize;
    let mut short_label = String::from("NGRF");
    let mut label = String::new();
    let mut intro_year = 0u16;
    let mut is_tram = false;
    for _ in 0..header.num_props {
        if i >= payload.len() {
            break;
        }
        let prop = payload[i];
        i += 1;
        match prop {
            PROP_LABEL => {
                if i + 4 > payload.len() {
                    break;
                }
                short_label = String::from_utf8_lossy(&payload[i..i + 4])
                    .trim_end_matches('\0')
                    .trim()
                    .to_string();
                if short_label.is_empty() {
                    short_label = "NGRF".into();
                }
                i += 4;
            }
            PROP_FLAGS => {
                if i >= payload.len() {
                    break;
                }
                is_tram = payload[i] & 0x01 != 0;
                i += 1;
            }
            PROP_INTRO_YEAR => {
                if i + 2 > payload.len() {
                    break;
                }
                intro_year = u16::from_le_bytes([payload[i], payload[i + 1]]);
                i += 2;
            }
            PROP_NAME_CSTRING => {
                let Some(nul) = payload[i..].iter().position(|&b| b == 0) else {
                    break;
                };
                label = String::from_utf8_lossy(&payload[i..i + nul]).to_string();
                i += nul + 1;
            }
            _ => break,
        }
    }
    if label.is_empty() {
        label.clone_from(&short_label);
    }
    Some(ParsedRoadTypeMeta {
        class: if is_tram {
            RoadTramType::Tram
        } else {
            RoadTramType::Road
        },
        label,
        short_label,
        intro_year,
    })
}

#[must_use]
pub fn collect_roadtype_metas_from_grf(data: &[u8]) -> Vec<ParsedRoadTypeMeta> {
    let mut out = Vec::new();
    let _ = for_each_pseudo_payload(data, |payload| {
        if let Some(meta) = parse_action0_roadtype_meta(payload) {
            out.push(meta);
        }
    });
    out
}

/// Reconstruye el catálogo road/tram desde el stack `enabled` + vanilla.
pub fn apply_newgrf_road_types(state: &mut GameState, search_dirs: &[&Path]) {
    let mut catalog = vanilla_road_type_catalog();
    let stack = state.newgrf_stack.clone();
    for entry in &stack {
        if !entry.enabled {
            continue;
        }
        let Some(path) = search_dirs
            .iter()
            .map(|d| d.join(&entry.filename))
            .find(|p| p.is_file())
        else {
            continue;
        };
        let Ok(data) = std::fs::read(&path) else {
            continue;
        };
        let gfx =
            crate::newgrf_sprites::collect_roadtype_sprite_graphics(&data).unwrap_or_default();
        for (local_idx, meta) in collect_roadtype_metas_from_grf(&data)
            .into_iter()
            .enumerate()
        {
            let Some(id) = next_free_road_type_id(&catalog) else {
                break;
            };
            let local_id = u8::try_from(local_idx).unwrap_or(0);
            let views = gfx
                .views_for_local_id(local_id)
                .map(<[crate::newgrf_sprites::DecodedSprite]>::to_vec)
                .unwrap_or_default();
            let preview = views.first().cloned();
            catalog.push(RoadTypeDef {
                id,
                class: meta.class,
                label: meta.label,
                short_label: meta.short_label,
                intro_year: meta.intro_year,
                from_newgrf: true,
                newgrf_preview: preview,
                newgrf_views: views,
            });
        }
    }
    state.road_type_catalog = catalog;
    if !state
        .road_type_catalog
        .iter()
        .any(|d| d.id == state.current_road_type)
    {
        state.current_road_type = RoadType::ROAD;
    }
    if !state
        .road_type_catalog
        .iter()
        .any(|d| d.id == state.current_tram_type)
    {
        state.current_tram_type = RoadType::TRAM;
    }
}

/// Aplica `RoadTypes` con directorios de búsqueda por defecto.
pub fn apply_newgrf_road_types_default_dirs(state: &mut GameState) {
    let owned = default_newgrf_search_dirs();
    let refs: Vec<&Path> = owned.iter().map(AsRef::as_ref).collect();
    apply_newgrf_road_types(state, &refs);
}

#[must_use]
pub fn parse_action0_station_meta(payload: &[u8]) -> Option<ParsedStationMeta> {
    let header = parse_action0_header(payload)?;
    if header.feature != ACTION0_FEATURE_STATIONS || header.num_ids == 0 {
        return None;
    }
    if payload.len() < 5 {
        return None;
    }
    let mut i = 5usize;
    let mut class_short = String::from("NGRF");
    let mut short_label = String::from("Stat");
    let mut label = String::new();
    let mut disallowed_platforms = 0u8;
    let mut disallowed_lengths = 0u8;
    for _ in 0..header.num_props {
        if i >= payload.len() {
            break;
        }
        let prop = payload[i];
        i += 1;
        match prop {
            PROP_LABEL => {
                if i + 4 > payload.len() {
                    break;
                }
                class_short = String::from_utf8_lossy(&payload[i..i + 4])
                    .trim_end_matches('\0')
                    .trim()
                    .to_string();
                if class_short.is_empty() {
                    class_short = "NGRF".into();
                }
                i += 4;
            }
            PROP_STATION_SPEC_SHORT => {
                if i + 4 > payload.len() {
                    break;
                }
                short_label = String::from_utf8_lossy(&payload[i..i + 4])
                    .trim_end_matches('\0')
                    .trim()
                    .to_string();
                if short_label.is_empty() {
                    short_label = "Stat".into();
                }
                i += 4;
            }
            PROP_STATION_DISALLOWED_PLATFORMS => {
                if i >= payload.len() {
                    break;
                }
                disallowed_platforms = payload[i];
                i += 1;
            }
            PROP_STATION_DISALLOWED_LENGTHS => {
                if i >= payload.len() {
                    break;
                }
                disallowed_lengths = payload[i];
                i += 1;
            }
            PROP_NAME_CSTRING => {
                let Some(nul) = payload[i..].iter().position(|&b| b == 0) else {
                    break;
                };
                label = String::from_utf8_lossy(&payload[i..i + nul]).to_string();
                i += nul + 1;
            }
            _ => break,
        }
    }
    if label.is_empty() {
        label.clone_from(&short_label);
    }
    let class_label = if class_short.eq_ignore_ascii_case("DFLT") {
        "Por defecto".into()
    } else {
        class_short.clone()
    };
    Some(ParsedStationMeta {
        class_short_label: class_short,
        class_label,
        short_label,
        label,
        disallowed_platforms,
        disallowed_lengths,
    })
}

#[must_use]
pub fn collect_station_metas_from_grf(data: &[u8]) -> Vec<ParsedStationMeta> {
    let mut out = Vec::new();
    let _ = for_each_pseudo_payload(data, |payload| {
        if let Some(meta) = parse_action0_station_meta(payload) {
            out.push(meta);
        }
    });
    out
}

fn resolve_or_create_station_class(
    classes: &mut Vec<StationClassDef>,
    meta: &ParsedStationMeta,
) -> Option<StationClassId> {
    if meta.class_short_label.eq_ignore_ascii_case("DFLT") {
        return Some(StationClassId::DEFAULT);
    }
    if let Some(existing) = classes
        .iter()
        .find(|c| c.short_label.eq_ignore_ascii_case(&meta.class_short_label))
    {
        return Some(existing.id);
    }
    let id = next_free_station_class_id(classes)?;
    classes.push(StationClassDef {
        id,
        label: meta.class_label.clone(),
        short_label: meta.class_short_label.clone(),
        from_newgrf: true,
    });
    Some(id)
}

/// Reconstruye catálogos de estación desde el stack `enabled` + vanilla.
pub fn apply_newgrf_stations(state: &mut GameState, search_dirs: &[&Path]) {
    let mut classes = vanilla_station_class_catalog();
    let mut specs = vanilla_station_spec_catalog();
    let stack = state.newgrf_stack.clone();
    for entry in &stack {
        if !entry.enabled {
            continue;
        }
        let Some(path) = search_dirs
            .iter()
            .map(|d| d.join(&entry.filename))
            .find(|p| p.is_file())
        else {
            continue;
        };
        let Ok(data) = std::fs::read(&path) else {
            continue;
        };
        let gfx = crate::newgrf_sprites::collect_station_sprite_graphics(&data).unwrap_or_default();
        for (local_idx, meta) in collect_station_metas_from_grf(&data)
            .into_iter()
            .enumerate()
        {
            let Some(class_id) = resolve_or_create_station_class(&mut classes, &meta) else {
                break;
            };
            let Some(spec_id) = next_free_station_spec_id(&specs) else {
                break;
            };
            let local_id = u8::try_from(local_idx).unwrap_or(0);
            let views = gfx
                .views_for_local_id(local_id)
                .map(<[crate::newgrf_sprites::DecodedSprite]>::to_vec)
                .unwrap_or_default();
            let preview = views.first().cloned();
            specs.push(StationSpecDef {
                id: spec_id,
                class: class_id,
                label: meta.label,
                short_label: meta.short_label,
                disallowed_platforms: meta.disallowed_platforms,
                disallowed_lengths: meta.disallowed_lengths,
                from_newgrf: true,
                newgrf_preview: preview,
                newgrf_views: views,
            });
        }
    }
    state.station_class_catalog = classes;
    state.station_spec_catalog = specs;
    if !state
        .station_class_catalog
        .iter()
        .any(|c| c.id == state.current_station_class)
    {
        state.current_station_class = StationClassId::DEFAULT;
    }
    if !state
        .station_spec_catalog
        .iter()
        .any(|s| s.id == state.current_station_spec)
    {
        state.current_station_spec = StationSpecId::DEFAULT_RAIL;
    }
}

/// Aplica Stations con directorios de búsqueda por defecto.
pub fn apply_newgrf_stations_default_dirs(state: &mut GameState) {
    let owned = default_newgrf_search_dirs();
    let refs: Vec<&Path> = owned.iter().map(AsRef::as_ref).collect();
    apply_newgrf_stations(state, &refs);
}

/// Refresco completo de catálogos Action0 tras cambiar el stack.
pub fn apply_newgrf_stack_catalogs_default_dirs(state: &mut GameState) {
    apply_newgrf_road_types_default_dirs(state);
    apply_newgrf_stations_default_dirs(state);
    apply_newgrf_vehicles_trains_default_dirs(state);
    apply_newgrf_action5_shore_default_dirs(state);
}

/// Aplica bloques Action5 shore (`0x0D`) del stack enabled → `shore_newgrf_sprites`.
pub fn apply_newgrf_action5_shore(state: &mut GameState, search_dirs: &[&Path]) {
    use crate::newgrf_sprites::{
        SHORE_ACTION5_SLOT_COUNT, collect_action5_blocks, merge_shore_action5_block,
    };
    let mut slots = vec![None; SHORE_ACTION5_SLOT_COUNT];
    let stack = state.newgrf_stack.clone();
    for entry in &stack {
        if !entry.enabled {
            continue;
        }
        let Some(path) = search_dirs
            .iter()
            .map(|d| d.join(&entry.filename))
            .find(|p| p.is_file())
        else {
            continue;
        };
        let Ok(data) = std::fs::read(&path) else {
            continue;
        };
        let Ok(blocks) = collect_action5_blocks(&data) else {
            continue;
        };
        for block in &blocks {
            merge_shore_action5_block(&mut slots, block);
        }
    }
    state.shore_newgrf_sprites = slots;
}

/// Action5 shore con directorios de búsqueda por defecto.
pub fn apply_newgrf_action5_shore_default_dirs(state: &mut GameState) {
    let owned = default_newgrf_search_dirs();
    let refs: Vec<&Path> = owned.iter().map(AsRef::as_ref).collect();
    apply_newgrf_action5_shore(state, &refs);
}

/// Prop velocidad máxima tren (uint16 LE) — extensión local / subset Action0.
const PROP_TRAIN_SPEED: u8 = 0x09;
/// Prop potencia (uint16 LE).
const PROP_TRAIN_POWER: u8 = 0x0B;

#[must_use]
pub fn parse_action0_train_meta(payload: &[u8]) -> Option<ParsedTrainMeta> {
    let header = parse_action0_header(payload)?;
    if header.feature != ACTION0_FEATURE_TRAINS || header.num_ids == 0 {
        return None;
    }
    if payload.len() < 5 {
        return None;
    }
    let mut i = 5usize;
    let mut name = String::new();
    let mut intro_year = 1920u16;
    let mut max_speed = 96u16;
    let mut power_hp = 500u32;
    for _ in 0..header.num_props {
        if i >= payload.len() {
            break;
        }
        let prop = payload[i];
        i += 1;
        match prop {
            PROP_INTRO_YEAR => {
                if i + 2 > payload.len() {
                    break;
                }
                intro_year = u16::from_le_bytes([payload[i], payload[i + 1]]);
                i += 2;
            }
            PROP_TRAIN_SPEED => {
                if i + 2 > payload.len() {
                    break;
                }
                max_speed = u16::from_le_bytes([payload[i], payload[i + 1]]).max(1);
                i += 2;
            }
            PROP_TRAIN_POWER => {
                if i + 2 > payload.len() {
                    break;
                }
                power_hp = u32::from(u16::from_le_bytes([payload[i], payload[i + 1]])).max(1);
                i += 2;
            }
            PROP_NAME_CSTRING => {
                let Some(nul) = payload[i..].iter().position(|&b| b == 0) else {
                    break;
                };
                name = String::from_utf8_lossy(&payload[i..i + nul]).to_string();
                i += nul + 1;
            }
            _ => break,
        }
    }
    if name.is_empty() {
        name = "NewGRF Train".into();
    }
    Some(ParsedTrainMeta {
        name,
        intro_year,
        max_speed,
        power_hp,
    })
}

#[must_use]
pub fn collect_train_metas_from_grf(data: &[u8]) -> Vec<ParsedTrainMeta> {
    let mut out = Vec::new();
    let _ = for_each_pseudo_payload(data, |payload| {
        if let Some(meta) = parse_action0_train_meta(payload) {
            out.push(meta);
        }
    });
    out
}

/// Reconstruye el catálogo de motores (vanilla + Action0/1/3 trains).
pub fn apply_newgrf_vehicles_trains(state: &mut GameState, search_dirs: &[&Path]) {
    let mut catalog = vanilla_engine_catalog();
    let stack = state.newgrf_stack.clone();
    for entry in &stack {
        if !entry.enabled {
            continue;
        }
        let Some(path) = search_dirs
            .iter()
            .map(|d| d.join(&entry.filename))
            .find(|p| p.is_file())
        else {
            continue;
        };
        let Ok(data) = std::fs::read(&path) else {
            continue;
        };
        let metas = collect_train_metas_from_grf(&data);
        let gfx = crate::newgrf_sprites::collect_train_sprite_graphics(&data).unwrap_or_default();
        // Emparejar Action0 (orden de aparición) con ids locales 0,1,2,…
        for (local_idx, meta) in metas.into_iter().enumerate() {
            let Some(id) = next_free_engine_id(&catalog) else {
                break;
            };
            let local_id = u8::try_from(local_idx).unwrap_or(0);
            let views = gfx
                .views_for_local_id(local_id)
                .map(<[crate::newgrf_sprites::DecodedSprite]>::to_vec)
                .unwrap_or_default();
            catalog.push(EngineDef {
                id,
                kind: VehicleKind::Train,
                name: meta.name,
                max_speed: meta.max_speed,
                price: (400_000_i64 * 20) >> 8,
                running_cost_year: (5_200 * 80) >> 8,
                capacity: 0,
                cargo: None,
                power_hp: meta.power_hp,
                weight_t: 80,
                intro_year: meta.intro_year,
                reliability_pct: 85,
                train_image_index: 2,
                from_newgrf: true,
                newgrf_views: views,
            });
        }
    }
    state.engine_catalog = catalog;
}

/// Aplica trains con directorios de búsqueda por defecto.
pub fn apply_newgrf_vehicles_trains_default_dirs(state: &mut GameState) {
    let owned = default_newgrf_search_dirs();
    let refs: Vec<&Path> = owned.iter().map(AsRef::as_ref).collect();
    apply_newgrf_vehicles_trains(state, &refs);
}

#[must_use]
pub fn build_action0_train_payload(
    intro_year: u16,
    max_speed: u16,
    power_hp: u16,
    name: &str,
) -> Vec<u8> {
    let mut p = vec![
        0x00,
        ACTION0_FEATURE_TRAINS,
        0x04,
        0x01,
        0x00,
        PROP_INTRO_YEAR,
    ];
    p.extend_from_slice(&intro_year.to_le_bytes());
    p.push(PROP_TRAIN_SPEED);
    p.extend_from_slice(&max_speed.to_le_bytes());
    p.push(PROP_TRAIN_POWER);
    p.extend_from_slice(&power_hp.to_le_bytes());
    p.push(PROP_NAME_CSTRING);
    p.extend_from_slice(name.as_bytes());
    p.push(0);
    p
}

#[must_use]
pub fn build_action0_station_payload(
    class_label: &[u8; 4],
    spec_short: &[u8; 4],
    disallowed_platforms: u8,
    disallowed_lengths: u8,
    name: &str,
) -> Vec<u8> {
    let mut p = vec![0x00, ACTION0_FEATURE_STATIONS, 0x05, 0x01, 0x00, PROP_LABEL];
    p.extend_from_slice(class_label);
    p.push(PROP_STATION_SPEC_SHORT);
    p.extend_from_slice(spec_short);
    p.push(PROP_STATION_DISALLOWED_PLATFORMS);
    p.push(disallowed_platforms);
    p.push(PROP_STATION_DISALLOWED_LENGTHS);
    p.push(disallowed_lengths);
    p.push(PROP_NAME_CSTRING);
    p.extend_from_slice(name.as_bytes());
    p.push(0);
    p
}

#[must_use]
pub fn default_newgrf_search_dirs() -> Vec<std::path::PathBuf> {
    let mut dirs = vec![
        std::path::PathBuf::from("assets/opengfx/opengfx2-32ez"),
        std::path::PathBuf::from("assets/newgrf"),
    ];
    if let Ok(extra) = std::env::var("OPENTTDRS_NEWGRF_DIR")
        && !extra.trim().is_empty()
    {
        dirs.push(std::path::PathBuf::from(extra));
    }
    dirs
}

#[must_use]
pub fn build_grf_v2_with_action0_and_action8(
    action0_payload: &[u8],
    grfid: [u8; 4],
    name: &str,
    description: &str,
) -> Vec<u8> {
    const SIG: [u8; 8] = [b'G', b'R', b'F', 0x82, 0x0D, 0x0A, 0x1A, 0x0A];
    let mut action8 = vec![0x08, 0x07];
    action8.extend_from_slice(&grfid);
    action8.extend_from_slice(name.as_bytes());
    action8.push(0);
    action8.extend_from_slice(description.as_bytes());
    action8.push(0);

    let mut data_section = Vec::new();
    for payload in [action0_payload, action8.as_slice()] {
        let size = u32::try_from(payload.len()).unwrap_or(0);
        data_section.extend_from_slice(&size.to_le_bytes());
        data_section.push(0xFF);
        data_section.extend_from_slice(payload);
    }
    data_section.extend_from_slice(&0u32.to_le_bytes());

    let sprite_offs = u32::try_from(1 + data_section.len()).unwrap_or(0);
    let mut out = Vec::new();
    out.extend_from_slice(&[0x00, 0x00]);
    out.extend_from_slice(&SIG);
    out.extend_from_slice(&sprite_offs.to_le_bytes());
    out.push(0x00);
    out.extend_from_slice(&data_section);
    out.extend_from_slice(&0u32.to_le_bytes());
    out
}

#[must_use]
pub fn build_action0_roadtype_payload(
    short_label: &[u8; 4],
    is_tram: bool,
    intro_year: u16,
    name: &str,
) -> Vec<u8> {
    let mut p = vec![
        0x00,
        ACTION0_FEATURE_ROADTYPES,
        0x04,
        0x01,
        0x00,
        PROP_LABEL,
    ];
    p.extend_from_slice(short_label);
    p.push(PROP_FLAGS);
    p.push(u8::from(is_tram));
    p.push(PROP_INTRO_YEAR);
    p.extend_from_slice(&intro_year.to_le_bytes());
    p.push(PROP_NAME_CSTRING);
    p.extend_from_slice(name.as_bytes());
    p.push(0);
    p
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::newgrf_config::build_minimal_grf_v2;

    #[test]
    fn inspect_minimal_counts_action8() {
        let bytes = build_minimal_grf_v2([b'T', b'E', 0x01, 0x00], "T", "D");
        let report = inspect_grf_bytes(&bytes).unwrap();
        assert_eq!(report.action_counts[0x08], 1);
        assert_eq!(report.pseudo_sprites, 1);
        assert!(report.action0_features.is_empty());
    }

    #[test]
    fn inspect_counts_action0_roadtypes_feature() {
        let a0 = build_action0_roadtype_payload(b"TEST", false, 1960, "Test Road");
        let bytes = build_grf_v2_with_action0_and_action8(&a0, [b'R', b'T', 0x00, 0x01], "RT", "d");
        let report = inspect_grf_bytes(&bytes).unwrap();
        assert_eq!(report.action_counts[0x00], 1);
        assert_eq!(report.action_counts[0x08], 1);
        assert_eq!(report.action0_features, vec![ACTION0_FEATURE_ROADTYPES]);
    }

    #[test]
    fn truncated_bytes_do_not_panic() {
        let err = inspect_grf_bytes(&[0x00]).unwrap_err();
        assert!(matches!(err, GrfScanError::TooShort));
        let _ = inspect_grf_bytes(&[0x01, 0x02, 0x03]).unwrap();
    }

    #[test]
    fn parse_action0_header_reads_fields() {
        let h = parse_action0_header(&[0x00, 0x12, 0x03, 0x02]).unwrap();
        assert_eq!(h.feature, 0x12);
        assert_eq!(h.num_props, 3);
        assert_eq!(h.num_ids, 2);
    }

    #[test]
    fn parse_roadtype_meta_and_apply_from_bytes() {
        let a0 = build_action0_roadtype_payload(b"COBB", false, 1850, "Adoquines");
        let meta = parse_action0_roadtype_meta(&a0).unwrap();
        assert_eq!(meta.short_label, "COBB");
        assert_eq!(meta.label, "Adoquines");
        assert_eq!(meta.class, RoadTramType::Road);
        assert_eq!(meta.intro_year, 1850);

        let mut state = GameState::new(4, 4);
        let bytes = build_grf_v2_with_action0_and_action8(&a0, [b'C', b'B', 0, 1], "cobble", "");
        let dir = tempfile_dir_with("cobble.grf", &bytes);
        apply_newgrf_road_types(&mut state, &[&dir]);
        assert_eq!(state.road_type_catalog.len(), 2);

        state
            .newgrf_stack
            .push(crate::NewGrfEntry::new("cobble.grf", 1));
        apply_newgrf_road_types(&mut state, &[&dir]);
        assert!(
            state
                .road_type_catalog
                .iter()
                .any(|d| d.from_newgrf && d.short_label == "COBB")
        );
        let id = state
            .road_type_catalog
            .iter()
            .find(|d| d.from_newgrf)
            .unwrap()
            .id;
        assert!(id.as_u8() >= 2);
        assert!(
            state
                .road_type_catalog
                .iter()
                .find(|d| d.from_newgrf)
                .unwrap()
                .newgrf_preview
                .is_none()
        );
    }

    #[test]
    fn apply_roadtype_with_action1_3_attaches_preview() {
        let a0 = build_action0_roadtype_payload(b"COBB", false, 1970, "Cobble Road");
        let mut indices = vec![0u8; 8 * 8];
        for y in 2..6 {
            for x in 2..6 {
                indices[y * 8 + x] = 174;
            }
        }
        let bytes = crate::build_grf_v2_roadtype_with_preview_sprite(
            &a0,
            0,
            8,
            8,
            &indices,
            [b'R', b'G', 0, 1],
            "rgfx",
        );
        let dir = tempfile_dir_with("rgfx.grf", &bytes);
        let mut state = GameState::new(4, 4);
        state
            .newgrf_stack
            .push(crate::NewGrfEntry::new("rgfx.grf", 2));
        apply_newgrf_road_types(&mut state, &[&dir]);
        let def = state
            .road_type_catalog
            .iter()
            .find(|d| d.from_newgrf)
            .unwrap();
        let preview = def.newgrf_preview_sprite().unwrap();
        assert_eq!(preview.width, 8);
        assert_eq!(preview.height, 8);
        assert_eq!(def.newgrf_views.len(), 1);
        assert!(def.newgrf_view(0).is_some());
    }

    #[test]
    fn parse_station_meta_and_apply_from_bytes() {
        let a0 = build_action0_station_payload(b"MODN", b"Plat", 0, 0, "Andén moderno");
        let meta = parse_action0_station_meta(&a0).unwrap();
        assert_eq!(meta.class_short_label, "MODN");
        assert_eq!(meta.short_label, "Plat");
        assert_eq!(meta.label, "Andén moderno");

        let mut state = GameState::new(4, 4);
        let bytes = build_grf_v2_with_action0_and_action8(&a0, [b'S', b'T', 0, 1], "stat", "");
        let dir = tempfile_dir_with("stat.grf", &bytes);
        apply_newgrf_stations(&mut state, &[&dir]);
        assert_eq!(state.station_spec_catalog.len(), 1);

        state
            .newgrf_stack
            .push(crate::NewGrfEntry::new("stat.grf", 1));
        apply_newgrf_stations(&mut state, &[&dir]);
        assert!(
            state
                .station_spec_catalog
                .iter()
                .any(|d| d.from_newgrf && d.short_label == "Plat")
        );
        assert!(
            state
                .station_class_catalog
                .iter()
                .any(|c| c.from_newgrf && c.short_label == "MODN")
        );
        let spec_id = state
            .station_spec_catalog
            .iter()
            .find(|d| d.from_newgrf)
            .unwrap()
            .id;
        assert!(spec_id.as_u16() >= 1);
        assert!(
            state
                .station_spec_catalog
                .iter()
                .find(|d| d.from_newgrf)
                .unwrap()
                .newgrf_preview
                .is_none()
        );
    }

    #[test]
    fn apply_station_with_action1_3_attaches_preview() {
        let a0 = build_action0_station_payload(b"MODN", b"Plat", 0, 0, "Sprite Andén");
        let mut indices = vec![0u8; 8 * 8];
        for y in 2..6 {
            for x in 2..6 {
                indices[y * 8 + x] = 174;
            }
        }
        let bytes = crate::build_grf_v2_station_with_preview_sprite(
            &a0,
            0,
            8,
            8,
            &indices,
            [b'S', b'G', 0, 1],
            "sgfx",
        );
        let dir = tempfile_dir_with("sgfx.grf", &bytes);
        let mut state = GameState::new(4, 4);
        state
            .newgrf_stack
            .push(crate::NewGrfEntry::new("sgfx.grf", 2));
        apply_newgrf_stations(&mut state, &[&dir]);
        let def = state
            .station_spec_catalog
            .iter()
            .find(|d| d.from_newgrf)
            .unwrap();
        let preview = def.newgrf_preview_sprite().unwrap();
        assert_eq!(preview.width, 8);
        assert_eq!(preview.height, 8);
        assert_eq!(def.newgrf_views.len(), 1);
        assert!(def.newgrf_view(0).is_some());
    }

    #[test]
    fn inspect_summarizes_action5_slots() {
        let mut indices = vec![0u8; 8 * 8];
        for y in 1..7 {
            for x in 1..7 {
                indices[y * 8 + x] = 174;
            }
        }
        let bytes = crate::build_grf_v2_action5_with_sprite(
            0x05,
            1039,
            8,
            8,
            &indices,
            [b'E', b'L', 0, 1],
            "elrail",
        );
        let report = inspect_grf_bytes(&bytes).unwrap();
        assert_eq!(report.action_counts[0x05], 1);
        assert_eq!(report.action5_slots.len(), 1);
        assert_eq!(report.action5_slots[0].type_id, 0x05);
        assert_eq!(report.action5_slots[0].offset, 1039);
        assert_eq!(report.action5_slots[0].preview_wh, Some((8, 8)));
        let summary = report.format_summary();
        assert!(summary.contains("Action5:"));
        assert!(summary.contains("catenary"));
    }

    #[test]
    fn apply_action5_shore_fills_slot_from_stack() {
        let mut indices = vec![0u8; 8 * 8];
        for y in 1..7 {
            for x in 1..7 {
                indices[y * 8 + x] = 174;
            }
        }
        // offset 3 < 18 → escribe en slot 3
        let bytes = crate::build_grf_v2_action5_with_sprite(
            0x0D,
            3,
            8,
            8,
            &indices,
            [b'S', b'H', 0, 2],
            "shore2",
        );
        let dir = tempfile_dir_with("shore2.grf", &bytes);
        let mut state = GameState::new(4, 4);
        state
            .newgrf_stack
            .push(crate::NewGrfEntry::new("shore2.grf", 2));
        apply_newgrf_action5_shore(&mut state, &[&dir]);
        assert_eq!(state.shore_newgrf_sprites.len(), 18);
        assert!(state.shore_newgrf_sprites[3].is_some());
        assert!(state.shore_newgrf_sprites[0].is_none());
        let spr = state.shore_newgrf_sprites[3].as_ref().unwrap();
        assert_eq!(spr.width, 8);
    }

    #[test]
    fn inspect_counts_action0_stations_feature() {
        let a0 = build_action0_station_payload(b"TEST", b"Spec", 0, 0, "Test Station");
        let bytes = build_grf_v2_with_action0_and_action8(&a0, [b'S', b'T', 0x00, 0x02], "ST", "d");
        let report = inspect_grf_bytes(&bytes).unwrap();
        assert_eq!(report.action0_features, vec![ACTION0_FEATURE_STATIONS]);
    }

    #[test]
    fn parse_train_meta_and_apply_from_bytes() {
        let a0 = build_action0_train_payload(1955, 120, 1500, "Locomotora NewGRF");
        let meta = parse_action0_train_meta(&a0).unwrap();
        assert_eq!(meta.name, "Locomotora NewGRF");
        assert_eq!(meta.intro_year, 1955);
        assert_eq!(meta.max_speed, 120);
        assert_eq!(meta.power_hp, 1500);

        let mut state = GameState::new(4, 4);
        let vanilla_len = state.engine_catalog.len();
        let bytes = build_grf_v2_with_action0_and_action8(&a0, [b'T', b'R', 0, 1], "train", "");
        let dir = tempfile_dir_with("train.grf", &bytes);
        state
            .newgrf_stack
            .push(crate::NewGrfEntry::new("train.grf", 2));
        apply_newgrf_vehicles_trains(&mut state, &[&dir]);
        assert_eq!(state.engine_catalog.len(), vanilla_len + 1);
        let eng = state.engine_catalog.iter().find(|e| e.from_newgrf).unwrap();
        assert!(eng.id >= crate::NEWGRF_ENGINE_ID_BASE);
        assert_eq!(eng.name, "Locomotora NewGRF");
        assert_eq!(eng.kind, VehicleKind::Train);
        assert!(eng.newgrf_preview().is_none());
    }

    #[test]
    fn apply_train_with_action1_3_attaches_preview() {
        let a0 = build_action0_train_payload(1960, 100, 800, "Sprite Loco");
        let mut indices = vec![0u8; 8 * 8];
        for y in 2..6 {
            for x in 2..6 {
                indices[y * 8 + x] = 174;
            }
        }
        let bytes = crate::build_grf_v2_train_with_preview_sprite(
            &a0,
            0,
            8,
            8,
            &indices,
            [b'T', b'S', 0, 2],
            "tsprite",
        );
        let dir = tempfile_dir_with("tsprite.grf", &bytes);
        let mut state = GameState::new(4, 4);
        state
            .newgrf_stack
            .push(crate::NewGrfEntry::new("tsprite.grf", 3));
        apply_newgrf_vehicles_trains(&mut state, &[&dir]);
        let eng = state.engine_catalog.iter().find(|e| e.from_newgrf).unwrap();
        let preview = eng.newgrf_preview().unwrap();
        assert_eq!(preview.width, 8);
        assert_eq!(preview.height, 8);
        assert_eq!(eng.newgrf_views.len(), 1);
        assert!(eng.newgrf_view(3).is_some());
    }

    #[test]
    fn inspect_counts_action0_trains_feature() {
        let a0 = build_action0_train_payload(1960, 100, 800, "T");
        let bytes = build_grf_v2_with_action0_and_action8(&a0, [b'T', b'0', 0, 1], "t", "d");
        let report = inspect_grf_bytes(&bytes).unwrap();
        assert_eq!(report.action0_features, vec![ACTION0_FEATURE_TRAINS]);
    }

    fn tempfile_dir_with(name: &str, bytes: &[u8]) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("openttdrs_ngr_{}_{}", std::process::id(), name));
        let _ = std::fs::create_dir_all(&dir);
        std::fs::write(dir.join(name), bytes).unwrap();
        dir
    }
}
