//! Configuración de generación procedural: clima, parámetros y helpers de suelo.

/// Clima del mundo (`LandscapeType` en `OpenTTD`).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum Climate {
    #[default]
    Temperate,
    SubArctic,
    SubTropical,
    Toyland,
}

impl Climate {
    /// Valor `LandscapeType` que consumen las condiciones `NewGRF` Action7/9.
    #[must_use]
    pub const fn newgrf_landscape_id(self) -> u8 {
        match self {
            Self::Temperate => 0,
            Self::SubArctic => 1,
            Self::SubTropical => 2,
            Self::Toyland => 3,
        }
    }

    /// Bit usado por `EngineInfo::climates` / Action0 prop `0x06`.
    #[must_use]
    pub const fn newgrf_landscape_bit(self) -> u8 {
        1 << self.newgrf_landscape_id()
    }

    /// Parsea nombres usados en `OPENTTDRS_CLIMATE` y saves JSON.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "temperate" | "temp" | "temperate_landscape" => Some(Self::Temperate),
            "arctic" | "sub_arctic" | "subarctic" | "snow" => Some(Self::SubArctic),
            "tropic" | "sub_tropical" | "subtropical" | "desert" => Some(Self::SubTropical),
            "toyland" | "toy" => Some(Self::Toyland),
            _ => None,
        }
    }

    #[must_use]
    pub const fn uses_snow_ground(self) -> bool {
        matches!(self, Self::SubArctic)
    }

    #[must_use]
    pub const fn uses_desert_patches(self) -> bool {
        matches!(self, Self::SubTropical)
    }
}

/// Tipo de relieve TGP (`GenworldMaxHeight` en `OpenTTD`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum TerrainType {
    VeryFlat = 0,
    #[default]
    Flat = 1,
    Hilly = 2,
    Mountainous = 3,
    Alpinist = 4,
}

impl TerrainType {
    #[must_use]
    pub const fn as_index(self) -> usize {
        self as u8 as usize
    }

    /// Mapea el legado `height_span` (3/6/10 del cliente) a tipo TGP.
    #[must_use]
    pub const fn from_height_span(span: u8) -> Self {
        match span {
            0..=4 => Self::Flat,
            5..=8 => Self::Hilly,
            9..=12 => Self::Mountainous,
            _ => Self::Alpinist,
        }
    }

    /// `height_span` aproximado para APIs que aún lo exponen.
    #[must_use]
    pub const fn to_height_span(self) -> u8 {
        match self {
            Self::VeryFlat => 3,
            Self::Flat => 5,
            Self::Hilly => 6,
            Self::Mountainous => 10,
            Self::Alpinist => 12,
        }
    }
}

/// Cantidad de mar/lagos (`quantity_sea_lakes`; 100 % ≡ 1024 en TGP).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum QuantitySeaLakes {
    #[default]
    VeryLow = 0,
    Low = 1,
    Medium = 2,
    High = 3,
}

impl QuantitySeaLakes {
    #[must_use]
    pub const fn as_index(self) -> usize {
        self as u8 as usize
    }

    /// Porcentaje fijo de agua ×1024 (`_water_percent` en `tgp.cpp`).
    #[must_use]
    pub const fn water_percent_x1024(self) -> i64 {
        [70, 170, 270, 420][self.as_index()]
    }
}

/// Suavidad del generador Perlin (`tgen_smoothness` 0..=3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum TgenSmoothness {
    VerySmooth = 0,
    #[default]
    Smooth = 1,
    Rough = 2,
    VeryRough = 3,
}

impl TgenSmoothness {
    #[must_use]
    pub const fn as_index(self) -> usize {
        self as u8 as usize
    }

    #[must_use]
    pub fn from_u8(v: u8) -> Self {
        match v.min(3) {
            0 => Self::VerySmooth,
            1 => Self::Smooth,
            2 => Self::Rough,
            _ => Self::VeryRough,
        }
    }
}

/// Subtipo de suelo en teselas `MP_CLEAR` (bits 2–4 de `m5`).
pub const CLEAR_GROUND_GRASS: u8 = 0;
pub const CLEAR_GROUND_ROUGH: u8 = 1;
pub const CLEAR_GROUND_ROCKY: u8 = 2;
/// Campo de granja (`CLEAR_FIELDS` en `clear_map.h`).
pub const CLEAR_GROUND_FIELDS: u8 = 3;
pub const CLEAR_GROUND_SNOW: u8 = 4;
pub const CLEAR_GROUND_DESERT: u8 = 5;

/// Altura de línea de nieve por defecto (`DEF_SNOWLINE_HEIGHT` / `snow_line_height` en `OpenTTD`).
pub const DEF_SNOW_LINE_HEIGHT: u8 = 10;

/// Cobertura de nieve por defecto (`DEF_SNOW_COVERAGE`).
pub const DEF_SNOW_COVERAGE: u8 = 40;

/// Cobertura de desierto por defecto (`DEF_DESERT_COVERAGE`).
pub const DEF_DESERT_COVERAGE: u8 = 50;

/// Empaqueta `ClearGround` + densidad de hierba en `m5`.
#[must_use]
pub const fn clear_ground_m5(ground: u8, density: u8) -> u8 {
    ((ground & 7) << 2) | (density & 3)
}

/// Resuelve el suelo visible según clima y datos de tesela (para render / gen).
///
/// En ártico la nieve vive en `m5` (`CLEAR_GROUND_SNOW`); no se fuerza aquí para
/// permitir deshielo estacional (`apply_seasonal_snow`).
#[must_use]
pub fn effective_clear_ground(climate: Climate, tile_m5: u8, tx: i32, ty: i32, seed: u64) -> u8 {
    use super::landcover::desert_patch;
    let explicit = (tile_m5 >> 2) & 0x7;
    if explicit != CLEAR_GROUND_GRASS {
        return explicit;
    }
    match climate {
        Climate::SubTropical if desert_patch(tx, ty, seed) => CLEAR_GROUND_DESERT,
        Climate::SubArctic | Climate::Temperate | Climate::SubTropical => CLEAR_GROUND_GRASS,
        Climate::Toyland => CLEAR_GROUND_ROUGH,
    }
}

/// Suelo inicial al generar mapa (ártico: nieve si `tile_z` ≥ línea de nieve, como `OpenTTD`).
#[must_use]
pub fn initial_clear_ground(climate: Climate, tx: i32, ty: i32, tile_z: u8, seed: u64) -> u8 {
    initial_clear_ground_with_lines(climate, tx, ty, tile_z, seed, DEF_SNOW_LINE_HEIGHT, None)
}

/// Como [`initial_clear_ground`], con línea de nieve/desierto calculada por cobertura TGP.
#[must_use]
pub fn initial_clear_ground_with_lines(
    climate: Climate,
    tx: i32,
    ty: i32,
    tile_z: u8,
    seed: u64,
    snow_line: u8,
    desert_line: Option<u8>,
) -> u8 {
    match climate {
        Climate::SubArctic => {
            // `k = z - snow_line + 1 >= 0` ⇒ `z + 1 >= snow_line`.
            if i32::from(tile_z) + 1 >= i32::from(snow_line) {
                CLEAR_GROUND_SNOW
            } else {
                CLEAR_GROUND_GRASS
            }
        }
        Climate::SubTropical => {
            if let Some(line) = desert_line
                && tile_z > 0
                && tile_z < line
            {
                return CLEAR_GROUND_DESERT;
            }
            effective_clear_ground(climate, 0, tx, ty, seed)
        }
        _ => effective_clear_ground(climate, 0, tx, ty, seed),
    }
}

/// Parámetros de generación procedural (alineados con dificultad / `game_creation` de `OpenTTD`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorldGenConfig {
    pub climate: Climate,
    pub seed: u64,
    /// Altura máxima de agua para heightmaps (`GetTileZ` ≤ `sea_level` → `MP_WATER`).
    /// En TGP el mar queda en altura 0 tras normalizar.
    pub sea_level: u8,
    /// Bordes del mapa con costa esculpida (modo isla / `water_borders` = todos).
    pub island: bool,
    /// Máscara explícita de `water_borders` de `OpenTTD`.
    ///
    /// `None` conserva la semántica histórica de `island` (todos o ningún
    /// borde). Cuando se especifica, los cuatro bits bajos representan
    /// `NE|SE|SW|NW`; el valor `0x10` solicita el modo `Random` de `OpenTTD` y
    /// consume el siguiente número del RNG de generación.
    pub water_borders: Option<u8>,
    /// Amplitud legado (cliente 3/6/10); se sincroniza con [`Self::terrain_type`].
    pub height_span: u8,
    /// Relieve TGP (`difficulty.terrain_type`).
    pub terrain_type: TerrainType,
    /// Mar/lagos TGP (`difficulty.quantity_sea_lakes`).
    pub quantity_sea_lakes: QuantitySeaLakes,
    /// Suavidad Perlin (`game_creation.tgen_smoothness`).
    pub tgen_smoothness: TgenSmoothness,
    /// Variedad de curvas TGP 0..=5 (`game_creation.variety`); 0 desactiva.
    pub variety: u8,
    /// % de tierra con nieve en ártico (`game_creation.snow_coverage`).
    pub snow_coverage: u8,
    /// % de tierra con desierto en trópico (`game_creation.desert_coverage`).
    pub desert_coverage: u8,
    /// Cantidad de ríos (`game_creation.amount_of_rivers`, 0..=3).
    pub amount_of_rivers: u8,
    /// Longitud mínima de los ríos cortos (`game_creation.min_river_length`).
    ///
    /// Los pozos largos multiplican este valor por cuatro, igual que
    /// `CreateRivers`. No se debe sustituir por la cantidad de teselas ya
    /// pintadas: el contrato mide la distancia Manhattan entre manantial y
    /// terminación.
    pub min_river_length: u8,
    /// Aleatoriedad del coste de ruta de río (`game_creation.river_route_random`).
    ///
    /// Se conserva en la configuración aunque el port de YAPF de ríos todavía
    /// está en RMAP-018; así los valores por defecto y los save/settings no
    /// quedan implícitos en el generador.
    pub river_route_random: u8,
    /// Cantidad de sorteos consumidos antes de TGP, como una partida nueva de
    /// `OpenTTD` (`StartupEconomy` consume uno). Se deja en cero por defecto
    /// para conservar la API histórica del generador embebido.
    pub startup_rng_draws: u8,
}

impl Default for WorldGenConfig {
    fn default() -> Self {
        Self {
            climate: Climate::Temperate,
            seed: 0,
            sea_level: 1,
            island: false,
            water_borders: None,
            height_span: TerrainType::Hilly.to_height_span(),
            terrain_type: TerrainType::Hilly,
            quantity_sea_lakes: QuantitySeaLakes::VeryLow,
            tgen_smoothness: TgenSmoothness::Smooth,
            variety: 0,
            snow_coverage: DEF_SNOW_COVERAGE,
            desert_coverage: DEF_DESERT_COVERAGE,
            amount_of_rivers: 2,
            min_river_length: 16,
            river_route_random: 5,
            startup_rng_draws: 0,
        }
    }
}

impl WorldGenConfig {
    /// Sincroniza `terrain_type` ↔ `height_span` a partir del span legado.
    #[must_use]
    pub const fn with_height_span(mut self, span: u8) -> Self {
        self.height_span = span;
        self.terrain_type = TerrainType::from_height_span(span);
        self
    }

    /// Fija el relieve TGP y actualiza `height_span` legado.
    #[must_use]
    pub const fn with_terrain_type(mut self, terrain: TerrainType) -> Self {
        self.terrain_type = terrain;
        self.height_span = terrain.to_height_span();
        self
    }
}

/// Rectángulo inclusivo `(x0, y0, x1, y1)` que no se modifica (zonas demo del cliente).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreserveRect {
    pub x0: i32,
    pub y0: i32,
    pub x1: i32,
    pub y1: i32,
}

impl PreserveRect {
    #[must_use]
    pub const fn contains(self, x: i32, y: i32) -> bool {
        x >= self.x0 && x <= self.x1 && y >= self.y0 && y <= self.y1
    }
}
