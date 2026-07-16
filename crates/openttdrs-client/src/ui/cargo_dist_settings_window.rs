//! Ventana de ajustes `CargoDist` (`cargo_dist.distribution`).

use bevy::prelude::*;
use openttdrs_core::Command;
use openttdrs_core::flow_stat::DistributionType;

use crate::state::SimWorld;
use crate::ui::floating_window::{
    FloatingWindow, FloatingWindowClosed, FloatingWindowId, TITLE_BROWN, WINDOW_TEXT,
    spawn_floating_window, window_text_font,
};
use crate::ui::font::UiFontRole;
use crate::ui::toolbar::BuildMenuUi;
use crate::ui::window_lifecycle::{
    close_floating_window_on_message, sync_floating_window_visibility,
};

const BTN_BG: Color = Color::srgb(0.36, 0.31, 0.21);
const BTN_BORDER: Color = Color::srgb(0.66, 0.58, 0.38);
const BTN_ACTIVE: Color = Color::srgb(0.98, 0.92, 0.35);

#[derive(Resource, Default)]
pub(crate) struct CargoDistSettingsWindowState {
    pub(crate) open: bool,
}

#[derive(Component, Clone, Copy)]
pub(crate) struct CargoDistSettingsAction(pub DistributionType);

pub(crate) fn setup_cargo_dist_settings_window(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
) {
    let asset_server = &*asset_server;
    let (_root, content) = spawn_floating_window(
        &mut commands,
        asset_server,
        FloatingWindowId::CargoDistSettings,
        "Distribución de carga",
        TITLE_BROWN,
        Vec2::new(300.0, 180.0),
        380.0,
    );
    commands.entity(content).with_children(|body| {
        body.spawn((
            Text::new(
                "Manual: hop desde órdenes del vehículo.\n\
                 Asimétrica: Demand + MCF OpenTTD (Dijkstra distancia/capacidad).\n\
                 Simétrica: Demand Symmetric OpenTTD (geografía + supply) + MCF.",
            ),
            window_text_font(asset_server, UiFontRole::Caption),
            TextColor(Color::srgb(0.82, 0.78, 0.68)),
        ));
        body.spawn((Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(8.0),
            margin: UiRect::top(Val::Px(12.0)),
            flex_wrap: FlexWrap::Wrap,
            ..default()
        },))
            .with_children(|row| {
                for (mode, label) in [
                    (DistributionType::Manual, "Manual"),
                    (DistributionType::Asymmetric, "Asimétrica"),
                    (DistributionType::Symmetric, "Simétrica"),
                ] {
                    row.spawn((
                        Button,
                        CargoDistSettingsAction(mode),
                        Node {
                            min_width: Val::Px(96.0),
                            height: Val::Px(28.0),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            border: UiRect::all(Val::Px(1.0)),
                            padding: UiRect::horizontal(Val::Px(8.0)),
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
    });
}

pub(crate) fn sync_cargo_dist_settings_window(
    state: Res<CargoDistSettingsWindowState>,
    sim: Res<SimWorld>,
    mut root_q: Query<(&FloatingWindow, &mut Visibility)>,
    mut buttons: Query<(&CargoDistSettingsAction, &mut BorderColor), Without<FloatingWindow>>,
) {
    sync_floating_window_visibility(&mut root_q, FloatingWindowId::CargoDistSettings, state.open);
    if !state.open {
        return;
    }

    let current = sim.state.cargo_dist.distribution;
    for (action, mut border) in &mut buttons {
        *border = if action.0 == current {
            BorderColor::all(BTN_ACTIVE)
        } else {
            BorderColor::all(BTN_BORDER)
        };
    }
}

pub(crate) fn handle_cargo_dist_settings_buttons(
    mut sim: ResMut<SimWorld>,
    buttons: Query<(&Interaction, &CargoDistSettingsAction), (Changed<Interaction>, With<Button>)>,
) {
    for (interaction, action) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let _ = crate::network::apply_player_command(
            &mut sim.state,
            &Command::SetCargoDistDistribution(action.0),
        );
    }
}

pub(crate) fn cargo_dist_settings_on_closed(
    mut closed: MessageReader<FloatingWindowClosed>,
    mut state: ResMut<CargoDistSettingsWindowState>,
) {
    close_floating_window_on_message(&mut closed, FloatingWindowId::CargoDistSettings, || {
        state.open = false;
    });
}
