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

/// Aceleración carretera modelo original (`RoadVehicle::UpdateSpeed`, `AM_ORIGINAL`).
pub const ROAD_ACCEL_ORIGINAL: u16 = 256;

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

/// Actualiza `cur_speed`/`subspeed` (`GroundVehicleBase::DoUpdateSpeed`).
#[must_use]
#[allow(clippy::cast_possible_truncation)] // `subspeed = (uint8_t)spd` en upstream
pub fn update_road_speed(
    cur_speed: u16,
    subspeed: u8,
    accel: u16,
    min_speed: u16,
    max_speed: u16,
) -> (u16, u8) {
    let spd = u16::from(subspeed).saturating_add(accel);
    let new_subspeed = spd as u8;
    let cur = i32::from(cur_speed);
    let max_i = i32::from(max_speed);
    let tempmax = if cur > max_i {
        std::cmp::max(cur - (cur / 10) - 1, max_i)
    } else {
        max_i
    };
    let new_cur = std::cmp::max(
        std::cmp::min(cur + i32::from(spd >> 8), tempmax),
        i32::from(min_speed),
    );
    (u16::try_from(new_cur).unwrap_or(0), new_subspeed)
}

/// Frenado simétrico al acelerador original (hacia velocidad 0).
#[must_use]
#[allow(clippy::cast_possible_truncation)] // `subspeed = (uint8_t)spd` en upstream
pub fn decelerate_road_speed(cur_speed: u16, subspeed: u8) -> (u16, u8) {
    let spd = u16::from(subspeed).saturating_add(ROAD_ACCEL_ORIGINAL);
    let new_subspeed = spd as u8;
    let dec = i32::from(spd >> 8);
    let new_cur = i32::from(cur_speed).saturating_sub(dec);
    let new_cur_u16 = u16::try_from(new_cur).unwrap_or(0);
    let final_sub = if new_cur_u16 == 0 { 0 } else { new_subspeed };
    (new_cur_u16, final_sub)
}

/// Avance sub-tile por tick (`GetAdvanceSpeed` × escala a `progress` 0–255).
#[must_use]
pub fn progress_step_for_speed(max_speed: u16, direction: VehicleDirection) -> u8 {
    if max_speed == 0 {
        return 0;
    }
    let advance = u32::from(max_speed) * 3 / 4;
    let tile_len = tile_progress_length(direction);
    let reference_advance = u32::from(REFERENCE_MAX_SPEED) * 3 / 4;
    let step = advance * u32::from(REFERENCE_PROGRESS_STEP) * TILE_AXIAL_DISTANCE
        / (reference_advance * tile_len);
    if step == 0 {
        return 0;
    }
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
    fn standstill_yields_zero_progress_step() {
        assert_eq!(progress_step_for_speed(0, DIR_SW), 0);
    }

    #[test]
    fn original_accel_reaches_max_in_reasonable_ticks() {
        let max = 112_u16;
        let mut cur = 0_u16;
        let mut sub = 0_u8;
        let mut ticks = 0_u32;
        while cur < max && ticks < 160 {
            (cur, sub) = update_road_speed(cur, sub, ROAD_ACCEL_ORIGINAL, 0, max);
            ticks += 1;
        }
        assert_eq!(cur, max);
        assert!(ticks > 1);
    }

    #[test]
    fn decelerate_from_cruise_stops_vehicle() {
        let mut cur = 112_u16;
        let mut sub = 0_u8;
        let mut ticks = 0_u32;
        while cur > 0 && ticks < 160 {
            (cur, sub) = decelerate_road_speed(cur, sub);
            ticks += 1;
        }
        assert_eq!(cur, 0);
        assert_eq!(sub, 0);
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
