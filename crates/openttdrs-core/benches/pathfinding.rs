//! Benchmarks headless de pathfinding road/rail (#116).
//!
//! Cold: `find_path` / YAPF sin caché.
//! Hot: `find_path_cached` con el mismo par origen→destino dentro del tick.

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
        b.iter_batched(
            || scenario("truck_bay"),
            |state| {
                let path = find_path(
                    &state.map,
                    TRUCK_BAY_LOAD_ROAD,
                    TRUCK_BAY_DELIVER_ROAD,
                    PathNetwork::Road,
                );
                black_box(path.map(|p| p.len()));
            },
            BatchSize::SmallInput,
        );
    });

    group.bench_function("road/truck_bay/hot_cache", |b| {
        b.iter_batched(
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
            |(state, mut cache)| {
                let path = find_path_cached(
                    &state.map,
                    &mut cache,
                    TRUCK_BAY_LOAD_ROAD,
                    TRUCK_BAY_DELIVER_ROAD,
                    PathNetwork::Road,
                    None,
                );
                black_box(path.map(|p| p.len()));
            },
            BatchSize::SmallInput,
        );
    });

    group.bench_function("rail/train_line/cold", |b| {
        b.iter_batched(
            || scenario("train_line"),
            |state| {
                let path = find_path(
                    &state.map,
                    TRAIN_LINE_DEPOT,
                    TRAIN_LINE_STATION_A,
                    PathNetwork::Rail,
                );
                black_box(path.map(|p| p.len()));
            },
            BatchSize::SmallInput,
        );
    });

    group.bench_function("rail/train_line/a_to_b/cold", |b| {
        b.iter_batched(
            || scenario("train_line"),
            |state| {
                let path = find_path(
                    &state.map,
                    TRAIN_LINE_STATION_A,
                    TRAIN_LINE_STATION_B,
                    PathNetwork::Rail,
                );
                black_box(path.map(|p| p.len()));
            },
            BatchSize::SmallInput,
        );
    });

    group.finish();
}

criterion_group!(benches, bench_pathfinding);
criterion_main!(benches);
