//! Humo de la chimenea de la central eléctrica, fiel al `EffectVehicle`
//! `EV_CHIMNEY_SMOKE` de OpenTTD (`industry_cmd.cpp` + `effectvehicle.cpp`):
//! se ancla en la tesela `GFX_POWERPLANT_CHIMNEY` en el punto de mundo
//! `(+15, +14, z+59)` y cicla `SPR_CHIMNEY_SMOKE_0..7` (un frame cada
//! 8 ticks ≈ 0.22 s, con fase inicial aleatoria).

use bevy::prelude::*;

use crate::bevy_app::UpdateSet;
use crate::iso::{overlay_pos, remap_tile_offset, wang_hash};
use crate::render::{AtlasSprite, MapVisualLayer, TileRenderContext, WorldAssets};
use crate::sprites::{CHIMNEY_SMOKE_FRAMES, CHIMNEY_SMOKE_META};
use crate::state::ClientScreen;

pub(crate) struct IndustrySmokePlugin;

impl Plugin for IndustrySmokePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            animate_chimney_smoke
                .in_set(UpdateSet::Visuals)
                .run_if(in_state(ClientScreen::InGame)),
        );
    }
}

/// `GetIndustryGfx` de la tesela de chimenea de la central (`industry_map.h`).
pub(crate) const GFX_POWERPLANT_CHIMNEY: u16 = 8;

/// Duración de cada frame (8 ticks de juego de ~27 ms).
const SMOKE_FRAME_SECS: f32 = 0.22;

/// Capa por encima del edificio de la industria (overlays usan 0.4/0.5).
const SMOKE_LAYER_FRAC: f32 = 0.55;

/// Frames del humo (`chimney_smoke_{i}.png`), insertado con la capa de mundo.
#[derive(Resource)]
pub(crate) struct ChimneySmokeFrames(pub(crate) Vec<AtlasSprite>);

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

/// Frame global del penacho para `elapsed_secs` y fase inicial (puro, testeable).
#[must_use]
pub(crate) fn smoke_frame_index(elapsed_secs: f32, phase: usize) -> usize {
    ((elapsed_secs / SMOKE_FRAME_SECS) as usize + phase) % CHIMNEY_SMOKE_FRAMES
}

pub(crate) fn animate_chimney_smoke(
    time: Res<Time>,
    frames: Option<Res<ChimneySmokeFrames>>,
    mut q: Query<(&ChimneySmoke, &mut Sprite, &mut Transform)>,
) {
    let Some(frames) = frames else {
        return;
    };
    for (smoke, mut sprite, mut transform) in &mut q {
        let idx = smoke_frame_index(time.elapsed_secs(), smoke.phase);
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

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use bevy::ecs::system::RunSystemOnce;

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

    #[test]
    fn frame_index_cycles_with_phase() {
        assert_eq!(smoke_frame_index(0.0, 0), 0);
        assert_eq!(smoke_frame_index(0.0, 3), 3);
        assert_eq!(smoke_frame_index(0.23, 0), 1);
        assert_eq!(smoke_frame_index(0.22 * 8.5, 0), 0);
    }

    #[test]
    fn animate_swaps_image_and_repositions() {
        let mut world = World::new();
        let mut time = Time::<()>::default();
        time.advance_by(std::time::Duration::from_millis(500));
        world.insert_resource(time);
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

        let expected = smoke_frame_index(0.5, 0);
        assert!(weak_sprite(expected as u128).matches(world.get::<Sprite>(e).unwrap()));
        assert_ne!(world.get::<Transform>(e).unwrap().translation, Vec3::ZERO);
    }
}
