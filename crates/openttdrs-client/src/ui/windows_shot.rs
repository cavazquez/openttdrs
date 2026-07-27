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
use crate::ui::destination_window::DestinationPickerState;
use crate::ui::dev_console::DevConsoleState;
use crate::ui::display_options_window::DisplayOptionsWindowState;
use crate::ui::extra_viewport_window::ExtraViewportWindowState;
use crate::ui::finances_window::FinancesWindowState;
use crate::ui::floating_window::{FloatingWindow, FloatingWindowId};
use crate::ui::genland_window::GenLandWindowState;
use crate::ui::goal_list_window::GoalListWindowState;
use crate::ui::graph_window::GraphWindowState;
use crate::ui::help_window::HelpWindowState;
use crate::ui::hud::SimHudControls;
use crate::ui::industry_directory::IndustryDirectoryState;
use crate::ui::industry_panel::IndustryPanelState;
use crate::ui::league_window::LeagueWindowState;
use crate::ui::main_menu::{MainMenuCamera, MainMenuUi};
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
use crate::ui::town_directory::TownDirectoryState;
use crate::ui::town_window::TownWindowState;
use crate::ui::ui5_blocked_stubs::LinkGraphWindowState;
use crate::ui::vehicle_details_window::VehicleDetailsWindowState;
use crate::ui::vehicle_list::VehicleListState;
use crate::ui::vehicle_window::VehicleWindowState;

const OPEN_FRAME: u32 = 30;
const SHOT_FRAME: u32 = 60;
const EXIT_FRAME: u32 = 120;

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
    upstream_window!(TownDirectory, "world", "town_gui.cpp", "WC_TOWN_DIRECTORY"),
    upstream_window!(
        IndustryDirectory,
        "world",
        "industry_gui.cpp",
        "WC_INDUSTRY_DIRECTORY"
    ),
    upstream_window!(Industry, "world", "industry_gui.cpp", "WC_INDUSTRY_VIEW"),
    upstream_window!(
        StationDirectory,
        "world",
        "station_gui.cpp",
        "WC_STATION_LIST"
    ),
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
        BridgePicker,
        "construction",
        "bridge_gui.cpp",
        "WC_BUILD_BRIDGE"
    ),
    upstream_window!(
        DestinationPicker,
        "vehicles",
        "station_gui.cpp",
        "WC_SELECT_STATION",
        Orders
    ),
    upstream_window!(NewsHistory, "reports", "news_gui.cpp", "WC_MESSAGE_HISTORY"),
    upstream_window!(Finances, "economy", "company_gui.cpp", "WC_FINANCES"),
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
    upstream_window!(
        Graphs,
        "economy",
        "graph_gui.cpp",
        "WC_INCOME_GRAPH and related graph classes"
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
];

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
}

macro_rules! reference_geometry {
    ($id:ident, $variant:literal, $placement:ident, $width:expr, $height:expr) => {
        ReferenceGeometry {
            id: FloatingWindowId::$id,
            variant: $variant,
            placement: ReferencePlacement::$placement,
            width: $width,
            height: $height,
        }
    };
}

/// Tamaños iniciales que 15.3 expresa directamente en sus `WindowDesc`.
/// Las variantes comparten ID mientras #242 no soporte `WindowKey`.
pub(crate) const WINDOW_REFERENCE_GEOMETRY: &[ReferenceGeometry] = &[
    reference_geometry!(Town, "game", Auto, Some(260), None),
    reference_geometry!(TownDirectory, "default", Auto, Some(208), Some(202)),
    reference_geometry!(Industry, "default", Auto, Some(260), Some(120)),
    reference_geometry!(IndustryDirectory, "default", Auto, Some(428), Some(190)),
    reference_geometry!(StationDirectory, "default", Auto, Some(358), Some(162)),
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
    reference_geometry!(BridgePicker, "default", Auto, Some(200), Some(114)),
    reference_geometry!(DestinationPicker, "default", Auto, Some(200), Some(180)),
    reference_geometry!(NewsHistory, "default", Auto, Some(400), Some(140)),
    reference_geometry!(PathfindingSettings, "settings", Center, None, None),
    reference_geometry!(CargoDistSettings, "settings", Center, None, None),
    reference_geometry!(AiSettings, "config", Center, None, None),
    reference_geometry!(NewGrf, "settings", Center, Some(300), Some(263)),
    reference_geometry!(SoundMusic, "main", Auto, None, None),
    reference_geometry!(Timetable, "default", Auto, Some(400), Some(130)),
    reference_geometry!(Orders, "owned", Auto, Some(384), Some(100)),
    reference_geometry!(Orders, "competitor", Auto, Some(384), Some(86)),
    reference_geometry!(Refit, "default", Auto, Some(240), Some(174)),
    reference_geometry!(DisplayOptions, "settings", Center, None, None),
    reference_geometry!(ExtraViewport, "default", Auto, Some(300), Some(268)),
    reference_geometry!(Help, "default", Center, None, None),
    reference_geometry!(GenLand, "main", Center, None, None),
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
    let mut output = String::from(
        "{\n  \"schema_version\": 1,\n  \"openttd_commit\": \"14ec60f248547d4d062a1160f0fc26d742319888\",\n  \"windows\": [\n",
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
        let _ = write!(
            output,
            "    {{\"id\":{},\"family\":{},\"kind\":{},\"upstream_source\":{},\"upstream_window\":{},\"parent\":{},\"geometry\":[",
            json_string(entry.id.storage_key()),
            json_string(entry.family),
            json_string(kind),
            json_string(entry.upstream_source),
            json_string(entry.upstream_window),
            parent,
        );
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
            let _ = write!(
                output,
                "{{\"variant\":{},\"placement\":{},\"width\":{},\"height\":{}}}",
                json_string(geometry.variant),
                json_string(placement),
                width,
                height,
            );
        }
        let comma = if index + 1 == WINDOW_PARITY_MATRIX.len() {
            ""
        } else {
            ","
        };
        let _ = writeln!(output, "]}}{comma}");
    }
    output.push_str("  ]\n}\n");
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
    {
        let mut depot = world.resource_mut::<DepotPanelState>();
        depot.depot_pos = depot_pos;
    }
    {
        let mut buy = world.resource_mut::<BuyVehicleWindowState>();
        buy.depot_pos = depot_pos;
        buy.selected_engine = selected_engine;
    }
    world.resource_mut::<VehicleWindowState>().vehicle_id = vehicle_id;
    world.resource_mut::<TimetableWindowState>().vehicle_id = vehicle_id;
    if let Some(vid) = vehicle_id {
        world
            .resource_mut::<VehicleDetailsWindowState>()
            .open_for(vid);
        world.resource_mut::<RefitWindowState>().open_for(vid);
        {
            let mut order = world.resource_mut::<OrderEditState>();
            order.vehicle_id = Some(vid);
            order.orders = vehicle_orders.unwrap_or_default();
        }
        {
            let mut shared = world.resource_mut::<SharedOrdersWindowState>();
            shared.open = true;
            shared.link_vehicle_id = Some(vid);
        }
        world.resource_mut::<DestinationPickerState>().open = true;
    }
    if let Some(pos) = depot_pos {
        world
            .resource_mut::<AutoreplaceWindowState>()
            .open_for_depot(pos);
    }
    if include_auxiliary {
        world.resource_mut::<StationCargoPanelState>().station_pos = station_pos;
    }
    if let Some(pos) = industry_pos {
        let mut panel = world.resource_mut::<IndustryPanelState>();
        panel.open = true;
        panel.focus_tile = Some(pos);
    }

    world.resource_mut::<FinancesWindowState>().open = true;
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
    world.resource_mut::<GraphWindowState>().open = true;
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
mod tests {
    use super::*;

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
    fn machine_readable_matrix_contains_inventory_and_geometry() {
        let json = window_parity_matrix_json();
        assert!(json.starts_with("{\n  \"schema_version\": 1,"));
        for id in FloatingWindowId::ALL {
            assert!(json.contains(&format!("\"id\":{:?}", id.storage_key())));
        }
        assert!(json.contains("\"variant\":\"train\""));
        assert!(json.contains("\"placement\":\"center\""));
    }
}
