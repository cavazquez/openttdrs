//! Ajustes de construcción persistidos por `OpenTTD` en `PATS` / `OPTS`.
//!
//! A diferencia de los bytes de mapa, el lado de conducción y de las señales
//! no vive en una tesela. Omitirlo al importar un `.sav` cambia la ubicación
//! de todos los postes de señal y hace que la misma partida se renderice de
//! forma distinta a `OpenTTD`.

use crate::engine::{RoadVehicleAccelerationModel, TrainAccelerationModel};
use crate::flow_stat::{CargoDistSettings, DistributionType};
use crate::town::TownCouncilTolerance;
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

fn linkgraph_distribution_from_u8(value: u8) -> Option<DistributionType> {
    DistributionType::from_openttd(value)
}

fn linkgraph_default_distribution_from_u8(value: u8) -> Option<DistributionType> {
    linkgraph_distribution_from_u8(value)
        .filter(|distribution| !matches!(distribution, DistributionType::Symmetric))
}

fn town_council_tolerance_from_u8(value: u8) -> Option<TownCouncilTolerance> {
    match value {
        0 => Some(TownCouncilTolerance::Lenient),
        1 => Some(TownCouncilTolerance::Neutral),
        2 => Some(TownCouncilTolerance::Hostile),
        3 => Some(TownCouncilTolerance::Permissive),
        _ => None,
    }
}

/// Subconjunto de ajustes de `PATS`/`OPTS` que el core puede ejecutar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools)]
pub(crate) struct ParsedSettings {
    pub construction: ConstructionSettings,
    pub pathfinding: PathfindingSettings,
    pub train_acceleration_model: TrainAccelerationModel,
    pub freight_trains: u8,
    pub road_vehicle_acceleration_model: RoadVehicleAccelerationModel,
    pub station_noise_level: bool,
    pub serve_neutral_industries: bool,
    pub vehicle_breakdowns: u8,
    pub selectgoods: bool,
    pub no_servicing_if_no_breakdowns: bool,
    pub subsidy_duration: u16,
    pub subsidy_multiplier: u8,
    pub disasters_enabled: bool,
    pub town_council_tolerance: TownCouncilTolerance,
    pub using_wallclock_units: bool,
    pub inflation_enabled: bool,
    pub recessions_enabled: bool,
    pub cargo_dist: CargoDistSettings,
}

impl Default for ParsedSettings {
    fn default() -> Self {
        Self {
            construction: ConstructionSettings::default(),
            pathfinding: PathfindingSettings::default(),
            train_acceleration_model: TrainAccelerationModel::Realistic,
            freight_trains: 1,
            road_vehicle_acceleration_model: RoadVehicleAccelerationModel::Realistic,
            station_noise_level: false,
            serve_neutral_industries: true,
            vehicle_breakdowns: 2,
            selectgoods: true,
            no_servicing_if_no_breakdowns: true,
            subsidy_duration: 1,
            subsidy_multiplier: 1,
            disasters_enabled: true,
            town_council_tolerance: TownCouncilTolerance::default(),
            using_wallclock_units: false,
            inflation_enabled: true,
            recessions_enabled: false,
            cargo_dist: CargoDistSettings::default(),
        }
    }
}

/// Lee el subconjunto de ajustes de PATS que el core ejecuta durante la
/// simulación. Los campos desconocidos se ignoran y conservan sus defaults.
#[allow(clippy::too_many_lines)]
pub(crate) fn settings_from_chunks(chunks: &[RawChunk]) -> ParsedSettings {
    let mut parsed = ParsedSettings::default();
    for name in ["PATS", "OPTS"] {
        let Some(chunk) = find_chunk(chunks, name) else {
            continue;
        };
        let Ok(rows) = parse_table_chunk(&chunk.body, false) else {
            continue;
        };
        // `PATS` es la tabla moderna y `OPTS` sólo el fallback de saves
        // antiguos. Si la primera tabla contiene cualquier ajuste conocido,
        // no dejar que el fallback sobrescriba valores explícitos.
        let mut found = false;
        for (_, record) in rows {
            if let Some(value) = record_get(&record, "construction.map_height_limit")
                .and_then(SlValue::as_u64)
                .and_then(|value| u8::try_from(value).ok())
            {
                parsed.construction.map_height_limit = value;
                found = true;
            }
            if let Some(value) = record_get(&record, "vehicle.road_side")
                .and_then(SlValue::as_u64)
                .and_then(|value| u8::try_from(value).ok())
                .and_then(road_side_from_u8)
            {
                parsed.construction.road_vehicle_driving_side = value;
                found = true;
            }
            if let Some(value) = record_get(&record, "construction.train_signal_side")
                .and_then(SlValue::as_u64)
                .and_then(|value| u8::try_from(value).ok())
                .and_then(train_signal_side_from_u8)
            {
                parsed.construction.train_signal_side = value;
                found = true;
            }
            if let Some(value) = record_get(&record, "construction.freeform_edges")
                .and_then(SlValue::as_u64)
                .and_then(bool_from_u64)
            {
                parsed.construction.freeform_edges = value;
                found = true;
            }
            if let Some(value) = record_get(&record, "vehicle.wagon_speed_limits")
                .and_then(SlValue::as_u64)
                .and_then(bool_from_u64)
            {
                parsed.construction.wagon_speed_limits = value;
                found = true;
            }
            if let Some(value) = record_get(&record, "vehicle.disable_elrails")
                .and_then(SlValue::as_u64)
                .and_then(bool_from_u64)
            {
                parsed.construction.disable_elrails = value;
                found = true;
            }
            if let Some(value) = record_get(&record, "vehicle.plane_speed")
                .and_then(SlValue::as_u64)
                .and_then(|value| u8::try_from(value).ok())
            {
                parsed.construction.plane_speed = value.clamp(1, 4);
                found = true;
            }
            if let Some(value) = record_get(&record, "vehicle.plane_crashes")
                .and_then(SlValue::as_u64)
                .and_then(|value| u8::try_from(value).ok())
            {
                parsed.construction.plane_crashes = value.min(2);
                found = true;
            }
            if let Some(value) = record_get(&record, "pf.wait_for_pbs_path")
                .and_then(SlValue::as_u64)
                .and_then(|value| u8::try_from(value).ok())
            {
                parsed.pathfinding.wait_for_pbs_path = value;
                found = true;
            }
            if let Some(value) = record_get(&record, "pf.path_backoff_interval")
                .and_then(SlValue::as_u64)
                .and_then(|value| u8::try_from(value).ok())
            {
                parsed.pathfinding.path_backoff_interval = value;
                found = true;
            }
            if let Some(value) = record_get(&record, "pf.reverse_at_signals")
                .and_then(SlValue::as_u64)
                .and_then(bool_from_u64)
            {
                parsed.pathfinding.reverse_at_signals = value;
                found = true;
            }
            if let Some(value) = record_get(&record, "pf.wait_oneway_signal")
                .and_then(SlValue::as_u64)
                .and_then(|value| u8::try_from(value).ok())
            {
                parsed.pathfinding.wait_oneway_signal = value;
                found = true;
            }
            if let Some(value) = record_get(&record, "pf.wait_twoway_signal")
                .and_then(SlValue::as_u64)
                .and_then(|value| u8::try_from(value).ok())
            {
                parsed.pathfinding.wait_twoway_signal = value;
                found = true;
            }
            if let Some(value) = record_get(&record, "pf.reserve_paths")
                .and_then(SlValue::as_u64)
                .and_then(bool_from_u64)
            {
                parsed.pathfinding.reserve_paths = value;
                found = true;
            }
            if let Some(value) =
                record_get(&record, "vehicle.train_acceleration_model").and_then(SlValue::as_u64)
            {
                parsed.train_acceleration_model = match value {
                    0 => TrainAccelerationModel::Original,
                    1 => TrainAccelerationModel::Realistic,
                    _ => parsed.train_acceleration_model,
                };
                found = true;
            }
            if let Some(value) = record_get(&record, "vehicle.freight_trains")
                .and_then(SlValue::as_u64)
                .and_then(|value| u8::try_from(value).ok())
            {
                parsed.freight_trains = value.max(1);
                found = true;
            }
            if let Some(value) =
                record_get(&record, "vehicle.roadveh_acceleration_model").and_then(SlValue::as_u64)
            {
                parsed.road_vehicle_acceleration_model = match value {
                    0 => RoadVehicleAccelerationModel::Original,
                    1 => RoadVehicleAccelerationModel::Realistic,
                    _ => parsed.road_vehicle_acceleration_model,
                };
                found = true;
            }
            if let Some(value) = record_get(&record, "economy.station_noise_level")
                .and_then(SlValue::as_u64)
                .and_then(bool_from_u64)
            {
                parsed.station_noise_level = value;
                found = true;
            }
            if let Some(value) = record_get(&record, "station.serve_neutral_industries")
                .and_then(SlValue::as_u64)
                .and_then(bool_from_u64)
            {
                parsed.serve_neutral_industries = value;
                found = true;
            }
            if let Some(value) = record_get(&record, "station.distant_join_stations")
                .and_then(SlValue::as_u64)
                .and_then(bool_from_u64)
            {
                parsed.construction.distant_join_stations = value;
                found = true;
            }
            if let Some(value) = record_get(&record, "difficulty.vehicle_breakdowns")
                .and_then(SlValue::as_u64)
                .and_then(|value| u8::try_from(value).ok())
            {
                parsed.vehicle_breakdowns = value.min(2);
                found = true;
            }
            if let Some(value) = record_get(&record, "order.selectgoods")
                .and_then(SlValue::as_u64)
                .and_then(bool_from_u64)
            {
                parsed.selectgoods = value;
                found = true;
            }
            if let Some(value) = record_get(&record, "order.no_servicing_if_no_breakdowns")
                .and_then(SlValue::as_u64)
                .and_then(bool_from_u64)
            {
                parsed.no_servicing_if_no_breakdowns = value;
                found = true;
            }
            if let Some(value) = record_get(&record, "difficulty.subsidy_duration")
                .and_then(SlValue::as_u64)
                .and_then(|value| u16::try_from(value).ok())
            {
                parsed.subsidy_duration = value.min(5_000);
                found = true;
            }
            if let Some(value) = record_get(&record, "difficulty.subsidy_multiplier")
                .and_then(SlValue::as_u64)
                .and_then(|value| u8::try_from(value).ok())
            {
                parsed.subsidy_multiplier = value.min(3);
                found = true;
            }
            if let Some(value) = record_get(&record, "difficulty.disasters")
                .and_then(SlValue::as_u64)
                .and_then(bool_from_u64)
            {
                parsed.disasters_enabled = value;
                found = true;
            }
            if let Some(value) = record_get(&record, "difficulty.town_council_tolerance")
                .and_then(SlValue::as_u64)
                .and_then(|value| u8::try_from(value).ok())
                .and_then(town_council_tolerance_from_u8)
            {
                parsed.town_council_tolerance = value;
                found = true;
            }
            if let Some(value) = record_get(&record, "economy.timekeeping_units")
                .and_then(SlValue::as_u64)
                .and_then(|value| u8::try_from(value).ok())
            {
                parsed.using_wallclock_units = value == 1;
                found = true;
            }
            if let Some(value) = record_get(&record, "economy.inflation")
                .and_then(SlValue::as_u64)
                .and_then(bool_from_u64)
            {
                parsed.inflation_enabled = value;
                found = true;
            }
            if let Some(value) = record_get(&record, "difficulty.economy")
                .and_then(SlValue::as_u64)
                .and_then(bool_from_u64)
            {
                parsed.recessions_enabled = value;
                found = true;
            }

            // LinkGraphSettings aparece en PATS desde SLV_183. Mantener el
            // perfil entero evita convertir cuatro modos de carga en un único
            // enum global al hidratar la simulación.
            let mut linkgraph = parsed.cargo_dist.per_cargo.unwrap_or_default();
            let mut found_linkgraph = false;
            if let Some(value) = record_get(&record, "linkgraph.recalc_interval")
                .and_then(SlValue::as_u64)
                .and_then(|value| u16::try_from(value).ok())
                .filter(|value| (4..=90).contains(value))
            {
                linkgraph.recalc_interval_seconds = value;
                found_linkgraph = true;
            }
            if let Some(value) = record_get(&record, "linkgraph.recalc_time")
                .and_then(SlValue::as_u64)
                .and_then(|value| u16::try_from(value).ok())
                .filter(|value| (1..=9_000).contains(value))
            {
                linkgraph.recalc_time_seconds = value;
                found_linkgraph = true;
            }
            if let Some(value) = record_get(&record, "linkgraph.distribution_pax")
                .and_then(SlValue::as_u64)
                .and_then(|value| u8::try_from(value).ok())
                .and_then(linkgraph_distribution_from_u8)
            {
                linkgraph.distribution_pax = value;
                found_linkgraph = true;
            }
            if let Some(value) = record_get(&record, "linkgraph.distribution_mail")
                .and_then(SlValue::as_u64)
                .and_then(|value| u8::try_from(value).ok())
                .and_then(linkgraph_distribution_from_u8)
            {
                linkgraph.distribution_mail = value;
                found_linkgraph = true;
            }
            if let Some(value) = record_get(&record, "linkgraph.distribution_armoured")
                .and_then(SlValue::as_u64)
                .and_then(|value| u8::try_from(value).ok())
                .and_then(linkgraph_distribution_from_u8)
            {
                linkgraph.distribution_armoured = value;
                found_linkgraph = true;
            }
            if let Some(value) = record_get(&record, "linkgraph.distribution_default")
                .and_then(SlValue::as_u64)
                .and_then(|value| u8::try_from(value).ok())
                .and_then(linkgraph_default_distribution_from_u8)
            {
                linkgraph.distribution_default = value;
                found_linkgraph = true;
            }
            if let Some(value) = record_get(&record, "linkgraph.accuracy")
                .and_then(SlValue::as_u64)
                .and_then(|value| u8::try_from(value).ok())
                .filter(|value| (2..=64).contains(value))
            {
                linkgraph.accuracy = value;
                found_linkgraph = true;
            }
            if let Some(value) = record_get(&record, "linkgraph.demand_distance")
                .and_then(SlValue::as_u64)
                .and_then(|value| u8::try_from(value).ok())
            {
                linkgraph.demand_distance = value;
                found_linkgraph = true;
            }
            if let Some(value) = record_get(&record, "linkgraph.demand_size")
                .and_then(SlValue::as_u64)
                .and_then(|value| u8::try_from(value).ok())
                .filter(|value| *value <= 100)
            {
                linkgraph.demand_size = value;
                found_linkgraph = true;
            }
            if let Some(value) = record_get(&record, "linkgraph.short_path_saturation")
                .and_then(SlValue::as_u64)
                .and_then(|value| u8::try_from(value).ok())
                .filter(|value| *value <= 250)
            {
                linkgraph.short_path_saturation = value;
                found_linkgraph = true;
            }
            if found_linkgraph {
                parsed.cargo_dist.per_cargo = Some(linkgraph);
                found = true;
            }
        }
        if found {
            break;
        }
    }
    parsed
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
        let settings = settings_from_chunks(&[pats([1, 1, 1])]).construction;
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
        let settings = settings_from_chunks(&[pats([0, 0, 0])]).construction;
        assert!(!settings.freeform_edges);
    }

    #[test]
    fn reads_map_height_limit_from_save_table() {
        let body = build_table_body(&[(2, "construction.map_height_limit")], &[vec![75]]);
        let chunk = RawChunk {
            name: *b"PATS",
            ch_type: CH_TABLE,
            body,
        };

        let settings = settings_from_chunks(&[chunk]).construction;
        assert_eq!(settings.map_height_limit, 75);
        assert_eq!(settings.effective_map_height_limit(), 75);
    }

    #[test]
    fn invalid_saved_enums_keep_safe_defaults() {
        let settings = settings_from_chunks(&[pats([9, 7, 2])]).construction;
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
                (2, "vehicle.freight_trains"),
                (2, "vehicle.roadveh_acceleration_model"),
                (2, "economy.station_noise_level"),
                (2, "difficulty.vehicle_breakdowns"),
            ],
            &[vec![2, 3, 0, 4, 5, 1, 0, 4, 0, 1, 2]],
        );
        let chunk = RawChunk {
            name: *b"PATS",
            ch_type: CH_TABLE,
            body,
        };
        let parsed = settings_from_chunks(&[chunk]);
        assert_eq!(parsed.pathfinding.wait_for_pbs_path, 2);
        assert_eq!(parsed.pathfinding.path_backoff_interval, 3);
        assert!(!parsed.pathfinding.reverse_at_signals);
        assert_eq!(parsed.pathfinding.wait_oneway_signal, 4);
        assert_eq!(parsed.pathfinding.wait_twoway_signal, 5);
        assert!(parsed.pathfinding.reserve_paths);
        assert_eq!(
            parsed.train_acceleration_model,
            TrainAccelerationModel::Original
        );
        assert_eq!(parsed.freight_trains, 4);
        assert_eq!(
            parsed.road_vehicle_acceleration_model,
            RoadVehicleAccelerationModel::Original
        );
        assert!(parsed.station_noise_level);
        assert_eq!(parsed.vehicle_breakdowns, 2);
    }

    #[test]
    fn reads_neutral_industry_service_setting() {
        let chunk = RawChunk {
            name: *b"PATS",
            ch_type: CH_TABLE,
            body: build_table_body(&[(1, "station.serve_neutral_industries")], &[vec![0]]),
        };
        let parsed = settings_from_chunks(&[chunk]);
        assert!(!parsed.serve_neutral_industries);
    }

    #[test]
    fn reads_distant_station_join_setting_with_native_default() {
        let disabled = RawChunk {
            name: *b"PATS",
            ch_type: CH_TABLE,
            body: build_table_body(&[(1, "station.distant_join_stations")], &[vec![0]]),
        };
        assert!(settings_from_chunks(&[]).construction.distant_join_stations);
        assert!(
            !settings_from_chunks(&[disabled])
                .construction
                .distant_join_stations
        );
    }

    #[test]
    fn reads_wagon_speed_limits_setting_with_native_default() {
        let disabled = RawChunk {
            name: *b"PATS",
            ch_type: CH_TABLE,
            body: build_table_body(&[(1, "vehicle.wagon_speed_limits")], &[vec![0]]),
        };
        assert!(settings_from_chunks(&[]).construction.wagon_speed_limits);
        assert!(
            !settings_from_chunks(&[disabled])
                .construction
                .wagon_speed_limits
        );
    }

    #[test]
    fn reads_disable_elrails_setting_with_native_default() {
        let disabled = RawChunk {
            name: *b"PATS",
            ch_type: CH_TABLE,
            body: build_table_body(&[(1, "vehicle.disable_elrails")], &[vec![1]]),
        };
        assert!(!settings_from_chunks(&[]).construction.disable_elrails);
        assert!(
            settings_from_chunks(&[disabled])
                .construction
                .disable_elrails
        );
    }

    #[test]
    fn reads_plane_speed_with_native_default_and_clamps_range() {
        assert_eq!(settings_from_chunks(&[]).construction.plane_speed, 4);
        let faster = RawChunk {
            name: *b"PATS",
            ch_type: CH_TABLE,
            body: build_table_body(&[(2, "vehicle.plane_speed")], &[vec![1]]),
        };
        assert_eq!(settings_from_chunks(&[faster]).construction.plane_speed, 1);
        let zero = RawChunk {
            name: *b"PATS",
            ch_type: CH_TABLE,
            body: build_table_body(&[(2, "vehicle.plane_speed")], &[vec![0]]),
        };
        assert_eq!(settings_from_chunks(&[zero]).construction.plane_speed, 1);
        let invalid = RawChunk {
            name: *b"PATS",
            ch_type: CH_TABLE,
            body: build_table_body(&[(2, "vehicle.plane_speed")], &[vec![9]]),
        };
        assert_eq!(settings_from_chunks(&[invalid]).construction.plane_speed, 4);
    }

    #[test]
    fn reads_plane_crashes_with_native_default_and_clamps_range() {
        assert_eq!(settings_from_chunks(&[]).construction.plane_crashes, 2);
        let reduced = RawChunk {
            name: *b"PATS",
            ch_type: CH_TABLE,
            body: build_table_body(&[(2, "vehicle.plane_crashes")], &[vec![1]]),
        };
        assert_eq!(
            settings_from_chunks(&[reduced]).construction.plane_crashes,
            1
        );
        let invalid = RawChunk {
            name: *b"PATS",
            ch_type: CH_TABLE,
            body: build_table_body(&[(2, "vehicle.plane_crashes")], &[vec![9]]),
        };
        assert_eq!(
            settings_from_chunks(&[invalid]).construction.plane_crashes,
            2
        );
    }

    #[test]
    fn freight_train_weight_multiplier_has_minimum_one() {
        let chunk = RawChunk {
            name: *b"PATS",
            ch_type: CH_TABLE,
            body: build_table_body(&[(2, "vehicle.freight_trains")], &[vec![0]]),
        };

        assert_eq!(settings_from_chunks(&[chunk]).freight_trains, 1);
    }

    #[test]
    fn reads_gameplay_settings_with_native_widths() {
        let body = build_table_body(
            &[
                (1, "order.selectgoods"),
                (1, "order.no_servicing_if_no_breakdowns"),
                (4, "difficulty.subsidy_duration"),
                (2, "difficulty.subsidy_multiplier"),
                (2, "difficulty.disasters"),
                (2, "difficulty.town_council_tolerance"),
                (2, "economy.timekeeping_units"),
                (2, "economy.inflation"),
                (2, "difficulty.economy"),
            ],
            &[{
                let mut record = vec![0, 0];
                record.extend_from_slice(&5_000u16.to_be_bytes());
                record.extend_from_slice(&[3, 0, 3, 1, 0, 1]);
                record
            }],
        );
        let chunk = RawChunk {
            name: *b"PATS",
            ch_type: CH_TABLE,
            body,
        };
        let parsed = settings_from_chunks(&[chunk]);
        assert!(!parsed.selectgoods);
        assert!(!parsed.no_servicing_if_no_breakdowns);
        assert_eq!(parsed.subsidy_duration, 5_000);
        assert_eq!(parsed.subsidy_multiplier, 3);
        assert!(!parsed.disasters_enabled);
        assert_eq!(
            parsed.town_council_tolerance,
            TownCouncilTolerance::Permissive
        );
        assert!(parsed.using_wallclock_units);
        assert!(!parsed.inflation_enabled);
        assert!(parsed.recessions_enabled);
    }

    #[test]
    fn reads_selectgoods_from_legacy_opts_fallback() {
        let chunk = RawChunk {
            name: *b"OPTS",
            ch_type: CH_TABLE,
            body: build_table_body(&[(1, "order.selectgoods")], &[vec![0]]),
        };

        assert!(!settings_from_chunks(&[chunk]).selectgoods);
    }

    #[test]
    fn reads_linkgraph_profile_with_native_seconds_and_four_classes() {
        let body = build_table_body(
            &[
                (4, "linkgraph.recalc_interval"),
                (4, "linkgraph.recalc_time"),
                (2, "linkgraph.distribution_pax"),
                (2, "linkgraph.distribution_mail"),
                (2, "linkgraph.distribution_armoured"),
                (2, "linkgraph.distribution_default"),
                (2, "linkgraph.accuracy"),
                (2, "linkgraph.demand_distance"),
                (2, "linkgraph.demand_size"),
                (2, "linkgraph.short_path_saturation"),
            ],
            &[{
                let mut record = Vec::new();
                record.extend_from_slice(&5_u16.to_be_bytes());
                record.extend_from_slice(&9_000_u16.to_be_bytes());
                record.extend_from_slice(&[2, 1, 0, 1, 64, 255, 0, 250]);
                record
            }],
        );
        let parsed = settings_from_chunks(&[RawChunk {
            name: *b"PATS",
            ch_type: CH_TABLE,
            body,
        }]);
        assert!(
            parsed.cargo_dist.per_cargo.is_some(),
            "PATS linkgraph profile"
        );
        let profile = parsed.cargo_dist.per_cargo.unwrap_or_default();
        assert_eq!(profile.recalc_interval_seconds, 5);
        assert_eq!(profile.recalc_time_seconds, 9_000);
        assert_eq!(profile.distribution_pax, DistributionType::Symmetric);
        assert_eq!(profile.distribution_mail, DistributionType::Asymmetric);
        assert_eq!(profile.distribution_armoured, DistributionType::Manual);
        assert_eq!(profile.distribution_default, DistributionType::Asymmetric);
        assert_eq!(profile.accuracy, 64);
        assert_eq!(profile.demand_distance, 255);
        assert_eq!(profile.demand_size, 0);
        assert_eq!(profile.short_path_saturation, 250);
        assert_eq!(parsed.cargo_dist.effective_recalc_interval_days(), 2);
    }
}
