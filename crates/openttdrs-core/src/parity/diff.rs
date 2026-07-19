//! Comparador de trazas de paridad: primera divergencia + agrupación por subsistema.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use crate::map::TileCoord;

use super::record::{ParityEvent, RailRecord, TickRecord, VehicleRecord};

/// Subsistema al que pertenece un campo divergente (para agrupar el reporte).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Subsystem {
    Movement,
    Speed,
    Orders,
    Cargo,
    Events,
    Structure,
    /// Track bits y estado de señales del mapa (Fase Rail 2).
    RailInfrastructure,
    /// Sub-teselas de las partes del tren (posición fina).
    TrainMotion,
    /// Partes del tren, cabeza/cola (trivial hoy, preparado para consist).
    ConsistGeometry,
    /// Ruta calculada (`path_next`, `PathRecomputed`).
    Pathfinding,
    /// Entrada a estación / plataforma (`at_platform`, `StationEntry`).
    StationEntry,
    /// Carga y descarga (eventos `Loading*`/`Unloading*`).
    Loading,
    /// Bloqueos por señal/tráfico y esperas (`SignalWait*`).
    Signaling,
    /// Reservas de vía (PBS): `reserved_len`, `blocked_by_reservation`, etc.
    Reservation,
    /// Entrada/salida de depósito.
    Depot,
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
            "rail_infrastructure" | "infraestructura" => Some(Self::RailInfrastructure),
            "train_motion" => Some(Self::TrainMotion),
            "consist_geometry" | "consist" => Some(Self::ConsistGeometry),
            "pathfinding" => Some(Self::Pathfinding),
            "station_entry" | "estacion" => Some(Self::StationEntry),
            "loading" => Some(Self::Loading),
            "signaling" | "senales" => Some(Self::Signaling),
            "reservation" | "reservas" => Some(Self::Reservation),
            "depot" | "deposito" => Some(Self::Depot),
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
            Self::RailInfrastructure => "rail_infrastructure",
            Self::TrainMotion => "train_motion",
            Self::ConsistGeometry => "consist_geometry",
            Self::Pathfinding => "pathfinding",
            Self::StationEntry => "station_entry",
            Self::Loading => "loading",
            Self::Signaling => "signaling",
            Self::Reservation => "reservation",
            Self::Depot => "depot",
        }
    }
}

fn field_subsystem(field: &str) -> Subsystem {
    if let Some(rest) = field.strip_prefix("rail.") {
        if rest.contains("subtile") {
            return Subsystem::TrainMotion;
        }
        return match rest {
            "track_bits_under" => Subsystem::RailInfrastructure,
            "blocked_by_signal" | "blocked_by_traffic" => Subsystem::Signaling,
            "blocked_by_reservation" | "reserved_len" | "reservation_end" => Subsystem::Reservation,
            "in_depot" => Subsystem::Depot,
            "at_platform" => Subsystem::StationEntry,
            // parts.len, parts[i].tile, parts[i].part_index, head_tile, tail_tile
            _ => Subsystem::ConsistGeometry,
        };
    }
    match field {
        "tile" | "progress" | "dir" | "depart_turn" => Subsystem::Movement,
        "speed" | "subspeed" => Subsystem::Speed,
        "cargo" => Subsystem::Cargo,
        "events" => Subsystem::Events,
        "path_next" => Subsystem::Pathfinding,
        _ => Subsystem::Orders,
    }
}

/// Subsistema de un evento (para clasificar divergencias de eventos).
const fn event_subsystem(e: &ParityEvent) -> Subsystem {
    match e {
        ParityEvent::SignalStateChanged { .. } => Subsystem::RailInfrastructure,
        ParityEvent::SignalWaitStarted { .. } | ParityEvent::SignalWaitFinished { .. } => {
            Subsystem::Signaling
        }
        ParityEvent::DepotEntry { .. } | ParityEvent::DepotExit { .. } => Subsystem::Depot,
        ParityEvent::StationEntry { .. } => Subsystem::StationEntry,
        ParityEvent::LoadingStarted { .. }
        | ParityEvent::LoadingFinished { .. }
        | ParityEvent::UnloadingStarted { .. }
        | ParityEvent::UnloadingFinished { .. } => Subsystem::Loading,
        ParityEvent::PathRecomputed { .. } => Subsystem::Pathfinding,
        _ => Subsystem::Events,
    }
}

/// Nombre estable del tipo de evento (misma convención `snake_case` del JSONL).
const fn event_type_name(e: &ParityEvent) -> &'static str {
    match e {
        ParityEvent::TileCrossed { .. } => "tile_crossed",
        ParityEvent::DirectionChanged { .. } => "direction_changed",
        ParityEvent::SpeedTrendChanged { .. } => "speed_trend_changed",
        ParityEvent::StationEntry { .. } => "station_entry",
        ParityEvent::LoadingStarted { .. } => "loading_started",
        ParityEvent::LoadingFinished { .. } => "loading_finished",
        ParityEvent::UnloadingStarted { .. } => "unloading_started",
        ParityEvent::UnloadingFinished { .. } => "unloading_finished",
        ParityEvent::Stop { .. } => "stop",
        ParityEvent::Start { .. } => "start",
        ParityEvent::DepartTurnStarted { .. } => "depart_turn_started",
        ParityEvent::DepartTurnEnded { .. } => "depart_turn_ended",
        ParityEvent::PathRecomputed { .. } => "path_recomputed",
        ParityEvent::OrderAdvanced { .. } => "order_advanced",
        ParityEvent::SignalWaitStarted { .. } => "signal_wait_started",
        ParityEvent::SignalWaitFinished { .. } => "signal_wait_finished",
        ParityEvent::DepotEntry { .. } => "depot_entry",
        ParityEvent::DepotExit { .. } => "depot_exit",
        ParityEvent::SignalStateChanged { .. } => "signal_state_changed",
    }
}

/// Teselas que menciona un evento (para el filtro `--tile`).
fn event_tiles(e: &ParityEvent) -> Vec<TileCoord> {
    match e {
        ParityEvent::TileCrossed { from, to, .. } => vec![*from, *to],
        ParityEvent::StationEntry { station, tile, .. } => vec![*station, *tile],
        ParityEvent::SignalWaitStarted { tile, .. }
        | ParityEvent::SignalWaitFinished { tile, .. }
        | ParityEvent::SignalStateChanged { tile, .. } => vec![*tile],
        ParityEvent::DepotEntry { depot, .. } | ParityEvent::DepotExit { depot, .. } => {
            vec![*depot]
        }
        _ => Vec::new(),
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
    /// Avisos no divergentes (p. ej. bloque `rail` presente en una sola traza,
    /// típico al comparar contra una traza previa a la Fase Rail 1).
    pub notes: Vec<String>,
}

impl DiffReport {
    #[must_use]
    pub const fn has_divergence(&self) -> bool {
        self.first.is_some()
    }
}

/// Tolerancia por defecto para `subtile_x/y` (medio píxel de la grilla de 16).
pub const DEFAULT_SUBTILE_EPSILON: f32 = 0.51;

/// Filtros del comparador.
#[derive(Debug, Clone)]
pub struct DiffFilter {
    pub vehicle: Option<u32>,
    pub subsystem: Option<Subsystem>,
    /// Solo divergencias que involucren esta tesela (señal, estación, segmento).
    pub tile: Option<TileCoord>,
    /// Solo divergencias de eventos de este tipo (`snake_case`, p. ej.
    /// `signal_wait_started`).
    pub event: Option<String>,
    /// Tolerancia para comparar `subtile_x/y` (el resto se compara exacto).
    pub subtile_epsilon: f32,
}

impl Default for DiffFilter {
    fn default() -> Self {
        Self {
            vehicle: None,
            subsystem: None,
            tile: None,
            event: None,
            subtile_epsilon: DEFAULT_SUBTILE_EPSILON,
        }
    }
}

type FieldDiff = (String, String, String);

fn push_ne(out: &mut Vec<FieldDiff>, field: String, ea: String, eb: String) {
    if ea != eb {
        out.push((field, ea, eb));
    }
}

fn rail_field_diffs(a: &RailRecord, b: &RailRecord, epsilon: f32) -> Vec<FieldDiff> {
    let mut out = Vec::new();
    if a.parts.len() != b.parts.len() {
        push_ne(
            &mut out,
            "rail.parts.len".to_string(),
            a.parts.len().to_string(),
            b.parts.len().to_string(),
        );
    }
    for (i, (pa, pb)) in a.parts.iter().zip(&b.parts).enumerate() {
        push_ne(
            &mut out,
            format!("rail.parts[{i}].part_index"),
            pa.part_index.to_string(),
            pb.part_index.to_string(),
        );
        push_ne(
            &mut out,
            format!("rail.parts[{i}].tile"),
            format!("{:?}", pa.tile),
            format!("{:?}", pb.tile),
        );
        // Sub-teselas: única comparación con tolerancia (posición fina de render).
        if (pa.subtile_x - pb.subtile_x).abs() > epsilon {
            out.push((
                format!("rail.parts[{i}].subtile_x"),
                format!("{:.3}", pa.subtile_x),
                format!("{:.3}", pb.subtile_x),
            ));
        }
        if (pa.subtile_y - pb.subtile_y).abs() > epsilon {
            out.push((
                format!("rail.parts[{i}].subtile_y"),
                format!("{:.3}", pa.subtile_y),
                format!("{:.3}", pb.subtile_y),
            ));
        }
    }
    push_ne(
        &mut out,
        "rail.head_tile".to_string(),
        format!("{:?}", a.head_tile),
        format!("{:?}", b.head_tile),
    );
    push_ne(
        &mut out,
        "rail.tail_tile".to_string(),
        format!("{:?}", a.tail_tile),
        format!("{:?}", b.tail_tile),
    );
    push_ne(
        &mut out,
        "rail.track_bits_under".to_string(),
        format!("{:#04x}", a.track_bits_under),
        format!("{:#04x}", b.track_bits_under),
    );
    push_ne(
        &mut out,
        "rail.blocked_by_signal".to_string(),
        a.blocked_by_signal.to_string(),
        b.blocked_by_signal.to_string(),
    );
    push_ne(
        &mut out,
        "rail.blocked_by_traffic".to_string(),
        a.blocked_by_traffic.to_string(),
        b.blocked_by_traffic.to_string(),
    );
    push_ne(
        &mut out,
        "rail.blocked_by_reservation".to_string(),
        a.blocked_by_reservation.to_string(),
        b.blocked_by_reservation.to_string(),
    );
    push_ne(
        &mut out,
        "rail.reserved_len".to_string(),
        a.reserved_len.to_string(),
        b.reserved_len.to_string(),
    );
    push_ne(
        &mut out,
        "rail.reservation_end".to_string(),
        format!("{:?}", a.reservation_end),
        format!("{:?}", b.reservation_end),
    );
    push_ne(
        &mut out,
        "rail.in_depot".to_string(),
        a.in_depot.to_string(),
        b.in_depot.to_string(),
    );
    push_ne(
        &mut out,
        "rail.at_platform".to_string(),
        a.at_platform.to_string(),
        b.at_platform.to_string(),
    );
    out
}

/// Diffs de campos entre dos registros del mismo vehículo. Devuelve además una
/// nota si el bloque `rail` existe en una sola traza (asimetría de esquema:
/// `missing_field`, no divergencia).
fn field_diffs(
    a: &VehicleRecord,
    b: &VehicleRecord,
    epsilon: f32,
) -> (Vec<FieldDiff>, Option<String>) {
    let mut out = Vec::new();
    let mut push = |field: &str, ea: String, eb: String| {
        if ea != eb {
            out.push((field.to_string(), ea, eb));
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

    let mut note = None;
    match (&a.rail, &b.rail) {
        (None, None) => {}
        (Some(ra), Some(rb)) => out.extend(rail_field_diffs(ra, rb, epsilon)),
        (Some(_), None) | (None, Some(_)) => {
            note = Some(format!(
                "vehículo {}: bloque `rail` presente en una sola traza (missing_field, \
                 no cuenta como divergencia; típico al comparar contra una traza pre-Rail 1)",
                a.id
            ));
        }
    }
    (out, note)
}

/// Teselas que involucra una divergencia de campos (para el filtro `--tile`).
fn record_tiles(a: &VehicleRecord, b: &VehicleRecord) -> Vec<TileCoord> {
    let mut tiles = vec![a.tile, b.tile, a.dest, b.dest];
    tiles.extend(a.path_next);
    tiles.extend(b.path_next);
    for r in [&a.rail, &b.rail].into_iter().flatten() {
        tiles.push(r.head_tile);
        tiles.push(r.tail_tile);
        tiles.extend(r.parts.iter().map(|p| p.tile));
    }
    tiles
}

/// Diferencia multiconjunto de eventos: (solo en A, solo en B).
fn event_multiset_diff(
    ev_a: &[ParityEvent],
    ev_b: &[ParityEvent],
) -> (Vec<ParityEvent>, Vec<ParityEvent>) {
    let mut only_a = Vec::new();
    let mut b_remaining: Vec<ParityEvent> = ev_b.to_vec();
    for e in ev_a {
        if let Some(i) = b_remaining.iter().position(|x| x == e) {
            b_remaining.remove(i);
        } else {
            only_a.push(e.clone());
        }
    }
    (only_a, b_remaining)
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

/// `true` si la divergencia pasa los filtros de subsistema/tesela/evento.
/// Las divergencias estructurales (tick/vehículo ausente) siempre pasan.
fn passes_filters(filter: &DiffFilter, d: &Divergence, tiles: &[TileCoord]) -> bool {
    if d.subsystem == Subsystem::Structure {
        return true;
    }
    if let Some(sub) = filter.subsystem
        && d.subsystem != sub
    {
        return false;
    }
    if let Some(t) = filter.tile
        && !tiles.contains(&t)
    {
        return false;
    }
    if let Some(ev) = &filter.event
        && d.field.strip_prefix("events.") != Some(ev.as_str())
    {
        return false;
    }
    true
}

fn push_event_divergences(
    report: &mut DiffReport,
    filter: &DiffFilter,
    push_diff: impl Fn(&mut DiffReport, Divergence, &[TileCoord]),
    tick: u64,
    ev_a: &[ParityEvent],
    ev_b: &[ParityEvent],
) {
    let (only_a, only_b) = event_multiset_diff(ev_a, ev_b);
    if only_a.is_empty() && only_b.is_empty() {
        if ev_a != ev_b {
            // Mismos eventos, distinto orden dentro del tick.
            push_diff(
                report,
                Divergence {
                    tick,
                    vehicle: filter.vehicle,
                    field: "events.order".to_string(),
                    expected: format!("{ev_a:?}"),
                    actual: format!("{ev_b:?}"),
                    subsystem: Subsystem::Events,
                },
                &[],
            );
        }
        return;
    }
    for (e, side) in only_a
        .iter()
        .map(|e| (e, true))
        .chain(only_b.iter().map(|e| (e, false)))
    {
        let (expected, actual) = if side {
            (format!("{e:?}"), "evento ausente".to_string())
        } else {
            ("evento ausente".to_string(), format!("{e:?}"))
        };
        push_diff(
            report,
            Divergence {
                tick,
                vehicle: e.vehicle(),
                field: format!("events.{}", event_type_name(e)),
                expected,
                actual,
                subsystem: event_subsystem(e),
            },
            &event_tiles(e),
        );
    }
}

fn rail_reservation_divergence(
    expected: &TickRecord,
    actual: &TickRecord,
    filter: &DiffFilter,
) -> Option<Divergence> {
    if filter.vehicle.is_some()
        || filter
            .subsystem
            .is_some_and(|subsystem| subsystem != Subsystem::Reservation)
        || expected.rail_reservations == actual.rail_reservations
    {
        return None;
    }
    let touches_filter_tile = filter.tile.is_none_or(|tile| {
        expected
            .rail_reservations
            .iter()
            .any(|reservation| reservation.tile == tile)
            || actual
                .rail_reservations
                .iter()
                .any(|reservation| reservation.tile == tile)
    });
    touches_filter_tile.then(|| Divergence {
        tick: expected.tick,
        vehicle: None,
        field: "rail_reservations".to_string(),
        expected: format!("{:?}", expected.rail_reservations),
        actual: format!("{:?}", actual.rail_reservations),
        subsystem: Subsystem::Reservation,
    })
}

/// Compara dos trazas y devuelve la primera divergencia + resumen por subsistema.
///
/// `expected` es la traza de referencia (lado A), `actual` la observada (lado B).
#[must_use]
pub fn compare_traces(
    expected: &[TickRecord],
    actual: &[TickRecord],
    filter: &DiffFilter,
) -> DiffReport {
    let mut report = DiffReport::default();
    let push_diff = |report: &mut DiffReport, d: Divergence, tiles: &[TileCoord]| {
        if !passes_filters(filter, &d, tiles) {
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
                &[],
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
                    &[],
                );
                continue;
            };
            let (diffs, note) = field_diffs(va, vbr, filter.subtile_epsilon);
            if let Some(note) = note
                && !report.notes.contains(&note)
            {
                report.notes.push(note);
            }
            let tiles = record_tiles(va, vbr);
            for (field, ea, eb) in diffs {
                let subsystem = field_subsystem(&field);
                push_diff(
                    &mut report,
                    Divergence {
                        tick: ra.tick,
                        vehicle: Some(va.id),
                        field,
                        expected: ea,
                        actual: eb,
                        subsystem,
                    },
                    &tiles,
                );
            }
        }

        if let Some(divergence) = rail_reservation_divergence(ra, rb, filter) {
            push_diff(&mut report, divergence, &[]);
        }

        let ev_a = events_for_vehicle(&ra.events, filter.vehicle);
        let ev_b = events_for_vehicle(&rb.events, filter.vehicle);
        push_event_divergences(&mut report, filter, push_diff, ra.tick, &ev_a, &ev_b);
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
                &[],
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
    for note in &report.notes {
        let _ = writeln!(out, "NOTA: {note}");
    }
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
            rail_reservations: Vec::new(),
        }
    }

    #[test]
    fn identical_traces_have_no_divergence() {
        let a = vec![record(1, 10), record(2, 20)];
        let report = compare_traces(&a, &a, &DiffFilter::default());
        assert!(!report.has_divergence());
        assert_eq!(report.total, 0);
        assert!(render_report(&report).contains("Sin divergencias"));
    }

    #[test]
    fn detects_mutation_at_exact_tick_and_field() {
        let a = vec![record(1, 10), record(2, 20), record(3, 30)];
        let mut b = a.clone();
        b[1].vehicles[0].speed = 99;
        let report = compare_traces(&a, &b, &DiffFilter::default());
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
            &DiffFilter {
                vehicle: Some(999),
                ..Default::default()
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
            &DiffFilter {
                subsystem: Some(Subsystem::Cargo),
                ..Default::default()
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
        let report = compare_traces(&a, &b, &DiffFilter::default());
        let first = report.first.unwrap();
        assert_eq!(first.subsystem, Subsystem::Structure);
        assert_eq!(first.tick, 2);
    }

    #[test]
    fn subsystem_parse_accepts_spanish_aliases() {
        assert_eq!(Subsystem::parse("movimiento"), Some(Subsystem::Movement));
        assert_eq!(Subsystem::parse("speed"), Some(Subsystem::Speed));
        assert_eq!(Subsystem::parse("senales"), Some(Subsystem::Signaling));
        assert_eq!(
            Subsystem::parse("rail_infrastructure"),
            Some(Subsystem::RailInfrastructure)
        );
        assert_eq!(Subsystem::parse("x"), None);
    }

    // ---- Fase Rail 2: fixtures artificiales con una divergencia por subsistema ----

    use super::super::record::{RailPartRecord, RailRecord, RailReservationRecord};

    fn rail_block() -> RailRecord {
        RailRecord {
            parts: vec![RailPartRecord {
                part_index: 0,
                tile: TileCoord::new(2, 3),
                subtile_x: 7.5,
                subtile_y: 8.0,
            }],
            head_tile: TileCoord::new(2, 3),
            tail_tile: TileCoord::new(2, 3),
            track_bits_under: 0x01,
            blocked_by_signal: false,
            blocked_by_traffic: false,
            blocked_by_reservation: false,
            reserved_len: 0,
            reservation_end: None,
            in_depot: false,
            at_platform: false,
        }
    }

    fn train_record(tick: u64) -> TickRecord {
        let mut r = record(tick, 20);
        r.vehicles[0].rail = Some(rail_block());
        r
    }

    /// Compara dos trazas de 1 tick donde `mutate` rompe el lado B.
    fn diff_one(mutate: impl FnOnce(&mut TickRecord)) -> DiffReport {
        let a = vec![train_record(1)];
        let mut b = a.clone();
        mutate(&mut b[0]);
        compare_traces(&a, &b, &DiffFilter::default())
    }

    #[test]
    fn track_bits_divergence_is_rail_infrastructure() {
        let report = diff_one(|r| r.vehicles[0].rail.as_mut().unwrap().track_bits_under = 0x08);
        let first = report.first.unwrap();
        assert_eq!(first.subsystem, Subsystem::RailInfrastructure);
        assert_eq!(first.field, "rail.track_bits_under");
        assert_eq!(first.expected, "0x01");
        assert_eq!(first.actual, "0x08");
    }

    #[test]
    fn map_reservation_divergence_is_reservation_subsystem() {
        let a = vec![train_record(1)];
        let mut b = a.clone();
        b[0].rail_reservations.push(RailReservationRecord {
            tile: TileCoord::new(3, 3),
            track_bits: 0x01,
        });
        let report = compare_traces(&a, &b, &DiffFilter::default());
        let first = report.first.unwrap();
        assert_eq!(first.subsystem, Subsystem::Reservation);
        assert_eq!(first.field, "rail_reservations");
    }

    #[test]
    fn subtile_divergence_is_train_motion_and_respects_epsilon() {
        // Dentro del epsilon por defecto (0.51): sin divergencia.
        let report = diff_one(|r| r.vehicles[0].rail.as_mut().unwrap().parts[0].subtile_x = 7.9);
        assert!(!report.has_divergence(), "0.4 < 0.51 no debe divergir");

        // Fuera del epsilon: train_motion.
        let report = diff_one(|r| r.vehicles[0].rail.as_mut().unwrap().parts[0].subtile_x = 9.0);
        let first = report.first.unwrap();
        assert_eq!(first.subsystem, Subsystem::TrainMotion);
        assert_eq!(first.field, "rail.parts[0].subtile_x");

        // Epsilon explícito más estricto: 0.4 sí diverge.
        let a = vec![train_record(1)];
        let mut b = a.clone();
        b[0].vehicles[0].rail.as_mut().unwrap().parts[0].subtile_y = 8.4;
        let report = compare_traces(
            &a,
            &b,
            &DiffFilter {
                subtile_epsilon: 0.1,
                ..Default::default()
            },
        );
        assert_eq!(report.first.unwrap().field, "rail.parts[0].subtile_y");
    }

    #[test]
    fn consist_divergence_is_consist_geometry() {
        let report =
            diff_one(|r| r.vehicles[0].rail.as_mut().unwrap().tail_tile = TileCoord::new(9, 9));
        let first = report.first.unwrap();
        assert_eq!(first.subsystem, Subsystem::ConsistGeometry);
        assert_eq!(first.field, "rail.tail_tile");
    }

    #[test]
    fn path_next_divergence_is_pathfinding() {
        let report = diff_one(|r| r.vehicles[0].path_next = Some(TileCoord::new(8, 8)));
        let first = report.first.unwrap();
        assert_eq!(first.subsystem, Subsystem::Pathfinding);
        assert_eq!(first.field, "path_next");
    }

    #[test]
    fn at_platform_divergence_is_station_entry() {
        let report = diff_one(|r| r.vehicles[0].rail.as_mut().unwrap().at_platform = true);
        let first = report.first.unwrap();
        assert_eq!(first.subsystem, Subsystem::StationEntry);
        assert_eq!(first.field, "rail.at_platform");
    }

    #[test]
    fn missing_loading_event_is_loading_subsystem() {
        let report = diff_one(|r| {
            r.events.push(ParityEvent::LoadingStarted {
                vehicle: 1,
                before: 0,
                after: 5,
            });
        });
        let first = report.first.unwrap();
        assert_eq!(first.subsystem, Subsystem::Loading);
        assert_eq!(first.field, "events.loading_started");
        assert_eq!(first.expected, "evento ausente");
        assert_eq!(first.vehicle, Some(1));
    }

    #[test]
    fn blocked_by_signal_divergence_is_signaling() {
        let report = diff_one(|r| r.vehicles[0].rail.as_mut().unwrap().blocked_by_signal = true);
        let first = report.first.unwrap();
        assert_eq!(first.subsystem, Subsystem::Signaling);
        assert_eq!(first.field, "rail.blocked_by_signal");
    }

    #[test]
    fn in_depot_divergence_is_depot_subsystem() {
        let report = diff_one(|r| r.vehicles[0].rail.as_mut().unwrap().in_depot = true);
        let first = report.first.unwrap();
        assert_eq!(first.subsystem, Subsystem::Depot);
        assert_eq!(first.field, "rail.in_depot");
    }

    #[test]
    fn signal_state_event_is_rail_infrastructure_and_tile_filterable() {
        let signal_tile = TileCoord::new(7, 6);
        let mutate = |r: &mut TickRecord| {
            r.events.push(ParityEvent::SignalStateChanged {
                tile: signal_tile,
                track_mask: 1,
                green: false,
            });
        };

        let report = diff_one(mutate);
        let first = report.first.unwrap();
        assert_eq!(first.subsystem, Subsystem::RailInfrastructure);
        assert_eq!(first.field, "events.signal_state_changed");
        assert_eq!(first.vehicle, None);

        // Filtro --tile: la tesela de la señal la incluye…
        let a = vec![train_record(1)];
        let mut b = a.clone();
        mutate(&mut b[0]);
        let report = compare_traces(
            &a,
            &b,
            &DiffFilter {
                tile: Some(signal_tile),
                ..Default::default()
            },
        );
        assert!(report.has_divergence());
        // …y otra tesela cualquiera la excluye.
        let report = compare_traces(
            &a,
            &b,
            &DiffFilter {
                tile: Some(TileCoord::new(0, 0)),
                ..Default::default()
            },
        );
        assert!(!report.has_divergence());
    }

    #[test]
    fn event_filter_selects_single_event_type() {
        let a = vec![train_record(1)];
        let mut b = a.clone();
        // Dos divergencias: un evento de señal y una de velocidad.
        b[0].events.push(ParityEvent::SignalWaitStarted {
            vehicle: 1,
            tile: TileCoord::new(2, 3),
        });
        b[0].vehicles[0].speed = 99;

        let report = compare_traces(
            &a,
            &b,
            &DiffFilter {
                event: Some("signal_wait_started".to_string()),
                ..Default::default()
            },
        );
        assert_eq!(report.total, 1, "solo la divergencia del evento pedido");
        assert_eq!(report.first.unwrap().subsystem, Subsystem::Signaling);
    }

    #[test]
    fn rail_block_on_one_side_is_note_not_divergence() {
        // Simula comparar una traza pre-Rail 1 (sin bloque rail) contra una nueva.
        let old = vec![record(1, 20)];
        let mut new = old.clone();
        new[0].vehicles[0].rail = Some(rail_block());
        let report = compare_traces(&old, &new, &DiffFilter::default());
        assert!(
            !report.has_divergence(),
            "missing_field no es divergencia: {:?}",
            report.first
        );
        assert_eq!(report.notes.len(), 1);
        assert!(report.notes[0].contains("una sola traza"));
        assert!(render_report(&report).contains("NOTA"));
    }

    #[test]
    fn jsonl_roundtrip_of_rail_fixture_keeps_comparison_exact() {
        // El mismo par de trazas comparado tras pasar por JSONL (camino de la CLI).
        let a = vec![train_record(1), train_record(2)];
        let mut b = a.clone();
        b[1].vehicles[0].rail.as_mut().unwrap().track_bits_under = 0x20;

        let mut buf_a = Vec::new();
        let mut buf_b = Vec::new();
        super::super::write_jsonl(&a, &mut buf_a).unwrap();
        super::super::write_jsonl(&b, &mut buf_b).unwrap();
        let a2 = super::super::read_jsonl(std::io::Cursor::new(buf_a)).unwrap();
        let b2 = super::super::read_jsonl(std::io::Cursor::new(buf_b)).unwrap();

        let report = compare_traces(&a2, &b2, &DiffFilter::default());
        let first = report.first.unwrap();
        assert_eq!(first.tick, 2);
        assert_eq!(first.subsystem, Subsystem::RailInfrastructure);
        assert_eq!(*report.by_subsystem.get("rail_infrastructure").unwrap(), 1);
    }
}
