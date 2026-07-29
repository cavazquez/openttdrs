//! Ventana flotante de horario por orden (Sprint F4).

use bevy::prelude::*;
use openttdrs_core::Command;
use openttdrs_core::prelude::*;
use openttdrs_core::{cycle_travel_ticks, cycle_wait_ticks};

use crate::render::RemapMapVisualsPending;
use crate::state::SimWorld;
use crate::ui::floating_window::{
    FloatingWindow, FloatingWindowClosed, FloatingWindowId, FloatingWindowTitleText, TITLE_CRIMSON,
    WINDOW_TEXT, WindowKey, spawn_floating_window_keyed, window_text_font,
};
use crate::ui::font::UiFontRole;
use crate::ui::hud::{HudBuildFeedback, push_build_command_error};
use crate::ui::toolbar::BuildMenuUi;
use crate::ui::vehicle_chain::{
    MAX_VEHICLE_CHAIN_SLOTS, VehicleChainRegistry, VehicleChainSlot, vehicle_window_key,
};

const TIMETABLE_ROWS: usize = 8;
const BASE_POS: Vec2 = Vec2::new(420.0, 220.0);
const SLOT_OFFSET: Vec2 = Vec2::new(36.0, 36.0);
const BTN_BG: Color = Color::srgb(0.36, 0.31, 0.21);
const BTN_BORDER: Color = Color::srgb(0.66, 0.58, 0.38);

#[derive(Resource, Debug)]
pub(crate) struct TimetableWindowState {
    pub(crate) slots: [Option<u32>; MAX_VEHICLE_CHAIN_SLOTS],
    pub(crate) focused: Option<u32>,
}

impl Default for TimetableWindowState {
    fn default() -> Self {
        Self {
            slots: [None; MAX_VEHICLE_CHAIN_SLOTS],
            focused: None,
        }
    }
}

impl TimetableWindowState {
    #[must_use]
    #[allow(dead_code)] // API multi-slot (#244); handlers usan slots[idx] directo.
    pub(crate) fn vehicle_id(&self) -> Option<u32> {
        self.focused
            .filter(|&id| self.slots.iter().any(|&s| s == Some(id)))
    }

    pub(crate) fn close_vehicle(&mut self, vehicle_id: u32) {
        for slot in &mut self.slots {
            if *slot == Some(vehicle_id) {
                *slot = None;
            }
        }
        if self.focused == Some(vehicle_id) {
            self.focused = self.slots.iter().flatten().next().copied();
        }
    }
}

#[derive(Component)]
pub(crate) struct TimetableSummaryText;

/// Contenedor de una fila de orden (no confundir con botones de la fila).
#[derive(Component, Clone, Copy)]
pub(crate) struct TimetableOrderRowStrip {
    index: usize,
}

#[derive(Component, Clone, Copy)]
pub(crate) struct TimetableOrderRowLabel {
    index: usize,
}

#[derive(Component, Clone, Copy)]
pub(crate) struct TimetableRowAction {
    index: usize,
    kind: TimetableRowButton,
}

#[derive(Component, Clone, Copy)]
pub(crate) enum TimetableWindowButton {
    ToggleTimetable,
    ToggleAutofill,
    ClearLateness,
    ToggleSeconds,
    Close,
}

#[derive(Component, Clone, Copy)]
pub(crate) enum TimetableRowButton {
    Wait,
    Travel,
}

pub(crate) fn setup_timetable_window(mut commands: Commands, asset_server: Res<AssetServer>) {
    let asset_server = &*asset_server;
    for slot in 0..MAX_VEHICLE_CHAIN_SLOTS {
        let slot_u8 = slot as u8;
        let pos = BASE_POS + SLOT_OFFSET * slot as f32;
        let (root, content) = spawn_floating_window_keyed(
            &mut commands,
            asset_server,
            WindowKey {
                class: FloatingWindowId::Timetable,
                instance: 0,
            },
            "Horario",
            TITLE_CRIMSON,
            pos,
            420.0,
        );
        commands.entity(root).insert(VehicleChainSlot(slot_u8));
        spawn_timetable_content(&mut commands, content, asset_server, slot_u8);
    }
}

fn spawn_timetable_content(
    commands: &mut Commands,
    content: Entity,
    asset_server: &AssetServer,
    chain_slot: u8,
) {
    let chain = VehicleChainSlot(chain_slot);
    commands.entity(content).with_children(|panel| {
        panel.spawn((
            TimetableSummaryText,
            chain,
            Text::new(""),
            window_text_font(asset_server, UiFontRole::Caption),
            TextColor(WINDOW_TEXT),
        ));
        panel
            .spawn(Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(2.0),
                ..default()
            })
            .with_children(|list| {
                for index in 0..TIMETABLE_ROWS {
                    spawn_timetable_row(list, asset_server, chain, index);
                }
            });
        panel
            .spawn(Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(4.0),
                flex_wrap: FlexWrap::Wrap,
                margin: UiRect::top(Val::Px(4.0)),
                ..default()
            })
            .with_children(|row| {
                spawn_tt_button(
                    row,
                    asset_server,
                    chain,
                    TimetableWindowButton::ToggleTimetable,
                    "Horario ON/OFF",
                );
                spawn_tt_button(
                    row,
                    asset_server,
                    chain,
                    TimetableWindowButton::ToggleAutofill,
                    "Autofill",
                );
                spawn_tt_button(
                    row,
                    asset_server,
                    chain,
                    TimetableWindowButton::ClearLateness,
                    "Poner en hora",
                );
                spawn_tt_button(
                    row,
                    asset_server,
                    chain,
                    TimetableWindowButton::ToggleSeconds,
                    "Ticks/Seg",
                );
                spawn_tt_button(
                    row,
                    asset_server,
                    chain,
                    TimetableWindowButton::Close,
                    "Cerrar",
                );
            });
    });
}

fn spawn_timetable_row(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    chain_slot: VehicleChainSlot,
    index: usize,
) {
    parent
        .spawn((
            TimetableOrderRowStrip { index },
            chain_slot,
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(4.0),
                display: Display::None,
                ..default()
            },
            BuildMenuUi,
        ))
        .with_children(|row| {
            row.spawn((
                TimetableOrderRowLabel { index },
                chain_slot,
                Text::new(""),
                window_text_font(asset_server, UiFontRole::Caption),
                TextColor(WINDOW_TEXT),
            ));
            spawn_row_btn(
                row,
                asset_server,
                chain_slot,
                index,
                TimetableRowButton::Wait,
                "Espera",
                72.0,
            );
            spawn_row_btn(
                row,
                asset_server,
                chain_slot,
                index,
                TimetableRowButton::Travel,
                "Viaje",
                72.0,
            );
        });
}

fn spawn_row_btn(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    chain_slot: VehicleChainSlot,
    index: usize,
    kind: TimetableRowButton,
    label: &'static str,
    width: f32,
) {
    parent.spawn((
        Button,
        TimetableRowAction { index, kind },
        chain_slot,
        Node {
            width: Val::Px(width),
            height: Val::Px(22.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        BackgroundColor(BTN_BG),
        BorderColor::all(BTN_BORDER),
        Interaction::default(),
        BuildMenuUi,
        children![(
            Text::new(label),
            window_text_font(asset_server, UiFontRole::Caption),
            TextColor(Color::srgb(0.92, 0.88, 0.72)),
        )],
    ));
}

fn spawn_tt_button(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    chain_slot: VehicleChainSlot,
    action: TimetableWindowButton,
    label: &str,
) {
    parent.spawn((
        Button,
        action,
        chain_slot,
        Node {
            height: Val::Px(24.0),
            padding: UiRect::horizontal(Val::Px(8.0)),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        BackgroundColor(BTN_BG),
        BorderColor::all(BTN_BORDER),
        Interaction::default(),
        BuildMenuUi,
        children![(
            Text::new(label.to_string()),
            window_text_font(asset_server, UiFontRole::Caption),
            TextColor(Color::srgb(0.92, 0.88, 0.72)),
        )],
    ));
}

fn format_ticks(ticks: u32, seconds_mode: bool) -> String {
    if seconds_mode {
        format!(
            "{:.0}s",
            ticks as f32 / openttdrs_core::SIM_TICKS_PER_SECOND as f32
        )
    } else {
        format!("{ticks}t")
    }
}

fn order_timing_label(order: VehicleOrder, seconds_mode: bool) -> String {
    match order {
        VehicleOrder::Station {
            wait_ticks,
            travel_ticks,
            ..
        }
        | VehicleOrder::Depot {
            wait_ticks,
            travel_ticks,
            ..
        } => {
            format!(
                "esp.{} viaje {}",
                format_ticks(wait_ticks, seconds_mode),
                format_ticks(travel_ticks, seconds_mode)
            )
        }
        VehicleOrder::Waypoint { travel_ticks, .. } => {
            format!("viaje {}", format_ticks(travel_ticks, seconds_mode))
        }
        VehicleOrder::Conditional { .. } => "condicional".into(),
        VehicleOrder::Tile(_) => "—".into(),
    }
}

pub(crate) fn open_timetable_for_vehicle(
    state: &mut TimetableWindowState,
    chain: &VehicleChainRegistry,
    vehicle_id: u32,
) {
    let Some(slot) = chain.slot_of(vehicle_id) else {
        return;
    };
    state.slots[slot as usize] = Some(vehicle_id);
    state.focused = Some(vehicle_id);
}

pub(crate) fn sync_timetable_window(
    tt_state: Res<TimetableWindowState>,
    chain: Res<VehicleChainRegistry>,
    sim: Res<SimWorld>,
    mut root_q: Query<(Entity, &mut FloatingWindow, &VehicleChainSlot, &mut Visibility)>,
    mut title_q: Query<(&FloatingWindowTitleText, &mut Text, &ChildOf)>,
    parents: Query<&ChildOf>,
    mut summary_q: Query<
        (&VehicleChainSlot, &mut Text),
        (With<TimetableSummaryText>, Without<FloatingWindowTitleText>),
    >,
    mut row_strip_q: Query<
        (&VehicleChainSlot, &TimetableOrderRowStrip, &mut Node),
        (Without<Button>, Without<TimetableOrderRowLabel>),
    >,
    mut row_label_q: Query<
        (&VehicleChainSlot, &TimetableOrderRowLabel, &mut Text),
        (
            Without<TimetableSummaryText>,
            Without<FloatingWindowTitleText>,
        ),
    >,
) {
    fn title_root_entity(child_of: &ChildOf, parents: &Query<&ChildOf>) -> Option<Entity> {
        let center = child_of.parent();
        let bar = parents.get(center).ok()?.parent();
        parents.get(bar).ok().map(|c| c.parent())
    }

    for (root_entity, mut win, slot, mut vis) in &mut root_q {
        if win.id != FloatingWindowId::Timetable {
            continue;
        }
        let idx = slot.0 as usize;
        if idx >= MAX_VEHICLE_CHAIN_SLOTS {
            continue;
        }
        let vehicle_id = tt_state.slots[idx].filter(|&id| chain.slot_of(id) == Some(slot.0));
        win.key = vehicle_window_key(FloatingWindowId::Timetable, vehicle_id.unwrap_or(0));
        let Some(vehicle_id) = vehicle_id else {
            *vis = Visibility::Hidden;
            continue;
        };
        let Some(vehicle) = sim.state.vehicles.iter().find(|v| v.id == vehicle_id) else {
            *vis = Visibility::Hidden;
            continue;
        };
        *vis = Visibility::Visible;
        let title_name = format!("Horario — {}", vehicle.display_name());
        for (title, mut text, child_of) in &mut title_q {
            if title.0 != FloatingWindowId::Timetable {
                continue;
            }
            if title_root_entity(child_of, &parents) == Some(root_entity) {
                **text = title_name.clone();
            }
        }
        for (sum_slot, mut summary) in &mut summary_q {
            if sum_slot.0 != slot.0 {
                continue;
            }
            let late = vehicle.timetable_lateness;
            let late_label = if late > 0 {
                format!("+{late}t tarde")
            } else if late < 0 {
                format!("{late}t adelantado")
            } else {
                "en hora".into()
            };
            **summary = format!(
                "Horario: {} · Autofill: {} · {late_label}",
                if vehicle.timetable_active { "ON" } else { "OFF" },
                if vehicle.timetable_autofill { "ON" } else { "OFF" },
            );
        }
        let seconds_mode = vehicle.timetable_display_seconds;
        for (strip_slot, strip, mut node) in &mut row_strip_q {
            if strip_slot.0 != slot.0 {
                continue;
            }
            node.display = if strip.index < vehicle.orders.len() {
                Display::Flex
            } else {
                Display::None
            };
        }
        for (label_slot, label, mut text) in &mut row_label_q {
            if label_slot.0 != slot.0 {
                continue;
            }
            if label.index < vehicle.orders.len() {
                **text = format!(
                    "{}. {}",
                    label.index + 1,
                    order_timing_label(vehicle.orders[label.index], seconds_mode)
                );
            } else {
                **text = String::new();
            }
        }
    }
}

pub(crate) fn timetable_window_on_closed(
    mut closed: MessageReader<FloatingWindowClosed>,
    mut tt_state: ResMut<TimetableWindowState>,
) {
    for msg in closed.read() {
        if msg.0.class != FloatingWindowId::Timetable {
            continue;
        }
        let vehicle_id = msg.0.instance;
        if vehicle_id == 0 {
            continue;
        }
        tt_state.close_vehicle(vehicle_id);
    }
}

pub(crate) fn handle_timetable_window_buttons(
    mut btn_q: Query<
        (&Interaction, &TimetableWindowButton, &VehicleChainSlot),
        (Changed<Interaction>, With<Button>),
    >,
    mut row_btn_q: Query<
        (&Interaction, &TimetableRowAction, &VehicleChainSlot),
        (
            Changed<Interaction>,
            With<Button>,
            Without<TimetableWindowButton>,
        ),
    >,
    mut tt_state: ResMut<TimetableWindowState>,
    mut sim: ResMut<SimWorld>,
    mut pending: ResMut<RemapMapVisualsPending>,
    mut hud_feedback: ResMut<HudBuildFeedback>,
    time: Res<Time>,
) {
    for (interaction, action, chain_slot) in &mut row_btn_q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let idx = chain_slot.0 as usize;
        let Some(vehicle_id) = tt_state.slots.get(idx).copied().flatten() else {
            continue;
        };
        tt_state.focused = Some(vehicle_id);
        let Some(vehicle) = sim.state.vehicles.iter().find(|v| v.id == vehicle_id) else {
            continue;
        };
        if action.index >= vehicle.orders.len() {
            continue;
        }
        let order = vehicle.orders[action.index];
        let cmd = match action.kind {
            TimetableRowButton::Wait => {
                let next = cycle_wait_ticks(order.wait_ticks());
                Command::SetVehicleOrderWaitTicks {
                    vehicle_id,
                    index: action.index,
                    wait_ticks: next,
                }
            }
            TimetableRowButton::Travel => {
                let next = cycle_travel_ticks(order.travel_ticks());
                Command::SetVehicleOrderTravelTicks {
                    vehicle_id,
                    index: action.index,
                    travel_ticks: next,
                }
            }
        };
        match crate::network::apply_player_command(&mut sim.state, &cmd) {
            Ok(()) => pending.pending = true,
            Err(e) => push_build_command_error(&mut hud_feedback, e, time.elapsed_secs()),
        }
    }

    for (interaction, button, chain_slot) in &mut btn_q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let idx = chain_slot.0 as usize;
        let Some(vehicle_id) = tt_state.slots.get(idx).copied().flatten() else {
            continue;
        };
        tt_state.focused = Some(vehicle_id);
        match button {
            TimetableWindowButton::ToggleTimetable => {
                let _ = crate::network::apply_player_command(
                    &mut sim.state,
                    &Command::ToggleVehicleTimetable(vehicle_id),
                );
                pending.pending = true;
            }
            TimetableWindowButton::ToggleAutofill => {
                let _ = crate::network::apply_player_command(
                    &mut sim.state,
                    &Command::ToggleVehicleTimetableAutofill(vehicle_id),
                );
                pending.pending = true;
            }
            TimetableWindowButton::ClearLateness => {
                let _ = crate::network::apply_player_command(
                    &mut sim.state,
                    &Command::ClearVehicleTimetableLateness(vehicle_id),
                );
                pending.pending = true;
            }
            TimetableWindowButton::ToggleSeconds => {
                if let Some(v) = sim.state.vehicles.iter_mut().find(|v| v.id == vehicle_id) {
                    v.timetable_display_seconds = !v.timetable_display_seconds;
                }
            }
            TimetableWindowButton::Close => {
                tt_state.close_vehicle(vehicle_id);
            }
        }
    }
}
