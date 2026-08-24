//! Mapa procedural limpio: hierba plana + zonas de prueba separadas.

use bevy::prelude::*;
use openttdrs_core::Command;
use openttdrs_core::prelude::*;
use openttdrs_core::{
    BridgeType, FACTORY_GRAIN_INPUT, FACTORY_LIVESTOCK_INPUT, FACTORY_STEEL_INPUT, Industry,
    IndustryKind, IndustrySpec, PathNetwork, PreserveRect, WorldGenConfig, WorldGenRng,
    apply_world_gen_with_rng, find_path, road_stop_approach_tile,
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
/// Fábrica que consume carbón (y madera) entregado en la parada de descarga.
pub const DEMO_ECONOMY_FACTORY: TileCoord = TileCoord::new(8, 2);
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

/// Zonas del mapa demo que deben permanecer planas tras `apply_world_gen`.
#[must_use]
pub(crate) fn demo_preserve_rects() -> Vec<PreserveRect> {
    vec![
        PreserveRect {
            x0: 0,
            y0: 0,
            x1: 13,
            y1: 5,
        },
        PreserveRect {
            x0: 1,
            y0: DEMO_ROAD_Y - 1,
            x1: 13,
            y1: DEMO_ROAD_Y + 1,
        },
        PreserveRect {
            x0: 1,
            y0: DEMO_RAIL_Y - 1,
            x1: 13,
            y1: DEMO_RAIL_Y + 1,
        },
        PreserveRect {
            x0: DEMO_BRIDGE_WATER_X0 - 1,
            y0: DEMO_BRIDGE_BANK_N - 1,
            x1: DEMO_BRIDGE_WATER_X1 + 1,
            y1: DEMO_BRIDGE_BANK_S + 1,
        },
        PreserveRect {
            x0: DEMO_TUNNEL_NE.x - 3,
            y0: DEMO_TUNNEL_NE.y - 2,
            x1: DEMO_TUNNEL_NE.x + 2,
            y1: DEMO_TUNNEL_NE.y + 2,
        },
        // Showcase acuático, ferroviario y aéreo del mapa completo.
        PreserveRect {
            x0: 4,
            y0: 20,
            x1: 59,
            y1: 29,
        },
        PreserveRect {
            x0: 5,
            y0: 34,
            x1: 58,
            y1: 43,
        },
        PreserveRect {
            x0: 4,
            y0: 46,
            x1: 59,
            y1: 54,
        },
        // Galería exhaustiva de 57 uniones ferroviarias (`TrackBits` 2..=6).
        PreserveRect {
            x0: 1,
            y0: 55,
            x1: 60,
            y1: 63,
        },
    ]
}

pub(crate) fn apply_optional_world_gen(
    state: &mut GameState,
    config: WorldGenConfig,
    preserve: &[PreserveRect],
) -> Option<WorldGenRng> {
    match apply_world_gen_with_rng(&mut state.map, &config, preserve) {
        Ok(rng) => Some(rng),
        Err(e) => {
            error!("Generación procedural fallida: {e:?}");
            None
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
    let _ = apply_command(
        state,
        &Command::PlaceRailSignal(
            TileCoord::new(8, DEMO_RAIL_Y),
            0,
            128,
            128,
            openttdrs_core::SIGTYPE_BLOCK,
        ),
    );
}

/// Industria + dos paradas de camión + ruta con órdenes para un ciclo jugable al arrancar.
pub(crate) fn place_demo_economy_loop(state: &mut GameState) {
    let _ = state
        .map
        .set_kind(DEMO_ECONOMY_INDUSTRY, TileKind::Industry);
    let mut mine = Industry::new(DEMO_ECONOMY_INDUSTRY, IndustryKind::CoalMine);
    mine.stock = 64;
    state.industries.push(mine);

    let _ = apply_command(
        state,
        &Command::PlaceIndustrySpec(DEMO_ECONOMY_FACTORY, IndustrySpec::Factory),
    );

    place_demo_truck_station(state, DEMO_ECONOMY_LOAD_STATION);
    place_demo_truck_station(state, DEMO_ECONOMY_DELIVER_STATION);
    seed_factory_inputs_at_deliver_station(state);

    let orders = vec![DEMO_ECONOMY_LOAD_STATION, DEMO_ECONOMY_DELIVER_STATION];
    let load_road = road_stop_approach_tile(&state.map, DEMO_ECONOMY_LOAD_STATION)
        .unwrap_or(DEMO_ECONOMY_LOAD_STATION);
    let mut truck = Vehicle::new(
        9010,
        VehicleKind::Truck,
        load_road,
        DEMO_ECONOMY_LOAD_STATION,
    );
    truck.running = true;
    truck.set_station_orders(orders);
    truck.sync_order_destination(&state.map);
    if truck.pos != truck.dest
        && let Some(path) = find_path(&state.map, truck.pos, truck.dest, PathNetwork::Road)
    {
        truck.path = path.into();
    }
    state.vehicles.push(truck);
}

/// Insumos temperate iniciales en la parada de descarga para que la fábrica procese.
fn seed_factory_inputs_at_deliver_station(state: &mut GameState) {
    if let Some(station) = state
        .stations
        .iter_mut()
        .find(|s| s.pos == DEMO_ECONOMY_DELIVER_STATION)
    {
        station.cargo_stock.livestock = FACTORY_LIVESTOCK_INPUT * 8;
        station.cargo_stock.grain = FACTORY_GRAIN_INPUT * 8;
        station.cargo_stock.steel = FACTORY_STEEL_INPUT * 8;
    }
}

fn place_demo_truck_station(state: &mut GameState, pos: TileCoord) {
    let _ = apply_command(
        state,
        &Command::PlaceStationDir(pos, DEMO_ECONOMY_STATION_ENTRANCE_DIR),
    );
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

/// Convierte las dos zonas de ingeniería de la demo en infraestructura
/// ferroviaria terminada. Quedan deliberadamente separadas del circuito de
/// producción: son una muestra visible y segura para probar construcción,
/// render, señales y reservas sin alterar el tráfico de la mina.
pub(crate) fn place_demo_rail_structures(state: &mut GameState) {
    let bridge_west = TileCoord::new(DEMO_BRIDGE_BANK_W, DEMO_BRIDGE_Y);
    let bridge_east = TileCoord::new(DEMO_BRIDGE_BANK_E, DEMO_BRIDGE_Y);
    if let Err(error) = apply_command(
        state,
        &Command::PlaceRailBridge(bridge_west, bridge_east, BridgeType::Wooden),
    ) {
        error!("Demo: no se pudo construir puente ferroviario: {error:?}");
    }
    // Aproximaciones cortas para que el puente se lea como un tramo de vía y
    // pueda prolongarse desde la interfaz sin reconstruir sus rampas.
    for x in [DEMO_BRIDGE_BANK_W - 1, DEMO_BRIDGE_BANK_E + 1] {
        if let Err(error) =
            apply_command(state, &Command::PlaceRail(TileCoord::new(x, DEMO_BRIDGE_Y)))
        {
            error!("Demo: no se pudo construir aproximación ferroviaria: {error:?}");
        }
    }

    // La cresta se prepara con entrada NE en (18,8) y salida complementaria
    // en (16,8). `PlaceRailTunnel` resuelve la boca opuesta desde el relieve;
    // pasarla explícitamente documenta la geometría visible de la demo.
    let tunnel_exit = TileCoord::new(DEMO_TUNNEL_NE.x - 2, DEMO_TUNNEL_NE.y);
    if let Err(error) = apply_command(
        state,
        &Command::PlaceRailTunnel(DEMO_TUNNEL_NE, tunnel_exit),
    ) {
        error!("Demo: no se pudo construir túnel ferroviario: {error:?}");
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
         economía mina ({},{}) → fábrica ({},{}) vía est ({},{}) → ({},{}) (camión #9010) | \
         puente agua x={DEMO_BRIDGE_WATER_X0}..{DEMO_BRIDGE_WATER_X1} y={DEMO_BRIDGE_WATER_Y0}..{DEMO_BRIDGE_WATER_Y1} \
         orillas x={DEMO_BRIDGE_BANK_W},{DEMO_BRIDGE_BANK_E} y={DEMO_BRIDGE_BANK_N},{DEMO_BRIDGE_BANK_S} \
         (puente ferroviario E–O terminado entre ({DEMO_BRIDGE_BANK_W},{DEMO_BRIDGE_Y}) y ({DEMO_BRIDGE_BANK_E},{DEMO_BRIDGE_Y})) | \
         túnel ferroviario NE ({}, {})",
        crate::state::MAP_W,
        crate::state::MAP_H,
        DEMO_ECONOMY_INDUSTRY.x,
        DEMO_ECONOMY_INDUSTRY.y,
        DEMO_ECONOMY_FACTORY.x,
        DEMO_ECONOMY_FACTORY.y,
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
        assert!(
            state.vehicles.is_empty(),
            "el trazado limpio no deja vehículos apagados"
        );
    }

    #[test]
    fn rail_engineering_demo_builds_bridge_and_tunnel() {
        let mut state = GameState::new(MAP_W, MAP_H);
        fill_flat_grass(&mut state);
        place_tunnel_demo_ridge(&mut state);
        place_bridge_demo_gap(&mut state);
        place_demo_rail_structures(&mut state);

        assert_eq!(
            state
                .map
                .get_kind(TileCoord::new(DEMO_BRIDGE_BANK_W, DEMO_BRIDGE_Y)),
            Some(TileKind::RailBridge)
        );
        assert_eq!(
            state
                .map
                .get_kind(TileCoord::new(DEMO_BRIDGE_BANK_E, DEMO_BRIDGE_Y)),
            Some(TileKind::RailBridge)
        );
        for x in (DEMO_TUNNEL_NE.x - 2)..=DEMO_TUNNEL_NE.x {
            assert_eq!(
                state.map.get_kind(TileCoord::new(x, DEMO_TUNNEL_NE.y)),
                Some(TileKind::RailTunnel),
                "tramo de túnel x={x}"
            );
        }
    }

    #[test]
    fn demo_economy_loop_has_industry_stations_and_ordered_truck() {
        let mut state = GameState::new(MAP_W, MAP_H);
        fill_flat_grass(&mut state);
        place_clean_demo_transport(&mut state);
        place_demo_economy_loop(&mut state);

        assert_eq!(state.industries.len(), 2);
        assert!(
            state
                .industries
                .iter()
                .any(|i| i.spec == Some(IndustrySpec::Factory)),
            "fábrica consumidora junto a la parada de descarga"
        );
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
    fn demo_economy_loop_transfers_cargo_over_sim_steps() {
        let mut state = GameState::new(MAP_W, MAP_H);
        fill_flat_grass(&mut state);
        place_clean_demo_transport(&mut state);
        place_demo_economy_loop(&mut state);

        for _ in 0..1200 {
            state.step();
        }
        assert!(state.stats.cargo_units_loaded > 0, "debe cargar en la mina");
        assert!(
            state.stats.cargo_units_delivered > 0,
            "el trasbordo registra las unidades descargadas en la estación lejana"
        );
        assert!(
            state.stats.cargo_income_earned > 0,
            "el trasbordo genera ingresos TTD"
        );
        let deliver = state
            .stations
            .iter()
            .find(|s| s.pos == DEMO_ECONOMY_DELIVER_STATION)
            .expect("parada descarga");
        assert!(
            deliver.cargo_stock.coal > 0 || deliver.income > 0,
            "carbón acumulado en parada de descarga tras el trasbordo del camión"
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
