//! Esquema de la traza de paridad (registros por tick + eventos discretos).
//!
//! El formato de salida es JSONL: una línea JSON por [`TickRecord`], apta para
//! diff línea a línea y para el comparador de primera divergencia.

use crate::map::TileCoord;
use crate::vehicle::{Vehicle, VehicleOrder};

/// Estado derivado del vehículo (no existe como enum en la sim; se deriva de campos).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TraceVehicleState {
    /// `running == false` (apagado / en depósito con stop).
    Stopped,
    /// Con órdenes pero sin ruta por red (`no_network_route_to_order`).
    Blocked,
    /// Espera de horario activa (`timetable_wait_remaining > 0`).
    WaitingTimetable,
    /// Animación de media vuelta al salir de una parada (`depart_turn > 0`).
    DepartTurn,
    /// Anclado en el destino (`progress == 255`, esperando carga/descarga/full load).
    Holding,
    /// En movimiento (`cur_speed > 0` con objetivo de movimiento).
    Moving,
    /// Sin objetivo, frenando hasta detenerse (`cur_speed > 0`).
    Decelerating,
    /// Sin objetivo y sin velocidad.
    Idle,
}

/// Instantánea de un vehículo al final de un tick de simulación.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct VehicleRecord {
    pub id: u32,
    pub tile: TileCoord,
    pub progress: u8,
    pub dir: u8,
    pub speed: u16,
    pub subspeed: u8,
    pub state: TraceVehicleState,
    pub order_index: usize,
    /// Tipo de la orden actual (`station`/`waypoint`/`depot`/`tile`/`conditional`).
    pub order_kind: Option<String>,
    pub dest: TileCoord,
    /// Siguiente tesela del path calculado (frente de `Vehicle::path`).
    pub path_next: Option<TileCoord>,
    pub cargo: u32,
    pub depart_turn: u8,
}

/// Tendencia de velocidad (para detectar inicio de aceleración/frenado).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpeedTrend {
    Accelerating,
    Decelerating,
}

/// Evento discreto detectado al comparar el estado antes/después de un tick.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ParityEvent {
    TileCrossed {
        vehicle: u32,
        from: TileCoord,
        to: TileCoord,
    },
    DirectionChanged {
        vehicle: u32,
        from: u8,
        to: u8,
    },
    /// La velocidad cambió de tendencia (p. ej. empezó a frenar).
    SpeedTrendChanged {
        vehicle: u32,
        trend: SpeedTrend,
        speed: u16,
    },
    /// El vehículo pasó a estar físicamente en una parada que sirve a su tipo.
    StationEntry {
        vehicle: u32,
        station: TileCoord,
        tile: TileCoord,
    },
    LoadingStarted {
        vehicle: u32,
        before: u32,
        after: u32,
    },
    LoadingFinished {
        vehicle: u32,
        cargo: u32,
    },
    UnloadingStarted {
        vehicle: u32,
        before: u32,
        after: u32,
    },
    UnloadingFinished {
        vehicle: u32,
    },
    /// `cur_speed` llegó a 0 viniendo de movimiento.
    Stop {
        vehicle: u32,
    },
    /// `cur_speed` dejó de ser 0.
    Start {
        vehicle: u32,
    },
    DepartTurnStarted {
        vehicle: u32,
    },
    DepartTurnEnded {
        vehicle: u32,
    },
    /// El pathfinder asignó ruta a un vehículo que no tenía (`path` vacío → no vacío).
    PathRecomputed {
        vehicle: u32,
        len: usize,
    },
    /// La orden activa cambió de índice.
    OrderAdvanced {
        vehicle: u32,
        from: usize,
        to: usize,
    },
}

impl ParityEvent {
    /// Vehículo al que refiere el evento.
    #[must_use]
    pub const fn vehicle(&self) -> u32 {
        match self {
            Self::TileCrossed { vehicle, .. }
            | Self::DirectionChanged { vehicle, .. }
            | Self::SpeedTrendChanged { vehicle, .. }
            | Self::StationEntry { vehicle, .. }
            | Self::LoadingStarted { vehicle, .. }
            | Self::LoadingFinished { vehicle, .. }
            | Self::UnloadingStarted { vehicle, .. }
            | Self::UnloadingFinished { vehicle }
            | Self::Stop { vehicle }
            | Self::Start { vehicle }
            | Self::DepartTurnStarted { vehicle }
            | Self::DepartTurnEnded { vehicle }
            | Self::PathRecomputed { vehicle, .. }
            | Self::OrderAdvanced { vehicle, .. } => *vehicle,
        }
    }
}

/// Registro completo de un tick de simulación (una línea del JSONL).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TickRecord {
    pub tick: u64,
    pub vehicles: Vec<VehicleRecord>,
    pub events: Vec<ParityEvent>,
}

/// Nombre estable del tipo de orden para la traza.
#[must_use]
pub fn order_kind_name(order: &VehicleOrder) -> &'static str {
    match order {
        VehicleOrder::Station { .. } => "station",
        VehicleOrder::Waypoint { .. } => "waypoint",
        VehicleOrder::Depot { .. } => "depot",
        VehicleOrder::Tile(_) => "tile",
        VehicleOrder::Conditional { .. } => "conditional",
    }
}

/// Deriva el estado observable del vehículo a partir de sus campos.
#[must_use]
pub fn derive_vehicle_state(v: &Vehicle) -> TraceVehicleState {
    if !v.running {
        return TraceVehicleState::Stopped;
    }
    if v.no_network_route_to_order {
        return TraceVehicleState::Blocked;
    }
    if v.timetable_active && v.timetable_wait_remaining > 0 {
        return TraceVehicleState::WaitingTimetable;
    }
    if v.depart_turn > 0 {
        return TraceVehicleState::DepartTurn;
    }
    if v.progress == 255 && v.pos == v.dest && !v.orders.is_empty() {
        return TraceVehicleState::Holding;
    }
    if v.cur_speed > 0 {
        if v.movement_target().is_some() {
            return TraceVehicleState::Moving;
        }
        return TraceVehicleState::Decelerating;
    }
    TraceVehicleState::Idle
}
