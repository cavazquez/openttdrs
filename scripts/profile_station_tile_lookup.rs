//! Legacy station ownership lookup on every Station tile in an imported SAV.
//! Compile against Cargo's exact release library artifact. Setup, hydration,
//! index construction, differential checks and output are outside sample timers.

use std::hint::black_box;
use std::time::Instant;

use openttdrs_core::{
    GameState, Map, Station, TerminalSpatialIndex, TileCoord, TileKind,
    apply_newgrf_stack_catalogs_default_dirs, sav, station_at_tile, station_at_tile_indexed,
};

#[inline(never)]
fn query_tiles(
    map: &Map,
    stations: &[Station],
    index: &TerminalSpatialIndex,
    tiles: &[TileCoord],
    passes: usize,
    indexed: bool,
) -> u64 {
    let mut checksum = 0xcbf2_9ce4_8422_2325_u64;
    for _ in 0..passes {
        for &tile in tiles {
            let station = if indexed {
                station_at_tile_indexed(black_box(map), black_box(stations), black_box(tile), index)
            } else {
                station_at_tile(black_box(map), black_box(stations), black_box(tile))
            };
            checksum = checksum.wrapping_mul(0x100_0000_01b3) ^ u64::from(station.is_some());
            if let Some(station) = station {
                for byte in station
                    .pos
                    .x
                    .to_le_bytes()
                    .into_iter()
                    .chain(station.pos.y.to_le_bytes())
                    .chain(station.ottd_station_id.unwrap_or(u32::MAX).to_le_bytes())
                {
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
        .ok_or("usage: station-tile-lookup SAVE.sav legacy|indexed [PASSES]")?;
    let mode = args.next().ok_or("missing lookup mode")?;
    let indexed = match mode.as_str() {
        "legacy" => false,
        "indexed" => true,
        _ => return Err("mode must be legacy or indexed".into()),
    };
    let passes: usize = args.next().map_or(Ok(4), |value| value.parse())?;
    let loaded = sav::load(&std::fs::read(path)?).map_err(|error| format!("SAV: {error}"))?;
    let mut state = GameState::from_sav_game(loaded);
    apply_newgrf_stack_catalogs_default_dirs(&mut state);
    let mut tiles = Vec::new();
    let (width, height) = state.map.dimensions();
    for y in 0..height {
        for x in 0..width {
            let tile = TileCoord::new(i32::try_from(x)?, i32::try_from(y)?);
            if state.map.get_kind(tile) == Some(TileKind::Station) {
                tiles.push(tile);
            }
        }
    }
    let mut index = TerminalSpatialIndex::default();
    let start = Instant::now();
    index.ensure_current(&state.map, &state.stations);
    let build_ms = start.elapsed().as_secs_f64() * 1_000.0;
    for &tile in &tiles {
        let expected = station_at_tile(&state.map, &state.stations, tile);
        let actual = station_at_tile_indexed(&state.map, &state.stations, tile, &index);
        assert!(match (expected, actual) {
            (None, None) => true,
            (Some(left), Some(right)) => std::ptr::eq(left, right),
            _ => false,
        });
    }
    let mut clone_total_ms = 0.0;
    for _ in 0..20 {
        let start = Instant::now();
        let mirror = black_box(index.clone());
        clone_total_ms += start.elapsed().as_secs_f64() * 1_000.0;
        drop(mirror);
    }
    eprintln!(
        "tick={} vehicles={} stations={} station_tiles={} index_build_ms={build_ms:.6} index_clone_mean_ms={:.6} exact_station_references=true",
        state.tick.get(),
        state.vehicles.len(),
        state.stations.len(),
        tiles.len(),
        clone_total_ms / 20.0,
    );
    let checksum = query_tiles(&state.map, &state.stations, &index, &tiles, passes, indexed);
    println!("sample,mode,stations,tiles,passes,queries,ms,checksum");
    for sample in 1..=5 {
        let start = Instant::now();
        let result = query_tiles(&state.map, &state.stations, &index, &tiles, passes, indexed);
        let ms = start.elapsed().as_secs_f64() * 1_000.0;
        assert_eq!(result, checksum);
        println!(
            "{sample},{mode},{},{},{passes},{},{ms:.6},{result}",
            state.stations.len(),
            tiles.len(),
            tiles.len() * passes,
        );
    }
    Ok(())
}
