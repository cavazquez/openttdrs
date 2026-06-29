//! Ventana flotante de horario por orden (Sprint F4).

use bevy::prelude::*;
use openttdrs_core::{Command, VehicleOrder, apply_command, cycle_travel_ticks, cycle_wait_ticks};

use crate::render::RemapMapVisualsPending;
use crate::state::SimWorld;
use crate::ui::floating_window::{
    FloatingWindow, FloatingWindowClosed, FloatingWindowId, FloatingWindowTitleText, TITLE_CRIMSON,
    WINDOW_TEXT, spawn_floating_window, window_text_font,
};
use crate::ui::font::UiFontRole;
use crate::ui::hud::{HudBuildFeedback, push_build_command_error};
use crate::ui::toolbar::BuildMenuUi;

const TIMETABLE_ROWS: usize = 8;
const BTN_BG: Color = Color::srgb(0.36, 0.31, 0.21);
const BTN_BORDER: Color = Color::srgb(0.66, 0.58, 0.38);

#[derive(Resource, Default)]
pub(crate) struct TimetableWindowState {
    pub(crate) vehicle_id: Option<u32>,
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
    let (_root, content) = spawn_floating_window(
        &mut commands,
        asset_server,
        FloatingWindowId::Timetable,
        "Horario",
        TITLE_CRIMSON,
        Vec2::new(420.0, 220.0),
        420.0,
    );
    commands.entity(content).with_children(|panel| {
        panel.spawn((
            TimetableSummaryText,
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
                    spawn_timetable_row(list, asset_server, index);
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
                    TimetableWindowButton::ToggleTimetable,
                    "Horario ON/OFF",
                );
                spawn_tt_button(
                    row,
                    asset_server,
                    TimetableWindowButton::ToggleAutofill,
                    "Autofill",
                );
                spawn_tt_button(
                    row,
                    asset_server,
                    TimetableWindowButton::ClearLateness,
                    "Poner en hora",
                );
                spawn_tt_button(
                    row,
                    asset_server,
                    TimetableWindowButton::ToggleSeconds,
                    "Ticks/Seg",
                );
                spawn_tt_button(row, asset_server, TimetableWindowButton::Close, "Cerrar");
            });
    });
}

fn spawn_timetable_row(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    index: usize,
) {
    parent
        .spawn((
            TimetableOrderRowStrip { index },
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
                Text::new(""),
                window_text_font(asset_server, UiFontRole::Caption),
                TextColor(WINDOW_TEXT),
            ));
            spawn_row_btn(
                row,
                asset_server,
                index,
                TimetableRowButton::Wait,
                "Espera",
                72.0,
            );
            spawn_row_btn(
                row,
                asset_server,
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
    index: usize,
    kind: TimetableRowButton,
    label: &'static str,
    width: f32,
) {
    parent.spawn((
        Button,
        TimetableRowAction { index, kind },
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
    action: TimetableWindowButton,
    label: &str,
) {
    parent.spawn((
        Button,
        action,
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
        format!("{:.0}s", ticks as f32 / 5.0)
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

pub(crate) fn open_timetable_for_vehicle(state: &mut TimetableWindowState, vehicle_id: u32) {
    state.vehicle_id = Some(vehicle_id);
}

pub(crate) fn sync_timetable_window(
    tt_state: Res<TimetableWindowState>,
    sim: Res<SimWorld>,
    mut root_q: Query<(&FloatingWindow, &mut Visibility)>,
    mut title_q: Query<(&FloatingWindowTitleText, &mut Text)>,
    mut summary_q: Query<&mut Text, (With<TimetableSummaryText>, Without<FloatingWindowTitleText>)>,
    mut row_strip_q: Query<
        (&TimetableOrderRowStrip, &mut Node),
        (Without<Button>, Without<TimetableOrderRowLabel>),
    >,
    mut row_label_q: Query<
        (&TimetableOrderRowLabel, &mut Text),
        (
            Without<TimetableSummaryText>,
            Without<FloatingWindowTitleText>,
        ),
    >,
) {
    let Some((_, mut vis)) = root_q
        .iter_mut()
        .find(|(w, _)| w.id == FloatingWindowId::Timetable)
    else {
        return;
    };
    let Some(vehicle_id) = tt_state.vehicle_id else {
        *vis = Visibility::Hidden;
        return;
    };
    let Some(vehicle) = sim.state.vehicles.iter().find(|v| v.id == vehicle_id) else {
        *vis = Visibility::Hidden;
        return;
    };
    *vis = Visibility::Visible;
    if let Some((_, mut title)) = title_q
        .iter_mut()
        .find(|(t, _)| t.0 == FloatingWindowId::Timetable)
    {
        **title = format!("Horario — {}", vehicle.display_name());
    }
    if let Ok(mut summary) = summary_q.single_mut() {
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
            if vehicle.timetable_active {
                "ON"
            } else {
                "OFF"
            },
            if vehicle.timetable_autofill {
                "ON"
            } else {
                "OFF"
            },
        );
    }
    let seconds_mode = vehicle.timetable_display_seconds;
    for (strip, mut node) in &mut row_strip_q {
        node.display = if strip.index < vehicle.orders.len() {
            Display::Flex
        } else {
            Display::None
        };
    }
    for (label, mut text) in &mut row_label_q {
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

pub(crate) fn timetable_window_on_closed(
    mut closed: MessageReader<FloatingWindowClosed>,
    mut tt_state: ResMut<TimetableWindowState>,
) {
    for msg in closed.read() {
        if msg.0 == FloatingWindowId::Timetable {
            tt_state.vehicle_id = None;
        }
    }
}

pub(crate) fn handle_timetable_window_buttons(
    mut btn_q: Query<(&Interaction, &TimetableWindowButton), (Changed<Interaction>, With<Button>)>,
    mut row_btn_q: Query<
        (&Interaction, &TimetableRowAction),
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
    for (interaction, action) in &mut row_btn_q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let Some(vehicle_id) = tt_state.vehicle_id else {
            continue;
        };
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
        match apply_command(&mut sim.state, &cmd) {
            Ok(()) => pending.pending = true,
            Err(e) => push_build_command_error(&mut hud_feedback, e, time.elapsed_secs()),
        }
    }

    for (interaction, button) in &mut btn_q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let Some(vehicle_id) = tt_state.vehicle_id else {
            continue;
        };
        match button {
            TimetableWindowButton::ToggleTimetable => {
                let _ = apply_command(&mut sim.state, &Command::ToggleVehicleTimetable(vehicle_id));
                pending.pending = true;
            }
            TimetableWindowButton::ToggleAutofill => {
                let _ = apply_command(
                    &mut sim.state,
                    &Command::ToggleVehicleTimetableAutofill(vehicle_id),
                );
                pending.pending = true;
            }
            TimetableWindowButton::ClearLateness => {
                let _ = apply_command(
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
                tt_state.vehicle_id = None;
            }
        }
    }
}
