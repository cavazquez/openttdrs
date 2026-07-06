//! Refit de tipo de carga (`OpenTTD` `CMD_REFIT_VEHICLE` simplificado).

use crate::cargo::CargoType;
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
}
