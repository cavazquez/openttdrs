//! Toolbar del scenario editor (#42 Fase 2): 19 botones al estilo OpenTTD.
//! Visible solo con [`EditorSession::active`].

use bevy::prelude::*;
use bevy::ui::FocusPolicy;
use openttdrs_core::{
    Command, RoadTramType, calendar_day_index, calendar_year_day,
    format_calendar_date,
};

use crate::render::{MapPreviewCamera, PrimaryGameCamera};
use crate::state::ingame_lifecycle::InGameUi;
use crate::state::{EditorSession, SimRunState, SimWorld, sim_is_paused, toggle_sim_run_state};
use crate::ui::audio_settings_window::SoundMusicWindowState;
use crate::ui::command_error_text::command_error_message;
use crate::ui::extra_viewport_window::ExtraViewportWindowState;
use crate::ui::genland_window::GenLandWindowState;
use crate::ui::help_window::HelpWindowState;
use crate::ui::hud::{HudBuildFeedback, SimHudControls};
use crate::ui::industry_directory::IndustryDirectoryState;
use crate::ui::save_window::{SaveWindowMode, SaveWindowState};
use crate::ui::sign_list_window::SignListWindowState;
use crate::ui::tile_inspector_window::TileInspectorWindowState;
use crate::ui::toolbar::build_input::cancel_placement;
use crate::ui::toolbar::road_type_selector::RoadTypePickerState;
use crate::ui::toolbar::{
    BuildMenuAction, BuildMenuUi, DragBuildState, ToolbarGroup, ToolbarState, ToolbarTooltipTarget,
    UiToolState,
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

/// Dropdown Pueblo del editor (Fundar / Casa).
#[derive(Resource, Default)]
pub(crate) struct EditorTownMenuState {
    pub(crate) open: bool,
}

#[derive(Component)]
pub(crate) struct EditorTownDropdownRoot;

#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EditorTownChoice {
    FoundTown,
    BuildHouse,
}

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
            Self::LandGenerate => "Generar paisaje + herramientas terraform",
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
                spawn_editor_btn(bar, EditorToolbarAction::LandGenerate);
                spawn_editor_town_btn(bar);
                for action in [
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

fn spawn_editor_town_btn(parent: &mut ChildSpawnerCommands) {
    let action = EditorToolbarAction::TownGenerate;
    parent
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                min_width: Val::Px(52.0),
                ..default()
            },
            BuildMenuUi,
            ZIndex(2),
        ))
        .with_children(|wrap| {
            wrap.spawn((
                Button,
                action,
                ToolbarTooltipTarget {
                    text: "Fundar pueblo / colocar casa",
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
                children![(
                    Text::new(action.label()),
                    TextFont {
                        font_size: FontSize::Rem(0.6),
                        ..default()
                    },
                    TextColor(Color::srgb(0.95, 0.93, 0.82)),
                )],
            ));
            wrap.spawn((
                EditorTownDropdownRoot,
                BuildMenuUi,
                Node {
                    position_type: PositionType::Absolute,
                    top: Val::Px(30.0),
                    left: Val::Px(0.0),
                    min_width: Val::Px(140.0),
                    padding: UiRect::all(Val::Px(4.0)),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(2.0),
                    border: UiRect::all(Val::Px(1.0)),
                    display: Display::None,
                    ..default()
                },
                BackgroundColor(BAR_BG),
                BorderColor::all(BTN_BORDER),
                GlobalZIndex(2200),
            ))
            .with_children(|menu| {
                spawn_town_choice(menu, EditorTownChoice::FoundTown, "Fundar pueblo");
                spawn_town_choice(menu, EditorTownChoice::BuildHouse, "Colocar casa");
            });
        });
}

fn spawn_town_choice(
    parent: &mut ChildSpawnerCommands,
    choice: EditorTownChoice,
    label: &'static str,
) {
    parent
        .spawn((
            Button,
            choice,
            BuildMenuUi,
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(24.0),
                justify_content: JustifyContent::FlexStart,
                align_items: AlignItems::Center,
                padding: UiRect::horizontal(Val::Px(8.0)),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(BTN_BG),
            BorderColor::all(BTN_BORDER),
            Interaction::default(),
        ))
        .with_children(|btn| {
            btn.spawn((
                Text::new(label),
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
    mut town_menu: ResMut<EditorTownMenuState>,
    mut editor_q: Query<&mut Visibility, With<EditorToolbarRoot>>,
    mut groups_q: Query<&mut Node, With<NormalToolbarGroups>>,
    mut normal_root_q: Query<&mut Node, (With<NormalToolbarRoot>, Without<NormalToolbarGroups>)>,
) {
    if !editor.active {
        town_menu.open = false;
    }
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

#[allow(clippy::too_many_arguments)]
pub(crate) fn sync_editor_toolbar_button_visuals(
    editor: Res<EditorSession>,
    run_state: Res<State<SimRunState>>,
    hud: Res<SimHudControls>,
    toolbar: Res<ToolbarState>,
    tool: Res<UiToolState>,
    genland: Res<GenLandWindowState>,
    town_menu: Res<EditorTownMenuState>,
    road_picker: Res<RoadTypePickerState>,
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
                genland.open || toolbar.active_group == Some(ToolbarGroup::Landscape)
            }
            EditorToolbarAction::TownGenerate => {
                town_menu.open
                    || matches!(
                        tool.active_tool,
                        Some(BuildMenuAction::FoundTown | BuildMenuAction::BuildHouse)
                    )
            }
            EditorToolbarAction::Industry => toolbar.active_group == Some(ToolbarGroup::Economy),
            EditorToolbarAction::Roads => {
                toolbar.active_group == Some(ToolbarGroup::Road)
                    && road_picker.open != Some(RoadTramType::Tram)
            }
            EditorToolbarAction::Trams => {
                toolbar.active_group == Some(ToolbarGroup::Road)
                    && road_picker.open == Some(RoadTramType::Tram)
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
                match crate::network::apply_player_command(&mut sim.state, &Command::CheatSetYear(next)) {
                    Ok(()) => {
                        feedback.message = Some(format!("Año → {next}"));
                        feedback.expires_at_secs = time.elapsed_secs() + 3.0;
                    }
                    Err(e) => {
                        feedback.message = Some(command_error_message(e).to_string());
                        feedback.expires_at_secs = time.elapsed_secs() + 4.0;
                    }
                }
            }
            _ => {}
        }
    }
}

/// Ventanas / GenLand / SmallMap / Settings.
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
    mut genland: ResMut<GenLandWindowState>,
    sim: Option<Res<SimWorld>>,
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
                if !genland.open
                    && let Some(sim) = sim.as_ref()
                {
                    let seed = sim.state.world_seed;
                    genland.seed = if seed == 0 { 42 } else { seed };
                    genland.climate = sim.state.climate;
                }
                genland.open = true;
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

/// Dropdown Pueblo + paneles Road/Water/Landscape del editor.
#[allow(clippy::too_many_arguments)]
pub(crate) fn handle_editor_toolbar_build_buttons(
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
    mut town_menu: ResMut<EditorTownMenuState>,
    mut road_picker: ResMut<RoadTypePickerState>,
) {
    if !editor.active {
        return;
    }
    for (interaction, action) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match *action {
            EditorToolbarAction::TownGenerate => {
                town_menu.open = !town_menu.open;
                tool.block_map_click = true;
            }
            EditorToolbarAction::Industry => {
                town_menu.open = false;
                open_group(&mut toolbar, &mut tool, ToolbarGroup::Economy);
            }
            EditorToolbarAction::Roads => {
                town_menu.open = false;
                open_group(&mut toolbar, &mut tool, ToolbarGroup::Road);
                road_picker.open = Some(RoadTramType::Road);
                road_picker.filter.clear();
            }
            EditorToolbarAction::Trams => {
                town_menu.open = false;
                open_group(&mut toolbar, &mut tool, ToolbarGroup::Road);
                road_picker.open = Some(RoadTramType::Tram);
                road_picker.filter.clear();
            }
            EditorToolbarAction::Water => {
                town_menu.open = false;
                open_group(&mut toolbar, &mut tool, ToolbarGroup::Water);
            }
            EditorToolbarAction::Trees => {
                town_menu.open = false;
                open_group(&mut toolbar, &mut tool, ToolbarGroup::Landscape);
                tool.active_tool = Some(BuildMenuAction::PlantTree);
            }
            EditorToolbarAction::Signs => {
                town_menu.open = false;
                open_group(&mut toolbar, &mut tool, ToolbarGroup::Landscape);
                tool.active_tool = Some(BuildMenuAction::PlaceSign);
            }
            _ => {}
        }
    }
}

pub(crate) fn sync_editor_town_dropdown(
    state: Res<EditorTownMenuState>,
    mut roots: Query<&mut Node, With<EditorTownDropdownRoot>>,
) {
    let display = if state.open {
        Display::Flex
    } else {
        Display::None
    };
    for mut node in &mut roots {
        node.display = display;
    }
}

pub(crate) fn handle_editor_town_dropdown(
    editor: Res<EditorSession>,
    mut state: ResMut<EditorTownMenuState>,
    mut toolbar: ResMut<ToolbarState>,
    mut tool: ResMut<UiToolState>,
    mut drag: ResMut<DragBuildState>,
    choices: Query<
        (&Interaction, &EditorTownChoice),
        (Changed<Interaction>, With<Button>, With<EditorTownChoice>),
    >,
) {
    if !editor.active || !state.open {
        return;
    }
    for (interaction, choice) in &choices {
        if *interaction != Interaction::Pressed {
            continue;
        }
        open_group(&mut toolbar, &mut tool, ToolbarGroup::Landscape);
        tool.active_tool = Some(match *choice {
            EditorTownChoice::FoundTown => BuildMenuAction::FoundTown,
            EditorTownChoice::BuildHouse => BuildMenuAction::BuildHouse,
        });
        // BuildHouse vive en panel Economía; abrir ambos grupos no aplica — Landscape + tool.
        if *choice == EditorTownChoice::BuildHouse {
            toolbar.active_group = Some(ToolbarGroup::Economy);
        }
        cancel_placement(&mut drag);
        state.open = false;
        tool.block_map_click = true;
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
