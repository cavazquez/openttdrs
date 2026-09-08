//! UI de información de tile seleccionado y menú de construcción (I6).

use bevy::ecs::system::SystemParam;
use bevy::input_focus::InputFocus;
use bevy::prelude::*;
use bevy::text::EditableText;

mod lifecycle;
mod plugins;

mod ai_settings_window;
pub(crate) mod audio_settings_window;
mod autoreplace_window;
mod buy_window;
mod cargo_dist_settings_window;
mod cargo_payment_window;
mod cheat_window;
pub(crate) mod command_error_text;
mod company_view_window;
mod destination_window;
mod dev_console;
mod dialog_windows;
mod display_options_window;
mod endscreen;
mod extra_viewport_window;
mod finances_window;
mod floating_window;
pub(crate) mod font;
mod genland_window;
mod goal_list_window;
mod graph_window;
mod help_window;
mod hotkeys;
mod hud;
mod industry_directory;
mod industry_panel;
mod industry_production_window;
mod league_window;
mod list_window;
mod main_menu;
mod main_menu_intro;
mod menu;
mod modal_stack;
mod navigation;
mod newgrf_window;
mod news_settings_window;
mod pathfinding_settings_window;
mod refit_window;
mod save_window;
mod scrollbar;
mod shared_orders_window;
mod sign_list_window;
mod sparkline;
mod station_directory;
mod station_pool;
mod statusbar;
mod story_window;
mod subsidy_list;
mod tile_inspector_window;
mod timetable_window;
mod toolbar;
mod town_authority_window;
mod town_directory;
mod town_window;
mod ui5_blocked_stubs;
#[cfg(test)]
mod ui_enum_inventory_test;
mod vehicle_chain;
mod vehicle_details_window;
mod vehicle_list;
mod vehicle_window;
mod window_lifecycle;
mod windows_shot;

#[cfg(test)]
pub(crate) use hud::HudBuildFeedback;
pub(crate) use hud::SimHudControls;
pub(crate) use main_menu::{MainMenuCamera, MainMenuUi, leave_main_menu};
pub(crate) use save_window::SaveWindowState;
#[cfg(test)]
pub(crate) use statusbar::{NewsUiState, drain_news_events};
pub(crate) use toolbar::{BuildMenuAction, OrderEditState, ToolbarState, UiToolState};
pub(crate) use ui5_blocked_stubs::{LinkGraphView, LinkGraphWindowState};

/// Estado compartido que determina si el teclado pertenece a la UI y no al
/// mundo. Conserva la política que ya aplicaban los atajos globales para que
/// cámara y toolbar no discrepen sobre el mismo frame de entrada.
#[derive(SystemParam)]
pub(crate) struct KeyboardCapture<'w, 's> {
    focus: Option<Res<'w, InputFocus>>,
    editable: Query<'w, 's, (), With<EditableText>>,
    save_window: Option<Res<'w, SaveWindowState>>,
    console: Option<Res<'w, dev_console::DevConsoleState>>,
    exit_modal: Query<'w, 's, &'static Node, With<toolbar::editor_toolbar::EditorExitConfirmRoot>>,
}

impl KeyboardCapture<'_, '_> {
    /// `true` cuando texto, guardado, consola o confirmación modal deben
    /// recibir las teclas antes que los controles de juego.
    #[must_use]
    pub(crate) fn active(&self) -> bool {
        let text_focused = self
            .focus
            .as_deref()
            .and_then(InputFocus::get)
            .is_some_and(|entity| self.editable.get(entity).is_ok());
        text_focused
            || self
                .save_window
                .as_deref()
                .is_some_and(|window| window.open)
            || self
                .console
                .as_deref()
                .is_some_and(dev_console::dev_console_captures_keyboard)
            || self
                .exit_modal
                .iter()
                .any(|node| node.display != Display::None)
    }
}

#[cfg(test)]
pub(crate) use toolbar::editor_toolbar::EditorExitConfirmRoot;

pub(crate) struct ClientUiPlugin;

impl Plugin for ClientUiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            floating_window::FloatingWindowPlugin,
            scrollbar::ClassicScrollbarPlugin,
            windows_shot::WindowsShotPlugin,
            crate::i18n::LocalizationPlugin,
            lifecycle::InGameLifecyclePlugin,
            plugins::MainMenuUiPlugin,
            plugins::HudUiPlugin,
            plugins::ToolbarUiPlugin,
            plugins::NavigationUiPlugin,
            plugins::SettingsWindowsPlugin,
            plugins::GameWindowsPlugin,
            plugins::EditorUiPlugin,
        ));
    }
}
