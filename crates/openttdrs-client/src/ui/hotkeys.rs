//! Registro central de comandos UI y bindings configurables (#236).

use std::collections::{HashMap, HashSet};

use bevy::input_focus::InputFocus;
use bevy::prelude::*;
use bevy::text::EditableText;

use crate::settings::ClientPreferences;
use crate::ui::save_window::SaveWindowState;
use crate::ui::toolbar::editor_toolbar::EditorExitConfirmRoot;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum UiCommandId {
    Pause,
    Settings,
    SaveLoad,
    SmallMap,
    TownDirectory,
    StationList,
    Finances,
    Graphs,
    League,
    IndustryDirectory,
    BuildRail,
    BuildRoad,
    BuildWater,
    BuildAir,
    Terraform,
    BuildTrees,
    Music,
    Help,
    ZoomIn,
    ZoomOut,
    ExtraViewport,
    Screenshot,
    Cheats,
    ToggleReservations,
    ToggleHud,
    CycleSavePath,
    RoadY,
    RoadX,
    Station,
    Clear,
}

impl UiCommandId {
    pub(crate) const ALL: &[Self] = &[
        Self::Pause,
        Self::Settings,
        Self::SaveLoad,
        Self::SmallMap,
        Self::TownDirectory,
        Self::StationList,
        Self::Finances,
        Self::Graphs,
        Self::League,
        Self::IndustryDirectory,
        Self::BuildRail,
        Self::BuildRoad,
        Self::BuildWater,
        Self::BuildAir,
        Self::Terraform,
        Self::BuildTrees,
        Self::Music,
        Self::Help,
        Self::ZoomIn,
        Self::ZoomOut,
        Self::ExtraViewport,
        Self::Screenshot,
        Self::Cheats,
        Self::ToggleReservations,
        Self::ToggleHud,
        Self::CycleSavePath,
        Self::RoadY,
        Self::RoadX,
        Self::Station,
        Self::Clear,
    ];

    pub(crate) const fn stable_id(self) -> &'static str {
        match self {
            Self::Pause => "pause",
            Self::Settings => "settings",
            Self::SaveLoad => "save_load",
            Self::SmallMap => "small_map",
            Self::TownDirectory => "town_directory",
            Self::StationList => "station_list",
            Self::Finances => "finances",
            Self::Graphs => "graphs",
            Self::League => "league",
            Self::IndustryDirectory => "industry_directory",
            Self::BuildRail => "build_rail",
            Self::BuildRoad => "build_road",
            Self::BuildWater => "build_water",
            Self::BuildAir => "build_air",
            Self::Terraform => "terraform",
            Self::BuildTrees => "build_trees",
            Self::Music => "music",
            Self::Help => "help",
            Self::ZoomIn => "zoom_in",
            Self::ZoomOut => "zoom_out",
            Self::ExtraViewport => "extra_viewport",
            Self::Screenshot => "screenshot",
            Self::Cheats => "cheats",
            Self::ToggleReservations => "toggle_reservations",
            Self::ToggleHud => "toggle_hud",
            Self::CycleSavePath => "cycle_save_path",
            Self::RoadY => "road_y",
            Self::RoadX => "road_x",
            Self::Station => "station",
            Self::Clear => "clear",
        }
    }

    fn from_stable_id(id: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .copied()
            .find(|command| command.stable_id() == id)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct HotkeyBinding {
    pub key: KeyCode,
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
}

impl HotkeyBinding {
    const fn plain(key: KeyCode) -> Self {
        Self {
            key,
            shift: false,
            ctrl: false,
            alt: false,
        }
    }
    const fn shift(key: KeyCode) -> Self {
        Self {
            key,
            shift: true,
            ctrl: false,
            alt: false,
        }
    }
    const fn ctrl(key: KeyCode) -> Self {
        Self {
            key,
            shift: false,
            ctrl: true,
            alt: false,
        }
    }
    const fn ctrl_alt(key: KeyCode) -> Self {
        Self {
            key,
            shift: false,
            ctrl: true,
            alt: true,
        }
    }

    pub(crate) fn label(self) -> String {
        let mut parts = Vec::new();
        if self.ctrl {
            parts.push("Ctrl");
        }
        if self.alt {
            parts.push("Alt");
        }
        if self.shift {
            parts.push("Shift");
        }
        parts.push(key_label(self.key));
        parts.join("+")
    }

    fn matches(self, keyboard: &ButtonInput<KeyCode>) -> bool {
        let shift = keyboard.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
        let ctrl = keyboard.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]);
        let alt = keyboard.any_pressed([KeyCode::AltLeft, KeyCode::AltRight]);
        keyboard.just_pressed(self.key) && (shift, ctrl, alt) == (self.shift, self.ctrl, self.alt)
    }
}

#[derive(Resource, Default)]
pub(crate) struct UiHotkeys {
    bindings: HashMap<UiCommandId, HotkeyBinding>,
    fired: HashSet<UiCommandId>,
    pub(crate) conflicts: Vec<(UiCommandId, UiCommandId, HotkeyBinding)>,
    loaded_overrides: String,
}

impl UiHotkeys {
    pub(crate) fn fired(&self, command: UiCommandId) -> bool {
        self.fired.contains(&command)
    }
    pub(crate) fn label(&self, command: UiCommandId) -> Option<String> {
        self.bindings
            .get(&command)
            .copied()
            .map(HotkeyBinding::label)
    }
}

fn defaults() -> HashMap<UiCommandId, HotkeyBinding> {
    use UiCommandId as C;
    [
        (C::Pause, HotkeyBinding::plain(KeyCode::F1)),
        (C::Settings, HotkeyBinding::plain(KeyCode::F2)),
        (C::SaveLoad, HotkeyBinding::plain(KeyCode::F3)),
        (C::SmallMap, HotkeyBinding::plain(KeyCode::F4)),
        (C::TownDirectory, HotkeyBinding::plain(KeyCode::F5)),
        (C::StationList, HotkeyBinding::plain(KeyCode::F7)),
        (C::Finances, HotkeyBinding::plain(KeyCode::F8)),
        (C::Graphs, HotkeyBinding::plain(KeyCode::F10)),
        (C::League, HotkeyBinding::plain(KeyCode::F11)),
        (C::IndustryDirectory, HotkeyBinding::plain(KeyCode::F12)),
        (C::BuildRail, HotkeyBinding::shift(KeyCode::F1)),
        (C::BuildRoad, HotkeyBinding::shift(KeyCode::F2)),
        (C::BuildWater, HotkeyBinding::shift(KeyCode::F3)),
        (C::BuildAir, HotkeyBinding::shift(KeyCode::F4)),
        (C::Terraform, HotkeyBinding::shift(KeyCode::F5)),
        (C::BuildTrees, HotkeyBinding::shift(KeyCode::F6)),
        (C::Music, HotkeyBinding::shift(KeyCode::F11)),
        (C::Help, HotkeyBinding::shift(KeyCode::F12)),
        (C::ZoomIn, HotkeyBinding::plain(KeyCode::Equal)),
        (C::ZoomOut, HotkeyBinding::plain(KeyCode::Minus)),
        (C::ExtraViewport, HotkeyBinding::ctrl(KeyCode::KeyV)),
        (C::Screenshot, HotkeyBinding::ctrl(KeyCode::KeyS)),
        (C::Cheats, HotkeyBinding::ctrl_alt(KeyCode::KeyC)),
        (C::ToggleReservations, HotkeyBinding::plain(KeyCode::KeyR)),
        (C::ToggleHud, HotkeyBinding::ctrl(KeyCode::KeyH)),
        (C::CycleSavePath, HotkeyBinding::ctrl(KeyCode::F4)),
        (C::RoadY, HotkeyBinding::plain(KeyCode::Digit1)),
        (C::RoadX, HotkeyBinding::plain(KeyCode::Digit2)),
        (C::Station, HotkeyBinding::plain(KeyCode::Digit3)),
        (C::Clear, HotkeyBinding::plain(KeyCode::KeyC)),
    ]
    .into_iter()
    .collect()
}

fn parse_overrides(text: &str) -> Vec<(UiCommandId, HotkeyBinding)> {
    text.split(';')
        .filter_map(|entry| {
            let (id, binding) = entry.trim().split_once('=')?;
            Some((
                UiCommandId::from_stable_id(id.trim())?,
                parse_binding(binding.trim())?,
            ))
        })
        .collect()
}

fn parse_binding(text: &str) -> Option<HotkeyBinding> {
    let mut binding = HotkeyBinding::plain(KeyCode::F1);
    let mut key = None;
    for part in text.split('+') {
        match part.trim().to_ascii_lowercase().as_str() {
            "shift" => binding.shift = true,
            "ctrl" | "control" => binding.ctrl = true,
            "alt" => binding.alt = true,
            value => key = parse_key(value),
        }
    }
    binding.key = key?;
    Some(binding)
}

fn parse_key(key: &str) -> Option<KeyCode> {
    Some(match key {
        "f1" => KeyCode::F1,
        "f2" => KeyCode::F2,
        "f3" => KeyCode::F3,
        "f4" => KeyCode::F4,
        "f5" => KeyCode::F5,
        "f6" => KeyCode::F6,
        "f7" => KeyCode::F7,
        "f8" => KeyCode::F8,
        "f9" => KeyCode::F9,
        "f10" => KeyCode::F10,
        "f11" => KeyCode::F11,
        "f12" => KeyCode::F12,
        "1" => KeyCode::Digit1,
        "2" => KeyCode::Digit2,
        "3" => KeyCode::Digit3,
        "4" => KeyCode::Digit4,
        "c" => KeyCode::KeyC,
        "h" => KeyCode::KeyH,
        "r" => KeyCode::KeyR,
        "v" => KeyCode::KeyV,
        "s" => KeyCode::KeyS,
        "=" | "+" => KeyCode::Equal,
        "-" => KeyCode::Minus,
        _ => return None,
    })
}

fn key_label(key: KeyCode) -> &'static str {
    match key {
        KeyCode::F1 => "F1",
        KeyCode::F2 => "F2",
        KeyCode::F3 => "F3",
        KeyCode::F4 => "F4",
        KeyCode::F5 => "F5",
        KeyCode::F6 => "F6",
        KeyCode::F7 => "F7",
        KeyCode::F8 => "F8",
        KeyCode::F9 => "F9",
        KeyCode::F10 => "F10",
        KeyCode::F11 => "F11",
        KeyCode::F12 => "F12",
        KeyCode::Digit1 => "1",
        KeyCode::Digit2 => "2",
        KeyCode::Digit3 => "3",
        KeyCode::Digit4 => "4",
        KeyCode::KeyC => "C",
        KeyCode::KeyH => "H",
        KeyCode::KeyR => "R",
        KeyCode::KeyV => "V",
        KeyCode::KeyS => "S",
        KeyCode::Equal => "+",
        KeyCode::Minus => "−",
        _ => "?",
    }
}

fn rebuild_bindings(hotkeys: &mut UiHotkeys, overrides: &str) {
    hotkeys.bindings = defaults();
    hotkeys.conflicts.clear();
    for (command, binding) in parse_overrides(overrides) {
        if let Some((&other, _)) = hotkeys
            .bindings
            .iter()
            .find(|(other, candidate)| **other != command && **candidate == binding)
        {
            hotkeys.conflicts.push((command, other, binding));
            continue;
        }
        hotkeys.bindings.insert(command, binding);
    }
    hotkeys.loaded_overrides = overrides.to_string();
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn dispatch_ui_hotkeys(
    keyboard: Res<ButtonInput<KeyCode>>,
    preferences: Res<ClientPreferences>,
    focus: Option<Res<InputFocus>>,
    editable: Query<(), With<EditableText>>,
    save_window: Option<Res<SaveWindowState>>,
    console: Option<Res<crate::ui::dev_console::DevConsoleState>>,
    network: Option<Res<crate::network::NetworkRuntime>>,
    exit_modal: Query<&Node, With<EditorExitConfirmRoot>>,
    mut hotkeys: ResMut<UiHotkeys>,
) {
    if hotkeys.bindings.is_empty() || hotkeys.loaded_overrides != preferences.toolbar_hotkeys {
        rebuild_bindings(&mut hotkeys, &preferences.toolbar_hotkeys);
    }
    hotkeys.fired.clear();
    let text_focused = focus
        .as_deref()
        .and_then(InputFocus::get)
        .is_some_and(|entity| editable.get(entity).is_ok());
    let captured = text_focused
        || save_window.as_deref().is_some_and(|window| window.open)
        || console
            .as_deref()
            .is_some_and(crate::ui::dev_console::dev_console_captures_keyboard)
        || exit_modal.iter().any(|node| node.display != Display::None);
    if captured {
        return;
    }
    let client_only = network
        .as_deref()
        .is_some_and(|runtime| runtime.role() == crate::network::NetworkRole::Client);
    let fired = hotkeys
        .bindings
        .iter()
        .filter_map(|(command, binding)| {
            (binding.matches(&keyboard) && !(client_only && *command == UiCommandId::Pause))
                .then_some(*command)
        })
        .collect::<Vec<_>>();
    hotkeys.fired.extend(fired);
}

pub(crate) fn handle_toolbar_command_hotkeys(
    hotkeys: Res<UiHotkeys>,
    editor: Option<Res<crate::state::EditorSession>>,
    mut routes: MessageWriter<crate::ui::navigation::OpenUiRoute>,
    mut toolbar: ResMut<crate::ui::toolbar::ToolbarState>,
    mut tool: ResMut<crate::ui::toolbar::UiToolState>,
    mut extra_viewport: ResMut<crate::ui::extra_viewport_window::ExtraViewportWindowState>,
    mut commands: Commands,
) {
    use crate::ui::navigation::UiRoute;
    use crate::ui::toolbar::{BuildMenuAction, ToolbarGroup};
    use UiCommandId as C;

    let in_editor = editor.as_deref().is_some_and(|session| session.active);
    for (command, route) in [
        (C::Settings, UiRoute::DisplayOptions),
        (C::TownDirectory, UiRoute::Towns),
        (C::StationList, UiRoute::Stations),
        (C::Finances, UiRoute::Finances),
        (
            C::Graphs,
            UiRoute::Graph(crate::ui::graph_window::GraphKind::Income),
        ),
        (C::League, UiRoute::League),
        (C::IndustryDirectory, UiRoute::Industries),
        (C::Music, UiRoute::SoundMusic),
        (C::Help, UiRoute::Help),
        (C::Cheats, UiRoute::Cheats),
    ] {
        if hotkeys.fired(command) {
            routes.write(crate::ui::navigation::OpenUiRoute(route));
        }
    }
    if hotkeys.fired(C::SaveLoad) {
        routes.write(crate::ui::navigation::OpenUiRoute(if in_editor {
            UiRoute::EditorSaveScenario
        } else {
            UiRoute::SaveGame
        }));
    }
    if hotkeys.fired(C::ExtraViewport) {
        extra_viewport.open = true;
    }
    if hotkeys.fired(C::Screenshot) {
        use bevy::render::view::screenshot::{Screenshot, save_to_disk};
        let dir = std::path::PathBuf::from("screenshots");
        let _ = std::fs::create_dir_all(&dir);
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |duration| duration.as_secs());
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(dir.join(format!("openttdrs-{timestamp}.png"))));
    }
    for (command, group) in [
        (C::BuildRail, ToolbarGroup::Rail),
        (C::BuildRoad, ToolbarGroup::Road),
        (C::BuildWater, ToolbarGroup::Water),
        (C::BuildAir, ToolbarGroup::Air),
        (C::Terraform, ToolbarGroup::Landscape),
    ] {
        if hotkeys.fired(command) {
            toolbar.active_group = Some(group);
            tool.active_tool = None;
        }
    }
    if hotkeys.fired(C::BuildTrees) {
        toolbar.active_group = Some(ToolbarGroup::Landscape);
        tool.active_tool = Some(BuildMenuAction::PlantTree);
    }
}

pub(crate) fn handle_zoom_hotkeys(
    hotkeys: Res<UiHotkeys>,
    sim: Option<Res<crate::state::SimWorld>>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    mut cameras: Query<
        &mut Projection,
        (
            With<crate::render::PrimaryGameCamera>,
            Without<crate::render::MapPreviewCamera>,
        ),
    >,
) {
    let factor = if hotkeys.fired(UiCommandId::ZoomIn) {
        Some(0.85)
    } else if hotkeys.fired(UiCommandId::ZoomOut) {
        Some(1.15)
    } else {
        None
    };
    let Some(factor) = factor else {
        return;
    };
    let Ok(mut projection) = cameras.single_mut() else {
        return;
    };
    let Projection::Orthographic(orthographic) = &mut *projection else {
        return;
    };
    let (map_width, map_height) = sim
        .as_deref()
        .map_or((64, 64), |sim| sim.state.map.dimensions());
    let large = crate::render::large_map_viewport_cull_enabled(map_width, map_height);
    let (width, height) = windows
        .iter()
        .next()
        .map_or((1280.0, 720.0), |window| (window.width(), window.height()));
    orthographic.scale =
        crate::render::clamp_ortho_scale(orthographic.scale * factor, width, height, large);
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    #[test]
    fn modifiers_distinguish_f1_from_shift_f1() {
        assert_ne!(
            defaults()[&UiCommandId::Pause],
            defaults()[&UiCommandId::BuildRail]
        );
    }

    #[test]
    fn overrides_are_parsed_and_conflicts_rejected() {
        let mut hotkeys = UiHotkeys::default();
        rebuild_bindings(&mut hotkeys, "pause=Ctrl+F9;settings=Ctrl+F9");
        assert_eq!(
            hotkeys.label(UiCommandId::Pause).as_deref(),
            Some("Ctrl+F9")
        );
        assert_eq!(hotkeys.conflicts.len(), 1);
        assert_ne!(
            hotkeys.bindings[&UiCommandId::Settings],
            HotkeyBinding::ctrl(KeyCode::F9)
        );
    }

    #[test]
    fn focused_text_has_priority_over_toolbar_binding() {
        let mut world = World::new();
        let focused = world.spawn(EditableText::new("")).id();
        world.insert_resource(InputFocus::from_entity(focused));
        world.insert_resource(ClientPreferences::default());
        world.insert_resource(UiHotkeys::default());
        let mut keyboard = ButtonInput::<KeyCode>::default();
        keyboard.press(KeyCode::F1);
        world.insert_resource(keyboard);

        world.run_system_once(dispatch_ui_hotkeys).unwrap();

        assert!(!world.resource::<UiHotkeys>().fired(UiCommandId::Pause));
    }
}
