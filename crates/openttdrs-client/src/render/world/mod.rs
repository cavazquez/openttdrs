//! Sistemas Bevy que construyen y refrescan la capa visual del mundo.

mod plugin;
mod remap;
mod tile_spawn;
mod viewport;

// Re-exports públicos para que el resto del crate pueda seguir usando los mismos símbolos
pub(crate) use plugin::{
    LoadedMapTileChunks, MapTileSpawnViewport, RemapMapVisualsPending, WorldRenderPlugin,
    request_map_visual_remap, request_map_visual_remap_with_labels,
};
pub(crate) use tile_spawn::spawn_intro_map_render;
pub(crate) use viewport::initial_map_camera_pose;

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use std::collections::HashSet;
    use std::fs;

    use super::*;
    use bevy::app::ScheduleRunnerPlugin;
    use bevy::asset::AssetPlugin;
    use bevy::ecs::system::RunSystemOnce;
    use bevy::image::ImagePlugin;
    use bevy::prelude::*;
    use openttdrs_core::prelude::{TileCoord, TileKind, Vehicle, VehicleKind};

    use super::tile_spawn::setup;
    use crate::iso::ground_draw_z;
    use crate::render::assets::stub_opengfx_tiles_for_tests;
    use crate::render::vehicles::VehicleIndex;
    use crate::render::viewport::{
        VIEWPORT_MARGIN_TILES, VIEWPORT_REBUILD_LEAD_TILES, ortho_visible_tile_bounds,
    };
    use crate::render::{
        MapPreviewCamera, MapTileChunk, MapVisualLayer, PrimaryGameCamera, VehicleSprite,
    };
    use crate::state::SimWorld;

    fn with_assets_app() -> App {
        with_assets_app_for_map(64, 64)
    }

    fn with_assets_app_for_map(width: u32, height: u32) -> App {
        let dir = tempfile::tempdir().expect("tempdir");
        stub_opengfx_tiles_for_tests(dir.path());
        let root = dir.path().to_str().expect("utf8");

        let mut app = App::new();
        app.add_plugins(MinimalPlugins.set(ScheduleRunnerPlugin::run_once()));
        app.add_plugins(AssetPlugin {
            file_path: root.into(),
            ..default()
        });
        app.add_plugins(ImagePlugin::default());
        // Las etiquetas de ciudades cargan la fuente Text2d en spawn_world_layer.
        app.init_asset::<Font>();
        app.init_asset::<Mesh>();
        app.init_asset::<ColorMaterial>();
        app.init_asset::<TextureAtlasLayout>();
        app.update();
        app.insert_resource(SimWorld {
            state: openttdrs_core::GameState::new(width, height),
            loaded_file: false,
            ottdmap_extras: None,
        });
        app.insert_resource(crate::settings::ClientPreferences::default());
        app.insert_resource(RemapMapVisualsPending::default());
        app.insert_resource(VehicleIndex::default());
        app.insert_resource(LoadedMapTileChunks::default());
        app
    }

    #[test]
    fn setup_and_apply_remap_execute_main_paths() {
        let mut app = with_assets_app();
        let world = app.world_mut();

        world.run_system_once(setup).unwrap();
        {
            let mut pending = world.resource_mut::<RemapMapVisualsPending>();
            pending.request_full_and_sync_camera();
        }
        world
            .run_system_once(remap::apply_remap_map_visuals)
            .unwrap();
    }

    #[test]
    fn setup_and_apply_remap_covers_multiple_fixed_zoom_levels() {
        let mut app = with_assets_app();
        let world = app.world_mut();
        world.run_system_once(setup).unwrap();

        // Cubrir la matriz completa de OpenTTD, incluidos In4x/In2x. Los
        // niveles de alejamiento pasan por el camino detallado u overview,
        // pero todos deben materializar al menos una capa de mapa.
        for scale in [0.25_f32, 0.5, 1.0, 2.0, 4.0, 8.0] {
            {
                let mut cameras =
                    world.query_filtered::<&mut Projection, With<PrimaryGameCamera>>();
                let mut projection = cameras.single_mut(world).expect("cámara principal");
                let Projection::Orthographic(projection) = &mut *projection else {
                    panic!("la cámara del mundo debe ser ortográfica");
                };
                projection.scale = scale;
            }
            {
                let mut pending = world.resource_mut::<RemapMapVisualsPending>();
                pending.request_full();
            }
            world
                .run_system_once(remap::apply_remap_map_visuals)
                .unwrap();

            let mut map_sprites =
                world.query_filtered::<(&MapTileChunk, &Sprite), With<MapVisualLayer>>();
            assert!(
                map_sprites.iter(world).next().is_some(),
                "el nivel de zoom {scale} no materializó sprites del mapa"
            );
        }
    }

    fn map_layer_entities_for_chunk(world: &mut World, chunk: (u32, u32)) -> HashSet<Entity> {
        let mut layers = world.query_filtered::<(Entity, &MapTileChunk), With<MapVisualLayer>>();
        layers
            .iter(world)
            .filter_map(|(entity, tile_chunk)| {
                ((tile_chunk.cx, tile_chunk.cy) == chunk).then_some(entity)
            })
            .collect()
    }

    fn map_layer_count(world: &mut World) -> usize {
        let mut layers = world.query_filtered::<Entity, With<MapVisualLayer>>();
        layers.iter(world).count()
    }

    fn move_primary_camera(world: &mut World, delta: Vec3) {
        let mut cameras = world.query_filtered::<&mut Transform, With<PrimaryGameCamera>>();
        let mut transform = cameras.single_mut(world).expect("cámara principal");
        transform.translation += delta;
    }

    #[test]
    fn viewport_pan_uses_incremental_remap_and_reuses_shared_chunks() {
        // 256² activa culling real y deja margen para panear sin que el
        // viewport inicial cubra el mapa completo.
        let mut app = with_assets_app_for_map(256, 256);
        let world = app.world_mut();
        world
            .resource_mut::<SimWorld>()
            .state
            .vehicles
            .push(Vehicle::new(
                77,
                VehicleKind::Bus,
                TileCoord::new(128, 128),
                TileCoord::new(129, 128),
            ));
        world.run_system_once(setup).expect("setup del mapa grande");

        let initial_full_chunks = world.resource::<LoadedMapTileChunks>().chunks.clone();
        let vehicle_before: HashSet<Entity> = {
            let mut vehicles = world.query_filtered::<Entity, With<VehicleSprite>>();
            vehicles.iter(world).collect()
        };
        assert_eq!(
            vehicle_before.len(),
            1,
            "fixture con vehículo materializado"
        );
        move_primary_camera(world, Vec3::new(1_024.0, 0.0, 0.0));
        world
            .run_system_once(viewport::sync_map_tile_spawn_viewport)
            .expect("paneo de viewport");

        let needed =
            crate::render::chunks_in_bounds(world.resource::<MapTileSpawnViewport>().bounds);
        let shared_chunk = initial_full_chunks
            .intersection(&needed)
            .copied()
            .next()
            .expect("el paneo conserva al menos un chunk completo");
        let shared_before = map_layer_entities_for_chunk(world, shared_chunk);
        assert!(
            !shared_before.is_empty(),
            "el chunk completo elegido debe tener sprites materializados"
        );
        {
            let pending = world.resource::<RemapMapVisualsPending>();
            assert!(pending.is_pending());
            assert!(
                !pending.is_full(),
                "pan detallado no debe pedir reconstrucción completa"
            );
            assert!(pending.labels_dirty_requested());
        }
        world
            .run_system_once(remap::apply_remap_map_visuals)
            .expect("remapeo incremental tras paneo");
        assert_eq!(
            map_layer_entities_for_chunk(world, shared_chunk),
            shared_before,
            "un chunk completo compartido conserva sus entidades ECS"
        );
        let vehicle_after: HashSet<Entity> = {
            let mut vehicles = world.query_filtered::<Entity, With<VehicleSprite>>();
            vehicles.iter(world).collect()
        };
        assert_eq!(
            vehicle_after, vehicle_before,
            "el paneo incremental no reinstancia vehículos ni sus cachés visuales"
        );

        // Tras estabilizar ambos extremos, volver a panear no puede acumular
        // entidades fuera del viewport. Es la ruta que antes degradaba una
        // carga/petición completa cuando se mezclaba con el paneo.
        move_primary_camera(world, Vec3::new(-1_024.0, 0.0, 0.0));
        world
            .run_system_once(viewport::sync_map_tile_spawn_viewport)
            .expect("paneo de retorno");
        world
            .run_system_once(remap::apply_remap_map_visuals)
            .expect("remapeo de retorno");
        let settled_count = map_layer_count(world);
        move_primary_camera(world, Vec3::new(1_024.0, 0.0, 0.0));
        world
            .run_system_once(viewport::sync_map_tile_spawn_viewport)
            .expect("segundo paneo");
        world
            .run_system_once(remap::apply_remap_map_visuals)
            .expect("segundo remapeo");
        move_primary_camera(world, Vec3::new(-1_024.0, 0.0, 0.0));
        world
            .run_system_once(viewport::sync_map_tile_spawn_viewport)
            .expect("segundo retorno");
        world
            .run_system_once(remap::apply_remap_map_visuals)
            .expect("segundo remapeo de retorno");
        assert_eq!(
            map_layer_count(world),
            settled_count,
            "panear ida/vuelta no debe dejar capas visuales acumuladas"
        );
    }

    #[test]
    fn same_size_hot_load_replaces_existing_map_visuals() {
        let mut app = with_assets_app();
        let world = app.world_mut();
        world
            .resource_mut::<SimWorld>()
            .state
            .map
            .set_kind(TileCoord::new(1, 1), TileKind::Water)
            .expect("tesela del mundo previo");
        world.run_system_once(setup).expect("setup inicial");
        let old_layers: HashSet<Entity> = {
            let mut layers = world.query_filtered::<Entity, With<MapVisualLayer>>();
            layers.iter(world).collect()
        };
        assert!(!old_layers.is_empty());
        {
            let mut pending = world.resource_mut::<RemapMapVisualsPending>();
            pending.request_incremental();
            pending.extend_refresh_chunks(&[(1, 1)]);
        }

        world
            .run_system_once(
                |mut sim: ResMut<SimWorld>,
                 mut vehicle_index: ResMut<VehicleIndex>,
                 mut pending: ResMut<RemapMapVisualsPending>,
                 mut commands: Commands| {
                    let mut loaded = openttdrs_core::GameState::new(64, 64);
                    loaded
                        .map
                        .set_kind(TileCoord::new(2, 2), TileKind::Water)
                        .expect("tesela del mundo cargado");
                    crate::persistence::apply_loaded_state(
                        &mut sim,
                        &mut vehicle_index,
                        &mut pending,
                        &mut commands,
                        loaded,
                    );
                },
            )
            .expect("carga en caliente");
        {
            let pending = world.resource::<RemapMapVisualsPending>();
            assert!(pending.is_pending());
            assert!(pending.is_full());
            assert!(pending.sync_camera_requested());
        }
        world
            .run_system_once(remap::apply_remap_map_visuals)
            .expect("remapeo completo tras carga");

        assert!(
            old_layers
                .iter()
                .all(|entity| world.get_entity(*entity).is_err()),
            "una carga de igual tamaño no puede conservar capas de la partida previa"
        );
        let mut layers = world.query_filtered::<Entity, With<MapVisualLayer>>();
        assert!(
            layers.iter(world).next().is_some(),
            "la nueva partida se dibuja"
        );
    }

    #[test]
    fn primary_world_camera_disables_msaa_for_pixel_exact_composition() {
        let mut app = with_assets_app();
        let world = app.world_mut();
        world.run_system_once(setup).unwrap();

        let mut cameras = world.query_filtered::<&Msaa, With<PrimaryGameCamera>>();
        assert_eq!(cameras.single(world).expect("cámara principal"), &Msaa::Off);
    }

    #[test]
    fn primary_world_camera_keeps_openttd_black_outside_map() {
        let mut app = with_assets_app();
        let world = app.world_mut();
        world.run_system_once(setup).unwrap();

        let mut cameras = world.query_filtered::<&Camera, With<PrimaryGameCamera>>();
        let camera = cameras.single(world).expect("cámara principal");
        assert!(
            matches!(camera.clear_color, ClearColorConfig::Custom(color) if color == Color::BLACK)
        );
        let (camera_near, camera_far) = {
            let mut projections = world.query_filtered::<&Projection, With<PrimaryGameCamera>>();
            let Projection::Orthographic(projection) =
                projections.single(world).expect("proyección")
            else {
                panic!("la cámara del mundo debe ser ortográfica");
            };
            (projection.near, projection.far)
        };
        assert_eq!(camera_near, super::tile_spawn::WORLD_CAMERA_NEAR);
        assert_eq!(camera_far, super::tile_spawn::WORLD_CAMERA_FAR);
        let mut cameras = world.query_filtered::<&Transform, With<PrimaryGameCamera>>();
        let camera_z = cameras
            .single(world)
            .expect("transformación de cámara")
            .translation
            .z;
        assert!(
            ground_draw_z(0, 0, 0.0) >= camera_z - camera_far,
            "el pase de suelo quedó fuera del plano delantero"
        );
        assert!(
            ground_draw_z(0, 0, 0.0) <= camera_z - camera_near,
            "el pase de suelo quedó fuera del plano trasero"
        );
    }

    /// Entrada automatizable del candidato para el contrato `world-draw`.
    ///
    /// No requiere ventana ni GPU: los stubs del atlas alcanzan porque la
    /// traza se toma antes de convertir el ID lógico a una textura. Se deja
    /// `ignore` para que el test normal no dependa de una partida local; el
    /// script `export_openttdrs_world_draw.sh` lo invoca explícitamente.
    #[test]
    #[ignore = "requiere OPENTTDRS_WORLD_DRAW_SAV y OPENTTDRS_WORLD_DRAW_OUT"]
    fn world_draw_trace_exports_requested_sav() {
        let sav = std::env::var("OPENTTDRS_WORLD_DRAW_SAV")
            .expect("OPENTTDRS_WORLD_DRAW_SAV debe apuntar a una partida .sav");
        let out = std::env::var("OPENTTDRS_WORLD_DRAW_OUT")
            .expect("OPENTTDRS_WORLD_DRAW_OUT debe indicar el JSONL de salida");

        let mut app = with_assets_app();
        let world = SimWorld::load_sav_file(&sav).expect("cargar partida .sav");
        assert!(
            world
                .state
                .runtime
                .foundation_newgrf_sprites
                .iter()
                .any(Option::is_some),
            "la carga directa del SAV debe rehidratar los cimientos Action5 base"
        );
        app.insert_resource(world);
        app.world_mut()
            .run_system_once(setup)
            .expect("spawn headless del mapa");

        let contents = fs::read_to_string(&out).expect("world-draw JSONL escrito");
        assert!(
            contents
                .lines()
                .next()
                .is_some_and(|row| row.contains("world-draw"))
        );
        assert!(
            contents
                .lines()
                .last()
                .is_some_and(|row| row.contains("\"kind\":\"complete\""))
        );
    }

    #[test]
    fn tile_kind_name_covers_all_variants() {
        use openttdrs_core::TileKind;

        for kind in [
            TileKind::Void,
            TileKind::Grass,
            TileKind::Water,
            TileKind::Road,
            TileKind::Rail,
            TileKind::RoadDepot,
            TileKind::RailDepot,
            TileKind::RoadTunnel,
            TileKind::RailTunnel,
            TileKind::RoadBridge,
            TileKind::RailBridge,
            TileKind::House,
            TileKind::Industry,
            TileKind::Station,
            TileKind::Forest,
            TileKind::CoalField,
            TileKind::Unknown(3),
        ] {
            assert!(!tile_spawn::tile_kind_name(kind).is_empty());
        }
    }

    #[test]
    fn apply_remap_returns_early_when_pending_false() {
        let mut app = with_assets_app();
        let world = app.world_mut();
        world.run_system_once(setup).unwrap();
        world
            .run_system_once(remap::apply_remap_map_visuals)
            .unwrap();
    }

    #[test]
    fn large_map_spawn_viewport_covers_fewer_tiles_than_full_map() {
        let bounds = ortho_visible_tile_bounds(
            Vec2::new(0.0, -200.0),
            2.0,
            1280.0,
            720.0,
            256,
            256,
            VIEWPORT_MARGIN_TILES,
        )
        .expand(VIEWPORT_REBUILD_LEAD_TILES, 256, 256);
        assert!(bounds.tile_count() < 256 * 256);
        assert!(bounds.tile_count() > 100);
    }

    #[test]
    fn sync_camera_for_sim_handles_camera_query_variants() {
        let mut world = World::new();
        let sim = SimWorld {
            loaded_file: true,
            ..SimWorld::default()
        };
        world.insert_resource(sim);

        // Sin cámara: no debe panicar.
        world
            .run_system_once(
                |sim: Res<SimWorld>,
                 mut q_cam: Query<
                    (&mut Transform, &mut Projection),
                    (With<PrimaryGameCamera>, Without<MapPreviewCamera>),
                >| {
                    viewport::sync_camera_for_sim(&mut q_cam, &sim);
                },
            )
            .unwrap();

        // Cámara ortográfica: debe ajustar escala/transform.
        world.spawn((
            PrimaryGameCamera,
            Transform::default(),
            Projection::Orthographic(OrthographicProjection::default_2d()),
        ));
        world
            .run_system_once(
                |sim: Res<SimWorld>,
                 mut q_cam: Query<
                    (&mut Transform, &mut Projection),
                    (With<PrimaryGameCamera>, Without<MapPreviewCamera>),
                >| {
                    viewport::sync_camera_for_sim(&mut q_cam, &sim);
                },
            )
            .unwrap();

        // Cámara no ortográfica: sigue sin panicar (sale por early return).
        world.spawn((
            PrimaryGameCamera,
            Transform::default(),
            Projection::Perspective(PerspectiveProjection::default()),
        ));
        world
            .run_system_once(
                |sim: Res<SimWorld>,
                 mut q_cam: Query<
                    (&mut Transform, &mut Projection),
                    (With<PrimaryGameCamera>, Without<MapPreviewCamera>),
                >| {
                    viewport::sync_camera_for_sim(&mut q_cam, &sim);
                },
            )
            .unwrap();
    }
}
