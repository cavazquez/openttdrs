//! Headless core dirty-tile notices, deduplicated within each cause and tick.
//! Compile against Cargo's exact release core artifact. This does not execute
//! client visual callbacks and does not measure FPS or certify client replay.

use std::collections::BTreeSet;
use std::io::{BufWriter, Write};

use openttdrs_core::{GameState, TileCoord, sav};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let path = args
        .next()
        .ok_or("usage: tile-dirty-causes SAVE.sav [TICKS]")?;
    let ticks: usize = args.next().map_or(Ok(200), |value| value.parse())?;
    if args.next().is_some() {
        return Err("unexpected argument".into());
    }
    let raw = std::fs::read(path)?;
    let mut state = GameState::from_sav_game(sav::load(&raw)?);
    for entry in &state.newgrf_stack {
        eprintln!(
            "grf filename={} id={:08x} enabled={} static={} params={:?}",
            entry.filename, entry.grfid, entry.enabled, entry.is_static, entry.params
        );
    }
    let mut output = BufWriter::new(std::io::stdout().lock());
    writeln!(output, "tick,cause,x,y,kind,m3hi,m5")?;
    for _ in 0..ticks {
        while !state.cargo_routing_ready_to_step() {
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        state.step();
        let tick = state.tick.get();
        let causes: [(&str, &[TileCoord]); 5] = [
            ("industry", &state.runtime.industry_tile_dirty),
            (
                "airport_animation",
                &state.runtime.airport_tile_animation_dirty,
            ),
            ("landscape", &state.runtime.landscape_tile_dirty),
            ("signal", &state.runtime.signal_tile_dirty),
            ("reservation", &state.runtime.reservation_tile_dirty),
        ];
        for (cause, coords) in causes {
            let unique: BTreeSet<_> = coords.iter().copied().collect();
            for coord in unique {
                if let Some(tile) = state.map.get(coord) {
                    writeln!(
                        output,
                        "{tick},{cause},{},{},{:?},{},{}",
                        coord.x, coord.y, tile.kind, tile.m3hi, tile.m5
                    )?;
                }
            }
        }
    }
    output.flush()?;
    Ok(())
}
