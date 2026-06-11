//! Validación de solo lectura alineada con [`apply_command`] (preview / HUD).

use crate::{GameState, IndustrySpec, StopKind};

use super::industry::check_place_industry_spec;
use super::transport::{
    check_bridge, check_clear_tile, check_place_rail, check_place_road_bits,
    check_rail_depot_placement, check_rail_station_area, check_road_depot_placement,
    check_single_transport_tile, check_station_placement, check_tunnel, rail_station_footprint,
};
use super::types::{Command, CommandError};

/// Devuelve el error que obtendría `apply_command` sin mutar el estado.
#[must_use]
pub fn command_would_fail(state: &GameState, cmd: &Command) -> Option<CommandError> {
    let map = &state.map;
    let stations = &state.stations;
    match cmd {
        Command::PlaceRoad(c) | Command::PlaceRoadBits(c, _) | Command::SetRoadBits(c, _) => {
            check_place_road_bits(map, *c).err()
        }
        Command::PlaceRail(c) | Command::PlaceRailBits(c, _) | Command::SetRailBits(c, _) => {
            check_place_rail(map, *c).err()
        }
        Command::PlaceRoadDepot(c) => {
            if (0..4).any(|dir| check_road_depot_placement(map, *c, dir).is_ok()) {
                None
            } else {
                Some(CommandError::StationNotAdjacentToTransport)
            }
        }
        Command::PlaceRoadDepotDir(c, dir) => check_road_depot_placement(map, *c, *dir).err(),
        Command::PlaceRailDepot(c) => {
            if (0..4).any(|dir| check_rail_depot_placement(map, *c, dir).is_ok()) {
                None
            } else {
                Some(CommandError::StationNotAdjacentToTransport)
            }
        }
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
            if (0..4).any(|dir| {
                check_station_placement(map, stations, *c, dir, StopKind::TruckStop).is_ok()
            }) {
                None
            } else {
                Some(CommandError::StationNotAdjacentToTransport)
            }
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
        Command::PlaceIndustry(c) => {
            check_place_industry_spec(map, *c, IndustrySpec::Factory).err()
        }
        Command::PlaceIndustryKind(c, kind) => {
            let spec = match kind {
                crate::IndustryKind::CoalMine => IndustrySpec::CoalMine,
                crate::IndustryKind::Forest => IndustrySpec::Forest,
                crate::IndustryKind::OilWell => IndustrySpec::OilWells,
                crate::IndustryKind::Factory => IndustrySpec::Factory,
            };
            check_place_industry_spec(map, *c, spec).err()
        }
        Command::PlaceIndustrySpec(c, spec) => check_place_industry_spec(map, *c, *spec).err(),
        Command::ClearTile(c) => check_clear_tile(map, *c).err(),
        Command::SetVehicleOrders(..)
        | Command::SetVehicleStationOrders(..)
        | Command::BuildRoadVehicleAtDepot(..)
        | Command::BuildVehicleAtDepot(..)
        | Command::SellVehicle(..)
        | Command::ToggleVehicleRunning(..)
        | Command::CloneVehicleOrders { .. } => None,
    }
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
