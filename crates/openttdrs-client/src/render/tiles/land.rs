use bevy::prelude::*;
use openttdrs_core::TileKind;

use super::{
    leveled_foundation_overlay_pos, sloped_or_flat_image, spawn_ground_sprite,
    spawn_leveled_foundation,
};
use crate::iso::{overlay_pos, remap_tile_offset, wang_hash};
use crate::render::{MapSpriteBatches, MapVisualLayer, TileRenderContext, WorldAssets};
use crate::sprites::{
    FENCE_MOD_BY_TILEH_NE, FENCE_MOD_BY_TILEH_NW, FENCE_MOD_BY_TILEH_SE, FENCE_MOD_BY_TILEH_SW,
    FENCE_SPRITE_META, FIELD_STATES, HOUSE_DRAW_DATA, TREE_LAYOUT_SPRITE, TREE_LAYOUT_XY,
    TREE_SPRITE_META, house_building_stage_from_tile, house_draw_data_index_for_tile,
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
    // Chimenea de la central terminada: penacho de humo animado encima.
    if gfx == crate::render::GFX_POWERPLANT_CHIMNEY && m1 & 0x80 != 0 {
        crate::render::spawn_chimney_smoke(commands, assets, ctx);
    }
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
                3 => {
                    // `DrawTile_Clear` Fields: estado de cultivo en bits 0–3 de
                    // m3 + offset de pendiente; cercas como overlay.
                    let state =
                        usize::from(ctx.tile.map_or(0, |t| t.m3 & 0x0F)).min(FIELD_STATES - 1);
                    let img = assets.fields[state * 15 + usize::from(tileh.min(14))].clone();
                    spawn_field_fences(commands, assets, ctx);
                    (img, Color::WHITE)
                }
                _ => (rough_img(), Color::srgb(0.78, 0.73, 0.58)), // rough/rocky
            }
        }
        TileKind::Grass => (grass_img(), Color::WHITE), // MP_OBJECT u otros
        // MP_TREES con TreeGround::Grass: hierba normal (los árboles van encima).
        TileKind::Forest => (grass_img(), Color::WHITE),
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

/// Cercas de campos de cultivo, fiel a `DrawClearLandFence` (`clear_cmd.cpp`):
/// tipo por lado en m4 (SE bits 2–4, SW 5–7), m3 (NE bits 5–7) y m6 (NW bits
/// 2–4); variante de sprite por pendiente (`_fence_mod_by_tileh_*`).
fn spawn_field_fences(commands: &mut Commands, assets: &WorldAssets, ctx: &TileRenderContext) {
    let Some(t) = ctx.tile else {
        return;
    };
    let tileh = usize::from(ctx.info.tileh & 0x1F);
    // (tipo, tabla de variantes, dx, dy, bit de esquina para z, capa)
    // NW usa CORNER_W (bit 1), NE CORNER_E (bit 4), SW/SE CORNER_S (bit 2).
    let sides: [(u8, &[u8; 32], f32, f32, u8, f32); 4] = [
        (
            (t.m6 >> 2) & 0x7,
            &FENCE_MOD_BY_TILEH_NW,
            0.0,
            -16.0,
            0x1,
            0.06,
        ),
        (
            (t.m3 >> 5) & 0x7,
            &FENCE_MOD_BY_TILEH_NE,
            -16.0,
            0.0,
            0x4,
            0.06,
        ),
        (
            (t.m3hi >> 5) & 0x7,
            &FENCE_MOD_BY_TILEH_SW,
            0.0,
            0.0,
            0x2,
            0.26,
        ),
        (
            (t.m3hi >> 2) & 0x7,
            &FENCE_MOD_BY_TILEH_SE,
            0.0,
            0.0,
            0x2,
            0.26,
        ),
    ];
    for (fence, mods, dx, dy, corner_bit, layer) in sides {
        if fence == 0 {
            continue;
        }
        let ftype = usize::from(fence - 1).min(5);
        let variant = usize::from(mods[tileh]).min(5);
        let meta = FENCE_SPRITE_META[ftype][variant];
        // `GetSlopePixelZInCorner`: esquina elevada = TILE_HEIGHT (8 unidades).
        let dz = if ctx.info.tileh & corner_bit != 0 {
            8.0
        } else {
            0.0
        };
        let off = remap_tile_offset(dx, dy, dz) * 0.5;
        let pos3 = overlay_pos(
            Vec2::new(ctx.iso_pos.x + off.x, ctx.iso_pos.y + off.y),
            meta.xrel,
            meta.yrel,
            meta.w,
            meta.h,
            ctx.info.base_z,
            layer,
            ctx.tx_i32(),
            ctx.ty_i32(),
        );
        commands.spawn((
            MapVisualLayer,
            Sprite {
                image: assets.fences[ftype * 6 + variant].clone(),
                color: Color::WHITE,
                ..default()
            },
            Transform::from_translation(pos3),
        ));
    }
}

/// Árboles de una tesela `MP_TREES`, fiel a `DrawTile_Trees` (`tree_cmd.cpp`):
/// 1–4 árboles según bits 6–7 de m5, posiciones de `_tree_layout_xy`, especie
/// por árbol de `_tree_layout_sprite[tipo×4 + variante]` y etapa de
/// crecimiento (bits 0–2 de m5) solo en el último árbol (el resto adulto +3).
pub(crate) fn push_forest_tree(
    assets: &WorldAssets,
    ctx: &TileRenderContext,
    batches: &mut MapSpriteBatches,
    map_w: u32,
) {
    let (tree_type, count, growth) = match ctx.tile {
        // MP_TREES real (nibble alto de mapt = 4): datos del save.
        Some(t) if (t.mapt >> 4) & 0xF == 4 => (
            u32::from(t.m3) % 12,
            usize::from((t.m5 >> 6) & 0x3) + 1,
            usize::from(t.m5 & 0x7).min(6),
        ),
        // Mapas generados sin datos: variedad determinista equivalente.
        _ => {
            let h = wang_hash(ctx.tx, ctx.ty, 0xCAFE);
            (h % 12, (h >> 8) as usize % 2 + 1, 3)
        }
    };

    // `tmp = CountBits(tile + x + y)` con x/y en unidades de mundo (×16).
    let tile_index = ctx.ty * map_w + ctx.tx;
    let tmp = (tile_index + ctx.tx * 16 + ctx.ty * 16).count_ones();
    let variant = (tmp & 0x3) as usize;
    let layout = ((tmp >> 2) & 0x3) as usize;

    let row = &TREE_LAYOUT_SPRITE[tree_type as usize * 4 + variant];
    for i in 0..count {
        let stage = if i == count - 1 { growth } else { 3 };
        let sprite_idx = row[i] as usize + stage;
        let meta = &TREE_SPRITE_META[sprite_idx];
        let (dx, dy) = TREE_LAYOUT_XY[layout][i];
        // Offset sub-tesela en pantalla (misma escala que iso(): remap × 0.5).
        let off = remap_tile_offset(f32::from(dx), f32::from(dy), 0.0) * 0.5;
        let pos3 = overlay_pos(
            Vec2::new(ctx.iso_pos.x + off.x, ctx.iso_pos.y + off.y),
            meta.xrel,
            meta.yrel,
            meta.w,
            meta.h,
            ctx.info.base_z,
            // Orden dentro de la tesela: los árboles más al sur tapan.
            0.3 + (f32::from(dx) + f32::from(dy)) * 1e-4,
            ctx.tx_i32(),
            ctx.ty_i32(),
        );
        batches.trees.push((
            Sprite {
                image: assets.trees[sprite_idx].clone(),
                ..default()
            },
            Transform::from_translation(pos3),
        ));
    }
}
