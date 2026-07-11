//! Navegación tipada (`UiRoute`) y reexport del menú declarativo.

use bevy::prelude::*;

use crate::ui::graph_window::GraphKind;
use crate::ui::vehicle_list::VehicleListKind;

pub(crate) use crate::ui::menu::{
    MenuId, ToolbarMenuState, dismiss_toolbar_menu_on_outside_click, handle_toolbar_menu_entries,
    handle_toolbar_menu_keyboard, handle_toolbar_navigation_button, spawn_menu_anchor_button,
    sync_toolbar_navigation_menu,
};

/// Destinos navegables desde toolbar/menús.
///
/// Solo se añaden variantes cuando existe un consumidor real para evitar
/// repetir el problema de ventanas registradas pero inalcanzables.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UiRoute {
    Towns,
    Industries,
    Stations,
    Subsidies,
    Vehicles(VehicleListKind),
    Finances,
    Graph(GraphKind),
    CargoPaymentRates,
    SignList,
    LinkGraph,
}

/// Petición tipada para abrir una superficie UI.
#[derive(Message, Debug, Clone, Copy)]
pub(crate) struct OpenUiRoute(pub(crate) UiRoute);

/// Botón textual de navegación global dentro de la barra superior.
pub(crate) fn spawn_world_navigation_button(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
) {
    spawn_menu_anchor_button(parent, asset_server, MenuId::World);
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
