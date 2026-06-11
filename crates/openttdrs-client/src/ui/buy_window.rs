//! Ventana flotante «Nuevos vehículos» estilo `OpenTTD`.
//!
//! Lista los modelos del catálogo filtrados por tipo de depósito (vía →
//! trenes, carretera → buses/camiones), con panel de stats del modelo
//! seleccionado y botón de compra (`BuildVehicleAtDepot`).

use bevy::prelude::*;
use openttdrs_core::{
    CargoType, Command, EngineDef, TileCoord, TileKind, VehicleKind, apply_command, engines_of_kind,
};

use crate::render::RemapMapVisualsPending;
use crate::state::SimWorld;
use crate::ui::floating_window::{
    FloatingWindow, FloatingWindowClosed, FloatingWindowId, FloatingWindowTitleText, TITLE_CRIMSON,
    WINDOW_TEXT, spawn_floating_window, window_text_font,
};
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
pub(crate) struct BuyVehicleBuyButton;

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
                            window_text_font(asset_server, 11.0),
                            TextColor(Color::srgb(0.92, 0.88, 0.72)),
                        )],
                    ));
                }
            });
        panel.spawn((
            BuyVehicleStatsText,
            Text::new(""),
            window_text_font(asset_server, 11.0),
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
                window_text_font(asset_server, 11.0),
                TextColor(Color::srgb(0.92, 0.88, 0.72)),
            )],
        ));
    });
}

/// Modelos disponibles según el tipo de depósito.
pub(crate) fn engines_for_depot(sim: &SimWorld, depot_pos: TileCoord) -> Vec<&'static EngineDef> {
    match sim.state.map.get_kind(depot_pos) {
        Some(TileKind::RailDepot) => engines_of_kind(VehicleKind::Train).collect(),
        Some(TileKind::RoadDepot) => engines_of_kind(VehicleKind::Bus)
            .chain(engines_of_kind(VehicleKind::Truck))
            .collect(),
        _ => Vec::new(),
    }
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
        _ => "Nuevos vehículos de carretera",
    }
}

#[allow(clippy::type_complexity)] // sistema ECS Bevy
pub(crate) fn sync_buy_window(
    buy_state: Res<BuyVehicleWindowState>,
    sim: Res<SimWorld>,
    mut root_q: Query<(&FloatingWindow, &mut Visibility)>,
    mut title_q: Query<(&FloatingWindowTitleText, &mut Text)>,
    mut row_q: Query<
        (
            &BuyVehicleRow,
            &Interaction,
            &mut Node,
            &mut BackgroundColor,
        ),
        With<Button>,
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
    let engines = engines_for_depot(&sim, depot_pos);
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
}

#[allow(clippy::too_many_arguments)] // sistema ECS Bevy
pub(crate) fn handle_buy_window_buttons(
    mut row_q: Query<(&Interaction, &BuyVehicleRow), (Changed<Interaction>, With<Button>)>,
    mut buy_q: Query<
        &Interaction,
        (
            Changed<Interaction>,
            With<Button>,
            With<BuyVehicleBuyButton>,
            Without<BuyVehicleRow>,
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
    for (interaction, row) in &mut row_q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        if let Some(engine) = engines_for_depot(&sim, depot_pos).get(row.slot) {
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
        }
    }
}
