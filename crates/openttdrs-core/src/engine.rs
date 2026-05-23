//! Motores base `OpenGFX` (velocidad máxima en unidades internas de `OpenTTD`).

use crate::vehicle::{VehicleDirection, VehicleKind};

/// Definición mínima de motor (paridad con `_orig_*_vehicle_info` del upstream).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EngineDef {
    pub id: u16,
    pub kind: VehicleKind,
    pub name: &'static str,
    /// Unidades `OpenTTD` (`ROV`/`RVI`: ~0,5 km/h por unidad en pantalla).
    pub max_speed: u16,
}

pub const ENGINE_BUS_MPS: u16 = 0;
pub const ENGINE_TRUCK_MPS: u16 = 10;
pub const ENGINE_TRAIN_KIRBY: u16 = 100;

/// Paso sub-tile del bus MPS en diagonal (~5 ticks/tesela con sim a 5 Hz).
pub const REFERENCE_PROGRESS_STEP: u8 = 51;

const REFERENCE_MAX_SPEED: u16 = 112;
const TILE_AXIAL_DISTANCE: u32 = 192;
const TILE_CORNER_DISTANCE: u32 = 256;

const ENGINES: &[EngineDef] = &[
    EngineDef {
        id: ENGINE_BUS_MPS,
        kind: VehicleKind::Bus,
        name: "MPS Regal Bus",
        max_speed: 112,
    },
    EngineDef {
        id: ENGINE_TRUCK_MPS,
        kind: VehicleKind::Truck,
        name: "MPS Mail Truck",
        max_speed: 96,
    },
    EngineDef {
        id: ENGINE_TRAIN_KIRBY,
        kind: VehicleKind::Train,
        name: "Kirby Paul Tank",
        max_speed: 64,
    },
];

#[must_use]
pub const fn default_engine_id(kind: VehicleKind) -> u16 {
    match kind {
        VehicleKind::Bus => ENGINE_BUS_MPS,
        VehicleKind::Truck => ENGINE_TRUCK_MPS,
        VehicleKind::Train => ENGINE_TRAIN_KIRBY,
    }
}

#[must_use]
pub fn engine_for_vehicle(kind: VehicleKind, id: u16) -> &'static EngineDef {
    if let Some(engine) = ENGINES
        .iter()
        .find(|engine| engine.kind == kind && engine.id == id)
    {
        return engine;
    }
    engine_for_vehicle(kind, default_engine_id(kind))
}

/// Longitud lógica de tesela (`GetAdvanceDistance` de `OpenTTD`).
#[must_use]
pub const fn tile_progress_length(direction: VehicleDirection) -> u32 {
    if direction & 1 == 1 {
        TILE_AXIAL_DISTANCE
    } else {
        TILE_CORNER_DISTANCE
    }
}

/// Avance sub-tile por tick (`GetAdvanceSpeed` × escala a `progress` 0–255).
#[must_use]
pub fn progress_step_for_speed(max_speed: u16, direction: VehicleDirection) -> u8 {
    let advance = u32::from(max_speed) * 3 / 4;
    let tile_len = tile_progress_length(direction);
    let reference_advance = u32::from(REFERENCE_MAX_SPEED) * 3 / 4;
    let step = advance * u32::from(REFERENCE_PROGRESS_STEP) * TILE_AXIAL_DISTANCE
        / (reference_advance * tile_len);
    step.clamp(1, 255) as u8
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::vehicle::DIR_SW;

    #[test]
    fn reference_bus_keeps_five_ticks_per_diagonal_tile() {
        let step = progress_step_for_speed(112, DIR_SW);
        assert_eq!(step, REFERENCE_PROGRESS_STEP);
        assert_eq!(255_u32.div_ceil(u32::from(step)), 5);
    }

    #[test]
    fn truck_is_slower_than_bus_train_slowest() {
        let bus = progress_step_for_speed(112, DIR_SW);
        let truck = progress_step_for_speed(96, DIR_SW);
        let train = progress_step_for_speed(64, DIR_SW);
        assert!(bus > truck);
        assert!(truck > train);
    }
}
