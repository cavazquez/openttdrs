//! Features de canal `NewGRF` (`CanalFeature` / Action0 `0x05`).
//!
//! IDs alineados con `OpenTTD` `newgrf.h` (`CF_*`).

use serde::{Deserialize, Serialize};

use crate::newgrf_sprites::{CALLBACK_FAILED, DecodedSprite, TrainSpriteGraphics};

/// `CF_END` — número de features de canal.
pub const CANAL_FEATURE_COUNT: usize = 9;

/// `CF_WATERSLOPE`.
pub const CF_WATERSLOPE: u8 = 0;
/// `CF_LOCKS`.
pub const CF_LOCKS: u8 = 1;
/// `CF_DIKES`.
pub const CF_DIKES: u8 = 2;
/// `CF_ICON`.
pub const CF_ICON: u8 = 3;
/// `CF_DOCKS`.
pub const CF_DOCKS: u8 = 4;
/// `CF_RIVER_SLOPE`.
pub const CF_RIVER_SLOPE: u8 = 5;
/// `CF_RIVER_EDGE`.
pub const CF_RIVER_EDGE: u8 = 6;
/// `CF_RIVER_GUI`.
pub const CF_RIVER_GUI: u8 = 7;
/// `CF_BUOY`.
pub const CF_BUOY: u8 = 8;

/// `CFF_HAS_FLAT_SPRITE`: el primer sprite del feature es el ground plano.
pub const CFF_HAS_FLAT_SPRITE: u8 = 1 << 0;

/// Spec de un feature de canal (Action0 `0x05` + vistas Action3 opcionales).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CanalFeatureDef {
    pub id: u8,
    /// Prop `0x08` callback mask.
    pub callback_mask: u8,
    /// Prop `0x09` flags de display.
    pub flags: u8,
    pub from_newgrf: bool,
    pub grfid: u32,
    /// Vistas Action1/3 del feature (`serde` skip: runtime).
    #[serde(skip)]
    pub newgrf_views: Vec<DecodedSprite>,
    /// Grafo Action2 completo para resolver vistas y callbacks por tesela.
    ///
    /// `OpenTTD` vuelve a evaluar el grupo del feature al consultar una
    /// pendiente o `GetCanalSpriteOffset`; conservar sólo `newgrf_views`
    /// congela esa decisión en el primer contexto de carga.
    #[serde(skip)]
    pub newgrf_runtime: Option<Box<TrainSpriteGraphics>>,
}

impl CanalFeatureDef {
    /// Vista Action1/3 re-resuelta con el contexto de la tesela.
    pub fn newgrf_view_runtime(
        &self,
        idx: usize,
        ctx: &mut crate::newgrf_sprites::Action2EvalCtx,
    ) -> Option<DecodedSprite> {
        let runtime = self.newgrf_runtime.as_ref()?;
        let views = runtime.views_for_local_id_ctx(self.id, ctx)?;
        if views.is_empty() {
            return None;
        }
        views.get(idx).cloned()
    }

    /// Ejecuta `CBID_CANALS_SPRITE_OFFSET` (`0x147`) si el feature lo habilitó.
    ///
    /// El callback devuelve el delta que se suma al offset actual; ante fallo
    /// se conserva exactamente el offset recibido, como `GetCanalSpriteOffset`.
    pub fn newgrf_sprite_offset(
        &self,
        current_offset: usize,
        ctx: &mut crate::newgrf_sprites::Action2EvalCtx,
    ) -> usize {
        if self.callback_mask & 1 == 0 {
            return current_offset;
        }
        let Some(runtime) = self.newgrf_runtime.as_ref() else {
            return current_offset;
        };
        let current = u32::try_from(current_offset).unwrap_or(u32::MAX);
        let callback = runtime.resolve_callback_ctx(
            self.id,
            crate::newgrf_sprites::CBID_CANALS_SPRITE_OFFSET,
            current,
            0,
            ctx,
        );
        if callback == CALLBACK_FAILED {
            current_offset
        } else {
            current_offset.saturating_add(usize::from(callback))
        }
    }
}

/// Catálogo vanilla: 9 features con flags/callbacks a 0.
#[must_use]
pub fn vanilla_canal_feature_catalog() -> Vec<CanalFeatureDef> {
    (0..CANAL_FEATURE_COUNT)
        .map(|id| CanalFeatureDef {
            id: u8::try_from(id).unwrap_or(0),
            callback_mask: 0,
            flags: 0,
            from_newgrf: false,
            grfid: 0,
            newgrf_views: Vec::new(),
            newgrf_runtime: None,
        })
        .collect()
}

/// Spec del feature `id` (`0..CF_END`).
#[must_use]
pub fn canal_feature_def(catalog: &[CanalFeatureDef], id: u8) -> Option<&CanalFeatureDef> {
    let idx = usize::from(id);
    if idx >= CANAL_FEATURE_COUNT {
        return None;
    }
    catalog.get(idx).filter(|d| d.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::newgrf_sprites::{
        Action2VarAdjust, Action2VarEntry, Action2VarTerm, TrainSpriteAssign,
    };

    fn callback_runtime(feature_id: u8, delta: u32) -> TrainSpriteGraphics {
        let mut runtime = TrainSpriteGraphics::default();
        runtime.assigns.push(TrainSpriteAssign {
            local_id: feature_id,
            set_id: 1,
        });
        runtime.action2_var.insert(
            1,
            Action2VarEntry {
                first: Action2VarTerm {
                    variable: 0x1A,
                    param: None,
                    adjust: Action2VarAdjust {
                        and_mask: delta,
                        ..Default::default()
                    },
                },
                ops: Vec::new(),
                ranges: Vec::new(),
                default: 0,
            },
        );
        runtime
    }

    #[test]
    fn canal_sprite_offset_callback_adds_delta_and_preserves_failure() {
        let mut feature = CanalFeatureDef {
            id: CF_DIKES,
            callback_mask: 1,
            flags: 0,
            from_newgrf: true,
            grfid: 0xCAFE,
            newgrf_views: Vec::new(),
            newgrf_runtime: Some(Box::new(callback_runtime(CF_DIKES, 7))),
        };
        let mut ctx = crate::newgrf_sprites::Action2EvalCtx::default();
        assert_eq!(feature.newgrf_sprite_offset(12, &mut ctx), 19);

        feature.callback_mask = 0;
        assert_eq!(feature.newgrf_sprite_offset(12, &mut ctx), 12);

        feature.callback_mask = 1;
        feature.newgrf_runtime = None;
        assert_eq!(feature.newgrf_sprite_offset(12, &mut ctx), 12);
    }

    #[test]
    fn canal_runtime_view_resolves_the_feature_local_id() {
        let sprite = DecodedSprite {
            width: 1,
            height: 1,
            x_offs: 0,
            y_offs: 0,
            rgba: vec![255, 0, 0, 255],
            mask: Vec::new(),
        };
        let mut runtime = TrainSpriteGraphics::default();
        runtime.sets = vec![vec![sprite.clone()]];
        runtime.assigns.push(TrainSpriteAssign {
            local_id: CF_RIVER_EDGE,
            set_id: 0,
        });
        let feature = CanalFeatureDef {
            id: CF_RIVER_EDGE,
            callback_mask: 0,
            flags: 0,
            from_newgrf: true,
            grfid: 0xBEEF,
            newgrf_views: vec![sprite.clone()],
            newgrf_runtime: Some(Box::new(runtime)),
        };
        let mut ctx = crate::newgrf_sprites::Action2EvalCtx::default();
        assert_eq!(feature.newgrf_view_runtime(0, &mut ctx), Some(sprite));
    }

    #[test]
    fn canal_runtime_view_does_not_wrap_missing_slot() {
        let sprite = DecodedSprite {
            width: 1,
            height: 1,
            x_offs: 0,
            y_offs: 0,
            rgba: vec![0, 255, 0, 255],
            mask: Vec::new(),
        };
        let mut runtime = TrainSpriteGraphics::default();
        runtime.sets = vec![vec![sprite.clone()]];
        runtime.assigns.push(TrainSpriteAssign {
            local_id: CF_RIVER_EDGE,
            set_id: 0,
        });
        let feature = CanalFeatureDef {
            id: CF_RIVER_EDGE,
            callback_mask: 0,
            flags: 0,
            from_newgrf: true,
            grfid: 0xBEEF,
            newgrf_views: Vec::new(),
            newgrf_runtime: Some(Box::new(runtime)),
        };
        let mut ctx = crate::newgrf_sprites::Action2EvalCtx::default();
        assert_eq!(feature.newgrf_view_runtime(1, &mut ctx), None);
    }
}
