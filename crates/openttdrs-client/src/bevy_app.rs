//! Composición de plugins, recursos y sistemas Bevy del cliente.

use std::time::Duration;

use bevy::app::{ScheduleRunnerPlugin, TaskPoolPlugin};
use bevy::image::ImageSamplerDescriptor;
use bevy::prelude::*;
use bevy::window::ExitCondition;
use bevy::winit::WinitPlugin;

use crate::app_icon::AppIconPlugin;
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

/// `headless`: sin ventana primaria (tests / cobertura en CI); evita que el proceso termine al no
/// haber ventanas (`ExitCondition::DontExit`).
pub(crate) fn build_client_app(asset_root: &str, headless: bool) -> App {
    let window_plugin = if headless {
        WindowPlugin {
            primary_window: None,
            primary_cursor_options: None,
            exit_condition: ExitCondition::DontExit,
            close_when_requested: false,
        }
    } else {
        WindowPlugin {
            primary_window: Some(Window {
                title: "openttdrs".into(),
                name: Some("openttdrs".into()),
                resolution: (1280_u32, 720_u32).into(),
                ..default()
            }),
            ..default()
        }
    };

    let mut default_plugins = DefaultPlugins
        .set(window_plugin)
        .set(AssetPlugin {
            file_path: asset_root.into(),
            ..default()
        })
        .set(ImagePlugin {
            default_sampler: ImageSamplerDescriptor::nearest(),
        });
    if headless {
        // `cargo test` corre en hilos secundarios; winit exige el hilo principal salvo que se
        // desactive el plugin y se use un runner de schedules (igual que `MinimalPlugins`).
        default_plugins = default_plugins
            .disable::<WinitPlugin>()
            .add_after::<TaskPoolPlugin>(ScheduleRunnerPlugin::run_loop(Duration::from_secs_f64(
                1.0 / 60.0,
            )));
    }

    let mut app = App::new();
    app.add_plugins(default_plugins);
    app.configure_sets(
        Startup,
        (StartupSet::World, StartupSet::Vehicles, StartupSet::Ui).chain(),
    );
    app.configure_sets(
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
    );
    app.init_state::<ClientScreen>();
    app.init_resource::<SimWorld>();
    app.add_plugins((
        WorldRenderPlugin,
        VehicleRenderPlugin,
        ClientUiPlugin,
        SimulationPlugin,
        PersistencePlugin,
        WindowStatusPlugin,
        WaterAnimationPlugin,
        DebugGizmosPlugin,
        CameraControlPlugin,
    ));
    if !headless {
        app.add_plugins(AppIconPlugin::new(asset_root));
    }
    app
}

pub(crate) fn run(asset_root: &str) {
    build_client_app(asset_root, false).run();
}
