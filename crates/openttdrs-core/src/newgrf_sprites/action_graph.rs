//! Parseo y construcción de grafos Action1/2/3 + builders sintéticos de GRF.

use crate::newgrf_actions::{
    ACTION0_FEATURE_INDUSTRYTILES, ACTION0_FEATURE_ROADTYPES, ACTION0_FEATURE_STATIONS,
    ACTION0_FEATURE_TRAINS,
};
use crate::newgrf_config::{GrfContainerVersion, GrfScanError, parse_grf_full};
use crate::newgrf_walk::{GrfEntry, walk_grf_entries};

use super::model::{
    Action2RandomEntry, Action2VarAdjust, Action2VarEntry, Action2VarOp, Action2VarTerm,
    TrainSpriteAssign, TrainSpriteGraphics, DecodedSprite,
};
use super::pixel_codec::{
    build_real_sprite_v1_chunked_payload, build_real_sprite_v1_compressed_payload,
    build_real_sprite_v1_uncompressed_payload, build_sprite_section_palette_entry,
    build_sprite_section_rgba_entry, decode_real_sprite_entry,
    index_sprite_section, resolve_fd_sprite,
};

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
pub(super) fn parse_action2_variational(payload: &[u8], feature: u8) -> Option<(u8, Action2VarEntry)> {
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
pub(super) fn parse_action2_random(payload: &[u8], feature: u8) -> Option<(u8, Action2RandomEntry)> {
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

pub(super) fn parse_action3_feature(payload: &[u8], feature: u8) -> Option<Vec<TrainSpriteAssign>> {
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
    // Saltar pares cargo (1+2 bytes) — MVP no los usa.
    let mut i = ids_end + 1;
    for _ in 0..num_cid {
        if i + 3 > payload.len() {
            return None;
        }
        i += 3; // cargo:u8 + set:u16
    }
    if i + 2 > payload.len() {
        return None;
    }
    let default_set = u16::from_le_bytes([payload[i], payload[i + 1]]);
    Some(
        ids.iter()
            .map(|&local_id| TrainSpriteAssign {
                local_id,
                set_id: default_set,
            })
            .collect(),
    )
}

/// Índice sprite section v2: `id` → lista `(info, body)` (body = tras el BYTE info).
#[must_use]
/// Features con cadena Action3→Action2→Action1 (trains / stations / roadtypes / industrytile).
fn supports_action2_chain(feature: u8) -> bool {
    matches!(
        feature,
        ACTION0_FEATURE_TRAINS
            | ACTION0_FEATURE_STATIONS
            | ACTION0_FEATURE_ROADTYPES
            | ACTION0_FEATURE_INDUSTRYTILES
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
            } else if let Some(assigns) = parse_action3_feature(payload, feature) {
                out.assigns.extend(assigns);
            }
        }
        GrfEntry::Real { info, payload } => {
            if sets_left > 0 || views_left_in_set > 0 {
                let decoded = if container == GrfContainerVersion::V2 && info == 0xFD {
                    if payload.len() >= 4 {
                        let id = u32::from_le_bytes([
                            payload[0],
                            payload[1],
                            payload[2],
                            payload[3],
                        ]);
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

/// Action1/3 roadtypes.
///
/// # Errors
///
/// Contenedor inválido.
pub fn collect_roadtype_sprite_graphics(data: &[u8]) -> Result<TrainSpriteGraphics, GrfScanError> {
    collect_feature_sprite_graphics(data, ACTION0_FEATURE_ROADTYPES)
}

/// Action1: 1 set × `num_ent` vistas para un feature.
#[must_use]
pub fn build_action1_feature_payload(feature: u8, num_sets: u8, num_ent: u8) -> Vec<u8> {
    vec![0x01, feature, num_sets, num_ent]
}

/// Action1 trains: 1 set × `num_ent` vistas.
#[must_use]
pub fn build_action1_trains_payload(num_sets: u8, num_ent: u8) -> Vec<u8> {
    build_action1_feature_payload(ACTION0_FEATURE_TRAINS, num_sets, num_ent)
}

/// Action3: un id local → set por defecto (sin cargos).
#[must_use]
pub fn build_action3_feature_payload(feature: u8, local_id: u8, default_set: u16) -> Vec<u8> {
    let mut p = vec![0x03, feature, 0x01, local_id, 0x00];
    p.extend_from_slice(&default_set.to_le_bytes());
    p
}

/// Action3 trains: un id local → set por defecto (sin cargos).
#[must_use]
pub fn build_action3_trains_payload(local_id: u8, default_set: u16) -> Vec<u8> {
    build_action3_feature_payload(ACTION0_FEATURE_TRAINS, local_id, default_set)
}

/// Action2 vehículo básico: 1 estado moving + 1 loading → mismos/distintos sets Action1.
#[must_use]
pub fn build_action2_vehicle_payload(
    feature: u8,
    set_id: u8,
    action1_moving: u16,
    action1_loading: u16,
) -> Vec<u8> {
    let mut p = vec![0x02, feature, set_id, 0x01, 0x01];
    p.extend_from_slice(&action1_moving.to_le_bytes());
    p.extend_from_slice(&action1_loading.to_le_bytes());
    p
}

/// Action2 trains: set-id → Action1 moving/loading.
#[must_use]
pub fn build_action2_trains_payload(
    set_id: u8,
    action1_moving: u16,
    action1_loading: u16,
) -> Vec<u8> {
    build_action2_vehicle_payload(
        ACTION0_FEATURE_TRAINS,
        set_id,
        action1_moving,
        action1_loading,
    )
}

/// Action2 single-set (roadtypes/canals/…): `01 00` + un set Action1.
#[must_use]
pub fn build_action2_single_set_payload(feature: u8, set_id: u8, action1_set: u16) -> Vec<u8> {
    let mut p = vec![0x02, feature, set_id, 0x01, 0x00];
    p.extend_from_slice(&action1_set.to_le_bytes());
    p
}

/// Action2 stations: `numlittlesets=0`, `numlotssets=1` → un set Action1.
#[must_use]
pub fn build_action2_stations_payload(set_id: u8, action1_set: u16) -> Vec<u8> {
    let mut p = vec![0x02, ACTION0_FEATURE_STATIONS, set_id, 0x00, 0x01];
    p.extend_from_slice(&action1_set.to_le_bytes());
    p
}

/// Action2 variational `0x81` con rangos opcionales (sin divide/modulo).
#[must_use]
pub fn build_action2_variational_payload(
    feature: u8,
    set_id: u8,
    variable: u8,
    shift: u8,
    and_mask: u8,
    ranges: &[(u16, u8, u8)],
    default_set: u16,
) -> Vec<u8> {
    build_action2_variational_divmod_payload(
        feature,
        set_id,
        variable,
        shift & 0x1F,
        and_mask,
        None,
        None,
        None,
        ranges,
        default_set,
    )
}

/// Action2 variational con add+divide o add+modulo (`shift` bits 6/7).
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn build_action2_variational_divmod_payload(
    feature: u8,
    set_id: u8,
    variable: u8,
    shift: u8,
    and_mask: u8,
    add_val: Option<u8>,
    divide_val: Option<u8>,
    modulo_val: Option<u8>,
    ranges: &[(u16, u8, u8)],
    default_set: u16,
) -> Vec<u8> {
    let mut shift_num = shift & 0x1F;
    if divide_val.is_some() {
        shift_num |= 0x40;
    } else if modulo_val.is_some() {
        shift_num |= 0x80;
    }
    let mut p = vec![0x02, feature, set_id, 0x81, variable, shift_num, and_mask];
    if let Some(add) = add_val {
        p.push(add);
        p.push(divide_val.or(modulo_val).unwrap_or(1));
    }
    p.push(u8::try_from(ranges.len()).unwrap_or(0));
    for &(result, low, high) in ranges {
        p.extend_from_slice(&result.to_le_bytes());
        p.push(low);
        p.push(high);
    }
    p.extend_from_slice(&default_set.to_le_bytes());
    p
}

/// Advanced variational: `variable` `+` literal `0x1A` (bit 5 en el primer término).
///
/// Cadena: `var (shift|0x20) and` → op `0x00` (+) → `0x1A shift=0 and=literal`.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn build_action2_variational_advanced_add_literal(
    feature: u8,
    set_id: u8,
    variable: u8,
    and_mask_var: u8,
    literal: u8,
    ranges: &[(u16, u8, u8)],
    default_set: u16,
) -> Vec<u8> {
    let mut p = vec![
        0x02,
        feature,
        set_id,
        0x81,
        variable,
        0x20, // shift 0 + bit 5 (continúa)
        and_mask_var,
        0x00, // +
        0x1A, // literal
        0x00, // shift 0, sin continuar
        literal,
    ];
    p.push(u8::try_from(ranges.len()).unwrap_or(0));
    for &(result, low, high) in ranges {
        p.extend_from_slice(&result.to_le_bytes());
        p.push(low);
        p.push(high);
    }
    p.extend_from_slice(&default_set.to_le_bytes());
    p
}

/// Action2 variational `nvar=0`: devuelve el literal `value` como resultado de callback.
///
/// Usa variable `0x1A` + `and_mask = value` (valor constante).
#[must_use]
pub fn build_action2_callback_literal_payload(feature: u8, set_id: u8, value: u8) -> Vec<u8> {
    build_action2_variational_payload(feature, set_id, 0x1A, 0x00, value, &[], 0)
}

/// Action2 variational `0x81` que siempre elige `default_set` (rango catch-all).
///
/// Nota: `nvar=0` en la spec es resultado de callback (p. ej. procedures `7E`),
/// no “usar default”; por eso aquí se emite un rango `0..=0xFF`.
#[must_use]
pub fn build_action2_variational_default_payload(
    feature: u8,
    set_id: u8,
    default_set: u16,
) -> Vec<u8> {
    build_action2_variational_payload(
        feature,
        set_id,
        0x00,
        0x00,
        0xFF,
        &[(default_set, 0, 0xFF)],
        default_set,
    )
}

/// Action2 variational trains → `default_set`.
#[must_use]
pub fn build_action2_trains_variational_default(set_id: u8, default_set: u16) -> Vec<u8> {
    build_action2_variational_default_payload(ACTION0_FEATURE_TRAINS, set_id, default_set)
}

/// Action2 random `0x80` trains.
#[must_use]
pub fn build_action2_trains_random(set_id: u8, randbit: u8, sets: &[u16]) -> Vec<u8> {
    let n = u8::try_from(sets.len()).unwrap_or(0);
    let mut p = vec![0x02, ACTION0_FEATURE_TRAINS, set_id, 0x80, 0x00, randbit, n];
    for &s in sets {
        p.extend_from_slice(&s.to_le_bytes());
    }
    p
}

/// Action2 random `0x84` trains (consist): `count` + triggers + randbit + sets.
#[must_use]
pub fn build_action2_trains_random_consist(
    set_id: u8,
    consist_count: u8,
    randbit: u8,
    sets: &[u16],
) -> Vec<u8> {
    let n = u8::try_from(sets.len()).unwrap_or(0);
    let mut p = vec![
        0x02,
        ACTION0_FEATURE_TRAINS,
        set_id,
        0x84,
        consist_count,
        0x00,
        randbit,
        n,
    ];
    for &s in sets {
        p.extend_from_slice(&s.to_le_bytes());
    }
    p
}

/// Append sprite real v2: `DWORD size` + `info` + payload (sin type duplicado).
pub(super) fn append_v2_real_sprite(data_section: &mut Vec<u8>, info: u8, payload: &[u8]) {
    let sz = u32::try_from(payload.len()).unwrap_or(0);
    data_section.extend_from_slice(&sz.to_le_bytes());
    data_section.push(info);
    data_section.extend_from_slice(payload);
}

/// GRF v2 sintético: Action0 + Action1 + sprite(s) + Action3 + Action8.
#[must_use]
#[expect(clippy::too_many_arguments)]
pub fn build_grf_v2_with_preview_sprite(
    action0: &[u8],
    feature: u8,
    local_id: u8,
    width: u16,
    height: u16,
    indices: &[u8],
    grfid: [u8; 4],
    name: &str,
) -> Vec<u8> {
    const SIG: [u8; 8] = [b'G', b'R', b'F', 0x82, 0x0D, 0x0A, 0x1A, 0x0A];
    let action1 = build_action1_feature_payload(feature, 1, 1);
    let action3 = build_action3_feature_payload(feature, local_id, 0);
    let mut action8 = vec![0x08, 0x07];
    action8.extend_from_slice(&grfid);
    action8.extend_from_slice(name.as_bytes());
    action8.push(0);
    action8.push(0);

    let sprite_body = build_real_sprite_v1_uncompressed_payload(
        width,
        height,
        -i16::try_from(width / 2).unwrap_or(0),
        -i16::try_from(height).unwrap_or(0),
        indices,
    );

    let mut data_section = Vec::new();
    for payload in [action0, action1.as_slice()] {
        let sz = u32::try_from(payload.len()).unwrap_or(0);
        data_section.extend_from_slice(&sz.to_le_bytes());
        data_section.push(0xFF);
        data_section.extend_from_slice(payload);
    }
    append_v2_real_sprite(&mut data_section, 0x01, &sprite_body);

    for payload in [action3.as_slice(), action8.as_slice()] {
        let sz = u32::try_from(payload.len()).unwrap_or(0);
        data_section.extend_from_slice(&sz.to_le_bytes());
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

/// GRF v2 sintético: Action0 train + Action1 + sprite(s) + Action3 + Action8.
#[must_use]
pub fn build_grf_v2_train_with_preview_sprite(
    action0: &[u8],
    local_id: u8,
    width: u16,
    height: u16,
    indices: &[u8],
    grfid: [u8; 4],
    name: &str,
) -> Vec<u8> {
    build_grf_v2_with_preview_sprite(
        action0,
        ACTION0_FEATURE_TRAINS,
        local_id,
        width,
        height,
        indices,
        grfid,
        name,
    )
}

/// GRF v2 train con sprite LZ77 (`info=0x03`).
#[must_use]
pub fn build_grf_v2_train_with_compressed_sprite(
    action0: &[u8],
    local_id: u8,
    width: u16,
    height: u16,
    indices: &[u8],
    grfid: [u8; 4],
    name: &str,
) -> Vec<u8> {
    build_grf_v2_train_with_sprite_info(
        action0,
        local_id,
        0x03,
        &build_real_sprite_v1_compressed_payload(
            width,
            height,
            -i16::try_from(width / 2).unwrap_or(0),
            -i16::try_from(height).unwrap_or(0),
            indices,
        ),
        grfid,
        name,
    )
}

/// GRF v2 canónico: Action1 + ref `0xFD` → sprite section (sin sprite inline).
#[must_use]
#[expect(clippy::too_many_arguments)]
pub fn build_grf_v2_train_with_fd_sprite(
    action0: &[u8],
    local_id: u8,
    sprite_id: u32,
    width: u16,
    height: u16,
    indices: &[u8],
    grfid: [u8; 4],
    name: &str,
) -> Vec<u8> {
    const SIG: [u8; 8] = [b'G', b'R', b'F', 0x82, 0x0D, 0x0A, 0x1A, 0x0A];
    let action1 = build_action1_trains_payload(1, 1);
    let action3 = build_action3_trains_payload(local_id, 0);
    let mut action8 = vec![0x08, 0x07];
    action8.extend_from_slice(&grfid);
    action8.extend_from_slice(name.as_bytes());
    action8.push(0);
    action8.push(0);

    let mut data_section = Vec::new();
    for payload in [action0, action1.as_slice()] {
        let sz = u32::try_from(payload.len()).unwrap_or(0);
        data_section.extend_from_slice(&sz.to_le_bytes());
        data_section.push(0xFF);
        data_section.extend_from_slice(payload);
    }
    // Ref 0xFD → sprite_id
    data_section.extend_from_slice(&4u32.to_le_bytes());
    data_section.push(0xFD);
    data_section.extend_from_slice(&sprite_id.to_le_bytes());

    for payload in [action3.as_slice(), action8.as_slice()] {
        let sz = u32::try_from(payload.len()).unwrap_or(0);
        data_section.extend_from_slice(&sz.to_le_bytes());
        data_section.push(0xFF);
        data_section.extend_from_slice(payload);
    }
    data_section.extend_from_slice(&0u32.to_le_bytes());

    let mut sprite_section = build_sprite_section_palette_entry(
        sprite_id,
        0,
        width,
        height,
        -i16::try_from(width / 2).unwrap_or(0),
        -i16::try_from(height).unwrap_or(0),
        indices,
    );
    sprite_section.extend_from_slice(&0u32.to_le_bytes());

    let sprite_offs = u32::try_from(1 + data_section.len()).unwrap_or(0);
    let mut out = Vec::new();
    out.extend_from_slice(&[0x00, 0x00]);
    out.extend_from_slice(&SIG);
    out.extend_from_slice(&sprite_offs.to_le_bytes());
    out.push(0x00);
    out.extend_from_slice(&data_section);
    out.extend_from_slice(&sprite_section);
    out
}

/// GRF v2 train con sprite chunked (`info=0x09`).
#[must_use]
pub fn build_grf_v2_train_with_chunked_sprite(
    action0: &[u8],
    local_id: u8,
    width: u16,
    height: u16,
    indices: &[u8],
    grfid: [u8; 4],
    name: &str,
) -> Option<Vec<u8>> {
    let payload = build_real_sprite_v1_chunked_payload(
        width,
        height,
        -i16::try_from(width / 2).unwrap_or(0),
        -i16::try_from(height).unwrap_or(0),
        indices,
    )?;
    Some(build_grf_v2_train_with_sprite_info(
        action0, local_id, 0x09, &payload, grfid, name,
    ))
}

fn build_grf_v2_train_with_sprite_info(
    action0: &[u8],
    local_id: u8,
    info: u8,
    sprite_body: &[u8],
    grfid: [u8; 4],
    name: &str,
) -> Vec<u8> {
    const SIG: [u8; 8] = [b'G', b'R', b'F', 0x82, 0x0D, 0x0A, 0x1A, 0x0A];
    let action1 = build_action1_trains_payload(1, 1);
    let action3 = build_action3_trains_payload(local_id, 0);
    let mut action8 = vec![0x08, 0x07];
    action8.extend_from_slice(&grfid);
    action8.extend_from_slice(name.as_bytes());
    action8.push(0);
    action8.push(0);

    let mut data_section = Vec::new();
    for payload in [action0, action1.as_slice()] {
        let sz = u32::try_from(payload.len()).unwrap_or(0);
        data_section.extend_from_slice(&sz.to_le_bytes());
        data_section.push(0xFF);
        data_section.extend_from_slice(payload);
    }
    append_v2_real_sprite(&mut data_section, info, sprite_body);

    for payload in [action3.as_slice(), action8.as_slice()] {
        let sz = u32::try_from(payload.len()).unwrap_or(0);
        data_section.extend_from_slice(&sz.to_le_bytes());
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

/// GRF v2: Action0 train + Action1 + sprite + Action2 + Action3 + Action8.
///
/// Action3 apunta a `action2_set_id` (≠ índice Action1); la cadena resuelve al set 0.
#[must_use]
#[expect(clippy::too_many_arguments)]
pub fn build_grf_v2_train_with_action2_chain(
    action0: &[u8],
    local_id: u8,
    action2_set_id: u8,
    width: u16,
    height: u16,
    indices: &[u8],
    grfid: [u8; 4],
    name: &str,
) -> Vec<u8> {
    const SIG: [u8; 8] = [b'G', b'R', b'F', 0x82, 0x0D, 0x0A, 0x1A, 0x0A];
    let action1 = build_action1_trains_payload(1, 1);
    let action2 = build_action2_trains_payload(action2_set_id, 0, 0);
    let action3 = build_action3_trains_payload(local_id, u16::from(action2_set_id));
    let mut action8 = vec![0x08, 0x07];
    action8.extend_from_slice(&grfid);
    action8.extend_from_slice(name.as_bytes());
    action8.push(0);
    action8.push(0);

    let sprite_body = build_real_sprite_v1_uncompressed_payload(
        width,
        height,
        -i16::try_from(width / 2).unwrap_or(0),
        -i16::try_from(height).unwrap_or(0),
        indices,
    );

    let mut data_section = Vec::new();
    for payload in [action0, action1.as_slice()] {
        let sz = u32::try_from(payload.len()).unwrap_or(0);
        data_section.extend_from_slice(&sz.to_le_bytes());
        data_section.push(0xFF);
        data_section.extend_from_slice(payload);
    }
    append_v2_real_sprite(&mut data_section, 0x01, &sprite_body);

    for payload in [action2.as_slice(), action3.as_slice(), action8.as_slice()] {
        let sz = u32::try_from(payload.len()).unwrap_or(0);
        data_section.extend_from_slice(&sz.to_le_bytes());
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

/// GRF v2: feature genérico con Action3 → Action2 básico → Action1.
#[must_use]
#[expect(clippy::too_many_arguments)]
pub fn build_grf_v2_feature_with_action2_chain(
    action0: &[u8],
    feature: u8,
    local_id: u8,
    action2_set_id: u8,
    action2_payload: &[u8],
    width: u16,
    height: u16,
    indices: &[u8],
    grfid: [u8; 4],
    name: &str,
) -> Vec<u8> {
    const SIG: [u8; 8] = [b'G', b'R', b'F', 0x82, 0x0D, 0x0A, 0x1A, 0x0A];
    let action1 = build_action1_feature_payload(feature, 1, 1);
    let action3 = build_action3_feature_payload(feature, local_id, u16::from(action2_set_id));
    let mut action8 = vec![0x08, 0x07];
    action8.extend_from_slice(&grfid);
    action8.extend_from_slice(name.as_bytes());
    action8.push(0);
    action8.push(0);

    let sprite_body = build_real_sprite_v1_uncompressed_payload(
        width,
        height,
        -i16::try_from(width / 2).unwrap_or(0),
        -i16::try_from(height).unwrap_or(0),
        indices,
    );

    let mut data_section = Vec::new();
    for payload in [action0, action1.as_slice()] {
        let sz = u32::try_from(payload.len()).unwrap_or(0);
        data_section.extend_from_slice(&sz.to_le_bytes());
        data_section.push(0xFF);
        data_section.extend_from_slice(payload);
    }
    append_v2_real_sprite(&mut data_section, 0x01, &sprite_body);

    for payload in [action2_payload, action3.as_slice(), action8.as_slice()] {
        let sz = u32::try_from(payload.len()).unwrap_or(0);
        data_section.extend_from_slice(&sz.to_le_bytes());
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

/// GRF v2 station: Action3 → Action2 → Action1.
#[must_use]
#[expect(clippy::too_many_arguments)]
pub fn build_grf_v2_station_with_action2_chain(
    action0: &[u8],
    local_id: u8,
    action2_set_id: u8,
    width: u16,
    height: u16,
    indices: &[u8],
    grfid: [u8; 4],
    name: &str,
) -> Vec<u8> {
    let a2 = build_action2_stations_payload(action2_set_id, 0);
    build_grf_v2_feature_with_action2_chain(
        action0,
        ACTION0_FEATURE_STATIONS,
        local_id,
        action2_set_id,
        &a2,
        width,
        height,
        indices,
        grfid,
        name,
    )
}

/// GRF v2 roadtype: Action3 → Action2 single-set → Action1.
#[must_use]
#[expect(clippy::too_many_arguments)]
pub fn build_grf_v2_roadtype_with_action2_chain(
    action0: &[u8],
    local_id: u8,
    action2_set_id: u8,
    width: u16,
    height: u16,
    indices: &[u8],
    grfid: [u8; 4],
    name: &str,
) -> Vec<u8> {
    let a2 = build_action2_single_set_payload(ACTION0_FEATURE_ROADTYPES, action2_set_id, 0);
    build_grf_v2_feature_with_action2_chain(
        action0,
        ACTION0_FEATURE_ROADTYPES,
        local_id,
        action2_set_id,
        &a2,
        width,
        height,
        indices,
        grfid,
        name,
    )
}

/// GRF v2: Action3 → variational default → Action2 básico → Action1.
#[must_use]
#[expect(clippy::too_many_arguments)]
pub fn build_grf_v2_train_with_variational_chain(
    action0: &[u8],
    local_id: u8,
    var_set_id: u8,
    basic_set_id: u8,
    width: u16,
    height: u16,
    indices: &[u8],
    grfid: [u8; 4],
    name: &str,
) -> Vec<u8> {
    const SIG: [u8; 8] = [b'G', b'R', b'F', 0x82, 0x0D, 0x0A, 0x1A, 0x0A];
    let action1 = build_action1_trains_payload(1, 1);
    let action2_basic = build_action2_trains_payload(basic_set_id, 0, 0);
    let action2_var = build_action2_trains_variational_default(var_set_id, u16::from(basic_set_id));
    let action3 = build_action3_trains_payload(local_id, u16::from(var_set_id));
    let mut action8 = vec![0x08, 0x07];
    action8.extend_from_slice(&grfid);
    action8.extend_from_slice(name.as_bytes());
    action8.push(0);
    action8.push(0);

    let sprite_body = build_real_sprite_v1_uncompressed_payload(
        width,
        height,
        -i16::try_from(width / 2).unwrap_or(0),
        -i16::try_from(height).unwrap_or(0),
        indices,
    );

    let mut data_section = Vec::new();
    for payload in [action0, action1.as_slice()] {
        let sz = u32::try_from(payload.len()).unwrap_or(0);
        data_section.extend_from_slice(&sz.to_le_bytes());
        data_section.push(0xFF);
        data_section.extend_from_slice(payload);
    }
    append_v2_real_sprite(&mut data_section, 0x01, &sprite_body);

    for payload in [
        action2_basic.as_slice(),
        action2_var.as_slice(),
        action3.as_slice(),
        action8.as_slice(),
    ] {
        let sz = u32::try_from(payload.len()).unwrap_or(0);
        data_section.extend_from_slice(&sz.to_le_bytes());
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

/// GRF v2 canónico: Action1 + ref `0xFD` → sprite section RGBA 32bpp.
#[must_use]
#[expect(clippy::too_many_arguments)]
pub fn build_grf_v2_train_with_fd_rgba_sprite(
    action0: &[u8],
    local_id: u8,
    sprite_id: u32,
    width: u16,
    height: u16,
    rgba: &[u8],
    grfid: [u8; 4],
    name: &str,
) -> Vec<u8> {
    const SIG: [u8; 8] = [b'G', b'R', b'F', 0x82, 0x0D, 0x0A, 0x1A, 0x0A];
    let action1 = build_action1_trains_payload(1, 1);
    let action3 = build_action3_trains_payload(local_id, 0);
    let mut action8 = vec![0x08, 0x07];
    action8.extend_from_slice(&grfid);
    action8.extend_from_slice(name.as_bytes());
    action8.push(0);
    action8.push(0);

    let mut data_section = Vec::new();
    for payload in [action0, action1.as_slice()] {
        let sz = u32::try_from(payload.len()).unwrap_or(0);
        data_section.extend_from_slice(&sz.to_le_bytes());
        data_section.push(0xFF);
        data_section.extend_from_slice(payload);
    }
    data_section.extend_from_slice(&4u32.to_le_bytes());
    data_section.push(0xFD);
    data_section.extend_from_slice(&sprite_id.to_le_bytes());

    for payload in [action3.as_slice(), action8.as_slice()] {
        let sz = u32::try_from(payload.len()).unwrap_or(0);
        data_section.extend_from_slice(&sz.to_le_bytes());
        data_section.push(0xFF);
        data_section.extend_from_slice(payload);
    }
    data_section.extend_from_slice(&0u32.to_le_bytes());

    let mut sprite_section = build_sprite_section_rgba_entry(
        sprite_id,
        0,
        width,
        height,
        -i16::try_from(width / 2).unwrap_or(0),
        -i16::try_from(height).unwrap_or(0),
        rgba,
    );
    sprite_section.extend_from_slice(&0u32.to_le_bytes());

    let sprite_offs = u32::try_from(1 + data_section.len()).unwrap_or(0);
    let mut out = Vec::new();
    out.extend_from_slice(&[0x00, 0x00]);
    out.extend_from_slice(&SIG);
    out.extend_from_slice(&sprite_offs.to_le_bytes());
    out.push(0x00);
    out.extend_from_slice(&data_section);
    out.extend_from_slice(&sprite_section);
    out
}

/// GRF v2 sintético: Action0 roadtype + Action1 + sprite + Action3 + Action8.
#[must_use]
pub fn build_grf_v2_roadtype_with_preview_sprite(
    action0: &[u8],
    local_id: u8,
    width: u16,
    height: u16,
    indices: &[u8],
    grfid: [u8; 4],
    name: &str,
) -> Vec<u8> {
    build_grf_v2_with_preview_sprite(
        action0,
        ACTION0_FEATURE_ROADTYPES,
        local_id,
        width,
        height,
        indices,
        grfid,
        name,
    )
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

/// GRF v2 sintético: Action0 industry tile + Action1 + sprite + Action3 + Action8.
#[must_use]
pub fn build_grf_v2_industry_tile_with_preview_sprite(
    action0: &[u8],
    local_id: u8,
    width: u16,
    height: u16,
    indices: &[u8],
    grfid: [u8; 4],
    name: &str,
) -> Vec<u8> {
    build_grf_v2_with_preview_sprite(
        action0,
        ACTION0_FEATURE_INDUSTRYTILES,
        local_id,
        width,
        height,
        indices,
        grfid,
        name,
    )
}

/// GRF v2 sintético: Action0 station + Action1 + sprite + Action3 + Action8.
#[must_use]
pub fn build_grf_v2_station_with_preview_sprite(
    action0: &[u8],
    local_id: u8,
    width: u16,
    height: u16,
    indices: &[u8],
    grfid: [u8; 4],
    name: &str,
) -> Vec<u8> {
    build_grf_v2_with_preview_sprite(
        action0,
        ACTION0_FEATURE_STATIONS,
        local_id,
        width,
        height,
        indices,
        grfid,
        name,
    )
}
