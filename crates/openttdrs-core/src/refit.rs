//! Refit de tipo de carga (`OpenTTD` `CMD_REFIT_VEHICLE` simplificado).

use crate::cargo::CargoType;
use crate::depot::{rail_depot_for_entrance_tile, rail_depot_mouth_dir};
use crate::map::{Map, TileKind};
use crate::vehicle::{Vehicle, VehicleKind};

const TRUCK_FREIGHT: [CargoType; 29] = [
    CargoType::Mail,
    CargoType::Goods,
    CargoType::Coal,
    CargoType::Wood,
    CargoType::Oil,
    CargoType::Grain,
    CargoType::Wheat,
    CargoType::Maize,
    CargoType::Livestock,
    CargoType::IronOre,
    CargoType::Steel,
    CargoType::Paper,
    CargoType::Gold,
    CargoType::Food,
    CargoType::Rubber,
    CargoType::Fruit,
    CargoType::CopperOre,
    CargoType::Water,
    CargoType::Diamonds,
    CargoType::Sugar,
    CargoType::Toys,
    CargoType::Batteries,
    CargoType::Candy,
    CargoType::Toffee,
    CargoType::Cola,
    CargoType::CottonCandy,
    CargoType::Bubbles,
    CargoType::Plastic,
    CargoType::FizzyDrinks,
];

const TRAIN_FREIGHT: [CargoType; 28] = [
    CargoType::Coal,
    CargoType::Wood,
    CargoType::Oil,
    CargoType::Goods,
    CargoType::Grain,
    CargoType::Wheat,
    CargoType::Maize,
    CargoType::Livestock,
    CargoType::IronOre,
    CargoType::Steel,
    CargoType::Paper,
    CargoType::Gold,
    CargoType::Food,
    CargoType::Rubber,
    CargoType::Fruit,
    CargoType::CopperOre,
    CargoType::Water,
    CargoType::Diamonds,
    CargoType::Sugar,
    CargoType::Toys,
    CargoType::Batteries,
    CargoType::Candy,
    CargoType::Toffee,
    CargoType::Cola,
    CargoType::CottonCandy,
    CargoType::Bubbles,
    CargoType::Plastic,
    CargoType::FizzyDrinks,
];

/// Tipos de carga que el vehículo puede adoptar en depósito.
#[must_use]
pub fn refittable_cargo_types(vehicle: &Vehicle) -> &'static [CargoType] {
    let engine = vehicle.effective_engine();
    match vehicle.kind {
        VehicleKind::Bus | VehicleKind::Tram => &[CargoType::Passengers],
        VehicleKind::Truck => &TRUCK_FREIGHT,
        VehicleKind::Ship => &[CargoType::Goods, CargoType::Oil, CargoType::Valuables],
        VehicleKind::Aircraft => &[CargoType::Passengers, CargoType::Mail],
        VehicleKind::Train => match engine.cargo {
            Some(CargoType::Passengers) => &[CargoType::Passengers],
            Some(CargoType::Mail) => &[CargoType::Mail],
            _ => &TRAIN_FREIGHT,
        },
    }
}

/// Refit según [`EngineDef::refit_mask`] (Action0 train `0x1D`) o listas vanilla.
#[must_use]
pub fn refittable_cargo_types_for_engine(engine: &crate::engine::EngineDef) -> Vec<CargoType> {
    if engine.refit_mask != 0 {
        return crate::cargo::ALL_CARGO_TYPES
            .iter()
            .copied()
            .filter(|c| engine.refit_mask & (1u32 << c.temperate_id()) != 0)
            .collect();
    }
    match engine.kind {
        VehicleKind::Bus | VehicleKind::Tram => vec![CargoType::Passengers],
        VehicleKind::Truck => TRUCK_FREIGHT.to_vec(),
        VehicleKind::Ship => vec![CargoType::Goods, CargoType::Oil, CargoType::Valuables],
        VehicleKind::Aircraft => vec![CargoType::Passengers, CargoType::Mail],
        VehicleKind::Train => match engine.cargo {
            Some(CargoType::Passengers) => vec![CargoType::Passengers],
            Some(CargoType::Mail) => vec![CargoType::Mail],
            _ => TRAIN_FREIGHT.to_vec(),
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
    if vehicle.kind == VehicleKind::Aircraft && map.get_kind(vehicle.pos) == Some(TileKind::Airport)
    {
        // Una tesela Airport no equivale a estar dentro del hangar. Taxi,
        // despegue y aterrizaje deben seguir visibles sobre el footprint.
        return vehicle.aircraft_phase == crate::vehicle::AircraftPhase::InHangar;
    }
    if vehicle_in_depot(map, vehicle.pos) {
        return vehicle_hidden_on_depot_tile(map, vehicle);
    }
    vehicle.kind == VehicleKind::Train
        && !vehicle.running
        && rail_depot_for_entrance_tile(map, vehicle.pos).is_some()
}

/// Tren oculto dentro de un túnel / en la rampa según `_tunnel_visibility_frame`.
#[must_use]
pub fn vehicle_hidden_in_tunnel(
    map: &Map,
    vehicle: &Vehicle,
    pos: crate::TileCoord,
    progress: u8,
) -> bool {
    if vehicle.kind != VehicleKind::Train {
        return false;
    }
    let Some(tile) = map.get(pos) else {
        return false;
    };
    if tile.kind != TileKind::RailTunnel {
        return false;
    }
    let Some((tileh, _)) = crate::map::tile_slope_and_z(map, pos) else {
        return true;
    };
    if !crate::map::is_tunnel_entrance_slope(tileh) {
        // Tramo interior: siempre oculto.
        return true;
    }
    let enter_diag = crate::map::inclined_slope_direction(tileh).unwrap_or(crate::vehicle::DIR_NE);
    let next_is_tunnel = vehicle
        .movement_target()
        .and_then(|p| map.get_kind(p))
        .is_some_and(|k| k == TileKind::RailTunnel);
    if next_is_tunnel {
        // Entrando: visible al inicio de la rampa, oculto al fondo.
        !crate::train_movement::tunnel_hides_train_at_progress(enter_diag, progress)
    } else {
        // Saliendo: oculto al inicio, visible al salir.
        crate::train_movement::tunnel_hides_train_at_progress(enter_diag, progress)
    }
}

/// Oculto en depósito o túnel (render / picking / humo).
#[must_use]
pub fn vehicle_hidden_from_view(
    map: &Map,
    vehicle: &Vehicle,
    pos: crate::TileCoord,
    progress: u8,
) -> bool {
    vehicle_hidden_on_map(map, vehicle) || vehicle_hidden_in_tunnel(map, vehicle, pos, progress)
}

fn vehicle_hidden_on_depot_tile(map: &Map, vehicle: &Vehicle) -> bool {
    if !vehicle.running {
        return true;
    }
    let Some(next) = vehicle.movement_target() else {
        return true;
    };
    if matches!(
        vehicle.kind,
        VehicleKind::Bus | VehicleKind::Truck | VehicleKind::Tram
    ) {
        return matches!(
            vehicle.road_depot_phase,
            crate::vehicle::RoadDepotPhase::InDepot
        );
    }
    if vehicle.kind != VehicleKind::Train {
        return false;
    }
    // `!depot_leave_cleared` ≡ `Track::Depot` / `VehState::Hidden` en OpenTTD.
    if !vehicle.depot_leave_cleared {
        return true;
    }
    if vehicle_in_depot(map, next) {
        return true;
    }
    // Cabeza ya autorizada: visible; residual de progreso solo si aún no cruzó.
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
    fn road_vehicle_in_depot_phase_is_hidden() {
        let mut map = crate::map::Map::new_flat(8, 8, 0);
        let depot = TileCoord::new(3, 3);
        map.set_kind(depot, TileKind::RoadDepot).unwrap();
        let mut bus = Vehicle::new(1, VehicleKind::Bus, depot, depot);
        bus.running = true;
        bus.road_depot_phase = crate::vehicle::RoadDepotPhase::InDepot;
        assert!(vehicle_hidden_on_map(&map, &bus));
    }

    #[test]
    fn aircraft_is_hidden_only_inside_hangar_not_while_taxiing() {
        let mut map = crate::map::Map::new_flat(8, 8, 0);
        let airport = TileCoord::new(3, 3);
        map.set_kind(airport, TileKind::Airport).unwrap();
        let mut aircraft = Vehicle::new(1, VehicleKind::Aircraft, airport, airport);
        aircraft.aircraft_phase = crate::vehicle::AircraftPhase::InHangar;
        assert!(vehicle_hidden_on_map(&map, &aircraft));
        aircraft.aircraft_phase = crate::vehicle::AircraftPhase::Taxi;
        assert!(!vehicle_hidden_on_map(&map, &aircraft));
        aircraft.aircraft_phase = crate::vehicle::AircraftPhase::Landing;
        assert!(!vehicle_hidden_on_map(&map, &aircraft));
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
        // Sin autorización de leave (= Track::Depot) permanece oculto.
        train.depot_leave_cleared = false;
        train.progress = 200;
        assert!(vehicle_hidden_on_map(&map, &train));
        train.depot_leave_cleared = true;
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

    #[test]
    fn train_inside_flat_tunnel_is_hidden() {
        let mut map = crate::map::Map::new_flat(8, 8, 0);
        let c = TileCoord::new(3, 3);
        map.set_kind(c, TileKind::RailTunnel).unwrap();
        let train = Vehicle::new(1, VehicleKind::Train, c, c);
        assert!(vehicle_hidden_in_tunnel(&map, &train, c, 0));
        assert!(vehicle_hidden_from_view(&map, &train, c, 128));
    }

    #[test]
    fn train_entering_tunnel_ramp_hides_after_visibility_frame() {
        use std::collections::VecDeque;
        let mut map = crate::map::Map::new_flat(12, 12, 1);
        // Pendiente NE en (5,5): boca de túnel.
        map.set_height(TileCoord::new(5, 5), 2).unwrap();
        map.set_height(TileCoord::new(5, 6), 2).unwrap();
        map.set_height(TileCoord::new(6, 5), 1).unwrap();
        map.set_height(TileCoord::new(6, 6), 1).unwrap();
        let entrance = TileCoord::new(5, 5);
        let interior = TileCoord::new(4, 5);
        map.set_kind(entrance, TileKind::RailTunnel).unwrap();
        map.set_kind(interior, TileKind::RailTunnel).unwrap();
        let mut train = Vehicle::new(1, VehicleKind::Train, entrance, interior);
        train.running = true;
        train.path = VecDeque::from([interior]);
        // Entrando: visible al inicio, oculto tras el frame.
        assert!(!vehicle_hidden_in_tunnel(&map, &train, entrance, 0));
        assert!(vehicle_hidden_in_tunnel(&map, &train, entrance, 200));
    }
}
