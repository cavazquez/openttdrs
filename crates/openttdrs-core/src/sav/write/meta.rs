//! Chunks de metadatos (DATE, PLYR).

use crate::game_state::GameState;
use crate::news::{CALENDAR_BASE_YEAR, calendar_day_index, calendar_year_day};

use super::super::SavError;
use super::chunks::table_chunk;

/// Fecha `OpenTTD` aproximada (días desde año 0) + tick monotónico.
pub(super) fn date_record(state: &GameState) -> Vec<u8> {
    let day_index = calendar_day_index(state.tick);
    let (year, doy) = calendar_year_day(day_index);
    // Aproximación: 365 * year + (doy - 1). Suficiente para roundtrip interno;
    // OpenTTD usa calendario gregoriano real — ver docs/PLANIFICACION.md § Export SAV.
    let calendar_date = i32::try_from(u64::from(year) * 365 + (doy.saturating_sub(1)))
        .unwrap_or(i32::try_from(u64::from(CALENDAR_BASE_YEAR) * 365).unwrap_or(0));
    let mut rec = Vec::with_capacity(12);
    rec.extend_from_slice(&calendar_date.to_be_bytes());
    rec.extend_from_slice(&state.tick.get().to_be_bytes());
    rec
}

pub(super) fn plyr_record(state: &GameState) -> Vec<u8> {
    let mut rec = Vec::with_capacity(9);
    rec.extend_from_slice(&state.economy.money.to_be_bytes());
    rec.push(state.company_colour);
    rec
}

/// Ajustes de construcción que afectan cómo OpenTTD interpreta el mapa al
/// cargarlo. El header deliberadamente contiene sólo campos que el core
/// modela; los demás settings de PATS conservan los defaults del juego.
pub(super) fn pats_chunk(state: &GameState) -> Result<Vec<u8>, SavError> {
    let road_side = u8::from(state.construction.road_drive_on_right());
    let signal_side = match state.construction.train_signal_side {
        crate::TrainSignalSide::Left => 0,
        crate::TrainSignalSide::RoadVehicleDrivingSide => 1,
        crate::TrainSignalSide::Right => 2,
    };
    table_chunk(
        *b"PATS",
        &[
            (2, "vehicle.road_side"),
            (2, "construction.train_signal_side"),
            (2, "construction.freeform_edges"),
        ],
        &[vec![
            road_side,
            signal_side,
            u8::from(state.construction.freeform_edges),
        ]],
    )
}
