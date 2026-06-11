//! Construye el bloque `.ottdmap` (MAP1 v1 + footers) desde los chunks del save,
//! port directo de `export_ottdmap_from_chunks` de `scripts/parse_sav.py` para
//! reutilizar `Map::from_ottd_binary_with_extras` y su pipeline ya validado.

use crate::tnbp_decode::read_sl_gamma;

use super::SavError;
use super::chunks::{CH_ARRAY, CH_TABLE, RawChunk, find_chunk};
use super::table::{SlValue, parse_table_chunk, record_get};

const SLV_INCREASE_HOUSE_LIMIT: u16 = 348;
/// PR#13030: a partir de aquí `WaterTileType` vive directo en bits 4–7 de m5.
const SLV_WATER_TILE_TYPE: u16 = 342;
const MP_HOUSE: u8 = 3;
const MP_STATION: u8 = 5;
const MP_WATER: u8 = 6;
const MP_OBJECT: u8 = 10;
/// Offset del byte `Industry.type` en INDY `CH_ARRAY` (saves ~200+, ver `parse_sav.py`).
const INDY_TYPE_BYTE_OFFSET: usize = 9;

pub(crate) fn dimensions(chunks: &[RawChunk]) -> Result<(u32, u32), SavError> {
    if let Some(maps) = find_chunk(chunks, "MAPS") {
        if maps.ch_type == CH_TABLE
            && let Ok(rows) = parse_table_chunk(&maps.body, false)
            && let Some((_, record)) = rows.first()
        {
            let dim_x = record_get(record, "dim_x").and_then(SlValue::as_u64);
            let dim_y = record_get(record, "dim_y").and_then(SlValue::as_u64);
            if let (Some(x), Some(y)) = (dim_x, dim_y) {
                #[allow(clippy::cast_possible_truncation)]
                return Ok((x as u32, y as u32));
            }
        }
        if maps.ch_type == super::chunks::CH_RIFF
            && let (Some(xb), Some(yb)) = (maps.body.get(0..4), maps.body.get(4..8))
            && let (Ok(xb), Ok(yb)) = (<[u8; 4]>::try_from(xb), <[u8; 4]>::try_from(yb))
        {
            return Ok((u32::from_be_bytes(xb), u32::from_be_bytes(yb)));
        }
    }
    if let Some(mapt) = find_chunk(chunks, "MAPT")
        && let Some(dims) = infer_dimensions(mapt.body.len())
    {
        return Ok(dims);
    }
    Err(SavError::BadFormat(
        "no se pudieron determinar las dimensiones del mapa (MAPS/MAPT)".into(),
    ))
}

fn infer_dimensions(mapt_len: usize) -> Option<(u32, u32)> {
    for bits_w in 6..=12u32 {
        let w = 1usize << bits_w;
        for bits_h in 6..=12u32 {
            let h = 1usize << bits_h;
            if w * h == mapt_len {
                #[allow(clippy::cast_possible_truncation)]
                return Some((w as u32, h as u32));
            }
        }
    }
    None
}

fn padded_plane(chunks: &[RawChunk], name: &str, len: usize) -> Vec<u8> {
    let mut plane = find_chunk(chunks, name)
        .map(|c| c.body.clone())
        .unwrap_or_default();
    plane.truncate(len);
    plane.resize(len, 0);
    plane
}

/// `(industry_index, type)` desde INDY: `CH_ARRAY` (byte fijo) o `CH_TABLE` (campo `type`).
pub(crate) fn indy_pairs(chunks: &[RawChunk]) -> Vec<(u16, u8)> {
    let Some(indy) = find_chunk(chunks, "INDY") else {
        return Vec::new();
    };
    match indy.ch_type {
        CH_ARRAY => indy_pairs_from_array(&indy.body),
        CH_TABLE => parse_table_chunk(&indy.body, false)
            .map(|rows| {
                rows.iter()
                    .filter_map(|(idx, record)| {
                        let ty = record_get(record, "type").and_then(SlValue::as_u64)?;
                        #[allow(clippy::cast_possible_truncation)]
                        Some((*idx as u16, ty as u8))
                    })
                    .collect()
            })
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

fn indy_pairs_from_array(body: &[u8]) -> Vec<(u16, u8)> {
    let mut out = Vec::new();
    let mut off = 0usize;
    let mut idx = 0u16;
    while let Ok(n) = read_sl_gamma(body, &mut off) {
        if n == 0 {
            break;
        }
        let len = n as usize - 1;
        if off + len > body.len() {
            break;
        }
        let record = &body[off..off + len];
        off += len;
        if record.len() > INDY_TYPE_BYTE_OFFSET {
            out.push((idx, record[INDY_TYPE_BYTE_OFFSET]));
        }
        idx += 1;
    }
    out
}

/// `tile_index → ObjectType` desde el chunk `OBJS` (`CH_TABLE`).
fn objs_types(chunks: &[RawChunk]) -> Vec<(u32, u8)> {
    let Some(objs) = find_chunk(chunks, "OBJS") else {
        return Vec::new();
    };
    if objs.ch_type != CH_TABLE {
        return Vec::new();
    }
    parse_table_chunk(&objs.body, false)
        .map(|rows| {
            rows.iter()
                .filter_map(|(_, record)| {
                    let tile = record_get(record, "location.tile").and_then(SlValue::as_u64)?;
                    let ty = record_get(record, "type").and_then(SlValue::as_u64)?;
                    #[allow(clippy::cast_possible_truncation)]
                    Some((tile as u32, ty as u8))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// m8 LE; en saves < 348 el `HouseID` vive en M3HI/M3LO (`afterload.cpp`).
#[allow(clippy::similar_names)]
fn build_m8_le(
    version: u16,
    mapt: &[u8],
    map8: &[u8],
    m3lo: &[u8],
    m3hi: &[u8],
    expected: usize,
) -> Vec<u8> {
    // MAP8 es `SLE_UINT16` big-endian en el save; el `.ottdmap` lo quiere LE.
    let mut buf = vec![0u8; expected * 2];
    for i in 0..expected {
        buf[i * 2] = map8.get(i * 2 + 1).copied().unwrap_or(0);
        buf[i * 2 + 1] = map8.get(i * 2).copied().unwrap_or(0);
    }
    if version < SLV_INCREASE_HOUSE_LIMIT {
        for i in 0..expected {
            if (mapt[i] >> 4) & 0xF != MP_HOUSE {
                continue;
            }
            let hid = u16::from(m3hi[i]) | (u16::from((m3lo[i] >> 6) & 1) << 8);
            buf[i * 2..i * 2 + 2].copy_from_slice(&hid.to_le_bytes());
        }
    }
    buf
}

fn first_tunnel_blob(chunks: &[RawChunk]) -> Option<&RawChunk> {
    ["TNBP", "TBUS", "TUNN"]
        .iter()
        .find_map(|name| find_chunk(chunks, name).filter(|c| !c.body.is_empty()))
}

/// Saves < `SLV_WATER_TILE_TYPE`: codificación vieja de agua en m5
/// (tipo en bits 4–7: 0x0 normal + flag coast en bit 0, 0x1 lock, 0x8 depot).
/// La convertimos a la enumeración nueva (Clear=0, Coast=1, Lock=2, Depot=3),
/// igual que `afterload.cpp`, preservando los bits bajos.
fn normalize_old_water_m5(version: u16, mapt: &[u8], plane5: &mut [u8]) {
    if version >= SLV_WATER_TILE_TYPE {
        return;
    }
    for (t, m5) in mapt.iter().zip(plane5.iter_mut()) {
        if (t >> 4) & 0xF != MP_WATER {
            continue;
        }
        let new_type = match (*m5 >> 4) & 0xF {
            0x0 => u8::from(*m5 & 1 != 0), // flag coast
            0x1 => 2,                      // lock
            0x8 => 3,                      // depot
            _ => 0,
        };
        *m5 = (*m5 & 0x0F) | (new_type << 4);
    }
}

/// Cuerpo `.ottdmap` completo (cabecera MAP1 + planos densos + footers).
#[allow(clippy::cast_possible_truncation, clippy::similar_names)]
pub(crate) fn export_ottdmap(chunks: &[RawChunk], version: u16) -> Result<Vec<u8>, SavError> {
    let (dim_x, dim_y) = dimensions(chunks)?;
    let expected = dim_x as usize * dim_y as usize;

    let mapt_raw = find_chunk(chunks, "MAPT")
        .map(|c| c.body.clone())
        .unwrap_or_default();
    if mapt_raw.len() < expected {
        return Err(SavError::BadFormat(format!(
            "MAPT demasiado corto: {} bytes, esperados {expected}",
            mapt_raw.len()
        )));
    }
    let mapt = &mapt_raw[..expected];

    let maph = padded_plane(chunks, "MAPH", expected);
    let map1 = padded_plane(chunks, "MAPO", expected);
    let map6 = padded_plane(chunks, "MAPE", expected);
    let map7 = padded_plane(chunks, "MAP7", expected);
    let m3lo = padded_plane(chunks, "M3LO", expected);
    let m3hi = padded_plane(chunks, "M3HI", expected);
    let mut map5 = padded_plane(chunks, "MAP5", expected);
    let map8 = padded_plane(chunks, "MAP8", expected * 2);
    let map2_raw = find_chunk(chunks, "MAP2")
        .map(|c| c.body.clone())
        .unwrap_or_default();

    normalize_old_water_m5(version, mapt, &mut map5);

    // ObjectType visible en m5 para MP_OBJECT (overlay del chunk OBJS).
    for (tile, ty) in objs_types(chunks) {
        let i = tile as usize;
        if i < expected && (mapt[i] >> 4) & 0xF == MP_OBJECT && ty != 0xFF {
            map5[i] = ty;
        }
    }

    let m8 = build_m8_le(version, mapt, &map8, &m3lo, &m3hi, expected);

    let (m2_lo, m2_hi): (Vec<u8>, Vec<u8>) = if map2_raw.len() >= 2 * expected {
        // MAP2 es `SLE_UINT16` big-endian en el save (TownID en MP_HOUSE,
        // tipo/variante de señal en MP_RAILWAY): byte alto primero.
        (
            (0..expected).map(|i| map2_raw[i * 2 + 1]).collect(),
            (0..expected).map(|i| map2_raw[i * 2]).collect(),
        )
    } else {
        let mut lo = map2_raw.clone();
        lo.truncate(expected);
        lo.resize(expected, 0);
        (lo, vec![0; expected])
    };

    let mut body = Vec::with_capacity(16 + expected * 12);
    body.extend_from_slice(b"MAP1");
    body.extend_from_slice(&dim_x.to_le_bytes());
    body.extend_from_slice(&dim_y.to_le_bytes());
    body.extend_from_slice(&1u16.to_le_bytes()); // format_version
    body.extend_from_slice(&1u16.to_le_bytes()); // flags: HAS_M2_HI
    body.extend_from_slice(mapt);
    body.extend_from_slice(&maph);
    body.extend_from_slice(&map1);
    body.extend_from_slice(&m2_lo);
    body.extend_from_slice(&m2_hi);
    body.extend_from_slice(&m3lo);
    body.extend_from_slice(&m3hi);
    body.extend_from_slice(&map5);
    body.extend_from_slice(&map6);
    body.extend_from_slice(&map7);
    body.extend_from_slice(&m8);

    // Footer INDP.
    let pairs = indy_pairs(chunks);
    body.extend_from_slice(b"INDP");
    body.extend_from_slice(&(pairs.len() as u32).to_le_bytes());
    for (i, t) in &pairs {
        body.extend_from_slice(&i.to_le_bytes());
        body.push(*t);
    }

    // Footer STNN (blob crudo).
    if let Some(stnn) = find_chunk(chunks, "STNN").filter(|c| !c.body.is_empty()) {
        body.extend_from_slice(b"STNN");
        body.extend_from_slice(&(stnn.body.len() as u32).to_le_bytes());
        body.extend_from_slice(&stnn.body);
    }

    // Footer TNBP (túneles JGR si existen).
    if let Some(tunnel) = first_tunnel_blob(chunks) {
        body.extend_from_slice(b"TNBP");
        body.extend_from_slice(&(tunnel.body.len() as u32).to_le_bytes());
        body.extend_from_slice(&tunnel.body);
    }

    // Footer STXY derivado de MAPT.
    let coords: Vec<(u16, u16)> = (0..expected)
        .filter(|&i| (mapt[i] >> 4) & 0xF == MP_STATION)
        .map(|i| ((i as u32 % dim_x) as u16, (i as u32 / dim_x) as u16))
        .collect();
    body.extend_from_slice(b"STXY");
    body.extend_from_slice(&(coords.len() as u32).to_le_bytes());
    for (x, y) in coords {
        body.extend_from_slice(&x.to_le_bytes());
        body.extend_from_slice(&y.to_le_bytes());
    }

    Ok(body)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::cast_possible_truncation)]
mod tests {
    use super::super::chunks::RawChunk;
    use super::*;

    fn riff(name: [u8; 4], body: Vec<u8>) -> RawChunk {
        RawChunk {
            name,
            ch_type: super::super::chunks::CH_RIFF,
            body,
        }
    }

    fn maps_table_chunk(dim_x: u32, dim_y: u32) -> RawChunk {
        use super::super::table::tests::build_table_body;
        let mut rec = Vec::new();
        rec.extend_from_slice(&dim_x.to_be_bytes());
        rec.extend_from_slice(&dim_y.to_be_bytes());
        let body = build_table_body(&[(6, "dim_x"), (6, "dim_y")], &[rec]);
        RawChunk {
            name: *b"MAPS",
            ch_type: CH_TABLE,
            body,
        }
    }

    #[test]
    fn exports_minimal_map_loadable_by_map_loader() {
        let w = 64u32;
        let h = 64u32;
        let n = (w * h) as usize;
        let chunks = vec![
            maps_table_chunk(w, h),
            riff(*b"MAPT", vec![0u8; n]),
            riff(*b"MAPH", vec![1u8; n]),
        ];
        let body = export_ottdmap(&chunks, 300).expect("export");
        let (map, extras) = crate::Map::from_ottd_binary_with_extras(&body).expect("load");
        assert_eq!(map.dimensions(), (w, h));
        assert!(extras.industry_types.is_empty());
        assert!(extras.station_xy.is_empty());
    }

    #[test]
    fn station_tiles_produce_stxy_footer() {
        let w = 64u32;
        let n = (w * w) as usize;
        let mut mapt = vec![0u8; n];
        mapt[65] = 5 << 4; // MP_STATION en (1,1)
        let chunks = vec![maps_table_chunk(w, w), riff(*b"MAPT", mapt)];
        let body = export_ottdmap(&chunks, 300).expect("export");
        let (_, extras) = crate::Map::from_ottd_binary_with_extras(&body).expect("load");
        assert_eq!(extras.station_xy, vec![(1, 1)]);
    }

    #[test]
    fn house_id_migrates_from_m3_in_old_saves() {
        let w = 64u32;
        let n = (w * w) as usize;
        let mut mapt = vec![0u8; n];
        mapt[0] = MP_HOUSE << 4;
        let mut m3lo = vec![0u8; n];
        let mut m3hi = vec![0u8; n];
        m3hi[0] = 0x2A;
        m3lo[0] = 0x40; // bit 6 → bit 8 del HouseID
        let chunks = vec![
            maps_table_chunk(w, w),
            riff(*b"MAPT", mapt),
            riff(*b"M3LO", m3lo),
            riff(*b"M3HI", m3hi),
        ];
        let body = export_ottdmap(&chunks, 347).expect("export");
        let (map, _) = crate::Map::from_ottd_binary_with_extras(&body).expect("load");
        let tile = map.get(crate::TileCoord::new(0, 0)).expect("tile");
        assert_eq!(tile.m8, 0x012A);
    }

    #[test]
    fn old_water_m5_encoding_is_normalized_before_slv_342() {
        let w = 64u32;
        let n = (w * w) as usize;
        let mut mapt = vec![0u8; n];
        let mut plane5 = vec![0u8; n];
        // (0,0) mar, (1,0) costa, (2,0) esclusa, (3,0) depósito naval (codificación vieja).
        mapt[..4].fill(MP_WATER << 4);
        plane5[0] = 0x00;
        plane5[1] = 0x01; // flag coast en bit 0
        plane5[2] = 0x12; // lock con orientación en bits bajos
        plane5[3] = 0x81; // depot con part/axis en bits bajos
        let chunks = vec![
            maps_table_chunk(w, w),
            riff(*b"MAPT", mapt),
            riff(*b"MAP5", plane5),
        ];
        let body = export_ottdmap(&chunks, 308).expect("export");
        let (map, _) = crate::Map::from_ottd_binary_with_extras(&body).expect("load");
        let m5_at = |x: i32| map.get(crate::TileCoord::new(x, 0)).expect("tile").m5;
        assert_eq!(m5_at(0), 0x00, "Clear");
        assert_eq!(m5_at(1), 0x11, "Coast (bits bajos preservados)");
        assert_eq!(m5_at(2), 0x22, "Lock");
        assert_eq!(m5_at(3), 0x31, "Depot");

        // Con version ≥ 342 el m5 se respeta tal cual.
        let chunks_new = vec![
            maps_table_chunk(w, w),
            riff(*b"MAPT", vec![MP_WATER << 4; n]),
            riff(*b"MAP5", vec![0x11u8; n]),
        ];
        let body = export_ottdmap(&chunks_new, 342).expect("export");
        let (map, _) = crate::Map::from_ottd_binary_with_extras(&body).expect("load");
        assert_eq!(map.get(crate::TileCoord::new(0, 0)).expect("tile").m5, 0x11);
    }

    #[test]
    fn missing_mapt_is_error() {
        let chunks = vec![maps_table_chunk(64, 64)];
        assert!(export_ottdmap(&chunks, 300).is_err());
    }
}
