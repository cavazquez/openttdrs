//! Helpers para gestionar la exclusividad de paneles/ventanas al seleccionar entidades.

use crate::ui::industry_panel::IndustryPanelState;
use crate::ui::toolbar::depot_panel::DepotPanelState;
use crate::ui::toolbar::station_panel::StationCargoPanelState;
use crate::ui::toolbar::OrderEditState;
use crate::ui::town_window::TownWindowState;
use crate::ui::vehicle_window::VehicleWindowState;
use openttdrs_core::Vehicle;

/// Clic en un vehículo del mapa: abre su ventana flotante (las órdenes se
/// abren desde el botón «Órdenes» de esa ventana).
#[allow(clippy::too_many_arguments)]
pub(crate) fn select_vehicle_on_map(
    order_state: &mut OrderEditState,
    depot_state: &mut DepotPanelState,
    station_panel: &mut StationCargoPanelState,
    industry_panel: &mut IndustryPanelState,
    town_window: &mut TownWindowState,
    vehicle_window: &mut VehicleWindowState,
    vehicle: &Vehicle,
) {
    vehicle_window.vehicle_id = Some(vehicle.id);
    order_state.clear();
    depot_state.depot_pos = None;
    depot_state.selected_vehicle = None;
    station_panel.station_pos = None;
    industry_panel.open = false;
    town_window.town_id = None;
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn open_town_window(
    town_window: &mut TownWindowState,
    depot_state: &mut DepotPanelState,
    station_panel: &mut StationCargoPanelState,
    industry_panel: &mut IndustryPanelState,
    order_state: &mut OrderEditState,
    vehicle_window: &mut VehicleWindowState,
    town_id: u32,
) {
    town_window.town_id = Some(town_id);
    depot_state.depot_pos = None;
    depot_state.selected_vehicle = None;
    station_panel.station_pos = None;
    industry_panel.open = false;
    order_state.clear();
    vehicle_window.vehicle_id = None;
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn open_industry_panel(
    industry_panel: &mut IndustryPanelState,
    depot_state: &mut DepotPanelState,
    order_state: &mut OrderEditState,
    station_panel: &mut StationCargoPanelState,
    town_window: &mut TownWindowState,
    vehicle_window: &mut VehicleWindowState,
    focus_tile: openttdrs_core::TileCoord,
) {
    industry_panel.open = true;
    industry_panel.focus_tile = Some(focus_tile);
    depot_state.depot_pos = None;
    depot_state.selected_vehicle = None;
    order_state.clear();
    station_panel.station_pos = None;
    town_window.town_id = None;
    vehicle_window.vehicle_id = None;
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn open_depot_panel(
    depot_state: &mut DepotPanelState,
    order_state: &mut OrderEditState,
    station_panel: &mut StationCargoPanelState,
    industry_panel: &mut IndustryPanelState,
    town_window: &mut TownWindowState,
    vehicle_window: &mut VehicleWindowState,
    depot_pos: openttdrs_core::TileCoord,
    vehicle_id: Option<u32>,
) {
    depot_state.depot_pos = Some(depot_pos);
    depot_state.selected_vehicle = vehicle_id;
    order_state.clear();
    station_panel.station_pos = None;
    industry_panel.open = false;
    town_window.town_id = None;
    vehicle_window.vehicle_id = None;
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn open_station_panel(
    station_panel: &mut StationCargoPanelState,
    depot_state: &mut DepotPanelState,
    order_state: &mut OrderEditState,
    industry_panel: &mut IndustryPanelState,
    town_window: &mut TownWindowState,
    vehicle_window: &mut VehicleWindowState,
    station_pos: openttdrs_core::TileCoord,
) {
    station_panel.station_pos = Some(station_pos);
    depot_state.depot_pos = None;
    depot_state.selected_vehicle = None;
    order_state.clear();
    industry_panel.open = false;
    town_window.town_id = None;
    vehicle_window.vehicle_id = None;
}
