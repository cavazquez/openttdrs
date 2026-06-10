//! Interacción de la ventana de partidas: lista, nombre, guardar/cargar/borrar.

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;

use openttdrs_core::save;

use crate::persistence::apply_loaded_state;
use crate::render::{RemapMapVisualsPending, VehicleIndex};
use crate::state::SimWorld;
use crate::ui::SimHudControls;

use super::{
    SAVE_WINDOW_ROWS, SaveFileKind, SaveWindowButton, SaveWindowConfirmText, SaveWindowMode,
    SaveWindowNameRow, SaveWindowNameText, SaveWindowPageText, SaveWindowRoot, SaveWindowRow,
    SaveWindowRowText, SaveWindowState, SaveWindowStatusText, SaveWindowTitle, list_save_entries,
    sanitize_filename_char, save_dir_from,
};

/// Largo máximo del nombre al guardar.
const MAX_FILENAME_CHARS: usize = 40;

/// Botón de la barra superior que abre la ventana en un modo dado.
#[derive(Component, Clone, Copy)]
pub(crate) struct SaveLoadToolbarButton(pub(crate) SaveWindowMode);

pub(crate) fn handle_save_load_toolbar_buttons(
    q: Query<(&Interaction, &SaveLoadToolbarButton), (Changed<Interaction>, With<Button>)>,
    mut state: ResMut<SaveWindowState>,
    hud: Res<SimHudControls>,
) {
    for (interaction, button) in &q {
        if *interaction == Interaction::Pressed {
            state.open_in_mode(button.0, &save_dir_from(&hud.json_save_path));
        }
    }
}

/// Escape cierra; en modo guardar el teclado edita el nombre del archivo.
pub(crate) fn save_window_keyboard(
    mut state: ResMut<SaveWindowState>,
    keys: Res<ButtonInput<KeyCode>>,
    mut key_events: MessageReader<KeyboardInput>,
) {
    if !state.open {
        key_events.clear();
        return;
    }
    if keys.just_pressed(KeyCode::Escape) {
        state.close();
        key_events.clear();
        return;
    }
    if state.mode != SaveWindowMode::Save {
        key_events.clear();
        return;
    }
    for ev in key_events.read() {
        if ev.state != ButtonState::Pressed {
            continue;
        }
        if matches!(ev.logical_key, Key::Backspace) {
            state.filename.pop();
            continue;
        }
        let Some(text) = &ev.text else {
            continue;
        };
        for c in text.chars() {
            if let Some(c) = sanitize_filename_char(c)
                && state.filename.chars().count() < MAX_FILENAME_CHARS
            {
                state.filename.push(c);
            }
        }
    }
}

pub(crate) fn handle_save_window_buttons(
    mut state: ResMut<SaveWindowState>,
    buttons: Query<(&Interaction, &SaveWindowButton), (Changed<Interaction>, With<Button>)>,
    rows: Query<(&Interaction, &SaveWindowRow), (Changed<Interaction>, With<Button>)>,
    mut hud: ResMut<SimHudControls>,
    mut sim: ResMut<SimWorld>,
    mut vehicle_index: ResMut<VehicleIndex>,
    mut remap: ResMut<RemapMapVisualsPending>,
) {
    if !state.open {
        return;
    }

    for (interaction, row) in &rows {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let idx = state.page * SAVE_WINDOW_ROWS + row.slot;
        if idx >= state.entries.len() {
            continue;
        }
        state.selected = Some(idx);
        if state.mode == SaveWindowMode::Save && state.entries[idx].kind == SaveFileKind::Json {
            let name = state.entries[idx].name.clone();
            state.filename = name.trim_end_matches(".json").to_string();
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
                    confirm_save(&mut state, &mut hud, &sim);
                }
                SaveWindowMode::Load => {
                    confirm_load(
                        &mut state,
                        &mut hud,
                        &mut sim,
                        &mut vehicle_index,
                        &mut remap,
                    );
                }
            },
        }
    }
}

fn confirm_save(state: &mut SaveWindowState, hud: &mut SimHudControls, sim: &SimWorld) {
    let name = state.filename.trim();
    if name.is_empty() {
        state.status = "Escribí un nombre para la partida.".into();
        return;
    }
    let dir = save_dir_from(&hud.json_save_path);
    if let Err(e) = std::fs::create_dir_all(&dir) {
        state.status = format!("No se pudo crear {}: {e}", dir.display());
        return;
    }
    let file = if name.to_ascii_lowercase().ends_with(".json") {
        name.to_string()
    } else {
        format!("{name}.json")
    };
    let path = dir.join(file);
    match save::save(&sim.state, &path) {
        Ok(()) => {
            let path_s = path.to_string_lossy().to_string();
            info!("Guardado en {path_s}");
            hud.json_save_path = path_s;
            state.close();
        }
        Err(e) => {
            state.status = format!("No se pudo guardar: {e}");
        }
    }
}

fn confirm_load(
    state: &mut SaveWindowState,
    hud: &mut SimHudControls,
    sim: &mut SimWorld,
    vehicle_index: &mut VehicleIndex,
    remap: &mut RemapMapVisualsPending,
) {
    let Some(idx) = state.selected else {
        state.status = "Elegí una partida para cargar.".into();
        return;
    };
    let entry = state.entries[idx].clone();
    let loaded = match entry.kind {
        SaveFileKind::Json => match std::fs::read_to_string(&entry.path) {
            Ok(text) => match save::load_from_str(&text) {
                Ok(loaded) => loaded,
                Err(e) => {
                    state.status = format!("JSON inválido ({}): {e}", entry.name);
                    return;
                }
            },
            Err(e) => {
                state.status = format!("No se pudo leer {}: {e}", entry.name);
                return;
            }
        },
        SaveFileKind::Sav => match std::fs::read(&entry.path) {
            Ok(bytes) => match crate::state::load_sav_state(&bytes) {
                Ok(loaded) => loaded,
                Err(e) => {
                    state.status = format!("Save OpenTTD ({}): {e}", entry.name);
                    return;
                }
            },
            Err(e) => {
                state.status = format!("No se pudo leer {}: {e}", entry.name);
                return;
            }
        },
    };
    apply_loaded_state(sim, vehicle_index, remap, loaded);
    if entry.kind == SaveFileKind::Json {
        hud.json_save_path = entry.path.to_string_lossy().to_string();
    }
    info!("Partida cargada desde {}", entry.path.display());
    state.close();
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
        Query<&mut Text, With<SaveWindowNameText>>,
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
    for mut t in &mut texts.p5() {
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
        **t = format!("{}.json", state.filename);
    }
    for mut t in &mut texts.p3() {
        **t = format!("{}/{}", state.page + 1, state.page_count());
    }
    for mut t in &mut texts.p4() {
        **t = state.status.clone();
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use bevy::ecs::system::RunSystemOnce;
    use bevy::prelude::*;

    use crate::render::{RemapMapVisualsPending, VehicleIndex};
    use crate::state::SimWorld;
    use crate::ui::SimHudControls;

    use super::super::{
        SaveWindowButton, SaveWindowMode, SaveWindowRow, SaveWindowState, save_dir_from,
    };
    use super::handle_save_window_buttons;

    fn base_world(save_path: &str) -> World {
        let mut world = World::new();
        world.insert_resource(SimWorld::default());
        world.insert_resource(VehicleIndex::default());
        world.insert_resource(RemapMapVisualsPending::default());
        world.insert_resource(SaveWindowState::default());
        world.insert_resource(SimHudControls {
            paused: false,
            sim_speed: 1.0,
            json_save_path: save_path.to_string(),
            minimap_visible: true,
        });
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
        world.resource_mut::<SaveWindowState>().filename = "mi_partida".into();
        press(&mut world, SaveWindowButton::Confirm);
        world.run_system_once(handle_save_window_buttons).unwrap();

        assert!(dir.path().join("mi_partida.json").exists());
        assert!(!world.resource::<SaveWindowState>().open);

        // Cargar: abrir, seleccionar fila 0 y confirmar.
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

        world
            .resource_mut::<SaveWindowState>()
            .open_in_mode(SaveWindowMode::Save, &save_dir_from(&save_path));
        world.resource_mut::<SaveWindowState>().filename = String::new();
        press(&mut world, SaveWindowButton::Confirm);
        world.run_system_once(handle_save_window_buttons).unwrap();

        let state = world.resource::<SaveWindowState>();
        assert!(state.open);
        assert!(!state.status.is_empty());
    }
}
