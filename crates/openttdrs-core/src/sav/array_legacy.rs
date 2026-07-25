//! Chunks `CH_ARRAY` / `CH_SPARSE_ARRAY` (SLV < ~295): registros binarios fijos
//! Layouts tomados de los `*_sl_compat.h` de `OpenTTD`.

use crate::tnbp_decode::read_sl_gamma;

use super::SavError;
use super::chunks::{CH_ARRAY, CH_RIFF, CH_SPARSE_ARRAY, CH_SPARSE_TABLE, CH_TABLE, RawChunk};
use super::table::{SlRecord, SlValue, parse_table_chunk};

/// SLV con tablas autodescriptivas (`CH_TABLE`) en la mayoría de entidades.
const SLV_TABLE_CHUNKS: u16 = 295;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScalarKind {
    U8,
    U16,
    U32,
    I64,
}

fn gamma(data: &[u8], off: &mut usize) -> Result<u32, SavError> {
    read_sl_gamma(data, off).map_err(|e| SavError::BadFormat(format!("gamma: {e:?}")))
}

fn read_string(data: &[u8], off: &mut usize) -> Result<String, SavError> {
    let len = gamma(data, off)? as usize;
    if *off + len > data.len() {
        return Err(SavError::BadFormat("string truncada".into()));
    }
    let s = String::from_utf8_lossy(&data[*off..*off + len]).into_owned();
    *off += len;
    Ok(s)
}

fn skip_scalar(kind: ScalarKind, data: &[u8], off: &mut usize) -> Result<(), SavError> {
    match kind {
        ScalarKind::U8 => *off += 1,
        ScalarKind::U16 => *off += 2,
        ScalarKind::U32 => *off += 4,
        ScalarKind::I64 => *off += 8,
    }
    if *off > data.len() {
        return Err(SavError::BadFormat("registro legacy truncado".into()));
    }
    Ok(())
}

fn read_u8(data: &[u8], off: &mut usize) -> Result<u8, SavError> {
    let v = *data
        .get(*off)
        .ok_or_else(|| SavError::BadFormat("u8 truncado".into()))?;
    *off += 1;
    Ok(v)
}

fn read_u16_be(data: &[u8], off: &mut usize) -> Result<u16, SavError> {
    if *off + 2 > data.len() {
        return Err(SavError::BadFormat("u16 truncado".into()));
    }
    let v = u16::from_be_bytes([data[*off], data[*off + 1]]);
    *off += 2;
    Ok(v)
}

fn read_u32_be(data: &[u8], off: &mut usize) -> Result<u32, SavError> {
    if *off + 4 > data.len() {
        return Err(SavError::BadFormat("u32 truncado".into()));
    }
    let bytes: [u8; 4] = data[*off..*off + 4]
        .try_into()
        .map_err(|_| SavError::BadFormat("u32 truncado".into()))?;
    let v = u32::from_be_bytes(bytes);
    *off += 4;
    Ok(v)
}

fn read_i32_be(data: &[u8], off: &mut usize) -> Result<i32, SavError> {
    let raw = read_u32_be(data, off)?;
    Ok(i32::try_from(raw).unwrap_or(i32::MAX))
}

fn read_i64_be(data: &[u8], off: &mut usize) -> Result<i64, SavError> {
    if *off + 8 > data.len() {
        return Err(SavError::BadFormat("i64 truncado".into()));
    }
    let bytes: [u8; 8] = data[*off..*off + 8]
        .try_into()
        .map_err(|_| SavError::BadFormat("i64 truncado".into()))?;
    let v = i64::from_be_bytes(bytes);
    *off += 8;
    Ok(v)
}

/// Registros `(índice, payload)` de un stream gamma (`CH_ARRAY` / `CH_SPARSE_ARRAY`).
#[must_use]
pub(crate) fn gamma_records(body: &[u8], sparse: bool) -> Vec<(u32, Vec<u8>)> {
    let mut out = Vec::new();
    let mut off = 0usize;
    let mut auto_index = 0u32;
    while off < body.len() {
        let Ok(n) = gamma(body, &mut off) else {
            break;
        };
        if n == 0 {
            break;
        }
        let record_end = off + n as usize - 1;
        if record_end > body.len() {
            break;
        }
        let index = if sparse {
            gamma(body, &mut off).unwrap_or(auto_index)
        } else {
            auto_index
        };
        let rec = body[off..record_end].to_vec();
        off = record_end;
        out.push((index, rec));
        auto_index += 1;
    }
    out
}

fn record(fields: &[(&str, SlValue)]) -> SlRecord {
    fields
        .iter()
        .map(|(k, v)| (k.to_string(), v.clone()))
        .collect()
}

/// STNN v211: `SlObject` directo — `facilities`, `SlStationBase` (xy, town, name…).
fn stnn_record(rec: &[u8]) -> Option<SlRecord> {
    if rec.len() < 11 {
        return None;
    }
    let facilities = rec[0];
    let xy = u32::from_be_bytes(rec[1..5].try_into().ok()?);
    let mut off = 5 + 4 + 2; // town u32 + string_id u16
    let name = read_string(rec, &mut off).ok()?;
    Some(record(&[
        ("xy", SlValue::Uint(u64::from(xy))),
        ("facilities", SlValue::Uint(u64::from(facilities))),
        (
            "name",
            SlValue::Str(if name.is_empty() { String::new() } else { name }),
        ),
    ]))
}

/// ORDR v211: `_order_sl_compat`.
fn ordr_record(rec: &[u8]) -> Option<SlRecord> {
    let mut off = 0usize;
    let order_type = read_u8(rec, &mut off).ok()?;
    let flags = read_u8(rec, &mut off).ok()?;
    let dest = read_u16_be(rec, &mut off).ok()?;
    let next = read_u32_be(rec, &mut off).ok()?;
    Some(record(&[
        ("type", SlValue::Uint(u64::from(order_type))),
        ("flags", SlValue::Uint(u64::from(flags))),
        ("dest", SlValue::Uint(u64::from(dest))),
        ("next", SlValue::Uint(u64::from(next))),
    ]))
}

/// ORDL v211: solo campo `first` (u32).
fn ordl_record(rec: &[u8]) -> Option<SlRecord> {
    if rec.len() < 4 {
        return None;
    }
    let first = u32::from_be_bytes(rec[0..4].try_into().ok()?);
    Some(record(&[("first", SlValue::Uint(u64::from(first)))]))
}

/// CITY v211: xy u32 + nombre (string).
fn city_record(rec: &[u8]) -> Option<SlRecord> {
    let mut off = 0usize;
    let xy = read_u32_be(rec, &mut off).ok()?;
    let name = read_string(rec, &mut off).ok()?;
    Some(record(&[
        ("xy", SlValue::Uint(u64::from(xy))),
        ("name", SlValue::Str(name)),
    ]))
}

/// PLYR v211: `_company_sl_compat` hasta `money` y `colour`.
fn plyr_record(rec: &[u8]) -> Option<SlRecord> {
    let mut off = 0usize;
    for _ in 0..3 {
        read_string(rec, &mut off).ok()?;
    }
    for _ in 0..3 {
        read_string(rec, &mut off).ok()?;
    }
    skip_scalar(ScalarKind::U32, rec, &mut off).ok()?; // face
    let money = read_i64_be(rec, &mut off).ok()?;
    skip_scalar(ScalarKind::I64, rec, &mut off).ok()?; // current_loan
    let colour = read_u8(rec, &mut off).ok()?;
    Some(record(&[
        ("money", SlValue::Int(money)),
        ("colour", SlValue::Uint(u64::from(colour))),
    ]))
}

/// DATE v211 (`CH_RIFF`): `_date_sl_compat`.
#[must_use]
pub(crate) fn date_from_riff(body: &[u8]) -> Option<(i32, u64)> {
    let mut off = 0usize;
    let date = read_i32_be(body, &mut off).ok()?;
    skip_scalar(ScalarKind::U16, body, &mut off).ok()?; // date_fract
    let tick = u64::from(read_u16_be(body, &mut off).ok()?);
    Some((date, tick))
}

/// Avanza el struct `common` de vehículo hasta los campos que importan.
fn vehicle_common_fields(rec: &[u8]) -> Option<(u32, u8, u8, u32, u8, u8, u8)> {
    let mut off = 0usize;
    let subtype = read_u8(rec, &mut off).ok()?;
    skip_scalar(ScalarKind::U32, rec, &mut off).ok()?; // next
    read_string(rec, &mut off).ok()?; // name
    skip_scalar(ScalarKind::U16, rec, &mut off).ok()?; // unitnumber
    skip_scalar(ScalarKind::U8, rec, &mut off).ok()?; // owner
    let tile = read_u32_be(rec, &mut off).ok()?;
    skip_scalar(ScalarKind::U32, rec, &mut off).ok()?; // dest_tile
    skip_scalar(ScalarKind::U16, rec, &mut off).ok()?; // x_pos
    skip_scalar(ScalarKind::U16, rec, &mut off).ok()?; // y_pos
    skip_scalar(ScalarKind::U16, rec, &mut off).ok()?; // z_pos
    skip_scalar(ScalarKind::U8, rec, &mut off).ok()?; // direction
    off += 2;
    skip_scalar(ScalarKind::U8, rec, &mut off).ok()?; // spritenum
    off += 5;
    skip_scalar(ScalarKind::U16, rec, &mut off).ok()?; // engine_type
    off += 2;
    skip_scalar(ScalarKind::U16, rec, &mut off).ok()?; // cur_speed
    skip_scalar(ScalarKind::U8, rec, &mut off).ok()?; // subspeed
    skip_scalar(ScalarKind::U8, rec, &mut off).ok()?; // acceleration
    skip_scalar(ScalarKind::U8, rec, &mut off).ok()?; // progress
    let vehstatus = read_u8(rec, &mut off).ok()?;
    skip_scalar(ScalarKind::U16, rec, &mut off).ok()?; // last_station_visited
    let cargo_type = read_u8(rec, &mut off).ok()?;
    skip_scalar(ScalarKind::U8, rec, &mut off).ok()?; // cargo_subtype
    skip_scalar(ScalarKind::U16, rec, &mut off).ok()?; // cargo_cap
    skip_scalar(ScalarKind::U8, rec, &mut off).ok()?; // day_counter
    skip_scalar(ScalarKind::U8, rec, &mut off).ok()?; // tick_counter
    skip_scalar(ScalarKind::U8, rec, &mut off).ok()?; // running_ticks
    let cur_implicit_order_index = read_u8(rec, &mut off).ok()?;
    let cur_real_order_index = read_u8(rec, &mut off).ok()?;
    off += 1 + 1 + 2;
    skip_scalar(ScalarKind::U8, rec, &mut off).ok()?; // refit_cargo
    skip_scalar(ScalarKind::U16, rec, &mut off).ok()?; // wait_time
    skip_scalar(ScalarKind::U16, rec, &mut off).ok()?; // travel_time
    skip_scalar(ScalarKind::U16, rec, &mut off).ok()?; // max_speed
    skip_scalar(ScalarKind::U32, rec, &mut off).ok()?; // timetable_start
    let mut orders = read_u32_be(rec, &mut off).ok()?;
    if orders == 0 || orders > 64 {
        orders = scan_order_ref(rec);
    }
    Some((
        tile,
        subtype,
        cargo_type,
        orders,
        cur_real_order_index,
        cur_implicit_order_index,
        vehstatus,
    ))
}

/// Busca referencia `OrderList` (1-based) en offsets típicos del save v211.
fn scan_order_ref(payload: &[u8]) -> u32 {
    for prefer in [16usize, 104, 57, 91, 4] {
        if prefer + 4 <= payload.len() {
            let v = u32::from_be_bytes(payload[prefer..prefer + 4].try_into().unwrap_or([0; 4]));
            if (1..=8).contains(&v) {
                return v;
            }
        }
    }
    0
}

fn vehs_record(rec: &[u8]) -> Option<SlRecord> {
    if rec.is_empty() {
        return None;
    }
    let vtype = rec[0];
    let (sub_name, payload_off) = match vtype {
        0 => ("train", 1usize),
        1 => ("roadveh", 1),
        _ => return None,
    };
    let payload = rec.get(payload_off..)?;
    let (tile, subtype, cargo_type, orders, cur_order, cur_implicit, vehstatus) =
        vehicle_common_fields(payload)?;
    let common = record(&[
        ("tile", SlValue::Uint(u64::from(tile))),
        ("subtype", SlValue::Uint(u64::from(subtype))),
        ("cargo_type", SlValue::Uint(u64::from(cargo_type))),
        ("orders", SlValue::Uint(u64::from(orders))),
        ("cur_real_order_index", SlValue::Uint(u64::from(cur_order))),
        (
            "cur_implicit_order_index",
            SlValue::Uint(u64::from(cur_implicit)),
        ),
        ("vehstatus", SlValue::Uint(u64::from(vehstatus))),
    ]);
    Some(record(&[
        ("type", SlValue::Uint(u64::from(vtype))),
        (
            sub_name,
            SlValue::Structs(vec![record(&[("common", SlValue::Structs(vec![common]))])]),
        ),
    ]))
}

fn legacy_record(chunk: &RawChunk, rec: &[u8]) -> Option<SlRecord> {
    let name = std::str::from_utf8(&chunk.name).unwrap_or("");
    match name {
        "STNN" => stnn_record(rec),
        "ORDR" => ordr_record(rec),
        "ORDL" => ordl_record(rec),
        "CITY" => city_record(rec),
        "PLYR" => plyr_record(rec),
        "VEHS" => vehs_record(rec),
        _ => None,
    }
}

/// Filas de un chunk: tablas modernas o arrays legacy según tipo / versión.
#[must_use]
pub(crate) fn chunk_rows(chunk: &RawChunk, save_version: u16) -> Vec<(u32, SlRecord)> {
    match chunk.ch_type {
        CH_TABLE => parse_table_chunk(&chunk.body, false).unwrap_or_default(),
        CH_SPARSE_TABLE => parse_table_chunk(&chunk.body, true).unwrap_or_default(),
        CH_ARRAY if save_version >= SLV_TABLE_CHUNKS => {
            parse_table_chunk(&chunk.body, false).unwrap_or_default()
        }
        CH_SPARSE_ARRAY if save_version >= SLV_TABLE_CHUNKS => {
            parse_table_chunk(&chunk.body, true).unwrap_or_default()
        }
        CH_ARRAY | CH_SPARSE_ARRAY | super::chunks::CH_READONLY => {
            gamma_records(&chunk.body, chunk.ch_type == CH_SPARSE_ARRAY)
                .into_iter()
                .filter_map(|(idx, rec)| legacy_record(chunk, &rec).map(|r| (idx, r)))
                .collect()
        }
        CH_RIFF if &chunk.name == b"DATE" => date_from_riff(&chunk.body)
            .map(|(date, tick)| {
                vec![(
                    0,
                    record(&[
                        ("date", SlValue::Int(i64::from(date))),
                        ("tick_counter", SlValue::Uint(tick)),
                    ]),
                )]
            })
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn parses_stationlist_stnn_record() {
        let mut path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.pop();
        path.pop();
        path.push("tests/fixtures/stationlist-test.sav");
        let raw = std::fs::read(path).expect("sav");
        let (data, _) = super::super::container::decompress(&raw).expect("decompress");
        let chunks = super::super::chunks::parse_chunks(&data).expect("chunks");
        let stnn = super::super::chunks::find_chunk(&chunks, "STNN").expect("STNN");
        let rows = chunk_rows(stnn, 211);
        assert!(rows.len() >= 8, "stationlist tiene ~8 estaciones");
        assert!(rows[0].1.iter().any(|(k, _)| k == "xy"));
    }
}
