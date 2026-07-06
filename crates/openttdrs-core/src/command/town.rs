//! Acciones de ciudad (`town_cmd.cpp` simplificado).

use crate::GameState;
use crate::town::{
    FUND_BUILDINGS_COST, FUND_BUILDINGS_RATING_BOOST, TOWN_ADVERTISE_COST,
    TOWN_ADVERTISE_RATING_BOOST,
};

use super::types::CommandError;

pub(crate) fn town_advertise(state: &mut GameState, town_id: u32) -> Result<(), CommandError> {
    let idx = town_index(state, town_id)?;
    if state.economy.money < TOWN_ADVERTISE_COST {
        return Err(CommandError::InsufficientFunds);
    }
    state.economy.money -= TOWN_ADVERTISE_COST;
    let delta = state.towns[idx].adjust_rating(TOWN_ADVERTISE_RATING_BOOST);
    state
        .pending_sim_events
        .push(crate::sim_events::SimEvent::TownRatingChanged { town_id, delta });
    Ok(())
}

pub(crate) fn town_fund_buildings(state: &mut GameState, town_id: u32) -> Result<(), CommandError> {
    let idx = town_index(state, town_id)?;
    if state.economy.money < FUND_BUILDINGS_COST {
        return Err(CommandError::InsufficientFunds);
    }
    state.economy.money -= FUND_BUILDINGS_COST;
    let delta = state.towns[idx].adjust_rating(FUND_BUILDINGS_RATING_BOOST);
    state.towns[idx].growth_funded += 1;
    state
        .pending_sim_events
        .push(crate::sim_events::SimEvent::TownRatingChanged { town_id, delta });
    Ok(())
}

fn town_index(state: &GameState, town_id: u32) -> Result<usize, CommandError> {
    state
        .towns
        .iter()
        .position(|t| t.id == town_id)
        .ok_or(CommandError::TownNotFound)
}
