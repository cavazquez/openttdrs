//! Toolbar del scenario editor (#42 Fase 2): 19 botones al estilo OpenTTD.
//! Visible solo con [`EditorSession::active`].

use bevy::prelude::*;
use bevy::ui::FocusPolicy;
use bevy::ui::UiScale;
use bevy::window::PrimaryWindow;
use openttdrs_core::Command;
use openttdrs_core::{
    RoadTramType, RoadTypeDef, calendar_day_index, calendar_year_day, format_calendar_date,
    list_road_types,
};

use crate::render::{
    MapPreviewCamera, PrimaryGameCamera, RemapMapVisualsPending, clamp_ortho_scale,
    large_map_viewport_cull_enabled,
};
use crate::state::ingame_lifecycle::InGameUi;
use crate::state::{EditorSession, SimRunState, SimWorld, sim_is_paused, toggle_sim_run_state};
use crate::ui::audio_settings_window::SoundMusicWindowState;
use crate::ui::command_error_text::command_error_message;
use crate::ui::extra_viewport_window::ExtraViewportWindowState;
use crate::ui::genland_window::GenLandWindowState;
use crate::ui::help_window::HelpWindowState;
use crate::ui::hud::{HudBuildFeedback, SimHudControls};
use crate::ui::industry_directory::IndustryDirectoryState;
use crate::ui::navigation::{MenuId, OpenUiRoute, UiRoute, spawn_menu_anchor_button_sized};
use crate::ui::save_window::{SaveWindowMode, SaveWindowState};
use crate::ui::sign_list_window::SignListWindowState;
use crate::ui::tile_inspector_window::TileInspectorWindowState;
use crate::ui::toolbar::build_input::cancel_placement;
use crate::ui::toolbar::road_type_selector::RoadTypePickerState;
use crate::ui::toolbar::{
    BuildMenuAction, BuildMenuUi, DragBuildState, ToolbarGroup, ToolbarIcon, ToolbarState,
    ToolbarTooltipTarget, UiToolState,
};
use crate::ui::town_directory::TownDirectoryState;

const BTN_BG: Color = Color::srgb(0.33, 0.28, 0.19);
const BTN_BORDER: Color = Color::srgb(0.64, 0.57, 0.39);
const BTN_ACTIVE: Color = Color::srgb(0.62, 0.54, 0.34);
const BTN_DISABLED: Color = Color::srgb(0.20, 0.18, 0.15);
const BAR_BG: Color = Color::srgba(0.18, 0.15, 0.11, 0.96);

const EDITOR_MIN_YEAR: u32 = openttdrs_core::CALENDAR_BASE_YEAR;
const EDITOR_MAX_YEAR: u32 = openttdrs_core::cheats::CHEAT_YEAR_MAX;

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

#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EditorToolbarHalf {
    Controls,
    Construction,
}

#[derive(Component)]
pub(crate) struct EditorToolbarSwitchButton;

#[derive(Resource, Default)]
pub(crate) struct EditorToolbarLayoutState {
    compact: bool,
    show_construction: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EditorExitTarget {
    MainMenu,
    Game,
}

#[derive(Resource, Default)]
pub(crate) struct EditorDocumentState {
    baseline_revision: u64,
    initialized: bool,
    exit_target: Option<EditorExitTarget>,
}

impl EditorDocumentState {
    fn is_dirty(&self) -> bool {
        self.initialized && crate::network::player_command_revision() != self.baseline_revision
    }

    pub(crate) fn mark_saved(&mut self) {
        self.baseline_revision = crate::network::player_command_revision();
        self.initialized = true;
    }

    fn mark_dirty(&mut self) {
        self.initialized = true;
        self.baseline_revision = crate::network::player_command_revision().wrapping_add(1);
    }
}

#[derive(Component)]
pub(crate) struct EditorExitConfirmRoot;

#[derive(Component, Clone, Copy)]
pub(crate) enum EditorExitConfirmButton {
    Discard,
    Cancel,
}

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

#[must_use]
fn editor_action_available(
    action: EditorToolbarAction,
    year: u32,
    road_catalog: &[RoadTypeDef],
) -> bool {
    match action {
        EditorToolbarAction::DateBackward => year > EDITOR_MIN_YEAR,
        EditorToolbarAction::DateForward => year < EDITOR_MAX_YEAR,
        EditorToolbarAction::Roads => {
            !list_road_types(road_catalog, RoadTramType::Road, "", year).is_empty()
        }
        EditorToolbarAction::Trams => {
            !list_road_types(road_catalog, RoadTramType::Tram, "", year).is_empty()
        }
        _ => true,
    }
}

fn editor_action_available_in_sim(action: EditorToolbarAction, sim: Option<&SimWorld>) -> bool {
    let Some(sim) = sim else {
        return !matches!(
            action,
            EditorToolbarAction::DateBackward
                | EditorToolbarAction::DateForward
                | EditorToolbarAction::Roads
                | EditorToolbarAction::Trams
        );
    };
    let (year, _) = calendar_year_day(calendar_day_index(sim.state.tick));
    editor_action_available(action, year, &sim.state.road_type_catalog)
}

fn heightmaps_dir() -> std::path::PathBuf {
    std::path::PathBuf::from("save/heightmaps")
}

fn latest_heightmap_path(dir: &std::path::Path) -> Option<std::path::PathBuf> {
    let mut files = std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            let is_hmap = path
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case("hmap"));
            if !is_hmap {
                return None;
            }
            let modified = entry.metadata().ok()?.modified().ok()?;
            Some((modified, path))
        })
        .collect::<Vec<_>>();
    files.sort_unstable_by_key(|entry| std::cmp::Reverse(entry.0));
    files.into_iter().next().map(|(_, path)| path)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn handle_editor_file_routes(
    editor: Res<EditorSession>,
    mut routes: MessageReader<OpenUiRoute>,
    mut save_window: ResMut<SaveWindowState>,
    mut sim: ResMut<SimWorld>,
    mut pending_remap: Option<ResMut<RemapMapVisualsPending>>,
    mut document: ResMut<EditorDocumentState>,
    mut next_screen: ResMut<NextState<crate::state::ClientScreen>>,
    mut suspended: ResMut<crate::state::SuspendedGameSession>,
    mut exit: MessageWriter<AppExit>,
) {
    if !editor.active {
        routes.clear();
        return;
    }
    for OpenUiRoute(route) in routes.read() {
        match route {
            UiRoute::EditorSaveScenario => {
                let dir = crate::state::scenarios_save_dir();
                let _ = std::fs::create_dir_all(&dir);
                save_window.open_in_mode(SaveWindowMode::Save, &dir);
            }
            UiRoute::EditorLoadScenario => {
                let dir = crate::state::scenarios_save_dir();
                save_window.open_in_mode(SaveWindowMode::Load, &dir);
            }
            UiRoute::EditorSaveHeightmap => {
                let dir = heightmaps_dir();
                let path = dir.join("scenario-editor.hmap");
                let result = std::fs::create_dir_all(&dir).and_then(|()| {
                    std::fs::write(&path, openttdrs_core::serialize_heightmap(&sim.state.map))
                });
                if let Err(error) = result {
                    warn!("No se pudo guardar {}: {error}", path.display());
                } else {
                    info!("Heightmap guardado en {}", path.display());
                }
            }
            UiRoute::EditorLoadHeightmap => {
                let Some(path) = latest_heightmap_path(&heightmaps_dir()) else {
                    warn!("No hay heightmaps en {}", heightmaps_dir().display());
                    continue;
                };
                let result = std::fs::read_to_string(&path)
                    .map_err(|error| error.to_string())
                    .and_then(|text| openttdrs_core::parse_hmap(&text))
                    .and_then(|data| {
                        let climate = sim.state.climate;
                        let seed = sim.state.world_seed;
                        openttdrs_core::apply_heightmap(&mut sim.state.map, &data, 1, climate, seed)
                            .map_err(|error| format!("{error:?}"))
                    });
                if let Err(error) = result {
                    warn!("No se pudo cargar {}: {error}", path.display());
                } else {
                    document.mark_dirty();
                    if let Some(remap) = pending_remap.as_deref_mut() {
                        remap.pending = true;
                        remap.sync_camera = true;
                        remap.full = true;
                    }
                }
            }
            UiRoute::EditorExit => {
                if document.is_dirty() {
                    document.exit_target = Some(EditorExitTarget::MainMenu);
                } else {
                    crate::ui::main_menu::return_to_main_menu(&mut next_screen, &mut suspended);
                }
            }
            UiRoute::ExitGame => {
                if document.is_dirty() {
                    document.exit_target = Some(EditorExitTarget::Game);
                } else {
                    exit.write(AppExit::Success);
                }
            }
            _ => {}
        }
    }
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

    const fn icon(self) -> Option<ToolbarIcon> {
        Some(match self {
            Self::Pause => ToolbarIcon::Pause,
            Self::FastForward => ToolbarIcon::FastForward,
            Self::Settings => ToolbarIcon::Settings,
            Self::Save => ToolbarIcon::Save,
            Self::SmallMap => ToolbarIcon::SmallMap,
            Self::ZoomIn => ToolbarIcon::ZoomIn,
            Self::ZoomOut => ToolbarIcon::ZoomOut,
            Self::LandGenerate => ToolbarIcon::Landscape,
            Self::TownGenerate => ToolbarIcon::Town,
            Self::Industry => ToolbarIcon::Industry,
            Self::Roads => ToolbarIcon::BuildRoad,
            Self::Trams => ToolbarIcon::BuildTram,
            Self::Water => ToolbarIcon::BuildWater,
            Self::Trees => ToolbarIcon::Trees,
            Self::Signs => ToolbarIcon::Sign,
            Self::MusicSound => ToolbarIcon::Music,
            Self::Help => ToolbarIcon::Help,
            Self::DateBackward | Self::DateForward => return None,
        })
    }
}

pub(crate) fn setup_editor_toolbar(mut commands: Commands, asset_server: Res<AssetServer>) {
    let asset_server = &*asset_server;
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
                bar.spawn((EditorToolbarHalf::Controls, editor_half_node()))
                    .with_children(|controls| {
                        for action in [EditorToolbarAction::Pause, EditorToolbarAction::FastForward]
                        {
                            spawn_editor_btn(controls, asset_server, action);
                        }
                        spawn_menu_anchor_button_sized(
                            controls,
                            asset_server,
                            MenuId::Settings,
                            64.0,
                            28.0,
                        );
                        spawn_menu_anchor_button_sized(
                            controls,
                            asset_server,
                            MenuId::EditorFile,
                            64.0,
                            28.0,
                        );
                        spawn_spacer_label(controls, "Scenario editor");
                        spawn_editor_btn(controls, asset_server, EditorToolbarAction::DateBackward);
                        controls.spawn((
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
                        spawn_editor_btn(controls, asset_server, EditorToolbarAction::DateForward);
                        spawn_spacer(controls);
                        spawn_menu_anchor_button_sized(
                            controls,
                            asset_server,
                            MenuId::EditorMap,
                            54.0,
                            28.0,
                        );
                        spawn_spacer(controls);
                        for action in [EditorToolbarAction::ZoomIn, EditorToolbarAction::ZoomOut] {
                            spawn_editor_btn(controls, asset_server, action);
                        }
                    });
                bar.spawn((EditorToolbarHalf::Construction, editor_half_node()))
                    .with_children(|construction| {
                        spawn_editor_btn(
                            construction,
                            asset_server,
                            EditorToolbarAction::LandGenerate,
                        );
                        spawn_editor_town_btn(construction, asset_server);
                        for action in [
                            EditorToolbarAction::Industry,
                            EditorToolbarAction::Roads,
                            EditorToolbarAction::Trams,
                            EditorToolbarAction::Water,
                            EditorToolbarAction::Trees,
                            EditorToolbarAction::Signs,
                        ] {
                            spawn_editor_btn(construction, asset_server, action);
                        }
                        spawn_spacer(construction);
                        spawn_editor_btn(
                            construction,
                            asset_server,
                            EditorToolbarAction::MusicSound,
                        );
                        spawn_menu_anchor_button_sized(
                            construction,
                            asset_server,
                            MenuId::Help,
                            52.0,
                            28.0,
                        );
                    });
                bar.spawn((
                    Button,
                    EditorToolbarSwitchButton,
                    Node {
                        display: Display::None,
                        width: Val::Px(34.0),
                        height: Val::Px(28.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        border: UiRect::all(Val::Px(1.0)),
                        ..default()
                    },
                    BackgroundColor(BTN_BG),
                    BorderColor::all(BTN_BORDER),
                    Interaction::default(),
                    children![(
                        ImageNode::new(asset_server.load::<Image>(ToolbarIcon::Switch.path())),
                        Node {
                            width: Val::Px(22.0),
                            height: Val::Px(22.0),
                            ..default()
                        },
                    )],
                ));
            });
        });
    commands
        .spawn((
            InGameUi,
            EditorExitConfirmRoot,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                top: Val::Px(0.0),
                bottom: Val::Px(0.0),
                display: Display::None,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.04, 0.03, 0.02, 0.72)),
            GlobalZIndex(4000),
            Interaction::default(),
        ))
        .with_children(|overlay| {
            overlay
                .spawn((
                    Node {
                        width: Val::Px(410.0),
                        padding: UiRect::all(Val::Px(16.0)),
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(12.0),
                        border: UiRect::all(Val::Px(2.0)),
                        ..default()
                    },
                    BackgroundColor(BAR_BG),
                    BorderColor::all(BTN_BORDER),
                ))
                .with_children(|panel| {
                    panel.spawn((
                        Text::new("El escenario tiene cambios sin guardar."),
                        TextFont {
                            font_size: FontSize::Rem(0.9),
                            ..default()
                        },
                        TextColor(Color::srgb(0.95, 0.93, 0.82)),
                    ));
                    panel
                        .spawn(Node {
                            flex_direction: FlexDirection::Row,
                            column_gap: Val::Px(8.0),
                            justify_content: JustifyContent::FlexEnd,
                            ..default()
                        })
                        .with_children(|buttons| {
                            spawn_exit_confirm_button(
                                buttons,
                                EditorExitConfirmButton::Cancel,
                                "Cancelar",
                            );
                            spawn_exit_confirm_button(
                                buttons,
                                EditorExitConfirmButton::Discard,
                                "Salir sin guardar",
                            );
                        });
                });
        });
}

fn spawn_exit_confirm_button(
    parent: &mut ChildSpawnerCommands,
    action: EditorExitConfirmButton,
    label: &'static str,
) {
    parent.spawn((
        Button,
        action,
        Node {
            height: Val::Px(30.0),
            padding: UiRect::horizontal(Val::Px(10.0)),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        BackgroundColor(BTN_BG),
        BorderColor::all(BTN_BORDER),
        Interaction::default(),
        children![(
            Text::new(label),
            TextFont {
                font_size: FontSize::Rem(0.7),
                ..default()
            },
            TextColor(Color::srgb(0.95, 0.93, 0.82)),
        )],
    ));
}

pub(crate) fn initialize_editor_document(
    editor: Res<EditorSession>,
    mut document: ResMut<EditorDocumentState>,
) {
    if editor.active && !document.initialized {
        document.mark_saved();
    } else if !editor.active {
        *document = EditorDocumentState::default();
    }
}

pub(crate) fn sync_editor_exit_confirmation(
    document: Res<EditorDocumentState>,
    mut roots: Query<&mut Node, With<EditorExitConfirmRoot>>,
) {
    for mut node in &mut roots {
        node.display = if document.exit_target.is_some() {
            Display::Flex
        } else {
            Display::None
        };
    }
}

pub(crate) fn handle_editor_exit_confirmation(
    mut document: ResMut<EditorDocumentState>,
    buttons: Query<(&Interaction, &EditorExitConfirmButton), (Changed<Interaction>, With<Button>)>,
    mut next_screen: ResMut<NextState<crate::state::ClientScreen>>,
    mut suspended: ResMut<crate::state::SuspendedGameSession>,
    mut exit: MessageWriter<AppExit>,
) {
    for (interaction, button) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match button {
            EditorExitConfirmButton::Cancel => document.exit_target = None,
            EditorExitConfirmButton::Discard => {
                let target = document.exit_target.take();
                match target {
                    Some(EditorExitTarget::MainMenu) => {
                        crate::ui::main_menu::return_to_main_menu(&mut next_screen, &mut suspended);
                    }
                    Some(EditorExitTarget::Game) => {
                        exit.write(AppExit::Success);
                    }
                    None => {}
                }
            }
        }
    }
}

fn editor_half_node() -> Node {
    Node {
        flex_direction: FlexDirection::Row,
        column_gap: Val::Px(2.0),
        align_items: AlignItems::Center,
        justify_content: JustifyContent::Center,
        ..default()
    }
}

fn editor_layout_is_compact(window_width: f32, ui_scale: f32) -> bool {
    window_width / ui_scale.max(0.5) < 1_250.0
}

pub(crate) fn handle_editor_toolbar_switch(
    mut state: ResMut<EditorToolbarLayoutState>,
    buttons: Query<&Interaction, (Changed<Interaction>, With<EditorToolbarSwitchButton>)>,
) {
    if buttons
        .iter()
        .any(|interaction| *interaction == Interaction::Pressed)
    {
        state.show_construction = !state.show_construction;
    }
}

pub(crate) fn sync_editor_toolbar_layout(
    windows: Query<&Window, With<PrimaryWindow>>,
    ui_scale: Res<UiScale>,
    mut state: ResMut<EditorToolbarLayoutState>,
    mut halves: Query<(&EditorToolbarHalf, &mut Node)>,
    mut switches: Query<&mut Node, (With<EditorToolbarSwitchButton>, Without<EditorToolbarHalf>)>,
) {
    let width = windows.iter().next().map_or(1_920.0, Window::width);
    state.compact = editor_layout_is_compact(width, ui_scale.0);
    for (half, mut node) in &mut halves {
        node.display = if !state.compact
            || matches!(
                (*half, state.show_construction),
                (EditorToolbarHalf::Controls, false) | (EditorToolbarHalf::Construction, true)
            ) {
            Display::Flex
        } else {
            Display::None
        };
    }
    for mut node in &mut switches {
        node.display = if state.compact {
            Display::Flex
        } else {
            Display::None
        };
    }
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

fn spawn_editor_btn(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    action: EditorToolbarAction,
) {
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
            if let Some(icon) = action.icon() {
                btn.spawn((
                    ImageNode::new(asset_server.load::<Image>(icon.path())),
                    Node {
                        width: Val::Px(22.0),
                        height: Val::Px(22.0),
                        ..default()
                    },
                ));
            } else {
                btn.spawn((
                    Text::new(action.label()),
                    TextFont {
                        font_size: FontSize::Rem(0.6),
                        ..default()
                    },
                    TextColor(Color::srgb(0.95, 0.93, 0.82)),
                ));
            }
        });
}

fn spawn_editor_town_btn(parent: &mut ChildSpawnerCommands, asset_server: &AssetServer) {
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
                    ImageNode::new(asset_server.load::<Image>(ToolbarIcon::Town.path())),
                    Node {
                        width: Val::Px(22.0),
                        height: Val::Px(22.0),
                        ..default()
                    },
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
    sim: Option<Res<SimWorld>>,
    mut q: Query<(&EditorToolbarAction, &Interaction, &mut BackgroundColor), With<Button>>,
) {
    if !editor.active {
        return;
    }
    let paused = sim_is_paused(&run_state);
    let fast = hud.sim_speed > 1.5;
    for (action, interaction, mut bg) in &mut q {
        if !editor_action_available_in_sim(*action, sim.as_deref()) {
            *bg = BackgroundColor(BTN_DISABLED);
            continue;
        }
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
    windows: Query<&Window, With<PrimaryWindow>>,
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
        if !editor_action_available_in_sim(*action, sim.as_deref()) {
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
            EditorToolbarAction::ZoomIn | EditorToolbarAction::ZoomOut => {
                if let Ok((_tf, mut projection)) = cam_q.single_mut()
                    && let Projection::Orthographic(o) = &mut *projection
                {
                    let (mw, mh) = sim
                        .as_ref()
                        .map(|s| s.state.map.dimensions())
                        .unwrap_or((64, 64));
                    let large_cull = large_map_viewport_cull_enabled(mw, mh);
                    let (win_w, win_h) = windows
                        .iter()
                        .next()
                        .map(|w| (w.width(), w.height()))
                        .unwrap_or((1280.0, 720.0));
                    let factor = if *action == EditorToolbarAction::ZoomIn {
                        0.85
                    } else {
                        1.15
                    };
                    o.scale = clamp_ortho_scale(o.scale * factor, win_w, win_h, large_cull);
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
                match crate::network::apply_player_command(
                    &mut sim.state,
                    &Command::CheatSetYear(next),
                ) {
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
    sim: Option<Res<SimWorld>>,
) {
    if !editor.active {
        return;
    }
    for (interaction, action) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        if !editor_action_available_in_sim(*action, sim.as_deref()) {
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

    #[test]
    fn date_buttons_stop_at_editor_limits() {
        let catalog = openttdrs_core::vanilla_road_type_catalog();
        assert!(!editor_action_available(
            EditorToolbarAction::DateBackward,
            EDITOR_MIN_YEAR,
            &catalog,
        ));
        assert!(editor_action_available(
            EditorToolbarAction::DateForward,
            EDITOR_MIN_YEAR,
            &catalog,
        ));
        assert!(!editor_action_available(
            EditorToolbarAction::DateForward,
            EDITOR_MAX_YEAR,
            &catalog,
        ));
    }

    #[test]
    fn road_and_tram_follow_available_catalog() {
        let catalog = openttdrs_core::vanilla_road_type_catalog();
        assert!(editor_action_available(
            EditorToolbarAction::Roads,
            1950,
            &catalog,
        ));
        assert!(editor_action_available(
            EditorToolbarAction::Trams,
            1950,
            &catalog,
        ));
        assert!(!editor_action_available(
            EditorToolbarAction::Roads,
            1950,
            &[],
        ));
        assert!(!editor_action_available(
            EditorToolbarAction::Trams,
            1950,
            &[],
        ));
    }

    #[test]
    fn editor_layout_switches_only_below_stable_width() {
        assert!(!editor_layout_is_compact(1_920.0, 1.0));
        assert!(!editor_layout_is_compact(1_250.0, 1.0));
        assert!(editor_layout_is_compact(1_249.0, 1.0));
        assert!(editor_layout_is_compact(1_920.0, 2.0));
        assert!(editor_layout_is_compact(800.0, 1.0));
    }

    #[test]
    fn dirty_document_requires_confirmation() {
        let mut document = EditorDocumentState::default();
        document.mark_saved();
        assert!(!document.is_dirty());
        document.mark_dirty();
        assert!(document.is_dirty());
    }
}
