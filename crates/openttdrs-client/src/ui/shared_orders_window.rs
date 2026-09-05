//! Lista de pools de órdenes compartidas.

use bevy::prelude::*;
use openttdrs_core::Command;

use crate::i18n::{Locale, localized_text};
use crate::render::RemapMapVisualsPending;
use crate::settings::ClientPreferences;
use crate::state::SimWorld;
use crate::ui::floating_window::{
    FloatingWindow, FloatingWindowClosed, FloatingWindowId, FloatingWindowTitleText, TITLE_BROWN,
    WINDOW_TEXT, spawn_floating_window, window_text_font,
};
use crate::ui::font::UiFontRole;
use crate::ui::hud::{HudBuildFeedback, push_build_command_error};
use crate::ui::scrollbar::spawn_classic_scroll_area_with;
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

fn shared_orders_hint(locale: Locale, link_vehicle_id: Option<u32>) -> String {
    match link_vehicle_id {
        Some(id) => match locale {
            Locale::Es => format!("Elige un pool y pulsa Vincular (vehículo #{id})."),
            Locale::En => format!("Choose a pool and press Link (vehicle #{id})."),
        },
        None => localized_text(
            locale,
            "Pools existentes. Abre desde Órdenes → Pools para vincular.",
        ),
    }
}

fn shared_orders_row(locale: Locale, pool_id: u32, order_count: usize, linked: usize) -> String {
    match locale {
        Locale::Es => format!("Pool #{pool_id} · {order_count} órdenes · {linked} vehículos"),
        Locale::En => format!("Pool #{pool_id} · {order_count} orders · {linked} vehicles"),
    }
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
            // El hint se materializa cada frame. Evitar una clave estática
            // impide que el sincronizador genérico pise su estado dinámico.
            Text::new("—"),
            window_text_font(asset_server, UiFontRole::Caption),
            TextColor(WINDOW_TEXT),
        ));
        spawn_classic_scroll_area_with(
            panel,
            asset_server,
            Node {
                flex_grow: 1.0,
                min_width: Val::Px(0.0),
                overflow: Overflow::scroll_y(),
                ..default()
            },
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(2.0),
                ..default()
            },
            BTN_BG,
            BTN_BORDER,
            (),
            (),
            |list| {
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
            },
            220.0,
        );
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
    sync_shared_orders_texts(
        Locale::Es,
        &state,
        &sim,
        &mut title_q,
        &mut hint_q,
        &mut row_text_q,
    );
}

/// Aplica el locale después de que el sincronizador de pools haya materializado
/// sus valores. Mantenerlo separado evita una query ECS de ocho parámetros y
/// garantiza que un hint dinámico no sea reemplazado por el catálogo estático.
pub(crate) fn sync_shared_orders_locale(
    state: Res<SharedOrdersWindowState>,
    sim: Res<SimWorld>,
    prefs: Res<ClientPreferences>,
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
    mut row_text_q: Query<
        (&SharedOrdersRowText, &mut Text),
        (
            Without<FloatingWindowTitleText>,
            Without<SharedOrdersHintText>,
        ),
    >,
) {
    if !state.open {
        return;
    }
    sync_shared_orders_texts(
        prefs.locale(),
        &state,
        &sim,
        &mut title_q,
        &mut hint_q,
        &mut row_text_q,
    );
}

fn sync_shared_orders_texts(
    locale: Locale,
    state: &SharedOrdersWindowState,
    sim: &SimWorld,
    title_q: &mut Query<
        (&FloatingWindowTitleText, &mut Text),
        (Without<SharedOrdersHintText>, Without<SharedOrdersRowText>),
    >,
    hint_q: &mut Query<
        &mut Text,
        (
            With<SharedOrdersHintText>,
            Without<FloatingWindowTitleText>,
            Without<SharedOrdersRowText>,
        ),
    >,
    row_text_q: &mut Query<
        (&SharedOrdersRowText, &mut Text),
        (
            Without<FloatingWindowTitleText>,
            Without<SharedOrdersHintText>,
        ),
    >,
) {
    if let Some((_, mut title)) = title_q
        .iter_mut()
        .find(|(text, _)| text.0 == FloatingWindowId::SharedOrders)
    {
        **title = localized_text(locale, "Órdenes compartidas");
    }
    if let Ok(mut hint) = hint_q.single_mut() {
        **hint = shared_orders_hint(locale, state.link_vehicle_id);
    }
    let pools = &sim.state.shared_order_lists;
    for (row_text, mut text) in row_text_q.iter_mut() {
        if let Some(pool) = pools.get(row_text.slot) {
            let linked = sim
                .state
                .vehicles
                .iter()
                .filter(|v| v.shared_order_id == Some(pool.id))
                .count();
            **text = shared_orders_row(locale, pool.id, pool.orders.len(), linked);
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
        if message.0.class == FloatingWindowId::SharedOrders {
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

    use crate::settings::ClientPreferences;
    use crate::ui::floating_window::WindowKey;

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

    #[test]
    fn shared_order_labels_follow_the_live_locale() {
        let mut world = World::new();
        let mut state = GameState::new(8, 8);
        state.shared_order_lists.push(SharedOrderList {
            id: 7,
            orders: vec![
                VehicleOrder::tile(TileCoord::new(1, 1)),
                VehicleOrder::tile(TileCoord::new(2, 2)),
            ],
        });
        world.insert_resource(SimWorld {
            state,
            ..SimWorld::default()
        });
        world.insert_resource(SharedOrdersWindowState {
            open: true,
            link_vehicle_id: Some(29),
            ..SharedOrdersWindowState::default()
        });
        world.insert_resource(ClientPreferences {
            language: "en".into(),
            ..ClientPreferences::default()
        });
        world.spawn((
            FloatingWindow {
                id: FloatingWindowId::SharedOrders,
                key: WindowKey::singleton(FloatingWindowId::SharedOrders),
            },
            Visibility::Hidden,
        ));
        let title = world
            .spawn((
                FloatingWindowTitleText(FloatingWindowId::SharedOrders),
                Text::new("—"),
            ))
            .id();
        let hint = world.spawn((SharedOrdersHintText, Text::new("—"))).id();
        let row = world
            .spawn((SharedOrdersRowText { slot: 0 }, Text::new("—")))
            .id();

        world.run_system_once(sync_shared_orders_window).unwrap();
        world.run_system_once(sync_shared_orders_locale).unwrap();
        assert_eq!(
            world.entity(title).get::<Text>().unwrap().as_str(),
            "Shared orders"
        );
        assert_eq!(
            world.entity(hint).get::<Text>().unwrap().as_str(),
            "Choose a pool and press Link (vehicle #29)."
        );
        assert_eq!(
            world.entity(row).get::<Text>().unwrap().as_str(),
            "Pool #7 · 2 orders · 0 vehicles"
        );

        world.resource_mut::<ClientPreferences>().language = "es-AR".into();
        world.run_system_once(sync_shared_orders_window).unwrap();
        world.run_system_once(sync_shared_orders_locale).unwrap();
        assert_eq!(
            world.entity(title).get::<Text>().unwrap().as_str(),
            "Órdenes compartidas"
        );
        assert_eq!(
            world.entity(hint).get::<Text>().unwrap().as_str(),
            "Elige un pool y pulsa Vincular (vehículo #29)."
        );
        assert_eq!(
            world.entity(row).get::<Text>().unwrap().as_str(),
            "Pool #7 · 2 órdenes · 0 vehículos"
        );
    }
}
