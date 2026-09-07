use bevy::prelude::*;

use crate::state::SimWorld;
use crate::ui::hud::HoveredTileCoord;
use crate::ui::toolbar::{BuildMenuAction, DragBuildState, StationBuildState, UiToolState};

pub(crate) fn rotate_station_with_right_click(
    mouse: Res<ButtonInput<MouseButton>>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut tool_state: ResMut<UiToolState>,
    mut station_state: ResMut<StationBuildState>,
    mut drag_state: ResMut<DragBuildState>,
    sim: Option<Res<SimWorld>>,
    hovered: Option<Res<HoveredTileCoord>>,
) {
    if !mouse.just_pressed(MouseButton::Right) {
        return;
    }
    if drag_state.armed {
        drag_state.armed = false;
        drag_state.start_tile = None;
        drag_state.last_tile = None;
        drag_state.last_action = None;
        drag_state.pending_tiles.clear();
        return;
    }
    match tool_state.active_tool {
        Some(BuildMenuAction::Station)
        | Some(BuildMenuAction::BusStop)
        | Some(BuildMenuAction::RoadDepot)
        | Some(BuildMenuAction::RailDepot)
        | Some(BuildMenuAction::ShipDepot)
        | Some(BuildMenuAction::Dock)
        | Some(BuildMenuAction::Lock) => {
            station_state.orientation = (station_state.orientation + 1) % 4;
        }
        Some(BuildMenuAction::RailStation) => {
            station_state.rail_axis_y = !station_state.rail_axis_y;
        }
        Some(BuildMenuAction::Airport) => {
            let selected_newgrf = sim.as_ref().and_then(|sim| {
                sim.state.current_airport_newgrf_id.and_then(|id| {
                    openttdrs_core::newgrf_airport_spec_def(&sim.state.airport_spec_catalog, id)
                })
            });
            if let Some(def) = selected_newgrf.filter(|def| !def.layouts.is_empty()) {
                let max = u8::try_from(def.layouts.len() - 1).unwrap_or(u8::MAX);
                let next = station_state.airport_layout.unwrap_or(0).min(max);
                let next = if next == max { 0 } else { next + 1 };
                station_state.airport_layout = Some(next);
                station_state.airport_newgrf_spec_id = Some(def.id);
                station_state.airport_axis_y = def.layouts[usize::from(next)].rotation & 6 == 2
                    || def.layouts[usize::from(next)].rotation & 6 == 6;
            } else {
                station_state.airport_layout = None;
                station_state.airport_newgrf_spec_id = None;
                station_state.airport_axis_y = !station_state.airport_axis_y;
            }
        }
        Some(BuildMenuAction::RailSignals) => {
            let ctrl =
                keyboard.pressed(KeyCode::ControlLeft) || keyboard.pressed(KeyCode::ControlRight);
            let shift =
                keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight);
            if shift {
                // Cicla densidad 1→2→4→8→12→16→1 (como pasos útiles de OpenTTD).
                station_state.signal_density = match station_state.signal_density {
                    1 => 2,
                    2 => 4,
                    4 => 8,
                    8 => 12,
                    12 => 16,
                    _ => 1,
                };
            } else if ctrl {
                station_state.signal_type =
                    openttdrs_core::next_placeable_signal_type(station_state.signal_type);
            } else if let (Some(sim), Some(hover)) = (sim.as_ref(), hovered.as_ref()) {
                if let Some(coord) = hover.pos {
                    if let Some(tile) = sim.state.map.get(coord) {
                        let tb = tile.m5 & 0x3F;
                        if let Some(track) = openttdrs_core::rail_signals::resolve_signal_track(
                            tb,
                            hover.fract_x,
                            hover.fract_y,
                        ) {
                            station_state.orientation =
                                openttdrs_core::rail_signals::cycle_signal_facing(
                                    track,
                                    station_state.orientation,
                                );
                        } else {
                            station_state.orientation = (station_state.orientation + 1) % 4;
                        }
                    } else {
                        station_state.orientation = (station_state.orientation + 1) % 4;
                    }
                } else {
                    station_state.orientation = (station_state.orientation + 1) % 4;
                }
            } else {
                station_state.orientation = (station_state.orientation + 1) % 4;
            }
        }
        Some(BuildMenuAction::RoadX) => {
            tool_state.active_tool = Some(BuildMenuAction::RoadY);
        }
        Some(BuildMenuAction::RoadY) => {
            tool_state.active_tool = Some(BuildMenuAction::RoadX);
        }
        Some(BuildMenuAction::Road) => {
            tool_state.active_tool = Some(BuildMenuAction::RoadX);
        }
        Some(BuildMenuAction::TramX) => {
            tool_state.active_tool = Some(BuildMenuAction::TramY);
        }
        Some(BuildMenuAction::TramY) => {
            tool_state.active_tool = Some(BuildMenuAction::TramX);
        }
        Some(BuildMenuAction::Tram) => {
            tool_state.active_tool = Some(BuildMenuAction::TramX);
        }
        _ => return,
    }
    drag_state.armed = false;
    drag_state.start_tile = None;
    drag_state.last_tile = None;
    drag_state.last_action = None;
    drag_state.pending_tiles.clear();
}
