//! Aceleración, velocidad y progreso sub-tile.

use crate::vehicle::VehicleDirection;

/// Paso sub-tile del bus MPS en diagonal a velocidad de crucero (`GetAdvanceSpeed` ×
/// `255/192` sobre `GetAdvanceDistance` diagonal — `vehicle_base.h:439-455`).
pub const REFERENCE_PROGRESS_STEP: u8 = 112;

/// Aceleración carretera modelo original (`RoadVehicle::UpdateSpeed`, `AM_ORIGINAL`).
pub const ROAD_ACCEL_ORIGINAL: u16 = 256;

/// Aceleración gravitatoria de `OpenTTD` (`GROUND_ACCELERATION`, N/tonelada efectiva).
pub const GROUND_ACCELERATION: u32 = 9800;

/// Área frontal de tren fuera de túnel (`Train::GetAirDragArea`).
pub const TRAIN_AIR_DRAG_AREA: u8 = 14;

const REFERENCE_MAX_SPEED: u16 = 112;
const TILE_AXIAL_DISTANCE: u32 = 192;
/// `TILE_CORNER_DISTANCE` de `OpenTTD` es 128; `GetAdvanceDistance` usa `* 2` → 256.
const TILE_CORNER_ADVANCE: u32 = 256;

/// Modelo de aceleración de tren (`_settings_game.vehicle.train_acceleration_model`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[repr(u8)]
pub enum TrainAccelerationModel {
    /// `AM_ORIGINAL`: `acceleration * 2` / freno `* 4`.
    #[default]
    Original = 0,
    /// `AM_REALISTIC`: `GetAcceleration()` por potencia/resistencia.
    Realistic = 1,
}

/// Resultado de `GroundVehicleBase::DoUpdateSpeed`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DoUpdateSpeedResult {
    pub cur_speed: u16,
    pub subspeed: u8,
    /// Distancia física del tick (`GetAdvanceSpeed(spd) + progress` previo, sin el resto).
    pub advance: u32,
}

/// Longitud lógica de tesela (`GetAdvanceDistance` de `OpenTTD`).
#[must_use]
pub const fn tile_progress_length(direction: VehicleDirection) -> u32 {
    if direction & 1 == 1 {
        TILE_AXIAL_DISTANCE
    } else {
        TILE_CORNER_ADVANCE
    }
}

/// Alias explícito de [`tile_progress_length`].
#[must_use]
pub const fn get_advance_distance(direction: VehicleDirection) -> u32 {
    tile_progress_length(direction)
}

/// `Vehicle::GetAdvanceSpeed`.
#[must_use]
pub const fn get_advance_speed(speed: u16) -> u32 {
    (speed as u32) * 3 / 4
}

/// Actualiza `cur_speed`/`subspeed` y devuelve la distancia (`DoUpdateSpeed`).
///
/// `prior_progress` es el remanente físico (`Vehicle::progress`) al inicio del
/// handler; `OpenTTD` lo suma y lo pone a 0 dentro de `DoUpdateSpeed`.
#[must_use]
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // `subspeed = (uint8_t)spd`
pub fn do_update_speed(
    cur_speed: u16,
    subspeed: u8,
    accel: i32,
    min_speed: u16,
    max_speed: u16,
    prior_progress: u8,
) -> DoUpdateSpeedResult {
    let spd = i32::from(subspeed).saturating_add(accel);
    let new_subspeed = spd as u8;
    let cur = i32::from(cur_speed);
    let max_i = i32::from(max_speed);
    let tempmax = if cur > max_i {
        std::cmp::max(cur - (cur / 10) - 1, max_i)
    } else {
        max_i
    };
    let speed_delta = spd >> 8;
    let new_cur = std::cmp::max(
        std::cmp::min(cur.saturating_add(speed_delta), tempmax),
        i32::from(min_speed),
    );
    let new_cur_u16 = u16::try_from(new_cur).unwrap_or(0);
    let advance = get_advance_speed(new_cur_u16).saturating_add(u32::from(prior_progress));
    DoUpdateSpeedResult {
        cur_speed: new_cur_u16,
        subspeed: new_subspeed,
        advance,
    }
}

/// Actualiza `cur_speed`/`subspeed` (`GroundVehicleBase::DoUpdateSpeed`) sin distancia.
#[must_use]
#[allow(clippy::cast_possible_truncation)] // `subspeed = (uint8_t)spd` en upstream
pub fn update_road_speed(
    cur_speed: u16,
    subspeed: u8,
    accel: u16,
    min_speed: u16,
    max_speed: u16,
) -> (u16, u8) {
    let r = do_update_speed(
        cur_speed,
        subspeed,
        i32::from(accel),
        min_speed,
        max_speed,
        0,
    );
    (r.cur_speed, r.subspeed)
}

/// Aceleración `AM_ORIGINAL` de tren (`Train::UpdateAcceleration`, `train_cmd.cpp:451`).
#[must_use]
pub fn train_acceleration(power_hp: u32, weight_t: u16) -> u8 {
    let weight = u32::from(weight_t.max(1));
    ((power_hp / weight) * 4).clamp(1, 255) as u8
}

/// Coeficiente de arrastre por defecto desde velocidad máxima de display (`PowerChanged`).
#[must_use]
pub fn train_default_air_drag(display_max_speed: u16, consist_parts: u32) -> u32 {
    let air_drag = if display_max_speed <= 10 {
        192
    } else {
        std::cmp::max(2048 / u32::from(display_max_speed.max(1)), 1)
    };
    let parts = consist_parts.max(1);
    air_drag + 3 * air_drag * parts / 20
}

/// Esfuerzo tractor máximo en N (`PowerChanged`: `weight * TE * g / 256`).
#[must_use]
pub fn train_max_te_n(weight_t: u16, tractive_effort: u8) -> u32 {
    u32::from(weight_t)
        .saturating_mul(u32::from(tractive_effort))
        .saturating_mul(GROUND_ACCELERATION)
        / 256
}

/// `Train::GetRollingFriction`.
#[must_use]
pub fn train_rolling_friction(speed: u16) -> u32 {
    15 * (512 + u32::from(speed)) / 512
}

/// `GroundVehicle::GetAcceleration` para tren no-maglev en llano (`AS_ACCEL`).
#[must_use]
pub fn train_realistic_acceleration(
    speed: u16,
    power_hp: u32,
    weight_t: u16,
    max_te_n: u32,
    air_drag: u32,
    area: u8,
    slope_resistance: i64,
) -> i32 {
    let mass = i64::from(weight_t.max(1));
    let power_w = i64::from(power_hp) * 746;
    let axle = 10 * mass;
    let mut resistance = axle + mass * i64::from(train_rolling_friction(speed));
    resistance +=
        i64::from(area) * i64::from(air_drag) * i64::from(speed) * i64::from(speed) / 1000;
    resistance += slope_resistance;

    let force = if speed > 0 {
        let mut force = power_w * 18 / (i64::from(speed) * 5);
        if force > i64::from(max_te_n) {
            force = i64::from(max_te_n);
        }
        force
    } else {
        let kick = std::cmp::min(i64::from(max_te_n), power_w);
        std::cmp::max(kick, mass * 8 + resistance)
    };

    if force == resistance {
        return 0;
    }
    let accel = (force - resistance) / (mass * 4);
    #[allow(clippy::cast_possible_truncation)] // OpenTTD usa `int` tras Clamp implícito
    if force < resistance {
        i32::try_from(accel.min(-1)).unwrap_or(i32::MIN)
    } else {
        i32::try_from(accel.max(1)).unwrap_or(i32::MAX)
    }
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

/// `Train::UpdateSpeed` + `DoUpdateSpeed` con distancia (original o realista).
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn update_train_speed(
    cur_speed: u16,
    subspeed: u8,
    prior_progress: u8,
    model: TrainAccelerationModel,
    power_hp: u32,
    weight_t: u16,
    max_te_n: u32,
    air_drag: u32,
    max_speed: u16,
    braking: bool,
) -> DoUpdateSpeedResult {
    match model {
        TrainAccelerationModel::Original => {
            let base = train_acceleration(power_hp, weight_t);
            let accel = if braking {
                -i32::from(base) * 4
            } else {
                i32::from(base) * 2
            };
            do_update_speed(cur_speed, subspeed, accel, 0, max_speed, prior_progress)
        }
        TrainAccelerationModel::Realistic => {
            let accel = if braking {
                // Frenado realista: `GetAcceleration` con `AS_BRAKE` (fuerza negativa).
                // Para el fixture PBS el tren acelera; usamos magnitud realista hacia 0.
                -train_realistic_acceleration(
                    cur_speed,
                    power_hp,
                    weight_t,
                    max_te_n,
                    air_drag,
                    TRAIN_AIR_DRAG_AREA,
                    0,
                )
                .abs()
                .max(1)
            } else {
                train_realistic_acceleration(
                    cur_speed,
                    power_hp,
                    weight_t,
                    max_te_n,
                    air_drag,
                    TRAIN_AIR_DRAG_AREA,
                    0,
                )
            };
            let min_speed = if braking { 0 } else { 2 };
            do_update_speed(
                cur_speed,
                subspeed,
                accel,
                min_speed,
                max_speed,
                prior_progress,
            )
        }
    }
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
///
/// Solo carretera/tranvía: los trenes usan distancia física + píxeles.
#[must_use]
pub fn progress_step_for_speed(max_speed: u16, direction: VehicleDirection) -> u8 {
    if max_speed == 0 {
        return 0;
    }
    let advance = get_advance_speed(max_speed);
    let tile_len = tile_progress_length(direction);
    let reference_advance = get_advance_speed(REFERENCE_MAX_SPEED);
    let step = advance * u32::from(REFERENCE_PROGRESS_STEP) * TILE_AXIAL_DISTANCE
        / (reference_advance * tile_len);
    if step == 0 {
        return 0;
    }
    step.clamp(1, 255) as u8
}

/// Progreso visual 0..=255 desde píxeles de vía (`16` píxeles / tesela).
#[must_use]
pub fn train_visual_progress_from_pixel(rail_pixel: u8) -> f32 {
    (f32::from(rail_pixel) / 16.0) * 255.0
}

/// Progreso visual ferroviario incluyendo el remanente físico entre píxeles.
///
/// El controlador conserva `progress < GetAdvanceDistance`; ignorarlo dejaba
/// al sprite quieto entre incrementos de `rail_pixel`, aun con render a 60 FPS.
#[must_use]
pub fn train_visual_progress_from_motion(
    rail_pixel: u8,
    progress: u8,
    advance_distance: u32,
) -> f32 {
    let advance_distance = u16::try_from(advance_distance.max(1)).unwrap_or(u16::MAX);
    let fractional_pixel = f32::from(progress) / f32::from(advance_distance);
    ((f32::from(rail_pixel.min(15)) + fractional_pixel) / 16.0) * 255.0
}

/// Longitud de unidad `OpenTTD` (`VEHICLE_LENGTH`) usada por [`get_curve_speed_limit`].
const CURVE_VEHICLE_LENGTH: i32 = 8;

/// `Train::GetCurveSpeedLimit` (`train_cmd.cpp:312-381`).
///
/// `units`: pares `(direction, cached_veh_length)` de cabeza a cola.
/// `railtype_curve_speed`: ventaja del railtype (vanilla 0).
#[must_use]
pub fn get_curve_speed_limit(
    model: TrainAccelerationModel,
    units: &[(u8, u8)],
    railtype_curve_speed: u8,
    cached_tilt: bool,
    cached_curve_speed_mod: i16,
) -> u16 {
    const ABSOLUTE_MAX: i32 = 65_535;
    if matches!(model, TrainAccelerationModel::Original) {
        return u16::MAX;
    }
    let mut max_speed = ABSOLUTE_MAX;
    let mut curvecount = [0_i32, 0_i32];
    let mut numcurve = 0_i32;
    let mut sum = 0_i32;
    let mut pos = 0_i32;
    let mut lastpos = -1_i32;

    for window in units.windows(2) {
        let (this_dir, next_len) = (window[0].0, window[1].1);
        let next_dir = window[1].0;
        pos += i32::from(next_len.max(1));
        let dirdiff = this_dir.wrapping_sub(next_dir) % 8;
        if dirdiff == 0 {
            continue;
        }
        if dirdiff == 7 {
            // DirDiff::Left45
            curvecount[0] += 1;
        }
        if dirdiff == 1 {
            // DirDiff::Right45
            curvecount[1] += 1;
        }
        if dirdiff == 1 || dirdiff == 7 {
            if lastpos != -1 {
                numcurve += 1;
                sum += pos - lastpos;
                if pos - lastpos <= CURVE_VEHICLE_LENGTH && max_speed > 88 {
                    max_speed = 88;
                }
            }
            lastpos = pos;
        }
        if dirdiff == 2 || dirdiff == 6 {
            max_speed = 61;
        }
    }

    if numcurve > 0 && max_speed > 88 {
        if curvecount[0] == 1 && curvecount[1] == 1 {
            max_speed = ABSOLUTE_MAX;
        } else {
            let avg = ((sum + CURVE_VEHICLE_LENGTH - 1) / CURVE_VEHICLE_LENGTH) / numcurve;
            let n = avg.clamp(1, 12);
            max_speed = 232 - (13 - n) * (13 - n);
        }
    }

    if max_speed != ABSOLUTE_MAX {
        max_speed += (max_speed / 2) * i32::from(railtype_curve_speed);
        if cached_tilt {
            max_speed += max_speed / 5;
        }
        max_speed += (max_speed * i32::from(cached_curve_speed_mod)) / 256;
        max_speed = max_speed.clamp(2, ABSOLUTE_MAX);
    }
    u16::try_from(max_speed).unwrap_or(u16::MAX)
}

/// Techo de aproximación a plataforma en `AM_REALISTIC` (`train_cmd.cpp:405-414`).
#[must_use]
pub fn train_realistic_station_max_speed(
    cur_speed: u16,
    distance_to_go: i32,
    current_max: u16,
) -> u16 {
    if distance_to_go <= 0 {
        return current_max;
    }
    let mut st_max_speed = 120_i32;
    let delta_v = i32::from(cur_speed) / (distance_to_go + 1);
    if i32::from(current_max) > i32::from(cur_speed) - delta_v {
        st_max_speed = i32::from(cur_speed) - (delta_v / 10);
    }
    st_max_speed = st_max_speed.max(25 * distance_to_go);
    let st = u16::try_from(st_max_speed.max(0)).unwrap_or(u16::MAX);
    current_max.min(st)
}

/// Esfuerzo tractor vanilla por `engine_id` interno (`RVI` param g).
#[must_use]
pub fn vanilla_train_tractive_effort(engine_id: u16) -> u8 {
    use super::{
        ENGINE_TRAIN_ASIASTAR, ENGINE_TRAIN_CHANEY_JUBILEE, ENGINE_TRAIN_DASH,
        ENGINE_TRAIN_FLOSS_47, ENGINE_TRAIN_GINZU_A4, ENGINE_TRAIN_KIRBY, ENGINE_TRAIN_LEV1,
        ENGINE_TRAIN_MANLEY_MOREL, ENGINE_TRAIN_SH_8P, ENGINE_TRAIN_SH_30, ENGINE_TRAIN_SH_40,
        ENGINE_TRAIN_SH_125, ENGINE_TRAIN_SH_HENDRY_25, ENGINE_TRAIN_TIM, ENGINE_TRAIN_UU_37,
        ENGINE_TRAIN_X2001,
    };
    match engine_id {
        ENGINE_TRAIN_KIRBY => 50,
        ENGINE_TRAIN_CHANEY_JUBILEE | ENGINE_TRAIN_UU_37 => 120,
        ENGINE_TRAIN_GINZU_A4 | ENGINE_TRAIN_FLOSS_47 => 140,
        ENGINE_TRAIN_SH_8P => 130,
        ENGINE_TRAIN_MANLEY_MOREL => 85,
        ENGINE_TRAIN_DASH => 70,
        ENGINE_TRAIN_SH_HENDRY_25 => 95,
        ENGINE_TRAIN_SH_125 => 190,
        ENGINE_TRAIN_SH_30 | ENGINE_TRAIN_X2001 => 180,
        ENGINE_TRAIN_SH_40 => 205,
        ENGINE_TRAIN_TIM => 240,
        ENGINE_TRAIN_ASIASTAR => 250,
        ENGINE_TRAIN_LEV1 => 200,
        _ => 75,
    }
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
    fn ginzu_realistic_accel_matches_pbs_oracle_series() {
        let power = 1200_u32;
        let weight = 162_u16;
        let te = 140_u8;
        let max_te = train_max_te_n(weight, te);
        let air = train_default_air_drag(128, 1);
        let samples = [(73, 59), (74, 58), (75, 57), (80, 52), (85, 48), (89, 46)];
        for (speed, expected) in samples {
            let accel = train_realistic_acceleration(
                speed,
                power,
                weight,
                max_te,
                air,
                TRAIN_AIR_DRAG_AREA,
                0,
            );
            assert_eq!(accel, expected, "speed {speed}");
        }
    }

    #[test]
    fn do_update_speed_adds_prior_progress_like_openttd() {
        let r = do_update_speed(73, 52, 59, 2, 128, 51);
        assert_eq!(r.cur_speed, 73);
        assert_eq!(r.subspeed, 111);
        assert_eq!(r.advance, get_advance_speed(73) + 51);
    }

    #[test]
    fn pbs_fixture_first_tick_double_loco_handler() {
        let power = 1200;
        let weight = 162;
        let te = train_max_te_n(weight, 140);
        let air = train_default_air_drag(128, 1);
        let mut speed = 73_u16;
        let mut sub = 52_u8;
        let mut progress = 51_u8;
        for _ in 0..2 {
            let r = update_train_speed(
                speed,
                sub,
                progress,
                TrainAccelerationModel::Realistic,
                power,
                weight,
                te,
                air,
                128,
                false,
            );
            speed = r.cur_speed;
            sub = r.subspeed;
            let adv = get_advance_distance(1);
            let mut j = r.advance;
            while j >= adv && speed > 0 {
                j -= adv;
            }
            progress = u8::try_from(j & 0xFF).unwrap();
        }
        assert_eq!((progress, speed, sub), (159, 73, 170));
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

    #[test]
    fn axial_and_corner_advance_distances_match_openttd() {
        assert_eq!(get_advance_distance(1), 192); // DIR_NE odd → axial
        assert_eq!(get_advance_distance(2), 256); // DIR_E even → corner*2
    }

    #[test]
    fn axial_controller_keeps_remainder_under_threshold() {
        // j = 54+100 = 154 < 192 → no paso de píxel; progress = 154.
        let r = do_update_speed(72, 0, 0, 0, 128, 100);
        assert_eq!(r.advance, 54 + 100);
        assert!(r.advance < get_advance_distance(1));
    }

    #[test]
    fn corner_threshold_is_wider_than_axial() {
        assert!(get_advance_distance(0) > get_advance_distance(1));
        assert!((train_visual_progress_from_pixel(8) - 127.5).abs() < f32::EPSILON);
        assert!((train_visual_progress_from_pixel(0)).abs() < f32::EPSILON);
        assert!((train_visual_progress_from_pixel(16) - 255.0).abs() < f32::EPSILON);
        assert!((train_visual_progress_from_motion(8, 96, 192) - 135.46875).abs() < f32::EPSILON);
    }

    #[test]
    fn curve_speed_limit_original_is_unbounded() {
        let units = [(1_u8, 8_u8), (3, 8)]; // 90°
        assert_eq!(
            get_curve_speed_limit(TrainAccelerationModel::Original, &units, 0, false, 0),
            u16::MAX
        );
    }

    #[test]
    fn curve_speed_limit_ninety_degree_is_61() {
        let units = [(1_u8, 8_u8), (3, 8)]; // DirDiff Left90
        assert_eq!(
            get_curve_speed_limit(TrainAccelerationModel::Realistic, &units, 0, false, 0),
            61
        );
    }

    #[test]
    fn curve_speed_limit_tight_45_pair_caps_88() {
        let units = [(1_u8, 8_u8), (2, 8), (3, 8)]; // dos Left45 seguidos
        assert_eq!(
            get_curve_speed_limit(TrainAccelerationModel::Realistic, &units, 0, false, 0),
            88
        );
    }

    #[test]
    fn curve_speed_limit_spacing_formula_matches_openttd() {
        // n=12 → 232 - 1² = 231; spacing largo entre dos 45° del mismo sentido.
        let units = [(1_u8, 8_u8), (2, 8), (2, 96), (3, 8)];
        assert_eq!(
            get_curve_speed_limit(TrainAccelerationModel::Realistic, &units, 0, false, 0),
            231
        );
    }

    #[test]
    fn curve_speed_limit_tilt_adds_twenty_percent() {
        let units = [(1_u8, 8_u8), (3, 8)];
        assert_eq!(
            get_curve_speed_limit(TrainAccelerationModel::Realistic, &units, 0, true, 0),
            73 // 61 + 61/5
        );
    }

    #[test]
    fn curve_speed_limit_mod_applies_fixed_point() {
        let units = [(1_u8, 8_u8), (3, 8)];
        assert_eq!(
            get_curve_speed_limit(TrainAccelerationModel::Realistic, &units, 0, false, 64),
            76 // 61 + 61*64/256
        );
    }

    #[test]
    fn station_approach_max_speed_respects_distance_floor() {
        // Alta velocidad: st_max = cur - delta_v/10 (196), por encima del suelo 100.
        assert_eq!(train_realistic_station_max_speed(200, 4, 250), 196);
        // Baja velocidad: manda el suelo 25·distance.
        assert_eq!(train_realistic_station_max_speed(10, 4, 250), 100);
        assert_eq!(train_realistic_station_max_speed(50, 0, 250), 250);
    }
}
