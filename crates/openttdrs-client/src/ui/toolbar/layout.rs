use bevy::prelude::*;
use bevy::ui::FocusPolicy;

use super::{
    BuildMenuAction, BuildMenuUi, SaveMenuAction, ToolButtonGroup, ToolSelectButton,
    ToolbarCloseButton, ToolbarGroup, ToolbarGroupButton, ToolbarTooltipTarget, TooltipBox,
    TooltipText,
};

/// Barra superior compacta tipo toolbar para seleccion rapida de herramienta.
pub(crate) fn setup_top_toolbar(mut commands: Commands, asset_server: Res<AssetServer>) {
    let root = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(10.0),
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(2.0),
                ..default()
            },
            BuildMenuUi,
            GlobalZIndex(2100),
        ))
        .id();

    commands.entity(root).with_children(|root| {
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
                (0_u8, "opengfx/tiles/rail_1005.png", ToolbarGroup::Rail),
                (1, "opengfx/tiles/road_flat_00.png", ToolbarGroup::Road),
                (
                    2,
                    "opengfx/tiles/house_church_build.png",
                    ToolbarGroup::Economy,
                ),
                (3, "opengfx/tiles/object_lighthouse.png", ToolbarGroup::Info),
                (
                    4,
                    "opengfx/tiles/object_transmitter.png",
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
                if i < 4 {
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
        });

        root.spawn((
            Node {
                flex_direction: FlexDirection::Column,
                column_gap: Val::Px(1.0),
                row_gap: Val::Px(1.0),
                padding: UiRect::all(Val::Px(1.0)),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(Color::srgb(0.44, 0.57, 0.31)),
            BorderColor::all(Color::srgb(0.18, 0.25, 0.12)),
            FocusPolicy::Block,
            BuildMenuUi,
            ToolButtonGroup(ToolbarGroup::Road),
            Interaction::default(),
        ))
        .with_children(|panel| {
            spawn_panel_title(panel, "Construccion de carretera", 504.0);

            panel
                .spawn((
                    Node {
                        flex_direction: FlexDirection::Row,
                        column_gap: Val::Px(1.0),
                        ..default()
                    },
                    BuildMenuUi,
                ))
                .with_children(|buttons| {
                    spawn_icon_tool_buttons(
                        buttons,
                        &asset_server,
                        &[
                            (
                                "Carretera NW-SE",
                                "opengfx/tiles/road_flat_00.png",
                                BuildMenuAction::RoadY,
                            ),
                            (
                                "Carretera NE-SW",
                                "opengfx/tiles/road_flat_01.png",
                                BuildMenuAction::RoadX,
                            ),
                            (
                                "Cruce de carretera",
                                "opengfx/tiles/road_flat_02.png",
                                BuildMenuAction::Road,
                            ),
                            (
                                "Deposito de carretera",
                                "opengfx/tiles/road_depot_0.png",
                                BuildMenuAction::RoadDepot,
                            ),
                            (
                                "Puente de carretera",
                                "opengfx/tiles/bridge_wood_road_x.png",
                                BuildMenuAction::RoadBridge,
                            ),
                            (
                                "Tunel de carretera",
                                "opengfx/tiles/tunnel_road_rear.png",
                                BuildMenuAction::RoadTunnel,
                            ),
                            ("Demoler", "text:💣", BuildMenuAction::Clear),
                            (
                                "Estacion",
                                "opengfx/tiles/truck_stop_ground_0.png",
                                BuildMenuAction::Station,
                            ),
                        ],
                    );
                });
        });

        root.spawn((
            Node {
                flex_direction: FlexDirection::Column,
                column_gap: Val::Px(1.0),
                row_gap: Val::Px(1.0),
                padding: UiRect::all(Val::Px(1.0)),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(Color::srgb(0.44, 0.57, 0.31)),
            BorderColor::all(Color::srgb(0.18, 0.25, 0.12)),
            FocusPolicy::Block,
            BuildMenuUi,
            ToolButtonGroup(ToolbarGroup::Rail),
            Interaction::default(),
        ))
        .with_children(|panel| {
            spawn_panel_title(panel, "Construccion ferroviaria", 392.0);
            panel
                .spawn((
                    Node {
                        flex_direction: FlexDirection::Row,
                        column_gap: Val::Px(1.0),
                        ..default()
                    },
                    BuildMenuUi,
                ))
                .with_children(|buttons| {
                    spawn_icon_tool_buttons(
                        buttons,
                        &asset_server,
                        &[
                            (
                                "Construir via",
                                "opengfx/tiles/rail_1005.png",
                                BuildMenuAction::Rail,
                            ),
                            (
                                "Deposito ferroviario",
                                "opengfx/tiles/rail_depot_ne.png",
                                BuildMenuAction::RailDepot,
                            ),
                            (
                                "Puente ferroviario",
                                "opengfx/tiles/bridge_wood_rail_x.png",
                                BuildMenuAction::RailBridge,
                            ),
                            (
                                "Tunel ferroviario",
                                "opengfx/tiles/tunnel_rail_rear.png",
                                BuildMenuAction::RailTunnel,
                            ),
                        ],
                    );
                });
        });

        for group in [
            ToolbarGroup::Economy,
            ToolbarGroup::Info,
            ToolbarGroup::Settings,
        ] {
            root.spawn((
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
            ))
            .with_children(|buttons| match group {
                ToolbarGroup::Economy => spawn_icon_tool_buttons(
                    buttons,
                    &asset_server,
                    &[
                        (
                            "Construir casa",
                            "opengfx/tiles/house_church_build.png",
                            BuildMenuAction::BuildHouse,
                        ),
                        (
                            "Mina de carbon",
                            "opengfx/tiles/industry_2013.png",
                            BuildMenuAction::BuildCoalMine,
                        ),
                        (
                            "Pozo petrolero",
                            "opengfx/tiles/industry_2028.png",
                            BuildMenuAction::BuildOilWell,
                        ),
                        (
                            "Fabrica",
                            "opengfx/tiles/industry_2169.png",
                            BuildMenuAction::BuildFactory,
                        ),
                        (
                            "Plantar bosque",
                            "opengfx/tiles/tree_01.png",
                            BuildMenuAction::BuildForest,
                        ),
                    ],
                ),
                ToolbarGroup::Info => spawn_icon_tool_buttons(
                    buttons,
                    &asset_server,
                    &[(
                        "Editar ordenes",
                        "opengfx/tiles/object_lighthouse.png",
                        BuildMenuAction::Orders,
                    )],
                ),
                ToolbarGroup::Settings => {
                    spawn_settings_buttons(buttons);
                }
                ToolbarGroup::Rail | ToolbarGroup::Road => {}
            });
        }

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
                    font_size: 11.0,
                    ..default()
                },
                TextColor(Color::srgb(0.94, 0.9, 0.76)),
            )],
        ));
    });
}

fn spawn_panel_title(parent: &mut ChildSpawnerCommands, title: &'static str, width: f32) {
    parent
        .spawn((
            Node {
                height: Val::Px(16.0),
                width: Val::Px(width),
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::FlexStart,
                align_items: AlignItems::Center,
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(Color::srgb(0.36, 0.52, 0.28)),
            BorderColor::all(Color::srgb(0.73, 0.84, 0.55)),
            BuildMenuUi,
        ))
        .with_children(|title_bar| {
            title_bar.spawn((
                Node {
                    width: Val::Px(15.0),
                    height: Val::Px(15.0),
                    ..default()
                },
                BuildMenuUi,
            ));
            title_bar.spawn((
                Node {
                    flex_grow: 1.0,
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                BuildMenuUi,
                children![(
                    Text::new(title),
                    TextFont {
                        font_size: 12.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.95, 0.96, 0.82)),
                )],
            ));
            title_bar.spawn((
                Button,
                ToolbarCloseButton,
                ToolbarTooltipTarget { text: "Cerrar" },
                BuildMenuUi,
                Node {
                    width: Val::Px(15.0),
                    height: Val::Px(15.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                BackgroundColor(Color::srgb(0.2, 0.31, 0.14)),
                BorderColor::all(Color::srgb(0.12, 0.16, 0.09)),
                Interaction::default(),
                children![(
                    Text::new("X"),
                    TextFont {
                        font_size: 12.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.02, 0.03, 0.02)),
                )],
            ));
        });
}

fn spawn_icon_tool_buttons(
    buttons: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    defs: &[(&'static str, &'static str, BuildMenuAction)],
) {
    for (tip, icon_path, action) in defs {
        buttons
            .spawn((
                Button,
                *action,
                ToolSelectButton,
                ToolbarTooltipTarget { text: tip },
                BuildMenuUi,
                Node {
                    width: Val::Px(53.0),
                    height: Val::Px(48.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.5, 0.63, 0.35)),
                BorderColor::all(Color::srgb(0.18, 0.25, 0.12)),
                Interaction::default(),
            ))
            .with_children(|p| {
                spawn_button_icon(p, asset_server, icon_path, 42.0, 34.0, false);
            });
    }
}

fn spawn_settings_buttons(buttons: &mut ChildSpawnerCommands) {
    for (label, tip, action) in [
        (
            "Guardar...",
            "Elegir archivo y guardar simulacion JSON",
            SaveMenuAction::SaveAs,
        ),
        (
            "Cargar...",
            "Elegir archivo y cargar simulacion JSON",
            SaveMenuAction::LoadFrom,
        ),
    ] {
        buttons.spawn((
            Button,
            action,
            ToolbarTooltipTarget { text: tip },
            BuildMenuUi,
            Node {
                width: Val::Px(120.0),
                height: Val::Px(32.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(Color::srgb(0.5, 0.63, 0.35)),
            BorderColor::all(Color::srgb(0.18, 0.25, 0.12)),
            Interaction::default(),
            children![(
                Text::new(label),
                TextFont {
                    font_size: 12.0,
                    ..default()
                },
                TextColor(Color::srgb(0.08, 0.07, 0.05)),
            )],
        ));
    }
}

fn spawn_button_icon(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    icon_path: &str,
    width: f32,
    height: f32,
    with_margin: bool,
) {
    let margin = if with_margin {
        UiRect::right(Val::Px(4.0))
    } else {
        UiRect::default()
    };
    if let Some(label) = icon_path.strip_prefix("text:") {
        parent.spawn((
            Text::new(label),
            TextFont {
                font_size: height + 4.0,
                ..default()
            },
            TextColor(Color::srgb(0.08, 0.07, 0.05)),
            Node {
                width: Val::Px(width),
                margin,
                ..default()
            },
        ));
    } else {
        parent.spawn((
            ImageNode::new(asset_server.load::<Image>(icon_path.to_string())),
            Node {
                width: Val::Px(width),
                height: Val::Px(height),
                margin,
                ..default()
            },
        ));
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use std::fs;
    use std::path::Path;

    use bevy::app::ScheduleRunnerPlugin;
    use bevy::asset::AssetPlugin;
    use bevy::ecs::system::RunSystemOnce;
    use bevy::image::ImagePlugin;
    use bevy::prelude::*;

    use super::setup_top_toolbar;

    const ONE_PX_PNG: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/one_pixel.png"
    ));

    fn write_png(root: &Path, rel: &str) {
        let p = root.join(rel);
        if let Some(dir) = p.parent() {
            fs::create_dir_all(dir).expect("mkdir");
        }
        fs::write(&p, ONE_PX_PNG).expect("write png");
    }

    fn stub_toolbar_pngs(root: &Path) {
        for rel in [
            "opengfx/tiles/rail_1005.png",
            "opengfx/tiles/road_flat_00.png",
            "opengfx/tiles/road_flat_01.png",
            "opengfx/tiles/road_flat_02.png",
            "opengfx/tiles/house_church_build.png",
            "opengfx/tiles/object_lighthouse.png",
            "opengfx/tiles/object_transmitter.png",
            "opengfx/tiles/road_depot_0.png",
            "opengfx/tiles/bridge_wood_road_x.png",
            "opengfx/tiles/tunnel_road_rear.png",
            "opengfx/tiles/truck_stop_ground_0.png",
            "opengfx/tiles/rail_depot_ne.png",
            "opengfx/tiles/bridge_wood_rail_x.png",
            "opengfx/tiles/tunnel_rail_rear.png",
            "opengfx/tiles/industry_2013.png",
            "opengfx/tiles/industry_2028.png",
            "opengfx/tiles/industry_2169.png",
            "opengfx/tiles/tree_01.png",
        ] {
            write_png(root, rel);
        }
    }

    #[test]
    fn setup_top_toolbar_loads_stub_icons() {
        let dir = tempfile::tempdir().expect("tempdir");
        stub_toolbar_pngs(dir.path());
        let root = dir.path().to_str().expect("utf8");

        let mut app = App::new();
        app.add_plugins(MinimalPlugins.set(ScheduleRunnerPlugin::run_once()));
        app.add_plugins(AssetPlugin {
            file_path: root.into(),
            ..default()
        });
        app.add_plugins(ImagePlugin::default());
        app.update();
        app.world_mut()
            .run_system_once(setup_top_toolbar)
            .expect("toolbar");
    }
}
