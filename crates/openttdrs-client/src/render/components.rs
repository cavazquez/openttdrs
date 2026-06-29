use std::collections::HashMap;

use bevy::prelude::*;

use crate::render::atlas::AtlasSprite;

/// Marca los tiles de agua plana: ciclan los frames `water_anim_{f}.png`.
#[derive(Component)]
pub(crate) struct WaterTile;

/// Tesela de orilla: slot `i` de `shore_full_{i:02}.png` para ciclar sus frames.
#[derive(Component)]
pub(crate) struct ShoreTile(pub(crate) u8);

/// Frames pre-horneados del ciclo de paleta del agua (dark + glitter water),
/// generados por `scripts/gen_water_anim_frames.py`. Frame 0 = sprite base.
#[derive(Resource)]
pub(crate) struct WaterAnimFrames {
    pub(crate) water: Vec<AtlasSprite>,
    pub(crate) shore: Vec<Vec<AtlasSprite>>,
}

/// Frames del fuego de refinería (`industry_{id}_fire_anim_{f}.png`), 7 pasos.
#[derive(Resource)]
pub(crate) struct RefineryFireAnimFrames {
    pub(crate) by_sprite: HashMap<u32, Vec<AtlasSprite>>,
}

/// Frames bebidas gaseosas (`industry_{id}_fizzy_anim_{f}.png`), 5 pasos.
#[derive(Resource)]
pub(crate) struct FizzyDrinkAnimFrames {
    pub(crate) by_sprite: HashMap<u32, Vec<AtlasSprite>>,
}

/// Teselas de suelo, vías, vehículos, etc.: se despawnan al recargar JSON (F9).
#[derive(Component)]
pub(crate) struct MapVisualLayer;

/// Bloque espacial del mapa (16×16 teselas) para culling incremental al hacer pan.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct MapTileChunk {
    pub cx: u32,
    pub cy: u32,
}

pub(crate) const MAP_TILE_CHUNK_SIZE: u32 = 16;

impl MapTileChunk {
    #[must_use]
    pub fn from_tile(tx: u32, ty: u32) -> Self {
        Self {
            cx: tx / MAP_TILE_CHUNK_SIZE,
            cy: ty / MAP_TILE_CHUNK_SIZE,
        }
    }
}

/// Cámara isométrica principal (ventana). Distingue de [`MapPreviewCamera`].
#[derive(Component)]
pub(crate) struct PrimaryGameCamera;

/// Cámaras que renderizan el mapa a una textura (no la ventana principal).
#[derive(Component)]
pub(crate) struct MapPreviewCamera;

/// Vista previa de industria en el panel lateral.
#[derive(Component)]
pub(crate) struct IndustryPreviewCamera;

/// Vista previa del vehículo seleccionado en el panel lateral.
#[derive(Component)]
pub(crate) struct VehiclePreviewCamera;

#[derive(Default)]
pub(crate) struct MapSpriteBatches {
    pub(super) water: Vec<(MapTileChunk, Sprite, Transform)>,
    pub(super) shore: Vec<(MapTileChunk, ShoreTile, Sprite, Transform)>,
    pub(super) trees: Vec<(MapTileChunk, Sprite, Transform)>,
}
