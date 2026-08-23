//! Fuego / metal fundido — ciclo de paleta `oil_refinery[7]` (`palette.cpp`).
//!
//! Frames pre-horneados por `scripts/gen_oil_refinery_anim_frames.py`;
//! refinería gfx 19–22 y suelos de acería gfx 52–57.

use bevy::prelude::*;

use crate::bevy_app::UpdateSet;
use crate::render::{RefineryFireAnimFrames, palette_animations_should_run};
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

/// OpenTTD: `EXTR2(512, 7)` sobre contador +8/tick ≈ un paso cada ~120 ms.
const REFINERY_FRAME_SECS: f32 = 0.12;

/// Frame global del ciclo en `elapsed_secs` (puro, testeable).
#[must_use]
pub(crate) fn refinery_fire_frame_index(elapsed_secs: f32) -> usize {
    (elapsed_secs / REFINERY_FRAME_SECS) as usize % REFINERY_FIRE_FRAME_COUNT
}

/// Usa reloj real: el virtual tiene `max_delta` de 1 tick de sim y puede
/// quedar pausado sin afectar el parpadeo de paleta (como el agua).
pub(crate) fn animate_refinery_fire(
    time: Res<Time<Real>>,
    frames: Option<Res<RefineryFireAnimFrames>>,
    mut last_frame: Local<Option<usize>>,
    mut q: Query<(&RefineryFireAnim, &mut Sprite)>,
) {
    let Some(frames) = frames else {
        return;
    };
    let idx = refinery_fire_frame_index(time.elapsed_secs());
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
        assert_eq!(refinery_fire_frame_index(0.0), 0);
        assert_eq!(refinery_fire_frame_index(0.13), 1);
        assert_eq!(
            refinery_fire_frame_index(REFINERY_FIRE_FRAME_COUNT as f32 * 0.12 + 0.01),
            0
        );
    }

    #[test]
    fn animate_refinery_fire_swaps_on_frame_change() {
        let mut world = World::new();
        let mut time = Time::<Real>::default();
        time.advance_by(std::time::Duration::from_millis(250));
        world.insert_resource(time);
        world.insert_resource(frames_resource());
        let ent = world
            .spawn((RefineryFireAnim { sprite_id: 2086 }, Sprite::default()))
            .id();

        world.run_system_once(animate_refinery_fire).unwrap();

        let frames = world.resource::<RefineryFireAnimFrames>();
        let idx = refinery_fire_frame_index(0.25);
        let expected = frames.by_sprite[&2086][idx].clone();
        assert!(expected.matches(world.get::<Sprite>(ent).unwrap()));
    }
}
