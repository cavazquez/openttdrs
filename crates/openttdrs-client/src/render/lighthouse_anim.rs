//! Faro / estadio — ciclo de paleta `lighthouse[4]` (`palette.cpp`).
//!
//! Frames pre-horneados por `scripts/gen_lighthouse_anim_frames.py`.
//! Sprites: object 2602 (faro) y house s2 1483–1486 (luces de estadio).

use bevy::prelude::*;

use crate::bevy_app::UpdateSet;
use crate::render::{
    LighthouseAnimFrames, PaletteAnimationClock, palette_animation_phase,
    palette_animations_should_run,
};
use crate::state::ClientScreen;

pub(crate) struct LighthouseAnimPlugin;

impl Plugin for LighthouseAnimPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            animate_lighthouse
                .in_set(UpdateSet::Visuals)
                .run_if(in_state(ClientScreen::InGame))
                .run_if(palette_animations_should_run),
        );
    }
}

/// Capa con luz animada (faro o estadio).
#[derive(Component, Clone, Copy)]
pub(crate) struct LighthouseAnim {
    pub(crate) sprite_id: u32,
}

/// Pasos del ciclo `EPV_CYCLES_LIGHTHOUSE`.
pub(crate) const LIGHTHOUSE_FRAME_COUNT: usize = 4;

/// OpenTTD: `EXTR(256, 4)` sobre el contador global +8 por pasada.
#[must_use]
pub(crate) const fn lighthouse_frame_index(counter: u16) -> usize {
    palette_animation_phase(counter, 256, LIGHTHOUSE_FRAME_COUNT as u16)
}

pub(crate) fn animate_lighthouse(
    clock: Res<PaletteAnimationClock>,
    frames: Option<Res<LighthouseAnimFrames>>,
    mut last_frame: Local<Option<usize>>,
    mut q: Query<(&LighthouseAnim, &mut Sprite)>,
) {
    let Some(frames) = frames else {
        return;
    };
    let idx = lighthouse_frame_index(clock.counter());
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

    fn frames_resource() -> LighthouseAnimFrames {
        let set: Vec<_> = (0..LIGHTHOUSE_FRAME_COUNT as u128)
            .map(weak_sprite)
            .collect();
        LighthouseAnimFrames {
            by_sprite: [(2602u32, set)].into_iter().collect(),
        }
    }

    #[test]
    fn frame_index_cycles() {
        assert_eq!(lighthouse_frame_index(0), 0);
        assert_eq!(lighthouse_frame_index(64), 1);
        assert_eq!(lighthouse_frame_index(128), 2);
        assert_eq!(lighthouse_frame_index(192), 3);
    }

    #[test]
    fn animate_lighthouse_swaps_on_frame_change() {
        let mut world = World::new();
        world.insert_resource(PaletteAnimationClock::from_counter(64));
        world.insert_resource(frames_resource());
        let ent = world
            .spawn((LighthouseAnim { sprite_id: 2602 }, Sprite::default()))
            .id();

        world.run_system_once(animate_lighthouse).unwrap();

        let frames = world.resource::<LighthouseAnimFrames>();
        let idx = lighthouse_frame_index(64);
        let expected = frames.by_sprite[&2602][idx].clone();
        assert!(expected.matches(world.get::<Sprite>(ent).unwrap()));
    }
}
