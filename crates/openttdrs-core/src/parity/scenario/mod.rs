//! Escenarios determinísticos para reproducir casos de paridad en headless.
//!
//! Familias: [`road`], [`rail`], [`economy`], [`ai`].

mod ai;
mod economy;
mod rail;
mod road;

#[cfg(test)]
mod tests;

pub use ai::build_ai_rival_line;
pub use economy::{build_breakdown, build_loan_interest, build_town_growth};
pub(crate) use rail::release_staged_depot_trains;
#[allow(unused_imports)] // constantes demo usadas por tests / tooling externo
pub use rail::{
    RAIL_SIGNALS_DEMO_BLOCKER2_ID, RAIL_SIGNALS_DEMO_DEPOT, RAIL_SIGNALS_DEMO_ENTRY,
    RAIL_SIGNALS_DEMO_EXIT1, RAIL_SIGNALS_DEMO_EXIT2, RAIL_SIGNALS_DEMO_FACTORY,
    RAIL_SIGNALS_DEMO_LEAD_ID, RAIL_SIGNALS_DEMO_LOAD_STATION, RAIL_SIGNALS_DEMO_MAIN_Y,
    RAIL_SIGNALS_DEMO_MINE, RAIL_SIGNALS_DEMO_PLAT1_Y, RAIL_SIGNALS_DEMO_PLAT2_Y,
    RAIL_SIGNALS_DEMO_TWO_WAY_EAST, RAIL_SIGNALS_DEMO_TWO_WAY_WEST,
    RAIL_SIGNALS_DEMO_UNLOAD_STATION, RAIL_SIGNALS_MIXED_TYPES, RAIL_SIGNALS_MIXED_Y,
    TRAIN_DUAL_COAL_MINE, TRAIN_DUAL_DEPOT, TRAIN_DUAL_DEPOT_EXIT, TRAIN_DUAL_FACTORY,
    TRAIN_DUAL_STATION_A, TRAIN_DUAL_STATION_B, TRAIN_DUAL_TRACK_OUT_Y, TRAIN_DUAL_TRACK_RET_Y,
    TRAIN_DUAL_VEHICLE_2_ID, TRAIN_DUAL_VEHICLE_ID, TRAIN_DUAL_VEHICLE_OUT_ID, TRAIN_LINE_CORNER,
    TRAIN_LINE_DEPOT, TRAIN_LINE_SIGNAL, TRAIN_LINE_STATION_A, TRAIN_LINE_STATION_B,
    TRAIN_LINE_VEHICLE_ID, TRAIN_PBS_GOAL_X, TRAIN_PBS_NORTH_ID, TRAIN_PBS_NORTH_Y,
    TRAIN_PBS_PATH_A, TRAIN_PBS_PATH_B, TRAIN_PBS_SOUTH_ID, TRAIN_PBS_SOUTH_Y,
    TRAIN_SIGNAL_BLOCK_TILE, TRAIN_SIGNAL_BLOCKER_ID, TRAIN_SIGNAL_LEAD_ID, TRAIN_SIGNAL_TILE,
    TRAIN_SUPPLY_BLOCK_TILE, TRAIN_SUPPLY_BLOCKER_ID, TRAIN_SUPPLY_FACTORY, TRAIN_SUPPLY_MINE,
    TRAIN_SUPPLY_SIGNAL_EAST, TRAIN_SUPPLY_SIGNAL_SOUTH, TRAIN_SUPPLY_SIGNAL_WEST,
    TRAIN_SUPPLY_VEHICLE_ID, TRAIN_SUPPLY_WAIT_SIGNAL, build_rail_signals_mixed, build_train_line,
    build_train_pbs, build_train_signal, build_train_supply, build_train_supply_dual,
    build_train_supply_signal_snapshot, rail_signals_mixed_coord,
};
pub use road::{
    TRUCK_BAY_DELIVER_ROAD, TRUCK_BAY_DELIVER_STOP, TRUCK_BAY_LOAD_ROAD, TRUCK_BAY_LOAD_STOP,
    TRUCK_BAY_VEHICLE_ID, build_truck_bay,
};

use crate::GameState;

/// Construye un escenario determinístico por nombre.
#[must_use]
pub fn build_scenario(name: &str) -> Option<GameState> {
    match name {
        "truck_bay" => Some(build_truck_bay()),
        "train_line" => Some(build_train_line()),
        "train_supply" => Some(build_train_supply()),
        "train_supply_dual" => Some(build_train_supply_dual()),
        "train_supply_signal" => Some(build_train_supply_signal_snapshot()),
        "train_signal" => Some(build_train_signal()),
        "train_pbs" => Some(build_train_pbs()),
        "ai_rival_line" => Some(build_ai_rival_line()),
        "rail_signals_mixed" => Some(build_rail_signals_mixed()),
        "loan_interest" => Some(build_loan_interest()),
        "town_growth" => Some(build_town_growth()),
        "breakdown" => Some(build_breakdown()),
        _ => None,
    }
}

/// Nombres de escenarios disponibles.
#[must_use]
pub fn scenario_names() -> &'static [&'static str] {
    &[
        "truck_bay",
        "train_line",
        "train_supply",
        "train_supply_dual",
        "train_supply_signal",
        "train_signal",
        "train_pbs",
        "ai_rival_line",
        "rail_signals_mixed",
        "loan_interest",
        "town_growth",
        "breakdown",
    ]
}

/// Exporta un escenario de paridad / Junctionary a JSON (`save::save`).
///
/// # Errors
///
/// Escenario desconocido o fallo de E/S / serialización.
pub fn export_junction_json(
    name: &str,
    path: &std::path::Path,
) -> Result<(), crate::save::SaveError> {
    let Some(state) = build_scenario(name) else {
        return Err(crate::save::SaveError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("escenario desconocido: {name}"),
        )));
    };
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)?;
    }
    crate::save::save(&state, path)
}
