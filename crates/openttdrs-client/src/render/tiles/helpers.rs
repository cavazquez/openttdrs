use bevy::prelude::*;

use crate::iso::{HEIGHT_PX, TILE_HALF_H, overlay_pos, tile_pos, tile_pos_half};
use crate::render::{MapVisualLayer, TileRenderContext, WorldAssets};
use crate::sprites::foundation_gfx_for_tileh;

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

/// Posición de overlay tras `DrawFoundation(FOUNDATION_LEVELED)` + `OffsetGroundSprite(0, -8)`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn leveled_foundation_overlay_pos(
    ref_pos: Vec2,
    xrel: f32,
    yrel: f32,
    w: f32,
    h: f32,
    base_z: u8,
    layer: f32,
    tx: i32,
    ty: i32,
) -> Vec3 {
    let mut pos = overlay_pos(
        ref_pos,
        xrel,
        yrel,
        w,
        h,
        base_z.saturating_add(1),
        layer,
        tx,
        ty,
    );
    pos.y -= HEIGHT_PX;
    pos
}

pub(crate) fn spawn_leveled_foundation(
    commands: &mut Commands,
    assets: &WorldAssets,
    ctx: &TileRenderContext,
    tileh: u8,
) {
    let Some(gfx) = foundation_gfx_for_tileh(tileh) else {
        return;
    };
    let Some(img) = assets.foundations.get((tileh - 1) as usize) else {
        return;
    };
    let pos = overlay_pos(
        ctx.iso_pos,
        gfx.xrel,
        gfx.yrel,
        gfx.w,
        gfx.h,
        ctx.info.base_z,
        0.36,
        ctx.tx_i32(),
        ctx.ty_i32(),
    );
    commands.spawn((
        MapVisualLayer,
        Sprite {
            image: img.clone(),
            color: Color::WHITE,
            ..default()
        },
        Transform::from_translation(pos).with_scale(Vec3::new(
            TILE_OVERLAP_SCALE,
            TILE_OVERLAP_SCALE,
            1.0,
        )),
    ));
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

pub(crate) fn push_water_sprite(
    batch_water: &mut Vec<(Sprite, Transform)>,
    h_water: &Handle<Image>,
    ctx: &TileRenderContext,
) {
    batch_water.push((
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
    use bevy::prelude::Vec2;

    use super::{FLAT_WATER_LAYER_FRAC, SHORE_LAYER_FRAC, leveled_foundation_overlay_pos};
    use crate::iso::{TILE_HALF_H, overlay_pos, tile_pos, tile_pos_half};

    #[test]
    fn leveled_overlay_matches_flat_elevation() {
        let flat = overlay_pos(Vec2::ZERO, 0.0, 0.0, 64.0, 40.0, 2, 0.5, 3, 4);
        let leveled =
            leveled_foundation_overlay_pos(Vec2::ZERO, 0.0, 0.0, 64.0, 40.0, 2, 0.5, 3, 4);
        assert!((flat.y - leveled.y).abs() < 0.01);
    }

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
