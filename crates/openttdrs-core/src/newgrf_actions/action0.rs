//! Parsing compartido de cabeceras y metadatos Action0.

use crate::newgrf_config::{GrfScanError, parse_grf_container};
use crate::newgrf_walk::for_each_pseudo_sprite;
use crate::road_type::RoadTramType;

/// Feature Action0: `Trains` (`OpenTTD` `GSF_TRAINS`).
pub const ACTION0_FEATURE_TRAINS: u8 = 0x00;
/// Feature Action0: `Stations` (`OpenTTD` `GSF_STATIONS`).
pub const ACTION0_FEATURE_STATIONS: u8 = 0x04;
/// Feature Action0: `IndustryTiles` (`OpenTTD` `GSF_INDUSTRYTILES`).
pub const ACTION0_FEATURE_INDUSTRYTILES: u8 = 0x09;
/// Feature Action0: `RoadTypes` (`OpenTTD` `GSF_ROADTYPES`).
pub const ACTION0_FEATURE_ROADTYPES: u8 = 0x12;

/// `IndustryTiles`: substitute vanilla gfx (`prop 0x08`).
const PROP_INDTILE_SUBST: u8 = 0x08;
/// `IndustryTiles`: override vanilla gfx (`prop 0x09`).
const PROP_INDTILE_OVERRIDE: u8 = 0x09;

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
/// Stations: custom tile layout (platforms × length).
const PROP_STATION_CUSTOM_LAYOUT: u8 = 0x0E;
/// Stations: copy custom layout from another station id.
const PROP_STATION_COPY_LAYOUT: u8 = 0x0F;
/// Prop año introducción (uint16 LE).
const PROP_INTRO_YEAR: u8 = 0x16;
/// Extensión local: nombre C-string (tests / GRFs propios).
const PROP_NAME_CSTRING: u8 = 0xFE;

/// Prop velocidad máxima tren (uint16 LE) — extensión local / subset Action0.
const PROP_TRAIN_SPEED: u8 = 0x09;
/// Prop potencia (uint16 LE).
const PROP_TRAIN_POWER: u8 = 0x0B;

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
    /// Layouts prop `0x0E`: `(platforms, length)` → tiletypes.
    pub custom_layouts: std::collections::HashMap<(u8, u8), Vec<u8>>,
    /// Prop `0x0F`: copiar layouts desde este id local (si definido).
    pub copy_layout_from: Option<u16>,
}

/// Metadatos `Trains` Action0 (antes de asignar ID ≥1000).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedTrainMeta {
    pub name: String,
    pub intro_year: u16,
    pub max_speed: u16,
    pub power_hp: u32,
}

/// Metadatos `IndustryTiles` Action0 (antes de asignar gfx ≥175).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedIndustryTileMeta {
    pub local_id: u8,
    /// Substitute vanilla (`prop 0x08`); obligatorio para crear slot.
    pub subst_id: u8,
    /// Override de gfx vanilla (`prop 0x09`).
    pub override_of: Option<u8>,
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
    for_each_pseudo_sprite(section, container, |payload| visit(payload));
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

fn read_four_char_label(payload: &[u8], i: &mut usize, fallback: &str) -> Option<String> {
    if *i + 4 > payload.len() {
        return None;
    }
    let mut s = String::from_utf8_lossy(&payload[*i..*i + 4])
        .trim_end_matches('\0')
        .trim()
        .to_string();
    *i += 4;
    if s.is_empty() {
        s = fallback.into();
    }
    Some(s)
}

fn parse_station_custom_layouts(
    payload: &[u8],
    i: &mut usize,
) -> std::collections::HashMap<(u8, u8), Vec<u8>> {
    let mut custom_layouts = std::collections::HashMap::new();
    loop {
        if *i + 2 > payload.len() {
            break;
        }
        let length = payload[*i];
        let number = payload[*i + 1];
        *i += 2;
        if length == 0 || number == 0 {
            break;
        }
        let n = usize::from(length).saturating_mul(usize::from(number));
        if *i + n > payload.len() {
            break;
        }
        let mut tiles = payload[*i..*i + n].to_vec();
        *i += n;
        for t in &mut tiles {
            *t &= !1u8;
        }
        custom_layouts.insert((number, length), tiles);
    }
    custom_layouts
}

fn parse_station_copy_layout_id(payload: &[u8], i: &mut usize) -> Option<u16> {
    if *i >= payload.len() {
        return None;
    }
    let b = payload[*i];
    *i += 1;
    if b == 0xFF {
        if *i + 1 > payload.len() {
            return None;
        }
        let id = u16::from_le_bytes([payload[*i], payload[*i + 1]]);
        *i += 2;
        Some(id)
    } else {
        Some(u16::from(b))
    }
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
    let mut custom_layouts = std::collections::HashMap::new();
    let mut copy_layout_from = None;
    for _ in 0..header.num_props {
        if i >= payload.len() {
            break;
        }
        let prop = payload[i];
        i += 1;
        match prop {
            PROP_LABEL => {
                let Some(s) = read_four_char_label(payload, &mut i, "NGRF") else {
                    break;
                };
                class_short = s;
            }
            PROP_STATION_SPEC_SHORT => {
                let Some(s) = read_four_char_label(payload, &mut i, "Stat") else {
                    break;
                };
                short_label = s;
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
            PROP_STATION_CUSTOM_LAYOUT => {
                custom_layouts = parse_station_custom_layouts(payload, &mut i);
            }
            PROP_STATION_COPY_LAYOUT => {
                let Some(id) = parse_station_copy_layout_id(payload, &mut i) else {
                    break;
                };
                copy_layout_from = Some(id);
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
        custom_layouts,
        copy_layout_from,
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

#[must_use]
pub fn parse_action0_industry_tile_meta(payload: &[u8]) -> Option<ParsedIndustryTileMeta> {
    let header = parse_action0_header(payload)?;
    if header.feature != ACTION0_FEATURE_INDUSTRYTILES || header.num_ids == 0 {
        return None;
    }
    if payload.len() < 5 {
        return None;
    }
    let local_id = payload[4];
    let mut i = 5usize;
    let mut subst_id: Option<u8> = None;
    let mut override_of: Option<u8> = None;
    for _ in 0..header.num_props {
        if i >= payload.len() {
            break;
        }
        let prop = payload[i];
        i += 1;
        match prop {
            PROP_INDTILE_SUBST => {
                if i >= payload.len() {
                    break;
                }
                let s = payload[i];
                i += 1;
                if u16::from(s) < crate::industry_tile::NEW_INDUSTRY_TILE_OFFSET {
                    subst_id = Some(s);
                }
            }
            PROP_INDTILE_OVERRIDE => {
                if i >= payload.len() {
                    break;
                }
                let o = payload[i];
                i += 1;
                if u16::from(o) < crate::industry_tile::NEW_INDUSTRY_TILE_OFFSET {
                    override_of = Some(o);
                }
            }
            0x0D | 0x0E | 0x10 | 0x11 | 0x12 => {
                if i >= payload.len() {
                    break;
                }
                i += 1;
            }
            0x0A | 0x0B | 0x0C | 0x0F => {
                if i + 2 > payload.len() {
                    break;
                }
                i += 2;
            }
            _ => break,
        }
    }
    Some(ParsedIndustryTileMeta {
        local_id,
        subst_id: subst_id?,
        override_of,
    })
}

#[must_use]
pub fn collect_industry_tile_metas_from_grf(data: &[u8]) -> Vec<ParsedIndustryTileMeta> {
    let mut out = Vec::new();
    let _ = for_each_pseudo_payload(data, |payload| {
        if let Some(meta) = parse_action0_industry_tile_meta(payload) {
            out.push(meta);
        }
    });
    out
}

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
