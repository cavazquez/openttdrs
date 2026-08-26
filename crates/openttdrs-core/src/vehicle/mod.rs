//! Vehículos: movimiento, órdenes, carga, fiabilidad y horarios.

mod cargo;
mod model;
mod movement;
mod operational_status;
mod order;
mod order_execution;
mod reliability;
pub(crate) use reliability::{
    init_vehicle_reliability_from_engine, process_vehicle_calendar_day,
    process_vehicle_economy_day, update_vehicle_servicing_flags,
};

// Re-exportaciones públicas desde model.rs
pub use model::{
    AircraftPhase, DIR_E, DIR_N, DIR_NE, DIR_NW, DIR_S, DIR_SE, DIR_SW, DIR_W, RoadDepotPhase,
    TimetableWaitKind, Vehicle, VehicleDirection, VehicleKind, VehicleRandomTrigger,
};

// Re-exportaciones públicas desde order.rs
pub use order::{
    MAX_VEHICLE_NAME_CHARS, OrderConditionComparator, OrderConditionKind, OrderLoadType,
    OrderNonStop, OrderStopLocation, OrderUnloadType, VehicleOrder,
};

// Re-exportaciones públicas desde reliability.rs
pub use reliability::{
    BREAKDOWN_DURATION_TICKS, DEFAULT_SERVICE_INTERVAL_DAYS, SERVICING_RELIABILITY_THRESHOLD,
};

// Re-exportaciones públicas desde movement.rs
pub use movement::VEHICLE_PROGRESS_STEP;

// Re-exportaciones públicas desde operational_status.rs
pub use operational_status::{VehicleIssueDetail, VehicleOperationalSummary};

/// Capacidad de carga por defecto (unidades de cargo).
pub const VEHICLE_CAPACITY: u32 = 20;

/// Sentido opuesto en la rosa de 8 direcciones `OpenTTD`.
#[must_use]
pub const fn reverse_direction(d: VehicleDirection) -> VehicleDirection {
    (d + 4) % 8
}

/// Dirección diagonal/cardinal desde un paso entre teselas adyacentes.
#[must_use]
pub fn direction_from_tile_step(
    from: crate::map::TileCoord,
    to: crate::map::TileCoord,
) -> VehicleDirection {
    use model::{DIR_NE, DIR_NW, DIR_SE, DIR_SW};
    match (to.x - from.x, to.y - from.y) {
        (-1, 0) => DIR_NE,
        (0, 1) => DIR_SE,
        (1, 0) => DIR_SW,
        (0, -1) => DIR_NW,
        _ => DIR_NE,
    }
}

#[cfg(test)]
mod tests;
