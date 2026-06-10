use super::{Map, MapError, Tile, TileKind};

pub(crate) const OTTDMAP_MAGIC_VERSIONED: &[u8; 4] = b"MAP1";
pub(crate) const OTTDMAP_HEADER_LEN_VERSIONED: usize = 16;
pub const OTTDMAP_FORMAT_VERSION_CURRENT: u16 = 1;
pub const OTTDMAP_FLAG_HAS_M2_HI: u16 = 1 << 0;

#[derive(Debug, Clone, Copy)]
struct OttdmapHeader {
    dense_offset: usize,
    width: u32,
    height: u32,
    flags: u16,
}

fn parse_ottdmap_header(data: &[u8]) -> Result<OttdmapHeader, MapError> {
    if data.len() < OTTDMAP_HEADER_LEN_VERSIONED || &data[0..4] != OTTDMAP_MAGIC_VERSIONED {
        return Err(MapError::OutOfBounds);
    }
    let width = u32::from_le_bytes(
        data[4..8]
            .try_into()
            .expect("checked versioned header length above"),
    );
    let height = u32::from_le_bytes(
        data[8..12]
            .try_into()
            .expect("checked versioned header length above"),
    );
    let format_version = u16::from_le_bytes(
        data[12..14]
            .try_into()
            .expect("checked versioned header length above"),
    );
    let flags = u16::from_le_bytes(
        data[14..16]
            .try_into()
            .expect("checked versioned header length above"),
    );
    if format_version != OTTDMAP_FORMAT_VERSION_CURRENT {
        return Err(MapError::OutOfBounds);
    }
    Ok(OttdmapHeader {
        dense_offset: OTTDMAP_HEADER_LEN_VERSIONED,
        width,
        height,
        flags,
    })
}

/// Rebanadas de los planos densos `.ottdmap` v1 (`MAP1`, cabecera versionada).
#[derive(Debug, Clone, Copy)]
struct OttdmapDenseSlices<'a> {
    tile_types: &'a [u8],
    heights: &'a [u8],
    m5: &'a [u8],
    m1: &'a [u8],
    m6: &'a [u8],
    m8: &'a [u8],
    m3: &'a [u8],
    m2: &'a [u8],
    m7: &'a [u8],
    m3hi: &'a [u8],
    m2_hi: &'a [u8],
}

fn ottdmap_dense_slices(
    data: &[u8],
    header: OttdmapHeader,
    n: usize,
) -> Result<OttdmapDenseSlices<'_>, MapError> {
    let dense_offset = header.dense_offset;
    let dense_len = if header.flags & OTTDMAP_FLAG_HAS_M2_HI != 0 {
        n * 12
    } else {
        n * 11
    };
    if data.len() < dense_offset + dense_len {
        return Err(MapError::OutOfBounds);
    }

    // Orden físico v1 en archivo: MAPT, MAPH, M1, M2, [M2_HI], M3, M3HI, M5, M6, M7, M8.
    let tile_types = &data[dense_offset..dense_offset + n];
    let heights = &data[dense_offset + n..dense_offset + 2 * n];
    let m1 = &data[dense_offset + 2 * n..dense_offset + 3 * n];
    let m2 = &data[dense_offset + 3 * n..dense_offset + 4 * n];
    let (m2_hi, base_after_m2_hi) = if header.flags & OTTDMAP_FLAG_HAS_M2_HI != 0 {
        (
            &data[dense_offset + 4 * n..dense_offset + 5 * n],
            dense_offset + 5 * n,
        )
    } else {
        (&[] as &[u8], dense_offset + 4 * n)
    };
    let m3 = &data[base_after_m2_hi..base_after_m2_hi + n];
    let m3hi = &data[base_after_m2_hi + n..base_after_m2_hi + 2 * n];
    let m5 = &data[base_after_m2_hi + 2 * n..base_after_m2_hi + 3 * n];
    let m6 = &data[base_after_m2_hi + 3 * n..base_after_m2_hi + 4 * n];
    let m7 = &data[base_after_m2_hi + 4 * n..base_after_m2_hi + 5 * n];
    let m8 = &data[base_after_m2_hi + 5 * n..base_after_m2_hi + 7 * n];
    Ok(OttdmapDenseSlices {
        tile_types,
        heights,
        m5,
        m1,
        m6,
        m8,
        m3,
        m2,
        m7,
        m3hi,
        m2_hi,
    })
}

#[inline]
fn ottd_m8_at(m8: &[u8], i: usize) -> u16 {
    let o = i * 2;
    if m8.len() < o + 2 {
        return 0;
    }
    u16::from_le_bytes([m8[o], m8[o + 1]])
}

#[inline]
fn ottd_byte_or(plane: &[u8], i: usize) -> u8 {
    if plane.is_empty() { 0 } else { plane[i] }
}

#[inline]
fn ottd_tile_kind(ottd_type: u8, m5: u8) -> TileKind {
    let transport_subtype = (m5 >> 6) & 0x3;
    match ottd_type {
        0 | 10 => TileKind::Grass,
        1 => {
            if transport_subtype == 2 {
                TileKind::RailDepot
            } else {
                TileKind::Rail
            }
        }
        2 => {
            if transport_subtype == 2 {
                TileKind::RoadDepot
            } else {
                TileKind::Road
            }
        }
        3 => TileKind::House,
        4 => TileKind::Forest,
        5 => TileKind::Station,
        6 => TileKind::Water,
        7 => TileKind::Void,
        8 => TileKind::Industry,
        9 => {
            let is_bridge = m5 & 0x80 != 0;
            // `TransportType` de OpenTTD en bits 2–3: 0 = rail, 1 = road.
            let transport = (m5 >> 2) & 0x3;
            if is_bridge {
                if transport == 0 {
                    TileKind::RailBridge
                } else {
                    TileKind::RoadBridge
                }
            } else if transport == 0 {
                TileKind::RailTunnel
            } else {
                TileKind::RoadTunnel
            }
        }
        t => TileKind::Unknown(t),
    }
}
impl Map {
    /// Carga un mapa desde un archivo `.ottdmap` generado por `scripts/parse_sav.py`.
    ///
    /// Formato:
    /// Formato binario `.ottdmap`:
    ///
    /// - Cabecera versionada: `MAP1` + `width` + `height` + `format_version` + `flags` (16 bytes)
    /// - Luego, el bloque de planos densos:
    /// - W×H bytes: `tile_type` (nibble alto = `TileType` `OpenTTD`)
    /// - W×H bytes: height por tesela
    /// - W×H bytes: m1 (owner, índice de industria)
    /// - W×H bytes: m2 (MAP2 byte bajo)
    /// - W×H bytes: m2_hi (MAP2 byte alto)
    /// - W×H bytes: m3 (M3LO; tram track bits 0–3 en carretera normal)
    /// - W×H bytes: m3hi (M3HI)
    /// - W×H bytes: m5 (road bits, TrackBits, gfx industria bajo, ObjectType)
    /// - W×H bytes: m6 (bit 2 = bit 8 del gfx industria; StationType)
    /// - W×H bytes: m7 (MAP7)
    /// - W×H×2 bytes: m8 LE (HouseID en MP_HOUSE; RoadType tram en bits 6–11 en MP_ROAD)
    ///
    /// Tras los planos denses pueden seguir footers (`INDP`, `STNN`, `TNBP`, `STXY`); `from_ottd_binary` los ignora.
    ///
    /// La correspondencia de tipos `OpenTTD` → `TileKind`:
    ///
    /// | `TileType` | Nombre         | `TileKind`         |
    /// |----------|----------------|------------------|
    /// | 0        | `MP_CLEAR`       | Grass            |
    /// | 1        | `MP_RAILWAY`     | Rail             |
    /// | 2        | `MP_ROAD`        | Road             |
    /// | 3        | `MP_HOUSE`       | House            |
    /// | 4        | `MP_TREES`       | Forest           |
    /// | 5        | `MP_STATION`     | Station          |
    /// | 6        | `MP_WATER`       | Water            |
    /// | 7        | `MP_VOID`        | Void             |
    /// | 8        | `MP_INDUSTRY`    | Industry/Coal    |
    /// | 9        | `MP_TUNNELBRIDGE`| Road/Rail tunnel or bridge (m5) |
    /// | 10       | `MP_OBJECT`      | Grass            |
    ///
    /// # Errors
    ///
    /// Devuelve `Err` si el archivo no usa cabecera `MAP1` o está truncado.
    #[allow(clippy::missing_panics_doc)]
    pub fn from_ottd_binary(data: &[u8]) -> Result<Self, MapError> {
        let header = parse_ottdmap_header(data)?;
        let width = header.width;
        let height = header.height;
        let n = (width as usize).saturating_mul(height as usize);
        let s = ottdmap_dense_slices(data, header, n)?;

        let mut tiles = Vec::with_capacity(n);
        for i in 0..n {
            let raw_type = s.tile_types[i];
            let ottd_type = (raw_type >> 4) & 0xF;
            let m5 = s.m5[i];
            tiles.push(Tile {
                height: s.heights[i],
                kind: ottd_tile_kind(ottd_type, m5),
                mapt: raw_type,
                m5,
                m1: ottd_byte_or(s.m1, i),
                m6: ottd_byte_or(s.m6, i),
                m8: ottd_m8_at(s.m8, i),
                m3: ottd_byte_or(s.m3, i),
                m2: ottd_byte_or(s.m2, i),
                m2_hi: ottd_byte_or(s.m2_hi, i),
                m7: ottd_byte_or(s.m7, i),
                m3hi: ottd_byte_or(s.m3hi, i),
            });
        }

        Ok(Self {
            width,
            height,
            tiles,
        })
    }

    /// Igual que [`Self::from_ottd_binary`], pero devuelve también los footers parseados (`INDP`, etc.).
    #[allow(clippy::missing_panics_doc)]
    pub fn from_ottd_binary_with_extras(
        data: &[u8],
    ) -> Result<(Self, crate::ottdmap_extras::OttdmapExtras), MapError> {
        let map = Self::from_ottd_binary(data)?;
        let n = (map.width as usize).saturating_mul(map.height as usize);
        let dense_end = crate::ottdmap_extras::dense_payload_end(data, n);
        let extras = crate::ottdmap_extras::OttdmapExtras::parse_footers(data, dense_end);
        Ok((map, extras))
    }
}
