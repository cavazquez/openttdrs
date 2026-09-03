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
/// Bit `IndustryTileCallbackMask::ShapeCheck`: consulta CB `0x2F` al
/// comprobar la pendiente durante la creación.
pub const INDUSTRY_TILE_CALLBACK_SHAPE_CHECK_MASK: u8 = 1 << 4;
/// Bit `IndustryTileCallbackMask::DrawFoundations`: consulta CB `0x30` al
/// dibujar una tesela de industria sobre una pendiente.
pub const INDUSTRY_TILE_CALLBACK_DRAW_FOUNDATIONS_MASK: u8 = 1 << 5;
/// Bit `IndustryTileCallbackMask::Autoslope`: consulta CB `0x3C` al
/// terraformar una tesela de industria.
pub const INDUSTRY_TILE_CALLBACK_AUTOSLOPE_MASK: u8 = 1 << 6;
/// Bit `IndustryTileCallbackMask::CargoAcceptance`: consulta CB `0x2B`.
pub const INDUSTRY_TILE_CALLBACK_CARGO_ACCEPTANCE_MASK: u8 = 1 << 2;
/// Bit `IndustryTileCallbackMask::AcceptCargo`: consulta CB `0x2C`.
pub const INDUSTRY_TILE_CALLBACK_ACCEPT_CARGO_MASK: u8 = 1 << 3;
/// Bit `IndustryTileSpecialFlag::AcceptsAllCargo` de `prop 0x12`.
pub const INDUSTRY_TILE_SPECIAL_ACCEPTS_ALL_CARGO_MASK: u8 = 1 << 1;

/// Réplica de `IsSlopeRefused` para el fallback de `IndustryTile`.
///
/// Las cuatro banderas bajas de `slopes_refused` describen la dirección que
/// no puede quedar elevada; `SLOPE_STEEP` rechaza siempre una pendiente de dos
/// niveles. `OpenTTD` compara contra la pendiente complementaria, por lo que no
/// basta con hacer un `&` directo entre las dos máscaras.
#[must_use]
pub const fn industry_tile_slope_refused(current: u8, refused: u8) -> bool {
    const SLOPE_STEEP: u8 = crate::map::SLOPE_STEEP;
    const SLOPE_W: u8 = 1;
    const SLOPE_S: u8 = 2;
    const SLOPE_E: u8 = 4;
    const SLOPE_N: u8 = 8;
    const SLOPE_NW: u8 = crate::map::SLOPE_NW;
    const SLOPE_NE: u8 = crate::map::SLOPE_NE;
    const SLOPE_SW: u8 = crate::map::SLOPE_SW;
    const SLOPE_SE: u8 = crate::map::SLOPE_SE;

    if current & SLOPE_STEEP != 0 {
        return true;
    }
    if current == 0 {
        return false;
    }
    if refused & SLOPE_STEEP != 0 {
        return true;
    }
    let complement = current ^ 0x0F;
    (refused & SLOPE_W != 0 && complement & SLOPE_NW != 0)
        || (refused & SLOPE_S != 0 && complement & SLOPE_NE != 0)
        || (refused & SLOPE_E != 0 && complement & SLOPE_SW != 0)
        || (refused & SLOPE_N != 0 && complement & SLOPE_SE != 0)
}

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
    /// Máscara de pendientes rechazadas (`prop 0x0D`), usada cuando no hay
    /// callback `CBID_INDTILE_SHAPE_CHECK`.
    #[serde(default)]
    pub slopes_refused: u8,
    /// Índices GRF-local de cargos aceptados (`0x0A`–`0x0C` / `0x13`).
    #[serde(default)]
    pub accepts_cargo_indices: Vec<u8>,
    /// Labels resueltos (`GetCargoTranslation` / `cargo_spec`).
    #[serde(default)]
    pub accepts_cargo_labels: Vec<String>,
    /// Cantidades de aceptación (octavos; `0x0A`–`0x0C` / `0x13`).
    #[serde(default)]
    pub acceptance: Vec<i8>,
    /// Callback mask (`prop 0x0E`):
    /// bit 0 = next frame 0x26, bit 1 = speed 0x27, bit 4 = shape check
    /// 0x2F, bit 5 = foundations 0x30 y bit 6 = autoslope 0x3C.
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
    /// Badges asociados por `prop 0x14` (`ReadBadgeList`).
    #[serde(default)]
    pub associated_badges: Vec<u16>,
    /// Tabla local→global de badges del GRF.
    #[serde(default)]
    pub newgrf_badge_translation: Vec<u16>,
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
    /// ¿La tesela hereda todos los cargos que acepta su industria padre?
    #[must_use]
    pub const fn accepts_all_cargo(&self) -> bool {
        self.animation_special_flags & INDUSTRY_TILE_SPECIAL_ACCEPTS_ALL_CARGO_MASK != 0
    }

    /// ¿El GRF decide las cantidades de aceptación mediante CB `0x2B`?
    #[must_use]
    pub const fn has_cargo_acceptance_callback(&self) -> bool {
        self.callback_mask & INDUSTRY_TILE_CALLBACK_CARGO_ACCEPTANCE_MASK != 0
    }

    /// ¿El GRF decide los tipos de cargo mediante CB `0x2C`?
    #[must_use]
    pub const fn has_accept_cargo_callback(&self) -> bool {
        self.callback_mask & INDUSTRY_TILE_CALLBACK_ACCEPT_CARGO_MASK != 0
    }

    /// ¿El GRF decide si se dibuja la fundación nivelada (`CB 0x30`)?
    #[must_use]
    pub const fn has_draw_foundations_callback(&self) -> bool {
        self.callback_mask & INDUSTRY_TILE_CALLBACK_DRAW_FOUNDATIONS_MASK != 0
    }

    /// ¿El GRF decide si la pendiente admite esta tesela (`CB 0x2F`)?
    #[must_use]
    pub const fn has_shape_check_callback(&self) -> bool {
        self.callback_mask & INDUSTRY_TILE_CALLBACK_SHAPE_CHECK_MASK != 0
    }

    /// ¿El GRF permite o bloquea el autoslope de esta tesela (`CB 0x3C`)?
    #[must_use]
    pub const fn has_autoslope_callback(&self) -> bool {
        self.callback_mask & INDUSTRY_TILE_CALLBACK_AUTOSLOPE_MASK != 0
    }

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

    /// Layout `TileSeq` de Action2 para una tesela de industria.
    ///
    /// La etapa de construcción se resuelve en la vista plana; una vez que
    /// Action2 selecciona el grupo de layout, cada referencia apunta al
    /// primer sprite de su set Action1 y no debe volver a indexarse por
    /// `idx`.
    pub fn newgrf_tile_layout_runtime(
        &self,
        idx: usize,
        ctx: &mut crate::newgrf_sprites::Action2EvalCtx,
    ) -> Option<crate::newgrf_sprites::ResolvedTileLayout> {
        let _ = idx;
        self.newgrf_runtime.as_ref()?.tile_layout_for_local_id_ctx(
            u16::from(self.newgrf_local_id),
            0,
            ctx,
        )
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
    use crate::newgrf_sprites::{
        DecodedSprite, TileLayout, TileLayoutSpriteRef, TrainSpriteAssign, TrainSpriteGraphics,
    };

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
    fn industry_tile_draw_foundations_callback_uses_upstream_mask() {
        let mut def = IndustryTileSpecDef {
            gfx: IndustryTileGfxId(NEW_INDUSTRY_TILE_OFFSET),
            subst_id: 0,
            from_newgrf: true,
            slopes_refused: 0,
            accepts_cargo_indices: Vec::new(),
            accepts_cargo_labels: Vec::new(),
            acceptance: Vec::new(),
            callback_mask: 0,
            animation_frames: 0,
            animation_status: 0,
            animation_speed: 0,
            animation_triggers: 0,
            animation_special_flags: 0,
            associated_badges: Vec::new(),
            newgrf_badge_translation: Vec::new(),
            newgrf_local_id: 0,
            newgrf_grfid: 1,
            newgrf_preview: None,
            newgrf_views: Vec::new(),
            newgrf_runtime: None,
        };
        assert!(!def.has_draw_foundations_callback());
        def.callback_mask = INDUSTRY_TILE_CALLBACK_DRAW_FOUNDATIONS_MASK;
        assert!(def.has_draw_foundations_callback());
    }

    #[test]
    fn industry_tile_callback_masks_match_upstream() {
        let mut def = IndustryTileSpecDef {
            gfx: IndustryTileGfxId(NEW_INDUSTRY_TILE_OFFSET),
            subst_id: 0,
            from_newgrf: true,
            slopes_refused: 0,
            accepts_cargo_indices: Vec::new(),
            accepts_cargo_labels: Vec::new(),
            acceptance: Vec::new(),
            callback_mask: INDUSTRY_TILE_CALLBACK_SHAPE_CHECK_MASK
                | INDUSTRY_TILE_CALLBACK_DRAW_FOUNDATIONS_MASK
                | INDUSTRY_TILE_CALLBACK_AUTOSLOPE_MASK,
            animation_frames: 0,
            animation_status: 0,
            animation_speed: 0,
            animation_triggers: 0,
            animation_special_flags: 0,
            associated_badges: Vec::new(),
            newgrf_badge_translation: Vec::new(),
            newgrf_local_id: 0,
            newgrf_grfid: 1,
            newgrf_preview: None,
            newgrf_views: Vec::new(),
            newgrf_runtime: None,
        };
        assert!(def.has_shape_check_callback());
        assert!(def.has_draw_foundations_callback());
        assert!(def.has_autoslope_callback());
        def.callback_mask = 0;
        assert!(!def.has_shape_check_callback());
        assert!(!def.has_draw_foundations_callback());
        assert!(!def.has_autoslope_callback());
    }

    #[test]
    fn industry_tile_slope_refusal_matches_complement_rule() {
        assert!(!industry_tile_slope_refused(0, 0xFF));
        assert!(industry_tile_slope_refused(crate::map::SLOPE_NE, 1));
        assert!(!industry_tile_slope_refused(crate::map::SLOPE_NE, 0));
        assert!(industry_tile_slope_refused(crate::map::SLOPE_STEEP, 0));
        assert!(industry_tile_slope_refused(
            crate::map::SLOPE_NE,
            crate::map::SLOPE_STEEP
        ));
    }

    #[test]
    fn next_free_starts_at_175() {
        assert_eq!(next_free_industry_tile_gfx_id(&[]), Some(175));
        let catalog = vec![IndustryTileSpecDef {
            gfx: IndustryTileGfxId(175),
            subst_id: 0,
            from_newgrf: true,
            slopes_refused: 0,
            accepts_cargo_indices: Vec::new(),
            accepts_cargo_labels: Vec::new(),
            acceptance: Vec::new(),
            callback_mask: 0,
            animation_frames: 0,
            animation_status: 0,
            animation_speed: 0,
            animation_triggers: 0,
            animation_special_flags: 0,
            associated_badges: Vec::new(),
            newgrf_badge_translation: Vec::new(),
            newgrf_local_id: 0,
            newgrf_grfid: 1,
            newgrf_preview: None,
            newgrf_views: Vec::new(),
            newgrf_runtime: None,
        }];
        assert_eq!(next_free_industry_tile_gfx_id(&catalog), Some(176));
    }

    #[test]
    fn runtime_tile_layout_resolves_ground_and_sequence() {
        let sprite = DecodedSprite {
            width: 2,
            height: 2,
            x_offs: -1,
            y_offs: 3,
            rgba: [64, 96, 128, 255].repeat(4),
            mask: Vec::new(),
        };
        let mut runtime = TrainSpriteGraphics {
            sets: vec![vec![sprite.clone()], vec![sprite.clone()]],
            assigns: vec![TrainSpriteAssign {
                local_id: 9,
                set_id: 6,
            }],
            ..Default::default()
        };
        runtime.tile_layouts.insert(
            6,
            TileLayout {
                ground: TileLayoutSpriteRef {
                    action1_set: Some(0),
                    ..Default::default()
                },
                sequence: vec![TileLayoutSpriteRef {
                    action1_set: Some(1),
                    origin: [3, 4, 5],
                    extent: [8, 8, 16],
                    ..Default::default()
                }],
            },
        );
        let def = IndustryTileSpecDef {
            gfx: IndustryTileGfxId(175),
            subst_id: 0,
            from_newgrf: true,
            slopes_refused: 0,
            accepts_cargo_indices: Vec::new(),
            accepts_cargo_labels: Vec::new(),
            acceptance: Vec::new(),
            callback_mask: 0,
            animation_frames: 0,
            animation_status: 0,
            animation_speed: 0,
            animation_triggers: 0,
            animation_special_flags: 0,
            associated_badges: Vec::new(),
            newgrf_badge_translation: Vec::new(),
            newgrf_local_id: 9,
            newgrf_grfid: 0,
            newgrf_preview: Some(sprite.clone()),
            newgrf_views: vec![sprite],
            newgrf_runtime: Some(Box::new(runtime)),
        };
        let mut ctx = crate::newgrf_sprites::Action2EvalCtx::default();
        let Some(layout) = def.newgrf_tile_layout_runtime(3, &mut ctx) else {
            panic!("industry TileSeq");
        };
        assert!(layout.complete);
        assert!(layout.ground.is_some());
        assert_eq!(layout.sequence[0].origin, [3, 4, 5]);
    }
}
