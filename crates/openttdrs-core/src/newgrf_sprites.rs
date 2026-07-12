//! Decode mínimo de sprites reales `NewGRF` + Action1/2/3 (trains / roadtypes, preview).
//!
//! Contenedor **v1** (inline) o **v2** (sprite section + `0xFD`).
//! 8bpp/32bpp plano / LZ77 / chunked; multi-zoom; máscara company-colour.
//! Action3→Action2 (básico / variational+advanced+7E/7C / random 80/83/84)→Action1.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::newgrf_actions::{
    ACTION0_FEATURE_ROADTYPES, ACTION0_FEATURE_STATIONS, ACTION0_FEATURE_TRAINS,
};
use crate::newgrf_company_ramp::{
    AUTHOR_CC_PALETTE_FIRST, COMPANY_COLOUR_COUNT, COMPANY_RAMP_RGB, COMPANY_RAMP_SHADES,
};
use crate::newgrf_config::{GrfContainerVersion, GrfScanError, parse_grf_full};
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
    /// Máscara 8bpp (mismo `width*height`); vacío si no hay.
    #[serde(default)]
    pub mask: Vec<u8>,
}

/// Asignación Action3: id local → set Action2 (o índice Action1 si no hay Action2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrainSpriteAssign {
    pub local_id: u8,
    pub set_id: u16,
}

/// Ajuste `varadjust` (shift/and [+add+div|mod]).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Action2VarAdjust {
    /// Bits 0..4 del `shift-num`.
    pub shift: u8,
    pub and_mask: u8,
    pub add_val: Option<u8>,
    pub divide_val: Option<u8>,
    pub modulo_val: Option<u8>,
}

/// Un término variable + ajuste (y parámetro opcional para `60+x`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Action2VarTerm {
    pub variable: u8,
    /// Parámetro tras variables `60+x` (p. ej. registro `7D`).
    pub param: Option<u8>,
    pub adjust: Action2VarAdjust,
}

/// Operación advanced: `operator` entre acumulador y el siguiente término.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Action2VarOp {
    pub operator: u8,
    pub rhs: Action2VarTerm,
}

/// Action2 variational (`0x81`/`0x82`): variable + rangos + default.
///
/// Con bit 5 en `shift-num` se encadena `ops` (advanced). Sin bit 5, `ops` vacío.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Action2VarEntry {
    pub first: Action2VarTerm,
    /// Cadena advanced (`operator` + término); vacía = variational simple.
    pub ops: Vec<Action2VarOp>,
    /// `(result_set, low, high)` inclusive.
    pub ranges: Vec<(u16, u8, u8)>,
    pub default: u16,
}

/// Action2 random (`0x80`/`0x83`/`0x84`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Action2RandomEntry {
    /// `0x80` propio, `0x83` related, `0x84` consist.
    pub typ: u8,
    /// Solo `0x84`: conteo desde vehículo de control (nibble bajo = offset).
    pub consist_count: u8,
    pub triggers: u8,
    pub randbit: u8,
    pub sets: Vec<u16>,
}

/// Contexto para evaluar variational / random (preview o runtime).
#[derive(Debug, Clone, Default)]
pub struct Action2EvalCtx {
    /// Valores de variables `NewGRF` (`variable` → raw).
    pub vars: HashMap<u8, u32>,
    /// Bits aleatorios del objeto (vehículo/estación/…).
    pub random_bits: u32,
    /// Bits de vehículos del consist indexados por offset (`0x84` nibble bajo).
    pub consist_random_bits: HashMap<u8, u32>,
    /// Registros temporales (variable `7D` / operador `\2sto`).
    pub temp_registers: HashMap<u8, u32>,
    /// Registros persistentes (variable `7C` / operador `\2psto`).
    pub persistent_registers: HashMap<u8, u32>,
    /// Último resultado de un `VarAction2` (variable `1C`; p. ej. tras procedure `7E`).
    pub last_result: u32,
}

/// Resultado de parsear Action1/2/3 de un feature (trains / roadtypes).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TrainSpriteGraphics {
    /// `sets[set_id][view]` — sets Action1 en orden de aparición.
    pub sets: Vec<Vec<DecodedSprite>>,
    pub assigns: Vec<TrainSpriteAssign>,
    /// Action2 set-id → índice del primer set Action1 “moving” (solo trains).
    pub action2_to_action1: HashMap<u8, u16>,
    /// Action2 variational completo (rangos + default / advanced).
    pub action2_var: HashMap<u8, Action2VarEntry>,
    /// Action2 random (`0x80`/`0x83`/`0x84`).
    pub action2_random: HashMap<u8, Action2RandomEntry>,
}

impl TrainSpriteGraphics {
    /// Preview (primera vista) para un id local.
    #[must_use]
    pub fn preview_for_local_id(&self, local_id: u8) -> Option<&DecodedSprite> {
        self.views_for_local_id(local_id)?.first()
    }

    /// Resuelve sin contexto (variational → `default`; random → set[0]).
    #[must_use]
    pub fn resolve_action1_set(&self, action3_set_id: u16) -> u16 {
        self.resolve_action1_set_ctx(action3_set_id, &mut Action2EvalCtx::default())
    }

    /// Resuelve Action3 → var/random → Action2 básico → Action1.
    pub fn resolve_action1_set_ctx(&self, action3_set_id: u16, ctx: &mut Action2EvalCtx) -> u16 {
        let mut id = action3_set_id;
        for _ in 0..8 {
            let a2 = u8::try_from(id).unwrap_or(u8::MAX);
            if let Some(rnd) = self.action2_random.get(&a2) {
                let next = eval_action2_random(rnd, ctx);
                if next & 0x8000 != 0 {
                    break;
                }
                id = next;
                continue;
            }
            if let Some(var) = self.action2_var.get(&a2).cloned() {
                let next = eval_action2_var(self, &var, ctx, 0);
                if next & 0x8000 != 0 {
                    break;
                }
                id = next;
                continue;
            }
            if let Some(&a1) = self.action2_to_action1.get(&a2) {
                return a1;
            }
            return id;
        }
        self.action2_to_action1
            .get(&u8::try_from(id).unwrap_or(u8::MAX))
            .copied()
            .unwrap_or(id)
    }

    /// Todas las vistas del set asignado al id local (ctx por defecto).
    #[must_use]
    pub fn views_for_local_id(&self, local_id: u8) -> Option<&[DecodedSprite]> {
        self.views_for_local_id_ctx(local_id, &mut Action2EvalCtx::default())
    }

    /// Vistas resolviendo Action2 con contexto (random/consist/advanced).
    pub fn views_for_local_id_ctx(
        &self,
        local_id: u8,
        ctx: &mut Action2EvalCtx,
    ) -> Option<&[DecodedSprite]> {
        let set_id = self
            .assigns
            .iter()
            .find(|a| a.local_id == local_id)
            .map(|a| a.set_id)
            .or_else(|| (!self.sets.is_empty()).then_some(0))?;
        let action1_idx = self.resolve_action1_set_ctx(set_id, ctx);
        self.sets
            .get(usize::from(action1_idx))
            .map(Vec::as_slice)
            .filter(|s| !s.is_empty())
    }

    /// ¿Necesita re-resolución en runtime (random o cualquier variational)?
    #[must_use]
    pub fn needs_runtime_resolve(&self) -> bool {
        !self.action2_random.is_empty() || !self.action2_var.is_empty()
    }
}

fn apply_var_adjust(raw: u32, adj: &Action2VarAdjust) -> i32 {
    // Cast wrapping: literales `0x1A` usan `0xFFFFFFFF` → `-1` en i32.
    let mut value = raw.wrapping_shr(u32::from(adj.shift & 0x1F)).cast_signed();
    value &= i32::from(adj.and_mask);
    if let Some(add) = adj.add_val {
        value = value.wrapping_add(i32::from(add));
    }
    if let Some(div) = adj.divide_val
        && div != 0
    {
        value /= i32::from(div);
    } else if let Some(modulo) = adj.modulo_val
        && modulo != 0
    {
        value %= i32::from(modulo);
    }
    value
}

fn read_action2_var(
    gfx: &TrainSpriteGraphics,
    ctx: &mut Action2EvalCtx,
    term: &Action2VarTerm,
    depth: u8,
) -> Option<u32> {
    match term.variable {
        // Literal: `and-mask` selecciona bits de 0xFFFFFFFF.
        0x1A => Some(0xFFFF_FFFF),
        // Resultado del VarAction2 anterior / procedure.
        0x1C => Some(ctx.last_result),
        // Registro persistente `7C[param]`.
        0x7C => {
            let idx = term.param.unwrap_or(0);
            Some(ctx.persistent_registers.get(&idx).copied().unwrap_or(0))
        }
        // Registro temporal `7D[param]`.
        0x7D => {
            let idx = term.param.unwrap_or(0);
            Some(ctx.temp_registers.get(&idx).copied().unwrap_or(0))
        }
        // Procedure call: parámetro = set-id Action2 a invocar.
        0x7E => {
            let proc_id = term.param.unwrap_or(0);
            Some(invoke_action2_procedure(gfx, proc_id, ctx, depth))
        }
        v => ctx.vars.get(&v).copied(),
    }
}

fn eval_term(
    gfx: &TrainSpriteGraphics,
    ctx: &mut Action2EvalCtx,
    term: &Action2VarTerm,
    depth: u8,
) -> Option<i32> {
    let raw = read_action2_var(gfx, ctx, term, depth)?;
    Some(apply_var_adjust(raw, &term.adjust))
}

fn apply_advanced_op(op: u8, val1: i32, val2: i32, ctx: &mut Action2EvalCtx) -> i32 {
    match op {
        0x00 => val1.wrapping_add(val2),
        0x01 => val1.wrapping_sub(val2),
        0x02 => val1.min(val2),
        0x03 => val1.max(val2),
        0x04 => val1.cast_unsigned().min(val2.cast_unsigned()).cast_signed(),
        0x05 => val1.cast_unsigned().max(val2.cast_unsigned()).cast_signed(),
        0x06 => {
            if val2 != 0 {
                val1 / val2
            } else {
                val1
            }
        }
        0x07 => {
            if val2 != 0 {
                val1 % val2
            } else {
                val1
            }
        }
        0x08 => {
            if val2 != 0 {
                (val1.cast_unsigned() / val2.cast_unsigned()).cast_signed()
            } else {
                val1
            }
        }
        0x09 => {
            if val2 != 0 {
                (val1.cast_unsigned() % val2.cast_unsigned()).cast_signed()
            } else {
                val1
            }
        }
        0x0A => val1.wrapping_mul(val2),
        0x0B => val1 & val2,
        0x0C => val1 | val2,
        0x0D => val1 ^ val2,
        // \2sto: temp_registers[val2] = val1
        0x0E => {
            let idx = u8::try_from(val2 & 0xFF).unwrap_or(0);
            ctx.temp_registers.insert(idx, val1.cast_unsigned());
            val1
        }
        // \2rst: result = val2
        0x0F => val2,
        // \2psto: persistent_registers[val2] = val1
        0x10 => {
            let idx = u8::try_from(val2 & 0xFF).unwrap_or(0);
            ctx.persistent_registers.insert(idx, val1.cast_unsigned());
            val1
        }
        0x11 => val1.rotate_right(val2.cast_unsigned() & 31),
        0x12 => match val1.cmp(&val2) {
            std::cmp::Ordering::Less => 0,
            std::cmp::Ordering::Equal => 1,
            std::cmp::Ordering::Greater => 2,
        },
        0x13 => {
            let a = val1.cast_unsigned();
            let b = val2.cast_unsigned();
            match a.cmp(&b) {
                std::cmp::Ordering::Less => 0,
                std::cmp::Ordering::Equal => 1,
                std::cmp::Ordering::Greater => 2,
            }
        }
        0x14 => val1.wrapping_shl(val2.cast_unsigned() & 31),
        0x15 => (val1.cast_unsigned() >> (val2.cast_unsigned() & 31)).cast_signed(),
        0x16 => val1.wrapping_shr(val2.cast_unsigned() & 31),
        _ => val1,
    }
}

/// Invoca un Action2 como procedure (`7E`); el valor calculado alimenta la variable.
fn invoke_action2_procedure(
    gfx: &TrainSpriteGraphics,
    set_id: u8,
    ctx: &mut Action2EvalCtx,
    depth: u8,
) -> u32 {
    if depth >= 8 {
        return 0;
    }
    let mut id = u16::from(set_id);
    for _ in 0..8 {
        let a2 = u8::try_from(id).unwrap_or(u8::MAX);
        if let Some(rnd) = gfx.action2_random.get(&a2) {
            let next = eval_action2_random(rnd, ctx);
            if next & 0x8000 != 0 {
                let v = u32::from(next & 0x7FFF);
                ctx.last_result = v;
                return v;
            }
            id = next;
            continue;
        }
        if let Some(var) = gfx.action2_var.get(&a2).cloned() {
            let next = eval_action2_var(gfx, &var, ctx, depth.saturating_add(1));
            if next & 0x8000 != 0 {
                // `last_result` guarda el acumulador completo (32-bit).
                return ctx.last_result;
            }
            id = next;
            continue;
        }
        // Cadena termina en Action2 básico → 0xFFFF.
        ctx.last_result = 0xFFFF;
        return 0xFFFF;
    }
    ctx.last_result = 0xFFFF;
    0xFFFF
}

fn eval_action2_var(
    gfx: &TrainSpriteGraphics,
    entry: &Action2VarEntry,
    ctx: &mut Action2EvalCtx,
    depth: u8,
) -> u16 {
    let Some(mut acc) = eval_term(gfx, ctx, &entry.first, depth) else {
        return entry.default;
    };
    for op in &entry.ops {
        let Some(rhs) = eval_term(gfx, ctx, &op.rhs, depth) else {
            return entry.default;
        };
        acc = apply_advanced_op(op.operator, acc, rhs, ctx);
    }
    ctx.last_result = acc.cast_unsigned();
    // nvar=0: devolver el valor calculado como callback (procedure / result).
    if entry.ranges.is_empty() {
        let low = u16::try_from(acc & 0x7FFF).unwrap_or(0);
        return low | 0x8000;
    }
    let value_u8 = u8::try_from(acc & 0xFF).unwrap_or(0);
    for &(result, low, high) in &entry.ranges {
        if value_u8 >= low && value_u8 <= high {
            return result;
        }
    }
    entry.default
}

fn eval_action2_random(entry: &Action2RandomEntry, ctx: &Action2EvalCtx) -> u16 {
    let count_key = entry.consist_count & 0x0F;
    let bits = if entry.typ == 0x84 {
        ctx.consist_random_bits
            .get(&count_key)
            .copied()
            .unwrap_or(ctx.random_bits)
    } else {
        ctx.random_bits
    };
    let n = entry.sets.len();
    if n == 0 {
        return 0;
    }
    let mask = n.next_power_of_two().saturating_sub(1);
    let idx = (usize::try_from(bits >> entry.randbit).unwrap_or(0) & mask) % n;
    entry.sets[idx]
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
    // Variational / random Action2 → `parse_action2_variational` / `parse_action2_random`.
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
fn parse_action2_variational(payload: &[u8], feature: u8) -> Option<(u8, Action2VarEntry)> {
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
fn parse_action2_random(payload: &[u8], feature: u8) -> Option<(u8, Action2RandomEntry)> {
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

/// Recorre el GRF y extrae sets Action1 + Action2 (trains) + asignaciones Action3.
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
            } else if feature == ACTION0_FEATURE_TRAINS
                && let Some((a2_id, var)) = parse_action2_variational(payload, feature)
            {
                out.action2_var.insert(a2_id, var);
            } else if feature == ACTION0_FEATURE_TRAINS
                && let Some((a2_id, rnd)) = parse_action2_random(payload, feature)
            {
                out.action2_random.insert(a2_id, rnd);
            } else if let Some(assigns) = parse_action3_feature(payload, feature) {
                out.assigns.extend(assigns);
            }
            i = end;
            continue;
        }

        let end = match container {
            GrfContainerVersion::V1 => i + 2 + size,
            GrfContainerVersion::V2 => payload_start + size,
        };
        if end > section.len() {
            break;
        }

        if sets_left > 0 || views_left_in_set > 0 {
            let decoded = if container == GrfContainerVersion::V2 && info == 0xFD {
                if size == 4 && payload_start + 4 <= section.len() {
                    let id = u32::from_le_bytes([
                        section[payload_start],
                        section[payload_start + 1],
                        section[payload_start + 2],
                        section[payload_start + 3],
                    ]);
                    resolve_fd_sprite(&sprite_index, id)
                } else {
                    None
                }
            } else if let Some(payload) = real_sprite_payload(section, i, size, header, container) {
                decode_real_sprite_entry(container, info, payload)
            } else {
                None
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

        i = end;
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
    let parsed = parse_grf_full(data)?;
    let container = parsed.container;
    let section = parsed.data_section;
    let sprite_index = index_sprite_section(parsed.sprite_section);
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

        let end = match container {
            GrfContainerVersion::V1 => i + 2 + size,
            GrfContainerVersion::V2 => payload_start + size,
        };
        if end > section.len() {
            break;
        }

        if in_block && sprites_left > 0 {
            let spr = if container == GrfContainerVersion::V2 && info == 0xFD {
                if size == 4 && payload_start + 4 <= section.len() {
                    let id = u32::from_le_bytes([
                        section[payload_start],
                        section[payload_start + 1],
                        section[payload_start + 2],
                        section[payload_start + 3],
                    ]);
                    resolve_fd_sprite(&sprite_index, id)
                } else {
                    None
                }
            } else if let Some(payload) = real_sprite_payload(section, i, size, header, container) {
                decode_real_sprite_entry(container, info, payload)
            } else {
                None
            };
            if let Some(spr) = spr {
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

        i = end;
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
    use crate::map::TileCoord;
    use crate::newgrf_actions::{build_action0_roadtype_payload, build_action0_train_payload};
    use crate::vehicle::Vehicle;

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
    fn collect_train_fd_sprite_from_sprite_section() {
        let a0 = build_action0_train_payload(1982, 100, 750, "FD Loco");
        let mut indices = vec![0u8; 8 * 8];
        for y in 2..6 {
            for x in 2..6 {
                indices[y * 8 + x] = 174;
            }
        }
        let bytes =
            build_grf_v2_train_with_fd_sprite(&a0, 0, 1, 8, 8, &indices, [b'T', b'F', 0, 1], "tfd");
        let gfx = collect_train_sprite_graphics(&bytes).unwrap();
        assert_eq!(gfx.sets.len(), 1);
        let preview = gfx.preview_for_local_id(0).unwrap();
        assert_eq!(preview.width, 8);
        assert!(preview.rgba.iter().any(|&b| b != 0));
    }

    #[test]
    fn decode_v2_section_palette_roundtrip() {
        let indices = [0u8, 174, 174, 0];
        let entry = build_sprite_section_palette_entry(7, 0, 2, 2, -1, -2, &indices);
        let index = index_sprite_section(&entry);
        let spr = resolve_fd_sprite(&index, 7).unwrap();
        assert_eq!(spr.width, 2);
        assert_eq!(spr.height, 2);
    }

    #[test]
    fn resolve_fd_prefers_normal_zoom_over_2x_in() {
        let normal = [10u8, 11, 12, 13];
        let zoom2 = [20u8, 21, 22, 23];
        let mut section = build_sprite_section_palette_entry(3, 2, 2, 2, 0, 0, &zoom2);
        // Mismo ID, zoom normal después (OpenTTD agrupa por id).
        section.extend(build_sprite_section_palette_entry(
            3, 0, 2, 2, 0, 0, &normal,
        ));
        let index = index_sprite_section(&section);
        let spr = resolve_fd_sprite(&index, 3).unwrap();
        // Pixel (0,0) del zoom normal = índice 10 → no transparente.
        assert_ne!(&spr.rgba[0..4], &[0, 0, 0, 0]);
        let only_2x = build_sprite_section_palette_entry(4, 2, 2, 2, 0, 0, &zoom2);
        let index2 = index_sprite_section(&only_2x);
        let spr2 = resolve_fd_sprite(&index2, 4).unwrap();
        assert_eq!(spr2.width, 2);
    }

    #[test]
    fn decode_v2_section_rgba_roundtrip() {
        let rgba = [10u8, 20, 30, 255, 40, 50, 60, 128, 0, 0, 0, 0, 1, 2, 3, 200];
        let entry = build_sprite_section_rgba_entry(9, 0, 2, 2, -1, -2, &rgba);
        let index = index_sprite_section(&entry);
        let spr = resolve_fd_sprite(&index, 9).unwrap();
        assert_eq!(spr.width, 2);
        assert_eq!(spr.height, 2);
        assert_eq!(spr.rgba, rgba);
    }

    #[test]
    fn resolve_fd_prefers_32bpp_over_palette_same_zoom() {
        let indices = [174u8, 0, 0, 174];
        let rgba = [1u8, 2, 3, 255, 4, 5, 6, 255, 7, 8, 9, 255, 10, 11, 12, 255];
        let mut section = build_sprite_section_palette_entry(5, 0, 2, 2, 0, 0, &indices);
        section.extend(build_sprite_section_rgba_entry(5, 0, 2, 2, 0, 0, &rgba));
        let index = index_sprite_section(&section);
        let spr = resolve_fd_sprite(&index, 5).unwrap();
        assert_eq!(&spr.rgba[0..4], &[1, 2, 3, 255]);
    }

    #[test]
    fn collect_train_fd_rgba_sprite_from_sprite_section() {
        let a0 = build_action0_train_payload(1983, 110, 780, "RGBA Loco");
        let mut rgba = vec![0u8; 8 * 8 * 4];
        for y in 2..6 {
            for x in 2..6 {
                let i = (y * 8 + x) * 4;
                rgba[i] = 200;
                rgba[i + 1] = 40;
                rgba[i + 2] = 40;
                rgba[i + 3] = 255;
            }
        }
        let bytes = build_grf_v2_train_with_fd_rgba_sprite(
            &a0,
            0,
            2,
            8,
            8,
            &rgba,
            [b'T', b'R', 0, 1],
            "trgba",
        );
        let gfx = collect_train_sprite_graphics(&bytes).unwrap();
        let preview = gfx.preview_for_local_id(0).unwrap();
        assert_eq!(preview.width, 8);
        assert!(preview.rgba.windows(4).any(|p| p == [200, 40, 40, 255]));
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
    fn parse_action2_variational_default_only() {
        let payload = [0x02, ACTION0_FEATURE_TRAINS, 0x01, 0x81, 0x00];
        assert!(parse_action2_vehicle_basic(&payload, ACTION0_FEATURE_TRAINS).is_none());
        let var = build_action2_trains_variational_default(9, 5);
        let parsed = parse_action2_variational(&var, ACTION0_FEATURE_TRAINS).unwrap();
        assert_eq!(parsed.0, 9);
        assert_eq!(parsed.1.default, 5);
        assert_eq!(parsed.1.ranges, vec![(5, 0, 0xFF)]);
        let basic = build_action2_trains_payload(3, 0, 0);
        assert_eq!(
            parse_action2_vehicle_basic(&basic, ACTION0_FEATURE_TRAINS),
            Some((3, 0))
        );
        assert!(parse_action2_variational(&basic, ACTION0_FEATURE_TRAINS).is_none());
    }

    #[test]
    fn collect_train_variational_chain_follows_default() {
        let a0 = build_action0_train_payload(1976, 130, 920, "Var Loco");
        let mut indices = vec![0u8; 8 * 8];
        for y in 2..6 {
            for x in 2..6 {
                indices[y * 8 + x] = 174;
            }
        }
        let var_id = 9u8;
        let basic_id = 7u8;
        let bytes = build_grf_v2_train_with_variational_chain(
            &a0,
            0,
            var_id,
            basic_id,
            8,
            8,
            &indices,
            [b'T', b'V', 0, 2],
            "tvar",
        );
        let gfx = collect_train_sprite_graphics(&bytes).unwrap();
        assert_eq!(
            gfx.action2_var.get(&var_id).map(|v| v.default),
            Some(u16::from(basic_id))
        );
        assert_eq!(gfx.action2_to_action1.get(&basic_id), Some(&0));
        assert_eq!(gfx.resolve_action1_set(u16::from(var_id)), 0);
        let preview = gfx.preview_for_local_id(0).unwrap();
        assert_eq!(preview.width, 8);
    }

    #[test]
    fn resolve_variational_divide_with_ctx() {
        let payload = build_action2_variational_divmod_payload(
            ACTION0_FEATURE_TRAINS,
            5,
            0x40,
            0,
            0xFF,
            Some(0),
            Some(10),
            None,
            &[(1, 2, 2)], // value 25/10 = 2
            9,
        );
        let (set_id, entry) = parse_action2_variational(&payload, ACTION0_FEATURE_TRAINS).unwrap();
        assert_eq!(set_id, 5);
        assert_eq!(entry.first.adjust.divide_val, Some(10));
        let mut gfx = TrainSpriteGraphics::default();
        gfx.action2_var.insert(5, entry);
        gfx.action2_to_action1.insert(1, 0);
        gfx.action2_to_action1.insert(9, 1);
        let mut ctx = Action2EvalCtx::default();
        ctx.vars.insert(0x40, 25);
        assert_eq!(gfx.resolve_action1_set_ctx(5, &mut ctx), 0);
        ctx.vars.insert(0x40, 5);
        assert_eq!(gfx.resolve_action1_set_ctx(5, &mut ctx), 1); // 5/10=0 → default 9
    }

    #[test]
    fn parse_and_resolve_advanced_variational_add_literal() {
        // var 0x40 (=5) + literal 3 = 8 → rango (1, 8, 8)
        let payload = build_action2_variational_advanced_add_literal(
            ACTION0_FEATURE_TRAINS,
            4,
            0x40,
            0xFF,
            3,
            &[(1, 8, 8)],
            9,
        );
        let (set_id, entry) = parse_action2_variational(&payload, ACTION0_FEATURE_TRAINS).unwrap();
        assert_eq!(set_id, 4);
        assert_eq!(entry.ops.len(), 1);
        assert_eq!(entry.ops[0].operator, 0x00);
        assert_eq!(entry.ops[0].rhs.variable, 0x1A);
        assert_eq!(entry.ops[0].rhs.adjust.and_mask, 3);
        let mut gfx = TrainSpriteGraphics::default();
        gfx.action2_var.insert(4, entry);
        gfx.action2_to_action1.insert(1, 0);
        gfx.action2_to_action1.insert(9, 1);
        let mut ctx = Action2EvalCtx::default();
        ctx.vars.insert(0x40, 5);
        assert_eq!(gfx.resolve_action1_set_ctx(4, &mut ctx), 0);
        ctx.vars.insert(0x40, 0);
        assert_eq!(gfx.resolve_action1_set_ctx(4, &mut ctx), 1); // 0+3=3 → default
        assert!(gfx.needs_runtime_resolve());
    }

    #[test]
    fn parse_and_resolve_random_consist_0x84() {
        let payload = build_action2_trains_random_consist(6, 2, 0, &[20, 21]);
        let (set_id, entry) = parse_action2_random(&payload, ACTION0_FEATURE_TRAINS).unwrap();
        assert_eq!(set_id, 6);
        assert_eq!(entry.typ, 0x84);
        assert_eq!(entry.consist_count, 2);
        let mut gfx = TrainSpriteGraphics::default();
        gfx.action2_random.insert(6, entry);
        gfx.action2_to_action1.insert(20, 0);
        gfx.action2_to_action1.insert(21, 1);
        let mut ctx = Action2EvalCtx::default();
        ctx.consist_random_bits.insert(2, 1);
        assert_eq!(gfx.resolve_action1_set_ctx(6, &mut ctx), 1);
        ctx.consist_random_bits.insert(2, 0);
        assert_eq!(gfx.resolve_action1_set_ctx(6, &mut ctx), 0);
    }

    #[test]
    fn resolve_variational_ranges_with_ctx() {
        let mut gfx = TrainSpriteGraphics::default();
        gfx.action2_var.insert(
            3,
            Action2VarEntry {
                first: Action2VarTerm {
                    variable: 0x40,
                    param: None,
                    adjust: Action2VarAdjust {
                        shift: 0,
                        and_mask: 0xFF,
                        add_val: None,
                        divide_val: None,
                        modulo_val: None,
                    },
                },
                ops: Vec::new(),
                ranges: vec![(7, 1, 1), (8, 2, 5)],
                default: 9,
            },
        );
        gfx.action2_to_action1.insert(7, 0);
        gfx.action2_to_action1.insert(8, 1);
        gfx.action2_to_action1.insert(9, 2);
        let mut ctx = Action2EvalCtx::default();
        assert_eq!(gfx.resolve_action1_set_ctx(3, &mut ctx), 2); // default → 9 → a1 2
        ctx.vars.insert(0x40, 1);
        assert_eq!(gfx.resolve_action1_set_ctx(3, &mut ctx), 0);
        ctx.vars.insert(0x40, 3);
        assert_eq!(gfx.resolve_action1_set_ctx(3, &mut ctx), 1);
    }

    #[test]
    fn resolve_random_action2_with_bits() {
        let mut gfx = TrainSpriteGraphics::default();
        gfx.action2_random.insert(
            4,
            Action2RandomEntry {
                typ: 0x80,
                consist_count: 0,
                triggers: 0,
                randbit: 0,
                sets: vec![10, 11],
            },
        );
        gfx.action2_to_action1.insert(10, 0);
        gfx.action2_to_action1.insert(11, 1);
        let mut ctx = Action2EvalCtx {
            random_bits: 0,
            ..Action2EvalCtx::default()
        };
        assert_eq!(gfx.resolve_action1_set_ctx(4, &mut ctx), 0);
        ctx.random_bits = 1;
        assert_eq!(gfx.resolve_action1_set_ctx(4, &mut ctx), 1);
        assert_eq!(gfx.resolve_action1_set(4), 0); // sin ctx → set[0]
    }

    #[test]
    fn needs_runtime_resolve_for_any_variational() {
        let mut gfx = TrainSpriteGraphics::default();
        assert!(!gfx.needs_runtime_resolve());
        gfx.action2_var.insert(
            1,
            Action2VarEntry {
                first: Action2VarTerm {
                    variable: 0x40,
                    param: None,
                    adjust: Action2VarAdjust {
                        shift: 0,
                        and_mask: 0xFF,
                        add_val: None,
                        divide_val: None,
                        modulo_val: None,
                    },
                },
                ops: Vec::new(),
                ranges: vec![(2, 1, 1)],
                default: 3,
            },
        );
        assert!(gfx.needs_runtime_resolve());
    }

    #[test]
    fn resolve_variational_var40_from_unit_ctx() {
        use crate::train_consist::action2_eval_ctx_for_unit;
        use crate::vehicle::VehicleKind;

        let mut vs = vec![
            Vehicle::new(
                1,
                VehicleKind::Train,
                TileCoord::new(0, 0),
                TileCoord::new(0, 0),
            ),
            Vehicle::new(
                2,
                VehicleKind::Train,
                TileCoord::new(0, 0),
                TileCoord::new(0, 0),
            ),
        ];
        vs[1].engine_id = Some(crate::engine::ENGINE_WAGON_PASSENGER);
        assert!(crate::train_consist::attach_wagon(&mut vs, 1, 2).is_ok());

        let mut gfx = TrainSpriteGraphics::default();
        // shift 0, and FF → ff position; rango set 7 si ff==1
        gfx.action2_var.insert(
            3,
            Action2VarEntry {
                first: Action2VarTerm {
                    variable: 0x40,
                    param: None,
                    adjust: Action2VarAdjust {
                        shift: 0,
                        and_mask: 0xFF,
                        add_val: None,
                        divide_val: None,
                        modulo_val: None,
                    },
                },
                ops: Vec::new(),
                ranges: vec![(7, 1, 1)],
                default: 9,
            },
        );
        gfx.action2_to_action1.insert(7, 0);
        gfx.action2_to_action1.insert(9, 1);

        let mut ctx_head = action2_eval_ctx_for_unit(&vs, 1, crate::tick::GameTick::new(0), &[], 0);
        assert_eq!(gfx.resolve_action1_set_ctx(3, &mut ctx_head), 1); // ff=0 → default

        let mut ctx_wagon =
            action2_eval_ctx_for_unit(&vs, 2, crate::tick::GameTick::new(0), &[], 0);
        assert_eq!(gfx.resolve_action1_set_ctx(3, &mut ctx_wagon), 0); // ff=1 → set 7
    }

    #[test]
    fn resolve_procedure_7e_and_psto() {
        // Procedure set 8: nvar=0 → callback con valor de var 0x40 (=7)
        let mut gfx = TrainSpriteGraphics::default();
        gfx.action2_var.insert(
            8,
            Action2VarEntry {
                first: Action2VarTerm {
                    variable: 0x40,
                    param: None,
                    adjust: Action2VarAdjust {
                        shift: 0,
                        and_mask: 0xFF,
                        add_val: None,
                        divide_val: None,
                        modulo_val: None,
                    },
                },
                ops: Vec::new(),
                ranges: Vec::new(), // nvar=0 → callback
                default: 0,
            },
        );
        // Caller set 3: 7E[8] → si valor==7 elige set 1
        gfx.action2_var.insert(
            3,
            Action2VarEntry {
                first: Action2VarTerm {
                    variable: 0x7E,
                    param: Some(8),
                    adjust: Action2VarAdjust {
                        shift: 0,
                        and_mask: 0xFF,
                        add_val: None,
                        divide_val: None,
                        modulo_val: None,
                    },
                },
                ops: Vec::new(),
                ranges: vec![(1, 7, 7)],
                default: 9,
            },
        );
        gfx.action2_to_action1.insert(1, 0);
        gfx.action2_to_action1.insert(9, 1);
        let mut ctx = Action2EvalCtx::default();
        ctx.vars.insert(0x40, 7);
        assert_eq!(gfx.resolve_action1_set_ctx(3, &mut ctx), 0);
        assert_eq!(ctx.last_result, 7);

        // \2psto: store 5 into persistent[2], then read 7C[2]
        gfx.action2_var.insert(
            4,
            Action2VarEntry {
                first: Action2VarTerm {
                    variable: 0x1A,
                    param: None,
                    adjust: Action2VarAdjust {
                        shift: 0,
                        and_mask: 5,
                        ..Action2VarAdjust::default()
                    },
                },
                ops: vec![
                    Action2VarOp {
                        operator: 0x10, // psto
                        rhs: Action2VarTerm {
                            variable: 0x1A,
                            param: None,
                            adjust: Action2VarAdjust {
                                shift: 0,
                                and_mask: 2, // register index
                                ..Action2VarAdjust::default()
                            },
                        },
                    },
                    Action2VarOp {
                        operator: 0x0F, // rst → start fresh with 7C[2]
                        rhs: Action2VarTerm {
                            variable: 0x7C,
                            param: Some(2),
                            adjust: Action2VarAdjust {
                                shift: 0,
                                and_mask: 0xFF,
                                ..Action2VarAdjust::default()
                            },
                        },
                    },
                ],
                ranges: vec![(1, 5, 5)],
                default: 9,
            },
        );
        let mut ctx2 = Action2EvalCtx::default();
        assert_eq!(gfx.resolve_action1_set_ctx(4, &mut ctx2), 0);
        assert_eq!(ctx2.persistent_registers.get(&2), Some(&5));
    }

    #[test]
    fn decode_v2_chunked_rgba_roundtrip() {
        let rgba = [10u8, 20, 30, 255, 40, 50, 60, 128, 0, 0, 0, 0, 1, 2, 3, 200];
        let entry = build_sprite_section_rgba_chunked_entry(11, 0, 2, 2, -1, -2, &rgba).unwrap();
        let index = index_sprite_section(&entry);
        let spr = resolve_fd_sprite(&index, 11).unwrap();
        assert_eq!(spr.rgba, rgba);
    }

    #[test]
    fn bake_company_mask_remaps_author_ramp() {
        let rgba = vec![128u8, 128, 128, 255, 200, 200, 200, 255];
        let mask = vec![AUTHOR_CC_PALETTE_FIRST, 0];
        let entry = build_sprite_section_rgba_mask_entry(12, 0, 2, 1, 0, 0, &rgba, &mask);
        let index = index_sprite_section(&entry);
        let spr = resolve_fd_sprite(&index, 12).unwrap();
        assert_eq!(spr.mask, mask);
        let baked = bake_sprite_company_mask(&spr, 4); // Red
        // Pixel 0 masked → rampa red; pixel 1 sin máscara.
        assert_ne!(&baked[0..3], &rgba[0..3]);
        assert_eq!(&baked[4..8], &rgba[4..8]);
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
