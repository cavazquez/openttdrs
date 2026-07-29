//! Features de canal `NewGRF` (`CanalFeature` / Action0 `0x05`).
//!
//! IDs alineados con `OpenTTD` `newgrf.h` (`CF_*`).

use serde::{Deserialize, Serialize};

use crate::newgrf_sprites::DecodedSprite;

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
