//! Humo de la chimenea de la central eléctrica, fiel al `EffectVehicle`
//! `EV_CHIMNEY_SMOKE` de OpenTTD (`industry_cmd.cpp` + `effectvehicle.cpp`):
//! se ancla en la tesela `GFX_POWERPLANT_CHIMNEY` en el punto de mundo
//! `(+15, +14, z+59)` y cicla `SPR_CHIMNEY_SMOKE_0..7` (un frame cada
//! 8 ticks de juego, con fase inicial aleatoria).

use bevy::prelude::*;

use crate::bevy_app::UpdateSet;
use crate::iso::{overlay_pos, remap_tile_offset, wang_hash};
use crate::render::{AtlasSprite, MapVisualLayer, TileRenderContext, WorldAssets};
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
                .run_if(in_state(ClientScreen::InGame)),
        );
    }
}

/// `GetIndustryGfx` de la chimenea de mina de cobre (`industry_map.h`).
pub(crate) const GFX_COPPER_MINE_CHIMNEY: u16 = 49;

/// `GetIndustryGfx` de la tesela de chimenea de la central (`industry_map.h`).
pub(crate) const GFX_POWERPLANT_CHIMNEY: u16 = 8;

/// `ChimneySmokeTick`: tras avanzar el sprite, `progress = 7` → 8 ticks/frame.
const CHIMNEY_SMOKE_TICKS_PER_FRAME: u64 = 8;

/// Humo mina cobre: sprite cada ~16 ticks (`SmokeTick`, `progress & 0xF == 4`).
const COPPER_SMOKE_TICKS_PER_FRAME: u64 = 16;

/// Ascenso por frame (~4 ticks entre pasos de `z_pos`).
const COPPER_SMOKE_RISE: f32 = 1.5;

/// Capa por encima del edificio de la industria (overlays usan 0.4/0.5).
const SMOKE_LAYER_FRAC: f32 = 0.55;

/// Frames del humo de chimenea (`chimney_smoke_{i}.png`).
#[derive(Resource)]
pub(crate) struct ChimneySmokeFrames(pub(crate) Vec<AtlasSprite>);

/// Frames del humo de mina de cobre (`mine_smoke_{i}.png`).
#[derive(Resource)]
pub(crate) struct CopperMineSmokeFrames(pub(crate) Vec<AtlasSprite>);

/// Penacho anclado a una chimenea; recalcula posición por frame (los NFO
/// offsets de cada sprite difieren unos píxeles).
#[derive(Component)]
pub(crate) struct ChimneySmoke {
    anchor: Vec2,
    base_z: u8,
    tile: (i32, i32),
    phase: usize,
}

/// Crea el penacho para una tesela de chimenea terminada.
pub(crate) fn spawn_chimney_smoke(
    commands: &mut Commands,
    assets: &WorldAssets,
    ctx: &TileRenderContext,
) {
    let phase = wang_hash(ctx.tx, ctx.ty, 0x5740) as usize % CHIMNEY_SMOKE_FRAMES;
    // `CreateChimneySmoke`: (x+15, y+14, z+59) en unidades de mundo.
    let off = remap_tile_offset(15.0, 14.0, 59.0) * 0.5;
    let anchor = Vec2::new(ctx.iso_pos.x + off.x, ctx.iso_pos.y + off.y);
    let (w, h, xrel, yrel) = CHIMNEY_SMOKE_META[phase];
    let pos3 = overlay_pos(
        anchor,
        xrel,
        yrel,
        w,
        h,
        ctx.info.base_z,
        SMOKE_LAYER_FRAC,
        ctx.tx_i32(),
        ctx.ty_i32(),
    );
    commands.spawn((
        MapVisualLayer,
        ctx.map_tile_chunk(),
        ChimneySmoke {
            anchor,
            base_z: ctx.info.base_z,
            tile: (ctx.tx_i32(), ctx.ty_i32()),
            phase,
        },
        assets.chimney_smoke[phase].sprite(),
        Transform::from_translation(pos3),
    ));
}

/// Penacho de mina de cobre; ciclo `SPR_SMOKE_0..4` con ligero ascenso.
#[derive(Component)]
pub(crate) struct CopperMineSmoke {
    anchor: Vec2,
    base_z: u8,
    tile: (i32, i32),
    phase: usize,
}

/// Crea humo para tesela `GFX_COPPER_MINE_CHIMNEY` terminada.
pub(crate) fn spawn_copper_mine_smoke(
    commands: &mut Commands,
    assets: &WorldAssets,
    ctx: &TileRenderContext,
) {
    let phase = wang_hash(ctx.tx, ctx.ty, 0xC0FF) as usize % COPPER_MINE_SMOKE_FRAMES;
    // `CreateEffectVehicleAbove`: (+6, +6, z=43).
    let off = remap_tile_offset(6.0, 6.0, 43.0) * 0.5;
    let anchor = Vec2::new(ctx.iso_pos.x + off.x, ctx.iso_pos.y + off.y);
    let (w, h, xrel, yrel) = COPPER_MINE_SMOKE_META[phase];
    let pos3 = overlay_pos(
        anchor,
        xrel,
        yrel,
        w,
        h,
        ctx.info.base_z,
        SMOKE_LAYER_FRAC,
        ctx.tx_i32(),
        ctx.ty_i32(),
    );
    commands.spawn((
        MapVisualLayer,
        ctx.map_tile_chunk(),
        CopperMineSmoke {
            anchor,
            base_z: ctx.info.base_z,
            tile: (ctx.tx_i32(), ctx.ty_i32()),
            phase,
        },
        assets.copper_mine_smoke[phase].sprite(),
        Transform::from_translation(pos3),
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
    mut q: Query<(&ChimneySmoke, &mut Sprite, &mut Transform)>,
) {
    let Some(frames) = frames else {
        return;
    };
    let tick = sim.state.tick.get();
    for (smoke, mut sprite, mut transform) in &mut q {
        let idx = smoke_frame_index(tick, smoke.phase);
        if frames.0[idx].matches(&sprite) {
            continue;
        }
        frames.0[idx].apply_to(&mut sprite);
        let (w, h, xrel, yrel) = CHIMNEY_SMOKE_META[idx];
        transform.translation = overlay_pos(
            smoke.anchor,
            xrel,
            yrel,
            w,
            h,
            smoke.base_z,
            SMOKE_LAYER_FRAC,
            smoke.tile.0,
            smoke.tile.1,
        );
    }
}

/// Frame del humo de mina de cobre según tick (`SmokeTick`, cada 16).
#[must_use]
pub(crate) fn copper_smoke_frame_index(tick: u64, phase: usize) -> usize {
    ((tick / COPPER_SMOKE_TICKS_PER_FRAME) as usize + phase) % COPPER_MINE_SMOKE_FRAMES
}

pub(crate) fn animate_copper_mine_smoke(
    sim: Res<SimWorld>,
    frames: Option<Res<CopperMineSmokeFrames>>,
    mut q: Query<(&CopperMineSmoke, &mut Sprite, &mut Transform)>,
) {
    let Some(frames) = frames else {
        return;
    };
    let tick = sim.state.tick.get();
    for (smoke, mut sprite, mut transform) in &mut q {
        let idx = copper_smoke_frame_index(tick, smoke.phase);
        if !frames.0[idx].matches(&sprite) {
            frames.0[idx].apply_to(&mut sprite);
        }
        let (w, h, xrel, yrel) = COPPER_MINE_SMOKE_META[idx];
        let rise = idx as f32 * COPPER_SMOKE_RISE;
        let mut pos3 = overlay_pos(
            smoke.anchor,
            xrel,
            yrel - rise,
            w,
            h,
            smoke.base_z,
            SMOKE_LAYER_FRAC,
            smoke.tile.0,
            smoke.tile.1,
        );
        pos3.z += rise * 0.01;
        transform.translation = pos3;
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use bevy::ecs::system::RunSystemOnce;
    use openttdrs_core::{GameState, GameTick};

    use super::*;

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
    fn copper_frame_index_uses_sixteen_ticks() {
        assert_eq!(copper_smoke_frame_index(0, 0), 0);
        assert_eq!(copper_smoke_frame_index(15, 0), 0);
        assert_eq!(copper_smoke_frame_index(16, 0), 1);
    }

    #[test]
    fn animate_swaps_image_and_repositions() {
        let mut world = World::new();
        world.insert_resource(sim_at_tick(20)); // frame 2 con phase 0
        world.insert_resource(ChimneySmokeFrames(
            (0..CHIMNEY_SMOKE_FRAMES as u128).map(weak_sprite).collect(),
        ));
        let e = world
            .spawn((
                ChimneySmoke {
                    anchor: Vec2::ZERO,
                    base_z: 0,
                    tile: (1, 1),
                    phase: 0,
                },
                Sprite::default(),
                Transform::default(),
            ))
            .id();

        world.run_system_once(animate_chimney_smoke).unwrap();

        let expected = smoke_frame_index(20, 0);
        assert!(weak_sprite(expected as u128).matches(world.get::<Sprite>(e).unwrap()));
        assert_ne!(world.get::<Transform>(e).unwrap().translation, Vec3::ZERO);
    }
}
