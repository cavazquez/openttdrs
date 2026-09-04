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

/// Refit según [`crate::engine::EngineDef`] y su campo `refit_mask`
/// (Action0 train `0x1D`) o listas vanilla.
#[must_use]
pub fn refittable_cargo_types_for_engine(engine: &crate::engine::EngineDef) -> Vec<CargoType> {
    let mut cargos = if engine.cargo_classes_specified {
        let mut class_cargos = crate::cargo::ALL_CARGO_TYPES
            .iter()
            .copied()
            .filter(|cargo| cargo_matches_classes(*cargo, engine, None))
            .collect::<Vec<_>>();
        // OpenTTD applies the legacy refit mask as an XOR over the class mask.
        // Older Rust callers that do not declare classes retain the historical
        // direct-mask behavior in the branch below.
        if engine.refit_mask != 0 {
            class_cargos = crate::cargo::ALL_CARGO_TYPES
                .iter()
                .copied()
                .filter(|cargo| {
                    cargo_matches_classes(*cargo, engine, None)
                        ^ (engine.refit_mask & (1u32 << cargo.temperate_id()) != 0)
                })
                .collect();
        }
        class_cargos
    } else if engine.refit_mask != 0 {
        crate::cargo::ALL_CARGO_TYPES
            .iter()
            .copied()
            .filter(|c| engine.refit_mask & (1u32 << c.temperate_id()) != 0)
            .collect()
    } else {
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
    };
    // CTT include/exclude se aplica sobre la máscara base. Esto conserva los
    // cargos por clases del vehículo y, a la vez, materializa ids custom que
    // no caben en el bitmask histórico de 32 bits.
    for &cargo in &engine.ctt_include_cargos {
        if !cargos.contains(&cargo) {
            cargos.push(cargo);
        }
    }
    cargos.retain(|cargo| !engine.ctt_exclude_cargos.contains(cargo));
    cargos
}

/// Variante que añade los cargos custom registrados por los `NewGRF` activos.
///
/// Las máscaras históricas de `EngineDef` sólo tienen 32 bits; los cargos con
/// ID superior a 31 no pueden expresarse allí y se admiten por la misma regla
/// de clase que el resto del vehículo. El límite coincide con los slots que
/// `CargoStock` puede transportar.
#[must_use]
pub fn refittable_cargo_types_for_engine_with_catalog(
    engine: &crate::engine::EngineDef,
    cargo_catalog: &[crate::cargo_spec::CargoSpecDef],
) -> Vec<CargoType> {
    let mut cargos = if engine.cargo_classes_specified {
        let mut candidates = crate::cargo::ALL_CARGO_TYPES.to_vec();
        for cargo in cargo_catalog
            .iter()
            .filter_map(crate::cargo_spec::CargoSpecDef::cargo_type)
        {
            if !candidates.contains(&cargo) {
                candidates.push(cargo);
            }
        }
        candidates
            .into_iter()
            .filter(|cargo| {
                let class_match = cargo_matches_classes(*cargo, engine, Some(cargo_catalog));
                if engine.refit_mask == 0 {
                    class_match
                } else {
                    class_match
                        ^ (u32::from(cargo.cargo_id()) < u32::BITS
                            && engine.refit_mask & (1_u32 << cargo.cargo_id()) != 0)
                }
            })
            .collect()
    } else {
        let mut base = refittable_cargo_types_for_engine(engine);
        if engine.ctt_include_cargos.is_empty() {
            let custom = cargo_catalog.iter().filter_map(|def| {
                let cargo = def.cargo_type()?;
                let vehicle_allows_freight = match engine.kind {
                    VehicleKind::Truck | VehicleKind::Ship => true,
                    VehicleKind::Train => {
                        !matches!(engine.cargo, Some(CargoType::Passengers | CargoType::Mail))
                    }
                    VehicleKind::Bus | VehicleKind::Tram | VehicleKind::Aircraft => false,
                };
                (cargo.is_freight() && vehicle_allows_freight).then_some(cargo)
            });
            for cargo in custom {
                let mask_allows = engine.refit_mask == 0
                    || (u32::from(cargo.cargo_id()) < u32::BITS
                        && engine.refit_mask & (1_u32 << cargo.cargo_id()) != 0);
                if mask_allows && !base.contains(&cargo) {
                    base.push(cargo);
                }
            }
        }
        base
    };
    cargos.retain(|cargo| !engine.ctt_exclude_cargos.contains(cargo));
    cargos
}

/// Devuelve las clases efectivas de un cargo. Los `CargoSpecDef` del catálogo
/// tienen prioridad para cargos custom y para overrides vanilla; si no existe
/// spec, se usa la tabla vanilla de [`CargoType`].
fn cargo_classes(
    cargo: CargoType,
    cargo_catalog: Option<&[crate::cargo_spec::CargoSpecDef]>,
) -> u16 {
    cargo_catalog
        .and_then(|catalog| crate::cargo_spec::cargo_spec_for_type(catalog, cargo))
        .map_or_else(|| cargo.classes(), |def| def.classes)
}

fn cargo_matches_classes(
    cargo: CargoType,
    engine: &crate::engine::EngineDef,
    cargo_catalog: Option<&[crate::cargo_spec::CargoSpecDef]>,
) -> bool {
    let classes = cargo_classes(cargo, cargo_catalog);
    classes & engine.cargo_classes_allowed != 0
        && classes & engine.cargo_classes_required == engine.cargo_classes_required
        && classes & engine.cargo_classes_disallowed == 0
}

/// Resuelve las opciones de un vehículo usando su motor efectivo y el
/// catálogo de cargos custom. Es la entrada para comandos que ya poseen
/// `GameState`; las APIs históricas sin catálogo quedan sin cambios.
#[must_use]
pub fn refittable_cargo_types_with_catalog(
    vehicle: &Vehicle,
    engine_catalog: &[crate::engine::EngineDef],
    cargo_catalog: &[crate::cargo_spec::CargoSpecDef],
) -> Vec<CargoType> {
    if let Some(engine) = vehicle
        .engine_id
        .and_then(|id| crate::engine::engine_in_catalog(engine_catalog, id))
    {
        return refittable_cargo_types_for_engine_with_catalog(engine, cargo_catalog);
    }
    let mut cargos = refittable_cargo_types(vehicle).to_vec();
    let allows_freight = matches!(
        vehicle.kind,
        VehicleKind::Truck | VehicleKind::Train | VehicleKind::Ship
    );
    if allows_freight {
        for def in cargo_catalog {
            let Some(cargo) = def.cargo_type() else {
                continue;
            };
            if cargo.is_freight() && !cargos.contains(&cargo) {
                cargos.push(cargo);
            }
        }
    }
    cargos
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

/// Variante de [`refit_allowed`] que incluye CTT y cargos custom del catálogo
/// activo al decidir si la ventana de depósito debe ofrecer refit.
#[must_use]
pub fn refit_allowed_with_catalog(
    vehicle: &Vehicle,
    map: &Map,
    engine_catalog: &[crate::engine::EngineDef],
    cargo_catalog: &[crate::cargo_spec::CargoSpecDef],
) -> bool {
    if vehicle.cargo != 0 || !vehicle_in_depot(map, vehicle.pos) {
        return false;
    }
    refittable_cargo_types_with_catalog(vehicle, engine_catalog, cargo_catalog).len() > 1
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
