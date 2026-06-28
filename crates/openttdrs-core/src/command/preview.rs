//! Validación de solo lectura alineada con [`apply_command`] (preview / HUD).

use crate::{GameState, IndustrySpec, StopKind};

use super::buy_land::check_buy_land;
use super::industry::check_place_industry_spec;
use super::terraform::{check_level_land, check_lower_land, check_raise_land};
use super::transport::{
    check_bridge, check_clear_tile, check_place_rail, check_place_rail_signal_oriented,
    check_place_rail_waypoint, check_place_road_bits, check_rail_depot_placement,
    check_rail_station_area, check_rail_trackbits_with_autoslope, check_remove_rail,
    check_road_depot_placement, check_single_transport_tile, check_station_placement, check_tunnel,
    merged_rail_trackbits_on_tile, rail_station_footprint, rail_trackbits_from_neighbors,
};
use super::types::{Command, CommandError};

fn preview_industry_error(
    map: &crate::map::Map,
    c: crate::map::TileCoord,
    spec: IndustrySpec,
) -> Option<CommandError> {
    check_place_industry_spec(map, c, spec).err()
}

fn preview_terraform(map: &crate::map::Map, cmd: &Command, tick: u64) -> Option<CommandError> {
    match cmd {
        Command::RaiseLand(c) => check_raise_land(map, *c, tick).err(),
        Command::LowerLand(c) => check_lower_land(map, *c, tick).err(),
        Command::LevelLand { from, to, mode } => {
            check_level_land(map, *from, *to, *mode, tick).err()
        }
        _ => None,
    }
}

fn preview_industry_cmd(map: &crate::map::Map, cmd: &Command) -> Option<CommandError> {
    match cmd {
        Command::PlaceIndustry(c) => preview_industry_error(map, *c, IndustrySpec::Factory),
        Command::PlaceIndustryKind(c, kind) => {
            let spec = match kind {
                crate::IndustryKind::CoalMine => IndustrySpec::CoalMine,
                crate::IndustryKind::Forest => IndustrySpec::Forest,
                crate::IndustryKind::OilWell => IndustrySpec::OilWells,
                crate::IndustryKind::Factory => IndustrySpec::Factory,
            };
            preview_industry_error(map, *c, spec)
        }
        Command::PlaceIndustrySpec(c, spec) => preview_industry_error(map, *c, *spec),
        _ => None,
    }
}

fn preview_depot_any<F>(
    map: &crate::map::Map,
    c: crate::map::TileCoord,
    check: F,
) -> Option<CommandError>
where
    F: Fn(&crate::map::Map, crate::map::TileCoord, u8) -> Result<(), CommandError>,
{
    if (0..4).any(|dir| check(map, c, dir).is_ok()) {
        None
    } else {
        Some(CommandError::StationNotAdjacentToTransport)
    }
}

fn preview_build_cmd(state: &GameState, cmd: &Command) -> Option<CommandError> {
    let map = &state.map;
    let stations = &state.stations;
    let tick = state.tick.get();
    match cmd {
        Command::PlaceRoad(c) | Command::PlaceRoadBits(c, _) | Command::SetRoadBits(c, _) => {
            check_place_road_bits(map, *c).err()
        }
        Command::PlaceRail(c) => check_place_rail(map, *c).err().or_else(|| {
            let tb = rail_trackbits_from_neighbors(map, *c);
            check_rail_trackbits_with_autoslope(map, *c, tb, tick).err()
        }),
        Command::PlaceRailBits(c, bits) => check_place_rail(map, *c).err().or_else(|| {
            let tb = merged_rail_trackbits_on_tile(map, *c, *bits);
            check_rail_trackbits_with_autoslope(map, *c, tb, tick).err()
        }),
        Command::SetRailBits(c, bits) => check_place_rail(map, *c)
            .err()
            .or_else(|| check_rail_trackbits_with_autoslope(map, *c, bits & 0x3F, tick).err()),
        Command::PlaceRailWaypoint(c) => check_place_rail_waypoint(map, *c, stations).err(),
        Command::RemoveRailBits(c, _) | Command::RemoveRail(c) => check_remove_rail(map, *c).err(),
        Command::PlaceRailSignal(c, orientation) => {
            check_place_rail_signal_oriented(map, *c, *orientation).err()
        }
        Command::PlaceRoadDepot(c) => preview_depot_any(map, *c, check_road_depot_placement),
        Command::PlaceRoadDepotDir(c, dir) => check_road_depot_placement(map, *c, *dir).err(),
        Command::PlaceRailDepot(c) => preview_depot_any(map, *c, check_rail_depot_placement),
        Command::PlaceRailDepotDir(c, dir) => check_rail_depot_placement(map, *c, *dir).err(),
        Command::PlaceHouse(c) | Command::PlaceForest(c) => {
            check_single_transport_tile(map, *c).err()
        }
        Command::PlaceRoadTunnel(a, _) | Command::PlaceRailTunnel(a, _) => {
            check_tunnel(map, *a).err()
        }
        Command::PlaceRoadBridge(a, b) | Command::PlaceRailBridge(a, b) => {
            check_bridge(map, *a, *b).err()
        }
        Command::PlaceStation(c) => {
            if stations.iter().any(|s| s.pos == *c) {
                return Some(CommandError::StationAlreadyExists);
            }
            preview_depot_any(map, *c, |m, tile, dir| {
                check_station_placement(m, stations, tile, dir, StopKind::TruckStop)
            })
        }
        Command::PlaceStationDir(c, dir) | Command::PlaceTruckStop(c, dir) => {
            check_station_placement(map, stations, *c, *dir, StopKind::TruckStop).err()
        }
        Command::PlaceBusStop(c, dir) => {
            check_station_placement(map, stations, *c, *dir, StopKind::BusStop).err()
        }
        Command::PlaceRailStation(c, dir) => {
            check_station_placement(map, stations, *c, *dir, StopKind::RailStation).err()
        }
        Command::PlaceRailStationArea {
            origin,
            axis_y,
            platforms,
            length,
        } => {
            let (w, h) =
                rail_station_footprint(*axis_y, (*platforms).clamp(1, 7), (*length).clamp(1, 7));
            check_rail_station_area(state, *origin, w, h).err()
        }
        Command::ClearTile(c) => check_clear_tile(map, *c).err(),
        Command::BuyLand(c) => check_buy_land(map, *c).err(),
        Command::BuyLandArea { from, to } => {
            let any_ok =
                super::buy_land::tile_rect(*from, *to).any(|c| check_buy_land(map, c).is_ok());
            if any_ok {
                None
            } else {
                Some(CommandError::CannotBuyLandHere)
            }
        }
        Command::PlaceIndustry(_)
        | Command::PlaceIndustryKind(_, _)
        | Command::PlaceIndustrySpec(_, _)
        | Command::RaiseLand(_)
        | Command::LowerLand(_)
        | Command::LevelLand { .. }
        | Command::SetVehicleOrders(..)
        | Command::SetVehicleStationOrders(..)
        | Command::SetVehicleOrderList(..)
        | Command::BuildRoadVehicleAtDepot(..)
        | Command::BuildVehicleAtDepot(..)
        | Command::SellVehicle(..)
        | Command::ToggleVehicleRunning(..)
        | Command::CloneVehicleOrders { .. } => None,
    }
}

/// Devuelve el error que obtendría `apply_command` sin mutar el estado.
#[must_use]
pub fn command_would_fail(state: &GameState, cmd: &Command) -> Option<CommandError> {
    if matches!(cmd, Command::BuyLand(_) | Command::BuyLandArea { .. }) {
        let quote = super::buy_land::buy_land_quote(state, cmd);
        if quote > 0 && state.economy.money < quote {
            return Some(CommandError::InsufficientFunds);
        }
    }
    if matches!(
        cmd,
        Command::RaiseLand(_) | Command::LowerLand(_) | Command::LevelLand { .. }
    ) {
        return preview_terraform(&state.map, cmd, state.tick.get());
    }
    if let Some(err) = preview_industry_cmd(&state.map, cmd) {
        return Some(err);
    }
    preview_build_cmd(state, cmd)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use crate::map::{TileCoord, TileKind};
    use crate::{Command, GameState};

    use super::command_would_fail;
    use crate::command::CommandError;

    #[test]
    fn command_would_fail_rejects_road_on_water() {
        let mut s = GameState::new(8, 8);
        let c = TileCoord::new(1, 1);
        s.map.set_kind(c, TileKind::Water).unwrap();
        assert_eq!(
            command_would_fail(&s, &Command::PlaceRoad(c)),
            Some(CommandError::CannotPlaceRoadOnWater)
        );
    }
}
