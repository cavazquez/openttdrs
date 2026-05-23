use openttdrs_core::{Command, TileCoord};

use crate::ui::toolbar::{BuildMenuAction, StationBuildState};

pub(crate) fn command_for_action(
    action: BuildMenuAction,
    pos: TileCoord,
    station_state: &StationBuildState,
) -> Option<Command> {
    match action {
        BuildMenuAction::Road => Some(Command::PlaceRoadBits(pos, 0x0F)),
        BuildMenuAction::RoadX => Some(Command::PlaceRoadBits(pos, 0x0A)),
        BuildMenuAction::RoadY => Some(Command::PlaceRoadBits(pos, 0x05)),
        BuildMenuAction::Rail => Some(Command::PlaceRail(pos)),
        BuildMenuAction::RailHorz => Some(Command::PlaceRailBits(pos, 0x0C)),
        BuildMenuAction::RailVert => Some(Command::PlaceRailBits(pos, 0x30)),
        BuildMenuAction::RailStation => {
            Some(Command::PlaceRailStation(pos, station_state.orientation))
        }
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
        | BuildMenuAction::RailTunnel
        | BuildMenuAction::Orders => None,
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
