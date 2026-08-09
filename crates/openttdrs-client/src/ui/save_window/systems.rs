//! Interacción de la ventana de partidas: lista, nombre, guardar/cargar/borrar.

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input_focus::{FocusCause, InputFocus};
use bevy::prelude::*;
use bevy::text::{EditableText, TextEdit};
// `TextEdit::Insert` espera el `SmolStr` de winit; usar su re-export evita
// depender de `smol_str` directo (y de que su versión coincida con la de winit).
use winit::keyboard::SmolStr;

use openttdrs_core::{sav, save};

use crate::persistence::apply_loaded_state;
use crate::render::{MapVisualLayer, RemapMapVisualsPending, ShoreTile, VehicleIndex, WaterTile};
use crate::state::SuspendedGameSession;
use crate::state::{ClientScreen, SimWorld};
use crate::ui::SimHudControls;
use crate::ui::main_menu::{MainMenuCamera, MainMenuUi, leave_main_menu};

use super::{
    SAVE_WINDOW_ROWS, SaveFileKind, SaveWindowButton, SaveWindowConfirmText, SaveWindowMode,
    SaveWindowNameRow, SaveWindowNameText, SaveWindowPageText, SaveWindowRoot, SaveWindowRow,
    SaveWindowRowText, SaveWindowState, SaveWindowStatusText, SaveWindowTitle, default_save_name,
    list_save_entries, sanitize_filename_char, save_dir_from,
};

/// Largo máximo del nombre al guardar.
const MAX_FILENAME_CHARS: usize = 40;

/// Escape cierra la ventana.
pub(crate) fn save_window_keyboard(
    mut state: ResMut<SaveWindowState>,
    keys: Res<ButtonInput<KeyCode>>,
) {
    if state.open && keys.just_pressed(KeyCode::Escape) {
        state.close();
    }
}

/// Entrada de teclado al campo nombre (`EditableTextInputPlugin` no está en crates.io 0.19).
pub(crate) fn save_window_editable_keyboard(
    state: Res<SaveWindowState>,
    mut key_events: MessageReader<KeyboardInput>,
    mut name_q: Query<&mut EditableText, With<SaveWindowNameText>>,
) {
    if !state.open || state.mode != SaveWindowMode::Save {
        key_events.clear();
        return;
    }
    let Ok(mut editable) = name_q.single_mut() else {
        key_events.clear();
        return;
    };
    for ev in key_events.read() {
        if ev.state != ButtonState::Pressed {
            continue;
        }
        if matches!(ev.logical_key, Key::Backspace) {
            editable.queue_edit(TextEdit::Backspace);
            continue;
        }
        if matches!(ev.logical_key, Key::Delete) {
            editable.queue_edit(TextEdit::Delete);
            continue;
        }
        let Some(text) = &ev.text else {
            continue;
        };
        for c in text.chars() {
            if sanitize_filename_char(c).is_some()
                && editable.value().chars().count() < MAX_FILENAME_CHARS
            {
                editable.queue_edit(TextEdit::Insert(SmolStr::from(c.to_string())));
            }
        }
    }
}

/// Clic en el campo de nombre mueve el foco de entrada.
pub(crate) fn save_window_name_click_focus(
    state: Res<SaveWindowState>,
    mut input_focus: ResMut<InputFocus>,
    q: Query<(Entity, &Interaction), (With<SaveWindowNameText>, Changed<Interaction>)>,
) {
    if !state.open || state.mode != SaveWindowMode::Save {
        return;
    }
    for (entity, interaction) in &q {
        if *interaction == Interaction::Pressed {
            input_focus.set(entity, FocusCause::Pressed);
        }
    }
}

/// Al abrir en modo guardar: foco en el campo de nombre y texto por defecto.
pub(crate) fn prepare_save_window_name(
    state: Res<SaveWindowState>,
    mut input_focus: ResMut<InputFocus>,
    mut name_q: Query<(Entity, &mut EditableText), With<SaveWindowNameText>>,
    mut last_open: Local<bool>,
) {
    let just_opened = state.open && !*last_open;
    *last_open = state.open;
    if !just_opened || state.mode != SaveWindowMode::Save {
        return;
    }
    let Ok((entity, mut editable)) = name_q.single_mut() else {
        return;
    };
    let name = if state.filename.is_empty() {
        default_save_name()
    } else {
        state.filename.clone()
    };
    editable.editor_mut().set_text(&name);
    editable.queue_edit(TextEdit::SelectAll);
    input_focus.set(entity, FocusCause::Navigated);
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn handle_save_window_buttons(
    mut state: ResMut<SaveWindowState>,
    buttons: Query<(&Interaction, &SaveWindowButton), (Changed<Interaction>, With<Button>)>,
    rows: Query<(&Interaction, &SaveWindowRow), (Changed<Interaction>, With<Button>)>,
    mut name_q: Query<&mut EditableText, With<SaveWindowNameText>>,
    mut hud: ResMut<SimHudControls>,
    mut sim: ResMut<SimWorld>,
    mut vehicle_index: ResMut<VehicleIndex>,
    mut remap: ResMut<RemapMapVisualsPending>,
    screen: Option<Res<State<ClientScreen>>>,
    mut next_screen: Option<ResMut<NextState<ClientScreen>>>,
    mut suspended: Option<ResMut<SuspendedGameSession>>,
    mut editor_document: Option<ResMut<crate::ui::toolbar::editor_toolbar::EditorDocumentState>>,
    q_menu: Query<Entity, With<MainMenuUi>>,
    q_menu_cam: Query<Entity, With<MainMenuCamera>>,
    intro_layers: Query<Entity, Or<(With<MapVisualLayer>, With<WaterTile>, With<ShoreTile>)>>,
    mut commands: Commands,
) {
    if !state.open {
        return;
    }

    let from_main_menu = screen
        .as_deref()
        .is_some_and(|s| *s.get() == ClientScreen::MainMenu);
    let mut loaded_from_menu = false;

    for (interaction, row) in &rows {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let idx = state.page * SAVE_WINDOW_ROWS + row.slot;
        if idx >= state.entries.len() {
            continue;
        }
        state.selected = Some(idx);
        if state.mode == SaveWindowMode::Save {
            let name = state.entries[idx].name.clone();
            let lower = name.to_ascii_lowercase();
            let stem = if lower.ends_with(".json") {
                &name[..name.len() - 5]
            } else if lower.ends_with(".sav") {
                &name[..name.len() - 4]
            } else {
                name.as_str()
            };
            state.filename = stem.to_string();
            if let Ok(mut editable) = name_q.single_mut() {
                editable.editor_mut().set_text(stem);
                editable.queue_edit(TextEdit::SelectAll);
            }
        }
    }

    for (interaction, button) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match button {
            SaveWindowButton::Cancel => state.close(),
            SaveWindowButton::PrevPage => {
                state.page = state.page.saturating_sub(1);
            }
            SaveWindowButton::NextPage => {
                let last = state.page_count() - 1;
                state.page = (state.page + 1).min(last);
            }
            SaveWindowButton::Delete => {
                let Some(idx) = state.selected else {
                    state.status = "Elegí una partida para borrar.".into();
                    continue;
                };
                let entry = state.entries[idx].clone();
                match std::fs::remove_file(&entry.path) {
                    Ok(()) => {
                        info!("Partida borrada: {}", entry.path.display());
                        state.status = format!("Borrada: {}", entry.name);
                        state.selected = None;
                        state.entries = list_save_entries(&save_dir_from(&hud.json_save_path));
                        let last = state.page_count() - 1;
                        state.page = state.page.min(last);
                    }
                    Err(e) => {
                        state.status = format!("No se pudo borrar {}: {e}", entry.name);
                    }
                }
            }
            SaveWindowButton::Confirm => match state.mode {
                SaveWindowMode::Save => {
                    let name_text = name_q
                        .single()
                        .ok()
                        .map(|e| e.value().to_string())
                        .unwrap_or_default();
                    if confirm_save(&mut state, &mut hud, &sim, &name_text)
                        && let Some(document) = editor_document.as_deref_mut()
                    {
                        document.mark_saved();
                    }
                }
                SaveWindowMode::Load => {
                    if confirm_load(
                        &mut state,
                        &mut hud,
                        &mut sim,
                        &mut vehicle_index,
                        &mut remap,
                        &mut commands,
                    ) && from_main_menu
                    {
                        loaded_from_menu = true;
                    }
                }
            },
        }
    }

    if loaded_from_menu && let Some(next) = next_screen.as_mut() {
        if let Some(suspended) = suspended.as_mut() {
            suspended.active = false;
        }
        leave_main_menu(
            &mut commands,
            &q_menu,
            &q_menu_cam,
            &intro_layers,
            next.as_mut(),
        );
    }
}

fn confirm_save(
    state: &mut SaveWindowState,
    hud: &mut SimHudControls,
    sim: &SimWorld,
    name: &str,
) -> bool {
    let name = name.trim();
    if name.is_empty() {
        state.status = "Escribí un nombre para la partida.".into();
        return false;
    }
    let dir = save_dir_from(&hud.json_save_path);
    if let Err(e) = std::fs::create_dir_all(&dir) {
        state.status = format!("No se pudo crear {}: {e}", dir.display());
        return false;
    }
    let lower = name.to_ascii_lowercase();
    let (file, as_json) = if lower.ends_with(".json") {
        (name.to_string(), true)
    } else if lower.ends_with(".sav") {
        (name.to_string(), false)
    } else {
        // Por defecto: formato OpenTTD `.sav` (mapa + DATE + PLYR).
        // Sufijo `.json` explícito conserva el save nativo completo.
        (format!("{name}.sav"), false)
    };
    let path = dir.join(file);
    let result = if as_json {
        save::save(&sim.state, &path).map_err(|e| e.to_string())
    } else {
        sav::save(&sim.state, &path).map_err(|e| e.to_string())
    };
    match result {
        Ok(()) => {
            let path_s = path.to_string_lossy().to_string();
            info!("Guardado en {path_s}");
            hud.json_save_path = path_s;
            state.close();
            true
        }
        Err(e) => {
            state.status = format!("No se pudo guardar: {e}");
            false
        }
    }
}

fn confirm_load(
    state: &mut SaveWindowState,
    hud: &mut SimHudControls,
    sim: &mut SimWorld,
    vehicle_index: &mut VehicleIndex,
    remap: &mut RemapMapVisualsPending,
    commands: &mut Commands,
) -> bool {
    let Some(idx) = state.selected else {
        state.status = "Elegí una partida para cargar.".into();
        return false;
    };
    let entry = state.entries[idx].clone();
    let loaded = match entry.kind {
        SaveFileKind::Json => match std::fs::read_to_string(&entry.path) {
            Ok(text) => match save::load_from_str(&text) {
                Ok(loaded) => loaded,
                Err(e) => {
                    state.status = format!("JSON inválido ({}): {e}", entry.name);
                    return false;
                }
            },
            Err(e) => {
                state.status = format!("No se pudo leer {}: {e}", entry.name);
                return false;
            }
        },
        SaveFileKind::Sav => match std::fs::read(&entry.path) {
            Ok(bytes) => match crate::state::load_sav_state(&bytes) {
                Ok(loaded) => loaded,
                Err(e) => {
                    state.status = format!("Save OpenTTD ({}): {e}", entry.name);
                    return false;
                }
            },
            Err(e) => {
                state.status = format!("No se pudo leer {}: {e}", entry.name);
                return false;
            }
        },
    };
    apply_loaded_state(sim, vehicle_index, remap, commands, loaded);
    hud.json_save_path = entry.path.to_string_lossy().to_string();
    info!("Partida cargada desde {}", entry.path.display());
    state.close();
    true
}

/// Refleja `SaveWindowState` en los nodos del modal.
#[allow(clippy::type_complexity)]
pub(crate) fn sync_save_window(
    state: Res<SaveWindowState>,
    mut root_q: Query<&mut Visibility, With<SaveWindowRoot>>,
    mut rows_q: Query<(&SaveWindowRow, &mut Node, &mut BackgroundColor)>,
    mut name_row_q: Query<&mut Node, (With<SaveWindowNameRow>, Without<SaveWindowRow>)>,
    mut texts: ParamSet<(
        Query<&mut Text, With<SaveWindowTitle>>,
        Query<(&SaveWindowRowText, &mut Text)>,
        Query<&mut Text, With<SaveWindowPageText>>,
        Query<&mut Text, With<SaveWindowStatusText>>,
        Query<&mut Text, With<SaveWindowConfirmText>>,
    )>,
) {
    if !state.is_changed() {
        return;
    }

    for mut vis in &mut root_q {
        *vis = if state.open {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
    if !state.open {
        return;
    }

    let (title, confirm) = match state.mode {
        SaveWindowMode::Save => ("Guardar partida", "Guardar"),
        SaveWindowMode::Load => ("Cargar partida", "Cargar"),
    };
    for mut t in &mut texts.p0() {
        **t = title.to_string();
    }
    for mut t in &mut texts.p4() {
        **t = confirm.to_string();
    }

    for (row, mut node, mut bg) in &mut rows_q {
        let idx = state.page * SAVE_WINDOW_ROWS + row.slot;
        if idx < state.entries.len() {
            node.display = Display::Flex;
            *bg = if state.selected == Some(idx) {
                BackgroundColor(Color::srgb(0.42, 0.36, 0.2))
            } else {
                BackgroundColor(Color::srgb(0.22, 0.18, 0.12))
            };
        } else {
            node.display = Display::None;
        }
    }
    for (row_text, mut t) in &mut texts.p1() {
        let idx = state.page * SAVE_WINDOW_ROWS + row_text.slot;
        if let Some(entry) = state.entries.get(idx) {
            **t = format!(
                "{:<30} {}  {:>9}",
                entry.name, entry.modified_label, entry.size_label
            );
        }
    }

    for mut node in &mut name_row_q {
        node.display = if state.mode == SaveWindowMode::Save {
            Display::Flex
        } else {
            Display::None
        };
    }
    for mut t in &mut texts.p2() {
        **t = format!("{}/{}", state.page + 1, state.page_count());
    }
    for mut t in &mut texts.p3() {
        **t = state.status.clone();
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use bevy::ecs::system::RunSystemOnce;
    use bevy::prelude::*;
    use bevy::text::EditableText;

    use crate::render::{RemapMapVisualsPending, VehicleIndex};
    use crate::state::SimWorld;
    use crate::ui::SimHudControls;

    use super::super::{
        SaveWindowButton, SaveWindowMode, SaveWindowNameText, SaveWindowRow, SaveWindowState,
        save_dir_from,
    };
    use super::handle_save_window_buttons;

    fn base_world(save_path: &str) -> World {
        let mut world = World::new();
        world.insert_resource(SimWorld::default());
        world.insert_resource(VehicleIndex::default());
        world.insert_resource(RemapMapVisualsPending::default());
        world.insert_resource(SaveWindowState::default());
        world.insert_resource(SimHudControls {
            sim_speed: 1.0,
            json_save_path: save_path.to_string(),
            minimap_visible: true,
            sfx_volume: 0.22,
            ..Default::default()
        });
        world.spawn((SaveWindowNameText, EditableText::new("mi_partida")));
        world
    }

    fn press(world: &mut World, button: SaveWindowButton) {
        world.spawn((Button, button, Interaction::Pressed));
    }

    #[test]
    fn save_then_load_roundtrip_via_window() {
        let dir = tempfile::tempdir().expect("tempdir");
        let save_path = dir.path().join("x.json").to_string_lossy().to_string();
        let mut world = base_world(&save_path);

        world
            .resource_mut::<SaveWindowState>()
            .open_in_mode(SaveWindowMode::Save, &save_dir_from(&save_path));
        press(&mut world, SaveWindowButton::Confirm);
        world.run_system_once(handle_save_window_buttons).unwrap();

        assert!(dir.path().join("mi_partida.sav").exists());
        assert!(!world.resource::<SaveWindowState>().open);

        world
            .resource_mut::<SaveWindowState>()
            .open_in_mode(SaveWindowMode::Load, &save_dir_from(&save_path));
        assert_eq!(world.resource::<SaveWindowState>().entries.len(), 1);
        world.spawn((Button, SaveWindowRow { slot: 0 }, Interaction::Pressed));
        press(&mut world, SaveWindowButton::Confirm);
        world.run_system_once(handle_save_window_buttons).unwrap();

        assert!(!world.resource::<SaveWindowState>().open);
        assert!(world.resource::<RemapMapVisualsPending>().pending);
    }

    #[test]
    fn save_json_when_extension_explicit() {
        let dir = tempfile::tempdir().expect("tempdir");
        let save_path = dir.path().join("x.json").to_string_lossy().to_string();
        let mut world = World::new();
        world.insert_resource(SimWorld::default());
        world.insert_resource(VehicleIndex::default());
        world.insert_resource(RemapMapVisualsPending::default());
        world.insert_resource(SaveWindowState::default());
        world.insert_resource(SimHudControls {
            sim_speed: 1.0,
            json_save_path: save_path.clone(),
            minimap_visible: true,
            sfx_volume: 0.22,
            ..Default::default()
        });
        world.spawn((SaveWindowNameText, EditableText::new("mi_partida.json")));
        world
            .resource_mut::<SaveWindowState>()
            .open_in_mode(SaveWindowMode::Save, &save_dir_from(&save_path));
        press(&mut world, SaveWindowButton::Confirm);
        world.run_system_once(handle_save_window_buttons).unwrap();
        assert!(dir.path().join("mi_partida.json").exists());
        assert!(!dir.path().join("mi_partida.sav").exists());
    }

    #[test]
    fn delete_removes_selected_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("vieja.json"), "{}").unwrap();
        let save_path = dir.path().join("x.json").to_string_lossy().to_string();
        let mut world = base_world(&save_path);

        world
            .resource_mut::<SaveWindowState>()
            .open_in_mode(SaveWindowMode::Load, &save_dir_from(&save_path));
        world.resource_mut::<SaveWindowState>().selected = Some(0);
        press(&mut world, SaveWindowButton::Delete);
        world.run_system_once(handle_save_window_buttons).unwrap();

        assert!(!dir.path().join("vieja.json").exists());
        assert!(world.resource::<SaveWindowState>().entries.is_empty());
    }

    #[test]
    fn confirm_save_requires_name() {
        let dir = tempfile::tempdir().expect("tempdir");
        let save_path = dir.path().join("x.json").to_string_lossy().to_string();
        let mut world = base_world(&save_path);
        {
            let mut q = world.query::<&mut EditableText>();
            q.single_mut(&mut world).unwrap().editor_mut().set_text("");
        }

        world
            .resource_mut::<SaveWindowState>()
            .open_in_mode(SaveWindowMode::Save, &save_dir_from(&save_path));
        press(&mut world, SaveWindowButton::Confirm);
        world.run_system_once(handle_save_window_buttons).unwrap();

        let state = world.resource::<SaveWindowState>();
        assert!(state.open);
        assert!(!state.status.is_empty());
    }
}
