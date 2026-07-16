//! Servidor dedicado headless (ADR 0001 / #21).
//!
//! ```text
//! cargo run -p openttdrs-net --bin openttdrs-dedicated -- --bind 0.0.0.0:3979
//! cargo run -p openttdrs-net --bin openttdrs-dedicated -- --seed 42
//! ```

use std::env;
use std::thread;
use std::time::{Duration, Instant};

use openttdrs_core::Command;
use openttdrs_core::prelude::*;
use openttdrs_core::{
    Climate, IndustryKind, IndustrySpec, WorldGenConfig, apply_world_gen, tile_slope_and_z,
};
use openttdrs_net::{DEFAULT_PORT, ListenServer, SessionEvent};

fn main() {
    let opts = parse_args(env::args().skip(1));
    let mut state = build_dedicated_world(opts.seed);
    let (mw, mh) = state.map.dimensions();
    eprintln!(
        "openttdrs-dedicated: mapa {mw}×{mh} seed={} pueblos={} industrias={}",
        opts.seed,
        state.towns.len(),
        state.industries.len()
    );

    let snapshot = match state.save_json() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: no se pudo serializar el estado: {e}");
            std::process::exit(1);
        }
    };

    let server = match ListenServer::start(&opts.bind, snapshot) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: no se pudo escuchar en {}: {e}", opts.bind);
            std::process::exit(1);
        }
    };

    eprintln!(
        "openttdrs-dedicated: bound {} (Ctrl+C para salir)",
        opts.bind
    );
    let tick_period = Duration::from_millis(27); // ~37 Hz
    let mut last_tick = Instant::now();
    let mut ticks_since_hash = 0u32;

    loop {
        while let Some(event) = server.try_recv() {
            match event {
                SessionEvent::Commit { command, seq } => {
                    if let Err(e) = apply_command(&mut state, &command) {
                        eprintln!("dedicated: reject commit seq={seq}: {e}");
                    } else {
                        eprintln!("dedicated: applied commit seq={seq}");
                        publish_snapshot(&server, &state);
                    }
                }
                SessionEvent::Disconnected { reason } => {
                    eprintln!("dedicated: {reason}");
                }
                other => {
                    eprintln!("dedicated: ignore {other:?}");
                }
            }
        }

        if last_tick.elapsed() >= tick_period {
            state.step();
            // Primero avisar a peers ya conectados; luego publicar snapshot para late-join.
            let _ = server.broadcast_advance(1);
            publish_snapshot(&server, &state);
            ticks_since_hash += 1;
            if ticks_since_hash >= 37 {
                ticks_since_hash = 0;
                let tick = state.tick.get();
                let hash = state.canonical_hash();
                let _ = server.broadcast_hash(tick, hash);
            }
            last_tick = Instant::now();
        }

        thread::sleep(Duration::from_millis(1));
    }
}

struct DedicatedOpts {
    bind: String,
    seed: u64,
}

/// Isla 64×64 con terreno, pueblos, industrias y un tramo de vía.
fn build_dedicated_world(seed: u64) -> GameState {
    let mut state = GameState::new(64, 64);
    state.climate = Climate::Temperate;
    state.world_seed = seed;
    state.economy.money = 500_000;
    state.sync_active_from_mirrors();

    let cfg = WorldGenConfig {
        climate: Climate::Temperate,
        seed,
        sea_level: 1,
        island: true,
        height_span: 6,
    };
    if let Err(e) = apply_world_gen(&mut state.map, &cfg, &[]) {
        eprintln!("warning: world_gen falló ({e:?}); mapa plano");
    }

    // Fundar pueblos en hierba plana (reintentos por candidatos).
    let town_targets = [(20, 20), (44, 22), (32, 42), (18, 40), (46, 40)];
    for &(tx, ty) in &town_targets {
        if try_found_town_near(&mut state, tx, ty, 6) && state.towns.len() >= 3 {
            break;
        }
    }

    let industry_kind_targets = [
        (IndustryKind::CoalMine, 24_i32, 28_i32),
        (IndustryKind::Forest, 28, 36),
        (IndustryKind::Factory, 38, 30),
    ];
    for (kind, x, y) in industry_kind_targets {
        let _ = apply_command(
            &mut state,
            &Command::PlaceIndustryKind(TileCoord::new(x, y), kind),
        );
    }
    let _ = apply_command(
        &mut state,
        &Command::PlaceIndustrySpec(TileCoord::new(40, 38), IndustrySpec::Sawmill),
    );

    // Tramo de vía en hierba plana cerca del primer pueblo (o centro).
    let rail_origin = state
        .towns
        .first()
        .map(|t| (t.pos.x + 2, t.pos.y + 4))
        .unwrap_or((28, 28));
    place_rail_strip(&mut state, rail_origin.0, rail_origin.1, 8);

    state
}

fn try_found_town_near(state: &mut GameState, cx: i32, cy: i32, radius: i32) -> bool {
    for dy in -radius..=radius {
        for dx in -radius..=radius {
            let c = TileCoord::new(cx + dx, cy + dy);
            if apply_command(state, &Command::FoundTown(c)).is_ok() {
                return true;
            }
        }
    }
    false
}

fn place_rail_strip(state: &mut GameState, x0: i32, y: i32, len: i32) {
    for x in x0..x0 + len {
        let c = TileCoord::new(x, y);
        if state.map.get_kind(c) != Some(TileKind::Grass) {
            continue;
        }
        if tile_slope_and_z(&state.map, c).is_none_or(|(h, _)| h != 0) {
            continue;
        }
        let _ = apply_command(state, &Command::PlaceRail(c));
    }
}

fn publish_snapshot(server: &ListenServer, state: &GameState) {
    match state.save_json() {
        Ok(json) => server.update_snapshot(json),
        Err(e) => eprintln!("dedicated: snapshot failed: {e}"),
    }
}

fn parse_args(mut args: impl Iterator<Item = String>) -> DedicatedOpts {
    let mut bind = format!("0.0.0.0:{DEFAULT_PORT}");
    let mut seed = 0x4F54_4452_u64; // "OTDR"
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--bind" => {
                if let Some(v) = args.next() {
                    bind = v;
                }
            }
            "--seed" => {
                if let Some(v) = args.next() {
                    seed = parse_seed(&v);
                }
            }
            "--help" | "-h" => {
                eprintln!(
                    "Usage: openttdrs-dedicated [--bind HOST:PORT] [--seed N]\n\
                     Default bind: 0.0.0.0:{DEFAULT_PORT}\n\
                     Default seed: 0x4F544452 (isla 64×64 con pueblos/industrias)"
                );
                std::process::exit(0);
            }
            other if other.starts_with("--bind=") => {
                bind = other.trim_start_matches("--bind=").to_string();
            }
            other if other.starts_with("--seed=") => {
                seed = parse_seed(other.trim_start_matches("--seed="));
            }
            other => {
                eprintln!("unknown arg: {other}");
                std::process::exit(2);
            }
        }
    }
    DedicatedOpts { bind, seed }
}

fn parse_seed(s: &str) -> u64 {
    let t = s.trim();
    if let Some(hex) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
        u64::from_str_radix(hex, 16).unwrap_or_else(|_| {
            eprintln!("error: seed hex inválido: {s}");
            std::process::exit(2);
        })
    } else {
        t.parse().unwrap_or_else(|_| {
            eprintln!("error: seed inválido: {s}");
            std::process::exit(2);
        })
    }
}
