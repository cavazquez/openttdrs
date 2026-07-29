use bevy::prelude::*;
use bevy::ui::FocusPolicy;

use crate::ui::floating_window::{
    TITLE_CRIMSON, WindowKey, spawn_floating_window_keyed, window_text_font,
};
use crate::ui::font::UiFontRole;
use crate::ui::scrollbar::spawn_classic_scroll_area_with;
use crate::ui::toolbar::{BuildMenuUi, OrderPanelButton, OrderPanelRoot};
use crate::ui::vehicle_chain::{MAX_VEHICLE_CHAIN_SLOTS, VehicleChainSlot};

use super::{ORDER_PANEL_LIST_MAX_HEIGHT, ORDER_PANEL_ROWS, OrderPanelRow, OrderPanelRowText};

const BASE_POS: Vec2 = Vec2::new(520.0, 72.0);
const SLOT_OFFSET: Vec2 = Vec2::new(40.0, 40.0);

pub(crate) fn setup_order_panel(mut commands: Commands, asset_server: Res<AssetServer>) {
    let asset_server = &*asset_server;
    for slot in 0..MAX_VEHICLE_CHAIN_SLOTS {
        let slot_u8 = slot as u8;
        let pos = BASE_POS + SLOT_OFFSET * slot as f32;
        let (root, content) = spawn_floating_window_keyed(
            &mut commands,
            asset_server,
            WindowKey {
                class: crate::ui::floating_window::FloatingWindowId::Orders,
                instance: 0,
            },
            "Órdenes",
            TITLE_CRIMSON,
            pos,
            440.0,
        );
        commands
            .entity(root)
            .insert((OrderPanelRoot, FocusPolicy::Block, VehicleChainSlot(slot_u8)));

        spawn_order_panel_content(&mut commands, content, asset_server, slot_u8);
    }
}

fn spawn_order_panel_content(
    commands: &mut Commands,
    content: Entity,
    asset_server: &AssetServer,
    chain_slot: u8,
) {
    let chain = VehicleChainSlot(chain_slot);
    commands.entity(content).with_children(|panel| {
        panel
            .spawn(Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(4.0),
                margin: UiRect::bottom(Val::Px(4.0)),
                ..default()
            })
            .with_children(|row| {
                spawn_order_button(
                    row,
                    asset_server,
                    chain,
                    OrderPanelButton::OpenTimetableWindow,
                    "Horario",
                );
            });
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
                row_gap: Val::Px(1.0),
                padding: UiRect::all(Val::Px(2.0)),
                ..default()
            },
            Color::srgb(0.16, 0.13, 0.09),
            Color::srgb(0.45, 0.39, 0.27),
            (),
            (),
            |list| {
                for row_slot in 0..ORDER_PANEL_ROWS {
                    spawn_order_panel_row(list, asset_server, chain, row_slot);
                }
            },
            ORDER_PANEL_LIST_MAX_HEIGHT,
        );
        panel
            .spawn(Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(4.0),
                ..default()
            })
            .with_children(|row| {
                spawn_order_button(
                    row,
                    asset_server,
                    chain,
                    OrderPanelButton::ToggleFullLoad,
                    "Modo carga",
                );
                spawn_order_button(
                    row,
                    asset_server,
                    chain,
                    OrderPanelButton::ToggleNoUnload,
                    "Modo descarga",
                );
                spawn_order_button(
                    row,
                    asset_server,
                    chain,
                    OrderPanelButton::ToggleDepotStop,
                    "Parar depósito",
                );
                spawn_order_button(
                    row,
                    asset_server,
                    chain,
                    OrderPanelButton::CycleDepotRefit,
                    "Refit orden",
                );
            });
        panel
            .spawn(Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(4.0),
                ..default()
            })
            .with_children(|row| {
                spawn_order_button(row, asset_server, chain, OrderPanelButton::MoveOrderUp, "↑");
                spawn_order_button(row, asset_server, chain, OrderPanelButton::MoveOrderDown, "↓");
                spawn_order_button(
                    row,
                    asset_server,
                    chain,
                    OrderPanelButton::SkipOrder,
                    "Saltarse",
                );
                spawn_order_button(
                    row,
                    asset_server,
                    chain,
                    OrderPanelButton::DeleteSelected,
                    "Eliminar",
                );
                spawn_order_button(
                    row,
                    asset_server,
                    chain,
                    OrderPanelButton::PickDestOnMap,
                    "Ir a",
                );
            });
        panel
            .spawn(Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(4.0),
                ..default()
            })
            .with_children(|row| {
                spawn_order_button(
                    row,
                    asset_server,
                    chain,
                    OrderPanelButton::ShareOrders,
                    "Compartir",
                );
                spawn_order_button(
                    row,
                    asset_server,
                    chain,
                    OrderPanelButton::UnlinkSharedOrders,
                    "Desvincular",
                );
                spawn_order_button(
                    row,
                    asset_server,
                    chain,
                    OrderPanelButton::OpenSharedOrders,
                    "Pools",
                );
            });
        panel
            .spawn(Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(4.0),
                ..default()
            })
            .with_children(|row| {
                spawn_order_button(
                    row,
                    asset_server,
                    chain,
                    OrderPanelButton::AddConditionalAbove,
                    "Cond. >50%",
                );
                spawn_order_button(
                    row,
                    asset_server,
                    chain,
                    OrderPanelButton::AddConditionalBelow,
                    "Cond. <50%",
                );
                spawn_order_button(
                    row,
                    asset_server,
                    chain,
                    OrderPanelButton::CycleConditional,
                    "Ciclar cond.",
                );
            });
    });
}

fn spawn_order_panel_row(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    chain_slot: VehicleChainSlot,
    row_slot: usize,
) {
    parent.spawn((
        Button,
        OrderPanelRow { slot: row_slot },
        chain_slot,
        Node {
            width: Val::Percent(100.0),
            height: Val::Px(22.0),
            padding: UiRect::horizontal(Val::Px(6.0)),
            justify_content: JustifyContent::FlexStart,
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(1.0)),
            display: Display::None,
            ..default()
        },
        BackgroundColor(Color::srgb(0.22, 0.18, 0.12)),
        BorderColor::all(Color::srgb(0.45, 0.39, 0.27)),
        Interaction::default(),
        BuildMenuUi,
        children![(
            OrderPanelRowText { slot: row_slot },
            chain_slot,
            Text::new(""),
            window_text_font(asset_server, UiFontRole::Caption),
            TextColor(Color::srgb(0.92, 0.88, 0.72)),
        )],
    ));
}

fn spawn_order_button(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    chain_slot: VehicleChainSlot,
    action: OrderPanelButton,
    label: &'static str,
) {
    parent.spawn((
        Button,
        action,
        chain_slot,
        Node {
            min_width: Val::Px(78.0),
            flex_grow: 1.0,
            padding: UiRect::horizontal(Val::Px(4.0)),
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
            Text::new(label),
            window_text_font(asset_server, UiFontRole::Caption),
            TextColor(Color::srgb(0.92, 0.88, 0.72)),
        )],
    ));
}
