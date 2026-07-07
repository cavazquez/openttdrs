//! Refit de tipo de carga (`OpenTTD` `CMD_REFIT_VEHICLE` simplificado).

use crate::cargo::CargoType;
use crate::depot::{rail_depot_for_entrance_tile, rail_depot_mouth_dir};
use crate::map::{Map, TileKind};
use crate::vehicle::{Vehicle, VehicleKind};

const TRUCK_FREIGHT: [CargoType; 5] = [
    CargoType::Mail,
    CargoType::Goods,
    CargoType::Coal,
    CargoType::Wood,
    CargoType::Oil,
];

const TRAIN_FREIGHT: [CargoType; 4] = [
    CargoType::Coal,
    CargoType::Wood,
    CargoType::Oil,
    CargoType::Goods,
];

/// Tipos de carga que el vehículo puede adoptar en depósito.
#[must_use]
pub fn refittable_cargo_types(vehicle: &Vehicle) -> &'static [CargoType] {
    let engine = vehicle.effective_engine();
    match vehicle.kind {
        VehicleKind::Bus => &[CargoType::Passengers],
        VehicleKind::Truck => &TRUCK_FREIGHT,
        VehicleKind::Ship => &[CargoType::Goods],
        VehicleKind::Aircraft => &[CargoType::Passengers, CargoType::Mail],
        VehicleKind::Train => match engine.cargo {
            Some(CargoType::Passengers) => &[CargoType::Passengers],
            Some(CargoType::Mail) => &[CargoType::Mail],
            Some(CargoType::Goods) => &[CargoType::Goods],
            Some(CargoType::Coal) => &[CargoType::Coal],
            Some(CargoType::Wood) => &[CargoType::Wood],
            Some(CargoType::Oil) => &[CargoType::Oil],
            None => &TRAIN_FREIGHT,
        },
    }
}

#[must_use]
pub fn vehicle_in_depot(map: &Map, pos: crate::TileCoord) -> bool {
    matches!(
        map.get_kind(pos),
        Some(TileKind::RoadDepot | TileKind::RailDepot | TileKind::ShipDepot | TileKind::Airport)
    )
}

/// Progreso mínimo en tesela de depósito para dibujar un tren saliendo por la boca
/// (`_fractcoords_behind` → `_fractcoords_enter` en `OpenTTD`).
const TRAIN_DEPOT_EXIT_VISIBILITY_PROGRESS: u8 = 192;

/// Vehículo oculto en el mapa isométrico (`OpenTTD` `VS_HIDDEN` al entrar al depósito).
#[must_use]
pub fn vehicle_hidden_on_map(map: &Map, vehicle: &Vehicle) -> bool {
    if vehicle_in_depot(map, vehicle.pos) {
        return vehicle_hidden_on_depot_tile(map, vehicle);
    }
    vehicle.kind == VehicleKind::Train
        && !vehicle.running
        && rail_depot_for_entrance_tile(map, vehicle.pos).is_some()
}

fn vehicle_hidden_on_depot_tile(map: &Map, vehicle: &Vehicle) -> bool {
    if !vehicle.running {
        return true;
    }
    let Some(next) = vehicle.movement_target() else {
        return true;
    };
    if vehicle.kind != VehicleKind::Train {
        return false;
    }
    if vehicle_in_depot(map, next) {
        return true;
    }
    rail_depot_mouth_dir(map, vehicle.pos)
        .is_some_and(|_| vehicle.progress < TRAIN_DEPOT_EXIT_VISIBILITY_PROGRESS)
}

/// Siguiente tipo en la lista de refit (ciclo).
#[must_use]
pub fn next_refit_cargo(vehicle: &Vehicle) -> Option<CargoType> {
    let options = refittable_cargo_types(vehicle);
    if options.len() <= 1 {
        return None;
    }
    let current = vehicle.cargo_type.unwrap_or(options[0]);
    let idx = options.iter().position(|&c| c == current).unwrap_or(0);
    Some(options[(idx + 1) % options.len()])
}

#[must_use]
pub fn refit_allowed(vehicle: &Vehicle, map: &Map) -> bool {
    vehicle.cargo == 0
        && vehicle_in_depot(map, vehicle.pos)
        && refittable_cargo_types(vehicle).len() > 1
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use crate::{TileCoord, Vehicle, VehicleKind};

    use super::*;

    #[test]
    fn truck_has_multiple_refit_options() {
        let v = Vehicle::new(
            1,
            VehicleKind::Truck,
            TileCoord::new(0, 0),
            TileCoord::new(0, 0),
        );
        assert!(refittable_cargo_types(&v).len() > 1);
    }

    #[test]
    fn bus_only_passengers() {
        let v = Vehicle::new(
            1,
            VehicleKind::Bus,
            TileCoord::new(0, 0),
            TileCoord::new(0, 0),
        );
        assert_eq!(refittable_cargo_types(&v), &[CargoType::Passengers]);
        assert!(next_refit_cargo(&v).is_none());
    }

    #[test]
    fn next_refit_cycles_freight() {
        let mut v = Vehicle::new(
            1,
            VehicleKind::Truck,
            TileCoord::new(0, 0),
            TileCoord::new(0, 0),
        );
        v.cargo_type = Some(CargoType::Coal);
        assert_eq!(next_refit_cargo(&v), Some(CargoType::Wood));
    }

    #[test]
    fn stopped_train_in_depot_is_hidden_on_map() {
        let mut map = crate::map::Map::new_flat(8, 8, 0);
        let depot = TileCoord::new(3, 3);
        map.set_kind(depot, TileKind::RailDepot).unwrap();
        let mut train = Vehicle::new(1, VehicleKind::Train, depot, depot);
        train.running = false;
        assert!(vehicle_hidden_on_map(&map, &train));
    }

    #[test]
    fn running_train_dwelling_in_depot_is_hidden_on_map() {
        let mut map = crate::map::Map::new_flat(8, 8, 0);
        let depot = TileCoord::new(3, 3);
        map.set_kind(depot, TileKind::RailDepot).unwrap();
        let mut train = Vehicle::new(1, VehicleKind::Train, depot, depot);
        train.running = true;
        train.cur_speed = 48;
        assert!(vehicle_hidden_on_map(&map, &train));
    }

    #[test]
    fn train_leaving_depot_shows_near_mouth_only() {
        use std::collections::VecDeque;

        let mut map = crate::map::Map::new_flat(8, 8, 0);
        let depot = TileCoord::new(3, 3);
        map.set_kind(depot, TileKind::RailDepot).unwrap();
        let mut depot_tile = map.get(depot).expect("depot");
        depot_tile.m5 = 2;
        map.set_tile(depot, depot_tile).expect("depot m5");
        let exit = TileCoord::new(4, 3);
        map.set_kind(exit, TileKind::Rail).unwrap();
        let mut train = Vehicle::new(1, VehicleKind::Train, depot, exit);
        train.running = true;
        train.cur_speed = 48;
        train.path = VecDeque::from([exit]);
        train.progress = 64;
        assert!(vehicle_hidden_on_map(&map, &train));
        train.progress = 200;
        assert!(!vehicle_hidden_on_map(&map, &train));
    }

    #[test]
    fn stopped_train_on_depot_entrance_rail_is_hidden() {
        let mut map = crate::map::Map::new_flat(8, 8, 0);
        let depot = TileCoord::new(3, 3);
        map.set_kind(depot, TileKind::RailDepot).unwrap();
        let mut depot_tile = map.get(depot).expect("depot");
        depot_tile.m5 = 2;
        map.set_tile(depot, depot_tile).expect("depot m5");
        let entrance = TileCoord::new(4, 3);
        map.set_kind(entrance, TileKind::Rail).unwrap();
        let mut train = Vehicle::new(1, VehicleKind::Train, entrance, depot);
        train.running = false;
        assert!(vehicle_hidden_on_map(&map, &train));
    }
}
