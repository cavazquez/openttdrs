//! Bebidas gaseosas — ciclo de paleta `fizzy_drink[5]` (`palette.cpp`).

use bevy::prelude::*;

use crate::bevy_app::UpdateSet;
use crate::render::{
    FizzyDrinkAnimFrames, PaletteAnimationClock, palette_animation_phase_reverse,
    palette_animations_should_run,
};
use crate::state::ClientScreen;

pub(crate) struct FizzyDrinkAnimPlugin;

impl Plugin for FizzyDrinkAnimPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            animate_fizzy_drink
                .in_set(UpdateSet::Visuals)
                .run_if(in_state(ClientScreen::InGame))
                .run_if(palette_animations_should_run),
        );
    }
}

/// Edificio u overlay con burbujas animadas por paleta.
#[derive(Component, Clone, Copy)]
pub(crate) struct FizzyDrinkAnim {
    pub(crate) sprite_id: u32,
}

/// Pasos del ciclo `EPV_CYCLES_FIZZY_DRINK`.
pub(crate) const FIZZY_DRINK_FRAME_COUNT: usize = 5;

#[must_use]
pub(crate) const fn fizzy_drink_frame_index(counter: u16) -> usize {
    palette_animation_phase_reverse(counter, 512, FIZZY_DRINK_FRAME_COUNT as u16)
}

pub(crate) fn animate_fizzy_drink(
    clock: Res<PaletteAnimationClock>,
    frames: Option<Res<FizzyDrinkAnimFrames>>,
    mut last_frame: Local<Option<usize>>,
    mut q: Query<(&FizzyDrinkAnim, &mut Sprite)>,
) {
    let Some(frames) = frames else {
        return;
    };
    let idx = fizzy_drink_frame_index(clock.counter());
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

    #[test]
    fn frame_index_cycles() {
        assert_eq!(fizzy_drink_frame_index(0), 4);
        assert_eq!(fizzy_drink_frame_index(64), 2);
        assert_eq!(fizzy_drink_frame_index(104), 0);
    }

    #[test]
    fn animate_swaps_on_frame_change() {
        let mut world = World::new();
        world.insert_resource(PaletteAnimationClock::from_counter(64));
        let set: Vec<_> = (0..FIZZY_DRINK_FRAME_COUNT as u128)
            .map(weak_sprite)
            .collect();
        world.insert_resource(FizzyDrinkAnimFrames {
            by_sprite: [(4764u32, set)].into_iter().collect(),
        });
        let ent = world
            .spawn((FizzyDrinkAnim { sprite_id: 4764 }, Sprite::default()))
            .id();

        world.run_system_once(animate_fizzy_drink).unwrap();

        let frames = world.resource::<FizzyDrinkAnimFrames>();
        let idx = fizzy_drink_frame_index(64);
        let expected = frames.by_sprite[&4764][idx].clone();
        assert!(expected.matches(world.get::<Sprite>(ent).unwrap()));
    }
}
