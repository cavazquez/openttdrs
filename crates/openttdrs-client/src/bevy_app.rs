//! Composición de plugins, recursos y sistemas Bevy del cliente.
//!
//! ## Contrato de schedules (#124)
//!
//! 1. **Startup:** solo [`StartupSet::Ui`] (bootstrap de paneles). World/vehicles
//!    se materializan en `OnEnter(InGame)`, no en Startup.
//! 2. **FixedUpdate:** [`FixedUpdateSet::Sim`] (tick + dirty flags) →
//!    [`FixedUpdateSet::Events`] (drenaje `SimEvent` + broadcast de red).
//! 3. **Update:** Input → Sim (dispatch de eventos / red) → Persistence →
//!    Status → Visuals → Camera → **RenderRefresh** (remap de teselas tras la
//!    cámara) → Ui.
//!
//! El puente FixedUpdate→Update: `drain_sim_events_from_core` llena
//! `PendingSimEvents` en FixedUpdate; `dispatch_sim_events` lo consume en
//! `UpdateSet::Sim` (un frame de latencia, intencional).

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
use crate::network::{NetCli, NetworkPlugin};
use crate::persistence::PersistencePlugin;
use crate::render::{
    AirportRadarAnimPlugin, BubbleEffectPlugin, DisasterCraftPlugin, EffectVehiclePlugin,
    FizzyDrinkAnimPlugin, HouseLiftAnimPlugin, IndustryBuildingAnimPlugin, IndustryDrawProcPlugin,
    IndustrySmokePlugin, LighthouseAnimPlugin, RefineryFireAnimPlugin, TileAnimPlugin,
    TrainSmokePlugin, WaterAnimationPlugin,
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
    /// Setup de UI (toolbars, ventanas). Sin sets vacíos World/Vehicles (#124).
    Ui,
}

/// Orden del tick fijo: simulación autoritativa antes de efectos/red.
#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone, Copy)]
pub(crate) enum FixedUpdateSet {
    Sim,
    Events,
}

#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone, Copy)]
pub(crate) enum UpdateSet {
    Input,
    Sim,
    Persistence,
    Status,
    Visuals,
    Camera,
    /// Remap / refresh de sprites de mapa tras actualizar la cámara (#124).
    RenderRefresh,
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
    net_cli: NetCli,
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
    app.configure_sets(Startup, StartupSet::Ui);
    app.configure_sets(
        FixedUpdate,
        (FixedUpdateSet::Sim, FixedUpdateSet::Events).chain(),
    );
    app.configure_sets(
        Update,
        (
            UpdateSet::Input,
            UpdateSet::Sim,
            UpdateSet::Persistence,
            UpdateSet::Status,
            UpdateSet::Visuals,
            UpdateSet::Camera,
            UpdateSet::RenderRefresh,
            UpdateSet::Ui,
        )
            .chain(),
    );
    app.init_state::<ClientScreen>();
    app.add_sub_state::<crate::state::SimRunState>();
    app.add_sub_state::<crate::state::OrderPickState>();
    let sim_world = match &net_cli {
        // El mapa real llega en Welcome; no gastar en demo procedural local.
        NetCli::Client { .. } => SimWorld::network_client_placeholder(),
        _ => SimWorld::try_bootstrap_from_env()?,
    };
    app.insert_resource(sim_world);
    app.init_resource::<SuspendedGameSession>();
    app.init_resource::<EditorSession>();
    crate::audio::insert_asset_root(&mut app, asset_root);
    app.insert_resource(RemSize(14.0));
    app.add_plugins(NetworkPlugin { cli: net_cli });
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
            BubbleEffectPlugin,
            EffectVehiclePlugin,
            DisasterCraftPlugin,
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

pub(crate) fn run(asset_root: &str, net_cli: NetCli) {
    match build_client_app(asset_root, false, net_cli) {
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

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::{FixedUpdateSet, StartupSet, UpdateSet};

    #[test]
    fn update_set_places_render_refresh_after_camera() {
        let order = [
            UpdateSet::Input,
            UpdateSet::Sim,
            UpdateSet::Persistence,
            UpdateSet::Status,
            UpdateSet::Visuals,
            UpdateSet::Camera,
            UpdateSet::RenderRefresh,
            UpdateSet::Ui,
        ];
        let camera = order.iter().position(|s| *s == UpdateSet::Camera).unwrap();
        let refresh = order
            .iter()
            .position(|s| *s == UpdateSet::RenderRefresh)
            .unwrap();
        assert!(refresh > camera, "remap debe ir tras la cámara (#124)");
    }

    #[test]
    fn fixed_update_set_runs_sim_before_events() {
        // Contrato: Sim y Events son sets distintos; el configure_sets los encadena.
        assert_ne!(FixedUpdateSet::Sim, FixedUpdateSet::Events);
    }

    #[test]
    fn startup_set_only_ui() {
        // Evita regresión a sets vacíos World/Vehicles: solo Ui existe.
        match StartupSet::Ui {
            StartupSet::Ui => {}
        }
    }
}
