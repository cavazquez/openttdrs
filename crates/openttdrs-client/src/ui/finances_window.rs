//! Ventana de finanzas de la compañía (clic en dinero de la barra inferior).

use bevy::prelude::*;
use openttdrs_core::{Command, LOAN_INTERVAL, apply_command, format_money};

use crate::state::SimWorld;
use crate::ui::floating_window::{
    FloatingWindow, FloatingWindowClosed, FloatingWindowId, FloatingWindowTitleText, TITLE_BROWN,
    WINDOW_TEXT, spawn_floating_window, window_text_font,
};
use crate::ui::font::UiFontRole;
use crate::ui::hud::{HudBuildFeedback, push_build_command_error};
use crate::ui::toolbar::BuildMenuUi;

#[derive(Resource, Default)]
pub(crate) struct FinancesWindowState {
    pub(crate) open: bool,
}

#[derive(Component)]
pub(crate) struct FinancesWindowBodyText;

#[derive(Component, Clone, Copy)]
pub(crate) enum FinancesWindowButton {
    IncreaseLoan,
    DecreaseLoan,
    BuyRival,
    OpenAiSettings,
}

#[derive(Default)]
pub(crate) struct FinancesSyncCache {
    snapshot: Option<FinancesSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CompanyFinanceRow {
    name: String,
    is_ai: bool,
    colour: u8,
    money: i64,
    cargo_income: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FinancesSnapshot {
    money: i64,
    loan: i64,
    max_loan: i64,
    cargo_income: u64,
    running_costs: u64,
    deliveries: u64,
    units_delivered: u64,
    vehicles: usize,
    stations: usize,
    rail_tiles: u32,
    road_tiles: u32,
    companies: Vec<CompanyFinanceRow>,
}

pub(crate) fn setup_finances_window(mut commands: Commands, asset_server: Res<AssetServer>) {
    let asset_server = &*asset_server;
    let (_root, content) = spawn_floating_window(
        &mut commands,
        asset_server,
        FloatingWindowId::Finances,
        "Finanzas",
        TITLE_BROWN,
        Vec2::new(720.0, 200.0),
        320.0,
    );
    commands.entity(content).with_children(|body| {
        body.spawn((
            FinancesWindowBodyText,
            Text::new(""),
            window_text_font(asset_server, UiFontRole::Caption),
            TextColor(WINDOW_TEXT),
        ));
        body.spawn((Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(8.0),
            margin: UiRect::top(Val::Px(8.0)),
            ..default()
        },))
            .with_children(|row| {
                for (label, button) in [
                    ("Pedir préstamo", FinancesWindowButton::IncreaseLoan),
                    ("Devolver préstamo", FinancesWindowButton::DecreaseLoan),
                    ("Comprar rival (quiebra)", FinancesWindowButton::BuyRival),
                    ("IA…", FinancesWindowButton::OpenAiSettings),
                ] {
                    row.spawn((
                        Button,
                        button,
                        Node {
                            flex_grow: 1.0,
                            height: Val::Px(24.0),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            border: UiRect::all(Val::Px(1.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.36, 0.31, 0.21)),
                        BorderColor::all(Color::srgb(0.66, 0.58, 0.38)),
                        Interaction::default(),
                        BuildMenuUi,
                        children![(
                            Text::new(label),
                            window_text_font(asset_server, UiFontRole::Caption),
                            TextColor(WINDOW_TEXT),
                        )],
                    ));
                }
            });
    });
}

pub(crate) fn open_finances_from_routes(
    mut routes: MessageReader<crate::ui::navigation::OpenUiRoute>,
    mut finances: ResMut<FinancesWindowState>,
) {
    for route in routes.read() {
        if matches!(route.0, crate::ui::navigation::UiRoute::Finances) {
            finances.open = true;
        }
    }
}

pub(crate) fn handle_open_finances_window(
    mut finances: ResMut<FinancesWindowState>,
    interaction_q: Query<
        &Interaction,
        (
            Changed<Interaction>,
            With<crate::ui::statusbar::StatusBarMoneyButton>,
        ),
    >,
) {
    for interaction in &interaction_q {
        if *interaction == Interaction::Pressed {
            finances.open = true;
        }
    }
}

pub(crate) fn handle_finances_window_buttons(
    buttons: Query<(&Interaction, &FinancesWindowButton), (Changed<Interaction>, With<Button>)>,
    mut sim: ResMut<SimWorld>,
    mut hud_feedback: ResMut<HudBuildFeedback>,
    mut ai_settings: ResMut<crate::ui::ai_settings_window::AiSettingsWindowState>,
    time: Res<Time>,
) {
    for (interaction, button) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let cmd = match button {
            FinancesWindowButton::OpenAiSettings => {
                ai_settings.open = true;
                continue;
            }
            FinancesWindowButton::IncreaseLoan => Command::IncreaseLoan,
            FinancesWindowButton::DecreaseLoan => Command::DecreaseLoan,
            FinancesWindowButton::BuyRival => {
                let Some(rival) = sim
                    .state
                    .companies
                    .iter()
                    .find(|c| c.id != sim.state.active_company)
                    .map(|c| c.id)
                else {
                    continue;
                };
                Command::BuyCompany(rival)
            }
        };
        if let Err(e) = apply_command(&mut sim.state, &cmd) {
            push_build_command_error(&mut hud_feedback, e, time.elapsed_secs());
        }
    }
}

pub(crate) fn sync_finances_window(
    finances: Res<FinancesWindowState>,
    sim: Res<SimWorld>,
    mut root_q: Query<(&FloatingWindow, &mut Visibility)>,
    mut title_q: Query<(&FloatingWindowTitleText, &mut Text)>,
    mut body_q: Query<
        &mut Text,
        (
            With<FinancesWindowBodyText>,
            Without<FloatingWindowTitleText>,
        ),
    >,
    mut cache: Local<FinancesSyncCache>,
) {
    let Some((_, mut vis)) = root_q
        .iter_mut()
        .find(|(w, _)| w.id == FloatingWindowId::Finances)
    else {
        return;
    };
    if !finances.open {
        *vis = Visibility::Hidden;
        cache.snapshot = None;
        return;
    }
    *vis = Visibility::Visible;

    let company = sim
        .state
        .companies
        .iter()
        .find(|c| c.id == sim.state.active_company);
    let cargo_income = company
        .map(|c| c.cargo_income_earned)
        .unwrap_or(sim.state.stats.cargo_income_earned);
    let running_costs = company
        .map(|c| c.vehicle_running_costs)
        .unwrap_or(sim.state.stats.vehicle_running_costs);
    let vehicles = sim
        .state
        .vehicles
        .iter()
        .filter(|v| v.is_consist_head() && v.owner == sim.state.active_company)
        .count();
    let stations = sim
        .state
        .stations
        .iter()
        .filter(|s| s.owner == sim.state.active_company)
        .count();
    let companies: Vec<CompanyFinanceRow> = sim
        .state
        .companies
        .iter()
        .map(|c| CompanyFinanceRow {
            name: c.name.clone(),
            is_ai: c.is_ai,
            colour: c.colour,
            money: c.economy.money,
            cargo_income: c.cargo_income_earned,
        })
        .collect();
    let soft = FinancesSnapshot {
        money: sim.state.economy.money,
        loan: sim.state.economy.loan,
        max_loan: sim.state.economy.max_loan,
        cargo_income,
        running_costs,
        deliveries: company
            .map(|c| c.cargo_deliveries)
            .unwrap_or(sim.state.stats.cargo_deliveries),
        units_delivered: sim.state.stats.cargo_units_delivered,
        vehicles,
        stations,
        rail_tiles: cache.snapshot.as_ref().map_or(0, |s| s.rail_tiles),
        road_tiles: cache.snapshot.as_ref().map_or(0, |s| s.road_tiles),
        companies,
    };
    let need_infra = cache.snapshot.as_ref().is_none_or(|prev| {
        prev.money != soft.money
            || prev.loan != soft.loan
            || prev.cargo_income != soft.cargo_income
            || prev.running_costs != soft.running_costs
            || prev.deliveries != soft.deliveries
            || prev.vehicles != soft.vehicles
            || prev.stations != soft.stations
            || prev.companies != soft.companies
    });
    let (rail_tiles, road_tiles) = if need_infra {
        let (mw, mh) = sim.state.map.dimensions();
        let mut rail_tiles = 0_u32;
        let mut road_tiles = 0_u32;
        for y in 0..mh {
            for x in 0..mw {
                match sim
                    .state
                    .map
                    .get_kind(openttdrs_core::TileCoord::new(x as i32, y as i32))
                {
                    Some(openttdrs_core::TileKind::Rail)
                    | Some(openttdrs_core::TileKind::RailDepot)
                    | Some(openttdrs_core::TileKind::RailBridge)
                    | Some(openttdrs_core::TileKind::RailTunnel) => rail_tiles += 1,
                    Some(openttdrs_core::TileKind::Road)
                    | Some(openttdrs_core::TileKind::RoadDepot)
                    | Some(openttdrs_core::TileKind::RoadBridge)
                    | Some(openttdrs_core::TileKind::RoadTunnel) => road_tiles += 1,
                    _ => {}
                }
            }
        }
        (rail_tiles, road_tiles)
    } else {
        (soft.rail_tiles, soft.road_tiles)
    };
    let snapshot = FinancesSnapshot {
        rail_tiles,
        road_tiles,
        ..soft
    };
    if cache.snapshot.as_ref() == Some(&snapshot) {
        return;
    }
    cache.snapshot = Some(snapshot.clone());

    if let Some((_, mut title)) = title_q
        .iter_mut()
        .find(|(t, _)| t.0 == FloatingWindowId::Finances)
    {
        let name = sim
            .state
            .companies
            .iter()
            .find(|c| c.id == sim.state.active_company)
            .map(|c| c.name.as_str())
            .unwrap_or(crate::ui::statusbar::COMPANY_DISPLAY_NAME);
        **title = name.to_string();
    }
    if let Ok(mut body) = body_q.single_mut() {
        let net = snapshot.money.saturating_sub(snapshot.loan);
        let profit = snapshot.cargo_income as i64 - snapshot.running_costs as i64;
        let mut companies_block = String::from("\n\nCompañías:");
        for row in &snapshot.companies {
            let tag = if row.is_ai { " (IA)" } else { "" };
            companies_block.push_str(&format!(
                "\n  {}{} · color #{} · {} · ingresos {}",
                row.name,
                tag,
                row.colour,
                format_money(row.money),
                format_money(row.cargo_income.cast_signed()),
            ));
        }
        **body = format!(
            "Efectivo: {}\nPréstamo: {} / {}\nPatrimonio neto: {}\n\
             (cada operación: {})\n\n\
             Ingresos por transporte: {}\nCostes de explotación: {}\n\
             Beneficio operativo: {}\n\
             Entregas: {} ({} unidades)\n\n\
             Infraestructura:\n\
               Vehículos: {}\n\
               Estaciones: {}\n\
               Vía: {} teselas · Carretera: {} teselas{}",
            format_money(snapshot.money),
            format_money(snapshot.loan),
            format_money(snapshot.max_loan),
            format_money(net),
            format_money(LOAN_INTERVAL),
            format_money(snapshot.cargo_income.cast_signed()),
            format_money(snapshot.running_costs.cast_signed()),
            format_money(profit),
            snapshot.deliveries,
            snapshot.units_delivered,
            snapshot.vehicles,
            snapshot.stations,
            snapshot.rail_tiles,
            snapshot.road_tiles,
            companies_block,
        );
    }
}

pub(crate) fn finances_window_on_closed(
    mut closed: MessageReader<FloatingWindowClosed>,
    mut finances: ResMut<FinancesWindowState>,
) {
    for msg in closed.read() {
        if msg.0 == FloatingWindowId::Finances {
            finances.open = false;
        }
    }
}
