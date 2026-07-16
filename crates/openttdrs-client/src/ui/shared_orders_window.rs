//! Lista de pools de órdenes compartidas.

use bevy::prelude::*;
use openttdrs_core::Command;

use crate::render::RemapMapVisualsPending;
use crate::state::SimWorld;
use crate::ui::floating_window::{
    FloatingWindow, FloatingWindowClosed, FloatingWindowId, FloatingWindowTitleText, TITLE_BROWN,
    WINDOW_TEXT, spawn_floating_window, window_text_font,
};
use crate::ui::font::UiFontRole;
use crate::ui::hud::{HudBuildFeedback, push_build_command_error};
use crate::ui::toolbar::BuildMenuUi;

const POOL_ROWS: usize = 12;
const BTN_BG: Color = Color::srgb(0.36, 0.31, 0.21);
const BTN_ACTIVE: Color = Color::srgb(0.58, 0.50, 0.31);
const BTN_HOVER: Color = Color::srgb(0.47, 0.41, 0.28);
const BTN_BORDER: Color = Color::srgb(0.66, 0.58, 0.38);

#[derive(Resource, Default)]
pub(crate) struct SharedOrdersWindowState {
    pub(crate) open: bool,
    /// Vehículo a vincular al elegir un pool (desde panel de órdenes).
    pub(crate) link_vehicle_id: Option<u32>,
    pub(crate) selected_shared_id: Option<u32>,
}

#[derive(Component)]
pub(crate) struct SharedOrdersHintText;

#[derive(Component, Clone, Copy)]
pub(crate) struct SharedOrdersRow {
    slot: usize,
}

#[derive(Component, Clone, Copy)]
pub(crate) struct SharedOrdersRowText {
    slot: usize,
}

#[derive(Component, Clone, Copy)]
pub(crate) enum SharedOrdersButton {
    LinkSelected,
}

pub(crate) fn setup_shared_orders_window(mut commands: Commands, asset_server: Res<AssetServer>) {
    let asset_server = &*asset_server;
    let (_root, content) = spawn_floating_window(
        &mut commands,
        asset_server,
        FloatingWindowId::SharedOrders,
        "Órdenes compartidas",
        TITLE_BROWN,
        Vec2::new(420.0, 120.0),
        340.0,
    );
    commands.entity(content).with_children(|panel| {
        panel.spawn((
            SharedOrdersHintText,
            Text::new("Pools de órdenes compartidas."),
            window_text_font(asset_server, UiFontRole::Caption),
            TextColor(WINDOW_TEXT),
        ));
        panel
            .spawn(Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(2.0),
                margin: UiRect::vertical(Val::Px(6.0)),
                max_height: Val::Px(220.0),
                overflow: Overflow::scroll_y(),
                ..default()
            })
            .with_children(|list| {
                for slot in 0..POOL_ROWS {
                    list.spawn((
                        Button,
                        SharedOrdersRow { slot },
                        Node {
                            width: Val::Percent(100.0),
                            height: Val::Px(24.0),
                            padding: UiRect::horizontal(Val::Px(6.0)),
                            justify_content: JustifyContent::FlexStart,
                            align_items: AlignItems::Center,
                            border: UiRect::all(Val::Px(1.0)),
                            display: Display::None,
                            ..default()
                        },
                        BackgroundColor(BTN_BG),
                        BorderColor::all(BTN_BORDER),
                        Interaction::default(),
                        BuildMenuUi,
                        children![(
                            SharedOrdersRowText { slot },
                            Text::new(""),
                            window_text_font(asset_server, UiFontRole::Caption),
                            TextColor(WINDOW_TEXT),
                        )],
                    ));
                }
            });
        panel.spawn((
            Button,
            SharedOrdersButton::LinkSelected,
            Node {
                min_width: Val::Px(140.0),
                height: Val::Px(24.0),
                padding: UiRect::horizontal(Val::Px(8.0)),
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
                Text::new("Vincular vehículo"),
                window_text_font(asset_server, UiFontRole::Caption),
                TextColor(WINDOW_TEXT),
            )],
        ));
    });
}

pub(crate) fn sync_shared_orders_window(
    state: Res<SharedOrdersWindowState>,
    sim: Res<SimWorld>,
    mut root_q: Query<(&FloatingWindow, &mut Visibility)>,
    mut title_q: Query<
        (&FloatingWindowTitleText, &mut Text),
        (Without<SharedOrdersHintText>, Without<SharedOrdersRowText>),
    >,
    mut hint_q: Query<
        &mut Text,
        (
            With<SharedOrdersHintText>,
            Without<FloatingWindowTitleText>,
            Without<SharedOrdersRowText>,
        ),
    >,
    mut row_q: Query<(
        &SharedOrdersRow,
        &Interaction,
        &mut Node,
        &mut BackgroundColor,
    )>,
    mut row_text_q: Query<
        (&SharedOrdersRowText, &mut Text),
        (
            Without<FloatingWindowTitleText>,
            Without<SharedOrdersHintText>,
        ),
    >,
) {
    let Some((_, mut vis)) = root_q
        .iter_mut()
        .find(|(window, _)| window.id == FloatingWindowId::SharedOrders)
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
        .find(|(text, _)| text.0 == FloatingWindowId::SharedOrders)
    {
        **title = "Órdenes compartidas".to_string();
    }
    if let Ok(mut hint) = hint_q.single_mut() {
        **hint = match state.link_vehicle_id {
            Some(id) => format!("Elige un pool y pulsa Vincular (vehículo #{id})."),
            None => "Pools existentes. Abre desde Órdenes → Pools para vincular.".to_string(),
        };
    }

    let pools = &sim.state.shared_order_lists;
    for (row, interaction, mut node, mut bg) in &mut row_q {
        let Some(pool) = pools.get(row.slot) else {
            node.display = Display::None;
            continue;
        };
        node.display = Display::Flex;
        let selected = state.selected_shared_id == Some(pool.id);
        *bg = if selected {
            BackgroundColor(BTN_ACTIVE)
        } else if *interaction == Interaction::Hovered {
            BackgroundColor(BTN_HOVER)
        } else {
            BackgroundColor(BTN_BG)
        };
    }
    for (row_text, mut text) in &mut row_text_q {
        if let Some(pool) = pools.get(row_text.slot) {
            let linked = sim
                .state
                .vehicles
                .iter()
                .filter(|v| v.shared_order_id == Some(pool.id))
                .count();
            **text = format!(
                "Pool #{} · {} órdenes · {linked} vehículos",
                pool.id,
                pool.orders.len()
            );
        } else {
            **text = String::new();
        }
    }
}

pub(crate) fn handle_shared_orders_buttons(
    mut rows: Query<(&Interaction, &SharedOrdersRow), (Changed<Interaction>, With<Button>)>,
    mut buttons: Query<
        (&Interaction, &SharedOrdersButton),
        (Changed<Interaction>, With<Button>, Without<SharedOrdersRow>),
    >,
    mut state: ResMut<SharedOrdersWindowState>,
    mut sim: ResMut<SimWorld>,
    mut pending: ResMut<RemapMapVisualsPending>,
    mut hud_feedback: ResMut<HudBuildFeedback>,
    time: Res<Time>,
) {
    if !state.open {
        return;
    }
    let pools: Vec<u32> = sim.state.shared_order_lists.iter().map(|p| p.id).collect();
    for (interaction, row) in &mut rows {
        if *interaction == Interaction::Pressed
            && let Some(&id) = pools.get(row.slot)
        {
            state.selected_shared_id = Some(id);
        }
    }
    for (interaction, button) in &mut buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match button {
            SharedOrdersButton::LinkSelected => {
                let (Some(vehicle_id), Some(shared_id)) =
                    (state.link_vehicle_id, state.selected_shared_id)
                else {
                    continue;
                };
                match crate::network::apply_player_command(
                    &mut sim.state,
                    &Command::LinkVehicleToSharedOrders {
                        vehicle_id,
                        shared_id,
                    },
                ) {
                    Ok(()) => pending.pending = true,
                    Err(e) => push_build_command_error(&mut hud_feedback, e, time.elapsed_secs()),
                }
            }
        }
    }
}

pub(crate) fn shared_orders_window_on_closed(
    mut closed: MessageReader<FloatingWindowClosed>,
    mut state: ResMut<SharedOrdersWindowState>,
) {
    for message in closed.read() {
        if message.0 == FloatingWindowId::SharedOrders {
            state.open = false;
            state.link_vehicle_id = None;
            state.selected_shared_id = None;
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use openttdrs_core::SharedOrderList;
    use openttdrs_core::prelude::*;

    #[test]
    fn link_selected_pool_sets_shared_order_id() {
        let mut world = World::new();
        let mut state = GameState::new(8, 8);
        state.shared_order_lists.push(SharedOrderList {
            id: 7,
            orders: vec![VehicleOrder::tile(TileCoord::new(1, 1))],
        });
        let mut vehicle = Vehicle::new(
            3,
            VehicleKind::Bus,
            TileCoord::new(0, 0),
            TileCoord::new(0, 0),
        );
        vehicle.orders = vec![VehicleOrder::tile(TileCoord::new(2, 2))];
        state.vehicles.push(vehicle);
        world.insert_resource(SimWorld {
            state,
            ..SimWorld::default()
        });
        world.insert_resource(SharedOrdersWindowState {
            open: true,
            link_vehicle_id: Some(3),
            selected_shared_id: Some(7),
        });
        world.init_resource::<RemapMapVisualsPending>();
        world.init_resource::<HudBuildFeedback>();
        world.insert_resource(Time::<()>::default());
        world.spawn((
            Button,
            SharedOrdersButton::LinkSelected,
            Interaction::Pressed,
        ));
        world.run_system_once(handle_shared_orders_buttons).unwrap();
        assert_eq!(
            world.resource::<SimWorld>().state.vehicles[0].shared_order_id,
            Some(7)
        );
    }
}
