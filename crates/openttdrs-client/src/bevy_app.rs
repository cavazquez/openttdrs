//! Composición de plugins, recursos y sistemas Bevy del cliente.

use bevy::image::ImageSamplerDescriptor;
use bevy::prelude::*;

use crate::camera::CameraControlPlugin;
use crate::debug_gizmos::DebugGizmosPlugin;
use crate::persistence::PersistencePlugin;
use crate::render::WaterAnimationPlugin;
use crate::render::{VehicleRenderPlugin, WorldRenderPlugin};
use crate::simulation::SimulationPlugin;
use crate::state::{ClientScreen, SimWorld};
use crate::ui::ClientUiPlugin;
use crate::window_status::WindowStatusPlugin;

#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone, Copy)]
pub(crate) enum StartupSet {
    World,
    Vehicles,
    Ui,
}

#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone, Copy)]
pub(crate) enum UpdateSet {
    Input,
    Sim,
    Persistence,
    RenderRefresh,
    Status,
    Visuals,
    Camera,
    Ui,
}

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
        .configure_sets(
            Startup,
            (StartupSet::World, StartupSet::Vehicles, StartupSet::Ui).chain(),
        )
        .configure_sets(
            Update,
            (
                UpdateSet::Input,
                UpdateSet::Sim,
                UpdateSet::Persistence,
                UpdateSet::RenderRefresh,
                UpdateSet::Status,
                UpdateSet::Visuals,
                UpdateSet::Camera,
                UpdateSet::Ui,
            )
                .chain(),
        )
        .init_state::<ClientScreen>()
        .init_resource::<SimWorld>()
        .add_plugins((
            WorldRenderPlugin,
            VehicleRenderPlugin,
            ClientUiPlugin,
            SimulationPlugin,
            PersistencePlugin,
            WindowStatusPlugin,
            WaterAnimationPlugin,
            DebugGizmosPlugin,
            CameraControlPlugin,
        ))
        .run();
}
