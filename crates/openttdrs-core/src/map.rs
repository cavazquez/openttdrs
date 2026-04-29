//! Estructura del mapa y carga de `.ottdmap` (v2–v5).
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
/// | `m1`  | MAP1 (`MAPO`)  | Owner/índice de industria |
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapError {
    OutOfBounds,
}

/// Rebanadas de los planos densos `.ottdmap` (v1–v5); campos vacíos = versión anterior.
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

fn ottdmap_dense_slices(data: &[u8], n: usize) -> Result<OttdmapDenseSlices<'_>, MapError> {
    if data.len() < 12 + n * 3 {
        return Err(MapError::OutOfBounds);
    }
    let tile_types = &data[12..12 + n];
    let heights = &data[12 + n..12 + 2 * n];
    let m5 = &data[12 + 2 * n..12 + 3 * n];
    let has_m1 = data.len() >= 12 + n * 4;
    let m1 = if has_m1 {
        &data[12 + 3 * n..12 + 4 * n]
    } else {
        &[]
    };
    let has_m6 = data.len() >= 12 + n * 5;
    let m6 = if has_m6 {
        &data[12 + 4 * n..12 + 5 * n]
    } else {
        &[]
    };
    let has_m8 = data.len() >= 12 + n * 5 + n * 2;
    let m8 = if has_m8 {
        &data[12 + 5 * n..12 + 5 * n + n * 2]
    } else {
        &[]
    };
    let m3_base = 12 + 7 * n;
    let has_m3 = data.len() >= m3_base + n;
    let m3 = if has_m3 {
        &data[m3_base..m3_base + n]
    } else {
        &[]
    };
    let v5_base = 12 + 8 * n;
    let has_v5_planes = data.len() >= v5_base + 3 * n;
    let (m2, m7, m3hi) = if has_v5_planes {
        (
            &data[v5_base..v5_base + n],
            &data[v5_base + n..v5_base + 2 * n],
            &data[v5_base + 2 * n..v5_base + 3 * n],
        )
    } else {
        (&[] as &[u8], &[] as &[u8], &[] as &[u8])
    };
    let m2_hi = if data.len() >= v5_base + 4 * n {
        &data[v5_base + 3 * n..v5_base + 4 * n]
    } else {
        &[] as &[u8]
    };
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
    /// Formato binario `.ottdmap` v5 (v2–v4 siguen siendo válidos):
    ///
    /// - 4 bytes: magic `MAPO`
    /// - 4 bytes LE: width
    /// - 4 bytes LE: height
    /// - W×H bytes: `tile_type` (nibble alto = `TileType` `OpenTTD`)
    /// - W×H bytes: height por tesela
    /// - W×H bytes: m5 (road bits, TrackBits, gfx industria bajo, ObjectType)
    /// - W×H bytes: m1 (owner, índice de industria)  [v2+]
    /// - W×H bytes: m6 (bit 2 = bit 8 del gfx industria; StationType)  [v3+]
    /// - W×H×2 bytes: m8 LE (HouseID en MP_HOUSE; RoadType tram en bits 6–11 en MP_ROAD)  [v3+]
    /// - W×H bytes: m3 (M3LO; tram track bits 0–3 en carretera normal)  [v4+]
    /// - W×H bytes: m2 (MAP2)  [v5+]
    /// - W×H bytes: m7 (MAP7)  [v5+]
    /// - W×H bytes: m3hi (M3HI)  [v5+]
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
    /// Devuelve `Err` si el archivo no tiene el magic correcto o está truncado.
    #[allow(clippy::missing_panics_doc)]
    pub fn from_ottd_binary(data: &[u8]) -> Result<Self, MapError> {
        if data.len() < 12 || &data[0..4] != b"MAPO" {
            return Err(MapError::OutOfBounds);
        }
        // Safe: ya verificamos que data.len() >= 12
        let width = u32::from_le_bytes(data[4..8].try_into().expect("checked above"));
        let height = u32::from_le_bytes(data[8..12].try_into().expect("checked above"));
        let n = (width as usize).saturating_mul(height as usize);
        let s = ottdmap_dense_slices(data, n)?;

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

    /// Mapa binario v3 mínimo 2×2 con una tesela casa y `m8 = 42` en el origen.
    fn minimal_ottdmap_v3() -> Vec<u8> {
        let w = 2_u32;
        let h = 2_u32;
        let n = 4_usize;
        let mut v = Vec::with_capacity(12 + n * 7);
        v.extend_from_slice(b"MAPO");
        v.extend_from_slice(&w.to_le_bytes());
        v.extend_from_slice(&h.to_le_bytes());
        // MAPT: tesela 0 = MP_HOUSE (nibble alto 3), resto Clear (0)
        v.extend_from_slice(&[0x30, 0x00, 0x00, 0x00]);
        v.extend_from_slice(&[1, 1, 1, 1]); // heights
        v.extend_from_slice(&[0; 4]); // m5
        v.extend_from_slice(&[0; 4]); // m1
        v.extend_from_slice(&[0; 4]); // m6
        // m8 LE por tesela; tesela 0 = 42
        v.extend_from_slice(&42_u16.to_le_bytes());
        v.extend_from_slice(&0_u16.to_le_bytes());
        v.extend_from_slice(&0_u16.to_le_bytes());
        v.extend_from_slice(&0_u16.to_le_bytes());
        v
    }

    /// v4: igual que ``minimal_ottdmap_v3`` con trailing M3LO (tesela 0 = 0xab).
    fn minimal_ottdmap_v4() -> Vec<u8> {
        let mut v = minimal_ottdmap_v3();
        v.extend_from_slice(&[0xAB, 0, 0, 0]); // M3LO por tesela 2×2
        v
    }

    /// v5: v4 + MAP2 / MAP7 / M3HI (tesela 0 = 0x11, 0x22, 0x33) + footer INDP ficticio (ignorado).
    fn minimal_ottdmap_v5_with_footer() -> Vec<u8> {
        let mut v = minimal_ottdmap_v4();
        v.extend_from_slice(&[0x11, 0, 0, 0]); // m2
        v.extend_from_slice(&[0x22, 0, 0, 0]); // m7
        v.extend_from_slice(&[0x33, 0, 0, 0]); // m3hi
        v.extend_from_slice(b"INDP");
        v.extend_from_slice(&0_u32.to_le_bytes()); // count = 0
        v
    }

    #[test]
    fn from_ottd_binary_loads_house_m8() {
        let bytes = minimal_ottdmap_v3();
        let map = Map::from_ottd_binary(&bytes).expect("mapa válido");
        assert_eq!(map.dimensions(), (2, 2));
        let t0 = map.get(TileCoord::new(0, 0)).expect("tile");
        assert_eq!(t0.kind, TileKind::House);
        assert_eq!(t0.m8, 42);
        let t1 = map.get(TileCoord::new(1, 0)).expect("tile");
        assert_eq!(t1.kind, TileKind::Grass);
        assert_eq!(t0.m3, 0, "v3 sin sección m3");
        assert_eq!(t1.m3, 0);
    }

    #[test]
    fn from_ottd_binary_loads_m3_v4() {
        let bytes = minimal_ottdmap_v4();
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
    fn from_ottd_binary_with_extras_reads_indp() {
        let bytes = minimal_ottdmap_v5_with_footer();
        let (map, ex) = Map::from_ottd_binary_with_extras(&bytes).expect("mapa válido");
        assert_eq!(map.dimensions(), (2, 2));
        assert!(ex.industry_types.is_empty());
    }

    #[test]
    fn from_ottd_binary_loads_m2_hi_plane() {
        let mut v = minimal_ottdmap_v4();
        v.extend_from_slice(&[0x11, 0, 0, 0]); // m2
        v.extend_from_slice(&[0x22, 0, 0, 0]); // m7
        v.extend_from_slice(&[0x33, 0, 0, 0]); // m3hi
        v.extend_from_slice(&[0xAA, 0, 0, 0xBB]); // m2_hi (tesela 3 = 0xBB)
        let map = Map::from_ottd_binary(&v).expect("mapa válido");
        let t3 = map.get(TileCoord::new(1, 1)).expect("tile");
        assert_eq!(t3.m2_hi, 0xBB);
        assert_eq!(t3.m2, 0);
    }

    #[test]
    fn from_ottd_binary_rejects_bad_magic() {
        let mut b = minimal_ottdmap_v3();
        b[0] = b'X';
        assert!(Map::from_ottd_binary(&b).is_err());
    }

    /// `.ottdmap` v1 mínimo (solo MAPT + heights + m5): una tesela `MP_WATER` Coast.
    /// `m5 = 0x10` → bits 4–7 = `WaterTileType::Coast` (`water_map.h` en OpenTTD).
    fn minimal_ottdmap_water_coast_v1() -> Vec<u8> {
        let w = 1_u32;
        let h = 1_u32;
        let mut v = Vec::with_capacity(15);
        v.extend_from_slice(b"MAPO");
        v.extend_from_slice(&w.to_le_bytes());
        v.extend_from_slice(&h.to_le_bytes());
        v.push(0x60); // nibble alto 6 = MP_WATER
        v.push(3); // altura
        v.push(0x10); // Coast
        v
    }

    /// 2×2 v1: hierba + agua Clear + agua Coast (comprueba que `m5` no se pierde por celda).
    fn minimal_ottdmap_mixed_water_v1() -> Vec<u8> {
        let w = 2_u32;
        let h = 2_u32;
        let n = 4_usize;
        let mut v = Vec::with_capacity(12 + n * 3);
        v.extend_from_slice(b"MAPO");
        v.extend_from_slice(&w.to_le_bytes());
        v.extend_from_slice(&h.to_le_bytes());
        // Orden i = y*width + x (x rápido): (0,0),(1,0),(0,1),(1,1)
        v.extend_from_slice(&[
            0x00, 0x60, // fila y=0: Clear, Water
            0x60, 0x60, // fila y=1: Water, Water
        ]);
        v.extend_from_slice(&[4, 1, 1, 1]); // heights
        v.extend_from_slice(&[0, 0, 0x10, 0]); // m5: Clear agua, Coast, Clear agua
        v
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
