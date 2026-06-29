//! Verificación visual automatizada de las ventanas flotantes.
//!
//! Con `OPENTTDRS_WINDOWS_SHOT=/ruta/captura.png` el cliente abre las
//! ventanas de pueblo, depósito, compra y vehículo sobre el estado cargado,
//! guarda una captura y sale. Pensado para comparar contra screenshots del
//! `OpenTTD` oficial sin interacción manual.

use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use openttdrs_core::{TileCoord, TileKind};

use crate::state::{ClientScreen, SimWorld};
use crate::ui::buy_window::BuyVehicleWindowState;
use crate::ui::main_menu::{MainMenuCamera, MainMenuUi};
use crate::ui::toolbar::DepotPanelState;
use crate::ui::town_window::TownWindowState;
use crate::ui::vehicle_window::VehicleWindowState;

const OPEN_FRAME: u32 = 30;
const SHOT_FRAME: u32 = 60;
const EXIT_FRAME: u32 = 120;

pub(crate) struct WindowsShotPlugin;

impl Plugin for WindowsShotPlugin {
    fn build(&self, app: &mut App) {
        if std::env::var_os("OPENTTDRS_WINDOWS_SHOT").is_some() {
            app.add_systems(
                Update,
                (
                    auto_start_game.run_if(in_state(ClientScreen::MainMenu)),
                    windows_shot_driver.run_if(in_state(ClientScreen::InGame)),
                ),
            );
        } else if std::env::var_os("OPENTTDRS_MAP_SHOT").is_some() {
            app.add_systems(
                Update,
                (
                    auto_start_game.run_if(in_state(ClientScreen::MainMenu)),
                    map_shot_driver
                        .run_if(in_state(ClientScreen::InGame))
                        // El fantasma de obra lee el cursor que fija el driver.
                        .before(crate::ui::toolbar::update_build_ghost_preview),
                ),
            );
        }
    }
}

/// Salta el menú principal directamente al juego (igual que pulsar «Jugar»).
fn auto_start_game(
    mut commands: Commands,
    q_menu: Query<Entity, With<MainMenuUi>>,
    q_menu_cam: Query<Entity, With<MainMenuCamera>>,
    mut next_screen: ResMut<NextState<ClientScreen>>,
) {
    for e in &q_menu {
        commands.entity(e).despawn();
    }
    for cam in &q_menu_cam {
        commands.entity(cam).despawn();
    }
    next_screen.set(ClientScreen::InGame);
}

fn first_depot(sim: &SimWorld) -> Option<TileCoord> {
    let (w, h) = sim.state.map.dimensions();
    for y in 0..h {
        for x in 0..w {
            let pos = TileCoord::new(x.cast_signed(), y.cast_signed());
            if matches!(
                sim.state.map.get_kind(pos),
                Some(TileKind::RoadDepot | TileKind::RailDepot)
            ) {
                return Some(pos);
            }
        }
    }
    None
}

/// Con `OPENTTDRS_MAP_SHOT=/ruta.png`: captura el mapa sin abrir ventanas y sale.
/// Con `OPENTTDRS_MAP_SHOT_TOOL=rail|rail_x|rail_y` activa además esa herramienta
/// y fija el cursor al centro de la ventana para capturar el fantasma de obra.
#[allow(clippy::too_many_arguments)] // sistema ECS Bevy
fn map_shot_driver(
    mut commands: Commands,
    mut frame: Local<u32>,
    mut windows: Query<&mut Window, With<bevy::window::PrimaryWindow>>,
    mut tool_state: ResMut<crate::ui::toolbar::UiToolState>,
    mut toolbar_state: ResMut<crate::ui::toolbar::ToolbarState>,
    mut station_state: ResMut<crate::ui::toolbar::StationBuildState>,
    mut sim: ResMut<SimWorld>,
    mut remap: ResMut<crate::render::RemapMapVisualsPending>,
    mut exit: MessageWriter<AppExit>,
) {
    *frame += 1;
    // `OPENTTDRS_MAP_SHOT_PLACE=x,y[;x,y…]`: aplica la herramienta en esas
    // teselas antes de la captura (p. ej. colocar vía/estación y ver el render).
    if *frame == OPEN_FRAME + 5
        && let Ok(spec) = std::env::var("OPENTTDRS_MAP_SHOT_PLACE")
        && let Some(action) = tool_state.active_tool
    {
        for part in spec.split(';') {
            let Some((x, y)) = part.split_once(',') else {
                continue;
            };
            let (Ok(x), Ok(y)) = (x.parse::<i32>(), y.parse::<i32>()) else {
                continue;
            };
            let Some(cmd) = crate::ui::toolbar::build_input::commands::command_for_action(
                action,
                TileCoord::new(x, y),
                &station_state,
                None,
                Some(&sim.state.map),
                None,
            ) else {
                continue;
            };
            let res = openttdrs_core::apply_command(&mut sim.state, &cmd);
            info!("map_shot: place en ({x},{y}) → {res:?}");
        }
        remap.pending = true;
    }
    if *frame >= OPEN_FRAME
        && let Ok(tool) = std::env::var("OPENTTDRS_MAP_SHOT_TOOL")
    {
        use crate::ui::toolbar::{BuildMenuAction, ToolbarGroup};
        tool_state.active_tool = match tool.as_str() {
            "rail" => Some(BuildMenuAction::Rail),
            "rail_x" => Some(BuildMenuAction::RailX),
            "rail_y" => Some(BuildMenuAction::RailY),
            "rail_station" => Some(BuildMenuAction::RailStation),
            _ => None,
        };
        // `OPENTTDRS_MAP_SHOT_STATION=AxL` (p. ej. 4x3): andenes × longitud.
        if let Ok(spec) = std::env::var("OPENTTDRS_MAP_SHOT_STATION")
            && let Some((a, l)) = spec.split_once('x')
            && let (Ok(a), Ok(l)) = (a.parse::<u8>(), l.parse::<u8>())
        {
            station_state.rail_platforms = a.clamp(1, 7);
            station_state.rail_length = l.clamp(1, 7);
        }
        // El panel del grupo debe estar abierto o `hide_tool_when_panel_closed`
        // limpia la herramienta en el mismo frame.
        toolbar_state.active_group = Some(ToolbarGroup::Rail);
        if let Ok(mut window) = windows.single_mut() {
            // `OPENTTDRS_MAP_SHOT_CURSOR=fx,fy` (fracciones 0..1) reubica el cursor.
            let (fx, fy) = std::env::var("OPENTTDRS_MAP_SHOT_CURSOR")
                .ok()
                .and_then(|s| {
                    let (a, b) = s.split_once(',')?;
                    Some((a.parse::<f32>().ok()?, b.parse::<f32>().ok()?))
                })
                .unwrap_or((0.5, 0.55));
            let center = Vec2::new(window.width() * fx, window.height() * fy);
            window.set_cursor_position(Some(center));
        }
    }
    if *frame == SHOT_FRAME
        && let Ok(path) = std::env::var("OPENTTDRS_MAP_SHOT")
    {
        if let Ok(window) = windows.single_mut() {
            info!(
                "map_shot: tool_activa={} cursor={:?}",
                tool_state.active_tool.is_some(),
                window.cursor_position()
            );
        }
        info!("map_shot: guardando captura en {path}");
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(path));
    }
    if *frame == EXIT_FRAME {
        exit.write(AppExit::Success);
    }
}

#[allow(clippy::too_many_arguments)] // sistema ECS Bevy
fn windows_shot_driver(
    mut commands: Commands,
    mut frame: Local<u32>,
    sim: Res<SimWorld>,
    mut town: ResMut<TownWindowState>,
    mut depot: ResMut<DepotPanelState>,
    mut buy: ResMut<BuyVehicleWindowState>,
    mut vehicle: ResMut<VehicleWindowState>,
    mut exit: MessageWriter<AppExit>,
) {
    *frame += 1;
    if *frame == OPEN_FRAME {
        town.town_id = sim.state.towns.first().map(|t| t.id);
        let depot_pos = first_depot(&sim);
        depot.depot_pos = depot_pos;
        buy.depot_pos = depot_pos;
        buy.selected_engine = depot_pos
            .and_then(|pos| {
                crate::ui::buy_window::engines_for_buy_window(
                    &sim,
                    pos,
                    openttdrs_core::EngineCatalogSort::default(),
                    openttdrs_core::RoadEngineFilter::default(),
                )
                .first()
                .copied()
            })
            .map(|e| e.id);
        vehicle.vehicle_id = sim.state.vehicles.first().map(|v| v.id);
        info!(
            "windows_shot: town={:?} depot={:?} vehicle={:?}",
            town.town_id, depot.depot_pos, vehicle.vehicle_id
        );
    }
    if *frame == SHOT_FRAME
        && let Ok(path) = std::env::var("OPENTTDRS_WINDOWS_SHOT")
    {
        info!("windows_shot: guardando captura en {path}");
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(path));
    }
    if *frame == EXIT_FRAME {
        exit.write(AppExit::Success);
    }
}
