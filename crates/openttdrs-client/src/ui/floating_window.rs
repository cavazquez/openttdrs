//! Mini-framework de ventanas flotantes estilo `OpenTTD`.
//!
//! Cada ventana tiene marco con barra de título (arrastrable), botón ✕ y un
//! nodo de contenido que llena el dueño. Pueden convivir varias abiertas;
//! clic en la barra la trae al frente. El cierre se comunica con el mensaje
//! [`FloatingWindowClosed`] para que cada ventana limpie su estado.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::bevy_app::UpdateSet;
use crate::settings::ClientPreferences;
use crate::state::ClientScreen;
use crate::ui::font::UiFontRole;
use crate::ui::toolbar::BuildMenuUi;

/// Fondo marrón clásico de las ventanas de `OpenTTD`.
pub(crate) const WINDOW_BG: Color = Color::srgb(0.45, 0.36, 0.26);
/// Borde exterior oscuro.
pub(crate) const WINDOW_BORDER: Color = Color::srgb(0.13, 0.10, 0.07);
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
    /// Lista global de estaciones.
    StationDirectory,
    /// Lista global de flota (filtro por tipo).
    VehicleList,
    /// Lista de subvenciones (ofertas y contratos).
    SubsidyList,
    Depot,
    BuyVehicle,
    Vehicle,
    /// «Selección de estación» de tren (opciones de la herramienta).
    RailStationPicker,
    /// Selección de aeropuerto (clase / tipo / orientación).
    AirportPicker,
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
        Self::StationDirectory,
        Self::VehicleList,
        Self::SubsidyList,
        Self::Depot,
        Self::BuyVehicle,
        Self::Vehicle,
        Self::RailStationPicker,
        Self::AirportPicker,
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
            Self::StationDirectory => "StationDirectory",
            Self::VehicleList => "VehicleList",
            Self::SubsidyList => "SubsidyList",
            Self::Depot => "Depot",
            Self::BuyVehicle => "BuyVehicle",
            Self::Vehicle => "Vehicle",
            Self::RailStationPicker => "RailStationPicker",
            Self::AirportPicker => "AirportPicker",
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

/// El usuario cerró la ventana con ✕; el dueño debe limpiar su estado.
#[derive(Message)]
pub(crate) struct FloatingWindowClosed(pub(crate) FloatingWindowId);

/// Drag en curso: ventana agarrada y offset cursor→esquina.
#[derive(Resource, Default)]
pub(crate) struct WindowDragState {
    window: Option<Entity>,
    grab_offset: Vec2,
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
            .init_resource::<WindowZCounter>()
            .add_message::<FloatingWindowClosed>()
            .add_systems(
                Update,
                (
                    begin_window_drag,
                    drag_floating_windows,
                    close_window_buttons,
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
                    close_window_buttons,
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
    let root = commands
        .spawn((
            FloatingWindow { id },
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
                    Button,
                    Node {
                        width: Val::Px(20.0),
                        height: Val::Percent(100.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        border: UiRect::right(Val::Px(1.0)),
                        ..default()
                    },
                    BackgroundColor(Color::NONE),
                    BorderColor::all(WINDOW_BORDER),
                    Interaction::default(),
                    BuildMenuUi,
                    children![(
                        Text::new("×"),
                        window_text_font(asset_server, UiFontRole::Caption),
                        TextColor(WINDOW_TEXT),
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
                // Hueco simétrico al botón ✕ para que el título quede centrado.
                bar.spawn(Node {
                    width: Val::Px(20.0),
                    ..default()
                });
            });
            content = win
                .spawn((
                    Node {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Column,
                        padding: UiRect::all(Val::Px(6.0)),
                        row_gap: Val::Px(4.0),
                        ..default()
                    },
                    BuildMenuUi,
                ))
                .id();
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

/// Botón ✕: oculta la ventana y avisa al dueño para que limpie su estado.
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
    fn plugin_build_registers_systems() {
        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin);
        app.init_state::<ClientScreen>();
        app.configure_sets(Update, UpdateSet::Ui);
        app.add_plugins(FloatingWindowPlugin);
        assert!(app.world().contains_resource::<WindowDragState>());
    }
}
