use bevy::prelude::*;
use openttdrs_core::Map;

use super::{SHORE_LAYER_FRAC, push_water_sprite, spawn_coast_debug_label};
use crate::iso::{
    shore_png_index, shore_sprite_half_h, shore_tileh_for_draw_shore, tile_pos_half,
    tile_slope_bits_from_heights,
};
use crate::render::{MapSpriteBatches, TileRenderContext, WorldAssets};

pub(crate) fn push_water_tile(
    commands: &mut Commands,
    map: &Map,
    map_dims: (u32, u32),
    assets: &WorldAssets,
    ctx: &TileRenderContext,
    debug_coast: bool,
    batches: &mut MapSpriteBatches,
) {
    if ctx.info.use_shore {
        // `DrawShoreTile(tileh)` — igual que OpenTTD: pendiente real del 2×2
        // cuando no es plana; si no, vecinos de tierra (`infer_coast`).
        let th = shore_tileh_for_draw_shore(map, ctx.tx, ctx.ty, map_dims.0, map_dims.1);
        if th != 0 {
            let si = shore_png_index(th);
            // Coast en OpenTTD dibuja solo `DrawShoreTile`: el PNG del set
            // completo ya incluye agua/tierra del rombo, con transparencia
            // solo fuera de él.
            batches.shore.push((
                crate::render::ShoreTile(si as u8),
                assets.shore[si].sprite(),
                Transform::from_translation(tile_pos_half(
                    ctx.tx_i32(),
                    ctx.ty_i32(),
                    ctx.info.base_z,
                    SHORE_LAYER_FRAC,
                    shore_sprite_half_h(th),
                )),
            ));
            if debug_coast {
                let (raw, _) = tile_slope_bits_from_heights(map, ctx.tx, ctx.ty);
                spawn_coast_debug_label(commands, ctx, raw, th, si);
            }
        } else {
            // Datos inválidos: OpenTTD asertea que Coast no es flat. Evitamos un hueco.
            push_water_sprite(&mut batches.water, &assets.water, ctx);
        }
    } else {
        // Agua libre (Clear, Lock, Depot en mapas típicos: Clear).
        push_water_sprite(&mut batches.water, &assets.water, ctx);
    }
}
