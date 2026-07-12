//! Ventana NewGRF: edición del stack (activar / reordenar / quitar).
//!
//! Config-only: no aplica Action0–14 ni cambia sprites en runtime. Las entradas
//! `is_static` (p. ej. OpenGFX) no se pueden desactivar ni eliminar.

use bevy::prelude::*;
use openttdrs_core::{
    Command, NewGrfEntry, apply_command, format_grfid, inspect_grf_file, scan_grf_file,
    validate_stack,
};

use crate::state::SimWorld;
use crate::ui::floating_window::{
    FloatingWindow, FloatingWindowClosed, FloatingWindowId, TITLE_BROWN, WINDOW_TEXT,
    spawn_floating_window, window_text_font,
};
use crate::ui::font::UiFontRole;
use crate::ui::hud::{HudBuildFeedback, push_build_command_error};
use crate::ui::toolbar::BuildMenuUi;

const BTN_BG: Color = Color::srgb(0.36, 0.31, 0.21);
const BTN_BORDER: Color = Color::srgb(0.66, 0.58, 0.38);
const ROW_BG: Color = Color::srgb(0.22, 0.18, 0.12);
const ROW_SEL: Color = Color::srgb(0.48, 0.41, 0.27);
const TEXT_COLOR: Color = Color::srgb(0.92, 0.88, 0.72);
const NEWGRF_ROWS: usize = 12;

#[derive(Resource, Default)]
pub(crate) struct NewGrfWindowState {
    pub(crate) open: bool,
    pub(crate) selected: Option<usize>,
    /// Texto de inspección (scan + validate_stack).
    pub(crate) inspect_text: String,
}

#[derive(Component)]
pub(crate) struct NewGrfInspectText;

#[derive(Component, Clone, Copy)]
pub(crate) struct NewGrfRow {
    index: usize,
}

#[derive(Component, Clone, Copy)]
pub(crate) struct NewGrfRowText {
    index: usize,
}

#[derive(Component, Clone, Copy)]
pub(crate) enum NewGrfAction {
    Toggle,
    MoveUp,
    MoveDown,
    Remove,
    /// Escanea directorios conocidos y añade el primer `.grf` ausente del stack.
    AddFromDisk,
    /// Re-escanea la entrada seleccionada (Action8 + histograma Action0–14).
    Inspect,
}

pub(crate) fn setup_newgrf_window(mut commands: Commands, asset_server: Res<AssetServer>) {
    let asset_server = &*asset_server;
    let (_root, content) = spawn_floating_window(
        &mut commands,
        asset_server,
        FloatingWindowId::NewGrf,
        "NewGRF",
        TITLE_BROWN,
        Vec2::new(280.0, 100.0),
        460.0,
    );
    commands.entity(content).with_children(|body| {
        body.spawn((
            Text::new(
                "Stack + Inspeccionar (histograma). RoadTypes Action0: metadatos en selector (sin sprites).",
            ),
            window_text_font(asset_server, UiFontRole::Caption),
            TextColor(Color::srgb(0.82, 0.78, 0.68)),
            Node {
                margin: UiRect::bottom(Val::Px(6.0)),
                ..default()
            },
        ));
        body.spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(2.0),
            max_height: Val::Px(220.0),
            overflow: Overflow::scroll_y(),
            margin: UiRect::bottom(Val::Px(6.0)),
            ..default()
        })
        .with_children(|list| {
            for index in 0..NEWGRF_ROWS {
                list.spawn((
                    Button,
                    NewGrfRow { index },
                    Node {
                        width: Val::Percent(100.0),
                        min_height: Val::Px(34.0),
                        padding: UiRect::all(Val::Px(4.0)),
                        border: UiRect::all(Val::Px(1.0)),
                        display: Display::None,
                        ..default()
                    },
                    BackgroundColor(ROW_BG),
                    BorderColor::all(BTN_BORDER),
                    Interaction::default(),
                    BuildMenuUi,
                ))
                .with_children(|row| {
                    row.spawn((
                        NewGrfRowText { index },
                        Text::new(""),
                        window_text_font(asset_server, UiFontRole::Caption),
                        TextColor(TEXT_COLOR),
                    ));
                });
            }
        });
        body.spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(4.0),
            ..default()
        })
        .with_children(|bar| {
            spawn_action_btn(bar, asset_server, NewGrfAction::Toggle, "ON/OFF");
            spawn_action_btn(bar, asset_server, NewGrfAction::MoveUp, "↑");
            spawn_action_btn(bar, asset_server, NewGrfAction::MoveDown, "↓");
            spawn_action_btn(bar, asset_server, NewGrfAction::Remove, "Quitar");
            spawn_action_btn(bar, asset_server, NewGrfAction::AddFromDisk, "Añadir…");
            spawn_action_btn(bar, asset_server, NewGrfAction::Inspect, "Inspeccionar");
        });
        body.spawn((
            NewGrfInspectText,
            Text::new(
                "Selecciona una entrada y pulsa Inspeccionar (scan + validate; sin Action0–14).",
            ),
            window_text_font(asset_server, UiFontRole::Caption),
            TextColor(Color::srgb(0.78, 0.84, 0.72)),
            Node {
                width: Val::Percent(100.0),
                margin: UiRect::top(Val::Px(8.0)),
                ..default()
            },
            BuildMenuUi,
        ));
    });
}

fn spawn_action_btn(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    action: NewGrfAction,
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

pub(crate) fn sync_newgrf_window(
    state: Res<NewGrfWindowState>,
    sim: Option<Res<SimWorld>>,
    mut windows: Query<(&FloatingWindow, &mut Visibility)>,
    mut rows: Query<(&NewGrfRow, &mut Node, &mut BackgroundColor), With<Button>>,
    mut texts: Query<(&NewGrfRowText, &mut Text), Without<NewGrfInspectText>>,
    mut inspect_q: Query<&mut Text, With<NewGrfInspectText>>,
) {
    let Some(sim) = sim else {
        return;
    };
    let visible = state.open;
    for (w, mut vis) in &mut windows {
        if w.id == FloatingWindowId::NewGrf {
            *vis = if visible {
                Visibility::Visible
            } else {
                Visibility::Hidden
            };
        }
    }
    if !visible {
        for (_, mut node, _) in &mut rows {
            node.display = Display::None;
        }
        return;
    }
    let stack = &sim.state.newgrf_stack;
    for (row, mut node, mut bg) in &mut rows {
        if let Some(entry) = stack.get(row.index) {
            node.display = Display::Flex;
            *bg = if state.selected == Some(row.index) {
                BackgroundColor(ROW_SEL)
            } else {
                BackgroundColor(ROW_BG)
            };
            let _ = entry;
        } else {
            node.display = Display::None;
        }
    }
    for (row_text, mut text) in &mut texts {
        if let Some(entry) = stack.get(row_text.index) {
            let flag = if entry.enabled { "ON" } else { "off" };
            let static_mark = if entry.is_static { " [base]" } else { "" };
            let name = if entry.name.is_empty() {
                entry.filename.as_str()
            } else {
                entry.name.as_str()
            };
            **text = format!(
                "{}. [{flag}] {name}{static_mark}\n   {}  {}",
                row_text.index + 1,
                format_grfid(entry.grfid),
                entry.filename
            );
        } else {
            **text = String::new();
        }
    }
    let inspect_body = if state.inspect_text.is_empty() {
        "Selecciona una entrada y pulsa Inspeccionar (scan + validate; sin Action0–14).".to_string()
    } else {
        state.inspect_text.clone()
    };
    for mut text in &mut inspect_q {
        **text = inspect_body.clone();
    }
}

pub(crate) fn handle_newgrf_window_buttons(
    mut row_q: Query<
        (&Interaction, &NewGrfRow),
        (Changed<Interaction>, With<Button>, Without<NewGrfAction>),
    >,
    mut action_q: Query<
        (&Interaction, &NewGrfAction),
        (Changed<Interaction>, With<Button>, Without<NewGrfRow>),
    >,
    mut state: ResMut<NewGrfWindowState>,
    mut sim: ResMut<SimWorld>,
    mut hud_feedback: ResMut<HudBuildFeedback>,
    time: Res<Time>,
) {
    if !state.open {
        return;
    }
    for (interaction, row) in &mut row_q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        if row.index < sim.state.newgrf_stack.len() {
            state.selected = Some(row.index);
        }
    }
    for (interaction, action) in &mut action_q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        if matches!(action, NewGrfAction::AddFromDisk) {
            match add_next_available_grf(&mut sim) {
                Ok(name) => {
                    state.selected = sim.state.newgrf_stack.len().checked_sub(1);
                    info!("NewGRF añadido (config): {name}");
                }
                Err(msg) => {
                    hud_feedback.message = Some(msg);
                    hud_feedback.expires_at_secs = time.elapsed_secs() + 3.5;
                }
            }
            continue;
        }
        if matches!(action, NewGrfAction::Inspect) {
            let Some(index) = state.selected else {
                state.inspect_text = "Selecciona una entrada del stack.".into();
                continue;
            };
            state.inspect_text = inspect_newgrf_entry(&sim.state.newgrf_stack, index);
            continue;
        }
        let Some(index) = state.selected else {
            continue;
        };
        if index >= sim.state.newgrf_stack.len() {
            state.selected = None;
            continue;
        }
        let cmd = match action {
            NewGrfAction::Toggle => {
                let enabled = !sim.state.newgrf_stack[index].enabled;
                Command::SetNewGrfEnabled { index, enabled }
            }
            NewGrfAction::MoveUp => {
                let Some(to) = index.checked_sub(1) else {
                    continue;
                };
                Command::MoveNewGrfInStack { from: index, to }
            }
            NewGrfAction::MoveDown => {
                let to = index + 1;
                if to >= sim.state.newgrf_stack.len() {
                    continue;
                }
                Command::MoveNewGrfInStack { from: index, to }
            }
            NewGrfAction::Remove => Command::RemoveNewGrfFromStack { index },
            NewGrfAction::AddFromDisk | NewGrfAction::Inspect => unreachable!(),
        };
        match apply_command(&mut sim.state, &cmd) {
            Ok(()) => {
                let len = sim.state.newgrf_stack.len();
                state.selected = match action {
                    NewGrfAction::Remove => {
                        if len == 0 {
                            None
                        } else {
                            Some(index.min(len - 1))
                        }
                    }
                    NewGrfAction::MoveUp => index.checked_sub(1).or(Some(index)),
                    NewGrfAction::MoveDown => Some((index + 1).min(len.saturating_sub(1))),
                    NewGrfAction::Toggle => Some(index),
                    NewGrfAction::AddFromDisk | NewGrfAction::Inspect => state.selected,
                };
            }
            Err(e) => push_build_command_error(&mut hud_feedback, e, time.elapsed_secs()),
        }
    }
}

fn inspect_newgrf_entry(stack: &[NewGrfEntry], index: usize) -> String {
    let Some(entry) = stack.get(index) else {
        return "Índice fuera de rango.".into();
    };
    let dirs = newgrf_search_dirs();
    let path = dirs
        .iter()
        .map(|d| d.join(&entry.filename))
        .find(|p| p.is_file());
    let mut lines = vec![
        format!(
            "[{}] {}  enabled={} static={}",
            format_grfid(entry.grfid),
            entry.filename,
            entry.enabled,
            entry.is_static
        ),
        format!(
            "name={}  ver={}",
            if entry.name.is_empty() {
                "—"
            } else {
                entry.name.as_str()
            },
            entry.grf_version
        ),
    ];
    match path {
        Some(p) => {
            match scan_grf_file(&p) {
                Ok(info) => {
                    lines.push(format!(
                        "scan: {:?} size={}B grfid={:?} name={:?}",
                        info.container,
                        info.file_size,
                        info.grfid.map(format_grfid),
                        info.name
                    ));
                    if let Some(desc) = info.description.filter(|d| !d.is_empty()) {
                        lines.push(format!("desc: {desc}"));
                    }
                }
                Err(e) => lines.push(format!("scan error: {e}")),
            }
            match inspect_grf_file(&p) {
                Ok(report) => {
                    lines.push("--- histograma Action0–14 ---".into());
                    lines.push(report.format_summary());
                }
                Err(e) => lines.push(format!("inspect error: {e}")),
            }
        }
        None => lines.push("archivo no encontrado en dirs de búsqueda".into()),
    }
    let dir_refs: Vec<&std::path::Path> = dirs.iter().map(std::path::PathBuf::as_path).collect();
    let issues = validate_stack(stack, &dir_refs);
    if issues.is_empty() {
        lines.push("validate_stack: OK".into());
    } else {
        lines.push(format!("validate_stack: {issues:?}"));
    }
    lines.join("\n")
}

fn newgrf_search_dirs() -> Vec<std::path::PathBuf> {
    let mut dirs = vec![
        std::path::PathBuf::from("assets/opengfx/opengfx2-32ez"),
        std::path::PathBuf::from("assets/newgrf"),
    ];
    if let Ok(extra) = std::env::var("OPENTTDRS_NEWGRF_DIR")
        && !extra.trim().is_empty()
    {
        dirs.push(std::path::PathBuf::from(extra));
    }
    dirs
}

fn add_next_available_grf(sim: &mut SimWorld) -> Result<String, String> {
    let known: std::collections::HashSet<u32> =
        sim.state.newgrf_stack.iter().map(|e| e.grfid).collect();
    let known_names: std::collections::HashSet<String> = sim
        .state
        .newgrf_stack
        .iter()
        .map(|e| e.filename.clone())
        .collect();
    for dir in newgrf_search_dirs() {
        let Ok(rd) = std::fs::read_dir(&dir) else {
            continue;
        };
        let mut files: Vec<_> = rd
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| {
                p.extension()
                    .and_then(|e| e.to_str())
                    .is_some_and(|e| e.eq_ignore_ascii_case("grf"))
            })
            .collect();
        files.sort();
        for path in files {
            let Some(file_name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if known_names.contains(file_name) {
                continue;
            }
            let info = scan_grf_file(&path).map_err(|e| format!("Scan {file_name}: {e}"))?;
            let Some(grfid) = info.grfid else {
                continue;
            };
            if known.contains(&grfid) {
                continue;
            }
            let mut entry = NewGrfEntry::new(file_name, grfid);
            entry.name = info.name.unwrap_or_default();
            entry.description = info.description.unwrap_or_default();
            entry.grf_version = info.grf_version.unwrap_or(0);
            apply_command(
                &mut sim.state,
                &Command::AddNewGrfToStack {
                    entry: entry.clone(),
                },
            )
            .map_err(|e| openttdrs_core::command_error_message(e).to_string())?;
            let label = if entry.name.is_empty() {
                file_name.to_string()
            } else {
                entry.name
            };
            return Ok(label);
        }
    }
    Err("No hay más .grf nuevos en assets/opengfx/opengfx2-32ez ni OPENTTDRS_NEWGRF_DIR.".into())
}

pub(crate) fn newgrf_window_on_closed(
    mut closed: MessageReader<FloatingWindowClosed>,
    mut state: ResMut<NewGrfWindowState>,
) {
    for msg in closed.read() {
        if msg.0 == FloatingWindowId::NewGrf {
            state.open = false;
            state.selected = None;
            state.inspect_text.clear();
        }
    }
}
