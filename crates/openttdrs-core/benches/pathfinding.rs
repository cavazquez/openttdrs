//! Benchmarks headless de pathfinding road/rail (#116).
//!
//! Cold: `find_path` / YAPF sin caché.
//! Hot: `find_path_cached` con el mismo par origen→destino dentro del tick.
//! `iter_batched_ref` mantiene el escenario, caché y ruta devuelta fuera del
//! intervalo: cada routine debe retornar la ruta opaca, no destruirla dentro.

use std::hint::black_box;

use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use openttdrs_core::parity::{
    TRAIN_LINE_DEPOT, TRAIN_LINE_STATION_A, TRAIN_LINE_STATION_B, TRUCK_BAY_DELIVER_ROAD,
    TRUCK_BAY_LOAD_ROAD,
};
use openttdrs_core::{PathCache, PathNetwork, find_path, find_path_cached};

#[path = "common.rs"]
mod common;

use common::scenario;

fn bench_pathfinding(c: &mut Criterion) {
    let mut group = c.benchmark_group("pathfinding");

    group.bench_function("road/truck_bay/cold", |b| {
        b.iter_batched_ref(
            || scenario("truck_bay"),
            |state| {
                black_box(find_path(
                    &state.map,
                    TRUCK_BAY_LOAD_ROAD,
                    TRUCK_BAY_DELIVER_ROAD,
                    PathNetwork::Road,
                ))
            },
            BatchSize::SmallInput,
        );
    });

    group.bench_function("road/truck_bay/hot_cache", |b| {
        b.iter_batched_ref(
            || {
                let state = scenario("truck_bay");
                let mut cache = PathCache::default();
                cache.begin_tick(1);
                // Miss inicial fuera del timer de iteración interna via setup.
                let _ = find_path_cached(
                    &state.map,
                    &mut cache,
                    TRUCK_BAY_LOAD_ROAD,
                    TRUCK_BAY_DELIVER_ROAD,
                    PathNetwork::Road,
                    None,
                );
                (state, cache)
            },
            |(state, cache)| {
                black_box(find_path_cached(
                    &state.map,
                    cache,
                    TRUCK_BAY_LOAD_ROAD,
                    TRUCK_BAY_DELIVER_ROAD,
                    PathNetwork::Road,
                    None,
                ))
            },
            BatchSize::SmallInput,
        );
    });

    group.bench_function("rail/train_line/cold", |b| {
        b.iter_batched_ref(
            || scenario("train_line"),
            |state| {
                black_box(find_path(
                    &state.map,
                    TRAIN_LINE_DEPOT,
                    TRAIN_LINE_STATION_A,
                    PathNetwork::Rail,
                ))
            },
            BatchSize::SmallInput,
        );
    });

    group.bench_function("rail/train_line/a_to_b/cold", |b| {
        b.iter_batched_ref(
            || scenario("train_line"),
            |state| {
                black_box(find_path(
                    &state.map,
                    TRAIN_LINE_STATION_A,
                    TRAIN_LINE_STATION_B,
                    PathNetwork::Rail,
                ))
            },
            BatchSize::SmallInput,
        );
    });

    group.finish();
}

criterion_group!(benches, bench_pathfinding);
criterion_main!(benches);
