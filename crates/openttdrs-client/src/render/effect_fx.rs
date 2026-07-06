//! FX efímeros: humo de avería, explosión y bulldozer en obras.

use bevy::prelude::*;

use openttdrs_core::TileCoord;

use crate::bevy_app::UpdateSet;
use crate::iso::tile_pos;
use crate::render::{ChimneySmokeFrames, MapVisualLayer, smoke::smoke_frame_index};
use crate::state::ClientScreen;

#[derive(Debug, Clone, Copy)]
pub(crate) enum FxSpawnKind {
    BreakdownSmoke,
    Explosion,
    RoadWorks,
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

    pub(crate) fn push_road_works(&mut self, at: TileCoord) {
        self.0.push((at, FxSpawnKind::RoadWorks));
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
    lifetime: Timer,
    phase: usize,
}

fn spawn_queued_fx(
    mut queue: ResMut<FxSpawnQueue>,
    smoke_frames: Res<ChimneySmokeFrames>,
    mut commands: Commands,
) {
    if smoke_frames.0.is_empty() {
        queue.0.clear();
        return;
    }
    for (at, kind) in queue.0.drain(..) {
        let pos = tile_pos(at.x, at.y, 0, 0.0);
        let duration = match kind {
            FxSpawnKind::BreakdownSmoke => 2.5,
            FxSpawnKind::Explosion => 1.2,
            FxSpawnKind::RoadWorks => 3.0,
        };
        let phase = (at.x as usize).wrapping_mul(3) % smoke_frames.0.len();
        let mut sprite = smoke_frames.0[phase].sprite();
        if matches!(kind, FxSpawnKind::Explosion) {
            sprite.color = Color::srgb(1.0, 0.55, 0.2);
        }
        commands.spawn((
            MapVisualLayer,
            EphemeralFx {
                kind,
                lifetime: Timer::from_seconds(duration, TimerMode::Once),
                phase,
            },
            sprite,
            Transform::from_translation(pos + Vec3::new(0.0, 12.0, 0.4)),
            Visibility::Visible,
        ));
    }
}

fn animate_ephemeral_fx(
    time: Res<Time>,
    frames: Res<ChimneySmokeFrames>,
    mut q: Query<(Entity, &mut Transform, &mut EphemeralFx, &mut Sprite)>,
    mut commands: Commands,
) {
    for (entity, mut transform, mut fx, mut sprite) in &mut q {
        fx.lifetime.tick(time.delta());
        let rise = match fx.kind {
            FxSpawnKind::BreakdownSmoke => 28.0,
            FxSpawnKind::Explosion | FxSpawnKind::RoadWorks => 0.0,
        };
        transform.translation.y += rise * time.delta_secs();
        if !frames.0.is_empty() {
            let idx = smoke_frame_index(time.elapsed_secs(), fx.phase);
            if let Some(frame) = frames.0.get(idx) {
                frame.apply_to(&mut sprite);
            }
        }
        if fx.lifetime.is_finished() {
            commands.entity(entity).despawn();
        }
    }
}
