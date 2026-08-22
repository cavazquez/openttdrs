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
    use std::fs;

    use super::*;
    use bevy::app::ScheduleRunnerPlugin;
    use bevy::asset::AssetPlugin;
    use bevy::ecs::system::RunSystemOnce;
    use bevy::image::ImagePlugin;
    use bevy::prelude::*;

    use super::tile_spawn::setup;
    use crate::render::assets::stub_opengfx_tiles_for_tests;
    use crate::render::vehicles::VehicleIndex;
    use crate::render::viewport::{
        VIEWPORT_MARGIN_TILES, VIEWPORT_REBUILD_LEAD_TILES, ortho_visible_tile_bounds,
    };
    use crate::render::{MapPreviewCamera, PrimaryGameCamera};
    use crate::state::SimWorld;

    fn with_assets_app() -> App {
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
        app.init_asset::<TextureAtlasLayout>();
        app.update();
        app.insert_resource(SimWorld::default());
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
            pending.pending = true;
            pending.sync_camera = true;
        }
        world
            .run_system_once(remap::apply_remap_map_visuals)
            .unwrap();
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
        let mut projections = world.query_filtered::<&Projection, With<PrimaryGameCamera>>();
        let Projection::Orthographic(projection) = projections.single(world).expect("proyección")
        else {
            panic!("la cámara del mundo debe ser ortográfica");
        };
        assert_eq!(projection.near, super::tile_spawn::WORLD_CAMERA_NEAR);
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
