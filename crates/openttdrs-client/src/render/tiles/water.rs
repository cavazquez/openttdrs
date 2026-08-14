use bevy::prelude::*;
use openttdrs_core::DecodedSprite;
use openttdrs_core::prelude::*;

use super::{SHORE_LAYER_FRAC, push_water_sprite, spawn_coast_debug_label};
use crate::iso::{
    GROUND_SPRITE_CENTER_X_OFFSET, TILE_HALF_H, shore_png_index, shore_sprite_half_h,
    shore_tileh_for_draw_shore, slope_half_h, tile_pos_half, tile_slope_bits_from_heights,
};
use crate::render::shore_newgrf::{NEWGRF_SHORE_TILE_FLAG, NewGrfShoreSpriteCache};
use crate::render::world_draw_trace::WorldDrawTrace;
use crate::render::{MapSpriteBatches, TileRenderContext, WorldAssets};

/// `SPR_FLAT_WATER_TILE` de `table/sprites.h`.
const SPR_FLAT_WATER_TILE: u32 = 4061;
/// `SPR_SHORE_BASE` resuelto por Action5 canals en OpenGFX/OpenGFX2.
const SPR_SHORE_BASE: u32 = 5936;

fn shore_sprite_id(tileh: u8) -> u32 {
    SPR_SHORE_BASE + shore_png_index(tileh) as u32
}

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
            // `DrawShoreTile` siempre entrega el slot Action5 global
            // `SPR_SHORE_BASE + tileh_to_shoresprite[tileh]`. Aunque el
            // cache lo materialice como NewGRF, éste continúa siendo el ID
            // lógico que expone el oráculo C++.
            WorldDrawTrace::record_sprite("water-shore", "ground", shore_sprite_id(th), false);
            let mut position = tile_pos_half(
                ctx.tx_i32(),
                ctx.ty_i32(),
                ctx.info.base_z,
                SHORE_LAYER_FRAC,
                shore_sprite_half_h(th),
            );
            // `DrawShoreTile` comparte el xrel=-31 de los ground sprites;
            // no hereda el centro geométrico -32 del Sprite de Bevy.
            position.x += GROUND_SPRITE_CENTER_X_OFFSET;
            let transform = Transform::from_translation(position);
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
            WorldDrawTrace::record_sprite(
                "water-ground-fallback",
                "ground",
                SPR_FLAT_WATER_TILE,
                true,
            );
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
                slope_half_h(ctx.info.tileh)
            };
            batches.water.push((
                ctx.map_tile_chunk(),
                crate::render::WaterTile::STATIC,
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
            // `DrawSeaWater` usa directamente `SPR_FLAT_WATER_TILE`. Las
            // clases canal/río entran por aquí en el renderer actual: la
            // auditoría dirá si su selección C++ requiere una rama propia.
            WorldDrawTrace::record_sprite("water-ground", "ground", SPR_FLAT_WATER_TILE, false);
            push_water_sprite(&mut batches.water, &assets.water, ctx);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{SPR_FLAT_WATER_TILE, shore_sprite_id};

    #[test]
    fn water_trace_sprite_ids_follow_openttd_water_and_shore_tables() {
        assert_eq!(SPR_FLAT_WATER_TILE, 4061);
        assert_eq!(shore_sprite_id(1), 5937); // SLOPE_W.
        assert_eq!(shore_sprite_id(23), 5936); // SLOPE_STEEP_S -> slot 0.
        assert_eq!(shore_sprite_id(27), 5941); // SLOPE_STEEP_N -> slot 5.
        assert_eq!(shore_sprite_id(30), 5951); // SLOPE_STEEP_E -> slot 15.
    }
}
