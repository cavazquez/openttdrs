//! Helpers para gestionar la exclusividad de paneles/ventanas al seleccionar entidades.

use crate::ui::industry_panel::IndustryPanelState;
use crate::ui::toolbar::OrderEditState;
use crate::ui::toolbar::depot_panel::DepotPanelState;
use crate::ui::toolbar::station_panel::StationCargoPanelState;
use crate::ui::town_window::TownWindowState;
use crate::ui::vehicle_chain::VehicleChainRegistry;
use crate::ui::vehicle_window::VehicleWindowState;
use openttdrs_core::Vehicle;

/// Clic en un vehículo del mapa: abre o enfoca su vista (#242).
/// No limpia Órdenes de *otros* vehículos.
#[allow(clippy::too_many_arguments)]
pub(crate) fn select_vehicle_on_map(
    _order_state: &mut OrderEditState,
    depot_state: &mut DepotPanelState,
    station_panel: &mut StationCargoPanelState,
    industry_panel: &mut IndustryPanelState,
    town_window: &mut TownWindowState,
    vehicle_window: &mut VehicleWindowState,
    chain: &mut VehicleChainRegistry,
    vehicle: &Vehicle,
) {
    vehicle_window.open_or_focus(chain, vehicle.id);
    depot_state.depot_pos = None;
    depot_state.selected_vehicle = None;
    station_panel.station_pos = None;
    station_panel.selected_tile = None;
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
    chain: &mut VehicleChainRegistry,
    town_id: u32,
) {
    town_window.town_id = Some(town_id);
    depot_state.depot_pos = None;
    depot_state.selected_vehicle = None;
    station_panel.station_pos = None;
    station_panel.selected_tile = None;
    industry_panel.open = false;
    order_state.clear();
    vehicle_window.clear_with_chain(chain);
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn open_industry_panel(
    industry_panel: &mut IndustryPanelState,
    depot_state: &mut DepotPanelState,
    order_state: &mut OrderEditState,
    station_panel: &mut StationCargoPanelState,
    town_window: &mut TownWindowState,
    vehicle_window: &mut VehicleWindowState,
    chain: &mut VehicleChainRegistry,
    focus_tile: openttdrs_core::TileCoord,
) {
    industry_panel.open = true;
    industry_panel.focus_tile = Some(focus_tile);
    depot_state.depot_pos = None;
    depot_state.selected_vehicle = None;
    order_state.clear();
    station_panel.station_pos = None;
    station_panel.selected_tile = None;
    town_window.town_id = None;
    vehicle_window.clear_with_chain(chain);
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn open_depot_panel(
    depot_state: &mut DepotPanelState,
    order_state: &mut OrderEditState,
    station_panel: &mut StationCargoPanelState,
    industry_panel: &mut IndustryPanelState,
    town_window: &mut TownWindowState,
    vehicle_window: &mut VehicleWindowState,
    chain: &mut VehicleChainRegistry,
    depot_pos: openttdrs_core::TileCoord,
    vehicle_id: Option<u32>,
) {
    depot_state.depot_pos = Some(depot_pos);
    depot_state.selected_vehicle = vehicle_id;
    order_state.clear();
    station_panel.station_pos = None;
    station_panel.selected_tile = None;
    industry_panel.open = false;
    town_window.town_id = None;
    vehicle_window.clear_with_chain(chain);
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn open_station_panel(
    station_panel: &mut StationCargoPanelState,
    depot_state: &mut DepotPanelState,
    order_state: &mut OrderEditState,
    industry_panel: &mut IndustryPanelState,
    town_window: &mut TownWindowState,
    vehicle_window: &mut VehicleWindowState,
    chain: &mut VehicleChainRegistry,
    station_pool: &mut crate::ui::station_pool::StationPoolRegistry,
    station_pos: openttdrs_core::TileCoord,
    selected_tile: openttdrs_core::TileCoord,
) {
    let _slot = station_pool.open_or_focus(station_pos);
    station_panel.station_pos = Some(station_pos);
    station_panel.selected_tile = Some(selected_tile);
    depot_state.depot_pos = None;
    depot_state.selected_vehicle = None;
    order_state.clear();
    industry_panel.open = false;
    town_window.town_id = None;
    vehicle_window.clear_with_chain(chain);
}
