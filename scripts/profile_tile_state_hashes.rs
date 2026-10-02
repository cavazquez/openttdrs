//! Headless sampled canonical-state proof; no client callbacks or FPS timings.
use openttdrs_core::{GameState, sav};
use std::io::{BufWriter, Write};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let path = args.next().ok_or("missing SAV path")?;
    let ticks: u64 = args.next().map_or(Ok(200), |s| s.parse())?;
    let raw = std::fs::read(path)?;
    let mut state = GameState::from_sav_game(sav::load(&raw)?);
    let mut output = BufWriter::new(std::io::stdout().lock());
    writeln!(output, "sample,tick,canonical_hash")?;
    writeln!(output, "0,{},{}", state.tick.get(), state.canonical_hash())?;
    for n in 1..=ticks {
        while !state.cargo_routing_ready_to_step() {
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        state.step();
        if n % 20 == 0 || n == ticks {
            writeln!(
                output,
                "{n},{},{}",
                state.tick.get(),
                state.canonical_hash()
            )?;
        }
    }
    output.flush()?;
    Ok(())
}
