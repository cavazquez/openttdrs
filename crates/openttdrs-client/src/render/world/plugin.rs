//! Plugin, recursos y SystemParams para world render.

use std::collections::HashSet;

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

use crate::bevy_app::UpdateSet;
use crate::render::{MapTileChunk, TileViewportBounds, large_map_viewport_cull_enabled};
use crate::state::ClientScreen;

use super::remap::apply_remap_map_visuals;
use super::tile_spawn::setup;
use super::viewport::sync_map_tile_spawn_viewport;

/// Queries de etiquetas del mapa (agrupadas para no superar el límite de params Bevy).
#[derive(SystemParam)]
pub(crate) struct MapLabelEntities<'w, 's> {
    pub towns: Query<'w, 's, Entity, With<crate::render::town_labels::TownLabel>>,
    pub stations: Query<'w, 's, Entity, With<crate::render::station_labels::StationLabel>>,
    pub signs: Query<'w, 's, Entity, With<crate::render::sign_labels::SignLabel>>,
}

/// Agrupa cachés NewGRF in-world para no superar el límite de 16 `SystemParam`.
#[derive(SystemParam)]
pub(crate) struct NewGrfMapSpriteCaches<'w> {
    pub road: ResMut<'w, crate::render::NewGrfRoadSpriteCache>,
    pub station: ResMut<'w, crate::render::NewGrfStationSpriteCache>,
    pub shore: ResMut<'w, crate::render::NewGrfShoreSpriteCache>,
    pub catenary: ResMut<'w, crate::render::NewGrfCatenarySpriteCache>,
    pub industry: ResMut<'w, crate::render::NewGrfIndustrySpriteCache>,
}

/// Petición de redibujo del mapa. `sync_camera`: solo tras F9 / cambio de tamaño.
#[derive(Resource)]
pub(crate) struct RemapMapVisualsPending {
    pub(crate) pending: bool,
    pub(crate) sync_camera: bool,
    /// Rebuild completo (construcción, F9). Pan en mapas grandes usa `full = false`.
    pub(crate) full: bool,
    /// Chunks a regenerar in-place (construcción dentro del viewport ya cargado).
    pub(crate) refresh_chunks: HashSet<(u32, u32)>,
}

impl RemapMapVisualsPending {
    pub(crate) fn extend_refresh_chunks(&mut self, tiles: &[(i32, i32)]) {
        for &(tx, ty) in tiles {
            if tx >= 0 && ty >= 0 {
                let ch = MapTileChunk::from_tile(tx as u32, ty as u32);
                self.refresh_chunks.insert((ch.cx, ch.cy));
            }
        }
    }
}

impl Default for RemapMapVisualsPending {
    fn default() -> Self {
        Self {
            pending: false,
            sync_camera: false,
            full: true,
            refresh_chunks: HashSet::new(),
        }
    }
}

/// Marca redibujo tras construcción/sim: en mapas con culling refresca solo los chunks tocados.
pub(crate) fn request_map_visual_remap(
    pending: &mut RemapMapVisualsPending,
    mw: u32,
    mh: u32,
    tiles: &[(i32, i32)],
) {
    pending.pending = true;
    pending.sync_camera = false;
    if large_map_viewport_cull_enabled(mw, mh) {
        pending.full = false;
        pending.extend_refresh_chunks(tiles);
    } else {
        pending.full = true;
    }
}

/// Bloques 16×16 ya instanciados (solo mapas con culling por viewport).
#[derive(Resource, Default)]
pub(crate) struct LoadedMapTileChunks {
    pub chunks: HashSet<(u32, u32)>,
}

/// Rectángulo de teselas para las que se generaron sprites (`MapVisualLayer`).
#[derive(Resource)]
pub(crate) struct MapTileSpawnViewport {
    pub(crate) bounds: TileViewportBounds,
    /// Último `OrthographicProjection::scale` usado para `bounds` (detectar zoom).
    pub(crate) last_ortho_scale: f32,
}

impl Default for MapTileSpawnViewport {
    fn default() -> Self {
        Self {
            bounds: TileViewportBounds::full(1, 1),
            last_ortho_scale: 1.0,
        }
    }
}

pub(crate) struct WorldRenderPlugin;

impl Plugin for WorldRenderPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<RemapMapVisualsPending>()
            .init_resource::<MapTileSpawnViewport>()
            .init_resource::<LoadedMapTileChunks>()
            .init_resource::<crate::render::NewGrfRoadSpriteCache>()
            .init_resource::<crate::render::NewGrfStationSpriteCache>()
            .init_resource::<crate::render::NewGrfShoreSpriteCache>()
            .init_resource::<crate::render::NewGrfCatenarySpriteCache>()
            .init_resource::<crate::render::NewGrfIndustrySpriteCache>()
            .add_systems(OnEnter(ClientScreen::InGame), setup)
            .add_systems(
                Update,
                (
                    sync_map_tile_spawn_viewport,
                    super::remap::sync_company_colored_sprites,
                    apply_remap_map_visuals,
                )
                    .chain()
                    .in_set(UpdateSet::RenderRefresh)
                    .after(crate::camera::move_camera)
                    .run_if(in_state(ClientScreen::InGame)),
            );
    }
}
