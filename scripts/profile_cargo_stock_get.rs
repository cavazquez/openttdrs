//! Read-only CargoStock lookup probe, with imported station stocks and all 64 slots.
//! Compile with rustc -O against the exact release library reported by Cargo JSON.
//! Load/hydration and output are outside the timer; this is not a tick benchmark.

use std::hint::black_box;
use std::time::Instant;

use openttdrs_core::cargo::{ALL_CARGO_TYPES, CUSTOM_CARGO_COUNT, CargoStock, CargoType};
use openttdrs_core::{GameState, apply_newgrf_stack_catalogs_default_dirs, sav};

#[inline(never)]
fn read_stocks(stocks: &[CargoStock], cargos: &[CargoType], passes: usize) -> u64 {
    let mut checksum = 0_u64;
    for _ in 0..passes {
        for stock in stocks {
            for &cargo in cargos {
                checksum += u64::from(black_box(stock).get(black_box(cargo)));
            }
        }
    }
    black_box(checksum)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let path = args
        .next()
        .ok_or("usage: cargo-stock-get SAVE.sav [PASSES]")?;
    let passes: usize = args.next().map_or(Ok(512), |value| value.parse())?;
    let loaded = sav::load(&std::fs::read(path)?).map_err(|error| format!("SAV: {error}"))?;
    let mut state = GameState::from_sav_game(loaded);
    apply_newgrf_stack_catalogs_default_dirs(&mut state);
    let stocks: Vec<_> = state
        .stations
        .iter()
        .map(|station| station.cargo_stock)
        .collect();
    let cargos: Vec<_> = ALL_CARGO_TYPES
        .into_iter()
        .chain((0..CUSTOM_CARGO_COUNT).map(openttdrs_core::cargo::custom_cargo))
        .collect();
    let reads = stocks.len() * cargos.len() * passes;
    eprintln!(
        "tick={} vehicles={} CargoStock_bytes={} stations={} cargos={}",
        state.tick.get(),
        state.vehicles.len(),
        std::mem::size_of::<CargoStock>(),
        stocks.len(),
        cargos.len()
    );
    let checksum = read_stocks(&stocks, &cargos, passes);
    println!("sample,stations,cargos,passes,reads,ms,checksum");
    for sample in 1..=5 {
        let start = Instant::now();
        let result = read_stocks(&stocks, &cargos, passes);
        let ms = start.elapsed().as_secs_f64() * 1_000.0;
        assert_eq!(result, checksum);
        println!(
            "{sample},{},{},{passes},{reads},{ms:.6},{result}",
            stocks.len(),
            cargos.len()
        );
    }
    Ok(())
}
