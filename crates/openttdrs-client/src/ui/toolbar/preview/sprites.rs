use bevy::prelude::*;

use crate::ui::toolbar::{BuildMenuAction, StationBuildState};

/// Frames del cursor de demolición (`animcursors.h` / SPR_CURSOR_DEMOLISH_FIRST..LAST).
const DEMOLISH_CURSOR_FRAMES: [&str; 4] = [
    "assets/opengfx/tiles/ui_demolish.png",
    "assets/opengfx/tiles/ui_demolish_1.png",
    "assets/opengfx/tiles/ui_demolish_2.png",
    "assets/opengfx/tiles/ui_demolish_3.png",
];

/// Índice de frame del cursor demolición (`anim_cursor_frame & 3`).
#[must_use]
pub(crate) fn demolish_cursor_frame_index(anim_cursor_frame: u8) -> usize {
    usize::from(anim_cursor_frame & 3)
}

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
    anim_cursor_frame: u8,
) -> Option<Handle<Image>> {
    const BUS_STOP_GROUNDS: [&str; 4] = [
        "assets/opengfx/tiles/bus_stop_ne_ground.png",
        "assets/opengfx/tiles/bus_stop_se_ground.png",
        "assets/opengfx/tiles/bus_stop_sw_ground.png",
        "assets/opengfx/tiles/bus_stop_nw_ground.png",
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
        BuildMenuAction::Tram => {
            Some(asset_server.load::<Image>("assets/opengfx/tiles/tram_flat_02.png"))
        }
        BuildMenuAction::TramX => {
            Some(asset_server.load::<Image>("assets/opengfx/tiles/tram_flat_01.png"))
        }
        BuildMenuAction::TramY => {
            Some(asset_server.load::<Image>("assets/opengfx/tiles/tram_flat_00.png"))
        }
        BuildMenuAction::RoadDepot => None,
        BuildMenuAction::ShipDepot => {
            Some(asset_server.load::<Image>("assets/opengfx/tiles/ship_depot_ne.png"))
        }
        BuildMenuAction::Dock => {
            let path = if station_state.orientation & 1 != 0 {
                "assets/opengfx/tiles/dock_flat_y.png"
            } else {
                "assets/opengfx/tiles/dock_flat_x.png"
            };
            Some(asset_server.load::<Image>(path))
        }
        BuildMenuAction::Canal => {
            Some(asset_server.load::<Image>("assets/opengfx/tiles/water_flat.png"))
        }
        BuildMenuAction::River => {
            Some(asset_server.load::<Image>("assets/opengfx/tiles/water_flat.png"))
        }
        BuildMenuAction::Buoy => Some(asset_server.load::<Image>("assets/opengfx/tiles/buoy.png")),
        BuildMenuAction::Aqueduct => {
            let path = if bridge_axis_y_from_tiles(preview_tiles) {
                "assets/opengfx/tiles/bridge_wood_road_y.png"
            } else {
                "assets/opengfx/tiles/bridge_wood_road_x.png"
            };
            Some(asset_server.load::<Image>(path))
        }
        BuildMenuAction::Lock => {
            let path = if station_state.orientation & 1 != 0 {
                "assets/opengfx/tiles/water_lock_ew_middle.png"
            } else {
                "assets/opengfx/tiles/water_lock_ns_middle.png"
            };
            Some(asset_server.load::<Image>(path))
        }
        BuildMenuAction::Airport => {
            Some(asset_server.load::<Image>("assets/opengfx/tiles/airport_runway_0.png"))
        }
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
            Some(asset_server.load::<Image>("assets/opengfx/tiles/rail_1012.png"))
        }
        BuildMenuAction::RailX => {
            Some(asset_server.load::<Image>("assets/opengfx/tiles/rail_1012.png"))
        }
        BuildMenuAction::RailY => {
            Some(asset_server.load::<Image>("assets/opengfx/tiles/rail_1011.png"))
        }
        BuildMenuAction::RailHorz => {
            Some(asset_server.load::<Image>("assets/opengfx/tiles/rail_1035.png"))
        }
        BuildMenuAction::RailVert => {
            Some(asset_server.load::<Image>("assets/opengfx/tiles/rail_1036.png"))
        }
        BuildMenuAction::RailWaypoint
        | BuildMenuAction::RoadWaypoint
        | BuildMenuAction::RailSignals
        | BuildMenuAction::RailRemove
        | BuildMenuAction::RailConvert => None,
        BuildMenuAction::RailStation => {
            let axis_y = station_state.orientation.is_multiple_of(2);
            let path = if axis_y {
                "assets/opengfx/tiles/rail_platform_y_front.png"
            } else {
                "assets/opengfx/tiles/rail_platform_x_front.png"
            };
            Some(asset_server.load::<Image>(path))
        }
        BuildMenuAction::RailDepot => None,
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
            let path = DEMOLISH_CURSOR_FRAMES[demolish_cursor_frame_index(anim_cursor_frame)];
            Some(asset_server.load::<Image>(path))
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
        BuildMenuAction::BuildCottonCandy => {
            Some(asset_server.load::<Image>("assets/opengfx/tiles/industry_2100.png"))
        }
        BuildMenuAction::BuildCandyFactory => {
            Some(asset_server.load::<Image>("assets/opengfx/tiles/industry_2111.png"))
        }
        BuildMenuAction::BuildBatteryFarm => {
            Some(asset_server.load::<Image>("assets/opengfx/tiles/industry_2130.png"))
        }
        BuildMenuAction::BuildColaWells => {
            Some(asset_server.load::<Image>("assets/opengfx/tiles/industry_2132.png"))
        }
        BuildMenuAction::BuildToyFactory => {
            Some(asset_server.load::<Image>("assets/opengfx/tiles/industry_2150.png"))
        }
        BuildMenuAction::BuildPlasticFountain => {
            Some(asset_server.load::<Image>("assets/opengfx/tiles/industry_2151.png"))
        }
        BuildMenuAction::BuildFizzyDrinkFactory => {
            Some(asset_server.load::<Image>("assets/opengfx/tiles/industry_2139.png"))
        }
        BuildMenuAction::BuildBubbleGenerator => {
            Some(asset_server.load::<Image>("assets/opengfx/tiles/industry_2128.png"))
        }
        BuildMenuAction::BuildToffeeQuarry => {
            Some(asset_server.load::<Image>("assets/opengfx/tiles/industry_2180.png"))
        }
        BuildMenuAction::BuildSugarMine => {
            Some(asset_server.load::<Image>("assets/opengfx/tiles/industry_2201.png"))
        }
        BuildMenuAction::RaiseLand => {
            Some(asset_server.load::<Image>("assets/opengfx/tiles/ui_terraform_up.png"))
        }
        BuildMenuAction::LowerLand => {
            Some(asset_server.load::<Image>("assets/opengfx/tiles/ui_terraform_down.png"))
        }
        BuildMenuAction::LevelLand => {
            Some(asset_server.load::<Image>("assets/opengfx/tiles/ui_terraform_level.png"))
        }
        BuildMenuAction::BuyLand => {
            Some(asset_server.load::<Image>("assets/opengfx/tiles/object_bought_land.png"))
        }
        BuildMenuAction::PlantTree => {
            Some(asset_server.load::<Image>("assets/opengfx/tiles/tree_01.png"))
        }
        BuildMenuAction::PlaceSign => {
            Some(asset_server.load::<Image>("assets/opengfx/tiles/object_lighthouse.png"))
        }
        BuildMenuAction::JoinStation => {
            Some(asset_server.load::<Image>("assets/opengfx/tiles/bus_stop_ne_ground.png"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demolish_frame_cycles_four() {
        assert_eq!(demolish_cursor_frame_index(0), 0);
        assert_eq!(demolish_cursor_frame_index(1), 1);
        assert_eq!(demolish_cursor_frame_index(3), 3);
        assert_eq!(demolish_cursor_frame_index(4), 0);
        assert_eq!(demolish_cursor_frame_index(7), 3);
        assert_eq!(
            DEMOLISH_CURSOR_FRAMES[demolish_cursor_frame_index(2)],
            "assets/opengfx/tiles/ui_demolish_2.png"
        );
    }
}
