//! Condición de ejecución para animaciones de paleta (`FullAnimation` de OpenTTD).

use bevy::prelude::*;

use crate::bevy_app::UpdateSet;
use crate::settings::ClientPreferences;
use crate::state::{SimRunState, sim_is_paused};

/// Reloj acumulado del bucle `DoPaletteAnimations` de OpenTTD.
///
/// Se alimenta con tiempo real para no depender de la velocidad de la
/// simulación, pero el sistema que lo avanza queda bajo el mismo gate que la
/// animación completa. Así una pausa no consume tiempo de presentación.
#[derive(Resource, Debug, Clone, Copy, Default)]
pub(crate) struct PaletteAnimationClock {
    elapsed_secs: f32,
}

impl PaletteAnimationClock {
    #[cfg(test)]
    pub(crate) fn from_elapsed_secs(elapsed_secs: f32) -> Self {
        Self {
            elapsed_secs: elapsed_secs.max(0.0),
        }
    }

    #[must_use]
    pub(crate) const fn elapsed_secs(self) -> f32 {
        self.elapsed_secs
    }
}

pub(crate) struct PaletteAnimationClockPlugin;

impl Plugin for PaletteAnimationClockPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PaletteAnimationClock>().add_systems(
            PreUpdate,
            advance_palette_animation_clock
                .in_set(UpdateSet::Visuals)
                .run_if(palette_animations_should_run),
        );
    }
}

fn advance_palette_animation_clock(
    time: Res<Time<Real>>,
    mut clock: ResMut<PaletteAnimationClock>,
) {
    clock.elapsed_secs += time.delta_secs().max(0.0);
}

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

    #[test]
    fn palette_clock_stops_while_paused_and_resumes_from_real_delta() {
        let mut world = World::new();
        world.insert_resource(ClientPreferences::default());
        world.insert_resource(State::new(SimRunState::Paused));
        world.insert_resource(PaletteAnimationClock::from_elapsed_secs(1.0));
        let mut time = Time::<Real>::default();
        time.advance_by(std::time::Duration::from_millis(500));
        world.insert_resource(time);

        let mut schedule = Schedule::default();
        schedule.add_systems(advance_palette_animation_clock.run_if(palette_animations_should_run));
        schedule.run(&mut world);
        assert_eq!(
            world.resource::<PaletteAnimationClock>().elapsed_secs(),
            1.0
        );

        world.insert_resource(State::new(SimRunState::Running));
        schedule.run(&mut world);
        assert_eq!(
            world.resource::<PaletteAnimationClock>().elapsed_secs(),
            1.5
        );
    }
}
