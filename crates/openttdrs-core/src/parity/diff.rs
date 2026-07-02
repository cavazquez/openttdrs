//! Comparador de trazas de paridad: primera divergencia + agrupación por subsistema.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use super::record::{ParityEvent, TickRecord, VehicleRecord};

/// Subsistema al que pertenece un campo divergente (para agrupar el reporte).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Subsystem {
    Movement,
    Speed,
    Orders,
    Cargo,
    Events,
    Structure,
}

impl Subsystem {
    /// Parsea el filtro `--subsystem` de la CLI.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "movement" | "movimiento" => Some(Self::Movement),
            "speed" | "velocidad" => Some(Self::Speed),
            "orders" | "ordenes" => Some(Self::Orders),
            "cargo" | "carga" => Some(Self::Cargo),
            "events" | "eventos" => Some(Self::Events),
            _ => None,
        }
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Movement => "movement",
            Self::Speed => "speed",
            Self::Orders => "orders",
            Self::Cargo => "cargo",
            Self::Events => "events",
            Self::Structure => "structure",
        }
    }
}

fn field_subsystem(field: &str) -> Subsystem {
    match field {
        "tile" | "progress" | "dir" | "depart_turn" => Subsystem::Movement,
        "speed" | "subspeed" => Subsystem::Speed,
        "cargo" => Subsystem::Cargo,
        "events" => Subsystem::Events,
        _ => Subsystem::Orders,
    }
}

/// Una diferencia concreta entre las dos trazas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Divergence {
    pub tick: u64,
    pub vehicle: Option<u32>,
    pub field: String,
    pub expected: String,
    pub actual: String,
    pub subsystem: Subsystem,
}

/// Resultado de comparar dos trazas.
#[derive(Debug, Clone, Default)]
pub struct DiffReport {
    /// Primera divergencia encontrada (orden: tick, vehículo, campo).
    pub first: Option<Divergence>,
    /// Eventos cercanos (±3 ticks) a la primera divergencia, por traza.
    pub context_expected: Vec<(u64, ParityEvent)>,
    pub context_actual: Vec<(u64, ParityEvent)>,
    /// Diferencias totales (incluida la primera).
    pub total: usize,
    /// Recuento de diferencias por subsistema.
    pub by_subsystem: BTreeMap<&'static str, usize>,
}

impl DiffReport {
    #[must_use]
    pub const fn has_divergence(&self) -> bool {
        self.first.is_some()
    }
}

/// Filtros del comparador.
#[derive(Debug, Clone, Copy, Default)]
pub struct DiffFilter {
    pub vehicle: Option<u32>,
    pub subsystem: Option<Subsystem>,
}

fn field_diffs(a: &VehicleRecord, b: &VehicleRecord) -> Vec<(&'static str, String, String)> {
    let mut out = Vec::new();
    let mut push = |field: &'static str, ea: String, eb: String| {
        if ea != eb {
            out.push((field, ea, eb));
        }
    };
    push("tile", format!("{:?}", a.tile), format!("{:?}", b.tile));
    push("progress", a.progress.to_string(), b.progress.to_string());
    push("dir", a.dir.to_string(), b.dir.to_string());
    push("speed", a.speed.to_string(), b.speed.to_string());
    push("subspeed", a.subspeed.to_string(), b.subspeed.to_string());
    push("state", format!("{:?}", a.state), format!("{:?}", b.state));
    push(
        "order_index",
        a.order_index.to_string(),
        b.order_index.to_string(),
    );
    push(
        "order_kind",
        format!("{:?}", a.order_kind),
        format!("{:?}", b.order_kind),
    );
    push("dest", format!("{:?}", a.dest), format!("{:?}", b.dest));
    push(
        "path_next",
        format!("{:?}", a.path_next),
        format!("{:?}", b.path_next),
    );
    push("cargo", a.cargo.to_string(), b.cargo.to_string());
    push(
        "depart_turn",
        a.depart_turn.to_string(),
        b.depart_turn.to_string(),
    );
    out
}

fn events_for_vehicle(events: &[ParityEvent], vehicle: Option<u32>) -> Vec<ParityEvent> {
    events
        .iter()
        .filter(|e| vehicle.is_none_or(|id| e.vehicle() == Some(id)))
        .cloned()
        .collect()
}

fn context_events(
    trace: &[TickRecord],
    tick: u64,
    vehicle: Option<u32>,
) -> Vec<(u64, ParityEvent)> {
    let lo = tick.saturating_sub(3);
    let hi = tick.saturating_add(3);
    trace
        .iter()
        .filter(|r| r.tick >= lo && r.tick <= hi)
        .flat_map(|r| {
            events_for_vehicle(&r.events, vehicle)
                .into_iter()
                .map(move |e| (r.tick, e))
        })
        .collect()
}

/// Compara dos trazas y devuelve la primera divergencia + resumen por subsistema.
///
/// `expected` es la traza de referencia (lado A), `actual` la observada (lado B).
#[must_use]
pub fn compare_traces(
    expected: &[TickRecord],
    actual: &[TickRecord],
    filter: DiffFilter,
) -> DiffReport {
    let mut report = DiffReport::default();
    let push_diff = |report: &mut DiffReport, d: Divergence| {
        if let Some(sub) = filter.subsystem
            && d.subsystem != sub
            && d.subsystem != Subsystem::Structure
        {
            return;
        }
        report.total += 1;
        *report.by_subsystem.entry(d.subsystem.label()).or_insert(0) += 1;
        if report.first.is_none() {
            report.first = Some(d);
        }
    };

    let by_tick_b: BTreeMap<u64, &TickRecord> = actual.iter().map(|r| (r.tick, r)).collect();

    for ra in expected {
        let Some(rb) = by_tick_b.get(&ra.tick) else {
            push_diff(
                &mut report,
                Divergence {
                    tick: ra.tick,
                    vehicle: None,
                    field: "tick".to_string(),
                    expected: format!("tick {} presente", ra.tick),
                    actual: "tick ausente".to_string(),
                    subsystem: Subsystem::Structure,
                },
            );
            continue;
        };

        let vb: BTreeMap<u32, &VehicleRecord> = rb.vehicles.iter().map(|v| (v.id, v)).collect();
        for va in &ra.vehicles {
            if filter.vehicle.is_some_and(|id| id != va.id) {
                continue;
            }
            let Some(vbr) = vb.get(&va.id) else {
                push_diff(
                    &mut report,
                    Divergence {
                        tick: ra.tick,
                        vehicle: Some(va.id),
                        field: "vehicle".to_string(),
                        expected: format!("vehículo {} presente", va.id),
                        actual: "vehículo ausente".to_string(),
                        subsystem: Subsystem::Structure,
                    },
                );
                continue;
            };
            for (field, ea, eb) in field_diffs(va, vbr) {
                push_diff(
                    &mut report,
                    Divergence {
                        tick: ra.tick,
                        vehicle: Some(va.id),
                        field: field.to_string(),
                        expected: ea,
                        actual: eb,
                        subsystem: field_subsystem(field),
                    },
                );
            }
        }

        let ev_a = events_for_vehicle(&ra.events, filter.vehicle);
        let ev_b = events_for_vehicle(&rb.events, filter.vehicle);
        if ev_a != ev_b {
            push_diff(
                &mut report,
                Divergence {
                    tick: ra.tick,
                    vehicle: filter.vehicle,
                    field: "events".to_string(),
                    expected: format!("{ev_a:?}"),
                    actual: format!("{ev_b:?}"),
                    subsystem: Subsystem::Events,
                },
            );
        }
    }

    for rb in actual {
        if !expected.iter().any(|ra| ra.tick == rb.tick) {
            push_diff(
                &mut report,
                Divergence {
                    tick: rb.tick,
                    vehicle: None,
                    field: "tick".to_string(),
                    expected: "tick ausente".to_string(),
                    actual: format!("tick {} presente", rb.tick),
                    subsystem: Subsystem::Structure,
                },
            );
        }
    }

    if let Some(first) = &report.first {
        report.context_expected = context_events(expected, first.tick, first.vehicle);
        report.context_actual = context_events(actual, first.tick, first.vehicle);
    }

    report
}

/// Reporte legible para humanos (salida de `parity_diff`).
#[must_use]
pub fn render_report(report: &DiffReport) -> String {
    let mut out = String::new();
    match &report.first {
        None => {
            out.push_str("Sin divergencias: las trazas son idénticas.\n");
        }
        Some(d) => {
            let _ = writeln!(out, "PRIMERA DIVERGENCIA");
            let _ = writeln!(out, "  tick:      {}", d.tick);
            if let Some(v) = d.vehicle {
                let _ = writeln!(out, "  vehículo:  {v}");
            }
            let _ = writeln!(out, "  campo:     {} [{}]", d.field, d.subsystem.label());
            let _ = writeln!(out, "  esperado:  {}", d.expected);
            let _ = writeln!(out, "  actual:    {}", d.actual);
            let _ = writeln!(out, "\nEventos cercanos (±3 ticks) — traza esperada:");
            if report.context_expected.is_empty() {
                let _ = writeln!(out, "  (ninguno)");
            }
            for (tick, e) in &report.context_expected {
                let _ = writeln!(out, "  t={tick}: {e:?}");
            }
            let _ = writeln!(out, "Eventos cercanos (±3 ticks) — traza actual:");
            if report.context_actual.is_empty() {
                let _ = writeln!(out, "  (ninguno)");
            }
            for (tick, e) in &report.context_actual {
                let _ = writeln!(out, "  t={tick}: {e:?}");
            }
            let _ = writeln!(
                out,
                "\nRESUMEN POR SUBSISTEMA ({} diferencias)",
                report.total
            );
            for (sub, n) in &report.by_subsystem {
                let _ = writeln!(out, "  {sub:<10} {n}");
            }
        }
    }
    out
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::super::record::TraceVehicleState;
    use super::*;
    use crate::map::TileCoord;

    fn record(tick: u64, speed: u16) -> TickRecord {
        TickRecord {
            tick,
            vehicles: vec![VehicleRecord {
                id: 1,
                tile: TileCoord::new(2, 3),
                progress: 10,
                dir: 5,
                speed,
                subspeed: 0,
                state: TraceVehicleState::Moving,
                order_index: 0,
                order_kind: Some("station".to_string()),
                dest: TileCoord::new(4, 6),
                path_next: Some(TileCoord::new(3, 3)),
                cargo: 0,
                depart_turn: 0,
                rail: None,
            }],
            events: Vec::new(),
        }
    }

    #[test]
    fn identical_traces_have_no_divergence() {
        let a = vec![record(1, 10), record(2, 20)];
        let report = compare_traces(&a, &a, DiffFilter::default());
        assert!(!report.has_divergence());
        assert_eq!(report.total, 0);
        assert!(render_report(&report).contains("Sin divergencias"));
    }

    #[test]
    fn detects_mutation_at_exact_tick_and_field() {
        let a = vec![record(1, 10), record(2, 20), record(3, 30)];
        let mut b = a.clone();
        b[1].vehicles[0].speed = 99;
        let report = compare_traces(&a, &b, DiffFilter::default());
        let first = report.first.unwrap();
        assert_eq!(first.tick, 2);
        assert_eq!(first.vehicle, Some(1));
        assert_eq!(first.field, "speed");
        assert_eq!(first.expected, "20");
        assert_eq!(first.actual, "99");
        assert_eq!(first.subsystem, Subsystem::Speed);
    }

    #[test]
    fn vehicle_filter_ignores_other_vehicles() {
        let a = vec![record(1, 10)];
        let mut b = a.clone();
        b[0].vehicles[0].speed = 5;
        let report = compare_traces(
            &a,
            &b,
            DiffFilter {
                vehicle: Some(999),
                subsystem: None,
            },
        );
        assert!(!report.has_divergence());
    }

    #[test]
    fn subsystem_filter_limits_fields() {
        let a = vec![record(1, 10)];
        let mut b = a.clone();
        b[0].vehicles[0].speed = 5;
        b[0].vehicles[0].cargo = 7;
        let report = compare_traces(
            &a,
            &b,
            DiffFilter {
                vehicle: None,
                subsystem: Some(Subsystem::Cargo),
            },
        );
        let first = report.first.unwrap();
        assert_eq!(first.field, "cargo");
        assert_eq!(report.total, 1);
    }

    #[test]
    fn missing_tick_is_structural_divergence() {
        let a = vec![record(1, 10), record(2, 20)];
        let b = vec![record(1, 10)];
        let report = compare_traces(&a, &b, DiffFilter::default());
        let first = report.first.unwrap();
        assert_eq!(first.subsystem, Subsystem::Structure);
        assert_eq!(first.tick, 2);
    }

    #[test]
    fn subsystem_parse_accepts_spanish_aliases() {
        assert_eq!(Subsystem::parse("movimiento"), Some(Subsystem::Movement));
        assert_eq!(Subsystem::parse("speed"), Some(Subsystem::Speed));
        assert_eq!(Subsystem::parse("x"), None);
    }
}
