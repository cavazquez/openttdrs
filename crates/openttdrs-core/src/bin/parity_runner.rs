//! Corre un escenario de paridad headless y escribe la traza JSONL.
//!
//! Uso:
//!
//! ```text
//! cargo run -p openttdrs-core --bin parity_runner -- \
//!     --scenario truck_bay --ticks 500 --out /tmp/truck_bay.jsonl \
//!     [--divergence-report docs/parity/divergences_found.md]
//! ```

use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use openttdrs_core::parity;

struct Args {
    scenario: String,
    ticks: u64,
    out: PathBuf,
    divergence_report: Option<PathBuf>,
}

fn print_usage() {
    eprintln!(
        "uso: parity_runner --scenario <nombre> [--ticks N] [--out traza.jsonl] [--divergence-report reporte.md]"
    );
    eprintln!("escenarios: {}", parity::scenario_names().join(", "));
}

fn parse_args() -> Result<Args, String> {
    let mut scenario = String::from("truck_bay");
    let mut ticks: u64 = 500;
    let mut out = PathBuf::from("/tmp/parity_trace.jsonl");
    let mut divergence_report = None;

    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        let mut value = |name: &str| it.next().ok_or_else(|| format!("falta el valor de {name}"));
        match arg.as_str() {
            "--scenario" => scenario = value("--scenario")?,
            "--ticks" => {
                ticks = value("--ticks")?
                    .parse()
                    .map_err(|e| format!("--ticks inválido: {e}"))?;
            }
            "--out" => out = PathBuf::from(value("--out")?),
            "--divergence-report" => {
                divergence_report = Some(PathBuf::from(value("--divergence-report")?));
            }
            "--help" | "-h" => {
                print_usage();
                std::process::exit(0);
            }
            other => return Err(format!("argumento desconocido: {other}")),
        }
    }
    Ok(Args {
        scenario,
        ticks,
        out,
        divergence_report,
    })
}

fn run(args: &Args) -> Result<(), String> {
    let Some(mut state) = parity::build_scenario(&args.scenario) else {
        return Err(format!(
            "escenario desconocido: {} (disponibles: {})",
            args.scenario,
            parity::scenario_names().join(", ")
        ));
    };

    state.enable_parity_trace();
    for _ in 0..args.ticks {
        state.step();
    }
    let records = state.take_parity_records();

    let file = std::fs::File::create(&args.out)
        .map_err(|e| format!("no se pudo crear {}: {e}", args.out.display()))?;
    let mut writer = BufWriter::new(file);
    parity::write_jsonl(&records, &mut writer).map_err(|e| format!("error escribiendo: {e}"))?;
    writer
        .flush()
        .map_err(|e| format!("error al volcar la traza: {e}"))?;

    let total_events: usize = records.iter().map(|r| r.events.len()).sum();
    println!(
        "escenario {} — {} ticks, {} vehículos, {} eventos → {}",
        args.scenario,
        records.len(),
        records.last().map_or(0, |r| r.vehicles.len()),
        total_events,
        args.out.display()
    );

    if let Some(report_path) = &args.divergence_report {
        let divergences = parity::report::detect_known_divergences(&records);
        let markdown = parity::report::divergences_markdown(&divergences);
        if let Some(parent) = report_path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("no se pudo crear {}: {e}", parent.display()))?;
        }
        std::fs::write(report_path, markdown)
            .map_err(|e| format!("no se pudo escribir {}: {e}", report_path.display()))?;
        let confirmed = divergences.iter().filter(|d| d.detected).count();
        println!(
            "reporte de divergencias conocidas ({confirmed}/{} confirmadas) → {}",
            divergences.len(),
            report_path.display()
        );
    }
    Ok(())
}

fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(args) => args,
        Err(e) => {
            eprintln!("error: {e}");
            print_usage();
            return ExitCode::from(2);
        }
    };
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
