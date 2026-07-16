//! Composición de plugins, recursos y sistemas Bevy del cliente.

use std::time::Duration;

use bevy::app::{ScheduleRunnerPlugin, TaskPoolPlugin};
use bevy::audio::AudioPlugin;
use bevy::image::ImageSamplerDescriptor;
use bevy::input_focus::tab_navigation::TabNavigationPlugin;
use bevy::prelude::*;
use bevy::text::RemSize;
use bevy::window::ExitCondition;
use bevy::winit::WinitPlugin;

use crate::app_icon::AppIconPlugin;
use crate::audio::{MusicPlugin, SimEventsPlugin, WorldSfxPlugin};
use crate::camera::CameraControlPlugin;
use crate::debug_gizmos::DebugGizmosPlugin;
use crate::persistence::PersistencePlugin;
use crate::render::{
    AirportRadarAnimPlugin, EffectVehiclePlugin, FizzyDrinkAnimPlugin, HouseLiftAnimPlugin,
    IndustryBuildingAnimPlugin, IndustryDrawProcPlugin, IndustrySmokePlugin, LighthouseAnimPlugin,
    RefineryFireAnimPlugin, TileAnimPlugin, TrainSmokePlugin, WaterAnimationPlugin,
};
use crate::render::{VehicleRenderPlugin, WorldRenderPlugin};
use crate::render_trace::RenderTracePlugin;
use crate::settings::{ClientSettingsPlugin, patch_window_plugin_for_settings};
use crate::simulation::SimulationPlugin;
use crate::state::{
    BootstrapLoadError, ClientScreen, EditorSession, SimWorld, SuspendedGameSession,
};
#[cfg(target_os = "linux")]
use crate::tray::TrayIconPlugin;
use crate::ui::ClientUiPlugin;
use crate::ui::font::sync_rem_size_from_window;
use crate::window_status::WindowStatusPlugin;

/// Identificador estable para `SettingsPlugin` (alineado con `repository` del workspace).
pub(crate) const CLIENT_SETTINGS_APP_ID: &str = "com.github.cavazquez.openttdrs";

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
///
/// Si `OTTDJSON_LOAD` / `OTTDMAP_FILE` están definidos y fallan, devuelve el error tipado
/// (sin caer a partida procedural).
pub(crate) fn build_client_app(
    asset_root: &str,
    headless: bool,
) -> Result<App, BootstrapLoadError> {
    let window_plugin = if headless {
        WindowPlugin {
            primary_window: None,
            primary_cursor_options: None,
            exit_condition: ExitCondition::DontExit,
            close_when_requested: false,
        }
    } else {
        patch_window_plugin_for_settings(WindowPlugin {
            primary_window: Some(Window {
                title: "openttdrs".into(),
                name: Some("openttdrs".into()),
                resolution: (1280_u32, 720_u32).into(),
                ..default()
            }),
            ..default()
        })
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
            .disable::<AudioPlugin>()
            .add_after::<TaskPoolPlugin>(ScheduleRunnerPlugin::run_loop(Duration::from_secs_f64(
                1.0 / 60.0,
            )));
    }

    let mut app = App::new();
    app.add_plugins(default_plugins);
    {
        use std::path::Path;

        let mut fonts = app.world_mut().resource_mut::<Assets<Font>>();
        crate::ui::font::install_utf8_default_font_into_assets(&mut fonts, Path::new(asset_root));
    }
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
    app.add_sub_state::<crate::state::SimRunState>();
    app.add_sub_state::<crate::state::OrderPickState>();
    let sim_world = SimWorld::try_bootstrap_from_env()?;
    app.insert_resource(sim_world);
    app.init_resource::<SuspendedGameSession>();
    app.init_resource::<EditorSession>();
    crate::audio::insert_asset_root(&mut app, asset_root);
    app.insert_resource(RemSize(14.0));
    app.add_plugins((
        (
            ClientSettingsPlugin,
            TabNavigationPlugin,
            WorldRenderPlugin,
            VehicleRenderPlugin,
            ClientUiPlugin,
            SimulationPlugin,
            SimEventsPlugin,
            WorldSfxPlugin,
        ),
        (
            MusicPlugin,
            PersistencePlugin,
            WindowStatusPlugin,
            WaterAnimationPlugin,
            RefineryFireAnimPlugin,
            FizzyDrinkAnimPlugin,
            LighthouseAnimPlugin,
            HouseLiftAnimPlugin,
            IndustrySmokePlugin,
            IndustryBuildingAnimPlugin,
            AirportRadarAnimPlugin,
        ),
        (
            IndustryDrawProcPlugin,
            TrainSmokePlugin,
            EffectVehiclePlugin,
            TileAnimPlugin,
            RenderTracePlugin,
        ),
    ));
    app.add_plugins((DebugGizmosPlugin, CameraControlPlugin));
    if !headless {
        app.add_plugins(AppIconPlugin::new(asset_root));
        #[cfg(target_os = "linux")]
        app.add_plugins(TrayIconPlugin::new(asset_root));
        app.add_systems(Update, sync_rem_size_from_window.in_set(UpdateSet::Status));
    }
    Ok(app)
}

pub(crate) fn run(asset_root: &str) {
    match build_client_app(asset_root, false) {
        Ok(mut app) => {
            app.run();
        }
        Err(err) => {
            error!("{err}");
            eprintln!("error: {err}");
            std::process::exit(1);
        }
    }
}
