//! Burbujas libres del generador Toyland (`EV_BUBBLE`).

use bevy::prelude::*;
use openttdrs_core::TileCoord;

use crate::bevy_app::UpdateSet;
use crate::iso::{road_vehicle_tile_anchor, wang_hash};
use crate::render::{MapVisualLayer, WorldAssets, palette_animations_should_run};
use crate::sprites::{BUBBLE_FRAMES, BUBBLE_META, TransparencyOption, with_to_alpha};
use crate::state::{ClientScreen, SimWorld};

const SPAWN_X: [i16; 4] = [11, 0, -4, -14];
const SPAWN_Y: [i16; 4] = [-4, -10, -4, 1];
const SPAWN_Z: [i16; 4] = [49, 59, 60, 65];

#[derive(Resource, Default)]
pub(crate) struct BubbleSpawnQueue(Vec<(TileCoord, u8)>);

impl BubbleSpawnQueue {
    pub(crate) fn push(&mut self, at: TileCoord, direction: u8) {
        self.0.push((at, direction & 3));
    }
}

pub(crate) struct BubbleEffectPlugin;

impl Plugin for BubbleEffectPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BubbleSpawnQueue>().add_systems(
            Update,
            (spawn_queued_bubbles, animate_bubbles)
                .chain()
                .in_set(UpdateSet::Visuals)
                .run_if(in_state(ClientScreen::InGame))
                .run_if(palette_animations_should_run),
        );
    }
}

#[derive(Component)]
struct BubbleEffect {
    at: TileCoord,
    direction: u8,
    float_direction: u8,
    seed: u32,
    started_tick: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BubblePhase {
    Generate,
    Float,
    Burst,
    Absorb,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BubbleState {
    frame: usize,
    x: i16,
    y: i16,
    z: i16,
    phase: BubblePhase,
}

fn burst_on_cycle(seed: u32, cycle: u32) -> bool {
    wang_hash(seed, cycle, 0xBABB_1E55).is_multiple_of(96)
}

fn apply_delta(state: &mut BubbleState, dx: i16, dy: i16, dz: i16, frame: usize) {
    state.x += dx;
    state.y += dy;
    state.z += dz;
    state.frame = frame;
}

fn apply_float_step(state: &mut BubbleState, direction: u8, step: usize) {
    let phase = step % 4;
    let (dx, dy) = if phase == 1 || phase == 3 {
        match direction & 3 {
            0 => (1, 0),
            1 => (-1, 0),
            2 => (0, 1),
            _ => (0, -1),
        }
    } else {
        (0, 0)
    };
    apply_delta(state, dx, dy, 1, [0, 1, 0, 2][phase]);
}

fn apply_absorb_step(state: &mut BubbleState, step: usize) -> bool {
    if step < 64 {
        apply_delta(state, 0, 0, 1, [0, 1, 0, 2][step % 4]);
        return true;
    }
    const APPROACH: [(i16, i16, i16, usize); 8] = [
        (2, 1, 3, 0),
        (1, 1, 3, 1),
        (2, 1, 3, 0),
        (1, 1, 3, 2),
        (2, 1, 3, 0),
        (1, 1, 3, 1),
        (2, 1, 3, 0),
        (1, 0, 1, 2),
    ];
    if let Some(&(dx, dy, dz, frame)) = APPROACH.get(step - 64) {
        apply_delta(state, dx, dy, dz, frame);
        return true;
    }
    if step < 80 {
        let drift = step - 72;
        apply_delta(
            state,
            i16::from(drift % 2 == 1),
            0,
            1,
            [0, 1, 0, 2][drift % 4],
        );
        return true;
    }
    if step < 85 {
        state.frame = 10 + step - 80;
        return true;
    }
    false
}

/// Evalúa las tablas `_bubble_movement` una vez cada cuatro ticks.
#[must_use]
fn bubble_state(
    age_ticks: u64,
    direction: u8,
    float_direction: u8,
    seed: u32,
) -> Option<BubbleState> {
    let actions = usize::try_from(age_ticks / 4).unwrap_or(usize::MAX);
    let mut state = BubbleState {
        frame: 3,
        x: 0,
        y: 0,
        z: 0,
        phase: BubblePhase::Generate,
    };
    match actions {
        0 => return Some(state),
        1 => {
            state.frame = 4;
            return Some(state);
        }
        2 => {
            state.frame = 5;
            return Some(state);
        }
        _ => {}
    }

    let movement_actions = actions - 2;
    if direction & 3 == 0 {
        state.phase = BubblePhase::Absorb;
        for step in 0..movement_actions {
            if !apply_absorb_step(&mut state, step) {
                return None;
            }
        }
        return Some(state);
    }

    state.phase = BubblePhase::Float;
    let mut burst_step = None;
    for step in 0..movement_actions {
        if let Some(index) = burst_step {
            if index >= 4 {
                return None;
            }
            state.phase = BubblePhase::Burst;
            apply_delta(&mut state, 0, 0, 1, [2, 7, 8, 9][index]);
            burst_step = Some(index + 1);
            continue;
        }
        if step > 0
            && step.is_multiple_of(4)
            && (SPAWN_Z[usize::from(direction & 3)] + state.z > 180
                || burst_on_cycle(seed, u32::try_from(step / 4).unwrap_or(u32::MAX)))
        {
            state.phase = BubblePhase::Burst;
            apply_delta(&mut state, 0, 0, 1, 2);
            burst_step = Some(1);
        } else {
            apply_float_step(&mut state, float_direction, step);
        }
    }
    Some(state)
}

fn bubble_translation(effect: &BubbleEffect, state: BubbleState) -> Vec3 {
    let index = usize::from(effect.direction & 3);
    let x = f32::from(SPAWN_X[index] + state.x);
    let y = f32::from(SPAWN_Y[index] + state.y);
    let z = f32::from(SPAWN_Z[index] + state.z);
    let anchor = road_vehicle_tile_anchor(effect.at.x, effect.at.y, x, y, z);
    let (w, h, xrel, yrel) = BUBBLE_META[state.frame];
    Vec3::new(
        anchor.x + xrel + w * 0.5,
        anchor.y - (yrel + h * 0.5),
        (effect.at.x + effect.at.y) as f32 * 0.01 + 0.62 + z * 0.0001,
    )
}

fn spawn_queued_bubbles(
    mut queue: ResMut<BubbleSpawnQueue>,
    sim: Res<SimWorld>,
    assets: Option<Res<WorldAssets>>,
    mut commands: Commands,
) {
    let Some(assets) = assets else {
        return;
    };
    if assets.bubble.len() != BUBBLE_FRAMES {
        queue.0.clear();
        return;
    }
    let tick = sim.state.tick.get();
    for (at, direction) in queue.0.drain(..) {
        let seed = wang_hash(at.x as u32, at.y as u32, tick as u32);
        let effect = BubbleEffect {
            at,
            direction,
            float_direction: (seed & 3) as u8,
            seed,
            started_tick: tick,
        };
        let Some(state) = bubble_state(0, direction, effect.float_direction, seed) else {
            continue;
        };
        let mut sprite = assets.bubble[state.frame].sprite();
        sprite.color = with_to_alpha(sprite.color, TransparencyOption::Industries);
        let translation = bubble_translation(&effect, state);
        commands.spawn((
            MapVisualLayer,
            effect,
            sprite,
            Transform::from_translation(translation),
            Visibility::Visible,
        ));
    }
}

fn animate_bubbles(
    sim: Res<SimWorld>,
    assets: Option<Res<WorldAssets>>,
    mut bubbles: Query<(Entity, &BubbleEffect, &mut Sprite, &mut Transform)>,
    mut commands: Commands,
) {
    let Some(assets) = assets else {
        return;
    };
    let tick = sim.state.tick.get();
    for (entity, effect, mut sprite, mut transform) in &mut bubbles {
        let age = tick.saturating_sub(effect.started_tick);
        let Some(state) = bubble_state(age, effect.direction, effect.float_direction, effect.seed)
        else {
            commands.entity(entity).despawn();
            continue;
        };
        if let Some(frame) = assets.bubble.get(state.frame)
            && !frame.matches(&sprite)
        {
            frame.apply_to(&mut sprite);
        }
        let translation = bubble_translation(effect, state);
        if transform.translation != translation {
            transform.translation = translation;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generation_uses_three_frames_then_enters_movement() {
        assert_eq!(bubble_state(0, 1, 0, 1).map(|s| s.frame), Some(3));
        assert_eq!(bubble_state(4, 1, 0, 1).map(|s| s.frame), Some(4));
        assert_eq!(bubble_state(8, 1, 0, 1).map(|s| s.frame), Some(5));
        assert_eq!(
            bubble_state(12, 1, 0, 1).map(|s| s.phase),
            Some(BubblePhase::Float)
        );
    }

    #[test]
    fn four_float_directions_follow_openttd_deltas() {
        let sw = bubble_state(28, 1, 0, 1).map(|s| (s.x, s.y));
        let ne = bubble_state(28, 1, 1, 1).map(|s| (s.x, s.y));
        let se = bubble_state(28, 1, 2, 1).map(|s| (s.x, s.y));
        let nw = bubble_state(28, 1, 3, 1).map(|s| (s.x, s.y));
        assert_eq!(sw, Some((2, 0)));
        assert_eq!(ne, Some((-2, 0)));
        assert_eq!(se, Some((0, 2)));
        assert_eq!(nw, Some((0, -2)));
    }

    #[test]
    fn absorb_path_reaches_final_frames_and_culls() {
        assert_eq!(
            bubble_state((3 + 80) * 4, 0, 0, 1).map(|s| s.frame),
            Some(10)
        );
        assert_eq!(
            bubble_state((3 + 84) * 4, 0, 0, 1).map(|s| s.frame),
            Some(14)
        );
        assert!(bubble_state((3 + 85) * 4, 0, 0, 1).is_none());
    }

    #[test]
    fn high_bubble_switches_to_burst_then_culls() {
        let state = bubble_state(600, 3, 0, 1);
        assert!(state.is_none(), "debe superar z=180, estallar y borrarse");
    }
}
