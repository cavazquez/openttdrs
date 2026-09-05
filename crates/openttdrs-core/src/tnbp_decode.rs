//! Decodificación best-effort del footer **TNBP** (blob del pool túnel/puente del save).
//!
//! `OpenTTD` **vanilla** actual no define un chunk `TNBP` en el código público; el blob suele
//! venir de forks (p. ej. JGRPP: chunk `TUNN` como `CH_TABLE` con cabecera `SlTable`).
//! `parse_sav.py` copia el payload crudo al footer `.ottdmap`; aquí se interpreta el stream
//! como **CH\_TABLE** / **CH\_ARRAY** de saveload (gamma + segmentos), alineado con
//! `SlReadSimpleGamma` y tipos de archivo `SLE_FILE_*` (nibble bajo).

/// Error al interpretar bytes TNBP.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TnbpDecodeError {
    Truncado,
    GammaNoSoportado,
    CabeceraTablaInvalida,
    FilaTruncada,
    TipoCampoNoSoportado(u8),
}

/// Valor leído de una celda de fila (tipos de archivo `OpenTTD` básicos).
#[derive(Debug, Clone, PartialEq)]
pub enum SlPrimitive {
    I8(i8),
    U8(u8),
    I16(i16),
    U16(u16),
    I32(i32),
    U32(u32),
    /// `SLE_FILE_STRINGID` (u16 BE en save).
    StringId(u16),
    /// `SLE_FILE_STRING` (gamma length + bytes UTF-8 en save).
    Str(String),
}

/// Campo de cabecera `SlTable`: nombre + tipo de archivo (`GetVarFileType`, nibble bajo).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlTableField {
    pub name: String,
    pub file_type: u8,
}

/// Registro de túnel al estilo JGR (`tunnel_sl.cpp`: `tile_n`, `tile_s`, `height`, `is_chunnel`, …).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct JgrTunnelRecord {
    pub tile_n: u32,
    pub tile_s: u32,
    pub height: u8,
    pub is_chunnel: bool,
    pub style_n: Option<u8>,
    pub style_s: Option<u8>,
}

/// Resultado de [`decode_tnbp_blob`].
#[derive(Debug, Clone, PartialEq)]
pub enum TnbpDecoded {
    /// Chunk estilo `CH_TABLE`: cabecera + filas.
    ChTable {
        fields: Vec<SlTableField>,
        rows: Vec<Vec<(String, SlPrimitive)>>,
        /// Filas del blob que no encajaron con la cabecera (versión distinta / campos extra).
        skipped_rows: usize,
    },
    /// No reconocido como tabla: segmentos gamma (como `skip_array` / `slurp_array_payload`).
    RawGammaSegments { segments: Vec<Vec<u8>> },
}

const SLE_FILE_END: u8 = 0;
const SLE_FILE_I8: u8 = 1;
const SLE_FILE_U8: u8 = 2;
const SLE_FILE_I16: u8 = 3;
const SLE_FILE_U16: u8 = 4;
const SLE_FILE_I32: u8 = 5;
const SLE_FILE_U32: u8 = 6;
const SLE_FILE_STRINGID: u8 = 9;
const SLE_FILE_STRING: u8 = 10;

/// Lee gamma al estilo `SlReadSimpleGamma` / `read_gamma` de `parse_sav.py`.
pub fn read_sl_gamma(data: &[u8], off: &mut usize) -> Result<u32, TnbpDecodeError> {
    let b0 = *data.get(*off).ok_or(TnbpDecodeError::Truncado)?;
    *off += 1;
    if b0 & 0x80 == 0 {
        return Ok(u32::from(b0));
    }
    let mut b = b0 & !0x80;
    if b & 0x40 == 0 {
        let lo = *data.get(*off).ok_or(TnbpDecodeError::Truncado)?;
        *off += 1;
        return Ok((u32::from(b) << 8) | u32::from(lo));
    }
    b &= !0x40;
    if b & 0x20 == 0 {
        let lo = read_u16_be(data, off)?;
        return Ok((u32::from(b) << 16) | u32::from(lo));
    }
    b &= !0x20;
    if b & 0x10 == 0 {
        let b1 = *data.get(*off).ok_or(TnbpDecodeError::Truncado)?;
        *off += 1;
        let b2 = *data.get(*off).ok_or(TnbpDecodeError::Truncado)?;
        *off += 1;
        let b3 = *data.get(*off).ok_or(TnbpDecodeError::Truncado)?;
        *off += 1;
        return Ok((u32::from(b) << 24)
            | (u32::from(b1) << 16)
            | (u32::from(b2) << 8)
            | u32::from(b3));
    }
    b &= !0x10;
    if b & 0x08 != 0 {
        return Err(TnbpDecodeError::GammaNoSoportado);
    }
    // 11110--- es sólo el prefijo: SlReadSimpleGamma descarta sus tres
    // bits bajos y consume los cuatro bytes siguientes como valor de 32 bits.
    read_u32_be(data, off)
}

fn read_u16_be(data: &[u8], off: &mut usize) -> Result<u16, TnbpDecodeError> {
    if *off + 2 > data.len() {
        return Err(TnbpDecodeError::Truncado);
    }
    let v = u16::from_be_bytes([data[*off], data[*off + 1]]);
    *off += 2;
    Ok(v)
}

fn read_u32_be(data: &[u8], off: &mut usize) -> Result<u32, TnbpDecodeError> {
    let hi = read_u16_be(data, off)?;
    let lo = read_u16_be(data, off)?;
    Ok(u32::from(hi) << 16 | u32::from(lo))
}

fn read_sl_string(data: &[u8], off: &mut usize) -> Result<String, TnbpDecodeError> {
    let len = usize::try_from(read_sl_gamma(data, off)?).map_err(|_| TnbpDecodeError::Truncado)?;
    if *off + len > data.len() {
        return Err(TnbpDecodeError::Truncado);
    }
    let s = std::str::from_utf8(&data[*off..*off + len])
        .map_err(|_| TnbpDecodeError::CabeceraTablaInvalida)?
        .to_string();
    *off += len;
    Ok(s)
}

/// Parte el payload interno de un chunk `CH_*` en segmentos `[gamma_len → payload de len-1]` hasta gamma 0.
pub fn split_sl_gamma_segments(blob: &[u8]) -> Result<Vec<&[u8]>, TnbpDecodeError> {
    let mut out = Vec::new();
    let mut off = 0usize;
    loop {
        let g = read_sl_gamma(blob, &mut off)?;
        if g == 0 {
            break;
        }
        let plen = usize::try_from(g.saturating_sub(1)).map_err(|_| TnbpDecodeError::Truncado)?;
        if off + plen > blob.len() {
            return Err(TnbpDecodeError::Truncado);
        }
        out.push(&blob[off..off + plen]);
        off += plen;
    }
    // Si quedan bytes tras el terminador gamma, se ignoran (best-effort).
    Ok(out)
}

fn parse_sl_table_header(hdr: &[u8]) -> Result<Vec<SlTableField>, TnbpDecodeError> {
    let mut off = 0usize;
    let mut fields = Vec::new();
    loop {
        let ftype = *hdr.get(off).ok_or(TnbpDecodeError::Truncado)?;
        off += 1;
        if ftype == SLE_FILE_END {
            break;
        }
        let name = read_sl_string(hdr, &mut off)?;
        fields.push(SlTableField {
            name,
            file_type: ftype & 0x0F,
        });
    }
    if off != hdr.len() {
        return Err(TnbpDecodeError::CabeceraTablaInvalida);
    }
    Ok(fields)
}

fn read_primitive(
    file_type: u8,
    row: &[u8],
    off: &mut usize,
) -> Result<SlPrimitive, TnbpDecodeError> {
    match file_type {
        SLE_FILE_I8 => {
            let v = *row.get(*off).ok_or(TnbpDecodeError::FilaTruncada)?;
            *off += 1;
            Ok(SlPrimitive::I8(i8::from_le_bytes([v])))
        }
        SLE_FILE_U8 => {
            let v = *row.get(*off).ok_or(TnbpDecodeError::FilaTruncada)?;
            *off += 1;
            Ok(SlPrimitive::U8(v))
        }
        SLE_FILE_I16 => {
            let raw = read_u16_be(row, off)?;
            Ok(SlPrimitive::I16(i16::from_ne_bytes(raw.to_ne_bytes())))
        }
        SLE_FILE_U16 => Ok(SlPrimitive::U16(read_u16_be(row, off)?)),
        SLE_FILE_I32 => {
            let raw = read_u32_be(row, off)?;
            Ok(SlPrimitive::I32(i32::from_ne_bytes(raw.to_ne_bytes())))
        }
        SLE_FILE_U32 => Ok(SlPrimitive::U32(read_u32_be(row, off)?)),
        SLE_FILE_STRINGID => Ok(SlPrimitive::StringId(read_u16_be(row, off)?)),
        SLE_FILE_STRING => {
            let s = read_sl_string(row, off)?;
            Ok(SlPrimitive::Str(s))
        }
        x => Err(TnbpDecodeError::TipoCampoNoSoportado(x)),
    }
}

fn parse_row(
    fields: &[SlTableField],
    payload: &[u8],
) -> Result<Vec<(String, SlPrimitive)>, TnbpDecodeError> {
    let mut off = 0usize;
    let mut cells = Vec::with_capacity(fields.len());
    for f in fields {
        let p = read_primitive(f.file_type, payload, &mut off)?;
        cells.push((f.name.clone(), p));
    }
    if off != payload.len() {
        // Filas con padding o versión distinta: best-effort si leímos al menos un campo.
        if cells.is_empty() {
            return Err(TnbpDecodeError::FilaTruncada);
        }
    }
    Ok(cells)
}

/// Interpreta el blob TNBP: intenta `CH_TABLE` (primer segmento = cabecera Sl); si falla, devuelve segmentos gamma crudos.
pub fn decode_tnbp_blob(blob: &[u8]) -> Result<TnbpDecoded, TnbpDecodeError> {
    let segments = split_sl_gamma_segments(blob)?;
    if segments.len() < 2 {
        return Ok(TnbpDecoded::RawGammaSegments {
            segments: segments.iter().map(|s| s.to_vec()).collect(),
        });
    }
    let header = parse_sl_table_header(segments[0]);
    let fields = match header {
        Ok(f) if !f.is_empty() => f,
        _ => {
            return Ok(TnbpDecoded::RawGammaSegments {
                segments: segments.iter().map(|s| s.to_vec()).collect(),
            });
        }
    };
    let mut rows = Vec::new();
    let mut skipped_rows = 0usize;
    for seg in segments.iter().skip(1) {
        match parse_row(&fields, seg) {
            Ok(r) => rows.push(r),
            Err(_) => {
                skipped_rows += 1;
            }
        }
    }
    if rows.is_empty() {
        return Ok(TnbpDecoded::RawGammaSegments {
            segments: segments.iter().map(|s| s.to_vec()).collect(),
        });
    }
    Ok(TnbpDecoded::ChTable {
        fields,
        rows,
        skipped_rows,
    })
}

fn u32_from_primitive(p: &SlPrimitive) -> Option<u32> {
    match p {
        SlPrimitive::U32(v) => Some(*v),
        SlPrimitive::I32(v) => u32::try_from(*v).ok(),
        _ => None,
    }
}

fn u8_from_primitive(p: &SlPrimitive) -> Option<u8> {
    match p {
        SlPrimitive::U8(v) => Some(*v),
        SlPrimitive::I8(v) => u8::try_from(*v).ok(),
        _ => None,
    }
}

fn bool_from_primitive(p: &SlPrimitive) -> Option<bool> {
    match p {
        SlPrimitive::I8(v) => Some(*v != 0),
        SlPrimitive::U8(v) => Some(*v != 0),
        _ => None,
    }
}

/// Si el decode fue tabla con campos `tile_n` / `tile_s` (JGR `Tunnel`), construye registros.
#[must_use]
pub fn jgr_tunnels_from_decoded(decoded: &TnbpDecoded) -> Vec<JgrTunnelRecord> {
    let TnbpDecoded::ChTable { rows, .. } = decoded else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for row in rows {
        let mut m: std::collections::HashMap<&str, &SlPrimitive> =
            std::collections::HashMap::with_capacity(row.len());
        for (k, v) in row {
            m.insert(k.as_str(), v);
        }
        let Some(tile_n) = m.get("tile_n").and_then(|p| u32_from_primitive(p)) else {
            continue;
        };
        let Some(tile_s) = m.get("tile_s").and_then(|p| u32_from_primitive(p)) else {
            continue;
        };
        let Some(height) = m.get("height").and_then(|p| u8_from_primitive(p)) else {
            continue;
        };
        let is_chunnel = m
            .get("is_chunnel")
            .and_then(|p| bool_from_primitive(p))
            .unwrap_or(false);
        let style_n = m.get("style_n").and_then(|p| u8_from_primitive(p));
        let style_s = m.get("style_s").and_then(|p| u8_from_primitive(p));
        out.push(JgrTunnelRecord {
            tile_n,
            tile_s,
            height,
            is_chunnel,
            style_n,
            style_s,
        });
    }
    out
}

/// Resumen JSON del blob TNBP (para herramientas / `GameState::save_json` enriquecido).
#[must_use]
pub fn tnbp_blob_to_json_value(blob: &[u8]) -> serde_json::Value {
    match decode_tnbp_blob(blob) {
        Err(e) => serde_json::json!({
            "ok": false,
            "error": format!("{e:?}"),
        }),
        Ok(dec) => {
            let jgr = jgr_tunnels_from_decoded(&dec);
            match dec {
                TnbpDecoded::ChTable {
                    fields,
                    rows,
                    skipped_rows,
                } => serde_json::json!({
                    "ok": true,
                    "kind": "ch_table",
                    "field_names": fields.iter().map(|f| &f.name).collect::<Vec<_>>(),
                    "row_count": rows.len(),
                    "skipped_rows": skipped_rows,
                    "jgr_tunnel_count": jgr.len(),
                    "jgr_tunnels": jgr,
                }),
                TnbpDecoded::RawGammaSegments { segments } => serde_json::json!({
                    "ok": true,
                    "kind": "raw_gamma_segments",
                    "segment_count": segments.len(),
                    "jgr_tunnel_count": jgr.len(),
                    "jgr_tunnels": jgr,
                }),
            }
        }
    }
}

#[cfg(test)]
#[allow(
    clippy::cast_possible_truncation,
    clippy::expect_used,
    clippy::match_wildcard_for_single_variants
)]
mod tests {
    use super::*;

    fn write_gamma(v: u32, buf: &mut Vec<u8>) {
        if v < (1 << 7) {
            buf.push(v as u8);
        } else if v < (1 << 14) {
            buf.push(0x80 | ((v >> 8) as u8));
            buf.push((v & 0xFF) as u8);
        } else {
            panic!("test solo usa gammas pequeños");
        }
    }

    fn write_string(s: &str, buf: &mut Vec<u8>) {
        write_gamma(s.len() as u32, buf);
        buf.extend_from_slice(s.as_bytes());
    }

    fn write_u32_be(v: u32, buf: &mut Vec<u8>) {
        buf.extend_from_slice(&v.to_be_bytes());
    }

    #[test]
    fn decode_minimal_jgr_style_tunnel_table() {
        let mut inner = Vec::new();
        // Cabecera SlTable
        let mut hdr = Vec::new();
        hdr.push(SLE_FILE_U32);
        write_string("tile_n", &mut hdr);
        hdr.push(SLE_FILE_U32);
        write_string("tile_s", &mut hdr);
        hdr.push(SLE_FILE_U8);
        write_string("height", &mut hdr);
        hdr.push(SLE_FILE_I8);
        write_string("is_chunnel", &mut hdr);
        hdr.push(SLE_FILE_END);
        // Segmento 0: cabecera
        write_gamma((hdr.len() + 1) as u32, &mut inner);
        inner.extend_from_slice(&hdr);
        // Fila 1
        let mut row = Vec::new();
        write_u32_be(100, &mut row);
        write_u32_be(200, &mut row);
        row.push(5u8);
        row.push(0i8 as u8);
        write_gamma((row.len() + 1) as u32, &mut inner);
        inner.extend_from_slice(&row);
        write_gamma(0, &mut inner);

        let dec = decode_tnbp_blob(&inner).expect("decode");
        match &dec {
            TnbpDecoded::ChTable {
                fields,
                rows,
                skipped_rows,
            } => {
                assert_eq!(fields.len(), 4);
                assert_eq!(rows.len(), 1);
                assert_eq!(*skipped_rows, 0);
            }
            _ => panic!("expected ChTable"),
        }
        let tunnels = jgr_tunnels_from_decoded(&dec);
        assert_eq!(tunnels.len(), 1);
        assert_eq!(tunnels[0].tile_n, 100);
        assert_eq!(tunnels[0].tile_s, 200);
        assert_eq!(tunnels[0].height, 5);
        assert!(!tunnels[0].is_chunnel);
    }

    #[test]
    fn raw_segments_when_not_table() {
        let mut inner = Vec::new();
        write_gamma(4, &mut inner); // len 3 payload
        inner.extend_from_slice(&[1, 2, 3]);
        write_gamma(0, &mut inner);
        let dec = decode_tnbp_blob(&inner).expect("decode");
        match dec {
            TnbpDecoded::RawGammaSegments { segments } => {
                assert_eq!(segments.len(), 1);
                assert_eq!(segments[0], vec![1u8, 2, 3]);
            }
            _ => panic!("expected raw"),
        }
    }

    #[test]
    fn decode_row_with_string_field() {
        let mut inner = Vec::new();
        let mut hdr = Vec::new();
        hdr.push(SLE_FILE_U32);
        write_string("id", &mut hdr);
        hdr.push(SLE_FILE_STRING);
        write_string("label", &mut hdr);
        hdr.push(SLE_FILE_END);
        write_gamma((hdr.len() + 1) as u32, &mut inner);
        inner.extend_from_slice(&hdr);
        let mut row = Vec::new();
        write_u32_be(7, &mut row);
        write_string("hi", &mut row);
        write_gamma((row.len() + 1) as u32, &mut inner);
        inner.extend_from_slice(&row);
        write_gamma(0, &mut inner);
        let dec = decode_tnbp_blob(&inner).expect("decode");
        let TnbpDecoded::ChTable { rows, .. } = dec else {
            panic!("ch_table");
        };
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].len(), 2);
        assert_eq!(rows[0][1].1, SlPrimitive::Str("hi".to_string()));
    }
}
