//! Decodificación genérica de chunks `CH_TABLE` / `CH_SPARSE_TABLE` (SLV ≥ 295).
//!
//! El header es autodescriptivo: lista de `(tipo, clave)` terminada en tipo 0,
//! con headers de structs anidados en profundidad (depth-first). Cada registro
//! se decodifica a pares `nombre → valor` sin conocer el esquema de antemano.

use crate::tnbp_decode::read_sl_gamma;

use super::SavError;

const SLE_FILE_STRINGID: u8 = 9;
const SLE_FILE_STRING: u8 = 10;
const SLE_FILE_STRUCT: u8 = 11;
const SLE_FILE_HAS_LENGTH: u8 = 0x10;

#[derive(Debug, Clone)]
pub(crate) struct TableField {
    pub(crate) name: String,
    /// `SLE_FILE_*` (nibble bajo).
    pub(crate) base: u8,
    /// Bit 0x10: el campo empieza con un gamma de cantidad en cada registro.
    pub(crate) has_length: bool,
    /// Subcampos cuando `base == SLE_FILE_STRUCT`.
    pub(crate) sub: Vec<TableField>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum SlValue {
    Uint(u64),
    Int(i64),
    Str(String),
    List(Vec<SlValue>),
    Structs(Vec<SlRecord>),
}

pub(crate) type SlRecord = Vec<(String, SlValue)>;

impl SlValue {
    pub(crate) fn as_u64(&self) -> Option<u64> {
        match self {
            Self::Uint(v) => Some(*v),
            Self::Int(v) => u64::try_from(*v).ok(),
            _ => None,
        }
    }

    pub(crate) fn as_i64(&self) -> Option<i64> {
        match self {
            Self::Int(v) => Some(*v),
            Self::Uint(v) => i64::try_from(*v).ok(),
            _ => None,
        }
    }

    pub(crate) fn as_str(&self) -> Option<&str> {
        match self {
            Self::Str(s) => Some(s),
            _ => None,
        }
    }
}

pub(crate) fn record_get<'a>(record: &'a SlRecord, name: &str) -> Option<&'a SlValue> {
    record
        .iter()
        .find_map(|(k, v)| if k == name { Some(v) } else { None })
}

fn gamma(data: &[u8], off: &mut usize) -> Result<u32, SavError> {
    read_sl_gamma(data, off).map_err(|e| SavError::BadFormat(format!("gamma: {e:?}")))
}

fn read_str(data: &[u8], off: &mut usize) -> Result<String, SavError> {
    let len = gamma(data, off)? as usize;
    if *off + len > data.len() {
        return Err(SavError::BadFormat("string truncada".into()));
    }
    let s = String::from_utf8_lossy(&data[*off..*off + len]).into_owned();
    *off += len;
    Ok(s)
}

/// Lista de campos terminada en tipo 0; tras la lista, los headers de los
/// structs encontrados, en orden y en profundidad (formato de savegame).
fn parse_field_list(block: &[u8], off: &mut usize) -> Result<Vec<TableField>, SavError> {
    let mut fields = Vec::new();
    loop {
        let Some(&ftype) = block.get(*off) else {
            return Err(SavError::BadFormat("header de tabla truncado".into()));
        };
        *off += 1;
        if ftype == 0 {
            break;
        }
        let name = read_str(block, off)?;
        fields.push(TableField {
            name,
            base: ftype & 0x0F,
            has_length: ftype & SLE_FILE_HAS_LENGTH != 0,
            sub: Vec::new(),
        });
    }
    for field in &mut fields {
        if field.base == SLE_FILE_STRUCT {
            field.sub = parse_field_list(block, off)?;
        }
    }
    Ok(fields)
}

fn scalar_size(base: u8) -> Result<usize, SavError> {
    Ok(match base {
        1 | 2 => 1,
        3 | 4 | SLE_FILE_STRINGID => 2,
        5 | 6 => 4,
        7 | 8 => 8,
        other => {
            return Err(SavError::BadFormat(format!(
                "tipo de campo no soportado: {other}"
            )));
        }
    })
}

fn read_scalar(base: u8, data: &[u8], off: &mut usize) -> Result<SlValue, SavError> {
    let size = scalar_size(base)?;
    if *off + size > data.len() {
        return Err(SavError::BadFormat(format!(
            "registro truncado (offset {}, tamaño {}, límite {})",
            *off,
            size,
            data.len()
        )));
    }
    let bytes = &data[*off..*off + size];
    *off += size;
    let unsigned = bytes.iter().fold(0u64, |acc, &b| (acc << 8) | u64::from(b));
    Ok(match base {
        // Tipos con signo (BE, complemento a dos del ancho del campo).
        1 | 3 | 5 | 7 => {
            let shift = 64 - size * 8;
            #[allow(clippy::cast_possible_wrap)]
            SlValue::Int(((unsigned << shift) as i64) >> shift)
        }
        _ => SlValue::Uint(unsigned),
    })
}

fn read_field(field: &TableField, data: &[u8], off: &mut usize) -> Result<SlValue, SavError> {
    if field.base == SLE_FILE_STRUCT {
        let count = gamma(data, off)?;
        let mut items = Vec::with_capacity(count as usize);
        for _ in 0..count {
            items.push(read_record_fields(&field.sub, data, off)?);
        }
        return Ok(SlValue::Structs(items));
    }
    if field.base == SLE_FILE_STRING {
        return Ok(SlValue::Str(read_str(data, off)?));
    }
    if field.has_length {
        let count = gamma(data, off)?;
        let mut items = Vec::with_capacity(count as usize);
        for _ in 0..count {
            items.push(read_scalar(field.base, data, off)?);
        }
        return Ok(SlValue::List(items));
    }
    read_scalar(field.base, data, off)
}

fn read_record_fields(
    fields: &[TableField],
    data: &[u8],
    off: &mut usize,
) -> Result<SlRecord, SavError> {
    let mut record = Vec::with_capacity(fields.len());
    for field in fields {
        let value = read_field(field, data, off)
            .map_err(|error| SavError::BadFormat(format!("campo {}: {error}", field.name)))?;
        record.push((field.name.clone(), value));
    }
    Ok(record)
}

/// Lee la cabecera de una tabla y devuelve el rango de bytes de sus campos.
///
/// El escritor usa esta vista para fusionar columnas que todavía no tienen un
/// modelo semántico: los bytes de esas columnas se conservan y sólo se
/// reemplazan los campos conocidos. `header_end` apunta al primer byte de la
/// primera fila (el rango de cabecera excluye el gamma de longitud).
pub(crate) fn parse_table_layout(body: &[u8]) -> Result<(usize, usize, Vec<TableField>), SavError> {
    let mut off = 0usize;
    let header_len = gamma(body, &mut off)? as usize;
    if header_len == 0 {
        return Err(SavError::BadFormat("tabla sin header".into()));
    }
    let header_start = off;
    let header_end = header_start
        .checked_add(header_len.saturating_sub(1))
        .ok_or_else(|| SavError::BadFormat("header de tabla overflow".into()))?;
    if header_end > body.len() {
        return Err(SavError::BadFormat("header de tabla truncado".into()));
    }
    let mut header_off = 0usize;
    let fields = parse_field_list(&body[header_start..header_end], &mut header_off)?;
    Ok((header_start, header_end, fields))
}

fn skip_field(field: &TableField, data: &[u8], off: &mut usize) -> Result<(), SavError> {
    if field.base == SLE_FILE_STRUCT {
        let count = gamma(data, off)?;
        for _ in 0..count {
            skip_record_fields(&field.sub, data, off)?;
        }
        return Ok(());
    }
    if field.base == SLE_FILE_STRING {
        let len = gamma(data, off)? as usize;
        *off = off
            .checked_add(len)
            .ok_or_else(|| SavError::BadFormat("string de tabla overflow".into()))?;
        if *off > data.len() {
            return Err(SavError::BadFormat("string de tabla truncada".into()));
        }
        return Ok(());
    }
    let count = if field.has_length {
        gamma(data, off)? as usize
    } else {
        1
    };
    let size = scalar_size(field.base)?;
    let bytes = size
        .checked_mul(count)
        .ok_or_else(|| SavError::BadFormat("campo de tabla overflow".into()))?;
    *off = off
        .checked_add(bytes)
        .ok_or_else(|| SavError::BadFormat("campo de tabla overflow".into()))?;
    if *off > data.len() {
        return Err(SavError::BadFormat("campo de tabla truncado".into()));
    }
    Ok(())
}

pub(crate) fn skip_record_fields(
    fields: &[TableField],
    data: &[u8],
    off: &mut usize,
) -> Result<(), SavError> {
    for field in fields {
        skip_field(field, data, off)?;
    }
    Ok(())
}

/// Rangos `(nombre, inicio, fin)` de los campos raíz de un registro.
///
/// Los offsets son relativos al payload de la fila (sin el índice gamma de
/// una tabla sparse). Los campos de longitud variable también se incluyen,
/// para que el escritor preserve columnas ajenas al reconstruir una fila.
pub(crate) fn field_byte_ranges(
    fields: &[TableField],
    record: &[u8],
) -> Result<Vec<(String, usize, usize)>, SavError> {
    let mut off = 0usize;
    let mut ranges = Vec::with_capacity(fields.len());
    for field in fields {
        let start = off;
        skip_field(field, record, &mut off)?;
        ranges.push((field.name.clone(), start, off));
    }
    Ok(ranges)
}

/// Decodifica el cuerpo gamma completo de un chunk tabla en `(índice, registro)`.
///
/// `sparse` indica `CH_SPARSE_TABLE` (cada registro empieza con su índice gamma).
pub(crate) fn parse_table_chunk(
    body: &[u8],
    sparse: bool,
) -> Result<Vec<(u32, SlRecord)>, SavError> {
    let (_, header_end, fields) = parse_table_layout(body)?;
    let mut off = header_end;

    let mut out = Vec::new();
    let mut auto_index = 0u32;
    loop {
        if off >= body.len() {
            break;
        }
        let n = gamma(body, &mut off)?;
        if n == 0 {
            break;
        }
        let record_len = n as usize - 1;
        // `SlIterateArray()` de OpenTTD avanza el índice de una tabla densa
        // aunque el slot esté vacío (`length == 1`, payload de cero bytes).
        // Pools grandes como `INDY` y `CAPA` de Kale contienen muchos de
        // estos huecos; tratarlos como un registro hace que el parser aborte
        // al intentar leer el primer campo desde un slice vacío.
        if record_len == 0 {
            if !sparse {
                auto_index += 1;
            }
            continue;
        }
        let record_end = off + record_len;
        if record_end > body.len() {
            return Err(SavError::BadFormat("registro de tabla truncado".into()));
        }
        let index = if sparse {
            gamma(body, &mut off)?
        } else {
            auto_index
        };
        let record = read_record_fields(&fields, &body[..record_end], &mut off)?;
        // Campos de versiones más nuevas que no modelamos: saltar al final del registro.
        off = record_end;
        out.push((index, record));
        auto_index += 1;
    }
    Ok(out)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::cast_possible_truncation)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn write_gamma(v: u32, buf: &mut Vec<u8>) {
        assert!(v < (1 << 14));
        if v < (1 << 7) {
            buf.push(v as u8);
        } else {
            buf.push(0x80 | ((v >> 8) as u8));
            buf.push((v & 0xFF) as u8);
        }
    }

    pub(crate) fn write_str(s: &str, buf: &mut Vec<u8>) {
        write_gamma(s.len() as u32, buf);
        buf.extend_from_slice(s.as_bytes());
    }

    /// Cuerpo de tabla: header (campos planos) + registros + terminador.
    pub(crate) fn build_table_body(header_fields: &[(u8, &str)], records: &[Vec<u8>]) -> Vec<u8> {
        let mut header = Vec::new();
        for (ftype, name) in header_fields {
            header.push(*ftype);
            write_str(name, &mut header);
        }
        header.push(0);

        let mut body = Vec::new();
        write_gamma(header.len() as u32 + 1, &mut body);
        body.extend_from_slice(&header);
        for r in records {
            write_gamma(r.len() as u32 + 1, &mut body);
            body.extend_from_slice(r);
        }
        write_gamma(0, &mut body);
        body
    }

    #[test]
    fn parses_flat_table_records() {
        let mut rec = Vec::new();
        rec.extend_from_slice(&123u32.to_be_bytes()); // xy U32
        write_str("Central", &mut rec); // name str
        rec.push(7); // facilities U8

        let body = build_table_body(
            &[(6, "xy"), (0x0A | 0x10, "name"), (2, "facilities")],
            &[rec],
        );
        let rows = parse_table_chunk(&body, false).expect("parse");
        assert_eq!(rows.len(), 1);
        let (idx, record) = &rows[0];
        assert_eq!(*idx, 0);
        assert_eq!(
            record_get(record, "xy").and_then(SlValue::as_u64),
            Some(123)
        );
        assert_eq!(
            record_get(record, "name").and_then(|v| v.as_str()),
            Some("Central")
        );
        assert_eq!(
            record_get(record, "facilities").and_then(SlValue::as_u64),
            Some(7)
        );
    }

    #[test]
    fn dense_table_skips_empty_pool_slots_and_keeps_index() {
        // `SlIterateArray` representa un slot de pool vacío como `length = 1`.
        // El siguiente registro denso debe conservar el índice 1, no abortar
        // intentando leer el campo `v` desde un payload vacío.
        let body = build_table_body(&[(2, "v")], &[Vec::new(), vec![9]]);

        let rows = parse_table_chunk(&body, false).expect("parse table with empty slot");

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].0, 1);
        assert_eq!(
            record_get(&rows[0].1, "v").and_then(SlValue::as_u64),
            Some(9)
        );
    }

    #[test]
    fn parses_nested_struct_and_list_fields() {
        // Header: u8 counter, struct goods (con u16 amount), lista u8 flags.
        let mut header = Vec::new();
        header.push(2); // U8 counter
        write_str("counter", &mut header);
        header.push(0x0B | 0x10); // struct goods
        write_str("goods", &mut header);
        header.push(2 | 0x10); // lista de U8
        write_str("flags", &mut header);
        header.push(0);
        // Sub-header de goods (depth-first tras la lista raíz).
        header.push(4); // U16 amount
        write_str("amount", &mut header);
        header.push(0);

        let mut rec = Vec::new();
        rec.push(9); // counter
        write_gamma(2, &mut rec); // goods ×2
        rec.extend_from_slice(&100u16.to_be_bytes());
        rec.extend_from_slice(&200u16.to_be_bytes());
        write_gamma(3, &mut rec); // flags ×3
        rec.extend_from_slice(&[1, 2, 3]);

        let mut body = Vec::new();
        write_gamma(header.len() as u32 + 1, &mut body);
        body.extend_from_slice(&header);
        write_gamma(rec.len() as u32 + 1, &mut body);
        body.extend_from_slice(&rec);
        write_gamma(0, &mut body);

        let rows = parse_table_chunk(&body, false).expect("parse");
        assert_eq!(rows.len(), 1);
        let record = &rows[0].1;
        assert_eq!(
            record_get(record, "counter").and_then(SlValue::as_u64),
            Some(9)
        );
        let SlValue::Structs(goods) = record_get(record, "goods").expect("goods") else {
            panic!("goods debería ser struct list");
        };
        assert_eq!(goods.len(), 2);
        assert_eq!(
            record_get(&goods[1], "amount").and_then(SlValue::as_u64),
            Some(200)
        );
        let SlValue::List(flags) = record_get(record, "flags").expect("flags") else {
            panic!("flags debería ser lista");
        };
        assert_eq!(flags.len(), 3);
    }

    #[test]
    fn sparse_table_reads_explicit_index() {
        let mut rec = Vec::new();
        write_gamma(42, &mut rec); // índice sparse
        rec.push(5); // valor U8

        let body = build_table_body(&[(2, "v")], &[rec]);
        let rows = parse_table_chunk(&body, true).expect("parse");
        assert_eq!(rows[0].0, 42);
        assert_eq!(
            record_get(&rows[0].1, "v").and_then(SlValue::as_u64),
            Some(5)
        );
    }

    #[test]
    fn signed_scalars_sign_extend() {
        let mut rec = Vec::new();
        rec.extend_from_slice(&(-5i16).to_be_bytes());
        let body = build_table_body(&[(3, "v")], &[rec]);
        let rows = parse_table_chunk(&body, false).expect("parse");
        assert_eq!(record_get(&rows[0].1, "v"), Some(&SlValue::Int(-5)));
    }
}
