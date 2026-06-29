use bevy::prelude::*;
use bevy::ui::FocusPolicy;

use super::super::{
    BuildMenuAction, BuildMenuUi, ToolButtonGroup, ToolbarGroup, ToolbarGroupButton,
    ToolbarTooltipTarget, TooltipBox, TooltipText,
};
use super::controls::{spawn_icon_tool_buttons, spawn_panel_title, spawn_settings_buttons};
use crate::ui::save_window::{SaveLoadToolbarButton, SaveWindowMode};

pub(super) fn spawn_toolbar_group_buttons(
    root: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
) {
    root.spawn((
        Node {
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(2.0),
            padding: UiRect::axes(Val::Px(8.0), Val::Px(5.0)),
            border: UiRect::all(Val::Px(3.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.2, 0.17, 0.12, 0.96)),
        BorderColor::all(Color::srgb(0.68, 0.61, 0.42)),
        FocusPolicy::Block,
        BuildMenuUi,
        Interaction::default(),
    ))
    .with_children(|parent| {
        for (i, icon_path, group) in [
            (
                0_u8,
                "assets/opengfx/tiles/rail_1005.png",
                ToolbarGroup::Rail,
            ),
            (
                1,
                "assets/opengfx/tiles/road_flat_00.png",
                ToolbarGroup::Road,
            ),
            (
                2,
                "assets/opengfx/tiles/house_church_build.png",
                ToolbarGroup::Economy,
            ),
            (
                3,
                "assets/opengfx/tiles/ui_terraform_up.png",
                ToolbarGroup::Landscape,
            ),
            (
                4,
                "assets/opengfx/tiles/object_lighthouse.png",
                ToolbarGroup::Info,
            ),
            (
                5,
                "assets/opengfx/tiles/object_transmitter.png",
                ToolbarGroup::Settings,
            ),
        ] {
            parent
                .spawn((
                    Button,
                    group,
                    ToolbarGroupButton,
                    ToolbarTooltipTarget {
                        text: match group {
                            ToolbarGroup::Rail => "Ferrocarriles",
                            ToolbarGroup::Road => "Carreteras",
                            ToolbarGroup::Economy => "Economia",
                            ToolbarGroup::Landscape => "Paisaje",
                            ToolbarGroup::Info => "Informacion",
                            ToolbarGroup::Settings => "Ajustes",
                        },
                    },
                    BuildMenuUi,
                    Node {
                        width: Val::Px(48.0),
                        height: Val::Px(48.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        border: UiRect::all(Val::Px(2.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.33, 0.28, 0.19)),
                    BorderColor::all(Color::srgb(0.64, 0.57, 0.39)),
                    Interaction::default(),
                ))
                .with_children(|p| {
                    p.spawn((
                        ImageNode::new(asset_server.load::<Image>(icon_path)),
                        Node {
                            width: Val::Px(32.0),
                            height: Val::Px(32.0),
                            padding: UiRect::all(Val::Px(1.0)),
                            ..default()
                        },
                    ));
                });
            if i < 5 {
                parent.spawn((
                    Node {
                        width: Val::Px(2.0),
                        height: Val::Px(40.0),
                        margin: UiRect::horizontal(Val::Px(2.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.62, 0.55, 0.38)),
                    BuildMenuUi,
                ));
            }
        }
        parent.spawn((
            Node {
                width: Val::Px(2.0),
                height: Val::Px(40.0),
                margin: UiRect::horizontal(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(Color::srgb(0.62, 0.55, 0.38)),
            BuildMenuUi,
        ));
        spawn_save_load_button(
            parent,
            SaveWindowMode::Save,
            "Guardar",
            "Guardar partida (ventana de partidas)",
        );
        spawn_save_load_button(
            parent,
            SaveWindowMode::Load,
            "Cargar",
            "Cargar partida (ventana de partidas)",
        );
    });
}

fn spawn_save_load_button(
    parent: &mut ChildSpawnerCommands,
    mode: SaveWindowMode,
    label: &'static str,
    tip: &'static str,
) {
    parent.spawn((
        Button,
        SaveLoadToolbarButton(mode),
        ToolbarTooltipTarget { text: tip },
        BuildMenuUi,
        Node {
            width: Val::Px(72.0),
            height: Val::Px(48.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(2.0)),
            ..default()
        },
        BackgroundColor(Color::srgb(0.33, 0.28, 0.19)),
        BorderColor::all(Color::srgb(0.64, 0.57, 0.39)),
        Interaction::default(),
        children![(
            Text::new(label),
            TextFont {
                font_size: FontSize::Rem(0.85),
                ..default()
            },
            TextColor(Color::srgb(0.95, 0.92, 0.78)),
        )],
    ));
}

pub(super) fn spawn_road_panel(root: &mut ChildSpawnerCommands, asset_server: &AssetServer) {
    root.spawn(tool_panel_node(ToolbarGroup::Road, false))
        .with_children(|panel| {
            spawn_panel_title(panel, "Construccion de carretera", 504.0);
            spawn_button_row(panel, |buttons| {
                spawn_icon_tool_buttons(
                    buttons,
                    asset_server,
                    &[
                        (
                            "Carretera NW-SE",
                            "assets/opengfx/tiles/road_flat_00.png",
                            BuildMenuAction::RoadY,
                        ),
                        (
                            "Carretera NE-SW",
                            "assets/opengfx/tiles/road_flat_01.png",
                            BuildMenuAction::RoadX,
                        ),
                        (
                            "Cruce de carretera",
                            "assets/opengfx/tiles/road_flat_02.png",
                            BuildMenuAction::Road,
                        ),
                        (
                            "Deposito de carretera",
                            "assets/opengfx/tiles/rail_1412.png",
                            BuildMenuAction::RoadDepot,
                        ),
                        (
                            "Puente de carretera",
                            "assets/opengfx/tiles/bridge_wood_road_x.png",
                            BuildMenuAction::RoadBridge,
                        ),
                        (
                            "Tunel de carretera",
                            "assets/opengfx/tiles/tunnel_road_rear.png",
                            BuildMenuAction::RoadTunnel,
                        ),
                        (
                            "Demoler",
                            "assets/opengfx/tiles/ui_demolish.png",
                            BuildMenuAction::Clear,
                        ),
                        (
                            "Estacion",
                            "assets/opengfx/tiles/truck_stop_ground_0.png",
                            BuildMenuAction::Station,
                        ),
                        (
                            "Parada de bus",
                            "assets/opengfx/tiles/bus_stop_ne_ground.png",
                            BuildMenuAction::BusStop,
                        ),
                    ],
                );
            });
        });
}

pub(super) fn spawn_rail_panel(root: &mut ChildSpawnerCommands, asset_server: &AssetServer) {
    root.spawn(tool_panel_node(ToolbarGroup::Rail, false))
        .with_children(|panel| {
            spawn_panel_title(panel, "Construccion de Ferrocarril", 770.0);
            spawn_button_row(panel, |buttons| {
                // Mismo orden que `_nested_build_rail_widgets` (rail_gui.cpp):
                // 4 direcciones + autorail | separador | resto de herramientas.
                spawn_icon_tool_buttons(
                    buttons,
                    asset_server,
                    &[
                        (
                            "Construir via N-S",
                            "assets/opengfx/tiles/toolbar_rail_rail_ns.png",
                            BuildMenuAction::RailVert,
                        ),
                        (
                            "Construir via NE-SW",
                            "assets/opengfx/tiles/toolbar_rail_rail_x.png",
                            BuildMenuAction::RailX,
                        ),
                        (
                            "Construir via E-O",
                            "assets/opengfx/tiles/toolbar_rail_rail_ew.png",
                            BuildMenuAction::RailHorz,
                        ),
                        (
                            "Construir via NW-SE",
                            "assets/opengfx/tiles/toolbar_rail_rail_y.png",
                            BuildMenuAction::RailY,
                        ),
                        (
                            "Autorail (direccion automatica)",
                            "assets/opengfx/tiles/toolbar_rail_autorail.png",
                            BuildMenuAction::Rail,
                        ),
                    ],
                );
                spawn_toolbar_spacer(buttons);
                spawn_icon_tool_buttons(
                    buttons,
                    asset_server,
                    &[
                        (
                            "Demoler",
                            "assets/opengfx/tiles/toolbar_rail_demolish.png",
                            BuildMenuAction::Clear,
                        ),
                        (
                            "Deposito ferroviario",
                            "assets/opengfx/tiles/toolbar_rail_depot.png",
                            BuildMenuAction::RailDepot,
                        ),
                        (
                            "Waypoint",
                            "assets/opengfx/tiles/toolbar_rail_waypoint.png",
                            BuildMenuAction::RailWaypoint,
                        ),
                        (
                            "Estacion de tren",
                            "assets/opengfx/tiles/toolbar_rail_station.png",
                            BuildMenuAction::RailStation,
                        ),
                        (
                            "Señales de bloque",
                            "assets/opengfx/tiles/toolbar_rail_signals.png",
                            BuildMenuAction::RailSignals,
                        ),
                        (
                            "Puente ferroviario",
                            "assets/opengfx/tiles/toolbar_rail_bridge.png",
                            BuildMenuAction::RailBridge,
                        ),
                        (
                            "Tunel ferroviario",
                            "assets/opengfx/tiles/toolbar_rail_tunnel.png",
                            BuildMenuAction::RailTunnel,
                        ),
                        (
                            "Quitar vía",
                            "assets/opengfx/tiles/toolbar_rail_remove.png",
                            BuildMenuAction::RailRemove,
                        ),
                    ],
                );
            });
        });
}

/// Separador vertical entre grupos de botones (panel oscuro en upstream).
fn spawn_toolbar_spacer(buttons: &mut ChildSpawnerCommands) {
    buttons.spawn((
        Node {
            width: Val::Px(8.0),
            height: Val::Px(48.0),
            ..default()
        },
        BackgroundColor(Color::srgb(0.36, 0.47, 0.26)),
        BuildMenuUi,
    ));
}

pub(super) fn spawn_secondary_tool_panels(
    root: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
) {
    for group in [
        ToolbarGroup::Economy,
        ToolbarGroup::Landscape,
        ToolbarGroup::Info,
        ToolbarGroup::Settings,
    ] {
        root.spawn(secondary_panel_node(group))
            .with_children(|buttons| match group {
                ToolbarGroup::Economy => spawn_icon_tool_buttons(
                    buttons,
                    asset_server,
                    &[
                        (
                            "Construir casa",
                            "assets/opengfx/tiles/house_church_build.png",
                            BuildMenuAction::BuildHouse,
                        ),
                        (
                            "Mina de carbón",
                            "assets/opengfx/tiles/industry_2013.png",
                            BuildMenuAction::BuildCoalMine,
                        ),
                        (
                            "Mina de hierro",
                            "assets/opengfx/tiles/industry_2092.png",
                            BuildMenuAction::BuildIronOreMine,
                        ),
                        (
                            "Mina de oro",
                            "assets/opengfx/tiles/industry_2247.png",
                            BuildMenuAction::BuildGoldMine,
                        ),
                        (
                            "Pozo petrolero",
                            "assets/opengfx/tiles/industry_2028.png",
                            BuildMenuAction::BuildOilWell,
                        ),
                        (
                            "Refinería",
                            "assets/opengfx/tiles/industry_2047.png",
                            BuildMenuAction::BuildOilRefinery,
                        ),
                        (
                            "Fábrica",
                            "assets/opengfx/tiles/industry_2169.png",
                            BuildMenuAction::BuildFactory,
                        ),
                        (
                            "Aserradero",
                            "assets/opengfx/tiles/industry_2063.png",
                            BuildMenuAction::BuildSawmill,
                        ),
                        (
                            "Plantar bosque",
                            "assets/opengfx/tiles/tree_01.png",
                            BuildMenuAction::BuildForest,
                        ),
                        (
                            "Granja",
                            "assets/opengfx/tiles/industry_2190.png",
                            BuildMenuAction::BuildFarm,
                        ),
                    ],
                ),
                ToolbarGroup::Landscape => spawn_icon_tool_buttons(
                    buttons,
                    asset_server,
                    &[
                        (
                            "Elevar terreno",
                            "assets/opengfx/tiles/ui_terraform_up.png",
                            BuildMenuAction::RaiseLand,
                        ),
                        (
                            "Bajar terreno",
                            "assets/opengfx/tiles/ui_terraform_down.png",
                            BuildMenuAction::LowerLand,
                        ),
                        (
                            "Nivelar terreno",
                            "assets/opengfx/tiles/ui_terraform_level.png",
                            BuildMenuAction::LevelLand,
                        ),
                        (
                            "Comprar terreno",
                            "assets/opengfx/tiles/object_bought_land.png",
                            BuildMenuAction::BuyLand,
                        ),
                    ],
                ),
                ToolbarGroup::Info => spawn_icon_tool_buttons(
                    buttons,
                    asset_server,
                    &[(
                        "Editar ordenes",
                        "assets/opengfx/tiles/object_lighthouse.png",
                        BuildMenuAction::Orders,
                    )],
                ),
                ToolbarGroup::Settings => spawn_settings_buttons(buttons),
                ToolbarGroup::Rail | ToolbarGroup::Road => {}
            });
    }
}

pub(super) fn spawn_toolbar_tooltip(root: &mut ChildSpawnerCommands) {
    root.spawn((
        Node {
            padding: UiRect::axes(Val::Px(6.0), Val::Px(3.0)),
            border: UiRect::all(Val::Px(1.0)),
            display: Display::None,
            ..default()
        },
        BackgroundColor(Color::srgba(0.15, 0.12, 0.08, 0.97)),
        BorderColor::all(Color::srgb(0.8, 0.72, 0.5)),
        BuildMenuUi,
        TooltipBox,
        children![(
            TooltipText,
            Text::new(""),
            TextFont {
                font_size: FontSize::Rem(0.7),
                ..default()
            },
            TextColor(Color::srgb(0.94, 0.9, 0.76)),
        )],
    ));
}

fn spawn_button_row(
    panel: &mut ChildSpawnerCommands,
    spawn_buttons: impl FnOnce(&mut ChildSpawnerCommands),
) {
    panel
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(1.0),
                ..default()
            },
            BuildMenuUi,
        ))
        .with_children(spawn_buttons);
}

fn tool_panel_node(group: ToolbarGroup, hidden: bool) -> impl Bundle {
    (
        Node {
            flex_direction: FlexDirection::Column,
            column_gap: Val::Px(1.0),
            row_gap: Val::Px(1.0),
            padding: UiRect::all(Val::Px(1.0)),
            border: UiRect::all(Val::Px(1.0)),
            display: if hidden { Display::None } else { Display::Flex },
            ..default()
        },
        BackgroundColor(Color::srgb(0.44, 0.57, 0.31)),
        BorderColor::all(Color::srgb(0.18, 0.25, 0.12)),
        FocusPolicy::Block,
        BuildMenuUi,
        ToolButtonGroup(group),
        Interaction::default(),
    )
}

fn secondary_panel_node(group: ToolbarGroup) -> impl Bundle {
    (
        Node {
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(1.0),
            padding: UiRect::all(Val::Px(1.0)),
            border: UiRect::all(Val::Px(1.0)),
            display: Display::None,
            ..default()
        },
        BackgroundColor(Color::srgb(0.44, 0.57, 0.31)),
        BorderColor::all(Color::srgb(0.18, 0.25, 0.12)),
        FocusPolicy::Block,
        BuildMenuUi,
        ToolButtonGroup(group),
        Interaction::default(),
    )
}
