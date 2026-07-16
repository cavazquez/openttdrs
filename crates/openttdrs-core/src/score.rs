//! Puntuación de fin de partida (highscore / endscreen).

use crate::game_state::{GameState, company_net_value};
use crate::rail_signals::calendar_year_at_tick;

/// Motivo del cierre de partida.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum GameOverReason {
    /// Tres cierres mensuales consecutivos en quiebra.
    Bankruptcy,
    /// El jugador retiró la compañía (fin voluntario).
    Retired,
}

impl GameOverReason {
    #[must_use]
    pub const fn label_es(self) -> &'static str {
        match self {
            Self::Bankruptcy => "Quiebra",
            Self::Retired => "Retiro",
        }
    }

    #[must_use]
    pub const fn storage_code(self) -> char {
        match self {
            Self::Bankruptcy => 'B',
            Self::Retired => 'R',
        }
    }

    #[must_use]
    pub fn from_storage_code(c: char) -> Option<Self> {
        match c {
            'B' | 'b' => Some(Self::Bankruptcy),
            'R' | 'r' => Some(Self::Retired),
            _ => None,
        }
    }
}

/// Snapshot de puntuación al cerrar la partida.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct GameScore {
    pub company_name: String,
    pub company_value: i64,
    pub calendar_year: u32,
    pub reason: GameOverReason,
}

/// Meses consecutivos en quiebra antes de `GameOver`.
pub const BANKRUPTCY_STREAK_LIMIT: u8 = 3;

/// Construye el snapshot de la compañía activa.
#[must_use]
pub fn snapshot_active_score(state: &GameState, reason: GameOverReason) -> GameScore {
    let (name, money, loan) = state
        .companies
        .get(state.active_company.index())
        .map_or_else(
            || ("Jugador".into(), state.economy.money, state.economy.loan),
            |c| (c.name.clone(), c.economy.money, c.economy.loan),
        );
    GameScore {
        company_name: name,
        company_value: company_net_value(money, loan),
        calendar_year: calendar_year_at_tick(state.tick),
        reason,
    }
}

/// Emite `GameOver` una sola vez y marca la partida como terminada.
pub fn finish_game(state: &mut GameState, reason: GameOverReason) -> Option<GameScore> {
    if state.game_finished {
        return None;
    }
    let score = snapshot_active_score(state, reason);
    state.game_finished = true;
    state
        .runtime
        .pending_sim_events
        .push(crate::sim_events::SimEvent::GameOver {
            company_name: score.company_name.clone(),
            company_value: score.company_value,
            calendar_year: score.calendar_year,
            reason: score.reason,
        });
    Some(score)
}

/// Retiro voluntario (UI / consola).
pub fn retire_game(state: &mut GameState) -> Option<GameScore> {
    finish_game(state, GameOverReason::Retired)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn finish_game_emits_once() {
        let mut state = GameState::new(8, 8);
        let first = finish_game(&mut state, GameOverReason::Retired).expect("first");
        assert!(state.game_finished);
        assert_eq!(first.reason, GameOverReason::Retired);
        assert!(finish_game(&mut state, GameOverReason::Bankruptcy).is_none());
    }

    #[test]
    fn reason_storage_roundtrip() {
        assert_eq!(
            GameOverReason::from_storage_code(GameOverReason::Bankruptcy.storage_code()),
            Some(GameOverReason::Bankruptcy)
        );
        assert_eq!(
            GameOverReason::from_storage_code(GameOverReason::Retired.storage_code()),
            Some(GameOverReason::Retired)
        );
    }
}
