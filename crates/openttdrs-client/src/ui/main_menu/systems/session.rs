use crate::render::{MapVisualLayer, ShoreTile, WaterTile};
use crate::state::bootstrap::NewGameSettings;
use crate::state::{
    ClientScreen, EditorSession, SimWorld, SuspendedGameSession, apply_editor_sandbox,
    editor_new_game_settings,
};
use crate::ui::main_menu_intro::despawn_main_menu_intro_layers;
use bevy::prelude::*;
use openttdrs_core::GameTick;

use crate::state::new_game::NewGameSeedSequence;

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

#[allow(clippy::too_many_arguments)]
pub(in crate::ui::main_menu) fn enter_new_game(
    commands: &mut Commands,
    q_menu: &Query<Entity, With<MainMenuUi>>,
    q_menu_cam: &Query<Entity, With<MainMenuCamera>>,
    intro_layers: &Query<Entity, Or<(With<MapVisualLayer>, With<WaterTile>, With<ShoreTile>)>>,
    settings: NewGameSettings,
    previous_tick: Option<GameTick>,
    auto_seeds: &mut NewGameSeedSequence,
    next_screen: &mut NextState<ClientScreen>,
    suspended: &mut SuspendedGameSession,
) {
    suspended.active = false;
    commands.insert_resource(EditorSession::inactive());
    commands.insert_resource(build_new_game_session(settings, previous_tick, auto_seeds));
    leave_main_menu(commands, q_menu, q_menu_cam, intro_layers, next_screen);
}

fn build_new_game_session(
    settings: NewGameSettings,
    previous_tick: Option<GameTick>,
    auto_seeds: &mut NewGameSeedSequence,
) -> SimWorld {
    let mut settings = settings.sanitized();
    if settings.world_gen && settings.seed == 0 {
        settings.seed = auto_seeds.next_seed();
    }

    let mut sim = SimWorld::from_new_game(&settings);
    if let Some(previous_tick) = previous_tick
        && previous_tick > sim.state.tick
    {
        sim.state.tick = previous_tick;
        sim.state.sync_timers_from_tick();
    }
    sim
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

#[cfg(test)]
mod tests {
    use super::{NewGameSeedSequence, build_new_game_session};
    use crate::state::bootstrap::NewGameSettings;
    use openttdrs_core::GameTick;

    #[test]
    fn automatic_new_games_materialize_distinct_world_seeds() {
        let settings = NewGameSettings {
            world_gen: true,
            island: true,
            preserve_demo: false,
            ..NewGameSettings::default()
        };
        let mut auto_seeds = NewGameSeedSequence::default();

        let first = build_new_game_session(settings, None, &mut auto_seeds);
        let second = build_new_game_session(settings, None, &mut auto_seeds);

        assert_ne!(first.state.world_seed, 0);
        assert_ne!(second.state.world_seed, 0);
        assert_ne!(first.state.world_seed, second.state.world_seed);
    }

    #[test]
    fn explicit_new_game_seed_stays_reproducible() {
        let settings = NewGameSettings {
            world_gen: true,
            island: true,
            preserve_demo: false,
            seed: 73,
            ..NewGameSettings::default()
        };
        let mut auto_seeds = NewGameSeedSequence::default();

        let sim = build_new_game_session(settings, None, &mut auto_seeds);

        assert_eq!(sim.state.world_seed, 73);
    }

    #[test]
    fn replacing_a_session_keeps_the_later_tick() {
        let mut auto_seeds = NewGameSeedSequence::default();
        let previous_tick = GameTick::new(123_456);

        let sim = build_new_game_session(
            NewGameSettings {
                preserve_demo: false,
                ..NewGameSettings::default()
            },
            Some(previous_tick),
            &mut auto_seeds,
        );

        assert_eq!(sim.state.tick, previous_tick);
    }
}
