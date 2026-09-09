//! Humo de industrias (`EV_CHIMNEY_SMOKE` y `EV_COPPER_MINE_SMOKE`).
//!
//! Ambos son `EffectVehicle` de OpenTTD: su sprite se ancla a coordenadas de
//! mundo y su caja es exactamente 1×1×1. Por eso se entregan al compositor
//! global de parents, no como overlays locales de la tesela emisora.

use bevy::prelude::*;
use openttdrs_core::{TILE_PIXEL_HEIGHT, TileCoord, partial_pixel_z};

use crate::bevy_app::UpdateSet;
use crate::iso::{road_vehicle_tile_anchor, wang_hash};
use crate::render::viewport_sort::ParentSpriteBounds;
use crate::render::{
    AtlasSprite, MapVisualLayer, TileRenderContext, ViewportSortableParent, WorldAssets,
    palette_animations_should_run, viewport_insertion_key, viewport_source_depth,
};
use crate::sprites::{
    CHIMNEY_SMOKE_FRAMES, CHIMNEY_SMOKE_META, COPPER_MINE_SMOKE_FRAMES, COPPER_MINE_SMOKE_META,
};
use crate::state::{ClientScreen, SimWorld};

pub(crate) struct IndustrySmokePlugin;

impl Plugin for IndustrySmokePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (animate_chimney_smoke, animate_copper_mine_smoke)
                .in_set(UpdateSet::Visuals)
                .run_if(in_state(ClientScreen::InGame))
                .run_if(palette_animations_should_run),
        );
    }
}

/// `GetIndustryGfx` de la chimenea de mina de cobre (`industry_map.h`).
pub(crate) const GFX_COPPER_MINE_CHIMNEY: u16 = 49;

/// `GetIndustryGfx` de la tesela de chimenea de la central (`industry_map.h`).
pub(crate) const GFX_POWERPLANT_CHIMNEY: u16 = 8;

/// `ChimneySmokeTick`: tras avanzar el sprite, `progress = 7` → 8 ticks/frame.
const CHIMNEY_SMOKE_TICKS_PER_FRAME: u64 = 8;

const TILE_SIZE_PX: i32 = 16;
const SMOKE_PARENT_ORDINAL: u8 = 0x80;
/// Identificadores sólo para trazas del compositor; los sprites visuales
/// siguen siendo los frames reales del humo.
const CHIMNEY_SMOKE_SORT_SPRITE_ID: u32 = 0xFFFE_0002;
const COPPER_MINE_SMOKE_SORT_SPRITE_ID: u32 = 0xFFFE_0003;
/// `SmokeTick` elimina el efecto al intentar avanzar más allá de
/// `SPR_SMOKE_4` en el tick 72. El emisor visual lo reinicia para representar
/// la siguiente emisión de la chimenea persistente.
const COPPER_SMOKE_CYCLE_TICKS: u64 = 72;

/// Frames del humo de chimenea (`chimney_smoke_{i}.png`).
#[derive(Resource)]
pub(crate) struct ChimneySmokeFrames(pub(crate) Vec<AtlasSprite>);

/// Frames del humo de mina de cobre (`mine_smoke_{i}.png`).
#[derive(Resource)]
pub(crate) struct CopperMineSmokeFrames(pub(crate) Vec<AtlasSprite>);

/// Posición de mundo de un `EffectVehicle`. Los `x/y` son coordenadas de
/// píxel OpenTTD y `z` también está expresada en píxeles de altura.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SmokeWorldPosition {
    x: i32,
    y: i32,
    z: i32,
    source_tile: TileCoord,
}

/// Penacho de la central. Los offsets NFO de cada frame modifican su ancla de
/// pantalla, pero no la caja física del `EffectVehicle`.
#[derive(Component)]
pub(crate) struct ChimneySmoke {
    position: SmokeWorldPosition,
    map_width: u32,
    phase: usize,
}

/// Máxima altura de terreno que usa `GetTileMaxPixelZ`. Muestrear las cuatro
/// esquinas conserva las pendientes medias y empinadas sin reimplementar su
/// codificación de bits.
fn tile_max_pixel_z(base_z: u8, tileh: u8) -> i32 {
    let corner_z = [(0.0, 0.0), (0.0, 15.0), (15.0, 0.0), (15.0, 15.0)]
        .into_iter()
        .map(|(x, y)| i32::from(partial_pixel_z(x, y, tileh)))
        .max()
        .unwrap_or(0);
    i32::from(base_z) * i32::from(TILE_PIXEL_HEIGHT) + corner_z
}

fn smoke_position(
    ctx: &TileRenderContext,
    local_x: i32,
    local_y: i32,
    terrain_z: i32,
    z_offset: i32,
) -> SmokeWorldPosition {
    SmokeWorldPosition {
        x: ctx
            .tx_i32()
            .saturating_mul(TILE_SIZE_PX)
            .saturating_add(local_x),
        y: ctx
            .ty_i32()
            .saturating_mul(TILE_SIZE_PX)
            .saturating_add(local_y),
        z: terrain_z.saturating_add(z_offset),
        source_tile: ctx.coord,
    }
}

/// `CreateChimneySmoke`: `(x + 15, y + 14, GetTileMaxPixelZ(tile) + 59)`.
fn chimney_smoke_position(ctx: &TileRenderContext) -> SmokeWorldPosition {
    smoke_position(
        ctx,
        15,
        14,
        tile_max_pixel_z(ctx.info.base_z, ctx.info.tileh),
        59,
    )
}

/// `CreateEffectVehicleAbove`: `(x + 6, y + 6, GetSlopePixelZ + 43)`.
fn copper_mine_smoke_position(ctx: &TileRenderContext) -> SmokeWorldPosition {
    let terrain_z = i32::from(ctx.info.base_z) * i32::from(TILE_PIXEL_HEIGHT)
        + i32::from(partial_pixel_z(6.0, 6.0, ctx.info.tileh));
    smoke_position(ctx, 6, 6, terrain_z, 43)
}

fn smoke_source_depth(position: SmokeWorldPosition, map_width: u32) -> f32 {
    let diagonal_depth = (position.source_tile.x + position.source_tile.y) as f32 * 0.01;
    let height_depth = position.z as f32 / f32::from(TILE_PIXEL_HEIGHT) * 0.0001;
    viewport_source_depth(
        diagonal_depth + height_depth + 0.001,
        u32::try_from(position.source_tile.x).unwrap_or(0),
        map_width,
    )
}

/// `EffectVehicle::UpdateDeltaXY` fija `{ {}, {1, 1, 1}, {} }`, que se
/// representa como un prisma inclusivo de un píxel en el compositor.
fn smoke_parent(
    position: SmokeWorldPosition,
    map_width: u32,
    sprite_id: u32,
) -> ViewportSortableParent {
    let source_x = u32::try_from(position.source_tile.x).unwrap_or(0);
    let source_y = u32::try_from(position.source_tile.y).unwrap_or(0);
    ViewportSortableParent {
        sprite_id,
        bounds: ParentSpriteBounds::new(
            position.x, position.y, position.z, position.x, position.y, position.z,
        ),
        insertion_key: viewport_insertion_key(source_x, source_y, SMOKE_PARENT_ORDINAL),
        source_depth: smoke_source_depth(position, map_width),
    }
}

fn smoke_translation(
    position: SmokeWorldPosition,
    frame: (f32, f32, f32, f32),
    source_depth: f32,
) -> Vec3 {
    let (w, h, xrel, yrel) = frame;
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

/// La profundidad Z del parent es salida del compositor global. Animar un
/// frame no puede restaurar su valor fuente entre dos pasadas de sort.
fn set_smoke_translation_if_changed(
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

/// Crea el penacho para una tesela de chimenea terminada.
pub(crate) fn spawn_chimney_smoke(
    commands: &mut Commands,
    assets: &WorldAssets,
    map_width: u32,
    ctx: &TileRenderContext,
) {
    let phase = wang_hash(ctx.tx, ctx.ty, 0x5740) as usize % CHIMNEY_SMOKE_FRAMES;
    let position = chimney_smoke_position(ctx);
    let parent = smoke_parent(position, map_width, CHIMNEY_SMOKE_SORT_SPRITE_ID);
    let translation = smoke_translation(position, CHIMNEY_SMOKE_META[phase], parent.source_depth);
    let color =
        crate::sprites::with_to_alpha(Color::WHITE, crate::sprites::TransparencyOption::Industries);
    commands.spawn((
        MapVisualLayer,
        ctx.map_tile_chunk(),
        ChimneySmoke {
            position,
            map_width,
            phase,
        },
        assets.chimney_smoke[phase].sprite_colored(color),
        Transform::from_translation(translation),
        parent,
    ));
}

/// Penacho de mina de cobre; el `SmokeTick` eleva su prisma cada cuatro ticks.
#[derive(Component)]
pub(crate) struct CopperMineSmoke {
    origin: SmokeWorldPosition,
    map_width: u32,
    phase: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CopperSmokeState {
    frame: usize,
    rise: u8,
}

/// Repite el ciclo efímero de `SmokeTick` para el emisor visual persistente.
/// Cada ciclo conserva `progress = 12`, avanza la Z cada cuatro ticks y deja
/// que el quinto avance de sprite reinicie el siguiente penacho.
fn copper_smoke_state(tick: u64, phase: usize) -> CopperSmokeState {
    let phase_offset = u64::try_from(phase).unwrap_or(0) * COPPER_SMOKE_CYCLE_TICKS
        / COPPER_MINE_SMOKE_FRAMES as u64;
    let age = tick.saturating_add(phase_offset) % COPPER_SMOKE_CYCLE_TICKS;
    let mut progress = 12_u8;
    let mut frame = 0_usize;
    let mut rise = 0_u8;
    for _ in 0..age {
        progress = progress.wrapping_add(1);
        if progress.is_multiple_of(4) {
            rise = rise.saturating_add(1);
        }
        if progress & 0x0F == 4 {
            frame = frame.saturating_add(1);
        }
    }
    CopperSmokeState { frame, rise }
}

/// Crea humo para tesela `GFX_COPPER_MINE_CHIMNEY` terminada.
pub(crate) fn spawn_copper_mine_smoke(
    commands: &mut Commands,
    assets: &WorldAssets,
    map_width: u32,
    ctx: &TileRenderContext,
) {
    let phase = wang_hash(ctx.tx, ctx.ty, 0xC0FF) as usize % COPPER_MINE_SMOKE_FRAMES;
    let origin = copper_mine_smoke_position(ctx);
    let state = copper_smoke_state(0, phase);
    let position = SmokeWorldPosition {
        z: origin.z.saturating_add(i32::from(state.rise)),
        ..origin
    };
    let parent = smoke_parent(position, map_width, COPPER_MINE_SMOKE_SORT_SPRITE_ID);
    let translation = smoke_translation(
        position,
        COPPER_MINE_SMOKE_META[state.frame],
        parent.source_depth,
    );
    let color =
        crate::sprites::with_to_alpha(Color::WHITE, crate::sprites::TransparencyOption::Industries);
    commands.spawn((
        MapVisualLayer,
        ctx.map_tile_chunk(),
        CopperMineSmoke {
            origin,
            map_width,
            phase,
        },
        assets.copper_mine_smoke[state.frame].sprite_colored(color),
        Transform::from_translation(translation),
        parent,
    ));
}

/// Frame del penacho según tick de juego y fase (`ChimneySmokeTick`).
#[must_use]
pub(crate) fn smoke_frame_index(tick: u64, phase: usize) -> usize {
    ((tick / CHIMNEY_SMOKE_TICKS_PER_FRAME) as usize + phase) % CHIMNEY_SMOKE_FRAMES
}

pub(crate) fn animate_chimney_smoke(
    sim: Res<SimWorld>,
    frames: Option<Res<ChimneySmokeFrames>>,
    mut q: Query<(
        &ChimneySmoke,
        &mut Sprite,
        &mut Transform,
        Option<&ViewportSortableParent>,
    )>,
) {
    let Some(frames) = frames else {
        return;
    };
    let tick = sim.state.tick.get();
    for (smoke, mut sprite, mut transform, parent) in &mut q {
        let idx = smoke_frame_index(tick, smoke.phase);
        if !frames.0[idx].matches(&sprite) {
            frames.0[idx].apply_to(&mut sprite);
        }
        let source_depth = parent.map_or_else(
            || smoke_source_depth(smoke.position, smoke.map_width),
            |parent| parent.source_depth,
        );
        let translation = smoke_translation(smoke.position, CHIMNEY_SMOKE_META[idx], source_depth);
        set_smoke_translation_if_changed(&mut transform, translation, parent.is_some());
    }
}

pub(crate) fn animate_copper_mine_smoke(
    sim: Res<SimWorld>,
    frames: Option<Res<CopperMineSmokeFrames>>,
    mut q: Query<(
        &CopperMineSmoke,
        &mut Sprite,
        &mut Transform,
        Option<&mut ViewportSortableParent>,
    )>,
) {
    let Some(frames) = frames else {
        return;
    };
    let tick = sim.state.tick.get();
    for (smoke, mut sprite, mut transform, parent) in &mut q {
        let state = copper_smoke_state(tick, smoke.phase);
        if !frames.0[state.frame].matches(&sprite) {
            frames.0[state.frame].apply_to(&mut sprite);
        }
        let position = SmokeWorldPosition {
            z: smoke.origin.z.saturating_add(i32::from(state.rise)),
            ..smoke.origin
        };
        let next_parent = smoke_parent(position, smoke.map_width, COPPER_MINE_SMOKE_SORT_SPRITE_ID);
        let translation = smoke_translation(
            position,
            COPPER_MINE_SMOKE_META[state.frame],
            next_parent.source_depth,
        );
        let preserves_sorted_depth = parent.is_some();
        set_smoke_translation_if_changed(&mut transform, translation, preserves_sorted_depth);
        if let Some(mut parent) = parent
            && *parent != next_parent
        {
            *parent = next_parent;
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use bevy::ecs::system::RunSystemOnce;
    use openttdrs_core::{GameState, GameTick, SLOPE_NE, TileKind};

    use super::*;
    use crate::render::grid::TileRenderInfo;
    use crate::render::{ViewportSortableChildDepthWindows, sort_viewport_sortable_parents};

    fn smoke_ctx(tx: u32, ty: u32, base_z: u8, tileh: u8) -> TileRenderContext {
        TileRenderContext {
            tx,
            ty,
            coord: TileCoord::new(tx as i32, ty as i32),
            tile: None,
            object_type: None,
            kind: TileKind::Industry,
            info: TileRenderInfo {
                tileh,
                base_z,
                use_shore: false,
            },
            iso_pos: Vec2::ZERO,
        }
    }

    fn weak_sprite(n: u128) -> AtlasSprite {
        AtlasSprite {
            image: Handle::Uuid(
                bevy::asset::uuid::Uuid::from_u128(1),
                std::marker::PhantomData,
            ),
            atlas: TextureAtlas {
                layout: Handle::Uuid(
                    bevy::asset::uuid::Uuid::from_u128(2),
                    std::marker::PhantomData,
                ),
                index: n as usize,
            },
            size: Vec2::ONE,
        }
    }

    fn sim_at_tick(tick: u64) -> SimWorld {
        let mut state = GameState::new(1, 1);
        state.tick = GameTick::new(tick);
        SimWorld {
            state,
            loaded_file: false,
            ottdmap_extras: None,
        }
    }

    #[test]
    fn frame_index_cycles_with_phase() {
        assert_eq!(smoke_frame_index(0, 0), 0);
        assert_eq!(smoke_frame_index(0, 3), 3);
        assert_eq!(smoke_frame_index(8, 0), 1);
        assert_eq!(smoke_frame_index(7, 0), 0);
        assert_eq!(smoke_frame_index(8 * 8, 0), 0);
    }

    #[test]
    fn copper_smoke_replays_the_rise_and_restart_of_smoke_tick() {
        assert_eq!(copper_smoke_state(0, 0).frame, 0);
        assert_eq!(
            copper_smoke_state(4, 0),
            CopperSmokeState { frame: 0, rise: 1 }
        );
        assert_eq!(
            copper_smoke_state(8, 0),
            CopperSmokeState { frame: 1, rise: 2 }
        );
        assert_eq!(
            copper_smoke_state(56, 0),
            CopperSmokeState { frame: 4, rise: 14 }
        );
        assert_eq!(
            copper_smoke_state(COPPER_SMOKE_CYCLE_TICKS, 0),
            CopperSmokeState { frame: 0, rise: 0 },
            "el siguiente penacho comienza después del Delete() de SmokeTick"
        );
    }

    #[test]
    fn industrial_smoke_uses_the_real_effect_vehicle_positions_on_slopes() {
        let ctx = smoke_ctx(2, 3, 4, SLOPE_NE);
        let chimney = chimney_smoke_position(&ctx);
        assert_eq!(
            chimney,
            SmokeWorldPosition {
                x: 47,
                y: 62,
                z: 99,
                source_tile: TileCoord::new(2, 3),
            },
            "CreateChimneySmoke usa GetTileMaxPixelZ, no la altura mínima"
        );
        let copper = copper_mine_smoke_position(&ctx);
        assert_eq!(copper.x, 38);
        assert_eq!(copper.y, 54);
        assert_eq!(
            copper.z,
            32 + i32::from(partial_pixel_z(6.0, 6.0, SLOPE_NE)) + 43,
            "CreateEffectVehicleAbove toma GetSlopePixelZ en el punto (6,6)"
        );
        let parent = smoke_parent(chimney, 8, CHIMNEY_SMOKE_SORT_SPRITE_ID);
        assert_eq!(
            parent.bounds,
            ParentSpriteBounds::new(47, 62, 99, 47, 62, 99),
            "EffectVehicle::UpdateDeltaXY conserva un prisma inclusivo 1×1×1"
        );
        assert_eq!(
            parent.insertion_key,
            viewport_insertion_key(2, 3, SMOKE_PARENT_ORDINAL)
        );
    }

    #[test]
    fn smoke_parent_enters_the_global_viewport_sorter() {
        let position = SmokeWorldPosition {
            x: 32,
            y: 48,
            z: 72,
            source_tile: TileCoord::new(2, 3),
        };
        let parent = smoke_parent(position, 8, CHIMNEY_SMOKE_SORT_SPRITE_ID);
        let mut world = World::new();
        world.init_resource::<ViewportSortableChildDepthWindows>();
        let smoke = world
            .spawn((
                parent,
                Transform::from_translation(smoke_translation(
                    position,
                    CHIMNEY_SMOKE_META[0],
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
            .expect("smoke transform")
            .translation
            .z;
        assert!(
            sorted_depth > parent.source_depth,
            "el humo debe recibir la profundidad resuelta por el compositor global"
        );
    }

    #[test]
    fn chimney_animation_keeps_the_depth_resolved_by_the_sorter() {
        let mut world = World::new();
        world.insert_resource(sim_at_tick(20)); // frame 2 con phase 0
        world.insert_resource(ChimneySmokeFrames(
            (0..CHIMNEY_SMOKE_FRAMES as u128).map(weak_sprite).collect(),
        ));
        let position = SmokeWorldPosition {
            x: 16,
            y: 16,
            z: 59,
            source_tile: TileCoord::new(1, 1),
        };
        let parent = smoke_parent(position, 4, CHIMNEY_SMOKE_SORT_SPRITE_ID);
        let sorted_depth = parent.source_depth + 0.02;
        let e = world
            .spawn((
                ChimneySmoke {
                    position,
                    map_width: 4,
                    phase: 0,
                },
                Sprite::default(),
                Transform::from_xyz(0.0, 0.0, sorted_depth),
                parent,
            ))
            .id();

        world.run_system_once(animate_chimney_smoke).unwrap();

        let expected = smoke_frame_index(20, 0);
        assert!(weak_sprite(expected as u128).matches(world.get::<Sprite>(e).unwrap()));
        assert_ne!(world.get::<Transform>(e).unwrap().translation, Vec3::ZERO);
        assert_eq!(
            world.get::<Transform>(e).unwrap().translation.z,
            sorted_depth,
            "el frame nuevo no debe devolver el efecto a source_depth"
        );
    }
}
