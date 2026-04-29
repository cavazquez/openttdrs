//! Composición de plugins, recursos y sistemas Bevy del cliente.

use bevy::image::ImageSamplerDescriptor;
use bevy::prelude::*;

use crate::camera::{CameraVelocity, move_camera};
use crate::debug_gizmos::{draw_industries, draw_stations};
use crate::persistence::handle_sim_json_hotkeys;
use crate::render::animate_water;
use crate::simulation::advance_sim;
use crate::state::SimWorld;
use crate::ui::{
    SelectedTileInfo, SimHudControls, build_menu_interaction, cycle_json_save_path_hotkey,
    handle_pause_toggle, handle_tile_click, setup_build_menu, setup_tile_info_ui,
    update_tile_info_text,
};
use crate::vehicle_render::{VehicleIndex, rebuild_vehicle_index, update_vehicles};
use crate::window_status::sync_window_title;
use crate::world_render::{RemapMapVisualsPending, apply_remap_map_visuals, setup};

pub(crate) fn run(asset_root: &str) {
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "openttdrs".into(),
                        resolution: (1280_u32, 720_u32).into(),
                        ..default()
                    }),
                    ..default()
                })
                .set(AssetPlugin {
                    file_path: asset_root.into(),
                    ..default()
                })
                .set(ImagePlugin {
                    default_sampler: ImageSamplerDescriptor::nearest(),
                }),
        )
        .init_resource::<SimWorld>()
        .init_resource::<SelectedTileInfo>()
        .init_resource::<CameraVelocity>()
        .init_resource::<VehicleIndex>()
        .init_resource::<RemapMapVisualsPending>()
        .init_resource::<SimHudControls>()
        .add_systems(
            Startup,
            (
                setup,
                rebuild_vehicle_index,
                setup_tile_info_ui,
                setup_build_menu,
            )
                .chain(),
        )
        .add_systems(
            Update,
            (
                handle_pause_toggle,
                cycle_json_save_path_hotkey,
                advance_sim,
                handle_sim_json_hotkeys,
                apply_remap_map_visuals,
                sync_window_title,
                update_vehicles,
                animate_water,
                draw_industries,
                draw_stations,
                move_camera,
                build_menu_interaction,
                handle_tile_click,
                update_tile_info_text,
            )
                .chain(),
        )
        .run();
}
