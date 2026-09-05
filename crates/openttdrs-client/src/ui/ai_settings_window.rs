//! Ventana de ajustes / debug de IA rival (UI-8 / #44).

use bevy::prelude::*;
use openttdrs_core::Command;
use openttdrs_core::{
    AiSettings, DEFAULT_AI_BUILD_MONEY_THRESHOLD, DEFAULT_AI_MAX_ROUTES, format_ai_debug_status,
    format_money,
};

use crate::i18n::Locale;
use crate::settings::ClientPreferences;
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

/// Localiza únicamente el chrome del resumen de IA que el core expone como
/// texto de diagnóstico. Los nombres, importes, cargos, rutas y coordenadas
/// quedan como datos literales de la partida.
fn localized_ai_debug_status(locale: Locale, status: &str) -> String {
    if locale == Locale::Es {
        return status.to_owned();
    }

    status
        .lines()
        .map(localize_ai_debug_line_en)
        .collect::<Vec<_>>()
        .join("\n")
}

fn localize_ai_debug_line_en(line: &str) -> String {
    if let Some(settings) = line.strip_prefix("IA: ") {
        let mut parts = settings.split(" · ");
        if let (Some(enabled), Some(threshold), Some(max_routes), None) =
            (parts.next(), parts.next(), parts.next(), parts.next())
            && let (Some(threshold), Some(max_routes)) = (
                threshold.strip_prefix("umbral "),
                max_routes.strip_prefix("máx. rutas rail "),
            )
        {
            return format!(
                "AI: {enabled} · cash threshold {threshold} · max. rail routes {max_routes}"
            );
        }
    }

    if let Some(routes) = line.strip_prefix("  Rutas / trenes: ") {
        return format!("  Routes / trains: {routes}");
    }
    if let Some(routes) = line.strip_prefix("  Rutas / buses: ") {
        return format!("  Routes / buses: {routes}");
    }
    if let Some(vehicle) = line.strip_prefix("  #") {
        let mut parts = vehicle.splitn(4, " · ");
        if let (Some(id), Some(cargo), Some(state), Some(route)) =
            (parts.next(), parts.next(), parts.next(), parts.next())
        {
            let state = match state {
                "marcha" => "running",
                "parado" => "stopped",
                value => value,
            };
            let route = if route == "sin órdenes" {
                "no orders"
            } else {
                route
            };
            return format!("  #{id} · {cargo} · {state} · {route}");
        }
    }
    if let Some((company, details)) = line.rsplit_once(" · color ")
        && let Some((colour, money)) = details.split_once(" · ")
    {
        return format!("{company} · colour {colour} · {money}");
    }

    match line {
        "Sin compañía IA en la partida." => "No AI company in the game.".into(),
        "  (sin trenes)" => "  (no trains)".into(),
        "  (sin buses)" => "  (no buses)".into(),
        _ => line.to_owned(),
    }
}

pub(crate) fn sync_ai_settings_window(
    state: Res<AiSettingsWindowState>,
    sim: Res<SimWorld>,
    prefs: Res<ClientPreferences>,
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
        *text = Text::new(localized_ai_debug_status(
            prefs.locale(),
            &format_ai_debug_status(&sim.state),
        ));
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
        let mut next = sim.state.ai;
        match *action {
            AiSettingsAction::ToggleEnabled => {
                next.enabled = !next.enabled;
            }
            AiSettingsAction::MoneyThreshold(v) => {
                next.build_money_threshold = v;
            }
            AiSettingsAction::MaxRoutes(v) => {
                next.max_routes = v;
            }
            AiSettingsAction::ResetDefaults => {
                next = AiSettings {
                    enabled: true,
                    build_money_threshold: DEFAULT_AI_BUILD_MONEY_THRESHOLD,
                    max_routes: DEFAULT_AI_MAX_ROUTES,
                };
            }
        }
        let _ = crate::network::apply_player_command(&mut sim.state, &Command::SetAiSettings(next));
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

#[cfg(test)]
mod tests {
    use super::localized_ai_debug_status;
    use crate::i18n::Locale;

    #[test]
    fn ai_debug_status_localizes_chrome_without_touching_game_data() {
        let status = concat!(
            "IA: ON · umbral $80.0K · máx. rutas rail 2\n",
            "TransCargo · color 3 · $120.0K\n",
            "  Rutas / trenes: 1 / 2\n",
            "  #7 · Goods · marcha · (1,2) → (3,4)\n",
            "RoadHaul · color 5 · $90.0K\n",
            "  Rutas / buses: 0 / 3\n",
            "  (sin buses)"
        );
        assert_eq!(localized_ai_debug_status(Locale::Es, status), status);
        assert_eq!(
            localized_ai_debug_status(Locale::En, status),
            concat!(
                "AI: ON · cash threshold $80.0K · max. rail routes 2\n",
                "TransCargo · colour 3 · $120.0K\n",
                "  Routes / trains: 1 / 2\n",
                "  #7 · Goods · running · (1,2) → (3,4)\n",
                "RoadHaul · colour 5 · $90.0K\n",
                "  Routes / buses: 0 / 3\n",
                "  (no buses)"
            )
        );
        assert_eq!(
            localized_ai_debug_status(Locale::En, "Sin compañía IA en la partida."),
            "No AI company in the game."
        );
    }
}
