//! Benchmarks headless de `GameState::step` (#116).
//!
//! Escenarios: flota parity (`truck_bay`, `train_pbs`) y mapa 256×256 procedural.
//! No muta fixtures ni goldens.

use std::hint::black_box;

use criterion::{BatchSize, Criterion, Throughput, criterion_group, criterion_main};

#[path = "common.rs"]
mod common;

use common::{large_world_gen_map, scenario, step_n};

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

    group.finish();
}

criterion_group!(benches, bench_sim_tick);
criterion_main!(benches);
