//! Parsing compartido de cabeceras y metadatos Action0.

use crate::newgrf_config::{GrfScanError, parse_grf_container};
use crate::newgrf_walk::for_each_pseudo_sprite;
use crate::road_type::RoadTramType;
use crate::vehicle::VehicleKind;

/// Feature Action0: `Trains` (`OpenTTD` `GSF_TRAINS`).
pub const ACTION0_FEATURE_TRAINS: u8 = 0x00;
/// Feature Action0: `RoadVehicles` (`OpenTTD` `GSF_ROADVEHICLES`).
pub const ACTION0_FEATURE_ROAD_VEHICLES: u8 = 0x01;
/// Feature Action0: `Ships` (`OpenTTD` `GSF_SHIPS`).
pub const ACTION0_FEATURE_SHIPS: u8 = 0x02;
/// Feature Action0: `Aircraft` (`OpenTTD` `GSF_AIRCRAFT`).
pub const ACTION0_FEATURE_AIRCRAFT: u8 = 0x03;
/// Feature Action0: `Stations` (`OpenTTD` `GSF_STATIONS`).
pub const ACTION0_FEATURE_STATIONS: u8 = 0x04;
/// Feature Action0: `IndustryTiles` (`OpenTTD` `GSF_INDUSTRYTILES`).
pub const ACTION0_FEATURE_INDUSTRYTILES: u8 = 0x09;
/// Feature Action0: `Cargoes` (`OpenTTD` `GSF_CARGOES`).
pub const ACTION0_FEATURE_CARGOES: u8 = 0x0B;
/// Feature Action0: `Objects` (`OpenTTD` `GSF_OBJECTS`).
pub const ACTION0_FEATURE_OBJECTS: u8 = 0x0F;
/// Feature Action0: `RailTypes` (`OpenTTD` `GSF_RAILTYPES`).
pub const ACTION0_FEATURE_RAILTYPES: u8 = 0x10;
/// Feature Action0: `RoadTypes` (`OpenTTD` `GSF_ROADTYPES`).
pub const ACTION0_FEATURE_ROADTYPES: u8 = 0x12;
/// Feature Action0: `RoadStops` (`OpenTTD` `GSF_ROADSTOPS`).
pub const ACTION0_FEATURE_ROADSTOPS: u8 = 0x14;
/// Feature Action0: `Badges` (`OpenTTD` `GSF_BADGES`).
pub const ACTION0_FEATURE_BADGES: u8 = 0x15;

/// `IndustryTiles`: substitute vanilla gfx (`prop 0x08`).
const PROP_INDTILE_SUBST: u8 = 0x08;
/// `IndustryTiles`: override vanilla gfx (`prop 0x09`).
const PROP_INDTILE_OVERRIDE: u8 = 0x09;

/// Prop etiqueta 4 chars (`RoadTypes` short / Stations class label / Badges / Objects / `RoadStops` class).
const PROP_LABEL: u8 = 0x08;
/// Prop flags (`RoadTypes`: bit0 = tram; `Badges`: DWORD).
const PROP_FLAGS: u8 = 0x09;
/// `RoadStops`: tipo de parada BYTE (`0` bus / `1` truck; OTTD `0x09`).
const PROP_ROADSTOP_STOP_TYPE: u8 = 0x09;
/// Cargoes: bit number (`OpenTTD` `0x08`).
const PROP_CARGO_BITNUM: u8 = 0x08;
/// Cargoes: label 4 chars (`OpenTTD` `0x17`).
const PROP_CARGO_LABEL: u8 = 0x17;
/// Objects: size BYTE (`OpenTTD` `0x0C`).
const PROP_OBJECT_SIZE: u8 = 0x0C;
/// Stations: callback mask (`OpenTTD` 15.3; consumida).
const PROP_STATION_CALLBACK_MASK: u8 = 0x0B;
/// Stations: platforms disallowed bitmask (`OpenTTD` `0x0C`).
const PROP_STATION_DISALLOWED_PLATFORMS: u8 = 0x0C;
/// Stations: lengths disallowed bitmask (`OpenTTD` `0x0D`).
const PROP_STATION_DISALLOWED_LENGTHS: u8 = 0x0D;
/// Stations: custom tile layout (platforms × length).
const PROP_STATION_CUSTOM_LAYOUT: u8 = 0x0E;
/// Stations: copy custom layout from another station id.
const PROP_STATION_COPY_LAYOUT: u8 = 0x0F;
/// Feature Action0: `TramTypes` (`OpenTTD` `GSF_TRAMTYPES`; mismo handler que `RoadTypes`).
pub const ACTION0_FEATURE_TRAMTYPES: u8 = 0x13;
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
    /// Prop `0x14` speed limit (`0` = sin techo).
    pub max_speed: u16,
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
    pub lifelength_years: u8,
    pub model_life_years: u8,
    pub reliability_spd_dec: u16,
    pub climate_mask: u8,
    pub load_amount: u8,
    pub train_image_index: u8,
    pub dual_headed: bool,
    pub capacity: u32,
    pub cargo: Option<crate::cargo::CargoType>,
    pub weight_t: u16,
    pub price_factor: u8,
    pub running_cost_factor: u8,
    pub pow_wag_power: u32,
    pub pow_wag_weight: u16,
    pub rail_tilts: bool,
    pub curve_speed_mod: i16,
}

/// Subset de propiedades Action0 que alimenta el catálogo jugable de vehículos.
///
/// Los campos no representados por [`crate::engine::EngineDef`] se consumen con
/// su ancho de `OpenTTD` 15.3, pero no se anuncian como aplicados.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedVehicleMeta {
    pub local_id: u8,
    pub kind: VehicleKind,
    pub name: String,
    pub intro_year: u16,
    pub max_speed: u16,
    pub price_factor: u8,
    pub running_cost_factor: u8,
    pub capacity: u32,
    pub cargo: Option<crate::cargo::CargoType>,
    pub power_hp: u32,
    pub weight_t: u16,
    pub lifelength_years: u8,
    pub model_life_years: u8,
    pub climate_mask: u8,
    pub load_amount: u8,
    pub reliability_spd_dec: u16,
}

impl ParsedVehicleMeta {
    fn defaults(feature: u8, local_id: u8) -> Option<Self> {
        let (kind, name, speed, price, running, capacity, cargo, power, weight, life) =
            match feature {
                ACTION0_FEATURE_ROAD_VEHICLES => (
                    VehicleKind::Bus,
                    "NewGRF Road Vehicle",
                    96,
                    128,
                    128,
                    20,
                    Some(crate::cargo::CargoType::Passengers),
                    100,
                    10,
                    30,
                ),
                ACTION0_FEATURE_SHIPS => (
                    VehicleKind::Ship,
                    "NewGRF Ship",
                    80,
                    128,
                    128,
                    100,
                    Some(crate::cargo::CargoType::Passengers),
                    0,
                    100,
                    30,
                ),
                ACTION0_FEATURE_AIRCRAFT => (
                    VehicleKind::Aircraft,
                    "NewGRF Aircraft",
                    320,
                    128,
                    128,
                    100,
                    Some(crate::cargo::CargoType::Passengers),
                    0,
                    30,
                    30,
                ),
                _ => return None,
            };
        Some(Self {
            local_id,
            kind,
            name: name.into(),
            intro_year: 1920,
            max_speed: speed,
            price_factor: price,
            running_cost_factor: running,
            capacity,
            cargo,
            power_hp: power,
            weight_t: weight,
            lifelength_years: life,
            model_life_years: u8::MAX,
            climate_mask: 0x0F,
            load_amount: 0,
            reliability_spd_dec: if feature == ACTION0_FEATURE_SHIPS {
                crate::engine::SHIP_RELIABILITY_SPD_DEC
            } else {
                crate::engine::DEFAULT_RELIABILITY_SPD_DEC
            },
        })
    }
}

/// Asociación local de `RailType` Action0 (`prop 0x08`) con una etiqueta global.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParsedRailTypeMeta {
    pub local_id: u8,
    pub label: crate::newgrf_type_tables::TypeLabel,
    /// Prop `0x14` speed limit (`0` = sin techo).
    pub max_speed: u16,
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

/// Metadatos `RoadStops` Action0 (antes de asignar IDs).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedRoadStopMeta {
    pub class_short_label: String,
    pub class_label: String,
    pub short_label: String,
    pub label: String,
    /// `0` = bus, `1` = truck (común).
    pub stop_type: u8,
}

/// Metadatos `Badges` Action0 (antes de asignar ID global).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedBadgeMeta {
    pub label: String,
    pub flags: u32,
}

/// Metadatos `Cargoes` Action0 (antes de registrar en catálogo).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedCargoMeta {
    pub local_id: u8,
    pub bitnum: u8,
    pub label: String,
    pub name: String,
}

/// Metadatos `Objects` Action0 (antes de asignar ID global).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedObjectMeta {
    pub local_id: u8,
    pub class_label: String,
    pub name: String,
    pub size: u8,
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
    let feature_tram = header.feature == ACTION0_FEATURE_TRAMTYPES;
    if !(header.feature == ACTION0_FEATURE_ROADTYPES || feature_tram) || header.num_ids == 0 {
        return None;
    }
    if payload.len() < 5 {
        return None;
    }
    let mut i = 5usize;
    let mut short_label = String::from("NGRF");
    let mut label = String::new();
    let mut intro_year = 0u16;
    let mut max_speed = 0u16;
    // Extensión local en RoadTypes: bit0 de `0x09` = tram. En OTTD `0x09` es string WORD.
    let mut is_tram = feature_tram;
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
            PROP_FLAGS if !feature_tram => {
                // Local: BYTE flags tram. OTTD: WORD string id — si hay ≥2 bytes y el
                // segundo no parece prop, tratar como WORD consumido.
                if i >= payload.len() {
                    break;
                }
                is_tram = payload[i] & 0x01 != 0;
                i += 1;
            }
            0x14 => {
                if i + 2 > payload.len() {
                    break;
                }
                max_speed = u16::from_le_bytes([payload[i], payload[i + 1]]);
                i += 2;
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
            // Strings OTTD 0x09–0x0D / 0x1B: WORD cada una (feature tram o road real).
            0x09 | 0x0A | 0x0B | 0x0C | 0x0D | 0x1B if feature_tram => {
                if i + 2 > payload.len() {
                    break;
                }
                i += 2;
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
        max_speed,
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

/// Lee etiquetas `RailType` de un Action0. La propiedad `0x08` contiene un DWORD
/// por id y suele ser la primera del bloque, como en la fase Reserve de `OpenTTD`.
#[must_use]
pub fn parse_action0_railtype_metas(payload: &[u8]) -> Option<Vec<ParsedRailTypeMeta>> {
    let header = parse_action0_header(payload)?;
    if header.feature != ACTION0_FEATURE_RAILTYPES || header.num_ids == 0 || payload.len() < 5 {
        return None;
    }
    let first_id = payload[4];
    let n = usize::from(header.num_ids);
    let mut i = 5usize;
    let mut labels: Option<Vec<crate::newgrf_type_tables::TypeLabel>> = None;
    let mut max_speeds = vec![0u16; n];
    for _ in 0..header.num_props {
        let prop = *payload.get(i)?;
        i += 1;
        match prop {
            PROP_LABEL => {
                let bytes = n.checked_mul(4)?;
                if i.checked_add(bytes)? > payload.len() {
                    return None;
                }
                let mut out = Vec::with_capacity(n);
                for offset in 0..n {
                    let start = i + offset * 4;
                    out.push(payload[start..start + 4].try_into().ok()?);
                }
                i += bytes;
                labels = Some(out);
            }
            0x14 => {
                // Speed limit WORD por id.
                let bytes = n.checked_mul(2)?;
                if i.checked_add(bytes)? > payload.len() {
                    return None;
                }
                for (offset, speed) in max_speeds.iter_mut().enumerate() {
                    let start = i + offset * 2;
                    *speed = u16::from_le_bytes([payload[start], payload[start + 1]]);
                }
                i += bytes;
            }
            // Tamaños fijos por id para poder alcanzar prop 08 / 14.
            0x09..=0x0D | 0x13 | 0x1B | 0x1C => {
                i = i.checked_add(2usize.checked_mul(n)?)?;
            }
            0x10..=0x12 | 0x15 | 0x16 | 0x1A => {
                i = i.checked_add(n)?;
            }
            0x17 => {
                i = i.checked_add(4usize.checked_mul(n)?)?;
            }
            _ => {
                return labels.map(|labs| {
                    labs.into_iter()
                        .enumerate()
                        .map(|(offset, label)| ParsedRailTypeMeta {
                            local_id: first_id.wrapping_add(u8::try_from(offset).unwrap_or(0)),
                            label,
                            max_speed: max_speeds.get(offset).copied().unwrap_or(0),
                        })
                        .collect()
                });
            }
        }
        if i > payload.len() {
            return None;
        }
    }
    labels.map(|labs| {
        labs.into_iter()
            .enumerate()
            .map(|(offset, label)| ParsedRailTypeMeta {
                local_id: first_id.wrapping_add(u8::try_from(offset).unwrap_or(0)),
                label,
                max_speed: max_speeds.get(offset).copied().unwrap_or(0),
            })
            .collect()
    })
}

#[must_use]
pub fn collect_railtype_metas_from_grf(data: &[u8]) -> Vec<ParsedRailTypeMeta> {
    let mut out = Vec::new();
    let _ = for_each_pseudo_payload(data, |payload| {
        if let Some(metas) = parse_action0_railtype_metas(payload) {
            out.extend(metas);
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
            // 0x0A copy sprite layout: extended-byte id (consumida; gfx vía Action1/3).
            0x0A => {
                if parse_station_copy_layout_id(payload, &mut i).is_none() {
                    break;
                }
            }
            PROP_STATION_CALLBACK_MASK => {
                if i >= payload.len() {
                    break;
                }
                i += 1;
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
            // 0x09 sprite layout es variable; sin consumidor aún → cortar el bloque.
            _ => break,
        }
    }
    Some(finish_parsed_station_meta(
        class_short,
        label,
        disallowed_platforms,
        disallowed_lengths,
        custom_layouts,
        copy_layout_from,
    ))
}

fn finish_parsed_station_meta(
    class_short: String,
    mut label: String,
    disallowed_platforms: u8,
    disallowed_lengths: u8,
    custom_layouts: std::collections::HashMap<(u8, u8), Vec<u8>>,
    copy_layout_from: Option<u16>,
) -> ParsedStationMeta {
    let short_label = {
        let ascii: String = label
            .chars()
            .filter(char::is_ascii_alphanumeric)
            .take(4)
            .collect();
        if ascii.is_empty() {
            String::from("Stat")
        } else {
            ascii
        }
    };
    if label.is_empty() {
        label.clone_from(&short_label);
    }
    let class_label = if class_short.eq_ignore_ascii_case("DFLT") {
        "Por defecto".into()
    } else {
        class_short.clone()
    };
    ParsedStationMeta {
        class_short_label: class_short,
        class_label,
        short_label,
        label,
        disallowed_platforms,
        disallowed_lengths,
        custom_layouts,
        copy_layout_from,
    }
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

/// Parsea Action0 `RoadStops` (`0x14`): class label 4 chars, stop type BYTE, nombre `0xFE`.
#[must_use]
pub fn parse_action0_roadstop_meta(payload: &[u8]) -> Option<ParsedRoadStopMeta> {
    let header = parse_action0_header(payload)?;
    if header.feature != ACTION0_FEATURE_ROADSTOPS || header.num_ids == 0 || payload.len() < 5 {
        return None;
    }
    let mut i = 5usize;
    let mut class_short = String::from("NGRF");
    let mut label = String::new();
    let mut stop_type = 0u8;
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
            PROP_ROADSTOP_STOP_TYPE => {
                if i >= payload.len() {
                    break;
                }
                stop_type = payload[i];
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
    Some(finish_parsed_roadstop_meta(class_short, label, stop_type))
}

fn finish_parsed_roadstop_meta(
    class_short: String,
    mut label: String,
    stop_type: u8,
) -> ParsedRoadStopMeta {
    let short_label = {
        let ascii: String = label
            .chars()
            .filter(char::is_ascii_alphanumeric)
            .take(4)
            .collect();
        if ascii.is_empty() {
            String::from("Stop")
        } else {
            ascii
        }
    };
    if label.is_empty() {
        label.clone_from(&short_label);
    }
    let class_label = class_short.clone();
    ParsedRoadStopMeta {
        class_short_label: class_short,
        class_label,
        short_label,
        label,
        stop_type,
    }
}

#[must_use]
pub fn collect_roadstop_metas_from_grf(data: &[u8]) -> Vec<ParsedRoadStopMeta> {
    let mut out = Vec::new();
    let _ = for_each_pseudo_payload(data, |payload| {
        if let Some(meta) = parse_action0_roadstop_meta(payload) {
            out.push(meta);
        }
    });
    out
}

/// Parsea Action0 `Badges` (`0x15`): label 4 chars / `0xFE` nombre, flags DWORD.
#[must_use]
pub fn parse_action0_badge_meta(payload: &[u8]) -> Option<ParsedBadgeMeta> {
    let header = parse_action0_header(payload)?;
    if header.feature != ACTION0_FEATURE_BADGES || header.num_ids == 0 || payload.len() < 5 {
        return None;
    }
    let mut i = 5usize;
    let mut label = String::new();
    let mut flags = 0u32;
    for _ in 0..header.num_props {
        if i >= payload.len() {
            break;
        }
        let prop = payload[i];
        i += 1;
        match prop {
            PROP_LABEL => {
                let Some(s) = read_four_char_label(payload, &mut i, "BDGE") else {
                    break;
                };
                label = s;
            }
            PROP_FLAGS => {
                if i + 4 > payload.len() {
                    break;
                }
                flags = u32::from_le_bytes([
                    payload[i],
                    payload[i + 1],
                    payload[i + 2],
                    payload[i + 3],
                ]);
                i += 4;
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
        return None;
    }
    Some(ParsedBadgeMeta { label, flags })
}

#[must_use]
pub fn collect_badge_metas_from_grf(data: &[u8]) -> Vec<ParsedBadgeMeta> {
    let mut out = Vec::new();
    let _ = for_each_pseudo_payload(data, |payload| {
        if let Some(meta) = parse_action0_badge_meta(payload) {
            out.push(meta);
        }
    });
    out
}

/// Parsea Action0 `Cargoes` (`0x0B`): bitnum, label 4 chars, nombre `0xFE`.
#[must_use]
pub fn parse_action0_cargo_meta(payload: &[u8]) -> Option<ParsedCargoMeta> {
    let header = parse_action0_header(payload)?;
    if header.feature != ACTION0_FEATURE_CARGOES || header.num_ids == 0 || payload.len() < 5 {
        return None;
    }
    let local_id = payload[4];
    let mut i = 5usize;
    let mut bitnum = 0u8;
    let mut label = String::new();
    let mut name = String::new();
    for _ in 0..header.num_props {
        if i >= payload.len() {
            break;
        }
        let prop = payload[i];
        i += 1;
        match prop {
            PROP_CARGO_BITNUM => {
                if i >= payload.len() {
                    break;
                }
                bitnum = payload[i];
                i += 1;
            }
            PROP_CARGO_LABEL => {
                let Some(s) = read_four_char_label(payload, &mut i, "CARG") else {
                    break;
                };
                label = s;
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
    if label.is_empty() {
        return None;
    }
    if name.is_empty() {
        name.clone_from(&label);
    }
    Some(ParsedCargoMeta {
        local_id,
        bitnum,
        label,
        name,
    })
}

#[must_use]
pub fn collect_cargo_metas_from_grf(data: &[u8]) -> Vec<ParsedCargoMeta> {
    let mut out = Vec::new();
    let _ = for_each_pseudo_payload(data, |payload| {
        if let Some(meta) = parse_action0_cargo_meta(payload) {
            out.push(meta);
        }
    });
    out
}

/// Parsea Action0 `Objects` (`0x0F`): class label, size, nombre `0xFE`.
#[must_use]
pub fn parse_action0_object_meta(payload: &[u8]) -> Option<ParsedObjectMeta> {
    let header = parse_action0_header(payload)?;
    if header.feature != ACTION0_FEATURE_OBJECTS || header.num_ids == 0 || payload.len() < 5 {
        return None;
    }
    let local_id = payload[4];
    let mut i = 5usize;
    let mut class_label = String::new();
    let mut name = String::new();
    let mut size = crate::object_spec::OBJECT_SIZE_1X1;
    for _ in 0..header.num_props {
        if i >= payload.len() {
            break;
        }
        let prop = payload[i];
        i += 1;
        match prop {
            PROP_LABEL => {
                let Some(s) = read_four_char_label(payload, &mut i, "OBJT") else {
                    break;
                };
                class_label = s;
            }
            PROP_OBJECT_SIZE => {
                if i >= payload.len() {
                    break;
                }
                size = payload[i];
                i += 1;
                let w = size & 0x0F;
                let h = (size >> 4) & 0x0F;
                if w == 0 || h == 0 {
                    size = crate::object_spec::OBJECT_SIZE_1X1;
                }
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
    if class_label.is_empty() {
        return None;
    }
    if name.is_empty() {
        name.clone_from(&class_label);
    }
    Some(ParsedObjectMeta {
        local_id,
        class_label,
        name,
        size,
    })
}

#[must_use]
pub fn collect_object_metas_from_grf(data: &[u8]) -> Vec<ParsedObjectMeta> {
    let mut out = Vec::new();
    let _ = for_each_pseudo_payload(data, |payload| {
        if let Some(meta) = parse_action0_object_meta(payload) {
            out.push(meta);
        }
    });
    out
}

#[must_use]
#[allow(clippy::too_many_lines)]
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
    let mut lifelength_years = 30u8;
    let mut model_life_years = u8::MAX;
    let mut reliability_spd_dec = crate::engine::DEFAULT_RELIABILITY_SPD_DEC;
    let mut climate_mask = 0x0Fu8;
    let mut load_amount = 0u8;
    let mut train_image_index = 2u8;
    let mut dual_headed = false;
    let mut capacity = 0u32;
    let mut cargo = None;
    let mut weight_t = 80u16;
    let mut price_factor = 20u8;
    let mut running_cost_factor = 80u8;
    let mut pow_wag_power = 0u32;
    let mut pow_wag_weight = 0u16;
    let mut rail_tilts = false;
    let mut curve_speed_mod = 0i16;
    for _ in 0..header.num_props {
        if i >= payload.len() {
            break;
        }
        let prop = payload[i];
        i += 1;
        match prop {
            0x00 => {
                if i + 2 > payload.len() {
                    break;
                }
                let days = u16::from_le_bytes([payload[i], payload[i + 1]]);
                intro_year = 1920u16.saturating_add(days / 365);
                i += 2;
            }
            0x02 => {
                reliability_spd_dec = u16::from(read_u8(payload, &mut i)?) << 2;
            }
            0x03 => {
                lifelength_years = read_u8(payload, &mut i)?;
            }
            0x04 => {
                model_life_years = read_u8(payload, &mut i)?;
            }
            0x06 => {
                climate_mask = read_u8(payload, &mut i)?;
            }
            0x07 => {
                load_amount = read_u8(payload, &mut i)?;
            }
            0x0D => {
                running_cost_factor = read_u8(payload, &mut i)?;
            }
            0x12 => {
                let mut spriteid = read_u8(payload, &mut i)?;
                // TTD sprite IDs → índice local (`CUSTOM_VEHICLE_SPRITENUM` = 0xFD).
                if spriteid < 0xFD {
                    spriteid >>= 1;
                }
                train_image_index = spriteid;
            }
            0x13 => {
                dual_headed = read_u8(payload, &mut i)? != 0;
            }
            0x14 => {
                capacity = u32::from(read_u8(payload, &mut i)?);
            }
            0x15 => {
                let ctype = read_u8(payload, &mut i)?;
                cargo = if ctype == 0xFF {
                    None
                } else {
                    crate::cargo::CargoType::from_temperate_id(ctype)
                };
            }
            0x16 => {
                // Upstream: peso low BYTE. Fixtures locales usan WORD año (1800..3000).
                if let Some(year) = payload.get(i..i.checked_add(2)?).and_then(|bytes| {
                    let year = u16::from_le_bytes([bytes[0], bytes[1]]);
                    (1800..3000).contains(&year).then_some(year)
                }) {
                    intro_year = year;
                    i += 2;
                } else {
                    weight_t = (weight_t & 0xFF00) | u16::from(read_u8(payload, &mut i)?);
                }
            }
            0x17 => {
                price_factor = read_u8(payload, &mut i)?;
            }
            0x1B => {
                pow_wag_power = u32::from(read_u16(payload, &mut i)?);
            }
            0x23 => {
                pow_wag_weight = u16::from(read_u8(payload, &mut i)?);
            }
            0x24 => {
                let hi = read_u8(payload, &mut i)?;
                if hi <= 4 {
                    weight_t = (u16::from(hi) << 8) | (weight_t & 0x00FF);
                }
            }
            0x27 => {
                // `EngineMiscFlag::RailTilts` = bit 0.
                rail_tilts = read_u8(payload, &mut i)? & 0x01 != 0;
            }
            0x2E => {
                curve_speed_mod = i16::from_le_bytes(read_u16(payload, &mut i)?.to_le_bytes());
            }
            // Anchos fijos restantes consumidos sin semántica runtime.
            0x08 | 0x0A | 0x0C | 0x0F | 0x10 | 0x11 | 0x18 | 0x19 | 0x1C | 0x1E | 0x1F | 0x20
            | 0x21 | 0x22 | 0x25 | 0x26 | 0x31 => {
                skip_bytes(payload, &mut i, 1)?;
            }
            0x1A => {
                // Extended byte sort order: BYTE en fixtures locales.
                skip_bytes(payload, &mut i, 1)?;
            }
            0x1D | 0x28 | 0x29 | 0x2B | 0x2F => {
                skip_bytes(payload, &mut i, 2)?;
            }
            // 0x0E running cost base; 0x2A/0x30 listas/DWORD: consumidas.
            0x0E | 0x2A | 0x30 => {
                skip_bytes(payload, &mut i, 4)?;
            }
            0x2C | 0x2D => {
                let count = usize::from(read_u8(payload, &mut i)?);
                skip_bytes(payload, &mut i, count)?;
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
        lifelength_years,
        model_life_years,
        reliability_spd_dec,
        climate_mask,
        load_amount,
        train_image_index,
        dual_headed,
        capacity,
        cargo,
        weight_t,
        price_factor,
        running_cost_factor,
        pow_wag_power,
        pow_wag_weight,
        rail_tilts,
        curve_speed_mod,
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

fn read_u8(payload: &[u8], i: &mut usize) -> Option<u8> {
    let value = *payload.get(*i)?;
    *i += 1;
    Some(value)
}

fn read_u16(payload: &[u8], i: &mut usize) -> Option<u16> {
    let bytes = payload.get(*i..i.checked_add(2)?)?;
    *i += 2;
    Some(u16::from_le_bytes(bytes.try_into().ok()?))
}

fn skip_bytes(payload: &[u8], i: &mut usize, amount: usize) -> Option<()> {
    let end = i.checked_add(amount)?;
    payload.get(*i..end)?;
    *i = end;
    Some(())
}

fn parse_common_vehicle_property(
    prop: u8,
    payload: &[u8],
    i: &mut usize,
    metas: &mut [ParsedVehicleMeta],
) -> Option<bool> {
    match prop {
        0x00 => {
            for meta in metas {
                let days = read_u16(payload, i)?;
                meta.intro_year = 1920u16.saturating_add(days / 365);
            }
        }
        0x02 => {
            for meta in metas {
                meta.reliability_spd_dec = u16::from(read_u8(payload, i)?) << 2;
            }
        }
        0x03 => {
            for meta in metas {
                meta.lifelength_years = read_u8(payload, i)?;
            }
        }
        0x04 => {
            for meta in metas {
                meta.model_life_years = read_u8(payload, i)?;
            }
        }
        0x06 => {
            for meta in metas {
                meta.climate_mask = read_u8(payload, i)?;
            }
        }
        0x07 => {
            for meta in metas {
                meta.load_amount = read_u8(payload, i)?;
            }
        }
        _ => return Some(false),
    }
    Some(true)
}

fn parse_road_vehicle_property(
    prop: u8,
    payload: &[u8],
    i: &mut usize,
    metas: &mut [ParsedVehicleMeta],
) -> Option<()> {
    match prop {
        0x08 | 0x15 => {
            for meta in metas {
                meta.max_speed = u16::from(read_u8(payload, i)?).max(1);
            }
        }
        0x09 => {
            for meta in metas {
                meta.running_cost_factor = read_u8(payload, i)?;
            }
        }
        // 0x0A/0x16/0x1F/0x27: dword props aún no mapeadas al runtime.
        0x0A | 0x16 | 0x1F | 0x27 => skip_bytes(payload, i, metas.len().checked_mul(4)?)?,
        // 0x05 translation table; 0x20/0x28 extended byte (fixtures usan BYTE).
        0x05 | 0x0E | 0x12 | 0x17 | 0x18 | 0x19 | 0x1A | 0x1B | 0x1C | 0x20 | 0x21 | 0x23
        | 0x28 => {
            skip_bytes(payload, i, metas.len())?;
        }
        0x0F => {
            for meta in metas {
                meta.capacity = u32::from(read_u8(payload, i)?);
            }
        }
        0x10 => {
            for meta in metas {
                let cargo = crate::cargo::CargoType::from_temperate_id(read_u8(payload, i)?);
                meta.cargo = cargo;
                meta.kind = if cargo == Some(crate::cargo::CargoType::Passengers) {
                    VehicleKind::Bus
                } else {
                    VehicleKind::Truck
                };
            }
        }
        0x11 => {
            for meta in metas {
                meta.price_factor = read_u8(payload, i)?;
            }
        }
        0x13 => {
            for meta in metas {
                meta.power_hp = u32::from(read_u8(payload, i)?) * 10;
            }
        }
        0x14 => {
            for meta in metas {
                meta.weight_t = u16::from(read_u8(payload, i)?).div_ceil(4);
            }
        }
        0x1D | 0x1E | 0x22 | 0x26 | 0x29 => {
            skip_bytes(payload, i, metas.len().checked_mul(2)?)?;
        }
        _ => return None,
    }
    Some(())
}

fn parse_ship_property(
    prop: u8,
    payload: &[u8],
    i: &mut usize,
    metas: &mut [ParsedVehicleMeta],
) -> Option<()> {
    match prop {
        0x08 | 0x09 | 0x10 | 0x12 | 0x13 | 0x14 | 0x15 | 0x16 | 0x17 | 0x1C | 0x24 => {
            skip_bytes(payload, i, metas.len())?;
        }
        0x0A => {
            for meta in metas {
                meta.price_factor = read_u8(payload, i)?;
            }
        }
        0x0B => {
            for meta in metas {
                meta.max_speed = u16::from(read_u8(payload, i)?).max(1);
            }
        }
        0x0C => {
            for meta in metas {
                meta.cargo = crate::cargo::CargoType::from_temperate_id(read_u8(payload, i)?);
            }
        }
        0x0D => {
            for meta in metas {
                meta.capacity = u32::from(read_u16(payload, i)?);
            }
        }
        0x0F => {
            for meta in metas {
                meta.running_cost_factor = read_u8(payload, i)?;
            }
        }
        0x11 | 0x1A | 0x21 => skip_bytes(payload, i, metas.len().checked_mul(4)?)?,
        0x18 | 0x19 | 0x1D | 0x20 | 0x25 => {
            skip_bytes(payload, i, metas.len().checked_mul(2)?)?;
        }
        0x1B | 0x22 => skip_bytes(payload, i, metas.len())?,
        0x23 => {
            for meta in metas {
                meta.max_speed = read_u16(payload, i)?.max(1);
            }
        }
        _ => return None,
    }
    Some(())
}

fn parse_aircraft_property(
    prop: u8,
    payload: &[u8],
    i: &mut usize,
    metas: &mut [ParsedVehicleMeta],
) -> Option<()> {
    match prop {
        0x08 | 0x09 | 0x0A | 0x0D | 0x12 | 0x14 | 0x15 | 0x16 | 0x17 | 0x1B | 0x22 => {
            skip_bytes(payload, i, metas.len())?;
        }
        0x0B => {
            for meta in metas {
                meta.price_factor = read_u8(payload, i)?;
            }
        }
        0x0C => {
            for meta in metas {
                meta.max_speed = (u16::from(read_u8(payload, i)?) * 128 / 10).max(1);
            }
        }
        0x0E => {
            for meta in metas {
                meta.running_cost_factor = read_u8(payload, i)?;
            }
        }
        0x0F => {
            for meta in metas {
                meta.capacity = u32::from(read_u16(payload, i)?);
            }
        }
        0x11 => skip_bytes(payload, i, metas.len())?, // mail capacity: EngineDef has one cargo
        0x13 | 0x1A | 0x21 => skip_bytes(payload, i, metas.len().checked_mul(4)?)?,
        0x18 | 0x19 | 0x1C | 0x1F | 0x20 | 0x23 => {
            skip_bytes(payload, i, metas.len().checked_mul(2)?)?;
        }
        _ => return None,
    }
    Some(())
}

/// Parsea un bloque Action0 completo de road vehicles, ships o aircraft.
#[must_use]
pub fn parse_action0_vehicle_metas(payload: &[u8]) -> Option<Vec<ParsedVehicleMeta>> {
    let header = parse_action0_header(payload)?;
    if !matches!(
        header.feature,
        ACTION0_FEATURE_ROAD_VEHICLES | ACTION0_FEATURE_SHIPS | ACTION0_FEATURE_AIRCRAFT
    ) || header.num_ids == 0
        || payload.len() < 5
    {
        return None;
    }
    let first_id = payload[4];
    let mut metas = (0..header.num_ids)
        .map(|offset| ParsedVehicleMeta::defaults(header.feature, first_id.wrapping_add(offset)))
        .collect::<Option<Vec<_>>>()?;
    let mut i = 5usize;
    for _ in 0..header.num_props {
        let prop = read_u8(payload, &mut i)?;
        if parse_common_vehicle_property(prop, payload, &mut i, &mut metas)? {
            continue;
        }
        match header.feature {
            ACTION0_FEATURE_ROAD_VEHICLES => {
                parse_road_vehicle_property(prop, payload, &mut i, &mut metas)?;
            }
            ACTION0_FEATURE_SHIPS => parse_ship_property(prop, payload, &mut i, &mut metas)?,
            ACTION0_FEATURE_AIRCRAFT => parse_aircraft_property(prop, payload, &mut i, &mut metas)?,
            _ => return None,
        }
    }
    Some(metas)
}

#[must_use]
pub fn collect_vehicle_metas_from_grf(data: &[u8], feature: u8) -> Vec<ParsedVehicleMeta> {
    let mut out = Vec::new();
    let _ = for_each_pseudo_payload(data, |payload| {
        if payload.get(1) == Some(&feature)
            && let Some(metas) = parse_action0_vehicle_metas(payload)
        {
            out.extend(metas);
        }
    });
    out
}
