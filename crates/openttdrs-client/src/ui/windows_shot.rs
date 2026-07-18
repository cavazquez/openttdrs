//! Verificación visual automatizada de las ventanas flotantes.
//!
//! Con `OPENTTDRS_WINDOWS_SHOT=/ruta/captura.png` el cliente abre **todas** las
//! [`FloatingWindowId`] (más SaveWindow / OrderPanel / paneles auxiliares),
//! guarda una captura y sale.
//!
//! Inventario cubierto: ver [`windows_shot_covered_ids`] (debe == `FloatingWindowId::ALL`).
//!
//! Resolución opcional: `OPENTTDRS_SHOT_RES=1280x720` o `1920x1080`.

use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use bevy::window::PrimaryWindow;
use openttdrs_core::prelude::*;

use crate::state::{ClientScreen, SimWorld};
use crate::ui::ai_settings_window::AiSettingsWindowState;
use crate::ui::audio_settings_window::SoundMusicWindowState;
use crate::ui::autoreplace_window::AutoreplaceWindowState;
use crate::ui::buy_window::BuyVehicleWindowState;
use crate::ui::cargo_payment_window::CargoPaymentWindowState;
use crate::ui::cheat_window::CheatWindowState;
use crate::ui::destination_window::DestinationPickerState;
use crate::ui::dev_console::DevConsoleState;
use crate::ui::display_options_window::DisplayOptionsWindowState;
use crate::ui::extra_viewport_window::ExtraViewportWindowState;
use crate::ui::finances_window::FinancesWindowState;
use crate::ui::floating_window::{FloatingWindow, FloatingWindowId};
use crate::ui::genland_window::GenLandWindowState;
use crate::ui::goal_list_window::GoalListWindowState;
use crate::ui::graph_window::GraphWindowState;
use crate::ui::help_window::HelpWindowState;
use crate::ui::hud::SimHudControls;
use crate::ui::industry_directory::IndustryDirectoryState;
use crate::ui::industry_panel::IndustryPanelState;
use crate::ui::league_window::LeagueWindowState;
use crate::ui::main_menu::{MainMenuCamera, MainMenuUi};
use crate::ui::newgrf_window::NewGrfWindowState;
use crate::ui::news_settings_window::NewsSettingsWindowState;
use crate::ui::pathfinding_settings_window::PathfindingSettingsWindowState;
use crate::ui::refit_window::RefitWindowState;
use crate::ui::save_window::{SaveWindowMode, SaveWindowState, save_dir_from};
use crate::ui::shared_orders_window::SharedOrdersWindowState;
use crate::ui::sign_list_window::SignListWindowState;
use crate::ui::station_directory::StationDirectoryState;
use crate::ui::statusbar::NewsHistoryState;
use crate::ui::story_window::StoryWindowState;
use crate::ui::subsidy_list::SubsidyListState;
use crate::ui::tile_inspector_window::TileInspectorWindowState;
use crate::ui::timetable_window::TimetableWindowState;
use crate::ui::toolbar::{
    BridgeBuildState, BuildMenuAction, DepotPanelState, OrderEditState, PendingBridge,
    StationCargoPanelState, StationCatalogKind, StationCatalogPickerState, ToolbarGroup,
    ToolbarState, UiToolState,
};
use crate::ui::town_directory::TownDirectoryState;
use crate::ui::town_window::TownWindowState;
use crate::ui::ui5_blocked_stubs::LinkGraphWindowState;
use crate::ui::vehicle_details_window::VehicleDetailsWindowState;
use crate::ui::vehicle_list::VehicleListState;
use crate::ui::vehicle_window::VehicleWindowState;

const OPEN_FRAME: u32 = 30;
const SHOT_FRAME: u32 = 60;
const EXIT_FRAME: u32 = 120;

/// Inventario de `FloatingWindowId` que `windows_shot` intenta abrir.
/// Debe coincidir con [`FloatingWindowId::ALL`] (test de cobertura).
#[allow(dead_code)] // consumido en tests de inventario
#[must_use]
pub(crate) fn windows_shot_covered_ids() -> &'static [FloatingWindowId] {
    FloatingWindowId::ALL
}

pub(crate) struct WindowsShotPlugin;

impl Plugin for WindowsShotPlugin {
    fn build(&self, app: &mut App) {
        if std::env::var_os("OPENTTDRS_WINDOWS_SHOT").is_some()
            || std::env::var_os("OPENTTDRS_MAP_SHOT").is_some()
        {
            app.add_systems(Startup, apply_shot_resolution);
        }
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

fn parse_shot_resolution() -> Option<(u32, u32)> {
    let Ok(raw) = std::env::var("OPENTTDRS_SHOT_RES") else {
        return None;
    };
    let (w, h) = raw.split_once('x').or_else(|| raw.split_once('X'))?;
    let w = w.parse().ok()?;
    let h = h.parse().ok()?;
    if w < 320 || h < 240 {
        return None;
    }
    Some((w, h))
}

fn apply_shot_resolution(mut windows: Query<&mut Window, With<PrimaryWindow>>) {
    let Some((w, h)) = parse_shot_resolution() else {
        return;
    };
    for mut window in &mut windows {
        window.resolution.set(w as f32, h as f32);
        info!("shot: resolución {w}×{h}");
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

fn first_station(sim: &SimWorld) -> Option<TileCoord> {
    sim.state.stations.first().map(|s| s.pos)
}

fn first_industry_tile(sim: &SimWorld) -> Option<TileCoord> {
    sim.state.industries.first().map(|i| i.pos)
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
                station_state.signal_type,
                false,
                sim.state.current_rail_type,
            ) else {
                continue;
            };
            let res = crate::network::apply_player_command(&mut sim.state, &cmd);
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

/// Abre todas las ventanas flotantes + paneles auxiliares para captura de paridad.
/// Sistema exclusivo: demasiados `ResMut` para el límite de SystemParam de Bevy.
fn windows_shot_driver(world: &mut World, mut frame: Local<u32>) {
    *frame += 1;
    if *frame == OPEN_FRAME {
        open_all_windows_for_shot(world);
    }

    // Fuerza visibilidad de pickers tool-gated (Airport/Signal/Bridge/RailStation)
    // que el sync ocultaría por la herramienta activa única.
    if (OPEN_FRAME..=SHOT_FRAME).contains(&*frame) {
        let mut q = world.query::<(&FloatingWindow, &mut Visibility)>();
        for (_, mut vis) in q.iter_mut(world) {
            *vis = Visibility::Visible;
        }
    }

    if *frame == SHOT_FRAME
        && let Ok(path) = std::env::var("OPENTTDRS_WINDOWS_SHOT")
    {
        info!("windows_shot: guardando captura en {path}");
        world
            .commands()
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(path));
    }
    if *frame == EXIT_FRAME {
        world.write_message(AppExit::Success);
    }
}

fn open_all_windows_for_shot(world: &mut World) {
    let town_id = world
        .resource::<SimWorld>()
        .state
        .towns
        .first()
        .map(|t| t.id);
    let vehicle_id = world
        .resource::<SimWorld>()
        .state
        .vehicles
        .first()
        .map(|v| v.id);
    let vehicle_orders = vehicle_id.and_then(|vid| {
        world
            .resource::<SimWorld>()
            .state
            .vehicles
            .iter()
            .find(|v| v.id == vid)
            .map(|v| v.orders.clone())
    });
    let depot_pos = first_depot(world.resource::<SimWorld>());
    let station_pos = first_station(world.resource::<SimWorld>());
    let industry_pos = first_industry_tile(world.resource::<SimWorld>());
    let selected_engine = depot_pos.and_then(|pos| {
        let sim = world.resource::<SimWorld>();
        crate::ui::buy_window::engines_for_buy_window(
            sim,
            pos,
            openttdrs_core::EngineCatalogSort::default(),
            openttdrs_core::RoadEngineFilter::default(),
            crate::ui::buy_window::RailBuyFilter::default(),
            "",
        )
        .first()
        .map(|e| e.id)
    });
    let save_dir = save_dir_from(&world.resource::<SimHudControls>().json_save_path);

    world.resource_mut::<TownWindowState>().town_id = town_id;
    {
        let mut depot = world.resource_mut::<DepotPanelState>();
        depot.depot_pos = depot_pos;
    }
    {
        let mut buy = world.resource_mut::<BuyVehicleWindowState>();
        buy.depot_pos = depot_pos;
        buy.selected_engine = selected_engine;
    }
    world.resource_mut::<VehicleWindowState>().vehicle_id = vehicle_id;
    world.resource_mut::<TimetableWindowState>().vehicle_id = vehicle_id;
    if let Some(vid) = vehicle_id {
        world
            .resource_mut::<VehicleDetailsWindowState>()
            .open_for(vid);
        world.resource_mut::<RefitWindowState>().open_for(vid);
        {
            let mut order = world.resource_mut::<OrderEditState>();
            order.vehicle_id = Some(vid);
            order.orders = vehicle_orders.unwrap_or_default();
        }
        {
            let mut shared = world.resource_mut::<SharedOrdersWindowState>();
            shared.open = true;
            shared.link_vehicle_id = Some(vid);
        }
        world.resource_mut::<DestinationPickerState>().open = true;
    }
    if let Some(pos) = depot_pos {
        world
            .resource_mut::<AutoreplaceWindowState>()
            .open_for_depot(pos);
    }
    world.resource_mut::<StationCargoPanelState>().station_pos = station_pos;
    if let Some(pos) = industry_pos {
        let mut panel = world.resource_mut::<IndustryPanelState>();
        panel.open = true;
        panel.focus_tile = Some(pos);
    }

    world.resource_mut::<FinancesWindowState>().open = true;
    world.resource_mut::<TownDirectoryState>().open = true;
    world.resource_mut::<IndustryDirectoryState>().open = true;
    world.resource_mut::<StationDirectoryState>().open = true;
    world.resource_mut::<VehicleListState>().open = true;
    world.resource_mut::<SubsidyListState>().open = true;
    world.resource_mut::<NewsHistoryState>().open = true;
    world.resource_mut::<NewsSettingsWindowState>().open = true;
    world.resource_mut::<PathfindingSettingsWindowState>().open = true;
    world.resource_mut::<AiSettingsWindowState>().open = true;
    world.resource_mut::<NewGrfWindowState>().open = true;
    world.resource_mut::<SoundMusicWindowState>().open = true;
    world.resource_mut::<GraphWindowState>().open = true;
    world.resource_mut::<CargoPaymentWindowState>().open = true;
    world.resource_mut::<DisplayOptionsWindowState>().open = true;
    world.resource_mut::<ExtraViewportWindowState>().open = true;
    world.resource_mut::<SignListWindowState>().open = true;
    world.resource_mut::<LinkGraphWindowState>().open = true;
    world.resource_mut::<HelpWindowState>().open = true;
    world.resource_mut::<DevConsoleState>().open = true;
    world.resource_mut::<TileInspectorWindowState>().open = true;
    world.resource_mut::<CheatWindowState>().open = true;
    world.resource_mut::<GenLandWindowState>().open = true;
    world.resource_mut::<GoalListWindowState>().open = true;
    world.resource_mut::<StoryWindowState>().open = true;
    world.resource_mut::<LeagueWindowState>().open = true;

    world
        .resource_mut::<SaveWindowState>()
        .open_in_mode(SaveWindowMode::Save, &save_dir);
    world.resource_mut::<StationCatalogPickerState>().open = Some(StationCatalogKind::Spec);

    world.resource_mut::<ToolbarState>().active_group = Some(ToolbarGroup::Rail);
    world.resource_mut::<UiToolState>().active_tool = Some(BuildMenuAction::RailStation);
    world.resource_mut::<BridgeBuildState>().pending = Some(PendingBridge {
        start: TileCoord::new(2, 2),
        end: TileCoord::new(6, 2),
        road: false,
    });

    info!(
        "windows_shot: abriendo ALL FloatingWindowId ({}) + Save/Order/panels",
        FloatingWindowId::ALL.len()
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_shot_covers_all_floating_ids() {
        assert_eq!(
            windows_shot_covered_ids(),
            FloatingWindowId::ALL,
            "actualizar windows_shot_covered_ids al añadir FloatingWindowId"
        );
    }
}
