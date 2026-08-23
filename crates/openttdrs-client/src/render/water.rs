//! Animación del agua por ciclo de paleta, fiel a `DoPaletteAnimations`
//! (`palette.cpp`): los índices 245–249 (dark water) y 250–254 (glitter
//! water) ciclan sus colores. Como este cliente usa sprites RGBA, los frames
//! del ciclo vienen pre-horneados (`scripts/gen_water_anim_frames.py`) y aquí
//! se redirigen 19 entradas compartidas del atlas, como la paleta global original.

use bevy::prelude::*;

use crate::bevy_app::UpdateSet;
use crate::render::{
    AtlasSprite, WaterAnimFrames, WaterAtlasAnimation, WorldAssets, palette_animations_should_run,
};
use crate::state::ClientScreen;

pub(crate) struct WaterAnimationPlugin;

/// Contador determinista de writes del atlas por cambio de frame. Permite
/// medir el pico sin depender del tiempo de pared ni del número de entidades.
#[derive(Resource, Default)]
pub(crate) struct WaterAnimationStats {
    pub(crate) last_rect_writes: usize,
    pub(crate) peak_rect_writes: usize,
}

impl Plugin for WaterAnimationPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WaterAnimationStats>().add_systems(
            Update,
            animate_water
                .in_set(UpdateSet::Visuals)
                .run_if(in_state(ClientScreen::MainMenu).or_else(in_state(ClientScreen::InGame)))
                .run_if(palette_animations_should_run),
        );
    }
}

/// Cadencia del loop de paleta de OpenTTD. En cada paso el contador suma 8.
const PALETTE_TICK_SECS: f32 = 0.03;
const PALETTE_COUNTER_STEP: u16 = 8;

/// Equivalente a `EXTR(p, q)` de `palette.cpp`, incluida la truncación u16.
#[must_use]
const fn palette_phase(counter: u16, multiplier: u16, phases: u16) -> usize {
    let wrapped = counter.wrapping_mul(multiplier);
    ((wrapped as u32 * phases as u32) >> 16) as usize
}

/// Fases `(dark, glitter)` para un valor del contador de `DoPaletteAnimations`.
#[must_use]
pub(crate) const fn water_palette_phases(counter: u16) -> (usize, usize) {
    (
        palette_phase(counter, 320, crate::sprites::DARK_WATER_FRAME_COUNT as u16),
        palette_phase(
            counter,
            128,
            crate::sprites::GLITTER_WATER_FRAME_COUNT as u16,
        ),
    )
}

/// Fases independientes en el instante indicado, usando el tick real de paleta.
#[must_use]
pub(crate) fn water_frame_indices(elapsed_secs: f32) -> (usize, usize) {
    let ticks = (elapsed_secs.max(0.0) / PALETTE_TICK_SECS).floor() as u64;
    let counter = (ticks.wrapping_mul(u64::from(PALETTE_COUNTER_STEP))) as u16;
    water_palette_phases(counter)
}

#[must_use]
const fn combined_frame_index(dark: usize, glitter: usize) -> usize {
    dark * crate::sprites::GLITTER_WATER_FRAME_COUNT + glitter
}

fn atlas_animation_target(
    target: &AtlasSprite,
    frames: &[AtlasSprite],
    layouts: &Assets<TextureAtlasLayout>,
) -> Option<WaterAtlasAnimation> {
    if frames.len() != crate::sprites::WATER_PALETTE_FRAME_COUNT
        || frames
            .iter()
            .any(|frame| frame.image != target.image || frame.atlas.layout != target.atlas.layout)
    {
        return None;
    }
    let layout = layouts.get(&target.atlas.layout)?;
    let frame_rects = frames
        .iter()
        .map(|frame| layout.textures.get(frame.atlas.index).copied())
        .collect::<Option<Vec<_>>>()?;
    Some(WaterAtlasAnimation {
        layout: target.atlas.layout.clone(),
        target_index: target.atlas.index,
        frame_rects,
    })
}

/// Captura los rects originales antes de empezar a redirigir las entradas base.
#[must_use]
pub(crate) fn water_anim_frames_from_assets(
    assets: &WorldAssets,
    layouts: &Assets<TextureAtlasLayout>,
) -> WaterAnimFrames {
    WaterAnimFrames {
        water: atlas_animation_target(&assets.water, &assets.water_frames, layouts),
        shore: assets
            .shore
            .iter()
            .zip(&assets.shore_frames)
            .filter_map(|(target, frames)| atlas_animation_target(target, frames, layouts))
            .collect(),
    }
}

fn apply_global_atlas_frame(
    target: &WaterAtlasAnimation,
    frame: usize,
    layouts: &mut Assets<TextureAtlasLayout>,
) -> bool {
    let Some(rect) = target.frame_rects.get(frame).copied() else {
        return false;
    };
    let Some(mut layout) = layouts.get_mut(&target.layout) else {
        return false;
    };
    let Some(current) = layout.textures.get_mut(target.target_index) else {
        return false;
    };
    if *current == rect {
        return false;
    }
    *current = rect;
    true
}

/// Redirige globalmente las entradas base del atlas al frame actual.
///
/// El número de writes es O(19), independientemente de cuántas teselas de
/// agua haya cargadas. Todos los `Sprite` conservan su handle/índice.
pub(crate) fn animate_water(
    time: Res<Time>,
    frames: Option<Res<WaterAnimFrames>>,
    mut layouts: ResMut<Assets<TextureAtlasLayout>>,
    mut stats: Option<ResMut<WaterAnimationStats>>,
    mut last_frame: Local<Option<(usize, usize)>>,
) {
    let Some(frames) = frames else {
        return;
    };
    let phases = water_frame_indices(time.elapsed_secs());
    if *last_frame == Some(phases) {
        return;
    }
    *last_frame = Some(phases);
    let idx = combined_frame_index(phases.0, phases.1);
    let mut writes = 0;
    if let Some(water) = &frames.water {
        writes += usize::from(apply_global_atlas_frame(water, idx, &mut layouts));
    }
    for shore in &frames.shore {
        writes += usize::from(apply_global_atlas_frame(shore, idx, &mut layouts));
    }
    if let Some(stats) = stats.as_deref_mut() {
        stats.last_rect_writes = writes;
        stats.peak_rect_writes = stats.peak_rect_writes.max(writes);
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use bevy::ecs::system::RunSystemOnce;

    use super::*;
    use crate::render::AtlasSprite;

    fn weak_sprite(n: usize, layout: &Handle<TextureAtlasLayout>) -> AtlasSprite {
        AtlasSprite {
            image: Handle::Uuid(
                bevy::asset::uuid::Uuid::from_u128(1),
                std::marker::PhantomData,
            ),
            atlas: TextureAtlas {
                layout: layout.clone(),
                index: n,
            },
            size: Vec2::ONE,
        }
    }

    fn frames_resource() -> (WaterAnimFrames, Assets<TextureAtlasLayout>) {
        let mut layouts = Assets::<TextureAtlasLayout>::default();
        let mut layout = TextureAtlasLayout::new_empty(UVec2::new(4096, 64));
        for i in 0..=crate::sprites::WATER_PALETTE_FRAME_COUNT {
            let x = u32::try_from(i * 2).unwrap();
            layout.add_texture(URect::new(x, 0, x + 1, 1));
        }
        let handle = layouts.add(layout);
        let target = weak_sprite(0, &handle);
        let frames: Vec<_> = (1..=crate::sprites::WATER_PALETTE_FRAME_COUNT)
            .map(|i| weak_sprite(i, &handle))
            .collect();
        let water = atlas_animation_target(&target, &frames, &layouts);
        (
            WaterAnimFrames {
                water,
                shore: Vec::new(),
            },
            layouts,
        )
    }

    #[test]
    fn phases_match_openttd_extr_golden_counters() {
        assert_eq!(water_palette_phases(0), (0, 0));
        assert_eq!(water_palette_phases(40), (0, 1));
        assert_eq!(water_palette_phases(48), (1, 1));
        assert_eq!(water_palette_phases(104), (2, 3));
        assert_eq!(water_palette_phases(208), (0, 6));
        assert_eq!(water_palette_phases(504), (2, 14));
        assert_eq!(water_palette_phases(512), (2, 0));
        assert_eq!(water_palette_phases(2040), (4, 14));
        assert_eq!(water_palette_phases(2048), (0, 0));
    }

    #[test]
    fn elapsed_time_advances_the_two_counters_independently() {
        assert_eq!(water_frame_indices(0.0), (0, 0));
        assert_eq!(water_frame_indices(0.15), (0, 1));
        assert_eq!(water_frame_indices(0.18), (1, 1));
        assert_eq!(water_frame_indices(1.92), (2, 0));
    }

    #[test]
    fn animate_water_swaps_images_on_frame_change() {
        let mut world = World::new();
        let mut time = Time::<()>::default();
        time.advance_by(std::time::Duration::from_millis(400));
        world.insert_resource(time);
        let (frames, layouts) = frames_resource();
        let expected_idx = {
            let phases = water_frame_indices(0.4);
            combined_frame_index(phases.0, phases.1)
        };
        let expected_rect = frames.water.as_ref().unwrap().frame_rects[expected_idx];
        let layout_handle = frames.water.as_ref().unwrap().layout.clone();
        world.insert_resource(frames);
        world.insert_resource(layouts);

        world.run_system_once(animate_water).unwrap();

        let layouts = world.resource::<Assets<TextureAtlasLayout>>();
        let target = &layouts.get(&layout_handle).unwrap().textures[0];
        assert_eq!(*target, expected_rect);
    }

    #[test]
    fn animate_water_without_frames_resource_is_noop() {
        let mut world = World::new();
        world.insert_resource(Time::<()>::default());
        world.insert_resource(Assets::<TextureAtlasLayout>::default());

        world.run_system_once(animate_water).unwrap();
    }

    #[test]
    fn large_water_population_keeps_peak_atlas_writes_constant() {
        let mut world = World::new();
        let mut time = Time::<()>::default();
        time.advance_by(std::time::Duration::from_millis(400));
        world.insert_resource(time);
        let (frames, layouts) = frames_resource();
        world.insert_resource(frames);
        world.insert_resource(layouts);
        world.insert_resource(WaterAnimationStats::default());
        for _ in 0..65_536 {
            world.spawn(crate::render::WaterTile::ANIMATED);
        }

        world.run_system_once(animate_water).unwrap();

        let stats = world.resource::<WaterAnimationStats>();
        assert_eq!(stats.last_rect_writes, 1);
        assert_eq!(stats.peak_rect_writes, 1);
        let mut query = world.query::<&crate::render::WaterTile>();
        assert_eq!(query.iter(&world).count(), 65_536);
    }
}
