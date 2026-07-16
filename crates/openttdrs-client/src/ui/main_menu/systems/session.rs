use bevy::prelude::*;

use crate::render::{MapVisualLayer, ShoreTile, WaterTile};
use crate::state::bootstrap::NewGameSettings;
use crate::state::{
    ClientScreen, EditorSession, SimWorld, SuspendedGameSession, apply_editor_sandbox,
    editor_new_game_settings,
};
use crate::ui::main_menu_intro::despawn_main_menu_intro_layers;

use super::super::{MainMenuCamera, MainMenuUi};

pub(crate) fn leave_main_menu(
    commands: &mut Commands,
    q_menu: &Query<Entity, With<MainMenuUi>>,
    q_menu_cam: &Query<Entity, With<MainMenuCamera>>,
    intro_layers: &Query<Entity, Or<(With<MapVisualLayer>, With<WaterTile>, With<ShoreTile>)>>,
    next_screen: &mut NextState<ClientScreen>,
) {
    despawn_main_menu_intro_layers(commands, intro_layers);
    for e in q_menu {
        commands.entity(e).despawn();
    }
    for cam in q_menu_cam {
        commands.entity(cam).despawn();
    }
    next_screen.set(ClientScreen::InGame);
}

/// Vuelve al menú principal; `OnExit(InGame)` desmonta la sesión en curso.
pub(crate) fn return_to_main_menu(
    next_screen: &mut NextState<ClientScreen>,
    suspended: &mut SuspendedGameSession,
) {
    suspended.active = true;
    // `suspended.editor` lo rellena `leave_ingame` al salir de InGame.
    info!("Volviendo al menu principal (partida suspendida)");
    next_screen.set(ClientScreen::MainMenu);
}

/// Reanuda la partida suspendida sin reemplazar `SimWorld`.
pub(crate) fn resume_suspended_game(
    commands: &mut Commands,
    q_menu: &Query<Entity, With<MainMenuUi>>,
    q_menu_cam: &Query<Entity, With<MainMenuCamera>>,
    intro_layers: &Query<Entity, Or<(With<MapVisualLayer>, With<WaterTile>, With<ShoreTile>)>>,
    next_screen: &mut NextState<ClientScreen>,
    suspended: &mut SuspendedGameSession,
) {
    let was_editor = suspended.editor;
    suspended.active = false;
    suspended.editor = false;
    commands.insert_resource(if was_editor {
        EditorSession::active()
    } else {
        EditorSession::inactive()
    });
    info!("Continuando partida suspendida");
    leave_main_menu(commands, q_menu, q_menu_cam, intro_layers, next_screen);
}

/// Salta el menú si el arranque cargó un JSON vía `OTTDJSON_LOAD` (escenarios `dev_bot`).
pub(crate) fn auto_start_preloaded_json(
    sim: Res<SimWorld>,
    mut commands: Commands,
    q_menu: Query<Entity, With<MainMenuUi>>,
    q_menu_cam: Query<Entity, With<MainMenuCamera>>,
    intro_layers: Query<Entity, Or<(With<MapVisualLayer>, With<WaterTile>, With<ShoreTile>)>>,
    mut next_screen: ResMut<NextState<ClientScreen>>,
    mut done: Local<bool>,
) {
    if *done || !sim.loaded_file || std::env::var_os("OTTDJSON_LOAD").is_none() {
        return;
    }
    *done = true;
    leave_main_menu(
        &mut commands,
        &q_menu,
        &q_menu_cam,
        &intro_layers,
        &mut next_screen,
    );
}

pub(in crate::ui::main_menu) fn enter_new_game(
    commands: &mut Commands,
    q_menu: &Query<Entity, With<MainMenuUi>>,
    q_menu_cam: &Query<Entity, With<MainMenuCamera>>,
    intro_layers: &Query<Entity, Or<(With<MapVisualLayer>, With<WaterTile>, With<ShoreTile>)>>,
    settings: NewGameSettings,
    next_screen: &mut NextState<ClientScreen>,
    suspended: &mut SuspendedGameSession,
) {
    suspended.active = false;
    commands.insert_resource(EditorSession::inactive());
    commands.insert_resource(SimWorld::from_new_game(&settings.sanitized()));
    leave_main_menu(commands, q_menu, q_menu_cam, intro_layers, next_screen);
}

pub(in crate::ui::main_menu) fn enter_editor(
    commands: &mut Commands,
    q_menu: &Query<Entity, With<MainMenuUi>>,
    q_menu_cam: &Query<Entity, With<MainMenuCamera>>,
    intro_layers: &Query<Entity, Or<(With<MapVisualLayer>, With<WaterTile>, With<ShoreTile>)>>,
    next_screen: &mut NextState<ClientScreen>,
    suspended: &mut SuspendedGameSession,
) {
    suspended.active = false;
    let mut sim = SimWorld::from_new_game(&editor_new_game_settings().sanitized());
    apply_editor_sandbox(&mut sim);
    commands.insert_resource(sim);
    commands.insert_resource(EditorSession::active());
    info!("Editor de escenarios: sandbox ON (dinero ∞, bulldozer, sin IA rival)");
    leave_main_menu(commands, q_menu, q_menu_cam, intro_layers, next_screen);
}
