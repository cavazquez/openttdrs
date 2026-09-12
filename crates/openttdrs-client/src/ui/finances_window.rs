//! Ventana de finanzas de la compañía (clic en dinero de la barra inferior).

use bevy::prelude::*;
use openttdrs_core::Command;
use openttdrs_core::{
    CompanyId, LOAN_INTERVAL, RailInfrastructureSummary, RoadInfrastructureSummary, RoadTramType,
    StationInfrastructureSummary, WaterInfrastructureSummary, format_money,
    rail_infrastructure_for_company, road_infrastructure_for_company_with_stations,
    station_infrastructure_for_company, water_infrastructure_for_company,
};

use crate::i18n::{Locale, localized_text};
use crate::settings::ClientPreferences;
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

#[derive(Component, Clone, Copy)]
pub(crate) struct FinancesWindowButtonText(pub(crate) FinancesWindowButton);

#[derive(Default)]
pub(crate) struct FinancesSyncCache {
    snapshot: Option<FinancesSnapshot>,
    locale: Option<Locale>,
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
    active_company: CompanyId,
    map_revision: u64,
    money: i64,
    loan: i64,
    max_loan: i64,
    cargo_income: u64,
    running_costs: u64,
    deliveries: u64,
    units_delivered: u64,
    vehicles: usize,
    stations: usize,
    rail_infrastructure: RailInfrastructureSummary,
    road_infrastructure: RoadInfrastructureSummary,
    water_infrastructure: WaterInfrastructureSummary,
    station_infrastructure: StationInfrastructureSummary,
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
                for button in [
                    FinancesWindowButton::IncreaseLoan,
                    FinancesWindowButton::DecreaseLoan,
                    FinancesWindowButton::BuyRival,
                    FinancesWindowButton::OpenAiSettings,
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
                            FinancesWindowButtonText(button),
                            Text::new(""),
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
        if let Err(e) = crate::network::apply_player_command(&mut sim.state, &cmd) {
            push_build_command_error(&mut hud_feedback, e, time.elapsed_secs());
        }
    }
}

fn finances_button_label(locale: Locale, button: FinancesWindowButton) -> String {
    localized_text(
        locale,
        match button {
            FinancesWindowButton::IncreaseLoan => "Pedir préstamo",
            FinancesWindowButton::DecreaseLoan => "Devolver préstamo",
            FinancesWindowButton::BuyRival => "Comprar rival (quiebra)",
            FinancesWindowButton::OpenAiSettings => "IA…",
        },
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn sync_finances_window(
    finances: Res<FinancesWindowState>,
    sim: Res<SimWorld>,
    prefs: Res<ClientPreferences>,
    mut root_q: Query<(&FloatingWindow, &mut Visibility)>,
    mut title_q: Query<(&FloatingWindowTitleText, &mut Text), Without<FinancesWindowBodyText>>,
    mut body_q: Query<
        &mut Text,
        (
            With<FinancesWindowBodyText>,
            Without<FloatingWindowTitleText>,
            Without<FinancesWindowButtonText>,
        ),
    >,
    mut button_text_q: Query<
        (&FinancesWindowButtonText, &mut Text),
        (
            Without<FloatingWindowTitleText>,
            Without<FinancesWindowBodyText>,
        ),
    >,
    mut cache: Local<FinancesSyncCache>,
) {
    let locale = prefs.locale();
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
    for (button, mut text) in &mut button_text_q {
        **text = finances_button_label(locale, button.0);
    }

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
        active_company: sim.state.active_company,
        map_revision: sim.state.map.mutation_revision(),
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
        rail_infrastructure: cache
            .snapshot
            .as_ref()
            .map_or_else(RailInfrastructureSummary::default, |s| {
                s.rail_infrastructure
            }),
        road_infrastructure: cache
            .snapshot
            .as_ref()
            .map_or_else(RoadInfrastructureSummary::default, |s| {
                s.road_infrastructure
            }),
        water_infrastructure: cache
            .snapshot
            .as_ref()
            .map_or_else(WaterInfrastructureSummary::default, |s| {
                s.water_infrastructure
            }),
        station_infrastructure: cache
            .snapshot
            .as_ref()
            .map_or_else(StationInfrastructureSummary::default, |s| {
                s.station_infrastructure
            }),
        companies,
    };
    let need_infra = cache.snapshot.as_ref().is_none_or(|prev| {
        prev.active_company != soft.active_company
            || prev.map_revision != soft.map_revision
            || prev.money != soft.money
            || prev.loan != soft.loan
            || prev.cargo_income != soft.cargo_income
            || prev.running_costs != soft.running_costs
            || prev.deliveries != soft.deliveries
            || prev.vehicles != soft.vehicles
            || prev.stations != soft.stations
            || prev.station_infrastructure != soft.station_infrastructure
            || prev.companies != soft.companies
    });
    let (rail_infrastructure, road_infrastructure, water_infrastructure, station_infrastructure) =
        if need_infra {
            let rail_infrastructure =
                rail_infrastructure_for_company(&sim.state.map, sim.state.active_company);
            let road_infrastructure = road_infrastructure_for_company_with_stations(
                &sim.state.map,
                &sim.state.stations,
                sim.state.active_company,
            );
            let water_infrastructure =
                water_infrastructure_for_company(&sim.state.map, sim.state.active_company);
            let station_infrastructure = station_infrastructure_for_company(
                &sim.state.map,
                &sim.state.stations,
                sim.state.active_company,
            );
            (
                rail_infrastructure,
                road_infrastructure,
                water_infrastructure,
                station_infrastructure,
            )
        } else {
            (
                soft.rail_infrastructure,
                soft.road_infrastructure,
                soft.water_infrastructure,
                soft.station_infrastructure,
            )
        };
    let snapshot = FinancesSnapshot {
        rail_infrastructure,
        road_infrastructure,
        water_infrastructure,
        station_infrastructure,
        ..soft
    };
    if cache.locale == Some(locale) && cache.snapshot.as_ref() == Some(&snapshot) {
        return;
    }
    cache.snapshot = Some(snapshot.clone());
    cache.locale = Some(locale);

    if let Some((_, mut title)) = title_q
        .iter_mut()
        .find(|(t, _)| t.0 == FloatingWindowId::Finances)
    {
        let name = sim
            .state
            .companies
            .iter()
            .find(|c| c.id == sim.state.active_company)
            .map(|c| c.name.clone())
            .unwrap_or_else(|| localized_text(locale, crate::ui::statusbar::COMPANY_DISPLAY_NAME));
        **title = name;
    }
    if let Ok(mut body) = body_q.single_mut() {
        let net = snapshot.money.saturating_sub(snapshot.loan);
        let profit = snapshot.cargo_income as i64 - snapshot.running_costs as i64;
        let mut companies_block = format!("\n\n{}:", localized_text(locale, "Compañías"));
        for row in &snapshot.companies {
            let tag = if row.is_ai {
                format!(" ({})", localized_text(locale, "IA"))
            } else {
                String::new()
            };
            companies_block.push_str(&format!(
                "\n  {}{} · {} #{} · {} {} · {} {}",
                row.name,
                tag,
                localized_text(locale, "color"),
                row.colour,
                localized_text(locale, "Dinero"),
                format_money(row.money),
                localized_text(locale, "ingresos"),
                format_money(row.cargo_income.cast_signed()),
            ));
        }
        **body = format!(
            "{}: {}\n{}: {} / {}\n{}: {}\n\
             ({}: {})\n\n\
             {}: {}\n{}: {}\n\
             {}: {}\n\
             {}: {} ({} {})\n\n\
             {}:\n\
               {}: {}\n\
               {}: {}\n\
               {}: {}\n\
             {}: {} {} ({}: {}, {}: {}, {}: {}, {}: {}; {}: {}) · {}: {} {} ({}: {}, {}: {}) · {}: {} {}{}",
            localized_text(locale, "Efectivo"),
            format_money(snapshot.money),
            localized_text(locale, "Préstamo"),
            format_money(snapshot.loan),
            format_money(snapshot.max_loan),
            localized_text(locale, "Patrimonio neto"),
            format_money(net),
            localized_text(locale, "cada operación"),
            format_money(LOAN_INTERVAL),
            localized_text(locale, "Ingresos por transporte"),
            format_money(snapshot.cargo_income.cast_signed()),
            localized_text(locale, "Costes de explotación"),
            format_money(snapshot.running_costs.cast_signed()),
            localized_text(locale, "Beneficio operativo"),
            format_money(profit),
            localized_text(locale, "Entregas"),
            snapshot.deliveries,
            snapshot.units_delivered,
            localized_text(locale, "unidades"),
            localized_text(locale, "Infraestructura"),
            localized_text(locale, "Vehículos"),
            snapshot.vehicles,
            localized_text(locale, "Estaciones"),
            snapshot.station_infrastructure.station_total(),
            localized_text(locale, "Aeropuertos"),
            snapshot.station_infrastructure.airport_total(),
            localized_text(locale, "Vía"),
            snapshot.rail_infrastructure.rail_total(),
            localized_text(locale, "piezas"),
            localized_text(locale, "Normal"),
            snapshot
                .rail_infrastructure
                .rail_type_count(openttdrs_core::RailType::Rail),
            localized_text(locale, "Eléctrica"),
            snapshot
                .rail_infrastructure
                .rail_type_count(openttdrs_core::RailType::Electric),
            localized_text(locale, "Monorail"),
            snapshot
                .rail_infrastructure
                .rail_type_count(openttdrs_core::RailType::Monorail),
            localized_text(locale, "Maglev"),
            snapshot
                .rail_infrastructure
                .rail_type_count(openttdrs_core::RailType::Maglev),
            localized_text(locale, "Señales"),
            snapshot.rail_infrastructure.signals,
            localized_text(locale, "Carretera"),
            snapshot.road_infrastructure.road_total(),
            localized_text(locale, "piezas"),
            localized_text(locale, "Carretera"),
            snapshot
                .road_infrastructure
                .road_class_total(RoadTramType::Road, &sim.state.road_type_catalog),
            localized_text(locale, "Tranvía"),
            snapshot
                .road_infrastructure
                .road_class_total(RoadTramType::Tram, &sim.state.road_type_catalog),
            localized_text(locale, "Agua"),
            snapshot.water_infrastructure.water_total(),
            localized_text(locale, "piezas"),
            companies_block,
        );
    }
}

pub(crate) fn finances_window_on_closed(
    mut closed: MessageReader<FloatingWindowClosed>,
    mut finances: ResMut<FinancesWindowState>,
) {
    for msg in closed.read() {
        if msg.0.class == FloatingWindowId::Finances {
            finances.open = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finances_chrome_localizes_without_translating_dynamic_company_data() {
        assert_eq!(
            finances_button_label(Locale::En, FinancesWindowButton::IncreaseLoan),
            "Take loan"
        );
        assert_eq!(
            finances_button_label(Locale::En, FinancesWindowButton::DecreaseLoan),
            "Repay loan"
        );
        assert_eq!(
            finances_button_label(Locale::En, FinancesWindowButton::BuyRival),
            "Buy rival (bankruptcy)"
        );
        assert_eq!(
            finances_button_label(Locale::En, FinancesWindowButton::OpenAiSettings),
            "AI…"
        );
        assert_eq!(localized_text(Locale::En, "Efectivo"), "Cash");
        assert_eq!(
            localized_text(Locale::En, "Infraestructura"),
            "Infrastructure"
        );
        assert_eq!(localized_text(Locale::En, "Empresa Ñandú"), "Empresa Ñandú");
    }
}
