//! Teselas de aeropuerto `NewGRF` (`AirportTiles`, feature `0x11`).
//!
//! Paridad MVP con `NEW_AIRPORTTILE_OFFSET` / manager de tiles:
//! slots globales ≥74, `subst_id` vanilla y overrides de gfx &lt;74.

use serde::{Deserialize, Serialize};

/// Primera tesela definida por `NewGRF` (`OpenTTD` `NEW_AIRPORTTILE_OFFSET`).
pub const NEW_AIRPORT_TILE_OFFSET: u16 = 74;
/// Total de slots de tesela de aeropuerto (`OpenTTD` `NUM_AIRPORTTILES`).
pub const NUM_AIRPORT_TILES: u16 = 256;
/// Id inválido.
pub const INVALID_AIRPORT_TILE: u16 = NUM_AIRPORT_TILES;

/// Identificador global de gfx de tesela de aeropuerto.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AirportTileGfxId(pub u16);

impl AirportTileGfxId {
    #[must_use]
    pub const fn as_u16(self) -> u16 {
        self.0
    }
}

/// Spec `NewGRF` de una tesela de aeropuerto (simplificado).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AirportTileSpecDef {
    /// Gfx global (≥ [`NEW_AIRPORT_TILE_OFFSET`] si `from_newgrf`).
    pub gfx: AirportTileGfxId,
    /// Fallback vanilla (`subst_id` &lt; 74).
    pub subst_id: u16,
    pub from_newgrf: bool,
    /// Callback mask (`prop 0x0E`); almacenado, sin ejecutar (#228).
    #[serde(default)]
    pub callback_mask: u8,
    /// Id local Action0/3 en el GRF.
    #[serde(default, skip)]
    pub newgrf_local_id: u8,
    /// GRFID del set.
    #[serde(default, skip)]
    pub newgrf_grfid: u32,
    #[serde(default, skip)]
    pub newgrf_preview: Option<crate::newgrf_sprites::DecodedSprite>,
    #[serde(default, skip)]
    pub newgrf_views: Vec<crate::newgrf_sprites::DecodedSprite>,
    #[serde(default, skip)]
    pub newgrf_runtime: Option<Box<crate::newgrf_sprites::TrainSpriteGraphics>>,
}

impl AirportTileSpecDef {
    #[must_use]
    pub fn newgrf_view(&self, idx: usize) -> Option<&crate::newgrf_sprites::DecodedSprite> {
        if self.newgrf_views.is_empty() {
            return self.newgrf_preview.as_ref();
        }
        self.newgrf_views.get(idx % self.newgrf_views.len())
    }

    #[must_use]
    pub fn has_newgrf_sprites(&self) -> bool {
        !self.newgrf_views.is_empty()
            || self.newgrf_preview.is_some()
            || self.newgrf_runtime.is_some()
    }
}

/// Tabla de overrides vanilla → gfx `NewGRF`.
#[must_use]
pub fn empty_airport_tile_overrides() -> Vec<u16> {
    vec![INVALID_AIRPORT_TILE; NEW_AIRPORT_TILE_OFFSET as usize]
}

/// Traduce gfx limpio aplicando override `NewGRF` (si hay).
#[must_use]
pub fn get_translated_airport_tile_id(clean: u16, overrides: &[u16]) -> u16 {
    if (clean as usize) < overrides.len() {
        let ovr = overrides[clean as usize];
        if ovr != INVALID_AIRPORT_TILE {
            return ovr;
        }
    }
    clean
}

/// Siguiente gfx libre ≥ [`NEW_AIRPORT_TILE_OFFSET`].
#[must_use]
pub fn next_free_airport_tile_gfx_id(catalog: &[AirportTileSpecDef]) -> Option<u16> {
    let mut used: Vec<u16> = catalog.iter().map(|d| d.gfx.as_u16()).collect();
    used.sort_unstable();
    used.dedup();
    let mut candidate = NEW_AIRPORT_TILE_OFFSET;
    for &u in &used {
        if u == candidate {
            candidate = candidate.saturating_add(1);
        } else if u > candidate {
            break;
        }
    }
    (candidate < NUM_AIRPORT_TILES).then_some(candidate)
}

/// Gfx de dibujo: id NewGRF si hay Action3; si no, `subst_id` fallback.
#[must_use]
pub fn resolve_airport_tile_draw_gfx(gfx: u16, catalog: &[AirportTileSpecDef]) -> u16 {
    if gfx < NEW_AIRPORT_TILE_OFFSET {
        return gfx;
    }
    let Some(def) = catalog.iter().find(|d| d.gfx.as_u16() == gfx) else {
        return gfx;
    };
    if def.has_newgrf_sprites() {
        gfx
    } else {
        def.subst_id
    }
}

/// Pieza de construcción: siempre `subst_id` vanilla (FTA NewGRF fuera de alcance).
#[must_use]
pub fn resolve_airport_tile_piece_gfx(gfx: u16, catalog: &[AirportTileSpecDef]) -> u8 {
    if gfx < NEW_AIRPORT_TILE_OFFSET {
        return u8::try_from(gfx).unwrap_or(0);
    }
    catalog
        .iter()
        .find(|d| d.gfx.as_u16() == gfx)
        .map(|d| u8::try_from(d.subst_id.min(255)).unwrap_or(0))
        .unwrap_or(0)
}
