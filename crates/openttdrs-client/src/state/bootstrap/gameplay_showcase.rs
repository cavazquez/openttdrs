//! Laboratorio jugable: ciudades, cadenas, hub y vía (fases B1–B4).

use bevy::prelude::*;
use openttdrs_core::Command;
use openttdrs_core::prelude::*;
use openttdrs_core::{CargoType, IndustryKind, IndustrySpec, PathNetwork, find_path};

/// Carretera del barrio residencial (eje X).
pub const SHOWCASE_TOWN_ROAD_Y: i32 = 4;
/// Vía del circuito de tren ampliado.
pub const SHOWCASE_RAIL_Y: i32 = 16;
/// Parada bus «centro».
pub const SHOWCASE_BUS_A: TileCoord = TileCoord::new(15, 3);
/// Parada bus «residencial».
pub const SHOWCASE_BUS_B: TileCoord = TileCoord::new(21, 3);
/// Hub de carga para la fábrica.
pub const SHOWCASE_FACTORY_HUB: TileCoord = TileCoord::new(18, 9);
/// Parada de entrega de madera al hub.
pub const SHOWCASE_WOOD_STATION: TileCoord = TileCoord::new(15, 9);
/// Bosque / fuente de madera.
pub const SHOWCASE_FOREST: TileCoord = TileCoord::new(15, 11);
/// Fábrica (consume madera+carbón del hub).
pub const SHOWCASE_FACTORY: TileCoord = TileCoord::new(19, 11);
/// Estación de tren «oeste» (sobre la vía y=SHOWCASE_RAIL_Y).
pub const SHOWCASE_RAIL_WEST: TileCoord = TileCoord::new(14, SHOWCASE_RAIL_Y);
/// Estación de tren «este».
pub const SHOWCASE_RAIL_EAST: TileCoord = TileCoord::new(21, SHOWCASE_RAIL_Y);
/// Depósito de tren al sur de la vía.
pub const SHOWCASE_RAIL_DEPOT: TileCoord = TileCoord::new(12, 17);

const STATION_ENTRANCE_SOUTH: u8 = 1;

/// Casas, buses, bosque, fábrica, hub, vía extendida y lab de pathfinding.
pub(crate) fn place_gameplay_showcase(state: &mut GameState) {
    state.economy.money = 500_000;
    place_factory_chain_block(state);
    place_town_block(state);
    // Tras industrias: el bosque (15,11) llega a y=15; vía en y=16 (#193).
    place_rail_showcase(state);
    place_pathfinding_lab(state);
    spawn_showcase_vehicles(state);
}

fn place_town_block(state: &mut GameState) {
    for (i, (x, y)) in [(15, 1), (17, 1), (19, 1), (16, 2), (18, 2), (20, 2)]
        .into_iter()
        .enumerate()
    {
        let c = TileCoord::new(x, y);
        let house_id = u16::try_from(8 + (i % 5)).unwrap_or(8);
        let _ = state.map.set_completed_house(c, house_id, 40);
    }
    // Entrada de ciudad: habilita el cartel y la ventana de pueblo del barrio.
    let mut town = openttdrs_core::Town {
        id: 1,
        pos: TileCoord::new(17, 2),
        name: "Villademo".to_string(),
        population: 6 * 8,
        passengers_served: 0,
        mail_served: 0,
        growth_funded: 0,
        ..Default::default()
    };
    town.init_growth_goals(state.climate);
    town.init_grow_counter();
    state.towns.push(town);
    for x in 14..=22_i32 {
        let _ = apply_command(
            state,
            &Command::SetRoadBits(TileCoord::new(x, SHOWCASE_TOWN_ROAD_Y), 0x0A),
        );
    }
    let _ = apply_command(
        state,
        &Command::PlaceBusStop(SHOWCASE_BUS_A, STATION_ENTRANCE_SOUTH),
    );
    let _ = apply_command(
        state,
        &Command::PlaceBusStop(SHOWCASE_BUS_B, STATION_ENTRANCE_SOUTH),
    );
    // El showcase no espera a que el bus marque la primera visita: sin esto,
    // selectgoods deja las paradas sin pasajeros hasta el segundo ciclo de pueblo.
    for station in &mut state.stations {
        if station.stop_kind == StopKind::BusStop {
            station.goods.get_mut(CargoType::Passengers).last_speed = 1;
            station.goods.get_mut(CargoType::Mail).last_speed = 1;
        }
    }
}

fn place_factory_chain_block(state: &mut GameState) {
    let _ = apply_command(
        state,
        &Command::PlaceIndustryKind(SHOWCASE_FOREST, IndustryKind::Forest),
    );
    if let Some(forest) = state
        .industries
        .iter_mut()
        .find(|i| i.pos == SHOWCASE_FOREST)
    {
        forest.stock = 40;
    }

    let _ = apply_command(
        state,
        &Command::PlaceIndustrySpec(SHOWCASE_FACTORY, IndustrySpec::Factory),
    );

    for x in 14..=22_i32 {
        let _ = apply_command(state, &Command::SetRoadBits(TileCoord::new(x, 10), 0x0A));
    }
    // Acceso al hub (18,9): cruce T en (18,10) sin romper el eje E–O.
    let _ = apply_command(state, &Command::PlaceRoadBits(TileCoord::new(18, 10), 0x05));

    let _ = apply_command(
        state,
        &Command::PlaceStationDir(SHOWCASE_FACTORY_HUB, STATION_ENTRANCE_SOUTH),
    );
    let _ = apply_command(
        state,
        &Command::PlaceStationDir(SHOWCASE_WOOD_STATION, STATION_ENTRANCE_SOUTH),
    );
}

fn place_rail_showcase(state: &mut GameState) {
    for x in 12..=22_i32 {
        if x == SHOWCASE_RAIL_WEST.x || x == SHOWCASE_RAIL_EAST.x {
            continue;
        }
        let _ = apply_command(
            state,
            &Command::PlaceRail(TileCoord::new(x, SHOWCASE_RAIL_Y)),
        );
    }
    let _ = apply_command(state, &Command::PlaceRailStation(SHOWCASE_RAIL_WEST, 2));
    let _ = apply_command(state, &Command::PlaceRailStation(SHOWCASE_RAIL_EAST, 0));
    let _ = apply_command(state, &Command::PlaceRailDepotDir(SHOWCASE_RAIL_DEPOT, 3));
}

/// Dos tramos paralelos sin conexión directa; el camión debe usar el conector en x=22.
fn place_pathfinding_lab(state: &mut GameState) {
    for x in 14..=21_i32 {
        let _ = apply_command(state, &Command::SetRoadBits(TileCoord::new(x, 12), 0x0A));
        let _ = apply_command(state, &Command::SetRoadBits(TileCoord::new(x, 13), 0x0A));
    }
    for y in 12..=13_i32 {
        let _ = apply_command(state, &Command::SetRoadBits(TileCoord::new(22, y), 0x0F));
    }
}

fn spawn_showcase_vehicles(state: &mut GameState) {
    spawn_bus_line(state);
    spawn_wood_to_hub_truck(state);
    spawn_rail_shuttle(state);
}

fn spawn_bus_line(state: &mut GameState) {
    let road_start = TileCoord::new(14, SHOWCASE_TOWN_ROAD_Y);
    let mut bus = Vehicle::new(9100, VehicleKind::Bus, road_start, SHOWCASE_BUS_B);
    bus.running = true;
    bus.set_station_orders(vec![SHOWCASE_BUS_A, SHOWCASE_BUS_B]);
    bus.sync_order_destination(&state.map);
    if let Some(path) = find_path(&state.map, road_start, bus.dest, PathNetwork::Road) {
        bus.path = path.into();
    }
    state.vehicles.push(bus);
}

fn spawn_wood_to_hub_truck(state: &mut GameState) {
    let start = TileCoord::new(15, 10);
    let mut truck = Vehicle::new(9101, VehicleKind::Truck, start, SHOWCASE_WOOD_STATION);
    truck.running = true;
    truck.set_station_orders(vec![
        SHOWCASE_WOOD_STATION,
        SHOWCASE_FACTORY_HUB,
        SHOWCASE_WOOD_STATION,
    ]);
    truck.sync_order_destination(&state.map);
    if let Some(path) = find_path(&state.map, start, truck.dest, PathNetwork::Road) {
        truck.path = path.into();
    }
    state.vehicles.push(truck);
}

fn spawn_rail_shuttle(state: &mut GameState) {
    let start = TileCoord::new(14, SHOWCASE_RAIL_Y);
    let mut train = Vehicle::new(9102, VehicleKind::Train, start, SHOWCASE_RAIL_WEST);
    train.running = true;
    train.set_vehicle_orders(vec![
        VehicleOrder::station(SHOWCASE_RAIL_WEST),
        VehicleOrder::station(SHOWCASE_RAIL_EAST),
        VehicleOrder::tile(SHOWCASE_RAIL_DEPOT),
    ]);
    train.sync_order_destination(&state.map);
    let rail_dest = train.dest;
    if start != rail_dest {
        if let Some(path) = find_path(&state.map, start, rail_dest, PathNetwork::Rail) {
            if let Some(&first) = path.first() {
                train.direction = openttdrs_core::direction_from_tile_step(start, first);
            }
            train.path = path.into();
        }
    } else if train.running {
        train.direction = openttdrs_core::DIR_SW;
    }
    state.vehicles.push(train);
}

pub(crate) fn log_gameplay_showcase_zones() {
    info!(
        "Showcase B1–B4: casas y buses ({},{})↔({},{}) en y={SHOWCASE_TOWN_ROAD_Y} | \
         cadena bosque ({},{}) → hub ({},{}) → fábrica ({},{}) | \
         mina legacy ({},{}) con camión #9010 | \
         tren #9102 vía y={SHOWCASE_RAIL_Y} (depósito {},{}) | \
         lab pathfinding: carreteras y=12/13 unidas en x=22",
        SHOWCASE_BUS_A.x,
        SHOWCASE_BUS_A.y,
        SHOWCASE_BUS_B.x,
        SHOWCASE_BUS_B.y,
        SHOWCASE_FOREST.x,
        SHOWCASE_FOREST.y,
        SHOWCASE_FACTORY_HUB.x,
        SHOWCASE_FACTORY_HUB.y,
        SHOWCASE_FACTORY.x,
        SHOWCASE_FACTORY.y,
        super::demo_layout::DEMO_ECONOMY_INDUSTRY.x,
        super::demo_layout::DEMO_ECONOMY_INDUSTRY.y,
        SHOWCASE_RAIL_DEPOT.x,
        SHOWCASE_RAIL_DEPOT.y,
    );
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::assertions_on_constants)]
mod tests {
    use super::*;
    use crate::state::{MAP_H, MAP_W};

    use openttdrs_core::{STATION_COVERAGE_RADIUS, TOWN_PRODUCE_TICKS, station_coverage_at};

    fn showcase_state() -> GameState {
        let mut state = GameState::new(MAP_W, MAP_H);
        super::super::demo_layout::fill_flat_grass(&mut state);
        super::super::demo_layout::place_clean_demo_transport(&mut state);
        super::super::demo_layout::place_demo_economy_loop(&mut state);
        place_gameplay_showcase(&mut state);
        state
    }

    #[test]
    fn showcase_has_town_factory_and_rail_assets() {
        let state = showcase_state();
        assert_eq!(
            state.map.get_kind(TileCoord::new(16, 2)),
            Some(TileKind::House)
        );
        assert!(
            state
                .stations
                .iter()
                .any(|s| s.stop_kind == StopKind::BusStop),
            "paradas bus"
        );
        assert!(
            state
                .industries
                .iter()
                .any(|i| i.spec == Some(IndustrySpec::Factory)),
            "fábrica"
        );
        assert!(
            state
                .vehicles
                .iter()
                .any(|v| v.id == 9100 && v.kind == VehicleKind::Bus),
            "bus showcase"
        );
        assert!(
            state
                .vehicles
                .iter()
                .any(|v| v.id == 9102 && v.kind == VehicleKind::Train),
            "tren showcase"
        );
    }

    #[test]
    fn showcase_town_generates_passengers_over_time() {
        let mut state = showcase_state();
        let bus_stop = state
            .stations
            .iter()
            .find(|s| s.stop_kind == StopKind::BusStop)
            .expect("parada bus");
        let coverage = station_coverage_at(
            &state.map,
            &state.industries,
            bus_stop.pos,
            STATION_COVERAGE_RADIUS,
        );
        assert!(coverage.house_tiles >= 3, "casas en cobertura");
        for _ in 0..TOWN_PRODUCE_TICKS {
            state.step();
        }
        assert!(
            state
                .stations
                .iter()
                .filter(|s| s.stop_kind == StopKind::BusStop)
                .any(|s| s.cargo_stock.passengers > 0),
            "pasajeros en parada"
        );
    }

    #[test]
    fn showcase_train_reaches_rail_depot_for_turnaround() {
        let state = showcase_state();
        assert_eq!(
            state.map.get_kind(SHOWCASE_RAIL_DEPOT),
            Some(TileKind::RailDepot)
        );
        assert!(
            SHOWCASE_RAIL_DEPOT.y > SHOWCASE_RAIL_Y,
            "depósito al sur de la vía (estaciones al norte)"
        );
        assert_eq!(
            state
                .map
                .get_kind(TileCoord::new(SHOWCASE_RAIL_DEPOT.x, SHOWCASE_RAIL_Y)),
            Some(TileKind::Rail),
            "boca norte del depósito da a la vía"
        );
        assert!(
            find_path(
                &state.map,
                SHOWCASE_RAIL_EAST,
                SHOWCASE_RAIL_DEPOT,
                PathNetwork::Rail,
            )
            .is_some(),
            "estación este → depósito por la vía"
        );
    }

    #[test]
    fn showcase_train_finds_path_between_rail_stations() {
        let state = showcase_state();
        let west = openttdrs_core::rail_station_stop_tile(&state.map, SHOWCASE_RAIL_WEST)
            .expect("plataforma oeste");
        let east = openttdrs_core::rail_station_stop_tile(&state.map, SHOWCASE_RAIL_EAST)
            .expect("plataforma este");
        assert_eq!(west.y, SHOWCASE_RAIL_Y);
        assert_eq!(east.y, SHOWCASE_RAIL_Y);
        assert!(
            find_path(&state.map, west, east, PathNetwork::Rail).is_some(),
            "parada oeste → este por plataforma/vía y={SHOWCASE_RAIL_Y}"
        );
        assert!(
            find_path(&state.map, east, west, PathNetwork::Rail).is_some(),
            "parada este → oeste"
        );
    }

    #[test]
    fn showcase_train_enters_rail_station_platform() {
        let mut state = showcase_state();
        let train_idx = state
            .vehicles
            .iter()
            .position(|v| v.id == 9102)
            .expect("tren showcase");
        let mut on_platform = false;
        for _ in 0..600 {
            state.step();
            let pos = state.vehicles[train_idx].pos;
            if state.map.get_kind(pos) == Some(TileKind::Station) {
                on_platform = true;
            }
        }
        assert!(
            on_platform,
            "el tren debe entrar a la plataforma al menos una vez (Rail 3C)"
        );
    }

    #[test]
    fn showcase_town_bus_stays_on_road_network() {
        let mut state = showcase_state();
        let idx = state
            .vehicles
            .iter()
            .position(|v| v.id == 9100)
            .expect("bus showcase");
        assert_eq!(
            state.map.get_kind(state.vehicles[idx].pos),
            Some(TileKind::Road),
            "bus arranca en carretera del barrio"
        );
        for step in 0..1200 {
            state.step();
            let pos = state.vehicles[idx].pos;
            let kind = state.map.get_kind(pos);
            // Fase 2: la tesela de la parada (bahía) es parte del recorrido.
            assert!(
                matches!(
                    kind,
                    Some(TileKind::Road)
                        | Some(TileKind::RoadBridge)
                        | Some(TileKind::RoadTunnel)
                        | Some(TileKind::Station)
                ),
                "bus #9100 fuera de red viaria en {pos:?} (tick {step})"
            );
        }
    }

    #[test]
    fn showcase_wood_truck_stays_on_road_network() {
        let mut state = showcase_state();
        let idx = state
            .vehicles
            .iter()
            .position(|v| v.id == 9101)
            .expect("camión bosque→hub");
        assert_eq!(
            state.map.get_kind(state.vehicles[idx].pos),
            Some(TileKind::Road),
            "arranca sobre carretera"
        );
        assert_eq!(
            state.vehicles[idx].dest, SHOWCASE_WOOD_STATION,
            "destino = tesela de la bahía de madera (Fase 2: entra a la parada)"
        );
        for step in 0..1200 {
            state.step();
            let pos = state.vehicles[idx].pos;
            let kind = state.map.get_kind(pos);
            // Fase 2: la tesela de la parada (bahía) es parte del recorrido.
            assert!(
                matches!(
                    kind,
                    Some(TileKind::Road)
                        | Some(TileKind::RoadBridge)
                        | Some(TileKind::RoadTunnel)
                        | Some(TileKind::Station)
                ),
                "camión #9101 en hierba en {pos:?} (tick {step})"
            );
        }
    }

    #[test]
    fn showcase_factory_chain_moves_wood() {
        let mut state = showcase_state();
        for _ in 0..2400 {
            state.step();
        }
        let hub = state
            .stations
            .iter()
            .find(|s| s.pos == SHOWCASE_FACTORY_HUB)
            .expect("hub");
        assert!(
            hub.cargo_stock.wood > 0 || state.stats.cargo_units_delivered > 0,
            "el camión debe mover madera del bosque al hub"
        );
    }

    #[test]
    fn showcase_path_lab_requires_detour() {
        let state = showcase_state();
        let path = find_path(
            &state.map,
            TileCoord::new(14, 12),
            TileCoord::new(14, 13),
            PathNetwork::Road,
        )
        .expect("conector en x=22");
        assert!(
            path.iter().any(|c| c.x == 22),
            "A* debe rodear por el conector este"
        );
    }

    #[test]
    fn showcase_factory_consumes_hub_cargo() {
        let mut state = GameState::new(MAP_W, MAP_H);
        super::super::demo_layout::fill_flat_grass(&mut state);
        state.economy.money = 500_000;
        place_factory_chain_block(&mut state);
        let hub_idx = state
            .stations
            .iter()
            .position(|s| s.pos == SHOWCASE_FACTORY_HUB)
            .expect("hub en showcase");
        state.stations[hub_idx].cargo_stock.wood = 4;
        state.stations[hub_idx].cargo_stock.coal = 2;
        for _ in 0..512 {
            state.step();
        }
        assert!(
            state
                .industries
                .iter()
                .any(|i| i.spec == Some(IndustrySpec::Factory) && i.stock > 0),
            "fábrica produce con insumos en hub"
        );
    }
}
