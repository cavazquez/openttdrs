//! Sprites y helpers compartidos de `EffectVehicle` (humo tren, explosión, avería).

use bevy::prelude::*;

use crate::iso::overlay_pos;
use crate::render::AtlasSprite;
use crate::sprites::{
    BREAKDOWN_SMOKE_META, DIESEL_SMOKE_META, ELECTRIC_SPARK_META, EXPLOSION_LARGE_META,
    STEAM_SMOKE_META,
};

/// Frames del atlas para efectos efímeros de vehículos y desastres.
#[derive(Resource, Clone)]
pub(crate) struct EffectVehicleFrames {
    pub(crate) steam: Vec<AtlasSprite>,
    pub(crate) diesel: Vec<AtlasSprite>,
    pub(crate) electric_spark: Vec<AtlasSprite>,
    pub(crate) explosion_large: Vec<AtlasSprite>,
    pub(crate) breakdown: Vec<AtlasSprite>,
}

impl EffectVehicleFrames {
    #[must_use]
    pub(crate) fn from_world_assets(assets: &crate::render::WorldAssets) -> Self {
        Self {
            steam: assets.steam_smoke.clone(),
            diesel: assets.diesel_smoke.clone(),
            electric_spark: assets.electric_spark.clone(),
            explosion_large: assets.explosion_large.clone(),
            breakdown: assets.breakdown_smoke.clone(),
        }
    }

    #[must_use]
    pub(crate) fn is_loaded(&self) -> bool {
        !self.steam.is_empty()
    }
}

/// Conjunto de frames + metadatos NFO para un tipo de efecto.
pub(crate) struct EffectSpriteSet<'a> {
    pub frames: &'a [AtlasSprite],
    pub meta: &'a [(f32, f32, f32, f32)],
    pub frame_secs: f32,
}

impl EffectVehicleFrames {
    #[must_use]
    pub(crate) fn steam_set(&self) -> EffectSpriteSet<'_> {
        EffectSpriteSet {
            frames: &self.steam,
            meta: &STEAM_SMOKE_META,
            frame_secs: 0.43,
        }
    }

    #[must_use]
    pub(crate) fn diesel_set(&self) -> EffectSpriteSet<'_> {
        EffectSpriteSet {
            frames: &self.diesel,
            meta: &DIESEL_SMOKE_META,
            frame_secs: 0.36,
        }
    }

    #[must_use]
    pub(crate) fn electric_set(&self) -> EffectSpriteSet<'_> {
        EffectSpriteSet {
            frames: &self.electric_spark,
            meta: &ELECTRIC_SPARK_META,
            frame_secs: 0.18,
        }
    }

    #[must_use]
    pub(crate) fn explosion_set(&self) -> EffectSpriteSet<'_> {
        EffectSpriteSet {
            frames: &self.explosion_large,
            meta: &EXPLOSION_LARGE_META,
            frame_secs: 0.07,
        }
    }

    #[must_use]
    pub(crate) fn breakdown_set(&self) -> EffectSpriteSet<'_> {
        EffectSpriteSet {
            frames: &self.breakdown,
            meta: &BREAKDOWN_SMOKE_META,
            frame_secs: 0.43,
        }
    }
}

#[must_use]
pub(crate) fn effect_frame_count(set: &EffectSpriteSet<'_>) -> usize {
    let n = set.meta.len();
    if set.frames.is_empty() {
        n
    } else {
        set.frames.len().min(n)
    }
}

#[must_use]
pub(crate) fn effect_frame_index(
    elapsed_secs: f32,
    phase: usize,
    set: &EffectSpriteSet<'_>,
) -> usize {
    let n = effect_frame_count(set);
    if n == 0 {
        return 0;
    }
    ((elapsed_secs / set.frame_secs) as usize + phase) % n
}

#[must_use]
pub(crate) fn effect_lifetime_secs(set: &EffectSpriteSet<'_>) -> f32 {
    effect_frame_count(set) as f32 * set.frame_secs
}

/// Posición en mundo para un frame de efecto anclado a `anchor` (p. ej. locomotora).
#[must_use]
pub(crate) fn effect_overlay_pos(
    anchor: Vec2,
    frame: usize,
    set: &EffectSpriteSet<'_>,
    base_z: u8,
    tile: (i32, i32),
    layer: f32,
    rise: f32,
) -> Vec3 {
    let idx = frame.min(set.meta.len().saturating_sub(1));
    let (w, h, xrel, yrel) = set.meta[idx];
    overlay_pos(
        anchor,
        xrel,
        yrel - rise,
        w,
        h,
        base_z,
        layer,
        tile.0,
        tile.1,
    )
}

pub(crate) fn apply_effect_frame(sprite: &mut Sprite, set: &EffectSpriteSet<'_>, frame: usize) {
    if let Some(atlas) = set.frames.get(frame) {
        atlas.apply_to(sprite);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_index_cycles() {
        let set = EffectSpriteSet {
            frames: &[],
            meta: &STEAM_SMOKE_META,
            frame_secs: 0.43,
        };
        assert_eq!(effect_frame_index(0.0, 0, &set), 0);
        assert_eq!(effect_frame_index(0.86, 0, &set), 2);
        assert_eq!(effect_frame_index(0.0, 3, &set), 3);
    }
}
