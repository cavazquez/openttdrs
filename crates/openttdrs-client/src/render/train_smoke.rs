//! Humo/chispas de locomotoras (`EV_STEAM_SMOKE`, `EV_DIESEL_SMOKE`, `EV_ELECTRIC_SPARK`).
//!
//! Tanto la decisión de emitir como el avance del efecto se apoyan en ticks de
//! simulación. Así el resultado no depende de FPS ni del reloj visual.

use bevy::prelude::*;

use openttdrs_core::prelude::*;
use openttdrs_core::{
    EngineDef, Vehicle, VehicleAdvancedVisualEffectSpawn, VehicleOrder, VehicleVisualEffectKind,
    extrapolate_vehicle_pose, resolve_vehicle_spawn_visual_effect_callback, retreat_vehicle_pose,
    train_smoke_kind, vehicle_visual_effect_spec,
};

use crate::audio::{PlayWorldSfx, play_vehicle_event_sound_with_default};
use crate::bevy_app::UpdateSet;
use crate::iso::wang_hash;
use crate::render::effect_vehicle::{
    EffectSpriteSet, EffectVehicleFrames, apply_effect_frame, effect_overlay_pos,
};
use crate::render::{
    MapVisualLayer, palette_animations_should_run, vehicles::vehicle_draw_anchor_from_pose,
};
use crate::settings::ClientPreferences;
use crate::simulation::SimClock;
use crate::state::{ClientScreen, SimWorld};
use crate::ui::SimHudControls;

/// Desplazamiento sub-tesela hacia atrás respecto a la locomotora.
const TRAIN_SMOKE_EMIT_BACK_PROGRESS: u8 = 28;
const MAX_TRAIN_SMOKE_EFFECTS: usize = 48;

pub(crate) struct TrainSmokePlugin;

impl Plugin for TrainSmokePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TrainSmokeSpawnClock>().add_systems(
            Update,
            (spawn_train_smoke, animate_train_smoke)
                .chain()
                .in_set(UpdateSet::Visuals)
                .run_if(in_state(ClientScreen::InGame))
                .run_if(palette_animations_should_run),
        );
    }
}

/// Evita volver a evaluar el mismo tick si Bevy dibuja varios frames entre ticks.
#[derive(Resource, Default)]
struct TrainSmokeSpawnClock {
    last_tick: Option<u64>,
}

#[derive(Component)]
pub(crate) struct TrainSmokeEffect {
    started_tick: u64,
    anchor: Vec2,
    base_z: u8,
    tile: (i32, i32),
    set: TrainSmokeSet,
    /// Desplazamiento adicional de un registro `0x100` avanzado.
    advanced_offset: Vec3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TrainSmokeSet {
    Steam,
    Diesel,
    Electric,
    Breakdown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct EffectTickState {
    frame: usize,
    rise: u8,
}

fn sprite_set<'a>(frames: &'a EffectVehicleFrames, kind: TrainSmokeSet) -> EffectSpriteSet<'a> {
    match kind {
        TrainSmokeSet::Steam => frames.steam_set(),
        TrainSmokeSet::Diesel => frames.diesel_set(),
        TrainSmokeSet::Electric => frames.electric_set(),
        TrainSmokeSet::Breakdown => frames.breakdown_set(),
    }
}

/// Estado exacto de `SteamSmokeTick`, `DieselSmokeTick` y `ElectricSparkTick`.
/// `None` equivale al `Delete()` del EffectVehicle de OpenTTD.
#[must_use]
fn effect_tick_state(kind: TrainSmokeSet, age_ticks: u64) -> Option<EffectTickState> {
    let mut frame = 0_usize;
    let mut rise = 0_u8;
    match kind {
        TrainSmokeSet::Steam => {
            let mut progress = 12_u8;
            for _ in 0..age_ticks.min(96) {
                progress = progress.wrapping_add(1);
                if progress & 7 == 0 {
                    rise = rise.saturating_add(1);
                }
                if progress & 0x0F == 4 {
                    frame += 1;
                    if frame >= 5 {
                        return None;
                    }
                }
            }
        }
        TrainSmokeSet::Diesel | TrainSmokeSet::Breakdown => {
            let mut progress = 0_u8;
            for _ in 0..age_ticks.min(64) {
                progress = progress.wrapping_add(1);
                if progress.is_multiple_of(4) {
                    rise = rise.saturating_add(1);
                } else if progress % 8 == 1 {
                    frame += 1;
                    if frame >= 6 {
                        return None;
                    }
                }
            }
        }
        TrainSmokeSet::Electric => {
            let mut progress = 1_u8;
            for _ in 0..age_ticks.min(32) {
                if progress < 2 {
                    progress += 1;
                } else {
                    progress = 0;
                    frame += 1;
                    if frame >= 6 {
                        return None;
                    }
                }
            }
        }
    }
    Some(EffectTickState { frame, rise })
}

fn deterministic_random16(vehicle_id: u32, tick: u64, salt: u32) -> u16 {
    let folded_tick = tick as u32 ^ (tick >> 32) as u32;
    wang_hash(vehicle_id, folded_tick, salt) as u16
}

fn chance16(random: u16, numerator: i32, denominator: u32) -> bool {
    if numerator <= 0 || denominator == 0 {
        return false;
    }
    let threshold =
        (u64::try_from(numerator).unwrap_or(0) * 65_536 / u64::from(denominator)).min(65_536);
    u64::from(random) < threshold
}

fn train_is_stopping_at_station(map: &Map, vehicle: &Vehicle) -> bool {
    openttdrs_core::train_on_rail_platform(map, vehicle.pos)
        && (vehicle.awaiting_load_window
            || vehicle.cargo_loading
            || vehicle.cargo_unloading
            || matches!(
                vehicle.current_order_ref(),
                Some(VehicleOrder::Station { .. })
            ))
}

/// Port de las reglas de `Vehicle::ShowVisualEffect` para trenes.
#[must_use]
#[cfg(test)]
fn train_smoke_to_emit(
    map: &Map,
    vehicle: &mut Vehicle,
    tick: u64,
    smoke_amount: u8,
) -> Option<TrainSmokeSet> {
    let engine = vehicle.effective_engine();
    train_smoke_to_emit_with_engine(map, vehicle, engine, tick, smoke_amount)
}

/// Igual que `train_smoke_to_emit`, pero usando el catálogo de motores de la
/// partida para resolver callbacks de vehículos NewGRF.
fn train_smoke_to_emit_with_engine(
    map: &Map,
    vehicle: &mut Vehicle,
    engine: &EngineDef,
    tick: u64,
    smoke_amount: u8,
) -> Option<TrainSmokeSet> {
    let amount = smoke_amount.min(2);
    if amount == 0
        || vehicle.kind != VehicleKind::Train
        || !engine.is_train_engine()
        || !vehicle.running
        || vehicle.crashed
        || vehicle.cur_speed < 2
        || !vehicle.depot_leave_cleared
        || openttdrs_core::vehicle_hidden_from_view(map, vehicle, vehicle.pos, vehicle.progress)
        || train_is_stopping_at_station(map, vehicle)
    {
        return None;
    }

    let max_speed = if vehicle.cached_max_speed == 0 || vehicle.cached_max_speed == u16::MAX {
        engine.max_speed.max(1)
    } else {
        vehicle.cached_max_speed.max(1)
    };
    let speed = vehicle.cur_speed.min(max_speed);
    let smoke_kind = match vehicle_visual_effect_spec(engine, vehicle).kind {
        VehicleVisualEffectKind::Disabled => return None,
        VehicleVisualEffectKind::Steam => openttdrs_core::TrainSmokeKind::Steam,
        VehicleVisualEffectKind::Diesel => openttdrs_core::TrainSmokeKind::Diesel,
        VehicleVisualEffectKind::Electric => openttdrs_core::TrainSmokeKind::Electric,
        VehicleVisualEffectKind::Default => train_smoke_kind(engine.id),
    };
    match smoke_kind {
        openttdrs_core::TrainSmokeKind::Steam => {
            let bits = u32::from((4_u8 >> amount) + (speed.saturating_mul(3) / max_speed) as u8);
            let mask = (1_u64 << bits.min(63)).saturating_sub(1);
            (tick & mask == 0).then_some(TrainSmokeSet::Steam)
        }
        openttdrs_core::TrainSmokeKind::Diesel => {
            let power = if vehicle.cached_power_hp == 0 {
                engine.power_hp
            } else {
                vehicle.cached_power_hp
            };
            let weight = if vehicle.cached_weight_t == 0 {
                engine.weight_t
            } else {
                vehicle.cached_weight_t
            };
            let power_shift = (power >> 10).min(31);
            let weight_shift = (u32::from(weight) >> 9).min(31);
            let power_weight_effect =
                (32_u32 >> power_shift) as i32 - (32_u32 >> weight_shift) as i32;
            let speed_limit = max_speed >> (2_u8 >> amount);
            let numerator = 64 - i32::from(speed) * 32 / i32::from(max_speed) + power_weight_effect;
            (speed < speed_limit
                && chance16(
                    deterministic_random16(vehicle.id, tick, 0xD1E5_E100),
                    numerator,
                    512_u32 >> amount,
                ))
            .then_some(TrainSmokeSet::Diesel)
        }
        openttdrs_core::TrainSmokeKind::Electric => {
            let numerator = 6 - i32::from(speed) * 4 / i32::from(max_speed);
            (tick & 3 == 0
                && chance16(
                    deterministic_random16(vehicle.id, tick, 0xE1EC_7A1C),
                    numerator,
                    360_u32 >> amount,
                ))
            .then_some(TrainSmokeSet::Electric)
        }
    }
}

fn advanced_effect_set(effect_type: u8) -> Option<TrainSmokeSet> {
    match effect_type {
        0xF1 => Some(TrainSmokeSet::Steam),
        0xF2 => Some(TrainSmokeSet::Diesel),
        0xF3 => Some(TrainSmokeSet::Electric),
        0xFA => Some(TrainSmokeSet::Breakdown),
        _ => None,
    }
}

/// Convierte offsets NFO relativos en un desplazamiento de overlay. La
/// posición recibida por `vehicle_draw_anchor_from_pose` ya está centrada, por
/// lo que `auto_center` no agrega un sesgo adicional; sí respetamos
/// `auto_rotate`, que es la parte observable para X/Y.
#[must_use]
fn advanced_effect_offset(
    vehicle: &Vehicle,
    spawn: VehicleAdvancedVisualEffectSpawn,
    auto_rotate: bool,
) -> Vec3 {
    let mut x = i32::from(spawn.x);
    let mut y = i32::from(spawn.y);
    if auto_rotate {
        const SMOKE_POS: [i32; 8] = [1, 1, 1, 0, -1, -1, -1, 0];
        let longitudinal = usize::from(vehicle.direction & 7);
        let transverse = (longitudinal + 2) & 7;
        let local_x = x;
        let local_y = y;
        x = SMOKE_POS[longitudinal] * local_x + SMOKE_POS[transverse] * local_y;
        y = SMOKE_POS[transverse] * local_x - SMOKE_POS[longitudinal] * local_y;
    }
    Vec3::new(x as f32, y as f32, f32::from(spawn.z))
}

#[allow(clippy::too_many_arguments)]
fn spawn_train_smoke(
    mut sim: ResMut<SimWorld>,
    sim_clock: Res<SimClock>,
    prefs: Res<ClientPreferences>,
    hud: Res<SimHudControls>,
    frames: Res<EffectVehicleFrames>,
    mut spawn_clock: ResMut<TrainSmokeSpawnClock>,
    mut commands: Commands,
    existing: Query<(), With<TrainSmokeEffect>>,
    mut sfx: MessageWriter<PlayWorldSfx>,
) {
    if !frames.is_loaded() {
        return;
    }
    let tick = sim.state.tick.get();
    if spawn_clock.last_tick == Some(tick) {
        return;
    }
    spawn_clock.last_tick = Some(tick);

    let mut active_count = existing.iter().count();
    let state = &mut sim.state;
    let map = &state.map;
    let engine_catalog = &state.engine_catalog;
    let mut visual_sound_events = Vec::new();
    for vehicle in &mut state.vehicles {
        if active_count >= MAX_TRAIN_SMOKE_EFFECTS {
            break;
        }
        let engine_id = vehicle
            .engine_id
            .unwrap_or_else(|| openttdrs_core::default_engine_id(vehicle.kind));
        let Some(engine) = openttdrs_core::engine_in_catalog(engine_catalog, engine_id)
            .or_else(|| openttdrs_core::engine_by_id(engine_id))
        else {
            continue;
        };
        let visual_spec = vehicle_visual_effect_spec(engine, vehicle);
        if visual_spec.advanced {
            // Un modelo avanzado todavía puede resolver a `Disabled` (por
            // ejemplo, un valor reservado del GRF). En ese caso OpenTTD no
            // invoca CB160: el vehículo queda sin efecto visual.
            if matches!(visual_spec.kind, VehicleVisualEffectKind::Disabled) {
                continue;
            }
            // `ShowVisualEffect` no emite el modelo vanilla cuando el bit 6
            // pide el callback avanzado: si el callback falla, el resultado
            // correcto es no crear efectos, no degradar a humo estándar.
            let random = deterministic_random16(vehicle.id, tick, 0x1600_0000);
            let advanced = resolve_vehicle_spawn_visual_effect_callback(engine, vehicle, random);
            let Some(advanced) = advanced else {
                continue;
            };
            let pose = extrapolate_vehicle_pose(vehicle, sim_clock.tick_alpha);
            let (anchor, base_z, tx, ty) = vehicle_draw_anchor_from_pose(vehicle, map, pose);
            let mut emitted = false;
            for spawn in advanced.spawns.iter().take(usize::from(advanced.count)) {
                let Some(set_kind) = advanced_effect_set(spawn.effect_type) else {
                    continue;
                };
                if active_count >= MAX_TRAIN_SMOKE_EFFECTS {
                    break;
                }
                let effect_set = sprite_set(&frames, set_kind);
                let Some(atlas) = effect_set.frames.first() else {
                    continue;
                };
                let advanced_offset = advanced_effect_offset(vehicle, *spawn, advanced.auto_rotate);
                let mut sprite = atlas.sprite();
                if matches!(set_kind, TrainSmokeSet::Electric) {
                    sprite.color = Color::srgb(0.85, 0.92, 1.0);
                }
                let pos = effect_overlay_pos(anchor, 0, &effect_set, base_z, (tx, ty), 0.38, 0.0)
                    + advanced_offset;
                commands.spawn((
                    MapVisualLayer,
                    TrainSmokeEffect {
                        started_tick: tick,
                        anchor,
                        base_z,
                        tile: (tx, ty),
                        set: set_kind,
                        advanced_offset,
                    },
                    sprite,
                    Transform::from_translation(pos),
                    Visibility::Visible,
                ));
                emitted = true;
                active_count += 1;
            }
            if emitted && hud.sound_vehicle && vehicle.is_consist_head() {
                visual_sound_events.push((vehicle.id, vehicle.pos));
            }
            continue;
        }
        let Some(set_kind) =
            train_smoke_to_emit_with_engine(map, vehicle, engine, tick, prefs.smoke_amount)
        else {
            continue;
        };
        let effect_set = sprite_set(&frames, set_kind);
        let Some(atlas) = effect_set.frames.first() else {
            continue;
        };
        let pose = retreat_vehicle_pose(
            vehicle,
            extrapolate_vehicle_pose(vehicle, sim_clock.tick_alpha),
            TRAIN_SMOKE_EMIT_BACK_PROGRESS,
        );
        let (anchor, base_z, tx, ty) = vehicle_draw_anchor_from_pose(vehicle, map, pose);
        let mut sprite = atlas.sprite();
        if matches!(set_kind, TrainSmokeSet::Electric) {
            sprite.color = Color::srgb(0.85, 0.92, 1.0);
        }
        let pos = effect_overlay_pos(anchor, 0, &effect_set, base_z, (tx, ty), 0.38, 0.0);
        commands.spawn((
            MapVisualLayer,
            TrainSmokeEffect {
                started_tick: tick,
                anchor,
                base_z,
                tile: (tx, ty),
                set: set_kind,
                advanced_offset: Vec3::ZERO,
            },
            sprite,
            Transform::from_translation(pos),
            Visibility::Visible,
        ));
        // `ShowVisualEffect` llama a `PlayVehicleSound(VSE_VISUAL_EFFECT)`
        // una vez por vehículo primario cuando al menos un humo/chispa fue
        // creado. Los efectos de los vagones se siguen dibujando, pero no
        // duplican el callback del consist.
        if hud.sound_vehicle && vehicle.is_consist_head() {
            visual_sound_events.push((vehicle.id, vehicle.pos));
        }
        active_count += 1;
    }
    for (vehicle_id, at) in visual_sound_events {
        play_vehicle_event_sound_with_default(
            &mut sim,
            &mut sfx,
            vehicle_id,
            VehicleSoundEvent::VisualEffect,
            None,
            at,
            0.35,
            5,
        );
    }
}

fn animate_train_smoke(
    sim: Res<SimWorld>,
    frames: Res<EffectVehicleFrames>,
    mut q: Query<(Entity, &mut Transform, &TrainSmokeEffect, &mut Sprite)>,
    mut commands: Commands,
) {
    if !frames.is_loaded() {
        return;
    }
    let tick = sim.state.tick.get();
    for (entity, mut transform, smoke, mut sprite) in &mut q {
        let age = tick.saturating_sub(smoke.started_tick);
        let Some(state) = effect_tick_state(smoke.set, age) else {
            commands.entity(entity).despawn();
            continue;
        };
        let effect_set = sprite_set(&frames, smoke.set);
        let frame_changed = !effect_set
            .frames
            .get(state.frame)
            .is_some_and(|atlas| atlas.matches(&sprite));
        if frame_changed {
            apply_effect_frame(&mut sprite, &effect_set, state.frame);
            if matches!(smoke.set, TrainSmokeSet::Electric) {
                sprite.color = Color::srgb(0.85, 0.92, 1.0);
            }
        }
        let pos = effect_overlay_pos(
            smoke.anchor,
            state.frame,
            &effect_set,
            smoke.base_z,
            smoke.tile,
            0.38,
            f32::from(state.rise),
        ) + smoke.advanced_offset;
        if transform.translation != pos {
            transform.translation = pos;
        }
    }
}

#[cfg(test)]
mod tests {
    use openttdrs_core::{
        Action2VarAdjust, Action2VarEntry, Action2VarTerm, ENGINE_TRAIN_ASIASTAR,
        ENGINE_TRAIN_KIRBY, Map, TileCoord, TrainSmokeKind, TrainSpriteAssign, TrainSpriteGraphics,
        Vehicle, VehicleKind, train_smoke_kind,
    };

    use super::*;

    fn running_train(engine_id: u16) -> Vehicle {
        let pos = TileCoord::new(1, 1);
        let mut vehicle = Vehicle::new(7, VehicleKind::Train, pos, TileCoord::new(2, 1));
        vehicle.engine_id = Some(engine_id);
        vehicle.cur_speed = 24;
        vehicle.depot_leave_cleared = true;
        vehicle
    }

    fn callback_literal(value: u8) -> TrainSpriteGraphics {
        let mut gfx = TrainSpriteGraphics::default();
        gfx.assigns.push(TrainSpriteAssign {
            local_id: 0,
            set_id: 2,
        });
        gfx.action2_var.insert(
            2,
            Action2VarEntry {
                first: Action2VarTerm {
                    variable: 0x1A,
                    param: None,
                    adjust: Action2VarAdjust {
                        shift: 0,
                        and_mask: u32::from(value),
                        ..Action2VarAdjust::default()
                    },
                },
                ops: Vec::new(),
                ranges: Vec::new(),
                default: 0,
            },
        );
        gfx
    }

    #[test]
    fn smoke_kind_matches_engine_class() {
        assert_eq!(train_smoke_kind(ENGINE_TRAIN_KIRBY), TrainSmokeKind::Steam);
        assert_eq!(
            train_smoke_kind(ENGINE_TRAIN_ASIASTAR),
            TrainSmokeKind::Electric
        );
    }

    #[test]
    fn smoke_call_site_honors_newgrf_visual_effect_disable() {
        let map = Map::new_flat(4, 4, 0);
        let mut vehicle = running_train(ENGINE_TRAIN_KIRBY);
        let Some(mut engine) = openttdrs_core::engine_by_id(ENGINE_TRAIN_KIRBY).cloned() else {
            panic!("motor vanilla ausente");
        };
        engine.newgrf_grfid = 0x5649_5355;
        engine.newgrf_local_id = 0;
        engine.vehicle_callback_mask = 1;
        // CB10 bit 6 = VE_DISABLE_EFFECT.
        engine.newgrf_runtime = Some(Box::new(callback_literal(0x40)));
        assert!(train_smoke_to_emit_with_engine(&map, &mut vehicle, &engine, 0, 2).is_none());
    }

    #[test]
    fn advanced_effect_offsets_rotate_with_vehicle_direction() {
        let mut vehicle = running_train(ENGINE_TRAIN_KIRBY);
        vehicle.direction = 0;
        let spawn = VehicleAdvancedVisualEffectSpawn {
            effect_type: 0xF1,
            x: 2,
            y: 3,
            z: -4,
        };
        assert_eq!(
            advanced_effect_offset(&vehicle, spawn, false),
            Vec3::new(2.0, 3.0, -4.0)
        );
        assert_eq!(
            advanced_effect_offset(&vehicle, spawn, true),
            Vec3::new(5.0, -1.0, -4.0)
        );
    }

    #[test]
    fn steam_effect_uses_openttd_frame_and_rise_cadence() {
        assert_eq!(
            effect_tick_state(TrainSmokeSet::Steam, 0),
            Some(EffectTickState { frame: 0, rise: 0 })
        );
        assert_eq!(
            effect_tick_state(TrainSmokeSet::Steam, 4),
            Some(EffectTickState { frame: 0, rise: 1 })
        );
        assert_eq!(
            effect_tick_state(TrainSmokeSet::Steam, 8),
            Some(EffectTickState { frame: 1, rise: 1 })
        );
        assert_eq!(
            effect_tick_state(TrainSmokeSet::Steam, 56),
            Some(EffectTickState { frame: 4, rise: 7 })
        );
        assert_eq!(effect_tick_state(TrainSmokeSet::Steam, 72), None);
    }

    #[test]
    fn diesel_and_electric_effects_reach_last_frame_then_cull() {
        assert_eq!(
            effect_tick_state(TrainSmokeSet::Diesel, 0),
            Some(EffectTickState { frame: 0, rise: 0 })
        );
        assert_eq!(
            effect_tick_state(TrainSmokeSet::Diesel, 33).map(|state| state.frame),
            Some(5)
        );
        assert_eq!(effect_tick_state(TrainSmokeSet::Diesel, 41), None);
        assert_eq!(
            effect_tick_state(TrainSmokeSet::Electric, 16).map(|state| state.frame),
            Some(5)
        );
        assert_eq!(effect_tick_state(TrainSmokeSet::Electric, 17), None);
    }

    #[test]
    fn emission_rejects_off_stopped_wagon_hidden_and_station_states() {
        let mut map = Map::new_flat(4, 4, 0);
        let mut vehicle = running_train(ENGINE_TRAIN_KIRBY);
        assert!(train_smoke_to_emit(&map, &mut vehicle, 0, 0).is_none());
        vehicle.cur_speed = 0;
        assert!(train_smoke_to_emit(&map, &mut vehicle, 0, 2).is_none());
        vehicle.cur_speed = 24;
        vehicle.running = false;
        assert!(train_smoke_to_emit(&map, &mut vehicle, 0, 2).is_none());
        vehicle.running = true;
        vehicle.engine_id = Some(openttdrs_core::ENGINE_WAGON_COAL);
        assert!(train_smoke_to_emit(&map, &mut vehicle, 0, 2).is_none());

        vehicle.engine_id = Some(ENGINE_TRAIN_KIRBY);
        vehicle.set_station_orders(vec![vehicle.pos]);
        assert!(map.set_kind(vehicle.pos, TileKind::Station).is_ok());
        assert!(train_smoke_to_emit(&map, &mut vehicle, 0, 2).is_none());
    }

    #[test]
    fn steam_density_uses_tick_mask_and_not_visual_time() {
        let map = Map::new_flat(4, 4, 0);
        let mut vehicle = running_train(ENGINE_TRAIN_KIRBY);
        assert_eq!(
            train_smoke_to_emit(&map, &mut vehicle, 0, 2),
            Some(TrainSmokeSet::Steam)
        );
        assert!(train_smoke_to_emit(&map, &mut vehicle, 1, 2).is_none());
    }
}
