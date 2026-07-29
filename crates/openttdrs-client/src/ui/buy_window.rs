//! Ventana flotante «Nuevos vehículos» estilo `OpenTTD`.
//!
//! Lista los modelos del catálogo filtrados por tipo de depósito (vía →
//! trenes, carretera → buses/camiones/tranvías), con panel de stats del modelo
//! seleccionado y botón de compra (`BuildVehicleAtDepot`).

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use bevy::text::EditableText;
use bevy::ui::widget::ImageNode;
use openttdrs_core::Command;
use openttdrs_core::prelude::*;
use openttdrs_core::{
    CargoType, DecodedSprite, DepotPurchaseKind, EngineCatalogSort, EngineDef, RoadEngineFilter,
    calendar_year_at_tick, engines_for_depot_kind_in,
};
use std::collections::HashMap;

use crate::render::newgrf_cache::{DecodedSpriteImagePolicy, decoded_sprite_image};
use crate::render::{RemapMapVisualsPending, TruckHandles};
use crate::sprites::CompanyColour;
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
const BTN_ACTIVE: Color = Color::srgb(0.58, 0.50, 0.31);

/// Caché de previews NewGRF (`engine_id`, `company_colour`) → textura RGBA.
#[derive(Resource, Default)]
pub(crate) struct NewGrfTrainPreviewCache {
    handles: HashMap<(u16, u8), Handle<Image>>,
}

fn decoded_sprite_to_image(sprite: &DecodedSprite, company_colour: u8) -> Image {
    decoded_sprite_image(
        sprite,
        DecodedSpriteImagePolicy::Masked {
            colour: CompanyColour::from_u8(company_colour),
        },
    )
}

/// Filtro locomotora/vagón en depósito de vía.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum RailBuyFilter {
    #[default]
    All,
    LocosOnly,
    WagonsOnly,
}

#[derive(Resource, Default)]
pub(crate) struct BuyVehicleWindowState {
    /// Depósito desde el que se abrió la ventana (`None` = cerrada).
    pub(crate) depot_pos: Option<TileCoord>,
    pub(crate) selected_engine: Option<u16>,
    pub(crate) sort: EngineCatalogSort,
    pub(crate) road_filter: RoadEngineFilter,
    pub(crate) rail_filter: RailBuyFilter,
    pub(crate) name_filter: String,
}

#[derive(Component, Clone, Copy)]
pub(crate) struct BuyVehicleRow {
    slot: usize,
}

#[derive(Component, Clone, Copy)]
pub(crate) struct BuyVehicleRowText {
    slot: usize,
}

#[derive(Component, Clone, Copy)]
pub(crate) struct BuyVehicleRowSprite {
    slot: usize,
}

const ROW_SPRITE_W: f32 = 40.0;
const ROW_SPRITE_H: f32 = 24.0;
const PLACEHOLDER_SPRITE: &str = "assets/opengfx/tiles/vehicle_train_e.png";

#[derive(Component)]
pub(crate) struct BuyVehicleStatsText;

#[derive(Component)]
pub(crate) struct BuyVehiclePreviewImage;

#[derive(Component)]
pub(crate) struct BuyVehicleBuyButton;

#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BuyVehicleToolbarButton {
    SortName,
    SortPrice,
    SortSpeed,
    SortYear,
    FilterAll,
    FilterBus,
    FilterTruck,
    FilterTram,
    FilterLocos,
    FilterWagons,
}

#[derive(Component)]
pub(crate) struct BuyVehicleSortToolbar;

#[derive(Component)]
pub(crate) struct BuyVehicleRoadToolbar;

#[derive(Component)]
pub(crate) struct BuyVehicleRailToolbar;

#[derive(Component)]
pub(crate) struct BuyVehicleSearchInput;

pub(crate) fn setup_buy_window(mut commands: Commands, asset_server: Res<AssetServer>) {
    let asset_server = &*asset_server;
    let (_root, content) = spawn_floating_window(
        &mut commands,
        asset_server,
        FloatingWindowId::BuyVehicle,
        "Nuevos vehículos",
        TITLE_CRIMSON,
        Vec2::new(120.0, 100.0),
        450.0,
    );
    commands.entity(content).with_children(|panel| {
        panel.spawn((
            BuyVehicleSearchInput,
            EditableText::new(""),
            Text::new("buscar…"),
            window_text_font(asset_server, UiFontRole::Caption),
            TextColor(Color::srgb(0.75, 0.72, 0.62)),
            Node {
                width: Val::Percent(100.0),
                min_height: Val::Px(22.0),
                padding: UiRect::horizontal(Val::Px(6.0)),
                margin: UiRect::bottom(Val::Px(4.0)),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(Color::srgb(0.18, 0.15, 0.10)),
            BorderColor::all(BTN_BORDER),
            BuildMenuUi,
        ));
        panel
            .spawn((
                BuyVehicleSortToolbar,
                Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(3.0),
                    flex_wrap: FlexWrap::Wrap,
                    row_gap: Val::Px(2.0),
                    margin: UiRect::bottom(Val::Px(2.0)),
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
            });
        panel
            .spawn((
                BuyVehicleRoadToolbar,
                Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(3.0),
                    flex_wrap: FlexWrap::Wrap,
                    row_gap: Val::Px(2.0),
                    display: Display::None,
                    margin: UiRect::bottom(Val::Px(2.0)),
                    ..default()
                },
                BuildMenuUi,
            ))
            .with_children(|row| {
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
                spawn_toolbar_button(
                    row,
                    asset_server,
                    BuyVehicleToolbarButton::FilterTram,
                    "Tranvías",
                );
            });
        panel
            .spawn((
                BuyVehicleRailToolbar,
                Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(3.0),
                    flex_wrap: FlexWrap::Wrap,
                    row_gap: Val::Px(2.0),
                    display: Display::None,
                    margin: UiRect::bottom(Val::Px(2.0)),
                    ..default()
                },
                BuildMenuUi,
            ))
            .with_children(|row| {
                spawn_toolbar_button(
                    row,
                    asset_server,
                    BuyVehicleToolbarButton::FilterAll,
                    "Todos",
                );
                spawn_toolbar_button(
                    row,
                    asset_server,
                    BuyVehicleToolbarButton::FilterLocos,
                    "Locomotoras",
                );
                spawn_toolbar_button(
                    row,
                    asset_server,
                    BuyVehicleToolbarButton::FilterWagons,
                    "Vagones",
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
                            height: Val::Px(28.0),
                            padding: UiRect::horizontal(Val::Px(4.0)),
                            column_gap: Val::Px(6.0),
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
                    ))
                    .with_children(|row| {
                        row.spawn((
                            BuyVehicleRowSprite { slot },
                            ImageNode::new(asset_server.load::<Image>(PLACEHOLDER_SPRITE)),
                            Node {
                                width: Val::Px(ROW_SPRITE_W),
                                height: Val::Px(ROW_SPRITE_H),
                                flex_shrink: 0.0,
                                ..default()
                            },
                        ));
                        row.spawn((
                            BuyVehicleRowText { slot },
                            Text::new(""),
                            window_text_font(asset_server, UiFontRole::Caption),
                            TextColor(Color::srgb(0.92, 0.88, 0.72)),
                        ));
                    });
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

fn depot_kind_at(sim: &SimWorld, depot_pos: TileCoord) -> DepotPurchaseKind {
    match sim.state.map.get_kind(depot_pos) {
        Some(TileKind::RailDepot) => DepotPurchaseKind::Rail,
        Some(TileKind::ShipDepot) => DepotPurchaseKind::Ship,
        Some(TileKind::Airport) => DepotPurchaseKind::Aircraft,
        _ => DepotPurchaseKind::Road,
    }
}

/// Modelos visibles según depósito, año, filtros y orden.
pub(crate) fn engines_for_buy_window<'a>(
    sim: &'a SimWorld,
    depot_pos: TileCoord,
    sort: EngineCatalogSort,
    road_filter: RoadEngineFilter,
    rail_filter: RailBuyFilter,
    name_filter: &str,
) -> Vec<&'a EngineDef> {
    let depot_kind = depot_kind_at(sim, depot_pos);
    let year = calendar_year_at_tick(sim.state.tick);
    let mut engines = engines_for_depot_kind_in(
        &sim.state.engine_catalog,
        depot_kind,
        year,
        sort,
        road_filter,
    );
    if depot_kind == DepotPurchaseKind::Aircraft {
        let heliport = openttdrs_core::airport_tile_is_heliport(&sim.state.map, depot_pos);
        engines.retain(|e| openttdrs_core::aircraft_is_helicopter(e.id) == heliport);
    }
    if depot_kind == DepotPurchaseKind::Rail {
        match rail_filter {
            RailBuyFilter::All => {}
            RailBuyFilter::LocosOnly => engines.retain(|e| e.is_train_engine()),
            RailBuyFilter::WagonsOnly => engines.retain(|e| e.is_wagon()),
        }
    }
    let needle = name_filter.trim().to_lowercase();
    if !needle.is_empty() {
        engines.retain(|e| e.name.to_lowercase().contains(&needle));
    }
    engines
}

fn cargo_label(cargo: Option<CargoType>) -> &'static str {
    match cargo {
        Some(c) => c.display_name(),
        None => "nada (solo locomotora)",
    }
}

fn stats_text(engine: &EngineDef) -> String {
    let role = if engine.is_wagon() {
        "Tipo: vagón (enganchar a locomotora)\n"
    } else if engine.kind == VehicleKind::Train {
        "Tipo: locomotora / DMU\n"
    } else {
        ""
    };
    let newgrf = if engine.from_newgrf {
        if engine.newgrf_preview().is_some() {
            "NewGRF: preview Action1/3\n"
        } else {
            "NewGRF: metadatos; sin sprites\n"
        }
    } else {
        ""
    };
    format!(
        "{role}{newgrf}Precio: ${}  Peso: {}t\nVelocidad: {}km/h  Potencia: {}cv\nCoste de operación: ${}/año\nCapacidad: {} {}\nDiseñado: {}  Fiabilidad: {}%",
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

fn preview_sprite_for_engine(
    trucks: &TruckHandles,
    engine: &EngineDef,
    cache: &mut NewGrfTrainPreviewCache,
    images: &mut Assets<Image>,
    company_colour: u8,
) -> Handle<Image> {
    if let Some(decoded) = engine.newgrf_preview() {
        return cache
            .handles
            .entry((engine.id, company_colour))
            .or_insert_with(|| images.add(decoded_sprite_to_image(decoded, company_colour)))
            .clone();
    }
    if engine.kind == VehicleKind::Train {
        trucks.train_preview(engine.train_image_index, 2)
    } else {
        trucks.intro_sprite_for_engine(engine, 2)
    }
}

fn toolbar_button_active(state: &BuyVehicleWindowState, button: BuyVehicleToolbarButton) -> bool {
    match button {
        BuyVehicleToolbarButton::SortName => state.sort == EngineCatalogSort::Name,
        BuyVehicleToolbarButton::SortPrice => state.sort == EngineCatalogSort::Price,
        BuyVehicleToolbarButton::SortSpeed => state.sort == EngineCatalogSort::Speed,
        BuyVehicleToolbarButton::SortYear => state.sort == EngineCatalogSort::IntroYear,
        BuyVehicleToolbarButton::FilterAll => {
            state.road_filter == RoadEngineFilter::All && state.rail_filter == RailBuyFilter::All
        }
        BuyVehicleToolbarButton::FilterBus => state.road_filter == RoadEngineFilter::BusOnly,
        BuyVehicleToolbarButton::FilterTruck => state.road_filter == RoadEngineFilter::TruckOnly,
        BuyVehicleToolbarButton::FilterTram => state.road_filter == RoadEngineFilter::TramOnly,
        BuyVehicleToolbarButton::FilterLocos => state.rail_filter == RailBuyFilter::LocosOnly,
        BuyVehicleToolbarButton::FilterWagons => state.rail_filter == RailBuyFilter::WagonsOnly,
    }
}

#[allow(clippy::too_many_arguments)] // sistema ECS Bevy
pub(crate) fn sync_buy_window(
    buy_state: Res<BuyVehicleWindowState>,
    sim: Res<SimWorld>,
    trucks: Option<Res<TruckHandles>>,
    mut preview_cache: ResMut<NewGrfTrainPreviewCache>,
    mut images: ResMut<Assets<Image>>,
    mut root_q: Query<(&FloatingWindow, &mut Visibility)>,
    mut title_q: Query<(&FloatingWindowTitleText, &mut Text)>,
    mut road_toolbar_q: Query<
        &mut Node,
        (
            With<BuyVehicleRoadToolbar>,
            Without<BuyVehicleRailToolbar>,
            Without<BuyVehicleRow>,
            Without<BuyVehiclePreviewImage>,
        ),
    >,
    mut rail_toolbar_q: Query<
        &mut Node,
        (
            With<BuyVehicleRailToolbar>,
            Without<BuyVehicleRoadToolbar>,
            Without<BuyVehicleRow>,
            Without<BuyVehiclePreviewImage>,
        ),
    >,
    mut toolbar_btn_q: Query<
        (&BuyVehicleToolbarButton, &Interaction, &mut BackgroundColor),
        (With<Button>, Without<BuyVehicleRow>),
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
            Without<BuyVehicleRailToolbar>,
        ),
    >,
    mut row_text_q: Query<(&BuyVehicleRowText, &mut Text), Without<FloatingWindowTitleText>>,
    mut row_sprite_q: Query<
        (&BuyVehicleRowSprite, &mut ImageNode),
        (Without<BuyVehiclePreviewImage>, Without<Button>),
    >,
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
            Without<BuyVehicleRowSprite>,
            Without<BuyVehicleRoadToolbar>,
            Without<BuyVehicleRailToolbar>,
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
    let kind = depot_kind_at(&sim, depot_pos);
    if let Ok(mut toolbar) = road_toolbar_q.single_mut() {
        toolbar.display = if kind == DepotPurchaseKind::Road {
            Display::Flex
        } else {
            Display::None
        };
    }
    if let Ok(mut toolbar) = rail_toolbar_q.single_mut() {
        toolbar.display = if kind == DepotPurchaseKind::Rail {
            Display::Flex
        } else {
            Display::None
        };
    }
    for (button, interaction, mut bg) in &mut toolbar_btn_q {
        *bg = if toolbar_button_active(&buy_state, *button) {
            BackgroundColor(BTN_ACTIVE)
        } else if *interaction == Interaction::Hovered {
            BackgroundColor(Color::srgb(0.47, 0.41, 0.28))
        } else {
            BackgroundColor(BTN_BG)
        };
    }
    let engines = engines_for_buy_window(
        &sim,
        depot_pos,
        buy_state.sort,
        buy_state.road_filter,
        buy_state.rail_filter,
        &buy_state.name_filter,
    );
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
            **text = if engine.from_newgrf {
                let tag = if engine.newgrf_preview().is_some() {
                    "gfx"
                } else {
                    "meta"
                };
                format!("{} · ${} · {tag}", engine.name, engine.price)
            } else {
                format!("{} · ${}", engine.name, engine.price)
            };
        } else {
            **text = String::new();
        }
    }
    if let Some(trucks) = trucks.as_ref() {
        for (sprite, mut image) in &mut row_sprite_q {
            if let Some(engine) = engines.get(sprite.slot) {
                image.image = preview_sprite_for_engine(
                    trucks,
                    engine,
                    &mut preview_cache,
                    &mut images,
                    sim.state.company_colour,
                );
            }
        }
    }
    if let Ok(mut stats) = stats_q.single_mut() {
        **stats = buy_state
            .selected_engine
            .and_then(|id| {
                openttdrs_core::engine_in_catalog(&sim.state.engine_catalog, id)
                    .or_else(|| openttdrs_core::engine_by_id(id))
            })
            .map_or_else(
                || "Selecciona un modelo para ver sus características.".to_string(),
                stats_text,
            );
    }
    if let Ok((mut image, mut node)) = preview_q.single_mut() {
        let engine = buy_state.selected_engine.and_then(|id| {
            openttdrs_core::engine_in_catalog(&sim.state.engine_catalog, id)
                .or_else(|| openttdrs_core::engine_by_id(id))
        });
        match (engine, trucks.as_ref()) {
            (Some(engine), Some(trucks)) => {
                image.image = preview_sprite_for_engine(
                    trucks,
                    engine,
                    &mut preview_cache,
                    &mut images,
                    sim.state.company_colour,
                );
                node.display = Display::Flex;
            }
            _ => {
                node.display = Display::None;
            }
        }
    }
}

pub(crate) fn buy_window_search_keyboard(
    mut key_events: MessageReader<KeyboardInput>,
    mut buy_state: ResMut<BuyVehicleWindowState>,
    mut inputs: Query<(&mut EditableText, &mut Text), With<BuyVehicleSearchInput>>,
) {
    if buy_state.depot_pos.is_none() {
        key_events.clear();
        return;
    }
    let Ok((mut editable, mut text)) = inputs.single_mut() else {
        key_events.clear();
        return;
    };
    for ev in key_events.read() {
        if ev.state != ButtonState::Pressed {
            continue;
        }
        if matches!(ev.logical_key, Key::Backspace) {
            editable.queue_edit(bevy::text::TextEdit::Backspace);
            continue;
        }
        if matches!(ev.logical_key, Key::Delete) {
            editable.queue_edit(bevy::text::TextEdit::Delete);
            continue;
        }
        let Some(typed) = &ev.text else {
            continue;
        };
        for c in typed.chars() {
            if !c.is_control() && editable.value().chars().count() < 24 {
                editable.queue_edit(bevy::text::TextEdit::Insert(
                    winit::keyboard::SmolStr::from(c.to_string()),
                ));
            }
        }
    }
    buy_state.name_filter = editable.value().to_string();
    if buy_state.name_filter.is_empty() {
        **text = "buscar…".into();
    } else {
        **text = buy_state.name_filter.clone();
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
    let kind = depot_kind_at(&sim, depot_pos);
    for (interaction, button) in &mut toolbar_q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match button {
            BuyVehicleToolbarButton::SortName => buy_state.sort = EngineCatalogSort::Name,
            BuyVehicleToolbarButton::SortPrice => buy_state.sort = EngineCatalogSort::Price,
            BuyVehicleToolbarButton::SortSpeed => buy_state.sort = EngineCatalogSort::Speed,
            BuyVehicleToolbarButton::SortYear => buy_state.sort = EngineCatalogSort::IntroYear,
            BuyVehicleToolbarButton::FilterAll => {
                buy_state.road_filter = RoadEngineFilter::All;
                buy_state.rail_filter = RailBuyFilter::All;
            }
            BuyVehicleToolbarButton::FilterBus => {
                buy_state.road_filter = RoadEngineFilter::BusOnly;
            }
            BuyVehicleToolbarButton::FilterTruck => {
                buy_state.road_filter = RoadEngineFilter::TruckOnly;
            }
            BuyVehicleToolbarButton::FilterTram => {
                buy_state.road_filter = RoadEngineFilter::TramOnly;
            }
            BuyVehicleToolbarButton::FilterLocos => {
                buy_state.rail_filter = RailBuyFilter::LocosOnly;
            }
            BuyVehicleToolbarButton::FilterWagons => {
                buy_state.rail_filter = RailBuyFilter::WagonsOnly;
            }
        }
        // Evitar aplicar filtros de carretera en depósito de vía y viceversa.
        if kind == DepotPurchaseKind::Rail {
            buy_state.road_filter = RoadEngineFilter::All;
        } else if kind == DepotPurchaseKind::Road {
            buy_state.rail_filter = RailBuyFilter::All;
        }
        buy_state.selected_engine = None;
    }
    for (interaction, row) in &mut row_q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        if let Some(engine) = engines_for_buy_window(
            &sim,
            depot_pos,
            buy_state.sort,
            buy_state.road_filter,
            buy_state.rail_filter,
            &buy_state.name_filter,
        )
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
        match crate::network::apply_player_command(
            &mut sim.state,
            &Command::BuildVehicleAtDepot(depot_pos, engine_id),
        ) {
            Ok(()) => {
                // Vagón recién comprado: enganchar a la primera locomotora del depósito.
                if let Some(engine) = openttdrs_core::engine_by_id(engine_id)
                    && engine.is_wagon()
                {
                    let wagon_id = sim.state.vehicles.iter().map(|v| v.id).max();
                    let head_id = sim
                        .state
                        .vehicles
                        .iter()
                        .find(|v| {
                            v.pos == depot_pos
                                && v.kind == VehicleKind::Train
                                && v.is_consist_head()
                                && !openttdrs_core::engine_by_id(v.engine_id.unwrap_or(0))
                                    .is_some_and(openttdrs_core::EngineDef::is_wagon)
                        })
                        .map(|v| v.id);
                    if let (Some(wagon_id), Some(head_id)) = (wagon_id, head_id)
                        && head_id != wagon_id
                    {
                        let _ = crate::network::apply_player_command(
                            &mut sim.state,
                            &Command::AttachWagonToConsist { head_id, wagon_id },
                        );
                    }
                }
                pending.pending = true;
            }
            Err(e) => push_build_command_error(&mut hud_feedback, e, time.elapsed_secs()),
        }
    }
}

/// Limpia el estado cuando el usuario cierra la ventana con ✕.
pub(crate) fn buy_window_on_closed(
    mut closed: MessageReader<FloatingWindowClosed>,
    mut buy_state: ResMut<BuyVehicleWindowState>,
    mut search_q: Query<(&mut EditableText, &mut Text), With<BuyVehicleSearchInput>>,
) {
    for msg in closed.read() {
        if msg.0.class == FloatingWindowId::BuyVehicle {
            buy_state.depot_pos = None;
            buy_state.selected_engine = None;
            buy_state.sort = EngineCatalogSort::default();
            buy_state.road_filter = RoadEngineFilter::default();
            buy_state.rail_filter = RailBuyFilter::default();
            buy_state.name_filter.clear();
            if let Ok((mut editable, mut text)) = search_q.single_mut() {
                *editable = EditableText::new("");
                **text = "buscar…".into();
            }
        }
    }
}
