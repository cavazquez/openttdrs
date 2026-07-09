use openttdrs_core::{
    Command, LevelMode, Map, TileCoord, TileKind,
    rail_signals::{
        rail_signal_present_mask, rail_tile_is_signals, resolve_signal_track, signal_on_track_mask,
    },
    road_bits_for_autoroute,
};

use super::rail_lane::rail_lane_bits_for_action;
use crate::ui::toolbar::{BuildMenuAction, StationBuildState};

#[allow(clippy::too_many_arguments)]
pub(crate) fn command_for_action(
    action: BuildMenuAction,
    pos: TileCoord,
    station_state: &StationBuildState,
    rail_lane_bits: Option<u8>,
    map: Option<&Map>,
    tile_fract: Option<(u8, u8)>,
    sig_type: u8,
    cycle_existing_signal_type: bool,
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
        BuildMenuAction::Clear => {
            let (fx, fy) = tile_fract.unwrap_or((128, 128));
            if let Some(map) = map
                && let Some(tile) = map.get(pos)
                && tile.kind == TileKind::Rail
                && rail_tile_is_signals(tile.m5)
            {
                let tb = tile.m5 & 0x3F;
                if let Some(track) = resolve_signal_track(tb, fx, fy) {
                    let present = rail_signal_present_mask(tile.m3);
                    if present & signal_on_track_mask(track) != 0 {
                        return Some(Command::RemoveRailSignal(pos, fx, fy));
                    }
                }
            }
            Some(Command::ClearTile(pos))
        }
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
            let (fx, fy) = tile_fract.unwrap_or((128, 128));
            if cycle_existing_signal_type
                && let Some(map) = map
                && let Some(tile) = map.get(pos)
                && rail_tile_is_signals(tile.m5)
            {
                let tb = tile.m5 & 0x3F;
                if let Some(track) = resolve_signal_track(tb, fx, fy) {
                    let present = rail_signal_present_mask(tile.m3);
                    if present & signal_on_track_mask(track) != 0 {
                        return Some(Command::CycleRailSignalType(pos, fx, fy));
                    }
                }
            }
            Some(Command::PlaceRailSignal(
                pos,
                station_state.orientation,
                fx,
                fy,
                sig_type,
            ))
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
        BuildMenuAction::BuildCottonCandy => Some(Command::PlaceIndustrySpec(
            pos,
            openttdrs_core::IndustrySpec::CottonCandy,
        )),
        BuildMenuAction::BuildCandyFactory => Some(Command::PlaceIndustrySpec(
            pos,
            openttdrs_core::IndustrySpec::CandyFactory,
        )),
        BuildMenuAction::BuildBatteryFarm => Some(Command::PlaceIndustrySpec(
            pos,
            openttdrs_core::IndustrySpec::BatteryFarm,
        )),
        BuildMenuAction::BuildColaWells => Some(Command::PlaceIndustrySpec(
            pos,
            openttdrs_core::IndustrySpec::ColaWells,
        )),
        BuildMenuAction::BuildToyFactory => Some(Command::PlaceIndustrySpec(
            pos,
            openttdrs_core::IndustrySpec::ToyFactory,
        )),
        BuildMenuAction::BuildPlasticFountain => Some(Command::PlaceIndustrySpec(
            pos,
            openttdrs_core::IndustrySpec::PlasticFountain,
        )),
        BuildMenuAction::BuildFizzyDrinkFactory => Some(Command::PlaceIndustrySpec(
            pos,
            openttdrs_core::IndustrySpec::FizzyDrinkFactory,
        )),
        BuildMenuAction::BuildBubbleGenerator => Some(Command::PlaceIndustrySpec(
            pos,
            openttdrs_core::IndustrySpec::BubbleGenerator,
        )),
        BuildMenuAction::BuildToffeeQuarry => Some(Command::PlaceIndustrySpec(
            pos,
            openttdrs_core::IndustrySpec::ToffeeQuarry,
        )),
        BuildMenuAction::BuildSugarMine => Some(Command::PlaceIndustrySpec(
            pos,
            openttdrs_core::IndustrySpec::SugarMine,
        )),
        BuildMenuAction::RaiseLand => Some(Command::RaiseLand(pos)),
        BuildMenuAction::LowerLand => Some(Command::LowerLand(pos)),
        BuildMenuAction::LevelLand => Some(Command::LevelLand {
            from: pos,
            to: pos,
            mode: LevelMode::Level,
        }),
        BuildMenuAction::BuyLand => Some(Command::BuyLand(pos)),
    }
}

/// Comando de compra de terreno para clic o arrastre en área.
pub(crate) fn buy_land_command_for_tiles(tiles: &[(i32, i32)]) -> Option<Command> {
    let &(sx, sy) = tiles.first()?;
    let &(ex, ey) = tiles.last()?;
    let from = TileCoord::new(sx, sy);
    let to = TileCoord::new(ex, ey);
    if tiles.len() == 1 {
        Some(Command::BuyLand(from))
    } else {
        Some(Command::BuyLandArea { from, to })
    }
}

/// Comando de terraform para clic o arrastre (rectángulo en área).
pub(crate) fn terraform_command_for_tiles(
    action: BuildMenuAction,
    tiles: &[(i32, i32)],
) -> Option<Command> {
    let &(sx, sy) = tiles.first()?;
    let &(ex, ey) = tiles.last()?;
    let from = TileCoord::new(sx, sy);
    let to = TileCoord::new(ex, ey);
    if tiles.len() == 1 {
        return match action {
            BuildMenuAction::RaiseLand => Some(Command::RaiseLand(from)),
            BuildMenuAction::LowerLand => Some(Command::LowerLand(from)),
            BuildMenuAction::LevelLand => Some(Command::LevelLand {
                from,
                to: from,
                mode: LevelMode::Level,
            }),
            _ => None,
        };
    }
    let mode = match action {
        BuildMenuAction::RaiseLand => LevelMode::Raise,
        BuildMenuAction::LowerLand => LevelMode::Lower,
        BuildMenuAction::LevelLand => LevelMode::Level,
        _ => return None,
    };
    Some(Command::LevelLand { from, to, mode })
}

pub(crate) fn command_for_line_action(
    action: BuildMenuAction,
    tiles: &[(i32, i32)],
    bridge_type: openttdrs_core::BridgeType,
) -> Option<Command> {
    let &(sx, sy) = tiles.first()?;
    let &(ex, ey) = tiles.last()?;
    let a = TileCoord::new(sx, sy);
    let b = TileCoord::new(ex, ey);
    match action {
        BuildMenuAction::RoadTunnel => Some(Command::PlaceRoadTunnel(a, b)),
        BuildMenuAction::RailTunnel => Some(Command::PlaceRailTunnel(a, b)),
        BuildMenuAction::RoadBridge => Some(Command::PlaceRoadBridge(a, b, bridge_type)),
        BuildMenuAction::RailBridge => Some(Command::PlaceRailBridge(a, b, bridge_type)),
        _ => None,
    }
}
