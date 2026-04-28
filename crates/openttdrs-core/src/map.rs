//! Estructura del mapa y carga de `.ottdmap` (v2/v3).
#![allow(clippy::doc_markdown, clippy::expect_used, clippy::unwrap_used)]

/// Coordenada de tesela en el plano X/Y del mapa (análoga a índices de tesela en `OpenTTD`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
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
/// | `m8`  | MAP8 (`MAP8`)  | HouseID en MP_HOUSE (12 bits); dato extra para NewGRF |
///
/// Para `MP_RAILWAY`, TrackBits ocupa **bits 0-5** de m5 (6 bits); bits 6-7 son `RailTileType`.
/// Para `MP_INDUSTRY`, gfx = `m5 | ((m6 >> 2) & 1) << 8` (9 bits).
/// Para `MP_OBJECT`, m5 contiene el `ObjectType` (precomputado por `parse_sav.py` desde OBJS).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapError {
    OutOfBounds,
}

/// Mapa rectangular denso en memoria.
#[derive(Debug, Clone)]
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
    /// Formato binario `.ottdmap` v3:
    ///
    /// - 4 bytes: magic `MAPO`
    /// - 4 bytes LE: width
    /// - 4 bytes LE: height
    /// - W×H bytes: `tile_type` (nibble alto = `TileType` `OpenTTD`)
    /// - W×H bytes: height por tesela
    /// - W×H bytes: m5 (road bits, TrackBits, gfx industria bajo, ObjectType)
    /// - W×H bytes: m1 (owner, índice de industria)  [v2+]
    /// - W×H bytes: m6 (bit 2 = bit 8 del gfx industria; StationType)  [v3+]
    /// - W×H×2 bytes: m8 LE (HouseID en MP_HOUSE; datos NewGRF)  [v3+]
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
        if data.len() < 12 + n * 3 {
            return Err(MapError::OutOfBounds);
        }

        let tile_types = &data[12..12 + n];
        let heights = &data[12 + n..12 + 2 * n];
        let m5_data = &data[12 + 2 * n..12 + 3 * n];
        // v2+: m1 después de m5
        let has_m1 = data.len() >= 12 + n * 4;
        let m1_data = if has_m1 {
            &data[12 + 3 * n..12 + 4 * n]
        } else {
            &[] as &[u8]
        };
        // v3+: m6 después de m1, luego m8 (2 bytes/tile LE)
        let has_m6 = data.len() >= 12 + n * 5;
        let m6_data = if has_m6 {
            &data[12 + 4 * n..12 + 5 * n]
        } else {
            &[] as &[u8]
        };
        let has_m8 = data.len() >= 12 + n * 5 + n * 2;
        let m8_data = if has_m8 {
            &data[12 + 5 * n..12 + 5 * n + n * 2]
        } else {
            &[] as &[u8]
        };

        let mut tiles = Vec::with_capacity(n);
        for i in 0..n {
            let raw_type = tile_types[i];
            let ottd_type = (raw_type >> 4) & 0xF;
            let m5 = m5_data[i];
            let m1 = if has_m1 { m1_data[i] } else { 0 };
            let m6 = if has_m6 { m6_data[i] } else { 0 };
            let m8 = if has_m8 {
                u16::from_le_bytes([m8_data[i * 2], m8_data[i * 2 + 1]])
            } else {
                0
            };

            let kind = match ottd_type {
                0 | 10 => TileKind::Grass, // MP_CLEAR, MP_OBJECT
                1 => TileKind::Rail,       // MP_RAILWAY
                2 => TileKind::Road,       // MP_ROAD
                3 => TileKind::House,      // MP_HOUSE
                4 => TileKind::Forest,     // MP_TREES
                5 => TileKind::Station,    // MP_STATION
                6 => TileKind::Water,      // MP_WATER
                7 => TileKind::Void,       // MP_VOID
                8 => TileKind::Industry,   // MP_INDUSTRY
                9 => {
                    // MP_TUNNELBRIDGE: Road o Rail según m5 bit 2
                    if m5 & 0x04 != 0 {
                        TileKind::Rail
                    } else {
                        TileKind::Road
                    }
                }
                t => TileKind::Unknown(t),
            };

            tiles.push(Tile {
                height: heights[i],
                kind,
                mapt: raw_type,
                m5,
                m1,
                m6,
                m8,
            });
        }

        Ok(Self {
            width,
            height,
            tiles,
        })
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
    }

    #[test]
    fn from_ottd_binary_rejects_bad_magic() {
        let mut b = minimal_ottdmap_v3();
        b[0] = b'X';
        assert!(Map::from_ottd_binary(&b).is_err());
    }
}
