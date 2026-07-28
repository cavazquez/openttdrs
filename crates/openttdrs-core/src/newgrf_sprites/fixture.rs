//! Builders sintéticos de GRF/sprite para tests y fixtures.
//!
//! Contiene todas las funciones `build_*` para construir sprites, actions y GRFs sintéticos
//! usados en tests. Las funciones de decodificación en runtime permanecen en sus módulos.

use crate::newgrf_actions::{
    ACTION0_FEATURE_HOUSES, ACTION0_FEATURE_INDUSTRYTILES, ACTION0_FEATURE_RAILTYPES,
    ACTION0_FEATURE_ROADTYPES, ACTION0_FEATURE_STATIONS, ACTION0_FEATURE_TRAINS,
};

use super::pixel_codec::{encode_chunked_8bpp_full_rows, encode_chunked_pixels_full_rows};

/// Cabecera dimensiones + índices (sin byte `type`; para payload v2 canónico).
#[must_use]
pub fn build_real_sprite_v1_dims(
    width: u16,
    height: u16,
    x_offs: i16,
    y_offs: i16,
    indices: &[u8],
) -> Vec<u8> {
    let mut body = Vec::with_capacity(7 + indices.len());
    body.push(u8::try_from(height).unwrap_or(1));
    body.extend_from_slice(&width.to_le_bytes());
    body.extend_from_slice(&x_offs.to_le_bytes());
    body.extend_from_slice(&y_offs.to_le_bytes());
    body.extend_from_slice(indices);
    body
}

/// Comprime índices solo con literales LZ77 (sin back-refs).
#[must_use]
pub fn compress_grf_lz77_literals(indices: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < indices.len() {
        let chunk = (indices.len() - i).min(0x80);
        let code = if chunk == 0x80 {
            0u8
        } else {
            u8::try_from(chunk).unwrap_or(0)
        };
        out.push(code);
        out.extend_from_slice(&indices[i..i + chunk]);
        i += chunk;
    }
    out
}

/// Construye un sprite real v1 sin comprimir (`type` + dims + índices).
#[must_use]
pub fn build_real_sprite_v1_uncompressed(
    width: u16,
    height: u16,
    x_offs: i16,
    y_offs: i16,
    indices: &[u8],
) -> Vec<u8> {
    let mut body = Vec::with_capacity(8 + indices.len());
    body.push(0x01); // type: image, uncompressed
    body.extend(build_real_sprite_v1_dims(
        width, height, x_offs, y_offs, indices,
    ));
    body
}

/// Sprite real v1 comprimido (`0x03` = transparente + LZ77).
#[must_use]
pub fn build_real_sprite_v1_compressed(
    width: u16,
    height: u16,
    x_offs: i16,
    y_offs: i16,
    indices: &[u8],
) -> Vec<u8> {
    let mut body = Vec::with_capacity(8 + indices.len() + 8);
    body.push(0x03); // transparent + compressed
    body.push(u8::try_from(height).unwrap_or(1));
    body.extend_from_slice(&width.to_le_bytes());
    body.extend_from_slice(&x_offs.to_le_bytes());
    body.extend_from_slice(&y_offs.to_le_bytes());
    body.extend(compress_grf_lz77_literals(indices));
    body
}

/// Sprite real v1 chunked (`0x09` = transparente + tile).
#[must_use]
pub fn build_real_sprite_v1_chunked(
    width: u16,
    height: u16,
    x_offs: i16,
    y_offs: i16,
    indices: &[u8],
) -> Option<Vec<u8>> {
    let chunked = encode_chunked_8bpp_full_rows(width, height, indices)?;
    let mut body = Vec::with_capacity(8 + chunked.len());
    body.push(0x09);
    body.push(u8::try_from(height).unwrap_or(1));
    body.extend_from_slice(&width.to_le_bytes());
    body.extend_from_slice(&x_offs.to_le_bytes());
    body.extend_from_slice(&y_offs.to_le_bytes());
    body.extend(chunked);
    Some(body)
}

/// Payload v2 canónico chunked (sin type; `info` = `0x09`).
#[must_use]
pub fn build_real_sprite_v1_chunked_payload(
    width: u16,
    height: u16,
    x_offs: i16,
    y_offs: i16,
    indices: &[u8],
) -> Option<Vec<u8>> {
    let chunked = encode_chunked_8bpp_full_rows(width, height, indices)?;
    let mut body = build_real_sprite_v1_dims(width, height, x_offs, y_offs, &[]);
    body.extend(chunked);
    Some(body)
}

/// Payload v2 canónico comprimido (sin type; el `info` del contenedor es `0x03`).
#[must_use]
pub fn build_real_sprite_v1_compressed_payload(
    width: u16,
    height: u16,
    x_offs: i16,
    y_offs: i16,
    indices: &[u8],
) -> Vec<u8> {
    let mut body = build_real_sprite_v1_dims(width, height, x_offs, y_offs, &[]);
    body.extend(compress_grf_lz77_literals(indices));
    body
}

/// Payload v2 canónico sin comprimir (sin type; `info` = `0x01`).
#[must_use]
pub fn build_real_sprite_v1_uncompressed_payload(
    width: u16,
    height: u16,
    x_offs: i16,
    y_offs: i16,
    indices: &[u8],
) -> Vec<u8> {
    build_real_sprite_v1_dims(width, height, x_offs, y_offs, indices)
}

/// Entrada sprite section v2: palette 8bpp + zoom + LZ77.
#[must_use]
pub fn build_sprite_section_palette_entry(
    sprite_id: u32,
    zoom: u8,
    width: u16,
    height: u16,
    x_offs: i16,
    y_offs: i16,
    indices: &[u8],
) -> Vec<u8> {
    let mut img = Vec::new();
    img.push(zoom);
    img.extend_from_slice(&height.to_le_bytes());
    img.extend_from_slice(&width.to_le_bytes());
    img.extend_from_slice(&x_offs.to_le_bytes());
    img.extend_from_slice(&y_offs.to_le_bytes());
    img.extend(compress_grf_lz77_literals(indices));
    let mut entry = Vec::with_capacity(8 + 1 + img.len());
    entry.extend_from_slice(&sprite_id.to_le_bytes());
    let size = u32::try_from(1 + img.len()).unwrap_or(0);
    entry.extend_from_slice(&size.to_le_bytes());
    entry.push(0x04); // palette only
    entry.extend(img);
    entry
}

/// Entrada sprite section v2: RGBA 32bpp (`info=0x03`) + zoom + LZ77.
#[must_use]
pub fn build_sprite_section_rgba_entry(
    sprite_id: u32,
    zoom: u8,
    width: u16,
    height: u16,
    x_offs: i16,
    y_offs: i16,
    rgba: &[u8],
) -> Vec<u8> {
    let mut img = Vec::new();
    img.push(zoom);
    img.extend_from_slice(&height.to_le_bytes());
    img.extend_from_slice(&width.to_le_bytes());
    img.extend_from_slice(&x_offs.to_le_bytes());
    img.extend_from_slice(&y_offs.to_le_bytes());
    img.extend(compress_grf_lz77_literals(rgba));
    let mut entry = Vec::with_capacity(8 + 1 + img.len());
    entry.extend_from_slice(&sprite_id.to_le_bytes());
    let size = u32::try_from(1 + img.len()).unwrap_or(0);
    entry.extend_from_slice(&size.to_le_bytes());
    entry.push(0x03); // RGB + alpha
    entry.extend(img);
    entry
}

/// Entrada sprite section v2: RGBA + máscara (`info=0x07`) + LZ77.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn build_sprite_section_rgba_mask_entry(
    sprite_id: u32,
    zoom: u8,
    width: u16,
    height: u16,
    x_offs: i16,
    y_offs: i16,
    rgba: &[u8],
    mask: &[u8],
) -> Vec<u8> {
    let n = usize::from(width) * usize::from(height);
    let mut pixels = Vec::with_capacity(n * 5);
    for i in 0..n {
        let o = i * 4;
        pixels.extend_from_slice(&rgba[o..o + 4]);
        pixels.push(mask.get(i).copied().unwrap_or(0));
    }
    let mut img = Vec::new();
    img.push(zoom);
    img.extend_from_slice(&height.to_le_bytes());
    img.extend_from_slice(&width.to_le_bytes());
    img.extend_from_slice(&x_offs.to_le_bytes());
    img.extend_from_slice(&y_offs.to_le_bytes());
    img.extend(compress_grf_lz77_literals(&pixels));
    let mut entry = Vec::with_capacity(8 + 1 + img.len());
    entry.extend_from_slice(&sprite_id.to_le_bytes());
    let size = u32::try_from(1 + img.len()).unwrap_or(0);
    entry.extend_from_slice(&size.to_le_bytes());
    entry.push(0x07); // RGB + alpha + palette mask
    entry.extend(img);
    entry
}

/// Entrada sprite section v2: RGBA chunked (`info=0x0B`).
#[must_use]
pub fn build_sprite_section_rgba_chunked_entry(
    sprite_id: u32,
    zoom: u8,
    width: u16,
    height: u16,
    x_offs: i16,
    y_offs: i16,
    rgba: &[u8],
) -> Option<Vec<u8>> {
    let chunked = encode_chunked_pixels_full_rows(width, height, 4, rgba)?;
    let mut img = Vec::new();
    img.push(zoom);
    img.extend_from_slice(&height.to_le_bytes());
    img.extend_from_slice(&width.to_le_bytes());
    img.extend_from_slice(&x_offs.to_le_bytes());
    img.extend_from_slice(&y_offs.to_le_bytes());
    let decomp = u32::try_from(chunked.len()).ok()?;
    img.extend_from_slice(&decomp.to_le_bytes());
    img.extend(compress_grf_lz77_literals(&chunked));
    let mut entry = Vec::with_capacity(8 + 1 + img.len());
    entry.extend_from_slice(&sprite_id.to_le_bytes());
    let size = u32::try_from(1 + img.len()).unwrap_or(0);
    entry.extend_from_slice(&size.to_le_bytes());
    entry.push(0x0B); // RGB + alpha + chunked
    entry.extend(img);
    Some(entry)
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

/// Action3 con un grupo específico (`cargo id` / `RailSpriteType`) y fallback.
#[must_use]
pub fn build_action3_feature_specific_payload(
    feature: u8,
    local_id: u8,
    selector: u8,
    set_id: u16,
    default_set: u16,
) -> Vec<u8> {
    let mut p = vec![0x03, feature, 0x01, local_id, 0x01, selector];
    p.extend_from_slice(&set_id.to_le_bytes());
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

/// GRF `RailType` sintético con 8 orientaciones rojas + 8 verdes.
///
/// Action3 asigna `RailSpriteType::Signals` y Action2 selecciona el set por
/// `param2 & 0xFF` (`SignalState`), igual que `GetCustomSignalSprite`.
#[must_use]
#[expect(clippy::too_many_arguments)]
pub fn build_grf_v2_railtype_signal_sprites(
    action0: &[u8],
    local_id: u8,
    width: u16,
    height: u16,
    red_indices: &[u8],
    green_indices: &[u8],
    grfid: [u8; 4],
    name: &str,
) -> Vec<u8> {
    const SIG: [u8; 8] = [b'G', b'R', b'F', 0x82, 0x0D, 0x0A, 0x1A, 0x0A];
    const ACTION2_SET: u8 = 0x20;
    let action1 = build_action1_feature_payload(ACTION0_FEATURE_RAILTYPES, 2, 8);
    let action2 = build_action2_variational_payload(
        ACTION0_FEATURE_RAILTYPES,
        ACTION2_SET,
        0x18,
        0,
        0xFF,
        &[(0, 0, 0), (1, 1, 1)],
        0,
    );
    let action3 = build_action3_feature_specific_payload(
        ACTION0_FEATURE_RAILTYPES,
        local_id,
        crate::rail_type::RAIL_SPRITE_TYPE_SIGNALS,
        u16::from(ACTION2_SET),
        0,
    );
    let mut action8 = vec![0x08, 0x07];
    action8.extend_from_slice(&grfid);
    action8.extend_from_slice(name.as_bytes());
    action8.push(0);
    action8.push(0);

    let mut data_section = Vec::new();
    for payload in [action0, action1.as_slice()] {
        let size = u32::try_from(payload.len()).unwrap_or(0);
        data_section.extend_from_slice(&size.to_le_bytes());
        data_section.push(0xFF);
        data_section.extend_from_slice(payload);
    }
    for indices in [red_indices, green_indices] {
        for image in 0..8i16 {
            let body = build_real_sprite_v1_uncompressed_payload(
                width,
                height,
                -i16::try_from(width / 2).unwrap_or(0) + image,
                -i16::try_from(height).unwrap_or(0),
                indices,
            );
            append_v2_real_sprite(&mut data_section, 0x01, &body);
        }
    }
    for payload in [action2.as_slice(), action3.as_slice(), action8.as_slice()] {
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

/// GRF v2 sintético: Action5 + un sprite + Action8.
#[must_use]
pub fn build_grf_v2_action5_with_sprite(
    type_id: u8,
    offset: u16,
    width: u16,
    height: u16,
    indices: &[u8],
    grfid: [u8; 4],
    name: &str,
) -> Vec<u8> {
    const SIG: [u8; 8] = [b'G', b'R', b'F', 0x82, 0x0D, 0x0A, 0x1A, 0x0A];
    let mut action5 = vec![0x05, type_id, 0x01];
    action5.extend_from_slice(&offset.to_le_bytes());
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
    let sz = u32::try_from(action5.len()).unwrap_or(0);
    data_section.extend_from_slice(&sz.to_le_bytes());
    data_section.push(0xFF);
    data_section.extend_from_slice(&action5);

    append_v2_real_sprite(&mut data_section, 0x01, &sprite_body);

    let sz = u32::try_from(action8.len()).unwrap_or(0);
    data_section.extend_from_slice(&sz.to_le_bytes());
    data_section.push(0xFF);
    data_section.extend_from_slice(&action8);
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

/// GRF v2 sintético: Action0 house + Action1 + sprite + Action3 + Action8.
#[must_use]
pub fn build_grf_v2_house_with_preview_sprite(
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
        ACTION0_FEATURE_HOUSES,
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
