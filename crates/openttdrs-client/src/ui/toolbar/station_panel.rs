use bevy::prelude::*;
use openttdrs_core::{TileCoord, VehicleOrder};

use crate::state::SimWorld;

use super::BuildMenuUi;

#[derive(Resource, Default)]
pub(crate) struct StationCargoPanelState {
    pub(crate) station_pos: Option<TileCoord>,
}

#[derive(Component)]
pub(crate) struct StationCargoPanelRoot;

#[derive(Component)]
pub(crate) struct StationCargoPanelText;

#[derive(Component, Clone, Copy)]
pub(crate) enum StationCargoPanelButton {
    Close,
}

pub(crate) fn setup_station_cargo_panel(mut commands: Commands) {
    commands
        .spawn((
            StationCargoPanelRoot,
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(12.0),
                bottom: Val::Px(300.0),
                width: Val::Px(360.0),
                padding: UiRect::all(Val::Px(8.0)),
                border: UiRect::all(Val::Px(2.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.1, 0.08, 0.06, 0.95)),
            BorderColor::all(Color::srgb(0.75, 0.67, 0.45)),
            Visibility::Hidden,
            BuildMenuUi,
        ))
        .with_children(|panel| {
            panel.spawn((
                StationCargoPanelText,
                Text::new("Estación"),
                TextFont {
                    font_size: 13.0,
                    ..default()
                },
                TextColor(Color::srgb(0.94, 0.9, 0.76)),
            ));
            panel
                .spawn(Node {
                    flex_direction: FlexDirection::Row,
                    ..default()
                })
                .with_children(|row| {
                    row.spawn((
                        Button,
                        StationCargoPanelButton::Close,
                        Node {
                            width: Val::Px(90.0),
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
                            Text::new("Cerrar"),
                            TextFont {
                                font_size: 11.0,
                                ..default()
                            },
                            TextColor(Color::srgb(0.92, 0.88, 0.72)),
                        )],
                    ));
                });
        });
}

pub(crate) fn sync_station_cargo_panel(
    station_panel: Res<StationCargoPanelState>,
    sim: Res<SimWorld>,
    mut root_q: Query<&mut Visibility, With<StationCargoPanelRoot>>,
    mut text_q: Query<&mut Text, With<StationCargoPanelText>>,
) {
    let Ok(mut vis) = root_q.single_mut() else {
        return;
    };
    let Some(station_pos) = station_panel.station_pos else {
        *vis = Visibility::Hidden;
        return;
    };
    *vis = Visibility::Visible;
    let Some(station) = sim.state.stations.iter().find(|st| st.pos == station_pos) else {
        return;
    };
    let mut out = format!(
        "Estación ({}, {}) {:?}\nColas cargo: pax:{} mail:{} goods:{} coal:{} wood:{} oil:{}",
        station_pos.x,
        station_pos.y,
        station.stop_kind,
        station.cargo_stock.passengers,
        station.cargo_stock.mail,
        station.cargo_stock.goods,
        station.cargo_stock.coal,
        station.cargo_stock.wood,
        station.cargo_stock.oil
    );
    let en_route = sim
        .state
        .vehicles
        .iter()
        .filter(|vehicle| {
            vehicle
                .orders
                .iter()
                .any(|order| matches!(order, VehicleOrder::Station { station } if *station == station_pos))
        })
        .count();
    out.push_str(&format!("\nVehículos en ruta a esta estación: {en_route}"));
    if let Ok(mut text) = text_q.single_mut() {
        **text = out;
    }
}

pub(crate) fn handle_station_cargo_panel_buttons(
    mut q: Query<(&Interaction, &StationCargoPanelButton), (Changed<Interaction>, With<Button>)>,
    mut station_panel: ResMut<StationCargoPanelState>,
) {
    for (interaction, button) in &mut q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        if matches!(button, StationCargoPanelButton::Close) {
            station_panel.station_pos = None;
        }
    }
}
