//! Reporte de divergencias conocidas contra `OpenTTD` (evidencia archivo:línea
//! de ambos lados). Se genera desde la traza del escenario `truck_bay`; las
//! divergencias detectadas NO rompen CI: se documentan en
//! `docs/parity/divergences_found.md` y quedan pendientes para la Fase 2.

use std::fmt::Write as _;

use super::record::{ParityEvent, TickRecord};
use super::scenario::{TRUCK_BAY_LOAD_ROAD, TRUCK_BAY_LOAD_STOP, TRUCK_BAY_VEHICLE_ID};

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

/// Divergencia 4: frecuencia del tick lógico. `OpenTTD` corre 74 ticks/día
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

/// Evalúa todas las divergencias conocidas sobre una traza de `truck_bay`.
#[must_use]
pub fn detect_known_divergences(records: &[TickRecord]) -> Vec<KnownDivergence> {
    vec![
        check_curve_speed_penalty(records),
        check_bay_stop_position(records),
        check_instant_loading(records),
        check_tick_rate(),
    ]
}

/// Markdown para `docs/parity/divergences_found.md`.
#[must_use]
pub fn divergences_markdown(divergences: &[KnownDivergence]) -> String {
    let mut out = String::new();
    out.push_str("# Divergencias conocidas openttdrs ↔ OpenTTD\n\n");
    out.push_str("Archivo generado por `parity_runner --divergence-report` sobre el escenario `truck_bay`.\n");
    out.push_str(
        "Estas divergencias son conocidas y NO rompen CI; su corrección es parte de la Fase 2.\n\n",
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
