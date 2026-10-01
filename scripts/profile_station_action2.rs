//! Complete station Action2 contexts on a real imported SAV, against immutable
//! before/after Cargo library artifacts. Setup, canonicalization, checksums,
//! trace output and context destruction are outside the sample timer.

use std::collections::HashMap;
use std::fmt::{Debug, Write as _};
use std::hint::black_box;
use std::io::Write as _;
use std::time::Instant;

use openttdrs_core::{
    Action2EvalCtx, GameState, GrfTypeTranslationTables, StationAction2WorldContext, TileCoord,
    TileKind, action2_eval_ctx_for_station_tile_with_catalog,
    action2_eval_ctx_for_station_tile_with_catalog_and_world,
    apply_newgrf_stack_catalogs_default_dirs, sav, station_at_tile, station_spec_def,
};

struct Query<'a> {
    coord: TileCoord,
    grf_version: u8,
    world: bool,
    type_tables: Option<&'a GrfTypeTranslationTables>,
}

fn sorted<K: Ord + Debug>(map: HashMap<K, u32>) -> Vec<(K, u32)> {
    let mut rows: Vec<_> = map.into_iter().collect();
    rows.sort_by(|left, right| left.0.cmp(&right.0));
    rows
}

fn canonical_context(ctx: Action2EvalCtx) -> String {
    // No '..': adding a context field requires extending this complete oracle.
    let Action2EvalCtx {
        vars,
        parameterized_vars,
        parent_vars,
        parent_parameterized_vars,
        random_bits,
        parent_random_bits,
        vehicle_palette_generation,
        parent_vehicle_palette_generation,
        consist_random_bits,
        relative_random_bits,
        relative_same_engine_random_bits,
        relative_vars,
        relative_parameterized_vars,
        temp_registers,
        registers_100,
        persistent_registers,
        persistent_storage_available,
        parent_persistent_registers,
        parent_persistent_storage_available,
        last_result,
        grf_params,
        vehicle_loading,
        vehicle_cargo,
        vehicle_capacity,
    } = ctx;
    let mut text = String::new();
    macro_rules! maps {
        ($($field:ident),+ $(,)?) => {$(
            write!(&mut text, "{}={:?};", stringify!($field), sorted($field)).unwrap();
        )+};
    }
    macro_rules! fields {
        ($($field:ident),+ $(,)?) => {$(
            write!(&mut text, "{}={:?};", stringify!($field), $field).unwrap();
        )+};
    }
    maps!(
        vars,
        parameterized_vars,
        parent_vars,
        parent_parameterized_vars,
        consist_random_bits,
        relative_random_bits,
        relative_same_engine_random_bits,
        relative_vars,
        relative_parameterized_vars,
        temp_registers,
        registers_100,
        persistent_registers,
        parent_persistent_registers,
    );
    fields!(
        random_bits,
        parent_random_bits,
        vehicle_palette_generation,
        parent_vehicle_palette_generation,
        persistent_storage_available,
        parent_persistent_storage_available,
        last_result,
        grf_params,
        vehicle_loading,
        vehicle_cargo,
        vehicle_capacity,
    );
    text
}

#[inline(never)]
fn build_contexts(state: &GameState, queries: &[Query<'_>], passes: usize) -> Vec<Action2EvalCtx> {
    let mut contexts = Vec::with_capacity(queries.len() * passes);
    for _ in 0..passes {
        for query in queries {
            let ctx = if query.world {
                action2_eval_ctx_for_station_tile_with_catalog_and_world(
                    black_box(&state.map),
                    black_box(&state.stations),
                    &state.station_spec_catalog,
                    query.coord,
                    0,
                    state.climate,
                    query.type_tables,
                    query.grf_version,
                    StationAction2WorldContext {
                        towns: &state.towns,
                        companies: &state.companies,
                        industries: &state.industries,
                        cargo_spec_catalog: &state.cargo_spec_catalog,
                    },
                )
            } else {
                action2_eval_ctx_for_station_tile_with_catalog(
                    black_box(&state.map),
                    black_box(&state.stations),
                    &state.station_spec_catalog,
                    query.coord,
                    0,
                    state.climate,
                    query.type_tables,
                    query.grf_version,
                )
            };
            contexts.push(ctx);
        }
    }
    black_box(contexts)
}

fn checksum(contexts: Vec<Action2EvalCtx>) -> u64 {
    let mut value = 0xcbf2_9ce4_8422_2325_u64;
    for ctx in contexts {
        for byte in canonical_context(ctx).bytes() {
            value = value.wrapping_mul(0x100_0000_01b3) ^ u64::from(byte);
        }
    }
    value
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let path = args
        .next()
        .ok_or("usage: station-action2 SAVE.sav [PASSES] [TRACE.txt]")?;
    let passes: usize = args.next().map_or(Ok(1), |value| value.parse())?;
    let trace = args.next();
    let loaded = sav::load(&std::fs::read(path)?).map_err(|error| format!("SAV: {error}"))?;
    let mut state = GameState::from_sav_game(loaded);
    apply_newgrf_stack_catalogs_default_dirs(&mut state);
    let (width, height) = state.map.dimensions();
    let mut queries = Vec::new();
    for y in 0..height {
        for x in 0..width {
            let coord = TileCoord::new(i32::try_from(x)?, i32::try_from(y)?);
            if state.map.get_kind(coord) != Some(TileKind::Station) {
                continue;
            }
            let def = station_at_tile(&state.map, &state.stations, coord).and_then(|station| {
                station_spec_def(&state.station_spec_catalog, station.station_spec)
            });
            for grf_version in [7, 8] {
                for world in [false, true] {
                    queries.push(Query {
                        coord,
                        grf_version,
                        world,
                        type_tables: def.and_then(|def| def.newgrf_type_tables.as_ref()),
                    });
                }
            }
        }
    }
    eprintln!(
        "tick={} vehicles={} stations={} tiles={} queries={} climate={:?}",
        state.tick.get(),
        state.vehicles.len(),
        state.stations.len(),
        queries.len() / 4,
        queries.len(),
        state.climate,
    );
    if let Some(path) = trace {
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)?;
        let mut output = std::io::BufWriter::new(file);
        for (query, ctx) in queries.iter().zip(build_contexts(&state, &queries, 1)) {
            writeln!(
                output,
                "{},{}\tgrf={}\tworld={}\t{}",
                query.coord.x,
                query.coord.y,
                query.grf_version,
                query.world,
                canonical_context(ctx),
            )?;
        }
        output.flush()?;
    }
    let expected = checksum(build_contexts(&state, &queries, passes));
    println!("sample,tiles,passes,contexts,ms,checksum");
    for sample in 1..=5 {
        let start = Instant::now();
        let contexts = build_contexts(&state, &queries, passes);
        let ms = start.elapsed().as_secs_f64() * 1_000.0;
        let value = checksum(contexts);
        assert_eq!(value, expected);
        println!(
            "{sample},{},{passes},{},{ms:.6},{value}",
            queries.len() / 4,
            queries.len() * passes,
        );
    }
    Ok(())
}
