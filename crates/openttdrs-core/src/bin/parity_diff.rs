//! Compara dos trazas de paridad JSONL y reporta la primera divergencia.
//!
//! Uso:
//!
//! ```text
//! cargo run -p openttdrs-core --bin parity_diff -- esperada.jsonl actual.jsonl \
//!     [--vehicle 1] [--subsystem movement|speed|orders|cargo|events|\
//!      rail_infrastructure|train_motion|consist_geometry|pathfinding|\
//!      station_entry|loading|signaling|reservation|depot] \
//!     [--tile x,y] [--event tipo_snake_case] [--subtile-epsilon 0.51] \
//!     [--json reporte.json]
//! ```
//!
//! Código de salida: 0 sin divergencias, 1 con divergencias, 2 error de uso.

use std::io::BufReader;
use std::path::PathBuf;
use std::process::ExitCode;

use openttdrs_core::TileCoord;
use openttdrs_core::parity::{self, DiffFilter, DiffReport, Subsystem};

struct Args {
    expected: PathBuf,
    actual: PathBuf,
    filter: DiffFilter,
    json_out: Option<PathBuf>,
}

fn print_usage() {
    eprintln!(
        "uso: parity_diff <esperada.jsonl> <actual.jsonl> [--vehicle N] [--subsystem S] \
         [--tile x,y] [--event tipo] [--subtile-epsilon F] [--json reporte.json]"
    );
    eprintln!(
        "subsistemas: movement, speed, orders, cargo, events, rail_infrastructure, \
         train_motion, consist_geometry, pathfinding, station_entry, loading, \
         signaling, reservation, depot"
    );
}

fn parse_tile(raw: &str) -> Result<TileCoord, String> {
    let (x, y) = raw
        .split_once(',')
        .ok_or_else(|| format!("--tile inválido (se espera x,y): {raw}"))?;
    Ok(TileCoord::new(
        x.trim()
            .parse()
            .map_err(|e| format!("--tile x inválido: {e}"))?,
        y.trim()
            .parse()
            .map_err(|e| format!("--tile y inválido: {e}"))?,
    ))
}

fn parse_args() -> Result<Args, String> {
    let mut positional: Vec<PathBuf> = Vec::new();
    let mut filter = DiffFilter::default();
    let mut json_out = None;

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
            "--tile" => filter.tile = Some(parse_tile(&value("--tile")?)?),
            "--event" => filter.event = Some(value("--event")?),
            "--subtile-epsilon" => {
                filter.subtile_epsilon = value("--subtile-epsilon")?
                    .parse()
                    .map_err(|e| format!("--subtile-epsilon inválido: {e}"))?;
            }
            "--json" => json_out = Some(PathBuf::from(value("--json")?)),
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
        json_out,
    })
}

fn read_trace(path: &PathBuf) -> Result<Vec<parity::TickRecord>, String> {
    let file = std::fs::File::open(path)
        .map_err(|e| format!("no se pudo abrir {}: {e}", path.display()))?;
    parity::read_jsonl(BufReader::new(file))
        .map_err(|e| format!("traza inválida {}: {e}", path.display()))
}

fn report_json(report: &DiffReport) -> serde_json::Value {
    let first_divergence = report.first.as_ref().map(|d| {
        serde_json::json!({
            "tick": d.tick,
            "vehicle": d.vehicle,
            "subsystem": d.subsystem.label(),
            "field": d.field,
            "a": d.expected,
            "b": d.actual,
        })
    });
    let by_subsystem: serde_json::Map<String, serde_json::Value> = report
        .by_subsystem
        .iter()
        .map(|(k, v)| ((*k).to_string(), serde_json::json!(v)))
        .collect();
    serde_json::json!({
        "first_divergence": first_divergence,
        "by_subsystem": by_subsystem,
        "total": report.total,
        "notes": report.notes,
    })
}

fn write_json_report(report: &DiffReport, path: &PathBuf) -> Result<(), String> {
    let value = report_json(report);
    let pretty = serde_json::to_string_pretty(&value)
        .map_err(|e| format!("no se pudo serializar el reporte: {e}"))?;
    std::fs::write(path, pretty + "\n")
        .map_err(|e| format!("no se pudo escribir {}: {e}", path.display()))
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
    let report = parity::compare_traces(&expected, &actual, &args.filter);
    print!("{}", parity::render_report(&report));

    if let Some(path) = &args.json_out {
        if let Err(e) = write_json_report(&report, path) {
            eprintln!("error: {e}");
            return ExitCode::from(2);
        }
        println!("reporte JSON → {}", path.display());
    }

    if report.has_divergence() {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}
