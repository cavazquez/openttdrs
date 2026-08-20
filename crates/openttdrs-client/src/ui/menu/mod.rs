//! Menús declarativos de toolbar (`MenuSpec` / UI-1).

mod chrome;
mod model;
mod systems;

pub(crate) use chrome::{spawn_menu_anchor_button, spawn_menu_anchor_button_sized};
pub(crate) use model::{MenuId, ToolbarContext};
pub(crate) use systems::{
    ToolbarMenuState, dismiss_toolbar_menu_on_outside_click, handle_toolbar_menu_entries,
    handle_toolbar_menu_keyboard, handle_toolbar_navigation_button, refresh_toolbar_context,
    sync_toolbar_localized_labels, sync_toolbar_navigation_menu,
};
