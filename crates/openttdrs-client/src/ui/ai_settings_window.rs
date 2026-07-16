//! Ventana de ajustes / debug de IA rival (UI-8 / #44).

use bevy::prelude::*;
use openttdrs_core::{
    AiSettings, DEFAULT_AI_BUILD_MONEY_THRESHOLD, DEFAULT_AI_MAX_ROUTES, format_ai_debug_status,
    format_money,
};

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

const MONEY_PRESETS: [i64; 4] = [50_000, 80_000, 150_000, 250_000];
const ROUTE_PRESETS: [u8; 4] = [1, 2, 3, 4];

#[derive(Resource, Default)]
pub(crate) struct AiSettingsWindowState {
    pub(crate) open: bool,
}

#[derive(Component, Clone, Copy)]
pub(crate) enum AiSettingsAction {
    ToggleEnabled,
    MoneyThreshold(i64),
    MaxRoutes(u8),
    ResetDefaults,
}

#[derive(Component)]
pub(crate) struct AiSettingsDebugText;

pub(crate) fn setup_ai_settings_window(mut commands: Commands, asset_server: Res<AssetServer>) {
    let asset_server = &*asset_server;
    let (_root, content) = spawn_floating_window(
        &mut commands,
        asset_server,
        FloatingWindowId::AiSettings,
        "IA / TransCargo",
        TITLE_BROWN,
        Vec2::new(300.0, 120.0),
        420.0,
    );
    commands.entity(content).with_children(|body| {
        body.spawn((
            Text::new("Ajustes del rival TransCargo (construcción mensual)."),
            window_text_font(asset_server, UiFontRole::Caption),
            TextColor(Color::srgb(0.82, 0.78, 0.68)),
        ));
        body.spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(8.0),
                margin: UiRect::top(Val::Px(8.0)),
                flex_wrap: FlexWrap::Wrap,
                ..default()
            },
            children![
                (
                    Button,
                    AiSettingsAction::ToggleEnabled,
                    Node {
                        min_width: Val::Px(120.0),
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
                        Text::new("IA activa"),
                        window_text_font(asset_server, UiFontRole::Caption),
                        TextColor(WINDOW_TEXT),
                    )],
                ),
                (
                    Button,
                    AiSettingsAction::ResetDefaults,
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
        body.spawn((
            Text::new("Umbral de dinero para nueva ruta"),
            window_text_font(asset_server, UiFontRole::Caption),
            TextColor(Color::srgb(0.82, 0.78, 0.68)),
            Node {
                margin: UiRect::top(Val::Px(10.0)),
                ..default()
            },
        ));
        spawn_money_row(body, asset_server);
        body.spawn((
            Text::new("Máximo de rutas (trenes)"),
            window_text_font(asset_server, UiFontRole::Caption),
            TextColor(Color::srgb(0.82, 0.78, 0.68)),
            Node {
                margin: UiRect::top(Val::Px(8.0)),
                ..default()
            },
        ));
        spawn_routes_row(body, asset_server);
        body.spawn((
            AiSettingsDebugText,
            Text::new(""),
            window_text_font(asset_server, UiFontRole::Caption),
            TextColor(WINDOW_TEXT),
            Node {
                margin: UiRect::top(Val::Px(12.0)),
                ..default()
            },
        ));
    });
}

fn spawn_money_row(body: &mut ChildSpawnerCommands, asset_server: &AssetServer) {
    body.spawn((Node {
        width: Val::Percent(100.0),
        flex_direction: FlexDirection::Row,
        column_gap: Val::Px(6.0),
        margin: UiRect::top(Val::Px(4.0)),
        flex_wrap: FlexWrap::Wrap,
        ..default()
    },))
        .with_children(|row| {
            for &value in &MONEY_PRESETS {
                row.spawn((
                    Button,
                    AiSettingsAction::MoneyThreshold(value),
                    Node {
                        min_width: Val::Px(64.0),
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
                        Text::new(short_money(value)),
                        window_text_font(asset_server, UiFontRole::Caption),
                        TextColor(WINDOW_TEXT),
                    )],
                ));
            }
        });
}

fn spawn_routes_row(body: &mut ChildSpawnerCommands, asset_server: &AssetServer) {
    body.spawn((Node {
        width: Val::Percent(100.0),
        flex_direction: FlexDirection::Row,
        column_gap: Val::Px(6.0),
        margin: UiRect::top(Val::Px(4.0)),
        flex_wrap: FlexWrap::Wrap,
        ..default()
    },))
        .with_children(|row| {
            for &value in &ROUTE_PRESETS {
                row.spawn((
                    Button,
                    AiSettingsAction::MaxRoutes(value),
                    Node {
                        min_width: Val::Px(36.0),
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
                        Text::new(value.to_string()),
                        window_text_font(asset_server, UiFontRole::Caption),
                        TextColor(WINDOW_TEXT),
                    )],
                ));
            }
        });
}

fn short_money(amount: i64) -> String {
    if amount >= 1_000 {
        format!("{}k", amount / 1_000)
    } else {
        format_money(amount)
    }
}

pub(crate) fn sync_ai_settings_window(
    state: Res<AiSettingsWindowState>,
    sim: Res<SimWorld>,
    mut root_q: Query<(&FloatingWindow, &mut Visibility)>,
    mut buttons: Query<(&AiSettingsAction, &mut BorderColor), Without<FloatingWindow>>,
    mut debug_q: Query<&mut Text, With<AiSettingsDebugText>>,
) {
    sync_floating_window_visibility(&mut root_q, FloatingWindowId::AiSettings, state.open);
    if !state.open {
        return;
    }

    let ai = sim.state.ai.clamped();
    for (action, mut border) in &mut buttons {
        let active = match *action {
            AiSettingsAction::ToggleEnabled => ai.enabled,
            AiSettingsAction::MoneyThreshold(v) => ai.build_money_threshold == v,
            AiSettingsAction::MaxRoutes(v) => ai.max_routes == v,
            AiSettingsAction::ResetDefaults => false,
        };
        *border = if active {
            BorderColor::all(BTN_ACTIVE)
        } else {
            BorderColor::all(BTN_BORDER)
        };
    }
    if let Ok(mut text) = debug_q.single_mut() {
        *text = Text::new(format_ai_debug_status(&sim.state));
    }
}

pub(crate) fn handle_ai_settings_buttons(
    mut sim: ResMut<SimWorld>,
    buttons: Query<(&Interaction, &AiSettingsAction), (Changed<Interaction>, With<Button>)>,
) {
    for (interaction, action) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match *action {
            AiSettingsAction::ToggleEnabled => {
                sim.state.ai.enabled = !sim.state.ai.enabled;
            }
            AiSettingsAction::MoneyThreshold(v) => {
                sim.state.ai.build_money_threshold = v;
                sim.state.ai = sim.state.ai.clamped();
            }
            AiSettingsAction::MaxRoutes(v) => {
                sim.state.ai.max_routes = v;
                sim.state.ai = sim.state.ai.clamped();
            }
            AiSettingsAction::ResetDefaults => {
                sim.state.ai = AiSettings {
                    enabled: true,
                    build_money_threshold: DEFAULT_AI_BUILD_MONEY_THRESHOLD,
                    max_routes: DEFAULT_AI_MAX_ROUTES,
                };
            }
        }
    }
}

pub(crate) fn ai_settings_on_closed(
    mut closed: MessageReader<FloatingWindowClosed>,
    mut state: ResMut<AiSettingsWindowState>,
) {
    close_floating_window_on_message(&mut closed, FloatingWindowId::AiSettings, || {
        state.open = false;
    });
}
