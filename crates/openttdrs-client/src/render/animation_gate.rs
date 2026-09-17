//! Condición de ejecución para animaciones de paleta (`FullAnimation` de OpenTTD).

use bevy::prelude::*;

use crate::settings::ClientPreferences;
use crate::state::{SimRunState, sim_is_paused};

/// Reloj acumulado del bucle `DoPaletteAnimations` de OpenTTD.
///
/// OpenTTD suma ocho unidades por pasada elegible del bucle principal. Se
/// conserva sólo el `u16` que consumen las macros `EXTR`/`EXTR2`; así el cliente
/// comparte fase exacta entre todos los ciclos y una pausa no consume tiempo
/// de presentación.
#[derive(Resource, Debug, Clone, Copy)]
pub(crate) struct PaletteAnimationClock {
    counter: u16,
}

impl Default for PaletteAnimationClock {
    fn default() -> Self {
        // `GfxInitPalettes` ejecuta una primera pasada de
        // `DoPaletteAnimations` antes de que OpenTTD dibuje la escena.
        Self {
            counter: PALETTE_ANIMATION_COUNTER_STEP,
        }
    }
}

impl PaletteAnimationClock {
    #[cfg(test)]
    pub(crate) const fn from_counter(counter: u16) -> Self {
        Self { counter }
    }

    #[must_use]
    pub(crate) const fn counter(self) -> u16 {
        self.counter
    }
}

/// Paso de `palette_animation_counter` en `DoPaletteAnimations`.
pub(crate) const PALETTE_ANIMATION_COUNTER_STEP: u16 = 8;

/// Equivalente a `EXTR(p, q)` de `palette.cpp`, incluida la truncación `u16`.
#[must_use]
pub(crate) const fn palette_animation_phase(counter: u16, multiplier: u16, phases: u16) -> usize {
    let wrapped = counter.wrapping_mul(multiplier);
    (((wrapped as u32) * (phases as u32)) >> 16) as usize
}

/// Equivalente a `EXTR2(p, q)` de `palette.cpp`.
#[must_use]
pub(crate) const fn palette_animation_phase_reverse(
    counter: u16,
    multiplier: u16,
    phases: u16,
) -> usize {
    let wrapped = (!counter).wrapping_mul(multiplier);
    (((wrapped as u32) * (phases as u32)) >> 16) as usize
}

pub(crate) struct PaletteAnimationClockPlugin;

impl Plugin for PaletteAnimationClockPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PaletteAnimationClock>().add_systems(
            PreUpdate,
            advance_palette_animation_clock.run_if(palette_animations_should_run),
        );
    }
}

fn advance_palette_animation_clock(mut clock: ResMut<PaletteAnimationClock>) {
    clock.counter = clock.counter.wrapping_add(PALETTE_ANIMATION_COUNTER_STEP);
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
        world.insert_resource(PaletteAnimationClock::from_counter(1_000));

        let mut schedule = Schedule::default();
        schedule.add_systems(advance_palette_animation_clock.run_if(palette_animations_should_run));
        schedule.run(&mut world);
        assert_eq!(world.resource::<PaletteAnimationClock>().counter(), 1_000);

        world.insert_resource(State::new(SimRunState::Running));
        schedule.run(&mut world);
        assert_eq!(world.resource::<PaletteAnimationClock>().counter(), 1_008);
    }

    #[test]
    fn palette_phase_matches_openttd_extr_and_extr2() {
        assert_eq!(palette_animation_phase(0, 256, 4), 0);
        assert_eq!(palette_animation_phase(64, 256, 4), 1);
        assert_eq!(palette_animation_phase_reverse(0, 512, 5), 4);
        assert_eq!(palette_animation_phase_reverse(64, 512, 5), 2);
        assert_eq!(palette_animation_phase_reverse(64, 512, 7), 3);
    }

    #[test]
    fn palette_clock_starts_after_native_palette_initialization() {
        assert_eq!(
            PaletteAnimationClock::default().counter(),
            PALETTE_ANIMATION_COUNTER_STEP
        );
    }
}
