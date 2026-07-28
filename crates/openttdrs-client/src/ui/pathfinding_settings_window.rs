//! Ventana de ajustes PBS / pathfinding (`pf.wait_for_pbs_path`, etc.).

use bevy::prelude::*;
use openttdrs_core::Command;
use openttdrs_core::{PBS_WAIT_FOREVER, PathfindingSettings};

use crate::state::SimWorld;
use crate::ui::floating_window::{
    FloatingWindow, FloatingWindowClosed, FloatingWindowId, TITLE_BROWN, WINDOW_TEXT,
    spawn_floating_window, window_text_font,
};
use crate::ui::font::UiFontRole;
use crate::ui::toolbar::BuildMenuUi;
use crate::ui::window_lifecycle::{
    close_floating_window_on_message, sync_floating_window_visibility,
};

const BTN_BG: Color = Color::srgb(0.36, 0.31, 0.21);
const BTN_BORDER: Color = Color::srgb(0.66, 0.58, 0.38);
const BTN_ACTIVE: Color = Color::srgb(0.98, 0.92, 0.35);

#[derive(Resource, Default)]
pub(crate) struct PathfindingSettingsWindowState {
    pub(crate) open: bool,
}

#[derive(Component, Clone, Copy)]
pub(crate) enum PathfindingSettingsAction {
    WaitDays(u8),
    Backoff(u8),
    ToggleReverse,
    ToggleReservePaths,
    ResetDefaults,
}

const WAIT_PRESETS: [u8; 5] = [2, 10, 30, 60, PBS_WAIT_FOREVER];
const BACKOFF_PRESETS: [u8; 4] = [1, 20, 60, PBS_WAIT_FOREVER];

pub(crate) fn setup_pathfinding_settings_window(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
) {
    let asset_server = &*asset_server;
    let (_root, content) = spawn_floating_window(
        &mut commands,
        asset_server,
        FloatingWindowId::PathfindingSettings,
        "Señales PBS",
        TITLE_BROWN,
        Vec2::new(280.0, 190.0),
        400.0,
    );
    commands.entity(content).with_children(|body| {
        body.spawn((
            Text::new("Espera ante path sin reserva (días). 255 = nunca girar."),
            window_text_font(asset_server, UiFontRole::Caption),
            TextColor(Color::srgb(0.82, 0.78, 0.68)),
        ));
        spawn_preset_row(
            body,
            asset_server,
            "Espera",
            &WAIT_PRESETS,
            PathfindingSettingsAction::WaitDays,
            wait_label,
        );
        body.spawn((
            Text::new("Intervalo de look-ahead (ticks). 255 = desactivar."),
            window_text_font(asset_server, UiFontRole::Caption),
            TextColor(Color::srgb(0.82, 0.78, 0.68)),
            Node {
                margin: UiRect::top(Val::Px(8.0)),
                ..default()
            },
        ));
        spawn_preset_row(
            body,
            asset_server,
            "Backoff",
            &BACKOFF_PRESETS,
            PathfindingSettingsAction::Backoff,
            backoff_label,
        );
        body.spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(8.0),
                margin: UiRect::top(Val::Px(10.0)),
                ..default()
            },
            children![
                (
                    Button,
                    PathfindingSettingsAction::ToggleReverse,
                    Node {
                        min_width: Val::Px(160.0),
                        height: Val::Px(26.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        border: UiRect::all(Val::Px(1.0)),
                        padding: UiRect::horizontal(Val::Px(8.0)),
                        ..default()
                    },
                    BackgroundColor(BTN_BG),
                    BorderColor::all(BTN_BORDER),
                    Interaction::default(),
                    BuildMenuUi,
                    children![(
                        Text::new("Girar en señales"),
                        window_text_font(asset_server, UiFontRole::Caption),
                        TextColor(WINDOW_TEXT),
                    )],
                ),
                (
                    Button,
                    PathfindingSettingsAction::ToggleReservePaths,
                    Node {
                        min_width: Val::Px(160.0),
                        height: Val::Px(26.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        border: UiRect::all(Val::Px(1.0)),
                        padding: UiRect::horizontal(Val::Px(8.0)),
                        ..default()
                    },
                    BackgroundColor(BTN_BG),
                    BorderColor::all(BTN_BORDER),
                    Interaction::default(),
                    BuildMenuUi,
                    children![(
                        Text::new("Siempre reservar"),
                        window_text_font(asset_server, UiFontRole::Caption),
                        TextColor(WINDOW_TEXT),
                    )],
                ),
                (
                    Button,
                    PathfindingSettingsAction::ResetDefaults,
                    Node {
                        min_width: Val::Px(100.0),
                        height: Val::Px(26.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        border: UiRect::all(Val::Px(1.0)),
                        padding: UiRect::horizontal(Val::Px(8.0)),
                        ..default()
                    },
                    BackgroundColor(BTN_BG),
                    BorderColor::all(BTN_BORDER),
                    Interaction::default(),
                    BuildMenuUi,
                    children![(
                        Text::new("Por defecto"),
                        window_text_font(asset_server, UiFontRole::Caption),
                        TextColor(WINDOW_TEXT),
                    )],
                ),
            ],
        ));
    });
}

fn spawn_preset_row(
    body: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    label: &str,
    presets: &[u8],
    action: impl Fn(u8) -> PathfindingSettingsAction,
    value_label: fn(u8) -> String,
) {
    body.spawn((Node {
        width: Val::Percent(100.0),
        flex_direction: FlexDirection::Row,
        align_items: AlignItems::Center,
        column_gap: Val::Px(6.0),
        margin: UiRect::top(Val::Px(4.0)),
        flex_wrap: FlexWrap::Wrap,
        ..default()
    },))
        .with_children(|row| {
            row.spawn((
                Node {
                    width: Val::Px(64.0),
                    ..default()
                },
                children![(
                    Text::new(label),
                    window_text_font(asset_server, UiFontRole::Caption),
                    TextColor(WINDOW_TEXT),
                )],
            ));
            for &value in presets {
                row.spawn((
                    Button,
                    action(value),
                    Node {
                        min_width: Val::Px(48.0),
                        height: Val::Px(24.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        border: UiRect::all(Val::Px(1.0)),
                        padding: UiRect::horizontal(Val::Px(4.0)),
                        ..default()
                    },
                    BackgroundColor(BTN_BG),
                    BorderColor::all(BTN_BORDER),
                    Interaction::default(),
                    BuildMenuUi,
                    children![(
                        Text::new(value_label(value)),
                        window_text_font(asset_server, UiFontRole::Caption),
                        TextColor(WINDOW_TEXT),
                    )],
                ));
            }
        });
}

fn wait_label(days: u8) -> String {
    if days == PBS_WAIT_FOREVER {
        "∞".into()
    } else {
        days.to_string()
    }
}

fn backoff_label(ticks: u8) -> String {
    if ticks == PBS_WAIT_FOREVER {
        "off".into()
    } else {
        ticks.to_string()
    }
}

pub(crate) fn sync_pathfinding_settings_window(
    state: Res<PathfindingSettingsWindowState>,
    sim: Res<SimWorld>,
    mut root_q: Query<(&FloatingWindow, &mut Visibility)>,
    mut buttons: Query<(&PathfindingSettingsAction, &mut BorderColor), Without<FloatingWindow>>,
) {
    sync_floating_window_visibility(
        &mut root_q,
        FloatingWindowId::PathfindingSettings,
        state.open,
    );
    if !state.open {
        return;
    }

    let pf = sim.state.pathfinding;
    for (action, mut border) in &mut buttons {
        let active = match *action {
            PathfindingSettingsAction::WaitDays(d) => pf.wait_for_pbs_path == d,
            PathfindingSettingsAction::Backoff(b) => pf.path_backoff_interval == b,
            PathfindingSettingsAction::ToggleReverse => pf.reverse_at_signals,
            PathfindingSettingsAction::ToggleReservePaths => pf.reserve_paths,
            PathfindingSettingsAction::ResetDefaults => false,
        };
        *border = if active {
            BorderColor::all(BTN_ACTIVE)
        } else {
            BorderColor::all(BTN_BORDER)
        };
    }
}

pub(crate) fn handle_pathfinding_settings_buttons(
    mut sim: ResMut<SimWorld>,
    buttons: Query<
        (&Interaction, &PathfindingSettingsAction),
        (Changed<Interaction>, With<Button>),
    >,
) {
    for (interaction, action) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let mut next = sim.state.pathfinding;
        match *action {
            PathfindingSettingsAction::WaitDays(d) => {
                next.wait_for_pbs_path = d.max(2);
            }
            PathfindingSettingsAction::Backoff(b) => {
                next.path_backoff_interval = b.max(1);
            }
            PathfindingSettingsAction::ToggleReverse => {
                next.reverse_at_signals = !next.reverse_at_signals;
            }
            PathfindingSettingsAction::ToggleReservePaths => {
                next.reserve_paths = !next.reserve_paths;
            }
            PathfindingSettingsAction::ResetDefaults => {
                next = PathfindingSettings::default();
            }
        }
        let _ = crate::network::apply_player_command(
            &mut sim.state,
            &Command::SetPathfindingSettings(next),
        );
    }
}

pub(crate) fn pathfinding_settings_on_closed(
    mut closed: MessageReader<FloatingWindowClosed>,
    mut state: ResMut<PathfindingSettingsWindowState>,
) {
    close_floating_window_on_message(&mut closed, FloatingWindowId::PathfindingSettings, || {
        state.open = false;
    });
}
