//! Perfila fases del tick en mapas grandes (headless).
//!
//! ```bash
//! cargo run -p openttdrs-core --release --bin sim_profile
//! cargo run -p openttdrs-core --release --bin sim_profile -- --side 1024 --ticks 200
//! cargo run -p openttdrs-core --release --bin sim_profile -- --side 1024 --climate subarctic
//! ```

#![allow(clippy::print_stdout, clippy::expect_used, clippy::unwrap_used)]

use std::env;
use std::time::Instant;

use openttdrs_core::{
    Climate, GameState, TickPhaseTimings, WorldGenConfig, apply_world_gen, step_profiled,
};

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let side = parse_u32(&args, "--side", 1024);
    let ticks = parse_u32(&args, "--ticks", 200);
    let climate = parse_climate(&args);
    let warm = parse_u32(&args, "--warm", 20);

    println!("=== sim_profile ===");
    println!("side={side}²  ticks={ticks}  warm={warm}  climate={climate:?}");
    println!("presupuesto 1× (~37 Hz) = 27_000 µs/tick");
    println!();

    let t0 = Instant::now();
    let mut state = GameState::new(side, side);
    let cfg = WorldGenConfig {
        climate,
        seed: 116,
        sea_level: 1,
        island: false,
        height_span: 6,
    };
    apply_world_gen(&mut state.map, &cfg, &[]).expect("world_gen");
    state.world_seed = cfg.seed;
    state.climate = cfg.climate;
    println!(
        "world_gen: {:.1} ms  map={}×{}",
        t0.elapsed().as_secs_f64() * 1000.0,
        side,
        side
    );

    for _ in 0..warm {
        state.step();
    }

    let mut acc = TickPhaseTimings::default();
    let mut max_total = 0_u64;
    let mut day_acc = TickPhaseTimings::default();
    let mut day_n = 0_u64;
    let wall = Instant::now();
    for _ in 0..ticks {
        let t = step_profiled(&mut state);
        acc.accumulate(t);
        max_total = max_total.max(t.total_ns);
        // Día de tránsito: tick múltiplo de 74 tras advance.
        let after = state.tick.get();
        if after > 0 && after.is_multiple_of(74) {
            day_acc.accumulate(t);
            day_n += 1;
        }
    }
    let wall_ms = wall.elapsed().as_secs_f64() * 1000.0;
    let mean = acc.mean(u64::from(ticks));

    println!();
    println!("=== Media por tick ({ticks} samples) ===");
    print_timings(&mean);
    println!("max total: {:>10.1} µs/tick", max_total as f64 / 1000.0);
    println!(
        "wall: {wall_ms:.1} ms  → {:.1} µs/tick efectivo",
        (wall_ms * 1000.0) / f64::from(ticks)
    );

    if day_n > 0 {
        println!();
        println!("=== Media en ticks de día de tránsito (nieve O(map); n={day_n}) ===");
        print_timings(&day_acc.mean(day_n));
    } else {
        println!();
        println!("(ningún tick de día de tránsito en la ventana; subí --ticks)");
    }
}

fn print_timings(t: &TickPhaseTimings) {
    let rows = [
        ("economy_and_world", t.economy_and_world_ns),
        ("routing_and_signals", t.routing_and_signals_ns),
        ("tile_animation", t.tile_animation_ns),
        ("cargo_transfer", t.cargo_transfer_ns),
        ("vehicle_ops_pre_move", t.vehicle_ops_pre_move_ns),
        ("movement", t.movement_ns),
        ("post_tick", t.post_tick_ns),
        ("TOTAL", t.total_ns),
    ];
    let total = t.total_ns.max(1) as f64;
    println!("{:>22}  {:>10}  {:>6}", "fase", "µs", "%");
    for (name, ns) in rows {
        let us = ns as f64 / 1000.0;
        let pct = (ns as f64 / total) * 100.0;
        println!("{name:>22}  {us:>10.1}  {pct:>5.1}%");
    }
}

fn parse_u32(args: &[String], key: &str, default: u32) -> u32 {
    let mut i = 0;
    while i < args.len() {
        if args[i] == key
            && let Some(v) = args.get(i + 1)
        {
            return v.parse().unwrap_or(default);
        }
        if let Some(v) = args[i].strip_prefix(&format!("{key}=")) {
            return v.parse().unwrap_or(default);
        }
        i += 1;
    }
    default
}

fn parse_climate(args: &[String]) -> Climate {
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--climate"
            && let Some(v) = args.get(i + 1)
        {
            return climate_from_str(v);
        }
        if let Some(v) = args[i].strip_prefix("--climate=") {
            return climate_from_str(v);
        }
        i += 1;
    }
    Climate::Temperate
}

fn climate_from_str(s: &str) -> Climate {
    match s.to_ascii_lowercase().as_str() {
        "temperate" | "temp" => Climate::Temperate,
        "subarctic" | "arctic" | "snow" => Climate::SubArctic,
        "subtropical" | "tropic" => Climate::SubTropical,
        "toyland" => Climate::Toyland,
        _ => Climate::Temperate,
    }
}
