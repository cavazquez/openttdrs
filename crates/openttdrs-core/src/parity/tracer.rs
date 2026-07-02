//! Tracer de paridad: captura un [`TickRecord`] por tick de simulación.
//!
//! Diseño no invasivo: en lugar de hooks dentro de la lógica de vehículos, el
//! tracer compara el estado al final de cada tick con el del tick anterior y
//! deriva los eventos por diferencia. Un único punto de instrumentación en
//! `sim_step::step` y coste cero cuando `GameState::parity` es `None`.

use std::collections::BTreeMap;

use crate::GameState;
use crate::map::TileCoord;
use crate::station;

use super::record::{
    ParityEvent, SpeedTrend, TickRecord, VehicleRecord, derive_vehicle_state, order_kind_name,
};

/// Estado mínimo del tick anterior para derivar eventos por diff.
#[derive(Debug, Clone)]
struct PrevVehicle {
    pos: TileCoord,
    dir: u8,
    speed: u16,
    cargo: u32,
    depart_turn: u8,
    path_was_empty: bool,
    order_index: usize,
    at_station: Option<TileCoord>,
    /// Última tendencia de velocidad observada (+1 acelera, −1 frena, 0 desconocida).
    trend: i8,
}

/// Acumulador de trazas de paridad (vive en `GameState` con `#[serde(skip)]`).
#[derive(Debug, Clone, Default)]
pub struct ParityTracer {
    records: Vec<TickRecord>,
    prev: BTreeMap<u32, PrevVehicle>,
}

impl ParityTracer {
    /// Crea un tracer con la línea base del estado actual (los eventos del
    /// primer tick se derivan contra este estado, no contra un vacío).
    #[must_use]
    pub fn with_baseline(state: &GameState) -> Self {
        Self {
            records: Vec::new(),
            prev: capture_prev(state),
        }
    }

    /// Registros acumulados hasta ahora.
    #[must_use]
    pub fn records(&self) -> &[TickRecord] {
        &self.records
    }

    /// Extrae y vacía los registros acumulados.
    pub fn drain_records(&mut self) -> Vec<TickRecord> {
        std::mem::take(&mut self.records)
    }
}

fn vehicle_station_tile(state: &GameState, v: &crate::Vehicle) -> Option<TileCoord> {
    state
        .stations
        .iter()
        .find(|s| station::vehicle_physically_at_station(&state.map, v, s))
        .map(|s| s.pos)
}

fn capture_prev(state: &GameState) -> BTreeMap<u32, PrevVehicle> {
    state
        .vehicles
        .iter()
        .map(|v| {
            (
                v.id,
                PrevVehicle {
                    pos: v.pos,
                    dir: v.direction,
                    speed: v.cur_speed,
                    cargo: v.cargo,
                    depart_turn: v.depart_turn,
                    path_was_empty: v.path.is_empty(),
                    order_index: v.current_order,
                    at_station: vehicle_station_tile(state, v),
                    trend: 0,
                },
            )
        })
        .collect()
}

fn push_movement_events(events: &mut Vec<ParityEvent>, v: &crate::Vehicle, p: &PrevVehicle) {
    if v.pos != p.pos {
        events.push(ParityEvent::TileCrossed {
            vehicle: v.id,
            from: p.pos,
            to: v.pos,
        });
    }
    if v.direction != p.dir {
        events.push(ParityEvent::DirectionChanged {
            vehicle: v.id,
            from: p.dir,
            to: v.direction,
        });
    }
    if p.depart_turn == 0 && v.depart_turn > 0 {
        events.push(ParityEvent::DepartTurnStarted { vehicle: v.id });
    }
    if p.depart_turn > 0 && v.depart_turn == 0 {
        events.push(ParityEvent::DepartTurnEnded { vehicle: v.id });
    }
}

fn push_speed_events(events: &mut Vec<ParityEvent>, v: &crate::Vehicle, p: &mut PrevVehicle) {
    if p.speed > 0 && v.cur_speed == 0 {
        events.push(ParityEvent::Stop { vehicle: v.id });
    }
    if p.speed == 0 && v.cur_speed > 0 {
        events.push(ParityEvent::Start { vehicle: v.id });
    }
    let delta = i32::from(v.cur_speed) - i32::from(p.speed);
    if delta != 0 {
        let trend: i8 = if delta > 0 { 1 } else { -1 };
        if p.trend != 0 && trend != p.trend {
            events.push(ParityEvent::SpeedTrendChanged {
                vehicle: v.id,
                trend: if trend > 0 {
                    SpeedTrend::Accelerating
                } else {
                    SpeedTrend::Decelerating
                },
                speed: v.cur_speed,
            });
        }
        p.trend = trend;
    }
}

fn push_cargo_and_order_events(
    events: &mut Vec<ParityEvent>,
    v: &crate::Vehicle,
    p: &PrevVehicle,
    at_station: Option<TileCoord>,
) {
    if let Some(station_pos) = at_station
        && p.at_station != Some(station_pos)
    {
        events.push(ParityEvent::StationEntry {
            vehicle: v.id,
            station: station_pos,
            tile: v.pos,
        });
    }
    if v.cargo > p.cargo {
        events.push(ParityEvent::LoadingStarted {
            vehicle: v.id,
            before: p.cargo,
            after: v.cargo,
        });
        // La carga en la sim actual es instantánea (un tick); en OpenTTD es
        // gradual. Se emiten ambos eventos para conservar el esquema.
        events.push(ParityEvent::LoadingFinished {
            vehicle: v.id,
            cargo: v.cargo,
        });
    }
    if v.cargo < p.cargo {
        events.push(ParityEvent::UnloadingStarted {
            vehicle: v.id,
            before: p.cargo,
            after: v.cargo,
        });
        events.push(ParityEvent::UnloadingFinished { vehicle: v.id });
    }
    if p.path_was_empty && !v.path.is_empty() {
        events.push(ParityEvent::PathRecomputed {
            vehicle: v.id,
            len: v.path.len(),
        });
    }
    if v.current_order != p.order_index {
        events.push(ParityEvent::OrderAdvanced {
            vehicle: v.id,
            from: p.order_index,
            to: v.current_order,
        });
    }
}

fn diff_events(state: &GameState, prev: &mut BTreeMap<u32, PrevVehicle>) -> Vec<ParityEvent> {
    let mut events = Vec::new();
    for v in &state.vehicles {
        let at_station = vehicle_station_tile(state, v);
        let Some(p) = prev.get_mut(&v.id) else {
            prev.insert(
                v.id,
                PrevVehicle {
                    pos: v.pos,
                    dir: v.direction,
                    speed: v.cur_speed,
                    cargo: v.cargo,
                    depart_turn: v.depart_turn,
                    path_was_empty: v.path.is_empty(),
                    order_index: v.current_order,
                    at_station,
                    trend: 0,
                },
            );
            continue;
        };

        push_movement_events(&mut events, v, p);
        push_speed_events(&mut events, v, p);
        push_cargo_and_order_events(&mut events, v, p, at_station);

        p.pos = v.pos;
        p.dir = v.direction;
        p.speed = v.cur_speed;
        p.cargo = v.cargo;
        p.depart_turn = v.depart_turn;
        p.path_was_empty = v.path.is_empty();
        p.order_index = v.current_order;
        p.at_station = at_station;
    }
    events
}

fn vehicle_record(v: &crate::Vehicle) -> VehicleRecord {
    VehicleRecord {
        id: v.id,
        tile: v.pos,
        progress: v.progress,
        dir: v.direction,
        speed: v.cur_speed,
        subspeed: v.subspeed,
        state: derive_vehicle_state(v),
        order_index: v.current_order,
        order_kind: v
            .orders
            .get(v.current_order)
            .map(|o| order_kind_name(o).to_string()),
        dest: v.dest,
        path_next: v.path.front().copied(),
        cargo: v.cargo,
        depart_turn: v.depart_turn,
    }
}

/// Registra el tick recién ejecutado (llamado al final de `sim_step::step`).
///
/// No hace nada si la traza está desactivada (`state.parity == None`).
pub(crate) fn record_tick(state: &mut GameState) {
    let Some(mut tracer) = state.parity.take() else {
        return;
    };
    let events = diff_events(state, &mut tracer.prev);
    let vehicles = state.vehicles.iter().map(vehicle_record).collect();
    tracer.records.push(TickRecord {
        tick: state.tick.get(),
        vehicles,
        events,
    });
    state.parity = Some(tracer);
}
