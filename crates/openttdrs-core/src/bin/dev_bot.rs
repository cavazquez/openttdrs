//! Sonda headless: ¿el tren carga, descarga y cuánto gana?
//!
//! ```text
//! cargo run -p openttdrs-core --bin dev_bot -- \
//!     --scenario train_line --vehicle 1 --ticks 12000 \
//!     [--out report.json] [--export-json save/scenario.json] [--require-delivery]
//! ```

use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use openttdrs_core::dev_metrics::{
    CargoProbeOptions, SignalWaitProbeOptions, probe_signal_wait, probe_vehicle_cargo_cycle,
};
use openttdrs_core::parity;
use openttdrs_core::save;

struct Args {
    scenario: String,
    vehicle_id: u32,
    max_ticks: u64,
    out: Option<PathBuf>,
    export_json: Option<PathBuf>,
    require_delivery: bool,
    require_signal_wait: bool,
}

fn print_usage() {
    eprintln!(
        "uso: dev_bot --scenario <nombre> [--vehicle ID] [--ticks N] [--out report.json] [--export-json partida.json] [--require-delivery] [--require-signal-wait]"
    );
    eprintln!("escenarios: {}", parity::scenario_names().join(", "));
}

fn parse_args() -> Result<Args, String> {
    let mut scenario = String::from("train_line");
    let mut vehicle_id: u32 = 1;
    let mut max_ticks: u64 = 12_000;
    let mut out = None;
    let mut export_json = None;
    let mut require_delivery = false;
    let mut require_signal_wait = false;

    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        let mut value = |name: &str| it.next().ok_or_else(|| format!("falta el valor de {name}"));
        match arg.as_str() {
            "--scenario" => scenario = value("--scenario")?,
            "--vehicle" => {
                vehicle_id = value("--vehicle")?
                    .parse()
                    .map_err(|e| format!("--vehicle inválido: {e}"))?;
            }
            "--ticks" => {
                max_ticks = value("--ticks")?
                    .parse()
                    .map_err(|e| format!("--ticks inválido: {e}"))?;
            }
            "--out" => out = Some(PathBuf::from(value("--out")?)),
            "--export-json" => export_json = Some(PathBuf::from(value("--export-json")?)),
            "--require-delivery" => require_delivery = true,
            "--require-signal-wait" => require_signal_wait = true,
            "--help" | "-h" => {
                print_usage();
                std::process::exit(0);
            }
            other => return Err(format!("argumento desconocido: {other}")),
        }
    }
    Ok(Args {
        scenario,
        vehicle_id,
        max_ticks,
        out,
        export_json,
        require_delivery,
        require_signal_wait,
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

    if let Some(path) = &args.export_json {
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        save::save(&state, path).map_err(|e| e.to_string())?;
        eprintln!(
            "partida exportada (tick 0) → {} — cargar en cliente con OTTDJSON_LOAD={}",
            path.display(),
            path.display()
        );
    }

    if args.require_signal_wait {
        if args.scenario != "train_supply" {
            return Err(" --require-signal-wait solo aplica al escenario train_supply".to_string());
        }
        let signal_report = probe_signal_wait(
            &mut state,
            &SignalWaitProbeOptions {
                vehicle_id: args.vehicle_id,
                signal_tile: parity::TRAIN_SUPPLY_WAIT_SIGNAL,
                blocker_id: Some(parity::TRAIN_SUPPLY_BLOCKER_ID),
                blocker_spawn_tile: Some(parity::TRAIN_SUPPLY_BLOCK_TILE),
                max_ticks_until_wait: 500,
                max_ticks_after_release: 300,
            },
        );
        eprintln!(
            "señal {:?} — bloqueador inyectado: {}, esperó: {}, reanudó: {}, ticks espera: {:?}→{:?}, simulados: {}",
            signal_report.signal_tile,
            signal_report.blocker_spawned,
            signal_report.waited,
            signal_report.resumed,
            signal_report.tick_wait_started,
            signal_report.tick_wait_finished,
            signal_report.ticks_run,
        );
        if args.require_signal_wait && (!signal_report.waited || !signal_report.resumed) {
            return Err(format!(
                "sin espera/reanudación en señal (waited={}, resumed={})",
                signal_report.waited, signal_report.resumed
            ));
        }
    } else if args.scenario == "train_supply_signal" {
        eprintln!(
            "escenario train_supply_signal: instantánea visual (tren en señal + bloqueador). \
             Para probar la espera dinámica use --scenario train_supply --require-signal-wait"
        );
    }

    let report = probe_vehicle_cargo_cycle(
        &mut state,
        &CargoProbeOptions {
            vehicle_id: args.vehicle_id,
            max_ticks: args.max_ticks,
        },
    );

    let json = serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?;
    if let Some(path) = &args.out {
        std::fs::write(path, &json).map_err(|e| e.to_string())?;
        eprintln!("informe → {}", path.display());
    } else {
        println!("{json}");
    }

    eprintln!(
        "vehículo {} — cargó: {}, descargó: {}, unidades: {}→{}, ingreso transporte: {}, dinero neto: {}, ticks: {}",
        report.vehicle_id,
        report.loaded,
        report.delivered,
        report.units_loaded_peak,
        report.units_delivered,
        report.delivery_income,
        report.money_net,
        report.ticks_run,
    );

    if args.require_delivery && !report.delivered {
        return Err(format!(
            "sin descarga en {} ticks (loaded={})",
            report.ticks_run, report.loaded
        ));
    }
    Ok(())
}

fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("{e}");
            print_usage();
            return ExitCode::from(2);
        }
    };
    if let Err(e) = run(&args) {
        let _ = writeln!(io::stderr(), "{e}");
        return ExitCode::from(1);
    }
    ExitCode::SUCCESS
}
