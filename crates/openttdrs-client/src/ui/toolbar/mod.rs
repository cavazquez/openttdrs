use bevy::prelude::*;

pub(crate) mod build_input;
mod depot_panel;
mod layout;
mod minimap;
mod order_panel;
mod preview;
mod settings;
mod station_panel;
mod systems;

pub(crate) use build_input::handle_tile_click;
pub(crate) use depot_panel::{
    DepotPanelState, handle_depot_panel_buttons, setup_depot_panel, sync_depot_panel,
};
pub(crate) use layout::setup_top_toolbar;
pub(crate) use minimap::{handle_minimap_click, setup_minimap, sync_minimap};
pub(crate) use order_panel::{handle_order_panel_buttons, setup_order_panel, sync_order_panel};
pub(crate) use preview::{rotate_station_with_right_click, update_build_ghost_preview};
pub(crate) use settings::handle_settings_menu_buttons;
pub(crate) use station_panel::{
    StationCargoPanelState, handle_station_cargo_panel_buttons, setup_station_cargo_panel,
    sync_station_cargo_panel,
};
pub(crate) use systems::{
    build_menu_interaction, close_toolbar_button_interaction, close_toolbar_panel_on_escape,
    hide_tool_when_panel_closed, toolbar_group_interaction, update_tool_button_visuals,
    update_toolbar_group_visuals, update_toolbar_tool_visibility, update_toolbar_tooltip,
};

/// Marca nodos del menu "Construir" para ignorar clics en el mapa cuando el cursor esta encima.
#[derive(Component)]
pub(crate) struct BuildMenuUi;

/// Accion del boton del menu de construccion.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BuildMenuAction {
    Road,
    RoadX,
    RoadY,
    RoadDepot,
    RoadBridge,
    RoadTunnel,
    BusStop,
    Rail,
    RailDepot,
    RailBridge,
    RailTunnel,
    Station,
    Clear,
    Orders,
    BuildHouse,
    BuildCoalMine,
    BuildIronOreMine,
    BuildGoldMine,
    BuildOilWell,
    BuildOilRefinery,
    BuildFactory,
    BuildSawmill,
    BuildForest,
    BuildFarm,
}

#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ToolbarGroup {
    Rail,
    Road,
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
pub(crate) struct ToolbarCloseButton;

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

/// Estado especifico de la herramienta de estacion.
#[derive(Resource, Default)]
pub(crate) struct StationBuildState {
    pub(crate) orientation: u8,
}

#[derive(Resource, Default)]
pub(crate) struct DragBuildState {
    pub(crate) armed: bool,
    pub(crate) start_tile: Option<(i32, i32)>,
    pub(crate) last_tile: Option<(i32, i32)>,
    pub(crate) last_action: Option<BuildMenuAction>,
    pub(crate) pending_tiles: Vec<(i32, i32)>,
}

#[derive(Resource, Default)]
pub(crate) struct OrderEditState {
    pub(crate) vehicle_id: Option<u32>,
    pub(crate) orders: Vec<openttdrs_core::VehicleOrder>,
}

#[derive(Component)]
pub(crate) struct OrderPanelRoot;

#[derive(Component)]
pub(crate) struct OrderPanelText;

#[derive(Component, Clone, Copy)]
pub(crate) enum OrderPanelButton {
    Close,
    ClearLast,
    ClearAll,
}

#[derive(Component, Clone, Copy)]
pub(crate) enum SaveMenuAction {
    SaveAs,
    LoadFrom,
    PauseResume,
    SpeedUp,
    Normalize,
    ZoomIn,
    ZoomOut,
}

#[derive(Resource)]
pub(crate) struct ToolbarState {
    pub(crate) active_group: Option<ToolbarGroup>,
}

impl Default for ToolbarState {
    fn default() -> Self {
        Self { active_group: None }
    }
}

/// Conservado por compatibilidad del pipeline startup; la UI vive en la toolbar superior.
pub(crate) fn setup_build_menu(_commands: Commands) {}
