//! Construye el bloque `.ottdmap` (MAP1 v1 + footers) desde los chunks del save,
//! port directo de `export_ottdmap_from_chunks` de `scripts/parse_sav.py` para
//! reutilizar `Map::from_ottd_binary_with_extras` y su pipeline ya validado.

use crate::tnbp_decode::read_sl_gamma;

use super::SavError;
use super::chunks::{CH_ARRAY, CH_TABLE, RawChunk, find_chunk};
use super::table::{SlValue, parse_table_chunk, record_get};

const SLV_INCREASE_HOUSE_LIMIT: u16 = 348;
/// PR#6811: a partir de aquí `RoadType` vive en m4 (road) / m8 bits 6–11 (tram).
const SLV_ROAD_TYPES: u16 = 214;
/// PR#13030: a partir de aquí `WaterTileType` vive directo en bits 4–7 de m5.
const SLV_WATER_TILE_TYPE: u16 = 342;
const MP_HOUSE: u8 = 3;
const MP_ROAD: u8 = 2;
const MP_STATION: u8 = 5;
const MP_TUNNELBRIDGE: u8 = 9;
const MP_WATER: u8 = 6;
const ROADTYPE_ROAD: u8 = 0;
const ROADTYPE_TRAM: u8 = 1;
const INVALID_ROADTYPE: u8 = 63;
const TRANSPORT_ROAD: u8 = 1;
const STATION_TYPE_TRUCK: u8 = 2;
const STATION_TYPE_BUS: u8 = 3;
const STATION_TYPE_ROAD_WAYPOINT: u8 = 8;
/// Offset del byte `Industry.type` en INDY `CH_ARRAY` (saves ~200+, ver `parse_sav.py`).
const INDY_TYPE_BYTE_OFFSET: usize = 9;
// OpenTTD crea mapas desde 64×64, pero el importador también consume fixtures
// estructurales mínimos del port. El límite superior y las multiplicaciones
// comprobadas son los que evitan reservas maliciosas.
const MIN_MAP_DIMENSION: u64 = 1;
const MAX_MAP_DIMENSION: u64 = 4096;

pub(crate) fn dimensions(chunks: &[RawChunk]) -> Result<(u32, u32), SavError> {
    if let Some(maps) = find_chunk(chunks, "MAPS") {
        if maps.ch_type == CH_TABLE
            && let Ok(rows) = parse_table_chunk(&maps.body, false)
            && let Some((_, record)) = rows.first()
        {
            let dim_x = record_get(record, "dim_x").and_then(SlValue::as_u64);
            let dim_y = record_get(record, "dim_y").and_then(SlValue::as_u64);
            if let (Some(x), Some(y)) = (dim_x, dim_y) {
                return validate_dimensions(x, y);
            }
        }
        if maps.ch_type == super::chunks::CH_RIFF
            && let (Some(xb), Some(yb)) = (maps.body.get(0..4), maps.body.get(4..8))
            && let (Ok(xb), Ok(yb)) = (<[u8; 4]>::try_from(xb), <[u8; 4]>::try_from(yb))
        {
            return validate_dimensions(
                u64::from(u32::from_be_bytes(xb)),
                u64::from(u32::from_be_bytes(yb)),
            );
        }
    }
    if let Some(mapt) = find_chunk(chunks, "MAPT")
        && let Some(dims) = infer_dimensions(mapt.body.len())
    {
        return validate_dimensions(u64::from(dims.0), u64::from(dims.1));
    }
    Err(SavError::BadFormat(
        "no se pudieron determinar las dimensiones del mapa (MAPS/MAPT)".into(),
    ))
}

fn validate_dimensions(width: u64, height: u64) -> Result<(u32, u32), SavError> {
    let supported = |dimension: u64| {
        (MIN_MAP_DIMENSION..=MAX_MAP_DIMENSION).contains(&dimension) && dimension.is_power_of_two()
    };
    if !supported(width) || !supported(height) {
        return Err(SavError::InvalidMapDimensions { width, height });
    }

    let width =
        u32::try_from(width).map_err(|_| SavError::InvalidMapDimensions { width, height })?;
    let height = u32::try_from(height).map_err(|_| SavError::InvalidMapDimensions {
        width: u64::from(width),
        height,
    })?;
    Ok((width, height))
}

fn map_tile_count(width: u32, height: u32) -> Result<usize, SavError> {
    usize::try_from(width)
        .ok()
        .and_then(|width| {
            usize::try_from(height)
                .ok()
                .and_then(|height| width.checked_mul(height))
        })
        .ok_or(SavError::InvalidMapDimensions {
            width: u64::from(width),
            height: u64::from(height),
        })
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

fn reserved_buffer(capacity: usize, context: &'static str) -> Result<Vec<u8>, SavError> {
    let mut buffer = Vec::new();
    buffer
        .try_reserve_exact(capacity)
        .map_err(|_| SavError::AllocationFailed {
            context,
            requested: capacity,
        })?;
    Ok(buffer)
}

fn zeroed_buffer(len: usize, context: &'static str) -> Result<Vec<u8>, SavError> {
    let mut buffer = reserved_buffer(len, context)?;
    buffer.resize(len, 0);
    Ok(buffer)
}

fn padded_plane(chunks: &[RawChunk], name: &str, len: usize) -> Result<Vec<u8>, SavError> {
    let mut plane = zeroed_buffer(len, "plano de mapa")?;
    if let Some(source) = find_chunk(chunks, name).map(|chunk| chunk.body.as_slice()) {
        let copied = source.len().min(len);
        plane[..copied].copy_from_slice(&source[..copied]);
    }
    Ok(plane)
}

fn map2_planes(
    data: &[u8],
    expected: usize,
    expected_twice: usize,
) -> Result<(Vec<u8>, Vec<u8>), SavError> {
    if data.len() >= expected_twice {
        // MAP2 es `SLE_UINT16` big-endian en el save (TownID en MP_HOUSE,
        // tipo/variante de señal en MP_RAILWAY): byte alto primero.
        let mut lo = zeroed_buffer(expected, "plano bajo MAP2")?;
        let mut hi = zeroed_buffer(expected, "plano alto MAP2")?;
        for (index, bytes) in data[..expected_twice].chunks_exact(2).enumerate() {
            hi[index] = bytes[0];
            lo[index] = bytes[1];
        }
        return Ok((lo, hi));
    }

    let mut lo = zeroed_buffer(expected, "plano bajo MAP2")?;
    let copied = data.len().min(expected);
    lo[..copied].copy_from_slice(&data[..copied]);
    Ok((lo, zeroed_buffer(expected, "plano alto MAP2")?))
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

/// `ObjectID → ObjectType` desde el pool denso `OBJS` (`CH_TABLE`).
///
/// El índice de fila es el `ObjectID` que `GetObjectIndex()` reconstruye desde
/// `m2 | (m5 << 16)`. `location.tile` sólo identifica el ancla geométrica del
/// objeto y no debe usarse como clave de la tesela ni sobrescribir `MAP5`.
fn objs_types(chunks: &[RawChunk]) -> Vec<(u32, u16)> {
    let Some(objs) = find_chunk(chunks, "OBJS") else {
        return Vec::new();
    };
    if objs.ch_type != CH_TABLE {
        return Vec::new();
    }
    parse_table_chunk(&objs.body, false)
        .map(|rows| {
            rows.iter()
                .filter_map(|(object_id, record)| {
                    let ty = record_get(record, "type").and_then(SlValue::as_u64)?;
                    let ty = u16::try_from(ty).ok()?;
                    (ty != u16::MAX).then_some((*object_id, ty))
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

fn tile_needs_road_types(mapt: u8, m5: u8, m6: u8) -> bool {
    match (mapt >> 4) & 0xF {
        MP_ROAD => true,
        MP_STATION => matches!(
            (m6 >> 3) & 0xF,
            STATION_TYPE_TRUCK | STATION_TYPE_BUS | STATION_TYPE_ROAD_WAYPOINT
        ),
        MP_TUNNELBRIDGE => ((m5 >> 2) & 0x3) == TRANSPORT_ROAD,
        _ => false,
    }
}

/// Saves < `SLV_ROAD_TYPES`: `RoadType` desde bits 6–7 de m7 → m4 + m8 bits 6–11.
fn apply_slv_road_types(
    version: u16,
    mapt: &[u8],
    m5: &[u8],
    m6: &[u8],
    m7: &mut [u8],
    m3hi: &mut [u8],
    m8_le: &mut [u8],
) {
    if version >= SLV_ROAD_TYPES {
        return;
    }
    let n = mapt.len();
    for i in 0..n {
        if !tile_needs_road_types(mapt[i], m5[i], m6[i]) {
            continue;
        }
        let road_rt = if m7[i] & (1 << 6) != 0 {
            ROADTYPE_ROAD
        } else {
            INVALID_ROADTYPE
        };
        let tram_rt = if m7[i] & (1 << 7) != 0 {
            ROADTYPE_TRAM
        } else {
            INVALID_ROADTYPE
        };
        m3hi[i] = (m3hi[i] & !0x3F) | (road_rt & 0x3F);
        let mut m8 = u16::from_le_bytes([m8_le[i * 2], m8_le[i * 2 + 1]]);
        m8 = (m8 & !(0x3F << 6)) | (u16::from(tram_rt & 0x3F) << 6);
        m8_le[i * 2..i * 2 + 2].copy_from_slice(&m8.to_le_bytes());
        m7[i] &= !0xC0;
    }
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
    let expected = map_tile_count(dim_x, dim_y)?;
    let expected_twice = expected
        .checked_mul(2)
        .ok_or(SavError::InvalidMapDimensions {
            width: u64::from(dim_x),
            height: u64::from(dim_y),
        })?;

    let mapt_raw = find_chunk(chunks, "MAPT").map_or(&[][..], |chunk| chunk.body.as_slice());
    if mapt_raw.len() < expected {
        return Err(SavError::BadFormat(format!(
            "MAPT demasiado corto: {} bytes, esperados {expected}",
            mapt_raw.len()
        )));
    }
    let mapt = &mapt_raw[..expected];

    let maph = padded_plane(chunks, "MAPH", expected)?;
    let map1 = padded_plane(chunks, "MAPO", expected)?;
    let map6 = padded_plane(chunks, "MAPE", expected)?;
    let mut map7 = padded_plane(chunks, "MAP7", expected)?;
    let m3lo = padded_plane(chunks, "M3LO", expected)?;
    let mut m3hi = padded_plane(chunks, "M3HI", expected)?;
    let mut map5 = padded_plane(chunks, "MAP5", expected)?;
    let map8 = padded_plane(chunks, "MAP8", expected_twice)?;
    let map2_raw = find_chunk(chunks, "MAP2").map_or(&[][..], |chunk| chunk.body.as_slice());

    normalize_old_water_m5(version, mapt, &mut map5);

    let mut m8 = build_m8_le(version, mapt, &map8, &m3lo, &m3hi, expected);
    apply_slv_road_types(version, mapt, &map5, &map6, &mut map7, &mut m3hi, &mut m8);

    let (m2_lo, m2_hi) = map2_planes(map2_raw, expected, expected_twice)?;

    let map_body_len = expected
        .checked_mul(12)
        .and_then(|size| size.checked_add(16))
        .ok_or(SavError::InvalidMapDimensions {
            width: u64::from(dim_x),
            height: u64::from(dim_y),
        })?;
    let mut body = reserved_buffer(map_body_len, "exportación ottdmap")?;
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

    // Footer OBTY: `MAP5` se conserva crudo; el tipo visual pertenece al pool
    // `OBJS` indexado por `ObjectID`.
    let object_types = objs_types(chunks);
    body.extend_from_slice(b"OBTY");
    body.extend_from_slice(&(object_types.len() as u32).to_le_bytes());
    for (object_id, object_type) in &object_types {
        body.extend_from_slice(&object_id.to_le_bytes());
        body.extend_from_slice(&object_type.to_le_bytes());
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
    fn object_pool_preserves_raw_map5_and_resolves_type_by_object_id() {
        use super::super::table::tests::build_table_body;

        let w = 64u32;
        let n = (w * w) as usize;
        let tile_index = 1usize;
        let object_id = 17u32;
        let mut mapt = vec![0u8; n];
        let mut map2_bytes = vec![0u8; n * 2];
        let mut records = vec![Vec::new(); object_id as usize];
        let mut object_record = Vec::new();
        object_record.extend_from_slice(&(tile_index as u32).to_be_bytes());
        object_record.extend_from_slice(&1u16.to_be_bytes()); // OBJECT_LIGHTHOUSE
        records.push(object_record);
        mapt[tile_index] = crate::map::OTTD_MP_OBJECT << 4;
        map2_bytes[tile_index * 2 + 1] = object_id as u8; // MAP2 del save es big-endian.

        let chunks = vec![
            maps_table_chunk(w, w),
            riff(*b"MAPT", mapt),
            riff(*b"MAP2", map2_bytes),
            RawChunk {
                name: *b"OBJS",
                ch_type: CH_TABLE,
                body: build_table_body(&[(6, "location.tile"), (4, "type")], &records),
            },
        ];
        let body = export_ottdmap(&chunks, 350).expect("export");
        let (map, extras) = crate::Map::from_ottd_binary_with_extras(&body).expect("load");
        let coord = crate::TileCoord::new(1, 0);
        let tile = map.get(coord).expect("object tile");

        assert_eq!(tile.m5, 0, "MAP5 permanece como byte alto del ObjectID");
        assert_eq!(extras.object_types, Some(vec![(object_id, 1)]));
        assert_eq!(map.object_type_at(coord), Some(1));
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
    fn road_types_migrate_from_m7_before_slv_214() {
        let w = 64u32;
        let n = (w * w) as usize;
        let mut mapt = vec![0u8; n];
        let mut map7_bytes = vec![0u8; n];
        mapt[0] = MP_ROAD << 4;
        map7_bytes[0] = 1 << 6; // pre-NRT: ROADTYPE_ROAD
        let chunks = vec![
            maps_table_chunk(w, w),
            riff(*b"MAPT", mapt),
            riff(*b"MAP7", map7_bytes),
        ];
        let body = export_ottdmap(&chunks, 211).expect("export");
        let (map, _) = crate::Map::from_ottd_binary_with_extras(&body).expect("load");
        let tile = map.get(crate::TileCoord::new(0, 0)).expect("tile");
        assert_eq!(tile.m3hi & 0x3F, ROADTYPE_ROAD);
        assert_eq!((tile.m8 >> 6) & 0x3F, u16::from(INVALID_ROADTYPE));
        assert_eq!(tile.m7 & 0xC0, 0);
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
        assert_eq!(
            map.get_kind(crate::TileCoord::new(3, 0)),
            Some(crate::TileKind::ShipDepot),
            "el WaterTileType::Depot importado conserva su semántica"
        );

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

    #[test]
    fn rejects_oversized_maps_dimensions_before_allocating_planes() {
        let chunks = vec![maps_table_chunk(8192, 8192)];
        assert_eq!(
            export_ottdmap(&chunks, 300),
            Err(SavError::InvalidMapDimensions {
                width: 8192,
                height: 8192,
            })
        );
    }

    #[test]
    fn rejects_non_power_of_two_maps_dimensions() {
        let chunks = vec![maps_table_chunk(192, 64)];
        assert_eq!(
            dimensions(&chunks),
            Err(SavError::InvalidMapDimensions {
                width: 192,
                height: 64,
            })
        );
    }

    #[test]
    fn accepts_rectangular_supported_maps_dimensions() {
        let chunks = vec![maps_table_chunk(64, 128)];
        assert_eq!(dimensions(&chunks), Ok((64, 128)));
    }

    #[test]
    fn accepts_minimal_power_of_two_fixture_dimensions() {
        let chunks = vec![maps_table_chunk(2, 2)];
        assert_eq!(dimensions(&chunks), Ok((2, 2)));
    }
}
