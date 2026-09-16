//! Fuego / metal fundido — ciclo de paleta `oil_refinery[7]` (`palette.cpp`).
//!
//! Frames pre-horneados por `scripts/gen_oil_refinery_anim_frames.py`;
//! refinería gfx 19–22 y suelos de acería gfx 52–57.

use bevy::prelude::*;

use crate::bevy_app::UpdateSet;
use crate::render::{
    PaletteAnimationClock, RefineryFireAnimFrames, palette_animation_phase_reverse,
    palette_animations_should_run,
};
use crate::state::ClientScreen;

pub(crate) struct RefineryFireAnimPlugin;

impl Plugin for RefineryFireAnimPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            animate_refinery_fire
                .in_set(UpdateSet::Visuals)
                .run_if(in_state(ClientScreen::InGame))
                .run_if(palette_animations_should_run),
        );
    }
}

/// Capa de edificio con llama animada (gfx 19–22 terminado).
#[derive(Component, Clone, Copy)]
pub(crate) struct RefineryFireAnim {
    pub(crate) sprite_id: u32,
}

/// Pasos del ciclo `EPV_CYCLES_OIL_REFINERY`.
pub(crate) const REFINERY_FIRE_FRAME_COUNT: usize = 7;

/// OpenTTD: `EXTR2(512, 7)` sobre el contador global +8 por pasada.
#[must_use]
pub(crate) const fn refinery_fire_frame_index(counter: u16) -> usize {
    palette_animation_phase_reverse(counter, 512, REFINERY_FIRE_FRAME_COUNT as u16)
}

pub(crate) fn animate_refinery_fire(
    clock: Res<PaletteAnimationClock>,
    frames: Option<Res<RefineryFireAnimFrames>>,
    mut last_frame: Local<Option<usize>>,
    mut q: Query<(&RefineryFireAnim, &mut Sprite)>,
) {
    let Some(frames) = frames else {
        return;
    };
    let idx = refinery_fire_frame_index(clock.counter());
    if *last_frame == Some(idx) {
        return;
    }
    *last_frame = Some(idx);
    for (anim, mut sprite) in &mut q {
        if let Some(set) = frames.by_sprite.get(&anim.sprite_id)
            && let Some(atlas) = set.get(idx)
        {
            atlas.apply_to(&mut sprite);
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use bevy::ecs::system::RunSystemOnce;

    use super::*;
    use crate::render::AtlasSprite;

    fn weak_sprite(n: u128) -> AtlasSprite {
        AtlasSprite {
            image: Handle::Uuid(
                bevy::asset::uuid::Uuid::from_u128(1),
                std::marker::PhantomData,
            ),
            atlas: TextureAtlas {
                layout: Handle::Uuid(
                    bevy::asset::uuid::Uuid::from_u128(2),
                    std::marker::PhantomData,
                ),
                index: n as usize,
            },
            size: Vec2::ONE,
        }
    }

    fn frames_resource() -> RefineryFireAnimFrames {
        let set: Vec<_> = (0..REFINERY_FIRE_FRAME_COUNT as u128)
            .map(weak_sprite)
            .collect();
        RefineryFireAnimFrames {
            by_sprite: [(2086u32, set)].into_iter().collect(),
        }
    }

    #[test]
    fn frame_index_cycles() {
        assert_eq!(refinery_fire_frame_index(0), 6);
        assert_eq!(refinery_fire_frame_index(64), 3);
        assert_eq!(refinery_fire_frame_index(112), 0);
    }

    #[test]
    fn animate_refinery_fire_swaps_on_frame_change() {
        let mut world = World::new();
        world.insert_resource(PaletteAnimationClock::from_counter(64));
        world.insert_resource(frames_resource());
        let ent = world
            .spawn((RefineryFireAnim { sprite_id: 2086 }, Sprite::default()))
            .id();

        world.run_system_once(animate_refinery_fire).unwrap();

        let frames = world.resource::<RefineryFireAnimFrames>();
        let idx = refinery_fire_frame_index(64);
        let expected = frames.by_sprite[&2086][idx].clone();
        assert!(expected.matches(world.get::<Sprite>(ent).unwrap()));
    }
}
