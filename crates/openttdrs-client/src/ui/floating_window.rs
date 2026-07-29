//! Mini-framework de ventanas flotantes estilo `OpenTTD`.
//!
//! Cada ventana tiene marco con barra de título (arrastrable), botón ✕ y un
//! nodo de contenido que llena el dueño. Pueden convivir varias abiertas;
//! clic en la barra la trae al frente. El cierre se comunica con el mensaje
//! [`FloatingWindowClosed`] para que cada ventana limpie su estado.

use bevy::prelude::*;
use bevy::ui::widget::ImageNode;
use bevy::window::PrimaryWindow;

use crate::bevy_app::UpdateSet;
use crate::settings::ClientPreferences;
use crate::state::ClientScreen;
use crate::ui::font::UiFontRole;
use crate::ui::toolbar::{BuildMenuUi, ToolbarTooltipTarget};
use crate::ui::windows_shot::{
    ReferencePlacement, reference_geometry_primary, window_descendant_ids,
};

/// Fondo marrón clásico de las ventanas de `OpenTTD`.
pub(crate) const WINDOW_BG: Color = Color::srgb(0.45, 0.36, 0.26);
/// Borde exterior oscuro.
pub(crate) const WINDOW_BORDER: Color = Color::srgb(0.13, 0.10, 0.07);
const WINDOW_FOCUSED_BORDER: Color = Color::srgb(0.76, 0.65, 0.39);
/// Texto crema por defecto.
pub(crate) const WINDOW_TEXT: Color = Color::srgb(0.95, 0.93, 0.82);
/// Barra de título marrón (ventanas genéricas).
pub(crate) const TITLE_BROWN: Color = Color::srgb(0.36, 0.28, 0.18);
/// Barra de título rosa/granate (ventanas de vehículos en `OpenTTD`).
pub(crate) const TITLE_CRIMSON: Color = Color::srgb(0.62, 0.24, 0.28);
/// Barra de título verde (ventanas de pueblo usan crema; verde para construcción).
pub(crate) const TITLE_CREAM: Color = Color::srgb(0.62, 0.56, 0.42);

/// Z base de las ventanas flotantes (sobre paneles fijos, bajo modales 2900+).
const WINDOW_BASE_Z: i32 = 2400;
/// Por encima del menú principal (`GlobalZIndex(3000)`) para recibir clics.
pub(crate) const MENU_OVERLAY_WINDOW_Z: i32 = 3100;
/// Altura de la barra de título.
const TITLE_BAR_H: f32 = 20.0;
/// Ancho del closebox; el sprite vanilla es 8×9 y queda centrado sin escalar.
const CLOSEBOX_W: f32 = 16.0;
const CLOSEBOX_ICON_W: f32 = 8.0;
const CLOSEBOX_ICON_H: f32 = 9.0;
const CLOSEBOX_IDLE: Color = Color::srgb(0.45, 0.36, 0.26);
const CLOSEBOX_HOVER: Color = Color::srgb(0.53, 0.43, 0.31);
const CLOSEBOX_PRESSED: Color = Color::srgb(0.29, 0.23, 0.16);
const CHROME_BUTTON_W: f32 = 16.0;
const CHROME_ICON_SIZE: f32 = 8.0;
const RESIZE_HANDLE_SIZE: f32 = 16.0;
const MIN_WINDOW_WIDTH: f32 = 160.0;
const MIN_WINDOW_HEIGHT: f32 = TITLE_BAR_H + 48.0;
/// Evitar solaparse con la toolbar superior al colocar (#243).
const TOOLBAR_AVOID: f32 = 40.0;
/// Evitar solaparse con la statusbar inferior al colocar (#243).
const STATUSBAR_AVOID: f32 = 28.0;
/// Viewport por defecto al spawnear (setup aún no tiene PrimaryWindow).
const DEFAULT_LAYOUT_VIEWPORT: Vec2 = Vec2::new(1280.0, 720.0);

/// Identifica la **clase** de ventana del juego.
///
/// Política actual: una entidad Bevy por clase (`instance == 0` en
/// [`WindowKey`]). Abrir otro vehículo/estación reutiliza la misma ventana y
/// sobrescribe el `Option<ID>` del resource. Multi-instance completo: #242.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) enum FloatingWindowId {
    Town,
    /// Directorio global de pueblos.
    TownDirectory,
    /// Directorio global de industrias.
    IndustryDirectory,
    /// Ficha de industria (viewport + stats; #179).
    Industry,
    /// Lista global de estaciones.
    StationDirectory,
    /// Ficha de estación (antes HUD `station_panel`; #245).
    Station,
    /// Lista global de flota (filtro por tipo).
    VehicleList,
    /// Lista de subvenciones (ofertas y contratos).
    SubsidyList,
    Depot,
    BuyVehicle,
    Vehicle,
    /// Detalles del vehículo (tabs Info/Carga/Capacidad/Totales); hija de View (#173).
    VehicleDetails,
    /// «Selección de estación» de tren (opciones de la herramienta).
    RailStationPicker,
    /// Selección de aeropuerto (clase / tipo / orientación).
    AirportPicker,
    /// Selección de parada road NewGRF (bus / camión).
    RoadStopPicker,
    /// Selección de objeto (vanilla + NewGRF 1×1).
    ObjectPicker,
    /// «Selección de puente» tras definir el tramo.
    BridgePicker,
    /// Lista de destinos para la ruta del vehículo.
    DestinationPicker,
    /// Historial de noticias (Message history).
    NewsHistory,
    /// Finanzas de la compañía.
    Finances,
    /// Configuración Off / Summary / Full por tipo de noticia.
    NewsSettings,
    /// Ajustes PBS / pathfinding (`pf.wait_for_pbs_path`, etc.).
    PathfindingSettings,
    /// Ajustes `CargoDist` (`Manual` / `Asymmetric` / `Symmetric`).
    CargoDistSettings,
    /// Ajustes / debug de IA rival (TransCargo; UI-8 / #44).
    AiSettings,
    /// Stack NewGRF activo (config-only; sin Action0–14).
    NewGrf,
    /// Volúmenes SFX/música, flags de sonido y jukebox OpenMSX.
    SoundMusic,
    /// Horario detallado del vehículo (F4).
    Timetable,
    /// Órdenes del vehículo (`OrdersWindow` OpenTTD / #176).
    Orders,
    /// Lista de cargas para refit en depósito.
    Refit,
    /// Pools de órdenes compartidas.
    SharedOrders,
    /// Reglas de autoreemplazo de motores.
    Autoreplace,
    /// Gráficos económicos (ingresos / beneficio).
    Graphs,
    /// Tarifas de pago por tipo de carga.
    CargoPaymentRates,
    /// Opciones de visualización (Display Options).
    DisplayOptions,
    /// Segunda cámara / ExtraViewport.
    ExtraViewport,
    /// Lista de carteles del mapa.
    SignList,
    /// Leyenda Link Graph / flujos observados.
    LinkGraphLegend,
    /// Selección de tipo/densidad de señales ferroviarias.
    SignalPicker,
    /// Ayuda / About / mapa de hotkeys (UI-7).
    Help,
    /// Consola / métricas FPS / toggles de debug (UI-8).
    DevConsole,
    /// Inspección estructurada del tile seleccionado (UI-8).
    TileInspector,
    /// Cheats formales (UI-8 / #45).
    CheatWindow,
    /// Generar paisaje (editor #42 GenLand).
    GenLand,
    /// Objetivos GameScript-lite (#43).
    Goals,
    /// Historia / story book (#43).
    Story,
    /// Liga / ranking de compañías (#43).
    League,
}

/// Clave equivalente a `WindowClass + WindowNumber` de OpenTTD (#242).
///
/// Hoy todas las ventanas usan `instance = 0` ([`WindowKey::singleton`]).
/// El tipo existe para migrar Closed/queries sin romper inventarios (#240).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) struct WindowKey {
    pub(crate) class: FloatingWindowId,
    pub(crate) instance: u32,
}

impl WindowKey {
    #[must_use]
    pub(crate) const fn singleton(class: FloatingWindowId) -> Self {
        Self {
            class,
            instance: 0,
        }
    }

    #[must_use]
    pub(crate) const fn class(self) -> FloatingWindowId {
        self.class
    }
}

#[allow(dead_code)] // inventarios UI-0 consumidos en tests
impl FloatingWindowId {
    /// Inventario estable UI-0 (#30): actualizar al añadir variantes.
    pub(crate) const ALL: &[Self] = &[
        Self::Town,
        Self::TownDirectory,
        Self::IndustryDirectory,
        Self::Industry,
        Self::StationDirectory,
        Self::Station,
        Self::VehicleList,
        Self::SubsidyList,
        Self::Depot,
        Self::BuyVehicle,
        Self::Vehicle,
        Self::VehicleDetails,
        Self::RailStationPicker,
        Self::AirportPicker,
        Self::RoadStopPicker,
        Self::ObjectPicker,
        Self::BridgePicker,
        Self::DestinationPicker,
        Self::NewsHistory,
        Self::Finances,
        Self::NewsSettings,
        Self::PathfindingSettings,
        Self::CargoDistSettings,
        Self::AiSettings,
        Self::NewGrf,
        Self::SoundMusic,
        Self::Timetable,
        Self::Orders,
        Self::Refit,
        Self::SharedOrders,
        Self::Autoreplace,
        Self::Graphs,
        Self::CargoPaymentRates,
        Self::DisplayOptions,
        Self::ExtraViewport,
        Self::SignList,
        Self::LinkGraphLegend,
        Self::SignalPicker,
        Self::Help,
        Self::DevConsole,
        Self::TileInspector,
        Self::CheatWindow,
        Self::GenLand,
        Self::Goals,
        Self::Story,
        Self::League,
    ];

    /// Clave estable para persistir posición en `ClientPreferences`.
    #[must_use]
    pub(crate) const fn storage_key(self) -> &'static str {
        match self {
            Self::Town => "Town",
            Self::TownDirectory => "TownDirectory",
            Self::IndustryDirectory => "IndustryDirectory",
            Self::Industry => "Industry",
            Self::StationDirectory => "StationDirectory",
            Self::Station => "Station",
            Self::VehicleList => "VehicleList",
            Self::SubsidyList => "SubsidyList",
            Self::Depot => "Depot",
            Self::BuyVehicle => "BuyVehicle",
            Self::Vehicle => "Vehicle",
            Self::VehicleDetails => "VehicleDetails",
            Self::RailStationPicker => "RailStationPicker",
            Self::AirportPicker => "AirportPicker",
            Self::RoadStopPicker => "RoadStopPicker",
            Self::ObjectPicker => "ObjectPicker",
            Self::BridgePicker => "BridgePicker",
            Self::DestinationPicker => "DestinationPicker",
            Self::NewsHistory => "NewsHistory",
            Self::Finances => "Finances",
            Self::NewsSettings => "NewsSettings",
            Self::PathfindingSettings => "PathfindingSettings",
            Self::CargoDistSettings => "CargoDistSettings",
            Self::AiSettings => "AiSettings",
            Self::NewGrf => "NewGrf",
            Self::SoundMusic => "SoundMusic",
            Self::Timetable => "Timetable",
            Self::Orders => "Orders",
            Self::Refit => "Refit",
            Self::SharedOrders => "SharedOrders",
            Self::Autoreplace => "Autoreplace",
            Self::Graphs => "Graphs",
            Self::CargoPaymentRates => "CargoPaymentRates",
            Self::DisplayOptions => "DisplayOptions",
            Self::ExtraViewport => "ExtraViewport",
            Self::SignList => "SignList",
            Self::LinkGraphLegend => "LinkGraphLegend",
            Self::SignalPicker => "SignalPicker",
            Self::Help => "Help",
            Self::DevConsole => "DevConsole",
            Self::TileInspector => "TileInspector",
            Self::CheatWindow => "CheatWindow",
            Self::GenLand => "GenLand",
            Self::Goals => "Goals",
            Self::Story => "Story",
            Self::League => "League",
        }
    }
}

/// Raíz de una ventana flotante.
#[derive(Component)]
pub(crate) struct FloatingWindow {
    pub(crate) id: FloatingWindowId,
    /// Clave class+instance; hoy siempre `singleton(id)` (#242 foundation).
    pub(crate) key: WindowKey,
}

/// Barra de título: zona de agarre del drag.
#[derive(Component)]
pub(crate) struct FloatingWindowTitleBar;

/// Texto del título (para actualizarlo en sync).
#[derive(Component)]
pub(crate) struct FloatingWindowTitleText(pub(crate) FloatingWindowId);

#[derive(Component)]
pub(crate) struct FloatingWindowCloseButton;

#[derive(Component)]
struct FloatingWindowContent(FloatingWindowId);

#[derive(Component, Default)]
struct FloatingWindowChromeState {
    shaded: bool,
    sticky: bool,
    /// Altura declarada antes de plegar (`Auto` o el tamaño elegido por resize).
    unshaded_height: Option<Val>,
}

#[derive(Component)]
struct FloatingWindowShadeButton;

#[derive(Component)]
struct FloatingWindowStickyButton;

#[derive(Component)]
struct FloatingWindowResizeButton;

#[derive(Clone, Copy, Default)]
struct WindowChromeCapabilities {
    shade: bool,
    sticky: bool,
    resize: bool,
}

/// Sólo activa widgets presentes en el `WindowDesc` equivalente de 15.3.
fn chrome_capabilities(id: FloatingWindowId) -> WindowChromeCapabilities {
    let shade = matches!(
        id,
        FloatingWindowId::Town
            | FloatingWindowId::TownDirectory
            | FloatingWindowId::IndustryDirectory
            | FloatingWindowId::Industry
            | FloatingWindowId::StationDirectory
            | FloatingWindowId::Station
            | FloatingWindowId::VehicleList
            | FloatingWindowId::SubsidyList
            | FloatingWindowId::BuyVehicle
            | FloatingWindowId::Vehicle
            | FloatingWindowId::VehicleDetails
            | FloatingWindowId::RailStationPicker
            | FloatingWindowId::AirportPicker
            | FloatingWindowId::RoadStopPicker
            | FloatingWindowId::ObjectPicker
            | FloatingWindowId::NewsHistory
            | FloatingWindowId::Finances
            | FloatingWindowId::SoundMusic
            | FloatingWindowId::Orders
            | FloatingWindowId::SharedOrders
            | FloatingWindowId::Autoreplace
            | FloatingWindowId::Graphs
            | FloatingWindowId::CargoPaymentRates
            | FloatingWindowId::SignList
            | FloatingWindowId::SignalPicker
            | FloatingWindowId::CheatWindow
            | FloatingWindowId::Goals
            | FloatingWindowId::Story
    );
    let sticky = shade
        && !matches!(
            id,
            FloatingWindowId::RailStationPicker
                | FloatingWindowId::AirportPicker
                | FloatingWindowId::RoadStopPicker
                | FloatingWindowId::ObjectPicker
                | FloatingWindowId::SignalPicker
        );
    let resize = matches!(
        id,
        FloatingWindowId::Town
            | FloatingWindowId::TownDirectory
            | FloatingWindowId::IndustryDirectory
            | FloatingWindowId::Industry
            | FloatingWindowId::StationDirectory
            | FloatingWindowId::Station
            | FloatingWindowId::VehicleList
            | FloatingWindowId::SubsidyList
            | FloatingWindowId::Depot
            | FloatingWindowId::BuyVehicle
            | FloatingWindowId::Vehicle
            | FloatingWindowId::VehicleDetails
            | FloatingWindowId::BridgePicker
            | FloatingWindowId::DestinationPicker
            | FloatingWindowId::NewsHistory
            | FloatingWindowId::NewsSettings
            | FloatingWindowId::PathfindingSettings
            | FloatingWindowId::CargoDistSettings
            | FloatingWindowId::AiSettings
            | FloatingWindowId::NewGrf
            | FloatingWindowId::Timetable
            | FloatingWindowId::Orders
            | FloatingWindowId::Refit
            | FloatingWindowId::SharedOrders
            | FloatingWindowId::Autoreplace
            | FloatingWindowId::Graphs
            | FloatingWindowId::CargoPaymentRates
            | FloatingWindowId::DisplayOptions
            | FloatingWindowId::ExtraViewport
            | FloatingWindowId::SignList
            | FloatingWindowId::Goals
            | FloatingWindowId::Story
            | FloatingWindowId::League
    );
    WindowChromeCapabilities {
        shade,
        sticky,
        resize,
    }
}

/// El usuario cerró la ventana; el dueño limpia el estado de esa instancia (#242).
#[derive(Message, Clone, Copy, Debug)]
pub(crate) struct FloatingWindowClosed(pub(crate) WindowKey);

impl FloatingWindowClosed {
    #[must_use]
    pub(crate) const fn class(self) -> FloatingWindowId {
        self.0.class
    }

    #[must_use]
    pub(crate) const fn key(self) -> WindowKey {
        self.0
    }
}

/// Drag en curso: ventana agarrada y offset cursor→esquina.
#[derive(Resource, Default)]
pub(crate) struct WindowDragState {
    window: Option<Entity>,
    grab_offset: Vec2,
}

#[derive(Resource, Default)]
struct WindowResizeState {
    window: Option<Entity>,
    start_cursor: Vec2,
    start_size: Vec2,
}

/// Contador para traer ventanas al frente.
#[derive(Resource)]
pub(crate) struct WindowZCounter(i32);

impl Default for WindowZCounter {
    fn default() -> Self {
        Self(WINDOW_BASE_Z)
    }
}

impl WindowZCounter {
    /// Incrementa y devuelve el nuevo z para elevar una ventana.
    #[allow(dead_code)]
    pub(crate) fn bump(&mut self) -> i32 {
        self.0 += 1;
        self.0
    }
}

pub(crate) struct FloatingWindowPlugin;

impl Plugin for FloatingWindowPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WindowDragState>()
            .init_resource::<WindowResizeState>()
            .init_resource::<WindowZCounter>()
            .add_message::<FloatingWindowClosed>()
            .add_systems(
                Update,
                (
                    begin_window_drag,
                    drag_floating_windows,
                    update_focused_window_style,
                    (begin_window_resize, resize_floating_windows).chain(),
                    close_window_buttons,
                    update_window_chrome_button_style,
                    update_window_chrome_buttons,
                    apply_saved_floating_window_positions,
                )
                    .in_set(UpdateSet::Ui)
                    .run_if(in_state(ClientScreen::InGame)),
            )
            .add_systems(
                Update,
                (
                    begin_window_drag,
                    drag_floating_windows,
                    update_focused_window_style,
                    (begin_window_resize, resize_floating_windows).chain(),
                    close_window_buttons,
                    update_window_chrome_button_style,
                    update_window_chrome_buttons,
                )
                    .in_set(UpdateSet::Ui)
                    .run_if(in_state(ClientScreen::MainMenu)),
            );
    }
}

/// `TextFont` con la fuente UTF-8 del proyecto (tildes, eñes, símbolos).
pub(crate) fn window_text_font(asset_server: &AssetServer, role: UiFontRole) -> TextFont {
    crate::ui::font::ui_text_font_loaded(asset_server, role)
}


/// Trae al frente una ventana existente (#242 reopen).
#[allow(dead_code)]
pub(crate) fn raise_floating_window(
    windows_q: &mut Query<(Entity, &FloatingWindow, &mut GlobalZIndex, &mut Visibility)>,
    z_counter: &mut WindowZCounter,
    key: WindowKey,
) -> bool {
    for (entity, win, mut z, mut vis) in windows_q.iter_mut() {
        if win.key != key {
            continue;
        }
        *vis = Visibility::Visible;
        z.0 = z_counter.bump();
        let _ = entity;
        return true;
    }
    false
}

/// Busca la entidad raíz con esa clave.
#[allow(dead_code)]
#[must_use]
pub(crate) fn find_floating_window_entity(
    windows_q: &Query<(Entity, &FloatingWindow)>,
    key: WindowKey,
) -> Option<Entity> {
    windows_q
        .iter()
        .find(|(_, win)| win.key == key)
        .map(|(entity, _)| entity)
}

/// Coloca una ventana según `WDP_AUTO` / `WDP_CENTER` y clampea fuera de
/// toolbar/statusbar (#243). `preferred` es la posición del caller en Auto.
#[must_use]
pub(crate) fn place_window(
    placement: ReferencePlacement,
    size: Vec2,
    viewport: Vec2,
    preferred: Vec2,
) -> Vec2 {
    let pos = match placement {
        ReferencePlacement::Center => Vec2::new(
            ((viewport.x - size.x) * 0.5).max(0.0),
            ((viewport.y - size.y) * 0.5).max(TOOLBAR_AVOID),
        ),
        ReferencePlacement::Auto => preferred,
    };
    clamp_window_position(pos, size, viewport)
}

/// Clampa con el tamaño real de la ventana (no sólo el caption).
#[must_use]
pub(crate) fn clamp_window_position(pos: Vec2, size: Vec2, viewport: Vec2) -> Vec2 {
    let max_x = (viewport.x - size.x.min(viewport.x)).max(0.0);
    let min_y = TOOLBAR_AVOID.min((viewport.y - TITLE_BAR_H).max(0.0));
    let max_y = (viewport.y - STATUSBAR_AVOID - TITLE_BAR_H)
        .max(min_y)
        .min((viewport.y - TITLE_BAR_H).max(0.0));
    Vec2::new(pos.x.clamp(0.0, max_x), pos.y.clamp(min_y, max_y))
}

/// Crea el marco de una ventana flotante (oculta) y devuelve
/// `(raíz, nodo de contenido)` para que el dueño la llene.
///
/// Si hay [`reference_geometry_primary`], aplica width/height/placement del
/// `WindowDesc` 15.3 (#243). `pos`/`width` quedan como fallback.
pub(crate) fn spawn_floating_window(
    commands: &mut Commands,
    asset_server: &AssetServer,
    id: FloatingWindowId,
    title: &str,
    title_color: Color,
    pos: Vec2,
    width: f32,
) -> (Entity, Entity) {
    spawn_floating_window_keyed(
        commands,
        asset_server,
        WindowKey::singleton(id),
        title,
        title_color,
        pos,
        width,
    )
}

/// Como [`spawn_floating_window`] pero con `WindowKey` explícito (#242).
pub(crate) fn spawn_floating_window_keyed(
    commands: &mut Commands,
    asset_server: &AssetServer,
    key: WindowKey,
    title: &str,
    title_color: Color,
    pos: Vec2,
    width: f32,
) -> (Entity, Entity) {
    let id = key.class;
    let mut content = Entity::PLACEHOLDER;
    let capabilities = chrome_capabilities(id);
    let geo = reference_geometry_primary(id);
    let width = geo
        .and_then(|geometry| geometry.width)
        .map_or(width, |w| f32::from(w));
    let height = geo.and_then(|geometry| geometry.height).map(f32::from);
    let placement = geo.map_or(ReferencePlacement::Auto, |geometry| geometry.placement);
    let size_for_place = Vec2::new(width, height.unwrap_or(MIN_WINDOW_HEIGHT));
    let pos = place_window(placement, size_for_place, DEFAULT_LAYOUT_VIEWPORT, pos);
    let mut root_node = Node {
        position_type: PositionType::Absolute,
        left: Val::Px(pos.x),
        top: Val::Px(pos.y),
        width: Val::Px(width),
        flex_direction: FlexDirection::Column,
        border: UiRect::all(Val::Px(1.0)),
        ..default()
    };
    if let Some(h) = height {
        root_node.height = Val::Px(h);
    }
    let root = commands
        .spawn((
            FloatingWindow { id, key },
            FloatingWindowChromeState::default(),
            root_node,
            BackgroundColor(WINDOW_BG),
            BorderColor::all(WINDOW_BORDER),
            GlobalZIndex(WINDOW_BASE_Z),
            Visibility::Hidden,
            BuildMenuUi,
            Interaction::default(),
        ))
        .with_children(|win| {
            win.spawn((
                FloatingWindowTitleBar,
                Button,
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(TITLE_BAR_H),
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    border: UiRect::bottom(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(title_color),
                BorderColor::all(WINDOW_BORDER),
                Interaction::default(),
                BuildMenuUi,
            ))
            .with_children(|bar| {
                bar.spawn((
                    FloatingWindowCloseButton,
                    ToolbarTooltipTarget {
                        text: "Cerrar ventana",
                    },
                    Button,
                    Node {
                        width: Val::Px(CLOSEBOX_W),
                        height: Val::Percent(100.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        border: UiRect::right(Val::Px(1.0)),
                        ..default()
                    },
                    BackgroundColor(CLOSEBOX_IDLE),
                    BorderColor::all(WINDOW_BORDER),
                    Interaction::default(),
                    BuildMenuUi,
                    children![(
                        ImageNode::new(
                            asset_server.load::<Image>("assets/opengfx/tiles/window_close.png"),
                        ),
                        Node {
                            width: Val::Px(CLOSEBOX_ICON_W),
                            height: Val::Px(CLOSEBOX_ICON_H),
                            ..default()
                        },
                    )],
                ));
                bar.spawn((
                    Node {
                        flex_grow: 1.0,
                        justify_content: JustifyContent::Center,
                        ..default()
                    },
                    children![(
                        FloatingWindowTitleText(id),
                        Text::new(title),
                        window_text_font(asset_server, UiFontRole::Caption),
                        TextColor(WINDOW_TEXT),
                    )],
                ));
                if capabilities.shade {
                    bar.spawn((
                        FloatingWindowShadeButton,
                        ToolbarTooltipTarget {
                            text: "Plegar / desplegar ventana",
                        },
                        Button,
                        Node {
                            width: Val::Px(CHROME_BUTTON_W),
                            height: Val::Percent(100.0),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            border: UiRect::left(Val::Px(1.0)),
                            ..default()
                        },
                        BackgroundColor(CLOSEBOX_IDLE),
                        BorderColor::all(WINDOW_BORDER),
                        Interaction::default(),
                        BuildMenuUi,
                        children![(
                            ImageNode::new(
                                asset_server
                                    .load::<Image>("assets/opengfx/tiles/window_unshade.png",)
                            ),
                            Node {
                                width: Val::Px(CHROME_ICON_SIZE),
                                height: Val::Px(CHROME_ICON_SIZE),
                                ..default()
                            },
                        )],
                    ));
                }
                if capabilities.sticky {
                    bar.spawn((
                        FloatingWindowStickyButton,
                        ToolbarTooltipTarget {
                            text: "Fijar / liberar ventana",
                        },
                        Button,
                        Node {
                            width: Val::Px(CHROME_BUTTON_W),
                            height: Val::Percent(100.0),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            border: UiRect::left(Val::Px(1.0)),
                            ..default()
                        },
                        BackgroundColor(CLOSEBOX_IDLE),
                        BorderColor::all(WINDOW_BORDER),
                        Interaction::default(),
                        BuildMenuUi,
                        children![(
                            ImageNode::new(
                                asset_server
                                    .load::<Image>("assets/opengfx/tiles/window_pin_down.png",)
                            ),
                            Node {
                                width: Val::Px(CHROME_ICON_SIZE),
                                height: Val::Px(CHROME_ICON_SIZE),
                                ..default()
                            },
                        )],
                    ));
                }
            });
            content = win
                .spawn((
                    FloatingWindowContent(id),
                    Node {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Column,
                        padding: UiRect {
                            left: Val::Px(6.0),
                            right: Val::Px(if capabilities.resize {
                                RESIZE_HANDLE_SIZE
                            } else {
                                6.0
                            }),
                            top: Val::Px(6.0),
                            bottom: Val::Px(if capabilities.resize {
                                RESIZE_HANDLE_SIZE
                            } else {
                                6.0
                            }),
                        },
                        row_gap: Val::Px(4.0),
                        ..default()
                    },
                    BuildMenuUi,
                ))
                .id();
            if capabilities.resize {
                win.spawn((
                    FloatingWindowResizeButton,
                    ToolbarTooltipTarget {
                        text: "Redimensionar ventana",
                    },
                    Button,
                    Node {
                        position_type: PositionType::Absolute,
                        right: Val::Px(0.0),
                        bottom: Val::Px(0.0),
                        width: Val::Px(RESIZE_HANDLE_SIZE),
                        height: Val::Px(RESIZE_HANDLE_SIZE),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BackgroundColor(CLOSEBOX_IDLE),
                    Interaction::default(),
                    BuildMenuUi,
                    children![(
                        ImageNode::new(
                            asset_server.load::<Image>("assets/opengfx/tiles/window_resize.png"),
                        ),
                        Node {
                            width: Val::Px(CHROME_ICON_SIZE),
                            height: Val::Px(CHROME_ICON_SIZE),
                            ..default()
                        },
                    )],
                ));
            }
        })
        .id();
    (root, content)
}

/// Posición destino del drag, clampeada con el tamaño real de la ventana (#243).
#[must_use]
pub(crate) fn drag_window_position(
    cursor: Vec2,
    grab_offset: Vec2,
    viewport: Vec2,
    window_size: Vec2,
) -> Vec2 {
    let target = cursor - grab_offset;
    clamp_window_position(target, window_size, viewport)
}

#[must_use]
fn resized_window_size(
    start_size: Vec2,
    cursor_delta: Vec2,
    viewport: Vec2,
    min_size: Vec2,
    step: Vec2,
) -> Vec2 {
    let step_x = step.x.max(1.0);
    let step_y = step.y.max(1.0);
    let raw = start_size + cursor_delta;
    let snapped = Vec2::new(
        (raw.x / step_x).round() * step_x,
        (raw.y / step_y).round() * step_y,
    );
    Vec2::new(
        snapped.x.clamp(min_size.x.max(MIN_WINDOW_WIDTH), viewport.x),
        snapped
            .y
            .clamp(min_size.y.max(MIN_WINDOW_HEIGHT), viewport.y),
    )
}

fn resize_limits_for(id: FloatingWindowId) -> (Vec2, Vec2) {
    let geo = reference_geometry_primary(id);
    let min = Vec2::new(
        geo.and_then(|g| g.min_width)
            .map_or(MIN_WINDOW_WIDTH, f32::from),
        geo.and_then(|g| g.min_height)
            .map_or(MIN_WINDOW_HEIGHT, f32::from),
    );
    let step = geo
        .and_then(|g| g.resize_step)
        .map_or(Vec2::ONE, |(x, y)| Vec2::new(f32::from(x), f32::from(y)));
    (min, step)
}

/// Presionar la barra de título: inicia drag y trae la ventana al frente.
fn begin_window_drag(
    bars: Query<(&Interaction, &ChildOf), (Changed<Interaction>, With<FloatingWindowTitleBar>)>,
    mut windows_q: Query<(Entity, &Node, &mut GlobalZIndex), With<FloatingWindow>>,
    primary: Query<&Window, With<PrimaryWindow>>,
    screen: Res<State<ClientScreen>>,
    mut drag: ResMut<WindowDragState>,
    mut z_counter: ResMut<WindowZCounter>,
) {
    let Ok(window) = primary.single() else {
        return;
    };
    for (interaction, child_of) in &bars {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let Ok((entity, node, mut z)) = windows_q.get_mut(child_of.parent()) else {
            continue;
        };
        let (Val::Px(left), Val::Px(top)) = (node.left, node.top) else {
            continue;
        };
        let Some(cursor) = window.cursor_position() else {
            continue;
        };
        drag.window = Some(entity);
        drag.grab_offset = cursor - Vec2::new(left, top);
        z_counter.0 += 1;
        // En el menú hay que quedar por encima de GlobalZIndex(3000).
        if *screen.get() == ClientScreen::MainMenu {
            z_counter.0 = z_counter.0.max(MENU_OVERLAY_WINDOW_Z + 1);
        }
        z.0 = z_counter.0;
    }
}

/// Mueve la ventana agarrada mientras el botón izquierdo siga presionado.
fn drag_floating_windows(
    mouse: Res<ButtonInput<MouseButton>>,
    primary: Query<&Window, With<PrimaryWindow>>,
    mut drag: ResMut<WindowDragState>,
    mut windows_q: Query<(&FloatingWindow, &mut Node, Option<&ComputedNode>)>,
    mut prefs: Option<ResMut<ClientPreferences>>,
) {
    let Some(entity) = drag.window else {
        return;
    };
    if !mouse.pressed(MouseButton::Left) {
        if let Ok((win, node, _)) = windows_q.get(entity)
            && let (Val::Px(x), Val::Px(y)) = (node.left, node.top)
            && let Some(prefs) = prefs.as_deref_mut()
        {
            let size = match (node.width, node.height) {
                (Val::Px(w), Val::Px(h)) => Some(Vec2::new(w, h)),
                _ => None,
            };
            prefs.set_window_layout_by_key(win.id.storage_key(), Vec2::new(x, y), size);
        }
        drag.window = None;
        return;
    }
    let Ok(window) = primary.single() else {
        return;
    };
    let Some(cursor) = window.cursor_position() else {
        return;
    };
    let Ok((_, mut node, computed)) = windows_q.get_mut(entity) else {
        drag.window = None;
        return;
    };
    let window_size = computed.map_or_else(
        || {
            let w = match node.width {
                Val::Px(v) => v,
                _ => MIN_WINDOW_WIDTH,
            };
            let h = match node.height {
                Val::Px(v) => v,
                _ => MIN_WINDOW_HEIGHT,
            };
            Vec2::new(w, h)
        },
        |c| c.size(),
    );
    let pos = drag_window_position(
        cursor,
        drag.grab_offset,
        Vec2::new(window.width(), window.height()),
        window_size,
    );
    node.left = Val::Px(pos.x);
    node.top = Val::Px(pos.y);
}

fn update_focused_window_style(
    mut windows: ParamSet<(
        Query<(Entity, &GlobalZIndex, &Visibility), With<FloatingWindow>>,
        Query<(Entity, &mut BorderColor), With<FloatingWindow>>,
    )>,
) {
    let focused = windows
        .p0()
        .iter()
        .filter(|(_, _, visibility)| **visibility != Visibility::Hidden)
        .max_by_key(|(_, z, _)| z.0)
        .map(|(entity, _, _)| entity);
    for (entity, mut border) in &mut windows.p1() {
        *border = BorderColor::all(if Some(entity) == focused {
            WINDOW_FOCUSED_BORDER
        } else {
            WINDOW_BORDER
        });
    }
}

fn begin_window_resize(
    buttons: Query<
        (&Interaction, &ChildOf),
        (Changed<Interaction>, With<FloatingWindowResizeButton>),
    >,
    windows: Query<&ComputedNode, With<FloatingWindow>>,
    primary: Query<&Window, With<PrimaryWindow>>,
    mut resize: ResMut<WindowResizeState>,
) {
    let Ok(primary) = primary.single() else {
        return;
    };
    let Some(cursor) = primary.cursor_position() else {
        return;
    };
    for (interaction, parent) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let Ok(computed) = windows.get(parent.parent()) else {
            continue;
        };
        resize.window = Some(parent.parent());
        resize.start_cursor = cursor;
        resize.start_size = computed.size();
    }
}

fn resize_floating_windows(
    mouse: Res<ButtonInput<MouseButton>>,
    primary: Query<&Window, With<PrimaryWindow>>,
    mut resize: ResMut<WindowResizeState>,
    mut windows: Query<(&FloatingWindow, &mut Node)>,
    mut prefs: Option<ResMut<ClientPreferences>>,
) {
    let Some(entity) = resize.window else {
        return;
    };
    if !mouse.pressed(MouseButton::Left) {
        if let Ok((win, node)) = windows.get(entity)
            && let (Val::Px(x), Val::Px(y), Val::Px(w), Val::Px(h)) =
                (node.left, node.top, node.width, node.height)
            && let Some(prefs) = prefs.as_deref_mut()
        {
            prefs.set_window_layout_by_key(
                win.id.storage_key(),
                Vec2::new(x, y),
                Some(Vec2::new(w, h)),
            );
        }
        resize.window = None;
        return;
    }
    let Ok(primary) = primary.single() else {
        return;
    };
    let Some(cursor) = primary.cursor_position() else {
        return;
    };
    let Ok((win, mut node)) = windows.get_mut(entity) else {
        resize.window = None;
        return;
    };
    let (min_size, step) = resize_limits_for(win.id);
    let size = resized_window_size(
        resize.start_size,
        cursor - resize.start_cursor,
        Vec2::new(primary.width(), primary.height()),
        min_size,
        step,
    );
    node.width = Val::Px(size.x);
    node.height = Val::Px(size.y);
}

/// Aplica posiciones/tamaños guardados una vez al entrar en partida (#243).
fn apply_saved_floating_window_positions(
    prefs: Res<ClientPreferences>,
    primary: Query<&Window, With<PrimaryWindow>>,
    mut applied: Local<bool>,
    mut windows_q: Query<(&FloatingWindow, &mut Node)>,
) {
    if *applied || windows_q.is_empty() {
        return;
    }
    let viewport = primary.single().map_or(DEFAULT_LAYOUT_VIEWPORT, |window| {
        Vec2::new(window.width(), window.height())
    });
    for (win, mut node) in &mut windows_q {
        let Some((pos, size)) = prefs.window_layout_by_key(win.id.storage_key()) else {
            continue;
        };
        if let Some(size) = size {
            node.width = Val::Px(size.x);
            node.height = Val::Px(size.y);
        }
        let size_for_clamp = match (node.width, node.height) {
            (Val::Px(w), Val::Px(h)) => Vec2::new(w, h),
            (Val::Px(w), _) => Vec2::new(w, MIN_WINDOW_HEIGHT),
            _ => Vec2::new(MIN_WINDOW_WIDTH, MIN_WINDOW_HEIGHT),
        };
        let pos = clamp_window_position(pos, size_for_clamp, viewport);
        node.left = Val::Px(pos.x);
        node.top = Val::Px(pos.y);
    }
    *applied = true;
}

fn closebox_color(interaction: Interaction) -> Color {
    match interaction {
        Interaction::None => CLOSEBOX_IDLE,
        Interaction::Hovered => CLOSEBOX_HOVER,
        Interaction::Pressed => CLOSEBOX_PRESSED,
    }
}

/// Replica el relieve visual del closebox en sus estados interactivos.
fn update_window_chrome_button_style(
    mut buttons: Query<
        (&Interaction, &mut BackgroundColor),
        (
            Changed<Interaction>,
            Or<(
                With<FloatingWindowCloseButton>,
                With<FloatingWindowShadeButton>,
                With<FloatingWindowStickyButton>,
                With<FloatingWindowResizeButton>,
            )>,
        ),
    >,
) {
    for (interaction, mut background) in &mut buttons {
        background.0 = closebox_color(*interaction);
    }
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn update_window_chrome_buttons(
    shade_buttons: Query<
        (&Interaction, &ChildOf, &Children),
        (Changed<Interaction>, With<FloatingWindowShadeButton>),
    >,
    sticky_buttons: Query<
        (&Interaction, &ChildOf, &Children),
        (Changed<Interaction>, With<FloatingWindowStickyButton>),
    >,
    title_parents: Query<&ChildOf, With<FloatingWindowTitleBar>>,
    mut windows: Query<(&FloatingWindow, &mut FloatingWindowChromeState, &mut Node)>,
    mut contents: Query<
        (&FloatingWindowContent, &mut Visibility),
        Without<FloatingWindowResizeButton>,
    >,
    mut resize_buttons: Query<
        (&ChildOf, &mut Visibility),
        (
            With<FloatingWindowResizeButton>,
            Without<FloatingWindowContent>,
        ),
    >,
    mut images: Query<&mut ImageNode>,
    asset_server: Res<AssetServer>,
) {
    for (interaction, title, children) in &shade_buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let Ok(root) = title_parents.get(title.parent()) else {
            continue;
        };
        let root_entity = root.parent();
        let Ok((window, mut state, mut window_node)) = windows.get_mut(root_entity) else {
            continue;
        };
        state.shaded = !state.shaded;
        if state.shaded {
            state.unshaded_height = Some(window_node.height);
            window_node.height = Val::Px(TITLE_BAR_H);
        } else {
            window_node.height = state.unshaded_height.take().unwrap_or(Val::Auto);
        }
        for (content, mut visibility) in &mut contents {
            if content.0 == window.id {
                *visibility = if state.shaded {
                    Visibility::Hidden
                } else {
                    Visibility::Inherited
                };
            }
        }
        for (parent, mut visibility) in &mut resize_buttons {
            if parent.parent() == root_entity {
                *visibility = if state.shaded {
                    Visibility::Hidden
                } else {
                    Visibility::Inherited
                };
            }
        }
        for child in children.iter() {
            if let Ok(mut image) = images.get_mut(child) {
                image.image = asset_server.load(if state.shaded {
                    "assets/opengfx/tiles/window_shade.png"
                } else {
                    "assets/opengfx/tiles/window_unshade.png"
                });
            }
        }
    }

    for (interaction, title, children) in &sticky_buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let Ok(root) = title_parents.get(title.parent()) else {
            continue;
        };
        let Ok((_window, mut state, _node)) = windows.get_mut(root.parent()) else {
            continue;
        };
        state.sticky = !state.sticky;
        for child in children.iter() {
            if let Ok(mut image) = images.get_mut(child) {
                image.image = asset_server.load(if state.sticky {
                    "assets/opengfx/tiles/window_pin_up.png"
                } else {
                    "assets/opengfx/tiles/window_pin_down.png"
                });
            }
        }
    }
}

/// Oculta `root` y descendientes con la misma `instance` (#242).
fn hide_window_and_descendants(
    root: WindowKey,
    windows_q: &mut Query<(&FloatingWindow, &mut Visibility)>,
    closed: &mut MessageWriter<FloatingWindowClosed>,
) {
    let mut keys = vec![root];
    for class in window_descendant_ids(root.class) {
        keys.push(WindowKey {
            class,
            instance: root.instance,
        });
    }
    for key in keys {
        for (win, mut vis) in windows_q.iter_mut() {
            if win.key == key && *vis != Visibility::Hidden {
                *vis = Visibility::Hidden;
                closed.write(FloatingWindowClosed(key));
            }
        }
    }
}

/// Closebox: oculta la ventana (y hijas) y avisa al dueño para que limpie su estado.
fn close_window_buttons(
    buttons: Query<
        (&Interaction, &ChildOf),
        (Changed<Interaction>, With<FloatingWindowCloseButton>),
    >,
    bars: Query<&ChildOf, With<FloatingWindowTitleBar>>,
    mut windows_q: Query<(&FloatingWindow, &mut Visibility)>,
    mut closed: MessageWriter<FloatingWindowClosed>,
) {
    for (interaction, child_of) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let Ok(bar_parent) = bars.get(child_of.parent()) else {
            continue;
        };
        let Ok((win, _)) = windows_q.get(bar_parent.parent()) else {
            continue;
        };
        let key = win.key;
        hide_window_and_descendants(key, &mut windows_q, &mut closed);
    }
}

/// Cierra la ventana flotante visible con mayor `GlobalZIndex` (p. ej. con **Esc**).
pub(crate) fn close_top_visible_floating_window(
    windows_q: &mut Query<(&FloatingWindow, &GlobalZIndex, &mut Visibility)>,
    closed: &mut MessageWriter<FloatingWindowClosed>,
) -> bool {
    let mut best: Option<(WindowKey, i32)> = None;
    for (win, z, vis) in windows_q.iter() {
        if *vis != Visibility::Visible {
            continue;
        }
        if best.map(|(_, bz)| z.0 > bz).unwrap_or(true) {
            best = Some((win.key, z.0));
        }
    }
    let Some((key, _)) = best else {
        return false;
    };
    // Esc cierra la superior y sus hijas de la misma instance (#242).
    let mut keys = vec![key];
    for class in window_descendant_ids(key.class) {
        keys.push(WindowKey {
            class,
            instance: key.instance,
        });
    }
    let mut closed_any = false;
    for close_key in keys {
        for (win, _, mut vis) in windows_q.iter_mut() {
            if win.key == close_key && *vis != Visibility::Hidden {
                *vis = Visibility::Hidden;
                closed.write(FloatingWindowClosed(close_key));
                closed_any = true;
            }
        }
    }
    closed_any
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drag_position_follows_cursor_minus_grab_offset() {
        let pos = drag_window_position(
            Vec2::new(300.0, 200.0),
            Vec2::new(40.0, 10.0),
            Vec2::new(1280.0, 720.0),
            Vec2::new(250.0, 134.0),
        );
        assert_eq!(pos, Vec2::new(260.0, 190.0));
    }

    #[test]
    fn drag_position_clamps_to_viewport() {
        let viewport = Vec2::new(1280.0, 720.0);
        let size = Vec2::new(260.0, 120.0);
        let off_top = drag_window_position(Vec2::new(100.0, -50.0), Vec2::ZERO, viewport, size);
        assert_eq!(off_top.y, TOOLBAR_AVOID);
        let off_right = drag_window_position(Vec2::new(5000.0, 100.0), Vec2::ZERO, viewport, size);
        assert!(off_right.x <= viewport.x - size.x);
        let off_bottom = drag_window_position(Vec2::new(100.0, 5000.0), Vec2::ZERO, viewport, size);
        assert!(off_bottom.y <= viewport.y - TITLE_BAR_H);
    }

    #[test]
    fn place_window_centers_and_avoids_chrome() {
        let viewport = Vec2::new(1280.0, 720.0);
        let size = Vec2::new(300.0, 263.0);
        let centered = place_window(ReferencePlacement::Center, size, viewport, Vec2::ZERO);
        assert!((centered.x - (viewport.x - size.x) * 0.5).abs() < 0.5);
        assert!(centered.y >= TOOLBAR_AVOID);
        let auto = place_window(
            ReferencePlacement::Auto,
            size,
            viewport,
            Vec2::new(10.0, 10.0),
        );
        assert_eq!(auto.y, TOOLBAR_AVOID);
    }

    #[test]
    fn storage_keys_are_stable() {
        assert_eq!(FloatingWindowId::Help.storage_key(), "Help");
        assert_eq!(FloatingWindowId::NewGrf.storage_key(), "NewGrf");
        assert_eq!(
            FloatingWindowId::DisplayOptions.storage_key(),
            "DisplayOptions"
        );
    }

    #[test]
    fn closebox_interaction_has_distinct_visual_states() {
        assert_eq!(closebox_color(Interaction::None), CLOSEBOX_IDLE);
        assert_eq!(closebox_color(Interaction::Hovered), CLOSEBOX_HOVER);
        assert_eq!(closebox_color(Interaction::Pressed), CLOSEBOX_PRESSED);
        assert_ne!(CLOSEBOX_IDLE, CLOSEBOX_PRESSED);
    }

    #[test]
    fn chrome_capabilities_are_opt_in_from_upstream_descriptors() {
        let town = chrome_capabilities(FloatingWindowId::Town);
        assert!(town.shade && town.sticky && town.resize);

        let station = chrome_capabilities(FloatingWindowId::Station);
        assert!(station.shade && station.sticky && station.resize);

        let help = chrome_capabilities(FloatingWindowId::Help);
        assert!(!help.shade && !help.sticky && !help.resize);

        // Economy (#247): Finances shade/sticky; Graphs también resize.
        let finances = chrome_capabilities(FloatingWindowId::Finances);
        assert!(finances.shade && finances.sticky && !finances.resize);
        let graphs = chrome_capabilities(FloatingWindowId::Graphs);
        assert!(graphs.shade && graphs.sticky && graphs.resize);

        // Settings (#248): Cheat shade/sticky; NewGrf resize centrado sin shade.
        let cheat = chrome_capabilities(FloatingWindowId::CheatWindow);
        assert!(cheat.shade && cheat.sticky && !cheat.resize);
        let newgrf = chrome_capabilities(FloatingWindowId::NewGrf);
        assert!(!newgrf.shade && !newgrf.sticky && newgrf.resize);
    }

    #[test]
    fn construction_tool_pickers_shade_without_sticky() {
        for id in [
            FloatingWindowId::RailStationPicker,
            FloatingWindowId::AirportPicker,
            FloatingWindowId::RoadStopPicker,
            FloatingWindowId::ObjectPicker,
            FloatingWindowId::SignalPicker,
        ] {
            let caps = chrome_capabilities(id);
            assert!(caps.shade, "{id:?} debe tener shade");
            assert!(!caps.sticky, "{id:?} no debe ser sticky");
        }
        // Bridge: resize sí, shade/sticky no (descriptor distinto).
        let bridge = chrome_capabilities(FloatingWindowId::BridgePicker);
        assert!(!bridge.shade);
        assert!(!bridge.sticky);
        assert!(bridge.resize);
    }

    #[test]
    fn resize_clamps_to_minimum_and_viewport() {
        assert_eq!(
            resized_window_size(
                Vec2::new(300.0, 200.0),
                Vec2::new(50.0, 30.0),
                Vec2::splat(800.0),
                Vec2::new(MIN_WINDOW_WIDTH, MIN_WINDOW_HEIGHT),
                Vec2::ONE,
            ),
            Vec2::new(350.0, 230.0)
        );
        assert_eq!(
            resized_window_size(
                Vec2::new(200.0, 100.0),
                Vec2::new(-500.0, -500.0),
                Vec2::splat(800.0),
                Vec2::new(MIN_WINDOW_WIDTH, MIN_WINDOW_HEIGHT),
                Vec2::ONE,
            ),
            Vec2::new(MIN_WINDOW_WIDTH, MIN_WINDOW_HEIGHT)
        );
        assert_eq!(
            resized_window_size(
                Vec2::new(700.0, 600.0),
                Vec2::splat(500.0),
                Vec2::new(900.0, 700.0),
                Vec2::new(MIN_WINDOW_WIDTH, MIN_WINDOW_HEIGHT),
                Vec2::ONE,
            ),
            Vec2::new(900.0, 700.0)
        );
    }

    #[test]
    fn window_component_carries_singleton_key() {
        let win = FloatingWindow {
            id: FloatingWindowId::Vehicle,
            key: WindowKey::singleton(FloatingWindowId::Vehicle),
        };
        assert_eq!(win.key.class(), win.id);
        assert_eq!(win.key.instance, 0);
    }

    #[test]
    fn vehicle_close_cascade_lists_orders_chain() {
        let descendants = window_descendant_ids(FloatingWindowId::Vehicle);
        assert!(descendants.contains(&FloatingWindowId::Orders));
        assert!(descendants.contains(&FloatingWindowId::DestinationPicker));
        assert!(descendants.contains(&FloatingWindowId::VehicleDetails));
    }

    #[test]
    fn closed_message_carries_window_key_instance() {
        let key = WindowKey {
            class: FloatingWindowId::Vehicle,
            instance: 7,
        };
        let msg = FloatingWindowClosed(key);
        assert_eq!(msg.class(), FloatingWindowId::Vehicle);
        assert_eq!(msg.key().instance, 7);
    }

    #[test]
    fn cascade_keys_keep_instance_across_parent_child() {
        let root = WindowKey {
            class: FloatingWindowId::Vehicle,
            instance: 11,
        };
        let mut keys = vec![root];
        for class in window_descendant_ids(root.class) {
            keys.push(WindowKey {
                class,
                instance: root.instance,
            });
        }
        assert!(keys.iter().all(|k| k.instance == 11));
        assert!(
            keys.iter()
                .any(|k| k.class == FloatingWindowId::Orders && k.instance == 11)
        );
        let other = WindowKey {
            class: FloatingWindowId::Orders,
            instance: 22,
        };
        assert!(!keys.contains(&other));
    }

    #[test]
    fn chrome_closebox_uses_opengfx_sprite_path_not_unicode() {
        // El marco spawnea ImageNode con window_close.png (regresión #241).
        let path = "assets/opengfx/tiles/window_close.png";
        assert!(path.contains("window_close"));
        assert!(!path.contains('×') && !path.contains('✕'));
    }

    #[test]
    fn plugin_build_registers_systems() {
        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin);
        app.init_state::<ClientScreen>();
        app.configure_sets(Update, UpdateSet::Ui);
        app.add_plugins(FloatingWindowPlugin);
        assert!(app.world().contains_resource::<WindowDragState>());
    }

    #[test]
    fn chrome_system_queries_are_runtime_compatible() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(bevy::asset::AssetPlugin::default())
            .init_asset::<Image>()
            .add_systems(Update, update_window_chrome_buttons);

        // Bevy valida conflictos entre queries al inicializar el sistema,
        // no durante la compilación ni al registrarlo en el schedule.
        app.update();
    }
}
