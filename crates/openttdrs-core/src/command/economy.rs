//! Préstamos de compañía (`misc_cmd.cpp` / `economy.cpp`).

use crate::GameState;
use crate::economy::{decrease_loan, increase_loan};

use super::types::CommandError;

pub(crate) fn increase_company_loan(state: &mut GameState) -> Result<(), CommandError> {
    increase_loan(&mut state.economy)?;
    Ok(())
}

pub(crate) fn decrease_company_loan(state: &mut GameState) -> Result<(), CommandError> {
    decrease_loan(&mut state.economy)?;
    Ok(())
}
