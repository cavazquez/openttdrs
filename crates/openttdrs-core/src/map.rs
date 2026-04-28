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

/// Una tesela con altura base, tipo semántico y byte auxiliar m5 de `OpenTTD`.
///
/// `m5` almacena el byte m5 del savegame original:
/// - Para `Road`: bits 0-3 = road bits (NW=1, SW=2, SE=4, NE=8), bits 6-7 = sub-tipo.
/// - Para `Rail`: bits 0-3 = trackbits.
/// - En mapas generados (no cargados desde .sav) vale 0.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tile {
    pub height: u8,
    pub kind: TileKind,
    /// Byte MAPT del savegame (nibble alto = `TileType` `OpenTTD`). 0 en mapas generados.
    pub mapt: u8,
    pub m5: u8,
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
                    m5: 0
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
    /// - 4 bytes: magic `MAPO`
    /// - 4 bytes LE: width
    /// - 4 bytes LE: height
    /// - W×H bytes: `tile_type` (nibble alto = `TileType` `OpenTTD`)
    /// - W×H bytes: height por tesela
    /// - W×H bytes: m5 (road bits, etc.)
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

        let mut tiles = Vec::with_capacity(n);
        for i in 0..n {
            let raw_type = tile_types[i];
            let ottd_type = (raw_type >> 4) & 0xF;
            let m5 = m5_data[i];

            let kind = match ottd_type {
                0 | 10 => TileKind::Grass, // MP_CLEAR, MP_OBJECT
                1 => TileKind::Rail,       // MP_RAILWAY
                2 => TileKind::Road,       // MP_ROAD
                3 => TileKind::House,      // MP_HOUSE
                4 => TileKind::Forest,     // MP_TREES
                5 => TileKind::Station,    // MP_STATION
                6 => TileKind::Water,      // MP_WATER
                7 => TileKind::Void,       // MP_VOID
                8 => {
                    // MP_INDUSTRY: el tipo exacto está en otros bytes (m1/m5).
                    let _ = m5;
                    TileKind::Industry
                }
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
            });
        }

        Ok(Self {
            width,
            height,
            tiles,
        })
    }
}
