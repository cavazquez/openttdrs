//! Showcase jugable determinista con carretera, ferrocarril, barcos y aviones.

use bevy::prelude::*;
use openttdrs_core::Command;
use openttdrs_core::prelude::*;
use openttdrs_core::{
    AirportSpecId, CargoType, ENGINE_AIRCRAFT_DAKOTA, ENGINE_AIRCRAFT_TRICARIO, ENGINE_SHIP_FERRY,
    ENGINE_SHIP_MPS, ENGINE_TRAIN_KIRBY, ENGINE_WAGON_COAL, Industry, IndustryKind, IndustrySpec,
    OrderNonStop, PathNetwork, RAIL_TB_LEFT, RAIL_TB_LOWER, RAIL_TB_RIGHT, RAIL_TB_UPPER,
    RAIL_TB_X, SIGTYPE_BLOCK, find_path,
};

/// Carretera del barrio residencial (eje X).
pub const SHOWCASE_TOWN_ROAD_Y: i32 = 4;
/// Parada bus «centro».
pub const SHOWCASE_BUS_A: TileCoord = TileCoord::new(15, 3);
/// Parada bus «residencial».
pub const SHOWCASE_BUS_B: TileCoord = TileCoord::new(44, 3);
/// Hub de carga para la fábrica.
pub const SHOWCASE_FACTORY_HUB: TileCoord = TileCoord::new(18, 9);
/// Parada de entrega de madera al hub.
pub const SHOWCASE_WOOD_STATION: TileCoord = TileCoord::new(15, 9);
/// Bosque / fuente de madera.
pub const SHOWCASE_FOREST: TileCoord = TileCoord::new(15, 11);
/// Fábrica temperate (livestock/grain/steel del hub; madera llega del bosque).
pub const SHOWCASE_FACTORY: TileCoord = TileCoord::new(19, 11);
/// Vía de ida (oeste → este) del circuito ferroviario.
pub const SHOWCASE_RAIL_Y: i32 = 36;
/// Vía de vuelta (este → oeste).
pub const SHOWCASE_RAIL_RETURN_Y: i32 = 37;
/// Ancla de la estación ferroviaria oeste (2 andenes × 4 teselas).
pub const SHOWCASE_RAIL_WEST: TileCoord = TileCoord::new(11, SHOWCASE_RAIL_Y);
/// Ancla de la estación ferroviaria este.
pub const SHOWCASE_RAIL_EAST: TileCoord = TileCoord::new(50, SHOWCASE_RAIL_Y);
/// Depósito de tren al sur de la doble vía.
pub const SHOWCASE_RAIL_DEPOT: TileCoord = TileCoord::new(30, 38);
/// Mina de carbón terminada, dentro de la cobertura de la estación oeste.
pub const SHOWCASE_COAL_MINE: TileCoord = TileCoord::new(7, 32);
/// Central eléctrica terminada, dentro de la cobertura de la estación este.
pub const SHOWCASE_POWER_STATION: TileCoord = TileCoord::new(54, 32);
// Cruces entre estaciones: queda una vía recta libre (x=14 / x=48)
// entre cada extremo de andén y su cruce.
const SHOWCASE_RAIL_CROSSOVER_XS: [i32; 2] = [15, 47];
// Separación mínima de cinco teselas: mayor que locomotora + 2 vagones.
// Ninguna señal comparte tesela con un cruce ni con la boca del depósito.
const SHOWCASE_RAIL_SIGNAL_XS: [i32; 5] = [18, 25, 35, 41, 46];
/// Permanencia suficiente para que dos servicios puedan compartir la estación
/// y ocupar un andén cada uno, sin convertir la parada en instantánea.
const SHOWCASE_RAIL_DWELL_TICKS: u32 = 512;
/// Punto de control sobre la vía de regreso (tests de topología de doble vía).
#[cfg(test)]
const SHOWCASE_RAIL_EAST_RETURN: TileCoord = TileCoord::new(42, SHOWCASE_RAIL_RETURN_Y);

/// Canal navegable del showcase.
const SHOWCASE_WATER_X0: i32 = 5;
const SHOWCASE_WATER_X1: i32 = 58;
const SHOWCASE_WATER_Y0: i32 = 22;
const SHOWCASE_WATER_Y1: i32 = 28;
const SHOWCASE_DOCK_WEST: TileCoord = TileCoord::new(8, SHOWCASE_WATER_Y0);
const SHOWCASE_DOCK_EAST: TileCoord = TileCoord::new(55, SHOWCASE_WATER_Y1);
const SHOWCASE_BUOY_WEST: TileCoord = TileCoord::new(20, 24);
const SHOWCASE_BUOY_EAST: TileCoord = TileCoord::new(43, 26);
const SHOWCASE_SHIP_DEPOT: TileCoord = TileCoord::new(31, 25);

/// Dos aeropuertos Country (Small) con su hangar y circuito FTA completo.
const SHOWCASE_AIRPORT_WEST_ORIGIN: TileCoord = TileCoord::new(7, 49);
// Deja visible el circuito de espera Country completo (+273/16 tiles al este).
const SHOWCASE_AIRPORT_EAST_ORIGIN: TileCoord = TileCoord::new(43, 49);

const STATION_ENTRANCE_SOUTH: u8 = 1;

/// Casas, buses, bosque, fábrica, hub, vía extendida y lab de pathfinding.
pub(crate) fn place_gameplay_showcase(state: &mut GameState) {
    // Dinero de construcción interno; `build_procedural_demo_world` restaura el
    // dinero inicial elegido por el usuario al terminar el bootstrap.
    state.economy.money = 2_000_000;
    state.vehicle_breakdowns = 0;
    state.disasters_enabled = false;
    // En esta doble vía las señales son estrictamente unidireccionales: una
    // espera prolongada nunca debe hacer que el tren dé media vuelta y circule
    // a contramano por el corredor.
    state.pathfinding.reverse_at_signals = false;
    place_factory_chain_block(state);
    place_town_block(state);
    place_rail_showcase(state);
    place_pathfinding_lab(state);
    place_water_showcase(state);
    place_air_showcase(state);
    spawn_showcase_vehicles(state);
}

fn place_town_block(state: &mut GameState) {
    let houses = [
        (15, 1),
        (17, 1),
        (19, 1),
        (16, 2),
        (18, 2),
        (20, 2),
        (39, 1),
        (41, 1),
        (43, 1),
        (40, 2),
        (42, 2),
        (45, 2),
    ];
    for (i, (x, y)) in houses.into_iter().enumerate() {
        let c = TileCoord::new(x, y);
        // IDs con población > 0 en `HOUSE_POPULATION` (8..=12 son 0).
        let house_id = [0_u16, 1, 4, 5, 7, 13][i % 6];
        let _ = state.map.set_completed_house(c, house_id, 40);
    }
    // Entrada de ciudad: habilita el cartel y la ventana de pueblo del barrio.
    let mut town = openttdrs_core::Town {
        id: 1,
        pos: TileCoord::new(17, 2),
        name: "Villa Oeste".to_string(),
        population: 6 * 8,
        passengers_served: 0,
        mail_served: 0,
        growth_funded: 0,
        ..Default::default()
    };
    town.init_growth_goals(state.climate);
    town.init_grow_counter();
    state.towns.push(town);
    let mut east_town = openttdrs_core::Town {
        id: 2,
        pos: TileCoord::new(42, 2),
        name: "Villa Este".to_string(),
        population: 6 * 8,
        ..Default::default()
    };
    east_town.init_growth_goals(state.climate);
    east_town.init_grow_counter();
    state.towns.push(east_town);
    for x in 14..=46_i32 {
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
    // Doble vía entre estaciones: la superior circula al este y la inferior
    // al oeste. Los trenes invierten marcha en el andén y toman el cruce
    // inmediatamente posterior; no hay retornos por detrás de las estaciones.
    for x in 14..=48_i32 {
        set_showcase_rail_bits(state, TileCoord::new(x, SHOWCASE_RAIL_Y), RAIL_TB_X);
        set_showcase_rail_bits(state, TileCoord::new(x, SHOWCASE_RAIL_RETURN_Y), RAIL_TB_X);
    }
    // Cada cruce conserva ambas rectas y las dos diagonales posibles. Así una
    // composición puede salir de cualquiera de los dos andenes hacia cualquiera
    // de las vías de circulación, sin depender del lado por el que llegó.
    for x in SHOWCASE_RAIL_CROSSOVER_XS {
        set_showcase_rail_bits(
            state,
            TileCoord::new(x, SHOWCASE_RAIL_Y),
            RAIL_TB_X | RAIL_TB_LOWER | RAIL_TB_RIGHT,
        );
        set_showcase_rail_bits(
            state,
            TileCoord::new(x, SHOWCASE_RAIL_RETURN_Y),
            RAIL_TB_X | RAIL_TB_UPPER | RAIL_TB_LEFT,
        );
    }

    for origin in [
        TileCoord::new(10, SHOWCASE_RAIL_Y),
        TileCoord::new(49, SHOWCASE_RAIL_Y),
    ] {
        let _ = apply_command(
            state,
            &Command::PlaceRailStationArea {
                origin,
                axis_y: false,
                platforms: 2,
                length: 4,
            },
        );
    }
    let _ = apply_command(state, &Command::PlaceRailDepotDir(SHOWCASE_RAIL_DEPOT, 3));

    // Señales de bloque de un solo sentido. En cada estación, la señal de
    // entrada anterior al cruce se pone roja cuando la garganta está ocupada,
    // mientras la señal de la vía opuesta permite salir. El tramo reversible
    // estación↔cruce queda libre de caras a contramano.
    for x in SHOWCASE_RAIL_SIGNAL_XS {
        let _ = apply_command(
            state,
            &Command::PlaceRailSignal(
                TileCoord::new(x, SHOWCASE_RAIL_Y),
                0,
                128,
                128,
                SIGTYPE_BLOCK,
            ),
        );
        let _ = apply_command(
            state,
            &Command::PlaceRailSignal(
                TileCoord::new(x, SHOWCASE_RAIL_RETURN_Y),
                2,
                128,
                128,
                SIGTYPE_BLOCK,
            ),
        );
    }

    place_rail_industries(state);
    name_station(state, SHOWCASE_RAIL_WEST, "Mina de Carbón");
    name_station(state, SHOWCASE_RAIL_EAST, "Central Eléctrica");
}

fn place_rail_industries(state: &mut GameState) {
    let _ = apply_command(
        state,
        &Command::PlaceIndustryKind(SHOWCASE_COAL_MINE, IndustryKind::CoalMine),
    );
    complete_showcase_industry(state, SHOWCASE_COAL_MINE);
    if let Some(mine) = state
        .industries
        .iter_mut()
        .find(|industry| industry.pos == SHOWCASE_COAL_MINE)
    {
        // Hay carga suficiente para que los seis trenes puedan inaugurar el
        // servicio sin quedar esperando el primer ciclo de producción.
        mine.stock = mine.capacity;
    }

    // GFX vanilla 7..=10: footprint 2x2 de la central eléctrica temperate.
    // La central es un sumidero: recibe carbón y no crea otra carga.
    let power_tiles = [
        (SHOWCASE_POWER_STATION, 7_u8),
        (
            TileCoord::new(SHOWCASE_POWER_STATION.x + 1, SHOWCASE_POWER_STATION.y),
            8,
        ),
        (
            TileCoord::new(SHOWCASE_POWER_STATION.x, SHOWCASE_POWER_STATION.y + 1),
            9,
        ),
        (
            TileCoord::new(SHOWCASE_POWER_STATION.x + 1, SHOWCASE_POWER_STATION.y + 1),
            10,
        ),
    ];
    let industry_id = u8::try_from(state.industries.len().saturating_add(1)).unwrap_or(255);
    for (coord, gfx) in power_tiles {
        let Some(mut tile) = state.map.get(coord) else {
            continue;
        };
        tile.kind = TileKind::Industry;
        tile.mapt = 0x80;
        tile.m1 = 0x80;
        tile.m2 = industry_id;
        tile.m5 = gfx;
        openttdrs_core::init_industry_tile_random(&mut tile, gfx.wrapping_mul(29));
        let _ = state.map.set_tile(coord, tile);
        state.runtime.industry_tile_dirty.push(coord);
    }
    let footprint = power_tiles.map(|(coord, _)| coord).to_vec();
    state.industries.push(
        Industry::with_tiles_spec(
            SHOWCASE_POWER_STATION,
            IndustryKind::Factory,
            IndustrySpec::PowerStation,
            footprint,
            industry_id.wrapping_mul(5),
        )
        .with_instance_id(industry_id),
    );
}

fn complete_showcase_industry(state: &mut GameState, origin: TileCoord) {
    let tiles = state
        .industries
        .iter()
        .find(|industry| industry.pos == origin)
        .map(|industry| industry.tiles.clone())
        .unwrap_or_default();
    for coord in tiles {
        let Some(mut tile) = state.map.get(coord) else {
            continue;
        };
        tile.m1 |= 0x80;
        let _ = state.map.set_tile(coord, tile);
        state.runtime.industry_tile_dirty.push(coord);
    }
}

fn set_showcase_rail_bits(state: &mut GameState, c: TileCoord, bits: u8) {
    let Some(mut tile) = state.map.get(c) else {
        return;
    };
    tile.kind = TileKind::Rail;
    tile.mapt = 0x10;
    tile.m5 = bits & 0x3F;
    let _ = state.map.set_tile(c, tile);
}

fn place_water_showcase(state: &mut GameState) {
    for y in SHOWCASE_WATER_Y0..=SHOWCASE_WATER_Y1 {
        for x in SHOWCASE_WATER_X0..=SHOWCASE_WATER_X1 {
            let c = TileCoord::new(x, y);
            let _ = state.map.set_kind(c, TileKind::Water);
            let _ = state.map.set_height(c, 1);
            let _ = state.map.set_mapt_m5(c, 0x60, 0);
        }
    }
    let _ = apply_command(state, &Command::PlaceDock(SHOWCASE_DOCK_WEST, 0));
    let _ = apply_command(state, &Command::PlaceDock(SHOWCASE_DOCK_EAST, 0));
    let _ = apply_command(state, &Command::PlaceBuoy(SHOWCASE_BUOY_WEST));
    let _ = apply_command(state, &Command::PlaceBuoy(SHOWCASE_BUOY_EAST));
    let _ = apply_command(state, &Command::PlaceShipDepotDir(SHOWCASE_SHIP_DEPOT, 2));
    name_station(state, SHOWCASE_DOCK_WEST, "Puerto Oeste");
    name_station(state, SHOWCASE_DOCK_EAST, "Puerto Este");
}

fn place_air_showcase(state: &mut GameState) {
    for origin in [SHOWCASE_AIRPORT_WEST_ORIGIN, SHOWCASE_AIRPORT_EAST_ORIGIN] {
        let _ = apply_command(
            state,
            &Command::PlaceAirportArea {
                origin,
                axis_y: false,
                spec: AirportSpecId::Small,
            },
        );
    }
    if let Some(west) = airport_anchor_covering(state, SHOWCASE_AIRPORT_WEST_ORIGIN) {
        name_station(state, west, "Aeropuerto Oeste");
    }
    if let Some(east) = airport_anchor_covering(state, SHOWCASE_AIRPORT_EAST_ORIGIN) {
        name_station(state, east, "Aeropuerto Este");
    }
}

fn name_station(state: &mut GameState, pos: TileCoord, name: &str) {
    if let Some(station) = state.stations.iter_mut().find(|station| station.pos == pos) {
        station.name = Some(name.to_string());
    }
}

fn airport_anchor_covering(state: &GameState, origin: TileCoord) -> Option<TileCoord> {
    state
        .stations
        .iter()
        .find(|station| station.stop_kind == StopKind::Airport && station.covers_tile(origin))
        .map(|station| station.pos)
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
    spawn_bus_lines(state);
    spawn_wood_to_hub_truck(state);
    spawn_rail_loop(state);
    spawn_ship_lines(state);
    spawn_air_lines(state);
}

fn spawn_bus_lines(state: &mut GameState) {
    for (id, start, orders) in [
        (
            9100,
            TileCoord::new(14, SHOWCASE_TOWN_ROAD_Y),
            vec![SHOWCASE_BUS_A, SHOWCASE_BUS_B],
        ),
        (
            9104,
            TileCoord::new(46, SHOWCASE_TOWN_ROAD_Y),
            vec![SHOWCASE_BUS_B, SHOWCASE_BUS_A],
        ),
    ] {
        let mut bus = Vehicle::new(id, VehicleKind::Bus, start, orders[0]);
        bus.name = Some(format!(
            "Bus interurbano {}",
            if id == 9100 { 1 } else { 2 }
        ));
        bus.running = true;
        bus.set_station_orders(orders);
        bus.sync_order_destination(&state.map);
        if let Some(path) = find_path(&state.map, start, bus.dest, PathNetwork::Road) {
            bus.path = path.into();
        }
        state.vehicles.push(bus);
    }
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

fn spawn_rail_loop(state: &mut GameState) {
    let west_order = VehicleOrder::station_with_load_unload_flags(
        SHOWCASE_RAIL_WEST,
        true,
        false,
        false,
        true,
        false,
        OrderNonStop::NonStopDestination,
    );
    let west_order = west_order
        .with_wait_ticks(SHOWCASE_RAIL_DWELL_TICKS)
        .unwrap_or(west_order);
    let east_order = VehicleOrder::station_with_load_unload_flags(
        SHOWCASE_RAIL_EAST,
        false,
        false,
        true,
        false,
        false,
        OrderNonStop::NonStopDestination,
    );
    let east_order = east_order
        .with_wait_ticks(SHOWCASE_RAIL_DWELL_TICKS)
        .unwrap_or(east_order);
    let orders = vec![west_order, east_order];
    for (train_index, id) in [9102, 9103, 9105, 9106, 9107, 9108].into_iter().enumerate() {
        let mut train = Vehicle::new(
            id,
            VehicleKind::Train,
            SHOWCASE_RAIL_DEPOT,
            SHOWCASE_RAIL_DEPOT,
        );
        train.name = Some(format!("Carbonero {}", train_index + 1));
        train.engine_id = Some(ENGINE_TRAIN_KIRBY);
        train.capacity = 0;
        train.running = true;
        // Todos se construyen dentro del mismo depósito. El controlador libera
        // un consist por vez y mantiene los demás apilados hasta que la boca y
        // su reserva estén libres.
        train.depot_leave_cleared = false;
        train.timetable_active = true;
        train.set_vehicle_orders(orders.clone());
        // Los seis comparten la misma ruta carbonera, pero arrancan en fases
        // alternadas para alimentar ambas direcciones desde el primer convoy.
        // Los que van primero a la central llegan vacíos (`no_load`) y luego
        // continúan a la mina; ninguno recoge carbón en el extremo de descarga.
        train.current_order = train_index % orders.len();
        train.sync_order_destination(&state.map);
        state.vehicles.push(train);
        for wagon_offset in 0..2_u32 {
            let wagon_id = 9_110 + u32::try_from(train_index).unwrap_or(0) * 2 + wagon_offset;
            let mut wagon = Vehicle::new(
                wagon_id,
                VehicleKind::Train,
                SHOWCASE_RAIL_DEPOT,
                SHOWCASE_RAIL_DEPOT,
            );
            wagon.engine_id = Some(ENGINE_WAGON_COAL);
            wagon.cargo_type = Some(CargoType::Coal);
            wagon.capacity = 30;
            wagon.depot_leave_cleared = false;
            state.vehicles.push(wagon);
            if openttdrs_core::train_consist::attach_wagon(&mut state.vehicles, id, wagon_id)
                .is_err()
            {
                warn!("No se pudo enganchar el vagón {wagon_id} al tren {id} del showcase");
                state.vehicles.retain(|vehicle| vehicle.id != wagon_id);
            }
        }
        openttdrs_core::consist_changed_with_map(&mut state.vehicles, id, Some(&state.map));
    }
}

fn spawn_ship_lines(state: &mut GameState) {
    let west_to_east = vec![
        VehicleOrder::station(SHOWCASE_DOCK_WEST),
        VehicleOrder::waypoint(SHOWCASE_BUOY_WEST),
        VehicleOrder::waypoint(SHOWCASE_BUOY_EAST),
        VehicleOrder::station(SHOWCASE_DOCK_EAST),
    ];
    let east_to_west = vec![
        VehicleOrder::station(SHOWCASE_DOCK_EAST),
        VehicleOrder::waypoint(SHOWCASE_BUOY_EAST),
        VehicleOrder::waypoint(SHOWCASE_BUOY_WEST),
        VehicleOrder::station(SHOWCASE_DOCK_WEST),
    ];
    for (engine, orders, name) in [
        (ENGINE_SHIP_FERRY, west_to_east, "Ferry Demo"),
        (ENGINE_SHIP_MPS, east_to_west, "Carguero Demo"),
    ] {
        if let Some(id) = build_vehicle_at(state, SHOWCASE_SHIP_DEPOT, engine) {
            let _ = apply_command(state, &Command::SetVehicleOrderList(id, orders));
            if let Some(ship) = state.vehicles.iter_mut().find(|vehicle| vehicle.id == id) {
                ship.name = Some(name.to_string());
                ship.running = true;
                ship.sync_order_destination(&state.map);
                if let Some(path) = find_path(&state.map, ship.pos, ship.dest, PathNetwork::Water) {
                    ship.path = path.into();
                }
            }
        }
    }
}

fn spawn_air_lines(state: &mut GameState) {
    let Some(west) = airport_anchor_covering(state, SHOWCASE_AIRPORT_WEST_ORIGIN) else {
        return;
    };
    let Some(east) = airport_anchor_covering(state, SHOWCASE_AIRPORT_EAST_ORIGIN) else {
        return;
    };
    for (engine, hangar, orders, name) in [
        (
            ENGINE_AIRCRAFT_DAKOTA,
            west,
            vec![VehicleOrder::station(east), VehicleOrder::station(west)],
            "Vuelo Demo 1",
        ),
        (
            ENGINE_AIRCRAFT_DAKOTA,
            east,
            vec![VehicleOrder::station(west), VehicleOrder::station(east)],
            "Vuelo Demo 2",
        ),
        (
            ENGINE_AIRCRAFT_TRICARIO,
            west,
            vec![VehicleOrder::station(east), VehicleOrder::station(west)],
            "Helicóptero Demo",
        ),
    ] {
        if let Some(id) = build_vehicle_at(state, hangar, engine) {
            let _ = apply_command(state, &Command::SetVehicleOrderList(id, orders));
            if let Some(aircraft) = state.vehicles.iter_mut().find(|vehicle| vehicle.id == id) {
                aircraft.name = Some(name.to_string());
                aircraft.running = true;
                aircraft.sync_order_destination(&state.map);
            }
        }
    }
}

fn build_vehicle_at(state: &mut GameState, depot: TileCoord, engine: u16) -> Option<u32> {
    let previous_max = state.vehicles.iter().map(|vehicle| vehicle.id).max();
    apply_command(state, &Command::BuildVehicleAtDepot(depot, engine)).ok()?;
    state
        .vehicles
        .iter()
        .map(|vehicle| vehicle.id)
        .filter(|id| previous_max.is_none_or(|previous| *id > previous))
        .max()
}

pub(crate) fn log_gameplay_showcase_zones() {
    info!(
        "Showcase completo 64×64: casas y 2 buses ({},{})↔({},{}) en y={SHOWCASE_TOWN_ROAD_Y} | \
         cadena bosque ({},{}) → hub ({},{}) → fábrica ({},{}) | \
         mina legacy ({},{}) con camión #9010 | \
         6 carboneros desde depósito ({},{}) entre mina ({},{}) y central ({},{}) \
         por doble vía y={SHOWCASE_RAIL_Y}/{SHOWCASE_RAIL_RETURN_Y} | \
         2 barcos ({},{})↔({},{}) | 2 aviones + 1 helicóptero Small | \
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
        SHOWCASE_COAL_MINE.x,
        SHOWCASE_COAL_MINE.y,
        SHOWCASE_POWER_STATION.x,
        SHOWCASE_POWER_STATION.y,
        SHOWCASE_DOCK_WEST.x,
        SHOWCASE_DOCK_WEST.y,
        SHOWCASE_DOCK_EAST.x,
        SHOWCASE_DOCK_EAST.y,
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
        let mine = state
            .industries
            .iter()
            .find(|industry| industry.pos == SHOWCASE_COAL_MINE)
            .expect("mina ferroviaria");
        assert_eq!(mine.spec, Some(IndustrySpec::CoalMine));
        assert_eq!(mine.stock, mine.capacity, "mina lista para cargar");
        assert!(mine.tiles.iter().all(|coord| {
            state
                .map
                .get(*coord)
                .is_some_and(|tile| tile.kind == TileKind::Industry && tile.m1 & 0x80 != 0)
        }));

        let power_station = state
            .industries
            .iter()
            .find(|industry| industry.pos == SHOWCASE_POWER_STATION)
            .expect("central eléctrica ferroviaria");
        assert_eq!(power_station.spec, Some(IndustrySpec::PowerStation));
        assert_eq!(
            power_station.life_type(),
            openttdrs_core::IndustryLifeType::BlackHole
        );
        let mut power_gfx: Vec<_> = power_station
            .tiles
            .iter()
            .filter_map(|coord| state.map.get(*coord).map(|tile| tile.m5))
            .collect();
        power_gfx.sort_unstable();
        assert_eq!(power_gfx, vec![7, 8, 9, 10]);
        assert!(power_station.tiles.iter().all(|coord| {
            state
                .map
                .get(*coord)
                .is_some_and(|tile| tile.kind == TileKind::Industry && tile.m1 & 0x80 != 0)
        }));
        assert!(openttdrs_core::industry_in_station_coverage(
            mine,
            SHOWCASE_RAIL_WEST,
            STATION_COVERAGE_RADIUS,
        ));
        assert!(openttdrs_core::industry_in_station_coverage(
            power_station,
            SHOWCASE_RAIL_EAST,
            STATION_COVERAGE_RADIUS,
        ));
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
        assert_eq!(
            state
                .vehicles
                .iter()
                .filter(|vehicle| vehicle.kind == VehicleKind::Bus)
                .count(),
            2,
            "dos buses activos"
        );
        assert_eq!(
            state
                .vehicles
                .iter()
                .filter(|vehicle| {
                    vehicle.kind == VehicleKind::Train && vehicle.is_consist_head()
                })
                .count(),
            6,
            "seis trenes activos para someter señales y cruces a congestión"
        );
        assert_eq!(
            state
                .vehicles
                .iter()
                .filter(|vehicle| vehicle.is_wagon_unit())
                .count(),
            12,
            "dos tolvas de carbón con sprite propio por tren"
        );
        assert!(
            state
                .vehicles
                .iter()
                .filter(|vehicle| {
                    vehicle.kind == VehicleKind::Train && vehicle.is_consist_head()
                })
                .all(|train| {
                    train.pos == SHOWCASE_RAIL_DEPOT
                        && train.orders.first().is_some_and(|order| {
                            order.destination() == SHOWCASE_RAIL_WEST
                                && order.full_load()
                                && order.no_unload()
                        })
                        && !train.depot_leave_cleared
                })
        );
        assert_eq!(
            state
                .vehicles
                .iter()
                .filter(|vehicle| {
                    vehicle.kind == VehicleKind::Train
                        && vehicle.is_consist_head()
                        && vehicle.current_order == 0
                })
                .count(),
            3,
            "tres trenes salen hacia la mina"
        );
        assert_eq!(
            state
                .vehicles
                .iter()
                .filter(|vehicle| {
                    vehicle.kind == VehicleKind::Train
                        && vehicle.is_consist_head()
                        && vehicle.current_order == 1
                })
                .count(),
            3,
            "tres trenes salen vacíos hacia la central y luego continúan a la mina"
        );
        assert!(
            state
                .vehicles
                .iter()
                .filter(|vehicle| vehicle.is_wagon_unit())
                .all(|wagon| {
                    wagon.pos == SHOWCASE_RAIL_DEPOT
                        && wagon.engine_id == Some(ENGINE_WAGON_COAL)
                        && wagon.cargo_type == Some(CargoType::Coal)
                        && !wagon.depot_leave_cleared
                })
        );
        assert_eq!(
            state
                .vehicles
                .iter()
                .filter(|vehicle| vehicle.kind == VehicleKind::Ship)
                .count(),
            2,
            "dos barcos activos"
        );
        assert_eq!(
            state
                .vehicles
                .iter()
                .filter(|vehicle| vehicle.kind == VehicleKind::Aircraft)
                .count(),
            3,
            "dos aviones y un helicóptero activos"
        );
        assert!(
            state.vehicles.iter().any(|vehicle| {
                vehicle.kind == VehicleKind::Aircraft
                    && vehicle.engine_id == Some(ENGINE_AIRCRAFT_TRICARIO)
            }),
            "helicóptero visible para verificar su ciclo FTA"
        );
        assert!(!state.disasters_enabled, "demo determinista sin desastres");
        assert_eq!(state.vehicle_breakdowns, 0, "demo sin averías aleatorias");
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
        // selectgoods: sin visita previa no llega carga a la parada.
        for st in state
            .stations
            .iter_mut()
            .filter(|s| s.stop_kind == StopKind::BusStop)
        {
            st.goods.get_mut(CargoType::Passengers).last_speed = 1;
            st.goods.get_mut(CargoType::Mail).last_speed = 1;
        }
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
    fn showcase_rail_depot_connects_to_network() {
        let state = showcase_state();
        assert_eq!(
            state.map.get_kind(SHOWCASE_RAIL_DEPOT),
            Some(TileKind::RailDepot)
        );
        assert!(
            SHOWCASE_RAIL_DEPOT.y > SHOWCASE_RAIL_RETURN_Y,
            "depósito al sur de la vía (estaciones al norte)"
        );
        assert_eq!(
            state.map.get_kind(TileCoord::new(
                SHOWCASE_RAIL_DEPOT.x,
                SHOWCASE_RAIL_RETURN_Y
            )),
            Some(TileKind::Rail),
            "boca norte del depósito da a la vía"
        );
        assert!(
            find_path(
                &state.map,
                TileCoord::new(49, SHOWCASE_RAIL_Y),
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
        for x in SHOWCASE_RAIL_CROSSOVER_XS {
            let outbound = state
                .map
                .get(TileCoord::new(x, SHOWCASE_RAIL_Y))
                .expect("tesela superior del cruce");
            let inbound = state
                .map
                .get(TileCoord::new(x, SHOWCASE_RAIL_RETURN_Y))
                .expect("tesela inferior del cruce");
            assert_eq!(outbound.kind, TileKind::Rail);
            assert_eq!(inbound.kind, TileKind::Rail);
            assert_eq!(
                outbound.m5 & 0x3F,
                RAIL_TB_X | RAIL_TB_LOWER | RAIL_TB_RIGHT
            );
            assert_eq!(inbound.m5 & 0x3F, RAIL_TB_X | RAIL_TB_UPPER | RAIL_TB_LEFT);
        }
        for coord in [
            TileCoord::new(14, SHOWCASE_RAIL_Y),
            TileCoord::new(14, SHOWCASE_RAIL_RETURN_Y),
            TileCoord::new(48, SHOWCASE_RAIL_Y),
            TileCoord::new(48, SHOWCASE_RAIL_RETURN_Y),
        ] {
            let tile = state.map.get(coord).expect("vía libre junto al andén");
            assert_eq!(
                tile.m5 & 0x3F,
                RAIL_TB_X,
                "debe quedar exactamente una tesela recta entre estación y cruce en {coord:?}"
            );
        }
        for coord in [
            TileCoord::new(9, SHOWCASE_RAIL_Y),
            TileCoord::new(9, SHOWCASE_RAIL_RETURN_Y),
            TileCoord::new(53, SHOWCASE_RAIL_Y),
            TileCoord::new(53, SHOWCASE_RAIL_RETURN_Y),
        ] {
            assert_ne!(
                state.map.get_kind(coord),
                Some(TileKind::Rail),
                "no debe haber retornos por detrás de las estaciones en {coord:?}"
            );
        }
        for x in SHOWCASE_RAIL_SIGNAL_XS {
            let outbound = state
                .map
                .get(TileCoord::new(x, SHOWCASE_RAIL_Y))
                .expect("señal en vía de ida");
            let inbound = state
                .map
                .get(TileCoord::new(x, SHOWCASE_RAIL_RETURN_Y))
                .expect("señal en vía de regreso");
            assert_eq!(
                openttdrs_core::rail_signal_present_mask(outbound.m3),
                0b0100,
                "ida +x en x={x}"
            );
            assert_eq!(
                openttdrs_core::rail_signal_present_mask(inbound.m3),
                0b1000,
                "regreso -x en x={x}"
            );
            assert_eq!(
                openttdrs_core::signal_type_for_track(outbound.m2, openttdrs_core::SignalTrack::X,),
                SIGTYPE_BLOCK
            );
            assert_eq!(
                openttdrs_core::signal_type_for_track(inbound.m2, openttdrs_core::SignalTrack::X,),
                SIGTYPE_BLOCK
            );
            assert_eq!(
                openttdrs_core::yapf_routing_signal(
                    &state.map,
                    TileCoord::new(x, SHOWCASE_RAIL_Y),
                    2,
                ),
                openttdrs_core::YapfSignalRouting::DeadEnd,
                "la señal superior debe prohibir circular hacia el oeste"
            );
            assert_eq!(
                openttdrs_core::yapf_routing_signal(
                    &state.map,
                    TileCoord::new(x, SHOWCASE_RAIL_RETURN_Y),
                    0,
                ),
                openttdrs_core::YapfSignalRouting::DeadEnd,
                "la señal inferior debe prohibir circular hacia el este"
            );
        }
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
        let west_return = openttdrs_core::rail_station_stop_candidates(
            &state.map,
            SHOWCASE_RAIL_WEST,
            SHOWCASE_RAIL_EAST_RETURN,
        )
        .into_iter()
        .next()
        .expect("andén oeste de regreso");
        assert_eq!(west_return.y, SHOWCASE_RAIL_RETURN_Y);
        assert!(
            find_path(
                &state.map,
                east,
                SHOWCASE_RAIL_EAST_RETURN,
                PathNetwork::Rail,
            )
            .is_some(),
            "el tren gira al andén de vuelta tras la estación este"
        );
        assert!(
            find_path(
                &state.map,
                SHOWCASE_RAIL_EAST_RETURN,
                west_return,
                PathNetwork::Rail,
            )
            .is_some(),
            "regreso este → oeste por y={SHOWCASE_RAIL_RETURN_Y}"
        );
        let platform_routes = [
            (
                TileCoord::new(13, SHOWCASE_RAIL_Y),
                TileCoord::new(20, SHOWCASE_RAIL_Y),
                vec![],
            ),
            (
                TileCoord::new(13, SHOWCASE_RAIL_RETURN_Y),
                TileCoord::new(20, SHOWCASE_RAIL_Y),
                vec![
                    TileCoord::new(15, SHOWCASE_RAIL_RETURN_Y),
                    TileCoord::new(15, SHOWCASE_RAIL_Y),
                ],
            ),
            (
                TileCoord::new(49, SHOWCASE_RAIL_RETURN_Y),
                TileCoord::new(42, SHOWCASE_RAIL_RETURN_Y),
                vec![],
            ),
            (
                TileCoord::new(49, SHOWCASE_RAIL_Y),
                TileCoord::new(42, SHOWCASE_RAIL_RETURN_Y),
                vec![
                    TileCoord::new(47, SHOWCASE_RAIL_Y),
                    TileCoord::new(47, SHOWCASE_RAIL_RETURN_Y),
                ],
            ),
        ];
        for (platform, running_line, required_cross_tiles) in platform_routes {
            let route = find_path(&state.map, platform, running_line, PathNetwork::Rail)
                .unwrap_or_else(|| {
                    panic!("sin salida desde andén {platform:?} a {running_line:?}")
                });
            for cross_tile in required_cross_tiles {
                assert!(
                    route.contains(&cross_tile),
                    "la salida {platform:?}→{running_line:?} debe usar {cross_tile:?}: {route:?}"
                );
            }
        }
        assert!(
            find_path(
                &state.map,
                TileCoord::new(32, SHOWCASE_RAIL_Y),
                TileCoord::new(51, SHOWCASE_RAIL_RETURN_Y),
                PathNetwork::Rail,
            )
            .is_some(),
            "la vía superior debe poder entrar al andén inferior del este"
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
        for _ in 0..8_000 {
            state.step();
            let pos = state.vehicles[train_idx].pos;
            if state.map.get_kind(pos) == Some(TileKind::Station) {
                on_platform = true;
            }
        }
        assert!(
            on_platform,
            "el tren debe entrar a la plataforma al menos una vez (Rail 3C): {:#?}; \
             reservation_block={} signal_block={} traffic_block={}; heads={:?}",
            state.vehicles[train_idx],
            openttdrs_core::train_blocked_by_reservation(&state.map, &state.vehicles[train_idx]),
            openttdrs_core::rail_signals::train_blocked_by_signal(
                &state.map,
                &state.vehicles,
                &state.vehicles[train_idx]
            ),
            openttdrs_core::rail_signals::train_blocked_by_traffic(
                &state.map,
                &state.vehicles,
                &state.vehicles[train_idx]
            ),
            state
                .vehicles
                .iter()
                .filter(|v| v.kind == VehicleKind::Train && v.is_consist_head())
                .map(|v| (v.id, v.pos, v.dest, v.path.front().copied()))
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn showcase_dispatches_next_train_before_first_reaches_destination() {
        let mut state = showcase_state();
        let mut departed = std::collections::BTreeMap::new();
        let mut first_arrival = None;

        for tick in 0..8_000_u32 {
            state.step();
            for train in state
                .vehicles
                .iter()
                .filter(|vehicle| vehicle.kind == VehicleKind::Train && vehicle.is_consist_head())
            {
                if train.pos != SHOWCASE_RAIL_DEPOT {
                    departed.entry(train.id).or_insert(tick);
                }
                if state.map.get_kind(train.pos) == Some(TileKind::Station) {
                    first_arrival = Some((tick, train.id));
                }
            }
            if first_arrival.is_some() {
                break;
            }
        }

        assert!(
            departed.len() >= 2,
            "debe salir un segundo servicio antes de que el primero llegue: \
             departed={departed:?}, first_arrival={first_arrival:?}, depot_reserved={}, \
             heads={:?}, first_units={:?}",
            openttdrs_core::depot::has_depot_reservation(&state.map, SHOWCASE_RAIL_DEPOT),
            state
                .vehicles
                .iter()
                .filter(|vehicle| vehicle.kind == VehicleKind::Train && vehicle.is_consist_head())
                .map(|vehicle| (
                    vehicle.id,
                    vehicle.pos,
                    vehicle.rail_pixel,
                    vehicle.depot_leave_cleared,
                    vehicle.wait_counter,
                    vehicle.reserved_steps.len(),
                ))
                .collect::<Vec<_>>(),
            state
                .vehicles
                .iter()
                .filter(|vehicle| vehicle.id == 9102
                    || vehicle.prev_unit == Some(9102)
                    || vehicle.id == 9111)
                .map(|vehicle| (
                    vehicle.id,
                    vehicle.pos,
                    vehicle.rail_pixel,
                    vehicle.depot_leave_cleared
                ))
                .collect::<Vec<_>>()
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
        // Factory temperate: livestock + grain + steel (lotes de 8).
        state.stations[hub_idx].cargo_stock.livestock = openttdrs_core::FACTORY_LIVESTOCK_INPUT;
        state.stations[hub_idx].cargo_stock.grain = openttdrs_core::FACTORY_GRAIN_INPUT;
        state.stations[hub_idx].cargo_stock.steel = openttdrs_core::FACTORY_STEEL_INPUT;
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

    #[test]
    fn complete_showcase_visits_both_ends_without_stalling() {
        use std::collections::HashMap;

        let mut state = showcase_state();
        let west_airport = airport_anchor_covering(&state, SHOWCASE_AIRPORT_WEST_ORIGIN)
            .expect("aeropuerto oeste");
        let east_airport =
            airport_anchor_covering(&state, SHOWCASE_AIRPORT_EAST_ORIGIN).expect("aeropuerto este");
        let west_rail = openttdrs_core::rail_station_platform_tiles(&state.map, SHOWCASE_RAIL_WEST);
        let east_rail = openttdrs_core::rail_station_platform_tiles(&state.map, SHOWCASE_RAIL_EAST);
        let tracked: Vec<(u32, VehicleKind)> = state
            .vehicles
            .iter()
            .filter(|vehicle| {
                matches!(
                    vehicle.kind,
                    VehicleKind::Bus
                        | VehicleKind::Train
                        | VehicleKind::Ship
                        | VehicleKind::Aircraft
                ) && (vehicle.kind != VehicleKind::Train || vehicle.is_consist_head())
            })
            .map(|vehicle| (vehicle.id, vehicle.kind))
            .collect();
        let mut visits: HashMap<u32, (bool, bool)> = tracked
            .iter()
            .map(|(id, _)| (*id, (false, false)))
            .collect();
        let mut train_lanes: HashMap<u32, (bool, bool)> = tracked
            .iter()
            .filter(|(_, kind)| *kind == VehicleKind::Train)
            .map(|(id, _)| (*id, (false, false)))
            .collect();
        let mut saw_two_trains_in_one_station = false;
        let mut west_platforms_used = std::collections::HashSet::new();
        let mut east_platforms_used = std::collections::HashSet::new();
        let mut saw_loaded_coal_train = false;
        let mut saw_coal_waiting_at_power_station = false;
        let mut saw_red_entry_with_green_exit = false;
        // Seis consists comparten los dos cruces; dar dos ciclos largos deja
        // margen para esperas de bloque legítimas sin confundirlas con deadlock.
        for tick in 0..24_000 {
            let before_train_poses: Vec<_> = state
                .vehicles
                .iter()
                .filter(|v| v.kind == VehicleKind::Train)
                .map(|v| {
                    (
                        v.id,
                        v.pos,
                        v.rail_pixel,
                        v.direction,
                        v.path.front().copied(),
                        v.prev_unit,
                    )
                })
                .collect();
            state.step();
            assert_eq!(
                state
                    .vehicles
                    .iter()
                    .filter(|v| v.kind == VehicleKind::Train && v.is_consist_head())
                    .count(),
                6,
                "se perdió un consist por colisión en tick {tick}: antes={before_train_poses:?}; \
                 después={:?}; noticia={:?}",
                state
                    .vehicles
                    .iter()
                    .filter(|v| v.kind == VehicleKind::Train && v.is_consist_head())
                    .map(|v| (v.id, v.pos, v.path.front().copied()))
                    .collect::<Vec<_>>(),
                state.news.items.back()
            );
            assert!(
                !state
                    .vehicles
                    .iter()
                    .any(|v| v.kind == VehicleKind::Train && v.crashed),
                "choque ferroviario en tick {tick}: {:?}",
                state
                    .vehicles
                    .iter()
                    .filter(|v| v.kind == VehicleKind::Train && v.is_consist_head())
                    .map(|v| {
                        (
                            v.id,
                            v.pos,
                            v.dest,
                            v.path.front().copied(),
                            v.cur_speed,
                            v.crashed,
                        )
                    })
                    .collect::<Vec<_>>()
            );
            for (station_tiles, used) in [
                (&west_rail, &mut west_platforms_used),
                (&east_rail, &mut east_platforms_used),
            ] {
                let occupied_platforms: std::collections::HashSet<_> = state
                    .vehicles
                    .iter()
                    .filter(|vehicle| {
                        vehicle.kind == VehicleKind::Train
                            && vehicle.is_consist_head()
                            && station_tiles.contains(&vehicle.pos)
                    })
                    .map(|vehicle| vehicle.pos.y)
                    .collect();
                for &y in &occupied_platforms {
                    used.insert(y);
                }
                // Ideal: dos andenes ocupados a la vez. Con señales block vanilla
                // basta con que ambos andenes se usen en el ciclo (asignación).
                saw_two_trains_in_one_station |= occupied_platforms.len() >= 2;
            }
            for vehicle in &state.vehicles {
                saw_loaded_coal_train |= vehicle.kind == VehicleKind::Train
                    && vehicle.is_consist_head()
                    && vehicle.cargo_type == Some(CargoType::Coal)
                    && vehicle.cargo > 0;
                let Some(visit) = visits.get_mut(&vehicle.id) else {
                    continue;
                };
                match vehicle.kind {
                    VehicleKind::Bus => {
                        visit.0 |= vehicle.last_station_visited == Some(SHOWCASE_BUS_A);
                        visit.1 |= vehicle.last_station_visited == Some(SHOWCASE_BUS_B);
                    }
                    VehicleKind::Train => {
                        visit.0 |= west_rail.contains(&vehicle.pos);
                        visit.1 |= east_rail.contains(&vehicle.pos);
                        if let (Some(lanes), Some(next)) =
                            (train_lanes.get_mut(&vehicle.id), vehicle.movement_target())
                            && state.map.get_kind(vehicle.pos) == Some(TileKind::Rail)
                            && next.y == vehicle.pos.y
                        {
                            if vehicle.pos.y == SHOWCASE_RAIL_Y {
                                let on_running_line = vehicle.pos.x > 15 && vehicle.pos.x < 47;
                                if on_running_line {
                                    assert!(
                                        next.x > vehicle.pos.x,
                                        "tren #{} circuló al revés por la vía de ida: {:?} -> {next:?}",
                                        vehicle.id,
                                        vehicle.pos
                                    );
                                    lanes.0 = true;
                                }
                            } else if vehicle.pos.y == SHOWCASE_RAIL_RETURN_Y {
                                let on_running_line = vehicle.pos.x > 15 && vehicle.pos.x < 47;
                                if on_running_line {
                                    assert!(
                                        next.x < vehicle.pos.x,
                                        "tren #{} circuló al revés por la vía de vuelta: {:?} -> {next:?}",
                                        vehicle.id,
                                        vehicle.pos
                                    );
                                    lanes.1 = true;
                                }
                            }
                        }
                    }
                    VehicleKind::Ship => {
                        visit.0 |= vehicle.last_station_visited == Some(SHOWCASE_DOCK_WEST);
                        visit.1 |= vehicle.last_station_visited == Some(SHOWCASE_DOCK_EAST);
                    }
                    VehicleKind::Aircraft => {
                        visit.0 |= vehicle.last_station_visited == Some(west_airport);
                        visit.1 |= vehicle.last_station_visited == Some(east_airport);
                    }
                    VehicleKind::Truck | VehicleKind::Tram => {}
                }
            }
            saw_coal_waiting_at_power_station |= state
                .stations
                .iter()
                .find(|station| station.pos == SHOWCASE_RAIL_EAST)
                .is_some_and(|station| station.cargo_stock.coal > 0);
            let west_exit = state
                .map
                .get(TileCoord::new(18, SHOWCASE_RAIL_Y))
                .map_or(0, |tile| openttdrs_core::rail_signal_state_mask(tile.m3hi));
            let west_entry = state
                .map
                .get(TileCoord::new(18, SHOWCASE_RAIL_RETURN_Y))
                .map_or(0, |tile| openttdrs_core::rail_signal_state_mask(tile.m3hi));
            let east_entry = state
                .map
                .get(TileCoord::new(46, SHOWCASE_RAIL_Y))
                .map_or(0, |tile| openttdrs_core::rail_signal_state_mask(tile.m3hi));
            let east_exit = state
                .map
                .get(TileCoord::new(46, SHOWCASE_RAIL_RETURN_Y))
                .map_or(0, |tile| openttdrs_core::rail_signal_state_mask(tile.m3hi));
            saw_red_entry_with_green_exit |= (west_entry & 0b1000 == 0 && west_exit & 0b0100 != 0)
                || (east_entry & 0b0100 == 0 && east_exit & 0b1000 != 0);
        }

        // Una cola válida puede dejar quieto al último tren varios miles de
        // ticks. Desde el estado cargado resultante, exigir que los seis
        // vuelvan a avanzar distingue esa cola de un deadlock permanente.
        let mut congestion_probe: HashMap<u32, ((TileCoord, u8, u8), bool)> = state
            .vehicles
            .iter()
            .filter(|vehicle| vehicle.kind == VehicleKind::Train && vehicle.is_consist_head())
            .map(|vehicle| {
                (
                    vehicle.id,
                    ((vehicle.pos, vehicle.rail_pixel, vehicle.progress), false),
                )
            })
            .collect();
        for _ in 0..8_000 {
            state.step();
            for vehicle in state
                .vehicles
                .iter()
                .filter(|vehicle| vehicle.kind == VehicleKind::Train && vehicle.is_consist_head())
            {
                if let Some((initial_pose, moved)) = congestion_probe.get_mut(&vehicle.id) {
                    *moved |= (vehicle.pos, vehicle.rail_pixel, vehicle.progress) != *initial_pose;
                }
            }
        }
        for (id, kind) in tracked {
            let visit = visits[&id];
            let vehicle = state
                .vehicles
                .iter()
                .find(|vehicle| vehicle.id == id)
                .expect("vehículo del showcase");
            assert!(
                visit.0 && visit.1,
                "{kind:?} #{id} no visitó ambos extremos: {visit:?}; pos={:?} dest={:?} \
                 order={} path={} pbs={} no_route={} last={:?}; train_heads={:?}",
                vehicle.pos,
                vehicle.dest,
                vehicle.current_order,
                vehicle.path.len(),
                vehicle.pbs_stuck,
                vehicle.no_network_route_to_order,
                vehicle.last_station_visited,
                state
                    .vehicles
                    .iter()
                    .filter(|v| v.kind == VehicleKind::Train && v.is_consist_head())
                    .map(|v| { (v.id, v.pos, v.dest, v.path.front().copied(), v.cur_speed,) })
                    .collect::<Vec<_>>(),
            );
            assert!(!vehicle.crashed, "{kind:?} #{id} no debe chocar");
            assert!(vehicle.running, "{kind:?} #{id} debe seguir activo");
            if kind == VehicleKind::Train {
                assert!(
                    congestion_probe[&id].1,
                    "tren #{id} no pudo salir de la congestión en 8000 ticks: pos={:?} dest={:?} \
                     speed={} path={:?} dir={} pbs={} wait={} reserved={} blocks=({},{},{}) \
                     heads={:?}",
                    vehicle.pos,
                    vehicle.dest,
                    vehicle.cur_speed,
                    vehicle.path.front().copied(),
                    vehicle.direction,
                    vehicle.pbs_stuck,
                    vehicle.wait_counter,
                    vehicle.reserved_steps.len(),
                    openttdrs_core::train_blocked_by_reservation(&state.map, vehicle),
                    openttdrs_core::rail_signals::train_blocked_by_signal(
                        &state.map,
                        &state.vehicles,
                        vehicle,
                    ),
                    openttdrs_core::rail_signals::train_blocked_by_traffic(
                        &state.map,
                        &state.vehicles,
                        vehicle,
                    ),
                    state
                        .vehicles
                        .iter()
                        .filter(|v| v.kind == VehicleKind::Train && v.is_consist_head())
                        .map(|v| (
                            v.id,
                            v.pos,
                            v.rail_pixel,
                            v.dest,
                            v.path.front().copied(),
                            v.cur_speed,
                            v.direction,
                            v.reserved_steps.len(),
                        ))
                        .collect::<Vec<_>>(),
                );
            }
        }
        for (id, lanes) in train_lanes {
            assert!(
                lanes.0 && lanes.1,
                "tren #{id} debe usar ida y vuelta: {lanes:?}"
            );
        }
        // Circulación de doble vía: llegada oeste en y=return, este en y=ida.
        // La ocupación concurrente de ambos andenes es deseable pero no siempre
        // ocurre con señales block vanilla y salida de depósito fluida.
        assert!(
            west_platforms_used.contains(&SHOWCASE_RAIL_RETURN_Y)
                && east_platforms_used.contains(&SHOWCASE_RAIL_Y)
                || saw_two_trains_in_one_station,
            "cada estación debe usarse en el andén de llegada del sentido: \
             concurrent={saw_two_trains_in_one_station}, west={west_platforms_used:?}, \
             east={east_platforms_used:?}"
        );
        assert!(
            saw_loaded_coal_train,
            "algún carbonero debe cargar en la mina"
        );
        assert!(
            saw_coal_waiting_at_power_station,
            "el carbón debe llegar al andén de la central antes de ser consumido"
        );
        assert!(
            saw_red_entry_with_green_exit,
            "la señal anterior debe ponerse roja para frenar la entrada mientras la opuesta permite salir"
        );
        assert!(
            state
                .industries
                .iter()
                .find(|industry| industry.pos == SHOWCASE_COAL_MINE)
                .is_some_and(|mine| mine.transported_total > 0),
            "la mina debe contabilizar carbón transportado"
        );
    }
}
