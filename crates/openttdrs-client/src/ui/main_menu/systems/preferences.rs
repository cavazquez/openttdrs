use bevy::prelude::*;

use super::super::widgets::{hover_secondary, option_button_bg};
use super::super::{
    MainMenuHighscoresButton, MainMenuHighscoresText, MainMenuPanel, MainMenuPreferencesButton,
    MainMenuResolutionButton, MainMenuSoundButton,
};

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

pub(crate) fn main_menu_preferences_interaction(
    mut panel: ResMut<MainMenuPanel>,
    mut prefs: ResMut<crate::settings::ClientPreferences>,
    mut windows: Query<&mut Window, With<bevy::window::PrimaryWindow>>,
    mut root_btn: Query<
        (&Interaction, &mut BackgroundColor),
        (
            Changed<Interaction>,
            With<MainMenuPreferencesButton>,
            Without<MainMenuResolutionButton>,
        ),
    >,
    mut res_btn: Query<
        (
            &Interaction,
            &MainMenuResolutionButton,
            &mut BackgroundColor,
        ),
        Without<MainMenuPreferencesButton>,
    >,
) {
    if *panel == MainMenuPanel::Root {
        for (interaction, mut bg) in &mut root_btn {
            if *interaction == Interaction::Pressed {
                *panel = MainMenuPanel::Preferences;
                return;
            }
            hover_secondary(interaction, &mut bg);
        }
        return;
    }
    if *panel != MainMenuPanel::Preferences {
        return;
    }
    for (interaction, btn, mut bg) in &mut res_btn {
        let selected = prefs.window_width == btn.width && prefs.window_height == btn.height;
        if *interaction == Interaction::Pressed {
            prefs.window_width = btn.width;
            prefs.window_height = btn.height;
            prefs.set_changed();
            if let Ok(mut window) = windows.single_mut() {
                window.resolution.set(btn.width as f32, btn.height as f32);
            }
        }
        *bg = option_button_bg(
            selected || (*interaction == Interaction::Pressed),
            *interaction,
        );
    }
}

pub(crate) fn sync_main_menu_preferences(
    panel: Res<MainMenuPanel>,
    prefs: Res<crate::settings::ClientPreferences>,
    mut res_btn: Query<(
        &MainMenuResolutionButton,
        &mut BackgroundColor,
        &Interaction,
    )>,
) {
    if *panel != MainMenuPanel::Preferences {
        return;
    }
    for (btn, mut bg, interaction) in &mut res_btn {
        let selected = prefs.window_width == btn.width && prefs.window_height == btn.height;
        *bg = option_button_bg(selected, *interaction);
    }
}

pub(crate) fn main_menu_sound_interaction(
    panel: Res<MainMenuPanel>,
    mut sound: ResMut<crate::ui::audio_settings_window::SoundMusicWindowState>,
    mut buttons: Query<
        (&Interaction, &mut BackgroundColor),
        (Changed<Interaction>, With<MainMenuSoundButton>),
    >,
) {
    if *panel != MainMenuPanel::Root {
        return;
    }
    for (interaction, mut bg) in &mut buttons {
        if *interaction == Interaction::Pressed {
            sound.open = true;
            return;
        }
        hover_secondary(interaction, &mut bg);
    }
}
