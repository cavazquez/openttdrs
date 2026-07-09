//! Tracer de paridad: captura un [`TickRecord`] por tick de simulación.
//!
//! Diseño no invasivo: en lugar de hooks dentro de la lógica de vehículos, el
//! tracer compara el estado al final de cada tick con el del tick anterior y
//! deriva los eventos por diferencia. Un único punto de instrumentación en
//! `sim_step::step` y coste cero cuando `GameState::parity` es `None`.

use std::collections::BTreeMap;

use crate::map::{Map, TileCoord, TileKind};
use crate::vehicle::VehicleKind;
use crate::{GameState, rail_signals, refit, road_movement, station};

use super::record::{
    ParityEvent, RailPartRecord, RailRecord, SpeedTrend, TickRecord, VehicleRecord,
    derive_vehicle_state, order_kind_name,
};

/// Estado mínimo del tick anterior para derivar eventos por diff.
#[derive(Debug, Clone)]
#[allow(clippy::struct_excessive_bools)]
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
    /// Solo trenes (Fase Rail 1): retención por señal e interior de depósito.
    blocked_by_signal: bool,
    in_depot: bool,
    /// Carga/descarga gradual en curso el tick anterior.
    cargo_transfer_was_active: bool,
}

/// Acumulador de trazas de paridad (vive en `GameState` con `#[serde(skip)]`).
#[derive(Debug, Clone, Default)]
pub struct ParityTracer {
    records: Vec<TickRecord>,
    prev: BTreeMap<u32, PrevVehicle>,
    /// Estado verde/rojo por señal (`state_mask & present_mask` de cada tesela
    /// `Rail` con señales) para derivar `SignalStateChanged` por diff.
    signal_states: BTreeMap<TileCoord, u8>,
}

impl ParityTracer {
    /// Crea un tracer con la línea base del estado actual (los eventos del
    /// primer tick se derivan contra este estado, no contra un vacío).
    #[must_use]
    pub fn with_baseline(state: &GameState) -> Self {
        Self {
            records: Vec::new(),
            prev: capture_prev(state),
            signal_states: capture_signal_states(&state.map),
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

/// Track bits transitables bajo la tesela (misma convención que el pathfinder:
/// túnel/puente ferroviario cuentan como `X|Y`; depósito/estación → 0).
fn track_bits_under(map: &Map, pos: TileCoord) -> u8 {
    match map.get(pos) {
        Some(t) if t.kind == TileKind::Rail => t.m5 & 0x3F,
        Some(t) if matches!(t.kind, TileKind::RailTunnel | TileKind::RailBridge) => 0x03,
        _ => 0,
    }
}

/// `true` si el tren está sobre una plataforma de estación ferroviaria
/// (tipo rail = 0 en `m6`).
fn train_at_rail_platform(map: &Map, pos: TileCoord) -> bool {
    station::train_on_rail_platform(map, pos)
}

/// Espeja la decisión de bloqueo por señal de `sim_step::move_vehicles` para
/// el próximo avance (solo lectura, cero efectos sobre la sim). Con
/// `force_proceed` la señal se ignora, igual que en la sim.
fn rail_blocked_by_signal(
    state: &GameState,
    _train_positions: &[TileCoord],
    v: &crate::Vehicle,
) -> bool {
    if !v.running || v.force_proceed {
        return false;
    }
    if v.movement_target().is_none() {
        return false;
    }
    rail_signals::train_blocked_by_signal(&state.map, &state.vehicles, v)
}

/// Bloque ferroviario del registro (solo cabezas de tren; `None` para el resto).
fn rail_snapshot(
    state: &GameState,
    train_positions: &[TileCoord],
    v: &crate::Vehicle,
) -> Option<RailRecord> {
    if v.kind != VehicleKind::Train || !v.is_consist_head() {
        return None;
    }
    let (subtile_x, subtile_y) = road_movement::vehicle_subtile(v);
    let occupied = crate::train_consist::consist_occupied_tiles(&state.vehicles, v.id);
    let parts: Vec<RailPartRecord> = occupied
        .iter()
        .enumerate()
        .map(|(i, tile)| RailPartRecord {
            part_index: i,
            tile: *tile,
            subtile_x: if i == 0 { subtile_x } else { 128.0 },
            subtile_y: if i == 0 { subtile_y } else { 128.0 },
        })
        .collect();
    let tail_tile = occupied.last().copied().unwrap_or(v.pos);
    Some(RailRecord {
        parts,
        head_tile: v.pos,
        tail_tile,
        track_bits_under: track_bits_under(&state.map, v.pos),
        blocked_by_signal: rail_blocked_by_signal(state, train_positions, v),
        blocked_by_traffic: rail_signals::train_blocked_by_traffic(&state.map, &state.vehicles, v),
        in_depot: refit::vehicle_in_depot(&state.map, v.pos),
        at_platform: train_at_rail_platform(&state.map, v.pos),
    })
}

fn train_positions(state: &GameState) -> Vec<TileCoord> {
    state
        .vehicles
        .iter()
        .filter(|v| v.kind == VehicleKind::Train && v.is_consist_head())
        .map(|v| v.pos)
        .collect()
}

/// Estado verde/rojo (`m3hi`, enmascarado por señales presentes) de cada
/// tesela `Rail` con señales del mapa.
fn capture_signal_states(map: &Map) -> BTreeMap<TileCoord, u8> {
    let (w, h) = map.dimensions();
    let mut out = BTreeMap::new();
    for y in 0..i32::try_from(h).unwrap_or(i32::MAX) {
        for x in 0..i32::try_from(w).unwrap_or(i32::MAX) {
            let c = TileCoord::new(x, y);
            let Some(t) = map.get(c) else {
                continue;
            };
            if t.kind != TileKind::Rail || !rail_signals::rail_tile_is_signals(t.m5) {
                continue;
            }
            let present = rail_signals::rail_signal_present_mask(t.m3);
            if present == 0 {
                continue;
            }
            out.insert(c, rail_signals::rail_signal_state_mask(t.m3hi) & present);
        }
    }
    out
}

/// Deriva `SignalStateChanged` comparando el estado de señales con el tick
/// anterior; actualiza `prev` con el estado actual.
fn signal_state_events(map: &Map, prev: &mut BTreeMap<TileCoord, u8>) -> Vec<ParityEvent> {
    let now = capture_signal_states(map);
    let mut events = Vec::new();
    for (&tile, &mask) in &now {
        let old = prev.get(&tile).copied().unwrap_or(0);
        let changed = old ^ mask;
        for bit in 0..4u8 {
            if changed & (1 << bit) != 0 {
                events.push(ParityEvent::SignalStateChanged {
                    tile,
                    track_mask: 1 << bit,
                    green: mask & (1 << bit) != 0,
                });
            }
        }
    }
    *prev = now;
    events
}

fn capture_prev(state: &GameState) -> BTreeMap<u32, PrevVehicle> {
    let trains = train_positions(state);
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
                    blocked_by_signal: rail_blocked_by_signal(state, &trains, v),
                    in_depot: v.kind == VehicleKind::Train
                        && refit::vehicle_in_depot(&state.map, v.pos),
                    cargo_transfer_was_active: v.cargo_transfer_active(),
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
        if !p.cargo_transfer_was_active {
            events.push(ParityEvent::LoadingStarted {
                vehicle: v.id,
                before: p.cargo,
                after: v.cargo,
            });
        }
        // Carga gradual: `LoadingFinished` solo al cerrar la transferencia.
        if !v.cargo_transfer_active() {
            events.push(ParityEvent::LoadingFinished {
                vehicle: v.id,
                cargo: v.cargo,
            });
        }
    } else if p.cargo_transfer_was_active && !v.cargo_transfer_active() && v.cargo >= p.cargo {
        // Terminó la carga sin más unidades este tick (p. ej. cola vacía).
        events.push(ParityEvent::LoadingFinished {
            vehicle: v.id,
            cargo: v.cargo,
        });
    }
    if v.cargo < p.cargo {
        if !p.cargo_transfer_was_active {
            events.push(ParityEvent::UnloadingStarted {
                vehicle: v.id,
                before: p.cargo,
                after: v.cargo,
            });
        }
        if !v.cargo_transfer_active() && v.cargo == 0 {
            events.push(ParityEvent::UnloadingFinished { vehicle: v.id });
        }
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

/// Transiciones ferroviarias: espera por señal y entrada/salida de depósito.
fn push_rail_events(
    events: &mut Vec<ParityEvent>,
    v: &crate::Vehicle,
    p: &PrevVehicle,
    rail: Option<&RailRecord>,
) {
    let Some(r) = rail else {
        return;
    };
    if !p.blocked_by_signal && r.blocked_by_signal {
        events.push(ParityEvent::SignalWaitStarted {
            vehicle: v.id,
            tile: v.pos,
        });
    }
    if p.blocked_by_signal && !r.blocked_by_signal {
        events.push(ParityEvent::SignalWaitFinished {
            vehicle: v.id,
            tile: p.pos,
        });
    }
    if !p.in_depot && r.in_depot {
        events.push(ParityEvent::DepotEntry {
            vehicle: v.id,
            depot: v.pos,
        });
    }
    if p.in_depot && !r.in_depot {
        events.push(ParityEvent::DepotExit {
            vehicle: v.id,
            depot: p.pos,
        });
    }
}

fn diff_events(
    state: &GameState,
    prev: &mut BTreeMap<u32, PrevVehicle>,
    rails: &BTreeMap<u32, RailRecord>,
) -> Vec<ParityEvent> {
    let mut events = Vec::new();
    for v in &state.vehicles {
        let at_station = vehicle_station_tile(state, v);
        let rail = rails.get(&v.id);
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
                    blocked_by_signal: rail.is_some_and(|r| r.blocked_by_signal),
                    in_depot: rail.is_some_and(|r| r.in_depot),
                    cargo_transfer_was_active: v.cargo_transfer_active(),
                },
            );
            continue;
        };

        push_movement_events(&mut events, v, p);
        push_speed_events(&mut events, v, p);
        push_cargo_and_order_events(&mut events, v, p, at_station);
        push_rail_events(&mut events, v, p, rail);

        p.pos = v.pos;
        p.dir = v.direction;
        p.speed = v.cur_speed;
        p.cargo = v.cargo;
        p.depart_turn = v.depart_turn;
        p.path_was_empty = v.path.is_empty();
        p.order_index = v.current_order;
        p.at_station = at_station;
        p.blocked_by_signal = rail.is_some_and(|r| r.blocked_by_signal);
        p.in_depot = rail.is_some_and(|r| r.in_depot);
        p.cargo_transfer_was_active = v.cargo_transfer_active();
    }
    events
}

fn vehicle_record(v: &crate::Vehicle, rail: Option<RailRecord>) -> VehicleRecord {
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
        rail,
    }
}

/// Registra el tick recién ejecutado (llamado al final de `sim_step::step`).
///
/// No hace nada si la traza está desactivada (`state.parity == None`).
pub(crate) fn record_tick(state: &mut GameState) {
    let Some(mut tracer) = state.parity.take() else {
        return;
    };
    let trains = train_positions(state);
    let rails: BTreeMap<u32, RailRecord> = state
        .vehicles
        .iter()
        .filter_map(|v| rail_snapshot(state, &trains, v).map(|r| (v.id, r)))
        .collect();
    let mut events = diff_events(state, &mut tracer.prev, &rails);
    events.extend(signal_state_events(&state.map, &mut tracer.signal_states));
    let vehicles = state
        .vehicles
        .iter()
        .map(|v| vehicle_record(v, rails.get(&v.id).cloned()))
        .collect();
    tracer.records.push(TickRecord {
        tick: state.tick.get(),
        vehicles,
        events,
    });
    state.parity = Some(tracer);
}
