use bevy::prelude::*;
use openttdrs_core::TileKind;

use super::{
    leveled_foundation_overlay_pos, sloped_or_flat_image, spawn_ground_sprite,
    spawn_leveled_foundation,
};
use crate::iso::{overlay_pos, wang_hash};
use crate::render::{MapSpriteBatches, MapVisualLayer, TileRenderContext, WorldAssets};
use crate::sprites::{
    HOUSE_DRAW_DATA, house_building_stage_from_tile, house_draw_data_index_for_tile,
};

pub(crate) fn spawn_house_tile(
    commands: &mut Commands,
    assets: &WorldAssets,
    ctx: &TileRenderContext,
    slope_half_ground: f32,
) {
    let tileh = ctx.info.tileh;
    let base_z = ctx.info.base_z;
    // GetCleanHouseType: GB(m8, 0, 12) — el resto es datos NewGRF
    let clean_house_id = ctx.tile.map_or(0u16, |t| t.m8 & 0xFFF);
    let house_base = sloped_or_flat_image(tileh, &assets.grass, &assets.grass_slopes);
    spawn_ground_sprite(commands, house_base, Color::WHITE, ctx, slope_half_ground);
    let (m5, m3) = ctx.tile.map_or((0u8, 0x80u8), |t| (t.m5, t.m3));
    let building_stage = house_building_stage_from_tile(m5, m3);
    let spec_idx =
        house_draw_data_index_for_tile(clean_house_id, ctx.tx_i32(), ctx.ty_i32(), building_stage);
    let spec = &HOUSE_DRAW_DATA[spec_idx];
    if spec.s1 != 0
        && let Some(img) = assets.houses.get(&spec.s1)
    {
        let pos3 = overlay_pos(
            ctx.iso_pos,
            spec.s1_xrel,
            spec.s1_yrel,
            spec.s1_w,
            spec.s1_h,
            base_z,
            0.4,
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
            Transform::from_translation(pos3),
        ));
    }
    if spec.s2 != 0
        && let Some(img) = assets.houses.get(&spec.s2)
    {
        let pos3 = overlay_pos(
            ctx.iso_pos,
            spec.s2_xrel,
            spec.s2_yrel,
            spec.s2_w,
            spec.s2_h,
            base_z,
            0.5,
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
            Transform::from_translation(pos3),
        ));
    }
}

pub(crate) fn spawn_industry_tile(
    commands: &mut Commands,
    assets: &WorldAssets,
    ctx: &TileRenderContext,
    slope_half_ground: f32,
) {
    let tileh = ctx.info.tileh;
    let base_z = ctx.info.base_z;
    // gfx de industria es de 9 bits: m5 (bits 0-7) | bit 2 de m6 (bit 8)
    // Fuente: GetCleanIndustryGfx() en industry_map.h de OpenTTD
    let gfx = ctx.tile.map_or(0u16, |t| {
        u16::from(t.m5) | (u16::from((t.m6 >> 2) & 1) << 8)
    });
    let m1 = ctx.tile.map_or(0x80, |t| t.m1);
    let entry = crate::sprites::industry_gfx_entry_for_tile(gfx, m1);
    crate::sprites::log_industry_gfx_once(gfx, m1, entry);
    // Terreno natural bajo la industria; en pendiente se añade cimiento nivelado (P4).
    let terrain_img = sloped_or_flat_image(tileh, &assets.rough, &assets.rough_slopes);
    let terrain_color = Color::srgb(0.55, 0.50, 0.45);
    spawn_ground_sprite(commands, terrain_img, terrain_color, ctx, slope_half_ground);
    let leveled = tileh != 0;
    if leveled {
        spawn_leveled_foundation(commands, assets, ctx, tileh);
    }
    let overlay_z = if leveled {
        base_z.saturating_add(crate::sprites::leveled_foundation_z_delta(tileh))
    } else {
        base_z
    };
    let overlay_at = |xrel, yrel, w, h, layer| {
        if leveled {
            leveled_foundation_overlay_pos(
                ctx.iso_pos,
                xrel,
                yrel,
                w,
                h,
                base_z,
                layer,
                ctx.tx_i32(),
                ctx.ty_i32(),
            )
        } else {
            crate::iso::overlay_pos(
                ctx.iso_pos,
                xrel,
                yrel,
                w,
                h,
                overlay_z,
                layer,
                ctx.tx_i32(),
                ctx.ty_i32(),
            )
        }
    };
    if let Some(s) = entry {
        if s.ground_sprite_id != 0
            && s.ground_w > 0.0
            && s.ground_h > 0.0
            && let Some(img) = assets.industries.get(&s.ground_sprite_id)
        {
            let pos_g = overlay_at(s.ground_xrel, s.ground_yrel, s.ground_w, s.ground_h, 0.45);
            commands.spawn((
                MapVisualLayer,
                Sprite {
                    image: img.clone(),
                    color: Color::WHITE,
                    ..default()
                },
                Transform::from_translation(pos_g),
            ));
        }
        if s.sprite_id != 0
            && s.w > 0.0
            && s.h > 0.0
            && let Some(img) = assets.industries.get(&s.sprite_id)
        {
            let pos3 = overlay_at(s.xrel, s.yrel, s.w, s.h, 0.5);
            commands.spawn((
                MapVisualLayer,
                Sprite {
                    image: img.clone(),
                    color: Color::WHITE,
                    ..default()
                },
                Transform::from_translation(pos3),
            ));
        }
    }
}

pub(crate) fn spawn_generic_land_tile(
    commands: &mut Commands,
    assets: &WorldAssets,
    ctx: &TileRenderContext,
    slope_half_ground: f32,
) {
    let tileh = ctx.info.tileh;
    let ottd_type = ctx.tile.map_or(0u8, |t| (t.mapt >> 4) & 0xF);
    let tile_m5 = ctx.tile.map_or(0u8, |t| t.m5);

    // MP_CLEAR (0): distinguir subtipo de suelo via m5 bits 2-4
    // MP_OBJECT (10): grass de base + overlay de objeto
    let grass_img = || sloped_or_flat_image(tileh, &assets.grass, &assets.grass_slopes);
    let rough_img = || sloped_or_flat_image(tileh, &assets.rough, &assets.rough_slopes);

    let (image, color) = match ctx.kind {
        TileKind::Grass if ottd_type == 0 => {
            // bits 2-4 de m5 = ClearGround
            // 0=grass, 1=rough, 2=rocky, 3=fields, 4=snow, 5=desert
            match (tile_m5 >> 2) & 0x7 {
                0 => (grass_img(), Color::WHITE),
                3 => (rough_img(), Color::srgb(0.82, 0.72, 0.45)), // campos
                _ => (rough_img(), Color::srgb(0.78, 0.73, 0.58)), // rough/rocky
            }
        }
        TileKind::Grass => (grass_img(), Color::WHITE), // MP_OBJECT u otros
        TileKind::Forest => (rough_img(), Color::srgb(0.6, 1.0, 0.45)),
        TileKind::CoalField => (rough_img(), Color::srgb(0.55, 0.50, 0.45)),
        TileKind::Unknown(_) => (grass_img(), Color::srgb(1.0, 0.0, 1.0)),
        TileKind::House
        | TileKind::Station
        | TileKind::Road
        | TileKind::Rail
        | TileKind::RoadDepot
        | TileKind::RailDepot
        | TileKind::RoadTunnel
        | TileKind::RailTunnel
        | TileKind::RoadBridge
        | TileKind::RailBridge
        | TileKind::Industry
        | TileKind::Water
        | TileKind::Void => unreachable!(),
    };
    spawn_ground_sprite(commands, image, color, ctx, slope_half_ground);

    // MP_OBJECT: renderizar faro o transmisor como overlay.
    // ObjectType de OpenTTD: 0=Transmisor, 1=Faro.
    if ottd_type == 10 {
        let (obj_img, obj_xrel, obj_yrel, obj_w, obj_h) = match tile_m5 {
            // OBJECT_TRANSMITTER=0: sprite 2601, 55x77, xrel=-26, yrel=-71
            0 => (Some(assets.transmitter.clone()), -26.0, -71.0, 55.0, 77.0),
            // OBJECT_LIGHTHOUSE=1: sprite 2602, 41x61, xrel=-22, yrel=-48
            1 => (Some(assets.lighthouse.clone()), -22.0, -48.0, 41.0, 61.0),
            _ => (None, 0.0, 0.0, 0.0, 0.0),
        };
        if let Some(img) = obj_img {
            let pos3 = overlay_pos(
                ctx.iso_pos,
                obj_xrel,
                obj_yrel,
                obj_w,
                obj_h,
                ctx.info.base_z,
                0.6,
                ctx.tx_i32(),
                ctx.ty_i32(),
            );
            commands.spawn((
                MapVisualLayer,
                Sprite {
                    image: img,
                    color: Color::WHITE,
                    ..default()
                },
                Transform::from_translation(pos3),
            ));
        }
    }
}

pub(crate) fn push_forest_tree(
    assets: &WorldAssets,
    ctx: &TileRenderContext,
    batches: &mut MapSpriteBatches,
) {
    let h = wang_hash(ctx.tx, ctx.ty, 0xCAFE);
    let tree_idx = (h % 3) as usize;
    let ox = ((h >> 2) % 17) as f32 - 8.0;
    let pos3 = overlay_pos(
        Vec2::new(ctx.iso_pos.x + ox, ctx.iso_pos.y),
        -19.0,
        -36.0,
        35.0,
        43.0,
        ctx.info.base_z,
        0.3,
        ctx.tx_i32(),
        ctx.ty_i32(),
    );
    batches.trees.push((
        Sprite {
            image: assets.trees[tree_idx].clone(),
            ..default()
        },
        Transform::from_translation(pos3),
    ));
}
