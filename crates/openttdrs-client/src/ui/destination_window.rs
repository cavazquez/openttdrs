//! Subventana «Destinos» para añadir paradas a la ruta del vehículo seleccionado.

use bevy::prelude::*;
use openttdrs_core::{StopKind, TileCoord, TileKind, VehicleKind};

use crate::render::RemapMapVisualsPending;
use crate::state::{OrderPickState, SimWorld};
use crate::ui::floating_window::{
    FloatingWindow, FloatingWindowClosed, FloatingWindowId, TITLE_BROWN, WINDOW_TEXT,
    spawn_floating_window, window_text_font,
};
use crate::ui::font::UiFontRole;
use crate::ui::hud::{HudBuildFeedback, push_build_command_error};
use crate::ui::toolbar::build_input::orders::order_for_clicked_tile;
use crate::ui::toolbar::{
    BuildMenuUi, DragBuildState, OrderEditState, start_order_destination_pick,
    try_append_order_at_tile,
};

const DEST_ROWS: usize = 28;

const BTN_BG: Color = Color::srgb(0.36, 0.31, 0.21);
const BTN_BG_HOVER: Color = Color::srgb(0.44, 0.38, 0.26);
const BTN_BORDER: Color = Color::srgb(0.66, 0.58, 0.38);

#[derive(Resource, Default)]
pub(crate) struct DestinationPickerState {
    pub open: bool,
}

#[derive(Clone)]
struct DestCandidate {
    label: String,
    pos: TileCoord,
}

#[derive(Component, Clone, Copy)]
pub(crate) struct DestinationPickerRow {
    slot: usize,
}

#[derive(Component, Clone, Copy)]
pub(crate) struct DestinationPickerRowText {
    slot: usize,
}

#[derive(Component, Clone, Copy)]
pub(crate) enum DestinationPickerButton {
    PickOnMap,
}

pub(crate) fn setup_destination_picker(mut commands: Commands, asset_server: Res<AssetServer>) {
    let asset_server = &*asset_server;
    let (_root, content) = spawn_floating_window(
        &mut commands,
        asset_server,
        FloatingWindowId::DestinationPicker,
        "Destinos",
        TITLE_BROWN,
        Vec2::new(280.0, 64.0),
        360.0,
    );
    commands.entity(content).with_children(|panel| {
        panel.spawn((
            Text::new("Elige un destino para añadirlo a la ruta."),
            window_text_font(asset_server, UiFontRole::Caption),
            TextColor(WINDOW_TEXT),
        ));
        panel
            .spawn(Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(2.0),
                margin: UiRect::top(Val::Px(6.0)),
                max_height: Val::Px(280.0),
                overflow: Overflow::scroll_y(),
                ..default()
            })
            .with_children(|list| {
                for slot in 0..DEST_ROWS {
                    list.spawn((
                        Button,
                        DestinationPickerRow { slot },
                        Node {
                            width: Val::Percent(100.0),
                            height: Val::Px(22.0),
                            padding: UiRect::horizontal(Val::Px(6.0)),
                            justify_content: JustifyContent::FlexStart,
                            align_items: AlignItems::Center,
                            border: UiRect::all(Val::Px(1.0)),
                            display: Display::None,
                            ..default()
                        },
                        BackgroundColor(BTN_BG),
                        BorderColor::all(BTN_BORDER),
                        Interaction::default(),
                        BuildMenuUi,
                        children![(
                            DestinationPickerRowText { slot },
                            Text::new(""),
                            window_text_font(asset_server, UiFontRole::Caption),
                            TextColor(Color::srgb(0.92, 0.88, 0.72)),
                        )],
                    ));
                }
            });
        panel
            .spawn((
                Button,
                DestinationPickerButton::PickOnMap,
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(24.0),
                    margin: UiRect::top(Val::Px(6.0)),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(BTN_BG),
                BorderColor::all(BTN_BORDER),
                Interaction::default(),
                BuildMenuUi,
            ))
            .with_children(|b| {
                b.spawn((
                    Text::new("Elegir en el mapa"),
                    window_text_font(asset_server, UiFontRole::Caption),
                    TextColor(Color::srgb(0.92, 0.88, 0.72)),
                ));
            });
    });
}

fn destinations_for_vehicle(sim: &SimWorld, vehicle_id: u32) -> Vec<DestCandidate> {
    let Some(vehicle) = sim.state.vehicles.iter().find(|v| v.id == vehicle_id) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for station in &sim.state.stations {
        if !station.can_service_vehicle(vehicle.kind) {
            continue;
        }
        let kind_label = match station.stop_kind {
            StopKind::BusStop => "Parada bus",
            StopKind::TruckStop => "Parada carga",
            StopKind::RailStation => "Estación tren",
            StopKind::Dock => "Muelle",
            StopKind::Airport => "Aeropuerto",
            StopKind::RailWaypoint => "Waypoint",
        };
        let name = station
            .name
            .as_deref()
            .filter(|n| !n.is_empty())
            .unwrap_or(kind_label);
        out.push(DestCandidate {
            label: format!("{name} ({}, {})", station.pos.x, station.pos.y),
            pos: station.pos,
        });
    }
    let depot_kind = match vehicle.kind {
        VehicleKind::Train => Some(TileKind::RailDepot),
        VehicleKind::Bus | VehicleKind::Truck => Some(TileKind::RoadDepot),
        VehicleKind::Ship => Some(TileKind::ShipDepot),
        VehicleKind::Aircraft => Some(TileKind::Airport),
    };
    if let Some(kind) = depot_kind {
        let (w, h) = sim.state.map.dimensions();
        for y in 0..h {
            for x in 0..w {
                let pos = TileCoord::new(x as i32, y as i32);
                if sim.state.map.get_kind(pos) == Some(kind)
                    && order_for_clicked_tile(sim, vehicle_id, pos).is_some()
                {
                    let label = if kind == TileKind::RailDepot {
                        "Depósito vía"
                    } else {
                        "Depósito"
                    };
                    out.push(DestCandidate {
                        label: format!("{label} ({x}, {y})"),
                        pos,
                    });
                }
            }
        }
    }
    out.sort_by(|a, b| a.label.cmp(&b.label));
    out
}

#[allow(clippy::type_complexity)]
pub(crate) fn sync_destination_picker(
    picker_state: Res<DestinationPickerState>,
    order_state: Res<OrderEditState>,
    mut root_q: Query<(&FloatingWindow, &mut Visibility)>,
    mut row_q: Query<
        (
            &DestinationPickerRow,
            &Interaction,
            &mut Node,
            &mut BackgroundColor,
        ),
        (With<Button>, With<DestinationPickerRow>),
    >,
    mut row_text_q: Query<(&DestinationPickerRowText, &mut Text)>,
    sim: Res<SimWorld>,
) {
    let Some((_, mut vis)) = root_q
        .iter_mut()
        .find(|(w, _)| w.id == FloatingWindowId::DestinationPicker)
    else {
        return;
    };
    if !picker_state.open || order_state.vehicle_id.is_none() {
        *vis = Visibility::Hidden;
        return;
    }
    *vis = Visibility::Visible;
    let candidates = order_state
        .vehicle_id
        .map(|id| destinations_for_vehicle(&sim, id))
        .unwrap_or_default();
    for (row, interaction, mut node, mut bg) in &mut row_q {
        if candidates.get(row.slot).is_none() {
            node.display = Display::None;
            continue;
        }
        node.display = Display::Flex;
        *bg = if *interaction == Interaction::Hovered {
            BackgroundColor(BTN_BG_HOVER)
        } else {
            BackgroundColor(BTN_BG)
        };
    }
    for (row_text, mut text) in &mut row_text_q {
        if let Some(candidate) = candidates.get(row_text.slot) {
            **text = candidate.label.clone();
        } else {
            **text = String::new();
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn handle_destination_picker_buttons(
    mut picker_state: ResMut<DestinationPickerState>,
    mut order_state: ResMut<OrderEditState>,
    mut next_pick: ResMut<NextState<OrderPickState>>,
    mut button_q: ParamSet<(
        Query<
            (&Interaction, &DestinationPickerRow),
            (
                Changed<Interaction>,
                With<Button>,
                With<DestinationPickerRow>,
            ),
        >,
        Query<
            (&Interaction, &DestinationPickerButton),
            (
                Changed<Interaction>,
                With<Button>,
                With<DestinationPickerButton>,
            ),
        >,
    )>,
    mut drag_state: ResMut<DragBuildState>,
    mut sim: ResMut<SimWorld>,
    mut pending: ResMut<RemapMapVisualsPending>,
    mut hud_feedback: ResMut<HudBuildFeedback>,
    time: Res<Time>,
) {
    if !picker_state.open {
        return;
    }
    let Some(vehicle_id) = order_state.vehicle_id else {
        return;
    };
    for (interaction, row) in button_q.p0().iter() {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let candidates = destinations_for_vehicle(&sim, vehicle_id);
        let Some(candidate) = candidates.get(row.slot) else {
            continue;
        };
        match try_append_order_at_tile(&mut sim, vehicle_id, candidate.pos, &mut order_state.orders)
        {
            Ok(()) => {
                pending.pending = true;
                picker_state.open = false;
            }
            Err(e) => push_build_command_error(&mut hud_feedback, e, time.elapsed_secs()),
        }
    }
    for (interaction, _) in button_q.p1().iter() {
        if *interaction != Interaction::Pressed {
            continue;
        }
        picker_state.open = false;
        start_order_destination_pick(&order_state, &mut next_pick);
        drag_state.armed = false;
        drag_state.pending_tiles.clear();
    }
}

pub(crate) fn destination_picker_on_closed(
    mut reader: MessageReader<FloatingWindowClosed>,
    mut picker_state: ResMut<DestinationPickerState>,
) {
    for msg in reader.read() {
        if msg.0 == FloatingWindowId::DestinationPicker {
            picker_state.open = false;
        }
    }
}
