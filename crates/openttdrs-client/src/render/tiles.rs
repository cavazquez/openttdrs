use bevy::prelude::*;
use openttdrs_core::{Map, TileKind};

use super::{MapSpriteBatches, MapVisualLayer, TileRenderContext, WaterTile, WorldAssets};
use crate::iso::{
    SLOPE_HALF_H, TILE_HALF_H, overlay_pos, shore_png_index, shore_tileh_for_draw_shore, tile_pos,
    tile_pos_half, tile_slope_bits_from_heights, wang_hash,
};
use crate::sprites::{
    HOUSE_DRAW_DATA, ROAD_FLAT_HALF_H, collect_rail_sprites, collect_signal_sprite_ids,
    house_draw_data_index_for_tile, is_road_level_crossing, level_crossing_has_rail_reservation,
    level_crossing_rail_sprite_id, rail_tile_is_signals, rail_track_base_color,
    rail_trackbits_for_render, road_bits_for_render, road_flat_sprite_color,
    road_flat_sprite_index, road_tile_has_tram_track,
};

/// Sesgo en la componente Z de **solo** el agua animada (sin sprite `shore_*`).
/// El orden de dibujo usa `(tx+ty)`; el mar al **este/sur** tiene suma mayor y acaba
/// encima del borde costero del vecino NO/NE → sierra y rectángulos azules oscuros.
const FLAT_WATER_LAYER_FRAC: f32 = -0.014;
/// Solape mínimo para ocultar costuras finas entre tiles adyacentes.
const TILE_OVERLAP_SCALE: f32 = 1.002;

fn sloped_or_flat_image(
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

fn spawn_ground_sprite(
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

fn push_water_sprite(
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

fn spawn_coast_debug_label(
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

pub(crate) fn spawn_road_tile(
    commands: &mut Commands,
    map: &Map,
    mw: u32,
    mh: u32,
    assets: &WorldAssets,
    ctx: &TileRenderContext,
    slope_half_ground: f32,
) {
    let tileh = ctx.info.tileh;
    let base_z = ctx.info.base_z;
    let rb = road_bits_for_render(map, ctx.coord, mw, mh);
    let fi = road_flat_sprite_index(tileh, rb);
    let road_half_h = if tileh == 0 {
        ROAD_FLAT_HALF_H[fi]
    } else {
        SLOPE_HALF_H[tileh as usize]
    };
    let road_paint = ctx.tile.map_or(Color::WHITE, |t| {
        road_flat_sprite_color(t.mapt, ctx.kind, t.m7)
    });
    if tileh != 0 {
        spawn_ground_sprite(
            commands,
            assets.grass_slopes[tileh as usize - 1].clone(),
            Color::WHITE,
            ctx,
            slope_half_ground,
        );
    }
    commands.spawn((
        MapVisualLayer,
        Sprite {
            image: assets.road_flat[fi].clone(),
            color: road_paint,
            ..default()
        },
        Transform::from_translation(tile_pos_half(
            ctx.tx_i32(),
            ctx.ty_i32(),
            base_z,
            0.02,
            road_half_h,
        ))
        .with_scale(Vec3::new(TILE_OVERLAP_SCALE, TILE_OVERLAP_SCALE, 1.0)),
    ));

    // Cruce a nivel: carretera + sprite de vía encima (`base_sprites.crossing + rail_axis`).
    if ctx
        .tile
        .is_some_and(|t| is_road_level_crossing(t.mapt, t.m5, ctx.kind))
    {
        let sid = ctx
            .tile
            .map(|t| level_crossing_rail_sprite_id(t.m5))
            .unwrap_or(1370);
        if let Some(img) = assets.rail.get(&sid) {
            let crossing_paint = ctx.tile.map_or(Color::srgb(0.88, 0.88, 0.97), |t| {
                let mut c = rail_track_base_color(t.mapt, TileKind::Rail, t.m5, t.m3);
                if level_crossing_has_rail_reservation(t.m5) {
                    c = c.mix(&Color::srgb(0.95, 0.52, 0.42), 0.26);
                }
                if road_tile_has_tram_track(t.m8) {
                    c = c.mix(&Color::srgb(0.55, 0.88, 0.58), 0.12);
                }
                c
            });
            commands.spawn((
                MapVisualLayer,
                Sprite {
                    image: img.clone(),
                    color: crossing_paint,
                    ..default()
                },
                Transform::from_translation(tile_pos_half(
                    ctx.tx_i32(),
                    ctx.ty_i32(),
                    base_z,
                    0.045,
                    road_half_h,
                )),
            ));
        }
    }
}

pub(crate) fn spawn_rail_tile(
    commands: &mut Commands,
    map: &Map,
    map_dims: (u32, u32),
    assets: &WorldAssets,
    ctx: &TileRenderContext,
    slope_half_ground: f32,
    rail_layers: &mut Vec<u32>,
) {
    let tileh = ctx.info.tileh;
    let base_z = ctx.info.base_z;
    if tileh != 0 {
        spawn_ground_sprite(
            commands,
            assets.grass_slopes[tileh as usize - 1].clone(),
            Color::WHITE,
            ctx,
            slope_half_ground,
        );
    }
    let rail_half_h = if tileh == 0 {
        TILE_HALF_H
    } else {
        SLOPE_HALF_H[tileh as usize]
    };
    collect_rail_sprites(
        rail_trackbits_for_render(map, ctx.coord, map_dims.0, map_dims.1),
        rail_layers,
    );
    let mut rail_paint = ctx.tile.map_or(Color::srgb(0.88, 0.88, 0.97), |t| {
        rail_track_base_color(t.mapt, ctx.kind, t.m5, t.m3)
    });
    if ctx.tile.is_some_and(|t| rail_tile_is_signals(t.m5)) {
        rail_paint = rail_paint.mix(&Color::srgb(0.95, 0.88, 0.55), 0.22);
    }
    for (i, sid) in rail_layers.iter().copied().enumerate() {
        let Some(img) = assets.rail.get(&sid) else {
            continue;
        };
        let z = 0.02 + i as f32 * 0.0004;
        commands.spawn((
            MapVisualLayer,
            Sprite {
                image: img.clone(),
                color: rail_paint,
                ..default()
            },
            Transform::from_translation(tile_pos_half(
                ctx.tx_i32(),
                ctx.ty_i32(),
                base_z,
                z,
                rail_half_h,
            ))
            .with_scale(Vec3::new(TILE_OVERLAP_SCALE, TILE_OVERLAP_SCALE, 1.0)),
        ));
    }
    if let Some(t) = ctx.tile.filter(|t| rail_tile_is_signals(t.m5)) {
        let sig_ids = collect_signal_sprite_ids(t.m2, t.m3, t.m3hi, t.m5);
        for (si, sid) in sig_ids.iter().copied().enumerate() {
            let Some(img) = assets.rail.get(&sid) else {
                continue;
            };
            let z = 0.032 + si as f32 * 0.0015;
            commands.spawn((
                MapVisualLayer,
                Sprite {
                    image: img.clone(),
                    color: Color::WHITE,
                    ..default()
                },
                Transform::from_translation(tile_pos_half(
                    ctx.tx_i32(),
                    ctx.ty_i32(),
                    base_z,
                    z,
                    rail_half_h,
                )),
            ));
        }
    }
}

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
    let spec_idx = house_draw_data_index_for_tile(clean_house_id, ctx.tx_i32(), ctx.ty_i32());
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
    let has_building = crate::sprites::industry_sprite_for_gfx(gfx).is_some();
    let (ground_img, ground_color) = if has_building {
        (
            sloped_or_flat_image(tileh, &assets.rough, &assets.rough_slopes),
            Color::srgb(0.55, 0.50, 0.45),
        )
    } else {
        (
            sloped_or_flat_image(tileh, &assets.grass, &assets.grass_slopes),
            Color::WHITE,
        )
    };
    spawn_ground_sprite(commands, ground_img, ground_color, ctx, slope_half_ground);
    if let Some(s) = crate::sprites::industry_sprite_for_gfx(gfx)
        && let Some(img) = assets.industries.get(&s.sprite_id)
    {
        let pos3 = overlay_pos(
            ctx.iso_pos,
            s.xrel,
            s.yrel,
            s.w,
            s.h,
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

pub(crate) fn spawn_station_tile(
    commands: &mut Commands,
    assets: &WorldAssets,
    ctx: &TileRenderContext,
    slope_half_ground: f32,
) {
    let tileh = ctx.info.tileh;
    if tileh != 0 {
        spawn_ground_sprite(
            commands,
            assets.grass_slopes[tileh as usize - 1].clone(),
            Color::WHITE,
            ctx,
            slope_half_ground,
        );
    }
    let dir = wang_hash(ctx.tx, ctx.ty, 0xCAFE) as usize % assets.station_grounds.len();
    commands.spawn((
        MapVisualLayer,
        Sprite {
            image: assets.station_grounds[dir].clone(),
            color: Color::WHITE,
            ..default()
        },
        Transform::from_translation(tile_pos(ctx.tx_i32(), ctx.ty_i32(), ctx.info.base_z, 0.01))
            .with_scale(Vec3::new(TILE_OVERLAP_SCALE, TILE_OVERLAP_SCALE, 1.0)),
    ));
}

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
        // Base de agua también en costa: los sprites `shore_*` tienen
        // transparencia y en OpenTTD se componen sobre agua.
        push_water_sprite(&mut batches.water, &assets.water, ctx);
        if th != 0 {
            let si = shore_png_index(th);
            // Los sprites de costa `shore_*.png` son 64x31 (half_h fijo),
            // no usan el half_h de pendientes de terreno.
            batches.shore.push((
                Sprite {
                    image: assets.shore[si].clone(),
                    color: Color::WHITE,
                    ..default()
                },
                Transform::from_translation(tile_pos_half(
                    ctx.tx_i32(),
                    ctx.ty_i32(),
                    ctx.info.base_z,
                    0.0,
                    TILE_HALF_H,
                )),
            ));
            if debug_coast {
                let (raw, _) = tile_slope_bits_from_heights(map, ctx.tx, ctx.ty);
                spawn_coast_debug_label(commands, ctx, raw, th, si);
            }
        }
    } else {
        // Agua libre (Clear, Lock, Depot en mapas típicos: Clear).
        push_water_sprite(&mut batches.water, &assets.water, ctx);
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

pub(crate) fn flush_map_batches(commands: &mut Commands, batches: MapSpriteBatches) {
    for (wt, sp, tr) in batches.water {
        commands.spawn((MapVisualLayer, wt, sp, tr));
    }
    for (sp, tr) in batches.shore {
        let mut tr = tr;
        tr.scale = Vec3::new(TILE_OVERLAP_SCALE, TILE_OVERLAP_SCALE, 1.0);
        commands.spawn((MapVisualLayer, sp, tr));
    }
    for (sp, tr) in batches.trees {
        commands.spawn((MapVisualLayer, sp, tr));
    }
}
