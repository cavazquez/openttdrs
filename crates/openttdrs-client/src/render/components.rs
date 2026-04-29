use bevy::prelude::*;

/// Marca los tiles de agua para la animación por ondas.
/// Almacena fases discretas por tile para emular el ciclado de paleta
/// (dark water 5 pasos + glitter 15 pasos).
#[derive(Component)]
pub(crate) struct WaterTile {
    pub(crate) dark_phase: u8,
    pub(crate) glitter_phase: u8,
}

/// Teselas de suelo, vías, vehículos, etc.: se despawnan al recargar JSON (F9).
#[derive(Component)]
pub(crate) struct MapVisualLayer;

#[derive(Default)]
pub(crate) struct MapSpriteBatches {
    pub(super) water: Vec<(WaterTile, Sprite, Transform)>,
    pub(super) shore: Vec<(Sprite, Transform)>,
    pub(super) trees: Vec<(Sprite, Transform)>,
}
