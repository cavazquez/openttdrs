use bevy::prelude::*;

mod airport_picker_window;
mod bridge_window;
pub(crate) mod build_input;
mod depot_panel;
mod layout;
mod minimap;
mod order_panel;
mod orders_cursor;
mod preview;
mod rail_station_window;
pub(crate) mod rail_type_selector;
pub(crate) mod road_type_selector;
mod settings;
mod signal_picker_window;
mod station_panel;
mod systems;

pub(crate) use airport_picker_window::{
    airport_picker_on_closed, handle_airport_picker_buttons, setup_airport_picker,
    sync_airport_picker,
};
pub(crate) use bridge_window::{
    BridgeBuildState, bridge_picker_on_closed, handle_bridge_picker_buttons, setup_bridge_picker,
    sync_bridge_picker,
};
pub(crate) use build_input::{handle_tile_click, sync_build_pointer_modifiers, update_cursor_tile};
pub(crate) use depot_panel::{
    DepotPanelState, begin_depot_list_drag, depot_panel_on_closed, finish_depot_list_drag,
    handle_depot_panel_buttons, setup_depot_panel, sync_depot_panel,
};
pub(crate) use layout::setup_top_toolbar;
pub(crate) use minimap::{
    MinimapLayerState, MinimapRoot, handle_minimap_click, handle_minimap_layer_buttons,
    setup_minimap, sync_minimap,
};
pub(crate) use order_panel::{
    handle_order_panel_buttons, open_order_edit_for_vehicle, setup_order_panel,
    start_order_destination_pick, sync_order_panel, try_append_order_at_tile,
};
pub(crate) use orders_cursor::sync_orders_pick_cursor;
pub(crate) use preview::{
    BuildGhostPreview, RailSignalGhost, RailSignalGhostState, economy_industry_tool_visible,
    lerp_ghost_previews, rotate_station_with_right_click, update_build_ghost_preview,
};
pub(crate) use rail_station_window::{
    NewGrfStationPreviewCache, StationCatalogPickerState, handle_rail_station_picker_buttons,
    handle_station_catalog_open_buttons, handle_station_class_select_buttons,
    handle_station_spec_select_buttons, rail_station_picker_on_closed, setup_rail_station_picker,
    station_catalog_filter_keyboard, sync_rail_station_picker, sync_station_catalog_entries,
    sync_station_spec_entry_previews,
};
pub(crate) use rail_type_selector::{
    handle_rail_type_select_buttons, sync_rail_type_select_visuals,
};
pub(crate) use road_type_selector::{
    NewGrfRoadTypePreviewCache, RoadTypeEscapeConsumed, RoadTypePickerState,
    close_road_type_picker_on_escape, handle_road_type_class_buttons,
    handle_road_type_select_buttons, road_type_filter_keyboard, sync_road_type_catalog_entries,
    sync_road_type_class_labels, sync_road_type_entry_previews, sync_road_type_entry_visibility,
    sync_road_type_popovers,
};
pub(crate) use settings::{
    handle_company_colour_swatches, handle_settings_menu_buttons,
    sync_company_colour_swatch_visuals,
};
pub(crate) use signal_picker_window::{
    handle_signal_picker_buttons, setup_signal_picker, signal_picker_on_closed, sync_signal_picker,
};
pub(crate) use station_panel::{
    StationCargoPanelState, handle_station_cargo_panel_buttons, handle_station_rename_buttons,
    setup_station_cargo_panel, station_rename_editable_keyboard, station_rename_keyboard,
    sync_station_cargo_panel,
};
pub(crate) use systems::{
    build_menu_interaction, close_toolbar_button_interaction, handle_ingame_escape,
    hide_tool_when_panel_closed, sync_climate_industry_tools, toolbar_click_beep,
    toolbar_group_interaction, update_tool_button_visuals, update_toolbar_group_visuals,
    update_toolbar_tool_visibility, update_toolbar_tooltip,
};

/// Marca nodos del menu "Construir" para ignorar clics en el mapa cuando el cursor esta encima.
#[derive(Component)]
pub(crate) struct BuildMenuUi;

/// Accion del boton del menu de construccion.
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum BuildMenuAction {
    Road,
    RoadX,
    RoadY,
    /// Tranvía autorail (bits automáticos).
    Tram,
    TramX,
    TramY,
    RoadDepot,
    RoadBridge,
    RoadTunnel,
    BusStop,
    /// Waypoint road sobre carretera recta.
    RoadWaypoint,
    RailStation,
    Rail,
    RailX,
    RailY,
    RailHorz,
    RailVert,
    RailDepot,
    RailBridge,
    RailTunnel,
    // Waypoint / señales / quitar vía: cableados al simulador.
    RailWaypoint,
    RailSignals,
    RailRemove,
    /// Convierte el tipo de vía existente (ciclo normal→eléc→mono→maglev).
    RailConvert,
    Station,
    Clear,
    Orders,
    ShipDepot,
    Dock,
    Canal,
    River,
    Buoy,
    Aqueduct,
    Lock,
    Airport,
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
    BuildCottonCandy,
    BuildCandyFactory,
    BuildBatteryFarm,
    BuildColaWells,
    BuildToyFactory,
    BuildPlasticFountain,
    BuildFizzyDrinkFactory,
    BuildBubbleGenerator,
    BuildToffeeQuarry,
    BuildSugarMine,
    RaiseLand,
    LowerLand,
    LevelLand,
    BuyLand,
    /// Plantar árbol en hierba / crecer etapa en bosque.
    PlantTree,
    /// Colocar cartel de texto en el mapa.
    PlaceSign,
    /// Unir dos paradas bus/camión adyacentes (2 clics).
    JoinStation,
}

#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum ToolbarGroup {
    Rail,
    Road,
    Water,
    Air,
    Economy,
    Landscape,
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
    /// Bloquea el siguiente clic de mapa (cierre/selección de menú toolbar).
    pub(crate) block_map_click: bool,
}

/// Estado especifico de la herramienta de estacion.
#[derive(Resource)]
pub(crate) struct StationBuildState {
    pub(crate) orientation: u8,
    /// Selección de estación de tren: eje de los andenes (ventana «Selección de estación»).
    pub(crate) rail_axis_y: bool,
    /// Número de andenes (1..=7).
    pub(crate) rail_platforms: u8,
    /// Longitud de andén (1..=7).
    pub(crate) rail_length: u8,
    /// Mostrar el halo de cobertura al previsualizar la estación de tren.
    pub(crate) rail_show_coverage: bool,
    /// Spec de aeropuerto activo (picker aéreo).
    pub(crate) airport_spec: openttdrs_core::AirportSpecId,
    /// Orientación del footprint aéreo.
    pub(crate) airport_axis_y: bool,
    /// Halo de cobertura al previsualizar aeropuerto.
    pub(crate) airport_show_coverage: bool,
    /// Tipo de señal a colocar (`SIGTYPE_*`; Ctrl cicla block→entry→exit→combo→path→path1vía).
    pub(crate) signal_type: u8,
    /// Densidad de señales al arrastrar (1..=20; OpenTTD default 4).
    pub(crate) signal_density: u8,
    /// Fract de tesela al iniciar arrastre de señales (elige carril HORZ/VERT).
    pub(crate) signal_drag_fract: Option<(u8, u8)>,
    /// Ctrl pulsado (actualizado cada frame para colocación PBS).
    pub(crate) ctrl_held: bool,
    /// Primera estación elegida al unir (herramienta JoinStation).
    pub(crate) join_keep: Option<openttdrs_core::TileCoord>,
}

impl Default for StationBuildState {
    fn default() -> Self {
        Self {
            orientation: 0,
            rail_axis_y: false,
            rail_platforms: 1,
            rail_length: 1,
            rail_show_coverage: true,
            airport_spec: openttdrs_core::AirportSpecId::Small,
            airport_axis_y: false,
            airport_show_coverage: true,
            signal_type: openttdrs_core::SIGTYPE_PATH,
            signal_density: 4,
            signal_drag_fract: None,
            ctrl_held: false,
            join_keep: None,
        }
    }
}

#[derive(Resource, Default)]
pub(crate) struct DragBuildState {
    pub(crate) armed: bool,
    pub(crate) start_tile: Option<(i32, i32)>,
    pub(crate) last_tile: Option<(i32, i32)>,
    pub(crate) last_action: Option<BuildMenuAction>,
    pub(crate) pending_tiles: Vec<(i32, i32)>,
    /// Carril paralelo elegido al iniciar el arrastre (`UPPER`/`LOWER`/`LEFT`/`RIGHT`).
    pub(crate) rail_lane_bit: Option<u8>,
    /// Posición de mundo al `mousedown` (detectar tap vs arrastre real en señales).
    pub(crate) press_world_pos: Option<bevy::math::Vec2>,
}

#[derive(Resource, Default)]
pub(crate) struct OrderEditState {
    pub(crate) vehicle_id: Option<u32>,
    pub(crate) orders: Vec<openttdrs_core::VehicleOrder>,
    /// Fila seleccionada en el panel (para borrar o editar flags).
    pub(crate) selected_slot: Option<usize>,
}

impl OrderEditState {
    pub(crate) fn clear(&mut self) {
        self.vehicle_id = None;
        self.orders.clear();
        self.selected_slot = None;
    }
}

#[derive(Component)]
pub(crate) struct OrderPanelRoot;

#[derive(Component)]
pub(crate) struct OrderPanelTitle;

#[derive(Component, Clone, Copy)]
pub(crate) enum OrderPanelButton {
    Close,
    /// «Ir a»: empieza a elegir el destino de una nueva orden.
    PickDestOnMap,
    /// Borra la orden de la fila seleccionada.
    DeleteSelected,
    /// Salta la orden actual sin cumplirla.
    SkipOrder,
    /// Alterna «carga completa» en la fila seleccionada.
    ToggleFullLoad,
    /// Alterna «no descargar» en la fila seleccionada.
    ToggleNoUnload,
    /// Alterna «parar en depósito» en una orden de depósito.
    ToggleDepotStop,
    /// Abre la ventana de horario (los ajustes de horario viven ahí).
    OpenTimetableWindow,
    /// Sube la orden seleccionada una posición.
    MoveOrderUp,
    /// Baja la orden seleccionada una posición.
    MoveOrderDown,
    /// Crea un pool de órdenes compartidas desde este vehículo.
    ShareOrders,
    /// Desvincula el vehículo de órdenes compartidas.
    UnlinkSharedOrders,
    /// Abre la lista de pools compartidos para vincular.
    OpenSharedOrders,
    /// Añade una orden condicional (carga > umbral).
    AddConditionalAbove,
    /// Añade una orden condicional (carga < umbral).
    AddConditionalBelow,
    /// Cicla condición/umbral de la orden condicional seleccionada.
    CycleConditional,
}

/// Muestra de color de compañía en el panel Ajustes (`0..16`).
#[derive(Component, Clone, Copy)]
pub(crate) struct CompanyColourSwatch(pub u8);

#[derive(Component, Clone, Copy)]
pub(crate) enum SaveMenuAction {
    SaveAs,
    LoadFrom,
    PauseResume,
    SpeedUp,
    Normalize,
    ZoomIn,
    ZoomOut,
    NewsSettings,
    PathfindingSettings,
    NewGrf,
    /// Cicla visible → transparente → oculta.
    CycleCatenaryDisplay,
    /// Opciones de visualización (minimapa, PBS, catenaria, labels…).
    DisplayOptions,
    /// Segunda cámara (ExtraViewport).
    ExtraViewport,
    /// Ayuda / About / hotkeys.
    Help,
    /// Consola / métricas / tools de desarrollo.
    DevConsole,
    /// Inspector de tile seleccionado.
    TileInspector,
    /// Retiro voluntario → endscreen / highscore.
    EndGame,
    ReturnToMainMenu,
}

/// Botón directo en la barra superior: abre la ventana unificada de sonido y música.
#[derive(Component)]
pub(crate) struct SoundMusicToolbarButton;

/// Barra superior de herramientas (visibilidad, tooltip).
#[derive(Resource, Default)]
pub(crate) struct ToolbarState {
    /// Frame de cursor animado (`table/animcursors.h`); sincronizado con `TileAnimClock`.
    pub(crate) anim_cursor_frame: u8,
    pub(crate) active_group: Option<ToolbarGroup>,
}

/// Conservado por compatibilidad del pipeline startup; la UI vive en la toolbar superior.
pub(crate) fn setup_build_menu(_commands: Commands) {}
