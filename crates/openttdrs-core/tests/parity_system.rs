//! Tests del sistema de paridad: esquema de traza, ciclo completo del
//! escenario `truck_bay`, detección de mutaciones, determinismo, roundtrip
//! de guardado a mitad de escenario y traza ferroviaria (Fase Rail 1).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use openttdrs_core::parity::{
    self, DiffFilter, ParityEvent, TickRecord, build_train_line, build_truck_bay, compare_traces,
};
use openttdrs_core::{GameState, TileCoord};

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
fn truck_trace_has_no_rail_block() {
    let mut state = build_truck_bay();
    let records = run_trace(&mut state, 20);
    for record in &records {
        assert!(
            record.vehicles.iter().all(|v| v.rail.is_none()),
            "los vehículos de carretera no llevan bloque rail"
        );
        // Y el JSONL tampoco contiene la clave (byte-compatible con trazas previas).
        let line = serde_json::to_string(record).unwrap();
        assert!(
            !line.contains("\"rail\""),
            "la clave rail no debe serializarse para camiones: {line}"
        );
    }
}

#[test]
fn rail_events_and_record_roundtrip_serde() {
    let events = vec![
        ParityEvent::SignalWaitStarted {
            vehicle: 1,
            tile: TileCoord::new(2, 0),
        },
        ParityEvent::SignalWaitFinished {
            vehicle: 1,
            tile: TileCoord::new(2, 0),
        },
        ParityEvent::DepotEntry {
            vehicle: 1,
            depot: TileCoord::new(4, 5),
        },
        ParityEvent::DepotExit {
            vehicle: 1,
            depot: TileCoord::new(4, 5),
        },
        ParityEvent::SignalStateChanged {
            tile: TileCoord::new(7, 6),
            track_mask: 0b01,
            green: false,
        },
    ];
    for event in &events {
        let json = serde_json::to_string(event).unwrap();
        let parsed: ParityEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(&parsed, event, "roundtrip de {json}");
    }
    assert_eq!(
        ParityEvent::SignalStateChanged {
            tile: TileCoord::new(0, 0),
            track_mask: 1,
            green: true,
        }
        .vehicle(),
        None,
        "evento de infraestructura sin vehículo"
    );

    // Un registro con bloque rail re-parsea idéntico (incluye floats de sub-tesela).
    let mut state = build_train_line();
    let records = run_trace(&mut state, 5);
    let line = serde_json::to_string(&records[0]).unwrap();
    let parsed: TickRecord = serde_json::from_str(&line).unwrap();
    assert_eq!(parsed, records[0]);
}

#[test]
fn train_line_emits_rail_block_and_events() {
    let mut state = build_train_line();
    let records = run_trace(&mut state, 600);

    for record in &records {
        let train = &record.vehicles[0];
        assert_eq!(train.id, parity::TRAIN_LINE_VEHICLE_ID);
        let rail = train
            .rail
            .as_ref()
            .expect("el tren siempre lleva bloque rail");
        assert_eq!(rail.parts.len(), 1, "tren puntual: una sola parte");
        assert_eq!(
            rail.head_tile, rail.tail_tile,
            "sin consist: cabeza == cola"
        );
        assert_eq!(rail.parts[0].tile, train.tile);
        assert!(
            !rail.at_platform,
            "divergencia documentada: el tren nunca pisa la plataforma (Fase Rail 3C)"
        );
    }

    let has =
        |pred: &dyn Fn(&ParityEvent) -> bool| records.iter().flat_map(|r| &r.events).any(pred);
    assert!(
        has(&|e| matches!(e, ParityEvent::DepotExit { .. })),
        "el tren sale del depósito"
    );
    assert!(
        has(&|e| matches!(e, ParityEvent::StationEntry { .. })),
        "llegada a la estación A"
    );
    assert!(
        has(&|e| matches!(e, ParityEvent::LoadingStarted { .. })),
        "carga de goods en A"
    );
    assert!(
        has(&|e| matches!(e, ParityEvent::OrderAdvanced { .. })),
        "avance de orden A → B"
    );
    assert!(
        has(&|e| matches!(e, ParityEvent::DirectionChanged { .. })),
        "curva de la L"
    );
    assert!(
        has(&|e| matches!(
            e,
            ParityEvent::SignalStateChanged {
                tile: parity::TRAIN_LINE_SIGNAL,
                ..
            }
        )),
        "la señal cambia de estado cuando el tren ocupa/libera el bloque"
    );
}

#[test]
fn train_line_trace_is_deterministic() {
    let mut a = build_train_line();
    let mut b = build_train_line();
    let trace_a = run_trace(&mut a, 300);
    let trace_b = run_trace(&mut b, 300);
    assert_eq!(trace_a, trace_b, "misma seed ⇒ trazas idénticas");
}

#[test]
fn signal_wait_events_emitted_with_two_trains() {
    use openttdrs_core::command::{Command, apply_command};
    use openttdrs_core::{Vehicle, VehicleKind};

    // Línea recta X con señal (en ambos sentidos) en (2,0). El tren de cabeza
    // arranca sobre la señal con vía libre; el que ocupa el bloque se agrega
    // DESPUÉS de activar la traza, para capturar la transición libre → espera.
    let mut state = GameState::new(12, 4);
    for x in 0..=6 {
        apply_command(
            &mut state,
            &Command::SetRailBits(TileCoord::new(x, 0), 0x01),
        )
        .unwrap();
    }
    let signal = TileCoord::new(2, 0);
    apply_command(&mut state, &Command::PlaceRailSignal(signal, 0, 128, 128)).unwrap();
    // Señal en ambos sentidos del carril X (bits 2 y 3), como `write_signal`
    // de los tests unitarios: garantiza que la salida +x esté señalizada.
    let mut tile = state.map.get(signal).unwrap();
    tile.m3 = (tile.m3 & 0x0F) | 0xC0;
    tile.m3hi = (tile.m3hi & 0x0F) | 0xC0;
    state.map.set_tile(signal, tile).unwrap();

    let mut lead = Vehicle::new(1, VehicleKind::Train, signal, TileCoord::new(6, 0));
    lead.path = (3..=6).map(|x| TileCoord::new(x, 0)).collect();
    lead.set_cruise_speed();
    state.vehicles.push(lead);

    state.enable_parity_trace();
    state.vehicles.push(Vehicle::new(
        2,
        VehicleKind::Train,
        TileCoord::new(4, 0),
        TileCoord::new(4, 0),
    ));
    for _ in 0..30 {
        state.step();
    }
    assert_eq!(
        state.vehicles[0].pos, signal,
        "el tren debe quedar esperando en la señal"
    );
    let blocker = state.vehicles.pop().expect("sacar el tren que bloquea");
    assert_eq!(blocker.id, 2);
    for _ in 0..120 {
        state.step();
    }
    let records = state.take_parity_records();

    let started_tick = records
        .iter()
        .find(|r| {
            r.events.iter().any(|e| {
                matches!(e, ParityEvent::SignalWaitStarted { vehicle: 1, tile } if *tile == signal)
            })
        })
        .map(|r| r.tick)
        .expect("debe emitirse signal_wait_started en la señal");
    let finished_tick = records
        .iter()
        .find(|r| {
            r.events.iter().any(|e| {
                matches!(e, ParityEvent::SignalWaitFinished { vehicle: 1, tile } if *tile == signal)
            })
        })
        .map(|r| r.tick)
        .expect("debe emitirse signal_wait_finished al liberarse el bloque");
    assert!(started_tick < finished_tick, "espera antes de liberación");
    assert!(
        state.vehicles[0].pos != signal || state.vehicles[0].progress > 0,
        "el tren retoma la marcha tras liberarse el bloque"
    );
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
