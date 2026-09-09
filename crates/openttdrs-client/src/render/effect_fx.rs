//! FX efímeros de vehículos y terreno (`EV_BREAKDOWN_SMOKE`, `EV_EXPLOSION_LARGE`).
//!
//! OpenTTD los representa como `EffectVehicle`: conservan una coordenada de
//! mundo y una caja física de 1×1×1 que participa en `ViewportSortParentSprites`.
//! No son overlays de una tesela; por eso este módulo resuelve la geometría al
//! crear el evento y entrega cada efecto al compositor global.

use bevy::prelude::*;

use openttdrs_core::{
    Map, TileCoord, Vehicle, extrapolate_vehicle_pose, slope_dz_at_subtile,
    vehicle_subtile_at_with_map,
};

use crate::bevy_app::UpdateSet;
use crate::iso::{road_vehicle_tile_anchor, tile_slope_and_min_z};
use crate::render::effect_vehicle::{
    EffectSpriteSet, EffectVehicleFrames, apply_effect_frame, effect_frame_count,
};
use crate::render::viewport_sort::ParentSpriteBounds;
use crate::render::{
    MapVisualLayer, ViewportSortableParent, vehicles::vehicle_draw_anchor_from_pose,
    viewport_insertion_key, viewport_source_depth,
};
use crate::simulation::SimClock;
use crate::state::{ClientScreen, SimWorld};

const TILE_SIZE_PX: i32 = 16;
const FX_PARENT_ORDINAL_BASE: u8 = 0x80;
const FX_SORT_SPRITE_ID_BASE: u32 = 0xFFFE_0020;
/// `ExplosionLargeTick` avanza el sprite una vez cada cuatro ticks.
const EXPLOSION_TICKS_PER_FRAME: u64 = 4;
/// `BreakdownSmokeTick` avanza/reinicia el sprite una vez cada ocho ticks.
const BREAKDOWN_TICKS_PER_FRAME: u64 = 8;
/// La avería nativa empieza con `breakdown_delay * 2`; una fallback de la
/// duración mínima evita una partícula eterna si el vehículo ya fue eliminado
/// cuando el cliente drena el evento.
const FALLBACK_BREAKDOWN_TICKS: u64 = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FxSpawnKind {
    BreakdownSmoke,
    Explosion,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FxSpawnOrigin {
    /// `CreateEffectVehicleAbove(tile * 16 + 8, ..., 2, EV_EXPLOSION_LARGE)`.
    AboveTile(TileCoord),
    /// `CreateEffectVehicleRel(vehicle, 4, 4, z, ...)`; `fallback` conserva
    /// el punto de sonido/evento si el vehículo dejó de existir antes del pase
    /// visual.
    RelativeVehicle {
        vehicle_id: u32,
        fallback: TileCoord,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FxSpawnRequest {
    kind: FxSpawnKind,
    origin: FxSpawnOrigin,
}

/// Cola de FX a crear tras procesar [`PendingSimEvents`](crate::audio::PendingSimEvents).
#[derive(Resource, Default)]
pub(crate) struct FxSpawnQueue(Vec<FxSpawnRequest>);

impl FxSpawnQueue {
    pub(crate) fn push_breakdown(&mut self, vehicle_id: u32, at: TileCoord) {
        self.0.push(FxSpawnRequest {
            kind: FxSpawnKind::BreakdownSmoke,
            origin: FxSpawnOrigin::RelativeVehicle {
                vehicle_id,
                fallback: at,
            },
        });
    }

    /// Explosión de terreno: el producer sólo conoce la tesela, igual que la
    /// orden de demolición que llama a `CreateEffectVehicleAbove` en upstream.
    pub(crate) fn push_explosion(&mut self, at: TileCoord) {
        self.0.push(FxSpawnRequest {
            kind: FxSpawnKind::Explosion,
            origin: FxSpawnOrigin::AboveTile(at),
        });
    }

    /// Explosión vinculada a un vehículo (choque/inundación/avión): OpenTTD
    /// usa `CreateEffectVehicleRel(v, 4, 4, 8, EV_EXPLOSION_LARGE)`.
    pub(crate) fn push_vehicle_explosion(&mut self, vehicle_id: u32, at: TileCoord) {
        self.0.push(FxSpawnRequest {
            kind: FxSpawnKind::Explosion,
            origin: FxSpawnOrigin::RelativeVehicle {
                vehicle_id,
                fallback: at,
            },
        });
    }
}

/// Desempate estable entre `EffectVehicle`s creados durante el mismo pase
/// visual. Los efectos no se guardan y el contador se reinicia con la sesión.
#[derive(Resource, Default)]
struct FxSortSequence(u8);

pub(crate) struct EffectVehiclePlugin;

impl Plugin for EffectVehiclePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FxSpawnQueue>()
            .init_resource::<FxSortSequence>()
            .add_systems(
                Update,
                (spawn_queued_fx, animate_ephemeral_fx)
                    .chain()
                    .in_set(UpdateSet::Visuals)
                    .run_if(in_state(ClientScreen::InGame)),
            );
    }
}

/// Coordenadas de mundo de un `EffectVehicle`. X/Y/Z están en píxeles nativos
/// de OpenTTD; `source_tile` sólo se limita al mapa para culling y orden de
/// inserción, nunca recorta la posición física del efecto.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FxWorldPosition {
    x: i32,
    y: i32,
    z: i32,
    source_tile: TileCoord,
}

#[derive(Component)]
struct EphemeralFx {
    kind: FxSpawnKind,
    started_tick: u64,
    lifetime_ticks: u64,
    position: FxWorldPosition,
    sort_ordinal: u8,
}

fn sprite_set<'a>(frames: &'a EffectVehicleFrames, kind: FxSpawnKind) -> EffectSpriteSet<'a> {
    match kind {
        FxSpawnKind::BreakdownSmoke => frames.breakdown_set(),
        FxSpawnKind::Explosion => frames.explosion_set(),
    }
}

/// Estado discreto de `ExplosionLargeTick` y `BreakdownSmokeTick`.
///
/// `None` corresponde a `Delete()` en el tick nativo. El humo de avería no
/// sube: `BreakdownSmokeTick` sólo cambia/reinicia el sprite y decrementa su
/// contador de animación.
#[must_use]
fn ephemeral_fx_frame(
    kind: FxSpawnKind,
    age_ticks: u64,
    lifetime_ticks: u64,
    frame_count: usize,
) -> Option<usize> {
    if frame_count == 0 || age_ticks >= lifetime_ticks {
        return None;
    }
    match kind {
        FxSpawnKind::Explosion => {
            let frame = usize::try_from(age_ticks / EXPLOSION_TICKS_PER_FRAME).ok()?;
            (frame < frame_count).then_some(frame)
        }
        FxSpawnKind::BreakdownSmoke => {
            let frame = usize::try_from(age_ticks / BREAKDOWN_TICKS_PER_FRAME).ok()?;
            Some(frame % frame_count)
        }
    }
}

/// Coordenada segura para la consulta de terreno de `CreateEffectVehicleAbove`.
/// El constructor C++ conserva X/Y sin recortar, pero toma Z del borde del
/// mapa si el emisor quedó apenas fuera de él.
fn fx_ground_sample(map: &Map, x: i32, y: i32) -> (TileCoord, i32, i32) {
    let (width, height) = map.dimensions();
    let max_x = i32::try_from(width.saturating_sub(1))
        .unwrap_or(i32::MAX)
        .saturating_mul(TILE_SIZE_PX);
    let max_y = i32::try_from(height.saturating_sub(1))
        .unwrap_or(i32::MAX)
        .saturating_mul(TILE_SIZE_PX);
    let safe_x = x.clamp(0, max_x);
    let safe_y = y.clamp(0, max_y);
    (
        TileCoord::new(
            safe_x.div_euclid(TILE_SIZE_PX),
            safe_y.div_euclid(TILE_SIZE_PX),
        ),
        safe_x.rem_euclid(TILE_SIZE_PX),
        safe_y.rem_euclid(TILE_SIZE_PX),
    )
}

/// `CreateEffectVehicleAbove` para una explosión de terreno en el centro de
/// su tesela: `(tile * 16 + 8, tile * 16 + 8, GetSlopePixelZ + 2)`.
fn tile_fx_world_position(map: &Map, at: TileCoord) -> FxWorldPosition {
    let x =
        at.x.saturating_mul(TILE_SIZE_PX)
            .saturating_add(TILE_SIZE_PX / 2);
    let y =
        at.y.saturating_mul(TILE_SIZE_PX)
            .saturating_add(TILE_SIZE_PX / 2);
    let (source_tile, sub_x, sub_y) = fx_ground_sample(map, x, y);
    let (tileh, base_z) = tile_slope_and_min_z(
        map,
        u32::try_from(source_tile.x).unwrap_or(0),
        u32::try_from(source_tile.y).unwrap_or(0),
    );
    let ground_z = i32::from(base_z) * i32::from(openttdrs_core::TILE_PIXEL_HEIGHT)
        + slope_dz_at_subtile(sub_x as f32, sub_y as f32, tileh).round() as i32;
    FxWorldPosition {
        x,
        y,
        z: ground_z.saturating_add(2),
        source_tile,
    }
}

#[must_use]
const fn vehicle_effect_offset(kind: FxSpawnKind) -> IVec3 {
    match kind {
        FxSpawnKind::BreakdownSmoke => IVec3::new(4, 4, 5),
        FxSpawnKind::Explosion => IVec3::new(4, 4, 8),
    }
}

/// Convierte la ancla visual actual del vehículo a la coordenada que usa
/// `CreateEffectVehicleRel`. Conserva la geometría de pendientes y puentes de
/// `vehicle_draw_anchor_from_pose`, a la vez que recupera el prisma entero
/// requerido por el sorter global.
fn vehicle_fx_world_position(
    vehicle: &Vehicle,
    map: &Map,
    pose: openttdrs_core::VehiclePose,
    offset: IVec3,
) -> FxWorldPosition {
    let (anchor, base_z, tx, ty) = vehicle_draw_anchor_from_pose(vehicle, map, pose);
    let (sub_x, sub_y) = vehicle_subtile_at_with_map(vehicle, pose, Some(map));
    let (tileh, _) = tile_slope_and_min_z(
        map,
        u32::try_from(tx).unwrap_or(0),
        u32::try_from(ty).unwrap_or(0),
    );
    let height_px = f32::from(openttdrs_core::TILE_PIXEL_HEIGHT);
    let terrain_z = f32::from(base_z) * height_px + slope_dz_at_subtile(sub_x, sub_y, tileh);
    // `road_vehicle_tile_anchor(0, 0, x, y, z)` proyecta
    // `(2 * (y - x), -(x + y - z))`. Invertirla conserva la ancla que ya
    // resolvió el renderer de vehículos antes de sumar el offset relativo.
    let projected_y = anchor.y + f32::from(base_z) * height_px;
    let world_sum = terrain_z - projected_y;
    let world_delta = anchor.x * 0.5;
    let x = ((world_sum - world_delta) * 0.5 + offset.x as f32).round() as i32;
    let y = ((world_sum + world_delta) * 0.5 + offset.y as f32).round() as i32;
    let z = (terrain_z + offset.z as f32).round() as i32;
    let (source_tile, _, _) = fx_ground_sample(map, x, y);
    FxWorldPosition {
        x,
        y,
        z,
        source_tile,
    }
}

fn resolve_fx_position(
    request: FxSpawnRequest,
    sim: &SimWorld,
    sim_clock: &SimClock,
) -> (FxWorldPosition, Option<u8>) {
    match request.origin {
        FxSpawnOrigin::AboveTile(at) => (tile_fx_world_position(&sim.state.map, at), None),
        FxSpawnOrigin::RelativeVehicle {
            vehicle_id,
            fallback,
        } => {
            let Some(vehicle) = sim
                .state
                .vehicles
                .iter()
                .find(|vehicle| vehicle.id == vehicle_id)
            else {
                return (tile_fx_world_position(&sim.state.map, fallback), None);
            };
            let pose = extrapolate_vehicle_pose(vehicle, sim_clock.tick_alpha);
            (
                vehicle_fx_world_position(
                    vehicle,
                    &sim.state.map,
                    pose,
                    vehicle_effect_offset(request.kind),
                ),
                Some(vehicle.breakdown_delay),
            )
        }
    }
}

fn fx_lifetime_ticks(kind: FxSpawnKind, frame_count: usize, breakdown_delay: Option<u8>) -> u64 {
    match kind {
        FxSpawnKind::Explosion => u64::try_from(frame_count)
            .unwrap_or(u64::MAX)
            .saturating_mul(EXPLOSION_TICKS_PER_FRAME),
        FxSpawnKind::BreakdownSmoke => breakdown_delay
            .map(|delay| u64::from(delay).saturating_mul(2))
            .filter(|&duration| duration > 0)
            .unwrap_or(FALLBACK_BREAKDOWN_TICKS),
    }
}

fn fx_source_depth(position: FxWorldPosition, map_width: u32) -> f32 {
    let diagonal_depth = (position.source_tile.x + position.source_tile.y) as f32 * 0.01;
    let height_depth = position.z as f32 / f32::from(openttdrs_core::TILE_PIXEL_HEIGHT) * 0.0001;
    viewport_source_depth(
        diagonal_depth + height_depth + 0.001,
        u32::try_from(position.source_tile.x).unwrap_or(0),
        map_width,
    )
}

fn fx_parent(effect: &EphemeralFx, map_width: u32) -> ViewportSortableParent {
    let position = effect.position;
    let source_x = u32::try_from(position.source_tile.x).unwrap_or(0);
    let source_y = u32::try_from(position.source_tile.y).unwrap_or(0);
    ViewportSortableParent {
        sprite_id: FX_SORT_SPRITE_ID_BASE
            + match effect.kind {
                FxSpawnKind::BreakdownSmoke => 0,
                FxSpawnKind::Explosion => 1,
            },
        // `EffectVehicle::UpdateDeltaXY`: `{ {}, {1, 1, 1}, {} }`.
        bounds: ParentSpriteBounds::new(
            position.x, position.y, position.z, position.x, position.y, position.z,
        ),
        insertion_key: viewport_insertion_key(
            source_x,
            source_y,
            FX_PARENT_ORDINAL_BASE | (effect.sort_ordinal & 0x7F),
        ),
        source_depth: fx_source_depth(position, map_width),
    }
}

fn fx_translation(
    position: FxWorldPosition,
    frame: usize,
    set: &EffectSpriteSet<'_>,
    source_depth: f32,
) -> Vec3 {
    let idx = frame.min(set.meta.len().saturating_sub(1));
    let (w, h, xrel, yrel) = set.meta[idx];
    let anchor = road_vehicle_tile_anchor(
        0,
        0,
        position.x as f32,
        position.y as f32,
        position.z as f32,
    );
    Vec3::new(
        anchor.x + xrel + w * 0.5,
        anchor.y - (yrel + h * 0.5),
        source_depth,
    )
}

/// El compositor escribe la profundidad efectiva. Cambiar el frame no puede
/// restaurar el slot fuente antes del siguiente pase global.
fn set_fx_translation_if_changed(
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

fn spawn_queued_fx(
    mut queue: ResMut<FxSpawnQueue>,
    mut sort_sequence: ResMut<FxSortSequence>,
    sim: Res<SimWorld>,
    sim_clock: Res<SimClock>,
    frames: Res<EffectVehicleFrames>,
    mut commands: Commands,
) {
    if !frames.is_loaded() {
        queue.0.clear();
        return;
    }
    let tick = sim.state.tick.get();
    let map_width = sim.state.map.dimensions().0;
    for request in queue.0.drain(..) {
        let effect_set = sprite_set(&frames, request.kind);
        if effect_set.frames.is_empty() || effect_set.meta.is_empty() {
            continue;
        }
        let frame_count = effect_frame_count(&effect_set);
        let (position, breakdown_delay) = resolve_fx_position(request, &sim, &sim_clock);
        let lifetime_ticks = fx_lifetime_ticks(request.kind, frame_count, breakdown_delay);
        let Some(frame) = ephemeral_fx_frame(request.kind, 0, lifetime_ticks, frame_count) else {
            continue;
        };
        let Some(atlas) = effect_set.frames.get(frame) else {
            continue;
        };
        let effect = EphemeralFx {
            kind: request.kind,
            started_tick: tick,
            lifetime_ticks,
            position,
            sort_ordinal: sort_sequence.0,
        };
        sort_sequence.0 = sort_sequence.0.wrapping_add(1);
        let parent = fx_parent(&effect, map_width);
        let mut sprite = atlas.sprite();
        if matches!(effect.kind, FxSpawnKind::Explosion) {
            sprite.color = Color::WHITE;
        }
        commands.spawn((
            MapVisualLayer,
            effect,
            sprite,
            Transform::from_translation(fx_translation(
                position,
                frame,
                &effect_set,
                parent.source_depth,
            )),
            Visibility::Visible,
            parent,
        ));
    }
}

fn animate_ephemeral_fx(
    sim: Res<SimWorld>,
    frames: Res<EffectVehicleFrames>,
    mut q: Query<(
        Entity,
        &mut Transform,
        &EphemeralFx,
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
    for (entity, mut transform, effect, mut sprite, parent) in &mut q {
        let effect_set = sprite_set(&frames, effect.kind);
        let Some(frame) = ephemeral_fx_frame(
            effect.kind,
            tick.saturating_sub(effect.started_tick),
            effect.lifetime_ticks,
            effect_frame_count(&effect_set),
        ) else {
            commands.entity(entity).despawn();
            continue;
        };
        if effect_set
            .frames
            .get(frame)
            .is_some_and(|atlas| !atlas.matches(&sprite))
        {
            apply_effect_frame(&mut sprite, &effect_set, frame);
        }
        if matches!(effect.kind, FxSpawnKind::Explosion) {
            sprite.color = Color::WHITE;
        }
        let next_parent = fx_parent(effect, map_width);
        let translation = fx_translation(
            effect.position,
            frame,
            &effect_set,
            next_parent.source_depth,
        );
        let preserves_sorted_depth = parent.is_some();
        set_fx_translation_if_changed(&mut transform, translation, preserves_sorted_depth);
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
    use bevy::ecs::system::RunSystemOnce;
    use openttdrs_core::{GameState, GameTick, Map, TileCoord};

    use super::*;
    use crate::render::{ViewportSortableChildDepthWindows, sort_viewport_sortable_parents};

    fn effect(kind: FxSpawnKind, position: FxWorldPosition, sort_ordinal: u8) -> EphemeralFx {
        EphemeralFx {
            kind,
            started_tick: 0,
            lifetime_ticks: 64,
            position,
            sort_ordinal,
        }
    }

    fn weak_sprite(index: usize) -> crate::render::AtlasSprite {
        crate::render::AtlasSprite {
            image: Handle::default(),
            atlas: TextureAtlas {
                layout: Handle::default(),
                index,
            },
            size: Vec2::ONE,
        }
    }

    fn loaded_effect_frames() -> EffectVehicleFrames {
        EffectVehicleFrames {
            // `is_loaded` usa steam como centinela; la explosión es el único
            // conjunto que consume esta prueba de despacho.
            steam: vec![weak_sprite(0)],
            diesel: Vec::new(),
            electric_spark: Vec::new(),
            explosion_large: vec![weak_sprite(1)],
            breakdown: Vec::new(),
        }
    }

    #[test]
    fn effects_follow_native_tick_cadence_without_screen_space_rise() {
        assert_eq!(
            ephemeral_fx_frame(FxSpawnKind::Explosion, 0, 64, 16),
            Some(0)
        );
        assert_eq!(
            ephemeral_fx_frame(FxSpawnKind::Explosion, 3, 64, 16),
            Some(0)
        );
        assert_eq!(
            ephemeral_fx_frame(FxSpawnKind::Explosion, 4, 64, 16),
            Some(1)
        );
        assert_eq!(
            ephemeral_fx_frame(FxSpawnKind::Explosion, 60, 64, 16),
            Some(15)
        );
        assert_eq!(ephemeral_fx_frame(FxSpawnKind::Explosion, 64, 64, 16), None);

        assert_eq!(
            ephemeral_fx_frame(FxSpawnKind::BreakdownSmoke, 0, 33, 4),
            Some(0)
        );
        assert_eq!(
            ephemeral_fx_frame(FxSpawnKind::BreakdownSmoke, 7, 33, 4),
            Some(0)
        );
        assert_eq!(
            ephemeral_fx_frame(FxSpawnKind::BreakdownSmoke, 8, 33, 4),
            Some(1)
        );
        assert_eq!(
            ephemeral_fx_frame(FxSpawnKind::BreakdownSmoke, 31, 33, 4),
            Some(3)
        );
        assert_eq!(
            ephemeral_fx_frame(FxSpawnKind::BreakdownSmoke, 32, 33, 4),
            Some(0)
        );
        assert_eq!(
            ephemeral_fx_frame(FxSpawnKind::BreakdownSmoke, 33, 33, 4),
            None
        );
    }

    #[test]
    fn tile_explosion_uses_effect_vehicle_above_geometry_and_prism() {
        let map = Map::new_flat(4, 4, 2);
        let position = tile_fx_world_position(&map, TileCoord::new(1, 2));
        assert_eq!(
            position,
            FxWorldPosition {
                x: 24,
                y: 40,
                z: 18,
                source_tile: TileCoord::new(1, 2),
            }
        );
        let effect = effect(FxSpawnKind::Explosion, position, 7);
        let parent = fx_parent(&effect, map.dimensions().0);
        assert_eq!(
            parent.bounds,
            ParentSpriteBounds::new(24, 40, 18, 24, 40, 18),
            "EffectVehicle::UpdateDeltaXY usa exactamente un prisma 1×1×1"
        );
        assert_eq!(
            parent.insertion_key,
            viewport_insertion_key(1, 2, FX_PARENT_ORDINAL_BASE | 7)
        );
    }

    #[test]
    fn vehicle_effects_keep_the_native_relative_offsets() {
        use openttdrs_core::{Vehicle, VehicleKind};

        let map = Map::new_flat(4, 4, 0);
        let vehicle = Vehicle::new(
            17,
            VehicleKind::Bus,
            TileCoord::new(1, 1),
            TileCoord::new(2, 1),
        );
        let pose = extrapolate_vehicle_pose(&vehicle, 0.0);
        let breakdown = vehicle_fx_world_position(
            &vehicle,
            &map,
            pose,
            vehicle_effect_offset(FxSpawnKind::BreakdownSmoke),
        );
        let explosion = vehicle_fx_world_position(
            &vehicle,
            &map,
            pose,
            vehicle_effect_offset(FxSpawnKind::Explosion),
        );

        assert_eq!(
            vehicle_effect_offset(FxSpawnKind::BreakdownSmoke),
            IVec3::new(4, 4, 5)
        );
        assert_eq!(
            vehicle_effect_offset(FxSpawnKind::Explosion),
            IVec3::new(4, 4, 8)
        );
        assert_eq!(explosion.x, breakdown.x);
        assert_eq!(explosion.y, breakdown.y);
        assert_eq!(explosion.z, breakdown.z + 3);
        assert_eq!(explosion.source_tile, breakdown.source_tile);
    }

    #[test]
    fn queued_tile_explosion_spawns_as_a_sortable_effect_vehicle() {
        let mut state = GameState::new(4, 4);
        state.map = Map::new_flat(4, 4, 0);
        state.tick = GameTick::new(37);
        let mut world = World::new();
        world.insert_resource(SimWorld {
            state,
            loaded_file: false,
            ottdmap_extras: None,
        });
        world.init_resource::<SimClock>();
        world.init_resource::<FxSpawnQueue>();
        world.init_resource::<FxSortSequence>();
        world.insert_resource(loaded_effect_frames());
        world
            .resource_mut::<FxSpawnQueue>()
            .push_explosion(TileCoord::new(1, 2));

        world.run_system_once(spawn_queued_fx).unwrap();

        let mut effects = world.query::<(
            &MapVisualLayer,
            &EphemeralFx,
            &Transform,
            &ViewportSortableParent,
        )>();
        let (_layer, effect, transform, parent) = effects.single(&world).unwrap();
        assert_eq!(effect.started_tick, 37);
        assert_eq!(
            effect.position,
            tile_fx_world_position(
                &world.resource::<SimWorld>().state.map,
                TileCoord::new(1, 2)
            )
        );
        assert_eq!(
            parent.bounds,
            ParentSpriteBounds::new(24, 40, 2, 24, 40, 2),
            "la cola visual despacha un EffectVehicle con prisma físico, no un overlay local"
        );
        assert_eq!(transform.translation.z, parent.source_depth);
    }

    #[test]
    fn ephemeral_fx_parent_enters_the_global_viewport_sorter() {
        let position = FxWorldPosition {
            x: 32,
            y: 48,
            z: 72,
            source_tile: TileCoord::new(2, 3),
        };
        let effect = effect(FxSpawnKind::Explosion, position, 5);
        let parent = fx_parent(&effect, 8);
        let empty_frames = EffectVehicleFrames {
            steam: Vec::new(),
            diesel: Vec::new(),
            electric_spark: Vec::new(),
            explosion_large: Vec::new(),
            breakdown: Vec::new(),
        };
        let mut world = World::new();
        world.init_resource::<ViewportSortableChildDepthWindows>();
        let fx = world
            .spawn((
                parent,
                Transform::from_translation(fx_translation(
                    position,
                    0,
                    &empty_frames.explosion_set(),
                    parent.source_depth,
                )),
            ))
            .id();
        world.spawn((
            ViewportSortableParent {
                sprite_id: 9_997,
                bounds: ParentSpriteBounds::new(31, 48, 72, 32, 48, 72),
                insertion_key: parent.insertion_key + 1,
                source_depth: parent.source_depth + 0.000_5,
            },
            Transform::from_xyz(0.0, 0.0, parent.source_depth + 0.000_5),
        ));

        let mut schedule = Schedule::default();
        schedule.add_systems(sort_viewport_sortable_parents);
        schedule.run(&mut world);

        assert!(
            world
                .entity(fx)
                .get::<Transform>()
                .expect("effect transform")
                .translation
                .z
                > parent.source_depth,
            "el EffectVehicle debe recibir la profundidad resuelta por el compositor global"
        );
    }
}
