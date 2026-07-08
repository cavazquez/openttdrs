//! Hooks para decisión automática de compañías (rivales CPU, `DevBot` futuro).
//!
//! Ver `docs/epics/ai_rivals.md`. La medición headless vive en [`crate::dev_metrics`].

use crate::GameState;
use crate::command::Command;

/// Política que propone comandos para una compañía (rival o bot de pruebas).
pub trait CompanyAi {
    fn decide(&self, state: &GameState) -> Vec<Command>;
}
