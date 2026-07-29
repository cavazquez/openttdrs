//! Tarifas de pago por tipo de carga (`CargoPaymentRates`).

use bevy::prelude::*;
use openttdrs_core::{ALL_CARGO_TYPES, CargoType, calendar_year_at_tick, cargo_display_name};

use crate::state::SimWorld;
use crate::ui::floating_window::{
    FloatingWindow, FloatingWindowClosed, FloatingWindowId, FloatingWindowTitleText, TITLE_BROWN,
    WINDOW_TEXT, spawn_floating_window, window_text_font,
};
use crate::ui::font::UiFontRole;
use crate::ui::navigation::{OpenUiRoute, UiRoute};
use crate::ui::toolbar::BuildMenuUi;

const CARGOS: &[CargoType] = &ALL_CARGO_TYPES;

#[derive(Resource, Default)]
pub(crate) struct CargoPaymentWindowState {
    pub(crate) open: bool,
}

#[derive(Component)]
pub(crate) struct CargoPaymentBodyText;

pub(crate) fn setup_cargo_payment_window(mut commands: Commands, asset_server: Res<AssetServer>) {
    let asset_server = &*asset_server;
    let (_root, content) = spawn_floating_window(
        &mut commands,
        asset_server,
        FloatingWindowId::CargoPaymentRates,
        "Tarifas de carga",
        TITLE_BROWN,
        Vec2::new(520.0, 120.0),
        360.0,
    );
    commands.entity(content).with_children(|panel| {
        panel.spawn((
            CargoPaymentBodyText,
            Text::new(""),
            window_text_font(asset_server, UiFontRole::Caption),
            TextColor(WINDOW_TEXT),
            BuildMenuUi,
        ));
    });
}

pub(crate) fn open_cargo_payment_from_routes(
    mut routes: MessageReader<OpenUiRoute>,
    mut state: ResMut<CargoPaymentWindowState>,
) {
    for route in routes.read() {
        if matches!(route.0, UiRoute::CargoPaymentRates) {
            state.open = true;
        }
    }
}

pub(crate) fn sync_cargo_payment_window(
    state: Res<CargoPaymentWindowState>,
    sim: Res<SimWorld>,
    mut root_q: Query<(&FloatingWindow, &mut Visibility)>,
    mut title_q: Query<(&FloatingWindowTitleText, &mut Text)>,
    mut body_q: Query<&mut Text, (With<CargoPaymentBodyText>, Without<FloatingWindowTitleText>)>,
) {
    let Some((_, mut vis)) = root_q
        .iter_mut()
        .find(|(window, _)| window.id == FloatingWindowId::CargoPaymentRates)
    else {
        return;
    };
    if !state.open {
        *vis = Visibility::Hidden;
        return;
    }
    *vis = Visibility::Visible;

    if let Some((_, mut title)) = title_q
        .iter_mut()
        .find(|(text, _)| text.0 == FloatingWindowId::CargoPaymentRates)
    {
        **title = "Tarifas de carga".to_string();
    }

    let year = calendar_year_at_tick(sim.state.tick);
    let mut lines = vec![
        format!("Año {year} · pago base por unidad (antes de distancia/tiempo)"),
        String::new(),
        format!(
            "{:<14} {:>8} {:>8} {:>8}",
            "Carga", "base", "rápido", "lento"
        ),
    ];
    for &cargo in CARGOS {
        let spec = cargo.payment_spec();
        lines.push(format!(
            "{:<14} {:>8} {:>8} {:>8}",
            cargo_display_name(cargo),
            spec.base_rate,
            spec.transit_fast_days,
            spec.transit_slow_days,
        ));
    }
    lines.push(String::new());
    lines.push("La inflación y el tiempo de tránsito modifican el pago real.".to_string());
    if let Ok(mut body) = body_q.single_mut() {
        **body = lines.join("\n");
    }
}

pub(crate) fn cargo_payment_window_on_closed(
    mut closed: MessageReader<FloatingWindowClosed>,
    mut state: ResMut<CargoPaymentWindowState>,
) {
    for message in closed.read() {
        if message.0.class == FloatingWindowId::CargoPaymentRates {
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
    fn route_opens_cargo_payment_window() {
        let mut world = World::new();
        world.init_resource::<CargoPaymentWindowState>();
        world.init_resource::<Messages<OpenUiRoute>>();
        world.write_message(OpenUiRoute(UiRoute::CargoPaymentRates));
        world
            .run_system_once(open_cargo_payment_from_routes)
            .unwrap();
        assert!(world.resource::<CargoPaymentWindowState>().open);
    }
}
