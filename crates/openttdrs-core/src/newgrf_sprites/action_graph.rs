//! Parseo y construcción de grafos Action1/2/3 + builders sintéticos de GRF.

use crate::newgrf_actions::{
    ACTION0_FEATURE_AIRCRAFT, ACTION0_FEATURE_CANALS, ACTION0_FEATURE_HOUSES,
    ACTION0_FEATURE_INDUSTRYTILES, ACTION0_FEATURE_OBJECTS, ACTION0_FEATURE_RAILTYPES,
    ACTION0_FEATURE_ROAD_VEHICLES, ACTION0_FEATURE_ROADSTOPS, ACTION0_FEATURE_ROADTYPES,
    ACTION0_FEATURE_SHIPS, ACTION0_FEATURE_STATIONS, ACTION0_FEATURE_TRAINS,
};
use crate::newgrf_config::{GrfContainerVersion, GrfScanError, parse_grf_full};
use crate::newgrf_walk::{GrfEntry, walk_grf_entries};

use super::model::{
    Action2RandomEntry, Action2VarAdjust, Action2VarEntry, Action2VarOp, Action2VarTerm,
    DecodedSprite, TrainSpriteAssign, TrainSpriteGraphics,
};
use super::pixel_codec::{decode_real_sprite_entry, index_sprite_section, resolve_fd_sprite};

fn parse_action1_feature(payload: &[u8], feature: u8) -> Option<(u8, u8)> {
    // 01 <feature> <num-sets> <num-ent>
    if payload.len() < 4 || payload[0] != 0x01 {
        return None;
    }
    if payload[1] != feature {
        return None;
    }
    let num_sets = payload[2];
    let num_ent = payload[3];
    if num_sets == 0 || num_ent == 0 {
        return None;
    }
    Some((num_sets, num_ent))
}

/// Action2 básico (vehículos / stations / roadtypes): `02 <feat> <set-id> <n1> <n2> <words…>`.
///
/// Devuelve `(action2_set_id, primer set Action1)`. Variational (`n1≥0x80`) → None.
/// Roadtypes usan `01 00` (un set); stations pueden tener `n1=0` y `n2>0`.
pub(super) fn parse_action2_basic(payload: &[u8], feature: u8) -> Option<(u8, u16)> {
    if payload.len() < 5 || payload[0] != 0x02 {
        return None;
    }
    if payload[1] != feature {
        return None;
    }
    let set_id = payload[2];
    let num_ent1 = payload[3];
    let num_ent2 = payload[4];
    // Variational / random Action2 → `parse_action2_variational` / `parse_action2_random`.
    if num_ent1 >= 0x80 {
        return None;
    }
    let n_words = usize::from(num_ent1) + usize::from(num_ent2);
    if n_words == 0 {
        return None;
    }
    let words_start = 5usize;
    let words_end = words_start.checked_add(n_words.checked_mul(2)?)?;
    if payload.len() < words_end {
        return None;
    }
    let a1 = u16::from_le_bytes([payload[words_start], payload[words_start + 1]]);
    Some((set_id, a1))
}

/// Lee `variable` [+param `60+x`] + `varadjust`. Devuelve `(término, bit5_continúa)`.
fn parse_var_term(payload: &[u8], i: &mut usize) -> Option<(Action2VarTerm, bool)> {
    if *i >= payload.len() {
        return None;
    }
    let variable = payload[*i];
    *i += 1;
    let param = if (0x60..=0x7F).contains(&variable) {
        if *i >= payload.len() {
            return None;
        }
        let p = payload[*i];
        *i += 1;
        Some(p)
    } else {
        None
    };
    if *i >= payload.len() {
        return None;
    }
    let shift_num = payload[*i];
    *i += 1;
    let continued = shift_num & 0x20 != 0;
    let do_divide = shift_num & 0x40 != 0;
    let do_modulo = shift_num & 0x80 != 0;
    if do_divide && do_modulo {
        return None;
    }
    if *i >= payload.len() {
        return None;
    }
    let and_mask = payload[*i];
    *i += 1;
    let mut add_val = None;
    let mut divide_val = None;
    let mut modulo_val = None;
    if do_divide || do_modulo {
        if *i + 2 > payload.len() {
            return None;
        }
        add_val = Some(payload[*i]);
        let operand = payload[*i + 1];
        *i += 2;
        if do_divide {
            divide_val = Some(operand);
        } else {
            modulo_val = Some(operand);
        }
    }
    Some((
        Action2VarTerm {
            variable,
            param,
            adjust: Action2VarAdjust {
                shift: shift_num & 0x1F,
                and_mask,
                add_val,
                divide_val,
                modulo_val,
            },
        },
        continued,
    ))
}

/// Action2 variational `0x81`/`0x82` (byte): simple, divide/modulo o advanced (bit 5).
pub(super) fn parse_action2_variational(
    payload: &[u8],
    feature: u8,
) -> Option<(u8, Action2VarEntry)> {
    if payload.len() < 8 || payload[0] != 0x02 || payload[1] != feature {
        return None;
    }
    let set_id = payload[2];
    let typ = payload[3];
    if typ != 0x81 && typ != 0x82 {
        return None;
    }
    let mut i = 4usize;
    let (first, mut continued) = parse_var_term(payload, &mut i)?;
    let mut ops = Vec::new();
    while continued {
        if i >= payload.len() {
            return None;
        }
        let operator = payload[i];
        i += 1;
        let (rhs, next) = parse_var_term(payload, &mut i)?;
        ops.push(Action2VarOp { operator, rhs });
        continued = next;
        if ops.len() > 32 {
            return None;
        }
    }
    if i >= payload.len() {
        return None;
    }
    let nvar = payload[i];
    i += 1;
    let mut ranges = Vec::with_capacity(usize::from(nvar));
    for _ in 0..nvar {
        if i + 4 > payload.len() {
            return None;
        }
        let result = u16::from_le_bytes([payload[i], payload[i + 1]]);
        let low = payload[i + 2];
        let high = payload[i + 3];
        ranges.push((result, low, high));
        i += 4;
    }
    if i + 2 > payload.len() {
        return None;
    }
    let default = u16::from_le_bytes([payload[i], payload[i + 1]]);
    Some((
        set_id,
        Action2VarEntry {
            first,
            ops,
            ranges,
            default,
        },
    ))
}

/// Action2 random `0x80`/`0x83`/`0x84`: triggers + randbit + n sets (potencia de 2).
pub(super) fn parse_action2_random(
    payload: &[u8],
    feature: u8,
) -> Option<(u8, Action2RandomEntry)> {
    if payload.len() < 8 || payload[0] != 0x02 || payload[1] != feature {
        return None;
    }
    let set_id = payload[2];
    let typ = payload[3];
    if typ != 0x80 && typ != 0x83 && typ != 0x84 {
        return None;
    }
    let mut i = 4usize;
    let consist_count = if typ == 0x84 {
        if i >= payload.len() {
            return None;
        }
        let c = payload[i];
        i += 1;
        c
    } else {
        0
    };
    if i + 3 > payload.len() {
        return None;
    }
    let triggers = payload[i];
    let randbit = payload[i + 1];
    let nrand = payload[i + 2];
    i += 3;
    if nrand == 0 || !nrand.is_power_of_two() {
        return None;
    }
    let n = usize::from(nrand);
    let words_end = i.checked_add(n.checked_mul(2)?)?;
    if payload.len() < words_end {
        return None;
    }
    let mut sets = Vec::with_capacity(n);
    for k in 0..n {
        let o = i + k * 2;
        sets.push(u16::from_le_bytes([payload[o], payload[o + 1]]));
    }
    Some((
        set_id,
        Action2RandomEntry {
            typ,
            consist_count,
            triggers,
            randbit,
            sets,
        },
    ))
}

type ParsedAction3 = (Vec<TrainSpriteAssign>, Vec<((u8, u8), u16)>);

pub(super) fn parse_action3_feature(payload: &[u8], feature: u8) -> Option<ParsedAction3> {
    // 03 <feature> <n-id> <ids…> <num-cid> [cargo…] <default:u16>
    if payload.len() < 6 || payload[0] != 0x03 {
        return None;
    }
    if payload[1] != feature {
        return None;
    }
    let n_id = payload[2];
    if n_id == 0 {
        return None;
    }
    let ids_end = 3 + usize::from(n_id);
    if payload.len() < ids_end + 1 + 2 {
        return None;
    }
    let ids = &payload[3..ids_end];
    let num_cid = payload[ids_end];
    let mut i = ids_end + 1;
    let mut specific = Vec::with_capacity(usize::from(num_cid) * ids.len());
    for _ in 0..num_cid {
        if i + 3 > payload.len() {
            return None;
        }
        let selector = payload[i];
        let set_id = u16::from_le_bytes([payload[i + 1], payload[i + 2]]);
        for &local_id in ids {
            specific.push(((local_id, selector), set_id));
        }
        i += 3;
    }
    if i + 2 > payload.len() {
        return None;
    }
    let default_set = u16::from_le_bytes([payload[i], payload[i + 1]]);
    let defaults = ids
        .iter()
        .map(|&local_id| TrainSpriteAssign {
            local_id,
            set_id: default_set,
        })
        .collect();
    Some((defaults, specific))
}

/// Índice sprite section v2: `id` → lista `(info, body)` (body = tras el BYTE info).
#[must_use]
/// Features con cadena Action3→Action2→Action1 (trains / stations / roadtypes / industrytile).
fn supports_action2_chain(feature: u8) -> bool {
    matches!(
        feature,
        ACTION0_FEATURE_TRAINS
            | ACTION0_FEATURE_ROAD_VEHICLES
            | ACTION0_FEATURE_SHIPS
            | ACTION0_FEATURE_AIRCRAFT
            | ACTION0_FEATURE_STATIONS
            | ACTION0_FEATURE_OBJECTS
            | ACTION0_FEATURE_RAILTYPES
            | ACTION0_FEATURE_ROADTYPES
            | ACTION0_FEATURE_ROADSTOPS
            | ACTION0_FEATURE_INDUSTRYTILES
            | ACTION0_FEATURE_HOUSES
    )
}

/// Recorre el GRF y extrae sets Action1 + Action2 + asignaciones Action3.
///
/// # Errors
///
/// Contenedor inválido.
#[allow(clippy::too_many_lines)]
pub fn collect_feature_sprite_graphics(
    data: &[u8],
    feature: u8,
) -> Result<TrainSpriteGraphics, GrfScanError> {
    let parsed = parse_grf_full(data)?;
    let container = parsed.container;
    let section = parsed.data_section;
    let sprite_index = index_sprite_section(parsed.sprite_section);
    let mut out = TrainSpriteGraphics::default();
    let mut current_set: Vec<DecodedSprite> = Vec::new();
    let mut views_left_in_set = 0u8;
    let mut sets_left = 0u8;
    let mut views_per_set = 0u8;

    walk_grf_entries(section, container, |entry| match entry {
        GrfEntry::Pseudo(payload) => {
            if let Some((ns, ne)) = parse_action1_feature(payload, feature) {
                if !current_set.is_empty() {
                    out.sets.push(std::mem::take(&mut current_set));
                }
                sets_left = ns;
                views_per_set = ne;
                views_left_in_set = ne;
            } else if supports_action2_chain(feature)
                && let Some((a2_id, a1_idx)) = parse_action2_basic(payload, feature)
            {
                out.action2_to_action1.insert(a2_id, a1_idx);
            } else if supports_action2_chain(feature)
                && let Some((a2_id, var)) = parse_action2_variational(payload, feature)
            {
                out.action2_var.insert(a2_id, var);
            } else if supports_action2_chain(feature)
                && let Some((a2_id, rnd)) = parse_action2_random(payload, feature)
            {
                out.action2_random.insert(a2_id, rnd);
            } else if let Some((assigns, specific)) = parse_action3_feature(payload, feature) {
                out.assigns.extend(assigns);
                out.specific_assigns.extend(specific);
            }
        }
        GrfEntry::Real { info, payload } => {
            if sets_left > 0 || views_left_in_set > 0 {
                let decoded = if container == GrfContainerVersion::V2 && info == 0xFD {
                    if payload.len() >= 4 {
                        let id =
                            u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);
                        resolve_fd_sprite(&sprite_index, id)
                    } else {
                        None
                    }
                } else {
                    decode_real_sprite_entry(container, info, payload)
                };
                if let Some(decoded) = decoded {
                    current_set.push(decoded);
                    views_left_in_set = views_left_in_set.saturating_sub(1);
                    if views_left_in_set == 0 {
                        out.sets.push(std::mem::take(&mut current_set));
                        sets_left = sets_left.saturating_sub(1);
                        if sets_left > 0 {
                            views_left_in_set = views_per_set;
                        }
                    }
                }
            }
        }
    });

    if !current_set.is_empty() {
        out.sets.push(current_set);
    }
    Ok(out)
}

/// Action1/3 trains.
///
/// # Errors
///
/// Contenedor inválido.
pub fn collect_train_sprite_graphics(data: &[u8]) -> Result<TrainSpriteGraphics, GrfScanError> {
    collect_feature_sprite_graphics(data, ACTION0_FEATURE_TRAINS)
}

/// Action1/2/3 road vehicles, incluido el selector Action3 por cargo.
pub fn collect_road_vehicle_sprite_graphics(
    data: &[u8],
) -> Result<TrainSpriteGraphics, GrfScanError> {
    collect_feature_sprite_graphics(data, ACTION0_FEATURE_ROAD_VEHICLES)
}

/// Action1/2/3 ships, incluido el selector Action3 por cargo.
pub fn collect_ship_sprite_graphics(data: &[u8]) -> Result<TrainSpriteGraphics, GrfScanError> {
    collect_feature_sprite_graphics(data, ACTION0_FEATURE_SHIPS)
}

/// Action1/2/3 aircraft, incluido el selector Action3 por cargo.
pub fn collect_aircraft_sprite_graphics(data: &[u8]) -> Result<TrainSpriteGraphics, GrfScanError> {
    collect_feature_sprite_graphics(data, ACTION0_FEATURE_AIRCRAFT)
}

/// Action1/3 roadtypes.
///
/// # Errors
///
/// Contenedor inválido.
pub fn collect_roadtype_sprite_graphics(data: &[u8]) -> Result<TrainSpriteGraphics, GrfScanError> {
    collect_feature_sprite_graphics(data, ACTION0_FEATURE_ROADTYPES)
}

/// Action1/2/3 `RailTypes` (`0x10`), incluidos grupos Action3 por `RailSpriteType`.
///
/// # Errors
///
/// Contenedor inválido.
pub fn collect_railtype_sprite_graphics(data: &[u8]) -> Result<TrainSpriteGraphics, GrfScanError> {
    collect_feature_sprite_graphics(data, ACTION0_FEATURE_RAILTYPES)
}

/// Action1/3 stations.
///
/// # Errors
///
/// Contenedor inválido.
pub fn collect_station_sprite_graphics(data: &[u8]) -> Result<TrainSpriteGraphics, GrfScanError> {
    collect_feature_sprite_graphics(data, ACTION0_FEATURE_STATIONS)
}

/// Action1/3 industry tiles.
///
/// # Errors
///
/// Contenedor inválido.
pub fn collect_industry_tile_sprite_graphics(
    data: &[u8],
) -> Result<TrainSpriteGraphics, GrfScanError> {
    collect_feature_sprite_graphics(data, ACTION0_FEATURE_INDUSTRYTILES)
}

/// Action1/3 houses (`0x07`).
///
/// # Errors
///
/// Contenedor inválido.
pub fn collect_house_sprite_graphics(data: &[u8]) -> Result<TrainSpriteGraphics, GrfScanError> {
    collect_feature_sprite_graphics(data, ACTION0_FEATURE_HOUSES)
}

/// Action1/3 objects.
///
/// # Errors
///
/// Contenedor inválido.
pub fn collect_object_sprite_graphics(data: &[u8]) -> Result<TrainSpriteGraphics, GrfScanError> {
    collect_feature_sprite_graphics(data, ACTION0_FEATURE_OBJECTS)
}

/// Action1/3 road stops.
///
/// # Errors
///
/// Contenedor inválido.
pub fn collect_roadstop_sprite_graphics(data: &[u8]) -> Result<TrainSpriteGraphics, GrfScanError> {
    collect_feature_sprite_graphics(data, ACTION0_FEATURE_ROADSTOPS)
}

/// Action1/3 canals (`0x05`).
///
/// # Errors
///
/// Contenedor inválido.
pub fn collect_canal_sprite_graphics(data: &[u8]) -> Result<TrainSpriteGraphics, GrfScanError> {
    collect_feature_sprite_graphics(data, ACTION0_FEATURE_CANALS)
}
