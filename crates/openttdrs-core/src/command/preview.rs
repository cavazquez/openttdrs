//! Validación de solo lectura alineada con [`apply_command`] (preview / HUD).

use crate::bridge_spec::{bridge_available_at_tick, bridge_build_cost};
use crate::{GameState, IndustrySpec, StopKind};

use super::buy_land::check_buy_land;
use super::industry::check_place_industry_spec;
use super::terraform::{check_level_land, check_lower_land, check_raise_land};
use super::town;
use super::transport::{
    check_airport_area, check_airport_placement, check_bridge, check_clear_tile,
    check_cycle_rail_signal_type, check_dock_placement, check_place_aqueduct, check_place_buoy,
    check_place_canal, check_place_lock, check_place_rail, check_place_rail_signal_oriented,
    check_place_rail_waypoint, check_place_river, check_place_road_bits, check_place_road_waypoint,
    check_rail_depot_placement, check_rail_station_area, check_rail_trackbits_with_autoslope,
    check_remove_rail, check_remove_rail_signal, check_road_depot_placement,
    check_ship_depot_placement, check_single_transport_tile, check_station_placement, check_tunnel,
    merged_rail_trackbits_on_tile, rail_station_footprint, rail_trackbits_from_neighbors,
};
use super::types::{Command, CommandError};
use super::util::require_tile_owned_by_active;

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

#[allow(clippy::too_many_lines)]
fn preview_build_cmd(state: &GameState, cmd: &Command) -> Option<CommandError> {
    let map = &state.map;
    let stations = &state.stations;
    let tick = state.tick.get();
    match cmd {
        Command::PlaceRoad(c)
        | Command::PlaceRoadBits(c, _)
        | Command::PlaceTramBits(c, _)
        | Command::RemoveTramBits(c)
        | Command::SetRoadBits(c, _) => require_tile_owned_by_active(state, *c)
            .err()
            .or_else(|| check_place_road_bits(map, *c).err()),
        Command::PlaceRail(c) => require_tile_owned_by_active(state, *c).err().or_else(|| {
            check_place_rail(map, *c).err().or_else(|| {
                let tb = rail_trackbits_from_neighbors(map, *c);
                check_rail_trackbits_with_autoslope(map, *c, tb, tick).err()
            })
        }),
        Command::PlaceRailBits(c, bits) => {
            require_tile_owned_by_active(state, *c).err().or_else(|| {
                check_place_rail(map, *c).err().or_else(|| {
                    let tb = merged_rail_trackbits_on_tile(map, *c, *bits);
                    check_rail_trackbits_with_autoslope(map, *c, tb, tick).err()
                })
            })
        }
        Command::SetRailBits(c, bits) => require_tile_owned_by_active(state, *c)
            .err()
            .or_else(|| check_place_rail(map, *c).err())
            .or_else(|| check_rail_trackbits_with_autoslope(map, *c, bits & 0x3F, tick).err()),
        Command::PlaceRailWaypoint(c) => require_tile_owned_by_active(state, *c)
            .err()
            .or_else(|| check_place_rail_waypoint(map, *c, stations).err()),
        Command::PlaceRoadWaypoint(c) => require_tile_owned_by_active(state, *c)
            .err()
            .or_else(|| check_place_road_waypoint(map, *c, stations).err()),
        Command::RemoveRailBits(c, _) | Command::RemoveRail(c) => {
            require_tile_owned_by_active(state, *c)
                .err()
                .or_else(|| check_remove_rail(map, *c).err())
        }
        Command::ConvertRail(c, _) => require_tile_owned_by_active(state, *c).err().or_else(|| {
            if map.get_kind(*c) == Some(crate::map::TileKind::Rail) {
                None
            } else {
                Some(CommandError::NoRailToConvert)
            }
        }),
        Command::PlaceRailSignal(c, orientation, fract_x, fract_y, _) => {
            require_tile_owned_by_active(state, *c).err().or_else(|| {
                check_place_rail_signal_oriented(map, *c, *orientation, *fract_x, *fract_y).err()
            })
        }
        Command::CycleRailSignalType(c, fract_x, fract_y) => {
            require_tile_owned_by_active(state, *c)
                .err()
                .or_else(|| check_cycle_rail_signal_type(map, *c, *fract_x, *fract_y).err())
        }
        Command::RemoveRailSignal(c, fract_x, fract_y) => require_tile_owned_by_active(state, *c)
            .err()
            .or_else(|| check_remove_rail_signal(map, *c, *fract_x, *fract_y).err()),
        Command::PlaceRoadDepot(c) => preview_depot_any(map, *c, check_road_depot_placement),
        Command::PlaceRoadDepotDir(c, dir) => check_road_depot_placement(map, *c, *dir).err(),
        Command::PlaceRailDepot(c) => preview_depot_any(map, *c, check_rail_depot_placement),
        Command::PlaceRailDepotDir(c, dir) => check_rail_depot_placement(map, *c, *dir).err(),
        Command::PlaceShipDepotDir(c, dir) => check_ship_depot_placement(map, *c, *dir).err(),
        Command::PlaceDock(c, _) => check_dock_placement(map, &state.stations, *c).err(),
        Command::PlaceAirport(c) => check_airport_placement(map, &state.stations, *c).err(),
        Command::PlaceAirportArea {
            origin,
            axis_y,
            spec,
        } => check_airport_area(state, *origin, *axis_y, *spec).err(),
        Command::PlaceCanal(c) => check_place_canal(map, *c).err(),
        Command::PlaceRiver(c) => check_place_river(map, *c).err(),
        Command::PlaceBuoy(c) => check_place_buoy(map, &state.stations, *c).err(),
        Command::PlaceAqueduct(a, b) => check_place_aqueduct(map, *a, *b).err(),
        Command::PlaceLock(c, axis_y) => check_place_lock(map, *c, *axis_y).err(),
        Command::PlaceHouse(c) | Command::PlaceForest(c) => {
            check_single_transport_tile(map, *c).err()
        }
        Command::PlaceRoadTunnel(a, _) | Command::PlaceRailTunnel(a, _) => {
            check_tunnel(map, *a).err()
        }
        Command::PlaceRoadBridge(a, b, bt) | Command::PlaceRailBridge(a, b, bt) => {
            check_bridge(map, *a, *b)
                .err()
                .or_else(|| {
                    if bridge_available_at_tick(*bt, state.tick, *a, *b) {
                        None
                    } else {
                        Some(CommandError::BridgeTypeNotAvailable)
                    }
                })
                .or_else(|| {
                    if state.economy.money >= bridge_build_cost(*bt, *a, *b) {
                        None
                    } else {
                        Some(CommandError::InsufficientFunds)
                    }
                })
        }
        Command::PlaceStation(c) => {
            if stations.iter().any(|s| s.pos == *c) {
                return Some(CommandError::StationAlreadyExists);
            }
            if !crate::town::authority_allows_new_station(&state.towns, *c) {
                return Some(CommandError::AuthorityRatingTooLow);
            }
            preview_depot_any(map, *c, |m, tile, dir| {
                check_station_placement(m, stations, tile, dir, StopKind::TruckStop)
            })
        }
        Command::PlaceStationDir(c, dir) | Command::PlaceTruckStop(c, dir) => {
            preview_station_with_authority(state, *c, *dir, StopKind::TruckStop)
        }
        Command::PlaceBusStop(c, dir) => {
            preview_station_with_authority(state, *c, *dir, StopKind::BusStop)
        }
        Command::PlaceRailStation(c, dir) => {
            preview_station_with_authority(state, *c, *dir, StopKind::RailStation)
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
        Command::ClearTile(c) => {
            let ownership = if state.cheats.magic_bulldozer_active() {
                None
            } else {
                require_tile_owned_by_active(state, *c).err()
            };
            ownership.or_else(|| check_clear_tile(map, *c).err())
        }
        Command::FoundTown(c) => town::check_found_town(state, *c).err(),
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
        | Command::AttachWagonToConsist { .. }
        | Command::DetachConsistUnit(..)
        | Command::MoveRailVehicle { .. }
        | Command::SellVehicle(..)
        | Command::ToggleVehicleRunning(..)
        | Command::CloneVehicleOrders { .. }
        | Command::CloneVehicleAtDepot { .. }
        | Command::SellAllVehiclesAtDepot(..)
        | Command::RemoveVehicleOrderAt { .. }
        | Command::SkipVehicleOrder(..)
        | Command::ToggleVehicleOrderFullLoad { .. }
        | Command::ToggleVehicleOrderNoUnload { .. }
        | Command::AppendGotoNearestDepot(..)
        | Command::RenameVehicle { .. }
        | Command::RenameStation { .. }
        | Command::SetDepotVehiclesRunning { .. }
        | Command::MoveVehicleOrder { .. }
        | Command::ToggleVehicleOrderDepotStop { .. }
        | Command::TurnAroundVehicle(..)
        | Command::ForceVehicleProceed(..)
        | Command::RefitVehicle { .. }
        | Command::CycleVehicleOrderDepotRefit { .. }
        | Command::ToggleVehicleTimetable(..)
        | Command::CycleVehicleOrderWait { .. }
        | Command::CycleVehicleOrderTravel { .. }
        | Command::SetAutoReplaceRule { .. }
        | Command::ClearAutoReplaceRule { .. }
        | Command::ToggleAutoReplaceRule { .. }
        | Command::CreateVehicleGroup { .. }
        | Command::RenameVehicleGroup { .. }
        | Command::AssignVehicleToGroup { .. }
        | Command::ClearVehicleTimetableLateness(..)
        | Command::SetVehicleOrderWaitTicks { .. }
        | Command::SetVehicleOrderTravelTicks { .. }
        | Command::ToggleVehicleTimetableAutofill(..)
        | Command::ToggleAutoReplaceOnlyWhenOld { .. }
        | Command::SetAutoReplaceRuleGroup { .. }
        | Command::DepotMassAutoreplace { .. }
        | Command::CreateSharedOrdersFromVehicle(..)
        | Command::LinkVehicleToSharedOrders { .. }
        | Command::UnlinkVehicleSharedOrders(..)
        | Command::SetSharedOrderAt { .. }
        | Command::SetVehicleOrderConditional { .. }
        | Command::DepotReorderVehicleSlot { .. }
        | Command::IncreaseLoan
        | Command::DecreaseLoan
        | Command::BuyCompany(_)
        | Command::TownAdvertise(_)
        | Command::TownFundBuildings(_)
        | Command::CheatSetEnabled(_)
        | Command::CheatAddMoney(_)
        | Command::CheatToggleInfiniteMoney
        | Command::CheatToggleMagicBulldozer
        | Command::PlantTree(_)
        | Command::ClearTree(_)
        | Command::PlaceSign { .. }
        | Command::RemoveSign { .. }
        | Command::RenameSign { .. }
        | Command::JoinStations { .. }
        | Command::SetNewGrfEnabled { .. }
        | Command::MoveNewGrfInStack { .. }
        | Command::RemoveNewGrfFromStack { .. }
        | Command::AddNewGrfToStack { .. } => None,
    }
}

fn preview_station_with_authority(
    state: &GameState,
    c: crate::map::TileCoord,
    dir: u8,
    stop_kind: StopKind,
) -> Option<CommandError> {
    if !crate::town::authority_allows_new_station(&state.towns, c) {
        return Some(CommandError::AuthorityRatingTooLow);
    }
    check_station_placement(&state.map, &state.stations, c, dir, stop_kind).err()
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
