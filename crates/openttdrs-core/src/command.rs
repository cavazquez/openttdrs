//! Comandos del jugador que mutan [`crate::GameState`] de forma validada e identificable.

use crate::map::{TileCoord, TileKind};
use crate::{
    BRIDGE_BUILD_COST_PER_TILE, CLEAR_TILE_COST, DEPOT_BUILD_COST, GameState, Industry,
    IndustryKind, RAIL_BUILD_COST, ROAD_BUILD_COST, STATION_BUILD_COST, Station,
    TUNNEL_BUILD_COST_PER_TILE,
};

/// Acción del jugador reproducible (p. ej. log para red en I8).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Command {
    /// Coloca carretera en la tesela (MVP: solo validación de terreno).
    PlaceRoad(TileCoord),
    /// Coloca o combina una pieza de carretera OpenTTD (`RoadBits`, bits 0..3).
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
    PlaceHouse(TileCoord),
    PlaceIndustry(TileCoord),
    PlaceIndustryKind(TileCoord, IndustryKind),
    PlaceForest(TileCoord),
    /// Añade una estación y marca la tesela como `TileKind::Station`.
    PlaceStation(TileCoord),
    /// Añade una estación de carretera con orientación visual `0..3`.
    PlaceStationDir(TileCoord, u8),
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
    VehicleNotFound,
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
        Command::PlaceHouse(c) => {
            place_single_transport_tile(state, *c, TileKind::House, 0x30, 0x00, 50)
        }
        Command::PlaceIndustry(c) => place_industry_sandbox(state, *c),
        Command::PlaceIndustryKind(c, kind) => place_industry_kind_sandbox(state, *c, *kind),
        Command::PlaceForest(c) => {
            place_single_transport_tile(state, *c, TileKind::Forest, 0x40, 0x00, 30)
        }
        Command::PlaceStation(c) => place_station(state, *c),
        Command::PlaceStationDir(c, dir) => place_station_dir(state, *c, *dir),
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
    place_station_dir(state, c, 0)
}

fn place_station_dir(state: &mut GameState, c: TileCoord, dir: u8) -> Result<(), CommandError> {
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
            state.stations.push(Station::new(c));
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
    state
        .map
        .set_kind(c, TileKind::Grass)
        .map_err(|_| CommandError::OutOfBounds)?;
    state
        .map
        .set_mapt_m5(c, 0x00, 0x00)
        .map_err(|_| CommandError::OutOfBounds)?;
    state.stations.retain(|s| s.pos != c);
    state.industries.retain(|industry| industry.pos != c);
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

fn place_industry_sandbox(state: &mut GameState, c: TileCoord) -> Result<(), CommandError> {
    place_industry_kind_sandbox(state, c, IndustryKind::Factory)
}

fn place_industry_kind_sandbox(
    state: &mut GameState,
    c: TileCoord,
    kind: IndustryKind,
) -> Result<(), CommandError> {
    let m5 = match kind {
        IndustryKind::CoalMine => 0x00,
        IndustryKind::Forest => 0x20,
        IndustryKind::OilWell => 0x2F,
        IndustryKind::Factory => 0x2B,
    };
    place_single_transport_tile(state, c, TileKind::Industry, 0x80, m5, 250)?;
    state.industries.retain(|industry| industry.pos != c);
    state.industries.push(Industry::new(c, kind));
    Ok(())
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
        assert!(s.industries.iter().any(|industry| {
            industry.pos == TileCoord::new(4, 1) && industry.kind == IndustryKind::CoalMine
        }));
    }
}
