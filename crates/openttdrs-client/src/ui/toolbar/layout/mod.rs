use bevy::prelude::*;

mod controls;
mod sections;

use sections::{
    spawn_rail_panel, spawn_road_panel, spawn_secondary_tool_panels, spawn_toolbar_group_buttons,
    spawn_toolbar_tooltip,
};

use super::BuildMenuUi;
use crate::state::ingame_lifecycle::InGameUi;

/// Barra superior compacta tipo toolbar para seleccion rapida de herramienta.
pub(crate) fn setup_top_toolbar(mut commands: Commands, asset_server: Res<AssetServer>) {
    let root = commands
        .spawn((
            InGameUi,
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
        spawn_rail_panel(root, &asset_server);
        spawn_secondary_tool_panels(root, &asset_server);
        spawn_toolbar_tooltip(root);
    });
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
            "assets/opengfx/tiles/rail_1005.png",
            "assets/opengfx/tiles/road_flat_00.png",
            "assets/opengfx/tiles/road_flat_01.png",
            "assets/opengfx/tiles/road_flat_02.png",
            "assets/opengfx/tiles/house_church_build.png",
            "assets/opengfx/tiles/object_lighthouse.png",
            "assets/opengfx/tiles/object_transmitter.png",
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
        app.update();
        app.world_mut()
            .run_system_once(setup_top_toolbar)
            .expect("toolbar");
    }
}
