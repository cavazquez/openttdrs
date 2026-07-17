use bevy::prelude::*;
use bevy::ui::FocusPolicy;

use crate::ui::floating_window::{TITLE_CRIMSON, spawn_floating_window, window_text_font};
use crate::ui::font::UiFontRole;
use crate::ui::toolbar::{BuildMenuUi, OrderPanelButton, OrderPanelRoot};

use super::{ORDER_PANEL_LIST_MAX_HEIGHT, ORDER_PANEL_ROWS, OrderPanelRow, OrderPanelRowText};

pub(crate) fn setup_order_panel(mut commands: Commands, asset_server: Res<AssetServer>) {
    let asset_server = &*asset_server;
    let (root, content) = spawn_floating_window(
        &mut commands,
        asset_server,
        crate::ui::floating_window::FloatingWindowId::Orders,
        "Órdenes",
        TITLE_CRIMSON,
        Vec2::new(520.0, 72.0),
        440.0,
    );
    commands
        .entity(root)
        .insert((OrderPanelRoot, FocusPolicy::Block));

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
                    OrderPanelButton::OpenTimetableWindow,
                    "Horario",
                );
            });
        // Lista/tabla de órdenes cargadas.
        panel
            .spawn((
                Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(1.0),
                    min_height: Val::Px(120.0),
                    max_height: Val::Px(ORDER_PANEL_LIST_MAX_HEIGHT),
                    overflow: Overflow::scroll_y(),
                    padding: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.16, 0.13, 0.09)),
                BuildMenuUi,
            ))
            .with_children(|list| {
                for slot in 0..ORDER_PANEL_ROWS {
                    spawn_order_panel_row(list, asset_server, slot);
                }
            });
        // Fila de modos de la orden seleccionada (carga / descarga).
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
                    OrderPanelButton::ToggleFullLoad,
                    "Carga completa",
                );
                spawn_order_button(
                    row,
                    asset_server,
                    OrderPanelButton::ToggleNoUnload,
                    "Descargar todo",
                );
                spawn_order_button(
                    row,
                    asset_server,
                    OrderPanelButton::ToggleDepotStop,
                    "Parar depósito",
                );
                spawn_order_button(
                    row,
                    asset_server,
                    OrderPanelButton::CycleDepotRefit,
                    "Refit orden",
                );
            });
        // Fila de acciones (saltarse / eliminar / ir a).
        panel
            .spawn(Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(4.0),
                ..default()
            })
            .with_children(|row| {
                spawn_order_button(row, asset_server, OrderPanelButton::MoveOrderUp, "↑");
                spawn_order_button(row, asset_server, OrderPanelButton::MoveOrderDown, "↓");
                spawn_order_button(row, asset_server, OrderPanelButton::SkipOrder, "Saltarse");
                spawn_order_button(
                    row,
                    asset_server,
                    OrderPanelButton::DeleteSelected,
                    "Eliminar",
                );
                spawn_order_button(row, asset_server, OrderPanelButton::PickDestOnMap, "Ir a");
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
                    OrderPanelButton::ShareOrders,
                    "Compartir",
                );
                spawn_order_button(
                    row,
                    asset_server,
                    OrderPanelButton::UnlinkSharedOrders,
                    "Desvincular",
                );
                spawn_order_button(
                    row,
                    asset_server,
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
                    OrderPanelButton::AddConditionalAbove,
                    "Cond. >50%",
                );
                spawn_order_button(
                    row,
                    asset_server,
                    OrderPanelButton::AddConditionalBelow,
                    "Cond. <50%",
                );
                spawn_order_button(
                    row,
                    asset_server,
                    OrderPanelButton::CycleConditional,
                    "Ciclar cond.",
                );
            });
    });
}

fn spawn_order_panel_row(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    slot: usize,
) {
    parent.spawn((
        Button,
        OrderPanelRow { slot },
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
            OrderPanelRowText { slot },
            Text::new(""),
            window_text_font(asset_server, UiFontRole::Caption),
            TextColor(Color::srgb(0.92, 0.88, 0.72)),
        )],
    ));
}

fn spawn_order_button(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    action: OrderPanelButton,
    label: &'static str,
) {
    parent.spawn((
        Button,
        action,
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
