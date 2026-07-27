use bevy::prelude::*;
use bevy::ui::UiScale;
use bevy::window::PrimaryWindow;

mod controls;
mod sections;

use sections::{
    spawn_air_panel, spawn_rail_panel, spawn_road_panel, spawn_secondary_tool_panels,
    spawn_toolbar_group_buttons, spawn_toolbar_tooltip, spawn_water_panel,
};

use super::BuildMenuUi;
use super::editor_toolbar::NormalToolbarRoot;
use crate::state::ingame_lifecycle::InGameUi;

const FULL_TOOLBAR_MIN_WIDTH: f32 = 1120.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum ToolbarLayoutMode {
    #[default]
    Full,
    Upper,
    Lower,
}

#[derive(Resource, Debug, Clone, Copy, Default)]
pub(crate) struct ToolbarLayoutState {
    pub(crate) mode: ToolbarLayoutMode,
}

#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ToolbarHalf {
    Upper,
    Lower,
}

#[derive(Component)]
pub(crate) struct ToolbarSwitchButton;

#[derive(Component)]
pub(crate) struct ToolbarSwitchLabel;

/// Slot cuyo ancho puede compactarse para respetar resolución y escala UI.
#[derive(Component, Clone, Copy)]
pub(crate) struct ResponsiveToolbarSlot {
    pub(crate) full_width: f32,
}

#[must_use]
pub(crate) fn compact_slot_width(ui_scale: f32) -> f32 {
    (60.0 / ui_scale.max(1.0)).clamp(30.0, 60.0)
}

#[must_use]
pub(crate) fn toolbar_layout_for_width(
    width: f32,
    ui_scale: f32,
    current: ToolbarLayoutMode,
) -> ToolbarLayoutMode {
    if width >= FULL_TOOLBAR_MIN_WIDTH * ui_scale.max(0.5) {
        ToolbarLayoutMode::Full
    } else if current == ToolbarLayoutMode::Full {
        ToolbarLayoutMode::Upper
    } else {
        current
    }
}

pub(crate) fn handle_toolbar_switch(
    interaction: Query<&Interaction, (Changed<Interaction>, With<ToolbarSwitchButton>)>,
    mut state: ResMut<ToolbarLayoutState>,
) {
    if interaction
        .iter()
        .any(|value| *value == Interaction::Pressed)
    {
        state.mode = match state.mode {
            ToolbarLayoutMode::Full | ToolbarLayoutMode::Lower => ToolbarLayoutMode::Upper,
            ToolbarLayoutMode::Upper => ToolbarLayoutMode::Lower,
        };
    }
}

pub(crate) fn sync_toolbar_layout(
    windows: Query<&Window, With<PrimaryWindow>>,
    ui_scale: Res<UiScale>,
    mut state: ResMut<ToolbarLayoutState>,
    mut halves: Query<(&ToolbarHalf, &mut Node)>,
    mut switch: Query<&mut Node, (With<ToolbarSwitchButton>, Without<ToolbarHalf>)>,
    mut label: Query<&mut Text, With<ToolbarSwitchLabel>>,
    mut responsive_slots: Query<
        (&ResponsiveToolbarSlot, &mut Node),
        (Without<ToolbarHalf>, Without<ToolbarSwitchButton>),
    >,
) {
    let Ok(window) = windows.single() else {
        return;
    };
    state.mode = toolbar_layout_for_width(window.width(), ui_scale.0, state.mode);
    for (half, mut node) in &mut halves {
        node.display = match state.mode {
            ToolbarLayoutMode::Full => Display::Flex,
            ToolbarLayoutMode::Upper if *half == ToolbarHalf::Upper => Display::Flex,
            ToolbarLayoutMode::Lower if *half == ToolbarHalf::Lower => Display::Flex,
            _ => Display::None,
        };
    }
    if let Ok(mut node) = switch.single_mut() {
        node.display = if state.mode == ToolbarLayoutMode::Full {
            Display::None
        } else {
            Display::Flex
        };
    }
    if let Ok(mut text) = label.single_mut() {
        **text = match state.mode {
            ToolbarLayoutMode::Lower => "▲",
            ToolbarLayoutMode::Upper | ToolbarLayoutMode::Full => "▼",
        }
        .into();
    }
    let compact_width = compact_slot_width(ui_scale.0);
    for (slot, mut node) in &mut responsive_slots {
        node.width = Val::Px(if state.mode == ToolbarLayoutMode::Full {
            slot.full_width
        } else {
            compact_width.min(slot.full_width)
        });
    }
}

/// Barra superior compacta tipo toolbar para seleccion rapida de herramienta.
pub(crate) fn setup_top_toolbar(mut commands: Commands, asset_server: Res<AssetServer>) {
    let root = commands
        .spawn((
            InGameUi,
            NormalToolbarRoot,
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
        spawn_toolbar_group_buttons(root, &asset_server);
        spawn_road_panel(root, &asset_server);
        spawn_water_panel(root, &asset_server);
        spawn_air_panel(root, &asset_server);
        spawn_rail_panel(root, &asset_server);
        spawn_secondary_tool_panels(root, &asset_server);
        spawn_toolbar_tooltip(root);
    });
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use std::fs;
    use std::path::Path;

    use bevy::app::ScheduleRunnerPlugin;
    use bevy::asset::AssetPlugin;
    use bevy::ecs::system::RunSystemOnce;
    use bevy::image::ImagePlugin;
    use bevy::prelude::*;
    use bevy::window::PrimaryWindow;

    use super::{
        ResponsiveToolbarSlot, ToolbarHalf, ToolbarLayoutMode, ToolbarLayoutState,
        ToolbarSwitchButton, ToolbarSwitchLabel, compact_slot_width, handle_toolbar_switch,
        setup_top_toolbar, sync_toolbar_layout, toolbar_layout_for_width,
    };

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
            "assets/opengfx/tiles/rail_1005.png",
            "assets/opengfx/tiles/road_flat_00.png",
            "assets/opengfx/tiles/road_flat_01.png",
            "assets/opengfx/tiles/road_flat_02.png",
            "assets/opengfx/tiles/tram_flat_00.png",
            "assets/opengfx/tiles/tram_flat_01.png",
            "assets/opengfx/tiles/tram_flat_02.png",
            "assets/opengfx/tiles/ship_depot_ne.png",
            "assets/opengfx/tiles/dock_flat_x.png",
            "assets/opengfx/tiles/water_flat.png",
            "assets/opengfx/tiles/toolbar_water_depot.png",
            "assets/opengfx/tiles/toolbar_water_dock.png",
            "assets/opengfx/tiles/toolbar_water_canal.png",
            "assets/opengfx/tiles/toolbar_water_river.png",
            "assets/opengfx/tiles/toolbar_water_buoy.png",
            "assets/opengfx/tiles/toolbar_water_aqueduct.png",
            "assets/opengfx/tiles/toolbar_water_lock.png",
            "assets/opengfx/tiles/airport_heliport.png",
            "assets/opengfx/tiles/airport_runway_0.png",
            "assets/opengfx/tiles/house_church_build.png",
            "assets/opengfx/tiles/object_lighthouse.png",
            "assets/opengfx/tiles/object_transmitter.png",
            "assets/opengfx/tiles/ui_settings.png",
            "assets/opengfx/tiles/ui_sound.png",
            "assets/opengfx/tiles/ui_terraform_up.png",
            "assets/opengfx/tiles/ui_terraform_down.png",
            "assets/opengfx/tiles/ui_terraform_level.png",
            "assets/opengfx/tiles/rail_1412.png",
            "assets/opengfx/tiles/bridge_wood_road_x.png",
            "assets/opengfx/tiles/tunnel_road_rear.png",
            "assets/opengfx/tiles/truck_stop_ground_0.png",
            "assets/opengfx/tiles/bus_stop_ne_ground.png",
            "assets/opengfx/tiles/rail_depot_ne.png",
            "assets/opengfx/tiles/bridge_wood_rail_x.png",
            "assets/opengfx/tiles/tunnel_rail_rear.png",
            "assets/opengfx/tiles/industry_2013.png",
            "assets/opengfx/tiles/industry_2028.png",
            "assets/opengfx/tiles/industry_2169.png",
            "assets/opengfx/tiles/tree_01.png",
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
        app.init_asset::<Font>();
        app.update();
        app.world_mut()
            .run_system_once(setup_top_toolbar)
            .expect("toolbar");
    }

    #[test]
    fn layout_policy_uses_stable_halves_when_full_toolbar_does_not_fit() {
        assert_eq!(
            toolbar_layout_for_width(1920.0, 1.0, ToolbarLayoutMode::Upper),
            ToolbarLayoutMode::Full
        );
        assert_eq!(
            toolbar_layout_for_width(1024.0, 1.0, ToolbarLayoutMode::Full),
            ToolbarLayoutMode::Upper
        );
        assert_eq!(
            toolbar_layout_for_width(800.0, 1.0, ToolbarLayoutMode::Lower),
            ToolbarLayoutMode::Lower
        );
        assert_eq!(
            toolbar_layout_for_width(1920.0, 2.0, ToolbarLayoutMode::Full),
            ToolbarLayoutMode::Upper
        );
        assert_eq!(compact_slot_width(1.0), 60.0);
        assert_eq!(compact_slot_width(1.5), 40.0);
        assert_eq!(compact_slot_width(2.0), 30.0);
    }

    #[test]
    fn compact_switch_alternates_halves() {
        let mut world = World::new();
        world.insert_resource(ToolbarLayoutState {
            mode: ToolbarLayoutMode::Upper,
        });
        world.spawn((Button, ToolbarSwitchButton, Interaction::Pressed));

        world.run_system_once(handle_toolbar_switch).unwrap();
        assert_eq!(
            world.resource::<ToolbarLayoutState>().mode,
            ToolbarLayoutMode::Lower
        );
    }

    #[test]
    fn sync_layout_hides_only_the_inactive_compact_half() {
        let mut world = World::new();
        world.insert_resource(ToolbarLayoutState {
            mode: ToolbarLayoutMode::Lower,
        });
        world.insert_resource(UiScale(2.0));
        world.spawn((
            Window {
                resolution: (800, 600).into(),
                ..default()
            },
            PrimaryWindow,
        ));
        let upper = world.spawn((ToolbarHalf::Upper, Node::default())).id();
        let lower = world.spawn((ToolbarHalf::Lower, Node::default())).id();
        let switch = world.spawn((ToolbarSwitchButton, Node::default())).id();
        let label = world.spawn((ToolbarSwitchLabel, Text::new(""))).id();
        let responsive = world
            .spawn((
                ResponsiveToolbarSlot { full_width: 78.0 },
                Node {
                    width: Val::Px(78.0),
                    ..default()
                },
            ))
            .id();

        world.run_system_once(sync_toolbar_layout).unwrap();

        assert_eq!(
            world.entity(upper).get::<Node>().unwrap().display,
            Display::None
        );
        assert_eq!(
            world.entity(lower).get::<Node>().unwrap().display,
            Display::Flex
        );
        assert_eq!(
            world.entity(switch).get::<Node>().unwrap().display,
            Display::Flex
        );
        assert_eq!(world.entity(label).get::<Text>().unwrap().as_str(), "▲");
        assert_eq!(
            world.entity(responsive).get::<Node>().unwrap().width,
            Val::Px(30.0)
        );
    }
}
