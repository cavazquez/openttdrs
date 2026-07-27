//! Plugin de UI para el editor de escenarios.

use bevy::prelude::*;

use crate::bevy_app::{StartupSet, UpdateSet};
use crate::state::ClientScreen;
use crate::ui::genland_window::{
    GenLandWindowState, genland_window_on_closed, handle_genland_buttons, setup_genland_window,
    sync_genland_window,
};
use crate::ui::main_menu::apply_pending_heightmap_on_enter;
use crate::ui::toolbar::{
    EditorDocumentState, EditorToolbarLayoutState, EditorTownMenuState,
    handle_editor_exit_confirmation, handle_editor_toolbar_build_buttons,
    handle_editor_toolbar_control_buttons, handle_editor_toolbar_switch,
    handle_editor_toolbar_tool_buttons, handle_editor_town_dropdown, initialize_editor_document,
    setup_editor_toolbar, sync_editor_exit_confirmation, sync_editor_toolbar_button_visuals,
    sync_editor_toolbar_date, sync_editor_toolbar_layout, sync_editor_toolbar_visibility,
    sync_editor_town_dropdown,
};

pub(crate) struct EditorUiPlugin;

impl Plugin for EditorUiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GenLandWindowState>()
            .init_resource::<EditorTownMenuState>()
            .init_resource::<EditorToolbarLayoutState>()
            .init_resource::<EditorDocumentState>()
            .init_resource::<crate::state::new_game::NewGameSettingsResource>()
            .add_systems(
                OnEnter(ClientScreen::InGame),
                apply_pending_heightmap_on_enter,
            )
            .add_systems(
                OnEnter(ClientScreen::InGame),
                (setup_editor_toolbar, setup_genland_window).in_set(StartupSet::Ui),
            )
            .add_systems(
                Update,
                (
                    sync_editor_toolbar_visibility,
                    initialize_editor_document,
                    handle_editor_toolbar_switch,
                    sync_editor_toolbar_layout,
                    sync_editor_exit_confirmation,
                    handle_editor_exit_confirmation,
                    sync_editor_toolbar_date,
                    sync_editor_toolbar_button_visuals,
                    sync_editor_town_dropdown,
                    handle_editor_toolbar_control_buttons,
                    handle_editor_toolbar_tool_buttons,
                    handle_editor_toolbar_build_buttons,
                    handle_editor_town_dropdown,
                    genland_window_on_closed,
                    sync_genland_window,
                    handle_genland_buttons,
                )
                    .in_set(UpdateSet::Ui)
                    .run_if(in_state(ClientScreen::InGame)),
            );
    }
}
