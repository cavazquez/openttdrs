//! Compara dos trazas de paridad JSONL y reporta la primera divergencia.
//!
//! Uso:
//!
//! ```text
//! cargo run -p openttdrs-core --bin parity_diff -- esperada.jsonl actual.jsonl \
//!     [--vehicle 1] [--subsystem movement|speed|orders|cargo|events]
//! ```
//!
//! Código de salida: 0 sin divergencias, 1 con divergencias, 2 error de uso.

use std::io::BufReader;
use std::path::PathBuf;
use std::process::ExitCode;

use openttdrs_core::parity::{self, DiffFilter, Subsystem};

struct Args {
    expected: PathBuf,
    actual: PathBuf,
    filter: DiffFilter,
}

fn print_usage() {
    eprintln!(
        "uso: parity_diff <esperada.jsonl> <actual.jsonl> [--vehicle N] [--subsystem movement|speed|orders|cargo|events]"
    );
}

fn parse_args() -> Result<Args, String> {
    let mut positional: Vec<PathBuf> = Vec::new();
    let mut filter = DiffFilter::default();

    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        let mut value = |name: &str| it.next().ok_or_else(|| format!("falta el valor de {name}"));
        match arg.as_str() {
            "--vehicle" => {
                filter.vehicle = Some(
                    value("--vehicle")?
                        .parse()
                        .map_err(|e| format!("--vehicle inválido: {e}"))?,
                );
            }
            "--subsystem" => {
                let raw = value("--subsystem")?;
                filter.subsystem = Some(
                    Subsystem::parse(&raw).ok_or_else(|| format!("--subsystem inválido: {raw}"))?,
                );
            }
            "--help" | "-h" => {
                print_usage();
                std::process::exit(0);
            }
            other => positional.push(PathBuf::from(other)),
        }
    }
    if positional.len() != 2 {
        return Err(format!(
            "se esperan 2 trazas JSONL, recibidas {}",
            positional.len()
        ));
    }
    let actual = positional.pop().unwrap_or_default();
    let expected = positional.pop().unwrap_or_default();
    Ok(Args {
        expected,
        actual,
        filter,
    })
}

fn read_trace(path: &PathBuf) -> Result<Vec<parity::TickRecord>, String> {
    let file = std::fs::File::open(path)
        .map_err(|e| format!("no se pudo abrir {}: {e}", path.display()))?;
    parity::read_jsonl(BufReader::new(file))
        .map_err(|e| format!("traza inválida {}: {e}", path.display()))
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

    let expected = match read_trace(&args.expected) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(2);
        }
    };
    let actual = match read_trace(&args.actual) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(2);
        }
    };

    println!(
        "esperada: {} ({} ticks) | actual: {} ({} ticks)",
        args.expected.display(),
        expected.len(),
        args.actual.display(),
        actual.len()
    );
    let report = parity::compare_traces(&expected, &actual, args.filter);
    print!("{}", parity::render_report(&report));

    if report.has_divergence() {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}
