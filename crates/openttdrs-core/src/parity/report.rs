//! Reporte de divergencias conocidas contra `OpenTTD` (evidencia archivo:línea
//! de ambos lados). Se genera desde la traza del escenario `truck_bay`; las
//! divergencias detectadas NO rompen CI: se documentan en
//! `docs/parity/divergences_found.md` y quedan pendientes para la Fase 2.

use std::collections::VecDeque;
use std::fmt::Write as _;

use super::record::{ParityEvent, TickRecord, TraceVehicleState, VehicleRecord};
use super::scenario::{
    TRAIN_LINE_VEHICLE_ID, TRAIN_SIGNAL_BLOCKER_ID, TRAIN_SIGNAL_LEAD_ID, TRAIN_SIGNAL_TILE,
    TRUCK_BAY_LOAD_ROAD, TRUCK_BAY_LOAD_STOP, TRUCK_BAY_VEHICLE_ID,
};
use crate::road_movement;
use crate::train_movement::is_diagonal_rail_piece;
use crate::vehicle::{Vehicle, VehicleKind};

/// Divergencia conocida detectada (o verificada) sobre una traza.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KnownDivergence {
    /// Identificador estable (`curve_speed_penalty`, `bay_stop_position`, …).
    pub id: &'static str,
    pub title: &'static str,
    /// La traza confirma que la divergencia sigue presente.
    pub detected: bool,
    /// Evidencia medida en la traza (ticks, velocidades, teselas).
    pub evidence: String,
    /// Referencia `archivo:línea` del comportamiento original en C++.
    pub openttd_ref: &'static str,
    /// Referencia `archivo:línea` del comportamiento actual en Rust.
    pub rust_ref: &'static str,
    /// Qué se espera implementar en la Fase 2.
    pub fix_phase2: &'static str,
}

fn speed_of(records: &[TickRecord], tick: u64, vehicle: u32) -> Option<u16> {
    records
        .iter()
        .find(|r| r.tick == tick)?
        .vehicles
        .iter()
        .find(|v| v.id == vehicle)
        .map(|v| v.speed)
}

fn is_road_vehicle(records: &[TickRecord], tick: u64, vehicle: u32) -> bool {
    records
        .iter()
        .find(|r| r.tick == tick)
        .and_then(|r| r.vehicles.iter().find(|v| v.id == vehicle))
        .is_some_and(|v| v.rail.is_none())
}

/// Divergencia 1: en `OpenTTD` (modelo original) todo cambio de dirección
/// reduce la velocidad un 25 % (`cur_speed -= cur_speed >> 2`). Implementada
/// en la Fase 2 (`Vehicle::set_direction_with_curve_penalty`); el chequeo
/// queda como regresión: si vuelve a detectarse, la paridad se rompió.
fn check_curve_speed_penalty(records: &[TickRecord]) -> KnownDivergence {
    let vehicle = TRUCK_BAY_VEHICLE_ID;
    let mut evidence = String::new();
    let mut detected = false;
    for r in records {
        for e in &r.events {
            let ParityEvent::DirectionChanged { from, to, .. } = e else {
                continue;
            };
            if e.vehicle() != Some(vehicle) || from % 2 == 0 || to % 2 == 0 {
                continue;
            }
            if !is_road_vehicle(records, r.tick, vehicle) {
                continue;
            }
            let before = speed_of(records, r.tick.saturating_sub(1), vehicle).unwrap_or(0);
            let after = speed_of(records, r.tick, vehicle).unwrap_or(0);
            if before == 0 {
                continue;
            }
            // Mismo orden que OpenTTD (`RoadVehController`): primero acelera
            // (`UpdateSpeed`, +1 con ROAD_ACCEL_ORIGINAL a crucero), después
            // penaliza el giro. La cota se calcula sobre `before + 1`.
            let accelerated = before.saturating_add(1);
            let expected_openttd = accelerated - (accelerated >> 2);
            let _ = writeln!(
                evidence,
                "- tick {}: giro dir {from}→{to}; velocidad {before}→{after} (OpenTTD esperaría ≤ {expected_openttd})",
                r.tick
            );
            if after > expected_openttd {
                detected = true;
            }
        }
    }
    if evidence.is_empty() {
        evidence.push_str("- la traza no contiene giros diagonales del camión\n");
    }
    KnownDivergence {
        id: "curve_speed_penalty",
        title: "Falta la penalización de velocidad del 25 % en curvas",
        detected,
        evidence,
        openttd_ref: "OpenTTD/src/roadveh_cmd.cpp:1481 (`v->cur_speed -= v->cur_speed >> 2`, AM_ORIGINAL; también :1353 y :1426)",
        rust_ref: "openttdrs/crates/openttdrs-core/src/vehicle.rs (`Vehicle::set_direction_with_curve_penalty`)",
        fix_phase2: "IMPLEMENTADA (Fase 2): `cur_speed -= cur_speed >> 2` al cambiar `direction` en bus/camión",
    }
}

/// Divergencia 2: en `OpenTTD` el vehículo entra a la tesela de la bahía y se
/// detiene en el frame indicado por `_road_stop_stop_frame` (valores 11–20).
/// Corregida en la Fase 2: `resolve_order_destination` apunta a la tesela de
/// la bahía y el camión carga dentro. El chequeo queda como regresión.
fn check_bay_stop_position(records: &[TickRecord]) -> KnownDivergence {
    let vehicle = TRUCK_BAY_VEHICLE_ID;
    let mut evidence = String::new();
    let mut detected = false;
    for r in records {
        for e in &r.events {
            let ParityEvent::LoadingStarted { .. } = e else {
                continue;
            };
            if e.vehicle() != Some(vehicle) {
                continue;
            }
            let Some(v) = r.vehicles.iter().find(|v| v.id == vehicle) else {
                continue;
            };
            let _ = writeln!(
                evidence,
                "- tick {}: carga iniciada con el camión en {:?} (bahía = {:?}, acceso = {:?})",
                r.tick, v.tile, TRUCK_BAY_LOAD_STOP, TRUCK_BAY_LOAD_ROAD
            );
            if v.tile == TRUCK_BAY_LOAD_ROAD && v.tile != TRUCK_BAY_LOAD_STOP {
                detected = true;
            }
        }
    }
    if evidence.is_empty() {
        evidence.push_str("- la traza no contiene eventos de carga del camión\n");
    }
    KnownDivergence {
        id: "bay_stop_position",
        title: "El camión se detiene en la carretera de acceso, no dentro de la bahía",
        detected,
        evidence,
        openttd_ref: "OpenTTD/src/table/roadveh_movement.h:1087-1093 (`_road_stop_stop_frame`, frames 11-20) y OpenTTD/src/roadveh_cmd.cpp:1496-1502 (chequeo del frame de parada)",
        rust_ref: "openttdrs/crates/openttdrs-core/src/station.rs (`resolve_order_destination` → tesela de la bahía; `is_connected_bay_road_stop`)",
        fix_phase2: "IMPLEMENTADA (Fase 2): bus/camión entra a la tesela de la bahía y carga dentro; pendiente afinar el punto exacto de parada (`_road_stop_stop_frame`) en el render",
    }
}

/// Divergencia 3: en `OpenTTD` la carga/descarga es gradual (por tick, en
/// `LoadUnloadVehicle`); en la sim Rust es instantánea (un tick).
fn check_instant_loading(records: &[TickRecord]) -> KnownDivergence {
    let vehicle = TRUCK_BAY_VEHICLE_ID;
    let mut evidence = String::new();
    let mut detected = false;
    for r in records {
        let started = r.events.iter().any(|e| {
            matches!(e, ParityEvent::LoadingStarted { .. }) && e.vehicle() == Some(vehicle)
        });
        let finished = r.events.iter().any(|e| {
            matches!(e, ParityEvent::LoadingFinished { .. }) && e.vehicle() == Some(vehicle)
        });
        if started && finished {
            detected = true;
            let _ = writeln!(
                evidence,
                "- tick {}: `loading_started` y `loading_finished` en el mismo tick (carga instantánea)",
                r.tick
            );
        }
    }
    if evidence.is_empty() {
        evidence.push_str("- la traza no contiene eventos de carga del camión\n");
    }
    KnownDivergence {
        id: "instant_loading",
        title: "Carga/descarga instantánea (OpenTTD la hace gradual por tick)",
        detected,
        evidence,
        openttd_ref: "OpenTTD/src/economy.cpp:1609 (`LoadUnloadVehicle`, transfiere por tick)",
        rust_ref: "openttdrs/crates/openttdrs-core/src/sim_step.rs:205-241 (`try_load_from_industry` carga la capacidad completa en un tick)",
        fix_phase2: "modelar carga gradual con velocidad de carga por tipo de cargo",
    }
}

/// Divergencia estructural: frecuencia del tick lógico. `OpenTTD` corre 74 ticks/día
/// (~33,3 ticks/s a velocidad normal); la sim Rust corre a 5 Hz con pasos
/// sub-tesela reescalados. Divergencia estructural, no medible en la traza.
fn check_tick_rate() -> KnownDivergence {
    KnownDivergence {
        id: "tick_rate",
        title: "Tick de simulación a 5 Hz frente a ~33,3 Hz de OpenTTD",
        detected: true,
        evidence: "- constante: `SIM_TICK_HZ = 5.0` con `REFERENCE_PROGRESS_STEP = 51` (5 ticks/tesela); OpenTTD avanza `frame` cada tick a 74 ticks/día\n".to_string(),
        openttd_ref: "OpenTTD/src/timer/timer_game_tick.h:77 (`DAY_TICKS = 74`)",
        rust_ref: "openttdrs/crates/openttdrs-client/src/simulation.rs:12 (`SIM_TICK_HZ = 5.0`) y openttdrs/crates/openttdrs-core/src/engine.rs:71 (`REFERENCE_PROGRESS_STEP = 51`)",
        fix_phase2: "DECIDIDO (Fase 2): se mantiene 5 Hz y la paridad se valida en unidades relativas; criterios de revisión en docs/parity/tick_rate_decision.md",
    }
}

fn trace_has_train(records: &[TickRecord]) -> bool {
    records
        .iter()
        .any(|r| r.vehicles.iter().any(|v| v.id == TRAIN_LINE_VEHICLE_ID))
}

fn trace_has_train_signal(records: &[TickRecord]) -> bool {
    records.iter().any(|r| {
        let ids: Vec<u32> = r.vehicles.iter().map(|v| v.id).collect();
        ids.contains(&TRAIN_SIGNAL_LEAD_ID) && ids.contains(&TRAIN_SIGNAL_BLOCKER_ID)
    })
}

/// Divergencia rail 1: tren reusa aceleración de carretera (`ROAD_ACCEL_ORIGINAL`)
/// en lugar de `Clamp(power/weight·4, 1, 255)` con `accel·2`.
/// Corregida en Rail 3B; el chequeo queda como regresión.
fn check_train_road_acceleration(records: &[TickRecord]) -> KnownDivergence {
    let vehicle = TRAIN_LINE_VEHICLE_ID;
    let mut evidence = String::new();
    let mut detected = false;

    // Tras salir del depósito, la carretera alcanza speed≥2 en ~2 ticks desde 0;
    // Kirby (accel=24) necesita varios ticks más.
    let mut ticks_to_two = None;
    let mut prev_speed = 0_u16;
    for r in records {
        let Some(speed) = speed_of(records, r.tick, vehicle) else {
            continue;
        };
        if speed >= 2 && prev_speed < 2 && ticks_to_two.is_none() {
            // Cuenta ticks consecutivos en movimiento desde el primer speed>0.
            let mut last_zero = r.tick;
            for back in (0..=r.tick).rev() {
                let s = speed_of(records, back, vehicle).unwrap_or(0);
                if s == 0 {
                    last_zero = back;
                    break;
                }
            }
            let accel_ticks = r.tick.saturating_sub(last_zero);
            let _ = ticks_to_two.insert(accel_ticks);
            let _ = writeln!(
                evidence,
                "- tick {}: speed≥2 tras {} ticks desde parado (carretera ≈1–2; Kirby AM_ORIGINAL ≫2)",
                r.tick, accel_ticks
            );
            if accel_ticks <= 2 {
                detected = true;
            }
        }
        prev_speed = speed;
    }

    if evidence.is_empty() {
        evidence.push_str("- la traza no contiene aceleración del tren desde parado\n");
    }
    KnownDivergence {
        id: "train_road_acceleration",
        title: "El tren acelera con la fórmula de carretera (ROAD_ACCEL_ORIGINAL) en lugar de power/weight·4",
        detected,
        evidence,
        openttd_ref: "OpenTTD/src/train_cmd.cpp:444-452 (`UpdateAcceleration`) y :3080-3090 (`UpdateSpeed` AM_ORIGINAL, `accel·2`)",
        rust_ref: "openttdrs/crates/openttdrs-core/src/vehicle.rs (`update_movement_speed` → `accelerate_train_speed`)",
        fix_phase2: "IMPLEMENTADA (Rail 3B): `train_acceleration` + `accel·2` / freno `accel·4`",
    }
}

/// Divergencia rail 2: sin frenado por curva `_accel_slowdown` en trenes.
/// Corregida en Rail 3B; el chequeo queda como regresión.
fn check_train_no_curve_braking(records: &[TickRecord]) -> KnownDivergence {
    let vehicle = TRAIN_LINE_VEHICLE_ID;
    let mut evidence = String::new();
    let mut detected = false;
    for r in records {
        for e in &r.events {
            let ParityEvent::DirectionChanged { from, to, .. } = e else {
                continue;
            };
            if e.vehicle() != Some(vehicle) {
                continue;
            }
            if records
                .iter()
                .find(|rec| rec.tick == r.tick)
                .and_then(|rec| rec.vehicles.iter().find(|v| v.id == vehicle))
                .is_none_or(|v| v.rail.is_none())
            {
                continue;
            }
            let before = speed_of(records, r.tick.saturating_sub(1), vehicle).unwrap_or(0);
            let after = speed_of(records, r.tick, vehicle).unwrap_or(0);
            if before == 0 {
                continue;
            }
            // Penalización mínima esperada: al menos 25 % (giro 45°) o 50 % (90°).
            let min_expected = before - (before >> 2);
            let _ = writeln!(
                evidence,
                "- tick {}: giro dir {from}→{to}; velocidad {before}→{after} (OpenTTD esperaría ≤ {min_expected})",
                r.tick
            );
            if after > min_expected {
                detected = true;
            }
        }
    }
    if evidence.is_empty() {
        evidence.push_str("- la traza no contiene giros del tren\n");
    }
    KnownDivergence {
        id: "train_no_curve_braking",
        title: "Falta el frenado por curva del tren (`_accel_slowdown`)",
        detected,
        evidence,
        openttd_ref: "OpenTTD/src/train_cmd.cpp:3147-3152 (`_accel_slowdown`), :3564-3568 (aplicación en locomotora)",
        rust_ref: "openttdrs/crates/openttdrs-core/src/vehicle.rs (`set_direction_with_curve_penalty` para `VehicleKind::Train`)",
        fix_phase2: "IMPLEMENTADA (Rail 3B): `cur_speed -= turn·cur_speed >> 8` con small_turn=64 / large_turn=128",
    }
}

fn check_train_platform_stop(records: &[TickRecord]) -> KnownDivergence {
    let vehicle = TRAIN_LINE_VEHICLE_ID;
    let mut evidence = String::new();
    let mut detected = false;
    for r in records {
        for e in &r.events {
            let ParityEvent::LoadingStarted { .. } = e else {
                continue;
            };
            if e.vehicle() != Some(vehicle) {
                continue;
            }
            let Some(v) = r.vehicles.iter().find(|v| v.id == vehicle) else {
                continue;
            };
            let on_platform = v.rail.as_ref().is_some_and(|rail| rail.at_platform);
            let _ = writeln!(
                evidence,
                "- tick {}: carga iniciada en {:?} (at_platform={on_platform})",
                r.tick, v.tile
            );
            if !on_platform {
                detected = true;
            }
        }
    }
    if evidence.is_empty() {
        evidence.push_str("- la traza no contiene eventos de carga del tren\n");
    }
    KnownDivergence {
        id: "train_platform_stop",
        title: "El tren carga desde la vía de acceso, no desde la plataforma",
        detected,
        evidence,
        openttd_ref: "OpenTTD/src/train_cmd.cpp:266-305 (`GetTrainStopLocation`) y :3097-3123 (`TrainEnterStation`)",
        rust_ref: "openttdrs/crates/openttdrs-core/src/station.rs (`resolve_order_destination` → `rail_station_stop_tile`)",
        fix_phase2: "IMPLEMENTADA (Rail 3C): destino = plataforma; `at_platform: true` en la traza",
    }
}

/// Regresión Rail 3D: el tren líder del escenario `train_signal` debe emitir
/// `SignalWaitStarted` y `SignalWaitFinished` en la señal al liberarse el bloque.
fn check_train_signal_wait(records: &[TickRecord]) -> KnownDivergence {
    let lead = TRAIN_SIGNAL_LEAD_ID;
    let signal = TRAIN_SIGNAL_TILE;
    let mut evidence = String::new();
    let mut started_tick = None;
    let mut finished_tick = None;
    for r in records {
        for e in &r.events {
            match e {
                ParityEvent::SignalWaitStarted { vehicle, tile }
                    if *vehicle == lead && *tile == signal =>
                {
                    if started_tick.is_none() {
                        started_tick = Some(r.tick);
                    }
                    let _ = writeln!(
                        evidence,
                        "- tick {}: SignalWaitStarted (vehículo {lead}, señal {signal:?})",
                        r.tick
                    );
                }
                ParityEvent::SignalWaitFinished { vehicle, tile }
                    if *vehicle == lead && *tile == signal =>
                {
                    finished_tick = Some(r.tick);
                    let _ = writeln!(
                        evidence,
                        "- tick {}: SignalWaitFinished (vehículo {lead}, señal {signal:?})",
                        r.tick
                    );
                }
                _ => {}
            }
        }
    }
    let detected =
        started_tick.is_none() || finished_tick.is_none() || started_tick >= finished_tick;
    if evidence.is_empty() {
        evidence
            .push_str("- la traza no contiene eventos SignalWait* del escenario train_signal\n");
    } else if let (Some(s), Some(f)) = (started_tick, finished_tick) {
        let _ = writeln!(
            evidence,
            "- duración de espera: {} ticks (tick {s} → {f})",
            f - s
        );
    }
    KnownDivergence {
        id: "train_signal_wait",
        title: "Espera en señal sin eventos SignalWait* o sin reanudación",
        detected,
        evidence,
        openttd_ref: "OpenTTD/src/train_cmd.cpp (espera ante señal / bloque ocupado)",
        rust_ref: "openttdrs/crates/openttdrs-core/src/parity/tracer.rs (`SignalWaitStarted` / `SignalWaitFinished`)",
        fix_phase2: "IMPLEMENTADA (Rail 3D): escenario `train_signal` + chequeo de regresión",
    }
}

fn subtile_from_train_record(v: &VehicleRecord) -> Option<(f32, f32)> {
    v.rail.as_ref()?;
    let mut train = Vehicle::new(v.id, VehicleKind::Train, v.tile, v.dest);
    train.progress = v.progress;
    train.direction = v.dir;
    train.cur_speed = v.speed;
    train.depart_turn = v.depart_turn;
    train.running = !matches!(v.state, TraceVehicleState::Stopped);
    if let Some(next) = v.path_next {
        train.path = VecDeque::from([next]);
    }
    Some(road_movement::vehicle_subtile(&train))
}

/// Regresión Rail 3E: la sub-tesela en la traza debe coincidir con
/// `vehicle_subtile` (misma función que usa el render a `tick_alpha = 0`).
fn check_train_render_subtile_consistency(records: &[TickRecord]) -> KnownDivergence {
    let mut evidence = String::new();
    let mut detected = false;
    for r in records {
        for v in &r.vehicles {
            let Some(rail) = &v.rail else {
                continue;
            };
            let Some(part) = rail.parts.first() else {
                continue;
            };
            let Some((sx, sy)) = subtile_from_train_record(v) else {
                continue;
            };
            if (part.subtile_x - sx).abs() > 0.01 || (part.subtile_y - sy).abs() > 0.01 {
                detected = true;
                let _ = writeln!(
                    evidence,
                    "- tick {}: traza ({:.3},{:.3}) ≠ render ({sx:.3},{sy:.3}) en {:?}",
                    r.tick, part.subtile_x, part.subtile_y, v.tile
                );
            }
        }
    }
    if evidence.is_empty() {
        evidence.push_str("- la traza rail y `vehicle_subtile` coinciden en todos los ticks\n");
    }
    KnownDivergence {
        id: "train_render_subtile_consistency",
        title: "La sub-tesela de la traza rail no coincide con la del render",
        detected,
        evidence,
        openttd_ref: "OpenTTD/src/vehicle.cpp:3359 (`_vehicle_subcoord` + progreso)",
        rust_ref: "openttdrs/crates/openttdrs-core/src/parity/tracer.rs + `road_movement::vehicle_subtile`",
        fix_phase2: "IMPLEMENTADA (Rail 3E): regresión traza ↔ render lógico",
    }
}

/// Divergencia conocida Rail 3E: en piezas diagonales puras el render usa
/// `train_straight_subtile` (centro de vía) en lugar de `_vehicle_subcoord`.
fn check_train_diagonal_subcoord_approximation(records: &[TickRecord]) -> KnownDivergence {
    let mut evidence = String::new();
    let mut detected = false;
    for r in records {
        for v in &r.vehicles {
            let Some(rail) = &v.rail else {
                continue;
            };
            if !is_diagonal_rail_piece(rail.track_bits_under) {
                continue;
            }
            detected = true;
            let _ = writeln!(
                evidence,
                "- tick {}: pieza diagonal track_bits={:#04x} en {:?} (render ≈ centro de vía)",
                r.tick, rail.track_bits_under, v.tile
            );
        }
    }
    if evidence.is_empty() {
        evidence
            .push_str("- la traza no recorre piezas diagonales puras (UPPER/LOWER/LEFT/RIGHT)\n");
    }
    KnownDivergence {
        id: "train_diagonal_subcoord_approximation",
        title: "Subcoordenadas por pieza: centro de vía en curvas diagonales",
        detected,
        evidence,
        openttd_ref: "OpenTTD/src/vehicle.cpp:3359-3392 (`_vehicle_subcoord` por enterdir×track)",
        rust_ref: "openttdrs/crates/openttdrs-core/src/road_movement.rs (`train_straight_subtile`, `TRAIN_TRACK_CENTER = 8`)",
        fix_phase2: "DECIDIDO (Rail 3E): divergencia cosmética documentada; X/Y usan el mismo eje que la entrada OpenTTD",
    }
}

/// Evalúa todas las divergencias conocidas sobre una traza de paridad.
#[must_use]
pub fn detect_known_divergences(records: &[TickRecord]) -> Vec<KnownDivergence> {
    let mut out = vec![
        check_curve_speed_penalty(records),
        check_bay_stop_position(records),
        check_instant_loading(records),
        check_tick_rate(),
    ];
    if trace_has_train(records) {
        out.push(check_train_road_acceleration(records));
        out.push(check_train_no_curve_braking(records));
        out.push(check_train_platform_stop(records));
        out.push(check_train_render_subtile_consistency(records));
        out.push(check_train_diagonal_subcoord_approximation(records));
    }
    if trace_has_train_signal(records) {
        out.push(check_train_signal_wait(records));
    }
    out
}

/// Markdown para `docs/parity/divergences_found.md`.
#[must_use]
pub fn divergences_markdown(divergences: &[KnownDivergence]) -> String {
    let mut out = String::new();
    out.push_str("# Divergencias conocidas openttdrs ↔ OpenTTD\n\n");
    out.push_str("Archivo generado por `parity_runner --divergence-report`.\n");
    out.push_str(
        "Estas divergencias son conocidas y NO rompen CI; su corrección es parte de las fases de paridad.\n\n",
    );
    for d in divergences {
        let status = if d.detected {
            "CONFIRMADA en la traza"
        } else {
            "no observada en esta traza"
        };
        let _ = writeln!(out, "## {} (`{}`)\n", d.title, d.id);
        let _ = writeln!(out, "Estado: **{status}**\n");
        let _ = writeln!(out, "Evidencia medida:\n\n{}", d.evidence);
        let _ = writeln!(out, "- Referencia OpenTTD: `{}`", d.openttd_ref);
        let _ = writeln!(out, "- Referencia Rust: `{}`", d.rust_ref);
        let _ = writeln!(out, "- Fase 2: {}\n", d.fix_phase2);
    }
    out
}
