//! Parpadeo de radio de la boya (`SPR_IMG_BUOY`, índices 239/240).
//!
//! OpenTTD cambia esos dos colores en `DoPaletteAnimations` sin cambiar el
//! sprite lógico. Como el atlas de Bevy es RGBA, los cuatro estados posibles
//! se generan desde el sheet 8bpp y se seleccionan con el mismo contador.

use bevy::prelude::*;

use crate::bevy_app::UpdateSet;
use crate::render::{PaletteAnimationClock, RadioBlinkAnimFrames, palette_animations_should_run};
use crate::state::ClientScreen;

pub(crate) struct RadioBlinkAnimPlugin;

impl Plugin for RadioBlinkAnimPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            animate_radio_blink
                .in_set(UpdateSet::Visuals)
                .run_if(in_state(ClientScreen::InGame))
                .run_if(palette_animations_should_run),
        );
    }
}

/// Sprite vanilla de boya que debe seguir el ciclo global de paleta.
#[derive(Component, Clone, Copy)]
pub(crate) struct RadioBlinkAnim;

/// OpenTTD avanza el contador global ocho unidades por tick de 30 ms.
pub(crate) const RADIO_BLINK_FRAME_COUNT: usize = 4;
const RADIO_BLINK_TICK_SECS: f32 = 0.03;
const RADIO_COUNTER_STEP: u16 = 8;

/// Valor rojo que `palette.cpp` escribe para una posición del contador.
#[must_use]
const fn radio_palette_value(index: u8) -> u8 {
    if index < 0x3f {
        255
    } else if index < 0x4a || index >= 0x75 {
        128
    } else {
        20
    }
}

/// Índice del frame RGBA equivalente al ciclo de `RadioTowerBlink`.
#[must_use]
pub(crate) fn radio_blink_frame_index(elapsed_secs: f32) -> usize {
    let ticks = (elapsed_secs.max(0.0) / RADIO_BLINK_TICK_SECS).floor() as u64;
    let counter = ticks.wrapping_mul(u64::from(RADIO_COUNTER_STEP)) as u16;
    let first = ((counter >> 1) & 0x7f) as u8;
    let values = (
        radio_palette_value(first),
        radio_palette_value(first ^ 0x40),
    );
    match values {
        (255, 128) => 0,
        (255, 20) => 1,
        (128, 255) => 2,
        (20, 255) => 3,
        // Los ticks de OpenTTD son múltiplos de cuatro en este índice; este
        // fallback conserva un estado válido si el helper se usa con otra
        // cadencia durante una prueba o una captura.
        _ => 0,
    }
}

pub(crate) fn animate_radio_blink(
    clock: Res<PaletteAnimationClock>,
    frames: Option<Res<RadioBlinkAnimFrames>>,
    mut last_frame: Local<Option<usize>>,
    mut q: Query<(&RadioBlinkAnim, &mut Sprite)>,
) {
    let Some(frames) = frames else {
        return;
    };
    let idx = radio_blink_frame_index(clock.elapsed_secs());
    if *last_frame == Some(idx) {
        return;
    }
    *last_frame = Some(idx);
    let Some(frame) = frames.0.get(idx) else {
        return;
    };
    for (_anim, mut sprite) in &mut q {
        frame.apply_to(&mut sprite);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_index_matches_the_four_palette_states() {
        let at_tick = |tick: u32| tick as f32 * RADIO_BLINK_TICK_SECS + 0.0001;
        assert_eq!(radio_blink_frame_index(0.0), 0);
        assert_eq!(radio_blink_frame_index(at_tick(3)), 1);
        assert_eq!(radio_blink_frame_index(at_tick(16)), 2);
        assert_eq!(radio_blink_frame_index(at_tick(19)), 3);
    }

    #[test]
    fn frame_index_is_bounded_for_long_elapsed_time() {
        assert!(radio_blink_frame_index(10_000.0) < RADIO_BLINK_FRAME_COUNT);
    }
}
