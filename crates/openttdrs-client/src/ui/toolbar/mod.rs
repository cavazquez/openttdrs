use bevy::prelude::*;

mod layout;
mod systems;

pub(crate) use layout::setup_top_toolbar;
pub(crate) use systems::{
    build_menu_interaction, handle_tile_click, toolbar_group_interaction, update_tool_button_visuals,
    update_toolbar_group_visuals, update_toolbar_tool_visibility, update_toolbar_tooltip,
};

/// Marca nodos del menu "Construir" para ignorar clics en el mapa cuando el cursor esta encima.
#[derive(Component)]
pub(crate) struct BuildMenuUi;

/// Accion del boton del menu de construccion.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BuildMenuAction {
    Road,
    Rail,
    Station,
    Clear,
}

#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ToolbarGroup {
    Transport,
    Build,
    Economy,
    Info,
    Settings,
}

/// Marca botones que seleccionan herramienta de construccion.
#[derive(Component)]
pub(crate) struct ToolSelectButton;

#[derive(Component)]
pub(crate) struct ToolbarGroupButton;

#[derive(Component)]
pub(crate) struct ToolButtonGroup(pub ToolbarGroup);

#[derive(Component)]
pub(crate) struct TooltipText;

#[derive(Component)]
pub(crate) struct TooltipBox;

#[derive(Component)]
pub(crate) struct ToolbarTooltipTarget {
    pub(crate) text: &'static str,
}

/// Herramienta de construccion activa elegida desde la UI.
#[derive(Resource, Default)]
pub(crate) struct UiToolState {
    pub(crate) active_tool: Option<BuildMenuAction>,
}

#[derive(Resource)]
pub(crate) struct ToolbarState {
    pub(crate) active_group: ToolbarGroup,
}

impl Default for ToolbarState {
    fn default() -> Self {
        Self {
            active_group: ToolbarGroup::Build,
        }
    }
}

/// Conservado por compatibilidad del pipeline startup; la UI vive en la toolbar superior.
pub(crate) fn setup_build_menu(_commands: Commands) {}
