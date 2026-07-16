use bevy::prelude::*;
use openttdrs_core::DecodedSprite;
use openttdrs_core::prelude::*;

use super::{SHORE_LAYER_FRAC, push_water_sprite, spawn_coast_debug_label};
use crate::iso::{
    SLOPE_HALF_H, TILE_HALF_H, shore_png_index, shore_sprite_half_h, shore_tileh_for_draw_shore,
    tile_pos_half, tile_slope_bits_from_heights,
};
use crate::render::shore_newgrf::{NEWGRF_SHORE_TILE_FLAG, NewGrfShoreSpriteCache};
use crate::render::{MapSpriteBatches, TileRenderContext, WorldAssets};

#[allow(clippy::too_many_arguments)]
pub(crate) fn push_water_tile(
    commands: &mut Commands,
    map: &Map,
    map_dims: (u32, u32),
    assets: &WorldAssets,
    ctx: &TileRenderContext,
    debug_coast: bool,
    batches: &mut MapSpriteBatches,
    shore_newgrf: &[Option<DecodedSprite>],
    shore_sprites: Option<&mut NewGrfShoreSpriteCache>,
    images: Option<&mut Assets<Image>>,
) {
    if ctx.info.use_shore {
        // `DrawShoreTile(tileh)` — igual que OpenTTD: pendiente real del 2×2
        // cuando no es plana; si no, vecinos de tierra (`infer_coast`).
        let th = shore_tileh_for_draw_shore(map, ctx.tx, ctx.ty, map_dims.0, map_dims.1);
        if th != 0 {
            let si = shore_png_index(th);
            let transform = Transform::from_translation(tile_pos_half(
                ctx.tx_i32(),
                ctx.ty_i32(),
                ctx.info.base_z,
                SHORE_LAYER_FRAC,
                shore_sprite_half_h(th),
            ));
            let mut used_newgrf = false;
            let sprite = if let (Some(cache), Some(images), Some(decoded)) = (
                shore_sprites,
                images,
                shore_newgrf.get(si).and_then(|s| s.as_ref()),
            ) {
                used_newgrf = true;
                let handle = cache.handle_for(si as u8, decoded, images);
                Sprite {
                    image: handle,
                    color: Color::WHITE,
                    ..default()
                }
            } else {
                assets.shore[si].sprite()
            };
            let shore_marker = if used_newgrf {
                crate::render::ShoreTile(si as u8 | NEWGRF_SHORE_TILE_FLAG)
            } else {
                crate::render::ShoreTile(si as u8)
            };
            // Coast en OpenTTD dibuja solo `DrawShoreTile`: el PNG del set
            // completo ya incluye agua/tierra del rombo, con transparencia
            // solo fuera de él.
            batches
                .shore
                .push((ctx.map_tile_chunk(), shore_marker, sprite, transform));
            if debug_coast {
                let (raw, _) = tile_slope_bits_from_heights(map, ctx.tx, ctx.ty);
                spawn_coast_debug_label(commands, ctx, raw, th, si);
            }
        } else {
            // Datos inválidos: OpenTTD asertea que Coast no es flat. Evitamos un hueco.
            push_water_sprite(&mut batches.water, &assets.water, ctx);
        }
    } else {
        // Agua libre (Clear) o esclusa (m5 subtype Lock = 2).
        let m5 = ctx.tile.map(|t| t.m5).unwrap_or(0);
        if (m5 >> 4) & 0x0F == 2 {
            let axis = usize::from(m5 & 1).min(1);
            let level = openttdrs_core::lock_sprite_level(map, ctx.coord).min(2);
            let half_h = if ctx.info.tileh == 0 {
                TILE_HALF_H
            } else {
                SLOPE_HALF_H[ctx.info.tileh as usize]
            };
            batches.water.push((
                ctx.map_tile_chunk(),
                assets.water_lock[axis][level].sprite(),
                Transform::from_translation(tile_pos_half(
                    ctx.tx_i32(),
                    ctx.ty_i32(),
                    ctx.info.base_z,
                    0.02,
                    half_h,
                )),
            ));
        } else {
            push_water_sprite(&mut batches.water, &assets.water, ctx);
        }
    }
}
