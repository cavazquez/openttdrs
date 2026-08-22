use bevy::app::AppExit;
use bevy::prelude::*;
use openttdrs_core::Climate;

use crate::render::{MapVisualLayer, ShoreTile, WaterTile};
use crate::state::bootstrap::{
    MapSizePreset, NewGameSettings, PopulationDensity, START_YEARS, STARTING_MONEY_OPTIONS,
    TerrainRoughness,
};
use crate::state::{ClientScreen, SuspendedGameSession, new_game::NewGameSettingsResource};
use crate::ui::SimHudControls;
use crate::ui::save_window::{SaveWindowMode, SaveWindowState, save_dir_from};

use super::super::labels::{
    localized_climate_label, localized_density_label, localized_roughness_label, panel_hints,
    panel_title,
};
use super::super::widgets::{hover_primary, hover_secondary};
use super::super::{
    MainMenuBackButton, MainMenuCamera, MainMenuContinueButton, MainMenuContinueWrap,
    MainMenuDemoButton, MainMenuDynamicText, MainMenuEditorButton, MainMenuHintsText,
    MainMenuLoadButton, MainMenuLocalizedText, MainMenuNewGameButton, MainMenuPanel,
    MainMenuQuitButton, MainMenuQuitConfirmNo, MainMenuQuitConfirmYes, MainMenuStartButton,
    MainMenuSubPanel, MainMenuTitleText, MainMenuUi,
};
use super::session::{enter_editor, enter_new_game, resume_suspended_game};

pub(crate) fn sync_main_menu_panel_visibility(
    panel: Res<MainMenuPanel>,
    prefs: Res<crate::settings::ClientPreferences>,
    mut subpanels: Query<(&MainMenuSubPanel, &mut Node, &mut Visibility)>,
    mut title_q: Query<&mut Text, (With<MainMenuTitleText>, Without<MainMenuHintsText>)>,
    mut hints_q: Query<
        &mut Text,
        (
            With<MainMenuHintsText>,
            Without<MainMenuTitleText>,
            Without<super::super::MainMenuSummaryText>,
        ),
    >,
) {
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
    if let Ok(mut title) = title_q.single_mut() {
        let translated = crate::i18n::text(prefs.locale(), panel_title(*panel));
        if **title != translated {
            **title = translated.to_owned();
        }
    }
    if let Ok(mut hints) = hints_q.single_mut() {
        let translated = crate::i18n::text(prefs.locale(), panel_hints(*panel));
        if **hints != translated {
            **hints = translated.to_owned();
        }
    }
}

/// Actualiza los textos de los botones que se crean en todos los subpaneles.
pub(crate) fn sync_main_menu_localized_labels(
    prefs: Res<crate::settings::ClientPreferences>,
    mut labels: Query<(&MainMenuLocalizedText, &mut Text)>,
    mut dynamic_labels: Query<(&MainMenuDynamicText, &mut Text)>,
) {
    let locale = prefs.locale();
    for (key, mut text) in &mut labels {
        let translated = crate::i18n::text(locale, key.0);
        if **text != translated {
            **text = translated.to_owned();
        }
    }
    for (key, mut text) in &mut dynamic_labels {
        let translated = match key {
            MainMenuDynamicText::Climate(value) => localized_climate_label(locale, *value),
            MainMenuDynamicText::Density(value) => localized_density_label(locale, *value),
            MainMenuDynamicText::Roughness(value) => localized_roughness_label(locale, *value),
        };
        if **text != translated {
            **text = translated.to_owned();
        }
    }
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

/// Botón «Editor de escenarios» (sistema aparte: límite ParamSet del menú).
#[allow(clippy::too_many_arguments)]
pub(crate) fn main_menu_editor_interaction(
    panel: Res<MainMenuPanel>,
    save_window: Res<SaveWindowState>,
    mut next_screen: ResMut<NextState<ClientScreen>>,
    mut suspended: ResMut<SuspendedGameSession>,
    q_menu: Query<Entity, With<MainMenuUi>>,
    q_menu_cam: Query<Entity, With<MainMenuCamera>>,
    intro_layers: Query<Entity, Or<(With<MapVisualLayer>, With<WaterTile>, With<ShoreTile>)>>,
    mut buttons: Query<
        (&Interaction, &mut BackgroundColor),
        (Changed<Interaction>, With<MainMenuEditorButton>),
    >,
    mut commands: Commands,
) {
    if save_window.open || *panel != MainMenuPanel::Root {
        return;
    }
    for (interaction, mut bg) in &mut buttons {
        if *interaction == Interaction::Pressed {
            enter_editor(
                &mut commands,
                &q_menu,
                &q_menu_cam,
                &intro_layers,
                &mut next_screen,
                &mut suspended,
            );
            return;
        }
        hover_secondary(interaction, &mut bg);
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
        MainMenuPanel::Scenarios if esc => {
            *panel = MainMenuPanel::Root;
            return;
        }
        MainMenuPanel::Preferences if esc => {
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
                        rival_ai: false,
                        disasters_enabled: false,
                        terrain_roughness: TerrainRoughness::Normal,
                        gamescript_demo: true,
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
        MainMenuPanel::Highscores | MainMenuPanel::Scenarios | MainMenuPanel::Preferences => {
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
