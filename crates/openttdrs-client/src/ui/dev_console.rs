//! Consola y herramientas de desarrollo (UI-8): FPS, toggles y comandos cortos.

use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;

use crate::render::MapVisualLayer;
use crate::settings::ClientPreferences;
use crate::state::SimWorld;
use crate::ui::cheat_window::CheatWindowState;
use crate::ui::command_error_text::command_error_message;
use crate::ui::endscreen::{RetireGameRequested, request_retire_game};
use crate::ui::floating_window::{
    FloatingWindow, FloatingWindowClosed, FloatingWindowId, TITLE_BROWN, WINDOW_TEXT,
    spawn_floating_window, window_text_font,
};
use crate::ui::font::UiFontRole;
use crate::ui::hud::SimHudControls;
use crate::ui::newgrf_window::NewGrfWindowState;
use crate::ui::tile_inspector_window::TileInspectorWindowState;
use crate::ui::toolbar::BuildMenuUi;

const BTN_BG: Color = Color::srgb(0.36, 0.31, 0.21);
const BTN_BORDER: Color = Color::srgb(0.66, 0.58, 0.38);
const BTN_ON: Color = Color::srgb(0.28, 0.42, 0.28);
const LOG_CAP: usize = 24;
const INPUT_CAP: usize = 96;

#[derive(Resource, Default)]
pub(crate) struct DevConsoleState {
    pub(crate) open: bool,
    pub(crate) input: String,
    pub(crate) log: Vec<String>,
}

#[derive(Component)]
pub(crate) struct DevConsoleMetricsText;

#[derive(Component)]
pub(crate) struct DevConsoleLogText;

#[derive(Component)]
pub(crate) struct DevConsoleInputText;

#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DevConsoleAction {
    ToggleOverlay,
    ToggleGizmos,
    OpenTileInspect,
    OpenNewGrf,
}

pub(crate) fn setup_dev_console(mut commands: Commands, asset_server: Res<AssetServer>) {
    let asset_server = &*asset_server;
    let (_root, content) = spawn_floating_window(
        &mut commands,
        asset_server,
        FloatingWindowId::DevConsole,
        "Consola / Dev",
        TITLE_BROWN,
        Vec2::new(420.0, 72.0),
        460.0,
    );
    commands.entity(content).with_children(|body| {
        body.spawn((
            DevConsoleMetricsText,
            Text::new("FPS —"),
            window_text_font(asset_server, UiFontRole::Caption),
            TextColor(Color::srgb(0.85, 1.0, 0.85)),
            Node {
                width: Val::Percent(100.0),
                margin: UiRect::bottom(Val::Px(6.0)),
                ..default()
            },
            BuildMenuUi,
        ));
        body.spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(4.0),
            margin: UiRect::bottom(Val::Px(6.0)),
            ..default()
        })
        .with_children(|row| {
            spawn_action_btn(
                row,
                asset_server,
                DevConsoleAction::ToggleOverlay,
                "Overlay",
            );
            spawn_action_btn(row, asset_server, DevConsoleAction::ToggleGizmos, "Gizmos");
            spawn_action_btn(
                row,
                asset_server,
                DevConsoleAction::OpenTileInspect,
                "Tile…",
            );
            spawn_action_btn(row, asset_server, DevConsoleAction::OpenNewGrf, "NewGRF");
        });
        body.spawn((
            DevConsoleLogText,
            Text::new("Escribe help y Enter. F3 / ` abre o cierra."),
            window_text_font(asset_server, UiFontRole::Caption),
            TextColor(WINDOW_TEXT),
            Node {
                width: Val::Percent(100.0),
                min_height: Val::Px(140.0),
                margin: UiRect::bottom(Val::Px(4.0)),
                ..default()
            },
            BuildMenuUi,
        ));
        body.spawn((
            DevConsoleInputText,
            Text::new("> "),
            window_text_font(asset_server, UiFontRole::Caption),
            TextColor(Color::srgb(0.98, 0.92, 0.55)),
            Node {
                width: Val::Percent(100.0),
                ..default()
            },
            BuildMenuUi,
        ));
    });
}

fn spawn_action_btn(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    action: DevConsoleAction,
    label: &'static str,
) {
    parent
        .spawn((
            Button,
            action,
            Node {
                flex_grow: 1.0,
                height: Val::Px(24.0),
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
        .with_children(|btn| {
            btn.spawn((
                Text::new(label),
                window_text_font(asset_server, UiFontRole::Caption),
                TextColor(WINDOW_TEXT),
            ));
        });
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn sync_dev_console(
    state: Res<DevConsoleState>,
    prefs: Res<ClientPreferences>,
    hud: Res<SimHudControls>,
    sim: Option<Res<SimWorld>>,
    diagnostics: Res<DiagnosticsStore>,
    map_q: Query<(), With<MapVisualLayer>>,
    mut windows: Query<(&FloatingWindow, &mut Visibility)>,
    mut metrics_q: Query<&mut Text, With<DevConsoleMetricsText>>,
    mut log_q: Query<&mut Text, (With<DevConsoleLogText>, Without<DevConsoleMetricsText>)>,
    mut input_q: Query<
        &mut Text,
        (
            With<DevConsoleInputText>,
            Without<DevConsoleMetricsText>,
            Without<DevConsoleLogText>,
        ),
    >,
    mut overlay_btn: Query<
        (&DevConsoleAction, &mut BackgroundColor),
        (With<Button>, With<DevConsoleAction>),
    >,
) {
    for (w, mut vis) in &mut windows {
        if w.id == FloatingWindowId::DevConsole {
            *vis = if state.open {
                Visibility::Visible
            } else {
                Visibility::Hidden
            };
        }
    }
    if !state.open {
        return;
    }
    let line = format_dev_metrics(&diagnostics, sim.as_deref(), &hud, map_q.iter().count());
    for mut text in &mut metrics_q {
        **text = line.clone();
    }
    let log_body = if state.log.is_empty() {
        "Escribe help y Enter. F3 / ` abre o cierra.".into()
    } else {
        state.log.join("\n")
    };
    for mut text in &mut log_q {
        **text = log_body.clone();
    }
    for mut text in &mut input_q {
        **text = format!("> {}", state.input);
    }
    for (action, mut bg) in &mut overlay_btn {
        *bg = BackgroundColor(match *action {
            DevConsoleAction::ToggleOverlay if prefs.show_diagnostics_overlay => BTN_ON,
            DevConsoleAction::ToggleGizmos if prefs.show_debug_gizmos => BTN_ON,
            _ => BTN_BG,
        });
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn handle_dev_console_buttons(
    buttons: Query<
        (&Interaction, &DevConsoleAction),
        (Changed<Interaction>, With<Button>, With<DevConsoleAction>),
    >,
    mut state: ResMut<DevConsoleState>,
    mut prefs: ResMut<ClientPreferences>,
    mut tile_inspector: ResMut<TileInspectorWindowState>,
    mut newgrf: ResMut<NewGrfWindowState>,
) {
    if !state.open {
        return;
    }
    for (interaction, action) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match *action {
            DevConsoleAction::ToggleOverlay => {
                prefs.show_diagnostics_overlay = !prefs.show_diagnostics_overlay;
                push_log(
                    &mut state,
                    format!(
                        "overlay {}",
                        if prefs.show_diagnostics_overlay {
                            "on"
                        } else {
                            "off"
                        }
                    ),
                );
            }
            DevConsoleAction::ToggleGizmos => {
                prefs.show_debug_gizmos = !prefs.show_debug_gizmos;
                push_log(
                    &mut state,
                    format!(
                        "gizmos {}",
                        if prefs.show_debug_gizmos { "on" } else { "off" }
                    ),
                );
            }
            DevConsoleAction::OpenTileInspect => {
                tile_inspector.open = true;
                push_log(&mut state, "tile inspector abierto".into());
            }
            DevConsoleAction::OpenNewGrf => {
                newgrf.open = true;
                push_log(&mut state, "NewGRF abierto".into());
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn handle_dev_console_keyboard(
    mut events: MessageReader<KeyboardInput>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<DevConsoleState>,
    mut prefs: ResMut<ClientPreferences>,
    mut tile_inspector: ResMut<TileInspectorWindowState>,
    mut newgrf: ResMut<NewGrfWindowState>,
    mut cheat_window: ResMut<CheatWindowState>,
    mut retire: ResMut<RetireGameRequested>,
    hud: Res<SimHudControls>,
    mut sim: Option<ResMut<SimWorld>>,
    diagnostics: Res<DiagnosticsStore>,
    map_q: Query<(), With<MapVisualLayer>>,
) {
    if keyboard.just_pressed(KeyCode::F3) || keyboard.just_pressed(KeyCode::Backquote) {
        state.open = !state.open;
        if state.open {
            push_log(&mut state, "consola abierta".into());
        }
        return;
    }
    if !state.open {
        return;
    }
    for ev in events.read() {
        if ev.state != ButtonState::Pressed {
            continue;
        }
        match &ev.logical_key {
            Key::Enter => {
                let cmd = state.input.trim().to_string();
                state.input.clear();
                if !cmd.is_empty() {
                    run_dev_command(
                        &mut state,
                        &mut prefs,
                        &mut tile_inspector,
                        &mut newgrf,
                        &mut cheat_window,
                        &mut retire,
                        &hud,
                        sim.as_deref_mut(),
                        &diagnostics,
                        map_q.iter().count(),
                        &cmd,
                    );
                }
            }
            Key::Backspace => {
                state.input.pop();
            }
            Key::Escape => {
                state.open = false;
            }
            Key::Character(c) => {
                if c.as_str() == "`" {
                    continue;
                }
                if state.input.chars().count() < INPUT_CAP {
                    state.input.push_str(c);
                }
            }
            _ => {}
        }
    }
}

pub(crate) fn dev_console_window_on_closed(
    mut closed: MessageReader<FloatingWindowClosed>,
    mut state: ResMut<DevConsoleState>,
) {
    for msg in closed.read() {
        if msg.0 == FloatingWindowId::DevConsole {
            state.open = false;
            state.input.clear();
        }
    }
}

/// True si la consola captura el teclado (hotkeys de juego deben ceder).
#[must_use]
pub(crate) fn dev_console_captures_keyboard(state: &DevConsoleState) -> bool {
    state.open
}

fn push_log(state: &mut DevConsoleState, line: String) {
    state.log.push(line);
    if state.log.len() > LOG_CAP {
        let drop_n = state.log.len() - LOG_CAP;
        state.log.drain(0..drop_n);
    }
}

fn format_dev_metrics(
    diagnostics: &DiagnosticsStore,
    sim: Option<&SimWorld>,
    hud: &SimHudControls,
    visual_entities: usize,
) -> String {
    let fps = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FPS)
        .and_then(bevy::diagnostic::Diagnostic::smoothed)
        .map(|f| format!("{f:.0}"))
        .unwrap_or_else(|| "—".into());
    let frame_ms = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FRAME_TIME)
        .and_then(bevy::diagnostic::Diagnostic::smoothed)
        .map(|f| format!("{:.1} ms", f * 1000.0))
        .unwrap_or_else(|| "—".into());
    let (tick, veh, stations) = sim
        .map(|s| {
            (
                s.state.tick.get(),
                s.state.vehicles.len(),
                s.state.stations.len(),
            )
        })
        .unwrap_or((0, 0, 0));
    format!(
        "FPS {fps} ({frame_ms}) | tick {tick} | speed {:.2}x | veh {veh} | est {stations} | visuales {visual_entities}",
        hud.sim_speed
    )
}

#[allow(clippy::too_many_arguments)]
fn run_dev_command(
    state: &mut DevConsoleState,
    prefs: &mut ClientPreferences,
    tile_inspector: &mut TileInspectorWindowState,
    newgrf: &mut NewGrfWindowState,
    cheat_window: &mut CheatWindowState,
    retire: &mut RetireGameRequested,
    hud: &SimHudControls,
    sim: Option<&mut SimWorld>,
    diagnostics: &DiagnosticsStore,
    visual_entities: usize,
    cmd: &str,
) {
    push_log(state, format!("> {cmd}"));
    let mut parts = cmd.split_whitespace();
    let Some(head) = parts.next() else {
        return;
    };
    match head.to_ascii_lowercase().as_str() {
        "help" | "?" | "list" | "cmds" => {
            push_log(
                state,
                "cmds: help|list | fps | overlay | gizmos | tile | newgrf | scenario | cheat | cheats | endgame | clear"
                    .into(),
            );
            push_log(
                state,
                "cheat: on|off | status | money [n] | infinite | bulldozer | year <n> | company <id>"
                    .into(),
            );
        }
        "cheats" | "cheatgui" => {
            cheat_window.open = true;
            push_log(state, "ventana Cheats abierta".into());
        }
        "cheat" => apply_cheat_command(state, sim, &mut parts),
        "scenario" | "junction" => match parts.next().unwrap_or("list") {
            "list" | "ls" => {
                push_log(
                    state,
                    format!(
                        "escenarios: {}",
                        openttdrs_core::parity::scenario_names().join(", ")
                    ),
                );
            }
            "export" => {
                let Some(name) = parts.next() else {
                    push_log(state, "uso: scenario export <nombre> [ruta.json]".into());
                    return;
                };
                let path = parts
                    .next()
                    .map(std::path::PathBuf::from)
                    .unwrap_or_else(|| {
                        std::path::PathBuf::from(format!("save/scenarios/{name}.json"))
                    });
                match openttdrs_core::parity::export_junction_json(name, &path) {
                    Ok(()) => push_log(
                        state,
                        format!(
                            "exportado {} → {} (OTTDJSON_LOAD={})",
                            name,
                            path.display(),
                            path.display()
                        ),
                    ),
                    Err(e) => push_log(state, format!("export falló: {e}")),
                }
            }
            other => push_log(
                state,
                format!("scenario: desconocido '{other}' (list|export)"),
            ),
        },
        "fps" | "stats" => {
            push_log(
                state,
                format_dev_metrics(diagnostics, sim.as_deref(), hud, visual_entities),
            );
        }
        "overlay" => {
            match parts.next().unwrap_or("toggle") {
                "on" | "1" | "true" => prefs.show_diagnostics_overlay = true,
                "off" | "0" | "false" => prefs.show_diagnostics_overlay = false,
                _ => prefs.show_diagnostics_overlay = !prefs.show_diagnostics_overlay,
            }
            push_log(
                state,
                format!(
                    "overlay={}",
                    if prefs.show_diagnostics_overlay {
                        "on"
                    } else {
                        "off"
                    }
                ),
            );
        }
        "gizmos" => {
            match parts.next().unwrap_or("toggle") {
                "on" | "1" | "true" => prefs.show_debug_gizmos = true,
                "off" | "0" | "false" => prefs.show_debug_gizmos = false,
                _ => prefs.show_debug_gizmos = !prefs.show_debug_gizmos,
            }
            push_log(
                state,
                format!(
                    "gizmos={}",
                    if prefs.show_debug_gizmos { "on" } else { "off" }
                ),
            );
        }
        "tile" | "inspect" => {
            tile_inspector.open = true;
            push_log(state, "tile inspector abierto".into());
        }
        "newgrf" => {
            newgrf.open = true;
            push_log(state, "NewGRF abierto".into());
        }
        "endgame" | "retire" => {
            request_retire_game(retire);
            push_log(state, "retiro solicitado".into());
        }
        "clear" | "cls" => {
            state.log.clear();
        }
        other => push_log(state, format!("desconocido: {other} (help)")),
    }
}

const DEFAULT_CHEAT_MONEY: i64 = 1_000_000;

/// Resultado de parsear `cheat …` (status no emite `Command`).
enum ParsedCheat {
    Status,
    Cmd(openttdrs_core::Command),
}

/// Parsea subcomandos `cheat on|off|status|money|infinite|bulldozer|year|company`.
fn parse_cheat_command<'a>(
    parts: &mut impl Iterator<Item = &'a str>,
) -> Result<ParsedCheat, &'static str> {
    match parts.next().unwrap_or("").to_ascii_lowercase().as_str() {
        "" | "status" | "info" => Ok(ParsedCheat::Status),
        "on" | "1" | "true" => Ok(ParsedCheat::Cmd(openttdrs_core::Command::CheatSetEnabled(
            true,
        ))),
        "off" | "0" | "false" => Ok(ParsedCheat::Cmd(openttdrs_core::Command::CheatSetEnabled(
            false,
        ))),
        "money" => {
            let amount = parts
                .next()
                .and_then(|s| s.parse::<i64>().ok())
                .unwrap_or(DEFAULT_CHEAT_MONEY);
            Ok(ParsedCheat::Cmd(openttdrs_core::Command::CheatAddMoney(
                amount,
            )))
        }
        "infinite" => Ok(ParsedCheat::Cmd(
            openttdrs_core::Command::CheatToggleInfiniteMoney,
        )),
        "bulldozer" | "magic" => Ok(ParsedCheat::Cmd(
            openttdrs_core::Command::CheatToggleMagicBulldozer,
        )),
        "year" | "date" => {
            let Some(raw) = parts.next() else {
                return Err("uso: cheat year <n>");
            };
            let Ok(year) = raw.parse::<u32>() else {
                return Err("uso: cheat year <n>");
            };
            Ok(ParsedCheat::Cmd(openttdrs_core::Command::CheatSetYear(
                year,
            )))
        }
        "company" | "cia" => {
            let Some(raw) = parts.next() else {
                return Err("uso: cheat company <id>");
            };
            let Ok(id) = raw.parse::<u8>() else {
                return Err("uso: cheat company <id>");
            };
            Ok(ParsedCheat::Cmd(
                openttdrs_core::Command::CheatSwitchCompany(openttdrs_core::CompanyId(id)),
            ))
        }
        _ => Err("cheat: desconocido (on|off|status|money|infinite|bulldozer|year|company)"),
    }
}

fn format_cheat_log(sim: &SimWorld) -> String {
    let c = &sim.state.cheats;
    let (year, _) =
        openttdrs_core::calendar_year_day(openttdrs_core::calendar_day_index(sim.state.tick));
    format!(
        "cheats enabled={} infinite={} bulldozer={} money={} year={} company={}",
        c.enabled,
        c.infinite_money,
        c.magic_bulldozer,
        sim.state.economy.money,
        year,
        sim.state.active_company.0
    )
}

fn apply_cheat_command<'a>(
    state: &mut DevConsoleState,
    sim: Option<&mut SimWorld>,
    parts: &mut impl Iterator<Item = &'a str>,
) {
    let Some(sim) = sim else {
        push_log(state, "cheat: sin SimWorld".into());
        return;
    };
    match parse_cheat_command(parts) {
        Ok(ParsedCheat::Status) => push_log(state, format_cheat_log(sim)),
        Ok(ParsedCheat::Cmd(cmd)) => match openttdrs_core::apply_command(&mut sim.state, &cmd) {
            Ok(()) => push_log(state, format_cheat_log(sim)),
            Err(e) => push_log(state, format!("cheat falló: {}", command_error_message(e))),
        },
        Err(msg) => push_log(state, msg.into()),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn run_help_and_clear_commands() {
        let mut state = DevConsoleState::default();
        let mut prefs = ClientPreferences::default();
        let mut tile = TileInspectorWindowState::default();
        let mut newgrf = NewGrfWindowState::default();
        let mut cheats = CheatWindowState::default();
        let mut retire = RetireGameRequested::default();
        let hud = SimHudControls::default();
        let diagnostics = DiagnosticsStore::default();
        run_dev_command(
            &mut state,
            &mut prefs,
            &mut tile,
            &mut newgrf,
            &mut cheats,
            &mut retire,
            &hud,
            None,
            &diagnostics,
            0,
            "help",
        );
        assert!(!state.log.is_empty());
        run_dev_command(
            &mut state,
            &mut prefs,
            &mut tile,
            &mut newgrf,
            &mut cheats,
            &mut retire,
            &hud,
            None,
            &diagnostics,
            0,
            "overlay on",
        );
        assert!(prefs.show_diagnostics_overlay);
        run_dev_command(
            &mut state,
            &mut prefs,
            &mut tile,
            &mut newgrf,
            &mut cheats,
            &mut retire,
            &hud,
            None,
            &diagnostics,
            0,
            "cheats",
        );
        assert!(cheats.open);
        run_dev_command(
            &mut state,
            &mut prefs,
            &mut tile,
            &mut newgrf,
            &mut cheats,
            &mut retire,
            &hud,
            None,
            &diagnostics,
            0,
            "clear",
        );
        assert!(state.log.is_empty());
    }

    #[test]
    fn parse_cheat_subcommands() {
        let mut on = "on".split_whitespace();
        assert!(matches!(
            parse_cheat_command(&mut on),
            Ok(ParsedCheat::Cmd(openttdrs_core::Command::CheatSetEnabled(
                true
            )))
        ));
        let mut money = "money 500".split_whitespace();
        assert!(matches!(
            parse_cheat_command(&mut money),
            Ok(ParsedCheat::Cmd(openttdrs_core::Command::CheatAddMoney(
                500
            )))
        ));
        let mut money_default = "money".split_whitespace();
        assert!(matches!(
            parse_cheat_command(&mut money_default),
            Ok(ParsedCheat::Cmd(openttdrs_core::Command::CheatAddMoney(
                DEFAULT_CHEAT_MONEY
            )))
        ));
        let mut infinite = "infinite".split_whitespace();
        assert!(matches!(
            parse_cheat_command(&mut infinite),
            Ok(ParsedCheat::Cmd(
                openttdrs_core::Command::CheatToggleInfiniteMoney
            ))
        ));
        let mut bulldozer = "bulldozer".split_whitespace();
        assert!(matches!(
            parse_cheat_command(&mut bulldozer),
            Ok(ParsedCheat::Cmd(
                openttdrs_core::Command::CheatToggleMagicBulldozer
            ))
        ));
        let mut year = "year 2000".split_whitespace();
        assert!(matches!(
            parse_cheat_command(&mut year),
            Ok(ParsedCheat::Cmd(openttdrs_core::Command::CheatSetYear(
                2000
            )))
        ));
        let mut company = "company 1".split_whitespace();
        assert!(matches!(
            parse_cheat_command(&mut company),
            Ok(ParsedCheat::Cmd(
                openttdrs_core::Command::CheatSwitchCompany(openttdrs_core::CompanyId(1))
            ))
        ));
        let mut status = "".split_whitespace();
        assert!(matches!(
            parse_cheat_command(&mut status),
            Ok(ParsedCheat::Status)
        ));
    }
}
