use bevy::prelude::*;

mod airport_picker_window;
mod bridge_window;
pub(crate) mod build_input;
mod company_selector;
mod construction_picker_windows;
mod depot_panel;
pub(crate) mod editor_toolbar;
mod icons;
mod layout;
mod minimap;
mod object_picker_window;
mod order_panel;
mod orders_cursor;
mod preview;
mod rail_station_window;
pub(crate) mod rail_type_selector;
mod road_stop_picker_window;
pub(crate) mod road_type_selector;
mod settings;
mod signal_picker_window;
mod station_panel;
mod systems;

pub(crate) use airport_picker_window::{
    airport_picker_on_closed, handle_airport_picker_buttons, setup_airport_picker,
    sync_airport_picker, sync_airport_preview_image,
};
pub(crate) use bridge_window::{
    BridgeBuildState, PendingBridge, bridge_picker_on_closed, handle_bridge_picker_buttons,
    setup_bridge_picker, sync_bridge_picker,
};
pub(crate) use build_input::{handle_tile_click, sync_build_pointer_modifiers, update_cursor_tile};
pub(crate) use company_selector::{handle_company_selector_buttons, sync_company_selector};
pub(crate) use construction_picker_windows::{
    buoy_picker_on_closed, depot_build_picker_on_closed, dock_picker_on_closed,
    handle_depot_build_picker_buttons, handle_dock_picker_buttons, rail_waypoint_picker_on_closed,
    road_waypoint_picker_on_closed, setup_buoy_picker, setup_depot_build_picker, setup_dock_picker,
    setup_rail_waypoint_picker, setup_road_waypoint_picker, setup_sign_picker,
    setup_terraform_picker, setup_tree_picker, sign_picker_on_closed, sync_buoy_picker,
    sync_depot_build_picker, sync_dock_picker, sync_rail_waypoint_picker,
    sync_road_waypoint_picker, sync_sign_picker, sync_terraform_picker, sync_tree_picker,
    terraform_picker_on_closed, tree_picker_on_closed,
};
pub(crate) use depot_panel::{
    DepotPanelState, begin_depot_list_drag, depot_panel_on_closed, finish_depot_list_drag,
    handle_depot_panel_buttons, setup_depot_panel, sync_depot_panel,
};
pub(crate) use editor_toolbar::{
    EditorDocumentState, EditorToolbarLayoutState, EditorTownMenuState,
    handle_editor_exit_confirmation, handle_editor_file_routes,
    handle_editor_toolbar_build_buttons, handle_editor_toolbar_control_buttons,
    handle_editor_toolbar_switch, handle_editor_toolbar_tool_buttons, handle_editor_town_dropdown,
    initialize_editor_document, setup_editor_toolbar, sync_editor_exit_confirmation,
    sync_editor_toolbar_button_visuals, sync_editor_toolbar_date, sync_editor_toolbar_layout,
    sync_editor_toolbar_visibility, sync_editor_town_dropdown,
};
pub(crate) use icons::{Action5GuiIconSlot, ToolbarIcon, sync_action5_gui_toolbar_icons};
pub(crate) use layout::{
    ResponsiveToolbarSlot, ToolbarLayoutState, handle_toolbar_switch, setup_top_toolbar,
    sync_toolbar_layout,
};
pub(crate) use minimap::{
    MinimapLayerState, MinimapRoot, handle_minimap_click, handle_minimap_layer_buttons,
    setup_minimap, sync_minimap,
};
pub(crate) use object_picker_window::{
    handle_object_picker_buttons, object_picker_on_closed, setup_object_picker,
    sync_object_catalog_entries, sync_object_picker, sync_object_preview_image,
};
pub(crate) use order_panel::{
    begin_order_list_drag, finish_order_list_drag, handle_order_panel_buttons,
    open_order_edit_for_vehicle, order_panel_on_closed, setup_order_panel,
    start_order_destination_pick, sync_order_panel, try_append_order_at_tile,
};
pub(crate) use orders_cursor::sync_orders_pick_cursor;
pub(crate) use preview::{
    BuildGhostPreview, RailSignalGhost, RailSignalGhostState, economy_industry_tool_visible,
    lerp_ghost_previews, rotate_station_with_right_click, update_build_ghost_preview,
};
pub(crate) use rail_station_window::{
    NewGrfStationPreviewCache, StationCatalogKind, StationCatalogPickerState,
    handle_rail_station_picker_buttons, handle_station_catalog_open_buttons,
    handle_station_class_select_buttons, handle_station_spec_select_buttons,
    rail_station_picker_on_closed, setup_rail_station_picker, station_catalog_filter_keyboard,
    sync_rail_station_picker, sync_station_catalog_entries, sync_station_spec_entry_previews,
};
pub(crate) use rail_type_selector::{
    handle_rail_type_select_buttons, sync_rail_toolbar_icons, sync_rail_type_select_visuals,
};
pub(crate) use road_stop_picker_window::{
    handle_road_stop_picker_buttons, road_stop_picker_on_closed, setup_road_stop_picker,
    sync_road_stop_catalog_entries, sync_road_stop_picker, sync_road_stop_preview_image,
};
pub(crate) use road_type_selector::{
    NewGrfRoadTypePreviewCache, RoadTypeEscapeConsumed, RoadTypePickerState,
    close_road_type_picker_on_escape, handle_road_type_class_buttons,
    handle_road_type_select_buttons, road_type_filter_keyboard, sync_road_type_catalog_entries,
    sync_road_type_class_labels, sync_road_type_entry_previews, sync_road_type_entry_visibility,
    sync_road_type_popovers,
};
pub(crate) use settings::{
    handle_cheats_menu_button, handle_company_colour_swatches,
    handle_road_driving_side_menu_button, handle_settings_menu_buttons,
    handle_vehicle_breakdowns_menu_button, sync_company_colour_swatch_visuals,
    sync_road_driving_side_button_label, sync_vehicle_breakdowns_button_label,
};
pub(crate) use signal_picker_window::{
    handle_signal_picker_buttons, setup_signal_picker, signal_picker_on_closed, sync_signal_picker,
};
pub(crate) use station_panel::{
    StationCargoPanelState, handle_station_cargo_panel_buttons, handle_station_rename_buttons,
    setup_station_cargo_panel, station_rename_editable_keyboard, station_rename_keyboard,
    station_view_on_closed, sync_station_cargo_panel,
};
pub(crate) use systems::{
    build_menu_interaction, close_toolbar_button_interaction, handle_ingame_escape,
    hide_tool_when_panel_closed, sync_climate_industry_tools, sync_editor_only_build_tools,
    toolbar_click_beep, toolbar_group_interaction, update_tool_button_visuals,
    update_toolbar_group_visuals, update_toolbar_tool_visibility, update_toolbar_tooltip,
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
    /// Quitar overlay de tranvía.
    TramRemove,
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
    /// Convierte la vía existente al tipo seleccionado (`current_rail_type`).
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
    /// Fundar un pueblo nuevo en hierba (CmdFoundTown).
    FoundTown,
    BuildCoalMine,
    BuildIronOreMine,
    BuildGoldMine,
    BuildOilWell,
    BuildOilRefinery,
    BuildFactory,
    BuildSawmill,
    BuildForest,
    BuildFarm,
    BuildFarmTropic,
    BuildCopperOreMine,
    BuildFactoryTropic,
    BuildFruitPlantation,
    BuildRubberPlantation,
    BuildPaperMill,
    BuildFoodProcessingPlant,
    BuildDiamondMine,
    BuildWaterSupply,
    BuildLumberMill,
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
    /// Faro vanilla (`OBJECT_TYPE_LIGHTHOUSE`).
    BuildLighthouse,
    /// Transmisor vanilla (`OBJECT_TYPE_TRANSMITTER`).
    BuildTransmitter,
    /// Colocar objeto NewGRF / seleccionado en el picker (`current_object_spec`).
    PlaceNewGrfObject,
    /// Unir dos paradas bus/camión adyacentes (2 clics).
    JoinStation,
}

#[allow(dead_code)] // inventarios UI-0 consumidos en tests
impl BuildMenuAction {
    /// Inventario estable UI-0 (#30): actualizar al añadir variantes.
    pub(crate) const ALL: &[Self] = &[
        Self::Road,
        Self::RoadX,
        Self::RoadY,
        Self::Tram,
        Self::TramX,
        Self::TramY,
        Self::TramRemove,
        Self::RoadDepot,
        Self::RoadBridge,
        Self::RoadTunnel,
        Self::BusStop,
        Self::RoadWaypoint,
        Self::RailStation,
        Self::Rail,
        Self::RailX,
        Self::RailY,
        Self::RailHorz,
        Self::RailVert,
        Self::RailDepot,
        Self::RailBridge,
        Self::RailTunnel,
        Self::RailWaypoint,
        Self::RailSignals,
        Self::RailRemove,
        Self::RailConvert,
        Self::Station,
        Self::Clear,
        Self::Orders,
        Self::ShipDepot,
        Self::Dock,
        Self::Canal,
        Self::River,
        Self::Buoy,
        Self::Aqueduct,
        Self::Lock,
        Self::Airport,
        Self::BuildHouse,
        Self::FoundTown,
        Self::BuildCoalMine,
        Self::BuildIronOreMine,
        Self::BuildGoldMine,
        Self::BuildOilWell,
        Self::BuildOilRefinery,
        Self::BuildFactory,
        Self::BuildSawmill,
        Self::BuildForest,
        Self::BuildFarm,
        Self::BuildFarmTropic,
        Self::BuildCopperOreMine,
        Self::BuildFactoryTropic,
        Self::BuildFruitPlantation,
        Self::BuildRubberPlantation,
        Self::BuildPaperMill,
        Self::BuildFoodProcessingPlant,
        Self::BuildDiamondMine,
        Self::BuildWaterSupply,
        Self::BuildLumberMill,
        Self::BuildCottonCandy,
        Self::BuildCandyFactory,
        Self::BuildBatteryFarm,
        Self::BuildColaWells,
        Self::BuildToyFactory,
        Self::BuildPlasticFountain,
        Self::BuildFizzyDrinkFactory,
        Self::BuildBubbleGenerator,
        Self::BuildToffeeQuarry,
        Self::BuildSugarMine,
        Self::RaiseLand,
        Self::LowerLand,
        Self::LevelLand,
        Self::BuyLand,
        Self::PlantTree,
        Self::PlaceSign,
        Self::BuildLighthouse,
        Self::BuildTransmitter,
        Self::PlaceNewGrfObject,
        Self::JoinStation,
    ];

    /// Herramientas de diseño de escenario: solo visibles/usables con editor activo.
    #[must_use]
    pub(crate) const fn is_editor_only(self) -> bool {
        matches!(
            self,
            Self::BuildHouse
                | Self::FoundTown
                | Self::River
                | Self::BuildCoalMine
                | Self::BuildIronOreMine
                | Self::BuildGoldMine
                | Self::BuildOilWell
                | Self::BuildOilRefinery
                | Self::BuildFactory
                | Self::BuildSawmill
                | Self::BuildForest
                | Self::BuildFarm
                | Self::BuildFarmTropic
                | Self::BuildCopperOreMine
                | Self::BuildFactoryTropic
                | Self::BuildFruitPlantation
                | Self::BuildRubberPlantation
                | Self::BuildPaperMill
                | Self::BuildFoodProcessingPlant
                | Self::BuildDiamondMine
                | Self::BuildWaterSupply
                | Self::BuildLumberMill
                | Self::BuildCottonCandy
                | Self::BuildCandyFactory
                | Self::BuildBatteryFarm
                | Self::BuildColaWells
                | Self::BuildToyFactory
                | Self::BuildPlasticFountain
                | Self::BuildFizzyDrinkFactory
                | Self::BuildBubbleGenerator
                | Self::BuildToffeeQuarry
                | Self::BuildSugarMine
        )
    }
}

#[cfg(test)]
mod editor_only_tests {
    use super::BuildMenuAction;

    #[test]
    fn editor_only_tools_cover_town_house_river_industries() {
        assert!(BuildMenuAction::FoundTown.is_editor_only());
        assert!(BuildMenuAction::BuildHouse.is_editor_only());
        assert!(BuildMenuAction::River.is_editor_only());
        assert!(BuildMenuAction::BuildCoalMine.is_editor_only());
        assert!(!BuildMenuAction::Rail.is_editor_only());
        assert!(!BuildMenuAction::RaiseLand.is_editor_only());
        assert!(!BuildMenuAction::PlantTree.is_editor_only());
    }
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

#[allow(dead_code)] // inventarios UI-0 consumidos en tests
impl ToolbarGroup {
    /// Inventario estable UI-0 (#30).
    pub(crate) const ALL: &[Self] = &[
        Self::Rail,
        Self::Road,
        Self::Water,
        Self::Air,
        Self::Economy,
        Self::Landscape,
        Self::Info,
        Self::Settings,
    ];
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
    /// Variante visual a colocar: 0=eléctrica, 1=semáforo.
    pub(crate) signal_variant: u8,
    /// Densidad de señales al arrastrar (1..=20; OpenTTD default 4).
    pub(crate) signal_density: u8,
    /// Fract de tesela al iniciar arrastre de señales (elige carril HORZ/VERT).
    pub(crate) signal_drag_fract: Option<(u8, u8)>,
    /// Ctrl pulsado (actualizado cada frame para colocación PBS).
    pub(crate) ctrl_held: bool,
    /// Shift pulsado; Ctrl+Shift+clic alterna eléctrica / semáforo.
    pub(crate) shift_held: bool,
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
            signal_variant: 0,
            signal_density: 4,
            signal_drag_fract: None,
            ctrl_held: false,
            shift_held: false,
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

/// Estado de un panel de órdenes (un vehículo / chain slot).
#[derive(Clone, Debug, Default)]
pub(crate) struct OrderSlotState {
    pub(crate) vehicle_id: Option<u32>,
    pub(crate) orders: Vec<openttdrs_core::VehicleOrder>,
    /// Fila seleccionada en el panel (para borrar o editar flags).
    pub(crate) selected_slot: Option<usize>,
    /// Origen de drag nativo para reordenar (`MoveVehicleOrder`, #194).
    pub(crate) list_drag_from: Option<usize>,
}

/// Multi-instancia (#244): hasta 2 paneles de órdenes concurrentes.
#[derive(Resource, Debug, Default)]
pub(crate) struct OrderEditState {
    pub(crate) slots: [OrderSlotState; crate::ui::vehicle_chain::MAX_VEHICLE_CHAIN_SLOTS],
    /// Vehículo enfocado (handlers / pick en mapa).
    pub(crate) focused: Option<u32>,
}

impl OrderEditState {
    /// Compat: vehicle_id del panel enfocado.
    #[must_use]
    pub(crate) fn vehicle_id(&self) -> Option<u32> {
        self.focused
            .filter(|&id| self.slots.iter().any(|s| s.vehicle_id == Some(id)))
    }

    #[must_use]
    pub(crate) fn focused_slot(&self) -> Option<&OrderSlotState> {
        let id = self.focused?;
        self.slots.iter().find(|s| s.vehicle_id == Some(id))
    }

    pub(crate) fn focused_slot_mut(&mut self) -> Option<&mut OrderSlotState> {
        let id = self.focused?;
        self.slots.iter_mut().find(|s| s.vehicle_id == Some(id))
    }

    /// Compat: órdenes del panel enfocado.
    #[must_use]
    pub(crate) fn orders(&self) -> &[openttdrs_core::VehicleOrder] {
        self.focused_slot()
            .map(|s| s.orders.as_slice())
            .unwrap_or(&[])
    }

    pub(crate) fn orders_mut(&mut self) -> Option<&mut Vec<openttdrs_core::VehicleOrder>> {
        self.focused_slot_mut().map(|s| &mut s.orders)
    }

    #[must_use]
    pub(crate) fn selected_slot(&self) -> Option<usize> {
        self.focused_slot().and_then(|s| s.selected_slot)
    }

    pub(crate) fn set_selected_slot(&mut self, slot: Option<usize>) {
        if let Some(s) = self.focused_slot_mut() {
            s.selected_slot = slot;
        }
    }

    #[must_use]
    pub(crate) fn list_drag_from(&self) -> Option<usize> {
        self.focused_slot().and_then(|s| s.list_drag_from)
    }

    pub(crate) fn set_list_drag_from(&mut self, from: Option<usize>) {
        if let Some(s) = self.focused_slot_mut() {
            s.list_drag_from = from;
        }
    }

    pub(crate) fn bind_slot(
        &mut self,
        chain_slot: u8,
        vehicle_id: u32,
        orders: Vec<openttdrs_core::VehicleOrder>,
        selected_slot: Option<usize>,
    ) {
        let idx = chain_slot as usize;
        if idx >= crate::ui::vehicle_chain::MAX_VEHICLE_CHAIN_SLOTS {
            return;
        }
        self.slots[idx] = OrderSlotState {
            vehicle_id: Some(vehicle_id),
            orders,
            selected_slot,
            list_drag_from: None,
        };
        self.focused = Some(vehicle_id);
    }

    pub(crate) fn close_vehicle(&mut self, vehicle_id: u32) {
        for slot in &mut self.slots {
            if slot.vehicle_id == Some(vehicle_id) {
                *slot = OrderSlotState::default();
            }
        }
        if self.focused == Some(vehicle_id) {
            self.focused = self.slots.iter().find_map(|s| s.vehicle_id);
        }
    }

    pub(crate) fn clear(&mut self) {
        *self = Self::default();
    }

    #[must_use]
    pub(crate) fn is_open_for(&self, vehicle_id: u32) -> bool {
        self.slots.iter().any(|s| s.vehicle_id == Some(vehicle_id))
    }
}

/// Marcador en la raíz flotante de órdenes (cleanup / queries).
#[derive(Component)]
pub(crate) struct OrderPanelRoot;

#[derive(Component, Clone, Copy)]
pub(crate) enum OrderPanelButton {
    /// «Ir a»: empieza a elegir el destino de una nueva orden.
    PickDestOnMap,
    /// Borra la orden de la fila seleccionada.
    DeleteSelected,
    /// Salta la orden actual sin cumplirla.
    SkipOrder,
    /// Cicla el tipo de carga en la fila seleccionada.
    ToggleFullLoad,
    /// Alterna paradas intermedias en una orden de estación.
    ToggleNonStop,
    /// Cicla la posición de parada en el andén.
    CycleStopLocation,
    /// Cicla el tipo de descarga en la fila seleccionada.
    ToggleNoUnload,
    /// Alterna «parar en depósito» en una orden de depósito.
    ToggleDepotStop,
    /// Cicla el refit automático de una orden de depósito.
    CycleDepotRefit,
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
    SlowDown,
    SpeedUp,
    Normalize,
    ZoomIn,
    ZoomOut,
    NewsSettings,
    PathfindingSettings,
    CycleVehicleBreakdowns,
    /// Cicla circulación vial izquierda / derecha (`vehicle.road_side`).
    ToggleRoadDrivingSide,
    CargoDistSettings,
    AiSettings,
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
    /// Ventana formal de cheats (#45).
    Cheats,
    /// Guardar JSON en `save/scenarios/` (editor #42).
    SaveScenario,
    /// Retiro voluntario → endscreen / highscore.
    EndGame,
    ReturnToMainMenu,
}

#[allow(dead_code)] // inventarios UI-0 consumidos en tests
impl SaveMenuAction {
    /// Inventario estable UI-0 (#30).
    pub(crate) const ALL: &[Self] = &[
        Self::SaveAs,
        Self::LoadFrom,
        Self::PauseResume,
        Self::SlowDown,
        Self::SpeedUp,
        Self::Normalize,
        Self::ZoomIn,
        Self::ZoomOut,
        Self::NewsSettings,
        Self::PathfindingSettings,
        Self::CycleVehicleBreakdowns,
        Self::ToggleRoadDrivingSide,
        Self::CargoDistSettings,
        Self::AiSettings,
        Self::NewGrf,
        Self::CycleCatenaryDisplay,
        Self::DisplayOptions,
        Self::ExtraViewport,
        Self::Help,
        Self::DevConsole,
        Self::TileInspector,
        Self::Cheats,
        Self::SaveScenario,
        Self::EndGame,
        Self::ReturnToMainMenu,
    ];
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
