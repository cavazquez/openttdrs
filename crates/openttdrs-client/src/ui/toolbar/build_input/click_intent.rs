//! Tipos puros para la resolución de intención de clic en el mapa.
//!
//! Este módulo define el contexto de clic y las intenciones posibles sin
//! depender de Commands o AssetServer de Bevy, facilitando pruebas unitarias.

use bevy::math::Vec2;
use openttdrs_core::prelude::*;

use crate::ui::toolbar::BuildMenuAction;

/// Contexto completo para decidir qué hacer ante un clic en el mapa.
#[derive(Debug, Clone)]
pub(crate) struct MapClickContext {
    pub tile_pos: TileCoord,
    pub world_pos: Vec2,
    pub tile_fract: (u8, u8),
    pub mouse_left_pressed: bool,
    pub mouse_right_pressed: bool,
    pub mouse_left_released: bool,
    pub active_tool: Option<BuildMenuAction>,
    pub drag_armed: bool,
    pub drag_last_action: Option<BuildMenuAction>,
    pub drag_start_tile: Option<(i32, i32)>,
    pub vehicle_under_cursor: Option<u32>,
    pub town_label_under_cursor: Option<u32>,
    pub tile_kind: Option<TileKind>,
    pub orders_mode: bool,
    pub order_pick_active: bool,
    pub order_vehicle_selected: bool,
    pub is_hangar: bool,
    pub station_pos_at_tile: Option<TileCoord>,
    pub join_station_keep: Option<TileCoord>,
    pub signal_tile_has_signals: bool,
    pub ctrl_held: bool,
}

/// Intención de acción resultante de un clic en el mapa.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum MapClickIntent {
    /// No hacer nada (clic fuera de contexto válido).
    Ignore,
    /// Cancelar drag actual (clic derecho).
    CancelDrag,
    /// Seleccionar tile para inspección.
    SelectTileForInspection(TileCoord),
    /// Manejar destino de orden (modo órdenes activo).
    HandleOrderDestination(TileCoord),
    /// Iniciar edición de órdenes de vehículo.
    StartOrderEditForVehicle(u32),
    /// Abrir ventana de vehículo.
    SelectVehicleOnMap(u32),
    /// Abrir ventana de pueblo.
    OpenTownWindow(u32),
    /// Abrir panel de industria.
    OpenIndustryPanel(TileCoord),
    /// Abrir panel de depósito.
    OpenDepotPanel {
        depot_pos: TileCoord,
        vehicle_id: Option<u32>,
    },
    /// Abrir panel de estación.
    OpenStationPanel(TileCoord),
    /// Iniciar drag de construcción.
    StartDrag {
        action: BuildMenuAction,
        start_tile: (i32, i32),
        rail_lane_bit: Option<u8>,
        signal_drag_fract: Option<(u8, u8)>,
        press_world_pos: Vec2,
    },
    /// Actualizar drag en progreso.
    UpdateDrag {
        end_tile: (i32, i32),
        signal_tap: bool,
    },
    /// Confirmar colocación drag.
    ConfirmDrag { signal_tap: bool },
    /// Herramienta JoinStation: primer, segundo o tercer clic.
    JoinStationClick {
        clicked: TileCoord,
        keep: Option<TileCoord>,
    },
    /// Construir acción inmediata (sin drag).
    BuildImmediate {
        action: BuildMenuAction,
        pos: TileCoord,
        rail_lane_bit: Option<u8>,
        tile_fract: (u8, u8),
        ctrl_held: bool,
        cycle_signal: bool,
    },
}

/// Resuelve la intención de clic basándose en el contexto sin efectos secundarios.
pub(crate) fn resolve_click_intent(ctx: &MapClickContext) -> MapClickIntent {
    // Prioridad 1: cancelar drag con clic derecho
    if ctx.mouse_right_pressed && ctx.drag_armed {
        return MapClickIntent::CancelDrag;
    }

    // Prioridad 2: modo órdenes
    if ctx.orders_mode {
        if ctx.order_pick_active {
            if ctx.mouse_right_pressed {
                // Cancelar selección de orden
                return MapClickIntent::Ignore; // Se maneja en apply como cancel
            }
            if ctx.mouse_left_pressed && ctx.order_vehicle_selected {
                return MapClickIntent::HandleOrderDestination(ctx.tile_pos);
            }
            return MapClickIntent::Ignore;
        }
        if ctx.mouse_left_pressed
            && ctx.active_tool == Some(BuildMenuAction::Orders)
            && let Some(vehicle_id) = ctx.vehicle_under_cursor
        {
            return MapClickIntent::StartOrderEditForVehicle(vehicle_id);
        }
        return MapClickIntent::Ignore;
    }

    // Prioridad 3: sin herramienta activa → selección/inspección
    let Some(action) = ctx.active_tool else {
        if ctx.mouse_left_pressed {
            // Prioridad: tipo de tile antes que vehicle/town, excepto para tiles sin interacción especial
            match ctx.tile_kind {
                Some(TileKind::Industry) => {
                    return MapClickIntent::OpenIndustryPanel(ctx.tile_pos);
                }
                Some(TileKind::RoadDepot)
                | Some(TileKind::RailDepot)
                | Some(TileKind::ShipDepot) => {
                    return MapClickIntent::OpenDepotPanel {
                        depot_pos: ctx.tile_pos,
                        vehicle_id: ctx.vehicle_under_cursor,
                    };
                }
                Some(TileKind::Airport) if ctx.is_hangar => {
                    return MapClickIntent::OpenDepotPanel {
                        depot_pos: ctx.tile_pos,
                        vehicle_id: ctx.vehicle_under_cursor,
                    };
                }
                Some(TileKind::Airport) => {
                    return MapClickIntent::OpenStationPanel(ctx.tile_pos);
                }
                Some(TileKind::Station) => {
                    let station_pos = ctx.station_pos_at_tile.unwrap_or(ctx.tile_pos);
                    return MapClickIntent::OpenStationPanel(station_pos);
                }
                Some(TileKind::House) => {
                    if let Some(town_id) = ctx.town_label_under_cursor {
                        return MapClickIntent::OpenTownWindow(town_id);
                    }
                }
                _ => {}
            }

            // Fallback: vehicle o town label
            if let Some(vehicle_id) = ctx.vehicle_under_cursor {
                return MapClickIntent::SelectVehicleOnMap(vehicle_id);
            }
            if let Some(town_id) = ctx.town_label_under_cursor {
                return MapClickIntent::OpenTownWindow(town_id);
            }

            return MapClickIntent::SelectTileForInspection(ctx.tile_pos);
        }
        return MapClickIntent::Ignore;
    };

    // Prioridad 4: acciones que soportan drag
    if action_supports_drag(action) {
        return resolve_drag_intent(ctx, action);
    }

    // Prioridad 5: JoinStation (sin drag)
    if ctx.mouse_left_pressed && action == BuildMenuAction::JoinStation {
        if let Some(clicked) = ctx.station_pos_at_tile {
            return MapClickIntent::JoinStationClick {
                clicked,
                keep: ctx.join_station_keep,
            };
        }
        return MapClickIntent::Ignore;
    }

    // Prioridad 6: construcción inmediata
    if ctx.mouse_left_pressed {
        let mut cycle_signal = false;
        if ctx.ctrl_held && action == BuildMenuAction::RailSignals && ctx.signal_tile_has_signals {
            cycle_signal = true;
        }
        return MapClickIntent::BuildImmediate {
            action,
            pos: ctx.tile_pos,
            rail_lane_bit: None,
            tile_fract: ctx.tile_fract,
            ctrl_held: ctx.ctrl_held,
            cycle_signal,
        };
    }

    MapClickIntent::Ignore
}

fn resolve_drag_intent(ctx: &MapClickContext, action: BuildMenuAction) -> MapClickIntent {
    if !ctx.drag_armed || ctx.drag_last_action != Some(action) {
        if ctx.mouse_left_pressed {
            let start = (ctx.tile_pos.x, ctx.tile_pos.y);
            let signal_drag_fract =
                if action == BuildMenuAction::RailSignals || action == BuildMenuAction::Clear {
                    Some(ctx.tile_fract)
                } else {
                    None
                };
            return MapClickIntent::StartDrag {
                action,
                start_tile: start,
                rail_lane_bit: None,
                signal_drag_fract,
                press_world_pos: ctx.world_pos,
            };
        }
        return MapClickIntent::Ignore;
    }

    let start = ctx
        .drag_start_tile
        .unwrap_or((ctx.tile_pos.x, ctx.tile_pos.y));
    const SIGNAL_TAP_MAX_PX: f32 = 10.0;
    let signal_tap = action == BuildMenuAction::RailSignals
        && ctx.world_pos.distance(ctx.world_pos) <= SIGNAL_TAP_MAX_PX;
    let end = if signal_tap {
        start
    } else {
        (ctx.tile_pos.x, ctx.tile_pos.y)
    };

    if ctx.mouse_left_released {
        return MapClickIntent::ConfirmDrag { signal_tap };
    }

    MapClickIntent::UpdateDrag {
        end_tile: end,
        signal_tap,
    }
}

fn action_supports_drag(action: BuildMenuAction) -> bool {
    matches!(
        action,
        BuildMenuAction::RailHorz
            | BuildMenuAction::RailVert
            | BuildMenuAction::RailBridge
            | BuildMenuAction::RoadBridge
            | BuildMenuAction::RoadTunnel
            | BuildMenuAction::RailTunnel
            | BuildMenuAction::Road
            | BuildMenuAction::RoadX
            | BuildMenuAction::RoadY
            | BuildMenuAction::Tram
            | BuildMenuAction::TramX
            | BuildMenuAction::TramY
            | BuildMenuAction::RailSignals
            | BuildMenuAction::Clear
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_ctx() -> MapClickContext {
        MapClickContext {
            tile_pos: TileCoord::new(10, 10),
            world_pos: Vec2::new(100.0, 100.0),
            tile_fract: (128, 128),
            mouse_left_pressed: false,
            mouse_right_pressed: false,
            mouse_left_released: false,
            active_tool: None,
            drag_armed: false,
            drag_last_action: None,
            drag_start_tile: None,
            vehicle_under_cursor: None,
            town_label_under_cursor: None,
            tile_kind: None,
            orders_mode: false,
            order_pick_active: false,
            order_vehicle_selected: false,
            is_hangar: false,
            station_pos_at_tile: None,
            join_station_keep: None,
            signal_tile_has_signals: false,
            ctrl_held: false,
        }
    }

    #[test]
    fn test_cancel_drag_on_right_click() {
        let mut ctx = default_ctx();
        ctx.mouse_right_pressed = true;
        ctx.drag_armed = true;
        let intent = resolve_click_intent(&ctx);
        assert_eq!(intent, MapClickIntent::CancelDrag);
    }

    #[test]
    fn test_select_tile_no_tool() {
        let mut ctx = default_ctx();
        ctx.mouse_left_pressed = true;
        ctx.active_tool = None;
        let intent = resolve_click_intent(&ctx);
        assert_eq!(
            intent,
            MapClickIntent::SelectTileForInspection(TileCoord::new(10, 10))
        );
    }

    #[test]
    fn test_select_vehicle_on_map() {
        let mut ctx = default_ctx();
        ctx.mouse_left_pressed = true;
        ctx.active_tool = None;
        ctx.vehicle_under_cursor = Some(42);
        let intent = resolve_click_intent(&ctx);
        assert_eq!(intent, MapClickIntent::SelectVehicleOnMap(42));
    }

    #[test]
    fn test_open_depot_panel() {
        let mut ctx = default_ctx();
        ctx.mouse_left_pressed = true;
        ctx.active_tool = None;
        ctx.tile_kind = Some(TileKind::RoadDepot);
        ctx.vehicle_under_cursor = Some(7);
        let intent = resolve_click_intent(&ctx);
        assert_eq!(
            intent,
            MapClickIntent::OpenDepotPanel {
                depot_pos: TileCoord::new(10, 10),
                vehicle_id: Some(7),
            }
        );
    }

    #[test]
    fn test_start_drag_rail() {
        let mut ctx = default_ctx();
        ctx.mouse_left_pressed = true;
        ctx.active_tool = Some(BuildMenuAction::RailHorz);
        let intent = resolve_click_intent(&ctx);
        match intent {
            MapClickIntent::StartDrag { action, .. } => {
                assert_eq!(action, BuildMenuAction::RailHorz);
            }
            _ => panic!("Expected StartDrag"),
        }
    }

    #[test]
    fn test_join_station_first_click() {
        let mut ctx = default_ctx();
        ctx.mouse_left_pressed = true;
        ctx.active_tool = Some(BuildMenuAction::JoinStation);
        ctx.station_pos_at_tile = Some(TileCoord::new(10, 10));
        let intent = resolve_click_intent(&ctx);
        assert_eq!(
            intent,
            MapClickIntent::JoinStationClick {
                clicked: TileCoord::new(10, 10),
                keep: None,
            }
        );
    }
}
