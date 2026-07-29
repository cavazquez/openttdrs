//! Plugin de UI para ventanas de juego (finances, vehicles, compra, etc.).

use bevy::prelude::*;

use crate::bevy_app::{StartupSet, UpdateSet};
use crate::state::ClientScreen;
use crate::ui::autoreplace_window::{
    AutoreplaceWindowState, autoreplace_window_on_closed, handle_autoreplace_buttons,
    setup_autoreplace_window, sync_autoreplace_window,
};
use crate::ui::buy_window::{
    BuyVehicleWindowState, NewGrfTrainPreviewCache, buy_window_on_closed,
    buy_window_search_keyboard, handle_buy_window_buttons, setup_buy_window, sync_buy_window,
};
use crate::ui::cargo_payment_window::{
    CargoPaymentWindowState, cargo_payment_window_on_closed, open_cargo_payment_from_routes,
    setup_cargo_payment_window, sync_cargo_payment_window,
};
use crate::ui::company_view_window::{
    CompanyViewWindowState, company_view_window_on_closed, handle_company_view_buttons,
    open_company_view_from_routes, setup_company_view_window, sync_company_view_window,
};
use crate::ui::destination_window::{
    DestinationPickerState, destination_picker_on_closed, handle_destination_picker_buttons,
    setup_destination_picker, sync_destination_picker,
};
use crate::ui::endscreen::{
    EndScreenState, RetireGameRequested, handle_endscreen_menu_button, process_retire_game_request,
    setup_endscreen, sync_endscreen, watch_game_over_events,
};
use crate::ui::finances_window::{
    FinancesWindowState, finances_window_on_closed, handle_finances_window_buttons,
    handle_open_finances_window, open_finances_from_routes, setup_finances_window,
    sync_finances_window,
};
use crate::ui::graph_window::{
    GraphWindowState, graph_window_on_closed, handle_graph_window_buttons, open_graph_from_routes,
    setup_graph_window, sync_graph_window,
};
use crate::ui::navigation::handle_toolbar_menu_entries;
use crate::ui::refit_window::{
    RefitWindowState, handle_refit_window_buttons, refit_window_on_closed, setup_refit_window,
    sync_refit_window,
};
use crate::ui::shared_orders_window::{
    SharedOrdersWindowState, handle_shared_orders_buttons, setup_shared_orders_window,
    shared_orders_window_on_closed, sync_shared_orders_window,
};
use crate::ui::timetable_window::{
    TimetableWindowState, handle_timetable_window_buttons, setup_timetable_window,
    sync_timetable_window, timetable_window_on_closed,
};
use crate::ui::vehicle_chain::VehicleChainRegistry;
use crate::ui::vehicle_details_window::{
    VehicleDetailsWindowState, handle_vehicle_details_buttons, setup_vehicle_details_window,
    sync_vehicle_details_window, vehicle_details_window_on_closed,
};
use crate::ui::vehicle_window::{
    VehicleWindowState, handle_vehicle_rename_buttons, handle_vehicle_window_buttons,
    setup_vehicle_window, sync_vehicle_window, vehicle_window_on_closed,
    vehicle_window_rename_editable_keyboard, vehicle_window_rename_keyboard,
};

pub(crate) struct GameWindowsPlugin;

impl Plugin for GameWindowsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FinancesWindowState>()
            .init_resource::<CompanyViewWindowState>()
            .init_resource::<GraphWindowState>()
            .init_resource::<CargoPaymentWindowState>()
            .init_resource::<EndScreenState>()
            .init_resource::<RetireGameRequested>()
            .init_resource::<BuyVehicleWindowState>()
            .init_resource::<NewGrfTrainPreviewCache>()
            .init_resource::<DestinationPickerState>()
            .init_resource::<VehicleWindowState>()
            .init_resource::<VehicleChainRegistry>()
            .init_resource::<VehicleDetailsWindowState>()
            .init_resource::<RefitWindowState>()
            .init_resource::<SharedOrdersWindowState>()
            .init_resource::<AutoreplaceWindowState>()
            .init_resource::<TimetableWindowState>()
            .add_systems(
                OnEnter(ClientScreen::InGame),
                (
                    setup_finances_window,
                    setup_company_view_window,
                    setup_buy_window,
                    setup_destination_picker,
                )
                    .in_set(StartupSet::Ui),
            )
            .add_systems(
                OnEnter(ClientScreen::InGame),
                (
                    setup_graph_window,
                    setup_cargo_payment_window,
                    setup_vehicle_window,
                    setup_vehicle_details_window,
                    setup_refit_window,
                    setup_shared_orders_window,
                    setup_autoreplace_window,
                    setup_timetable_window,
                )
                    .in_set(StartupSet::Ui),
            )
            .add_systems(
                OnEnter(ClientScreen::InGame),
                setup_endscreen.in_set(StartupSet::Ui),
            )
            .add_systems(
                Update,
                (
                    handle_open_finances_window,
                    finances_window_on_closed,
                    sync_finances_window,
                    handle_finances_window_buttons,
                    company_view_window_on_closed,
                    sync_company_view_window,
                    handle_company_view_buttons,
                )
                    .in_set(UpdateSet::Ui)
                    .run_if(in_state(ClientScreen::InGame)),
            )
            .add_systems(
                Update,
                (
                    graph_window_on_closed,
                    cargo_payment_window_on_closed,
                    handle_graph_window_buttons,
                    sync_graph_window,
                    sync_cargo_payment_window,
                )
                    .in_set(UpdateSet::Ui)
                    .run_if(in_state(ClientScreen::InGame)),
            )
            .add_systems(
                Update,
                (
                    process_retire_game_request,
                    watch_game_over_events,
                    sync_endscreen,
                    handle_endscreen_menu_button,
                )
                    .in_set(UpdateSet::Ui)
                    .run_if(in_state(ClientScreen::InGame)),
            )
            .add_systems(
                Update,
                (
                    handle_buy_window_buttons,
                    buy_window_search_keyboard,
                    buy_window_on_closed,
                    handle_destination_picker_buttons,
                    destination_picker_on_closed,
                    handle_vehicle_window_buttons,
                    handle_vehicle_rename_buttons,
                    vehicle_window_rename_keyboard,
                    vehicle_window_rename_editable_keyboard,
                    vehicle_window_on_closed,
                    handle_vehicle_details_buttons,
                    vehicle_details_window_on_closed,
                    handle_timetable_window_buttons,
                    timetable_window_on_closed,
                )
                    .in_set(UpdateSet::Ui)
                    .run_if(in_state(ClientScreen::InGame)),
            )
            .add_systems(
                Update,
                (
                    handle_refit_window_buttons,
                    refit_window_on_closed,
                    handle_shared_orders_buttons,
                    shared_orders_window_on_closed,
                    handle_autoreplace_buttons,
                    autoreplace_window_on_closed,
                )
                    .in_set(UpdateSet::Ui)
                    .run_if(in_state(ClientScreen::InGame)),
            )
            .add_systems(
                Update,
                (
                    sync_buy_window,
                    sync_destination_picker,
                    sync_vehicle_window,
                    sync_vehicle_details_window,
                    sync_timetable_window,
                )
                    .in_set(UpdateSet::Ui)
                    .run_if(in_state(ClientScreen::InGame)),
            )
            .add_systems(
                Update,
                (
                    sync_refit_window,
                    sync_shared_orders_window,
                    sync_autoreplace_window,
                )
                    .in_set(UpdateSet::Ui)
                    .run_if(in_state(ClientScreen::InGame)),
            )
            .add_systems(
                Update,
                (
                    open_finances_from_routes,
                    open_company_view_from_routes,
                    open_graph_from_routes,
                    open_cargo_payment_from_routes,
                )
                    .after(handle_toolbar_menu_entries)
                    .in_set(UpdateSet::Ui)
                    .run_if(in_state(ClientScreen::InGame)),
            );
    }
}
