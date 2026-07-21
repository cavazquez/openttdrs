//! Eventos efímeros de simulación consumidos por el cliente (audio, FX, UI).

use crate::cargo::CargoType;
use crate::map::TileCoord;

/// Tipo de construcción para SFX (`SND_*_CONSTRUCTION_*` en `OpenTTD`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConstructionKind {
    Rail,
    Road,
    Water,
    Bridge,
    Other,
}

/// Tipo de desastre (`disaster_vehicle.cpp`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisasterKind {
    SmallUfo,
    Airplane,
    Helicopter,
    BigUfo,
    Submarine,
    CoalMineSubsidence,
}

/// Humo/chispas de locomotora (`EffectVehicleType`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrainSmokeKind {
    Steam,
    Diesel,
    Electric,
}

/// Fase de motor en marcha (`VSE_RUNNING` / `VSE_RUNNING_16` / `VSE_STOPPED_16`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VehicleRunningPhase {
    /// Cruce del contador de movimiento (∝ velocidad).
    Running,
    /// Pulso cada 16 ticks con velocidad > 0.
    Running16,
    /// Pulso cada 16 ticks parado / frenando.
    Stopped16,
}

/// Evento discreto emitido durante un tick de simulación.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SimEvent {
    Income {
        amount: i64,
        at: TileCoord,
    },
    Construction {
        kind: ConstructionKind,
        at: TileCoord,
    },
    Demolition {
        at: TileCoord,
    },
    VehicleDepart {
        vehicle_id: u32,
        at: TileCoord,
        kind: crate::vehicle::VehicleKind,
    },
    /// Motor en marcha / idle (estilo `vehicle.cpp` `motion_counter`).
    VehicleRunning {
        vehicle_id: u32,
        at: TileCoord,
        kind: crate::vehicle::VehicleKind,
        phase: VehicleRunningPhase,
    },
    LevelCrossing {
        at: TileCoord,
    },
    Breakdown {
        vehicle_id: u32,
        at: TileCoord,
        kind: crate::vehicle::VehicleKind,
    },
    Disaster {
        kind: DisasterKind,
        at: TileCoord,
    },
    TownRatingChanged {
        town_id: u32,
        delta: i8,
    },
    SubsidyCreated {
        industry_pos: TileCoord,
        station_pos: TileCoord,
        cargo: CargoType,
    },
    SubsidyAwarded {
        cargo: CargoType,
        company: crate::company::CompanyId,
    },
    NewsTicker,
    NewsApplause,
    NewsChime,
    LoanInterestPaid {
        amount: i64,
    },
    BankruptcyWarning,
    /// Choque de trenes (misma compañía).
    TrainCollision {
        at: TileCoord,
        vehicle_a: u32,
        vehicle_b: u32,
    },
    /// Fin de partida (quiebra definitiva o retiro).
    GameOver {
        company_name: String,
        company_value: i64,
        calendar_year: u32,
        reason: crate::score::GameOverReason,
    },
    AircraftTakeoff {
        vehicle_id: u32,
        at: TileCoord,
    },
    AircraftLanding {
        vehicle_id: u32,
        at: TileCoord,
    },
    /// Jet estrellado en pista corta (`MaybeCrashAirplane`).
    AircraftCrash {
        vehicle_id: u32,
        at: TileCoord,
    },
}

/// Cola de eventos del tick actual (no persistida).
#[derive(Debug, Default, Clone)]
pub struct SimEventQueue {
    events: Vec<SimEvent>,
}

impl SimEventQueue {
    #[must_use]
    pub const fn new() -> Self {
        Self { events: Vec::new() }
    }

    pub fn push(&mut self, event: SimEvent) {
        self.events.push(event);
    }

    /// Extrae y vacía todos los eventos pendientes.
    pub fn drain(&mut self) -> Vec<SimEvent> {
        std::mem::take(&mut self.events)
    }

    /// Descarta eventos sin reproducirlos (p. ej. `apply_command` del bootstrap del mapa).
    pub fn discard_all(&mut self) {
        self.events.clear();
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// Vista de eventos aún no drenados (p. ej. remap visual antes del SFX).
    pub fn iter(&self) -> impl Iterator<Item = &SimEvent> {
        self.events.iter()
    }
}
