//! Humo/chispas de locomotoras (`EV_STEAM_SMOKE`, `EV_DIESEL_SMOKE`, `EV_ELECTRIC_SPARK`).

use bevy::prelude::*;

use openttdrs_core::{VehicleKind, default_engine_id, train_smoke_kind};

use crate::bevy_app::UpdateSet;
use crate::render::{
    ChimneySmokeFrames, MapVisualLayer, smoke::smoke_frame_index, vehicles::vehicle_sprite_pos,
};
use crate::simulation::SimClock;
use crate::state::{ClientScreen, SimWorld};

pub(crate) struct TrainSmokePlugin;

impl Plugin for TrainSmokePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (spawn_train_smoke, animate_train_smoke)
                .chain()
                .in_set(UpdateSet::Visuals)
                .run_if(in_state(ClientScreen::InGame)),
        );
    }
}

#[derive(Component)]
pub(crate) struct TrainSmokeEffect {
    lifetime: Timer,
    phase: usize,
    rise: f32,
}

fn spawn_train_smoke(
    sim: Res<SimWorld>,
    sim_clock: Res<SimClock>,
    smoke_frames: Res<ChimneySmokeFrames>,
    mut commands: Commands,
    existing: Query<&TrainSmokeEffect>,
) {
    if smoke_frames.0.is_empty() {
        return;
    }
    if !existing.is_empty() && existing.iter().count() > 48 {
        return;
    }
    let map = &sim.state.map;
    let tick_alpha = sim_clock.tick_alpha;
    for v in &sim.state.vehicles {
        if v.kind != VehicleKind::Train || !v.running || v.cur_speed == 0 {
            continue;
        }
        if !(u64::from(v.id) + sim.state.tick.get()).is_multiple_of(3) {
            continue;
        }
        let engine_id = v
            .engine_id
            .unwrap_or_else(|| default_engine_id(VehicleKind::Train));
        let kind = train_smoke_kind(engine_id);
        let pos = vehicle_sprite_pos(v, map, tick_alpha);
        let phase = usize::from(v.id as u8).wrapping_mul(7) % smoke_frames.0.len();
        let rise = match kind {
            openttdrs_core::TrainSmokeKind::Steam => 1.2,
            openttdrs_core::TrainSmokeKind::Diesel => 0.9,
            openttdrs_core::TrainSmokeKind::Electric => 0.4,
        };
        let sprite = smoke_frames.0[phase].sprite();
        commands.spawn((
            MapVisualLayer,
            TrainSmokeEffect {
                lifetime: Timer::from_seconds(1.4, TimerMode::Once),
                phase,
                rise,
            },
            sprite,
            Transform::from_translation(pos + Vec3::new(0.0, 8.0, 0.3)),
            Visibility::Visible,
        ));
    }
}

fn animate_train_smoke(
    time: Res<Time>,
    smoke_frames: Res<ChimneySmokeFrames>,
    mut q: Query<(Entity, &mut Transform, &mut TrainSmokeEffect, &mut Sprite)>,
    mut commands: Commands,
) {
    if smoke_frames.0.is_empty() {
        return;
    }
    for (entity, mut transform, mut smoke, mut sprite) in &mut q {
        smoke.lifetime.tick(time.delta());
        transform.translation.y += smoke.rise * 40.0 * time.delta_secs();
        transform.translation.z += 0.02;
        let idx = smoke_frame_index(time.elapsed_secs(), smoke.phase);
        if let Some(frame) = smoke_frames.0.get(idx) {
            frame.apply_to(&mut sprite);
        }
        if smoke.lifetime.is_finished() {
            commands.entity(entity).despawn();
        }
    }
}

#[cfg(test)]
mod tests {
    use openttdrs_core::{
        ENGINE_TRAIN_ASIASTAR, ENGINE_TRAIN_KIRBY, TrainSmokeKind, train_smoke_kind,
    };

    #[test]
    fn smoke_kind_matches_engine_class() {
        assert_eq!(train_smoke_kind(ENGINE_TRAIN_KIRBY), TrainSmokeKind::Steam);
        assert_eq!(
            train_smoke_kind(ENGINE_TRAIN_ASIASTAR),
            TrainSmokeKind::Electric
        );
    }
}
