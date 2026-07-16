//! Sistema Bevy delgado que recopila contexto de clic y delega resolución/aplicación.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use openttdrs_core::TileKind;

use crate::iso::{world_pos_to_tile_coord, world_pos_to_tile_fract};
use crate::render::{
    MapPreviewCamera, PrimaryGameCamera, pick_vehicle_id_at_world, town_id_at_label_pos,
};
use crate::state::{OrderPickState, order_pick_active};
use crate::ui::hud::HoveredTileCoord;
use crate::ui::save_window::SaveWindowState;
use crate::ui::toolbar::minimap::minimap_contains_cursor;
use crate::ui::toolbar::minimap::{MinimapCell, MinimapLayerState, MinimapRoot};
use crate::ui::toolbar::{BuildMenuAction, BuildMenuUi, UiToolState};
use crate::ui::town_window::town_for_house_tile;

use super::apply_intent::{IntentApplyContext, apply_intent};
use super::click_intent::{MapClickContext, resolve_click_intent};
use super::drag::action_supports_drag;

/// Estados de paneles/ventanas mutuamente excluyentes, agrupados para no
/// exceder el límite de parámetros de sistema de Bevy.
#[derive(bevy::ecs::system::SystemParam)]
pub(crate) struct PanelStates<'w> {
    pick_state: Res<'w, State<OrderPickState>>,
    minimap_layers: Res<'w, MinimapLayerState>,
}

/// Sistema principal delgado que maneja clics en el mapa.
/// Recolecta contexto, resuelve la intención y la aplica.
#[allow(clippy::too_many_arguments)] // sistema ECS Bevy
pub(crate) fn handle_tile_click(
    mouse: Res<ButtonInput<MouseButton>>,
    save_window: Option<Res<SaveWindowState>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cam_q: Query<(&Camera, &GlobalTransform), (With<PrimaryGameCamera>, Without<MapPreviewCamera>)>,
    mut tool_state: ResMut<UiToolState>,
    panels: PanelStates,
    mut apply_ctx: IntentApplyContext,
    toolbar_pointer: Query<
        &Interaction,
        (
            With<BuildMenuUi>,
            Without<MinimapRoot>,
            Without<MinimapCell>,
        ),
    >,
    hovered: Res<HoveredTileCoord>,
    time: Res<Time>,
) {
    let minimap_layers = &*panels.minimap_layers;

    // Early exits: guardar ventana, block_map_click, toolbar, etc.
    if save_window.is_some_and(|w| w.open) {
        return;
    }
    if mouse.just_pressed(MouseButton::Left) && tool_state.block_map_click {
        tool_state.block_map_click = false;
        return;
    }
    if toolbar_pointer.iter().any(|i| *i != Interaction::None)
        && mouse.just_pressed(MouseButton::Left)
    {
        return;
    }

    let Ok(window) = windows.single() else {
        return;
    };
    let Some(cursor_pos) = window.cursor_position() else {
        return;
    };
    if minimap_contains_cursor(cursor_pos, window, minimap_layers) {
        return;
    }
    let Ok((camera, cam_tf)) = cam_q.single() else {
        return;
    };
    let Ok(world_pos) = camera.viewport_to_world_2d(cam_tf, cursor_pos) else {
        return;
    };

    // Caso especial: confirmar drag fuera del mapa
    if world_pos_to_tile_coord(world_pos, &apply_ctx.sim.state.map).is_none() {
        if apply_ctx.drag_state.armed
            && mouse.just_released(MouseButton::Left)
            && let Some(action) = tool_state.active_tool
            && action_supports_drag(action)
            && apply_ctx.drag_state.last_action == Some(action)
        {
            let ctx = MapClickContext {
                tile_pos: openttdrs_core::TileCoord::new(0, 0),
                world_pos,
                tile_fract: (128, 128),
                mouse_left_pressed: false,
                mouse_right_pressed: false,
                mouse_left_released: true,
                active_tool: tool_state.active_tool,
                drag_armed: apply_ctx.drag_state.armed,
                drag_last_action: apply_ctx.drag_state.last_action,
                drag_start_tile: apply_ctx.drag_state.start_tile,
                drag_press_world_pos: apply_ctx.drag_state.press_world_pos,
                vehicle_under_cursor: None,
                town_label_under_cursor: None,
                tile_kind: None,
                orders_mode: false,
                order_pick_active: false,
                order_vehicle_selected: false,
                is_hangar: false,
                station_pos_at_tile: None,
                join_station_keep: apply_ctx.station_state.join_keep,
                signal_tile_has_signals: false,
                ctrl_held: apply_ctx.station_state.ctrl_held,
            };
            let intent = resolve_click_intent(&ctx);
            apply_intent(intent, &mut apply_ctx, time.elapsed_secs());
        }
        return;
    }

    let Some((tx, ty)) = world_pos_to_tile_coord(world_pos, &apply_ctx.sim.state.map) else {
        return;
    };
    let pos = openttdrs_core::TileCoord::new(tx, ty);

    // Construir contexto completo
    let (build_pos, tile_fract) = if tool_state.active_tool == Some(BuildMenuAction::RailSignals) {
        let Some(hpos) = hovered.pos else {
            return;
        };
        (hpos, (hovered.fract_x, hovered.fract_y))
    } else if tool_state.active_tool == Some(BuildMenuAction::Clear) {
        if let Some(hpos) = hovered.pos
            && let Some(tile) = apply_ctx.sim.state.map.get(hpos)
            && tile.kind == TileKind::Rail
            && openttdrs_core::rail_signals::rail_tile_is_signals(tile.m5)
        {
            (hpos, (hovered.fract_x, hovered.fract_y))
        } else {
            (
                pos,
                world_pos_to_tile_fract(world_pos, &apply_ctx.sim.state.map, tx, ty),
            )
        }
    } else {
        (
            pos,
            world_pos_to_tile_fract(world_pos, &apply_ctx.sim.state.map, tx, ty),
        )
    };

    let vehicle_under_cursor = pick_vehicle_id_at_world(world_pos, &apply_ctx.sim);
    let town_label_under_cursor = town_id_at_label_pos(&apply_ctx.sim, world_pos);
    let tile_kind = apply_ctx.sim.state.map.get_kind(build_pos);
    let is_hangar = tile_kind == Some(TileKind::Airport)
        && openttdrs_core::airport_tile_is_hangar(&apply_ctx.sim.state.map, build_pos);
    let station_pos_at_tile = openttdrs_core::station_at_tile(
        &apply_ctx.sim.state.map,
        &apply_ctx.sim.state.stations,
        build_pos,
    )
    .map(|s| s.pos);

    let orders_mode = order_pick_active(&panels.pick_state)
        || tool_state.active_tool == Some(BuildMenuAction::Orders);
    let order_pick_active_flag = order_pick_active(&panels.pick_state);
    let order_vehicle_selected = apply_ctx.order_state.vehicle_id.is_some();

    let signal_tile_has_signals = if tool_state.active_tool == Some(BuildMenuAction::RailSignals) {
        if let Some(tile) = apply_ctx.sim.state.map.get(build_pos)
            && tile.kind == TileKind::Rail
            && openttdrs_core::rail_signals::rail_tile_is_signals(tile.m5)
        {
            let tb = tile.m5 & 0x3F;
            let (fx, fy) = tile_fract;
            openttdrs_core::rail_signals::resolve_signal_track(tb, fx, fy).is_some_and(|track| {
                openttdrs_core::rail_signals::rail_signal_present_mask(tile.m3)
                    & openttdrs_core::rail_signals::signal_on_track_mask(track)
                    != 0
            })
        } else {
            false
        }
    } else {
        false
    };

    let ctx = MapClickContext {
        tile_pos: build_pos,
        world_pos,
        tile_fract,
        mouse_left_pressed: mouse.just_pressed(MouseButton::Left),
        mouse_right_pressed: mouse.just_pressed(MouseButton::Right),
        mouse_left_released: mouse.just_released(MouseButton::Left),
        active_tool: tool_state.active_tool,
        drag_armed: apply_ctx.drag_state.armed,
        drag_last_action: apply_ctx.drag_state.last_action,
        drag_start_tile: apply_ctx.drag_state.start_tile,
        drag_press_world_pos: apply_ctx.drag_state.press_world_pos,
        vehicle_under_cursor,
        town_label_under_cursor: if tile_kind == Some(TileKind::House) {
            town_for_house_tile(&apply_ctx.sim.state, build_pos)
        } else {
            town_label_under_cursor
        },
        tile_kind,
        orders_mode,
        order_pick_active: order_pick_active_flag,
        order_vehicle_selected,
        is_hangar,
        station_pos_at_tile,
        join_station_keep: apply_ctx.station_state.join_keep,
        signal_tile_has_signals,
        ctrl_held: apply_ctx.station_state.ctrl_held,
    };

    let intent = resolve_click_intent(&ctx);
    apply_intent(intent, &mut apply_ctx, time.elapsed_secs());
}

pub(crate) fn sync_build_pointer_modifiers(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut station_state: ResMut<crate::ui::toolbar::StationBuildState>,
) {
    station_state.ctrl_held =
        keyboard.pressed(KeyCode::ControlLeft) || keyboard.pressed(KeyCode::ControlRight);
}
