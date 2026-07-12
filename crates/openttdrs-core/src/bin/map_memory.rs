//! Mide densidad de `Tile`/`Map` frente al layout OpenTTD (~12 B/tile).
//!
//! ```bash
//! cargo run -p openttdrs-core --bin map_memory
//! cargo run -p openttdrs-core --bin map_memory -- --alloc-max 1024
//! ```

#![allow(clippy::print_stdout, clippy::expect_used)]

use std::time::Instant;

use openttdrs_core::{Map, Tile, TileKind};

/// Bytes por tesela en OpenTTD (`TileBase` 8 + `TileExtended` 4).
const OPENTTD_BYTES_PER_TILE: usize = 12;

fn rss_kib() -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    for line in status.lines() {
        let Some(rest) = line.strip_prefix("VmRSS:") else {
            continue;
        };
        return rest.split_whitespace().next()?.parse().ok();
    }
    None
}

fn parse_alloc_max(args: &[String]) -> u32 {
    let mut max = 2048_u32;
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--alloc-max"
            && let Some(v) = args.get(i + 1)
        {
            max = v.parse().unwrap_or(max);
            i += 2;
            continue;
        }
        if let Some(v) = args[i].strip_prefix("--alloc-max=") {
            max = v.parse().unwrap_or(max);
        }
        i += 1;
    }
    max
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let alloc_max = parse_alloc_max(&args);

    let tile_b = size_of::<Tile>();
    let kind_b = size_of::<TileKind>();
    let align = align_of::<Tile>();

    println!("=== Densidad de tesela ===");
    println!("size_of::<Tile>()     = {tile_b} B");
    println!("size_of::<TileKind>() = {kind_b} B");
    println!("align_of::<Tile>()    = {align}");
    println!("OpenTTD (base+ext)    = {OPENTTD_BYTES_PER_TILE} B/tile");
    println!(
        "overhead vs OpenTTD   = {:+} B/tile ({:.1}%)",
        tile_b as isize - OPENTTD_BYTES_PER_TILE as isize,
        (tile_b as f64 / OPENTTD_BYTES_PER_TILE as f64 - 1.0) * 100.0
    );
    println!();
    println!("=== Estimación teórica (solo Vec<Tile>) ===");
    println!(
        "{:>8}  {:>12}  {:>12}  {:>12}",
        "lado", "tiles", "openttdrs", "OpenTTD~"
    );
    for side in [64_u32, 128, 256, 512, 1024, 2048, 4096] {
        let n = u64::from(side) * u64::from(side);
        let ours = n * tile_b as u64;
        let ottd = n * OPENTTD_BYTES_PER_TILE as u64;
        println!(
            "{side:>5}²  {n:>12}  {:>9.1} MiB  {:>9.1} MiB",
            ours as f64 / (1024.0 * 1024.0),
            ottd as f64 / (1024.0 * 1024.0),
        );
    }

    println!();
    println!("=== Alloc real (Map::new_flat, touch páginas) — max {alloc_max} ===");
    println!(
        "{:>8}  {:>10}  {:>12}  {:>10}  {:>10}",
        "lado", "ms", "estimación", "ΔRSS", "capacity"
    );

    let baseline = rss_kib();
    for side in [64_u32, 128, 256, 512, 1024, 2048, 4096] {
        if side > alloc_max {
            println!("{side:>5}²  (omitido: > --alloc-max {alloc_max})");
            continue;
        }
        let before = rss_kib();
        let t0 = Instant::now();
        let map = Map::new_flat(side, side, 1);
        // Fuerza commit de páginas para que VmRSS refleje el coste real.
        let mut sink = 0_u64;
        for tile in map.tiles() {
            sink = sink.wrapping_add(u64::from(tile.height));
        }
        std::hint::black_box(sink);
        let elapsed = t0.elapsed();
        let after = rss_kib();
        let n = u64::from(side) * u64::from(side);
        let est_mib = (n * tile_b as u64) as f64 / (1024.0 * 1024.0);
        let delta = match (before, after) {
            (Some(b), Some(a)) => format!("{:.1} MiB", (a.saturating_sub(b)) as f64 / 1024.0),
            _ => "n/a".into(),
        };
        let cap = size_of_val(map.tiles());
        println!(
            "{side:>5}²  {:>8.1}  {est_mib:>9.1} MiB  {delta:>10}  {:>7.1} MiB",
            elapsed.as_secs_f64() * 1000.0,
            cap as f64 / (1024.0 * 1024.0),
        );
        drop(map);
    }

    if let Some(base) = baseline {
        println!();
        println!("VmRSS baseline al inicio ≈ {:.1} MiB", base as f64 / 1024.0);
    }
}
