use bevy::prelude::*;

use crate::ui::toolbar::{BuildMenuAction, StationBuildState};

pub(crate) fn preview_image_for_action(
    action: BuildMenuAction,
    asset_server: &AssetServer,
    station_state: &StationBuildState,
) -> Option<Handle<Image>> {
    const BUS_STOP_GROUNDS: [&str; 4] = [
        "assets/opengfx/tiles/bus_stop_ne_ground.png",
        "assets/opengfx/tiles/bus_stop_se_ground.png",
        "assets/opengfx/tiles/bus_stop_sw_ground.png",
        "assets/opengfx/tiles/bus_stop_nw_ground.png",
    ];
    const ROAD_DEPOTS: [&str; 4] = [
        "assets/opengfx/tiles/rail_1412.png",
        "assets/opengfx/tiles/road_depot_1.png",
        "assets/opengfx/tiles/road_depot_3.png",
        "assets/opengfx/tiles/rail_1413.png",
    ];

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
        BuildMenuAction::RoadDepot => Some(
            asset_server.load::<Image>(ROAD_DEPOTS[usize::from(station_state.orientation.min(3))]),
        ),
        BuildMenuAction::RoadBridge => {
            Some(asset_server.load::<Image>("assets/opengfx/tiles/bridge_wood_road_x.png"))
        }
        BuildMenuAction::RoadTunnel => {
            Some(asset_server.load::<Image>("assets/opengfx/tiles/tunnel_road_rear.png"))
        }
        BuildMenuAction::Rail => {
            Some(asset_server.load::<Image>("assets/opengfx/tiles/rail_1005.png"))
        }
        BuildMenuAction::RailDepot => {
            Some(asset_server.load::<Image>("assets/opengfx/tiles/rail_depot_ne.png"))
        }
        BuildMenuAction::RailBridge => {
            Some(asset_server.load::<Image>("assets/opengfx/tiles/bridge_wood_rail_x.png"))
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
