use bevy::prelude::*;

use crate::ui::toolbar::{BuildMenuAction, StationBuildState};

fn bridge_axis_y_from_tiles(tiles: &[(i32, i32)]) -> bool {
    let Some(&(sx, sy)) = tiles.first() else {
        return false;
    };
    let Some(&(ex, ey)) = tiles.last() else {
        return false;
    };
    (ex - sx).abs() < (ey - sy).abs()
}

pub(crate) fn preview_image_for_action(
    action: BuildMenuAction,
    asset_server: &AssetServer,
    station_state: &StationBuildState,
    preview_tiles: &[(i32, i32)],
) -> Option<Handle<Image>> {
    const BUS_STOP_GROUNDS: [&str; 4] = [
        "assets/opengfx/tiles/bus_stop_ne_ground.png",
        "assets/opengfx/tiles/bus_stop_se_ground.png",
        "assets/opengfx/tiles/bus_stop_sw_ground.png",
        "assets/opengfx/tiles/bus_stop_nw_ground.png",
    ];
    use crate::sprites::ROAD_DEPOT_BUILDING_BY_DIR;

    match action {
        BuildMenuAction::Station => Some(asset_server.load::<Image>(format!(
            "assets/opengfx/tiles/truck_stop_ground_{}.png",
            station_state.orientation
        ))),
        BuildMenuAction::BusStop => Some(
            asset_server
                .load::<Image>(BUS_STOP_GROUNDS[usize::from(station_state.orientation.min(3))]),
        ),
        BuildMenuAction::Road => {
            Some(asset_server.load::<Image>("assets/opengfx/tiles/road_flat_02.png"))
        }
        BuildMenuAction::RoadX => {
            Some(asset_server.load::<Image>("assets/opengfx/tiles/road_flat_01.png"))
        }
        BuildMenuAction::RoadY => {
            Some(asset_server.load::<Image>("assets/opengfx/tiles/road_flat_00.png"))
        }
        BuildMenuAction::RoadDepot => Some(asset_server.load::<Image>(
            ROAD_DEPOT_BUILDING_BY_DIR[usize::from(station_state.orientation.min(3))],
        )),
        BuildMenuAction::RoadBridge => {
            let path = if bridge_axis_y_from_tiles(preview_tiles) {
                "assets/opengfx/tiles/bridge_wood_road_y.png"
            } else {
                "assets/opengfx/tiles/bridge_wood_road_x.png"
            };
            Some(asset_server.load::<Image>(path))
        }
        BuildMenuAction::RoadTunnel => {
            Some(asset_server.load::<Image>("assets/opengfx/tiles/tunnel_road_rear.png"))
        }
        BuildMenuAction::Rail => {
            Some(asset_server.load::<Image>("assets/opengfx/tiles/rail_1005.png"))
        }
        BuildMenuAction::RailStation => {
            let axis_y = station_state.orientation.is_multiple_of(2);
            let path = if axis_y {
                "assets/opengfx/tiles/rail_platform_y_front.png"
            } else {
                "assets/opengfx/tiles/rail_platform_x_front.png"
            };
            Some(asset_server.load::<Image>(path))
        }
        BuildMenuAction::RailDepot => {
            Some(asset_server.load::<Image>("assets/opengfx/tiles/rail_depot_ne.png"))
        }
        BuildMenuAction::RailBridge => {
            let path = if bridge_axis_y_from_tiles(preview_tiles) {
                "assets/opengfx/tiles/bridge_wood_rail_y.png"
            } else {
                "assets/opengfx/tiles/bridge_wood_rail_x.png"
            };
            Some(asset_server.load::<Image>(path))
        }
        BuildMenuAction::RailTunnel => {
            Some(asset_server.load::<Image>("assets/opengfx/tiles/tunnel_rail_rear.png"))
        }
        BuildMenuAction::Clear => {
            Some(asset_server.load::<Image>("assets/opengfx/tiles/grass_rough.png"))
        }
        BuildMenuAction::Orders => None,
        BuildMenuAction::BuildHouse => {
            Some(asset_server.load::<Image>("assets/opengfx/tiles/house_church_build.png"))
        }
        BuildMenuAction::BuildCoalMine => {
            Some(asset_server.load::<Image>("assets/opengfx/tiles/industry_2013.png"))
        }
        BuildMenuAction::BuildIronOreMine => {
            Some(asset_server.load::<Image>("assets/opengfx/tiles/industry_2092.png"))
        }
        BuildMenuAction::BuildGoldMine => {
            Some(asset_server.load::<Image>("assets/opengfx/tiles/industry_2247.png"))
        }
        BuildMenuAction::BuildOilWell => {
            Some(asset_server.load::<Image>("assets/opengfx/tiles/industry_2028.png"))
        }
        BuildMenuAction::BuildOilRefinery => {
            Some(asset_server.load::<Image>("assets/opengfx/tiles/industry_2047.png"))
        }
        BuildMenuAction::BuildFactory => {
            Some(asset_server.load::<Image>("assets/opengfx/tiles/industry_2169.png"))
        }
        BuildMenuAction::BuildSawmill => {
            Some(asset_server.load::<Image>("assets/opengfx/tiles/industry_2063.png"))
        }
        BuildMenuAction::BuildForest => {
            Some(asset_server.load::<Image>("assets/opengfx/tiles/tree_01.png"))
        }
        BuildMenuAction::BuildFarm => {
            Some(asset_server.load::<Image>("assets/opengfx/tiles/industry_2190.png"))
        }
    }
}
