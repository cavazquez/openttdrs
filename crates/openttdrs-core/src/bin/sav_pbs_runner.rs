//! Ejecuta una traza PBS desde el mismo `.sav` que usa el oráculo OpenTTD.

use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use openttdrs_core::{
    GameState, SavVehicleKind, TileCoord, TileKind, VehicleKind, consist_unit_ids,
    consist_unit_poses, decode_rail_reservation_m2_hi, sav,
};
use serde::Serialize;

struct Args {
    save: PathBuf,
    ticks: u64,
    out: PathBuf,
}

#[derive(Serialize)]
struct PbsUnit {
    index: usize,
    x: i32,
    y: i32,
    rail_pixel: u8,
    direction: u8,
}

#[derive(Serialize)]
struct PbsTrain {
    vehicle: u32,
    x: i32,
    y: i32,
    progress: u8,
    speed: u16,
    subspeed: u8,
    direction: u8,
    units: Vec<PbsUnit>,
}

#[derive(Serialize)]
struct PbsRoadVehicle {
    vehicle: u32,
    x: i32,
    y: i32,
    progress: u8,
    speed: u16,
    subspeed: u8,
    direction: u8,
    state: u8,
    frame: u8,
    blocked_ctr: u16,
    overtaking: u8,
    overtaking_ctr: u8,
    crashed_ctr: u16,
    reverse_ctr: u8,
}

#[derive(Serialize)]
struct PbsReservation {
    x: i32,
    y: i32,
    track_bits: u8,
}

#[derive(Serialize)]
struct PbsTraceRow {
    kind: &'static str,
    tick: u64,
    trains: Vec<PbsTrain>,
    road_vehicles: Vec<PbsRoadVehicle>,
    rail_reservations: Vec<PbsReservation>,
}

fn print_usage() {
    eprintln!("uso: sav_pbs_runner <partida.sav> [--ticks N] [--out traza.jsonl]");
}

fn parse_args() -> Result<Args, String> {
    let mut save = None;
    let mut ticks = 40;
    let mut out = PathBuf::from("/tmp/openttdrs-pbs-from-sav.jsonl");
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        let mut value = |name: &str| it.next().ok_or_else(|| format!("falta el valor de {name}"));
        match arg.as_str() {
            "--ticks" => {
                ticks = value("--ticks")?
                    .parse()
                    .map_err(|e| format!("--ticks inválido: {e}"))?;
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
    if ticks == 0 {
        return Err("--ticks debe ser positivo".to_string());
    }
    Ok(Args { save, ticks, out })
}

fn units_for_head(state: &GameState, head_id: u32) -> Vec<PbsUnit> {
    let ids = consist_unit_ids(&state.vehicles, head_id);
    let poses = consist_unit_poses(&state.vehicles, head_id);
    ids.into_iter()
        .zip(poses)
        .enumerate()
        .map(|(index, (_id, pose))| PbsUnit {
            index,
            x: pose.tile.x,
            y: pose.tile.y,
            rail_pixel: pose.rail_pixel,
            direction: pose.direction,
        })
        .collect()
}

fn trace_row(state: &GameState, kind: &'static str) -> PbsTraceRow {
    let trains = state
        .vehicles
        .iter()
        .filter(|vehicle| vehicle.kind == VehicleKind::Train && vehicle.is_consist_head())
        .map(|vehicle| PbsTrain {
            vehicle: vehicle.id,
            x: vehicle.pos.x,
            y: vehicle.pos.y,
            progress: vehicle.progress,
            speed: vehicle.cur_speed,
            subspeed: vehicle.subspeed,
            direction: vehicle.direction,
            units: units_for_head(state, vehicle.id),
        })
        .collect();
    let road_vehicles = state
        .vehicles
        .iter()
        .filter(|vehicle| {
            matches!(
                vehicle.kind,
                VehicleKind::Bus | VehicleKind::Truck | VehicleKind::Tram
            )
        })
        .map(|vehicle| PbsRoadVehicle {
            vehicle: vehicle.id,
            x: vehicle.pos.x,
            y: vehicle.pos.y,
            progress: vehicle.progress,
            speed: vehicle.cur_speed,
            subspeed: vehicle.subspeed,
            direction: vehicle.direction,
            state: vehicle.road_state,
            frame: vehicle.frame,
            blocked_ctr: vehicle.blocked_ctr,
            overtaking: vehicle.overtaking,
            overtaking_ctr: vehicle.overtaking_ctr,
            crashed_ctr: vehicle.crashed_ctr,
            reverse_ctr: vehicle.reverse_ctr,
        })
        .collect();
    let (width, height) = state.map.dimensions();
    let mut rail_reservations = Vec::new();
    for y in 0..height as i32 {
        for x in 0..width as i32 {
            let tile = TileCoord::new(x, y);
            let Some(data) = state.map.get(tile) else {
                continue;
            };
            if data.kind != TileKind::Rail {
                continue;
            }
            let track_bits = decode_rail_reservation_m2_hi(data.m2_hi);
            if track_bits != 0 {
                rail_reservations.push(PbsReservation { x, y, track_bits });
            }
        }
    }
    PbsTraceRow {
        kind,
        tick: state.tick.get(),
        trains,
        road_vehicles,
        rail_reservations,
    }
}

fn write_row(writer: &mut BufWriter<std::fs::File>, row: &impl Serialize) -> Result<(), String> {
    serde_json::to_writer(&mut *writer, row).map_err(|e| format!("error serializando: {e}"))?;
    writer
        .write_all(b"\n")
        .map_err(|e| format!("error escribiendo: {e}"))
}

fn run(args: &Args) -> Result<(), String> {
    let raw = std::fs::read(&args.save)
        .map_err(|e| format!("no se pudo leer {}: {e}", args.save.display()))?;
    let sav = sav::load(&raw).map_err(|e| format!("save inválido {}: {e}", args.save.display()))?;
    let imported_order_count: usize = sav
        .vehicles
        .iter()
        .map(|vehicle| vehicle.orders.len())
        .sum();
    let raw_orders: Vec<_> = sav
        .vehicles
        .iter()
        .flat_map(|vehicle| vehicle.orders.iter())
        .map(|order| (order.order_type, order.dest))
        .collect();
    let raw_motion: Vec<_> = sav
        .vehicles
        .iter()
        .filter(|vehicle| vehicle.kind == SavVehicleKind::Train)
        .map(|vehicle| {
            (
                vehicle.progress,
                vehicle.cur_speed,
                vehicle.subspeed,
                vehicle.direction,
                vehicle.engine_type,
            )
        })
        .collect();
    let mut state = GameState::from_sav_game(sav);
    let station_count = state.stations.len();
    let train_count = state
        .vehicles
        .iter()
        .filter(|vehicle| matches!(vehicle.kind, openttdrs_core::VehicleKind::Train))
        .count();
    if train_count == 0 {
        return Err("el save no contiene trenes importables".to_string());
    }
    let file = std::fs::File::create(&args.out)
        .map_err(|e| format!("no se pudo crear {}: {e}", args.out.display()))?;
    let mut writer = BufWriter::new(file);
    write_row(
        &mut writer,
        &serde_json::json!({
            "kind": "metadata",
            "schema_version": 2,
            "trace": "pbs_and_road_vehicle_dynamics",
            "producer": "openttdrs",
            "source_path": args.save,
            "initial_sample_point": "after_sav_import",
            "tick_sample_point": "after_game_state_step",
            "max_ticks": args.ticks,
            "roadveh_acceleration_model": state.road_vehicle_acceleration_model as u8,
        }),
    )?;
    write_row(&mut writer, &trace_row(&state, "initial"))?;
    for _ in 0..args.ticks {
        state.step();
        write_row(&mut writer, &trace_row(&state, "tick"))?;
    }
    writer
        .flush()
        .map_err(|e| format!("no se pudo volcar {}: {e}", args.out.display()))?;
    println!(
        "{} — {} ticks, {train_count} tren(es), {station_count} estación(es), {imported_order_count} orden(es) importada(s) → {}",
        args.save.display(),
        args.ticks,
        args.out.display()
    );
    if !raw_orders.is_empty() {
        println!("órdenes SAV type/dest: {raw_orders:?}");
    }
    println!("movimiento SAV progress/speed/subspeed/direction/engine: {raw_motion:?}");
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
