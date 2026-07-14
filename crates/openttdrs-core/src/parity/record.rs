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

/// Posición de una parte del tren (Fase Rail 1: siempre una sola parte porque
/// el tren de la sim es puntual; el esquema admite consist futura).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RailPartRecord {
    pub part_index: usize,
    pub tile: TileCoord,
    /// Sub-tesela de render 0..16 (misma que `road_movement::vehicle_subtile`).
    pub subtile_x: f32,
    pub subtile_y: f32,
}

/// Bloque ferroviario de la traza (solo se emite para trenes; los registros de
/// vehículos de carretera no cambian ni un byte).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[allow(clippy::struct_excessive_bools)] // esquema de traza JSONL: flags independientes, no estados
pub struct RailRecord {
    /// Partes del tren de cabeza a cola. Hoy: exactamente una.
    pub parts: Vec<RailPartRecord>,
    pub head_tile: TileCoord,
    /// Igual a `head_tile` mientras no exista consist.
    pub tail_tile: TileCoord,
    /// Track bits (`m5 & 0x3F`) de la tesela actual; túnel/puente → `X|Y`;
    /// otras teselas (depósito, estación) → 0.
    pub track_bits_under: u8,
    /// El tren no avanzaría este tick por señal en rojo (espeja la decisión de
    /// `sim_step`: `false` si está parado o con `force_proceed`).
    pub blocked_by_signal: bool,
    /// El tren no avanzaría este tick por otro tren delante.
    pub blocked_by_traffic: bool,
    /// Bloqueo PBS: falta reserva en el siguiente paso (`train_blocked_by_reservation`).
    #[serde(default)]
    pub blocked_by_reservation: bool,
    /// Longitud de `Vehicle::reserved_steps` (reserva PBS activa).
    #[serde(default)]
    pub reserved_len: u16,
    /// Última tesela de la reserva (safe wait / fin de path), si hay pasos.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reservation_end: Option<TileCoord>,
    pub in_depot: bool,
    /// El tren está físicamente en plataforma de estación rail.
    pub at_platform: bool,
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
    /// Bloque ferroviario (solo trenes; ausente en el JSONL para el resto).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rail: Option<RailRecord>,
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
    /// Un tren empezó a esperar por señal en rojo (Fase Rail 1).
    SignalWaitStarted {
        vehicle: u32,
        /// Tesela donde está la señal que lo retiene (la tesela del tren).
        tile: TileCoord,
    },
    /// El tren dejó de estar retenido por la señal.
    SignalWaitFinished {
        vehicle: u32,
        tile: TileCoord,
    },
    /// Un tren entró a una tesela de depósito.
    DepotEntry {
        vehicle: u32,
        depot: TileCoord,
    },
    /// Un tren salió de una tesela de depósito.
    DepotExit {
        vehicle: u32,
        depot: TileCoord,
    },
    /// Una señal del mapa cambió de estado (derivado del diff de `m3hi`).
    /// Sin vehículo asociado: es un evento de infraestructura.
    SignalStateChanged {
        tile: TileCoord,
        /// Bit de señal que cambió (`1 << sig_bit`, 0..4).
        track_mask: u8,
        green: bool,
    },
}

impl ParityEvent {
    /// Vehículo al que refiere el evento (`None` para eventos de
    /// infraestructura como [`ParityEvent::SignalStateChanged`]).
    #[must_use]
    pub const fn vehicle(&self) -> Option<u32> {
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
            | Self::OrderAdvanced { vehicle, .. }
            | Self::SignalWaitStarted { vehicle, .. }
            | Self::SignalWaitFinished { vehicle, .. }
            | Self::DepotEntry { vehicle, .. }
            | Self::DepotExit { vehicle, .. } => Some(*vehicle),
            Self::SignalStateChanged { .. } => None,
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
