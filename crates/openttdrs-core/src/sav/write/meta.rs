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

pub(super) fn plyr_records(state: &GameState) -> Vec<Vec<u8>> {
    if state.companies.is_empty() {
        let mut rec = Vec::with_capacity(9);
        rec.extend_from_slice(&state.economy.money.to_be_bytes());
        rec.push(state.company_colour);
        return vec![rec];
    }
    state
        .companies
        .iter()
        .map(|company| {
            let mut rec = Vec::with_capacity(9);
            let (money, colour) = if company.id == state.active_company {
                (state.economy.money, state.company_colour)
            } else {
                (company.economy.money, company.colour)
            };
            rec.extend_from_slice(&money.to_be_bytes());
            rec.push(colour);
            rec
        })
        .collect()
}

/// Ajustes de partida que afectan cómo `OpenTTD` interpreta y simula el mapa al
/// cargarlo. El header contiene el subconjunto que el core modela; los demás
/// settings de PATS conservan los defaults del juego.
pub(super) fn pats_chunk(state: &GameState) -> Result<Vec<u8>, SavError> {
    let landscape = match state.climate {
        crate::Climate::Temperate => 0,
        crate::Climate::SubArctic => 1,
        crate::Climate::SubTropical => 2,
        crate::Climate::Toyland => 3,
    };
    let road_side = u8::from(state.construction.road_drive_on_right());
    let signal_side = match state.construction.train_signal_side {
        crate::TrainSignalSide::Left => 0,
        crate::TrainSignalSide::RoadVehicleDrivingSide => 1,
        crate::TrainSignalSide::Right => 2,
    };
    table_chunk(
        *b"PATS",
        &[
            (2, "game_creation.landscape"),
            (2, "vehicle.road_side"),
            (2, "construction.train_signal_side"),
            (2, "construction.freeform_edges"),
            (2, "pf.wait_for_pbs_path"),
            (2, "pf.path_backoff_interval"),
            (2, "pf.reverse_at_signals"),
            (2, "pf.wait_oneway_signal"),
            (2, "pf.wait_twoway_signal"),
            (2, "pf.reserve_paths"),
            (2, "vehicle.train_acceleration_model"),
            (2, "economy.station_noise_level"),
            (2, "difficulty.vehicle_breakdowns"),
            (2, "order.no_servicing_if_no_breakdowns"),
            (4, "difficulty.subsidy_duration"),
            (2, "difficulty.subsidy_multiplier"),
            (2, "difficulty.disasters"),
            (2, "difficulty.town_council_tolerance"),
            (2, "economy.timekeeping_units"),
            (2, "economy.inflation"),
            (2, "difficulty.economy"),
        ],
        &[{
            let mut record = vec![
                landscape,
                road_side,
                signal_side,
                u8::from(state.construction.freeform_edges),
                state.pathfinding.wait_for_pbs_path,
                state.pathfinding.path_backoff_interval,
                u8::from(state.pathfinding.reverse_at_signals),
                state.pathfinding.wait_oneway_signal,
                state.pathfinding.wait_twoway_signal,
                u8::from(state.pathfinding.reserve_paths),
                state.train_acceleration_model as u8,
                u8::from(state.station_noise_level),
                state.vehicle_breakdowns.min(2),
                u8::from(state.no_servicing_if_no_breakdowns),
            ];
            record.extend_from_slice(&state.subsidy_duration.to_be_bytes());
            record.extend_from_slice(&[
                state.subsidy_multiplier.min(3),
                u8::from(state.disasters_enabled),
                state.town_council_tolerance as u8,
                u8::from(state.using_wallclock_units),
                u8::from(state.global_economy.inflation_enabled),
                u8::from(state.global_economy.recessions_enabled),
            ]);
            record
        }],
    )
}

/// Serializa el registro global `ECMY` que `OpenTTD` usa para reanudar inflación,
/// recesiones y el reparto diario de cambios de industria.
pub(super) fn ecmy_chunk(state: &GameState) -> Result<Vec<u8>, SavError> {
    let economy = &state.global_economy;
    let mut record = Vec::with_capacity(30);
    record.extend_from_slice(&economy.inflation_prices.to_be_bytes());
    record.extend_from_slice(&economy.inflation_payment.to_be_bytes());
    record.extend_from_slice(&economy.fluct.to_be_bytes());
    record.push(economy.interest_rate);
    record.push(economy.infl_amount);
    record.push(economy.infl_amount_pr);
    record.extend_from_slice(&economy.industry_daily_change_counter.to_be_bytes());
    table_chunk(
        *b"ECMY",
        &[
            (8, "inflation_prices"),
            (8, "inflation_payment"),
            (3, "fluct"),
            (2, "interest_rate"),
            (2, "infl_amount"),
            (2, "infl_amount_pr"),
            (6, "industry_daily_change_counter"),
        ],
        &[record],
    )
}

/// Serializa el pool `CAPY` preservado desde un save. El runtime no crea
/// nuevos pagos activos, por lo que un estado recién iniciado simplemente no
/// emite este chunk.
pub(super) fn capy_chunk(state: &GameState) -> Result<Option<Vec<u8>>, SavError> {
    if state.cargo_payments.is_empty() {
        return Ok(None);
    }
    let max_id = state
        .cargo_payments
        .iter()
        .map(|payment| payment.id)
        .max()
        .unwrap_or(0);
    let Some(record_count) = usize::try_from(max_id)
        .ok()
        .and_then(|id| id.checked_add(1))
    else {
        return Err(SavError::BadFormat("pool CAPY demasiado grande".into()));
    };
    let mut records = vec![Vec::new(); record_count];
    for payment in &state.cargo_payments {
        let Ok(id) = usize::try_from(payment.id) else {
            return Err(SavError::BadFormat("índice CAPY fuera de rango".into()));
        };
        let Some(record) = records.get_mut(id) else {
            return Err(SavError::BadFormat("índice CAPY fuera de rango".into()));
        };
        let front = payment
            .front_vehicle_ref
            .map_or(0, |reference| reference.saturating_add(1));
        record.extend_from_slice(&front.to_be_bytes());
        record.extend_from_slice(&payment.route_profit.to_be_bytes());
        record.extend_from_slice(&payment.visual_profit.to_be_bytes());
        record.extend_from_slice(&payment.visual_transfer.to_be_bytes());
    }
    table_chunk(
        *b"CAPY",
        &[
            (6, "front"),
            (7, "route_profit"),
            (7, "visual_profit"),
            (7, "visual_transfer"),
        ],
        &records,
    )
    .map(Some)
}
