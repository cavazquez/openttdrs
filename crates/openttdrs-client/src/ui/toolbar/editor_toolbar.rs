//! Toolbar del scenario editor (#42 Fase 2): 19 botones al estilo OpenTTD.
//! Visible solo con [`EditorSession::active`].

use bevy::prelude::*;
use bevy::ui::FocusPolicy;
use openttdrs_core::{
    Command, apply_command, calendar_day_index, calendar_year_day, format_calendar_date,
};

use crate::render::{MapPreviewCamera, PrimaryGameCamera};
use crate::state::ingame_lifecycle::InGameUi;
use crate::state::{EditorSession, SimRunState, SimWorld, sim_is_paused, toggle_sim_run_state};
use crate::ui::audio_settings_window::SoundMusicWindowState;
use crate::ui::extra_viewport_window::ExtraViewportWindowState;
use crate::ui::help_window::HelpWindowState;
use crate::ui::hud::{HudBuildFeedback, SimHudControls};
use crate::ui::industry_directory::IndustryDirectoryState;
use crate::ui::save_window::{SaveWindowMode, SaveWindowState};
use crate::ui::sign_list_window::SignListWindowState;
use crate::ui::tile_inspector_window::TileInspectorWindowState;
use crate::ui::toolbar::{
    BuildMenuAction, BuildMenuUi, ToolbarGroup, ToolbarState, ToolbarTooltipTarget, UiToolState,
};
use crate::ui::town_directory::TownDirectoryState;

const BTN_BG: Color = Color::srgb(0.33, 0.28, 0.19);
const BTN_BORDER: Color = Color::srgb(0.64, 0.57, 0.39);
const BTN_ACTIVE: Color = Color::srgb(0.62, 0.54, 0.34);
const BAR_BG: Color = Color::srgba(0.18, 0.15, 0.11, 0.96);

/// Raíz de la toolbar normal de partida (paneles + grupos).
#[derive(Component)]
pub(crate) struct NormalToolbarRoot;

/// Fila de grupos de la toolbar normal (Rail…Settings); se oculta en editor.
#[derive(Component)]
pub(crate) struct NormalToolbarGroups;

/// Raíz de la toolbar del scenario editor.
#[derive(Component)]
pub(crate) struct EditorToolbarRoot;

/// Texto del año en el bloque Date.
#[derive(Component)]
pub(crate) struct EditorToolbarDateText;

/// Acción de uno de los 19 botones (paridad `ToolbarEditorWidgets`).
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum EditorToolbarAction {
    Pause,
    FastForward,
    Settings,
    Save,
    DateBackward,
    DateForward,
    SmallMap,
    ZoomIn,
    ZoomOut,
    LandGenerate,
    TownGenerate,
    Industry,
    Roads,
    Trams,
    Water,
    Trees,
    Signs,
    MusicSound,
    Help,
}

#[allow(dead_code)] // inventarios UI-0 / tests
impl EditorToolbarAction {
    /// Inventario estable de los 19 botones clicables.
    pub(crate) const ALL: &[Self] = &[
        Self::Pause,
        Self::FastForward,
        Self::Settings,
        Self::Save,
        Self::DateBackward,
        Self::DateForward,
        Self::SmallMap,
        Self::ZoomIn,
        Self::ZoomOut,
        Self::LandGenerate,
        Self::TownGenerate,
        Self::Industry,
        Self::Roads,
        Self::Trams,
        Self::Water,
        Self::Trees,
        Self::Signs,
        Self::MusicSound,
        Self::Help,
    ];

    const fn label(self) -> &'static str {
        match self {
            Self::Pause => "❚❚",
            Self::FastForward => "▶▶",
            Self::Settings => "Ajustes",
            Self::Save => "Guardar",
            Self::DateBackward => "Año−",
            Self::DateForward => "Año+",
            Self::SmallMap => "Mapa",
            Self::ZoomIn => "Zoom+",
            Self::ZoomOut => "Zoom−",
            Self::LandGenerate => "Terreno",
            Self::TownGenerate => "Pueblo",
            Self::Industry => "Industria",
            Self::Roads => "Carretera",
            Self::Trams => "Tranvía",
            Self::Water => "Agua",
            Self::Trees => "Árboles",
            Self::Signs => "Carteles",
            Self::MusicSound => "Música",
            Self::Help => "Ayuda",
        }
    }

    const fn tooltip(self) -> &'static str {
        match self {
            Self::Pause => "Pausa",
            Self::FastForward => "Acelerar / normalizar velocidad",
            Self::Settings => "Abrir panel Ajustes",
            Self::Save => "Guardar escenario en save/scenarios/",
            Self::DateBackward => "Retroceder un año",
            Self::DateForward => "Avanzar un año",
            Self::SmallMap => "Minimapa / directorios / vista extra",
            Self::ZoomIn => "Acercar cámara",
            Self::ZoomOut => "Alejar cámara",
            Self::LandGenerate => "Herramientas de paisaje (GenLand OOS)",
            Self::TownGenerate => "Fundar pueblo",
            Self::Industry => "Colocar industrias",
            Self::Roads => "Carreteras",
            Self::Trams => "Tranvías (panel carretera)",
            Self::Water => "Agua / muelles",
            Self::Trees => "Plantar árbol",
            Self::Signs => "Colocar cartel",
            Self::MusicSound => "Sonido / música",
            Self::Help => "Ayuda + inspector de tile",
        }
    }
}

pub(crate) fn setup_editor_toolbar(mut commands: Commands) {
    commands
        .spawn((
            InGameUi,
            EditorToolbarRoot,
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(10.0),
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(2.0),
                ..default()
            },
            Visibility::Hidden,
            BuildMenuUi,
            GlobalZIndex(2101),
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(2.0),
                    padding: UiRect::axes(Val::Px(6.0), Val::Px(4.0)),
                    border: UiRect::all(Val::Px(2.0)),
                    align_items: AlignItems::Center,
                    flex_wrap: FlexWrap::Wrap,
                    max_width: Val::Percent(98.0),
                    justify_content: JustifyContent::Center,
                    ..default()
                },
                BackgroundColor(BAR_BG),
                BorderColor::all(Color::srgb(0.68, 0.61, 0.42)),
                FocusPolicy::Block,
                BuildMenuUi,
                Interaction::default(),
            ))
            .with_children(|bar| {
                for action in [
                    EditorToolbarAction::Pause,
                    EditorToolbarAction::FastForward,
                    EditorToolbarAction::Settings,
                    EditorToolbarAction::Save,
                ] {
                    spawn_editor_btn(bar, action);
                }
                spawn_spacer_label(bar, "Scenario editor");
                spawn_editor_btn(bar, EditorToolbarAction::DateBackward);
                bar.spawn((
                    EditorToolbarDateText,
                    Text::new("—"),
                    TextFont {
                        font_size: FontSize::Rem(0.65),
                        ..default()
                    },
                    TextColor(Color::srgb(0.95, 0.93, 0.82)),
                    Node {
                        min_width: Val::Px(72.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        padding: UiRect::horizontal(Val::Px(4.0)),
                        ..default()
                    },
                    BuildMenuUi,
                ));
                spawn_editor_btn(bar, EditorToolbarAction::DateForward);
                spawn_spacer(bar);
                spawn_editor_btn(bar, EditorToolbarAction::SmallMap);
                spawn_spacer(bar);
                for action in [EditorToolbarAction::ZoomIn, EditorToolbarAction::ZoomOut] {
                    spawn_editor_btn(bar, action);
                }
                spawn_spacer(bar);
                for action in [
                    EditorToolbarAction::LandGenerate,
                    EditorToolbarAction::TownGenerate,
                    EditorToolbarAction::Industry,
                    EditorToolbarAction::Roads,
                    EditorToolbarAction::Trams,
                    EditorToolbarAction::Water,
                    EditorToolbarAction::Trees,
                    EditorToolbarAction::Signs,
                ] {
                    spawn_editor_btn(bar, action);
                }
                spawn_spacer(bar);
                for action in [EditorToolbarAction::MusicSound, EditorToolbarAction::Help] {
                    spawn_editor_btn(bar, action);
                }
            });
        });
}

fn spawn_spacer(parent: &mut ChildSpawnerCommands) {
    parent.spawn((
        Node {
            width: Val::Px(6.0),
            height: Val::Px(1.0),
            ..default()
        },
        BuildMenuUi,
    ));
}

fn spawn_spacer_label(parent: &mut ChildSpawnerCommands, label: &'static str) {
    parent.spawn((
        Text::new(label),
        TextFont {
            font_size: FontSize::Rem(0.6),
            ..default()
        },
        TextColor(Color::srgb(0.85, 0.78, 0.55)),
        Node {
            margin: UiRect::horizontal(Val::Px(6.0)),
            ..default()
        },
        BuildMenuUi,
    ));
}

fn spawn_editor_btn(parent: &mut ChildSpawnerCommands, action: EditorToolbarAction) {
    parent
        .spawn((
            Button,
            action,
            ToolbarTooltipTarget {
                text: action.tooltip(),
            },
            BuildMenuUi,
            Node {
                min_width: Val::Px(52.0),
                height: Val::Px(28.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border: UiRect::all(Val::Px(1.0)),
                padding: UiRect::horizontal(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(BTN_BG),
            BorderColor::all(BTN_BORDER),
            Interaction::default(),
        ))
        .with_children(|btn| {
            btn.spawn((
                Text::new(action.label()),
                TextFont {
                    font_size: FontSize::Rem(0.6),
                    ..default()
                },
                TextColor(Color::srgb(0.95, 0.93, 0.82)),
            ));
        });
}

/// Alterna visibilidad: barra editor vs fila de grupos normal (los paneles siguen).
pub(crate) fn sync_editor_toolbar_visibility(
    editor: Res<EditorSession>,
    mut editor_q: Query<&mut Visibility, With<EditorToolbarRoot>>,
    mut groups_q: Query<&mut Node, With<NormalToolbarGroups>>,
    mut normal_root_q: Query<&mut Node, (With<NormalToolbarRoot>, Without<NormalToolbarGroups>)>,
) {
    let editor_vis = if editor.active {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
    for mut vis in &mut editor_q {
        *vis = editor_vis;
    }
    for mut node in &mut groups_q {
        node.display = if editor.active {
            Display::None
        } else {
            Display::Flex
        };
    }
    // Baja los paneles bajo la barra del editor (~36 px).
    for mut node in &mut normal_root_q {
        node.top = if editor.active {
            Val::Px(46.0)
        } else {
            Val::Px(10.0)
        };
    }
}

pub(crate) fn sync_editor_toolbar_date(
    editor: Res<EditorSession>,
    sim: Option<Res<SimWorld>>,
    mut q: Query<&mut Text, With<EditorToolbarDateText>>,
) {
    if !editor.active {
        return;
    }
    let Some(sim) = sim.as_deref() else {
        return;
    };
    let label = format_calendar_date(sim.state.tick);
    for mut text in &mut q {
        if **text != label {
            **text = label.clone();
        }
    }
}

pub(crate) fn sync_editor_toolbar_button_visuals(
    editor: Res<EditorSession>,
    run_state: Res<State<SimRunState>>,
    hud: Res<SimHudControls>,
    toolbar: Res<ToolbarState>,
    tool: Res<UiToolState>,
    mut q: Query<(&EditorToolbarAction, &Interaction, &mut BackgroundColor), With<Button>>,
) {
    if !editor.active {
        return;
    }
    let paused = sim_is_paused(&run_state);
    let fast = hud.sim_speed > 1.5;
    for (action, interaction, mut bg) in &mut q {
        let active = match *action {
            EditorToolbarAction::Pause => paused,
            EditorToolbarAction::FastForward => fast,
            EditorToolbarAction::Settings => toolbar.active_group == Some(ToolbarGroup::Settings),
            EditorToolbarAction::LandGenerate => {
                toolbar.active_group == Some(ToolbarGroup::Landscape)
            }
            EditorToolbarAction::TownGenerate => {
                tool.active_tool == Some(BuildMenuAction::FoundTown)
            }
            EditorToolbarAction::Industry => toolbar.active_group == Some(ToolbarGroup::Economy),
            EditorToolbarAction::Roads | EditorToolbarAction::Trams => {
                toolbar.active_group == Some(ToolbarGroup::Road)
            }
            EditorToolbarAction::Water => toolbar.active_group == Some(ToolbarGroup::Water),
            EditorToolbarAction::Trees => tool.active_tool == Some(BuildMenuAction::PlantTree),
            EditorToolbarAction::Signs => tool.active_tool == Some(BuildMenuAction::PlaceSign),
            _ => false,
        };
        *bg = BackgroundColor(if active {
            BTN_ACTIVE
        } else if *interaction == Interaction::Hovered {
            Color::srgb(0.42, 0.36, 0.24)
        } else {
            BTN_BG
        });
    }
}

/// Handlers de control (pausa, velocidad, zoom, guardar, fecha).
#[allow(clippy::too_many_arguments)]
pub(crate) fn handle_editor_toolbar_control_buttons(
    editor: Res<EditorSession>,
    buttons: Query<
        (&Interaction, &EditorToolbarAction),
        (
            Changed<Interaction>,
            With<Button>,
            With<EditorToolbarAction>,
        ),
    >,
    mut hud: ResMut<SimHudControls>,
    run_state: Res<State<SimRunState>>,
    mut next_run: ResMut<NextState<SimRunState>>,
    mut save_window: ResMut<SaveWindowState>,
    mut sim: Option<ResMut<SimWorld>>,
    mut cam_q: Query<
        (&mut Transform, &mut Projection),
        (With<PrimaryGameCamera>, Without<MapPreviewCamera>),
    >,
    mut feedback: ResMut<HudBuildFeedback>,
    time: Res<Time>,
) {
    if !editor.active {
        return;
    }
    for (interaction, action) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match *action {
            EditorToolbarAction::Pause => {
                toggle_sim_run_state(&run_state, &mut next_run);
            }
            EditorToolbarAction::FastForward => {
                hud.sim_speed = if hud.sim_speed < 1.5 {
                    2.0
                } else if hud.sim_speed < 3.5 {
                    4.0
                } else {
                    1.0
                };
            }
            EditorToolbarAction::Save => {
                let dir = crate::state::scenarios_save_dir();
                let _ = std::fs::create_dir_all(&dir);
                save_window.open_in_mode(SaveWindowMode::Save, &dir);
            }
            EditorToolbarAction::ZoomIn => {
                if let Ok((_tf, mut projection)) = cam_q.single_mut()
                    && let Projection::Orthographic(o) = &mut *projection
                {
                    o.scale = (o.scale * 0.85).max(0.25);
                }
            }
            EditorToolbarAction::ZoomOut => {
                if let Ok((_tf, mut projection)) = cam_q.single_mut()
                    && let Projection::Orthographic(o) = &mut *projection
                {
                    o.scale = (o.scale * 1.15).min(20.0);
                }
            }
            EditorToolbarAction::DateBackward | EditorToolbarAction::DateForward => {
                let Some(sim) = sim.as_deref_mut() else {
                    continue;
                };
                let (year, _) = calendar_year_day(calendar_day_index(sim.state.tick));
                let next = if *action == EditorToolbarAction::DateBackward {
                    year.saturating_sub(1)
                } else {
                    year.saturating_add(1)
                };
                match apply_command(&mut sim.state, &Command::CheatSetYear(next)) {
                    Ok(()) => {
                        feedback.message = Some(format!("Año → {next}"));
                        feedback.expires_at_secs = time.elapsed_secs() + 3.0;
                    }
                    Err(e) => {
                        feedback.message =
                            Some(openttdrs_core::command_error_message(e).to_string());
                        feedback.expires_at_secs = time.elapsed_secs() + 4.0;
                    }
                }
            }
            _ => {}
        }
    }
}

/// Handlers de herramientas / grupos / ventanas.
#[allow(clippy::too_many_arguments)]
pub(crate) fn handle_editor_toolbar_tool_buttons(
    editor: Res<EditorSession>,
    buttons: Query<
        (&Interaction, &EditorToolbarAction),
        (
            Changed<Interaction>,
            With<Button>,
            With<EditorToolbarAction>,
        ),
    >,
    mut toolbar: ResMut<ToolbarState>,
    mut tool: ResMut<UiToolState>,
    mut hud: ResMut<SimHudControls>,
    mut sound: ResMut<SoundMusicWindowState>,
    mut help: ResMut<HelpWindowState>,
    mut tile: ResMut<TileInspectorWindowState>,
    mut towns: ResMut<TownDirectoryState>,
    mut industries: ResMut<IndustryDirectoryState>,
    mut signs: ResMut<SignListWindowState>,
    mut extra_view: ResMut<ExtraViewportWindowState>,
    mut feedback: ResMut<HudBuildFeedback>,
    time: Res<Time>,
) {
    if !editor.active {
        return;
    }
    for (interaction, action) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match *action {
            EditorToolbarAction::Settings => {
                open_group(&mut toolbar, &mut tool, ToolbarGroup::Settings);
            }
            EditorToolbarAction::SmallMap => {
                // Ciclo ligero: minimapa → dirs → extra viewport.
                if !hud.minimap_visible {
                    hud.minimap_visible = true;
                    feedback.message = Some("Minimapa ON".into());
                } else if !towns.open {
                    towns.open = true;
                    feedback.message = Some("Directorio de pueblos".into());
                } else if !industries.open {
                    industries.open = true;
                    feedback.message = Some("Directorio de industrias".into());
                } else if !signs.open {
                    signs.open = true;
                    feedback.message = Some("Lista de carteles".into());
                } else if !extra_view.open {
                    extra_view.open = true;
                    feedback.message = Some("Vista extra".into());
                } else {
                    towns.open = false;
                    industries.open = false;
                    signs.open = false;
                    extra_view.open = false;
                    feedback.message = Some("Paneles mapa cerrados".into());
                }
                feedback.expires_at_secs = time.elapsed_secs() + 2.5;
            }
            EditorToolbarAction::LandGenerate => {
                open_group(&mut toolbar, &mut tool, ToolbarGroup::Landscape);
                feedback.message =
                    Some("Terreno: herramientas paisaje (GenLand completo OOS)".into());
                feedback.expires_at_secs = time.elapsed_secs() + 3.0;
            }
            EditorToolbarAction::TownGenerate => {
                open_group(&mut toolbar, &mut tool, ToolbarGroup::Landscape);
                tool.active_tool = Some(BuildMenuAction::FoundTown);
            }
            EditorToolbarAction::Industry => {
                open_group(&mut toolbar, &mut tool, ToolbarGroup::Economy);
            }
            EditorToolbarAction::Roads => {
                open_group(&mut toolbar, &mut tool, ToolbarGroup::Road);
            }
            EditorToolbarAction::Trams => {
                open_group(&mut toolbar, &mut tool, ToolbarGroup::Road);
                feedback.message =
                    Some("Tranvía: usar herramientas tram del panel Carretera".into());
                feedback.expires_at_secs = time.elapsed_secs() + 3.0;
            }
            EditorToolbarAction::Water => {
                open_group(&mut toolbar, &mut tool, ToolbarGroup::Water);
            }
            EditorToolbarAction::Trees => {
                open_group(&mut toolbar, &mut tool, ToolbarGroup::Landscape);
                tool.active_tool = Some(BuildMenuAction::PlantTree);
            }
            EditorToolbarAction::Signs => {
                open_group(&mut toolbar, &mut tool, ToolbarGroup::Landscape);
                tool.active_tool = Some(BuildMenuAction::PlaceSign);
            }
            EditorToolbarAction::MusicSound => {
                sound.open = true;
            }
            EditorToolbarAction::Help => {
                help.open = true;
                tile.open = true;
            }
            _ => {}
        }
    }
}

fn open_group(toolbar: &mut ToolbarState, tool: &mut UiToolState, group: ToolbarGroup) {
    toolbar.active_group = Some(group);
    tool.active_tool = None;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn editor_toolbar_has_nineteen_actions() {
        assert_eq!(EditorToolbarAction::ALL.len(), 19);
        let mut labels = EditorToolbarAction::ALL
            .iter()
            .map(|a| a.label())
            .collect::<Vec<_>>();
        labels.sort_unstable();
        labels.dedup();
        assert_eq!(labels.len(), 19, "labels deben ser únicos");
    }
}
