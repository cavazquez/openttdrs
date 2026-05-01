//! Comandos del jugador que mutan [`crate::GameState`] de forma validada e identificable.

mod industry;
mod transport;
mod vehicles;

pub use industry::industry_template;

use crate::map::{TileCoord, TileKind};
use crate::{
    BRIDGE_BUILD_COST_PER_TILE, DEPOT_BUILD_COST, GameState, IndustryKind, IndustrySpec, StopKind,
    TUNNEL_BUILD_COST_PER_TILE, VehicleKind,
};

/// Acción del jugador reproducible (p. ej. log para red en I8).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Command {
    /// Coloca carretera en la tesela (MVP: solo validación de terreno).
    PlaceRoad(TileCoord),
    /// Coloca o combina una pieza de carretera `OpenTTD` (`RoadBits`, bits 0..3).
    PlaceRoadBits(TileCoord, u8),
    /// Reemplaza la geometría de carretera de la tesela con `RoadBits` exactos.
    SetRoadBits(TileCoord, u8),
    /// Coloca via de tren en la tesela (MVP: validacion de terreno).
    PlaceRail(TileCoord),
    PlaceRoadDepot(TileCoord),
    PlaceRoadDepotDir(TileCoord, u8),
    PlaceRailDepot(TileCoord),
    PlaceRoadTunnel(TileCoord, TileCoord),
    PlaceRailTunnel(TileCoord, TileCoord),
    PlaceRoadBridge(TileCoord, TileCoord),
    PlaceRailBridge(TileCoord, TileCoord),
    SetVehicleOrders(u32, Vec<TileCoord>),
    SetVehicleStationOrders(u32, Vec<TileCoord>),
    PlaceHouse(TileCoord),
    PlaceIndustry(TileCoord),
    PlaceIndustryKind(TileCoord, IndustryKind),
    PlaceIndustrySpec(TileCoord, IndustrySpec),
    PlaceForest(TileCoord),
    /// Añade una estación y marca la tesela como `TileKind::Station`.
    PlaceStation(TileCoord),
    /// Añade una estación de carretera con orientación visual `0..3`.
    PlaceStationDir(TileCoord, u8),
    PlaceBusStop(TileCoord, u8),
    PlaceTruckStop(TileCoord, u8),
    BuildRoadVehicleAtDepot(TileCoord, VehicleKind),
    SellVehicle(u32),
    ToggleVehicleRunning(u32),
    CloneVehicleOrders {
        from_vehicle_id: u32,
        to_vehicle_id: u32,
    },
    /// Limpia la tesela y vuelve a `TileKind::Grass`.
    ClearTile(TileCoord),
}

/// Fallo al aplicar un comando (estado sin cambios).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandError {
    OutOfBounds,
    CannotPlaceRoadOnWater,
    CannotPlaceRoadOnVoid,
    CannotPlaceRailOnWater,
    CannotPlaceRailOnVoid,
    CannotPlaceStationOnWater,
    CannotPlaceStationOnVoid,
    StationAlreadyExists,
    StationNotFound,
    VehicleNotFound,
    InvalidDepotTile,
    VehicleKindNotAllowed,
    IncompatibleStopForVehicle,
}

/// Aplica `cmd` a `state` o devuelve error sin mutar.
///
/// # Errors
///
/// Ver variantes de [`CommandError`].
pub fn apply_command(state: &mut GameState, cmd: &Command) -> Result<(), CommandError> {
    match cmd {
        Command::PlaceRoad(c) => transport::place_road(state, *c),
        Command::PlaceRoadBits(c, bits) => transport::place_road_bits(state, *c, *bits),
        Command::SetRoadBits(c, bits) => transport::set_road_bits(state, *c, *bits),
        Command::PlaceRail(c) => transport::place_rail(state, *c),
        Command::PlaceRoadDepot(c) => transport::place_road_depot_dir(state, *c, 0),
        Command::PlaceRoadDepotDir(c, dir) => transport::place_road_depot_dir(state, *c, *dir),
        Command::PlaceRailDepot(c) => transport::place_single_transport_tile(
            state,
            *c,
            TileKind::RailDepot,
            0x10,
            0xC0,
            DEPOT_BUILD_COST,
        ),
        Command::PlaceRoadTunnel(a, b) => transport::place_tunnel_or_bridge(
            state,
            *a,
            *b,
            TileKind::RoadTunnel,
            0x90,
            0x00,
            TUNNEL_BUILD_COST_PER_TILE,
        ),
        Command::PlaceRailTunnel(a, b) => transport::place_tunnel_or_bridge(
            state,
            *a,
            *b,
            TileKind::RailTunnel,
            0x90,
            0x04,
            TUNNEL_BUILD_COST_PER_TILE,
        ),
        Command::PlaceRoadBridge(a, b) => transport::place_tunnel_or_bridge(
            state,
            *a,
            *b,
            TileKind::RoadBridge,
            0x90,
            0x80,
            BRIDGE_BUILD_COST_PER_TILE,
        ),
        Command::PlaceRailBridge(a, b) => transport::place_tunnel_or_bridge(
            state,
            *a,
            *b,
            TileKind::RailBridge,
            0x90,
            0x84,
            BRIDGE_BUILD_COST_PER_TILE,
        ),
        Command::SetVehicleOrders(id, orders) => {
            vehicles::set_vehicle_orders(state, *id, orders.clone())
        }
        Command::SetVehicleStationOrders(id, stations) => {
            vehicles::set_vehicle_station_orders(state, *id, stations.clone())
        }
        Command::PlaceHouse(c) => {
            transport::place_single_transport_tile(state, *c, TileKind::House, 0x30, 0x00, 50)
        }
        Command::PlaceIndustry(c) => industry::place_industry_sandbox(state, *c),
        Command::PlaceIndustryKind(c, kind) => {
            industry::place_industry_kind_sandbox(state, *c, *kind)
        }
        Command::PlaceIndustrySpec(c, spec) => {
            industry::place_industry_spec_sandbox(state, *c, *spec)
        }
        Command::PlaceForest(c) => {
            transport::place_single_transport_tile(state, *c, TileKind::Forest, 0x40, 0x00, 30)
        }
        Command::PlaceStation(c) => transport::place_station(state, *c),
        Command::PlaceStationDir(c, dir) => transport::place_station_dir(state, *c, *dir),
        Command::PlaceBusStop(c, dir) => {
            transport::place_stop_kind(state, *c, *dir, StopKind::BusStop)
        }
        Command::PlaceTruckStop(c, dir) => {
            transport::place_stop_kind(state, *c, *dir, StopKind::TruckStop)
        }
        Command::BuildRoadVehicleAtDepot(c, kind) => {
            vehicles::build_road_vehicle_at_depot(state, *c, *kind)
        }
        Command::SellVehicle(id) => vehicles::sell_vehicle(state, *id),
        Command::ToggleVehicleRunning(id) => vehicles::toggle_vehicle_running(state, *id),
        Command::CloneVehicleOrders {
            from_vehicle_id,
            to_vehicle_id,
        } => vehicles::clone_vehicle_orders(state, *from_vehicle_id, *to_vehicle_id),
        Command::ClearTile(c) => transport::clear_tile(state, *c),
    }
}

pub(super) fn in_bounds(map: &crate::map::Map, c: TileCoord) -> Result<(), CommandError> {
    if map.get(c).is_none() {
        Err(CommandError::OutOfBounds)
    } else {
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::{GameState, ROAD_BUILD_COST, TileKind, Vehicle};

    #[test]
    fn place_road_mutates_tile_kind() {
        let mut s = GameState::new(8, 8);
        let c = TileCoord::new(3, 4);
        let money_before = s.economy.money;
        assert_eq!(s.map.get_kind(c), Some(TileKind::Grass));
        apply_command(&mut s, &Command::PlaceRoad(c)).unwrap();
        assert_eq!(s.map.get_kind(c), Some(TileKind::Road));
        assert_eq!(s.map.get(c).unwrap().m5 & 0x0F, 0x05);
        assert_eq!((s.map.get(c).unwrap().mapt >> 4) & 0x0F, 2);
        assert_eq!(s.economy.money, money_before - ROAD_BUILD_COST);
    }

    #[test]
    fn place_road_bits_combines_directions() {
        let mut s = GameState::new(8, 8);
        let c = TileCoord::new(3, 4);
        apply_command(&mut s, &Command::PlaceRoadBits(c, 0x05)).unwrap();
        apply_command(&mut s, &Command::PlaceRoadBits(c, 0x0A)).unwrap();
        assert_eq!(s.map.get(c).unwrap().m5 & 0x0F, 0x0F);
    }

    #[test]
    fn set_road_bits_replaces_existing_directions() {
        let mut s = GameState::new(8, 8);
        let c = TileCoord::new(3, 4);
        apply_command(&mut s, &Command::PlaceRoadBits(c, 0x0F)).unwrap();
        apply_command(&mut s, &Command::SetRoadBits(c, 0x0A)).unwrap();
        assert_eq!(s.map.get(c).unwrap().m5 & 0x0F, 0x0A);
    }

    #[test]
    fn set_road_bits_clears_forest_auxiliary_planes() {
        let mut s = GameState::new(8, 8);
        let c = TileCoord::new(3, 4);
        let mut tile = s.map.get(c).unwrap();
        tile.kind = TileKind::Forest;
        tile.mapt = 0x40;
        tile.m5 = 0x83;
        tile.m3 = 0x06;
        tile.m7 = 0x20;
        tile.m8 = 0x1234;
        s.map.set_tile(c, tile).unwrap();

        apply_command(&mut s, &Command::SetRoadBits(c, 0x0A)).unwrap();

        let tile = s.map.get(c).unwrap();
        assert_eq!(tile.kind, TileKind::Road);
        assert_eq!(tile.mapt, 0x20);
        assert_eq!(tile.m5, 0x0A);
        assert_eq!(tile.m3, 0);
        assert_eq!(tile.m7, 0);
        assert_eq!(tile.m8, 0);
    }

    #[test]
    fn place_road_on_water_returns_error() {
        let mut s = GameState::new(8, 8);
        let c = TileCoord::new(1, 1);
        let money_before = s.economy.money;
        s.map.set_kind(c, TileKind::Water).unwrap();
        let e = apply_command(&mut s, &Command::PlaceRoad(c)).unwrap_err();
        assert_eq!(e, CommandError::CannotPlaceRoadOnWater);
        assert_eq!(s.map.get_kind(c), Some(TileKind::Water));
        assert_eq!(s.economy.money, money_before);
    }

    #[test]
    fn command_sequence_is_deterministic() {
        let cmds = [
            Command::PlaceRoad(TileCoord::new(0, 0)),
            Command::PlaceRail(TileCoord::new(0, 1)),
            Command::PlaceRoad(TileCoord::new(1, 0)),
            Command::PlaceStation(TileCoord::new(2, 0)),
            Command::ClearTile(TileCoord::new(1, 0)),
        ];
        let mut a = GameState::new(8, 8);
        let mut b = GameState::new(8, 8);
        for cmd in &cmds {
            apply_command(&mut a, cmd).unwrap();
            apply_command(&mut b, cmd).unwrap();
        }
        let ja = a.save_json().unwrap();
        let jb = b.save_json().unwrap();
        assert_eq!(ja, jb);
    }

    #[test]
    fn place_station_duplicate_errors() {
        let mut s = GameState::new(8, 8);
        let c = TileCoord::new(2, 2);
        apply_command(&mut s, &Command::PlaceStation(c)).unwrap();
        let e = apply_command(&mut s, &Command::PlaceStation(c)).unwrap_err();
        assert_eq!(e, CommandError::StationAlreadyExists);
        assert_eq!(s.stations.len(), 1);
    }

    #[test]
    fn place_station_dir_preserves_orientation_in_m5() {
        let mut s = GameState::new(8, 8);
        let c = TileCoord::new(1, 1);
        apply_command(&mut s, &Command::PlaceStationDir(c, 2)).unwrap();
        let tile = s.map.get(c).unwrap();
        assert_eq!(tile.kind, TileKind::Station);
        assert_eq!((tile.mapt >> 4) & 0x0F, 5);
        assert_eq!(tile.m5 & 0x03, 2);
    }

    #[test]
    fn build_road_vehicle_at_depot_creates_stopped_bus() {
        let mut s = GameState::new(8, 8);
        let depot = TileCoord::new(2, 2);
        apply_command(&mut s, &Command::PlaceRoadDepot(depot)).unwrap();
        apply_command(
            &mut s,
            &Command::BuildRoadVehicleAtDepot(depot, VehicleKind::Bus),
        )
        .unwrap();
        assert_eq!(s.vehicles.len(), 1);
        assert_eq!(s.vehicles[0].kind, VehicleKind::Bus);
        assert!(!s.vehicles[0].running);
    }

    #[test]
    fn place_road_depot_dir_preserves_orientation_in_m5() {
        let mut s = GameState::new(8, 8);
        let depot = TileCoord::new(2, 2);
        apply_command(&mut s, &Command::PlaceRoadDepotDir(depot, 3)).unwrap();
        let tile = s.map.get(depot).unwrap();
        assert_eq!(tile.kind, TileKind::RoadDepot);
        assert_eq!(tile.m5 & 0x03, 3);
        let exit = TileCoord::new(2, 1);
        assert_eq!(s.map.get_kind(exit), Some(TileKind::Road));
        assert_eq!(s.map.get(exit).unwrap().m5 & 0x0F, 0x04);
    }

    #[test]
    fn toggle_road_vehicle_running_targets_depot_exit() {
        let mut s = GameState::new(8, 8);
        let depot = TileCoord::new(2, 2);
        let exit = TileCoord::new(3, 2);
        apply_command(&mut s, &Command::PlaceRoadDepotDir(depot, 2)).unwrap();
        apply_command(&mut s, &Command::PlaceRoad(exit)).unwrap();
        apply_command(
            &mut s,
            &Command::BuildRoadVehicleAtDepot(depot, VehicleKind::Truck),
        )
        .unwrap();

        apply_command(&mut s, &Command::ToggleVehicleRunning(1)).unwrap();

        assert!(s.vehicles[0].running);
        assert_eq!(s.vehicles[0].dest, exit);
    }

    #[test]
    fn toggle_road_vehicle_running_targets_reachable_road_not_depot_mouth() {
        let mut s = GameState::new(8, 8);
        let depot = TileCoord::new(2, 2);
        let far = TileCoord::new(5, 2);
        apply_command(&mut s, &Command::PlaceRoadDepotDir(depot, 2)).unwrap();
        apply_command(&mut s, &Command::PlaceRoadBits(TileCoord::new(4, 2), 0x0A)).unwrap();
        apply_command(&mut s, &Command::PlaceRoadBits(far, 0x0A)).unwrap();
        apply_command(
            &mut s,
            &Command::BuildRoadVehicleAtDepot(depot, VehicleKind::Truck),
        )
        .unwrap();

        apply_command(&mut s, &Command::ToggleVehicleRunning(1)).unwrap();

        assert_eq!(s.vehicles[0].dest, far);
    }

    #[test]
    fn set_vehicle_station_orders_rejects_incompatible_stop_kind() {
        let mut s = GameState::new(8, 8);
        let stop = TileCoord::new(1, 1);
        apply_command(&mut s, &Command::PlaceBusStop(stop, 0)).unwrap();
        s.vehicles
            .push(Vehicle::new(10, VehicleKind::Truck, stop, stop));
        let e =
            apply_command(&mut s, &Command::SetVehicleStationOrders(10, vec![stop])).unwrap_err();
        assert_eq!(e, CommandError::IncompatibleStopForVehicle);
    }

    #[test]
    fn clear_tile_sets_grass_and_removes_station() {
        let mut s = GameState::new(8, 8);
        let c = TileCoord::new(2, 2);
        apply_command(&mut s, &Command::PlaceStation(c)).unwrap();
        assert_eq!(s.map.get_kind(c), Some(TileKind::Station));
        assert_eq!(s.stations.len(), 1);
        apply_command(&mut s, &Command::ClearTile(c)).unwrap();
        assert_eq!(s.map.get_kind(c), Some(TileKind::Grass));
        assert!(s.stations.is_empty());
    }

    #[test]
    fn place_rail_mutates_tile_kind() {
        let mut s = GameState::new(8, 8);
        let c = TileCoord::new(1, 3);
        assert_eq!(s.map.get_kind(c), Some(TileKind::Grass));
        apply_command(&mut s, &Command::PlaceRail(c)).unwrap();
        assert_eq!(s.map.get_kind(c), Some(TileKind::Rail));
    }

    #[test]
    fn bridge_cost_scales_with_line_length() {
        let mut s = GameState::new(8, 8);
        let money_before = s.economy.money;
        apply_command(
            &mut s,
            &Command::PlaceRoadBridge(TileCoord::new(1, 1), TileCoord::new(4, 1)),
        )
        .unwrap();
        assert_eq!(
            s.economy.money,
            money_before - BRIDGE_BUILD_COST_PER_TILE * 4
        );
    }

    #[test]
    fn set_vehicle_orders_assigns_circular_route() {
        let mut s = GameState::new(8, 8);
        s.vehicles.push(crate::Vehicle::new(
            7,
            crate::VehicleKind::Truck,
            TileCoord::new(0, 0),
            TileCoord::new(1, 0),
        ));
        apply_command(
            &mut s,
            &Command::SetVehicleOrders(7, vec![TileCoord::new(2, 0), TileCoord::new(2, 2)]),
        )
        .unwrap();
        assert_eq!(s.vehicles[0].dest, TileCoord::new(2, 0));
        assert_eq!(s.vehicles[0].orders.len(), 2);
    }

    #[test]
    fn set_vehicle_station_orders_requires_existing_stations() {
        let mut s = GameState::new(8, 8);
        s.vehicles.push(crate::Vehicle::new(
            7,
            crate::VehicleKind::Truck,
            TileCoord::new(0, 0),
            TileCoord::new(1, 0),
        ));
        let missing = apply_command(
            &mut s,
            &Command::SetVehicleStationOrders(7, vec![TileCoord::new(2, 0)]),
        )
        .unwrap_err();
        assert_eq!(missing, CommandError::StationNotFound);

        s.stations.push(crate::Station::new(TileCoord::new(2, 0)));
        apply_command(
            &mut s,
            &Command::SetVehicleStationOrders(7, vec![TileCoord::new(2, 0)]),
        )
        .unwrap();
        assert!(matches!(
            s.vehicles[0].orders[0],
            crate::VehicleOrder::Station { .. }
        ));
        assert_eq!(s.vehicles[0].dest, TileCoord::new(2, 0));
    }

    #[test]
    fn sandbox_commands_place_visible_tile_kinds() {
        let mut s = GameState::new(8, 8);
        apply_command(&mut s, &Command::PlaceHouse(TileCoord::new(1, 1))).unwrap();
        apply_command(&mut s, &Command::PlaceIndustry(TileCoord::new(2, 1))).unwrap();
        apply_command(&mut s, &Command::PlaceForest(TileCoord::new(3, 1))).unwrap();
        apply_command(
            &mut s,
            &Command::PlaceIndustryKind(TileCoord::new(4, 1), IndustryKind::CoalMine),
        )
        .unwrap();
        assert_eq!(s.map.get_kind(TileCoord::new(1, 1)), Some(TileKind::House));
        assert_eq!(
            s.map.get_kind(TileCoord::new(2, 1)),
            Some(TileKind::Industry)
        );
        assert_eq!(s.map.get_kind(TileCoord::new(3, 1)), Some(TileKind::Forest));
        assert_eq!(
            s.map.get_kind(TileCoord::new(4, 1)),
            Some(TileKind::Industry)
        );
        // CoalMine ahora ocupa múltiples tiles (2x2).
        assert_eq!(
            s.map.get_kind(TileCoord::new(5, 1)),
            Some(TileKind::Industry)
        );
        assert_eq!(
            s.map.get_kind(TileCoord::new(4, 2)),
            Some(TileKind::Industry)
        );
        assert_eq!(
            s.map.get_kind(TileCoord::new(5, 2)),
            Some(TileKind::Industry)
        );
        assert!(s.industries.iter().any(|industry| {
            industry.pos == TileCoord::new(4, 1) && industry.kind == IndustryKind::CoalMine
        }));
    }

    #[test]
    fn clear_any_industry_tile_removes_whole_industry_footprint() {
        let mut s = GameState::new(10, 10);
        let origin = TileCoord::new(2, 2);
        apply_command(
            &mut s,
            &Command::PlaceIndustryKind(origin, IndustryKind::Factory),
        )
        .unwrap();
        assert_eq!(s.industries.len(), 1);
        let target_inside = TileCoord::new(3, 2);
        apply_command(&mut s, &Command::ClearTile(target_inside)).unwrap();
        assert!(s.industries.is_empty());
        // Factory template cubre también (4,3).
        assert_eq!(s.map.get_kind(TileCoord::new(4, 3)), Some(TileKind::Grass));
    }
}
