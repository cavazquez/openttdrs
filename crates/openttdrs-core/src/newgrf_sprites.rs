//! Decode mínimo de sprites reales `NewGRF` + Action1/2/3 (trains / roadtypes, preview).
//!
//! MVP: contenedor **v1** (o entradas reales inline), 8bpp plano, **LZ77**
//! (bit `0x02`) y **chunked** tile (bit `0x08`). Action3→Action2→Action1
//! estático para **trains** (default moving; sin variational/callbacks).
//! Road/station siguen Action3→Action1 directo. Action5 shore runtime parcial;
//! 32bpp / trozos anchos (`width>256`) OOS.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::newgrf_actions::{
    ACTION0_FEATURE_ROADTYPES, ACTION0_FEATURE_STATIONS, ACTION0_FEATURE_TRAINS,
};
use crate::newgrf_config::{GrfContainerVersion, GrfScanError, parse_grf_container};
use crate::newgrf_palette_data::DOS_PALETTE_RGB;

/// Sprite RGBA decodificado (índice 0 → alpha 0).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecodedSprite {
    pub width: u16,
    pub height: u16,
    pub x_offs: i16,
    pub y_offs: i16,
    /// `width * height * 4` bytes RGBA.
    pub rgba: Vec<u8>,
}

/// Asignación Action3: id local → set Action2 (o índice Action1 si no hay Action2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrainSpriteAssign {
    pub local_id: u8,
    pub set_id: u16,
}

/// Resultado de parsear Action1/2/3 de un feature (trains / roadtypes).
#[derive(Debug, Clone, Default)]
pub struct TrainSpriteGraphics {
    /// `sets[set_id][view]` — sets Action1 en orden de aparición.
    pub sets: Vec<Vec<DecodedSprite>>,
    pub assigns: Vec<TrainSpriteAssign>,
    /// Action2 set-id → índice del primer set Action1 “moving” (solo trains).
    pub action2_to_action1: HashMap<u8, u16>,
}

impl TrainSpriteGraphics {
    /// Preview (primera vista) para un id local.
    #[must_use]
    pub fn preview_for_local_id(&self, local_id: u8) -> Option<&DecodedSprite> {
        self.views_for_local_id(local_id)?.first()
    }

    /// Resuelve Action3 → Action2 (si hay) → Action1.
    #[must_use]
    pub fn resolve_action1_set(&self, action3_set_id: u16) -> u16 {
        let a2 = u8::try_from(action3_set_id).unwrap_or(u8::MAX);
        self.action2_to_action1
            .get(&a2)
            .copied()
            .unwrap_or(action3_set_id)
    }

    /// Todas las vistas del set asignado al id local.
    #[must_use]
    pub fn views_for_local_id(&self, local_id: u8) -> Option<&[DecodedSprite]> {
        let set_id = self
            .assigns
            .iter()
            .find(|a| a.local_id == local_id)
            .map(|a| a.set_id)
            .or_else(|| (!self.sets.is_empty()).then_some(0))?;
        let action1_idx = self.resolve_action1_set(set_id);
        self.sets
            .get(usize::from(action1_idx))
            .map(Vec::as_slice)
            .filter(|s| !s.is_empty())
    }
}

/// Convierte índices 8bpp → RGBA con paleta DOS.
#[must_use]
pub fn indices_to_rgba(indices: &[u8], width: u16, height: u16) -> Option<Vec<u8>> {
    let expected = usize::from(width).checked_mul(usize::from(height))?;
    if indices.len() < expected {
        return None;
    }
    let mut rgba = Vec::with_capacity(expected * 4);
    for &idx in &indices[..expected] {
        if idx == 0 {
            rgba.extend_from_slice(&[0, 0, 0, 0]);
        } else {
            let [r, g, b] = DOS_PALETTE_RGB[usize::from(idx)];
            rgba.extend_from_slice(&[r, g, b, 255]);
        }
    }
    Some(rgba)
}

/// Descomprime stream LZ77 de sprites `NewGRF` (variante TTDP / `DecodeSingleSprite`).
///
/// `code >= 0`: literales (`0` → 0x80 bytes). `code < 0`: copia desde `offset` atrás.
#[must_use]
pub fn decompress_grf_lz77(src: &[u8], out_len: usize) -> Option<Vec<u8>> {
    if out_len == 0 || out_len > 512 * 512 {
        return None;
    }
    let mut dest = Vec::with_capacity(out_len);
    let mut i = 0usize;
    while dest.len() < out_len {
        if i >= src.len() {
            return None;
        }
        let code = src[i].cast_signed();
        i += 1;
        if code >= 0 {
            let size = if code == 0 {
                0x80usize
            } else {
                usize::try_from(code).ok()?
            };
            if dest.len().checked_add(size)? > out_len || i.checked_add(size)? > src.len() {
                return None;
            }
            dest.extend_from_slice(&src[i..i + size]);
            i += size;
        } else {
            if i >= src.len() {
                return None;
            }
            let data_offset = (usize::from(code.cast_unsigned() & 7) << 8) | usize::from(src[i]);
            i += 1;
            let size = usize::try_from(i32::from(-(code >> 3))).ok()?;
            if size == 0
                || data_offset == 0
                || data_offset > dest.len()
                || dest.len().checked_add(size)? > out_len
            {
                return None;
            }
            for _ in 0..size {
                let b = dest[dest.len() - data_offset];
                dest.push(b);
            }
        }
    }
    Some(dest)
}

/// LZ77 con tamaño de salida: plano = `pixel_len`; chunked prueba candidatos.
fn decompress_sprite_lz77(
    data: &[u8],
    pixel_len: usize,
    chunked: bool,
    width: u16,
    height: u16,
) -> Option<Vec<u8>> {
    if !chunked {
        return decompress_grf_lz77(data, pixel_len);
    }
    // V1 usa `w*h`; buffers chunked reales/sintéticos pueden ser mayores.
    let full_rows = usize::from(height).checked_mul(4 + usize::from(width))?;
    let candidates = [
        pixel_len,
        full_rows,
        data.len().saturating_mul(4).max(full_rows),
    ];
    for &n in &candidates {
        if let Some(buf) = decompress_grf_lz77(data, n)
            && decode_chunked_8bpp(&buf, width, height).is_some()
        {
            return Some(buf);
        }
    }
    None
}

/// Decodifica buffer “tile” chunked 8bpp (offsets u16 + trozos `cinfo`/`cofs`).
///
/// Formato `OpenTTD`/`grf.txt` para `width ≤ 256`: por fila, offset LE desde el
/// inicio del buffer; trozos `(length|last, skip, pixels…)`.
#[must_use]
pub fn decode_chunked_8bpp(buf: &[u8], width: u16, height: u16) -> Option<Vec<u8>> {
    let w = usize::from(width);
    let h = usize::from(height);
    if w == 0 || h == 0 || w > 256 {
        return None;
    }
    let table_bytes = h.checked_mul(2)?;
    if buf.len() < table_bytes {
        return None;
    }
    let mut out = vec![0u8; w.checked_mul(h)?];
    for y in 0..h {
        let offset = usize::from(u16::from_le_bytes([buf[y * 2], buf[y * 2 + 1]]));
        if offset >= buf.len() {
            return None;
        }
        let mut dest = offset;
        loop {
            if dest + 2 > buf.len() {
                return None;
            }
            let cinfo = buf[dest];
            let last = cinfo & 0x80 != 0;
            let length = usize::from(cinfo & 0x7F);
            let skip = usize::from(buf[dest + 1]);
            dest += 2;
            if skip.checked_add(length)? > w || dest.checked_add(length)? > buf.len() {
                return None;
            }
            let row_at = y.checked_mul(w)?.checked_add(skip)?;
            out[row_at..row_at + length].copy_from_slice(&buf[dest..dest + length]);
            dest += length;
            if last {
                break;
            }
        }
    }
    Some(out)
}

/// Codifica índices planos → buffer chunked (1 trozo de fila completa, `width ≤ 127`).
#[must_use]
pub fn encode_chunked_8bpp_full_rows(width: u16, height: u16, indices: &[u8]) -> Option<Vec<u8>> {
    let w = usize::from(width);
    let h = usize::from(height);
    if w == 0 || h == 0 || w > 127 || indices.len() < w.checked_mul(h)? {
        return None;
    }
    let table_bytes = h * 2;
    let mut body = Vec::with_capacity(h * (2 + w));
    let mut offsets = Vec::with_capacity(h);
    for y in 0..h {
        offsets.push(u16::try_from(table_bytes + body.len()).ok()?);
        let cinfo = 0x80u8 | u8::try_from(w).ok()?;
        body.push(cinfo);
        body.push(0); // skip
        let row = y * w;
        body.extend_from_slice(&indices[row..row + w]);
    }
    let mut out = Vec::with_capacity(table_bytes + body.len());
    for off in offsets {
        out.extend_from_slice(&off.to_le_bytes());
    }
    out.extend(body);
    Some(out)
}

/// Decodifica sprite real v1: `sprite_type` + `height…` + datos (plano, LZ77 y/o chunked).
///
/// - Sin bit `0x02`: datos planos (o buffer chunked crudo).
/// - Con bit `0x02`: stream LZ77 → buffer intermedio (`width*height` bytes).
/// - Con bit `0x08`: el buffer intermedio es tile chunked → índices con huecos 0.
#[must_use]
pub fn decode_real_sprite_v1(sprite_type: u8, dim_and_data: &[u8]) -> Option<DecodedSprite> {
    if dim_and_data.len() < 7 {
        return None;
    }
    let height = u16::from(dim_and_data[0]);
    let width = u16::from_le_bytes([dim_and_data[1], dim_and_data[2]]);
    let x_offs = i16::from_le_bytes([dim_and_data[3], dim_and_data[4]]);
    let y_offs = i16::from_le_bytes([dim_and_data[5], dim_and_data[6]]);
    if width == 0 || height == 0 || width > 512 || height > 512 {
        return None;
    }
    let pixel_len = usize::from(width).checked_mul(usize::from(height))?;
    let data = &dim_and_data[7..];
    let chunked = sprite_type & 0x08 != 0;
    let intermediate = if sprite_type & 0x02 != 0 {
        decompress_sprite_lz77(data, pixel_len, chunked, width, height)?
    } else if chunked {
        data.to_vec()
    } else {
        if data.len() < pixel_len {
            return None;
        }
        data[..pixel_len].to_vec()
    };
    let indices = if chunked {
        decode_chunked_8bpp(&intermediate, width, height)?
    } else {
        if intermediate.len() < pixel_len {
            return None;
        }
        intermediate[..pixel_len].to_vec()
    };
    let rgba = indices_to_rgba(&indices, width, height)?;
    Some(DecodedSprite {
        width,
        height,
        x_offs,
        y_offs,
        rgba,
    })
}

/// Compat: primer byte = `sprite_type`, resto = dimensiones + datos.
#[must_use]
pub fn decode_real_sprite_v1_uncompressed(type_and_rest: &[u8]) -> Option<DecodedSprite> {
    if type_and_rest.is_empty() {
        return None;
    }
    decode_real_sprite_v1(type_and_rest[0], &type_and_rest[1..])
}

/// Decodifica entrada real del data section (`info` del contenedor + payload).
fn decode_real_sprite_entry(
    container: GrfContainerVersion,
    info: u8,
    payload: &[u8],
) -> Option<DecodedSprite> {
    match container {
        GrfContainerVersion::V1 => {
            // V1: el slice incluye el byte type (= info).
            decode_real_sprite_v1_uncompressed(payload)
        }
        GrfContainerVersion::V2 => {
            // Canónico: `info` = type, payload empieza en height.
            decode_real_sprite_v1(info, payload).or_else(|| {
                // Compat builders antiguos: type duplicado al inicio del payload.
                if payload.first() == Some(&info) {
                    decode_real_sprite_v1_uncompressed(payload)
                } else {
                    None
                }
            })
        }
    }
}

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

/// Action2 vehículo básico: `02 <feat> <set-id> <n-load> <n-loading> <words…>`.
///
/// Devuelve `(action2_set_id, primer Action1 set moving)`. Variational (`≥0x80`) → None.
fn parse_action2_vehicle_basic(payload: &[u8], feature: u8) -> Option<(u8, u16)> {
    if payload.len() < 5 || payload[0] != 0x02 {
        return None;
    }
    if payload[1] != feature {
        return None;
    }
    let set_id = payload[2];
    let num_load = payload[3];
    let num_loading = payload[4];
    // Variational / random Action2 (0x81 / 0x82 / …) → OOS.
    if num_load >= 0x80 || num_load == 0 || num_loading == 0 {
        return None;
    }
    let n_words = usize::from(num_load) + usize::from(num_loading);
    let words_start = 5usize;
    let words_end = words_start.checked_add(n_words.checked_mul(2)?)?;
    if payload.len() < words_end {
        return None;
    }
    let a1 = u16::from_le_bytes([payload[words_start], payload[words_start + 1]]);
    Some((set_id, a1))
}

fn parse_action3_feature(payload: &[u8], feature: u8) -> Option<Vec<TrainSpriteAssign>> {
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

/// Recorre el GRF y extrae sets Action1 + Action2 (trains) + asignaciones Action3.
///
/// # Errors
///
/// Contenedor inválido.
pub fn collect_feature_sprite_graphics(
    data: &[u8],
    feature: u8,
) -> Result<TrainSpriteGraphics, GrfScanError> {
    let (container, section) = parse_grf_container(data)?;
    let mut out = TrainSpriteGraphics::default();
    let mut current_set: Vec<DecodedSprite> = Vec::new();
    let mut views_left_in_set = 0u8;
    let mut sets_left = 0u8;
    let mut views_per_set = 0u8;

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
            let payload = &section[payload_start..end];
            if let Some((ns, ne)) = parse_action1_feature(payload, feature) {
                if !current_set.is_empty() {
                    out.sets.push(std::mem::take(&mut current_set));
                }
                sets_left = ns;
                views_per_set = ne;
                views_left_in_set = ne;
            } else if feature == ACTION0_FEATURE_TRAINS
                && let Some((a2_id, a1_idx)) = parse_action2_vehicle_basic(payload, feature)
            {
                out.action2_to_action1.insert(a2_id, a1_idx);
            } else if let Some(assigns) = parse_action3_feature(payload, feature) {
                out.assigns.extend(assigns);
            }
            i = end;
            continue;
        }

        let Some(payload) = real_sprite_payload(section, i, size, header, container) else {
            break;
        };

        if (sets_left > 0 || views_left_in_set > 0)
            && let Some(decoded) = decode_real_sprite_entry(container, info, payload)
        {
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

        i = match container {
            GrfContainerVersion::V1 => i + 2 + size,
            GrfContainerVersion::V2 => payload_start + size,
        };
    }
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

/// Append sprite real v2: `DWORD size` + `info` + payload (sin type duplicado).
fn append_v2_real_sprite(data_section: &mut Vec<u8>, info: u8, payload: &[u8]) {
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

/// Tipo Action5: shore / coastline (`ACT5_SHORELINE`).
pub const ACTION5_TYPE_SHORE: u8 = 0x0D;
/// Tipo Action5: catenaria (`ACT5_ELRAIL`).
pub const ACTION5_TYPE_CATENARY: u8 = 0x05;
/// Slots `SPR_SHORE_BASE + 0..17`.
pub const SHORE_ACTION5_SLOT_COUNT: usize = 18;
/// Orden del bloque de 10 («missing shore sprites», `newgrf_act5.cpp`).
pub const SHORE_MISSING_BLOCK_SLOTS: [usize; 10] = [0, 5, 7, 10, 11, 13, 14, 15, 16, 17];
/// Slots Action5 catenary `OpenGFX`: wires 0..23 + entrances 24..27 + pylons 28..35.
pub const CATENARY_ACTION5_SLOT_COUNT: usize = 36;
/// Base `OpenTTD` de wires (`SPR_WIRE_*` / `rail_1039`).
pub const CATENARY_WIRE_SPRITE_BASE: u32 = 1039;
/// IDs virtuales de entrada de túnel en el cliente (`WSO_ENTRANCE_*`).
pub const CATENARY_ENTRANCE_SPRITE_BASE: u32 = 910_063;
/// IDs virtuales de postes PPP en el cliente (`PSO_*`).
pub const CATENARY_PYLON_SPRITE_BASE: u32 = 910_067;

/// Bloque Action5 parseado (tipo + offset + sprites siguientes).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Action5Block {
    pub type_id: u8,
    pub num_sprites: u8,
    pub offset: u16,
    /// Primer sprite real decodificado (8bpp sin comprimir), si se pudo.
    pub first_preview: Option<DecodedSprite>,
    /// Todos los sprites del bloque que se pudieron decodificar (orden de archivo).
    pub sprites: Vec<DecodedSprite>,
}

/// Nombre corto de tipos Action5 conocidos (resto = `other`).
#[must_use]
pub fn action5_type_name(type_id: u8) -> &'static str {
    match type_id {
        0x04 | 0x06 => "foundations",
        0x05 => "catenary",
        0x07 => "gui",
        0x08 => "airport-preview",
        0x09 => "roadstops",
        0x0A => "oneway-road",
        0x0B => "bridge",
        0x0C => "grass",
        0x0D => "shore",
        _ => "other",
    }
}

fn parse_action5_header(payload: &[u8]) -> Option<(u8, u8, u16)> {
    // 05 <type> <num-sprites> <offset:u16 LE>
    if payload.len() < 5 || payload[0] != 0x05 {
        return None;
    }
    let type_id = payload[1];
    let num_sprites = payload[2];
    if num_sprites == 0 {
        return None;
    }
    let offset = u16::from_le_bytes([payload[3], payload[4]]);
    Some((type_id, num_sprites, offset))
}

fn read_grf_entry_header(
    section: &[u8],
    i: usize,
    container: GrfContainerVersion,
) -> Option<(usize, usize)> {
    match container {
        GrfContainerVersion::V2 => {
            if i + 5 > section.len() {
                return None;
            }
            let size =
                u32::from_le_bytes([section[i], section[i + 1], section[i + 2], section[i + 3]])
                    as usize;
            (size > 0).then_some((size, 5))
        }
        GrfContainerVersion::V1 => {
            if i + 3 > section.len() {
                return None;
            }
            let size = u16::from_le_bytes([section[i], section[i + 1]]) as usize;
            (size > 0).then_some((size, 3))
        }
    }
}

fn real_sprite_payload(
    section: &[u8],
    i: usize,
    size: usize,
    header: usize,
    container: GrfContainerVersion,
) -> Option<&[u8]> {
    let payload_start = i + header;
    match container {
        GrfContainerVersion::V1 => {
            let start = i + 2;
            let end = start + size;
            (end <= section.len()).then(|| &section[start..end])
        }
        GrfContainerVersion::V2 => {
            let end = payload_start + size;
            (end <= section.len()).then(|| &section[payload_start..end])
        }
    }
}

fn finish_action5_block(
    type_id: u8,
    num_sprites: u8,
    offset: u16,
    sprites: Vec<DecodedSprite>,
) -> Action5Block {
    Action5Block {
        type_id,
        num_sprites,
        offset,
        first_preview: sprites.first().cloned(),
        sprites,
    }
}

/// Recorre el GRF y extrae bloques Action5 + sprites decodificados de cada bloque.
///
/// # Errors
///
/// Contenedor inválido.
pub fn collect_action5_blocks(data: &[u8]) -> Result<Vec<Action5Block>, GrfScanError> {
    let (container, section) = parse_grf_container(data)?;
    let mut out = Vec::new();
    let mut sprites_left = 0u8;
    let mut cur_type = 0u8;
    let mut cur_num = 0u8;
    let mut cur_offset = 0u16;
    let mut sprites: Vec<DecodedSprite> = Vec::new();
    let mut in_block = false;

    let mut i = 0usize;
    while i < section.len() {
        let Some((size, header)) = read_grf_entry_header(section, i, container) else {
            break;
        };
        let info = section[i + header - 1];
        let payload_start = i + header;
        if info == 0xFF {
            let end = payload_start + size;
            if end > section.len() {
                break;
            }
            let payload = &section[payload_start..end];
            if let Some((type_id, num, offset)) = parse_action5_header(payload) {
                if in_block {
                    out.push(finish_action5_block(
                        cur_type,
                        cur_num,
                        cur_offset,
                        std::mem::take(&mut sprites),
                    ));
                }
                cur_type = type_id;
                cur_num = num;
                cur_offset = offset;
                sprites_left = num;
                sprites.clear();
                in_block = true;
            }
            i = end;
            continue;
        }

        let Some(payload) = real_sprite_payload(section, i, size, header, container) else {
            break;
        };

        if in_block && sprites_left > 0 {
            if let Some(spr) = decode_real_sprite_entry(container, info, payload) {
                sprites.push(spr);
            }
            sprites_left = sprites_left.saturating_sub(1);
            if sprites_left == 0 {
                out.push(finish_action5_block(
                    cur_type,
                    cur_num,
                    cur_offset,
                    std::mem::take(&mut sprites),
                ));
                in_block = false;
            }
        }

        i = match container {
            GrfContainerVersion::V1 => i + 2 + size,
            GrfContainerVersion::V2 => payload_start + size,
        };
    }
    if in_block {
        out.push(finish_action5_block(cur_type, cur_num, cur_offset, sprites));
    }
    Ok(out)
}

/// Fusiona un bloque Action5 shore (`0x0D`) en la tabla de 18 slots.
///
/// - 10 sprites → tabla «missing» de `OpenTTD`.
/// - 16 sprites → slots `0..15`.
/// - resto → escribe desde `offset` si `offset < 18`; si no, desde el slot 0.
pub fn merge_shore_action5_block(slots: &mut [Option<DecodedSprite>], block: &Action5Block) {
    if block.type_id != ACTION5_TYPE_SHORE || slots.len() < SHORE_ACTION5_SLOT_COUNT {
        return;
    }
    let sprites = &block.sprites;
    if sprites.is_empty() {
        return;
    }
    if block.num_sprites == 10 && sprites.len() >= 10 {
        for (i, &slot) in SHORE_MISSING_BLOCK_SLOTS.iter().enumerate() {
            slots[slot] = Some(sprites[i].clone());
        }
        return;
    }
    if block.num_sprites == 16 && sprites.len() >= 16 {
        for i in 0..16 {
            slots[i] = Some(sprites[i].clone());
        }
        return;
    }
    let base = if usize::from(block.offset) < SHORE_ACTION5_SLOT_COUNT {
        usize::from(block.offset)
    } else {
        0
    };
    for (i, spr) in sprites.iter().enumerate() {
        let slot = base + i;
        if slot >= SHORE_ACTION5_SLOT_COUNT {
            break;
        }
        slots[slot] = Some(spr.clone());
    }
}

/// Índice local Action5 (0..35) para un `sprite_id` de catenaria del cliente.
#[must_use]
pub fn catenary_action5_local_slot(sprite_id: u32) -> Option<usize> {
    if (CATENARY_WIRE_SPRITE_BASE..=CATENARY_WIRE_SPRITE_BASE + 23).contains(&sprite_id) {
        return Some((sprite_id - CATENARY_WIRE_SPRITE_BASE) as usize);
    }
    if (CATENARY_ENTRANCE_SPRITE_BASE..=CATENARY_ENTRANCE_SPRITE_BASE + 3).contains(&sprite_id) {
        return Some(24 + (sprite_id - CATENARY_ENTRANCE_SPRITE_BASE) as usize);
    }
    if (CATENARY_PYLON_SPRITE_BASE..=CATENARY_PYLON_SPRITE_BASE + 7).contains(&sprite_id) {
        return Some(28 + (sprite_id - CATENARY_PYLON_SPRITE_BASE) as usize);
    }
    None
}

/// Fusiona un bloque Action5 catenary (`0x05`) en la tabla de slots locales.
///
/// El `offset` 1039 (base `OpenTTD`) o 0 empieza en el slot 0; un offset `< 36`
/// se usa como índice de inicio (GRFs de prueba).
pub fn merge_catenary_action5_block(slots: &mut [Option<DecodedSprite>], block: &Action5Block) {
    if block.type_id != ACTION5_TYPE_CATENARY || slots.len() < CATENARY_ACTION5_SLOT_COUNT {
        return;
    }
    if block.sprites.is_empty() {
        return;
    }
    let wire_base = u16::try_from(CATENARY_WIRE_SPRITE_BASE).unwrap_or(1039);
    let base = if block.offset == wire_base || block.offset == 0 {
        0
    } else if usize::from(block.offset) < CATENARY_ACTION5_SLOT_COUNT {
        usize::from(block.offset)
    } else {
        0
    };
    for (i, spr) in block.sprites.iter().enumerate() {
        let slot = base + i;
        if slot >= CATENARY_ACTION5_SLOT_COUNT {
            break;
        }
        slots[slot] = Some(spr.clone());
    }
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

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::newgrf_actions::{build_action0_roadtype_payload, build_action0_train_payload};

    #[test]
    fn decode_flat_8bpp_applies_palette_and_transparency() {
        let w = 2u16;
        let h = 2u16;
        let indices = [0u8, 174, 174, 0]; // 174 ≈ rojo en DOS
        let body = build_real_sprite_v1_uncompressed(w, h, -1, -2, &indices);
        let spr = decode_real_sprite_v1_uncompressed(&body).unwrap();
        assert_eq!(spr.width, 2);
        assert_eq!(spr.height, 2);
        assert_eq!(spr.rgba.len(), 16);
        assert_eq!(&spr.rgba[0..4], &[0, 0, 0, 0]);
        assert_eq!(spr.rgba[7], 255); // alpha del pixel rojo
    }

    #[test]
    fn decompress_lz77_literals_and_backref() {
        // Literal 2 bytes + backref length 2, offset 2 (code 0xF0 = -16).
        let stream = [0x02u8, 0xAA, 0xBB, 0xF0, 0x02];
        let out = decompress_grf_lz77(&stream, 4).unwrap();
        assert_eq!(out, vec![0xAA, 0xBB, 0xAA, 0xBB]);
        let lit = compress_grf_lz77_literals(&[1, 2, 3, 4]);
        assert_eq!(decompress_grf_lz77(&lit, 4).unwrap(), vec![1, 2, 3, 4]);
    }

    #[test]
    fn decode_compressed_sprite_matches_uncompressed() {
        let indices = [0u8, 174, 174, 0];
        let plain = build_real_sprite_v1_uncompressed(2, 2, -1, -2, &indices);
        let compressed = build_real_sprite_v1_compressed(2, 2, -1, -2, &indices);
        let a = decode_real_sprite_v1_uncompressed(&plain).unwrap();
        let b = decode_real_sprite_v1_uncompressed(&compressed).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn decode_chunked_sprite_matches_flat() {
        let indices = [0u8, 174, 174, 0, 174, 0, 0, 174];
        let plain = build_real_sprite_v1_uncompressed(4, 2, -1, -2, &indices);
        let chunked = build_real_sprite_v1_chunked(4, 2, -1, -2, &indices).unwrap();
        let a = decode_real_sprite_v1_uncompressed(&plain).unwrap();
        let b = decode_real_sprite_v1_uncompressed(&chunked).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn collect_train_chunked_sprite_from_synthetic_grf() {
        let a0 = build_action0_train_payload(1981, 95, 720, "Chunk Loco");
        let mut indices = vec![0u8; 8 * 8];
        for y in 2..6 {
            for x in 2..6 {
                indices[y * 8 + x] = 174;
            }
        }
        let bytes = build_grf_v2_train_with_chunked_sprite(
            &a0,
            0,
            8,
            8,
            &indices,
            [b'T', b'C', 0, 1],
            "tchunk",
        )
        .unwrap();
        let gfx = collect_train_sprite_graphics(&bytes).unwrap();
        let preview = gfx.preview_for_local_id(0).unwrap();
        assert_eq!(preview.width, 8);
        assert!(preview.rgba.iter().any(|&b| b != 0));
    }

    #[test]
    fn collect_train_compressed_sprite_from_synthetic_grf() {
        let a0 = build_action0_train_payload(1980, 90, 700, "LZ Loco");
        let mut indices = vec![0u8; 8 * 8];
        for y in 2..6 {
            for x in 2..6 {
                indices[y * 8 + x] = 174;
            }
        }
        let bytes = build_grf_v2_train_with_compressed_sprite(
            &a0,
            0,
            8,
            8,
            &indices,
            [b'T', b'Z', 0, 1],
            "tlz",
        );
        let gfx = collect_train_sprite_graphics(&bytes).unwrap();
        let preview = gfx.preview_for_local_id(0).unwrap();
        assert_eq!(preview.width, 8);
        assert!(preview.rgba.iter().any(|&b| b != 0));
    }

    #[test]
    fn collect_action1_3_preview_from_synthetic_grf() {
        let a0 = build_action0_train_payload(1960, 100, 800, "Sprite Loco");
        let mut indices = vec![0u8; 8 * 8];
        for y in 2..6 {
            for x in 2..6 {
                indices[y * 8 + x] = 174;
            }
        }
        let bytes = build_grf_v2_train_with_preview_sprite(
            &a0,
            0,
            8,
            8,
            &indices,
            [b'T', b'S', 0, 1],
            "tsprite",
        );
        let gfx = collect_train_sprite_graphics(&bytes).unwrap();
        assert_eq!(gfx.sets.len(), 1);
        assert_eq!(gfx.sets[0].len(), 1);
        assert_eq!(gfx.assigns.len(), 1);
        let preview = gfx.preview_for_local_id(0).unwrap();
        assert_eq!(preview.width, 8);
        assert_eq!(preview.height, 8);
        assert!(preview.rgba.iter().any(|&b| b != 0));
    }

    #[test]
    fn collect_train_action2_chain_resolves_to_action1_set() {
        let a0 = build_action0_train_payload(1975, 120, 900, "A2 Loco");
        let mut indices = vec![0u8; 8 * 8];
        for y in 2..6 {
            for x in 2..6 {
                indices[y * 8 + x] = 174;
            }
        }
        let a2_id = 7u8;
        let bytes = build_grf_v2_train_with_action2_chain(
            &a0,
            0,
            a2_id,
            8,
            8,
            &indices,
            [b'T', b'A', 0, 2],
            "ta2",
        );
        let gfx = collect_train_sprite_graphics(&bytes).unwrap();
        assert_eq!(gfx.sets.len(), 1);
        assert_eq!(gfx.action2_to_action1.get(&a2_id), Some(&0));
        assert_eq!(gfx.assigns[0].set_id, u16::from(a2_id));
        // Sin Action2: sets[7] no existe; con resolución → set 0.
        assert!(gfx.sets.get(usize::from(a2_id)).is_none());
        let preview = gfx.preview_for_local_id(0).unwrap();
        assert_eq!(preview.width, 8);
        assert_eq!(gfx.resolve_action1_set(u16::from(a2_id)), 0);
    }

    #[test]
    fn parse_action2_skips_variational() {
        // 02 trains set=1 variational 0x81 …
        let payload = [0x02, ACTION0_FEATURE_TRAINS, 0x01, 0x81, 0x00];
        assert!(parse_action2_vehicle_basic(&payload, ACTION0_FEATURE_TRAINS).is_none());
        let basic = build_action2_trains_payload(3, 0, 0);
        assert_eq!(
            parse_action2_vehicle_basic(&basic, ACTION0_FEATURE_TRAINS),
            Some((3, 0))
        );
    }

    #[test]
    fn collect_roadtype_preview_from_synthetic_grf() {
        let a0 = build_action0_roadtype_payload(b"COBB", false, 1970, "Cobble");
        let mut indices = vec![0u8; 8 * 8];
        for y in 1..7 {
            for x in 1..7 {
                indices[y * 8 + x] = 200;
            }
        }
        let bytes = build_grf_v2_roadtype_with_preview_sprite(
            &a0,
            0,
            8,
            8,
            &indices,
            [b'R', b'T', 0, 2],
            "rtgfx",
        );
        let gfx = collect_roadtype_sprite_graphics(&bytes).unwrap();
        assert_eq!(gfx.sets.len(), 1);
        let preview = gfx.preview_for_local_id(0).unwrap();
        assert_eq!(preview.width, 8);
        assert!(preview.rgba.iter().any(|&b| b != 0));
        // Feature distinto: trains no ve el set.
        let trains = collect_train_sprite_graphics(&bytes).unwrap();
        assert!(trains.sets.is_empty());
    }

    #[test]
    fn collect_station_preview_from_synthetic_grf() {
        use crate::newgrf_actions::build_action0_station_payload;
        let a0 = build_action0_station_payload(b"MODN", b"Plat", 0, 0, "Andén");
        let mut indices = vec![0u8; 8 * 8];
        for y in 2..6 {
            for x in 2..6 {
                indices[y * 8 + x] = 174;
            }
        }
        let bytes = build_grf_v2_station_with_preview_sprite(
            &a0,
            0,
            8,
            8,
            &indices,
            [b'S', b'T', 0, 3],
            "stgfx",
        );
        let gfx = collect_station_sprite_graphics(&bytes).unwrap();
        assert_eq!(gfx.sets.len(), 1);
        let preview = gfx.preview_for_local_id(0).unwrap();
        assert_eq!(preview.width, 8);
    }

    #[test]
    fn collect_action5_block_with_preview() {
        let mut indices = vec![0u8; 8 * 8];
        for y in 1..7 {
            for x in 1..7 {
                indices[y * 8 + x] = 174;
            }
        }
        let bytes = build_grf_v2_action5_with_sprite(
            0x0D,
            4804,
            8,
            8,
            &indices,
            [b'S', b'H', 0, 1],
            "shore",
        );
        let blocks = collect_action5_blocks(&bytes).unwrap();
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].type_id, 0x0D);
        assert_eq!(blocks[0].num_sprites, 1);
        assert_eq!(blocks[0].offset, 4804);
        assert_eq!(blocks[0].sprites.len(), 1);
        assert_eq!(action5_type_name(0x0D), "shore");
        let preview = blocks[0].first_preview.as_ref().unwrap();
        assert_eq!(preview.width, 8);
        assert!(preview.rgba.iter().any(|&b| b != 0));
        let mut slots = vec![None; SHORE_ACTION5_SLOT_COUNT];
        merge_shore_action5_block(&mut slots, &blocks[0]);
        // offset 4804 ≥ 18 → escribe en slot 0
        assert!(slots[0].is_some());
    }
}
