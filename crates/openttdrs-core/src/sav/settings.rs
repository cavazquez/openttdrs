//! Ajustes de construcción persistidos por `OpenTTD` en `PATS` / `OPTS`.
//!
//! A diferencia de los bytes de mapa, el lado de conducción y de las señales
//! no vive en una tesela. Omitirlo al importar un `.sav` cambia la ubicación
//! de todos los postes de señal y hace que la misma partida se renderice de
//! forma distinta a `OpenTTD`.

use crate::engine::TrainAccelerationModel;
use crate::{ConstructionSettings, PathfindingSettings, RoadVehicleDrivingSide, TrainSignalSide};

use super::chunks::{RawChunk, find_chunk};
use super::table::{SlValue, parse_table_chunk, record_get};

fn road_side_from_u8(value: u8) -> Option<RoadVehicleDrivingSide> {
    match value {
        0 => Some(RoadVehicleDrivingSide::Left),
        1 => Some(RoadVehicleDrivingSide::Right),
        _ => None,
    }
}

fn train_signal_side_from_u8(value: u8) -> Option<TrainSignalSide> {
    match value {
        0 => Some(TrainSignalSide::Left),
        1 => Some(TrainSignalSide::RoadVehicleDrivingSide),
        2 => Some(TrainSignalSide::Right),
        _ => None,
    }
}

fn bool_from_u64(value: u64) -> Option<bool> {
    match value {
        0 => Some(false),
        1 => Some(true),
        _ => None,
    }
}

/// Lee el subconjunto de ajustes de PATS que el core ejecuta durante la
/// simulación. Los campos desconocidos se ignoran y conservan sus defaults.
pub(crate) fn settings_from_chunks(
    chunks: &[RawChunk],
) -> (
    ConstructionSettings,
    PathfindingSettings,
    TrainAccelerationModel,
    bool,
    u8,
) {
    let mut settings = ConstructionSettings::default();
    let mut pathfinding = PathfindingSettings::default();
    let mut train_acceleration_model = TrainAccelerationModel::Realistic;
    let mut station_noise_level = false;
    let mut vehicle_breakdowns = 2;
    for name in ["PATS", "OPTS"] {
        let Some(chunk) = find_chunk(chunks, name) else {
            continue;
        };
        let Ok(rows) = parse_table_chunk(&chunk.body, false) else {
            continue;
        };
        let mut found = false;
        for (_, record) in rows {
            if let Some(value) = record_get(&record, "vehicle.road_side")
                .and_then(SlValue::as_u64)
                .and_then(|value| u8::try_from(value).ok())
                .and_then(road_side_from_u8)
            {
                settings.road_vehicle_driving_side = value;
                found = true;
            }
            if let Some(value) = record_get(&record, "construction.train_signal_side")
                .and_then(SlValue::as_u64)
                .and_then(|value| u8::try_from(value).ok())
                .and_then(train_signal_side_from_u8)
            {
                settings.train_signal_side = value;
                found = true;
            }
            if let Some(value) = record_get(&record, "construction.freeform_edges")
                .and_then(SlValue::as_u64)
                .and_then(bool_from_u64)
            {
                settings.freeform_edges = value;
                found = true;
            }
            if let Some(value) = record_get(&record, "pf.wait_for_pbs_path")
                .and_then(SlValue::as_u64)
                .and_then(|value| u8::try_from(value).ok())
            {
                pathfinding.wait_for_pbs_path = value;
            }
            if let Some(value) = record_get(&record, "pf.path_backoff_interval")
                .and_then(SlValue::as_u64)
                .and_then(|value| u8::try_from(value).ok())
            {
                pathfinding.path_backoff_interval = value;
            }
            if let Some(value) = record_get(&record, "pf.reverse_at_signals")
                .and_then(SlValue::as_u64)
                .and_then(bool_from_u64)
            {
                pathfinding.reverse_at_signals = value;
            }
            if let Some(value) = record_get(&record, "pf.wait_oneway_signal")
                .and_then(SlValue::as_u64)
                .and_then(|value| u8::try_from(value).ok())
            {
                pathfinding.wait_oneway_signal = value;
            }
            if let Some(value) = record_get(&record, "pf.wait_twoway_signal")
                .and_then(SlValue::as_u64)
                .and_then(|value| u8::try_from(value).ok())
            {
                pathfinding.wait_twoway_signal = value;
            }
            if let Some(value) = record_get(&record, "pf.reserve_paths")
                .and_then(SlValue::as_u64)
                .and_then(bool_from_u64)
            {
                pathfinding.reserve_paths = value;
            }
            if let Some(value) =
                record_get(&record, "vehicle.train_acceleration_model").and_then(SlValue::as_u64)
            {
                train_acceleration_model = match value {
                    0 => TrainAccelerationModel::Original,
                    1 => TrainAccelerationModel::Realistic,
                    _ => train_acceleration_model,
                };
            }
            if let Some(value) = record_get(&record, "economy.station_noise_level")
                .and_then(SlValue::as_u64)
                .and_then(bool_from_u64)
            {
                station_noise_level = value;
            }
            if let Some(value) = record_get(&record, "difficulty.vehicle_breakdowns")
                .and_then(SlValue::as_u64)
                .and_then(|value| u8::try_from(value).ok())
            {
                vehicle_breakdowns = value.min(2);
            }
        }
        if found {
            break;
        }
    }
    (
        settings,
        pathfinding,
        train_acceleration_model,
        station_noise_level,
        vehicle_breakdowns,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sav::chunks::CH_TABLE;
    use crate::sav::table::tests::build_table_body;

    fn pats(values: [u8; 3]) -> RawChunk {
        RawChunk {
            name: *b"PATS",
            ch_type: CH_TABLE,
            body: build_table_body(
                &[
                    (2, "vehicle.road_side"),
                    (2, "construction.train_signal_side"),
                    (1, "construction.freeform_edges"),
                ],
                &[values.to_vec()],
            ),
        }
    }

    #[test]
    fn reads_right_side_settings_from_modern_save_table() {
        let settings = settings_from_chunks(&[pats([1, 1, 1])]).0;
        assert_eq!(
            settings.road_vehicle_driving_side,
            RoadVehicleDrivingSide::Right
        );
        assert_eq!(
            settings.train_signal_side,
            TrainSignalSide::RoadVehicleDrivingSide
        );
        assert!(settings.signals_on_right());
        assert!(settings.freeform_edges);
    }

    #[test]
    fn reads_disabled_freeform_edges_from_save_table() {
        let settings = settings_from_chunks(&[pats([0, 0, 0])]).0;
        assert!(!settings.freeform_edges);
    }

    #[test]
    fn invalid_saved_enums_keep_safe_defaults() {
        let settings = settings_from_chunks(&[pats([9, 7, 2])]).0;
        assert_eq!(settings, ConstructionSettings::default());
    }

    #[test]
    fn reads_simulation_settings_from_pats() {
        let body = build_table_body(
            &[
                (2, "pf.wait_for_pbs_path"),
                (2, "pf.path_backoff_interval"),
                (2, "pf.reverse_at_signals"),
                (2, "pf.wait_oneway_signal"),
                (2, "pf.wait_twoway_signal"),
                (2, "pf.reserve_paths"),
                (2, "vehicle.train_acceleration_model"),
                (2, "economy.station_noise_level"),
                (2, "difficulty.vehicle_breakdowns"),
            ],
            &[vec![2, 3, 0, 4, 5, 1, 0, 1, 2]],
        );
        let chunk = RawChunk {
            name: *b"PATS",
            ch_type: CH_TABLE,
            body,
        };
        let (_, pathfinding, acceleration, noise, breakdowns) = settings_from_chunks(&[chunk]);
        assert_eq!(pathfinding.wait_for_pbs_path, 2);
        assert_eq!(pathfinding.path_backoff_interval, 3);
        assert!(!pathfinding.reverse_at_signals);
        assert_eq!(pathfinding.wait_oneway_signal, 4);
        assert_eq!(pathfinding.wait_twoway_signal, 5);
        assert!(pathfinding.reserve_paths);
        assert_eq!(acceleration, TrainAccelerationModel::Original);
        assert!(noise);
        assert_eq!(breakdowns, 2);
    }
}
