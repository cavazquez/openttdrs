//! Decodificación y codificación de píxeles de sprites `NewGRF`.
//!
//! Soporta v1 (inline) y v2 (sprite section), 8bpp/32bpp, LZ77, chunked, máscaras.

use std::collections::HashMap;

use crate::newgrf_company_ramp::{
    AUTHOR_CC_PALETTE_FIRST, COMPANY_COLOUR_COUNT, COMPANY_RAMP_RGB, COMPANY_RAMP_SHADES,
};
use crate::newgrf_config::GrfContainerVersion;
use crate::newgrf_palette_data::DOS_PALETTE_RGB;

use super::model::DecodedSprite;

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

/// Decodifica buffer "tile" chunked 8bpp (offsets u16 + trozos `cinfo`/`cofs`).
#[must_use]
pub fn decode_chunked_8bpp(buf: &[u8], width: u16, height: u16) -> Option<Vec<u8>> {
    decode_chunked_pixels(buf, width, height, 1)
}

/// Decodifica buffer chunked con `bpp` bytes/píxel (8bpp o 32bpp RGB/A/M).
///
/// Formato `OpenTTD` v2 `width ≤ 256`: por fila, offset LE; trozos
/// `(length|last, skip, pixels×bpp…)`.
#[must_use]
pub fn decode_chunked_pixels(buf: &[u8], width: u16, height: u16, bpp: usize) -> Option<Vec<u8>> {
    let w = usize::from(width);
    let h = usize::from(height);
    if w == 0 || h == 0 || w > 256 || bpp == 0 || bpp > 5 {
        return None;
    }
    let table_bytes = h.checked_mul(2)?;
    if buf.len() < table_bytes {
        return None;
    }
    let mut out = vec![0u8; w.checked_mul(h)?.checked_mul(bpp)?];
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
            let nbytes = length.checked_mul(bpp)?;
            if skip.checked_add(length)? > w || dest.checked_add(nbytes)? > buf.len() {
                return None;
            }
            let row_at = y.checked_mul(w)?.checked_add(skip)?.checked_mul(bpp)?;
            out[row_at..row_at + nbytes].copy_from_slice(&buf[dest..dest + nbytes]);
            dest += nbytes;
            if last {
                break;
            }
        }
    }
    Some(out)
}

/// Codifica píxeles planos → chunked (1 trozo de fila completa, `width ≤ 127`).
#[must_use]
pub fn encode_chunked_pixels_full_rows(
    width: u16,
    height: u16,
    bpp: usize,
    pixels: &[u8],
) -> Option<Vec<u8>> {
    let w = usize::from(width);
    let h = usize::from(height);
    let row_bytes = w.checked_mul(bpp)?;
    if w == 0 || h == 0 || w > 127 || bpp == 0 || pixels.len() < h.checked_mul(row_bytes)? {
        return None;
    }
    let table_bytes = h * 2;
    let mut body = Vec::with_capacity(h * (2 + row_bytes));
    let mut offsets = Vec::with_capacity(h);
    for y in 0..h {
        offsets.push(u16::try_from(table_bytes + body.len()).ok()?);
        let cinfo = 0x80u8 | u8::try_from(w).ok()?;
        body.push(cinfo);
        body.push(0);
        let row = y * row_bytes;
        body.extend_from_slice(&pixels[row..row + row_bytes]);
    }
    let mut out = Vec::with_capacity(table_bytes + body.len());
    for off in offsets {
        out.extend_from_slice(&off.to_le_bytes());
    }
    out.extend(body);
    Some(out)
}

/// Codifica índices planos → buffer chunked (1 trozo de fila completa, `width ≤ 127`).
#[must_use]
pub fn encode_chunked_8bpp_full_rows(width: u16, height: u16, indices: &[u8]) -> Option<Vec<u8>> {
    encode_chunked_pixels_full_rows(width, height, 1, indices)
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
        mask: Vec::new(),
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
pub(super) fn decode_real_sprite_entry(
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

/// Índice sprite section v2: `id` → lista `(info, body)` (body = tras el BYTE info).
#[must_use]
pub fn index_sprite_section(section: &[u8]) -> HashMap<u32, Vec<(u8, &[u8])>> {
    let mut map: HashMap<u32, Vec<(u8, &[u8])>> = HashMap::new();
    let mut i = 0usize;
    while i + 8 <= section.len() {
        let id = u32::from_le_bytes([section[i], section[i + 1], section[i + 2], section[i + 3]]);
        if id == 0 {
            break;
        }
        let size = u32::from_le_bytes([
            section[i + 4],
            section[i + 5],
            section[i + 6],
            section[i + 7],
        ]) as usize;
        let info_at = i + 8;
        if size == 0 || info_at + size > section.len() {
            break;
        }
        let info = section[info_at];
        let body = &section[info_at + 1..info_at + size];
        map.entry(id).or_default().push((info, body));
        i = info_at + size;
    }
    map
}

/// Orden de preferencia de zoom v2 (`OpenTTD` `zoom_lvl_map` invertido para UI).
///
/// 0=normal, 2=2×in, 1=4×in, 3=2×out, 4=4×out, 5=8×out.
pub const SPRITE_V2_ZOOM_PREFERENCE: [u8; 6] = [0, 2, 1, 3, 4, 5];

/// Bytes por píxel según bits `info` v2 (RGB / alpha / palette).
#[must_use]
pub const fn sprite_v2_bpp(info: u8) -> usize {
    let mut bpp = 0usize;
    if info & 0x01 != 0 {
        bpp += 3;
    }
    if info & 0x02 != 0 {
        bpp += 1;
    }
    if info & 0x04 != 0 {
        bpp += 1;
    }
    bpp
}

/// Convierte buffer descomprimido v2 (componentes R,G,B,A,M) → `(rgba, mask)`.
fn v2_pixels_to_rgba_mask(
    info: u8,
    pixels: &[u8],
    width: u16,
    height: u16,
) -> Option<(Vec<u8>, Vec<u8>)> {
    let pixel_count = usize::from(width).checked_mul(usize::from(height))?;
    let bpp = sprite_v2_bpp(info);
    if bpp == 0 || pixels.len() < pixel_count.checked_mul(bpp)? {
        return None;
    }
    let has_rgb = info & 0x01 != 0;
    let has_a = info & 0x02 != 0;
    let has_pal = info & 0x04 != 0;
    if has_pal && !has_rgb {
        let rgba = indices_to_rgba(&pixels[..pixel_count], width, height)?;
        return Some((rgba, Vec::new()));
    }
    let mut rgba = Vec::with_capacity(pixel_count * 4);
    let mut mask = if has_pal {
        Vec::with_capacity(pixel_count)
    } else {
        Vec::new()
    };
    for idx in 0..pixel_count {
        let px = &pixels[idx * bpp..];
        let mut o = 0usize;
        let (red, green, blue) = if has_rgb {
            o = 3;
            (px[0], px[1], px[2])
        } else {
            (0, 0, 0)
        };
        let alpha = if has_a {
            let a = px[o];
            o += 1;
            a
        } else {
            255
        };
        if has_pal {
            mask.push(px.get(o).copied().unwrap_or(0));
        }
        rgba.extend_from_slice(&[red, green, blue, alpha]);
    }
    Some((rgba, mask))
}

const DEFAULT_BRIGHTNESS: u32 = 128;

/// Ajusta brillo estilo `OpenTTD` (`AdjustBrightness`, aproximación).
fn adjust_brightness_rgb(rgb: [u8; 3], brightness: u8) -> [u8; 3] {
    let bright = u32::from(brightness);
    if bright == DEFAULT_BRIGHTNESS {
        return rgb;
    }
    let scale = |channel: u8| -> u8 {
        let scaled = (u32::from(channel) * bright) >> 7;
        u8::try_from(scaled.min(255)).unwrap_or(255)
    };
    [scale(rgb[0]), scale(rgb[1]), scale(rgb[2])]
}

/// Aplica máscara company-colour in-place sobre RGBA (`mask==0` → sin cambio).
///
/// Índices `198..=205` (rampa autor) se remapean a la rampa de `company_colour`.
pub fn apply_company_colour_mask(rgba: &mut [u8], mask: &[u8], company_colour: u8) {
    let company = usize::from(company_colour) % COMPANY_COLOUR_COUNT;
    let pixel_count = mask.len().min(rgba.len() / 4);
    let author_end =
        AUTHOR_CC_PALETTE_FIRST.saturating_add(u8::try_from(COMPANY_RAMP_SHADES).unwrap_or(8));
    for idx in 0..pixel_count {
        let mask_idx = mask[idx];
        if mask_idx == 0 {
            continue;
        }
        let px = &mut rgba[idx * 4..idx * 4 + 4];
        let brightness = px[0].max(px[1]).max(px[2]);
        let base = if (AUTHOR_CC_PALETTE_FIRST..author_end).contains(&mask_idx) {
            let shade = usize::from(mask_idx - AUTHOR_CC_PALETTE_FIRST) % COMPANY_RAMP_SHADES;
            COMPANY_RAMP_RGB[company * COMPANY_RAMP_SHADES + shade]
        } else {
            DOS_PALETTE_RGB[usize::from(mask_idx)]
        };
        let tuned = adjust_brightness_rgb(base, brightness);
        px[0] = tuned[0];
        px[1] = tuned[1];
        px[2] = tuned[2];
    }
}

/// Hornea la máscara del sprite con el color de compañía (copia RGBA).
#[must_use]
pub fn bake_sprite_company_mask(sprite: &DecodedSprite, company_colour: u8) -> Vec<u8> {
    let mut rgba = sprite.rgba.clone();
    if !sprite.mask.is_empty() {
        apply_company_colour_mask(&mut rgba, &sprite.mask, company_colour);
    }
    rgba
}

/// Decodifica imagen de sprite section v2 (8bpp / 32bpp / máscara / chunked).
///
/// Devuelve `(zoom, sprite)`.
#[must_use]
pub fn decode_real_sprite_v2_section_zoom(info: u8, body: &[u8]) -> Option<(u8, DecodedSprite)> {
    if info == 0xFF {
        return None;
    }
    let colour = info & 0x07;
    let bpp = sprite_v2_bpp(colour);
    if bpp == 0 {
        return None;
    }
    if body.len() < 9 {
        return None;
    }
    let zoom = body[0];
    if zoom > 5 {
        return None;
    }
    let height = u16::from_le_bytes([body[1], body[2]]);
    let width = u16::from_le_bytes([body[3], body[4]]);
    let x_offs = i16::from_le_bytes([body[5], body[6]]);
    let y_offs = i16::from_le_bytes([body[7], body[8]]);
    if width == 0 || height == 0 || width > 512 || height > 512 {
        return None;
    }
    let chunked = info & 0x08 != 0;
    let mut pos = 9usize;
    let pixel_count = usize::from(width).checked_mul(usize::from(height))?;
    let decomp_size = if chunked {
        if body.len() < pos + 4 {
            return None;
        }
        let n =
            u32::from_le_bytes([body[pos], body[pos + 1], body[pos + 2], body[pos + 3]]) as usize;
        pos += 4;
        n
    } else {
        pixel_count.checked_mul(bpp)?
    };
    if decomp_size == 0 || decomp_size > 512 * 512 * 5 {
        return None;
    }
    let intermediate = decompress_grf_lz77(&body[pos..], decomp_size)?;
    let flat = if chunked {
        decode_chunked_pixels(&intermediate, width, height, bpp)?
    } else {
        intermediate
    };
    let (rgba, mask) = v2_pixels_to_rgba_mask(colour, &flat, width, height)?;
    Some((
        zoom,
        DecodedSprite {
            width,
            height,
            x_offs,
            y_offs,
            rgba,
            mask,
        },
    ))
}

/// Decodifica imagen v2; solo zoom normal (`0`).
#[must_use]
pub fn decode_real_sprite_v2_section(info: u8, body: &[u8]) -> Option<DecodedSprite> {
    let (zoom, spr) = decode_real_sprite_v2_section_zoom(info, body)?;
    (zoom == 0).then_some(spr)
}

/// Mejor vista del ID: zoom preferido, luego 32bpp sobre 8bpp.
#[must_use]
pub fn resolve_fd_sprite<S: ::std::hash::BuildHasher>(
    index: &HashMap<u32, Vec<(u8, &[u8])>, S>,
    sprite_id: u32,
) -> Option<DecodedSprite> {
    let entries = index.get(&sprite_id)?;
    let mut best: Option<(usize, usize, DecodedSprite)> = None;
    for &(info, body) in entries {
        let Some((zoom, spr)) = decode_real_sprite_v2_section_zoom(info, body) else {
            continue;
        };
        let zoom_rank = SPRITE_V2_ZOOM_PREFERENCE
            .iter()
            .position(|&z| z == zoom)
            .unwrap_or(usize::MAX);
        let depth_rank = usize::from(info & 0x01 == 0);
        let better = best
            .as_ref()
            .is_none_or(|(bz, bd, _)| (zoom_rank, depth_rank) < (*bz, *bd));
        if better {
            best = Some((zoom_rank, depth_rank, spr));
            if zoom_rank == 0 && depth_rank == 0 {
                break;
            }
        }
    }
    best.map(|(_, _, spr)| spr)
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
