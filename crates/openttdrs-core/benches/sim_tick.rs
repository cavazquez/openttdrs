//! Benchmarks headless de `GameState::step` (#116).
//!
//! Escenarios: flota parity (`truck_bay`, `train_pbs`) y mapas procedurales 256²–4096².
//! No muta fixtures ni goldens.

use std::hint::black_box;

use criterion::{BatchSize, Criterion, Throughput, criterion_group, criterion_main};

#[path = "common.rs"]
mod common;

use common::{
    cargodist_unload_burst, indexed_signal_map_sized, large_world_gen_map,
    large_world_gen_map_sized, scenario, step_n,
};

fn bench_sim_tick(c: &mut Criterion) {
    let mut group = c.benchmark_group("sim_tick");

    for (label, name, ticks) in [
        ("truck_bay/100", "truck_bay", 100_u32),
        ("truck_bay/500", "truck_bay", 500),
        ("train_pbs/100", "train_pbs", 100),
        ("train_pbs/500", "train_pbs", 500),
    ] {
        group.throughput(Throughput::Elements(u64::from(ticks)));
        group.bench_function(label, |b| {
            b.iter_batched(
                || scenario(name),
                |mut state| {
                    step_n(&mut state, ticks);
                    black_box(state.tick.get());
                },
                BatchSize::SmallInput,
            );
        });
    }

    group.throughput(Throughput::Elements(50));
    group.bench_function("large_256_world_gen/50", |b| {
        b.iter_batched(
            large_world_gen_map,
            |mut state| {
                step_n(&mut state, 50);
                black_box(state.tick.get());
            },
            BatchSize::LargeInput,
        );
    });

    // 1024² ≈ 14 MiB de tiles: clonar plantilla por iteración es viable.
    group.throughput(Throughput::Elements(50));
    group.bench_function("large_1024_world_gen/50", |b| {
        let template = large_world_gen_map_sized(1024);
        b.iter_batched(
            || template.clone(),
            |mut state| {
                step_n(&mut state, 50);
                black_box(state.tick.get());
            },
            BatchSize::LargeInput,
        );
    });

    // 4096² ≈ 224 MiB: evitar clonar; medir ticks en estado estable (acumula tick).
    group.throughput(Throughput::Elements(20));
    group.bench_function("large_4096_world_gen/20", |b| {
        let mut state = large_world_gen_map_sized(4096);
        b.iter(|| {
            step_n(&mut state, 20);
            black_box(state.tick.get());
        });
    });

    group.throughput(Throughput::Elements(128));
    group.bench_function("cargodist/unload_burst_128", |b| {
        b.iter_batched(
            || cargodist_unload_burst(128),
            |mut state| {
                state.step();
                black_box(state.runtime.station_flow_rebuilds);
            },
            BatchSize::LargeInput,
        );
    });

    group.finish();

    let mut group = c.benchmark_group("signal_glob_indexed");
    for side in [1_024_u32, 4_096] {
        let mut state = indexed_signal_map_sized(side);
        let signal_count = state.runtime.signal_spatial_index.len();
        group.throughput(Throughput::Elements(
            u64::try_from(signal_count).unwrap_or(u64::MAX),
        ));
        group.bench_function(format!("dense_{side}"), |b| {
            b.iter(|| {
                state.runtime.signal_tile_dirty.clear();
                openttdrs_core::rail_signals::enqueue_trains_for_signal_update(
                    &mut state.runtime.signal_globset,
                    &state.vehicles,
                );
                openttdrs_core::rail_signals::drain_signal_globset_indexed_with_wormholes(
                    &mut state.map,
                    &state.vehicles,
                    &mut state.runtime.signal_tile_dirty,
                    &mut state.runtime.signal_globset,
                    &mut state.runtime.signal_spatial_index,
                    None,
                );
                black_box(state.runtime.signal_tile_dirty.len());
            });
        });
    }
    group.finish();

    let mut group = c.benchmark_group("fleet_indices");
    const FLEET_SIZE: u32 = 16_384;
    let pos = openttdrs_core::TileCoord::new(1, 1);
    let mut fleet: Vec<_> = (0..FLEET_SIZE)
        .map(|id| openttdrs_core::Vehicle::new(id, openttdrs_core::VehicleKind::Train, pos, pos))
        .collect();
    for base in (0..FLEET_SIZE).step_by(64) {
        for offset in 0..63_u32 {
            let slot = usize::try_from(base + offset).unwrap_or(0);
            fleet[slot].next_unit = Some(base + offset + 1);
            fleet[slot + 1].prev_unit = Some(base + offset);
        }
    }
    group.throughput(Throughput::Elements(u64::from(FLEET_SIZE)));
    group.bench_function("rebuild_16384", |b| {
        let mut index = openttdrs_core::FleetIndex::default();
        b.iter(|| {
            index.rebuild(&fleet);
            black_box(index.slot(FLEET_SIZE - 1));
            black_box(index.consist(0).len());
        });
    });
    let mut index = openttdrs_core::FleetIndex::default();
    index.rebuild(&fleet);
    group.bench_function("lookup_16384", |b| {
        b.iter(|| {
            for id in (0..FLEET_SIZE).step_by(127) {
                black_box(index.slot(id));
                black_box(index.head_id(id));
            }
        });
    });
    group.finish();

    let mut group = c.benchmark_group("depot_spatial_index");
    let mut map = openttdrs_core::Map::new_flat(4_096, 4_096, 0);
    for n in 0..128_i32 {
        map.set_kind(
            openttdrs_core::TileCoord::new(32 * n, 31 * n),
            openttdrs_core::TileKind::RoadDepot,
        )
        .unwrap_or_else(|error| panic!("depot bench setup: {error:?}"));
    }
    let mut depot_index = openttdrs_core::depot::DepotSpatialIndex::default();
    let from = openttdrs_core::TileCoord::new(2_048, 2_048);
    black_box(openttdrs_core::depot::nearest_depot_tile_indexed(
        &map,
        from,
        openttdrs_core::VehicleKind::Truck,
        &mut depot_index,
    ));
    assert_eq!(depot_index.full_map_scans(), 1);
    group.bench_function("lookup_4096_squared", |b| {
        b.iter(|| {
            black_box(openttdrs_core::depot::nearest_depot_tile_indexed(
                &map,
                from,
                openttdrs_core::VehicleKind::Truck,
                &mut depot_index,
            ));
        });
    });
    group.finish();
}

criterion_group!(benches, bench_sim_tick);
criterion_main!(benches);
