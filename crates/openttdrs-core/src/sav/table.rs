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
        return Err(SavError::BadFormat("registro truncado".into()));
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
        let value = read_field(field, data, off)?;
        record.push((field.name.clone(), value));
    }
    Ok(record)
}

/// Decodifica el cuerpo gamma completo de un chunk tabla en `(índice, registro)`.
///
/// `sparse` indica `CH_SPARSE_TABLE` (cada registro empieza con su índice gamma).
pub(crate) fn parse_table_chunk(
    body: &[u8],
    sparse: bool,
) -> Result<Vec<(u32, SlRecord)>, SavError> {
    let mut off = 0usize;
    let header_len = gamma(body, &mut off)? as usize;
    if header_len == 0 {
        return Err(SavError::BadFormat("tabla sin header".into()));
    }
    let header_end = off + header_len - 1;
    if header_end > body.len() {
        return Err(SavError::BadFormat("header de tabla truncado".into()));
    }
    let block = &body[off..header_end];
    let mut hoff = 0usize;
    let fields = parse_field_list(block, &mut hoff)?;
    off = header_end;

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
        let record_end = off + n as usize - 1;
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
