//! Parpadeo de radio de la boya (`SPR_IMG_BUOY`, índices 239/240).
//!
//! OpenTTD cambia esos dos colores en `DoPaletteAnimations` sin cambiar el
//! sprite lógico. Como el atlas de Bevy es RGBA, los cuatro estados posibles
//! se generan desde el sheet 8bpp y se seleccionan con el mismo contador.

use bevy::prelude::*;

use crate::bevy_app::UpdateSet;
use crate::render::{PaletteAnimationClock, RadioBlinkAnimFrames};
use crate::state::ClientScreen;

pub(crate) struct RadioBlinkAnimPlugin;

impl Plugin for RadioBlinkAnimPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            animate_radio_blink
                .in_set(UpdateSet::Visuals)
                .run_if(in_state(ClientScreen::InGame)),
        );
    }
}

/// Sprite vanilla de boya que debe seguir el ciclo global de paleta.
#[derive(Component, Clone, Copy)]
pub(crate) struct RadioBlinkAnim;

/// OpenTTD avanza el contador global ocho unidades por pasada elegible.
pub(crate) const RADIO_BLINK_FRAME_COUNT: usize = 4;

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
pub(crate) const fn radio_blink_frame_index(counter: u16) -> usize {
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
    mut sprites: ParamSet<(
        Query<&mut Sprite, With<RadioBlinkAnim>>,
        Query<&mut Sprite, (With<RadioBlinkAnim>, Added<RadioBlinkAnim>)>,
    )>,
) {
    let Some(frames) = frames else {
        return;
    };
    let idx = radio_blink_frame_index(clock.counter());
    let Some(frame) = frames.0.get(idx) else {
        return;
    };
    let frame_changed = *last_frame != Some(idx) || frames.is_changed();
    *last_frame = Some(idx);
    if frame_changed {
        for mut sprite in sprites.p0().iter_mut() {
            frame.apply_to(&mut sprite);
        }
    } else {
        // A rebuild can materialize a new buoy after the global phase was
        // already observed. It still needs the current RGBA frame while the
        // palette gate is disabled (for example in a CLEAN capture).
        for mut sprite in sprites.p1().iter_mut() {
            frame.apply_to(&mut sprite);
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::needless_borrow, clippy::unwrap_used)]
mod tests {
    use bevy::ecs::system::RunSystemOnce;

    use super::*;
    use crate::render::animation_gate::PALETTE_ANIMATION_COUNTER_STEP;

    fn test_atlas_sprite(index: usize) -> crate::render::AtlasSprite {
        crate::render::AtlasSprite {
            image: Handle::Uuid(
                bevy::asset::uuid::Uuid::from_u128(1),
                std::marker::PhantomData,
            ),
            atlas: TextureAtlas {
                layout: Handle::Uuid(
                    bevy::asset::uuid::Uuid::from_u128(2),
                    std::marker::PhantomData,
                ),
                index,
            },
            size: Vec2::ONE,
        }
    }

    #[test]
    fn frame_index_matches_the_four_palette_states() {
        let at_tick = |tick: u16| tick.wrapping_mul(PALETTE_ANIMATION_COUNTER_STEP);
        assert_eq!(radio_blink_frame_index(0), 0);
        assert_eq!(radio_blink_frame_index(at_tick(3)), 1);
        assert_eq!(radio_blink_frame_index(at_tick(16)), 2);
        assert_eq!(radio_blink_frame_index(at_tick(19)), 3);
    }

    #[test]
    fn frame_index_is_bounded_for_long_elapsed_time() {
        assert!(radio_blink_frame_index(u16::MAX) < RADIO_BLINK_FRAME_COUNT);
    }

    #[test]
    fn syncs_new_buoy_to_current_frame_when_animation_is_frozen() {
        let frames: Vec<_> = (0..RADIO_BLINK_FRAME_COUNT)
            .map(test_atlas_sprite)
            .collect();
        let expected = frames[radio_blink_frame_index(PALETTE_ANIMATION_COUNTER_STEP)].clone();
        let mut world = World::new();
        world.insert_resource(PaletteAnimationClock::default());
        world.insert_resource(RadioBlinkAnimFrames(frames));
        let entity = world
            .spawn((RadioBlinkAnim, test_atlas_sprite(0).sprite()))
            .id();

        world.run_system_once(animate_radio_blink).unwrap();

        assert!(expected.matches(&world.get::<Sprite>(entity).unwrap()));
    }
}
