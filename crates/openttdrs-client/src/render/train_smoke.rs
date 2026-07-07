//! Humo/chispas de locomotoras (`EV_STEAM_SMOKE`, `EV_DIESEL_SMOKE`, `EV_ELECTRIC_SPARK`).

use std::collections::HashMap;

use bevy::prelude::*;

use openttdrs_core::{
    VehicleKind, default_engine_id, extrapolate_vehicle_pose, retreat_vehicle_pose,
    train_smoke_kind,
};

use crate::bevy_app::UpdateSet;
use crate::render::effect_vehicle::{
    EffectSpriteSet, EffectVehicleFrames, apply_effect_frame, effect_frame_index,
    effect_lifetime_secs, effect_overlay_pos,
};
use crate::render::{MapVisualLayer, vehicles::vehicle_draw_anchor_from_pose};
use crate::simulation::SimClock;
use crate::state::{ClientScreen, SimWorld};

/// Intervalo entre partículas de humo (segundos de reloj visual).
const TRAIN_SMOKE_SPAWN_INTERVAL_SECS: f32 = 0.11;
/// Desplazamiento sub-tesela hacia atrás respecto a la locomotora (cola de humo).
const TRAIN_SMOKE_EMIT_BACK_PROGRESS: u8 = 28;

pub(crate) struct TrainSmokePlugin;

impl Plugin for TrainSmokePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TrainSmokeSpawnClock>().add_systems(
            Update,
            (spawn_train_smoke, cull_train_smoke, animate_train_smoke)
                .chain()
                .in_set(UpdateSet::Visuals)
                .run_if(in_state(ClientScreen::InGame)),
        );
    }
}

#[derive(Resource, Default)]
struct TrainSmokeSpawnClock {
    last_spawn: HashMap<u32, f32>,
}

#[derive(Component)]
pub(crate) struct TrainSmokeEffect {
    vehicle_id: u32,
    started: f32,
    phase: usize,
    rise: f32,
    anchor: Vec2,
    base_z: u8,
    tile: (i32, i32),
    set: TrainSmokeSet,
}

#[derive(Clone, Copy)]
enum TrainSmokeSet {
    Steam,
    Diesel,
    Electric,
}

fn sprite_set<'a>(frames: &'a EffectVehicleFrames, kind: TrainSmokeSet) -> EffectSpriteSet<'a> {
    match kind {
        TrainSmokeSet::Steam => frames.steam_set(),
        TrainSmokeSet::Diesel => frames.diesel_set(),
        TrainSmokeSet::Electric => frames.electric_set(),
    }
}

fn spawn_train_smoke(
    sim: Res<SimWorld>,
    sim_clock: Res<SimClock>,
    frames: Res<EffectVehicleFrames>,
    time: Res<Time>,
    mut spawn_clock: ResMut<TrainSmokeSpawnClock>,
    mut commands: Commands,
    existing: Query<&TrainSmokeEffect>,
) {
    if !frames.is_loaded() {
        return;
    }
    if existing.iter().count() > 48 {
        return;
    }
    let map = &sim.state.map;
    let tick_alpha = sim_clock.tick_alpha;
    let elapsed = time.elapsed_secs();
    for v in &sim.state.vehicles {
        if v.kind != VehicleKind::Train
            || openttdrs_core::vehicle_hidden_on_map(map, v)
            || !v.running
            || v.cur_speed == 0
        {
            continue;
        }
        let last = spawn_clock.last_spawn.get(&v.id).copied().unwrap_or(-1.0);
        if elapsed - last < TRAIN_SMOKE_SPAWN_INTERVAL_SECS {
            continue;
        }
        let engine_id = v
            .engine_id
            .unwrap_or_else(|| default_engine_id(VehicleKind::Train));
        let smoke_kind = train_smoke_kind(engine_id);
        let set_kind = match smoke_kind {
            openttdrs_core::TrainSmokeKind::Steam => TrainSmokeSet::Steam,
            openttdrs_core::TrainSmokeKind::Diesel => TrainSmokeSet::Diesel,
            openttdrs_core::TrainSmokeKind::Electric => TrainSmokeSet::Electric,
        };
        let effect_set = sprite_set(&frames, set_kind);
        if effect_set.frames.is_empty() {
            continue;
        }
        let pose = retreat_vehicle_pose(
            v,
            extrapolate_vehicle_pose(v, tick_alpha),
            TRAIN_SMOKE_EMIT_BACK_PROGRESS,
        );
        let (anchor, base_z, tx, ty) = vehicle_draw_anchor_from_pose(v, map, pose);
        let rise = match set_kind {
            TrainSmokeSet::Steam => 6.0,
            TrainSmokeSet::Diesel => 4.0,
            TrainSmokeSet::Electric => 0.0,
        };
        let phase = usize::from(v.id as u8).wrapping_mul(7) % effect_set.frames.len().max(1);
        let frame = effect_frame_index(0.0, phase, &effect_set);
        let Some(atlas) = effect_set.frames.get(frame) else {
            continue;
        };
        let mut sprite = atlas.sprite();
        if matches!(set_kind, TrainSmokeSet::Electric) {
            sprite.color = Color::srgb(0.85, 0.92, 1.0);
        }
        let pos = effect_overlay_pos(anchor, frame, &effect_set, base_z, (tx, ty), 0.38, 0.0);
        spawn_clock.last_spawn.insert(v.id, elapsed);
        commands.spawn((
            MapVisualLayer,
            TrainSmokeEffect {
                vehicle_id: v.id,
                started: elapsed,
                phase,
                rise,
                anchor,
                base_z,
                tile: (tx, ty),
                set: set_kind,
            },
            sprite,
            Transform::from_translation(pos),
            Visibility::Visible,
        ));
    }
}

fn cull_train_smoke(
    sim: Res<SimWorld>,
    mut spawn_clock: ResMut<TrainSmokeSpawnClock>,
    q: Query<(Entity, &TrainSmokeEffect)>,
    mut commands: Commands,
) {
    let map = &sim.state.map;
    for (entity, smoke) in &q {
        let hide = sim
            .state
            .vehicles
            .iter()
            .find(|v| v.id == smoke.vehicle_id)
            .is_none_or(|v| {
                openttdrs_core::vehicle_hidden_on_map(map, v) || !v.running || v.cur_speed == 0
            });
        if hide {
            spawn_clock.last_spawn.remove(&smoke.vehicle_id);
            commands.entity(entity).despawn();
        }
    }
}

fn animate_train_smoke(
    time: Res<Time>,
    frames: Res<EffectVehicleFrames>,
    mut q: Query<(Entity, &mut Transform, &mut TrainSmokeEffect, &mut Sprite)>,
    mut commands: Commands,
) {
    if !frames.is_loaded() {
        return;
    }
    let elapsed = time.elapsed_secs();
    let dt = time.delta_secs();
    for (entity, mut transform, mut smoke, mut sprite) in &mut q {
        let effect_set = sprite_set(&frames, smoke.set);
        let age = elapsed - smoke.started;
        if age >= effect_lifetime_secs(&effect_set) {
            commands.entity(entity).despawn();
            continue;
        }
        smoke.rise += match smoke.set {
            TrainSmokeSet::Steam => 28.0 * dt,
            TrainSmokeSet::Diesel => 22.0 * dt,
            TrainSmokeSet::Electric => 0.0,
        };
        let frame = effect_frame_index(age, smoke.phase, &effect_set);
        apply_effect_frame(&mut sprite, &effect_set, frame);
        if matches!(smoke.set, TrainSmokeSet::Electric) {
            sprite.color = Color::srgb(0.85, 0.92, 1.0);
        }
        transform.translation = effect_overlay_pos(
            smoke.anchor,
            frame,
            &effect_set,
            smoke.base_z,
            smoke.tile,
            0.38,
            smoke.rise,
        );
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
