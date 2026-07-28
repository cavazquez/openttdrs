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
/// Margen mínimo visible al clampear el arrastre.
const DRAG_MARGIN: f32 = 48.0;

/// Identifica cada ventana del juego (**una instancia por id**).
///
/// Política MVP (UI-4): no hay multi-instance. Abrir otro vehículo/estación
/// reutiliza la misma ventana y sobrescribe el `Option<ID>` del resource
/// asociado (`VehicleWindowState`, `OrderEditState`, etc.). Multi-instance
/// vía `WindowKey { kind, instance }` queda para una fase posterior.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
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

#[allow(dead_code)] // inventarios UI-0 consumidos en tests
impl FloatingWindowId {
    /// Inventario estable UI-0 (#30): actualizar al añadir variantes.
    pub(crate) const ALL: &[Self] = &[
        Self::Town,
        Self::TownDirectory,
        Self::IndustryDirectory,
        Self::Industry,
        Self::StationDirectory,
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

/// El usuario cerró la ventana con ✕; el dueño debe limpiar su estado.
#[derive(Message)]
pub(crate) struct FloatingWindowClosed(pub(crate) FloatingWindowId);

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

/// Crea el marco de una ventana flotante (oculta) y devuelve
/// `(raíz, nodo de contenido)` para que el dueño la llene.
pub(crate) fn spawn_floating_window(
    commands: &mut Commands,
    asset_server: &AssetServer,
    id: FloatingWindowId,
    title: &str,
    title_color: Color,
    pos: Vec2,
    width: f32,
) -> (Entity, Entity) {
    let mut content = Entity::PLACEHOLDER;
    let capabilities = chrome_capabilities(id);
    let root = commands
        .spawn((
            FloatingWindow { id },
            FloatingWindowChromeState::default(),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(pos.x),
                top: Val::Px(pos.y),
                width: Val::Px(width),
                flex_direction: FlexDirection::Column,
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
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

/// Posición destino del drag, clampeada para que la barra siga accesible.
#[must_use]
pub(crate) fn drag_window_position(cursor: Vec2, grab_offset: Vec2, viewport: Vec2) -> Vec2 {
    let target = cursor - grab_offset;
    Vec2::new(
        target
            .x
            .clamp(DRAG_MARGIN - 200.0, (viewport.x - DRAG_MARGIN).max(0.0)),
        target.y.clamp(0.0, (viewport.y - TITLE_BAR_H).max(0.0)),
    )
}

#[must_use]
fn resized_window_size(start_size: Vec2, cursor_delta: Vec2, viewport: Vec2) -> Vec2 {
    Vec2::new(
        (start_size.x + cursor_delta.x).clamp(MIN_WINDOW_WIDTH, viewport.x),
        (start_size.y + cursor_delta.y).clamp(MIN_WINDOW_HEIGHT, viewport.y),
    )
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
    mut windows_q: Query<(&FloatingWindow, &mut Node)>,
    mut prefs: Option<ResMut<ClientPreferences>>,
) {
    let Some(entity) = drag.window else {
        return;
    };
    if !mouse.pressed(MouseButton::Left) {
        if let Ok((win, node)) = windows_q.get(entity)
            && let (Val::Px(x), Val::Px(y)) = (node.left, node.top)
            && let Some(prefs) = prefs.as_deref_mut()
        {
            prefs.set_window_pos_by_key(win.id.storage_key(), Vec2::new(x, y));
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
    let Ok((_, mut node)) = windows_q.get_mut(entity) else {
        drag.window = None;
        return;
    };
    let pos = drag_window_position(
        cursor,
        drag.grab_offset,
        Vec2::new(window.width(), window.height()),
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
    mut windows: Query<&mut Node, With<FloatingWindow>>,
) {
    let Some(entity) = resize.window else {
        return;
    };
    if !mouse.pressed(MouseButton::Left) {
        resize.window = None;
        return;
    }
    let Ok(primary) = primary.single() else {
        return;
    };
    let Some(cursor) = primary.cursor_position() else {
        return;
    };
    let Ok(mut node) = windows.get_mut(entity) else {
        resize.window = None;
        return;
    };
    let size = resized_window_size(
        resize.start_size,
        cursor - resize.start_cursor,
        Vec2::new(primary.width(), primary.height()),
    );
    node.width = Val::Px(size.x);
    node.height = Val::Px(size.y);
}

/// Aplica posiciones guardadas una vez al entrar en partida.
fn apply_saved_floating_window_positions(
    prefs: Res<ClientPreferences>,
    mut applied: Local<bool>,
    mut windows_q: Query<(&FloatingWindow, &mut Node)>,
) {
    if *applied || windows_q.is_empty() {
        return;
    }
    for (win, mut node) in &mut windows_q {
        if let Some(pos) = prefs.window_pos_by_key(win.id.storage_key()) {
            node.left = Val::Px(pos.x);
            node.top = Val::Px(pos.y);
        }
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

/// Closebox: oculta la ventana y avisa al dueño para que limpie su estado.
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
        let Ok((win, mut vis)) = windows_q.get_mut(bar_parent.parent()) else {
            continue;
        };
        *vis = Visibility::Hidden;
        closed.write(FloatingWindowClosed(win.id));
    }
}

/// Cierra la ventana flotante visible con mayor `GlobalZIndex` (p. ej. con **Esc**).
pub(crate) fn close_top_visible_floating_window(
    windows_q: &mut Query<(&FloatingWindow, &GlobalZIndex, &mut Visibility)>,
    closed: &mut MessageWriter<FloatingWindowClosed>,
) -> bool {
    let mut best: Option<(FloatingWindowId, i32)> = None;
    for (win, z, vis) in windows_q.iter() {
        if *vis != Visibility::Visible {
            continue;
        }
        if best.map(|(_, bz)| z.0 > bz).unwrap_or(true) {
            best = Some((win.id, z.0));
        }
    }
    let Some((id, _)) = best else {
        return false;
    };
    for (win, _, mut vis) in windows_q.iter_mut() {
        if win.id == id {
            *vis = Visibility::Hidden;
            closed.write(FloatingWindowClosed(id));
            return true;
        }
    }
    false
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
        );
        assert_eq!(pos, Vec2::new(260.0, 190.0));
    }

    #[test]
    fn drag_position_clamps_to_viewport() {
        let viewport = Vec2::new(1280.0, 720.0);
        let off_top = drag_window_position(Vec2::new(100.0, -50.0), Vec2::ZERO, viewport);
        assert_eq!(off_top.y, 0.0);
        let off_right = drag_window_position(Vec2::new(5000.0, 100.0), Vec2::ZERO, viewport);
        assert!(off_right.x <= viewport.x - 48.0);
        let off_bottom = drag_window_position(Vec2::new(100.0, 5000.0), Vec2::ZERO, viewport);
        assert!(off_bottom.y <= viewport.y - 20.0);
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

        let help = chrome_capabilities(FloatingWindowId::Help);
        assert!(!help.shade && !help.sticky && !help.resize);
    }

    #[test]
    fn resize_clamps_to_minimum_and_viewport() {
        assert_eq!(
            resized_window_size(
                Vec2::new(300.0, 200.0),
                Vec2::new(50.0, 30.0),
                Vec2::splat(800.0)
            ),
            Vec2::new(350.0, 230.0)
        );
        assert_eq!(
            resized_window_size(
                Vec2::new(200.0, 100.0),
                Vec2::new(-500.0, -500.0),
                Vec2::splat(800.0)
            ),
            Vec2::new(MIN_WINDOW_WIDTH, MIN_WINDOW_HEIGHT)
        );
        assert_eq!(
            resized_window_size(
                Vec2::new(700.0, 600.0),
                Vec2::splat(500.0),
                Vec2::new(900.0, 700.0)
            ),
            Vec2::new(900.0, 700.0)
        );
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
