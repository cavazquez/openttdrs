//! Ejecuta una traza de aeropuertos/aviones (FTA) desde el mismo `.sav` que
//! usa el oráculo `OpenTTD` (`scripts/export_openttd_airport_fta_trace.sh`).
//!
//! Notas de fidelidad respecto al esquema del oráculo
//! (`{aircraft:[{...}], airports:[{...}]}`):
//! - `vehicle`/`engine`/`x`/`y` (avión): `x`/`y` en `OpenTTD` vienen de
//!   `TileX/TileY(v->tile)`, un campo vestigial que un avión bajo control FTA
//!   nunca actualiza (se congela en su valor de importación, típicamente
//!   `(0,0)`). Lo reproducimos igual: valor crudo del `.sav`, congelado, no
//!   la posición real que trackeamos internamente en `Vehicle::pos`.
//! - `pos`/`previous_pos`/`state`/`targetairport`/`speed`/`direction`/`running`:
//!   estado FTA vivo, tomado de la simulación tick a tick.
//! - `x_pos`/`y_pos`/`z_pos`: posición viva en sub-tesela calculada a partir de
//!   `AirportMovingData`, más la altitud escalada. Al importar una aeronave que
//!   todavía no activó FTA, usamos temporalmente el centro de su tesela.
//! - `airports[].{x,y,w,h,type,layout}`: estáticos, tomados crudos del `.sav`
//!   (no de nuestro `AirportSpecId` interno, que remapea el `type`).
//! - `airports[].blocks`: dinámico, vivo desde `Station::airport_blocks`.

use std::collections::HashMap;
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use openttdrs_core::{GameState, SavVehicleKind, TileCoord, VehicleKind, sav};
use serde::Serialize;

struct Args {
    save: PathBuf,
    ticks: u64,
    out: PathBuf,
}

#[derive(Serialize)]
struct FtaAircraft {
    vehicle: u32,
    engine: u16,
    x: i32,
    y: i32,
    x_pos: i32,
    y_pos: i32,
    z_pos: i32,
    direction: u8,
    pos: u8,
    previous_pos: u8,
    state: u8,
    targetairport: u32,
    speed: u16,
    running: bool,
}

#[derive(Serialize)]
struct FtaAirport {
    station: u32,
    x: i32,
    y: i32,
    w: u16,
    h: u16,
    #[serde(rename = "type")]
    airport_type: u8,
    layout: u8,
    blocks: u64,
}

#[derive(Serialize)]
struct FtaTraceRow {
    kind: &'static str,
    tick: u64,
    aircraft: Vec<FtaAircraft>,
    airports: Vec<FtaAirport>,
}

/// Metadata cruda del `.sav` capturada antes de consumir `SavGame`, para
/// reportar campos que `GameState` no conserva (o remapea) tal cual.
struct FrozenAircraft {
    engine_type: u16,
    /// `TileX/TileY(v->tile)` crudo (vestigial; ver doc del módulo).
    tile: TileCoord,
}

struct FrozenAirport {
    station_id: u32,
    pos: TileCoord,
    w: u16,
    h: u16,
    airport_type: u8,
    layout: u8,
}

fn print_usage() {
    eprintln!("uso: sav_airport_fta_runner <partida.sav> [--ticks N] [--out traza.jsonl]");
}

fn parse_args() -> Result<Args, String> {
    let mut save = None;
    let mut ticks = 80;
    let mut out = PathBuf::from("/tmp/openttdrs-airport-fta-from-sav.jsonl");
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

fn trace_row(
    state: &GameState,
    kind: &'static str,
    frozen_aircraft: &HashMap<u32, FrozenAircraft>,
    frozen_airports: &[FrozenAirport],
    station_pos_to_id: &HashMap<TileCoord, u32>,
) -> FtaTraceRow {
    let aircraft = state
        .vehicles
        .iter()
        .filter(|v| v.kind == VehicleKind::Aircraft)
        .map(|v| {
            let frozen = frozen_aircraft.get(&v.id);
            let target = v.airport_fta_station.unwrap_or(v.dest);
            let targetairport = station_pos_to_id.get(&target).copied().unwrap_or(u32::MAX);
            let (x_pos, y_pos) = if v.airport_subpos_valid {
                (v.airport_sub_x, v.airport_sub_y)
            } else {
                (v.pos.x * 16 + 8, v.pos.y * 16 + 8)
            };
            FtaAircraft {
                vehicle: v.id,
                engine: frozen.map_or(0, |f| f.engine_type),
                x: frozen.map_or(0, |f| f.tile.x),
                y: frozen.map_or(0, |f| f.tile.y),
                x_pos,
                y_pos,
                z_pos: i32::from(v.altitude) * 16,
                direction: v.direction,
                pos: v.airport_pos,
                previous_pos: v.airport_prev_pos,
                state: v.airport_heading.as_u8(),
                targetairport,
                speed: v.cur_speed,
                running: v.running,
            }
        })
        .collect();
    let airports = frozen_airports
        .iter()
        .filter_map(|fa| {
            let station = state
                .stations
                .iter()
                .find(|s| s.pos == fa.pos && s.stop_kind == openttdrs_core::StopKind::Airport)?;
            Some(FtaAirport {
                station: fa.station_id,
                x: fa.pos.x,
                y: fa.pos.y,
                w: fa.w,
                h: fa.h,
                airport_type: fa.airport_type,
                layout: fa.layout,
                blocks: station.airport_blocks,
            })
        })
        .collect();
    FtaTraceRow {
        kind,
        tick: state.tick.get(),
        aircraft,
        airports,
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

    let frozen_aircraft: HashMap<u32, FrozenAircraft> = sav
        .vehicles
        .iter()
        .enumerate()
        .filter(|(_, v)| v.kind == SavVehicleKind::Aircraft)
        .map(|(i, v)| {
            #[allow(clippy::cast_possible_truncation)]
            let id = i as u32;
            (
                id,
                FrozenAircraft {
                    engine_type: v.engine_type,
                    tile: v.raw_tile,
                },
            )
        })
        .collect();
    let frozen_airports: Vec<FrozenAirport> = sav
        .stations
        .iter()
        .filter(|st| st.airport_w > 0 && st.airport_h > 0)
        .map(|st| FrozenAirport {
            station_id: st.station_id,
            pos: st.pos,
            w: st.airport_w,
            h: st.airport_h,
            airport_type: st.airport_type,
            layout: st.airport_layout,
        })
        .collect();
    let station_pos_to_id: HashMap<TileCoord, u32> = frozen_airports
        .iter()
        .map(|fa| (fa.pos, fa.station_id))
        .collect();
    let aircraft_count = frozen_aircraft.len();
    let airport_count = frozen_airports.len();
    if aircraft_count == 0 {
        return Err("el save no contiene aviones importables".to_string());
    }

    let mut state = GameState::from_sav_game(sav);
    let file = std::fs::File::create(&args.out)
        .map_err(|e| format!("no se pudo crear {}: {e}", args.out.display()))?;
    let mut writer = BufWriter::new(file);
    write_row(
        &mut writer,
        &serde_json::json!({
            "kind": "metadata",
            "schema_version": 1,
            "producer": "openttdrs",
            "trace": "airport_fta",
            "source_path": args.save,
            "initial_sample_point": "after_sav_import",
            "tick_sample_point": "after_game_state_step",
            "max_ticks": args.ticks,
        }),
    )?;
    write_row(
        &mut writer,
        &trace_row(
            &state,
            "initial",
            &frozen_aircraft,
            &frozen_airports,
            &station_pos_to_id,
        ),
    )?;
    for _ in 0..args.ticks {
        state.step();
        write_row(
            &mut writer,
            &trace_row(
                &state,
                "tick",
                &frozen_aircraft,
                &frozen_airports,
                &station_pos_to_id,
            ),
        )?;
    }
    writer
        .flush()
        .map_err(|e| format!("no se pudo volcar {}: {e}", args.out.display()))?;
    println!(
        "{} — {} ticks, {aircraft_count} avión(es), {airport_count} aeropuerto(s) → {}",
        args.save.display(),
        args.ticks,
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
