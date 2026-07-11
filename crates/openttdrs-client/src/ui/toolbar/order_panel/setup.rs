use bevy::prelude::*;
use bevy::ui::{FocusPolicy, GlobalZIndex};

use crate::ui::toolbar::{BuildMenuUi, OrderPanelButton, OrderPanelRoot, OrderPanelTitle};

use super::{ORDER_PANEL_LIST_MAX_HEIGHT, ORDER_PANEL_ROWS, OrderPanelRow, OrderPanelRowText};

const UI_FONT: &str = "static/fonts/DejaVuSansMono.ttf";
const HEADER_BG: Color = Color::srgb(0.55, 0.18, 0.32);

pub(crate) fn setup_order_panel(mut commands: Commands, asset_server: Res<AssetServer>) {
    let ui_font = asset_server.load::<Font>(UI_FONT);

    commands
        .spawn((
            OrderPanelRoot,
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(20.0),
                top: Val::Px(72.0),
                width: Val::Px(440.0),
                padding: UiRect::all(Val::Px(4.0)),
                border: UiRect::all(Val::Px(2.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(4.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.13, 0.1, 0.07, 0.97)),
            BorderColor::all(Color::srgb(0.75, 0.67, 0.45)),
            GlobalZIndex(2200),
            Visibility::Hidden,
            BuildMenuUi,
            FocusPolicy::Block,
            // Captura el clic en áreas vacías (lista/fondo) para que no
            // atraviese la ventana hacia el mapa (`handle_tile_click`).
            Interaction::default(),
        ))
        .with_children(|panel| {
            spawn_header(panel, &ui_font);
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
                        spawn_order_panel_row(list, slot);
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
                    spawn_order_button(row, OrderPanelButton::ToggleFullLoad, "Carga completa");
                    spawn_order_button(row, OrderPanelButton::ToggleNoUnload, "Descargar todo");
                    spawn_order_button(row, OrderPanelButton::ToggleDepotStop, "Parar depósito");
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
                    spawn_order_button(row, OrderPanelButton::MoveOrderUp, "↑");
                    spawn_order_button(row, OrderPanelButton::MoveOrderDown, "↓");
                    spawn_order_button(row, OrderPanelButton::SkipOrder, "Saltarse");
                    spawn_order_button(row, OrderPanelButton::DeleteSelected, "Eliminar");
                    spawn_order_button(row, OrderPanelButton::PickDestOnMap, "Ir a");
                });
            panel
                .spawn(Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(4.0),
                    ..default()
                })
                .with_children(|row| {
                    spawn_order_button(row, OrderPanelButton::ShareOrders, "Compartir");
                    spawn_order_button(row, OrderPanelButton::UnlinkSharedOrders, "Desvincular");
                    spawn_order_button(row, OrderPanelButton::OpenSharedOrders, "Pools");
                });
            panel
                .spawn(Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(4.0),
                    ..default()
                })
                .with_children(|row| {
                    spawn_order_button(row, OrderPanelButton::AddConditionalAbove, "Cond. >50%");
                    spawn_order_button(row, OrderPanelButton::AddConditionalBelow, "Cond. <50%");
                    spawn_order_button(row, OrderPanelButton::CycleConditional, "Ciclar cond.");
                });
        });
}

fn spawn_header(panel: &mut ChildSpawnerCommands, ui_font: &Handle<Font>) {
    panel
        .spawn((
            Node {
                width: Val::Percent(100.0),
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(6.0),
                padding: UiRect::horizontal(Val::Px(4.0)),
                height: Val::Px(24.0),
                ..default()
            },
            BackgroundColor(HEADER_BG),
            BuildMenuUi,
        ))
        .with_children(|row| {
            row.spawn((
                OrderPanelTitle,
                Text::new("Órdenes"),
                TextFont {
                    font: ui_font.clone().into(),
                    font_size: FontSize::Rem(0.85),
                    ..default()
                },
                TextColor(Color::srgb(0.98, 0.95, 0.85)),
                BuildMenuUi,
            ));
            row.spawn((
                Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(4.0),
                    align_items: AlignItems::Center,
                    ..default()
                },
                BuildMenuUi,
            ))
            .with_children(|actions| {
                spawn_order_button(actions, OrderPanelButton::OpenTimetableWindow, "Horario");
                actions
                    .spawn((
                        OrderPanelButton::Close,
                        Button,
                        Node {
                            width: Val::Px(24.0),
                            height: Val::Px(20.0),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.42, 0.36, 0.24)),
                        BorderColor::all(Color::srgb(0.7, 0.62, 0.42)),
                        Interaction::default(),
                        BuildMenuUi,
                    ))
                    .with_children(|b| {
                        b.spawn((
                            Text::new("✕"),
                            TextFont {
                                font: ui_font.clone().into(),
                                font_size: FontSize::Rem(0.8),
                                ..default()
                            },
                            TextColor(Color::srgb(0.92, 0.88, 0.78)),
                        ));
                    });
            });
        });
}

fn spawn_order_panel_row(parent: &mut ChildSpawnerCommands, slot: usize) {
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
            TextFont {
                font_size: FontSize::Rem(0.7),
                ..default()
            },
            TextColor(Color::srgb(0.92, 0.88, 0.72)),
        )],
    ));
}

fn spawn_order_button(
    parent: &mut ChildSpawnerCommands,
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
            TextFont {
                font_size: FontSize::Rem(0.7),
                ..default()
            },
            TextColor(Color::srgb(0.92, 0.88, 0.72)),
        )],
    ));
}
