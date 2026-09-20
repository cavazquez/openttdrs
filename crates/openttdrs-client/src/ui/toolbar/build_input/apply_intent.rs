//! Aplicar intenciones de clic resueltas como efectos ECS (Commands, feedback, drag, paneles).

use bevy::prelude::*;
use openttdrs_core::BridgeType;
use openttdrs_core::Command;
use openttdrs_core::prelude::*;

use crate::i18n::Locale;
use crate::render::{RemapMapVisualsPending, request_map_visual_remap_with_labels};
use crate::settings::ClientPreferences;
use crate::state::{OrderPickState, SimWorld};
use crate::ui::hud::{
    HudBuildFeedback, SelectedTileInfo, enqueue_build_place_flash, push_build_command_error,
    push_object_slope_error, push_station_slope_error,
};
use crate::ui::industry_panel::IndustryPanelState;
use crate::ui::toolbar::bridge_window::{BridgeBuildState, PendingBridge};
use crate::ui::toolbar::depot_panel::DepotPanelState;
use crate::ui::toolbar::order_panel::{
    open_order_edit_for_vehicle, start_order_destination_pick, try_append_order_at_tile,
};
use crate::ui::toolbar::preview::rail_signal_flash_position;
use crate::ui::toolbar::station_panel::StationCargoPanelState;
use crate::ui::toolbar::{BuildMenuAction, DragBuildState, OrderEditState, StationBuildState};
use crate::ui::town_window::TownWindowState;
use crate::ui::vehicle_chain::VehicleChainRegistry;
use crate::ui::vehicle_window::VehicleWindowState;

use super::click_intent::MapClickIntent;
use super::commands::command_for_action;
use super::drag::{
    action_is_tunnel, apply_drag_action, drag_line_tiles_with_rail_bit, subsample_drag_tiles,
    tunnel_placement_is_valid,
};
use super::orders::order_pick_valid;
use super::placement::cancel_placement;
use super::remap_plan::tiles_for_visual_remap;
use super::selection;

/// Parámetros ECS para aplicar intenciones.
#[derive(bevy::ecs::system::SystemParam)]
pub(crate) struct IntentApplyContext<'w> {
    pub sim: ResMut<'w, SimWorld>,
    pub prefs: Option<Res<'w, ClientPreferences>>,
    pub selected: ResMut<'w, SelectedTileInfo>,
    pub drag_state: ResMut<'w, DragBuildState>,
    pub station_state: ResMut<'w, StationBuildState>,
    pub bridge_state: ResMut<'w, BridgeBuildState>,
    pub pending: ResMut<'w, RemapMapVisualsPending>,
    pub hud_feedback: ResMut<'w, HudBuildFeedback>,
    pub order_state: ResMut<'w, OrderEditState>,
    pub pick_next: ResMut<'w, NextState<OrderPickState>>,
    pub depot_state: ResMut<'w, DepotPanelState>,
    pub station_panel: ResMut<'w, StationCargoPanelState>,
    pub industry_panel: ResMut<'w, IndustryPanelState>,
    pub town_window: ResMut<'w, TownWindowState>,
    pub vehicle_window: ResMut<'w, VehicleWindowState>,
    pub vehicle_chain: ResMut<'w, VehicleChainRegistry>,
    pub station_pool: ResMut<'w, crate::ui::station_pool::StationPoolRegistry>,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn apply_intent(intent: MapClickIntent, ctx: &mut IntentApplyContext, time_secs: f32) {
    match intent {
        MapClickIntent::Ignore => {}
        MapClickIntent::CancelDrag => {
            cancel_placement(&mut ctx.drag_state);
            ctx.station_state.signal_drag_fract = None;
        }
        MapClickIntent::SelectTileForInspection(pos) => {
            ctx.selected.pos = Some(pos);
        }
        MapClickIntent::HandleOrderDestination(pos) => {
            // Replicar lógica de handle_order_destination_click sin verificar mouse
            let Some(vehicle_id) = ctx.order_state.vehicle_id() else {
                return;
            };
            // Prioridad 1: añadir la parada válida
            if order_pick_valid(&ctx.sim, vehicle_id, pos) {
                let result = {
                    let Some(orders) = ctx.order_state.orders_mut() else {
                        return;
                    };
                    try_append_order_at_tile(&mut ctx.sim, vehicle_id, pos, orders)
                };
                match result {
                    Ok(()) => {
                        ctx.pending.request_full();
                        let len = ctx.order_state.orders().len();
                        ctx.order_state.set_selected_slot(len.checked_sub(1));
                    }
                    Err(e) => {
                        if let Some(orders) = ctx.order_state.orders_mut() {
                            orders.pop();
                        }
                        push_build_command_error(&mut ctx.hud_feedback, e, time_secs);
                    }
                }
                return;
            }
            // Prioridad 2: clic sobre otro vehículo
            if let Some(vehicle) = ctx.sim.state.vehicles.iter().find(|v| v.pos == pos) {
                open_order_edit_for_vehicle(
                    &mut ctx.order_state,
                    &mut ctx.vehicle_chain,
                    vehicle,
                    &mut ctx.pick_next,
                );
                return;
            }
            // Estación incompatible
            if ctx.sim.state.stations.iter().any(|s| s.pos == pos) {
                let err = ctx
                    .sim
                    .state
                    .vehicles
                    .iter()
                    .find(|v| v.id == vehicle_id)
                    .and_then(|v| {
                        ctx.sim
                            .state
                            .stations
                            .iter()
                            .find(|s| s.pos == pos)
                            .filter(|s| !s.can_service_vehicle(v.kind))
                            .map(|_| CommandError::IncompatibleStopForVehicle)
                    })
                    .unwrap_or(CommandError::StationNotFound);
                push_build_command_error(&mut ctx.hud_feedback, err, time_secs);
            }
        }
        MapClickIntent::StartOrderEditForVehicle(vehicle_id) => {
            if let Some(vehicle) = ctx.sim.state.vehicles.iter().find(|v| v.id == vehicle_id) {
                open_order_edit_for_vehicle(
                    &mut ctx.order_state,
                    &mut ctx.vehicle_chain,
                    vehicle,
                    &mut ctx.pick_next,
                );
                start_order_destination_pick(&ctx.order_state, &mut ctx.pick_next);
            }
        }
        MapClickIntent::SelectVehicleOnMap(vehicle_id) => {
            if let Some(vehicle) = ctx.sim.state.vehicles.iter().find(|v| v.id == vehicle_id) {
                selection::select_vehicle_on_map(
                    &mut ctx.order_state,
                    &mut ctx.depot_state,
                    &mut ctx.station_panel,
                    &mut ctx.industry_panel,
                    &mut ctx.town_window,
                    &mut ctx.vehicle_window,
                    &mut ctx.vehicle_chain,
                    vehicle,
                );
            }
        }
        MapClickIntent::OpenTownWindow(town_id) => {
            selection::open_town_window(
                &mut ctx.town_window,
                &mut ctx.depot_state,
                &mut ctx.station_panel,
                &mut ctx.industry_panel,
                &mut ctx.order_state,
                &mut ctx.vehicle_window,
                &mut ctx.vehicle_chain,
                town_id,
            );
        }
        MapClickIntent::OpenIndustryPanel(pos) => {
            selection::open_industry_panel(
                &mut ctx.industry_panel,
                &mut ctx.depot_state,
                &mut ctx.order_state,
                &mut ctx.station_panel,
                &mut ctx.town_window,
                &mut ctx.vehicle_window,
                &mut ctx.vehicle_chain,
                pos,
            );
        }
        MapClickIntent::OpenDepotPanel {
            depot_pos,
            vehicle_id,
        } => {
            let depot_pos = selection::canonical_depot_panel_pos(&ctx.sim.state.map, depot_pos);
            selection::open_depot_panel(
                &mut ctx.depot_state,
                &mut ctx.order_state,
                &mut ctx.station_panel,
                &mut ctx.industry_panel,
                &mut ctx.town_window,
                &mut ctx.vehicle_window,
                &mut ctx.vehicle_chain,
                depot_pos,
                vehicle_id,
            );
        }
        MapClickIntent::OpenStationPanel {
            station_pos,
            selected_tile,
        } => {
            selection::open_station_panel(
                &mut ctx.station_panel,
                &mut ctx.depot_state,
                &mut ctx.order_state,
                &mut ctx.industry_panel,
                &mut ctx.town_window,
                &mut ctx.vehicle_window,
                &mut ctx.vehicle_chain,
                &mut ctx.station_pool,
                station_pos,
                selected_tile,
            );
        }
        MapClickIntent::StartDrag {
            action,
            start_tile,
            rail_lane_bit,
            signal_drag_fract,
            press_world_pos,
        } => {
            ctx.drag_state.armed = true;
            ctx.drag_state.start_tile = Some(start_tile);
            ctx.drag_state.last_tile = Some(start_tile);
            ctx.drag_state.last_action = Some(action);
            ctx.drag_state.pending_tiles = vec![start_tile];
            ctx.drag_state.rail_lane_bit = rail_lane_bit;
            ctx.drag_state.press_world_pos = Some(press_world_pos);
            if let Some(fract) = signal_drag_fract {
                ctx.station_state.signal_drag_fract = Some(fract);
            }
        }
        MapClickIntent::UpdateDrag {
            end_tile,
            signal_tap: _,
        } => {
            if let Some(action) = ctx.drag_state.last_action
                && let Some(start) = ctx.drag_state.start_tile
            {
                let line = drag_line_tiles_with_rail_bit(
                    Some(&ctx.sim.state.map),
                    action,
                    start,
                    end_tile,
                    ctx.drag_state.rail_lane_bit,
                );
                ctx.drag_state.pending_tiles = if action == BuildMenuAction::RailSignals {
                    subsample_drag_tiles(&line, ctx.station_state.signal_density)
                } else {
                    line
                };
                ctx.drag_state.last_tile = Some(end_tile);
            }
        }
        MapClickIntent::ConfirmDrag { signal_tap: _ } => {
            if let Some(action) = ctx.drag_state.last_action {
                let build_pos = ctx
                    .drag_state
                    .start_tile
                    .map(|(x, y)| TileCoord::new(x, y))
                    .unwrap_or(TileCoord::new(0, 0));
                confirm_drag_placement(
                    action,
                    &mut ctx.drag_state,
                    &mut ctx.sim,
                    &ctx.station_state,
                    build_pos,
                    &mut ctx.bridge_state,
                    &mut ctx.pending,
                    &mut ctx.hud_feedback,
                    time_secs,
                );
                ctx.station_state.signal_drag_fract = None;
                ctx.drag_state.press_world_pos = None;
            }
        }
        MapClickIntent::JoinStationClick { clicked, keep } => match keep {
            None => {
                ctx.station_state.join_keep = Some(clicked);
            }
            Some(k) if k == clicked => {
                ctx.station_state.join_keep = None;
            }
            Some(k) => {
                match crate::network::apply_player_command(
                    &mut ctx.sim.state,
                    &Command::JoinStations {
                        keep: k,
                        merge: clicked,
                    },
                ) {
                    Ok(()) => {
                        ctx.station_state.join_keep = None;
                        let (mw, mh) = ctx.sim.state.map.dimensions();
                        request_map_visual_remap_with_labels(&mut ctx.pending, mw, mh, &[]);
                    }
                    Err(e) => {
                        push_build_command_error(&mut ctx.hud_feedback, e, time_secs);
                    }
                }
            }
        },
        MapClickIntent::BuildImmediate {
            action,
            pos,
            rail_lane_bit,
            tile_fract,
            ctrl_held,
            cycle_signal,
            cycle_signal_variant,
        } => {
            let mut sig_type = ctx.station_state.signal_type;
            let cycle = cycle_signal;
            if ctrl_held && action == BuildMenuAction::RailSignals && !cycle_signal {
                sig_type = openttdrs_core::next_placeable_signal_type(sig_type);
            }
            let signal_variant_cmd =
                if cycle_signal_variant && action == BuildMenuAction::RailSignals {
                    Some(openttdrs_core::Command::CycleRailSignalVariant(
                        pos,
                        tile_fract.0,
                        tile_fract.1,
                    ))
                } else {
                    None
                };
            if let Some(cmd) = signal_variant_cmd.or_else(|| {
                command_for_action(
                    action,
                    pos,
                    &ctx.station_state,
                    rail_lane_bit,
                    Some(&ctx.sim.state.map),
                    Some(tile_fract),
                    sig_type,
                    cycle,
                    ctx.sim.state.current_rail_type,
                    ctx.sim.state.current_object_spec,
                )
            }) {
                if let Err(e) = crate::network::apply_player_command(&mut ctx.sim.state, &cmd) {
                    if action == BuildMenuAction::RailStation {
                        let locale = ctx
                            .prefs
                            .as_ref()
                            .map_or(Locale::Es, |prefs| prefs.locale());
                        push_station_slope_error(
                            &mut ctx.hud_feedback,
                            &mut ctx.sim,
                            e,
                            locale,
                            time_secs,
                        );
                    } else if matches!(
                        action,
                        BuildMenuAction::BuildLighthouse
                            | BuildMenuAction::BuildTransmitter
                            | BuildMenuAction::PlaceNewGrfObject
                    ) {
                        let locale = ctx
                            .prefs
                            .as_ref()
                            .map_or(Locale::Es, |prefs| prefs.locale());
                        push_object_slope_error(
                            &mut ctx.hud_feedback,
                            &mut ctx.sim,
                            e,
                            locale,
                            time_secs,
                        );
                    } else {
                        push_build_command_error(&mut ctx.hud_feedback, e, time_secs);
                    }
                } else {
                    if ctrl_held && action == BuildMenuAction::RailSignals && !cycle {
                        ctx.station_state.signal_type = sig_type;
                    }
                    let (mw, mh) = ctx.sim.state.map.dimensions();
                    let tiles = tiles_for_visual_remap(Some(&ctx.sim.state.map), action, pos, &[]);
                    request_map_visual_remap_with_labels(&mut ctx.pending, mw, mh, &tiles);
                    if action == BuildMenuAction::RailSignals
                        && let Some(flash_pos) = rail_signal_flash_position(
                            &ctx.sim.state.map,
                            pos,
                            ctx.station_state.orientation,
                            tile_fract.0,
                            tile_fract.1,
                            ctx.sim.state.tick,
                        )
                    {
                        enqueue_build_place_flash(&mut ctx.hud_feedback, flash_pos);
                    }
                }
            }
        }
    }
}

/// Arrastrar: clic para anclar, mover el ratón y soltar para confirmar. Clic derecho cancela.
#[allow(clippy::too_many_arguments)]
fn confirm_drag_placement(
    action: BuildMenuAction,
    drag_state: &mut DragBuildState,
    sim: &mut SimWorld,
    station_state: &StationBuildState,
    build_pos: TileCoord,
    bridge_state: &mut BridgeBuildState,
    pending: &mut RemapMapVisualsPending,
    hud_feedback: &mut HudBuildFeedback,
    time_secs: f32,
) {
    if matches!(
        action,
        BuildMenuAction::RoadBridge | BuildMenuAction::RailBridge
    ) {
        let tiles = std::mem::take(&mut drag_state.pending_tiles);
        cancel_placement(drag_state);
        if tiles.len() < 3 {
            push_build_command_error(hud_feedback, CommandError::InvalidBridgeSpan, time_secs);
            return;
        }
        let start = TileCoord::new(tiles[0].0, tiles[0].1);
        let end = TileCoord::new(tiles[tiles.len() - 1].0, tiles[tiles.len() - 1].1);
        let probe = if action == BuildMenuAction::RoadBridge {
            Command::PlaceRoadBridge(start, end, BridgeType::Wooden)
        } else {
            Command::PlaceRailBridge(start, end, BridgeType::Wooden)
        };
        match command_would_fail(&sim.state, &probe) {
            None
            | Some(CommandError::BridgeTypeNotAvailable)
            | Some(CommandError::InsufficientFunds) => {
                bridge_state.pending = Some(PendingBridge {
                    start,
                    end,
                    road: action == BuildMenuAction::RoadBridge,
                });
            }
            Some(e) => {
                push_build_command_error(hud_feedback, e, time_secs);
            }
        }
        return;
    }

    if action_is_tunnel(action)
        && !tunnel_placement_is_valid(&sim.state, action, &drag_state.pending_tiles)
    {
        cancel_placement(drag_state);
        return;
    }

    let tiles = std::mem::take(&mut drag_state.pending_tiles);
    let remap_tiles = tiles_for_visual_remap(Some(&sim.state.map), action, build_pos, &tiles);
    let lane = drag_state.rail_lane_bit;
    let (changed, err) = apply_drag_action(sim, action, tiles, station_state, lane);
    cancel_placement(drag_state);
    if changed {
        let (mw, mh) = sim.state.map.dimensions();
        request_map_visual_remap_with_labels(pending, mw, mh, &remap_tiles);
    } else if let Some(e) = err {
        push_build_command_error(hud_feedback, e, time_secs);
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::state::insert_test_order_pick_state;
    use crate::ui::refit_window::RefitWindowState;
    use crate::ui::shared_orders_window::SharedOrdersWindowState;
    use crate::ui::timetable_window::TimetableWindowState;
    use crate::ui::toolbar::build_input::click_intent::{MapClickContext, resolve_click_intent};
    use crate::ui::toolbar::{
        BridgeBuildState, DepotPanelState, DragBuildState, OrderPanelButton, StationBuildState,
        StationCargoPanelState, handle_order_panel_buttons,
    };
    use crate::ui::vehicle_chain::{VehicleChainRegistry, VehicleChainSlot};
    use crate::ui::vehicle_details_window::VehicleDetailsWindowState;
    use crate::ui::vehicle_window::{
        VehicleWindowButton, VehicleWindowState, handle_vehicle_window_buttons,
    };
    use bevy::ecs::system::RunSystemOnce;
    use bevy::math::Vec2;

    const DEMO_TRUCK_ID: u32 = 9010;

    #[derive(Resource)]
    struct V1OrderIntent(MapClickIntent);

    fn apply_v1_order_intent(intent: Res<V1OrderIntent>, mut ctx: IntentApplyContext) {
        apply_intent(intent.0.clone(), &mut ctx, 0.0);
    }

    fn v1_orders_world() -> World {
        let mut world = World::new();
        world.insert_resource(SimWorld::default());
        world.init_resource::<SelectedTileInfo>();
        world.init_resource::<DragBuildState>();
        world.init_resource::<StationBuildState>();
        world.init_resource::<BridgeBuildState>();
        world.init_resource::<RemapMapVisualsPending>();
        world.init_resource::<HudBuildFeedback>();
        world.init_resource::<OrderEditState>();
        insert_test_order_pick_state(&mut world);
        world.init_resource::<DepotPanelState>();
        world.init_resource::<StationCargoPanelState>();
        world.init_resource::<IndustryPanelState>();
        world.init_resource::<TownWindowState>();
        world.init_resource::<VehicleWindowState>();
        world.init_resource::<VehicleChainRegistry>();
        world.init_resource::<crate::ui::station_pool::StationPoolRegistry>();
        world.init_resource::<VehicleDetailsWindowState>();
        world.init_resource::<RefitWindowState>();
        world.init_resource::<TimetableWindowState>();
        world.init_resource::<SharedOrdersWindowState>();
        world.insert_resource(Time::<()>::default());
        world
    }

    fn apply_v1_intent(world: &mut World, intent: MapClickIntent) {
        world.insert_resource(V1OrderIntent(intent));
        world
            .run_system_once(apply_v1_order_intent)
            .expect("aplicar intención UI V1");
    }

    fn press_vehicle_orders_button(world: &mut World) {
        let button = world
            .spawn((Button, VehicleWindowButton::Orders, Interaction::Pressed))
            .id();
        world
            .run_system_once(handle_vehicle_window_buttons)
            .expect("pulsar Órdenes en la ventana de vehículo");
        world.despawn(button);
    }

    fn press_order_panel_button(world: &mut World, button: OrderPanelButton) {
        let button = world
            .spawn((Button, button, VehicleChainSlot(0), Interaction::Pressed))
            .id();
        world
            .run_system_once(handle_order_panel_buttons)
            .expect("pulsar control del panel de órdenes");
        world.despawn(button);
    }

    fn map_destination_click(pos: TileCoord) -> MapClickIntent {
        let intent = resolve_click_intent(&MapClickContext {
            tile_pos: pos,
            world_pos: Vec2::ZERO,
            tile_fract: (0, 0),
            mouse_left_pressed: true,
            mouse_right_pressed: false,
            mouse_left_released: false,
            active_tool: None,
            drag_armed: false,
            drag_last_action: None,
            drag_start_tile: None,
            drag_press_world_pos: None,
            vehicle_under_cursor: None,
            town_label_under_cursor: None,
            tile_kind: Some(TileKind::Station),
            orders_mode: true,
            order_pick_active: true,
            order_vehicle_selected: true,
            is_hangar: false,
            station_pos_at_tile: Some(pos),
            join_station_keep: None,
            signal_tile_has_signals: false,
            ctrl_held: false,
            shift_held: false,
        });
        assert_eq!(intent, MapClickIntent::HandleOrderDestination(pos));
        intent
    }

    #[test]
    fn v1_orders_ui_edits_demo_truck_route_and_runs_it() {
        use crate::state::bootstrap::{DEMO_ECONOMY_DELIVER_STATION, DEMO_ECONOMY_LOAD_STATION};

        let mut world = v1_orders_world();
        assert_eq!(
            world
                .resource::<SimWorld>()
                .state
                .vehicles
                .iter()
                .find(|vehicle| vehicle.id == DEMO_TRUCK_ID)
                .expect("camión vial del fixture")
                .orders
                .len(),
            2
        );

        // La vista se abre por el mismo intent de mapa que usa el cliente.
        apply_v1_intent(
            &mut world,
            MapClickIntent::SelectVehicleOnMap(DEMO_TRUCK_ID),
        );
        assert_eq!(
            world.resource::<VehicleWindowState>().vehicle_id,
            Some(DEMO_TRUCK_ID)
        );
        press_vehicle_orders_button(&mut world);
        assert_eq!(
            world.resource::<OrderEditState>().vehicle_id(),
            Some(DEMO_TRUCK_ID)
        );

        // Vaciar el recorrido existente únicamente mediante el control visible.
        press_order_panel_button(&mut world, OrderPanelButton::DeleteSelected);
        press_order_panel_button(&mut world, OrderPanelButton::DeleteSelected);
        assert!(
            world
                .resource::<SimWorld>()
                .state
                .vehicles
                .iter()
                .find(|vehicle| vehicle.id == DEMO_TRUCK_ID)
                .expect("camión vial del fixture")
                .orders
                .is_empty(),
            "el test no muta la lista directamente"
        );

        // «Ir a» inicia el picker y cada clic se resuelve/aplica por el ECS de producción.
        press_order_panel_button(&mut world, OrderPanelButton::PickDestOnMap);
        apply_v1_intent(&mut world, map_destination_click(DEMO_ECONOMY_LOAD_STATION));
        apply_v1_intent(
            &mut world,
            map_destination_click(DEMO_ECONOMY_DELIVER_STATION),
        );
        let expected = vec![
            VehicleOrder::station(DEMO_ECONOMY_LOAD_STATION),
            VehicleOrder::station(DEMO_ECONOMY_DELIVER_STATION),
        ];
        assert_eq!(
            world
                .resource::<SimWorld>()
                .state
                .vehicles
                .iter()
                .find(|vehicle| vehicle.id == DEMO_TRUCK_ID)
                .expect("camión vial del fixture")
                .orders,
            expected
        );

        // Eliminar y reponer el segundo destino prueba ambas acciones de la UI.
        press_order_panel_button(&mut world, OrderPanelButton::DeleteSelected);
        assert_eq!(
            world
                .resource::<SimWorld>()
                .state
                .vehicles
                .iter()
                .find(|vehicle| vehicle.id == DEMO_TRUCK_ID)
                .expect("camión vial del fixture")
                .orders,
            vec![VehicleOrder::station(DEMO_ECONOMY_LOAD_STATION)]
        );
        apply_v1_intent(
            &mut world,
            map_destination_click(DEMO_ECONOMY_DELIVER_STATION),
        );

        let mut carried_cargo = false;
        let mut reached_delivery = false;
        for _ in 0..1_200 {
            let mut sim = world.resource_mut::<SimWorld>();
            sim.state.step();
            let truck = sim
                .state
                .vehicles
                .iter()
                .find(|vehicle| vehicle.id == DEMO_TRUCK_ID)
                .expect("camión vial del fixture");
            carried_cargo |= truck.cargo > 0;
            reached_delivery |= truck.last_station_visited == Some(DEMO_ECONOMY_DELIVER_STATION);
        }
        let truck = world
            .resource::<SimWorld>()
            .state
            .vehicles
            .iter()
            .find(|vehicle| vehicle.id == DEMO_TRUCK_ID)
            .expect("camión vial del fixture");
        assert_eq!(truck.orders, expected);
        assert!(carried_cargo, "el camión debe cargar en la mina");
        assert!(
            reached_delivery,
            "el camión debe ejecutar la orden de central tras reponerla"
        );
    }
}
