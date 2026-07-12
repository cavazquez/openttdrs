use bevy::app::AppExit;
use bevy::prelude::*;
use openttdrs_core::Climate;

use crate::render::{MapVisualLayer, ShoreTile, WaterTile};
use crate::state::bootstrap::{
    MapSizePreset, NewGameSettings, PopulationDensity, START_YEARS, STARTING_MONEY_OPTIONS,
};
use crate::state::{
    ClientScreen, SimWorld, SuspendedGameSession, new_game::NewGameSettingsResource,
};
use crate::ui::SimHudControls;
use crate::ui::main_menu_intro::despawn_main_menu_intro_layers;
use crate::ui::save_window::{SaveWindowMode, SaveWindowState, save_dir_from};

use super::labels::{adjust_seed, cycle_density, panel_hints, panel_title, summary_text};
use super::widgets::{
    hover_primary, hover_secondary, option_button_bg, seed_button_bg, toggle_button_bg,
};
use super::{
    MainMenuBackButton, MainMenuCamera, MainMenuClimateButton, MainMenuContinueButton,
    MainMenuContinueWrap, MainMenuDemoButton, MainMenuDensityButton, MainMenuDensityTarget,
    MainMenuHighscoresButton, MainMenuHighscoresText, MainMenuHintsText, MainMenuLoadButton,
    MainMenuMapSizeButton, MainMenuNewGameButton, MainMenuPanel, MainMenuQuitButton,
    MainMenuQuitConfirmNo, MainMenuQuitConfirmYes, MainMenuSeedDecButton, MainMenuSeedIncButton,
    MainMenuStartButton, MainMenuStartYearButton, MainMenuStartingMoneyButton, MainMenuSubPanel,
    MainMenuSummaryText, MainMenuTitleText, MainMenuToggle, MainMenuUi,
};

pub(crate) fn sync_main_menu_panel_visibility(
    panel: Res<MainMenuPanel>,
    mut subpanels: Query<(&MainMenuSubPanel, &mut Node, &mut Visibility)>,
    mut title_q: Query<&mut Text, (With<MainMenuTitleText>, Without<MainMenuHintsText>)>,
    mut hints_q: Query<
        &mut Text,
        (
            With<MainMenuHintsText>,
            Without<MainMenuTitleText>,
            Without<MainMenuSummaryText>,
        ),
    >,
) {
    let panel_changed = panel.is_changed();
    for (sub, mut node, mut vis) in &mut subpanels {
        let active = sub.0 == *panel;
        node.display = if active {
            Display::DEFAULT
        } else {
            Display::None
        };
        *vis = if active {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
    if !panel_changed {
        return;
    }
    if let Ok(mut title) = title_q.single_mut() {
        title.0 = panel_title(*panel).to_string();
    }
    if let Ok(mut hints) = hints_q.single_mut() {
        hints.0 = panel_hints(*panel).to_string();
    }
}

pub(crate) fn sync_main_menu_summary(
    settings: Res<NewGameSettingsResource>,
    panel: Res<MainMenuPanel>,
    mut q: Query<&mut Text, With<MainMenuSummaryText>>,
) {
    if !settings.is_changed() && !panel.is_changed() {
        return;
    }
    if *panel != MainMenuPanel::NewGame {
        return;
    }
    for mut text in &mut q {
        text.0 = summary_text(settings.settings());
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn main_menu_options_interaction(
    panel: Res<MainMenuPanel>,
    mut settings: ResMut<NewGameSettingsResource>,
    mut button_sets: ParamSet<(
        Query<(&Interaction, &MainMenuClimateButton, &mut BackgroundColor)>,
        Query<(&Interaction, &MainMenuMapSizeButton, &mut BackgroundColor)>,
        Query<(&Interaction, &MainMenuStartYearButton, &mut BackgroundColor)>,
        Query<(&Interaction, &MainMenuToggle, &mut BackgroundColor)>,
        Query<(&Interaction, &mut BackgroundColor), With<MainMenuSeedDecButton>>,
        Query<(&Interaction, &mut BackgroundColor), With<MainMenuSeedIncButton>>,
        Query<(&Interaction, &MainMenuDensityButton, &mut BackgroundColor)>,
        Query<(
            &Interaction,
            &MainMenuStartingMoneyButton,
            &mut BackgroundColor,
        )>,
    )>,
    keys: Res<ButtonInput<KeyCode>>,
) {
    if *panel != MainMenuPanel::NewGame {
        return;
    }

    if keys.just_pressed(KeyCode::Digit1) {
        settings.0.climate = Climate::Temperate;
    }
    if keys.just_pressed(KeyCode::Digit2) {
        settings.0.climate = Climate::SubArctic;
    }
    if keys.just_pressed(KeyCode::Digit3) {
        settings.0.climate = Climate::SubTropical;
    }
    if keys.just_pressed(KeyCode::Digit4) {
        settings.0.climate = Climate::Toyland;
    }
    if keys.just_pressed(KeyCode::BracketLeft) {
        adjust_seed(&mut settings.0.seed, -1);
    }
    if keys.just_pressed(KeyCode::BracketRight) {
        adjust_seed(&mut settings.0.seed, 1);
    }
    if keys.just_pressed(KeyCode::KeyZ) {
        cycle_density(&mut settings.0.town_density);
    }
    if keys.just_pressed(KeyCode::KeyX) {
        cycle_density(&mut settings.0.industry_density);
    }

    for (interaction, btn, mut bg) in &mut button_sets.p0() {
        if *interaction == Interaction::Pressed {
            settings.0.climate = btn.0;
        }
        *bg = option_button_bg(settings.0.climate == btn.0, *interaction);
    }

    for (interaction, btn, mut bg) in &mut button_sets.p1() {
        if *interaction == Interaction::Pressed {
            settings.0.map_size = btn.0;
            if settings.0.map_size != MapSizePreset::Compact {
                settings.0.preserve_demo = false;
            }
        }
        *bg = option_button_bg(settings.0.map_size == btn.0, *interaction);
    }

    for (interaction, btn, mut bg) in &mut button_sets.p2() {
        if *interaction == Interaction::Pressed {
            settings.0.start_year = btn.0;
        }
        *bg = option_button_bg(settings.0.start_year == btn.0, *interaction);
    }

    for (interaction, toggle, mut bg) in &mut button_sets.p3() {
        if *interaction == Interaction::Pressed {
            match toggle {
                MainMenuToggle::WorldGen => settings.0.world_gen = !settings.0.world_gen,
                MainMenuToggle::Island => settings.0.island = !settings.0.island,
                MainMenuToggle::PreserveDemo => {
                    if settings.0.map_size == MapSizePreset::Compact {
                        settings.0.preserve_demo = !settings.0.preserve_demo;
                    }
                }
                MainMenuToggle::RivalAi => settings.0.rival_ai = !settings.0.rival_ai,
            }
        }
        let on = match toggle {
            MainMenuToggle::WorldGen => settings.0.world_gen,
            MainMenuToggle::Island => settings.0.island,
            MainMenuToggle::PreserveDemo => {
                settings.0.preserve_demo && settings.0.map_size == MapSizePreset::Compact
            }
            MainMenuToggle::RivalAi => settings.0.rival_ai,
        };
        *bg = toggle_button_bg(on, *interaction);
    }

    for (interaction, mut bg) in &mut button_sets.p4() {
        if *interaction == Interaction::Pressed {
            adjust_seed(&mut settings.0.seed, -1);
        }
        *bg = seed_button_bg(*interaction);
    }
    for (interaction, mut bg) in &mut button_sets.p5() {
        if *interaction == Interaction::Pressed {
            adjust_seed(&mut settings.0.seed, 1);
        }
        *bg = seed_button_bg(*interaction);
    }

    for (interaction, btn, mut bg) in &mut button_sets.p6() {
        if *interaction == Interaction::Pressed {
            match btn.1 {
                MainMenuDensityTarget::Town => settings.0.town_density = btn.0,
                MainMenuDensityTarget::Industry => settings.0.industry_density = btn.0,
            }
        }
        let selected = match btn.1 {
            MainMenuDensityTarget::Town => settings.0.town_density == btn.0,
            MainMenuDensityTarget::Industry => settings.0.industry_density == btn.0,
        };
        *bg = option_button_bg(selected, *interaction);
    }

    for (interaction, btn, mut bg) in &mut button_sets.p7() {
        if *interaction == Interaction::Pressed {
            settings.0.starting_money = btn.0;
        }
        *bg = option_button_bg(settings.0.starting_money == btn.0, *interaction);
    }
}

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
    suspended.active = false;
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

fn enter_new_game(
    commands: &mut Commands,
    q_menu: &Query<Entity, With<MainMenuUi>>,
    q_menu_cam: &Query<Entity, With<MainMenuCamera>>,
    intro_layers: &Query<Entity, Or<(With<MapVisualLayer>, With<WaterTile>, With<ShoreTile>)>>,
    settings: NewGameSettings,
    next_screen: &mut NextState<ClientScreen>,
    suspended: &mut SuspendedGameSession,
) {
    suspended.active = false;
    commands.insert_resource(SimWorld::from_new_game(&settings.sanitized()));
    leave_main_menu(commands, q_menu, q_menu_cam, intro_layers, next_screen);
}

pub(crate) fn sync_main_menu_continue_button(
    suspended: Res<SuspendedGameSession>,
    panel: Res<MainMenuPanel>,
    mut q: Query<(&mut Node, &mut Visibility), With<MainMenuContinueWrap>>,
) {
    if !suspended.is_changed() && !panel.is_changed() {
        return;
    }
    let show = suspended.active && *panel == MainMenuPanel::Root;
    for (mut node, mut vis) in &mut q {
        node.display = if show {
            Display::DEFAULT
        } else {
            Display::None
        };
        *vis = if show {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
}

/// Botón «Continuar partida» en sistema aparte (evita B0001 con el `ParamSet` del menú).
#[allow(clippy::too_many_arguments)]
pub(crate) fn main_menu_continue_interaction(
    panel: Res<MainMenuPanel>,
    save_window: Res<SaveWindowState>,
    mut next_screen: ResMut<NextState<ClientScreen>>,
    mut suspended: ResMut<SuspendedGameSession>,
    q_menu: Query<Entity, With<MainMenuUi>>,
    q_menu_cam: Query<Entity, With<MainMenuCamera>>,
    intro_layers: Query<Entity, Or<(With<MapVisualLayer>, With<WaterTile>, With<ShoreTile>)>>,
    mut buttons: Query<
        (&Interaction, &mut BackgroundColor),
        (Changed<Interaction>, With<MainMenuContinueButton>),
    >,
    mut commands: Commands,
) {
    if save_window.open || !suspended.active || *panel != MainMenuPanel::Root {
        return;
    }
    for (interaction, mut bg) in &mut buttons {
        if *interaction == Interaction::Pressed {
            resume_suspended_game(
                &mut commands,
                &q_menu,
                &q_menu_cam,
                &intro_layers,
                &mut next_screen,
                &mut suspended,
            );
            return;
        }
        hover_primary(interaction, &mut bg);
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn main_menu_interaction(
    mut panel: ResMut<MainMenuPanel>,
    mut next_screen: ResMut<NextState<ClientScreen>>,
    mut settings: ResMut<NewGameSettingsResource>,
    mut save_window: ResMut<SaveWindowState>,
    mut suspended: ResMut<SuspendedGameSession>,
    hud: Res<SimHudControls>,
    q_menu: Query<Entity, With<MainMenuUi>>,
    q_menu_cam: Query<Entity, With<MainMenuCamera>>,
    intro_layers: Query<Entity, Or<(With<MapVisualLayer>, With<WaterTile>, With<ShoreTile>)>>,
    mut button_sets: ParamSet<(
        Query<
            (&Interaction, &mut BackgroundColor),
            (Changed<Interaction>, With<MainMenuNewGameButton>),
        >,
        Query<
            (&Interaction, &mut BackgroundColor),
            (Changed<Interaction>, With<MainMenuLoadButton>),
        >,
        Query<
            (&Interaction, &mut BackgroundColor),
            (Changed<Interaction>, With<MainMenuDemoButton>),
        >,
        Query<
            (&Interaction, &mut BackgroundColor),
            (Changed<Interaction>, With<MainMenuQuitButton>),
        >,
        Query<
            (&Interaction, &mut BackgroundColor),
            (Changed<Interaction>, With<MainMenuBackButton>),
        >,
        Query<
            (&Interaction, &mut BackgroundColor),
            (Changed<Interaction>, With<MainMenuStartButton>),
        >,
        Query<
            (&Interaction, &mut BackgroundColor),
            (Changed<Interaction>, With<MainMenuQuitConfirmYes>),
        >,
        Query<
            (&Interaction, &mut BackgroundColor),
            (Changed<Interaction>, With<MainMenuQuitConfirmNo>),
        >,
    )>,
    keys: Res<ButtonInput<KeyCode>>,
    mut exit: MessageWriter<AppExit>,
    mut commands: Commands,
) {
    if save_window.open {
        return;
    }

    let esc = keys.just_pressed(KeyCode::Escape);
    match *panel {
        MainMenuPanel::Root if esc => {
            *panel = MainMenuPanel::QuitConfirm;
            return;
        }
        MainMenuPanel::NewGame if esc => {
            *panel = MainMenuPanel::Root;
            return;
        }
        MainMenuPanel::Highscores if esc => {
            *panel = MainMenuPanel::Root;
            return;
        }
        MainMenuPanel::QuitConfirm if esc => {
            *panel = MainMenuPanel::Root;
            return;
        }
        _ => {}
    }

    match *panel {
        MainMenuPanel::Root => {
            for (interaction, mut bg) in &mut button_sets.p0() {
                if *interaction == Interaction::Pressed {
                    *panel = MainMenuPanel::NewGame;
                    return;
                }
                hover_primary(interaction, &mut bg);
            }
            for (interaction, mut bg) in &mut button_sets.p1() {
                if *interaction == Interaction::Pressed {
                    save_window
                        .open_in_mode(SaveWindowMode::Load, &save_dir_from(&hud.json_save_path));
                    return;
                }
                hover_primary(interaction, &mut bg);
            }
            for (interaction, mut bg) in &mut button_sets.p2() {
                if *interaction == Interaction::Pressed {
                    settings.0 = NewGameSettings {
                        climate: Climate::Temperate,
                        map_size: MapSizePreset::Compact,
                        start_year: START_YEARS[0],
                        world_gen: false,
                        island: false,
                        preserve_demo: true,
                        seed: 0,
                        town_density: PopulationDensity::Normal,
                        industry_density: PopulationDensity::Normal,
                        starting_money: STARTING_MONEY_OPTIONS[1],
                        rival_ai: true,
                    };
                    enter_new_game(
                        &mut commands,
                        &q_menu,
                        &q_menu_cam,
                        &intro_layers,
                        settings.settings(),
                        &mut next_screen,
                        &mut suspended,
                    );
                    return;
                }
                hover_secondary(interaction, &mut bg);
            }
            for (interaction, mut bg) in &mut button_sets.p3() {
                if *interaction == Interaction::Pressed {
                    *panel = MainMenuPanel::QuitConfirm;
                    return;
                }
                hover_secondary(interaction, &mut bg);
            }
        }
        MainMenuPanel::NewGame => {
            let start_via_key =
                keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::Space);
            let mut start_requested = start_via_key;
            for (interaction, mut bg) in &mut button_sets.p5() {
                if *interaction == Interaction::Pressed {
                    start_requested = true;
                }
                hover_primary(interaction, &mut bg);
            }
            if start_requested {
                enter_new_game(
                    &mut commands,
                    &q_menu,
                    &q_menu_cam,
                    &intro_layers,
                    settings.settings(),
                    &mut next_screen,
                    &mut suspended,
                );
                return;
            }
            for (interaction, mut bg) in &mut button_sets.p4() {
                if *interaction == Interaction::Pressed {
                    *panel = MainMenuPanel::Root;
                    return;
                }
                hover_secondary(interaction, &mut bg);
            }
        }
        MainMenuPanel::QuitConfirm => {
            for (interaction, mut bg) in &mut button_sets.p6() {
                if *interaction == Interaction::Pressed {
                    exit.write(AppExit::Success);
                    return;
                }
                hover_primary(interaction, &mut bg);
            }
            for (interaction, mut bg) in &mut button_sets.p7() {
                if *interaction == Interaction::Pressed {
                    *panel = MainMenuPanel::Root;
                    return;
                }
                hover_secondary(interaction, &mut bg);
            }
        }
        MainMenuPanel::Highscores => {
            for (interaction, mut bg) in &mut button_sets.p4() {
                if *interaction == Interaction::Pressed {
                    *panel = MainMenuPanel::Root;
                    return;
                }
                hover_secondary(interaction, &mut bg);
            }
        }
    }
}

pub(crate) fn main_menu_highscores_interaction(
    mut panel: ResMut<MainMenuPanel>,
    mut buttons: Query<
        (&Interaction, &mut BackgroundColor),
        (Changed<Interaction>, With<MainMenuHighscoresButton>),
    >,
) {
    if *panel != MainMenuPanel::Root {
        return;
    }
    for (interaction, mut bg) in &mut buttons {
        if *interaction == Interaction::Pressed {
            *panel = MainMenuPanel::Highscores;
            return;
        }
        hover_secondary(interaction, &mut bg);
    }
}

pub(crate) fn sync_main_menu_highscores(
    panel: Res<MainMenuPanel>,
    prefs: Option<Res<crate::settings::ClientPreferences>>,
    mut q: Query<&mut Text, With<MainMenuHighscoresText>>,
) {
    if *panel != MainMenuPanel::Highscores {
        return;
    }
    if !panel.is_changed() && prefs.as_ref().is_none_or(|p| !p.is_changed()) {
        // Still refresh when opening: panel.is_changed covers that.
    }
    let body = prefs
        .as_ref()
        .map(|p| {
            let entries = p.highscore_entries();
            if entries.is_empty() {
                "(sin puntuaciones aún — finaliza una partida)".into()
            } else {
                entries
                    .iter()
                    .enumerate()
                    .map(|(i, e)| {
                        format!(
                            "{}. {}  {}  ({})  {}\n",
                            i + 1,
                            e.company_name,
                            openttdrs_core::format_money(e.company_value),
                            e.calendar_year,
                            e.reason.label_es()
                        )
                    })
                    .collect::<String>()
            }
        })
        .unwrap_or_else(|| "(preferencias no cargadas)".into());
    for mut text in &mut q {
        **text = body.clone();
    }
}
