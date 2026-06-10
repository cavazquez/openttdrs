//! Animación del agua por ciclo de paleta, fiel a `DoPaletteAnimations`
//! (`palette.cpp`): los índices 245–249 (dark water) y 250–254 (glitter
//! water) ciclan sus colores. Como este cliente usa sprites RGBA, los frames
//! del ciclo vienen pre-horneados (`scripts/gen_water_anim_frames.py`) y aquí
//! solo se intercambia la imagen — global, igual que la paleta del original.

use bevy::prelude::*;

use crate::bevy_app::UpdateSet;
use crate::render::{ShoreTile, WaterAnimFrames, WaterTile};
use crate::state::ClientScreen;

pub(crate) struct WaterAnimationPlugin;

impl Plugin for WaterAnimationPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            animate_water
                .in_set(UpdateSet::Visuals)
                .run_if(in_state(ClientScreen::InGame)),
        );
    }
}

/// Frames del ciclo completo (dark water 5 pasos × glitter 15 pasos).
pub(crate) const WATER_FRAME_COUNT: usize = 15;

/// Duración de cada paso. En OpenTTD el contador avanza 8/tick (~30 ms) y el
/// dark water cambia de entrada cada ~150 ms (`EXTR(320, 5)`).
const WATER_FRAME_SECS: f32 = 0.15;

/// Frame global del ciclo de agua en el instante `elapsed_secs` (puro, testeable).
#[must_use]
pub(crate) fn water_frame_index(elapsed_secs: f32) -> usize {
    (elapsed_secs / WATER_FRAME_SECS) as usize % WATER_FRAME_COUNT
}

/// Intercambia los sprites de agua/orilla al frame del ciclo actual.
/// Solo escribe cuando el frame global cambia (~6–7 veces por segundo).
pub(crate) fn animate_water(
    time: Res<Time>,
    frames: Option<Res<WaterAnimFrames>>,
    mut last_frame: Local<Option<usize>>,
    mut water_q: Query<&mut Sprite, (With<WaterTile>, Without<ShoreTile>)>,
    mut shore_q: Query<(&ShoreTile, &mut Sprite), Without<WaterTile>>,
) {
    let Some(frames) = frames else {
        return;
    };
    let idx = water_frame_index(time.elapsed_secs());
    if *last_frame == Some(idx) {
        return;
    }
    *last_frame = Some(idx);
    for mut sprite in &mut water_q {
        sprite.image = frames.water[idx].clone();
    }
    for (shore, mut sprite) in &mut shore_q {
        if let Some(shore_set) = frames.shore.get(usize::from(shore.0)) {
            sprite.image = shore_set[idx].clone();
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use bevy::ecs::system::RunSystemOnce;

    use super::*;

    fn weak_handle(n: u128) -> Handle<Image> {
        Handle::Uuid(
            bevy::asset::uuid::Uuid::from_u128(n),
            std::marker::PhantomData,
        )
    }

    fn frames_resource() -> WaterAnimFrames {
        WaterAnimFrames {
            water: (0..WATER_FRAME_COUNT as u128).map(weak_handle).collect(),
            shore: (0..18u128)
                .map(|i| {
                    (0..WATER_FRAME_COUNT as u128)
                        .map(|f| weak_handle(2000 + i * 100 + f))
                        .collect()
                })
                .collect(),
        }
    }

    #[test]
    fn frame_index_cycles_over_time() {
        assert_eq!(water_frame_index(0.0), 0);
        assert_eq!(water_frame_index(0.16), 1);
        assert_eq!(water_frame_index(WATER_FRAME_COUNT as f32 * 0.15 + 0.01), 0);
    }

    #[test]
    fn animate_water_swaps_images_on_frame_change() {
        let mut world = World::new();
        let mut time = Time::<()>::default();
        time.advance_by(std::time::Duration::from_millis(400));
        world.insert_resource(time);
        world.insert_resource(frames_resource());
        let water = world.spawn((WaterTile, Sprite::default())).id();
        let shore = world.spawn((ShoreTile(3), Sprite::default())).id();

        world.run_system_once(animate_water).unwrap();

        let frames = world.resource::<WaterAnimFrames>();
        let expected_idx = water_frame_index(0.4);
        let expected_water = frames.water[expected_idx].clone();
        let expected_shore = frames.shore[3][expected_idx].clone();
        assert_eq!(world.get::<Sprite>(water).unwrap().image, expected_water);
        assert_eq!(world.get::<Sprite>(shore).unwrap().image, expected_shore);
    }

    #[test]
    fn animate_water_without_frames_resource_is_noop() {
        let mut world = World::new();
        world.insert_resource(Time::<()>::default());
        let water = world.spawn((WaterTile, Sprite::default())).id();

        world.run_system_once(animate_water).unwrap();

        assert_eq!(world.get::<Sprite>(water).unwrap().image, Handle::default());
    }
}
