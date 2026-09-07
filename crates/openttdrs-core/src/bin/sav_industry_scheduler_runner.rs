//! Emite la traza diaria de industrias desde un `.sav` importado por Rust.

use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use openttdrs_core::{
    DAY_TICKS, GameState, INDUSTRY_BUILD_TYPE_COUNT, IndustrySchedulerTraceSample, sav,
};
use serde::Serialize;

struct Args {
    save: PathBuf,
    days: u64,
    out: PathBuf,
}

#[derive(Serialize)]
struct Metadata<'a> {
    kind: &'static str,
    schema_version: u8,
    producer: &'static str,
    trace: &'static str,
    source_path: &'a str,
    initial_sample_point: &'static str,
    day_sample_point: &'static str,
    max_days: u64,
    industry_type_count: usize,
}

#[derive(Serialize)]
struct TraceRow<'a> {
    kind: &'static str,
    #[serde(flatten)]
    sample: &'a IndustrySchedulerTraceSample,
}

fn print_usage() {
    eprintln!("uso: sav_industry_scheduler_runner <partida.sav> [--days N] [--out traza.jsonl]");
}

fn parse_args() -> Result<Args, String> {
    let mut save = None;
    let mut days = 40;
    let mut out = PathBuf::from("/tmp/openttdrs-industry-scheduler-from-sav.jsonl");
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        let mut value = |name: &str| it.next().ok_or_else(|| format!("falta el valor de {name}"));
        match arg.as_str() {
            "--days" => {
                days = value("--days")?
                    .parse()
                    .map_err(|e| format!("--days inválido: {e}"))?;
            }
            "--out" => out = PathBuf::from(value("--out")?),
            "--help" | "-h" => {
                print_usage();
                std::process::exit(0);
            }
            path if save.is_none() => save = Some(PathBuf::from(path)),
            other => return Err(format!("argumento desconocido: {other}")),
        }
    }
    let save = save.ok_or_else(|| "falta <partida.sav>".to_string())?;
    if days == 0 {
        return Err("--days debe ser positivo".to_string());
    }
    Ok(Args { save, days, out })
}

fn write_row(writer: &mut BufWriter<std::fs::File>, row: &impl Serialize) -> Result<(), String> {
    serde_json::to_writer(&mut *writer, row).map_err(|e| format!("error serializando: {e}"))?;
    writer
        .write_all(b"\n")
        .map_err(|e| format!("error escribiendo: {e}"))
}

/// Avanza hasta el próximo corte post-timer, no hasta el final de una jornada
/// arbitraria. El scheduler captura la muestra dentro de `on_tick_industry`,
/// antes de que las fases posteriores puedan cambiar el estado observado.
fn next_daily_sample(state: &mut GameState) -> Result<IndustrySchedulerTraceSample, String> {
    for _ in 0..usize::from(DAY_TICKS) {
        state.step();
        let mut samples = state.take_industry_scheduler_trace_samples();
        match samples.len() {
            0 => continue,
            1 => {
                return samples
                    .pop()
                    .ok_or_else(|| "muestra diaria ausente".to_string());
            }
            count => return Err(format!("se capturaron {count} muestras en un solo tick")),
        }
    }
    Err(format!(
        "no se alcanzó un borde diario tras {DAY_TICKS} ticks"
    ))
}

fn run(args: &Args) -> Result<(), String> {
    let raw = std::fs::read(&args.save)
        .map_err(|e| format!("no se pudo leer {}: {e}", args.save.display()))?;
    let sav = sav::load(&raw).map_err(|e| format!("save inválido {}: {e}", args.save.display()))?;
    let mut state = GameState::from_sav_game(sav);
    let source_path = args.save.to_string_lossy();
    let initial = IndustrySchedulerTraceSample::from_state(&state, 0, Vec::new());
    state.enable_industry_scheduler_trace();

    let file = std::fs::File::create(&args.out)
        .map_err(|e| format!("no se pudo crear {}: {e}", args.out.display()))?;
    let mut writer = BufWriter::new(file);
    write_row(
        &mut writer,
        &Metadata {
            kind: "metadata",
            schema_version: 1,
            producer: "openttdrs",
            trace: "industry_scheduler",
            source_path: &source_path,
            initial_sample_point: "after_load_game",
            day_sample_point: "after_industry_daily_timer",
            max_days: args.days,
            industry_type_count: INDUSTRY_BUILD_TYPE_COUNT,
        },
    )?;
    write_row(
        &mut writer,
        &TraceRow {
            kind: "initial",
            sample: &initial,
        },
    )?;
    for _ in 0..args.days {
        let sample = next_daily_sample(&mut state)?;
        write_row(
            &mut writer,
            &TraceRow {
                kind: "day",
                sample: &sample,
            },
        )?;
    }
    writer
        .flush()
        .map_err(|e| format!("no se pudo volcar {}: {e}", args.out.display()))?;
    println!(
        "{} — {} día(s), {} industria(s) inicial(es) → {}",
        args.save.display(),
        args.days,
        initial.industries.len(),
        args.out.display()
    );
    Ok(())
}

fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(args) => args,
        Err(error) => {
            eprintln!("error: {error}");
            print_usage();
            return ExitCode::from(2);
        }
    };
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}
