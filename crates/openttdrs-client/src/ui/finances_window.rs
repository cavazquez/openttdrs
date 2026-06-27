//! Ventana de finanzas de la compañía (clic en dinero de la barra inferior).

use bevy::prelude::*;
use openttdrs_core::format_money;

use crate::state::SimWorld;
use crate::ui::floating_window::{
    FloatingWindow, FloatingWindowClosed, FloatingWindowId, FloatingWindowTitleText, TITLE_BROWN,
    WINDOW_TEXT, spawn_floating_window, window_text_font,
};
use crate::ui::font::UiFontRole;

#[derive(Resource, Default)]
pub(crate) struct FinancesWindowState {
    pub(crate) open: bool,
}

#[derive(Component)]
pub(crate) struct FinancesWindowBodyText;

#[derive(Default)]
pub(crate) struct FinancesSyncCache {
    snapshot: Option<FinancesSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FinancesSnapshot {
    money: i64,
    loan: i64,
    cargo_income: u64,
    running_costs: u64,
    deliveries: u64,
}

pub(crate) fn setup_finances_window(mut commands: Commands, asset_server: Res<AssetServer>) {
    let asset_server = &*asset_server;
    let (_root, content) = spawn_floating_window(
        &mut commands,
        asset_server,
        FloatingWindowId::Finances,
        "Finanzas",
        TITLE_BROWN,
        Vec2::new(720.0, 120.0),
        280.0,
    );
    commands.entity(content).with_children(|body| {
        body.spawn((
            FinancesWindowBodyText,
            Text::new(""),
            window_text_font(asset_server, UiFontRole::Caption),
            TextColor(WINDOW_TEXT),
        ));
    });
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

    let snapshot = FinancesSnapshot {
        money: sim.state.economy.money,
        loan: sim.state.economy.loan,
        cargo_income: sim.state.stats.cargo_income_earned,
        running_costs: sim.state.stats.vehicle_running_costs,
        deliveries: sim.state.stats.cargo_deliveries,
    };
    if cache.snapshot.as_ref() == Some(&snapshot) {
        return;
    }
    cache.snapshot = Some(snapshot.clone());

    if let Some((_, mut title)) = title_q
        .iter_mut()
        .find(|(t, _)| t.0 == FloatingWindowId::Finances)
    {
        **title = crate::ui::statusbar::COMPANY_DISPLAY_NAME.to_string();
    }
    if let Ok(mut body) = body_q.single_mut() {
        let net = snapshot.money.saturating_sub(snapshot.loan);
        **body = format!(
            "Efectivo: {}\nPréstamo: {}\nPatrimonio neto: {}\n\n\
             Ingresos por transporte: {}\nCostes de explotación: {}\n\
             Entregas completadas: {}",
            format_money(snapshot.money),
            format_money(snapshot.loan),
            format_money(net),
            format_money(snapshot.cargo_income.cast_signed()),
            format_money(snapshot.running_costs.cast_signed()),
            snapshot.deliveries,
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
