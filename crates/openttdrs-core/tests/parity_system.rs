//! Tests del sistema de paridad: esquema de traza, ciclo completo del
//! escenario `truck_bay`, detección de mutaciones, determinismo y roundtrip
//! de guardado a mitad de escenario.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use openttdrs_core::GameState;
use openttdrs_core::parity::{
    self, DiffFilter, ParityEvent, TickRecord, build_truck_bay, compare_traces,
};

fn run_trace(state: &mut GameState, ticks: u64) -> Vec<TickRecord> {
    state.enable_parity_trace();
    for _ in 0..ticks {
        state.step();
    }
    state.take_parity_records()
}

#[test]
fn trace_schema_is_valid_for_50_ticks() {
    let mut state = build_truck_bay();
    let records = run_trace(&mut state, 50);
    assert_eq!(records.len(), 50, "un registro por tick");
    for (i, record) in records.iter().enumerate() {
        assert_eq!(record.tick, i as u64 + 1, "ticks consecutivos desde 1");
        assert_eq!(record.vehicles.len(), 1, "el camión está en cada registro");
        let v = &record.vehicles[0];
        assert_eq!(v.id, parity::TRUCK_BAY_VEHICLE_ID);
        assert!(v.dir < 8);
        assert_eq!(v.order_kind.as_deref(), Some("station"));
        // Cada línea serializa y re-parsea al mismo registro (esquema estable).
        let line = serde_json::to_string(record).unwrap();
        let parsed: TickRecord = serde_json::from_str(&line).unwrap();
        assert_eq!(&parsed, record);
    }
}

#[test]
fn truck_bay_completes_arrival_load_departure_cycle() {
    let mut state = build_truck_bay();
    let records = run_trace(&mut state, 500);

    let has =
        |pred: &dyn Fn(&ParityEvent) -> bool| records.iter().flat_map(|r| &r.events).any(pred);
    assert!(
        has(&|e| matches!(e, ParityEvent::StationEntry { .. })),
        "llegada a la parada"
    );
    assert!(
        has(&|e| matches!(e, ParityEvent::LoadingStarted { .. })),
        "carga iniciada"
    );
    assert!(
        has(&|e| matches!(e, ParityEvent::LoadingFinished { .. })),
        "carga terminada"
    );
    assert!(
        has(&|e| matches!(e, ParityEvent::UnloadingStarted { .. })),
        "descarga en destino"
    );
    assert!(
        has(&|e| matches!(e, ParityEvent::DepartTurnStarted { .. })),
        "giro de salida de la bahía"
    );
    assert!(
        has(&|e| matches!(e, ParityEvent::OrderAdvanced { .. })),
        "avance de orden tras cargar"
    );
    assert!(
        has(&|e| matches!(e, ParityEvent::DirectionChanged { .. })),
        "curvas de 90° en la ruta"
    );
    assert!(
        state.stats.cargo_units_delivered > 0,
        "el ciclo entrega cargo"
    );
}

#[test]
fn diff_detects_artificial_mutation_at_exact_tick() {
    let mut state = build_truck_bay();
    let records = run_trace(&mut state, 100);
    let mut mutated = records.clone();
    mutated[59].vehicles[0].progress = mutated[59].vehicles[0].progress.wrapping_add(7);

    let report = compare_traces(&records, &mutated, DiffFilter::default());
    let first = report.first.expect("la mutación debe detectarse");
    assert_eq!(first.tick, records[59].tick);
    assert_eq!(first.vehicle, Some(parity::TRUCK_BAY_VEHICLE_ID));
    assert_eq!(first.field, "progress");
}

#[test]
fn same_scenario_twice_is_deterministic() {
    let mut a = build_truck_bay();
    let mut b = build_truck_bay();
    let trace_a = run_trace(&mut a, 300);
    let trace_b = run_trace(&mut b, 300);
    assert_eq!(trace_a, trace_b, "misma seed ⇒ trazas idénticas");
    let report = compare_traces(&trace_a, &trace_b, DiffFilter::default());
    assert!(!report.has_divergence());
}

#[test]
fn save_json_roundtrip_mid_scenario_preserves_trace() {
    // Corre 120 ticks, guarda, y compara los 180 ticks siguientes del estado
    // original contra los del estado recargado desde JSON.
    let mut original = build_truck_bay();
    for _ in 0..120 {
        original.step();
    }
    let json = original.save_json().expect("guardar a mitad de escenario");
    let mut restored = GameState::load_json(&json).expect("recargar el save");

    let trace_original = run_trace(&mut original, 180);
    let trace_restored = run_trace(&mut restored, 180);
    let report = compare_traces(&trace_original, &trace_restored, DiffFilter::default());
    assert!(
        !report.has_divergence(),
        "divergencia tras roundtrip: {:?}",
        report.first
    );
}
