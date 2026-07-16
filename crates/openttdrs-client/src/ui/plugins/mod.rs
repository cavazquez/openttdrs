//! Subplugins de UI organizados por funcionalidad.

mod editor;
mod game_windows;
mod hud;
mod main_menu;
mod navigation;
mod settings_windows;
mod toolbar;

pub(crate) use editor::EditorUiPlugin;
pub(crate) use game_windows::GameWindowsPlugin;
pub(crate) use hud::HudUiPlugin;
pub(crate) use main_menu::MainMenuUiPlugin;
pub(crate) use navigation::NavigationUiPlugin;
pub(crate) use settings_windows::SettingsWindowsPlugin;
pub(crate) use toolbar::ToolbarUiPlugin;
