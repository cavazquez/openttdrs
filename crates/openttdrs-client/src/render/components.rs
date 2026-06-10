use bevy::prelude::*;

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
    pub(crate) water: Vec<Handle<Image>>,
    pub(crate) shore: Vec<Vec<Handle<Image>>>,
}

/// Teselas de suelo, vías, vehículos, etc.: se despawnan al recargar JSON (F9).
#[derive(Component)]
pub(crate) struct MapVisualLayer;

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
    pub(super) water: Vec<(Sprite, Transform)>,
    pub(super) shore: Vec<(ShoreTile, Sprite, Transform)>,
    pub(super) trees: Vec<(Sprite, Transform)>,
}
