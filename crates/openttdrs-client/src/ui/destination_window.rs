//! Subventana «Destinos» para añadir paradas a la ruta del vehículo seleccionado.
//!
//! Multi-instancia (#244): un picker por chain slot, abierto desde su panel Órdenes.

use bevy::prelude::*;
use openttdrs_core::prelude::*;

use crate::render::RemapMapVisualsPending;
use crate::state::{OrderPickState, SimWorld};
use crate::ui::floating_window::{
    FloatingWindow, FloatingWindowClosed, FloatingWindowId, TITLE_BROWN, WINDOW_TEXT, WindowKey,
    spawn_floating_window_keyed, window_text_font,
};
use crate::ui::font::UiFontRole;
use crate::ui::hud::{HudBuildFeedback, push_build_command_error};
use crate::ui::scrollbar::spawn_classic_scroll_area_with;
use crate::ui::toolbar::build_input::orders::order_for_clicked_tile;
use crate::ui::toolbar::{
    BuildMenuUi, DragBuildState, OrderEditState, start_order_destination_pick,
    try_append_order_at_tile,
};
use crate::ui::vehicle_chain::{
    MAX_VEHICLE_CHAIN_SLOTS, VehicleChainSlot, vehicle_window_key,
};

const DEST_ROWS: usize = 28;
const BASE_POS: Vec2 = Vec2::new(280.0, 64.0);
const SLOT_OFFSET: Vec2 = Vec2::new(36.0, 36.0);

const BTN_BG: Color = Color::srgb(0.36, 0.31, 0.21);
const BTN_BG_HOVER: Color = Color::srgb(0.44, 0.38, 0.26);
const BTN_BORDER: Color = Color::srgb(0.66, 0.58, 0.38);

#[derive(Resource, Debug)]
pub(crate) struct DestinationPickerState {
    /// Abierto por chain slot (ligado al panel Órdenes que lo abrió).
    pub open: [bool; MAX_VEHICLE_CHAIN_SLOTS],
}

impl Default for DestinationPickerState {
    fn default() -> Self {
        Self {
            open: [false; MAX_VEHICLE_CHAIN_SLOTS],
        }
    }
}

impl DestinationPickerState {
    pub(crate) fn open_for_chain_slot(&mut self, slot: u8) {
        if (slot as usize) < MAX_VEHICLE_CHAIN_SLOTS {
            self.open[slot as usize] = true;
        }
    }

    pub(crate) fn close_slot(&mut self, slot: u8) {
        if (slot as usize) < MAX_VEHICLE_CHAIN_SLOTS {
            self.open[slot as usize] = false;
        }
    }

    pub(crate) fn close_vehicle(&mut self, order_state: &OrderEditState, vehicle_id: u32) {
        for (i, slot) in order_state.slots.iter().enumerate() {
            if slot.vehicle_id == Some(vehicle_id) {
                self.open[i] = false;
            }
        }
    }

    #[must_use]
    pub(crate) fn any_open(&self) -> bool {
        self.open.iter().any(|&o| o)
    }
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
    for slot in 0..MAX_VEHICLE_CHAIN_SLOTS {
        let slot_u8 = slot as u8;
        let pos = BASE_POS + SLOT_OFFSET * slot as f32;
        let (root, content) = spawn_floating_window_keyed(
            &mut commands,
            asset_server,
            WindowKey {
                class: FloatingWindowId::DestinationPicker,
                instance: 0,
            },
            "Destinos",
            TITLE_BROWN,
            pos,
            360.0,
        );
        commands.entity(root).insert(VehicleChainSlot(slot_u8));
        spawn_destination_content(&mut commands, content, asset_server, slot_u8);
    }
}

fn spawn_destination_content(
    commands: &mut Commands,
    content: Entity,
    asset_server: &AssetServer,
    chain_slot: u8,
) {
    let chain = VehicleChainSlot(chain_slot);
    commands.entity(content).with_children(|panel| {
        panel.spawn((
            Text::new("Elige un destino para añadirlo a la ruta."),
            window_text_font(asset_server, UiFontRole::Caption),
            TextColor(WINDOW_TEXT),
        ));
        spawn_classic_scroll_area_with(
            panel,
            asset_server,
            Node {
                flex_grow: 1.0,
                min_width: Val::Px(0.0),
                overflow: Overflow::scroll_y(),
                ..default()
            },
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(2.0),
                ..default()
            },
            Color::srgb(0.22, 0.18, 0.12),
            BTN_BORDER,
            (),
            (),
            |list| {
                for row_slot in 0..DEST_ROWS {
                    list.spawn((
                        Button,
                        DestinationPickerRow { slot: row_slot },
                        chain,
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
                            DestinationPickerRowText { slot: row_slot },
                            chain,
                            Text::new(""),
                            window_text_font(asset_server, UiFontRole::Caption),
                            TextColor(Color::srgb(0.92, 0.88, 0.72)),
                        )],
                    ));
                }
            },
            280.0,
        );
        panel
            .spawn((
                Button,
                DestinationPickerButton::PickOnMap,
                chain,
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
            StopKind::Buoy => "Boya",
            StopKind::Airport => "Aeropuerto",
            StopKind::RailWaypoint => "Waypoint",
            StopKind::RoadWaypoint => "Waypoint road",
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
        VehicleKind::Bus | VehicleKind::Truck | VehicleKind::Tram => Some(TileKind::RoadDepot),
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
    mut root_q: Query<(&mut FloatingWindow, &VehicleChainSlot, &mut Visibility)>,
    mut row_q: Query<
        (
            &VehicleChainSlot,
            &DestinationPickerRow,
            &Interaction,
            &mut Node,
            &mut BackgroundColor,
        ),
        (With<Button>, With<DestinationPickerRow>),
    >,
    mut row_text_q: Query<(&VehicleChainSlot, &DestinationPickerRowText, &mut Text)>,
    sim: Res<SimWorld>,
) {
    for (mut win, chain_slot, mut vis) in &mut root_q {
        if win.id != FloatingWindowId::DestinationPicker {
            continue;
        }
        let idx = chain_slot.0 as usize;
        if idx >= MAX_VEHICLE_CHAIN_SLOTS {
            continue;
        }
        let vehicle_id = order_state.slots[idx].vehicle_id;
        win.key = vehicle_window_key(
            FloatingWindowId::DestinationPicker,
            vehicle_id.unwrap_or(0),
        );
        let show = picker_state.open[idx] && vehicle_id.is_some();
        if !show {
            *vis = Visibility::Hidden;
            for (row_chain, _, _, mut node, _) in &mut row_q {
                if row_chain.0 == chain_slot.0 {
                    node.display = Display::None;
                }
            }
            continue;
        }
        *vis = Visibility::Visible;
        let candidates = vehicle_id
            .map(|id| destinations_for_vehicle(&sim, id))
            .unwrap_or_default();
        for (row_chain, row, interaction, mut node, mut bg) in &mut row_q {
            if row_chain.0 != chain_slot.0 {
                continue;
            }
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
        for (text_chain, row_text, mut text) in &mut row_text_q {
            if text_chain.0 != chain_slot.0 {
                continue;
            }
            if let Some(candidate) = candidates.get(row_text.slot) {
                **text = candidate.label.clone();
            } else {
                **text = String::new();
            }
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
            (&Interaction, &DestinationPickerRow, &VehicleChainSlot),
            (
                Changed<Interaction>,
                With<Button>,
                With<DestinationPickerRow>,
            ),
        >,
        Query<
            (&Interaction, &DestinationPickerButton, &VehicleChainSlot),
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
    if !picker_state.any_open() {
        return;
    }
    for (interaction, row, chain_slot) in button_q.p0().iter() {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let idx = chain_slot.0 as usize;
        if idx >= MAX_VEHICLE_CHAIN_SLOTS || !picker_state.open[idx] {
            continue;
        }
        let Some(vehicle_id) = order_state.slots[idx].vehicle_id else {
            continue;
        };
        order_state.focused = Some(vehicle_id);
        let candidates = destinations_for_vehicle(&sim, vehicle_id);
        let Some(candidate) = candidates.get(row.slot) else {
            continue;
        };
        let Some(orders) = order_state.orders_mut() else {
            continue;
        };
        match try_append_order_at_tile(&mut sim, vehicle_id, candidate.pos, orders) {
            Ok(()) => {
                pending.pending = true;
                picker_state.close_slot(chain_slot.0);
            }
            Err(e) => push_build_command_error(&mut hud_feedback, e, time.elapsed_secs()),
        }
    }
    for (interaction, _, chain_slot) in button_q.p1().iter() {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let idx = chain_slot.0 as usize;
        if idx >= MAX_VEHICLE_CHAIN_SLOTS {
            continue;
        }
        if let Some(vid) = order_state.slots[idx].vehicle_id {
            order_state.focused = Some(vid);
        }
        picker_state.close_slot(chain_slot.0);
        start_order_destination_pick(&order_state, &mut next_pick);
        drag_state.armed = false;
        drag_state.pending_tiles.clear();
    }
}

pub(crate) fn destination_picker_on_closed(
    mut reader: MessageReader<FloatingWindowClosed>,
    mut picker_state: ResMut<DestinationPickerState>,
    order_state: Res<OrderEditState>,
) {
    for msg in reader.read() {
        if msg.0.class != FloatingWindowId::DestinationPicker {
            continue;
        }
        let vehicle_id = msg.0.instance;
        if vehicle_id == 0 {
            continue;
        }
        picker_state.close_vehicle(&order_state, vehicle_id);
    }
}
