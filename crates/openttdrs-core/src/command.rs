//! Comandos del jugador que mutan [`crate::GameState`] de forma validada e identificable.

use crate::map::{TileCoord, TileKind};
use crate::{
    BRIDGE_BUILD_COST_PER_TILE, CLEAR_TILE_COST, DEPOT_BUILD_COST, GameState, Industry,
    IndustryKind, IndustrySpec, RAIL_BUILD_COST, ROAD_BUILD_COST, STATION_BUILD_COST, Station,
    StopKind, TUNNEL_BUILD_COST_PER_TILE, Vehicle, VehicleKind,
};

/// Acción del jugador reproducible (p. ej. log para red en I8).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Command {
    /// Coloca carretera en la tesela (MVP: solo validación de terreno).
    PlaceRoad(TileCoord),
    /// Coloca o combina una pieza de carretera `OpenTTD` (`RoadBits`, bits 0..3).
    PlaceRoadBits(TileCoord, u8),
    /// Coloca via de tren en la tesela (MVP: validacion de terreno).
    PlaceRail(TileCoord),
    PlaceRoadDepot(TileCoord),
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
        Command::PlaceRoad(c) => place_road(state, *c),
        Command::PlaceRoadBits(c, bits) => place_road_bits(state, *c, *bits),
        Command::PlaceRail(c) => place_rail(state, *c),
        Command::PlaceRoadDepot(c) => place_single_transport_tile(
            state,
            *c,
            TileKind::RoadDepot,
            0x20,
            0x20,
            DEPOT_BUILD_COST,
        ),
        Command::PlaceRailDepot(c) => place_single_transport_tile(
            state,
            *c,
            TileKind::RailDepot,
            0x10,
            0xC0,
            DEPOT_BUILD_COST,
        ),
        Command::PlaceRoadTunnel(a, b) => place_tunnel_or_bridge(
            state,
            *a,
            *b,
            TileKind::RoadTunnel,
            0x90,
            0x00,
            TUNNEL_BUILD_COST_PER_TILE,
        ),
        Command::PlaceRailTunnel(a, b) => place_tunnel_or_bridge(
            state,
            *a,
            *b,
            TileKind::RailTunnel,
            0x90,
            0x04,
            TUNNEL_BUILD_COST_PER_TILE,
        ),
        Command::PlaceRoadBridge(a, b) => place_tunnel_or_bridge(
            state,
            *a,
            *b,
            TileKind::RoadBridge,
            0x90,
            0x80,
            BRIDGE_BUILD_COST_PER_TILE,
        ),
        Command::PlaceRailBridge(a, b) => place_tunnel_or_bridge(
            state,
            *a,
            *b,
            TileKind::RailBridge,
            0x90,
            0x84,
            BRIDGE_BUILD_COST_PER_TILE,
        ),
        Command::SetVehicleOrders(id, orders) => set_vehicle_orders(state, *id, orders.clone()),
        Command::SetVehicleStationOrders(id, stations) => {
            set_vehicle_station_orders(state, *id, stations.clone())
        }
        Command::PlaceHouse(c) => {
            place_single_transport_tile(state, *c, TileKind::House, 0x30, 0x00, 50)
        }
        Command::PlaceIndustry(c) => place_industry_sandbox(state, *c),
        Command::PlaceIndustryKind(c, kind) => place_industry_kind_sandbox(state, *c, *kind),
        Command::PlaceIndustrySpec(c, spec) => place_industry_spec_sandbox(state, *c, *spec),
        Command::PlaceForest(c) => {
            place_single_transport_tile(state, *c, TileKind::Forest, 0x40, 0x00, 30)
        }
        Command::PlaceStation(c) => place_station(state, *c),
        Command::PlaceStationDir(c, dir) => place_station_dir(state, *c, *dir),
        Command::PlaceBusStop(c, dir) => place_stop_kind(state, *c, *dir, StopKind::BusStop),
        Command::PlaceTruckStop(c, dir) => place_stop_kind(state, *c, *dir, StopKind::TruckStop),
        Command::BuildRoadVehicleAtDepot(c, kind) => build_road_vehicle_at_depot(state, *c, *kind),
        Command::SellVehicle(id) => sell_vehicle(state, *id),
        Command::ToggleVehicleRunning(id) => toggle_vehicle_running(state, *id),
        Command::CloneVehicleOrders {
            from_vehicle_id,
            to_vehicle_id,
        } => clone_vehicle_orders(state, *from_vehicle_id, *to_vehicle_id),
        Command::ClearTile(c) => clear_tile(state, *c),
    }
}

fn place_road(state: &mut GameState, c: TileCoord) -> Result<(), CommandError> {
    place_road_bits(state, c, 0x05)
}

fn transport_tile_is_buildable(kind: TileKind) -> bool {
    !matches!(kind, TileKind::Water | TileKind::Void)
}

fn build_error_for_kind(kind: TileKind) -> CommandError {
    match kind {
        TileKind::Water => CommandError::CannotPlaceRoadOnWater,
        TileKind::Void => CommandError::CannotPlaceRoadOnVoid,
        _ => CommandError::OutOfBounds,
    }
}

fn place_single_transport_tile(
    state: &mut GameState,
    c: TileCoord,
    kind_to_place: TileKind,
    mapt: u8,
    m5: u8,
    cost: i64,
) -> Result<(), CommandError> {
    in_bounds(&state.map, c)?;
    let kind = state.map.get_kind(c).unwrap_or(TileKind::Grass);
    if !transport_tile_is_buildable(kind) {
        return Err(build_error_for_kind(kind));
    }
    state
        .map
        .set_kind(c, kind_to_place)
        .map_err(|_| CommandError::OutOfBounds)?;
    state
        .map
        .set_mapt_m5(c, mapt, m5)
        .map_err(|_| CommandError::OutOfBounds)?;
    state.economy.money -= cost;
    Ok(())
}

fn axis_line(a: TileCoord, b: TileCoord) -> Vec<TileCoord> {
    if (b.x - a.x).abs() >= (b.y - a.y).abs() {
        let step = if b.x >= a.x { 1 } else { -1 };
        let mut out = Vec::new();
        let mut x = a.x;
        loop {
            out.push(TileCoord::new(x, a.y));
            if x == b.x {
                break;
            }
            x += step;
        }
        out
    } else {
        let step = if b.y >= a.y { 1 } else { -1 };
        let mut out = Vec::new();
        let mut y = a.y;
        loop {
            out.push(TileCoord::new(a.x, y));
            if y == b.y {
                break;
            }
            y += step;
        }
        out
    }
}

fn place_tunnel_or_bridge(
    state: &mut GameState,
    a: TileCoord,
    b: TileCoord,
    kind_to_place: TileKind,
    mapt: u8,
    m5: u8,
    cost_per_tile: i64,
) -> Result<(), CommandError> {
    let line = axis_line(a, b);
    if line.len() < 2 {
        return Err(CommandError::OutOfBounds);
    }
    for c in &line {
        in_bounds(&state.map, *c)?;
        let kind = state.map.get_kind(*c).unwrap_or(TileKind::Grass);
        if !transport_tile_is_buildable(kind) {
            return Err(build_error_for_kind(kind));
        }
    }
    let cost = cost_per_tile * i64::try_from(line.len()).unwrap_or(i64::MAX);
    for c in line {
        state
            .map
            .set_kind(c, kind_to_place)
            .map_err(|_| CommandError::OutOfBounds)?;
        state
            .map
            .set_mapt_m5(c, mapt, m5)
            .map_err(|_| CommandError::OutOfBounds)?;
    }
    state.economy.money -= cost;
    Ok(())
}

fn place_road_bits(state: &mut GameState, c: TileCoord, bits: u8) -> Result<(), CommandError> {
    in_bounds(&state.map, c)?;
    let kind = state.map.get_kind(c).unwrap_or(TileKind::Grass);
    match kind {
        TileKind::Water => Err(CommandError::CannotPlaceRoadOnWater),
        TileKind::Void => Err(CommandError::CannotPlaceRoadOnVoid),
        _ => {
            let existing = state.map.get(c).map_or(0, |t| {
                if t.kind == TileKind::Road {
                    t.m5 & 0x0F
                } else {
                    0
                }
            });
            let road_bits = (existing | (bits & 0x0F)).max(0x01);
            state
                .map
                .set_kind(c, TileKind::Road)
                .map_err(|_| CommandError::OutOfBounds)?;
            // MP_ROAD normal tile: low nibble stores road bits, high bits subtype=0.
            state
                .map
                .set_mapt_m5(c, 0x20, road_bits)
                .map_err(|_| CommandError::OutOfBounds)?;
            state.economy.money -= ROAD_BUILD_COST;
            Ok(())
        }
    }
}

fn place_station(state: &mut GameState, c: TileCoord) -> Result<(), CommandError> {
    place_stop_kind(state, c, 0, StopKind::TruckStop)
}

fn place_station_dir(state: &mut GameState, c: TileCoord, dir: u8) -> Result<(), CommandError> {
    place_stop_kind(state, c, dir, StopKind::TruckStop)
}

fn place_stop_kind(
    state: &mut GameState,
    c: TileCoord,
    dir: u8,
    stop_kind: StopKind,
) -> Result<(), CommandError> {
    in_bounds(&state.map, c)?;
    if state.stations.iter().any(|s| s.pos == c) {
        return Err(CommandError::StationAlreadyExists);
    }
    let kind = state.map.get_kind(c).unwrap_or(TileKind::Grass);
    match kind {
        TileKind::Water => Err(CommandError::CannotPlaceStationOnWater),
        TileKind::Void => Err(CommandError::CannotPlaceStationOnVoid),
        _ => {
            state
                .map
                .set_kind(c, TileKind::Station)
                .map_err(|_| CommandError::OutOfBounds)?;
            state
                .map
                .set_mapt_m5(c, 0x50, dir & 0x03)
                .map_err(|_| CommandError::OutOfBounds)?;
            state.stations.push(Station::new_with_kind(c, stop_kind));
            state.economy.money -= STATION_BUILD_COST;
            Ok(())
        }
    }
}

fn place_rail(state: &mut GameState, c: TileCoord) -> Result<(), CommandError> {
    in_bounds(&state.map, c)?;
    let kind = state.map.get_kind(c).unwrap_or(TileKind::Grass);
    match kind {
        TileKind::Water => Err(CommandError::CannotPlaceRailOnWater),
        TileKind::Void => Err(CommandError::CannotPlaceRailOnVoid),
        _ => {
            state
                .map
                .set_kind(c, TileKind::Rail)
                .map_err(|_| CommandError::OutOfBounds)?;
            state.economy.money -= RAIL_BUILD_COST;
            Ok(())
        }
    }
}

fn clear_tile(state: &mut GameState, c: TileCoord) -> Result<(), CommandError> {
    in_bounds(&state.map, c)?;
    if let Some(industry_idx) = state.industries.iter().position(|i| i.contains_tile(c)) {
        let industry_tiles = state.industries[industry_idx].tiles.clone();
        for tile in industry_tiles {
            state
                .map
                .set_kind(tile, TileKind::Grass)
                .map_err(|_| CommandError::OutOfBounds)?;
            state
                .map
                .set_mapt_m5(tile, 0x00, 0x00)
                .map_err(|_| CommandError::OutOfBounds)?;
        }
        state.industries.remove(industry_idx);
        state.economy.money -= CLEAR_TILE_COST;
        return Ok(());
    }
    state
        .map
        .set_kind(c, TileKind::Grass)
        .map_err(|_| CommandError::OutOfBounds)?;
    state
        .map
        .set_mapt_m5(c, 0x00, 0x00)
        .map_err(|_| CommandError::OutOfBounds)?;
    state.stations.retain(|s| s.pos != c);
    state
        .industries
        .retain(|industry| !industry.contains_tile(c));
    state.economy.money -= CLEAR_TILE_COST;
    Ok(())
}

fn set_vehicle_orders(
    state: &mut GameState,
    id: u32,
    orders: Vec<TileCoord>,
) -> Result<(), CommandError> {
    for order in &orders {
        in_bounds(&state.map, *order)?;
    }
    let Some(vehicle) = state.vehicles.iter_mut().find(|v| v.id == id) else {
        return Err(CommandError::VehicleNotFound);
    };
    vehicle.set_orders(orders);
    Ok(())
}

fn set_vehicle_station_orders(
    state: &mut GameState,
    id: u32,
    stations: Vec<TileCoord>,
) -> Result<(), CommandError> {
    let Some(vehicle_idx) = state.vehicles.iter().position(|v| v.id == id) else {
        return Err(CommandError::VehicleNotFound);
    };
    let vehicle_kind = state.vehicles[vehicle_idx].kind;
    for station in &stations {
        in_bounds(&state.map, *station)?;
        let Some(st) = state.stations.iter().find(|s| s.pos == *station) else {
            return Err(CommandError::StationNotFound);
        };
        if !st.can_service_vehicle(vehicle_kind) {
            return Err(CommandError::IncompatibleStopForVehicle);
        }
    }
    let vehicle = &mut state.vehicles[vehicle_idx];
    vehicle.set_station_orders(stations);
    Ok(())
}

fn build_road_vehicle_at_depot(
    state: &mut GameState,
    depot_pos: TileCoord,
    kind: VehicleKind,
) -> Result<(), CommandError> {
    in_bounds(&state.map, depot_pos)?;
    let Some(tile) = state.map.get(depot_pos) else {
        return Err(CommandError::OutOfBounds);
    };
    if tile.kind != TileKind::RoadDepot {
        return Err(CommandError::InvalidDepotTile);
    }
    if matches!(kind, VehicleKind::Train) {
        return Err(CommandError::VehicleKindNotAllowed);
    }
    let next_id = state
        .vehicles
        .iter()
        .map(|v| v.id)
        .max()
        .map_or(1, |v| v.saturating_add(1));
    let mut vehicle = Vehicle::new(next_id, kind, depot_pos, depot_pos);
    vehicle.running = false;
    state.vehicles.push(vehicle);
    Ok(())
}

fn sell_vehicle(state: &mut GameState, vehicle_id: u32) -> Result<(), CommandError> {
    let Some(idx) = state.vehicles.iter().position(|v| v.id == vehicle_id) else {
        return Err(CommandError::VehicleNotFound);
    };
    state.vehicles.remove(idx);
    Ok(())
}

fn toggle_vehicle_running(state: &mut GameState, vehicle_id: u32) -> Result<(), CommandError> {
    let Some(vehicle) = state.vehicles.iter_mut().find(|v| v.id == vehicle_id) else {
        return Err(CommandError::VehicleNotFound);
    };
    vehicle.running = !vehicle.running;
    Ok(())
}

fn clone_vehicle_orders(
    state: &mut GameState,
    from_vehicle_id: u32,
    to_vehicle_id: u32,
) -> Result<(), CommandError> {
    let Some(src_idx) = state.vehicles.iter().position(|v| v.id == from_vehicle_id) else {
        return Err(CommandError::VehicleNotFound);
    };
    let Some(dst_idx) = state.vehicles.iter().position(|v| v.id == to_vehicle_id) else {
        return Err(CommandError::VehicleNotFound);
    };
    let src_orders = state.vehicles[src_idx].orders.clone();
    state.vehicles[dst_idx].set_vehicle_orders(src_orders);
    Ok(())
}

fn place_industry_sandbox(state: &mut GameState, c: TileCoord) -> Result<(), CommandError> {
    place_industry_spec_sandbox(state, c, IndustrySpec::Factory)
}

fn place_industry_kind_sandbox(
    state: &mut GameState,
    c: TileCoord,
    kind: IndustryKind,
) -> Result<(), CommandError> {
    let spec = match kind {
        IndustryKind::CoalMine => IndustrySpec::CoalMine,
        IndustryKind::Forest => IndustrySpec::Forest,
        IndustryKind::OilWell => IndustrySpec::OilWells,
        IndustryKind::Factory => IndustrySpec::Factory,
    };
    place_industry_spec_sandbox(state, c, spec)
}

fn place_industry_spec_sandbox(
    state: &mut GameState,
    c: TileCoord,
    spec: IndustrySpec,
) -> Result<(), CommandError> {
    let template = industry_template(c, spec);
    for (tile, _) in &template {
        in_bounds(&state.map, *tile)?;
        let existing_kind = state.map.get_kind(*tile).unwrap_or(TileKind::Grass);
        if !transport_tile_is_buildable(existing_kind) {
            return Err(build_error_for_kind(existing_kind));
        }
    }
    let footprint: Vec<TileCoord> = template.iter().map(|(tile, _)| *tile).collect();
    for (tile, m5) in &template {
        state
            .map
            .set_kind(*tile, TileKind::Industry)
            .map_err(|_| CommandError::OutOfBounds)?;
        state
            .map
            .set_mapt_m5(*tile, 0x80, *m5)
            .map_err(|_| CommandError::OutOfBounds)?;
    }
    state
        .industries
        .retain(|industry| !industry.contains_tile(c));
    state
        .industries
        .push(Industry::with_tiles_spec(c, spec.kind(), spec, footprint));
    state.economy.money -= 250;
    Ok(())
}

#[must_use]
pub fn industry_template(c: TileCoord, spec: IndustrySpec) -> Vec<(TileCoord, u8)> {
    const COAL_MINE_LAYOUTS: [&[(i32, i32, u8)]; 4] = [
        // OpenTTD _tile_table_coal_mine_0.
        &[
            (1, 1, 0),
            (1, 2, 2),
            (0, 0, 5),
            (1, 0, 6),
            (2, 0, 3),
            (2, 2, 3),
        ],
        // OpenTTD _tile_table_coal_mine_1.
        &[
            (1, 1, 0),
            (1, 2, 2),
            (2, 0, 0),
            (2, 1, 2),
            (1, 0, 3),
            (0, 0, 3),
            (0, 1, 4),
            (0, 2, 4),
            (2, 2, 4),
        ],
        // OpenTTD _tile_table_coal_mine_2.
        &[
            (0, 0, 0),
            (0, 1, 2),
            (0, 2, 5),
            (1, 0, 3),
            (1, 1, 3),
            (1, 2, 6),
        ],
        // OpenTTD _tile_table_coal_mine_3.
        &[
            (0, 1, 0),
            (0, 2, 2),
            (0, 3, 4),
            (1, 0, 5),
            (1, 1, 0),
            (1, 2, 2),
            (1, 3, 3),
            (2, 0, 6),
            (2, 1, 4),
            (2, 2, 3),
        ],
    ];
    const METAL_MINE_LAYOUTS: [&[(i32, i32, u8)]; 2] = [
        // OpenTTD _tile_table_copper_mine_0.
        &[
            (0, 0, 47),
            (0, 1, 49),
            (0, 2, 51),
            (1, 0, 47),
            (1, 1, 49),
            (1, 2, 50),
            (2, 0, 51),
            (2, 1, 51),
        ],
        // OpenTTD _tile_table_copper_mine_1.
        &[
            (0, 0, 50),
            (0, 1, 47),
            (0, 2, 49),
            (1, 0, 47),
            (1, 1, 49),
            (1, 2, 51),
            (2, 0, 51),
            (2, 1, 47),
            (2, 2, 49),
        ],
    ];
    const GOLD_MINE_LAYOUTS: [&[(i32, i32, u8)]; 1] = [
        // OpenTTD _tile_table_gold_mine_0.
        &[
            (0, 0, 72),
            (0, 1, 73),
            (0, 2, 74),
            (0, 3, 75),
            (1, 0, 76),
            (1, 1, 77),
            (1, 2, 78),
            (1, 3, 79),
            (2, 0, 80),
            (2, 1, 81),
            (2, 2, 82),
            (2, 3, 83),
            (3, 0, 84),
            (3, 1, 85),
            (3, 2, 86),
            (3, 3, 87),
        ],
    ];
    const FOREST_LAYOUTS: [&[(i32, i32, u8)]; 2] = [
        // OpenTTD _tile_table_forest_0.
        &[
            (0, 0, 16),
            (0, 1, 16),
            (0, 2, 16),
            (0, 3, 16),
            (1, 0, 16),
            (1, 1, 16),
            (1, 2, 16),
            (1, 3, 16),
            (2, 0, 16),
            (2, 1, 16),
            (2, 2, 16),
            (2, 3, 16),
            (3, 0, 16),
            (3, 1, 16),
            (3, 2, 16),
            (3, 3, 16),
            (1, 4, 16),
            (2, 4, 16),
        ],
        // OpenTTD _tile_table_forest_1.
        &[
            (0, 0, 16),
            (1, 0, 16),
            (2, 0, 16),
            (3, 0, 16),
            (4, 0, 16),
            (0, 1, 16),
            (1, 1, 16),
            (2, 1, 16),
            (3, 1, 16),
            (4, 1, 16),
            (0, 2, 16),
            (1, 2, 16),
            (2, 2, 16),
            (3, 2, 16),
            (4, 2, 16),
            (0, 3, 16),
            (1, 3, 16),
            (2, 3, 16),
            (3, 3, 16),
            (4, 3, 16),
            (1, 4, 16),
            (2, 4, 16),
            (3, 4, 16),
        ],
    ];
    const FARM_LAYOUTS: [&[(i32, i32, u8)]; 3] = [
        // OpenTTD _tile_table_farm_0.
        &[
            (1, 0, 33),
            (1, 1, 34),
            (1, 2, 36),
            (0, 0, 37),
            (0, 1, 37),
            (0, 2, 36),
            (2, 0, 35),
            (2, 1, 38),
            (2, 2, 38),
        ],
        // OpenTTD _tile_table_farm_1.
        &[
            (1, 1, 33),
            (1, 2, 34),
            (0, 0, 35),
            (0, 1, 36),
            (0, 2, 36),
            (0, 3, 35),
            (1, 0, 37),
            (1, 3, 38),
            (2, 0, 37),
            (2, 1, 37),
            (2, 2, 38),
            (2, 3, 38),
        ],
        // OpenTTD _tile_table_farm_2.
        &[
            (2, 0, 33),
            (2, 1, 34),
            (0, 0, 36),
            (0, 1, 36),
            (0, 2, 37),
            (0, 3, 37),
            (1, 0, 35),
            (1, 1, 38),
            (1, 2, 38),
            (1, 3, 37),
            (2, 2, 37),
            (2, 3, 35),
        ],
    ];
    const OIL_LAYOUTS: [&[(i32, i32, u8)]; 2] = [
        // OpenTTD _tile_table_oil_well_0.
        &[(0, 0, 29), (1, 0, 29), (2, 0, 29), (0, 1, 29), (0, 2, 29)],
        // OpenTTD _tile_table_oil_well_1.
        &[(0, 0, 29), (1, 0, 29), (1, 1, 29), (2, 2, 29), (2, 3, 29)],
    ];
    const REFINERY_LAYOUTS: [&[(i32, i32, u8)]; 2] = [
        // OpenTTD _tile_table_oil_refinery_0.
        &[
            (0, 0, 20),
            (0, 1, 21),
            (0, 2, 22),
            (0, 3, 21),
            (1, 0, 20),
            (1, 1, 19),
            (1, 2, 22),
            (1, 3, 20),
            (2, 1, 18),
            (2, 2, 18),
            (2, 3, 18),
            (3, 2, 18),
            (3, 3, 18),
            (2, 0, 23),
            (3, 1, 23),
        ],
        // OpenTTD _tile_table_oil_refinery_1.
        &[
            (0, 0, 18),
            (0, 1, 18),
            (0, 2, 21),
            (0, 3, 22),
            (0, 4, 20),
            (1, 0, 18),
            (1, 1, 18),
            (1, 2, 19),
            (1, 3, 20),
            (2, 0, 18),
            (2, 1, 18),
            (2, 2, 19),
            (2, 3, 22),
            (1, 4, 23),
            (2, 4, 23),
        ],
    ];
    const FACTORY_LAYOUTS: [&[(i32, i32, u8)]; 2] = [
        // OpenTTD _tile_table_factory_0.
        &[
            (0, 0, 39),
            (0, 1, 40),
            (1, 0, 41),
            (1, 1, 42),
            (0, 2, 39),
            (0, 3, 40),
            (1, 2, 41),
            (1, 3, 42),
            (2, 1, 39),
            (2, 2, 40),
            (3, 1, 41),
            (3, 2, 42),
        ],
        // OpenTTD _tile_table_factory_1.
        &[
            (0, 0, 39),
            (0, 1, 40),
            (1, 0, 41),
            (1, 1, 42),
            (2, 0, 39),
            (2, 1, 40),
            (3, 0, 41),
            (3, 1, 42),
            (1, 2, 39),
            (1, 3, 40),
            (2, 2, 41),
            (2, 3, 42),
        ],
    ];
    const SAWMILL_LAYOUTS: [&[(i32, i32, u8)]; 2] = [
        // OpenTTD _tile_table_sawmill_0.
        &[
            (1, 0, 14),
            (1, 1, 12),
            (1, 2, 11),
            (2, 0, 14),
            (2, 1, 13),
            (0, 0, 15),
            (0, 1, 15),
            (0, 2, 12),
        ],
        // OpenTTD _tile_table_sawmill_1.
        &[
            (0, 0, 15),
            (0, 1, 11),
            (0, 2, 14),
            (1, 0, 15),
            (1, 1, 13),
            (1, 2, 12),
            (2, 0, 11),
            (2, 1, 13),
        ],
    ];
    const IRON_MINE_LAYOUTS: [&[(i32, i32, u8)]; 1] = [
        // OpenTTD _tile_table_iron_mine_0.
        &[
            (0, 0, 100),
            (0, 1, 101),
            (0, 2, 102),
            (0, 3, 103),
            (1, 0, 104),
            (1, 1, 105),
            (1, 2, 106),
            (1, 3, 107),
            (2, 0, 108),
            (2, 1, 109),
            (2, 2, 110),
            (2, 3, 111),
            (3, 0, 112),
            (3, 1, 113),
            (3, 2, 114),
            (3, 3, 115),
        ],
    ];

    let offsets_and_gfx = match spec {
        IndustrySpec::CoalMine => choose_layout(c, &COAL_MINE_LAYOUTS),
        IndustrySpec::IronOreMine => choose_layout(c, &IRON_MINE_LAYOUTS),
        IndustrySpec::CopperOreMine => choose_layout(c, &METAL_MINE_LAYOUTS),
        IndustrySpec::GoldMine => choose_layout(c, &GOLD_MINE_LAYOUTS),
        IndustrySpec::Forest => choose_layout(c, &FOREST_LAYOUTS),
        IndustrySpec::Farm => choose_layout(c, &FARM_LAYOUTS),
        IndustrySpec::OilWells => choose_layout(c, &OIL_LAYOUTS),
        IndustrySpec::OilRefinery => choose_layout(c, &REFINERY_LAYOUTS),
        IndustrySpec::Factory => choose_layout(c, &FACTORY_LAYOUTS),
        IndustrySpec::Sawmill => choose_layout(c, &SAWMILL_LAYOUTS),
    };

    offsets_and_gfx
        .iter()
        .map(|(dx, dy, m5)| (TileCoord::new(c.x + dx, c.y + dy), *m5))
        .collect()
}

fn choose_layout<'a>(c: TileCoord, layouts: &'a [&'a [(i32, i32, u8)]]) -> &'a [(i32, i32, u8)] {
    let seed = i64::from(c.x)
        .wrapping_mul(31)
        .wrapping_add(i64::from(c.y).wrapping_mul(17));
    let idx = usize::try_from(seed.unsigned_abs()).unwrap_or(0) % layouts.len();
    layouts[idx]
}

fn in_bounds(map: &crate::map::Map, c: TileCoord) -> Result<(), CommandError> {
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
    use crate::{GameState, TileKind};

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
