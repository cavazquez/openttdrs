//! Plugin, recursos y SystemParams para world render.

use std::collections::HashSet;

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

use crate::bevy_app::UpdateSet;
use crate::render::{
    MapTileChunk, TileViewportBounds, chunk_tile_bounds, chunks_in_bounds,
    large_map_viewport_cull_enabled, sort_viewport_sortable_parents,
    sync_viewport_sortable_children,
};
use crate::state::ClientScreen;

use super::remap::apply_remap_map_visuals;
use super::tile_spawn::setup;
use super::viewport::sync_map_tile_spawn_viewport;

const LABEL_LOD_ZOOM: f32 = 4.0;
/// OpenTTD usa `FS_SMALL` desde Out4x. La fuente Bevy es la misma familia,
/// por lo que este factor reproduce su altura sin rasterizar otra textura.
const SMALL_LABEL_SCALE: f32 = 0.70;
/// El PNG de agua de OpenGFX mide 64×31, pero su rombo lógico ocupa 64×32.
/// A `Out2x` y posteriores, reducir directamente la caja de 31 filas deja
/// medios píxeles sin cubrir entre vecinos y aparece el framebuffer negro.
const FLAT_WATER_RASTER_FOOTPRINT: Vec2 = Vec2::new(64.0, 32.0);
const WATER_SEAM_FIX_ZOOM: f32 = 2.0;

/// Evita recorrer todas las teselas de agua cada frame: sólo se reaplica al
/// cruzar el umbral de zoom o a sprites de agua que acaba de materializar el
/// remapeo incremental.
#[derive(Resource, Default)]
struct FlatWaterRasterFootprintState {
    zoomed_out: Option<bool>,
}

#[must_use]
fn flat_water_raster_footprint(camera_scale: f32) -> Option<Vec2> {
    (camera_scale >= WATER_SEAM_FIX_ZOOM).then_some(FLAT_WATER_RASTER_FOOTPRINT)
}

/// Emula el footprint entero que conserva el blitter 8bpp de OpenTTD al
/// alejar: no cambia el PNG ni el zoom normal, pero asegura que dos rombos
/// vecinos siguen compartiendo un píxel tras la reducción ortográfica.
fn sync_flat_water_raster_footprint(
    cam_q: Query<
        &Projection,
        (
            With<crate::render::PrimaryGameCamera>,
            Without<crate::render::MapPreviewCamera>,
        ),
    >,
    mut state: ResMut<FlatWaterRasterFootprintState>,
    mut water: ParamSet<(
        Query<(&crate::render::WaterTile, &mut Sprite)>,
        Query<(&crate::render::WaterTile, &mut Sprite), Added<crate::render::WaterTile>>,
    )>,
) {
    let scale = cam_q
        .single()
        .ok()
        .and_then(|projection| match projection {
            Projection::Orthographic(orthographic) => Some(orthographic.scale),
            _ => None,
        })
        .unwrap_or(1.0);
    let custom_size = flat_water_raster_footprint(scale);
    let zoomed_out = custom_size.is_some();

    if state.zoomed_out != Some(zoomed_out) {
        state.zoomed_out = Some(zoomed_out);
        for (water, mut sprite) in &mut water.p0() {
            if water.is_palette_animated() {
                sprite.custom_size = custom_size;
            }
        }
    } else {
        for (water, mut sprite) in &mut water.p1() {
            if water.is_palette_animated() {
                sprite.custom_size = custom_size;
            }
        }
    }
}

#[must_use]
fn label_visual_scale(camera_scale: f32, overview: bool) -> f32 {
    camera_scale * if overview { SMALL_LABEL_SCALE } else { 1.0 }
}

/// Mantiene los carteles legibles al alejar la cámara.
///
/// Los dos nodos de cada cartel (fondo y texto) comparten `MapLabelLod`, de
/// modo que el fondo y el texto cambian de variante juntos. La escala compensa
/// el `OrthographicProjection`: el tamaño en pantalla deja de caer a un píxel
/// cuando el mapa entra en vista general.
pub(crate) fn sync_map_label_lod(
    cam_q: Query<
        &Projection,
        (
            With<crate::render::PrimaryGameCamera>,
            Without<crate::render::MapPreviewCamera>,
        ),
    >,
    mut labels: Query<(
        Entity,
        &crate::render::MapLabelLod,
        &mut Transform,
        &mut Visibility,
        Option<&mut Text2d>,
        Option<&crate::render::MapLabelText>,
        Option<&mut Sprite>,
    )>,
) {
    let scale = cam_q
        .single()
        .ok()
        .and_then(|projection| match projection {
            Projection::Orthographic(orthographic) => Some(orthographic.scale),
            _ => None,
        })
        .unwrap_or(1.0)
        .max(1.0);
    let overview = scale >= LABEL_LOD_ZOOM;

    for (_, lod, mut transform, mut visibility, text, text_meta, sprite) in &mut labels {
        // OpenTTD no elimina carteles por colisión: en Out4x/Out8x todos los
        // signos dentro del rectángulo de viewport se agregan al pase de
        // texto y pueden superponerse (visible en mapas densos). Restaurar la
        // visibilidad también corrige el retorno desde una captura anterior.
        *visibility = Visibility::Visible;
        let visual_scale = label_visual_scale(scale, overview);
        let base_size = if overview { lod.small_size } else { lod.size };
        transform.scale = Vec3::splat(visual_scale);
        if let Some(mut sprite) = sprite {
            sprite.custom_size = Some(base_size);
        }
        if let (Some(mut text), Some(meta)) = (text, text_meta) {
            let value = if overview { &meta.small } else { &meta.normal };
            if text.0 != *value {
                text.0.clone_from(value);
            }
        }
    }
}

/// Queries de etiquetas del mapa (agrupadas para no superar el límite de params Bevy).
#[derive(SystemParam)]
pub(crate) struct MapLabelEntities<'w, 's> {
    pub towns: Query<'w, 's, Entity, With<crate::render::town_labels::TownLabel>>,
    pub stations: Query<'w, 's, Entity, With<crate::render::station_labels::StationLabel>>,
    pub signs: Query<'w, 's, Entity, With<crate::render::sign_labels::SignLabel>>,
    pub spatial_index: ResMut<'w, crate::render::MapLabelSpatialIndex>,
}

/// Agrupa cachés NewGRF in-world para no superar el límite de 16 `SystemParam`.
#[derive(SystemParam)]
pub(crate) struct NewGrfMapSpriteCaches<'w> {
    pub road: ResMut<'w, crate::render::NewGrfRoadSpriteCache>,
    pub station: ResMut<'w, crate::render::NewGrfStationSpriteCache>,
    pub shore: ResMut<'w, crate::render::NewGrfShoreSpriteCache>,
    pub catenary: ResMut<'w, crate::render::NewGrfCatenarySpriteCache>,
    pub signal: ResMut<'w, crate::render::NewGrfSignalSpriteCache>,
    pub industry: ResMut<'w, crate::render::NewGrfIndustrySpriteCache>,
    pub house: ResMut<'w, crate::render::NewGrfHouseSpriteCache>,
    pub object: ResMut<'w, crate::render::NewGrfObjectSpriteCache>,
    pub action5: ResMut<'w, crate::render::NewGrfAction5SpriteCache>,
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
    /// Las etiquetas de pueblos, estaciones o carteles deben recalcularse aunque
    /// el conjunto de chunks visibles no cambie.
    ///
    /// Las etiquetas viven fuera de los chunks. No se marca para cambios de
    /// señales o reservas: esos cambios pueden ocurrir cada tick y no alteran
    /// ninguna etiqueta.
    pub(crate) labels_dirty: bool,
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
            labels_dirty: false,
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

/// Igual que [`request_map_visual_remap`], pero para una modificación que puede
/// crear, eliminar o renombrar una etiqueta del mapa.
pub(crate) fn request_map_visual_remap_with_labels(
    pending: &mut RemapMapVisualsPending,
    mw: u32,
    mh: u32,
    tiles: &[(i32, i32)],
) {
    request_map_visual_remap(pending, mw, mh, tiles);
    pending.labels_dirty = true;
}

/// Bloques 16×16 ya instanciados (solo mapas con culling por viewport).
///
/// `chunks` contiene únicamente bloques completos. Una carga inicial puede
/// dibujar un rectángulo cuyo borde corta chunks; esos se guardan en
/// `partial_chunks` y se completan antes de que el renderer los trate como
/// reutilizables en un paneo posterior.
#[derive(Resource, Default)]
pub(crate) struct LoadedMapTileChunks {
    pub chunks: HashSet<(u32, u32)>,
    pub partial_chunks: HashSet<(u32, u32)>,
}

/// Cambios de entidades necesarios para llevar un viewport a chunks completos.
pub(crate) struct IncrementalChunkRemapPlan {
    pub(crate) to_remove: HashSet<(u32, u32)>,
    pub(crate) to_add: HashSet<(u32, u32)>,
    pub(crate) to_despawn: HashSet<(u32, u32)>,
}

impl LoadedMapTileChunks {
    #[must_use]
    pub(crate) fn from_spawn_bounds(bounds: TileViewportBounds, mw: u32, mh: u32) -> Self {
        let mut loaded = Self::default();
        loaded.set_spawn_bounds(bounds, mw, mh);
        loaded
    }

    pub(crate) fn set_spawn_bounds(&mut self, bounds: TileViewportBounds, mw: u32, mh: u32) {
        self.chunks.clear();
        self.partial_chunks.clear();
        for (cx, cy) in chunks_in_bounds(bounds) {
            if bounds.contains(chunk_tile_bounds(cx, cy, mw, mh)) {
                self.chunks.insert((cx, cy));
            } else {
                self.partial_chunks.insert((cx, cy));
            }
        }
    }

    #[must_use]
    pub(crate) fn is_empty(&self) -> bool {
        self.chunks.is_empty() && self.partial_chunks.is_empty()
    }

    #[must_use]
    pub(crate) fn all_chunks(&self) -> HashSet<(u32, u32)> {
        self.chunks.union(&self.partial_chunks).copied().collect()
    }

    #[must_use]
    pub(crate) fn plan_incremental_remap(
        &self,
        needed: &HashSet<(u32, u32)>,
        refresh_chunks: &HashSet<(u32, u32)>,
    ) -> IncrementalChunkRemapPlan {
        let known = self.all_chunks();
        let to_remove: HashSet<_> = known.difference(needed).copied().collect();
        // Un parcial cuenta como ausente: se despawnea su fracción antigua y
        // se genera el bloque 16×16 completo en esta pasada.
        let to_add: HashSet<_> = needed.difference(&self.chunks).copied().collect();
        let to_upgrade = self
            .partial_chunks
            .intersection(needed)
            .copied()
            .collect::<HashSet<_>>();
        let mut to_despawn = to_remove.clone();
        to_despawn.extend(to_upgrade);
        to_despawn.extend(refresh_chunks.iter().copied());
        IncrementalChunkRemapPlan {
            to_remove,
            to_add,
            to_despawn,
        }
    }
}

/// Rectángulo de teselas para las que se generaron sprites (`MapVisualLayer`).
#[derive(Resource)]
pub(crate) struct MapTileSpawnViewport {
    pub(crate) bounds: TileViewportBounds,
    /// Último `OrthographicProjection::scale` usado para `bounds` (detectar zoom).
    pub(crate) last_ortho_scale: f32,
    /// Representación materializada del viewport: `None` es detalle y `Some`
    /// es el stride del overview (`Out4x`/`Out8x`). Cuando cambia, no alcanza
    /// con refrescar chunks: hay que reemplazar los sprites agregados por los
    /// detallados (o viceversa).
    pub(crate) last_overview_stride: Option<u32>,
}

impl Default for MapTileSpawnViewport {
    fn default() -> Self {
        Self {
            bounds: TileViewportBounds::full(1, 1),
            last_ortho_scale: 1.0,
            last_overview_stride: None,
        }
    }
}

pub(crate) struct WorldRenderPlugin;

impl Plugin for WorldRenderPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<RemapMapVisualsPending>()
            .init_resource::<MapTileSpawnViewport>()
            .init_resource::<FlatWaterRasterFootprintState>()
            .init_resource::<crate::render::ViewportSortableChildDepthWindows>()
            .init_resource::<crate::render::MapLabelSpatialIndex>()
            .init_resource::<LoadedMapTileChunks>()
            .init_resource::<crate::render::NewGrfRoadSpriteCache>()
            .init_resource::<crate::render::NewGrfStationSpriteCache>()
            .init_resource::<crate::render::NewGrfShoreSpriteCache>()
            .init_resource::<crate::render::NewGrfCatenarySpriteCache>()
            .init_resource::<crate::render::NewGrfSignalSpriteCache>()
            .init_resource::<crate::render::NewGrfIndustrySpriteCache>()
            .init_resource::<crate::render::NewGrfHouseSpriteCache>()
            .init_resource::<crate::render::NewGrfObjectSpriteCache>()
            .init_resource::<crate::render::NewGrfAction5SpriteCache>()
            .add_systems(OnEnter(ClientScreen::InGame), setup)
            .add_systems(
                Update,
                (
                    sync_map_tile_spawn_viewport,
                    super::remap::sync_company_colored_sprites,
                    apply_remap_map_visuals,
                    sort_viewport_sortable_parents,
                    sync_viewport_sortable_children,
                    sync_flat_water_raster_footprint,
                    sync_map_label_lod,
                )
                    .chain()
                    .in_set(UpdateSet::RenderRefresh)
                    .after(crate::camera::move_camera)
                    .run_if(in_state(ClientScreen::InGame)),
            );
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use bevy::ecs::system::RunSystemOnce;
    use bevy::prelude::{OrthographicProjection, Projection, Sprite, Vec2, World};

    use super::{
        FLAT_WATER_RASTER_FOOTPRINT, FlatWaterRasterFootprintState, LoadedMapTileChunks,
        flat_water_raster_footprint, sync_flat_water_raster_footprint,
    };
    use crate::render::{TileViewportBounds, chunks_in_bounds};

    #[test]
    fn small_label_scale_matches_out_levels() {
        assert_eq!(super::label_visual_scale(2.0, false), 2.0);
        assert!((super::label_visual_scale(8.0, true) - 5.6).abs() < f32::EPSILON);
    }

    #[test]
    fn flat_water_uses_the_full_logical_footprint_only_at_out_zooms() {
        for scale in [0.25, 0.5, 1.0] {
            assert_eq!(
                flat_water_raster_footprint(scale),
                None,
                "In4x/In2x/Normal conservan los 64×31 PNG nativos"
            );
        }
        for scale in [2.0, 4.0, 8.0] {
            assert_eq!(
                flat_water_raster_footprint(scale),
                Some(FLAT_WATER_RASTER_FOOTPRINT),
                "Out2x/Out4x/Out8x cierran la huella de agua a 64×32"
            );
        }
    }

    #[test]
    fn flat_water_footprint_tracks_zoom_and_newly_spawned_tiles() {
        let mut world = World::new();
        world.insert_resource(FlatWaterRasterFootprintState::default());
        let camera = world
            .spawn((
                crate::render::PrimaryGameCamera,
                Projection::Orthographic(OrthographicProjection {
                    scale: 2.0,
                    ..OrthographicProjection::default_2d()
                }),
            ))
            .id();
        let animated = world
            .spawn((crate::render::WaterTile::ANIMATED, Sprite::default()))
            .id();
        let static_water = world
            .spawn((
                crate::render::WaterTile::STATIC,
                Sprite {
                    custom_size: Some(Vec2::new(9.0, 9.0)),
                    ..Default::default()
                },
            ))
            .id();

        assert!(
            world
                .run_system_once(sync_flat_water_raster_footprint)
                .is_ok(),
            "sync a Out2x"
        );
        assert_eq!(
            world
                .get::<Sprite>(animated)
                .and_then(|sprite| sprite.custom_size),
            Some(FLAT_WATER_RASTER_FOOTPRINT)
        );
        assert_eq!(
            world
                .get::<Sprite>(static_water)
                .and_then(|sprite| sprite.custom_size),
            Some(Vec2::new(9.0, 9.0)),
            "una esclusa estática no adopta la geometría de agua animada"
        );

        let late = world
            .spawn((crate::render::WaterTile::ANIMATED, Sprite::default()))
            .id();
        assert!(
            world
                .run_system_once(sync_flat_water_raster_footprint)
                .is_ok(),
            "sync incremental a Out2x"
        );
        assert_eq!(
            world
                .get::<Sprite>(late)
                .and_then(|sprite| sprite.custom_size),
            Some(FLAT_WATER_RASTER_FOOTPRINT),
            "un chunk materializado tras el zoom debe conservar la huella"
        );

        let Some(mut projection) = world.get_mut::<Projection>(camera) else {
            panic!("cámara ortográfica");
        };
        let Projection::Orthographic(projection) = &mut *projection else {
            panic!("cámara ortográfica");
        };
        projection.scale = 1.0;
        assert!(
            world
                .run_system_once(sync_flat_water_raster_footprint)
                .is_ok(),
            "sync de vuelta a Normal"
        );
        for entity in [animated, late] {
            assert_eq!(
                world
                    .get::<Sprite>(entity)
                    .and_then(|sprite| sprite.custom_size),
                None,
                "Normal restaura la geometría exacta del PNG"
            );
        }
    }

    #[test]
    fn partial_boundary_chunks_are_not_marked_reusable() {
        let bounds = TileViewportBounds {
            tx0: 3,
            ty0: 16,
            tx1: 35,
            ty1: 34,
        };
        let loaded = LoadedMapTileChunks::from_spawn_bounds(bounds, 256, 256);

        // Sólo el bloque central 16×16 fue materializado por completo.
        assert_eq!(loaded.chunks.len(), 1);
        assert!(loaded.chunks.contains(&(1, 1)));
        assert_eq!(loaded.partial_chunks.len(), 5);
        assert!(loaded.partial_chunks.contains(&(0, 1)));
        assert!(loaded.partial_chunks.contains(&(2, 2)));
        assert_eq!(loaded.all_chunks().len(), 6);
        assert!(!loaded.is_empty());
    }

    #[test]
    fn incremental_plan_upgrades_a_partial_chunk_before_reuse() {
        let loaded = LoadedMapTileChunks::from_spawn_bounds(
            TileViewportBounds {
                tx0: 3,
                ty0: 16,
                tx1: 35,
                ty1: 34,
            },
            256,
            256,
        );
        let needed = chunks_in_bounds(TileViewportBounds {
            tx0: 0,
            ty0: 16,
            tx1: 48,
            ty1: 48,
        });
        let plan = loaded.plan_incremental_remap(&needed, &HashSet::new());

        // (0,1) ya tenía sprites para x=3..15, pero no para x=0..2.
        // Debe reemplazarse por el chunk completo, no tratarse como cargado.
        assert!(loaded.partial_chunks.contains(&(0, 1)));
        assert!(plan.to_add.contains(&(0, 1)));
        assert!(plan.to_despawn.contains(&(0, 1)));
        assert!(!plan.to_remove.contains(&(0, 1)));
    }
}
