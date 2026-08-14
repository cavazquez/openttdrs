//! Ajustes de construcción persistidos por `OpenTTD` en `PATS` / `OPTS`.
//!
//! A diferencia de los bytes de mapa, el lado de conducción y de las señales
//! no vive en una tesela. Omitirlo al importar un `.sav` cambia la ubicación
//! de todos los postes de señal y hace que la misma partida se renderice de
//! forma distinta a `OpenTTD`.

use crate::{ConstructionSettings, RoadVehicleDrivingSide, TrainSignalSide};

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

/// Lee los ajustes que determinan el lado de los vehículos y las señales.
///
/// `PATS` es el chunk moderno del savegame; `OPTS` cubre variantes antiguas.
/// Si el campo falta o tiene una enumeración desconocida se conserva el valor
/// por defecto, igual que `OpenTTD` al cargar un save anterior al ajuste.
#[must_use]
pub(crate) fn construction_settings_from_chunks(chunks: &[RawChunk]) -> ConstructionSettings {
    for name in ["PATS", "OPTS"] {
        let Some(chunk) = find_chunk(chunks, name) else {
            continue;
        };
        let Ok(rows) = parse_table_chunk(&chunk.body, false) else {
            continue;
        };
        let mut settings = ConstructionSettings::default();
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
        }
        if found {
            return settings;
        }
    }
    ConstructionSettings::default()
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
        let settings = construction_settings_from_chunks(&[pats([1, 1, 1])]);
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
        let settings = construction_settings_from_chunks(&[pats([0, 0, 0])]);
        assert!(!settings.freeform_edges);
    }

    #[test]
    fn invalid_saved_enums_keep_safe_defaults() {
        let settings = construction_settings_from_chunks(&[pats([9, 7, 2])]);
        assert_eq!(settings, ConstructionSettings::default());
    }
}
