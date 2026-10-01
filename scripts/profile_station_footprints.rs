//! Ordered station footprint traversal on the real imported rail anchors.
//! Compile with rustc -O against Cargo's exact release library artifact.
//! Import/hydration and output are outside the timer; this is not a tick probe.

use std::hint::black_box;
use std::time::Instant;

use openttdrs_core::{
    GameState, Map, StopKind, TileCoord, apply_newgrf_stack_catalogs_default_dirs, sav,
    station_footprint_tiles,
};

#[inline(never)]
fn walk_footprints(map: &Map, anchors: &[TileCoord], passes: usize) -> u64 {
    let mut checksum = 0xcbf2_9ce4_8422_2325_u64;
    for _ in 0..passes {
        for &anchor in anchors {
            let tiles = station_footprint_tiles(black_box(map), black_box(anchor));
            checksum = checksum.wrapping_mul(0x100_0000_01b3) ^ tiles.len() as u64;
            for tile in tiles {
                for byte in tile.x.to_le_bytes().into_iter().chain(tile.y.to_le_bytes()) {
                    checksum = checksum.wrapping_mul(0x100_0000_01b3) ^ u64::from(byte);
                }
            }
        }
    }
    black_box(checksum)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let path = args
        .next()
        .ok_or("usage: station-footprints SAVE.sav [PASSES]")?;
    let passes: usize = args.next().map_or(Ok(128), |value| value.parse())?;
    let loaded = sav::load(&std::fs::read(path)?).map_err(|error| format!("SAV: {error}"))?;
    let mut state = GameState::from_sav_game(loaded);
    apply_newgrf_stack_catalogs_default_dirs(&mut state);
    let anchors: Vec<_> = state
        .stations
        .iter()
        .filter(|station| {
            matches!(
                station.stop_kind,
                StopKind::RailStation | StopKind::RailWaypoint
            )
        })
        .map(|station| station.pos)
        .collect();
    let tiles: usize = anchors
        .iter()
        .map(|&anchor| station_footprint_tiles(&state.map, anchor).len())
        .sum();
    eprintln!(
        "tick={} vehicles={} stations={} rail_anchors={} footprint_tiles={tiles}",
        state.tick.get(),
        state.vehicles.len(),
        state.stations.len(),
        anchors.len()
    );
    let checksum = walk_footprints(&state.map, &anchors, passes);
    println!("sample,anchors,passes,queries,tiles_per_pass,ms,checksum");
    for sample in 1..=5 {
        let start = Instant::now();
        let result = walk_footprints(&state.map, &anchors, passes);
        let ms = start.elapsed().as_secs_f64() * 1_000.0;
        assert_eq!(result, checksum);
        println!(
            "{sample},{},{passes},{},{tiles},{ms:.6},{result}",
            anchors.len(),
            anchors.len() * passes
        );
    }
    Ok(())
}
