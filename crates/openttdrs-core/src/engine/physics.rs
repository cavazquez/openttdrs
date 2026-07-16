//! Aceleración, velocidad y progreso sub-tile.

use crate::vehicle::VehicleDirection;

/// Paso sub-tile del bus MPS en diagonal a velocidad de crucero (`GetAdvanceSpeed` ×
/// `255/192` sobre `GetAdvanceDistance` diagonal — `vehicle_base.h:439-455`).
pub const REFERENCE_PROGRESS_STEP: u8 = 112;

/// Aceleración carretera modelo original (`RoadVehicle::UpdateSpeed`, `AM_ORIGINAL`).
pub const ROAD_ACCEL_ORIGINAL: u16 = 256;

const REFERENCE_MAX_SPEED: u16 = 112;
const TILE_AXIAL_DISTANCE: u32 = 192;
const TILE_CORNER_DISTANCE: u32 = 256;

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

/// Aceleración `AM_ORIGINAL` de tren (`Train::UpdateAcceleration`, `train_cmd.cpp:451`).
#[must_use]
pub fn train_acceleration(power_hp: u32, weight_t: u16) -> u8 {
    let weight = u32::from(weight_t.max(1));
    ((power_hp / weight) * 4).clamp(1, 255) as u8
}

/// Avance de velocidad de tren `AM_ORIGINAL` (`Train::UpdateSpeed`, `accel·2`).
#[must_use]
pub fn accelerate_train_speed(
    cur_speed: u16,
    subspeed: u8,
    power_hp: u32,
    weight_t: u16,
    max_speed: u16,
) -> (u16, u8) {
    let accel = u16::from(train_acceleration(power_hp, weight_t));
    let delta = accel.saturating_mul(2);
    update_road_speed(cur_speed, subspeed, delta, 0, max_speed)
}

/// Frenado de tren `AM_ORIGINAL` (`Train::UpdateSpeed`, `accel·4` hacia 0).
#[must_use]
#[allow(clippy::cast_possible_truncation)] // `subspeed = (uint8_t)spd` en upstream
pub fn decelerate_train_speed(cur_speed: u16, subspeed: u8, accel: u8) -> (u16, u8) {
    let delta = u16::from(accel).saturating_mul(4);
    let spd = u16::from(subspeed).saturating_add(delta);
    let new_subspeed = spd as u8;
    let dec = i32::from(spd >> 8);
    let new_cur = i32::from(cur_speed).saturating_sub(dec);
    let new_cur_u16 = u16::try_from(new_cur).unwrap_or(0);
    let final_sub = if new_cur_u16 == 0 { 0 } else { new_subspeed };
    (new_cur_u16, final_sub)
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
    fn reference_bus_diagonal_tile_matches_openttd_advance_speed() {
        let step = progress_step_for_speed(112, DIR_SW);
        assert_eq!(step, REFERENCE_PROGRESS_STEP);
        let ticks = 255_u32.div_ceil(u32::from(step));
        // OpenTTD: 192 / (112*3/4) ≈ 2,3 ticks/tesela a crucero.
        assert_eq!(ticks, 3);
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
    fn kirby_train_acceleration_matches_upstream() {
        assert_eq!(train_acceleration(300, 47), 24);
    }

    #[test]
    fn train_accel_slower_than_road_at_standstill() {
        let mut road_cur = 0_u16;
        let road_sub;
        (road_cur, road_sub) = update_road_speed(road_cur, 0, ROAD_ACCEL_ORIGINAL, 0, 64);
        let _ = road_sub;
        assert_eq!(road_cur, 1, "carretera: +1 en el primer tick");

        let mut train_cur = 0_u16;
        let mut train_sub = 0_u8;
        let mut ticks = 0_u32;
        while train_cur < 1 && ticks < 20 {
            (train_cur, train_sub) = accelerate_train_speed(train_cur, train_sub, 300, 47, 64);
            ticks += 1;
        }
        assert_eq!(train_cur, 1);
        assert!(
            ticks > 1,
            "Kirby AM_ORIGINAL tarda más que carretera en el primer +1"
        );
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
