//! Verificación visual automatizada de las ventanas flotantes.
//!
//! Con `OPENTTDRS_WINDOWS_SHOT=/ruta/captura.png` el cliente abre **todas** las
//! [`FloatingWindowId`] (más SaveWindow / OrderPanel / paneles auxiliares),
//! guarda una captura y sale. `OPENTTDRS_WINDOW_SHOT_ID=Town` limita la captura
//! a una sola entrada de [`WINDOW_PARITY_MATRIX`].
//!
//! Inventario cubierto: ver [`windows_shot_covered_ids`] (debe == `FloatingWindowId::ALL`).
//!
//! Resolución opcional: `OPENTTDRS_SHOT_RES=1280x720` o `1920x1080`.
//! Escala opcional: `OPENTTDRS_SHOT_UI_SCALE=1` o `2`.
//! Para `TownAuthority`, `OPENTTDRS_TOWN_AUTHORITY_SHOT_STATE=normal|no-funds|unavailable`
//! prepara estados reproducibles para el oráculo visual de #295.

use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use bevy::ui::UiScale;
use bevy::window::PrimaryWindow;
use openttdrs_core::prelude::*;
use std::fmt::Write as _;

use crate::bevy_app::UpdateSet;
use crate::state::{ClientScreen, SimWorld};
use crate::ui::ai_settings_window::AiSettingsWindowState;
use crate::ui::audio_settings_window::SoundMusicWindowState;
use crate::ui::autoreplace_window::AutoreplaceWindowState;
use crate::ui::buy_window::BuyVehicleWindowState;
use crate::ui::cargo_payment_window::CargoPaymentWindowState;
use crate::ui::cheat_window::CheatWindowState;
use crate::ui::company_view_window::CompanyViewWindowState;
use crate::ui::destination_window::DestinationPickerState;
use crate::ui::dev_console::DevConsoleState;
use crate::ui::dialog_windows::{
    ErrorDialogWindowState, OskWindowState, QueryStringWindowState, open_error_modal,
    open_osk_for_query, open_query_for_newgrf_rename,
};
use crate::ui::display_options_window::DisplayOptionsWindowState;
use crate::ui::extra_viewport_window::ExtraViewportWindowState;
use crate::ui::finances_window::FinancesWindowState;
use crate::ui::floating_window::{FloatingWindow, FloatingWindowId, WindowKey};
use crate::ui::genland_window::GenLandWindowState;
use crate::ui::goal_list_window::GoalListWindowState;
use crate::ui::graph_window::GraphWindowState;
use crate::ui::help_window::HelpWindowState;
use crate::ui::hud::SimHudControls;
use crate::ui::industry_directory::IndustryDirectoryState;
use crate::ui::industry_panel::IndustryPanelState;
use crate::ui::industry_production_window::IndustryProductionWindowState;
use crate::ui::league_window::LeagueWindowState;
use crate::ui::main_menu::{MainMenuCamera, MainMenuUi};
use crate::ui::modal_stack::ModalStack;
use crate::ui::newgrf_window::NewGrfWindowState;
use crate::ui::news_settings_window::NewsSettingsWindowState;
use crate::ui::pathfinding_settings_window::PathfindingSettingsWindowState;
use crate::ui::refit_window::RefitWindowState;
use crate::ui::save_window::{SaveWindowMode, SaveWindowState, save_dir_from};
use crate::ui::shared_orders_window::SharedOrdersWindowState;
use crate::ui::sign_list_window::SignListWindowState;
use crate::ui::station_directory::StationDirectoryState;
use crate::ui::statusbar::NewsHistoryState;
use crate::ui::story_window::StoryWindowState;
use crate::ui::subsidy_list::SubsidyListState;
use crate::ui::tile_inspector_window::TileInspectorWindowState;
use crate::ui::timetable_window::TimetableWindowState;
use crate::ui::toolbar::{
    BridgeBuildState, BuildMenuAction, DepotPanelState, OrderEditState, PendingBridge,
    StationCargoPanelState, StationCatalogKind, StationCatalogPickerState, ToolbarGroup,
    ToolbarState, UiToolState,
};
use crate::ui::town_authority_window::TownAuthorityWindowState;
use crate::ui::town_directory::TownDirectoryState;
use crate::ui::town_window::TownWindowState;
use crate::ui::ui5_blocked_stubs::LinkGraphWindowState;
use crate::ui::vehicle_chain::VehicleChainRegistry;
use crate::ui::vehicle_details_window::VehicleDetailsWindowState;
use crate::ui::vehicle_list::VehicleListState;
use crate::ui::vehicle_window::VehicleWindowState;

const OPEN_FRAME: u32 = 30;
const SHOT_FRAME: u32 = 60;
const EXIT_FRAME: u32 = 120;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TownAuthorityShotState {
    Normal,
    NoFunds,
    Unavailable,
}

fn town_authority_shot_state() -> TownAuthorityShotState {
    parse_town_authority_shot_state(
        std::env::var("OPENTTDRS_TOWN_AUTHORITY_SHOT_STATE")
            .ok()
            .as_deref(),
    )
}

fn parse_town_authority_shot_state(raw: Option<&str>) -> TownAuthorityShotState {
    match raw {
        Some("no-funds") => TownAuthorityShotState::NoFunds,
        Some("unavailable") => TownAuthorityShotState::Unavailable,
        _ => TownAuthorityShotState::Normal,
    }
}

/// Inventario de `FloatingWindowId` que `windows_shot` intenta abrir.
/// Debe coincidir con [`FloatingWindowId::ALL`] (test de cobertura).
#[allow(dead_code)] // consumido en tests de inventario
#[must_use]
pub(crate) fn windows_shot_covered_ids() -> &'static [FloatingWindowId] {
    FloatingWindowId::ALL
}

/// Estado de la ruta equivalente en OpenTTD 15.3.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WindowParityKind {
    /// Ventana directamente comparable con una clase upstream.
    Upstream,
    /// UI propia del port: se inventaría, pero no se fuerza una falsa equivalencia.
    Extension,
}

/// Entrada del inventario verificable de ventanas (#240).
#[derive(Clone, Copy, Debug)]
pub(crate) struct WindowParityEntry {
    pub(crate) id: FloatingWindowId,
    pub(crate) family: &'static str,
    pub(crate) upstream_source: &'static str,
    pub(crate) upstream_window: &'static str,
    pub(crate) parent: Option<FloatingWindowId>,
    pub(crate) kind: WindowParityKind,
}

macro_rules! upstream_window {
    ($id:ident, $family:literal, $source:literal, $window:literal) => {
        WindowParityEntry {
            id: FloatingWindowId::$id,
            family: $family,
            upstream_source: $source,
            upstream_window: $window,
            parent: None,
            kind: WindowParityKind::Upstream,
        }
    };
    ($id:ident, $family:literal, $source:literal, $window:literal, $parent:ident) => {
        WindowParityEntry {
            id: FloatingWindowId::$id,
            family: $family,
            upstream_source: $source,
            upstream_window: $window,
            parent: Some(FloatingWindowId::$parent),
            kind: WindowParityKind::Upstream,
        }
    };
}

macro_rules! extension_window {
    ($id:ident, $family:literal) => {
        WindowParityEntry {
            id: FloatingWindowId::$id,
            family: $family,
            upstream_source: "",
            upstream_window: "",
            parent: None,
            kind: WindowParityKind::Extension,
        }
    };
}

/// Matriz OpenTTD 15.3 commit `14ec60f`: cada ventana del port aparece una vez.
pub(crate) const WINDOW_PARITY_MATRIX: &[WindowParityEntry] = &[
    upstream_window!(Town, "world", "town_gui.cpp", "WC_TOWN_VIEW"),
    upstream_window!(
        TownAuthority,
        "world",
        "town_gui.cpp",
        "WC_TOWN_AUTHORITY",
        Town
    ),
    upstream_window!(TownDirectory, "world", "town_gui.cpp", "WC_TOWN_DIRECTORY"),
    upstream_window!(
        IndustryDirectory,
        "world",
        "industry_gui.cpp",
        "WC_INDUSTRY_DIRECTORY"
    ),
    upstream_window!(Industry, "world", "industry_gui.cpp", "WC_INDUSTRY_VIEW"),
    upstream_window!(
        IndustryProduction,
        "world",
        "graph_gui.cpp",
        "WC_INDUSTRY_PRODUCTION",
        Industry
    ),
    upstream_window!(
        StationDirectory,
        "world",
        "station_gui.cpp",
        "WC_STATION_LIST"
    ),
    upstream_window!(Station, "world", "station_gui.cpp", "WC_STATION_VIEW"),
    upstream_window!(
        VehicleList,
        "vehicles",
        "vehicle_gui.cpp",
        "WC_TRAINS_LIST/WC_ROADVEH_LIST/WC_SHIPS_LIST/WC_AIRCRAFT_LIST"
    ),
    upstream_window!(
        SubsidyList,
        "economy",
        "subsidy_gui.cpp",
        "WC_SUBSIDIES_LIST"
    ),
    upstream_window!(Depot, "vehicles", "depot_gui.cpp", "WC_VEHICLE_DEPOT"),
    upstream_window!(
        BuyVehicle,
        "vehicles",
        "build_vehicle_gui.cpp",
        "WC_BUILD_VEHICLE",
        Depot
    ),
    upstream_window!(Vehicle, "vehicles", "vehicle_gui.cpp", "WC_VEHICLE_VIEW"),
    upstream_window!(
        VehicleDetails,
        "vehicles",
        "vehicle_gui.cpp",
        "WC_VEHICLE_DETAILS",
        Vehicle
    ),
    upstream_window!(
        RailStationPicker,
        "construction",
        "rail_gui.cpp",
        "WC_BUILD_STATION"
    ),
    upstream_window!(
        AirportPicker,
        "construction",
        "airport_gui.cpp",
        "WC_BUILD_STATION"
    ),
    upstream_window!(
        RoadStopPicker,
        "construction",
        "road_gui.cpp",
        "WC_BUILD_STATION"
    ),
    upstream_window!(
        ObjectPicker,
        "construction",
        "object_gui.cpp",
        "WC_BUILD_OBJECT"
    ),
    upstream_window!(
        BridgePicker,
        "construction",
        "bridge_gui.cpp",
        "WC_BUILD_BRIDGE"
    ),
    upstream_window!(DockPicker, "construction", "dock_gui.cpp", "WC_BUILD_DOCK"),
    upstream_window!(BuoyPicker, "construction", "dock_gui.cpp", "WC_BUILD_BUOY"),
    extension_window!(RailWaypointPicker, "construction"),
    extension_window!(RoadWaypointPicker, "construction"),
    extension_window!(TreePicker, "editor"),
    extension_window!(TerraformPicker, "editor"),
    extension_window!(SignPicker, "world"),
    extension_window!(DepotBuildPicker, "construction"),
    upstream_window!(
        DestinationPicker,
        "vehicles",
        "station_gui.cpp",
        "WC_SELECT_STATION",
        Orders
    ),
    upstream_window!(NewsHistory, "reports", "news_gui.cpp", "WC_MESSAGE_HISTORY"),
    upstream_window!(Finances, "economy", "company_gui.cpp", "WC_FINANCES"),
    upstream_window!(CompanyView, "economy", "company_gui.cpp", "WC_COMPANY"),
    upstream_window!(
        NewsSettings,
        "reports",
        "news_gui.cpp",
        "WC_MESSAGE_OPTIONS"
    ),
    upstream_window!(
        PathfindingSettings,
        "settings",
        "settings_gui.cpp",
        "WC_GAME_OPTIONS"
    ),
    upstream_window!(
        CargoDistSettings,
        "settings",
        "settings_gui.cpp",
        "WC_GAME_OPTIONS"
    ),
    upstream_window!(AiSettings, "settings", "ai/ai_gui.cpp", "WC_GAME_OPTIONS"),
    upstream_window!(
        NewGrf,
        "settings",
        "newgrf_gui.cpp",
        "WC_GAME_OPTIONS/GS_NEWGRF"
    ),
    upstream_window!(SoundMusic, "settings", "music_gui.cpp", "WC_MUSIC_WINDOW"),
    upstream_window!(
        Timetable,
        "vehicles",
        "timetable_gui.cpp",
        "WC_VEHICLE_TIMETABLE",
        Vehicle
    ),
    upstream_window!(
        Orders,
        "vehicles",
        "order_gui.cpp",
        "WC_VEHICLE_ORDERS",
        Vehicle
    ),
    upstream_window!(
        Refit,
        "vehicles",
        "vehicle_gui.cpp",
        "WC_VEHICLE_REFIT",
        Vehicle
    ),
    upstream_window!(
        SharedOrders,
        "vehicles",
        "vehicle_gui.cpp",
        "WC_VEHICLE_LIST"
    ),
    upstream_window!(
        Autoreplace,
        "vehicles",
        "autoreplace_gui.cpp",
        "WC_REPLACE_VEHICLE"
    ),
    upstream_window!(GraphIncome, "economy", "graph_gui.cpp", "WC_INCOME_GRAPH"),
    upstream_window!(
        GraphOperatingProfit,
        "economy",
        "graph_gui.cpp",
        "WC_OPERATING_PROFIT_GRAPH"
    ),
    upstream_window!(
        GraphCompanyValue,
        "economy",
        "graph_gui.cpp",
        "WC_COMPANY_VALUE"
    ),
    upstream_window!(
        CargoPaymentRates,
        "economy",
        "graph_gui.cpp",
        "WC_PAYMENT_RATES"
    ),
    upstream_window!(
        DisplayOptions,
        "settings",
        "settings_gui.cpp",
        "WC_GAME_OPTIONS"
    ),
    upstream_window!(
        ExtraViewport,
        "world",
        "viewport_gui.cpp",
        "WC_EXTRA_VIEW_PORT"
    ),
    upstream_window!(SignList, "world", "signs_gui.cpp", "WC_SIGN_LIST"),
    upstream_window!(
        LinkGraphLegend,
        "world",
        "linkgraph/linkgraph_gui.cpp",
        "WC_LINKGRAPH_LEGEND"
    ),
    upstream_window!(
        SignalPicker,
        "construction",
        "rail_gui.cpp",
        "WC_BUILD_SIGNAL"
    ),
    upstream_window!(Help, "settings", "help_gui.cpp", "WC_HELPWIN"),
    extension_window!(DevConsole, "debug"),
    upstream_window!(TileInspector, "debug", "misc_gui.cpp", "WC_LAND_INFO"),
    upstream_window!(CheatWindow, "settings", "cheat_gui.cpp", "WC_CHEATS"),
    upstream_window!(
        GenLand,
        "editor",
        "genworld_gui.cpp",
        "WC_GENERATE_LANDSCAPE"
    ),
    upstream_window!(Goals, "gamescript", "goal_gui.cpp", "WC_GOALS_LIST"),
    upstream_window!(Story, "gamescript", "story_gui.cpp", "WC_STORY_BOOK"),
    upstream_window!(League, "gamescript", "league_gui.cpp", "WC_LEAGUE"),
    upstream_window!(
        QueryString,
        "dialogs",
        "querystring_gui.cpp",
        "WC_QUERY_STRING"
    ),
    upstream_window!(ErrorDialog, "dialogs", "error_gui.cpp", "WC_ERRMSG"),
    upstream_window!(OnScreenKeyboard, "dialogs", "osk_gui.cpp", "WC_OSK"),
];

/// Path relativo al crate client (`src/`) de la implementación Rust (#240).
#[must_use]
pub(crate) const fn window_rust_impl(id: FloatingWindowId) -> &'static str {
    match id {
        FloatingWindowId::Town => "ui/town_window.rs",
        FloatingWindowId::TownAuthority => "ui/town_authority_window.rs",
        FloatingWindowId::TownDirectory => "ui/town_directory.rs",
        FloatingWindowId::IndustryDirectory => "ui/industry_directory.rs",
        FloatingWindowId::Industry => "ui/industry_panel/mod.rs",
        FloatingWindowId::IndustryProduction => "ui/industry_production_window.rs",
        FloatingWindowId::StationDirectory => "ui/station_directory.rs",
        FloatingWindowId::Station => "ui/toolbar/station_panel.rs",
        FloatingWindowId::VehicleList => "ui/vehicle_list.rs",
        FloatingWindowId::SubsidyList => "ui/subsidy_list.rs",
        FloatingWindowId::Depot => "ui/toolbar/mod.rs",
        FloatingWindowId::BuyVehicle => "ui/buy_window.rs",
        FloatingWindowId::Vehicle => "ui/vehicle_window/mod.rs",
        FloatingWindowId::VehicleDetails => "ui/vehicle_details_window/mod.rs",
        FloatingWindowId::RailStationPicker => "ui/toolbar/rail_station_window.rs",
        FloatingWindowId::AirportPicker => "ui/toolbar/airport_picker_window.rs",
        FloatingWindowId::RoadStopPicker => "ui/toolbar/road_stop_picker_window.rs",
        FloatingWindowId::ObjectPicker => "ui/toolbar/object_picker_window.rs",
        FloatingWindowId::BridgePicker => "ui/toolbar/bridge_window.rs",
        FloatingWindowId::DockPicker => "ui/toolbar/construction_picker_windows.rs",
        FloatingWindowId::BuoyPicker => "ui/toolbar/construction_picker_windows.rs",
        FloatingWindowId::RailWaypointPicker => "ui/toolbar/construction_picker_windows.rs",
        FloatingWindowId::RoadWaypointPicker => "ui/toolbar/construction_picker_windows.rs",
        FloatingWindowId::TreePicker => "ui/toolbar/construction_picker_windows.rs",
        FloatingWindowId::TerraformPicker => "ui/toolbar/construction_picker_windows.rs",
        FloatingWindowId::SignPicker => "ui/toolbar/construction_picker_windows.rs",
        FloatingWindowId::DepotBuildPicker => "ui/toolbar/construction_picker_windows.rs",
        FloatingWindowId::DestinationPicker => "ui/destination_window.rs",
        FloatingWindowId::NewsHistory => "ui/statusbar/history.rs",
        FloatingWindowId::Finances => "ui/finances_window.rs",
        FloatingWindowId::CompanyView => "ui/company_view_window.rs",
        FloatingWindowId::NewsSettings => "ui/news_settings_window.rs",
        FloatingWindowId::PathfindingSettings => "ui/pathfinding_settings_window.rs",
        FloatingWindowId::CargoDistSettings => "ui/cargo_dist_settings_window.rs",
        FloatingWindowId::AiSettings => "ui/ai_settings_window.rs",
        FloatingWindowId::NewGrf => "ui/newgrf_window.rs",
        FloatingWindowId::SoundMusic => "ui/audio_settings_window.rs",
        FloatingWindowId::Timetable => "ui/timetable_window.rs",
        FloatingWindowId::Orders => "ui/toolbar/mod.rs",
        FloatingWindowId::Refit => "ui/refit_window.rs",
        FloatingWindowId::SharedOrders => "ui/shared_orders_window.rs",
        FloatingWindowId::Autoreplace => "ui/autoreplace_window.rs",
        FloatingWindowId::GraphIncome => "ui/graph_window.rs",
        FloatingWindowId::GraphOperatingProfit => "ui/graph_window.rs",
        FloatingWindowId::GraphCompanyValue => "ui/graph_window.rs",
        FloatingWindowId::CargoPaymentRates => "ui/cargo_payment_window.rs",
        FloatingWindowId::DisplayOptions => "ui/display_options_window.rs",
        FloatingWindowId::ExtraViewport => "ui/extra_viewport_window.rs",
        FloatingWindowId::SignList => "ui/sign_list_window.rs",
        FloatingWindowId::LinkGraphLegend => "ui/ui5_blocked_stubs.rs",
        FloatingWindowId::SignalPicker => "ui/toolbar/signal_picker_window.rs",
        FloatingWindowId::Help => "ui/help_window.rs",
        FloatingWindowId::DevConsole => "ui/dev_console.rs",
        FloatingWindowId::TileInspector => "ui/tile_inspector_window.rs",
        FloatingWindowId::CheatWindow => "ui/cheat_window.rs",
        FloatingWindowId::GenLand => "ui/genland_window.rs",
        FloatingWindowId::Goals => "ui/goal_list_window.rs",
        FloatingWindowId::Story => "ui/story_window.rs",
        FloatingWindowId::League => "ui/league_window.rs",
        FloatingWindowId::QueryString => "ui/dialog_windows.rs",
        FloatingWindowId::ErrorDialog => "ui/dialog_windows.rs",
        FloatingWindowId::OnScreenKeyboard => "ui/dialog_windows.rs",
    }
}

/// Stem de captura esperada: `window_<storage_key>_1x.png` (#240).
#[must_use]
pub(crate) fn window_capture_stem(id: FloatingWindowId) -> String {
    format!("window_{}_1x", id.storage_key())
}

/// Ruta versionada de una captura para resolución y escala concretas (#284).
#[must_use]
pub(crate) fn window_capture_path(
    id: FloatingWindowId,
    width: u16,
    height: u16,
    ui_scale: u8,
) -> String {
    format!(
        "docs/parity/screenshots/{width}x{height}/window_{}_{}x.png",
        id.storage_key(),
        ui_scale
    )
}

#[must_use]
fn window_visual_states(id: FloatingWindowId) -> &'static [&'static str] {
    if DIALOGS_FAMILY_WINDOW_IDS.contains(&id) {
        &["normal", "pressed", "disabled", "modal"]
    } else {
        &[
            "normal", "pressed", "disabled", "shaded", "sticky", "resized",
        ]
    }
}

/// Gap conocido: categoría → issue GitHub.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct WindowKnownGap {
    pub(crate) category: &'static str,
    pub(crate) issue: u16,
}

/// Gaps documentados mientras faltan capturas/oráculo pixel.
///
/// - `geometry→#243` sólo sin fila en [`WINDOW_REFERENCE_GEOMETRY`].
/// - `lifecycle→#242` no aplica a la familia vehículo (pools #244), a la
///   familia construction de pickers existentes (#246), a la familia world
///   inventariada (#245), a la familia economy/reports (#247), ni a la familia
///   dialogs/settings (#248); sí al resto de clases aún singleton.
#[must_use]
pub(crate) fn window_known_gaps(id: FloatingWindowId) -> &'static [WindowKnownGap] {
    const CAPTURE_ONLY: &[WindowKnownGap] = &[WindowKnownGap {
        category: "capture",
        issue: 240,
    }];
    const WITH_GEOMETRY: &[WindowKnownGap] = &[
        WindowKnownGap {
            category: "capture",
            issue: 240,
        },
        WindowKnownGap {
            category: "lifecycle",
            issue: 242,
        },
    ];
    const WITHOUT_GEOMETRY: &[WindowKnownGap] = &[
        WindowKnownGap {
            category: "capture",
            issue: 240,
        },
        WindowKnownGap {
            category: "lifecycle",
            issue: 242,
        },
        WindowKnownGap {
            category: "geometry",
            issue: 243,
        },
    ];
    if VEHICLE_FAMILY_WINDOW_IDS.contains(&id)
        || CONSTRUCTION_FAMILY_WINDOW_IDS.contains(&id)
        || WORLD_FAMILY_WINDOW_IDS.contains(&id)
        || ECONOMY_FAMILY_WINDOW_IDS.contains(&id)
        || SETTINGS_FAMILY_WINDOW_IDS.contains(&id)
        || DIALOGS_FAMILY_WINDOW_IDS.contains(&id)
    {
        return CAPTURE_ONLY;
    }
    if reference_geometry_primary(id).is_some() {
        WITH_GEOMETRY
    } else {
        WITHOUT_GEOMETRY
    }
}

/// Hijos directos según [`WINDOW_PARITY_MATRIX`] (ownership padre/hija, #242).
#[must_use]
pub(crate) fn window_child_ids(parent: FloatingWindowId) -> Vec<FloatingWindowId> {
    WINDOW_PARITY_MATRIX
        .iter()
        .filter(|entry| entry.parent == Some(parent))
        .map(|entry| entry.id)
        .collect()
}

/// Descendientes (BFS) para cierre en cascada singleton (#242 foundation).
#[must_use]
pub(crate) fn window_descendant_ids(root: FloatingWindowId) -> Vec<FloatingWindowId> {
    let mut out = Vec::new();
    let mut stack = window_child_ids(root);
    while let Some(id) = stack.pop() {
        if out.contains(&id) {
            continue;
        }
        out.push(id);
        stack.extend(window_child_ids(id));
    }
    out
}

/// ¿La captura 1280×720 1× está pendiente? Ausencia → issue, no silencio (#240).
#[must_use]
pub(crate) fn capture_is_pending(id: FloatingWindowId) -> Option<u16> {
    match id {
        FloatingWindowId::Station => None,
        _ => Some(240),
    }
}

/// Política inicial declarada por `WindowDesc` en OpenTTD 15.3.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ReferencePlacement {
    Auto,
    Center,
}

/// Geometría explícita del descriptor upstream. Un eje `None` significa que
/// 15.3 lo calcula desde el árbol de widgets (`0` o constante dinámica).
#[derive(Clone, Copy, Debug)]
pub(crate) struct ReferenceGeometry {
    pub(crate) id: FloatingWindowId,
    pub(crate) variant: &'static str,
    pub(crate) placement: ReferencePlacement,
    pub(crate) width: Option<u16>,
    pub(crate) height: Option<u16>,
    /// Mínimo de resize; por defecto el tamaño inicial si está definido (#243).
    pub(crate) min_width: Option<u16>,
    pub(crate) min_height: Option<u16>,
    /// Paso de resize `(dx, dy)` en px; `None` → 1×1.
    pub(crate) resize_step: Option<(u16, u16)>,
}

macro_rules! reference_geometry {
    ($id:ident, $variant:literal, $placement:ident, $width:expr, $height:expr) => {
        ReferenceGeometry {
            id: FloatingWindowId::$id,
            variant: $variant,
            placement: ReferencePlacement::$placement,
            width: $width,
            height: $height,
            min_width: $width,
            min_height: $height,
            resize_step: Some((1, 1)),
        }
    };
}

/// Variante de geometría según tipo de vehículo (#244).
#[must_use]
#[allow(dead_code)] // consumido en tests de geometría por kind
pub(crate) fn reference_geometry_for_vehicle_kind(
    id: FloatingWindowId,
    kind: openttdrs_core::VehicleKind,
) -> Option<&'static ReferenceGeometry> {
    let prefer = match kind {
        openttdrs_core::VehicleKind::Train => "train",
        _ => "road/ship/aircraft",
    };
    WINDOW_REFERENCE_GEOMETRY
        .iter()
        .find(|geometry| geometry.id == id && geometry.variant == prefer)
        .or_else(|| reference_geometry_primary(id))
}

/// Inventario de la familia vehículo cubierta por #244.
pub(crate) const VEHICLE_FAMILY_WINDOW_IDS: &[FloatingWindowId] = &[
    FloatingWindowId::VehicleList,
    FloatingWindowId::Depot,
    FloatingWindowId::BuyVehicle,
    FloatingWindowId::Vehicle,
    FloatingWindowId::VehicleDetails,
    FloatingWindowId::Orders,
    FloatingWindowId::Timetable,
    FloatingWindowId::Refit,
    FloatingWindowId::SharedOrders,
    FloatingWindowId::Autoreplace,
    FloatingWindowId::DestinationPicker,
];

/// Inventario de pickers construction cubiertos por #246 y #270 (#270 greenfield).
pub(crate) const CONSTRUCTION_FAMILY_WINDOW_IDS: &[FloatingWindowId] = &[
    FloatingWindowId::RailStationPicker,
    FloatingWindowId::AirportPicker,
    FloatingWindowId::RoadStopPicker,
    FloatingWindowId::ObjectPicker,
    FloatingWindowId::BridgePicker,
    FloatingWindowId::DockPicker,
    FloatingWindowId::BuoyPicker,
    FloatingWindowId::RailWaypointPicker,
    FloatingWindowId::RoadWaypointPicker,
    FloatingWindowId::TreePicker,
    FloatingWindowId::TerraformPicker,
    FloatingWindowId::SignPicker,
    FloatingWindowId::DepotBuildPicker,
    FloatingWindowId::SignalPicker,
];

/// Inventario world (Town/Industry/Station + hijas + directorios) cubierto por #245/#269.
///
/// Residual: capturas PNG (#240); dual-entity Station (pool stub hoy); chrome
/// Station fino (#240); Authority acciones 15.3 completas; plot Production.
pub(crate) const WORLD_FAMILY_WINDOW_IDS: &[FloatingWindowId] = &[
    FloatingWindowId::Town,
    FloatingWindowId::TownAuthority,
    FloatingWindowId::TownDirectory,
    FloatingWindowId::Industry,
    FloatingWindowId::IndustryProduction,
    FloatingWindowId::IndustryDirectory,
    FloatingWindowId::StationDirectory,
    FloatingWindowId::Station,
];

/// Inventario economy/reports cubiertos por #247/#271.
///
/// Residual: Livery / ManagerFace / Infrastructure detallado; polish plot;
/// PerformanceHistory como clase propia; capturas PNG (#240).
pub(crate) const ECONOMY_FAMILY_WINDOW_IDS: &[FloatingWindowId] = &[
    FloatingWindowId::Finances,
    FloatingWindowId::CompanyView,
    FloatingWindowId::GraphIncome,
    FloatingWindowId::GraphOperatingProfit,
    FloatingWindowId::GraphCompanyValue,
    FloatingWindowId::CargoPaymentRates,
    FloatingWindowId::SubsidyList,
    FloatingWindowId::League,
    FloatingWindowId::NewsHistory,
    FloatingWindowId::NewsSettings,
];

/// Inventario settings/dialogs existentes cubiertos por #248 slice 1.
///
/// `DevConsole` / `TileInspector` quedan en familia matriz `"debug"` (fuera de
/// este slice). Follow-up: pila modal/parent ownership, OSK, Enter/Escape
/// pixel-perfect; capturas PNG.
pub(crate) const SETTINGS_FAMILY_WINDOW_IDS: &[FloatingWindowId] = &[
    FloatingWindowId::NewGrf,
    FloatingWindowId::SoundMusic,
    FloatingWindowId::DisplayOptions,
    FloatingWindowId::PathfindingSettings,
    FloatingWindowId::CargoDistSettings,
    FloatingWindowId::AiSettings,
    FloatingWindowId::Help,
    FloatingWindowId::CheatWindow,
];

/// Inventario diálogos modales (#272): QueryString / Error / OSK.
///
/// Snapshots residual → #240. Pila modal / Enter / Escape en `modal_stack.rs`.
pub(crate) const DIALOGS_FAMILY_WINDOW_IDS: &[FloatingWindowId] = &[
    FloatingWindowId::QueryString,
    FloatingWindowId::ErrorDialog,
    FloatingWindowId::OnScreenKeyboard,
];

/// Variante preferida al spawnear (singleton): `default`/`game`/`owned`/`settings`…
#[must_use]
pub(crate) fn reference_geometry_primary(
    id: FloatingWindowId,
) -> Option<&'static ReferenceGeometry> {
    let matches: Vec<_> = WINDOW_REFERENCE_GEOMETRY
        .iter()
        .filter(|geometry| geometry.id == id)
        .collect();
    if matches.is_empty() {
        return None;
    }
    for preferred in [
        "default", "game", "owned", "settings", "main", "config", "train",
    ] {
        if let Some(geometry) = matches
            .iter()
            .find(|geometry| geometry.variant == preferred)
        {
            return Some(*geometry);
        }
    }
    Some(matches[0])
}

/// Tamaños iniciales que 15.3 expresa directamente en sus `WindowDesc`.
/// Las variantes comparten class; `WindowKey.instance` sigue en 0 hasta #242.
pub(crate) const WINDOW_REFERENCE_GEOMETRY: &[ReferenceGeometry] = &[
    reference_geometry!(Town, "game", Auto, Some(260), None),
    reference_geometry!(TownAuthority, "default", Auto, Some(300), None),
    reference_geometry!(TownDirectory, "default", Auto, Some(208), Some(202)),
    reference_geometry!(Industry, "default", Auto, Some(260), Some(120)),
    reference_geometry!(IndustryProduction, "default", Auto, Some(300), Some(215)),
    reference_geometry!(IndustryDirectory, "default", Auto, Some(428), Some(190)),
    reference_geometry!(StationDirectory, "default", Auto, Some(358), Some(162)),
    reference_geometry!(Station, "default", Auto, Some(249), Some(117)),
    reference_geometry!(VehicleList, "train", Auto, Some(325), Some(246)),
    reference_geometry!(
        VehicleList,
        "road/ship/aircraft",
        Auto,
        Some(260),
        Some(246)
    ),
    reference_geometry!(Vehicle, "train", Auto, Some(250), Some(134)),
    reference_geometry!(Vehicle, "road/ship/aircraft", Auto, Some(250), Some(116)),
    reference_geometry!(VehicleDetails, "train", Auto, Some(405), Some(178)),
    reference_geometry!(
        VehicleDetails,
        "road/ship/aircraft",
        Auto,
        Some(405),
        Some(113)
    ),
    reference_geometry!(RailStationPicker, "default", Auto, Some(280), Some(220)),
    reference_geometry!(AirportPicker, "default", Auto, Some(320), Some(220)),
    reference_geometry!(RoadStopPicker, "default", Auto, Some(220), Some(180)),
    reference_geometry!(ObjectPicker, "default", Auto, Some(220), Some(200)),
    reference_geometry!(BridgePicker, "default", Auto, Some(200), Some(114)),
    reference_geometry!(DockPicker, "default", Auto, Some(258), Some(120)),
    reference_geometry!(BuoyPicker, "default", Auto, Some(258), Some(96)),
    reference_geometry!(RailWaypointPicker, "default", Auto, Some(284), Some(96)),
    reference_geometry!(RoadWaypointPicker, "default", Auto, Some(284), Some(96)),
    reference_geometry!(TreePicker, "default", Auto, Some(260), Some(96)),
    reference_geometry!(TerraformPicker, "default", Auto, Some(260), Some(96)),
    reference_geometry!(SignPicker, "default", Auto, Some(300), Some(96)),
    reference_geometry!(DepotBuildPicker, "default", Auto, Some(260), Some(96)),
    reference_geometry!(SignalPicker, "default", Auto, Some(200), Some(140)),
    reference_geometry!(DestinationPicker, "default", Auto, Some(200), Some(180)),
    reference_geometry!(SubsidyList, "default", Auto, Some(500), Some(127)),
    reference_geometry!(NewsHistory, "default", Auto, Some(400), Some(140)),
    reference_geometry!(Finances, "default", Auto, None, None),
    reference_geometry!(CompanyView, "default", Auto, Some(280), None),
    reference_geometry!(NewsSettings, "settings", Center, None, None),
    reference_geometry!(PathfindingSettings, "settings", Center, None, None),
    reference_geometry!(CargoDistSettings, "settings", Center, None, None),
    reference_geometry!(AiSettings, "config", Center, None, None),
    reference_geometry!(NewGrf, "settings", Center, Some(300), Some(263)),
    reference_geometry!(SoundMusic, "main", Auto, None, None),
    reference_geometry!(Timetable, "default", Auto, Some(400), Some(130)),
    reference_geometry!(Orders, "owned", Auto, Some(384), Some(100)),
    reference_geometry!(Orders, "competitor", Auto, Some(384), Some(86)),
    reference_geometry!(Refit, "default", Auto, Some(240), Some(174)),
    reference_geometry!(GraphIncome, "default", Auto, None, None),
    reference_geometry!(GraphOperatingProfit, "default", Auto, None, None),
    reference_geometry!(GraphCompanyValue, "default", Auto, None, None),
    reference_geometry!(CargoPaymentRates, "default", Auto, None, None),
    reference_geometry!(DisplayOptions, "settings", Center, None, None),
    reference_geometry!(ExtraViewport, "default", Auto, Some(300), Some(268)),
    reference_geometry!(Help, "default", Center, None, None),
    reference_geometry!(CheatWindow, "default", Auto, None, None),
    reference_geometry!(GenLand, "main", Center, None, None),
    reference_geometry!(League, "default", Auto, None, None),
    reference_geometry!(QueryString, "default", Center, Some(300), Some(90)),
    reference_geometry!(ErrorDialog, "default", Center, Some(280), Some(90)),
    reference_geometry!(OnScreenKeyboard, "default", Center, Some(360), Some(144)),
];

fn window_id_by_storage_key(requested: &str) -> Option<FloatingWindowId> {
    WINDOW_PARITY_MATRIX
        .iter()
        .find(|entry| {
            entry
                .id
                .storage_key()
                .eq_ignore_ascii_case(requested.trim())
        })
        .map(|entry| entry.id)
}

fn requested_window_shot_id() -> Result<Option<FloatingWindowId>, String> {
    let Ok(requested) = std::env::var("OPENTTDRS_WINDOW_SHOT_ID") else {
        return Ok(None);
    };
    if requested.trim().is_empty() {
        return Ok(None);
    }
    window_id_by_storage_key(&requested)
        .map(Some)
        .ok_or(requested)
}

fn json_string(value: &str) -> String {
    format!("{value:?}")
}

fn window_parity_matrix_json() -> String {
    let mut missing_captures = Vec::new();
    let mut output = String::from(
        "{\n  \"schema_version\": 2,\n  \"openttd_commit\": \"14ec60f248547d4d062a1160f0fc26d742319888\",\n  \"windows\": [\n",
    );
    for (index, entry) in WINDOW_PARITY_MATRIX.iter().enumerate() {
        let kind = match entry.kind {
            WindowParityKind::Upstream => "upstream",
            WindowParityKind::Extension => "extension",
        };
        let parent = entry
            .parent
            .map(|id| json_string(id.storage_key()))
            .unwrap_or_else(|| "null".to_owned());
        let rust_impl = window_rust_impl(entry.id);
        let capture = window_capture_stem(entry.id);
        let key = WindowKey::singleton(entry.id);
        if capture_is_pending(entry.id).is_some() {
            missing_captures.push(entry.id.storage_key());
        }
        let _ = write!(
            output,
            "    {{\"id\":{},\"family\":{},\"kind\":{},\"upstream_source\":{},\"upstream_window\":{},\"parent\":{},\"rust_impl\":{},\"capture_stem\":{},\"states\":[{}],\"captures\":[{}],\"window_key\":{{\"class\":{},\"instance\":{}}},\"known_gaps\":[",
            json_string(entry.id.storage_key()),
            json_string(entry.family),
            json_string(kind),
            json_string(entry.upstream_source),
            json_string(entry.upstream_window),
            parent,
            json_string(rust_impl),
            json_string(&capture),
            window_visual_states(entry.id)
                .iter()
                .map(|state| json_string(state))
                .collect::<Vec<_>>()
                .join(","),
            [
                (1280, 720, 1),
                (1280, 720, 2),
                (1920, 1080, 1),
                (1920, 1080, 2),
            ]
            .iter()
            .map(|&(width, height, scale)| json_string(&window_capture_path(
                entry.id, width, height, scale
            )))
            .collect::<Vec<_>>()
            .join(","),
            json_string(key.class.storage_key()),
            key.instance,
        );
        for (gap_i, gap) in window_known_gaps(entry.id).iter().enumerate() {
            if gap_i > 0 {
                output.push(',');
            }
            let _ = write!(
                output,
                "{{\"category\":{},\"issue\":{}}}",
                json_string(gap.category),
                gap.issue,
            );
        }
        output.push_str("],\"geometry\":[");
        for (geometry_index, geometry) in WINDOW_REFERENCE_GEOMETRY
            .iter()
            .filter(|geometry| geometry.id == entry.id)
            .enumerate()
        {
            if geometry_index > 0 {
                output.push(',');
            }
            let placement = match geometry.placement {
                ReferencePlacement::Auto => "auto",
                ReferencePlacement::Center => "center",
            };
            let width = geometry
                .width
                .map_or_else(|| "null".to_owned(), |v| v.to_string());
            let height = geometry
                .height
                .map_or_else(|| "null".to_owned(), |v| v.to_string());
            let min_width = geometry
                .min_width
                .map_or_else(|| "null".to_owned(), |v| v.to_string());
            let min_height = geometry
                .min_height
                .map_or_else(|| "null".to_owned(), |v| v.to_string());
            let (step_x, step_y) = geometry.resize_step.unwrap_or((1, 1));
            let _ = write!(
                output,
                "{{\"variant\":{},\"placement\":{},\"width\":{},\"height\":{},\"min_width\":{},\"min_height\":{},\"resize_step\":[{step_x},{step_y}]}}",
                json_string(geometry.variant),
                json_string(placement),
                width,
                height,
                min_width,
                min_height,
            );
        }
        let comma = if index + 1 == WINDOW_PARITY_MATRIX.len() {
            ""
        } else {
            ","
        };
        let _ = writeln!(output, "]}}{comma}");
    }
    output.push_str("  ],\n  \"report\": {\n");
    output.push_str(&format!(
        "    \"missing_captures\": [{}],\n",
        missing_captures
            .iter()
            .map(|id| json_string(id))
            .collect::<Vec<_>>()
            .join(", ")
    ));
    output.push_str("    \"missing_rust_files\": [],\n");
    output.push_str(
        "    \"notes\": \"Pixel/chrome diffs → #241; multi-instance instance≠0 → #242; geometry gaps = clases sin WindowDesc inventariado\"\n",
    );
    output.push_str("  }\n}\n");
    output
}

fn export_window_parity_matrix_if_requested() {
    let Ok(path) = std::env::var("OPENTTDRS_WINDOW_MATRIX") else {
        return;
    };
    match std::fs::write(&path, window_parity_matrix_json()) {
        Ok(()) => info!("window parity: matriz JSON guardada en {path}"),
        Err(error) => error!("window parity: no se pudo escribir {path}: {error}"),
    }
}

pub(crate) struct WindowsShotPlugin;

impl Plugin for WindowsShotPlugin {
    fn build(&self, app: &mut App) {
        export_window_parity_matrix_if_requested();
        if std::env::var_os("OPENTTDRS_WINDOWS_SHOT").is_some()
            || std::env::var_os("OPENTTDRS_MAP_SHOT").is_some()
        {
            app.add_systems(Startup, apply_shot_settings);
        }
        if std::env::var_os("OPENTTDRS_WINDOWS_SHOT").is_some() {
            app.add_systems(
                Update,
                (
                    auto_start_game.run_if(in_state(ClientScreen::MainMenu)),
                    windows_shot_driver
                        .run_if(in_state(ClientScreen::InGame))
                        // Los sync de cada ventana también viven en `UpdateSet::Ui`.
                        // Ocultar antes permite que vuelvan a mostrarla en el mismo frame.
                        .after(UpdateSet::Ui),
                ),
            );
        } else if std::env::var_os("OPENTTDRS_MAP_SHOT").is_some() {
            app.add_systems(
                Update,
                (
                    auto_start_game.run_if(in_state(ClientScreen::MainMenu)),
                    map_shot_driver
                        .run_if(in_state(ClientScreen::InGame))
                        // El fantasma de obra lee el cursor que fija el driver.
                        .before(crate::ui::toolbar::update_build_ghost_preview),
                ),
            );
        }
    }
}

fn parse_shot_resolution() -> Option<(u32, u32)> {
    let Ok(raw) = std::env::var("OPENTTDRS_SHOT_RES") else {
        return None;
    };
    let (w, h) = raw.split_once('x').or_else(|| raw.split_once('X'))?;
    let w = w.parse().ok()?;
    let h = h.parse().ok()?;
    if w < 320 || h < 240 {
        return None;
    }
    Some((w, h))
}

fn parse_shot_ui_scale(raw: &str) -> Option<f32> {
    let scale = raw.trim().parse::<f32>().ok()?;
    (scale.is_finite() && (0.5..=4.0).contains(&scale)).then_some(scale)
}

fn apply_shot_settings(
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
    mut ui_scale: ResMut<UiScale>,
) {
    if let Some((w, h)) = parse_shot_resolution() {
        for mut window in &mut windows {
            window.resolution.set(w as f32, h as f32);
            info!("shot: resolución {w}×{h}");
        }
    }
    if let Ok(raw) = std::env::var("OPENTTDRS_SHOT_UI_SCALE") {
        if let Some(scale) = parse_shot_ui_scale(&raw) {
            ui_scale.0 = scale;
            info!("shot: escala UI {scale}");
        } else {
            error!("shot: OPENTTDRS_SHOT_UI_SCALE inválida: {raw:?}; rango permitido 0.5..=4");
        }
    }
}

/// Salta el menú principal directamente al juego (igual que pulsar «Jugar»).
fn auto_start_game(
    mut commands: Commands,
    q_menu: Query<Entity, With<MainMenuUi>>,
    q_menu_cam: Query<Entity, With<MainMenuCamera>>,
    mut next_screen: ResMut<NextState<ClientScreen>>,
) {
    for e in &q_menu {
        commands.entity(e).despawn();
    }
    for cam in &q_menu_cam {
        commands.entity(cam).despawn();
    }
    next_screen.set(ClientScreen::InGame);
}

fn first_depot(sim: &SimWorld) -> Option<TileCoord> {
    let (w, h) = sim.state.map.dimensions();
    for y in 0..h {
        for x in 0..w {
            let pos = TileCoord::new(x.cast_signed(), y.cast_signed());
            if matches!(
                sim.state.map.get_kind(pos),
                Some(TileKind::RoadDepot | TileKind::RailDepot)
            ) {
                return Some(pos);
            }
        }
    }
    None
}

fn first_station(sim: &SimWorld) -> Option<TileCoord> {
    sim.state.stations.first().map(|s| s.pos)
}

fn first_industry_tile(sim: &SimWorld) -> Option<TileCoord> {
    sim.state.industries.first().map(|i| i.pos)
}

/// Con `OPENTTDRS_MAP_SHOT=/ruta.png`: captura el mapa sin abrir ventanas y sale.
/// Con `OPENTTDRS_MAP_SHOT_TOOL=rail|rail_x|rail_y` activa además esa herramienta
/// y fija el cursor al centro de la ventana para capturar el fantasma de obra.
#[allow(clippy::too_many_arguments)] // sistema ECS Bevy
fn map_shot_driver(
    mut commands: Commands,
    mut frame: Local<u32>,
    mut windows: Query<&mut Window, With<bevy::window::PrimaryWindow>>,
    mut tool_state: ResMut<crate::ui::toolbar::UiToolState>,
    mut toolbar_state: ResMut<crate::ui::toolbar::ToolbarState>,
    mut station_state: ResMut<crate::ui::toolbar::StationBuildState>,
    mut sim: ResMut<SimWorld>,
    mut remap: ResMut<crate::render::RemapMapVisualsPending>,
    mut exit: MessageWriter<AppExit>,
) {
    *frame += 1;
    // `OPENTTDRS_MAP_SHOT_PLACE=x,y[;x,y…]`: aplica la herramienta en esas
    // teselas antes de la captura (p. ej. colocar vía/estación y ver el render).
    if *frame == OPEN_FRAME + 5
        && let Ok(spec) = std::env::var("OPENTTDRS_MAP_SHOT_PLACE")
        && let Some(action) = tool_state.active_tool
    {
        for part in spec.split(';') {
            let Some((x, y)) = part.split_once(',') else {
                continue;
            };
            let (Ok(x), Ok(y)) = (x.parse::<i32>(), y.parse::<i32>()) else {
                continue;
            };
            let Some(cmd) = crate::ui::toolbar::build_input::commands::command_for_action(
                action,
                TileCoord::new(x, y),
                &station_state,
                None,
                Some(&sim.state.map),
                None,
                station_state.signal_type,
                false,
                sim.state.current_rail_type,
                sim.state.current_object_spec,
            ) else {
                continue;
            };
            let res = crate::network::apply_player_command(&mut sim.state, &cmd);
            info!("map_shot: place en ({x},{y}) → {res:?}");
        }
        remap.pending = true;
    }
    if *frame >= OPEN_FRAME
        && let Ok(tool) = std::env::var("OPENTTDRS_MAP_SHOT_TOOL")
    {
        use crate::ui::toolbar::{BuildMenuAction, ToolbarGroup};
        tool_state.active_tool = match tool.as_str() {
            "rail" => Some(BuildMenuAction::Rail),
            "rail_x" => Some(BuildMenuAction::RailX),
            "rail_y" => Some(BuildMenuAction::RailY),
            "rail_station" => Some(BuildMenuAction::RailStation),
            _ => None,
        };
        // `OPENTTDRS_MAP_SHOT_STATION=AxL` (p. ej. 4x3): andenes × longitud.
        if let Ok(spec) = std::env::var("OPENTTDRS_MAP_SHOT_STATION")
            && let Some((a, l)) = spec.split_once('x')
            && let (Ok(a), Ok(l)) = (a.parse::<u8>(), l.parse::<u8>())
        {
            station_state.rail_platforms = a.clamp(1, 7);
            station_state.rail_length = l.clamp(1, 7);
        }
        // El panel del grupo debe estar abierto o `hide_tool_when_panel_closed`
        // limpia la herramienta en el mismo frame.
        toolbar_state.active_group = Some(ToolbarGroup::Rail);
        if let Ok(mut window) = windows.single_mut() {
            // `OPENTTDRS_MAP_SHOT_CURSOR=fx,fy` (fracciones 0..1) reubica el cursor.
            let (fx, fy) = std::env::var("OPENTTDRS_MAP_SHOT_CURSOR")
                .ok()
                .and_then(|s| {
                    let (a, b) = s.split_once(',')?;
                    Some((a.parse::<f32>().ok()?, b.parse::<f32>().ok()?))
                })
                .unwrap_or((0.5, 0.55));
            let center = Vec2::new(window.width() * fx, window.height() * fy);
            window.set_cursor_position(Some(center));
        }
    }
    if *frame == SHOT_FRAME
        && let Ok(path) = std::env::var("OPENTTDRS_MAP_SHOT")
    {
        if let Ok(window) = windows.single_mut() {
            info!(
                "map_shot: tool_activa={} cursor={:?}",
                tool_state.active_tool.is_some(),
                window.cursor_position()
            );
        }
        info!("map_shot: guardando captura en {path}");
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(path));
    }
    if *frame == EXIT_FRAME {
        exit.write(AppExit::Success);
    }
}

/// Abre todas las ventanas flotantes + paneles auxiliares para captura de paridad.
/// Sistema exclusivo: demasiados `ResMut` para el límite de SystemParam de Bevy.
fn windows_shot_driver(world: &mut World, mut frame: Local<u32>) {
    *frame += 1;
    if *frame == OPEN_FRAME {
        let include_auxiliary = requested_window_shot_id().is_ok_and(|id| id.is_none());
        open_all_windows_for_shot(world, include_auxiliary);
    }

    // Fuerza visibilidad de pickers tool-gated. En modo individual oculta el
    // resto aunque sus sistemas de sync intenten reabrirlos.
    if (OPEN_FRAME..=SHOT_FRAME).contains(&*frame) {
        let selection = requested_window_shot_id();
        let mut q = world.query::<(&FloatingWindow, &mut Visibility)>();
        for (window, mut vis) in q.iter_mut(world) {
            *vis = if selection
                .as_ref()
                .is_ok_and(|selected| selected.is_none_or(|id| id == window.id))
            {
                Visibility::Visible
            } else {
                Visibility::Hidden
            };
        }
    }

    if *frame == SHOT_FRAME {
        match requested_window_shot_id() {
            Ok(_) => {
                if let Ok(path) = std::env::var("OPENTTDRS_WINDOWS_SHOT") {
                    info!("windows_shot: guardando captura en {path}");
                    world
                        .commands()
                        .spawn(Screenshot::primary_window())
                        .observe(save_to_disk(path));
                }
            }
            Err(id) => error!(
                "windows_shot: OPENTTDRS_WINDOW_SHOT_ID desconocido: {id:?}; no se genera captura"
            ),
        }
    }
    if *frame == EXIT_FRAME {
        world.write_message(AppExit::Success);
    }
}

fn open_all_windows_for_shot(world: &mut World, include_auxiliary: bool) {
    let town_id = world
        .resource::<SimWorld>()
        .state
        .towns
        .first()
        .map(|t| t.id);
    if requested_window_shot_id() == Ok(Some(FloatingWindowId::TownAuthority)) {
        match town_authority_shot_state() {
            TownAuthorityShotState::Normal => {}
            TownAuthorityShotState::NoFunds => {
                world.resource_mut::<SimWorld>().state.economy.money = 0;
            }
            TownAuthorityShotState::Unavailable => {
                if let Some(town_id) = town_id
                    && let Some(town) = world
                        .resource_mut::<SimWorld>()
                        .state
                        .towns
                        .iter_mut()
                        .find(|town| town.id == town_id)
                {
                    town.road_build_months = 1;
                }
            }
        }
    }
    let vehicle_id = world
        .resource::<SimWorld>()
        .state
        .vehicles
        .first()
        .map(|v| v.id);
    let vehicle_orders = vehicle_id.and_then(|vid| {
        world
            .resource::<SimWorld>()
            .state
            .vehicles
            .iter()
            .find(|v| v.id == vid)
            .map(|v| v.orders.clone())
    });
    let depot_pos = first_depot(world.resource::<SimWorld>());
    let station_pos = first_station(world.resource::<SimWorld>());
    let industry_pos = first_industry_tile(world.resource::<SimWorld>());
    let selected_engine = depot_pos.and_then(|pos| {
        let sim = world.resource::<SimWorld>();
        crate::ui::buy_window::engines_for_buy_window(
            sim,
            pos,
            openttdrs_core::EngineCatalogSort::default(),
            openttdrs_core::RoadEngineFilter::default(),
            crate::ui::buy_window::RailBuyFilter::default(),
            "",
        )
        .first()
        .map(|e| e.id)
    });
    let save_dir = save_dir_from(&world.resource::<SimHudControls>().json_save_path);

    world.resource_mut::<TownWindowState>().town_id = town_id;
    world.resource_mut::<TownAuthorityWindowState>().town_id = town_id;
    let station_positions: Vec<_> = world
        .resource::<SimWorld>()
        .state
        .stations
        .iter()
        .filter(|station| !station.is_waypoint())
        .map(|station| station.pos)
        .take(crate::ui::station_pool::MAX_STATION_POOL_SLOTS)
        .collect();
    if let Some(&focused) = station_positions.last() {
        {
            let mut pool = world.resource_mut::<crate::ui::station_pool::StationPoolRegistry>();
            for (slot, position) in pool.slots.iter_mut().zip(station_positions.iter().copied()) {
                *slot = Some(position);
            }
            pool.focused = Some(focused);
        }
        let mut panel = world.resource_mut::<StationCargoPanelState>();
        panel.station_pos = Some(focused);
        panel.rename_editing = false;
    }
    {
        let mut depot = world.resource_mut::<DepotPanelState>();
        depot.depot_pos = depot_pos;
    }
    {
        let mut buy = world.resource_mut::<BuyVehicleWindowState>();
        buy.depot_pos = depot_pos;
        buy.selected_engine = selected_engine;
    }
    if let Some(vid) = vehicle_id {
        world.resource_scope(|world, mut chain: Mut<VehicleChainRegistry>| {
            world
                .resource_mut::<VehicleWindowState>()
                .open_or_focus(&mut chain, vid);
        });
    } else {
        world.resource_mut::<VehicleChainRegistry>().clear();
        let mut vehicle_window = world.resource_mut::<VehicleWindowState>();
        vehicle_window.vehicle_id = None;
        vehicle_window.open.clear();
        vehicle_window.rename_editing = false;
    }
    if let Some(vid) = vehicle_id {
        let slot = world
            .resource::<VehicleChainRegistry>()
            .slot_of(vid)
            .unwrap_or(0);
        {
            let mut tt = world.resource_mut::<TimetableWindowState>();
            tt.slots[slot as usize] = Some(vid);
            tt.focused = Some(vid);
        }
        world.resource_scope(|world, chain: Mut<VehicleChainRegistry>| {
            world
                .resource_mut::<VehicleDetailsWindowState>()
                .open_for(&chain, vid);
            world
                .resource_mut::<RefitWindowState>()
                .open_for(&chain, vid);
        });
        {
            let mut order = world.resource_mut::<OrderEditState>();
            order.bind_slot(slot, vid, vehicle_orders.unwrap_or_default(), None);
        }
        {
            let mut shared = world.resource_mut::<SharedOrdersWindowState>();
            shared.open = true;
            shared.link_vehicle_id = Some(vid);
        }
        world
            .resource_mut::<DestinationPickerState>()
            .open_for_chain_slot(slot);
    } else {
        let mut tt = world.resource_mut::<TimetableWindowState>();
        tt.focused = None;
        tt.slots = [None; 2];
    }
    if let Some(pos) = depot_pos {
        world
            .resource_mut::<AutoreplaceWindowState>()
            .open_for_depot(pos);
    }
    world.resource_mut::<StationCargoPanelState>().station_pos = station_pos;
    if let Some(pos) = industry_pos {
        let mut panel = world.resource_mut::<IndustryPanelState>();
        panel.open = true;
        panel.focus_tile = Some(pos);
    }

    world.resource_mut::<FinancesWindowState>().open = true;
    world.resource_mut::<CompanyViewWindowState>().open = true;
    world.resource_mut::<TownAuthorityWindowState>().open = true;
    world.resource_mut::<IndustryProductionWindowState>().open = true;
    world.resource_mut::<TownDirectoryState>().open = true;
    world.resource_mut::<IndustryDirectoryState>().open = true;
    world.resource_mut::<StationDirectoryState>().open = true;
    world.resource_mut::<VehicleListState>().open = true;
    world.resource_mut::<SubsidyListState>().open = true;
    world.resource_mut::<NewsHistoryState>().open = true;
    world.resource_mut::<NewsSettingsWindowState>().open = true;
    world.resource_mut::<PathfindingSettingsWindowState>().open = true;
    world.resource_mut::<AiSettingsWindowState>().open = true;
    world.resource_mut::<NewGrfWindowState>().open = true;
    world.resource_mut::<SoundMusicWindowState>().open = true;
    {
        let mut graphs = world.resource_mut::<GraphWindowState>();
        graphs.income_open = true;
        graphs.profit_open = true;
        graphs.value_open = true;
    }
    world.resource_mut::<CargoPaymentWindowState>().open = true;
    world.resource_mut::<DisplayOptionsWindowState>().open = true;
    world.resource_mut::<ExtraViewportWindowState>().open = true;
    world.resource_mut::<SignListWindowState>().open = true;
    world.resource_mut::<LinkGraphWindowState>().open = true;
    world.resource_mut::<HelpWindowState>().open = true;
    world.resource_mut::<DevConsoleState>().open = true;
    world.resource_mut::<TileInspectorWindowState>().open = true;
    world.resource_mut::<CheatWindowState>().open = true;
    world.resource_mut::<GenLandWindowState>().open = true;
    world.resource_mut::<GoalListWindowState>().open = true;
    world.resource_mut::<StoryWindowState>().open = true;
    world.resource_mut::<LeagueWindowState>().open = true;
    {
        let mut stack = world.resource_mut::<ModalStack>();
        open_query_for_newgrf_rename(&mut stack, "windows_shot");
        open_osk_for_query(&mut stack, "windows_shot");
        open_error_modal(&mut stack, "windows_shot error");
    }
    world.resource_mut::<QueryStringWindowState>().open = true;
    world.resource_mut::<ErrorDialogWindowState>().open = true;
    world.resource_mut::<OskWindowState>().open = true;

    if include_auxiliary {
        world
            .resource_mut::<SaveWindowState>()
            .open_in_mode(SaveWindowMode::Save, &save_dir);
        world.resource_mut::<StationCatalogPickerState>().open = Some(StationCatalogKind::Spec);

        world.resource_mut::<ToolbarState>().active_group = Some(ToolbarGroup::Rail);
        world.resource_mut::<UiToolState>().active_tool = Some(BuildMenuAction::RailStation);
        world.resource_mut::<BridgeBuildState>().pending = Some(PendingBridge {
            start: TileCoord::new(2, 2),
            end: TileCoord::new(6, 2),
            road: false,
        });
    }

    info!(
        "windows_shot: abriendo ALL FloatingWindowId ({}); auxiliares={include_auxiliary}",
        FloatingWindowId::ALL.len(),
    );
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn windows_shot_covers_all_floating_ids() {
        assert_eq!(
            windows_shot_covered_ids(),
            FloatingWindowId::ALL,
            "actualizar windows_shot_covered_ids al añadir FloatingWindowId"
        );
    }

    #[test]
    fn parity_matrix_covers_every_window_exactly_once() {
        assert_eq!(WINDOW_PARITY_MATRIX.len(), FloatingWindowId::ALL.len());
        for id in FloatingWindowId::ALL {
            assert_eq!(
                WINDOW_PARITY_MATRIX
                    .iter()
                    .filter(|entry| entry.id == *id)
                    .count(),
                1,
                "la matriz debe contener exactamente una entrada para {id:?}"
            );
        }
    }

    #[test]
    fn upstream_entries_name_their_reference() {
        for entry in WINDOW_PARITY_MATRIX {
            if entry.kind == WindowParityKind::Upstream {
                assert!(!entry.family.is_empty());
                assert!(entry.upstream_source.ends_with(".cpp"));
                assert!(!entry.upstream_window.is_empty());
            }
            if let Some(parent) = entry.parent {
                assert_ne!(parent, entry.id);
                assert!(FloatingWindowId::ALL.contains(&parent));
            }
        }
    }

    #[test]
    fn reference_geometry_points_to_inventory_and_has_unique_variants() {
        for (index, geometry) in WINDOW_REFERENCE_GEOMETRY.iter().enumerate() {
            assert!(FloatingWindowId::ALL.contains(&geometry.id));
            assert!(!geometry.variant.is_empty());
            assert!(geometry.width.is_none_or(|value| value > 0));
            assert!(geometry.height.is_none_or(|value| value > 0));
            assert!(
                !WINDOW_REFERENCE_GEOMETRY[..index]
                    .iter()
                    .any(|other| other.id == geometry.id && other.variant == geometry.variant)
            );
            let _placement = geometry.placement;
        }
    }

    #[test]
    fn individual_shot_ids_use_stable_storage_keys() {
        assert_eq!(
            window_id_by_storage_key("Vehicle"),
            Some(FloatingWindowId::Vehicle)
        );
        assert_eq!(
            window_id_by_storage_key("orders"),
            Some(FloatingWindowId::Orders)
        );
        assert_eq!(window_id_by_storage_key("does-not-exist"), None);
    }

    #[test]
    fn shot_ui_scale_is_finite_and_bounded() {
        assert_eq!(parse_shot_ui_scale("1"), Some(1.0));
        assert_eq!(parse_shot_ui_scale("2.0"), Some(2.0));
        assert_eq!(parse_shot_ui_scale("0"), None);
        assert_eq!(parse_shot_ui_scale("NaN"), None);
        assert_eq!(parse_shot_ui_scale("5"), None);
    }

    #[test]
    fn town_authority_shot_state_defaults_and_accepts_known_variants() {
        assert_eq!(
            parse_town_authority_shot_state(None),
            TownAuthorityShotState::Normal
        );
        assert_eq!(
            parse_town_authority_shot_state(Some("no-funds")),
            TownAuthorityShotState::NoFunds
        );
        assert_eq!(
            parse_town_authority_shot_state(Some("unavailable")),
            TownAuthorityShotState::Unavailable
        );
        assert_eq!(
            parse_town_authority_shot_state(Some("invalid")),
            TownAuthorityShotState::Normal
        );
    }

    #[test]
    fn machine_readable_matrix_contains_inventory_and_geometry() {
        let json = window_parity_matrix_json();
        assert!(json.starts_with("{\n  \"schema_version\": 2,"));
        for id in FloatingWindowId::ALL {
            assert!(json.contains(&format!("\"id\":{:?}", id.storage_key())));
            assert!(json.contains(&format!("\"rust_impl\":{:?}", window_rust_impl(*id))));
        }
        assert!(json.contains("\"variant\":\"train\""));
        assert!(json.contains("\"placement\":\"center\""));
        assert!(json.contains("\"missing_captures\""));
        assert!(json.contains("\"known_gaps\""));
        assert!(json.contains("\"window_key\""));
        assert!(json.contains("\"states\":[\"normal\",\"pressed\",\"disabled\""));
        assert!(json.contains("docs/parity/screenshots/1920x1080/window_Town_2x.png"));
    }

    #[test]
    fn capture_matrix_covers_supported_resolutions_and_scales() {
        let id = FloatingWindowId::Station;
        assert_eq!(
            window_capture_path(id, 1280, 720, 1),
            "docs/parity/screenshots/1280x720/window_Station_1x.png"
        );
        assert_eq!(
            window_capture_path(id, 1920, 1080, 2),
            "docs/parity/screenshots/1920x1080/window_Station_2x.png"
        );
        assert!(window_visual_states(id).contains(&"resized"));
        assert!(window_visual_states(FloatingWindowId::ErrorDialog).contains(&"modal"));
    }

    #[test]
    fn every_window_links_existing_rust_impl() {
        let src_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
        for id in FloatingWindowId::ALL {
            let rel = window_rust_impl(*id);
            let path = src_root.join(rel);
            assert!(
                path.is_file(),
                "rust_impl de {id:?} no existe: {} ({rel})",
                path.display()
            );
        }
    }

    #[test]
    fn capture_absence_is_tracked_with_issue_not_silently() {
        for id in FloatingWindowId::ALL {
            let stem = window_capture_stem(*id);
            let capture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../docs/parity/screenshots/1280x720")
                .join(format!("{stem}.png"));
            if capture_path.is_file() {
                assert!(
                    capture_is_pending(*id).is_none(),
                    "captura de {id:?} existe pero sigue en pending"
                );
            } else {
                let issue =
                    capture_is_pending(*id).expect("ausencia de captura debe citar issue (#240)");
                assert!(issue >= 240, "issue de captura inválido: {issue}");
                let gaps = window_known_gaps(*id);
                assert!(
                    gaps.iter()
                        .any(|g| g.category == "capture" && g.issue == issue),
                    "known_gaps debe incluir capture→#{issue} para {id:?}"
                );
            }
        }
    }

    #[test]
    fn window_key_singleton_matches_class() {
        for id in FloatingWindowId::ALL {
            let key = WindowKey::singleton(*id);
            assert_eq!(key.class(), *id);
            assert_eq!(key.instance, 0);
        }
    }

    #[test]
    fn parent_child_matrix_covers_vehicle_and_depot_chains() {
        let vehicle_children = window_child_ids(FloatingWindowId::Vehicle);
        for expected in [
            FloatingWindowId::VehicleDetails,
            FloatingWindowId::Timetable,
            FloatingWindowId::Orders,
            FloatingWindowId::Refit,
        ] {
            assert!(
                vehicle_children.contains(&expected),
                "Vehicle debe listar hijo {expected:?}"
            );
        }
        assert_eq!(
            window_child_ids(FloatingWindowId::Orders),
            vec![FloatingWindowId::DestinationPicker]
        );
        assert_eq!(
            window_child_ids(FloatingWindowId::Depot),
            vec![FloatingWindowId::BuyVehicle]
        );
        let descendants = window_descendant_ids(FloatingWindowId::Vehicle);
        assert!(descendants.contains(&FloatingWindowId::DestinationPicker));
        assert!(!descendants.contains(&FloatingWindowId::Vehicle));
    }

    #[test]
    fn geometry_gap_only_for_classes_without_descriptor() {
        assert!(
            window_known_gaps(FloatingWindowId::Town)
                .iter()
                .all(|g| g.category != "geometry")
        );
        assert!(
            window_known_gaps(FloatingWindowId::Finances)
                .iter()
                .all(|g| g.category != "geometry")
        );
        assert!(
            window_known_gaps(FloatingWindowId::SignList)
                .iter()
                .any(|g| g.category == "geometry" && g.issue == 243)
        );
        let geo = reference_geometry_primary(FloatingWindowId::NewGrf).expect("NewGrf");
        assert_eq!(geo.placement, ReferencePlacement::Center);
        assert_eq!(geo.width, Some(300));
        assert_eq!(geo.resize_step, Some((1, 1)));
    }

    #[test]
    fn vehicle_family_inventory_is_in_parity_matrix() {
        for id in VEHICLE_FAMILY_WINDOW_IDS {
            assert!(
                WINDOW_PARITY_MATRIX.iter().any(|e| e.id == *id),
                "familia vehículo falta en matriz: {id:?}"
            );
            assert!(
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("src")
                    .join(window_rust_impl(*id))
                    .is_file(),
                "rust_impl de familia vehículo ausente: {id:?}"
            );
        }
        let train = reference_geometry_for_vehicle_kind(
            FloatingWindowId::Vehicle,
            openttdrs_core::VehicleKind::Train,
        )
        .expect("train");
        let road = reference_geometry_for_vehicle_kind(
            FloatingWindowId::Vehicle,
            openttdrs_core::VehicleKind::Truck,
        )
        .expect("road");
        assert_eq!(train.height, Some(134));
        assert_eq!(road.height, Some(116));
    }

    #[test]
    fn construction_family_inventory_is_in_parity_matrix() {
        for id in CONSTRUCTION_FAMILY_WINDOW_IDS {
            assert!(
                WINDOW_PARITY_MATRIX.iter().any(|e| e.id == *id),
                "familia construction falta en matriz: {id:?}"
            );
            assert!(
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("src")
                    .join(window_rust_impl(*id))
                    .is_file(),
                "rust_impl de familia construction ausente: {id:?}"
            );
            assert!(
                reference_geometry_primary(*id).is_some(),
                "familia construction sin geometría primary: {id:?}"
            );
        }
    }

    #[test]
    fn construction_family_known_gaps_are_capture_only() {
        for id in CONSTRUCTION_FAMILY_WINDOW_IDS {
            let gaps = window_known_gaps(*id);
            assert_eq!(
                gaps,
                &[WindowKnownGap {
                    category: "capture",
                    issue: 240,
                }],
                "familia construction debe ser solo capture→#240: {id:?}"
            );
            assert!(
                gaps.iter()
                    .all(|g| g.category != "lifecycle" && g.category != "geometry"),
                "sin lifecycle→#242 ni geometry→#243 para {id:?}"
            );
        }
    }

    #[test]
    fn world_family_inventory_is_in_parity_matrix() {
        for id in WORLD_FAMILY_WINDOW_IDS {
            assert!(
                WINDOW_PARITY_MATRIX.iter().any(|e| e.id == *id),
                "familia world falta en matriz: {id:?}"
            );
            assert!(
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("src")
                    .join(window_rust_impl(*id))
                    .is_file(),
                "rust_impl de familia world ausente: {id:?}"
            );
            assert!(
                reference_geometry_primary(*id).is_some(),
                "familia world sin geometría primary: {id:?}"
            );
        }
        let station = reference_geometry_primary(FloatingWindowId::Station).expect("Station");
        assert_eq!(station.width, Some(249));
        assert_eq!(station.height, Some(117));
    }

    #[test]
    fn world_family_known_gaps_are_capture_only() {
        for id in WORLD_FAMILY_WINDOW_IDS {
            let gaps = window_known_gaps(*id);
            assert_eq!(
                gaps,
                &[WindowKnownGap {
                    category: "capture",
                    issue: 240,
                }],
                "familia world debe ser solo capture→#240: {id:?}"
            );
            assert!(
                gaps.iter()
                    .all(|g| g.category != "lifecycle" && g.category != "geometry"),
                "sin lifecycle→#242 ni geometry→#243 para {id:?}"
            );
        }
    }

    #[test]
    fn world_family_parent_child_matrix() {
        assert_eq!(
            window_child_ids(FloatingWindowId::Town),
            vec![FloatingWindowId::TownAuthority]
        );
        assert_eq!(
            window_child_ids(FloatingWindowId::Industry),
            vec![FloatingWindowId::IndustryProduction]
        );
        assert!(
            window_descendant_ids(FloatingWindowId::Town)
                .contains(&FloatingWindowId::TownAuthority)
        );
    }

    #[test]
    fn economy_family_graph_classes_are_distinct() {
        assert!(ECONOMY_FAMILY_WINDOW_IDS.contains(&FloatingWindowId::GraphIncome));
        assert!(ECONOMY_FAMILY_WINDOW_IDS.contains(&FloatingWindowId::GraphOperatingProfit));
        assert!(ECONOMY_FAMILY_WINDOW_IDS.contains(&FloatingWindowId::GraphCompanyValue));
        assert!(ECONOMY_FAMILY_WINDOW_IDS.contains(&FloatingWindowId::CompanyView));
        assert!(
            !ECONOMY_FAMILY_WINDOW_IDS
                .iter()
                .any(|id| id.storage_key() == "Graphs")
        );
        assert_eq!(
            crate::ui::graph_window::GraphKind::Income.window_id(),
            FloatingWindowId::GraphIncome
        );
        assert_eq!(
            crate::ui::graph_window::GraphKind::OperatingProfit.window_id(),
            FloatingWindowId::GraphOperatingProfit
        );
        assert_eq!(
            crate::ui::graph_window::GraphKind::CompanyValue.window_id(),
            FloatingWindowId::GraphCompanyValue
        );
    }

    #[test]
    fn economy_family_inventory_is_in_parity_matrix() {
        for id in ECONOMY_FAMILY_WINDOW_IDS {
            assert!(
                WINDOW_PARITY_MATRIX.iter().any(|e| e.id == *id),
                "familia economy falta en matriz: {id:?}"
            );
            assert!(
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("src")
                    .join(window_rust_impl(*id))
                    .is_file(),
                "rust_impl de familia economy ausente: {id:?}"
            );
            assert!(
                reference_geometry_primary(*id).is_some(),
                "familia economy sin geometría primary: {id:?}"
            );
        }
        let finances = reference_geometry_primary(FloatingWindowId::Finances).expect("Finances");
        assert_eq!(finances.placement, ReferencePlacement::Auto);
        assert_eq!(finances.width, None);
        assert_eq!(finances.height, None);
        let subsidy = reference_geometry_primary(FloatingWindowId::SubsidyList).expect("Subsidy");
        assert_eq!(subsidy.width, Some(500));
        assert_eq!(subsidy.height, Some(127));
    }

    #[test]
    fn economy_family_known_gaps_are_capture_only() {
        for id in ECONOMY_FAMILY_WINDOW_IDS {
            let gaps = window_known_gaps(*id);
            assert_eq!(
                gaps,
                &[WindowKnownGap {
                    category: "capture",
                    issue: 240,
                }],
                "familia economy debe ser solo capture→#240: {id:?}"
            );
            assert!(
                gaps.iter()
                    .all(|g| g.category != "lifecycle" && g.category != "geometry"),
                "sin lifecycle→#242 ni geometry→#243 para {id:?}"
            );
        }
    }

    #[test]
    fn settings_family_inventory_is_in_parity_matrix() {
        for id in SETTINGS_FAMILY_WINDOW_IDS {
            assert!(
                WINDOW_PARITY_MATRIX.iter().any(|e| e.id == *id),
                "familia settings falta en matriz: {id:?}"
            );
            assert!(
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("src")
                    .join(window_rust_impl(*id))
                    .is_file(),
                "rust_impl de familia settings ausente: {id:?}"
            );
            assert!(
                reference_geometry_primary(*id).is_some(),
                "familia settings sin geometría primary: {id:?}"
            );
        }
        let cheat = reference_geometry_primary(FloatingWindowId::CheatWindow).expect("Cheat");
        assert_eq!(cheat.placement, ReferencePlacement::Auto);
        assert_eq!(cheat.width, None);
        assert!(!SETTINGS_FAMILY_WINDOW_IDS.contains(&FloatingWindowId::DevConsole));
        assert!(!SETTINGS_FAMILY_WINDOW_IDS.contains(&FloatingWindowId::TileInspector));
    }

    #[test]
    fn settings_family_known_gaps_are_capture_only() {
        for id in SETTINGS_FAMILY_WINDOW_IDS {
            let gaps = window_known_gaps(*id);
            assert_eq!(
                gaps,
                &[WindowKnownGap {
                    category: "capture",
                    issue: 240,
                }],
                "familia settings debe ser solo capture→#240: {id:?}"
            );
            assert!(
                gaps.iter()
                    .all(|g| g.category != "lifecycle" && g.category != "geometry"),
                "sin lifecycle→#242 ni geometry→#243 para {id:?}"
            );
        }
    }

    #[test]
    fn dialogs_family_inventory_is_in_parity_matrix() {
        for id in DIALOGS_FAMILY_WINDOW_IDS {
            assert!(
                WINDOW_PARITY_MATRIX.iter().any(|e| e.id == *id),
                "familia dialogs falta en matriz: {id:?}"
            );
            assert!(
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("src")
                    .join(window_rust_impl(*id))
                    .is_file(),
                "rust_impl de familia dialogs ausente: {id:?}"
            );
            assert!(
                reference_geometry_primary(*id).is_some(),
                "familia dialogs sin geometría primary: {id:?}"
            );
            assert_eq!(
                WINDOW_PARITY_MATRIX
                    .iter()
                    .find(|e| e.id == *id)
                    .map(|e| e.family),
                Some("dialogs"),
                "familia matriz debe ser dialogs: {id:?}"
            );
        }
    }

    #[test]
    fn dialogs_family_known_gaps_are_capture_only() {
        for id in DIALOGS_FAMILY_WINDOW_IDS {
            let gaps = window_known_gaps(*id);
            assert_eq!(
                gaps,
                &[WindowKnownGap {
                    category: "capture",
                    issue: 240,
                }],
                "familia dialogs debe ser solo capture→#240: {id:?}"
            );
        }
    }
}
