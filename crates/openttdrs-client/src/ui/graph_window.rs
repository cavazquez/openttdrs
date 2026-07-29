//! Gráficos económicos (Income / Operating Profit / Company Value) filtrados por compañía.

use bevy::prelude::*;
use openttdrs_core::prelude::*;
use openttdrs_core::format_money;

use crate::state::SimWorld;
use crate::ui::floating_window::{
    FloatingWindow, FloatingWindowClosed, FloatingWindowId, FloatingWindowTitleText, TITLE_BROWN,
    WINDOW_TEXT, spawn_floating_window, window_text_font,
};
use crate::ui::font::UiFontRole;
use crate::ui::navigation::{OpenUiRoute, UiRoute};
use crate::ui::toolbar::BuildMenuUi;

const BAR_COUNT: usize = 36;
const BAR_MAX_H: f32 = 120.0;
const BAR_W: f32 = 8.0;
const INCOME_COLOR: Color = Color::srgb(0.35, 0.72, 0.38);
const PROFIT_POS: Color = Color::srgb(0.28, 0.55, 0.85);
const PROFIT_NEG: Color = Color::srgb(0.85, 0.32, 0.28);
const VALUE_POS: Color = Color::srgb(0.72, 0.58, 0.22);
const VALUE_NEG: Color = Color::srgb(0.75, 0.28, 0.35);
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum GraphKind {
    #[default]
    Income,
    OperatingProfit,
    CompanyValue,
    /// Rating 0..=1000 por trimestre (`performance_history`).
    /// Residual: reutiliza la ventana GraphIncome (#271).
    PerformanceHistory,
}

impl GraphKind {
    /// Clase flotante 15.3 asociada (PerformanceHistory → Income).
    #[must_use]
    pub(crate) const fn window_id(self) -> FloatingWindowId {
        match self {
            Self::Income | Self::PerformanceHistory => FloatingWindowId::GraphIncome,
            Self::OperatingProfit => FloatingWindowId::GraphOperatingProfit,
            Self::CompanyValue => FloatingWindowId::GraphCompanyValue,
        }
    }

    #[must_use]
    const fn title_label(self) -> &'static str {
        match self {
            Self::Income => "Ingresos",
            Self::OperatingProfit => "Beneficio operativo",
            Self::CompanyValue => "Valor compañía",
            Self::PerformanceHistory => "Rendimiento",
        }
    }
}

#[derive(Resource)]
pub(crate) struct GraphWindowState {
    /// Ventanas de gráfico abiertas por clase principal (#271).
    pub(crate) income_open: bool,
    pub(crate) profit_open: bool,
    pub(crate) value_open: bool,
    /// Kind activo en GraphIncome (Income o PerformanceHistory residual).
    pub(crate) income_kind: GraphKind,
    /// Compañía cuyas series se muestran (`None` = seguir la activa).
    pub(crate) filter_company: Option<CompanyId>,
}

impl Default for GraphWindowState {
    fn default() -> Self {
        Self {
            income_open: false,
            profit_open: false,
            value_open: false,
            income_kind: GraphKind::Income,
            filter_company: None,
        }
    }
}

impl GraphWindowState {
    #[must_use]
    pub(crate) fn resolved_company(&self, active: CompanyId) -> CompanyId {
        self.filter_company.unwrap_or(active)
    }

    pub(crate) fn set_open(&mut self, kind: GraphKind, open: bool) {
        match kind.window_id() {
            FloatingWindowId::GraphIncome => {
                self.income_open = open;
                if open {
                    self.income_kind = kind;
                }
            }
            FloatingWindowId::GraphOperatingProfit => self.profit_open = open,
            FloatingWindowId::GraphCompanyValue => self.value_open = open,
            _ => {}
        }
    }

    /// Compat tests / windows_shot: alguna clase abierta.
    #[must_use]
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn open(&self) -> bool {
        self.income_open || self.profit_open || self.value_open
    }
}

#[derive(Component)]
pub(crate) struct GraphWindowHintText;

#[derive(Component, Clone, Copy)]
pub(crate) struct GraphBar {
    slot: usize,
    kind: GraphKind,
}

fn spawn_one_graph_window(
    commands: &mut Commands,
    asset_server: &AssetServer,
    kind: GraphKind,
    pos: Vec2,
) {
    let id = kind.window_id();
    let (_root, content) = spawn_floating_window(
        commands,
        asset_server,
        id,
        kind.title_label(),
        TITLE_BROWN,
        pos,
        420.0,
    );
    commands.entity(content).with_children(|panel| {
        panel.spawn((
            GraphWindowHintText,
            Text::new(""),
            window_text_font(asset_server, UiFontRole::Caption),
            TextColor(WINDOW_TEXT),
        ));
        panel
            .spawn(Node {
                width: Val::Percent(100.0),
                height: Val::Px(BAR_MAX_H + 8.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::FlexEnd,
                column_gap: Val::Px(1.0),
                margin: UiRect::top(Val::Px(8.0)),
                padding: UiRect::all(Val::Px(4.0)),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            })
            .insert((
                BackgroundColor(Color::srgb(0.16, 0.13, 0.09)),
                BorderColor::all(Color::srgb(0.45, 0.39, 0.27)),
                BuildMenuUi,
            ))
            .with_children(|chart| {
                for slot in 0..BAR_COUNT {
                    chart.spawn((
                        GraphBar { slot, kind },
                        Node {
                            width: Val::Px(BAR_W),
                            height: Val::Px(2.0),
                            ..default()
                        },
                        BackgroundColor(INCOME_COLOR),
                        BuildMenuUi,
                    ));
                }
            });
    });
}

pub(crate) fn setup_graph_window(mut commands: Commands, asset_server: Res<AssetServer>) {
    let asset_server = &*asset_server;
    spawn_one_graph_window(
        &mut commands,
        asset_server,
        GraphKind::Income,
        Vec2::new(200.0, 80.0),
    );
    spawn_one_graph_window(
        &mut commands,
        asset_server,
        GraphKind::OperatingProfit,
        Vec2::new(240.0, 120.0),
    );
    spawn_one_graph_window(
        &mut commands,
        asset_server,
        GraphKind::CompanyValue,
        Vec2::new(280.0, 160.0),
    );
}

pub(crate) fn open_graph_from_routes(
    mut routes: MessageReader<OpenUiRoute>,
    mut state: ResMut<GraphWindowState>,
    sim: Res<SimWorld>,
) {
    for route in routes.read() {
        if let UiRoute::Graph(kind) = route.0 {
            state.set_open(kind, true);
            state.filter_company = Some(sim.state.active_company);
        }
    }
}

pub(crate) fn handle_graph_window_buttons() {
    // Filtro por compañía: residual polish (#271); menú ancla a compañía activa.
}

fn graph_series_for(
    kind: GraphKind,
    sim: &SimWorld,
    filter_id: CompanyId,
) -> (Vec<i64>, &'static str) {
    let company = sim.state.companies.iter().find(|c| c.id == filter_id);
    match kind {
        GraphKind::PerformanceHistory => {
            let q = company
                .map(|c| c.quarterly_economy.samples.as_slice())
                .unwrap_or(&[]);
            (
                q.iter().map(|s| i64::from(s.performance_history)).collect(),
                "trimestrales",
            )
        }
        _ => {
            let samples = company
                .map(|c| c.economy_history.samples.as_slice())
                .unwrap_or(&[]);
            let legacy = &sim.state.stats.economy_history.samples;
            let samples = if samples.is_empty() && filter_id == sim.state.active_company {
                legacy.as_slice()
            } else {
                samples
            };
            (
                samples
                    .iter()
                    .map(|s| match kind {
                        GraphKind::Income => s.income as i64,
                        GraphKind::OperatingProfit => s.operating_profit(),
                        GraphKind::CompanyValue => s.company_value,
                        GraphKind::PerformanceHistory => 0,
                    })
                    .collect(),
                "mensuales",
            )
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn sync_graph_window(
    mut state: ResMut<GraphWindowState>,
    sim: Res<SimWorld>,
    mut root_q: Query<(&FloatingWindow, &mut Visibility)>,
    mut title_q: Query<(&FloatingWindowTitleText, &mut Text), Without<GraphWindowHintText>>,
    mut hint_q: Query<&mut Text, (With<GraphWindowHintText>, Without<FloatingWindowTitleText>)>,
    mut bars: Query<(&GraphBar, &mut Node, &mut BackgroundColor)>,
) {
    if state.filter_company.is_none() {
        state.filter_company = Some(sim.state.active_company);
    }
    let filter_id = state.resolved_company(sim.state.active_company);
    if !sim.state.companies.iter().any(|c| c.id == filter_id) {
        state.filter_company = Some(sim.state.active_company);
    }
    let filter_id = state.resolved_company(sim.state.active_company);
    let company_name = sim
        .state
        .companies
        .iter()
        .find(|c| c.id == filter_id)
        .map(|c| c.name.as_str())
        .unwrap_or("Compañía");

    let windows = [
        (
            FloatingWindowId::GraphIncome,
            if state.income_kind == GraphKind::PerformanceHistory {
                GraphKind::PerformanceHistory
            } else {
                GraphKind::Income
            },
            state.income_open,
            GraphKind::Income,
        ),
        (
            FloatingWindowId::GraphOperatingProfit,
            GraphKind::OperatingProfit,
            state.profit_open,
            GraphKind::OperatingProfit,
        ),
        (
            FloatingWindowId::GraphCompanyValue,
            GraphKind::CompanyValue,
            state.value_open,
            GraphKind::CompanyValue,
        ),
    ];

    for (id, series_kind, open, bar_kind) in windows {
        let Some((_, mut vis)) = root_q.iter_mut().find(|(w, _)| w.id == id) else {
            continue;
        };
        if !open {
            *vis = Visibility::Hidden;
            continue;
        }
        *vis = Visibility::Visible;

        if let Some((_, mut t)) = title_q.iter_mut().find(|(text, _)| text.0 == id) {
            **t = format!("{} — {company_name}", series_kind.title_label());
        }

        let (values, _) = graph_series_for(series_kind, &sim, filter_id);
        let max_abs = values
            .iter()
            .map(|v| v.unsigned_abs())
            .max()
            .unwrap_or(1)
            .max(1);

        if let Some(mut hint) = hint_q.iter_mut().next() {
            if values.is_empty() {
                **hint = format!("Sin datos de {company_name} (avanza el tiempo).");
            } else {
                let last = *values.last().unwrap_or(&0);
                **hint = format!("{company_name} · Último: {}", format_money(last));
            }
        }

        let start = values.len().saturating_sub(BAR_COUNT);
        for (bar, mut node, mut bg) in &mut bars {
            if bar.kind != bar_kind {
                continue;
            }
            let idx = start + bar.slot;
            let Some(&value) = values.get(idx) else {
                node.height = Val::Px(2.0);
                *bg = BackgroundColor(Color::srgb(0.28, 0.24, 0.17));
                continue;
            };
            let h = (value.unsigned_abs() as f32 / max_abs as f32 * BAR_MAX_H).max(2.0);
            node.height = Val::Px(h);
            *bg = BackgroundColor(match series_kind {
                GraphKind::Income => INCOME_COLOR,
                GraphKind::OperatingProfit if value >= 0 => PROFIT_POS,
                GraphKind::OperatingProfit => PROFIT_NEG,
                GraphKind::CompanyValue if value >= 0 => VALUE_POS,
                GraphKind::CompanyValue => VALUE_NEG,
                GraphKind::PerformanceHistory => Color::srgb(0.45, 0.62, 0.78),
            });
        }
    }
}

pub(crate) fn graph_window_on_closed(
    mut closed: MessageReader<FloatingWindowClosed>,
    mut state: ResMut<GraphWindowState>,
) {
    for message in closed.read() {
        match message.0.class {
            FloatingWindowId::GraphIncome => state.income_open = false,
            FloatingWindowId::GraphOperatingProfit => state.profit_open = false,
            FloatingWindowId::GraphCompanyValue => state.value_open = false,
            _ => {}
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use openttdrs_core::MonthlyEconomySample;

    #[test]
    fn route_opens_income_graph_for_active_company() {
        let mut world = World::new();
        world.init_resource::<GraphWindowState>();
        world.insert_resource(SimWorld {
            state: GameState::new(8, 8),
            ..SimWorld::default()
        });
        world.init_resource::<Messages<OpenUiRoute>>();
        world.write_message(OpenUiRoute(UiRoute::Graph(GraphKind::Income)));
        world.run_system_once(open_graph_from_routes).unwrap();
        let state = world.resource::<GraphWindowState>();
        assert!(state.income_open);
        assert_eq!(state.income_kind, GraphKind::Income);
        assert_eq!(state.filter_company, Some(CompanyId::PLAYER));
        assert_eq!(GraphKind::Income.window_id(), FloatingWindowId::GraphIncome);
    }

    #[test]
    fn route_opens_company_value_graph() {
        let mut world = World::new();
        world.init_resource::<GraphWindowState>();
        world.insert_resource(SimWorld {
            state: GameState::new(8, 8),
            ..SimWorld::default()
        });
        world.init_resource::<Messages<OpenUiRoute>>();
        world.write_message(OpenUiRoute(UiRoute::Graph(GraphKind::CompanyValue)));
        world.run_system_once(open_graph_from_routes).unwrap();
        let state = world.resource::<GraphWindowState>();
        assert!(state.value_open);
        assert_eq!(
            GraphKind::CompanyValue.window_id(),
            FloatingWindowId::GraphCompanyValue
        );
        assert_eq!(
            GraphKind::OperatingProfit.window_id(),
            FloatingWindowId::GraphOperatingProfit
        );
    }

    #[test]
    fn operating_profit_uses_sample_helper() {
        let sample = MonthlyEconomySample {
            income: 1000,
            running_costs: 400,
            deliveries: 2,
            company_value: 80_000,
        };
        assert_eq!(sample.operating_profit(), 600);
        assert_eq!(sample.company_value, 80_000);
    }

    #[test]
    fn empty_history_keeps_graph_openable() {
        let mut state = GraphWindowState {
            filter_company: Some(CompanyId::PLAYER),
            ..Default::default()
        };
        state.set_open(GraphKind::OperatingProfit, true);
        assert!(state.open());
        assert_eq!(state.resolved_company(CompanyId::PLAYER), CompanyId::PLAYER);
        let gs = GameState::new(8, 8);
        assert!(
            gs.companies
                .first()
                .is_none_or(|c| c.economy_history.samples.is_empty())
        );
    }

    #[test]
    fn resolved_company_falls_back_to_active() {
        let state = GraphWindowState::default();
        assert_eq!(state.resolved_company(CompanyId(2)), CompanyId(2));
        let pinned = GraphWindowState {
            filter_company: Some(CompanyId(1)),
            ..Default::default()
        };
        assert_eq!(pinned.resolved_company(CompanyId::PLAYER), CompanyId(1));
    }
}
