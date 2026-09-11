//! Humo/chispas de locomotoras (`EV_STEAM_SMOKE`, `EV_DIESEL_SMOKE`, `EV_ELECTRIC_SPARK`).
//!
//! Tanto la decisión de emitir como el avance del efecto se apoyan en ticks de
//! simulación. Así el resultado no depende de FPS ni del reloj visual.

use bevy::prelude::*;

use openttdrs_core::prelude::*;
use openttdrs_core::{
    EngineDef, Vehicle, VehicleAdvancedVisualEffectSpawn, VehicleOrder, VehicleVisualEffectKind,
    extrapolate_vehicle_pose, resolve_vehicle_spawn_visual_effect_callback, slope_dz_at_subtile,
    train_smoke_kind, vehicle_subtile_at_with_map, vehicle_visual_effect_spec,
};

use crate::audio::{PlayWorldSfx, play_vehicle_event_sound_with_default};
use crate::bevy_app::UpdateSet;
use crate::iso::{road_vehicle_tile_anchor, tile_slope_and_min_z, wang_hash};
use crate::render::effect_vehicle::{EffectSpriteSet, EffectVehicleFrames, apply_effect_frame};
use crate::render::viewport_sort::ParentSpriteBounds;
use crate::render::{
    MapVisualLayer, ViewportSortableParent, palette_animations_should_run,
    vehicles::vehicle_draw_anchor_from_pose, viewport_insertion_key, viewport_source_depth,
};
use crate::settings::ClientPreferences;
use crate::simulation::SimClock;
use crate::state::{ClientScreen, SimWorld};
use crate::ui::SimHudControls;

const MAX_TRAIN_SMOKE_EFFECTS: usize = 48;
const TRAIN_EFFECT_PARENT_ORDINAL_BASE: u8 = 0x80;
const TRAIN_EFFECT_SORT_SPRITE_ID_BASE: u32 = 0xFFFE_0010;
const TILE_SIZE_PX: i32 = 16;

pub(crate) struct TrainSmokePlugin;

impl Plugin for TrainSmokePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TrainSmokeSpawnClock>()
            .init_resource::<TrainSmokeSortSequence>()
            .add_systems(
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

/// Desempate estable para `EffectVehicle`s creados en el mismo pase visual.
/// El límite simultáneo de efectos es menor que los 128 ordinales disponibles,
/// por lo que el wrap no puede volver a empatar dos penachos vivos.
#[derive(Resource, Default)]
struct TrainSmokeSortSequence(u8);

#[derive(Component)]
pub(crate) struct TrainSmokeEffect {
    started_tick: u64,
    origin: TrainSmokeWorldPosition,
    set: TrainSmokeSet,
    sort_ordinal: u8,
}

/// Posición de mundo del `EffectVehicle` que OpenTTD crea con
/// `CreateEffectVehicleRel`. La traslación puede conservar fracciones de la
/// interpolación de render; el prisma del compositor se redondea al píxel de
/// mundo que usa el vehículo nativo.
#[derive(Debug, Clone, Copy, PartialEq)]
struct TrainSmokeWorldPosition {
    x: f32,
    y: f32,
    z: f32,
    source_tile: TileCoord,
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

fn deterministic_random16(vehicle_id: u32, tick_counter: u8, salt: u32) -> u16 {
    wang_hash(vehicle_id, u32::from(tick_counter), salt) as u16
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

/// Replica el filtro `HasPowerOnRail` de `Vehicle::ShowVisualEffect`.
///
/// La compatibilidad de una locomotora eléctrica con una vía normal no
/// implica que tenga tracción allí: los efectos se suprimen cuando el
/// railtype actual no pertenece a la máscara alimentada por el motor. Las
/// demás clases de vehículo no pasan por este filtro nativo.
#[must_use]
fn vehicle_has_power_on_current_rail(map: &Map, vehicle: &Vehicle, engine: &EngineDef) -> bool {
    if vehicle.kind != VehicleKind::Train {
        return true;
    }
    let Some(tile) = map.get(vehicle.pos) else {
        return true;
    };
    let rail_type = openttdrs_core::rail_type_from_tile(tile);
    let required = engine.required_rail_type.map_or_else(
        || openttdrs_core::required_rail_type_for_engine(engine.id),
        openttdrs_core::RailType::from_u8,
    );
    openttdrs_core::railtypes_mask_contains(
        openttdrs_core::powered_railtypes_mask(required),
        rail_type,
    )
}

/// Port de las reglas de `Vehicle::ShowVisualEffect` para trenes.
#[must_use]
#[cfg(test)]
fn train_smoke_to_emit(
    map: &Map,
    vehicle: &mut Vehicle,
    tick_counter: u8,
    smoke_amount: u8,
) -> Option<TrainSmokeSet> {
    vehicle.newgrf_tick_counter = tick_counter;
    let engine = vehicle.effective_engine();
    train_smoke_to_emit_with_engine(map, vehicle, engine, smoke_amount)
}

/// Igual que `train_smoke_to_emit`, pero usando el catálogo de motores de la
/// partida para resolver callbacks de vehículos NewGRF.
fn train_smoke_to_emit_with_engine(
    map: &Map,
    vehicle: &mut Vehicle,
    engine: &EngineDef,
    smoke_amount: u8,
) -> Option<TrainSmokeSet> {
    let amount = smoke_amount.min(2);
    if amount == 0
        || vehicle.kind != VehicleKind::Train
        || !vehicle_has_power_on_current_rail(map, vehicle, engine)
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
        VehicleVisualEffectKind::Default if engine.is_train_engine() => train_smoke_kind(engine.id),
        // OpenTTD disables the default effect for wagons; only an explicit
        // CB10 result may opt a wagon into a standard effect.
        VehicleVisualEffectKind::Default => return None,
    };
    let tick_counter = vehicle.newgrf_tick_counter;
    match smoke_kind {
        openttdrs_core::TrainSmokeKind::Steam => {
            let bits = u32::from((4_u8 >> amount) + (speed.saturating_mul(3) / max_speed) as u8);
            let mask = (1_u64 << bits.min(63)).saturating_sub(1);
            (u64::from(tick_counter) & mask == 0).then_some(TrainSmokeSet::Steam)
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
                    deterministic_random16(vehicle.id, tick_counter, 0xD1E5_E100),
                    numerator,
                    512_u32 >> amount,
                ))
            .then_some(TrainSmokeSet::Diesel)
        }
        openttdrs_core::TrainSmokeKind::Electric => {
            let numerator = 6 - i32::from(speed) * 4 / i32::from(max_speed);
            (tick_counter & 3 == 0
                && chance16(
                    deterministic_random16(vehicle.id, tick_counter, 0xE1EC_7A1C),
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

/// Proyecta `SpawnAdvancedVisualEffect`: rotación signed de 8 bits y centro
/// dependiente del tipo de vehículo y del bit 13 del callback.
#[must_use]
fn advanced_effect_world_offset(
    vehicle: &Vehicle,
    spawn: VehicleAdvancedVisualEffectSpawn,
    auto_center: bool,
    auto_rotate: bool,
) -> IVec3 {
    let mut x = i32::from(spawn.x);
    let mut y = i32::from(spawn.y);
    let direction = if vehicle.kind == VehicleKind::Train && vehicle.train_flags & (1 << 4) != 0 {
        vehicle.direction.wrapping_add(4) & 7
    } else {
        vehicle.direction & 7
    };
    if auto_rotate {
        const SMOKE_POS: [i32; 8] = [1, 1, 1, 0, -1, -1, -1, 0];
        let longitudinal = usize::from(direction);
        let transverse = (longitudinal + 2) & 7;
        let local_x = x;
        let local_y = y;
        // Upstream asigna el resultado a int8_t antes de sumar el centro.
        x = i32::from((SMOKE_POS[longitudinal] * local_x + SMOKE_POS[transverse] * local_y) as i8);
        y = i32::from((SMOKE_POS[transverse] * local_x - SMOKE_POS[longitudinal] * local_y) as i8);
    }
    {
        // OpenTTD centra los efectos de carretera respecto del frente de la
        // unidad y los de tren respecto de la posición de su sprite. Barcos y
        // aeronaves ya reciben una posición centrada de `CreateEffectVehicle`.
        // El centro se suma después de rotar el registro, como en
        // `SpawnAdvancedVisualEffect` upstream.
        let length_delta = i32::from(openttdrs_core::train_consist::VEHICLE_LENGTH)
            - i32::from(vehicle.unit_length.max(1));
        let longitudinal = match vehicle.kind {
            VehicleKind::Bus | VehicleKind::Truck | VehicleKind::Tram if auto_center => {
                -length_delta / 2
            }
            VehicleKind::Train if !auto_center => length_delta / 2,
            _ => 0,
        };
        const SMOKE_POS: [i32; 8] = [1, 1, 1, 0, -1, -1, -1, 0];
        let transverse = (usize::from(direction) + 2) & 7;
        x += SMOKE_POS[usize::from(direction)] * longitudinal;
        y += SMOKE_POS[transverse] * longitudinal;
    }
    IVec3::new(x, y, i32::from(spawn.z))
}

/// Convierte un offset relativo de `CreateEffectVehicleRel` a pantalla. Se
/// conserva para las comprobaciones de CB160; el spawn real usa las mismas
/// coordenadas de mundo para construir el prisma sortable.
#[must_use]
#[cfg(test)]
fn vehicle_effect_overlay_offset(offset: IVec3) -> Vec3 {
    let screen = road_vehicle_tile_anchor(0, 0, offset.x as f32, offset.y as f32, offset.z as f32);
    Vec3::new(screen.x, screen.y, 0.0)
}

#[must_use]
#[cfg(test)]
fn advanced_effect_offset(
    vehicle: &Vehicle,
    spawn: VehicleAdvancedVisualEffectSpawn,
    auto_center: bool,
    auto_rotate: bool,
) -> Vec3 {
    vehicle_effect_overlay_offset(advanced_effect_world_offset(
        vehicle,
        spawn,
        auto_center,
        auto_rotate,
    ))
}

/// Replica la decisión de `Vehicle::ShowVisualEffect` que precede a CB160.
/// El callback avanzado no debe ejecutarse para un vehículo detenido, oculto,
/// dentro de un depósito/túnel o cubierto por un puente.
#[must_use]
fn advanced_effect_should_emit(
    map: &Map,
    vehicle: &Vehicle,
    engine: &EngineDef,
    kind: VehicleVisualEffectKind,
    smoke_amount: u8,
) -> bool {
    let amount = smoke_amount.min(2);
    if amount == 0
        || !vehicle.running
        || vehicle.crashed
        || vehicle.cur_speed < 2
        || !vehicle_has_power_on_current_rail(map, vehicle, engine)
        || openttdrs_core::vehicle_hidden_from_view(map, vehicle, vehicle.pos, vehicle.progress)
        || openttdrs_core::vehicle_in_depot(map, vehicle.pos)
        || map
            .get_kind(vehicle.pos)
            .is_some_and(|tile| matches!(tile, TileKind::RailTunnel | TileKind::RoadTunnel))
        || map
            .get(vehicle.pos)
            .is_some_and(|tile| openttdrs_core::bridge_above_axis_from_mapt(tile.mapt).is_some())
    {
        return false;
    }
    if vehicle.kind == VehicleKind::Train
        && (vehicle.train_flags & 1 != 0 || train_is_stopping_at_station(map, vehicle))
    {
        return false;
    }
    let max_speed = if vehicle.cached_max_speed == 0 || vehicle.cached_max_speed == u16::MAX {
        engine.max_speed.max(1)
    } else {
        vehicle.cached_max_speed.max(1)
    };
    let speed = vehicle.cur_speed.min(max_speed);
    let tick_counter = vehicle.newgrf_tick_counter;
    match kind {
        VehicleVisualEffectKind::Steam => {
            let bits = u32::from((4_u8 >> amount) + (speed.saturating_mul(3) / max_speed) as u8);
            u64::from(tick_counter) & (1_u64 << bits.min(63)).saturating_sub(1) == 0
        }
        VehicleVisualEffectKind::Diesel => {
            let power_weight_effect = if vehicle.kind == VehicleKind::Train {
                let power_shift = (vehicle.cached_power_hp >> 10).min(31);
                let weight_shift = (u32::from(vehicle.cached_weight_t) >> 9).min(31);
                (32_u32 >> power_shift) as i32 - (32_u32 >> weight_shift) as i32
            } else {
                0
            };
            let speed_limit = max_speed >> (2_u8 >> amount);
            let numerator = 64 - i32::from(speed) * 32 / i32::from(max_speed) + power_weight_effect;
            speed < speed_limit
                && chance16(
                    deterministic_random16(vehicle.id, tick_counter, 0xD1E5_E100),
                    numerator,
                    512_u32 >> amount,
                )
        }
        VehicleVisualEffectKind::Electric => {
            let numerator = 6 - i32::from(speed) * 4 / i32::from(max_speed);
            tick_counter & 3 == 0
                && chance16(
                    deterministic_random16(vehicle.id, tick_counter, 0xE1EC_7A1C),
                    numerator,
                    360_u32 >> amount,
                )
        }
        VehicleVisualEffectKind::Default | VehicleVisualEffectKind::Disabled => false,
    }
}

/// Offset de `CreateEffectVehicleRel` para el modelo vanilla de CB10.
/// `offset=8` es el centro de una unidad estándar; OpenTTD corrige las
/// unidades ferroviarias acortadas y respeta la inversión visual de trenes.
#[must_use]
fn standard_effect_world_offset(vehicle: &Vehicle, offset: u8) -> IVec3 {
    let direction = if vehicle.kind == VehicleKind::Train && vehicle.train_flags & (1 << 4) != 0 {
        vehicle.direction.wrapping_add(4) & 7
    } else {
        vehicle.direction & 7
    };
    let mut longitudinal =
        i32::from(offset) - i32::from(openttdrs_core::train_consist::VEHICLE_LENGTH);
    if vehicle.kind == VehicleKind::Train {
        longitudinal += (i32::from(openttdrs_core::train_consist::VEHICLE_LENGTH)
            - i32::from(vehicle.unit_length.max(1)))
            / 2;
    }
    const SMOKE_POS: [i32; 8] = [1, 1, 1, 0, -1, -1, -1, 0];
    let transverse = (usize::from(direction) + 2) & 7;
    IVec3::new(
        SMOKE_POS[usize::from(direction)] * longitudinal,
        SMOKE_POS[transverse] * longitudinal,
        10,
    )
}

#[must_use]
#[cfg(test)]
fn standard_effect_offset(vehicle: &Vehicle, offset: u8) -> Vec3 {
    vehicle_effect_overlay_offset(standard_effect_world_offset(vehicle, offset))
}

/// Tesela segura que referencia un `EffectVehicle` aun si CB160 lo desplazó
/// fuera del borde del mapa. OpenTTD conserva las coordenadas del efecto, pero
/// sus consultas de mapa se limitan al borde más próximo.
fn train_smoke_source_tile(map: &Map, x: f32, y: f32) -> TileCoord {
    let (width, height) = map.dimensions();
    let max_x = i32::try_from(width.saturating_sub(1))
        .unwrap_or(i32::MAX)
        .saturating_mul(TILE_SIZE_PX);
    let max_y = i32::try_from(height.saturating_sub(1))
        .unwrap_or(i32::MAX)
        .saturating_mul(TILE_SIZE_PX);
    let x = x.round() as i32;
    let y = y.round() as i32;
    TileCoord::new(
        x.clamp(0, max_x).div_euclid(TILE_SIZE_PX),
        y.clamp(0, max_y).div_euclid(TILE_SIZE_PX),
    )
}

/// Convierte la ancla de dibujo de un vehículo al punto de mundo que usa
/// `CreateEffectVehicleRel`. Resolver a partir de la proyección conserva el
/// cruce continuo de rampas de puentes que ya modela
/// `vehicle_draw_anchor_from_pose`, a la vez que recupera una caja de mundo
/// para el compositor global.
fn train_smoke_world_position(
    vehicle: &Vehicle,
    map: &Map,
    pose: openttdrs_core::VehiclePose,
    offset: IVec3,
) -> TrainSmokeWorldPosition {
    let (anchor, base_z, tx, ty) = vehicle_draw_anchor_from_pose(vehicle, map, pose);
    let (sub_x, sub_y) = vehicle_subtile_at_with_map(vehicle, pose, Some(map));
    let (tileh, _) = tile_slope_and_min_z(
        map,
        u32::try_from(tx).unwrap_or(0),
        u32::try_from(ty).unwrap_or(0),
    );
    let height_px = f32::from(openttdrs_core::TILE_PIXEL_HEIGHT);
    let terrain_z = f32::from(base_z) * height_px + slope_dz_at_subtile(sub_x, sub_y, tileh);
    // `road_vehicle_tile_anchor(0, 0, x, y, z)` cumple:
    //   screen.x = 2 · (y - x), screen.y = -(x + y - z).
    // La ancla histórica ya incluye la pendiente; el desplazamiento por
    // `base_z` lo aplicaba `effect_overlay_pos`. Invertir esa proyección
    // mantiene exactamente el mismo X/Y visual y también en los puentes.
    let projected_y = anchor.y + f32::from(base_z) * height_px;
    let world_sum = terrain_z - projected_y;
    let world_delta = anchor.x * 0.5;
    let x = (world_sum - world_delta) * 0.5 + offset.x as f32;
    let y = (world_sum + world_delta) * 0.5 + offset.y as f32;
    let aircraft_altitude = if vehicle.kind == VehicleKind::Aircraft {
        f32::from(vehicle.altitude) * height_px
    } else {
        0.0
    };
    TrainSmokeWorldPosition {
        x,
        y,
        z: terrain_z + aircraft_altitude + offset.z as f32,
        source_tile: train_smoke_source_tile(map, x, y),
    }
}

fn train_smoke_source_depth(position: TrainSmokeWorldPosition, map_width: u32) -> f32 {
    let diagonal_depth = (position.source_tile.x + position.source_tile.y) as f32 * 0.01;
    let height_depth = position.z / f32::from(openttdrs_core::TILE_PIXEL_HEIGHT) * 0.0001;
    viewport_source_depth(
        diagonal_depth + height_depth + 0.001,
        u32::try_from(position.source_tile.x).unwrap_or(0),
        map_width,
    )
}

fn train_smoke_sort_sprite_id(set: TrainSmokeSet) -> u32 {
    TRAIN_EFFECT_SORT_SPRITE_ID_BASE
        + match set {
            TrainSmokeSet::Steam => 0,
            TrainSmokeSet::Diesel => 1,
            TrainSmokeSet::Electric => 2,
            TrainSmokeSet::Breakdown => 3,
        }
}

/// `EffectVehicle::UpdateDeltaXY` fija `{ {}, {1, 1, 1}, {} }`; la caja
/// inclusiva del compositor es por tanto exactamente un píxel cúbico.
fn train_smoke_parent(
    position: TrainSmokeWorldPosition,
    set: TrainSmokeSet,
    sort_ordinal: u8,
    map_width: u32,
) -> ViewportSortableParent {
    let x = position.x.round() as i32;
    let y = position.y.round() as i32;
    let z = position.z.round() as i32;
    let source_x = u32::try_from(position.source_tile.x).unwrap_or(0);
    let source_y = u32::try_from(position.source_tile.y).unwrap_or(0);
    ViewportSortableParent {
        sprite_id: train_smoke_sort_sprite_id(set),
        bounds: ParentSpriteBounds::new(x, y, z, x, y, z),
        insertion_key: viewport_insertion_key(
            source_x,
            source_y,
            TRAIN_EFFECT_PARENT_ORDINAL_BASE | (sort_ordinal & 0x7F),
        ),
        source_depth: train_smoke_source_depth(position, map_width),
    }
}

fn train_smoke_translation(
    position: TrainSmokeWorldPosition,
    frame: usize,
    set: &EffectSpriteSet<'_>,
    source_depth: f32,
) -> Vec3 {
    let idx = frame.min(set.meta.len().saturating_sub(1));
    let (w, h, xrel, yrel) = set.meta[idx];
    let anchor = road_vehicle_tile_anchor(0, 0, position.x, position.y, position.z);
    Vec3::new(
        anchor.x + xrel + w * 0.5,
        anchor.y - (yrel + h * 0.5),
        source_depth,
    )
}

/// El sorter escribe la Z efectiva del parent. La animación puede cambiar
/// sprite y altura física, pero no debe restaurar la profundidad fuente en un
/// frame estable.
fn set_train_smoke_translation_if_changed(
    transform: &mut Mut<Transform>,
    source_translation: Vec3,
    preserves_sorted_depth: bool,
) {
    let translation = if preserves_sorted_depth {
        Vec3::new(
            source_translation.x,
            source_translation.y,
            transform.translation.z,
        )
    } else {
        source_translation
    };
    if transform.translation != translation {
        transform.translation = translation;
    }
}

fn spawn_train_smoke_effect(
    commands: &mut Commands,
    frames: &EffectVehicleFrames,
    tick: u64,
    set: TrainSmokeSet,
    origin: TrainSmokeWorldPosition,
    sort_ordinal: u8,
    map_width: u32,
) -> bool {
    let effect_set = sprite_set(frames, set);
    let Some(atlas) = effect_set.frames.first() else {
        return false;
    };
    let parent = train_smoke_parent(origin, set, sort_ordinal, map_width);
    let mut sprite = atlas.sprite();
    if matches!(set, TrainSmokeSet::Electric) {
        sprite.color = Color::srgb(0.85, 0.92, 1.0);
    }
    commands.spawn((
        MapVisualLayer,
        TrainSmokeEffect {
            started_tick: tick,
            origin,
            set,
            sort_ordinal,
        },
        sprite,
        Transform::from_translation(train_smoke_translation(
            origin,
            0,
            &effect_set,
            parent.source_depth,
        )),
        Visibility::Visible,
        parent,
    ));
    true
}

#[allow(clippy::too_many_arguments)]
fn spawn_train_smoke(
    mut sim: ResMut<SimWorld>,
    sim_clock: Res<SimClock>,
    prefs: Res<ClientPreferences>,
    hud: Res<SimHudControls>,
    frames: Res<EffectVehicleFrames>,
    mut spawn_clock: ResMut<TrainSmokeSpawnClock>,
    mut sort_sequence: ResMut<TrainSmokeSortSequence>,
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
    let map_width = map.dimensions().0;
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
            if !advanced_effect_should_emit(
                map,
                vehicle,
                engine,
                visual_spec.kind,
                prefs.smoke_amount,
            ) {
                continue;
            }
            // `ShowVisualEffect` no emite el modelo vanilla cuando el bit 6
            // pide el callback avanzado: si el callback falla, el resultado
            // correcto es no crear efectos, no degradar a humo estándar.
            let random =
                deterministic_random16(vehicle.id, vehicle.newgrf_tick_counter, 0x1600_0000);
            let advanced = resolve_vehicle_spawn_visual_effect_callback(engine, vehicle, random);
            let Some(advanced) = advanced else {
                continue;
            };
            let pose = extrapolate_vehicle_pose(vehicle, sim_clock.tick_alpha);
            let mut emitted = false;
            for spawn in advanced.spawns.iter().take(usize::from(advanced.count)) {
                let Some(set_kind) = advanced_effect_set(spawn.effect_type) else {
                    continue;
                };
                if active_count >= MAX_TRAIN_SMOKE_EFFECTS {
                    break;
                }
                let offset = advanced_effect_world_offset(
                    vehicle,
                    *spawn,
                    advanced.auto_center,
                    advanced.auto_rotate,
                );
                let origin = train_smoke_world_position(vehicle, map, pose, offset);
                let sort_ordinal = sort_sequence.0;
                if spawn_train_smoke_effect(
                    &mut commands,
                    &frames,
                    tick,
                    set_kind,
                    origin,
                    sort_ordinal,
                    map_width,
                ) {
                    sort_sequence.0 = sort_sequence.0.wrapping_add(1);
                    emitted = true;
                    active_count += 1;
                }
            }
            if emitted && hud.sound_vehicle && vehicle.is_consist_head() {
                visual_sound_events.push((vehicle.id, vehicle.pos));
            }
            continue;
        }
        let set_kind = if vehicle.kind == VehicleKind::Train {
            train_smoke_to_emit_with_engine(map, vehicle, engine, prefs.smoke_amount)
        } else if matches!(
            visual_spec.kind,
            VehicleVisualEffectKind::Steam
                | VehicleVisualEffectKind::Diesel
                | VehicleVisualEffectKind::Electric
        ) && advanced_effect_should_emit(
            map,
            vehicle,
            engine,
            visual_spec.kind,
            prefs.smoke_amount,
        ) {
            Some(match visual_spec.kind {
                VehicleVisualEffectKind::Steam => TrainSmokeSet::Steam,
                VehicleVisualEffectKind::Diesel => TrainSmokeSet::Diesel,
                VehicleVisualEffectKind::Electric => TrainSmokeSet::Electric,
                VehicleVisualEffectKind::Default | VehicleVisualEffectKind::Disabled => {
                    unreachable!("standard model was checked above")
                }
            })
        } else {
            None
        };
        let Some(set_kind) = set_kind else {
            continue;
        };
        let pose = extrapolate_vehicle_pose(vehicle, sim_clock.tick_alpha);
        let origin = train_smoke_world_position(
            vehicle,
            map,
            pose,
            standard_effect_world_offset(vehicle, visual_spec.offset),
        );
        let sort_ordinal = sort_sequence.0;
        if !spawn_train_smoke_effect(
            &mut commands,
            &frames,
            tick,
            set_kind,
            origin,
            sort_ordinal,
            map_width,
        ) {
            continue;
        }
        sort_sequence.0 = sort_sequence.0.wrapping_add(1);
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
    mut q: Query<(
        Entity,
        &mut Transform,
        &TrainSmokeEffect,
        &mut Sprite,
        Option<&mut ViewportSortableParent>,
    )>,
    mut commands: Commands,
) {
    if !frames.is_loaded() {
        return;
    }
    let tick = sim.state.tick.get();
    let map_width = sim.state.map.dimensions().0;
    for (entity, mut transform, smoke, mut sprite, parent) in &mut q {
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
        let position = TrainSmokeWorldPosition {
            z: smoke.origin.z + f32::from(state.rise),
            ..smoke.origin
        };
        let next_parent = train_smoke_parent(position, smoke.set, smoke.sort_ordinal, map_width);
        let translation =
            train_smoke_translation(position, state.frame, &effect_set, next_parent.source_depth);
        let preserves_sorted_depth = parent.is_some();
        set_train_smoke_translation_if_changed(&mut transform, translation, preserves_sorted_depth);
        if let Some(mut parent) = parent {
            if *parent != next_parent {
                *parent = next_parent;
            }
        } else {
            commands.entity(entity).insert(next_parent);
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
    use crate::render::effect_vehicle::effect_overlay_pos;
    use crate::render::{ViewportSortableChildDepthWindows, sort_viewport_sortable_parents};

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
        assert!(train_smoke_to_emit_with_engine(&map, &mut vehicle, &engine, 2).is_none());
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
            advanced_effect_offset(&vehicle, spawn, false, false),
            Vec3::new(2.0, -9.0, 0.0)
        );
        assert_eq!(
            advanced_effect_offset(&vehicle, spawn, false, true),
            Vec3::new(-12.0, -8.0, 0.0)
        );
    }

    #[test]
    fn advanced_effect_offset_applies_vehicle_center_by_kind() {
        let spawn = VehicleAdvancedVisualEffectSpawn {
            effect_type: 0xF1,
            ..VehicleAdvancedVisualEffectSpawn::default()
        };
        let mut train = running_train(ENGINE_TRAIN_KIRBY);
        train.unit_length = 4;
        train.direction = 0;
        assert_eq!(
            advanced_effect_offset(&train, spawn, false, false),
            Vec3::new(0.0, -4.0, 0.0)
        );
        assert_eq!(
            advanced_effect_offset(
                &train,
                VehicleAdvancedVisualEffectSpawn {
                    x: 2,
                    y: 3,
                    ..spawn
                },
                false,
                true,
            ),
            Vec3::new(-12.0, -8.0, 0.0)
        );

        let mut bus = Vehicle::new(
            8,
            VehicleKind::Bus,
            TileCoord::new(1, 1),
            TileCoord::new(2, 1),
        );
        bus.unit_length = 4;
        bus.direction = 0;
        assert_eq!(
            advanced_effect_offset(&bus, spawn, true, false),
            Vec3::new(0.0, 4.0, 0.0)
        );

        let mut ship = Vehicle::new(
            9,
            VehicleKind::Ship,
            TileCoord::new(1, 1),
            TileCoord::new(2, 1),
        );
        ship.unit_length = 4;
        assert_eq!(
            advanced_effect_offset(&ship, spawn, true, false),
            Vec3::ZERO
        );
    }

    #[test]
    fn advanced_effect_emission_works_for_non_train_vehicles_and_stops_when_hidden() {
        let map = Map::new_flat(8, 8, 0);
        for kind in [VehicleKind::Bus, VehicleKind::Ship, VehicleKind::Aircraft] {
            let mut vehicle = Vehicle::new(
                30 + kind as u32,
                kind,
                TileCoord::new(2, 2),
                TileCoord::new(3, 2),
            );
            vehicle.cur_speed = 24;
            let engine = vehicle.effective_engine();
            assert!(advanced_effect_should_emit(
                &map,
                &vehicle,
                engine,
                VehicleVisualEffectKind::Steam,
                2
            ));
            vehicle.newgrf_tick_counter = 1;
            assert!(!advanced_effect_should_emit(
                &map,
                &vehicle,
                engine,
                VehicleVisualEffectKind::Steam,
                2
            ));
            vehicle.running = false;
            assert!(!advanced_effect_should_emit(
                &map,
                &vehicle,
                engine,
                VehicleVisualEffectKind::Steam,
                2
            ));
        }
    }

    #[test]
    fn standard_effect_offset_uses_cb10_offset_and_train_flip() {
        let mut train = running_train(ENGINE_TRAIN_KIRBY);
        train.direction = 0;
        train.unit_length = 4;
        assert_eq!(standard_effect_offset(&train, 8), Vec3::new(0.0, 6.0, 0.0));
        train.train_flags |= 1 << 4;
        assert_eq!(standard_effect_offset(&train, 8), Vec3::new(0.0, 14.0, 0.0));

        let mut bus = Vehicle::new(
            40,
            VehicleKind::Bus,
            TileCoord::new(1, 1),
            TileCoord::new(2, 1),
        );
        bus.direction = 0;
        assert_eq!(standard_effect_offset(&bus, 4), Vec3::new(0.0, 18.0, 0.0));
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn train_effect_world_position_preserves_legacy_projection_on_a_slope() {
        let mut map = Map::new_flat(4, 4, 4);
        map.set_height(TileCoord::new(1, 1), 5)
            .expect("slope corner");
        let mut vehicle = running_train(ENGINE_TRAIN_KIRBY);
        vehicle.direction = 0;
        vehicle.unit_length = 4;
        let pose = openttdrs_core::VehiclePose::from_vehicle(&vehicle);
        let offset = standard_effect_world_offset(&vehicle, 8);
        let origin = train_smoke_world_position(&vehicle, &map, pose, offset);
        let (anchor, base_z, _, _) = vehicle_draw_anchor_from_pose(&vehicle, &map, pose);
        let legacy_offset = vehicle_effect_overlay_offset(offset);
        let legacy = Vec2::new(
            anchor.x + legacy_offset.x,
            anchor.y
                + legacy_offset.y
                + f32::from(base_z) * f32::from(openttdrs_core::TILE_PIXEL_HEIGHT),
        );
        assert_eq!(
            road_vehicle_tile_anchor(0, 0, origin.x, origin.y, origin.z),
            legacy,
            "la posición de mundo reproduce el ancla local previa incluso sobre una pendiente"
        );
        let parent = train_smoke_parent(origin, TrainSmokeSet::Steam, 9, map.dimensions().0);
        assert_eq!(
            parent.bounds,
            ParentSpriteBounds::new(
                origin.x.round() as i32,
                origin.y.round() as i32,
                origin.z.round() as i32,
                origin.x.round() as i32,
                origin.y.round() as i32,
                origin.z.round() as i32,
            ),
            "EffectVehicle::UpdateDeltaXY usa un prisma inclusivo de 1×1×1"
        );
    }

    #[test]
    fn aircraft_effect_world_position_tracks_flight_altitude() {
        let map = Map::new_flat(4, 4, 0);
        let pos = TileCoord::new(1, 1);
        let mut aircraft = Vehicle::new(12, VehicleKind::Aircraft, pos, pos);
        aircraft.aircraft_phase = openttdrs_core::AircraftPhase::Flying;
        let pose = openttdrs_core::VehiclePose::from_vehicle(&aircraft);
        let ground = train_smoke_world_position(&aircraft, &map, pose, IVec3::ZERO);

        aircraft.altitude = openttdrs_core::aircraft_movement::AIRCRAFT_CRUISE_ALTITUDE;
        let airborne = train_smoke_world_position(&aircraft, &map, pose, IVec3::ZERO);

        assert_eq!(airborne.x, ground.x);
        assert_eq!(airborne.y, ground.y);
        assert_eq!(
            airborne.z - ground.z,
            f32::from(openttdrs_core::aircraft_movement::AIRCRAFT_CRUISE_ALTITUDE)
                * f32::from(openttdrs_core::TILE_PIXEL_HEIGHT)
        );
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn cb160_positions_match_native_oracle_at_every_zoom() {
        use bevy::camera::CameraProjection;

        // Generated by executing SpawnAdvancedVisualEffect from OpenTTD 15.3.
        let fixture = include_str!("../../tests/fixtures/vehicle_effect_positions.tsv");
        for line in fixture.lines().skip(1) {
            let fields: Vec<i32> = line
                .split('\t')
                .map(|value| value.parse().unwrap())
                .collect();
            assert_eq!(fields.len(), 12);
            let kind = [
                VehicleKind::Train,
                VehicleKind::Bus,
                VehicleKind::Ship,
                VehicleKind::Aircraft,
            ][fields[0] as usize];
            let mut vehicle = Vehicle::new(1, kind, TileCoord::new(1, 1), TileCoord::new(2, 1));
            vehicle.direction = fields[1] as u8;
            vehicle.unit_length = fields[2] as u8;
            vehicle.train_flags = (fields[3] as u16) << 4;
            let actual = advanced_effect_offset(
                &vehicle,
                VehicleAdvancedVisualEffectSpawn {
                    effect_type: 0xF1,
                    x: fields[6] as i8,
                    y: fields[7] as i8,
                    z: fields[8] as i8,
                },
                fields[4] != 0,
                fields[5] != 0,
            );
            // RemapCoords / ZOOM_BASE, Y inverted for Bevy, at Normal zoom.
            let expected = Vec3::new(
                ((fields[10] - fields[9]) * 2) as f32,
                (fields[11] - fields[9] - fields[10]) as f32,
                0.0,
            );
            assert_eq!(actual, expected, "oracle row: {line}");
            for scale in [0.25, 0.5, 1.0, 2.0, 4.0, 8.0] {
                let mut projection = OrthographicProjection {
                    scale,
                    ..OrthographicProjection::default_2d()
                };
                projection.update(1280.0, 720.0);
                let matrix = projection.get_clip_from_view();
                let projected = matrix.project_point3(actual) - matrix.project_point3(Vec3::ZERO);
                let pixels = projected.truncate() * Vec2::new(640.0, 360.0);
                assert!(
                    pixels.distance(expected.truncate() / scale) < 0.001,
                    "scale={scale}, oracle row: {line}, pixels={pixels:?}"
                );
            }
        }
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn standard_smoke_keeps_its_world_prism_and_resolved_depth_through_animation() {
        use crate::render::AtlasSprite;

        let mut state = GameState::from_map(Map::new_flat(4, 4, 0));
        let mut vehicle = running_train(ENGINE_TRAIN_KIRBY);
        vehicle.direction = 0;
        vehicle.unit_length = 4;
        state.vehicles.push(vehicle.clone());
        let frames = EffectVehicleFrames {
            steam: (0..5)
                .map(|index| AtlasSprite {
                    image: Handle::default(),
                    atlas: TextureAtlas {
                        layout: Handle::default(),
                        index,
                    },
                    size: Vec2::splat(16.0),
                })
                .collect(),
            diesel: Vec::new(),
            electric_spark: Vec::new(),
            explosion_large: Vec::new(),
            breakdown: Vec::new(),
        };
        let pose = openttdrs_core::VehiclePose::from_vehicle(&vehicle);
        let (anchor, base_z, tx, ty) = vehicle_draw_anchor_from_pose(&vehicle, &state.map, pose);
        let origin = train_smoke_world_position(
            &vehicle,
            &state.map,
            pose,
            standard_effect_world_offset(&vehicle, 8),
        );
        let mut app = App::new();
        app.insert_resource(SimWorld {
            state,
            loaded_file: false,
            ottdmap_extras: None,
        })
        .insert_resource(frames.clone())
        .insert_resource(SimHudControls {
            sound_vehicle: false,
            ..default()
        })
        .init_resource::<ClientPreferences>()
        .init_resource::<SimClock>()
        .init_resource::<TrainSmokeSpawnClock>()
        .init_resource::<TrainSmokeSortSequence>()
        .add_message::<PlayWorldSfx>()
        .add_systems(Update, (spawn_train_smoke, animate_train_smoke).chain());
        app.update();
        let mut query = app.world_mut().query::<(
            Entity,
            &Transform,
            &TrainSmokeEffect,
            &ViewportSortableParent,
        )>();
        let (entity, transform, smoke, parent) = query.single(app.world()).unwrap();
        let emission_offset = standard_effect_offset(&vehicle, 8);
        assert_eq!(smoke.origin, origin);
        assert_eq!(
            parent,
            &train_smoke_parent(origin, TrainSmokeSet::Steam, 0, 4),
            "el efecto estándar entra como un prisma EffectVehicle real"
        );
        assert_eq!(
            transform.translation.truncate(),
            (effect_overlay_pos(anchor, 0, &frames.steam_set(), base_z, (tx, ty), 0.38, 0.0)
                + emission_offset)
                .truncate(),
            "la transición a coordenadas de mundo no cambia la proyección histórica"
        );
        let sorted_depth = 9.75;
        app.world_mut()
            .get_mut::<Transform>(entity)
            .unwrap()
            .translation
            .z = sorted_depth;

        // Stop further emissions; the existing effect continues to rise and
        // change atlas frames from its original world position without
        // replacing the Z previously resolved by the global sorter.
        app.world_mut().resource_mut::<SimWorld>().state.vehicles[0].running = false;
        for (tick, frame, rise) in [(1, 0, 0_u8), (4, 0, 1), (8, 1, 1)] {
            app.world_mut().resource_mut::<SimWorld>().state.tick = GameTick::new(tick);
            app.update();
            let position = TrainSmokeWorldPosition {
                z: origin.z + f32::from(rise),
                ..origin
            };
            let expected_parent = train_smoke_parent(position, TrainSmokeSet::Steam, 0, 4);
            let transform = app.world().get::<Transform>(entity).unwrap();
            assert_eq!(
                transform.translation.truncate(),
                (effect_overlay_pos(
                    anchor,
                    frame,
                    &frames.steam_set(),
                    base_z,
                    (tx, ty),
                    0.38,
                    f32::from(rise)
                ) + emission_offset)
                    .truncate()
            );
            assert_eq!(transform.translation.z, sorted_depth);
            assert_eq!(
                app.world().get::<ViewportSortableParent>(entity),
                Some(&expected_parent),
                "la elevación de SmokeTick actualiza el prisma sin perder su identidad"
            );
        }
    }

    #[test]
    fn train_smoke_parent_enters_the_global_viewport_sorter() {
        let position = TrainSmokeWorldPosition {
            x: 32.0,
            y: 48.0,
            z: 72.0,
            source_tile: TileCoord::new(2, 3),
        };
        let parent = train_smoke_parent(position, TrainSmokeSet::Steam, 5, 8);
        let mut world = World::new();
        world.init_resource::<ViewportSortableChildDepthWindows>();
        let smoke = world
            .spawn((
                parent,
                Transform::from_translation(train_smoke_translation(
                    position,
                    0,
                    &EffectVehicleFrames {
                        steam: Vec::new(),
                        diesel: Vec::new(),
                        electric_spark: Vec::new(),
                        explosion_large: Vec::new(),
                        breakdown: Vec::new(),
                    }
                    .steam_set(),
                    parent.source_depth,
                )),
            ))
            .id();
        world.spawn((
            ViewportSortableParent {
                sprite_id: 9_998,
                bounds: ParentSpriteBounds::new(31, 48, 72, 32, 48, 72),
                insertion_key: parent.insertion_key + 1,
                source_depth: parent.source_depth + 0.000_5,
            },
            Transform::from_xyz(0.0, 0.0, parent.source_depth + 0.000_5),
        ));

        let mut schedule = Schedule::default();
        schedule.add_systems(sort_viewport_sortable_parents);
        schedule.run(&mut world);

        let sorted_depth = world
            .entity(smoke)
            .get::<Transform>()
            .expect("train smoke transform")
            .translation
            .z;
        assert!(
            sorted_depth > parent.source_depth,
            "el humo de vehículo debe recibir la profundidad resuelta por el compositor global"
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
    fn steam_density_uses_vehicle_tick_counter() {
        let map = Map::new_flat(4, 4, 0);
        let mut vehicle = running_train(ENGINE_TRAIN_KIRBY);
        let engine = vehicle.effective_engine();
        vehicle.newgrf_tick_counter = 0;
        assert_eq!(
            train_smoke_to_emit_with_engine(&map, &mut vehicle, engine, 2),
            Some(TrainSmokeSet::Steam)
        );
        vehicle.newgrf_tick_counter = 1;
        assert!(train_smoke_to_emit_with_engine(&map, &mut vehicle, engine, 2).is_none());
    }

    #[test]
    fn visual_effects_skip_unpowered_electric_train_on_normal_rail() {
        use openttdrs_core::{RailType, set_rail_type_on_tile};

        let mut map = Map::new_flat(4, 4, 0);
        let mut vehicle = running_train(openttdrs_core::engine::ENGINE_TRAIN_SH_30);
        let pos = vehicle.pos;
        map.set_kind(pos, TileKind::Rail).expect("rail kind");
        let rail = set_rail_type_on_tile(map.get(pos).expect("rail tile"), RailType::Rail);
        map.set_tile(pos, rail).expect("normal rail");
        let engine = vehicle.effective_engine();

        assert!(!vehicle_has_power_on_current_rail(&map, &vehicle, engine));
        assert!(train_smoke_to_emit_with_engine(&map, &mut vehicle, engine, 2).is_none());
        assert!(!advanced_effect_should_emit(
            &map,
            &vehicle,
            engine,
            VehicleVisualEffectKind::Electric,
            2,
        ));

        let electric =
            set_rail_type_on_tile(map.get(pos).expect("normal rail tile"), RailType::Electric);
        map.set_tile(pos, electric).expect("electric rail");
        assert!(vehicle_has_power_on_current_rail(&map, &vehicle, engine));
    }

    #[test]
    fn explicit_cb10_can_enable_standard_effect_on_a_wagon() {
        let map = Map::new_flat(4, 4, 0);
        let mut vehicle = running_train(openttdrs_core::ENGINE_WAGON_COAL);
        let mut engine = openttdrs_core::engine::engine_by_id(openttdrs_core::ENGINE_WAGON_COAL)
            .expect("vagón vanilla")
            .clone();
        engine.newgrf_grfid = 0x5649_5355;
        engine.newgrf_local_id = 0;
        engine.vehicle_callback_mask = 1;
        // CB10 type=steam; a default wagon remains disabled below.
        engine.newgrf_runtime = Some(Box::new(callback_literal(0x10)));

        assert_eq!(
            train_smoke_to_emit_with_engine(&map, &mut vehicle, &engine, 2),
            Some(TrainSmokeSet::Steam)
        );

        engine.newgrf_runtime = None;
        assert!(train_smoke_to_emit_with_engine(&map, &mut vehicle, &engine, 2).is_none());
    }
}
