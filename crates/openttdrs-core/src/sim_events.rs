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
    },
    LevelCrossing {
        at: TileCoord,
    },
    Breakdown {
        vehicle_id: u32,
        at: TileCoord,
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
    },
    NewsTicker,
    NewsApplause,
    NewsChime,
    LoanInterestPaid {
        amount: i64,
    },
    BankruptcyWarning,
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

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }
}
