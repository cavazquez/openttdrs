//! Ventana flotante «Nuevos vehículos» estilo `OpenTTD`.
//!
//! Lista los modelos del catálogo filtrados por tipo de depósito (vía →
//! trenes, carretera → buses/camiones), con panel de stats del modelo
//! seleccionado y botón de compra (`BuildVehicleAtDepot`).

use bevy::prelude::*;
use bevy::ui::widget::ImageNode;
use openttdrs_core::{
    CargoType, Command, DepotPurchaseKind, EngineCatalogSort, EngineDef, RoadEngineFilter,
    TileCoord, TileKind, VehicleKind, apply_command, calendar_year_at_tick, engines_for_depot_kind,
};

use crate::render::{RemapMapVisualsPending, TruckHandles};
use crate::state::SimWorld;
use crate::ui::floating_window::{
    FloatingWindow, FloatingWindowClosed, FloatingWindowId, FloatingWindowTitleText, TITLE_CRIMSON,
    WINDOW_TEXT, spawn_floating_window, window_text_font,
};
use crate::ui::font::UiFontRole;
use crate::ui::hud::{HudBuildFeedback, push_build_command_error};
use crate::ui::toolbar::BuildMenuUi;

/// Filas suficientes para el catálogo más largo (trenes).
const BUY_ROWS: usize = 16;

const BTN_BG: Color = Color::srgb(0.36, 0.31, 0.21);
const BTN_BORDER: Color = Color::srgb(0.66, 0.58, 0.38);

#[derive(Resource, Default)]
pub(crate) struct BuyVehicleWindowState {
    /// Depósito desde el que se abrió la ventana (`None` = cerrada).
    pub(crate) depot_pos: Option<TileCoord>,
    pub(crate) selected_engine: Option<u16>,
    pub(crate) sort: EngineCatalogSort,
    pub(crate) road_filter: RoadEngineFilter,
}

#[derive(Component, Clone, Copy)]
pub(crate) struct BuyVehicleRow {
    slot: usize,
}

#[derive(Component, Clone, Copy)]
pub(crate) struct BuyVehicleRowText {
    slot: usize,
}

#[derive(Component)]
pub(crate) struct BuyVehicleStatsText;

#[derive(Component)]
pub(crate) struct BuyVehiclePreviewImage;

#[derive(Component)]
pub(crate) struct BuyVehicleBuyButton;

#[derive(Component, Clone, Copy)]
pub(crate) enum BuyVehicleToolbarButton {
    SortName,
    SortPrice,
    SortSpeed,
    SortYear,
    FilterAll,
    FilterBus,
    FilterTruck,
}

#[derive(Component)]
pub(crate) struct BuyVehicleRoadToolbar;

pub(crate) fn setup_buy_window(mut commands: Commands, asset_server: Res<AssetServer>) {
    let asset_server = &*asset_server;
    let (_root, content) = spawn_floating_window(
        &mut commands,
        asset_server,
        FloatingWindowId::BuyVehicle,
        "Nuevos vehículos",
        TITLE_CRIMSON,
        Vec2::new(120.0, 120.0),
        430.0,
    );
    commands.entity(content).with_children(|panel| {
        panel
            .spawn((
                BuyVehicleRoadToolbar,
                Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(3.0),
                    flex_wrap: FlexWrap::Wrap,
                    row_gap: Val::Px(2.0),
                    display: Display::None,
                    ..default()
                },
                BuildMenuUi,
            ))
            .with_children(|row| {
                spawn_toolbar_button(
                    row,
                    asset_server,
                    BuyVehicleToolbarButton::SortName,
                    "Nombre",
                );
                spawn_toolbar_button(
                    row,
                    asset_server,
                    BuyVehicleToolbarButton::SortPrice,
                    "Precio",
                );
                spawn_toolbar_button(
                    row,
                    asset_server,
                    BuyVehicleToolbarButton::SortSpeed,
                    "Vel.",
                );
                spawn_toolbar_button(row, asset_server, BuyVehicleToolbarButton::SortYear, "Año");
                spawn_toolbar_button(
                    row,
                    asset_server,
                    BuyVehicleToolbarButton::FilterAll,
                    "Todos",
                );
                spawn_toolbar_button(
                    row,
                    asset_server,
                    BuyVehicleToolbarButton::FilterBus,
                    "Buses",
                );
                spawn_toolbar_button(
                    row,
                    asset_server,
                    BuyVehicleToolbarButton::FilterTruck,
                    "Camiones",
                );
            });
        panel
            .spawn(Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(1.0),
                ..default()
            })
            .with_children(|list| {
                for slot in 0..BUY_ROWS {
                    list.spawn((
                        Button,
                        BuyVehicleRow { slot },
                        Node {
                            width: Val::Percent(100.0),
                            height: Val::Px(20.0),
                            padding: UiRect::horizontal(Val::Px(6.0)),
                            justify_content: JustifyContent::FlexStart,
                            align_items: AlignItems::Center,
                            border: UiRect::all(Val::Px(1.0)),
                            display: Display::None,
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.22, 0.18, 0.12)),
                        BorderColor::all(Color::srgb(0.45, 0.39, 0.27)),
                        Interaction::default(),
                        BuildMenuUi,
                        children![(
                            BuyVehicleRowText { slot },
                            Text::new(""),
                            window_text_font(asset_server, UiFontRole::Caption),
                            TextColor(Color::srgb(0.92, 0.88, 0.72)),
                        )],
                    ));
                }
            });
        panel.spawn((
            BuyVehiclePreviewImage,
            ImageNode::new(asset_server.load::<Image>("assets/opengfx/tiles/vehicle_train_e.png")),
            Node {
                width: Val::Px(96.0),
                height: Val::Px(64.0),
                margin: UiRect::top(Val::Px(4.0)),
                align_self: AlignSelf::Center,
                display: Display::None,
                ..default()
            },
        ));
        panel.spawn((
            BuyVehicleStatsText,
            Text::new(""),
            window_text_font(asset_server, UiFontRole::Caption),
            TextColor(WINDOW_TEXT),
            Node {
                margin: UiRect::top(Val::Px(4.0)),
                ..default()
            },
        ));
        panel.spawn((
            Button,
            BuyVehicleBuyButton,
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(24.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(BTN_BG),
            BorderColor::all(BTN_BORDER),
            Interaction::default(),
            BuildMenuUi,
            children![(
                Text::new("Comprar vehículo"),
                window_text_font(asset_server, UiFontRole::Caption),
                TextColor(Color::srgb(0.92, 0.88, 0.72)),
            )],
        ));
    });
}

fn spawn_toolbar_button(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    action: BuyVehicleToolbarButton,
    label: &'static str,
) {
    parent.spawn((
        Button,
        action,
        Node {
            min_width: Val::Px(52.0),
            padding: UiRect::horizontal(Val::Px(4.0)),
            height: Val::Px(20.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        BackgroundColor(BTN_BG),
        BorderColor::all(BTN_BORDER),
        Interaction::default(),
        BuildMenuUi,
        children![(
            Text::new(label),
            window_text_font(asset_server, UiFontRole::Caption),
            TextColor(Color::srgb(0.92, 0.88, 0.72)),
        )],
    ));
}

/// Modelos visibles según depósito, año, filtro y orden.
pub(crate) fn engines_for_buy_window(
    sim: &SimWorld,
    depot_pos: TileCoord,
    sort: EngineCatalogSort,
    road_filter: RoadEngineFilter,
) -> Vec<&'static EngineDef> {
    let depot_kind = match sim.state.map.get_kind(depot_pos) {
        Some(TileKind::RailDepot) => DepotPurchaseKind::Rail,
        Some(TileKind::ShipDepot) => DepotPurchaseKind::Ship,
        Some(TileKind::Airport) => DepotPurchaseKind::Aircraft,
        _ => DepotPurchaseKind::Road,
    };
    let year = calendar_year_at_tick(sim.state.tick);
    let mut engines = engines_for_depot_kind(depot_kind, year, sort, road_filter);
    if depot_kind == DepotPurchaseKind::Aircraft {
        let heliport = openttdrs_core::airport_tile_is_heliport(&sim.state.map, depot_pos);
        engines.retain(|e| openttdrs_core::aircraft_is_helicopter(e.id) == heliport);
    }
    engines
}

fn cargo_label(cargo: Option<CargoType>) -> &'static str {
    match cargo {
        Some(CargoType::Passengers) => "pasajeros",
        Some(CargoType::Mail) => "sacas de correo",
        Some(CargoType::Goods) => "cajas de mercancías",
        Some(CargoType::Coal) => "toneladas de carbón",
        Some(CargoType::Wood) => "toneladas de madera",
        Some(CargoType::Oil) => "litros de petróleo",
        None => "nada (solo locomotora)",
    }
}

fn stats_text(engine: &EngineDef) -> String {
    format!(
        "Precio: ${}  Peso: {}t\nVelocidad: {}km/h  Potencia: {}cv\nCoste de operación: ${}/año\nCapacidad: {} {}\nDiseñado: {}  Fiabilidad: {}%",
        engine.price,
        engine.weight_t,
        engine.speed_kmh(),
        engine.power_hp,
        engine.running_cost_year,
        engine.capacity,
        cargo_label(engine.cargo),
        engine.intro_year,
        engine.reliability_pct,
    )
}

fn buy_window_title(sim: &SimWorld, depot_pos: TileCoord) -> &'static str {
    match sim.state.map.get_kind(depot_pos) {
        Some(TileKind::RailDepot) => "Nuevos vehículos ferroviarios",
        Some(TileKind::ShipDepot) => "Nuevos barcos",
        Some(TileKind::Airport)
            if openttdrs_core::airport_tile_is_heliport(&sim.state.map, depot_pos) =>
        {
            "Nuevos helicópteros"
        }
        Some(TileKind::Airport) => "Nuevos aviones",
        _ => "Nuevos vehículos de carretera",
    }
}

fn preview_sprite_for_engine(trucks: &TruckHandles, engine: &EngineDef) -> Handle<Image> {
    if engine.kind == VehicleKind::Train {
        trucks.train_preview(engine.train_image_index, 2)
    } else {
        trucks.intro_sprite_for_engine(engine, 2)
    }
}

#[allow(clippy::too_many_arguments)] // sistema ECS Bevy
pub(crate) fn sync_buy_window(
    buy_state: Res<BuyVehicleWindowState>,
    sim: Res<SimWorld>,
    trucks: Option<Res<TruckHandles>>,
    mut root_q: Query<(&FloatingWindow, &mut Visibility)>,
    mut title_q: Query<(&FloatingWindowTitleText, &mut Text)>,
    mut road_toolbar_q: Query<
        &mut Node,
        (
            With<BuyVehicleRoadToolbar>,
            Without<BuyVehicleRow>,
            Without<BuyVehiclePreviewImage>,
        ),
    >,
    mut row_q: Query<
        (
            &BuyVehicleRow,
            &Interaction,
            &mut Node,
            &mut BackgroundColor,
        ),
        (
            With<Button>,
            With<BuyVehicleRow>,
            Without<BuyVehiclePreviewImage>,
            Without<BuyVehicleRoadToolbar>,
        ),
    >,
    mut row_text_q: Query<(&BuyVehicleRowText, &mut Text), Without<FloatingWindowTitleText>>,
    mut stats_q: Query<
        &mut Text,
        (
            With<BuyVehicleStatsText>,
            Without<BuyVehicleRowText>,
            Without<FloatingWindowTitleText>,
        ),
    >,
    mut preview_q: Query<
        (&mut ImageNode, &mut Node),
        (
            With<BuyVehiclePreviewImage>,
            Without<BuyVehicleRow>,
            Without<BuyVehicleRoadToolbar>,
            Without<Button>,
        ),
    >,
) {
    let Some((_, mut vis)) = root_q
        .iter_mut()
        .find(|(w, _)| w.id == FloatingWindowId::BuyVehicle)
    else {
        return;
    };
    let Some(depot_pos) = buy_state.depot_pos else {
        *vis = Visibility::Hidden;
        return;
    };
    *vis = Visibility::Visible;
    if let Some((_, mut title)) = title_q
        .iter_mut()
        .find(|(t, _)| t.0 == FloatingWindowId::BuyVehicle)
    {
        **title = buy_window_title(&sim, depot_pos).to_string();
    }
    let hide_road_filters = matches!(
        sim.state.map.get_kind(depot_pos),
        Some(TileKind::RailDepot | TileKind::ShipDepot | TileKind::Airport)
    );
    if let Ok(mut toolbar) = road_toolbar_q.single_mut() {
        toolbar.display = if hide_road_filters {
            Display::None
        } else {
            Display::Flex
        };
    }
    let engines = engines_for_buy_window(&sim, depot_pos, buy_state.sort, buy_state.road_filter);
    for (row, interaction, mut node, mut bg) in &mut row_q {
        let Some(engine) = engines.get(row.slot) else {
            node.display = Display::None;
            continue;
        };
        node.display = Display::Flex;
        let selected = buy_state.selected_engine == Some(engine.id);
        *bg = if selected {
            BackgroundColor(Color::srgb(0.48, 0.41, 0.27))
        } else if *interaction == Interaction::Hovered {
            BackgroundColor(Color::srgb(0.34, 0.29, 0.2))
        } else {
            BackgroundColor(Color::srgb(0.22, 0.18, 0.12))
        };
    }
    for (row_text, mut text) in &mut row_text_q {
        if let Some(engine) = engines.get(row_text.slot) {
            **text = format!("{:<28} ${}", engine.name, engine.price);
        } else {
            **text = String::new();
        }
    }
    if let Ok(mut stats) = stats_q.single_mut() {
        **stats = buy_state
            .selected_engine
            .and_then(openttdrs_core::engine_by_id)
            .map_or_else(
                || "Selecciona un modelo para ver sus características.".to_string(),
                stats_text,
            );
    }
    if let Ok((mut image, mut node)) = preview_q.single_mut() {
        match (
            buy_state
                .selected_engine
                .and_then(openttdrs_core::engine_by_id),
            trucks.as_ref(),
        ) {
            (Some(engine), Some(trucks)) => {
                image.image = preview_sprite_for_engine(trucks, engine);
                node.display = Display::Flex;
            }
            _ => {
                node.display = Display::None;
            }
        }
    }
}

#[allow(clippy::too_many_arguments)] // sistema ECS Bevy
pub(crate) fn handle_buy_window_buttons(
    mut row_q: Query<(&Interaction, &BuyVehicleRow), (Changed<Interaction>, With<Button>)>,
    mut toolbar_q: Query<
        (&Interaction, &BuyVehicleToolbarButton),
        (
            Changed<Interaction>,
            With<Button>,
            Without<BuyVehicleRow>,
            Without<BuyVehicleBuyButton>,
        ),
    >,
    mut buy_q: Query<
        &Interaction,
        (
            Changed<Interaction>,
            With<Button>,
            With<BuyVehicleBuyButton>,
            Without<BuyVehicleRow>,
            Without<BuyVehicleToolbarButton>,
        ),
    >,
    mut buy_state: ResMut<BuyVehicleWindowState>,
    mut sim: ResMut<SimWorld>,
    mut pending: ResMut<RemapMapVisualsPending>,
    mut hud_feedback: ResMut<HudBuildFeedback>,
    time: Res<Time>,
) {
    let Some(depot_pos) = buy_state.depot_pos else {
        return;
    };
    for (interaction, button) in &mut toolbar_q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match button {
            BuyVehicleToolbarButton::SortName => buy_state.sort = EngineCatalogSort::Name,
            BuyVehicleToolbarButton::SortPrice => buy_state.sort = EngineCatalogSort::Price,
            BuyVehicleToolbarButton::SortSpeed => buy_state.sort = EngineCatalogSort::Speed,
            BuyVehicleToolbarButton::SortYear => buy_state.sort = EngineCatalogSort::IntroYear,
            BuyVehicleToolbarButton::FilterAll => buy_state.road_filter = RoadEngineFilter::All,
            BuyVehicleToolbarButton::FilterBus => buy_state.road_filter = RoadEngineFilter::BusOnly,
            BuyVehicleToolbarButton::FilterTruck => {
                buy_state.road_filter = RoadEngineFilter::TruckOnly;
            }
        }
        buy_state.selected_engine = None;
    }
    for (interaction, row) in &mut row_q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        if let Some(engine) =
            engines_for_buy_window(&sim, depot_pos, buy_state.sort, buy_state.road_filter)
                .get(row.slot)
        {
            buy_state.selected_engine = Some(engine.id);
        }
    }
    for interaction in &mut buy_q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let Some(engine_id) = buy_state.selected_engine else {
            continue;
        };
        match apply_command(
            &mut sim.state,
            &Command::BuildVehicleAtDepot(depot_pos, engine_id),
        ) {
            Ok(()) => pending.pending = true,
            Err(e) => push_build_command_error(&mut hud_feedback, e, time.elapsed_secs()),
        }
    }
}

/// Limpia el estado cuando el usuario cierra la ventana con ✕.
pub(crate) fn buy_window_on_closed(
    mut closed: MessageReader<FloatingWindowClosed>,
    mut buy_state: ResMut<BuyVehicleWindowState>,
) {
    for msg in closed.read() {
        if msg.0 == FloatingWindowId::BuyVehicle {
            buy_state.depot_pos = None;
            buy_state.selected_engine = None;
            buy_state.sort = EngineCatalogSort::default();
            buy_state.road_filter = RoadEngineFilter::default();
        }
    }
}
