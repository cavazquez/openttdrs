//! Plugin de UI para el menú principal.

use bevy::prelude::*;

use crate::bevy_app::UpdateSet;
use crate::state::ClientScreen;
use crate::ui::audio_settings_window::{
    SoundMusicWindowState, handle_audio_settings_buttons, handle_music_window_buttons,
    handle_volume_sliders, setup_sound_music_window, sound_music_window_on_closed,
    sync_sound_music_window,
};
use crate::ui::hud::{HudSfxHandles, load_hud_sfx};
use crate::ui::main_menu::{
    auto_start_preloaded_json, main_menu_continue_interaction, main_menu_editor_interaction,
    main_menu_highscores_interaction, main_menu_interaction, main_menu_options_interaction,
    main_menu_preferences_interaction, main_menu_roughness_interaction,
    main_menu_scenarios_interaction, main_menu_sound_interaction, setup_main_menu,
    sync_main_menu_continue_button, sync_main_menu_heightmap_slots, sync_main_menu_highscores,
    sync_main_menu_localized_labels, sync_main_menu_panel_visibility, sync_main_menu_preferences,
    sync_main_menu_summary,
};
use crate::ui::main_menu_intro::{
    animate_main_menu_intro_traffic, cleanup_main_menu_on_exit, pan_main_menu_intro_camera,
    setup_main_menu_intro,
};
use crate::ui::save_window::{
    SaveWindowState, handle_save_window_buttons, prepare_save_window_name,
    save_window_editable_keyboard, save_window_keyboard, save_window_name_click_focus,
    setup_save_window, sync_save_window,
};
use crate::ui::toolbar::{ToolbarState, toolbar_click_beep};

pub(crate) struct MainMenuUiPlugin;

impl Plugin for MainMenuUiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SoundMusicWindowState>()
            .init_resource::<HudSfxHandles>()
            .init_resource::<SaveWindowState>()
            .init_resource::<ToolbarState>()
            .add_systems(OnExit(ClientScreen::MainMenu), cleanup_main_menu_on_exit)
            .add_systems(
                OnEnter(ClientScreen::MainMenu),
                (
                    setup_main_menu_intro,
                    setup_main_menu,
                    setup_save_window,
                    setup_sound_music_window,
                    load_hud_sfx,
                ),
            )
            .add_systems(
                Update,
                (
                    pan_main_menu_intro_camera,
                    animate_main_menu_intro_traffic,
                    auto_start_preloaded_json,
                    (main_menu_interaction, main_menu_continue_interaction).chain(),
                    main_menu_editor_interaction,
                    main_menu_highscores_interaction,
                    main_menu_scenarios_interaction,
                    main_menu_preferences_interaction,
                    main_menu_sound_interaction,
                    main_menu_options_interaction,
                    main_menu_roughness_interaction,
                    sync_main_menu_panel_visibility,
                    sync_main_menu_summary,
                    sync_main_menu_continue_button,
                    sync_main_menu_localized_labels,
                    sync_main_menu_highscores,
                    sync_main_menu_heightmap_slots,
                    sync_main_menu_preferences,
                )
                    .run_if(in_state(ClientScreen::MainMenu)),
            )
            .add_systems(
                Update,
                (
                    handle_audio_settings_buttons,
                    handle_volume_sliders,
                    handle_music_window_buttons,
                    sound_music_window_on_closed,
                    sync_sound_music_window,
                    toolbar_click_beep,
                )
                    .run_if(in_state(ClientScreen::MainMenu)),
            )
            .add_systems(
                Update,
                (
                    save_window_keyboard,
                    save_window_editable_keyboard,
                    save_window_name_click_focus,
                    handle_save_window_buttons,
                    sync_save_window,
                    prepare_save_window_name,
                )
                    .in_set(UpdateSet::Ui)
                    .run_if(in_state(ClientScreen::MainMenu)),
            );
    }
}
