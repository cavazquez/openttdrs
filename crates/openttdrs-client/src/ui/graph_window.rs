//! Gráficos económicos (Income / Operating Profit / Company Value) filtrados por compañía.

use bevy::prelude::*;
use openttdrs_core::prelude::*;
use openttdrs_core::{company_net_value, format_money};

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
const BTN_BG: Color = Color::srgb(0.36, 0.31, 0.21);
const BTN_ACTIVE: Color = Color::srgb(0.58, 0.50, 0.31);
const BTN_BORDER: Color = Color::srgb(0.66, 0.58, 0.38);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum GraphKind {
    #[default]
    Income,
    OperatingProfit,
    CompanyValue,
    /// Rating 0..=1000 por trimestre (`performance_history`).
    PerformanceHistory,
}

#[derive(Resource)]
pub(crate) struct GraphWindowState {
    pub(crate) open: bool,
    pub(crate) kind: GraphKind,
    /// Compañía cuyas series se muestran (`None` = seguir la activa).
    pub(crate) filter_company: Option<CompanyId>,
}

impl Default for GraphWindowState {
    fn default() -> Self {
        Self {
            open: false,
            kind: GraphKind::Income,
            filter_company: None,
        }
    }
}

impl GraphWindowState {
    #[must_use]
    pub(crate) fn resolved_company(&self, active: CompanyId) -> CompanyId {
        self.filter_company.unwrap_or(active)
    }
}

#[derive(Component)]
pub(crate) struct GraphWindowHintText;

#[derive(Component, Clone, Copy)]
pub(crate) struct GraphBar {
    slot: usize,
}

#[derive(Component, Clone, Copy)]
pub(crate) struct GraphKindButton(GraphKind);

#[derive(Component, Clone, Copy)]
pub(crate) struct GraphCompanyButton(CompanyId);

#[derive(Component)]
pub(crate) struct GraphCompanyFilterRow;

pub(crate) fn setup_graph_window(mut commands: Commands, asset_server: Res<AssetServer>) {
    let asset_server = &*asset_server;
    let (_root, content) = spawn_floating_window(
        &mut commands,
        asset_server,
        FloatingWindowId::Graphs,
        "Gráficos",
        TITLE_BROWN,
        Vec2::new(200.0, 80.0),
        420.0,
    );
    commands.entity(content).with_children(|panel| {
        panel
            .spawn(Node {
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                column_gap: Val::Px(4.0),
                row_gap: Val::Px(4.0),
                margin: UiRect::bottom(Val::Px(6.0)),
                ..default()
            })
            .with_children(|row| {
                spawn_kind_button(row, asset_server, GraphKind::Income, "Ingresos");
                spawn_kind_button(
                    row,
                    asset_server,
                    GraphKind::OperatingProfit,
                    "Beneficio operativo",
                );
                spawn_kind_button(row, asset_server, GraphKind::CompanyValue, "Valor compañía");
                spawn_kind_button(
                    row,
                    asset_server,
                    GraphKind::PerformanceHistory,
                    "Rendimiento",
                );
            });
        panel.spawn((
            GraphCompanyFilterRow,
            Node {
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                column_gap: Val::Px(4.0),
                row_gap: Val::Px(4.0),
                margin: UiRect::bottom(Val::Px(6.0)),
                ..default()
            },
            BuildMenuUi,
        ));
        panel
            .spawn(Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(12.0),
                align_items: AlignItems::Center,
                margin: UiRect::bottom(Val::Px(4.0)),
                ..default()
            })
            .with_children(|legend| {
                spawn_legend_swatch(legend, asset_server, INCOME_COLOR, "Ingresos");
                spawn_legend_swatch(legend, asset_server, PROFIT_POS, "Beneficio +");
                spawn_legend_swatch(legend, asset_server, PROFIT_NEG, "Beneficio −");
                spawn_legend_swatch(legend, asset_server, VALUE_POS, "Valor");
            });
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
                        GraphBar { slot },
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

fn spawn_kind_button(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    kind: GraphKind,
    label: &'static str,
) {
    parent.spawn((
        Button,
        GraphKindButton(kind),
        Node {
            min_width: Val::Px(110.0),
            height: Val::Px(24.0),
            padding: UiRect::horizontal(Val::Px(6.0)),
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
            TextColor(WINDOW_TEXT),
        )],
    ));
}

fn spawn_legend_swatch(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    color: Color,
    label: &'static str,
) {
    parent
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(4.0),
            align_items: AlignItems::Center,
            ..default()
        })
        .with_children(|row| {
            row.spawn((
                Node {
                    width: Val::Px(10.0),
                    height: Val::Px(10.0),
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(color),
                BorderColor::all(Color::srgb(0.55, 0.48, 0.35)),
                BuildMenuUi,
            ));
            row.spawn((
                Text::new(label),
                window_text_font(asset_server, UiFontRole::Caption),
                TextColor(Color::srgb(0.82, 0.78, 0.68)),
            ));
        });
}

pub(crate) fn open_graph_from_routes(
    mut routes: MessageReader<OpenUiRoute>,
    mut state: ResMut<GraphWindowState>,
    sim: Res<SimWorld>,
) {
    for route in routes.read() {
        if let UiRoute::Graph(kind) = route.0 {
            state.kind = kind;
            state.open = true;
            // Al abrir desde menú, anclar a la compañía activa.
            state.filter_company = Some(sim.state.active_company);
        }
    }
}

pub(crate) fn handle_graph_window_buttons(
    mut kind_buttons: Query<(&Interaction, &GraphKindButton), (Changed<Interaction>, With<Button>)>,
    mut company_buttons: Query<
        (&Interaction, &GraphCompanyButton),
        (Changed<Interaction>, With<Button>, Without<GraphKindButton>),
    >,
    mut state: ResMut<GraphWindowState>,
) {
    for (interaction, button) in &mut kind_buttons {
        if *interaction == Interaction::Pressed {
            state.kind = button.0;
            state.open = true;
        }
    }
    for (interaction, button) in &mut company_buttons {
        if *interaction == Interaction::Pressed {
            state.filter_company = Some(button.0);
            state.open = true;
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn sync_graph_window(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut state: ResMut<GraphWindowState>,
    sim: Res<SimWorld>,
    mut root_q: Query<(&FloatingWindow, &mut Visibility)>,
    mut title_q: Query<(&FloatingWindowTitleText, &mut Text), Without<GraphWindowHintText>>,
    mut hint_q: Query<&mut Text, (With<GraphWindowHintText>, Without<FloatingWindowTitleText>)>,
    mut bars: Query<(&GraphBar, &mut Node, &mut BackgroundColor), Without<GraphKindButton>>,
    mut kind_buttons: Query<
        (&GraphKindButton, &Interaction, &mut BackgroundColor),
        (With<Button>, Without<GraphBar>, Without<GraphCompanyButton>),
    >,
    company_row: Query<Entity, With<GraphCompanyFilterRow>>,
    existing_company_btns: Query<Entity, With<GraphCompanyButton>>,
    mut company_btn_style: Query<
        (&GraphCompanyButton, &Interaction, &mut BackgroundColor),
        (With<Button>, Without<GraphKindButton>, Without<GraphBar>),
    >,
) {
    let Some((_, mut vis)) = root_q
        .iter_mut()
        .find(|(window, _)| window.id == FloatingWindowId::Graphs)
    else {
        return;
    };
    if !state.open {
        *vis = Visibility::Hidden;
        return;
    }
    *vis = Visibility::Visible;

    if state.filter_company.is_none() {
        state.filter_company = Some(sim.state.active_company);
    }
    let filter_id = state.resolved_company(sim.state.active_company);
    // Si la compañía filtrada ya no existe, volver a la activa.
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

    let title = match state.kind {
        GraphKind::Income => format!("Ingresos — {company_name}"),
        GraphKind::OperatingProfit => format!("Beneficio operativo — {company_name}"),
        GraphKind::CompanyValue => format!("Valor — {company_name}"),
        GraphKind::PerformanceHistory => format!("Rendimiento — {company_name}"),
    };
    if let Some((_, mut t)) = title_q
        .iter_mut()
        .find(|(text, _)| text.0 == FloatingWindowId::Graphs)
    {
        **t = title;
    }

    for (button, interaction, mut bg) in &mut kind_buttons {
        *bg = if button.0 == state.kind {
            BackgroundColor(BTN_ACTIVE)
        } else if *interaction == Interaction::Hovered {
            BackgroundColor(Color::srgb(0.47, 0.41, 0.28))
        } else {
            BackgroundColor(BTN_BG)
        };
    }

    // Rebuild company filter buttons if the pool changed.
    let want_ids: Vec<CompanyId> = sim.state.companies.iter().map(|c| c.id).collect();
    let have_ids: Vec<CompanyId> = company_btn_style.iter().map(|(b, _, _)| b.0).collect();
    if want_ids != have_ids
        && let Ok(row) = company_row.single()
    {
        for e in &existing_company_btns {
            commands.entity(e).despawn();
        }
        let asset_server = &*asset_server;
        commands.entity(row).with_children(|parent| {
            for company in &sim.state.companies {
                let label = if company.id == sim.state.active_company {
                    format!("{}*", company.name)
                } else {
                    company.name.clone()
                };
                parent.spawn((
                    Button,
                    GraphCompanyButton(company.id),
                    Node {
                        min_width: Val::Px(72.0),
                        height: Val::Px(22.0),
                        padding: UiRect::horizontal(Val::Px(6.0)),
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
                        TextColor(WINDOW_TEXT),
                    )],
                ));
            }
        });
    }

    for (button, interaction, mut bg) in &mut company_btn_style {
        *bg = if button.0 == filter_id {
            BackgroundColor(BTN_ACTIVE)
        } else if *interaction == Interaction::Hovered {
            BackgroundColor(Color::srgb(0.47, 0.41, 0.28))
        } else {
            BackgroundColor(BTN_BG)
        };
    }

    let company = sim.state.companies.iter().find(|c| c.id == filter_id);
    let (values, period_label): (Vec<i64>, &str) = match state.kind {
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
            // Fallback: saves antiguos sin historial por compañía.
            let legacy = &sim.state.stats.economy_history.samples;
            let samples = if samples.is_empty() && filter_id == sim.state.active_company {
                legacy.as_slice()
            } else {
                samples
            };
            (
                samples
                    .iter()
                    .map(|s| match state.kind {
                        GraphKind::Income => s.income as i64,
                        GraphKind::OperatingProfit => s.operating_profit(),
                        GraphKind::CompanyValue => s.company_value,
                        GraphKind::PerformanceHistory => 0,
                    })
                    .collect(),
                "mensuales",
            )
        }
    };
    let max_abs = values
        .iter()
        .map(|v| v.unsigned_abs())
        .max()
        .unwrap_or(1)
        .max(1);

    if let Ok(mut hint) = hint_q.single_mut() {
        if values.is_empty() {
            **hint = format!("Sin datos {period_label} de {company_name} (avanza el tiempo).");
        } else {
            let last = *values.last().unwrap_or(&0);
            match state.kind {
                GraphKind::CompanyValue => {
                    let (money, loan) = company
                        .map(|c| (c.economy.money, c.economy.loan))
                        .unwrap_or((sim.state.economy.money, sim.state.economy.loan));
                    let live = company_net_value(money, loan);
                    **hint = format!(
                        "{company_name} · Último cierre: {} · Actual: {}",
                        format_money(last),
                        format_money(live),
                    );
                }
                GraphKind::PerformanceHistory => {
                    let live_value = company
                        .map(|c| openttdrs_core::calculate_company_value(&sim.state, c.id))
                        .unwrap_or(0);
                    **hint = format!(
                        "{company_name} · Último trimestre: {last}/1000 · Valoración activos: {}",
                        format_money(live_value),
                    );
                }
                _ => {
                    let lifetime_income = company
                        .map(|c| c.cargo_income_earned)
                        .unwrap_or(sim.state.stats.cargo_income_earned);
                    let lifetime_costs = company
                        .map(|c| c.vehicle_running_costs)
                        .unwrap_or(sim.state.stats.vehicle_running_costs);
                    let lifetime_profit = lifetime_income as i64 - lifetime_costs as i64;
                    **hint = format!(
                        "{company_name} · Último mes: {} · Acumulado: ingresos {} · costes {} · beneficio {}",
                        format_money(last),
                        format_money(lifetime_income as i64),
                        format_money(lifetime_costs as i64),
                        format_money(lifetime_profit),
                    );
                }
            }
        }
    }

    let start = values.len().saturating_sub(BAR_COUNT);
    for (bar, mut node, mut bg) in &mut bars {
        let idx = start + bar.slot;
        let Some(&value) = values.get(idx) else {
            node.height = Val::Px(2.0);
            *bg = BackgroundColor(Color::srgb(0.28, 0.24, 0.17));
            continue;
        };
        let h = (value.unsigned_abs() as f32 / max_abs as f32 * BAR_MAX_H).max(2.0);
        node.height = Val::Px(h);
        *bg = BackgroundColor(match state.kind {
            GraphKind::Income => INCOME_COLOR,
            GraphKind::OperatingProfit if value >= 0 => PROFIT_POS,
            GraphKind::OperatingProfit => PROFIT_NEG,
            GraphKind::CompanyValue if value >= 0 => VALUE_POS,
            GraphKind::CompanyValue => VALUE_NEG,
            GraphKind::PerformanceHistory => Color::srgb(0.45, 0.62, 0.78),
        });
    }
}

pub(crate) fn graph_window_on_closed(
    mut closed: MessageReader<FloatingWindowClosed>,
    mut state: ResMut<GraphWindowState>,
) {
    for message in closed.read() {
        if message.0 == FloatingWindowId::Graphs {
            state.open = false;
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
        assert!(state.open);
        assert_eq!(state.kind, GraphKind::Income);
        assert_eq!(state.filter_company, Some(CompanyId::PLAYER));
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
        assert_eq!(
            world.resource::<GraphWindowState>().kind,
            GraphKind::CompanyValue
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
        let state = GraphWindowState {
            open: true,
            kind: GraphKind::OperatingProfit,
            filter_company: Some(CompanyId::PLAYER),
        };
        assert!(state.open);
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
