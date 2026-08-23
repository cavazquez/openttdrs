//! Chunks de metadatos (DATE, PLYR).

use crate::game_state::GameState;
use crate::news::{CALENDAR_BASE_YEAR, calendar_day_index, calendar_year_day};

use super::super::SavError;
use super::super::chunks::CH_TABLE;
use super::chunks::{raw_table_chunk, table_chunk};
use super::codec::{write_gamma, write_str};

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

fn append_company_settings(
    record: &mut Vec<u8>,
    company: &crate::company::Company,
    engine_renew_list_head: Option<u16>,
) -> Result<(), SavError> {
    // `settings` es un struct de una sola entrada. OpenTTD aplana los nombres
    // de sus subcampos (`settings.vehicle.*`) en ese header.
    write_gamma(1, record)?;
    // `SLE_REF(..., REF_ENGINE_RENEWS)`: null = 0, los IDs del pool se
    // serializan como `index + 1` y desde SLV_69 ocupan u32.
    let renew_head = engine_renew_list_head.map_or(0, |id| u32::from(id) + 1);
    record.extend_from_slice(&renew_head.to_be_bytes());
    record.push(u8::from(company.engine_renew));
    record.extend_from_slice(&company.engine_renew_months.to_be_bytes());
    let money = u32::try_from(company.engine_renew_money.max(0)).unwrap_or(u32::MAX);
    record.extend_from_slice(&money.to_be_bytes());
    record.push(u8::from(company.renew_keep_length));
    record.push(u8::from(company.servint_ispercent));
    record.extend_from_slice(&company.servint_trains.to_be_bytes());
    record.extend_from_slice(&company.servint_roadveh.to_be_bytes());
    record.extend_from_slice(&company.servint_aircraft.to_be_bytes());
    record.extend_from_slice(&company.servint_ships.to_be_bytes());
    Ok(())
}

fn append_company_liveries(
    record: &mut Vec<u8>,
    company: &crate::company::Company,
) -> Result<(), SavError> {
    let liveries = company.effective_liveries();
    let count = u32::try_from(liveries.len())
        .map_err(|_| SavError::BadFormat("demasiadas libreas de compañía".into()))?;
    write_gamma(count, record)?;
    for livery in liveries {
        record.push(livery.in_use);
        record.push(livery.colour1);
        record.push(livery.colour2);
    }
    Ok(())
}

fn plyr_records(
    state: &GameState,
    autoreplace_export: &super::fleet::AutoreplaceExport,
) -> Result<Vec<Vec<u8>>, SavError> {
    if state.companies.is_empty() {
        let mut rec = Vec::with_capacity(112);
        let company = crate::company::Company::player(
            crate::game_state::CompanyEconomy::default(),
            state.company_colour,
        );
        write_str("Jugador", &mut rec)?;
        write_str(company.president_name.as_deref().unwrap_or(""), &mut rec)?;
        rec.extend_from_slice(&company.manager_face.to_be_bytes());
        write_str(
            company.manager_face_style.as_deref().unwrap_or(""),
            &mut rec,
        )?;
        rec.extend_from_slice(&state.economy.money.to_be_bytes());
        rec.extend_from_slice(&state.economy.loan.to_be_bytes());
        rec.push(state.company_colour);
        rec.push(0);
        rec.push(company.bankruptcy_months);
        append_company_settings(
            &mut rec,
            &company,
            autoreplace_export.company_head(crate::CompanyId::PLAYER),
        )?;
        append_company_liveries(&mut rec, &company)?;
        return Ok(vec![rec]);
    }
    state
        .companies
        .iter()
        .map(|company| {
            let mut rec = Vec::with_capacity(112);
            let (money, loan, colour) = if company.id == state.active_company {
                (
                    state.economy.money,
                    state.economy.loan,
                    state.company_colour,
                )
            } else {
                (company.economy.money, company.economy.loan, company.colour)
            };
            // El writer es inmutable; normalizar la copia evita emitir un
            // `colour` espejo distinto del esquema por defecto en estados
            // creados por JSON/tests anteriores a `Company::set_colour`.
            let mut company_to_write = company.clone();
            if company_to_write.colour != colour {
                company_to_write.set_colour(colour);
            }
            write_str(&company.name, &mut rec)?;
            write_str(company.president_name.as_deref().unwrap_or(""), &mut rec)?;
            rec.extend_from_slice(&company.manager_face.to_be_bytes());
            write_str(
                company.manager_face_style.as_deref().unwrap_or(""),
                &mut rec,
            )?;
            rec.extend_from_slice(&money.to_be_bytes());
            rec.extend_from_slice(&loan.to_be_bytes());
            rec.push(colour);
            rec.push(u8::from(company.is_ai));
            rec.push(company.bankruptcy_months);
            append_company_settings(
                &mut rec,
                &company_to_write,
                autoreplace_export.company_head(company.id),
            )?;
            append_company_liveries(&mut rec, &company_to_write)?;
            Ok(rec)
        })
        .collect()
}

/// Serializa `PLYR` con el subconjunto de `CompanySettings` que ejecuta el
/// core. La estructura anidada es importante: `OpenTTD` usa los nombres de
/// `settings.*` para aplicar compatibilidad entre versiones del save.
pub(super) fn plyr_chunk(
    state: &GameState,
    autoreplace_export: &super::fleet::AutoreplaceExport,
) -> Result<Vec<u8>, SavError> {
    let mut header = Vec::new();
    header.push(0x0A | 0x10);
    write_str("name", &mut header)?;
    header.push(0x0A | 0x10);
    write_str("president_name", &mut header)?;
    header.push(6);
    write_str("face", &mut header)?;
    header.push(0x0A | 0x10);
    write_str("face_style", &mut header)?;
    header.push(7);
    write_str("money", &mut header)?;
    header.push(7);
    write_str("current_loan", &mut header)?;
    header.push(2);
    write_str("colour", &mut header)?;
    header.push(1);
    write_str("is_ai", &mut header)?;
    header.push(2);
    write_str("months_of_bankruptcy", &mut header)?;
    header.push(0x1B);
    write_str("settings", &mut header)?;
    header.push(0x1B);
    write_str("liveries", &mut header)?;
    header.push(0);

    header.push(6);
    write_str("engine_renew_list", &mut header)?;
    header.push(1);
    write_str("settings.engine_renew", &mut header)?;
    header.push(3);
    write_str("settings.engine_renew_months", &mut header)?;
    header.push(6);
    write_str("settings.engine_renew_money", &mut header)?;
    header.push(1);
    write_str("settings.renew_keep_length", &mut header)?;
    header.push(1);
    write_str("settings.vehicle.servint_ispercent", &mut header)?;
    header.push(4);
    write_str("settings.vehicle.servint_trains", &mut header)?;
    header.push(4);
    write_str("settings.vehicle.servint_roadveh", &mut header)?;
    header.push(4);
    write_str("settings.vehicle.servint_aircraft", &mut header)?;
    header.push(4);
    write_str("settings.vehicle.servint_ships", &mut header)?;
    header.push(0);

    header.push(2);
    write_str("in_use", &mut header)?;
    header.push(2);
    write_str("colour1", &mut header)?;
    header.push(2);
    write_str("colour2", &mut header)?;
    header.push(0);

    raw_table_chunk(
        *b"PLYR",
        &header,
        &plyr_records(state, autoreplace_export)?,
        CH_TABLE,
    )
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
            (1, "construction.freeform_edges"),
            (2, "pf.wait_for_pbs_path"),
            (2, "pf.path_backoff_interval"),
            (1, "pf.reverse_at_signals"),
            (2, "pf.wait_oneway_signal"),
            (2, "pf.wait_twoway_signal"),
            (1, "pf.reserve_paths"),
            (2, "vehicle.train_acceleration_model"),
            (1, "economy.station_noise_level"),
            (2, "difficulty.vehicle_breakdowns"),
            (1, "order.no_servicing_if_no_breakdowns"),
            (4, "difficulty.subsidy_duration"),
            (2, "difficulty.subsidy_multiplier"),
            (1, "difficulty.disasters"),
            (2, "difficulty.town_council_tolerance"),
            (2, "economy.timekeeping_units"),
            (1, "economy.inflation"),
            (1, "difficulty.economy"),
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
