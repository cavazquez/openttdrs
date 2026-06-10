//! Mapa procedural limpio: hierba plana + zonas de prueba separadas.

use bevy::prelude::*;
use openttdrs_core::{
    Command, GameState, Industry, IndustryKind, PathNetwork, TileCoord, TileKind, Vehicle,
    VehicleKind, apply_command, find_path,
};

/// Carretera horizontal de demo (eje X).
pub const DEMO_ROAD_Y: i32 = 6;
/// Vía horizontal de demo (eje X), lejos de la carretera.
pub const DEMO_RAIL_Y: i32 = 14;
/// Centro del canal de agua (puente E–O).
pub const DEMO_BRIDGE_Y: i32 = 10;
/// Rectángulo de agua para puentes (inclusive).
pub const DEMO_BRIDGE_WATER_X0: i32 = 4;
pub const DEMO_BRIDGE_WATER_X1: i32 = 14;
pub const DEMO_BRIDGE_WATER_Y0: i32 = 9;
pub const DEMO_BRIDGE_WATER_Y1: i32 = 11;
/// Orillas de hierba (bermas) alrededor del canal; puente entre (3,10) y (15,10).
pub const DEMO_BRIDGE_BANK_W: i32 = 3;
pub const DEMO_BRIDGE_BANK_E: i32 = 15;
pub const DEMO_BRIDGE_BANK_N: i32 = 8;
pub const DEMO_BRIDGE_BANK_S: i32 = 12;
/// Entrada NE del túnel de demo (pendiente inclinada).
pub const DEMO_TUNNEL_NE: TileCoord = TileCoord::new(18, 8);
/// Mina de carbón del ciclo económico demo (cobertura desde estación de carga).
pub const DEMO_ECONOMY_INDUSTRY: TileCoord = TileCoord::new(2, 3);
/// Parada de camión en hierba **al norte** de la carretera demo (carga).
pub const DEMO_ECONOMY_LOAD_STATION: TileCoord = TileCoord::new(3, DEMO_ROAD_Y - 1);
/// Parada de camión en hierba al norte de la carretera (descarga / ingresos).
pub const DEMO_ECONOMY_DELIVER_STATION: TileCoord = TileCoord::new(10, DEMO_ROAD_Y - 1);
/// Entrada de parada hacia la carretera en `y = DEMO_ROAD_Y` (tesela al sur → `DIAGDIR_SE`).
const DEMO_ECONOMY_STATION_ENTRANCE_DIR: u8 = 1;

/// `WaterTileType::Coast` en bits 4–7 de `m5` (fuerza sprites `shore_*` en el borde).
const WATER_COAST_M5: u8 = 0x10;
/// `ClearGround` rough en bits 2–4 de `m5` (berma más oscura en `land.rs`).
const BERM_ROUGH_M5: u8 = 0x0C;
const CHANNEL_Z: u8 = 1;

/// Hierba plana en todo el mapa (sin agua/bosque aleatorio).
pub(crate) fn fill_flat_grass(state: &mut GameState) {
    let (mw, mh) = state.map.dimensions();
    for y in 0..mh {
        for x in 0..mw {
            let c = TileCoord::new(x as i32, y as i32);
            if state.map.get(c).is_some() {
                let _ = state.map.set_kind(c, TileKind::Grass);
                let _ = state.map.set_height(c, CHANNEL_Z);
            }
        }
    }
}

/// Una línea de carretera y otra de vía, separadas por el hueco del puente.
pub(crate) fn place_clean_demo_transport(state: &mut GameState) {
    for x in 2..=12 {
        let _ = apply_command(
            state,
            &Command::PlaceRoadBits(TileCoord::new(x, DEMO_ROAD_Y), 0x0A),
        );
        let _ = apply_command(state, &Command::PlaceRail(TileCoord::new(x, DEMO_RAIL_Y)));
    }
    place_demo_road_vehicles(state);
    place_demo_rail_vehicle(state);
}

/// Industria + dos paradas de camión + ruta con órdenes para un ciclo jugable al arrancar.
pub(crate) fn place_demo_economy_loop(state: &mut GameState) {
    let _ = state
        .map
        .set_kind(DEMO_ECONOMY_INDUSTRY, TileKind::Industry);
    let mut mine = Industry::new(DEMO_ECONOMY_INDUSTRY, IndustryKind::CoalMine);
    mine.stock = 64;
    state.industries.push(mine);

    place_demo_truck_station(state, DEMO_ECONOMY_LOAD_STATION);
    place_demo_truck_station(state, DEMO_ECONOMY_DELIVER_STATION);

    let orders = vec![DEMO_ECONOMY_LOAD_STATION, DEMO_ECONOMY_DELIVER_STATION];
    let mut truck = Vehicle::new(
        9010,
        VehicleKind::Truck,
        DEMO_ECONOMY_LOAD_STATION,
        DEMO_ECONOMY_DELIVER_STATION,
    );
    truck.running = true;
    truck.set_station_orders(orders);
    if let Some(path) = find_path(
        &state.map,
        DEMO_ECONOMY_LOAD_STATION,
        DEMO_ECONOMY_DELIVER_STATION,
        PathNetwork::Road,
    ) {
        truck.path = path.into();
    }
    state.vehicles.push(truck);
}

fn place_demo_truck_station(state: &mut GameState, pos: TileCoord) {
    let _ = apply_command(
        state,
        &Command::PlaceStationDir(pos, DEMO_ECONOMY_STATION_ENTRANCE_DIR),
    );
}

/// Bus + camión en la carretera demo (para probar sprites sin depósito).
fn place_demo_road_vehicles(state: &mut GameState) {
    use openttdrs_core::{DIR_SW, PathNetwork, Vehicle, VehicleKind, find_path};

    let start_bus = TileCoord::new(3, DEMO_ROAD_Y);
    let start_truck = TileCoord::new(5, DEMO_ROAD_Y);
    let end = TileCoord::new(11, DEMO_ROAD_Y);

    if let Some(path) = find_path(&state.map, start_bus, end, PathNetwork::Road) {
        let mut bus = Vehicle::new(9001, VehicleKind::Bus, start_bus, end);
        bus.running = false;
        bus.direction = DIR_SW;
        bus.path = path.into();
        state.vehicles.push(bus);
    }

    if let Some(path) = find_path(&state.map, start_truck, end, PathNetwork::Road) {
        let mut truck = Vehicle::new(9002, VehicleKind::Truck, start_truck, end);
        truck.running = false;
        truck.cargo = truck.capacity / 2 + 1;
        truck.direction = DIR_SW;
        truck.path = path.into();
        state.vehicles.push(truck);
    }
}

/// Tren en la vía demo (Kirby Paul Tank, más lento que bus/camión).
fn place_demo_rail_vehicle(state: &mut GameState) {
    use openttdrs_core::{DIR_SW, PathNetwork, Vehicle, VehicleKind, find_path};

    let start = TileCoord::new(4, DEMO_RAIL_Y);
    let end = TileCoord::new(10, DEMO_RAIL_Y);

    if let Some(path) = find_path(&state.map, start, end, PathNetwork::Rail) {
        let mut train = Vehicle::new(9003, VehicleKind::Train, start, end);
        train.running = false;
        train.direction = DIR_SW;
        train.path = path.into();
        state.vehicles.push(train);
    }
}

/// Canal de agua con orillas costeras y bermas de hierba (zona de puente legible).
pub(crate) fn place_bridge_demo_gap(state: &mut GameState) {
    for y in DEMO_BRIDGE_WATER_Y0..=DEMO_BRIDGE_WATER_Y1 {
        for x in DEMO_BRIDGE_WATER_X0..=DEMO_BRIDGE_WATER_X1 {
            let c = TileCoord::new(x, y);
            let on_edge = x == DEMO_BRIDGE_WATER_X0
                || x == DEMO_BRIDGE_WATER_X1
                || y == DEMO_BRIDGE_WATER_Y0
                || y == DEMO_BRIDGE_WATER_Y1;
            set_water_cell(state, c, on_edge);
        }
    }

    for x in DEMO_BRIDGE_BANK_W..=DEMO_BRIDGE_BANK_E {
        for y in [DEMO_BRIDGE_BANK_N, DEMO_BRIDGE_BANK_S] {
            set_berm_cell(state, TileCoord::new(x, y));
        }
    }
    for y in DEMO_BRIDGE_BANK_N..=DEMO_BRIDGE_BANK_S {
        for x in [DEMO_BRIDGE_BANK_W, DEMO_BRIDGE_BANK_E] {
            set_berm_cell(state, TileCoord::new(x, y));
        }
    }
}

fn set_water_cell(state: &mut GameState, c: TileCoord, coast: bool) {
    let _ = state.map.set_kind(c, TileKind::Water);
    let _ = state.map.set_height(c, CHANNEL_Z);
    let m5 = if coast { WATER_COAST_M5 } else { 0 };
    let _ = state.map.set_mapt_m5(c, 0, m5);
}

fn set_berm_cell(state: &mut GameState, c: TileCoord) {
    let _ = state.map.set_kind(c, TileKind::Grass);
    let _ = state.map.set_height(c, CHANNEL_Z);
    let _ = state.map.set_mapt_m5(c, 0, BERM_ROUGH_M5);
}

pub(crate) fn log_procedural_demo_zones() {
    info!(
        "Mapa demo ({}×{}): carretera y={DEMO_ROAD_Y} x=2..12 (bus+camión) | \
         vía y={DEMO_RAIL_Y} x=2..12 (tren) | \
         economía mina ({},{}) → est ({},{}) → ({},{}) (camión #9010) | \
         puente agua x={DEMO_BRIDGE_WATER_X0}..{DEMO_BRIDGE_WATER_X1} y={DEMO_BRIDGE_WATER_Y0}..{DEMO_BRIDGE_WATER_Y1} \
         orillas x={DEMO_BRIDGE_BANK_W},{DEMO_BRIDGE_BANK_E} y={DEMO_BRIDGE_BANK_N},{DEMO_BRIDGE_BANK_S} \
         (construir E–O entre ({DEMO_BRIDGE_BANK_W},{DEMO_BRIDGE_Y}) y ({DEMO_BRIDGE_BANK_E},{DEMO_BRIDGE_Y})) | \
         túnel NE ({}, {})",
        crate::state::MAP_W,
        crate::state::MAP_H,
        DEMO_ECONOMY_INDUSTRY.x,
        DEMO_ECONOMY_INDUSTRY.y,
        DEMO_ECONOMY_LOAD_STATION.x,
        DEMO_ECONOMY_LOAD_STATION.y,
        DEMO_ECONOMY_DELIVER_STATION.x,
        DEMO_ECONOMY_DELIVER_STATION.y,
        DEMO_TUNNEL_NE.x,
        DEMO_TUNNEL_NE.y
    );
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::render::RenderGrid;
    use crate::state::bootstrap::place_tunnel_demo_ridge;
    use crate::state::{MAP_H, MAP_W};
    use openttdrs_core::tunnel_preview_path;

    #[test]
    fn clean_demo_has_road_rail_and_tunnel_spot() {
        let mut state = GameState::new(MAP_W, MAP_H);
        fill_flat_grass(&mut state);
        place_clean_demo_transport(&mut state);
        place_tunnel_demo_ridge(&mut state);
        place_bridge_demo_gap(&mut state);

        assert_eq!(
            state.map.get_kind(TileCoord::new(6, DEMO_ROAD_Y)),
            Some(TileKind::Road)
        );
        assert_eq!(
            state.map.get_kind(TileCoord::new(6, DEMO_RAIL_Y)),
            Some(TileKind::Rail)
        );
        let rail = state.map.get(TileCoord::new(6, DEMO_RAIL_Y)).unwrap();
        assert_eq!(rail.mapt, 0x10);
        assert_eq!(rail.m5 & 0x3F, 0x01, "vía demo horizontal: Track X");
        assert_eq!(
            state.map.get_kind(TileCoord::new(9, DEMO_BRIDGE_Y)),
            Some(TileKind::Water)
        );
        assert_eq!(
            state
                .map
                .get_kind(TileCoord::new(DEMO_BRIDGE_BANK_W, DEMO_BRIDGE_Y)),
            Some(TileKind::Grass)
        );
        assert!(tunnel_preview_path(&state.map, DEMO_TUNNEL_NE).is_some());
        assert_eq!(state.vehicles.len(), 3);
        assert!(
            state
                .vehicles
                .iter()
                .any(|v| v.kind == openttdrs_core::VehicleKind::Train)
        );
    }

    #[test]
    fn demo_economy_loop_has_industry_stations_and_ordered_truck() {
        let mut state = GameState::new(MAP_W, MAP_H);
        fill_flat_grass(&mut state);
        place_clean_demo_transport(&mut state);
        place_demo_economy_loop(&mut state);

        assert_eq!(state.industries.len(), 1);
        assert_eq!(state.stations.len(), 2);
        assert_eq!(
            state.map.get_kind(DEMO_ECONOMY_LOAD_STATION),
            Some(TileKind::Station)
        );
        assert_eq!(
            state.map.get_kind(DEMO_ECONOMY_DELIVER_STATION),
            Some(TileKind::Station)
        );
        assert_eq!(
            state.map.get_kind(TileCoord::new(3, DEMO_ROAD_Y)),
            Some(TileKind::Road),
            "la carretera demo no debe quedar cubierta por la parada"
        );
        assert_eq!(
            state.map.get_kind(TileCoord::new(10, DEMO_ROAD_Y)),
            Some(TileKind::Road)
        );
        let load = state.map.get(DEMO_ECONOMY_LOAD_STATION).unwrap();
        assert_ne!(load.m3 & 0x0F, 0, "boca de parada hacia la carretera");
        assert_eq!(load.m5 & 0x03, DEMO_ECONOMY_STATION_ENTRANCE_DIR);
        let truck = state
            .vehicles
            .iter()
            .find(|v| v.id == 9010)
            .expect("camión económico demo");
        assert_eq!(truck.orders.len(), 2);
        assert!(truck.running);
    }

    #[test]
    fn demo_economy_loop_delivers_cargo_over_sim_steps() {
        let mut state = GameState::new(MAP_W, MAP_H);
        fill_flat_grass(&mut state);
        place_clean_demo_transport(&mut state);
        place_demo_economy_loop(&mut state);

        for _ in 0..800 {
            state.step();
        }
        assert!(state.stats.cargo_units_loaded > 0, "debe cargar en la mina");
        assert!(
            state.stats.cargo_units_delivered > 0,
            "debe entregar en la estación lejana"
        );
        assert!(
            state.stats.cargo_income_earned > 0,
            "entrega genera ingresos TTD"
        );
    }

    #[test]
    fn bridge_channel_center_is_open_water_edges_marked_coast() {
        let mut state = GameState::new(MAP_W, MAP_H);
        fill_flat_grass(&mut state);
        place_bridge_demo_gap(&mut state);

        let center = state.map.get(TileCoord::new(9, DEMO_BRIDGE_Y)).unwrap();
        let edge = state
            .map
            .get(TileCoord::new(DEMO_BRIDGE_WATER_X0, DEMO_BRIDGE_Y))
            .unwrap();
        assert_eq!(center.kind, TileKind::Water);
        assert_eq!(edge.kind, TileKind::Water);
        assert_eq!((center.m5 >> 4) & 0x0F, 0, "centro del canal: agua abierta");
        assert_eq!(
            (edge.m5 >> 4) & 0x0F,
            1,
            "borde del canal: WaterTileType::Coast"
        );

        let grid = RenderGrid::from_map(&state.map, MAP_W, MAP_H);
        let center_ctx =
            crate::render::TileRenderContext::new(&state.map, &grid, 9, DEMO_BRIDGE_Y as u32);
        let edge_ctx = crate::render::TileRenderContext::new(
            &state.map,
            &grid,
            DEMO_BRIDGE_WATER_X0 as u32,
            DEMO_BRIDGE_Y as u32,
        );
        assert!(!center_ctx.info.use_shore, "centro del canal: agua abierta");
        assert!(edge_ctx.info.use_shore, "borde del canal: costa");
    }
}
