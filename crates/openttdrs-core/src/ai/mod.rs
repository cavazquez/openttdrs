//! Hooks para decisión automática de compañías (rivales CPU).
//!
//! Ver `docs/epics/ai_rivals.md`. La medición headless vive en [`crate::dev_metrics`].

mod rule_based;

pub use rule_based::TransCargoAi;

use crate::GameState;
use crate::command::Command;
use crate::economy::TICKS_PER_MONTH;

/// Política que propone comandos para una compañía (rival o bot de pruebas).
pub trait CompanyAi {
    fn decide(&self, state: &GameState) -> Vec<Command>;
}

/// Ciclo mensual de IA (Fase 4c).
pub fn tick_ai_companies(state: &mut GameState, tick: u64) {
    if tick == 0 || !tick.is_multiple_of(TICKS_PER_MONTH) {
        return;
    }
    if !state.companies.iter().any(|c| c.is_ai) {
        return;
    }
    rule_based::tick_transcargo(state);
}
