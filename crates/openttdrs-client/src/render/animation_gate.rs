//! Condición de ejecución para animaciones de paleta (`FullAnimation` de OpenTTD).

use bevy::prelude::*;

use crate::settings::ClientPreferences;
use crate::state::{SimRunState, sim_is_paused};

/// `true` si deben correr ciclos de paleta (agua, refinería, fizzy, etc.).
///
/// Equivalente a OpenTTD: `FullAnimation` activo y el juego no pausado.
/// En menú principal (sin `SimRunState`) solo mira la preferencia.
#[must_use]
pub(crate) fn palette_animations_should_run(
    prefs: Res<ClientPreferences>,
    run_state: Option<Res<State<SimRunState>>>,
) -> bool {
    animations_enabled(prefs.full_animation, run_state.as_deref())
}

#[must_use]
pub(crate) fn animations_enabled(
    full_animation: bool,
    run_state: Option<&State<SimRunState>>,
) -> bool {
    if !full_animation {
        return false;
    }
    match run_state {
        Some(rs) => !sim_is_paused(rs),
        None => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::SimRunState;

    #[test]
    fn off_pref_disables_even_when_running() {
        assert!(!animations_enabled(
            false,
            Some(&State::new(SimRunState::Running))
        ));
    }

    #[test]
    fn pause_disables_when_pref_on() {
        assert!(!animations_enabled(
            true,
            Some(&State::new(SimRunState::Paused))
        ));
    }

    #[test]
    fn running_with_pref_enables() {
        assert!(animations_enabled(
            true,
            Some(&State::new(SimRunState::Running))
        ));
    }

    #[test]
    fn no_run_state_follows_pref() {
        assert!(animations_enabled(true, None));
        assert!(!animations_enabled(false, None));
    }
}
