//! Company View stub (`WC_COMPANY`) — familia economy (#271).
//!
//! Muestra datos de compañía existentes (nombre, dinero, préstamo, flota).
//! Livery / ManagerFace / Infrastructure detallado: residual.

use bevy::prelude::*;
use openttdrs_core::format_money;

use crate::state::SimWorld;
use crate::ui::finances_window::FinancesWindowState;
use crate::ui::floating_window::{
    FloatingWindow, FloatingWindowClosed, FloatingWindowId, FloatingWindowTitleText, TITLE_BROWN,
    WINDOW_TEXT, spawn_floating_window, window_text_font,
};
use crate::ui::font::UiFontRole;
use crate::ui::navigation::{OpenUiRoute, UiRoute};
use crate::ui::toolbar::BuildMenuUi;

#[derive(Resource, Default)]
pub(crate) struct CompanyViewWindowState {
    pub(crate) open: bool,
}

#[derive(Component)]
pub(crate) struct CompanyViewBodyText;

#[derive(Component, Clone, Copy)]
pub(crate) enum CompanyViewButton {
    OpenFinances,
}

pub(crate) fn setup_company_view_window(mut commands: Commands, asset_server: Res<AssetServer>) {
    let asset_server = &*asset_server;
    let (_root, content) = spawn_floating_window(
        &mut commands,
        asset_server,
        FloatingWindowId::CompanyView,
        "Compañía",
        TITLE_BROWN,
        Vec2::new(220.0, 100.0),
        280.0,
    );
    commands.entity(content).with_children(|panel| {
        panel.spawn((
            CompanyViewBodyText,
            Text::new(""),
            window_text_font(asset_server, UiFontRole::Caption),
            TextColor(WINDOW_TEXT),
            BuildMenuUi,
        ));
        panel.spawn((
            Button,
            CompanyViewButton::OpenFinances,
            Node {
                margin: UiRect::top(Val::Px(6.0)),
                height: Val::Px(22.0),
                padding: UiRect::horizontal(Val::Px(8.0)),
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
                Text::new("Finanzas…"),
                window_text_font(asset_server, UiFontRole::Caption),
                TextColor(WINDOW_TEXT),
            )],
        ));
    });
}

pub(crate) fn open_company_view_from_routes(
    mut routes: MessageReader<OpenUiRoute>,
    mut state: ResMut<CompanyViewWindowState>,
) {
    for route in routes.read() {
        if matches!(route.0, UiRoute::CompanyView) {
            state.open = true;
        }
    }
}

pub(crate) fn handle_company_view_buttons(
    buttons: Query<(&Interaction, &CompanyViewButton), (Changed<Interaction>, With<Button>)>,
    mut finances: ResMut<FinancesWindowState>,
) {
    for (interaction, button) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match button {
            CompanyViewButton::OpenFinances => finances.open = true,
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn sync_company_view_window(
    state: Res<CompanyViewWindowState>,
    sim: Res<SimWorld>,
    mut root_q: Query<(&FloatingWindow, &mut Visibility)>,
    mut title_q: Query<(&FloatingWindowTitleText, &mut Text), Without<CompanyViewBodyText>>,
    mut body_q: Query<&mut Text, (With<CompanyViewBodyText>, Without<FloatingWindowTitleText>)>,
) {
    let Some((_, mut vis)) = root_q
        .iter_mut()
        .find(|(w, _)| w.id == FloatingWindowId::CompanyView)
    else {
        return;
    };
    if !state.open {
        *vis = Visibility::Hidden;
        return;
    }
    *vis = Visibility::Visible;

    let company = sim
        .state
        .companies
        .iter()
        .find(|c| c.id == sim.state.active_company);
    let name = company.map(|c| c.name.as_str()).unwrap_or("Compañía");
    let money = company
        .map(|c| c.economy.money)
        .unwrap_or(sim.state.economy.money);
    let loan = company
        .map(|c| c.economy.loan)
        .unwrap_or(sim.state.economy.loan);
    let fleet = sim
        .state
        .vehicles
        .iter()
        .filter(|v| v.owner == sim.state.active_company)
        .count();

    if let Some((_, mut title)) = title_q
        .iter_mut()
        .find(|(tt, _)| tt.0 == FloatingWindowId::CompanyView)
    {
        **title = format!("Compañía — {name}");
    }
    if let Ok(mut body) = body_q.single_mut() {
        **body = format!(
            "{name}\nDinero: {}\nPréstamo: {}\nFlota: {fleet} vehículos\n\
             Stub — Livery/ManagerFace/Infrastructure residual (#271).",
            format_money(money),
            format_money(loan),
        );
    }
}

pub(crate) fn company_view_window_on_closed(
    mut closed: MessageReader<FloatingWindowClosed>,
    mut state: ResMut<CompanyViewWindowState>,
) {
    for msg in closed.read() {
        if msg.0.class == FloatingWindowId::CompanyView {
            state.open = false;
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    #[test]
    fn route_opens_company_view() {
        let mut world = World::new();
        world.init_resource::<CompanyViewWindowState>();
        world.init_resource::<Messages<OpenUiRoute>>();
        world.write_message(OpenUiRoute(UiRoute::CompanyView));
        world
            .run_system_once(open_company_view_from_routes)
            .unwrap();
        assert!(world.resource::<CompanyViewWindowState>().open);
    }
}
