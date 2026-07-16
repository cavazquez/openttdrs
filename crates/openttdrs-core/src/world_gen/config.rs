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

/// Subtipo de suelo en teselas `MP_CLEAR` (bits 2–4 de `m5`).
pub const CLEAR_GROUND_GRASS: u8 = 0;
pub const CLEAR_GROUND_ROUGH: u8 = 1;
pub const CLEAR_GROUND_ROCKY: u8 = 2;
pub const CLEAR_GROUND_SNOW: u8 = 4;
pub const CLEAR_GROUND_DESERT: u8 = 5;

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

/// Suelo inicial al generar mapa (ártico: nieve al norte de la línea estacional).
#[must_use]
pub fn initial_clear_ground(climate: Climate, tx: i32, ty: i32, map_h: i32, seed: u64) -> u8 {
    match climate {
        Climate::SubArctic => {
            let snow_line = map_h * 2 / 5;
            if ty < snow_line {
                CLEAR_GROUND_SNOW
            } else {
                CLEAR_GROUND_GRASS
            }
        }
        _ => effective_clear_ground(climate, 0, tx, ty, seed),
    }
}

/// Parámetros de generación procedural.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorldGenConfig {
    pub climate: Climate,
    pub seed: u64,
    /// Altura máxima de agua (`GetTileZ` ≤ `sea_level` → `MP_WATER`).
    pub sea_level: u8,
    /// Bordes del mapa más bajos → costas / isla jugable.
    pub island: bool,
    /// Amplitud de relieve (`n * height_span` sobre el nivel del mar); típico 3/6/10.
    pub height_span: u8,
}

impl Default for WorldGenConfig {
    fn default() -> Self {
        Self {
            climate: Climate::Temperate,
            seed: 0,
            sea_level: 1,
            island: false,
            height_span: 6,
        }
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
