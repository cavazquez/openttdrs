use openttdrs_core::{Command, Map, TileCoord, road_bits_for_autoroute};

use super::rail_lane::rail_lane_bits_for_action;
use crate::ui::toolbar::{BuildMenuAction, StationBuildState};

pub(crate) fn command_for_action(
    action: BuildMenuAction,
    pos: TileCoord,
    station_state: &StationBuildState,
    rail_lane_bits: Option<u8>,
    map: Option<&Map>,
) -> Option<Command> {
    match action {
        BuildMenuAction::Road => {
            let bits = map.map(|m| road_bits_for_autoroute(m, pos)).unwrap_or(0x0A);
            Some(Command::PlaceRoadBits(pos, bits))
        }
        BuildMenuAction::RoadX => Some(Command::PlaceRoadBits(pos, 0x0A)),
        BuildMenuAction::RoadY => Some(Command::PlaceRoadBits(pos, 0x05)),
        BuildMenuAction::Rail => Some(Command::PlaceRail(pos)),
        BuildMenuAction::RailX => Some(Command::PlaceRailBits(pos, 0x01)),
        BuildMenuAction::RailY => Some(Command::PlaceRailBits(pos, 0x02)),
        BuildMenuAction::RailHorz | BuildMenuAction::RailVert => {
            let bits = rail_lane_bits.or_else(|| rail_lane_bits_for_action(action, None))?;
            Some(Command::PlaceRailBits(pos, bits))
        }
        BuildMenuAction::RailStation => Some(Command::PlaceRailStationArea {
            origin: pos,
            axis_y: station_state.rail_axis_y,
            platforms: station_state.rail_platforms,
            length: station_state.rail_length,
        }),
        BuildMenuAction::Station => Some(Command::PlaceStationDir(pos, station_state.orientation)),
        BuildMenuAction::BusStop => Some(Command::PlaceBusStop(pos, station_state.orientation)),
        BuildMenuAction::Clear => Some(Command::ClearTile(pos)),
        BuildMenuAction::RoadDepot => {
            Some(Command::PlaceRoadDepotDir(pos, station_state.orientation))
        }
        BuildMenuAction::RailDepot => {
            Some(Command::PlaceRailDepotDir(pos, station_state.orientation))
        }
        BuildMenuAction::RoadBridge
        | BuildMenuAction::RoadTunnel
        | BuildMenuAction::RailBridge
        | BuildMenuAction::RailTunnel => None,
        BuildMenuAction::RailRemove => Some(Command::RemoveRail(pos)),
        BuildMenuAction::RailSignals => {
            Some(Command::PlaceRailSignal(pos, station_state.orientation))
        }
        BuildMenuAction::RailConvert | BuildMenuAction::Orders => None,
        BuildMenuAction::RailWaypoint => Some(Command::PlaceRailWaypoint(pos)),
        BuildMenuAction::BuildHouse => Some(Command::PlaceHouse(pos)),
        BuildMenuAction::BuildCoalMine => Some(Command::PlaceIndustrySpec(
            pos,
            openttdrs_core::IndustrySpec::CoalMine,
        )),
        BuildMenuAction::BuildIronOreMine => Some(Command::PlaceIndustrySpec(
            pos,
            openttdrs_core::IndustrySpec::IronOreMine,
        )),
        BuildMenuAction::BuildGoldMine => Some(Command::PlaceIndustrySpec(
            pos,
            openttdrs_core::IndustrySpec::GoldMine,
        )),
        BuildMenuAction::BuildOilWell => Some(Command::PlaceIndustrySpec(
            pos,
            openttdrs_core::IndustrySpec::OilWells,
        )),
        BuildMenuAction::BuildOilRefinery => Some(Command::PlaceIndustrySpec(
            pos,
            openttdrs_core::IndustrySpec::OilRefinery,
        )),
        BuildMenuAction::BuildFactory => Some(Command::PlaceIndustrySpec(
            pos,
            openttdrs_core::IndustrySpec::Factory,
        )),
        BuildMenuAction::BuildSawmill => Some(Command::PlaceIndustrySpec(
            pos,
            openttdrs_core::IndustrySpec::Sawmill,
        )),
        BuildMenuAction::BuildForest => Some(Command::PlaceIndustrySpec(
            pos,
            openttdrs_core::IndustrySpec::Forest,
        )),
        BuildMenuAction::BuildFarm => Some(Command::PlaceIndustrySpec(
            pos,
            openttdrs_core::IndustrySpec::Farm,
        )),
    }
}

pub(crate) fn command_for_line_action(
    action: BuildMenuAction,
    tiles: &[(i32, i32)],
) -> Option<Command> {
    let &(sx, sy) = tiles.first()?;
    let &(ex, ey) = tiles.last()?;
    let a = TileCoord::new(sx, sy);
    let b = TileCoord::new(ex, ey);
    match action {
        BuildMenuAction::RoadTunnel => Some(Command::PlaceRoadTunnel(a, b)),
        BuildMenuAction::RailTunnel => Some(Command::PlaceRailTunnel(a, b)),
        BuildMenuAction::RoadBridge => Some(Command::PlaceRoadBridge(a, b)),
        BuildMenuAction::RailBridge => Some(Command::PlaceRailBridge(a, b)),
        _ => None,
    }
}
