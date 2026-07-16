//! Subplugins de UI organizados por funcionalidad.

mod main_menu;
mod hud;
mod toolbar;
mod navigation;
mod settings_windows;
mod game_windows;
mod editor;

pub(crate) use main_menu::MainMenuUiPlugin;
pub(crate) use hud::HudUiPlugin;
pub(crate) use toolbar::ToolbarUiPlugin;
pub(crate) use navigation::NavigationUiPlugin;
pub(crate) use settings_windows::SettingsWindowsPlugin;
pub(crate) use game_windows::GameWindowsPlugin;
pub(crate) use editor::EditorUiPlugin;
