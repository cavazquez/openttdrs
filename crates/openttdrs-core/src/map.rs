//! Estructura del mapa y carga de `.ottdmap` versionado (`MAP1`).
#![allow(clippy::doc_markdown, clippy::expect_used, clippy::unwrap_used)]

/// Coordenada de tesela en el plano X/Y del mapa (análoga a índices de tesela en `OpenTTD`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct TileCoord {
    pub x: i32,
    pub y: i32,
}

impl TileCoord {
    #[must_use]
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

/// Nibble alto de `mapt` / MAPT: `TileType::TunnelBridge` en OpenTTD (= 9).
pub const OTTD_TILETYPE_TUNNELBRIDGE: u8 = 9;

/// Convierte un `TileIndex` de OpenTTD a coordenadas cuando el mapa es potencia de 2 en X e Y
/// (misma convención que `TileXY`: `tile = x | (y << log2(map_w))`).
#[must_use]
pub fn openttd_tile_index_to_coord(tile: u32, map_w: u32, map_h: u32) -> Option<TileCoord> {
    if !map_w.is_power_of_two() || !map_h.is_power_of_two() {
        return None;
    }
    let log_w = map_w.trailing_zeros();
    let x = tile & (map_w - 1);
    let y = tile >> log_w;
    if y >= map_h {
        return None;
    }
    let xi = i32::try_from(x).ok()?;
    let yi = i32::try_from(y).ok()?;
    Some(TileCoord::new(xi, yi))
}

/// Tipo semántico de una tesela.
///
/// Cubre los tipos de `TileType` de `OpenTTD` necesarios para el renderer.
/// Los tipos sin sprite dedicado se renderizan con un color de fallback.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum TileKind {
    #[default]
    Grass,
    Water,
    Forest,
    CoalField,
    Road,
    Rail,
    House,       // MP_HOUSE  (3) – edificio urbano
    Station,     // MP_STATION (5)
    Industry,    // MP_INDUSTRY (8) – genérico (sin sub-tipo conocido)
    Void,        // MP_VOID (7) – borde del mapa
    Unknown(u8), // cualquier tipo no mapeado (raw nibble alto de tile_type)
}

/// Una tesela con altura base, tipo semántico y bytes auxiliares de `OpenTTD`.
///
/// Datos de mapa OpenTTD para una tesela.
///
/// | Campo | Fuente     | Uso principal |
/// |-------|-----------|---------------|
/// | `m5`  | MAP5 (`MAP5`)  | Road bits (0-3), TrackBits (0-5), gfx industria (0-7), ObjectType (MP_OBJECT) |
/// | `m1`  | MAP1 (chunk `MAPO`)  | Owner/índice de industria |
/// | `m6`  | MAP6 (`MAPE`)  | bit 2 = bit 8 del gfx de industria (9 bits totales); StationType en MP_STATION |
/// | `m8`  | MAP8 (`MAP8`)  | HouseID en MP_HOUSE (12 bits); RoadType tram en bits 6–11 en MP_ROAD (`road_map.h`) |
/// | `m3`  | M3LO (byte bajo de `m3`) | v4+: bits 0–3 = tram track bits en carretera normal; 4–7 = owner tranvía |
/// | `m2`  | MAP2 | v5+: índice town/station/industry según tipo de tesela |
/// | `m7`  | MAP7 | v5+: reserva cruces, NewGRF en mapa, etc. |
/// | `m3hi` | M3HI | v5+: byte **`m4()`** del mapa OpenTTD (`M3HI` en `map_sl.cpp`; señales: `GetSignalStates` en nibble alto) |
/// | `m2_hi` | MAP2 hi | v5+12: byte alto de **`m2()`** 16-bit por tesela (reserva PBS en bits altos del save) |
///
/// Para `MP_RAILWAY`, TrackBits ocupa **bits 0-5** de m5 (6 bits); bits 6-7 son `RailTileType`.
/// Para `MP_INDUSTRY`, gfx = `m5 | ((m6 >> 2) & 1) << 8` (9 bits).
/// Para `MP_OBJECT`, m5 contiene el `ObjectType` (precomputado por `parse_sav.py` desde OBJS).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Tile {
    pub height: u8,
    pub kind: TileKind,
    /// Byte MAPT del savegame (nibble alto = `TileType` `OpenTTD`). 0 en mapas generados.
    pub mapt: u8,
    pub m5: u8,
    /// Byte M1 del savegame. Para industrias: bits 0-6 = índice de industria.
    pub m1: u8,
    /// Byte M6 del savegame. bit 2 = bit 8 del gfx de industria.
    pub m6: u8,
    /// Bytes M8 del savegame (little-endian, 2 bytes). HouseID en MP_HOUSE.
    pub m8: u16,
    /// Byte M3LO del savegame (`.ottdmap` v4+). `0` si el archivo no incluye la sección.
    pub m3: u8,
    /// Byte bajo de MAP2 (`.ottdmap` v5+); en el save OpenTTD `m2()` es `u16` LE.
    pub m2: u8,
    /// Byte alto de MAP2 (`.ottdmap` v5+12); `0` si el archivo no incluye el plano extra.
    pub m2_hi: u8,
    /// Byte MAP7 (`.ottdmap` v5+).
    pub m7: u8,
    /// Byte M3HI = **`m4()`** en OpenTTD (`.ottdmap` v5+).
    pub m3hi: u8,
}

impl Tile {
    /// Nibble alto del tipo de tesela OpenTTD (`mapt >> 4`).
    #[must_use]
    pub fn ottd_type_nibble(self) -> u8 {
        (self.mapt >> 4) & 0x0F
    }

    /// `true` si MAPT indica `MP_TUNNELBRIDGE`.
    #[must_use]
    pub fn is_tunnel_bridge_tile(self) -> bool {
        self.ottd_type_nibble() == OTTD_TILETYPE_TUNNELBRIDGE
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapError {
    OutOfBounds,
}

pub(crate) const OTTDMAP_MAGIC_VERSIONED: &[u8; 4] = b"MAP1";
pub(crate) const OTTDMAP_HEADER_LEN_VERSIONED: usize = 16;
pub const OTTDMAP_FORMAT_VERSION_CURRENT: u16 = 1;
pub const OTTDMAP_FLAG_HAS_M2_HI: u16 = 1 << 0;

#[derive(Debug, Clone, Copy)]
struct OttdmapHeader {
    dense_offset: usize,
    width: u32,
    height: u32,
    format_version: u16,
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
        format_version,
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
        (&data[dense_offset + 4 * n..dense_offset + 5 * n], dense_offset + 5 * n)
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
    match ottd_type {
        0 | 10 => TileKind::Grass,
        1 => TileKind::Rail,
        2 => TileKind::Road,
        3 => TileKind::House,
        4 => TileKind::Forest,
        5 => TileKind::Station,
        6 => TileKind::Water,
        7 => TileKind::Void,
        8 => TileKind::Industry,
        9 => {
            if m5 & 0x04 != 0 {
                TileKind::Rail
            } else {
                TileKind::Road
            }
        }
        t => TileKind::Unknown(t),
    }
}

/// Mapa rectangular denso en memoria.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Map {
    width: u32,
    height: u32,
    tiles: Vec<Tile>,
}

impl Map {
    /// Crea un mapa plano con la misma altura en todas las teselas.
    ///
    /// # Panics
    ///
    /// Si `width * height` desborda `u32` o no cabe en `usize` (caso atípico en 64 bits).
    #[must_use]
    pub fn new_flat(width: u32, height: u32, level: u8) -> Self {
        let len = width.checked_mul(height).expect("width*height overflow");
        let count = usize::try_from(len).expect("map tile count must fit usize");
        Self {
            width,
            height,
            tiles: vec![
                Tile {
                    height: level,
                    kind: TileKind::Grass,
                    mapt: 0,
                    m5: 0,
                    m1: 0,
                    m6: 0,
                    m8: 0,
                    m3: 0,
                    m2: 0,
                    m2_hi: 0,
                    m7: 0,
                    m3hi: 0,
                };
                count
            ],
        }
    }

    #[must_use]
    pub const fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// Cuenta extremos JGR (`tile_n` / `tile_s`) que caen en teselas `MP_TUNNELBRIDGE` del mapa.
    ///
    /// Devuelve `(coincidencias_norte, coincidencias_sur, total_registros)`.
    #[must_use]
    pub fn jgr_tunnel_endpoint_match_stats(
        &self,
        tunnels: &[crate::tnbp_decode::JgrTunnelRecord],
    ) -> (usize, usize, usize) {
        let w = self.width;
        let h = self.height;
        let mut n_ok = 0usize;
        let mut s_ok = 0usize;
        for t in tunnels {
            if let Some(c) = openttd_tile_index_to_coord(t.tile_n, w, h)
                && self.get(c).is_some_and(Tile::is_tunnel_bridge_tile)
            {
                n_ok += 1;
            }
            if let Some(c) = openttd_tile_index_to_coord(t.tile_s, w, h)
                && self.get(c).is_some_and(Tile::is_tunnel_bridge_tile)
            {
                s_ok += 1;
            }
        }
        (n_ok, s_ok, tunnels.len())
    }

    fn index(&self, c: TileCoord) -> Option<usize> {
        if c.x < 0 || c.y < 0 {
            return None;
        }
        let ux = u32::try_from(c.x).ok()?;
        let uy = u32::try_from(c.y).ok()?;
        if ux >= self.width || uy >= self.height {
            return None;
        }
        Some(usize::try_from(uy * self.width + ux).unwrap())
    }

    #[must_use]
    pub fn get(&self, c: TileCoord) -> Option<Tile> {
        let i = self.index(c)?;
        self.tiles.get(i).copied()
    }

    pub fn set_height(&mut self, c: TileCoord, height: u8) -> Result<(), MapError> {
        let i = self.index(c).ok_or(MapError::OutOfBounds)?;
        self.tiles[i].height = height;
        Ok(())
    }

    pub fn set_kind(&mut self, c: TileCoord, kind: TileKind) -> Result<(), MapError> {
        let i = self.index(c).ok_or(MapError::OutOfBounds)?;
        self.tiles[i].kind = kind;
        Ok(())
    }

    #[must_use]
    pub fn get_kind(&self, c: TileCoord) -> Option<TileKind> {
        let i = self.index(c)?;
        Some(self.tiles[i].kind)
    }

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
    /// | 9        | `MP_TUNNELBRIDGE`| Road/Rail        |
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
        let _version = header.format_version;
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

#[cfg(test)]
mod ottdmap_binary_tests {
    use super::*;

    fn push_map1_header(v: &mut Vec<u8>, w: u32, h: u32) {
        v.extend_from_slice(OTTDMAP_MAGIC_VERSIONED);
        v.extend_from_slice(&w.to_le_bytes());
        v.extend_from_slice(&h.to_le_bytes());
        v.extend_from_slice(&OTTDMAP_FORMAT_VERSION_CURRENT.to_le_bytes());
        v.extend_from_slice(&OTTDMAP_FLAG_HAS_M2_HI.to_le_bytes());
    }

    fn build_ottdmap_2x2(
        mapt: [u8; 4],
        heights: [u8; 4],
        m1: [u8; 4],
        m2: [u8; 4],
        m2_hi: [u8; 4],
        m3: [u8; 4],
        m3hi: [u8; 4],
        m5: [u8; 4],
        m6: [u8; 4],
        m7: [u8; 4],
        m8: [u16; 4],
    ) -> Vec<u8> {
        let mut v = Vec::with_capacity(16 + 12 * 4);
        push_map1_header(&mut v, 2, 2);
        v.extend_from_slice(&mapt);
        v.extend_from_slice(&heights);
        v.extend_from_slice(&m1);
        v.extend_from_slice(&m2);
        v.extend_from_slice(&m2_hi);
        v.extend_from_slice(&m3);
        v.extend_from_slice(&m3hi);
        v.extend_from_slice(&m5);
        v.extend_from_slice(&m6);
        v.extend_from_slice(&m7);
        for x in m8 {
            v.extend_from_slice(&x.to_le_bytes());
        }
        v
    }

    /// Mapa binario 2×2 con una tesela casa y `m8 = 42` en el origen.
    fn minimal_ottdmap_v1() -> Vec<u8> {
        build_ottdmap_2x2(
            [0x30, 0x00, 0x00, 0x00], // MAPT: tesela 0 = MP_HOUSE
            [1, 1, 1, 1],             // MAPH
            [0; 4],                   // m1
            [0; 4],                   // m2
            [0; 4],                   // m2_hi
            [0; 4],                   // m3
            [0; 4],                   // m3hi
            [0; 4],                   // m5
            [0; 4],                   // m6
            [0; 4],                   // m7
            [42, 0, 0, 0],            // m8
        )
    }

    fn minimal_ottdmap_with_m3() -> Vec<u8> {
        build_ottdmap_2x2(
            [0x30, 0x00, 0x00, 0x00],
            [1, 1, 1, 1],
            [0; 4],
            [0; 4],
            [0; 4],
            [0xAB, 0, 0, 0], // m3
            [0; 4],
            [0; 4],
            [0; 4],
            [0; 4],
            [42, 0, 0, 0],
        )
    }

    /// Formato v1 completo + footer INDP ficticio (ignorado por `from_ottd_binary`).
    fn minimal_ottdmap_v5_with_footer() -> Vec<u8> {
        let mut v = build_ottdmap_2x2(
            [0x30, 0x00, 0x00, 0x00],
            [1, 1, 1, 1],
            [0; 4],
            [0x11, 0, 0, 0], // m2
            [0; 4],          // m2_hi
            [0xAB, 0, 0, 0], // m3
            [0x33, 0, 0, 0], // m3hi
            [0; 4],          // m5
            [0; 4],          // m6
            [0x22, 0, 0, 0], // m7
            [42, 0, 0, 0],   // m8
        );
        v.extend_from_slice(b"INDP");
        v.extend_from_slice(&0_u32.to_le_bytes()); // count = 0
        v
    }

    #[test]
    fn openttd_tile_index_roundtrip_2x2() {
        assert_eq!(
            openttd_tile_index_to_coord(0, 2, 2),
            Some(TileCoord::new(0, 0))
        );
        assert_eq!(
            openttd_tile_index_to_coord(1, 2, 2),
            Some(TileCoord::new(1, 0))
        );
        assert_eq!(
            openttd_tile_index_to_coord(2, 2, 2),
            Some(TileCoord::new(0, 1))
        );
        assert_eq!(
            openttd_tile_index_to_coord(3, 2, 2),
            Some(TileCoord::new(1, 1))
        );
        assert_eq!(openttd_tile_index_to_coord(4, 2, 2), None);
        assert_eq!(openttd_tile_index_to_coord(0, 3, 3), None);
    }

    #[test]
    fn from_ottd_binary_loads_house_m8() {
        let bytes = minimal_ottdmap_v1();
        let map = Map::from_ottd_binary(&bytes).expect("mapa válido");
        assert_eq!(map.dimensions(), (2, 2));
        let t0 = map.get(TileCoord::new(0, 0)).expect("tile");
        assert_eq!(t0.kind, TileKind::House);
        assert_eq!(t0.m8, 42);
        let t1 = map.get(TileCoord::new(1, 0)).expect("tile");
        assert_eq!(t1.kind, TileKind::Grass);
        assert_eq!(t0.m3, 0);
        assert_eq!(t1.m3, 0);
    }

    #[test]
    fn from_ottd_binary_loads_m3_v4() {
        let bytes = minimal_ottdmap_with_m3();
        let map = Map::from_ottd_binary(&bytes).expect("mapa válido");
        let t0 = map.get(TileCoord::new(0, 0)).expect("tile");
        assert_eq!(t0.m3, 0xAB);
        let t1 = map.get(TileCoord::new(1, 0)).expect("tile");
        assert_eq!(t1.m3, 0);
    }

    #[test]
    fn from_ottd_binary_loads_v5_planes_and_ignores_indp_footer() {
        let map = Map::from_ottd_binary(&minimal_ottdmap_v5_with_footer()).expect("mapa válido");
        let t0 = map.get(TileCoord::new(0, 0)).expect("tile");
        assert_eq!(t0.m2, 0x11);
        assert_eq!(t0.m7, 0x22);
        assert_eq!(t0.m3hi, 0x33);
        assert_eq!(t0.m3, 0xAB);
    }

    #[test]
    fn from_ottd_binary_loads_versioned_header() {
        let map = Map::from_ottd_binary(&minimal_ottdmap_v5_with_footer()).expect("mapa válido");
        let t0 = map.get(TileCoord::new(0, 0)).expect("tile");
        assert_eq!(t0.m2, 0x11);
        assert_eq!(t0.m7, 0x22);
        assert_eq!(t0.m3hi, 0x33);
    }

    #[test]
    fn from_ottd_binary_with_extras_reads_indp() {
        let bytes = minimal_ottdmap_v5_with_footer();
        let (map, ex) = Map::from_ottd_binary_with_extras(&bytes).expect("mapa válido");
        assert_eq!(map.dimensions(), (2, 2));
        assert!(ex.industry_types.is_empty());
    }

    #[test]
    fn from_ottd_binary_loads_m2_hi_plane() {
        let v = build_ottdmap_2x2(
            [0x30, 0x00, 0x00, 0x00],
            [1, 1, 1, 1],
            [0; 4],
            [0x11, 0, 0, 0],
            [0xAA, 0, 0, 0xBB], // m2_hi (tesela 3 = 0xBB)
            [0; 4],
            [0; 4],
            [0; 4],
            [0; 4],
            [0; 4],
            [0; 4],
        );
        let map = Map::from_ottd_binary(&v).expect("mapa válido");
        let t3 = map.get(TileCoord::new(1, 1)).expect("tile");
        assert_eq!(t3.m2_hi, 0xBB);
        assert_eq!(t3.m2, 0);
    }

    #[test]
    fn from_ottd_binary_rejects_bad_magic() {
        let mut b = minimal_ottdmap_v1();
        b[0] = b'X';
        assert!(Map::from_ottd_binary(&b).is_err());
    }

    #[test]
    fn from_ottd_binary_rejects_legacy_mapo_header() {
        let mut b = minimal_ottdmap_v1();
        b[0..4].copy_from_slice(b"MAPO");
        assert!(Map::from_ottd_binary(&b).is_err());
    }

    /// `.ottdmap` v1: una tesela `MP_WATER` Coast (`m5 = 0x10` en bits 4–7).
    fn minimal_ottdmap_water_coast_v1() -> Vec<u8> {
        let mut v = Vec::with_capacity(16 + 12);
        push_map1_header(&mut v, 1, 1);
        v.push(0x60); // MAPT nibble alto 6 = MP_WATER
        v.push(3); // MAPH
        v.push(0); // m1
        v.push(0); // m2
        v.push(0); // m2_hi
        v.push(0); // m3
        v.push(0); // m3hi
        v.push(0x10); // m5: Coast
        v.push(0); // m6
        v.push(0); // m7
        v.extend_from_slice(&0u16.to_le_bytes()); // m8
        v
    }

    /// 2×2 v1: hierba + agua Clear + agua Coast (comprueba que `m5` no se pierde por celda).
    fn minimal_ottdmap_mixed_water_v1() -> Vec<u8> {
        build_ottdmap_2x2(
            [0x00, 0x60, 0x60, 0x60], // MAPT
            [4, 1, 1, 1],             // MAPH
            [0; 4],
            [0; 4],
            [0; 4],
            [0; 4],
            [0; 4],
            [0, 0, 0x10, 0], // m5: Clear agua, Coast, Clear agua
            [0; 4],
            [0; 4],
            [0; 4],
        )
    }

    #[test]
    fn from_ottd_binary_preserves_water_coast_m5() {
        let map = Map::from_ottd_binary(&minimal_ottdmap_water_coast_v1()).expect("mapa válido");
        assert_eq!(map.dimensions(), (1, 1));
        let t = map.get(TileCoord::new(0, 0)).expect("tile");
        assert_eq!(t.kind, TileKind::Water);
        assert_eq!(t.m5, 0x10, "WaterTileType::Coast en bits 4–7");
        assert_eq!((t.m5 >> 4) & 0x0F, 1);
    }

    #[test]
    fn from_ottd_binary_mixed_water_m5_per_tile() {
        let map = Map::from_ottd_binary(&minimal_ottdmap_mixed_water_v1()).expect("mapa válido");
        assert_eq!(map.dimensions(), (2, 2));
        let clear_land = map.get(TileCoord::new(0, 0)).expect("tile");
        assert_eq!(clear_land.kind, TileKind::Grass);
        assert_eq!(clear_land.m5, 0);

        let sea_clear = map.get(TileCoord::new(1, 0)).expect("tile");
        assert_eq!(sea_clear.kind, TileKind::Water);
        assert_eq!(sea_clear.m5, 0);
        assert_eq!((sea_clear.m5 >> 4) & 0x0F, 0, "Clear");

        let coast = map.get(TileCoord::new(0, 1)).expect("tile");
        assert_eq!(coast.kind, TileKind::Water);
        assert_eq!(coast.m5, 0x10);
        assert_eq!((coast.m5 >> 4) & 0x0F, 1, "Coast");

        let sea2 = map.get(TileCoord::new(1, 1)).expect("tile");
        assert_eq!(sea2.kind, TileKind::Water);
        assert_eq!(sea2.m5, 0);
    }
}
