//! Navegación tipada (`UiRoute`) y reexport del menú declarativo.

use bevy::prelude::*;

use crate::ui::graph_window::GraphKind;
use crate::ui::vehicle_list::VehicleListKind;

pub(crate) use crate::ui::menu::{
    MenuId, ToolbarContext, ToolbarMenuState, dismiss_toolbar_menu_on_outside_click,
    handle_toolbar_menu_entries, handle_toolbar_menu_keyboard, handle_toolbar_navigation_button,
    refresh_toolbar_context, spawn_menu_anchor_button, spawn_menu_anchor_button_sized,
    sync_toolbar_localized_labels, sync_toolbar_navigation_menu,
};

/// Destinos navegables desde toolbar/menús.
///
/// Solo se añaden variantes cuando existe un consumidor real para evitar
/// repetir el problema de ventanas registradas pero inalcanzables.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UiRoute {
    SaveGame,
    LoadGame,
    ReturnMainMenu,
    ExitGame,
    EditorSaveScenario,
    EditorLoadScenario,
    EditorSaveHeightmap,
    EditorLoadHeightmap,
    EditorExit,
    Towns,
    Industries,
    Stations,
    Subsidies,
    Vehicles(VehicleListKind),
    Finances,
    CompanyView,
    Graph(GraphKind),
    CargoPaymentRates,
    SignList,
    LinkGraph,
    Goals,
    Story,
    League,
    DisplayOptions,
    SoundMusic,
    PathfindingSettings,
    CargoDistSettings,
    AiSettings,
    NewGrf,
    NewsSettings,
    NewsHistory,
    Help,
    DevConsole,
    TileInspector,
    Cheats,
}

/// Petición tipada para abrir una superficie UI.
#[derive(Message, Debug, Clone, Copy)]
pub(crate) struct OpenUiRoute(pub(crate) UiRoute);

#[allow(clippy::too_many_arguments)]
pub(crate) fn open_settings_windows_from_routes(
    mut routes: MessageReader<OpenUiRoute>,
    mut display: ResMut<crate::ui::display_options_window::DisplayOptionsWindowState>,
    mut sound: ResMut<crate::ui::audio_settings_window::SoundMusicWindowState>,
    mut pathfinding: ResMut<crate::ui::pathfinding_settings_window::PathfindingSettingsWindowState>,
    mut cargo_dist: ResMut<crate::ui::cargo_dist_settings_window::CargoDistSettingsWindowState>,
    mut ai: ResMut<crate::ui::ai_settings_window::AiSettingsWindowState>,
    mut newgrf: ResMut<crate::ui::newgrf_window::NewGrfWindowState>,
    mut news: ResMut<crate::ui::news_settings_window::NewsSettingsWindowState>,
) {
    for OpenUiRoute(route) in routes.read() {
        match route {
            UiRoute::DisplayOptions => display.open = true,
            UiRoute::SoundMusic => sound.open = true,
            UiRoute::PathfindingSettings => pathfinding.open = true,
            UiRoute::CargoDistSettings => cargo_dist.open = true,
            UiRoute::AiSettings => ai.open = true,
            UiRoute::NewGrf => newgrf.open = true,
            UiRoute::NewsSettings => news.open = true,
            _ => {}
        }
    }
}

pub(crate) fn open_help_windows_from_routes(
    mut routes: MessageReader<OpenUiRoute>,
    mut help: ResMut<crate::ui::help_window::HelpWindowState>,
    mut console: ResMut<crate::ui::dev_console::DevConsoleState>,
    mut inspector: ResMut<crate::ui::tile_inspector_window::TileInspectorWindowState>,
    mut cheats: ResMut<crate::ui::cheat_window::CheatWindowState>,
) {
    for OpenUiRoute(route) in routes.read() {
        match route {
            UiRoute::Help => help.open = true,
            UiRoute::DevConsole => console.open = true,
            UiRoute::TileInspector => inspector.open = true,
            UiRoute::Cheats => cheats.open = true,
            _ => {}
        }
    }
}

pub(crate) fn open_file_actions_from_routes(
    mut routes: MessageReader<OpenUiRoute>,
    hud: Res<crate::ui::hud::SimHudControls>,
    mut save_window: ResMut<crate::ui::save_window::SaveWindowState>,
    mut next_screen: ResMut<NextState<crate::state::ClientScreen>>,
    mut suspended: ResMut<crate::state::SuspendedGameSession>,
    mut exit: MessageWriter<AppExit>,
    editor: Option<Res<crate::state::EditorSession>>,
) {
    use crate::ui::save_window::{SaveWindowMode, save_dir_from};

    for OpenUiRoute(route) in routes.read() {
        match route {
            UiRoute::SaveGame => {
                save_window.open_in_mode(SaveWindowMode::Save, &save_dir_from(&hud.json_save_path));
            }
            UiRoute::LoadGame => {
                save_window.open_in_mode(SaveWindowMode::Load, &save_dir_from(&hud.json_save_path));
            }
            UiRoute::ReturnMainMenu => {
                crate::ui::main_menu::return_to_main_menu(&mut next_screen, &mut suspended);
            }
            UiRoute::ExitGame if !editor.as_deref().is_some_and(|session| session.active) => {
                exit.write(AppExit::Success);
            }
            _ => {}
        }
    }
}

pub(crate) fn open_message_windows_from_routes(
    mut routes: MessageReader<OpenUiRoute>,
    mut history: ResMut<crate::ui::statusbar::NewsHistoryState>,
) {
    for OpenUiRoute(route) in routes.read() {
        if *route == UiRoute::NewsHistory {
            history.open = true;
        }
    }
}

/// Botón textual de navegación global dentro de la barra superior.
pub(crate) fn spawn_world_navigation_button(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
) {
    spawn_menu_anchor_button(parent, asset_server, MenuId::World);
}

pub(crate) fn spawn_file_navigation_button(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
) {
    spawn_menu_anchor_button(parent, asset_server, MenuId::File);
}

pub(crate) fn spawn_fleet_navigation_button(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
) {
    spawn_menu_anchor_button(parent, asset_server, MenuId::Fleet);
}

pub(crate) fn spawn_economy_navigation_button(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
) {
    spawn_menu_anchor_button(parent, asset_server, MenuId::Economy);
}

pub(crate) fn spawn_map_navigation_button(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
) {
    spawn_menu_anchor_button(parent, asset_server, MenuId::Map);
}

pub(crate) fn spawn_industries_navigation_button(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
) {
    spawn_menu_anchor_button(parent, asset_server, MenuId::Industries);
}

pub(crate) fn spawn_settings_navigation_button(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
) {
    spawn_menu_anchor_button(parent, asset_server, MenuId::Settings);
}

pub(crate) fn spawn_help_navigation_button(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
) {
    spawn_menu_anchor_button(parent, asset_server, MenuId::Help);
}

pub(crate) fn spawn_messages_navigation_button(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
) {
    spawn_menu_anchor_button(parent, asset_server, MenuId::Messages);
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    #[test]
    fn help_route_opens_existing_help_window() {
        let mut world = World::new();
        world.init_resource::<Messages<OpenUiRoute>>();
        world.init_resource::<crate::ui::help_window::HelpWindowState>();
        world.init_resource::<crate::ui::dev_console::DevConsoleState>();
        world.init_resource::<crate::ui::tile_inspector_window::TileInspectorWindowState>();
        world.init_resource::<crate::ui::cheat_window::CheatWindowState>();
        world.write_message(OpenUiRoute(UiRoute::Help));

        world
            .run_system_once(open_help_windows_from_routes)
            .unwrap();

        assert!(
            world
                .resource::<crate::ui::help_window::HelpWindowState>()
                .open
        );
    }

    #[test]
    fn news_history_route_opens_existing_history_window() {
        let mut world = World::new();
        world.init_resource::<Messages<OpenUiRoute>>();
        world.init_resource::<crate::ui::statusbar::NewsHistoryState>();
        world.write_message(OpenUiRoute(UiRoute::NewsHistory));

        world
            .run_system_once(open_message_windows_from_routes)
            .unwrap();

        assert!(
            world
                .resource::<crate::ui::statusbar::NewsHistoryState>()
                .open
        );
    }
}
