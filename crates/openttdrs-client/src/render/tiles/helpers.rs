use bevy::prelude::*;

use crate::iso::{TILE_HALF_H, tile_pos, tile_pos_half, wang_hash};
use crate::render::{MapVisualLayer, TileRenderContext, WaterTile};

/// Sesgo en la componente Z de **solo** el agua animada (sin sprite `shore_*`).
/// El orden de dibujo usa `(tx+ty)`; el mar al **este/sur** tiene suma mayor y acaba
/// encima del borde costero del vecino NO/NE → sierra y rectángulos azules oscuros.
pub(crate) const FLAT_WATER_LAYER_FRAC: f32 = -0.030;
/// Costa entre tierra y agua: debe tapar agua vecina, pero no pintar su parte azul
/// encima de la tierra que queda del lado interior de la orilla.
pub(crate) const SHORE_LAYER_FRAC: f32 = -0.015;
/// Solape mínimo para ocultar costuras finas entre tiles adyacentes.
pub(crate) const TILE_OVERLAP_SCALE: f32 = 1.002;
/// Capa de tranvía (`tram_flat_*`, SPR_TRAMWAY_OVERLAY) por encima del asfalto.
pub(crate) const TRAM_OVERLAY_LAYER_FRAC: f32 = 0.028;

pub(crate) fn sloped_or_flat_image(
    tileh: u8,
    flat: &Handle<Image>,
    slopes: &[Handle<Image>],
) -> Handle<Image> {
    if tileh == 0 {
        flat.clone()
    } else {
        slopes[tileh as usize - 1].clone()
    }
}

pub(crate) fn spawn_ground_sprite(
    commands: &mut Commands,
    image: Handle<Image>,
    color: Color,
    ctx: &TileRenderContext,
    half_h: f32,
) {
    commands.spawn((
        MapVisualLayer,
        Sprite {
            image,
            color,
            ..default()
        },
        Transform::from_translation(tile_pos_half(
            ctx.tx_i32(),
            ctx.ty_i32(),
            ctx.info.base_z,
            0.0,
            half_h,
        ))
        .with_scale(Vec3::new(TILE_OVERLAP_SCALE, TILE_OVERLAP_SCALE, 1.0)),
    ));
}

fn water_phases(tx: u32, ty: u32) -> WaterTile {
    WaterTile {
        dark_phase: ((tx + 2 * ty).rem_euclid(5)) as u8,
        glitter_phase: (wang_hash(tx, ty, 0xA9FE) % 15) as u8,
    }
}

pub(crate) fn push_water_sprite(
    batch_water: &mut Vec<(WaterTile, Sprite, Transform)>,
    h_water: &Handle<Image>,
    ctx: &TileRenderContext,
) {
    batch_water.push((
        water_phases(ctx.tx, ctx.ty),
        Sprite {
            image: h_water.clone(),
            color: Color::WHITE,
            ..default()
        },
        Transform::from_translation(tile_pos(
            ctx.tx_i32(),
            ctx.ty_i32(),
            ctx.info.base_z,
            FLAT_WATER_LAYER_FRAC,
        ))
        .with_scale(Vec3::new(TILE_OVERLAP_SCALE, TILE_OVERLAP_SCALE, 1.0)),
    ));
}

pub(crate) fn spawn_coast_debug_label(
    commands: &mut Commands,
    ctx: &TileRenderContext,
    raw: u8,
    tileh: u8,
    shore_index: usize,
) {
    let label = format!("r{raw}/t{tileh}/s{shore_index}");
    commands.spawn((
        MapVisualLayer,
        Text2d::new(label),
        TextFont {
            font_size: 9.0,
            ..default()
        },
        TextColor(Color::srgb(1.0, 0.95, 0.4)),
        Transform::from_translation(Vec3::new(
            ctx.iso_pos.x - 18.0,
            ctx.iso_pos.y - TILE_HALF_H + f32::from(ctx.info.base_z) * 8.0 - 3.0,
            (ctx.tx + ctx.ty) as f32 * 0.01 + f32::from(ctx.info.base_z) * 0.001 + 0.95,
        )),
    ));
}

#[cfg(test)]
mod tests {
    use super::{FLAT_WATER_LAYER_FRAC, SHORE_LAYER_FRAC};
    use crate::iso::{TILE_HALF_H, tile_pos, tile_pos_half};

    #[test]
    fn shore_z_sits_between_neighbor_land_and_water() {
        let tx = 10;
        let ty = 10;
        let shore = tile_pos_half(tx, ty, 0, SHORE_LAYER_FRAC, TILE_HALF_H).z;
        let inner_land = tile_pos(tx - 1, ty, 0, 0.0).z;
        let outer_water = tile_pos(tx + 1, ty, 0, FLAT_WATER_LAYER_FRAC).z;

        assert!(shore < inner_land);
        assert!(shore > outer_water);
    }
}
