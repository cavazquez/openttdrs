//! Helpers compartidos entre benches headless (#116).
#![allow(dead_code)] // cada [[bench]] incluye el módulo completo
#![allow(clippy::expect_used)] // fixtures de bench: fallo = setup inválido

use openttdrs_core::flow_stat::DistributionType;
use openttdrs_core::map::RAIL_TB_X;
use openttdrs_core::parity::build_scenario;
use openttdrs_core::rail_signals::{
    RAIL_TILE_NORMAL, RAIL_TILE_SIGNALS, SIGTYPE_BLOCK, SignalTrack,
    drain_signal_globset_indexed_with_wormholes, enqueue_trains_for_signal_update,
    signal_placement_for_track,
};
use openttdrs_core::{
    CargoType, Climate, GameState, Station, TileCoord, TileKind, Vehicle, VehicleKind,
    WorldGenConfig, apply_world_gen,
};

/// Escenario parity o panic con mensaje claro (fixture de bench, no runtime).
#[must_use]
pub fn scenario(name: &str) -> GameState {
    build_scenario(name).unwrap_or_else(|| panic!("escenario parity desconocido: {name}"))
}

/// Mapa grande procedural (256×256) sin flota — mide coste de tick sobre terreno.
#[must_use]
pub fn large_world_gen_map() -> GameState {
    large_world_gen_map_sized(256)
}

/// Mapa procedural cuadrado `side×side` sin flota (seed 116, temperate).
#[must_use]
pub fn large_world_gen_map_sized(side: u32) -> GameState {
    let mut state = GameState::new(side, side);
    let cfg = WorldGenConfig {
        climate: Climate::Temperate,
        seed: 116,
        sea_level: 1,
        island: false,
        ..WorldGenConfig::default().with_height_span(6)
    };
    apply_world_gen(&mut state.map, &cfg, &[]).unwrap_or_else(|e| {
        panic!("world_gen bench {side}×{side}: {e:?}");
    });
    state.world_seed = cfg.seed;
    state.climate = cfg.climate;
    state
}

/// Ráfaga de vehículos que descargan en el mismo tick con CargoDist activo.
///
/// Expone regresiones donde cada entrega vuelve a ejecutar Demand + MCF (#215).
#[must_use]
pub fn cargodist_unload_burst(vehicle_count: u32) -> GameState {
    let mut state = GameState::new(64, 8);
    let origin = TileCoord::new(4, 0);
    let destination = TileCoord::new(48, 0);
    state.stations.push(Station::new(origin));
    state.stations.push(Station::new(destination));
    state.cargo_dist.distribution = DistributionType::Asymmetric;

    for id in 0..vehicle_count {
        let mut vehicle = Vehicle::new(id, VehicleKind::Truck, destination, destination);
        vehicle.cargo = 10;
        vehicle.cargo_type = Some(CargoType::Goods);
        vehicle.cargo_source = Some(origin);
        vehicle.ensure_packets_from_legacy();
        vehicle.last_pickup_station = Some(origin);
        state.vehicles.push(vehicle);
    }
    state
}

/// Mapa cuadrado con corredores ferroviarios señalizados e índice ya inicializado.
///
/// Hay un corredor cada 256 filas y una señal cada 32 teselas; cada tren invalida
/// un bloque. El setup fuerza el único barrido completo fuera de la medición (#214).
#[must_use]
pub fn indexed_signal_map_sized(side: u32) -> GameState {
    let mut state = GameState::new(side, side);
    let side_i32 = i32::try_from(side).expect("side cabe en i32");
    let corridor_count = (side / 256).max(1);
    let placement =
        signal_placement_for_track(SignalTrack::X, 0, 1, SIGTYPE_BLOCK).expect("señal X");

    for lane in 0..corridor_count {
        let y = i32::try_from((lane + 1) * side / (corridor_count + 1)).expect("y cabe en i32");
        for x in 0..side_i32 {
            let c = TileCoord::new(x, y);
            let mut tile = state.map.get(c).expect("tesela de benchmark");
            tile.kind = TileKind::Rail;
            if x % 32 == 16 {
                tile.m5 = RAIL_TB_X | (RAIL_TILE_SIGNALS << 6);
                tile.m2 = placement.m2;
                tile.m3 = placement.m3;
                tile.m3hi = placement.m3hi;
            } else {
                tile.m5 = RAIL_TB_X | (RAIL_TILE_NORMAL << 6);
            }
            state.map.set_tile(c, tile).expect("tesela rail");
        }
        let train_pos = TileCoord::new(side_i32 / 2 + 1, y);
        state
            .vehicles
            .push(Vehicle::new(lane, VehicleKind::Train, train_pos, train_pos));
    }

    enqueue_trains_for_signal_update(&mut state.runtime.signal_globset, &state.vehicles);
    drain_signal_globset_indexed_with_wormholes(
        &mut state.map,
        &state.vehicles,
        &mut state.runtime.signal_tile_dirty,
        &mut state.runtime.signal_globset,
        &mut state.runtime.signal_spatial_index,
        None,
    );
    assert_eq!(state.runtime.signal_spatial_index.full_map_scans(), 1);
    state
}

/// Avanza exactamente `n` ticks.
pub fn step_n(state: &mut GameState, n: u32) {
    for _ in 0..n {
        state.step();
    }
}
