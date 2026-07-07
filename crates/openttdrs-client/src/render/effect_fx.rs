//! FX efímeros: humo de avería y explosión (`EV_BREAKDOWN_SMOKE`, `EV_EXPLOSION_LARGE`).

use bevy::prelude::*;

use openttdrs_core::TileCoord;

use crate::bevy_app::UpdateSet;
use crate::iso::{iso, overlay_pos};
use crate::render::MapVisualLayer;
use crate::render::effect_vehicle::{
    EffectSpriteSet, EffectVehicleFrames, apply_effect_frame, effect_frame_index,
    effect_lifetime_secs,
};
use crate::state::ClientScreen;

#[derive(Debug, Clone, Copy)]
pub(crate) enum FxSpawnKind {
    BreakdownSmoke,
    Explosion,
}

/// Cola de FX a crear tras procesar [`PendingSimEvents`](crate::audio::PendingSimEvents).
#[derive(Resource, Default)]
pub(crate) struct FxSpawnQueue(Vec<(TileCoord, FxSpawnKind)>);

impl FxSpawnQueue {
    pub(crate) fn push_breakdown(&mut self, at: TileCoord) {
        self.0.push((at, FxSpawnKind::BreakdownSmoke));
    }

    pub(crate) fn push_explosion(&mut self, at: TileCoord) {
        self.0.push((at, FxSpawnKind::Explosion));
    }
}

pub(crate) struct EffectVehiclePlugin;

impl Plugin for EffectVehiclePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FxSpawnQueue>().add_systems(
            Update,
            (spawn_queued_fx, animate_ephemeral_fx)
                .chain()
                .in_set(UpdateSet::Visuals)
                .run_if(in_state(ClientScreen::InGame)),
        );
    }
}

#[derive(Component)]
struct EphemeralFx {
    kind: FxSpawnKind,
    started: f32,
    phase: usize,
    anchor: Vec2,
    base_z: u8,
    tile: (i32, i32),
}

fn sprite_set<'a>(frames: &'a EffectVehicleFrames, kind: FxSpawnKind) -> EffectSpriteSet<'a> {
    match kind {
        FxSpawnKind::BreakdownSmoke => frames.breakdown_set(),
        FxSpawnKind::Explosion => frames.explosion_set(),
    }
}

fn spawn_queued_fx(
    mut queue: ResMut<FxSpawnQueue>,
    frames: Res<EffectVehicleFrames>,
    time: Res<Time>,
    mut commands: Commands,
) {
    if !frames.is_loaded() {
        queue.0.clear();
        return;
    }
    let elapsed = time.elapsed_secs();
    for (at, kind) in queue.0.drain(..) {
        let effect_set = sprite_set(&frames, kind);
        if effect_set.frames.is_empty() {
            continue;
        }
        let phase = (at.x as usize).wrapping_mul(3) % effect_set.frames.len();
        let frame = effect_frame_index(0.0, phase, &effect_set);
        let Some(atlas) = effect_set.frames.get(frame) else {
            continue;
        };
        let anchor = iso(at.x, at.y);
        let base_z = 0u8;
        let (w, h, xrel, yrel) = effect_set.meta[frame.min(effect_set.meta.len() - 1)];
        let pos = overlay_pos(anchor, xrel, yrel, w, h, base_z, 0.42, at.x, at.y);
        let mut sprite = atlas.sprite();
        if matches!(kind, FxSpawnKind::Explosion) {
            sprite.color = Color::WHITE;
        }
        commands.spawn((
            MapVisualLayer,
            EphemeralFx {
                kind,
                started: elapsed,
                phase,
                anchor,
                base_z,
                tile: (at.x, at.y),
            },
            sprite,
            Transform::from_translation(pos + Vec3::new(0.0, 8.0, 0.35)),
            Visibility::Visible,
        ));
    }
}

fn animate_ephemeral_fx(
    time: Res<Time>,
    frames: Res<EffectVehicleFrames>,
    mut q: Query<(Entity, &mut Transform, &mut EphemeralFx, &mut Sprite)>,
    mut commands: Commands,
) {
    if !frames.is_loaded() {
        return;
    }
    let elapsed = time.elapsed_secs();
    let dt = time.delta_secs();
    for (entity, mut transform, fx, mut sprite) in &mut q {
        let effect_set = sprite_set(&frames, fx.kind);
        let age = elapsed - fx.started;
        if age >= effect_lifetime_secs(&effect_set) {
            commands.entity(entity).despawn();
            continue;
        }
        let rise = match fx.kind {
            FxSpawnKind::BreakdownSmoke => 24.0 * dt,
            FxSpawnKind::Explosion => 0.0,
        };
        let frame = effect_frame_index(age, fx.phase, &effect_set);
        apply_effect_frame(&mut sprite, &effect_set, frame);
        let (w, h, xrel, yrel) = effect_set.meta[frame.min(effect_set.meta.len() - 1)];
        let y_off = match fx.kind {
            FxSpawnKind::BreakdownSmoke => rise,
            FxSpawnKind::Explosion => 0.0,
        };
        transform.translation = overlay_pos(
            fx.anchor,
            xrel,
            yrel - y_off,
            w,
            h,
            fx.base_z,
            0.42,
            fx.tile.0,
            fx.tile.1,
        ) + Vec3::new(0.0, 8.0, 0.35);
    }
}
