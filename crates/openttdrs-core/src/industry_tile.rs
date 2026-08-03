//! Teselas de industria `NewGRF` (`IndustryTiles`, feature `0x09`).
//!
//! Paridad MVP con `NEW_INDUSTRYTILEOFFSET` / `GetTranslatedIndustryTileID`:
//! slots globales ≥175, `subst_id` vanilla y overrides de gfx &lt;175.

use serde::{Deserialize, Serialize};

/// Primera tesela definida por `NewGRF` (`OpenTTD` `NEW_INDUSTRYTILEOFFSET`).
pub const NEW_INDUSTRY_TILE_OFFSET: u16 = 175;
/// Total de slots de tesela de industria (`OpenTTD` `NUM_INDUSTRYTILES`).
pub const NUM_INDUSTRY_TILES: u16 = 512;
/// Id inválido (`OpenTTD` `INVALID_INDUSTRYTILE`).
pub const INVALID_INDUSTRY_TILE: u16 = NUM_INDUSTRY_TILES;

/// Identificador global de gfx de tesela de industria.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct IndustryTileGfxId(pub u16);

impl IndustryTileGfxId {
    #[must_use]
    pub const fn as_u16(self) -> u16 {
        self.0
    }
}

/// Spec `NewGRF` de una tesela de industria (simplificado).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndustryTileSpecDef {
    /// Gfx global (≥ [`NEW_INDUSTRY_TILE_OFFSET`] si `from_newgrf`).
    pub gfx: IndustryTileGfxId,
    /// Fallback vanilla (`subst_id` &lt; 175).
    pub subst_id: u16,
    pub from_newgrf: bool,
    /// Índices GRF-local de cargos aceptados (`0x0A`–`0x0C` / `0x13`).
    #[serde(default)]
    pub accepts_cargo_indices: Vec<u8>,
    /// Labels resueltos (`GetCargoTranslation` / `cargo_spec`).
    #[serde(default)]
    pub accepts_cargo_labels: Vec<String>,
    /// Cantidades de aceptación (octavos; `0x0A`–`0x0C` / `0x13`).
    #[serde(default)]
    pub acceptance: Vec<i8>,
    /// Callback mask (`prop 0x0E`): bit 0 = next frame 0x26, bit 1 = speed 0x27.
    #[serde(default)]
    pub callback_mask: u8,
    /// `prop 0x0F`: último frame de animación permitido.
    #[serde(default)]
    pub animation_frames: u8,
    /// `prop 0x0F`: 1 = looping; otros valores finalizan al llegar al último frame.
    #[serde(default)]
    pub animation_status: u8,
    /// `prop 0x10`: espera como potencia de dos de ticks.
    #[serde(default)]
    pub animation_speed: u8,
    /// `prop 0x11`: triggers que invocan el callback 0x25.
    #[serde(default)]
    pub animation_triggers: u8,
    /// `prop 0x12`: bit 0 pasa los random bits al callback 0x26.
    #[serde(default)]
    pub animation_special_flags: u8,
    /// Id local Action3 en el GRF.
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

impl IndustryTileSpecDef {
    #[must_use]
    pub fn newgrf_view(&self, idx: usize) -> Option<&crate::newgrf_sprites::DecodedSprite> {
        if self.newgrf_views.is_empty() {
            return self.newgrf_preview.as_ref();
        }
        self.newgrf_views.get(idx % self.newgrf_views.len())
    }

    pub fn newgrf_view_runtime(
        &self,
        idx: usize,
        ctx: &mut crate::newgrf_sprites::Action2EvalCtx,
    ) -> Option<crate::newgrf_sprites::DecodedSprite> {
        let runtime = self.newgrf_runtime.as_ref()?;
        let views = runtime.views_for_local_id_ctx(self.newgrf_local_id, ctx)?;
        if views.is_empty() {
            return None;
        }
        Some(views[idx % views.len()].clone())
    }

    #[must_use]
    pub fn has_newgrf_sprites(&self) -> bool {
        !self.newgrf_views.is_empty()
            || self.newgrf_preview.is_some()
            || self.newgrf_runtime.is_some()
    }
}

/// Tabla de overrides vanilla → gfx `NewGRF` (`GetTranslatedIndustryTileID`).
#[must_use]
pub fn empty_industry_tile_overrides() -> Vec<u16> {
    vec![INVALID_INDUSTRY_TILE; NEW_INDUSTRY_TILE_OFFSET as usize]
}

/// Traduce gfx limpio aplicando override `NewGRF` (si hay).
#[must_use]
pub fn get_translated_industry_tile_id(clean: u16, overrides: &[u16]) -> u16 {
    if clean == 0xFF {
        return clean;
    }
    if let Some(&ovr) = overrides.get(usize::from(clean))
        && ovr != INVALID_INDUSTRY_TILE
    {
        return ovr;
    }
    clean
}

/// Gfx limpio de tesela (`GetCleanIndustryGfx`).
#[must_use]
pub fn get_clean_industry_gfx(m5: u8, m6: u8) -> u16 {
    u16::from(m5) | (u16::from((m6 >> 2) & 1) << 8)
}

#[must_use]
pub fn industry_tile_spec_def(
    catalog: &[IndustryTileSpecDef],
    gfx: u16,
) -> Option<&IndustryTileSpecDef> {
    catalog.iter().find(|d| d.gfx.as_u16() == gfx)
}

/// Siguiente id libre en `[NEW_INDUSTRY_TILE_OFFSET, NUM_INDUSTRY_TILES)`.
#[must_use]
pub fn next_free_industry_tile_gfx_id(catalog: &[IndustryTileSpecDef]) -> Option<u16> {
    (NEW_INDUSTRY_TILE_OFFSET..NUM_INDUSTRY_TILES)
        .find(|&id| !catalog.iter().any(|d| d.gfx.as_u16() == id))
}

/// Gfx de dibujo: id `NewGRF` si hay Action3; si no, `subst_id` fallback.
#[must_use]
pub fn resolve_industry_tile_draw_gfx(gfx: u16, catalog: &[IndustryTileSpecDef]) -> u16 {
    let Some(def) = industry_tile_spec_def(catalog, gfx) else {
        return gfx;
    };
    if def.has_newgrf_sprites() {
        gfx
    } else {
        def.subst_id
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn translate_applies_override() {
        let mut ovr = empty_industry_tile_overrides();
        ovr[42] = 200;
        assert_eq!(get_translated_industry_tile_id(42, &ovr), 200);
        assert_eq!(get_translated_industry_tile_id(41, &ovr), 41);
        assert_eq!(get_translated_industry_tile_id(200, &ovr), 200);
        assert_eq!(ovr.len(), NEW_INDUSTRY_TILE_OFFSET as usize);
    }

    #[test]
    fn next_free_starts_at_175() {
        assert_eq!(next_free_industry_tile_gfx_id(&[]), Some(175));
        let catalog = vec![IndustryTileSpecDef {
            gfx: IndustryTileGfxId(175),
            subst_id: 0,
            from_newgrf: true,
            accepts_cargo_indices: Vec::new(),
            accepts_cargo_labels: Vec::new(),
            acceptance: Vec::new(),
            callback_mask: 0,
            animation_frames: 0,
            animation_status: 0,
            animation_speed: 0,
            animation_triggers: 0,
            animation_special_flags: 0,
            newgrf_local_id: 0,
            newgrf_grfid: 1,
            newgrf_preview: None,
            newgrf_views: Vec::new(),
            newgrf_runtime: None,
        }];
        assert_eq!(next_free_industry_tile_gfx_id(&catalog), Some(176));
    }
}
