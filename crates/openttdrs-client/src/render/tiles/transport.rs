use bevy::prelude::*;
use openttdrs_core::prelude::*;
use openttdrs_core::{Climate, RoadTypeDef, bridge_above_axis_from_mapt};

use super::{TRAM_OVERLAY_LAYER_FRAC, spawn_ground_sprite, spawn_rail_foundation};
use crate::iso::{SLOPE_HALF_H, TILE_HALF_H, overlay_pos, remap_tile_offset, tile_pos_half};
use crate::render::catenary_newgrf::catenary_sprite_colored;
use crate::render::road_newgrf::{
    NewGrfRoadSpriteCache, newgrf_road_def_for_tile, newgrf_tram_def_for_tile,
    road_newgrf_view_index,
};
use crate::render::{MapVisualLayer, TileRenderContext, WorldAssets};
use crate::sprites::{
    RAIL_GROUND_SNOW_OR_DESERT, ROAD_FLAT_HALF_H, ROAD_STREETLIGHT_META, ROADSIDE_LAMPS,
    ROADSIDE_TREE_META, ROADSIDE_TREES, TRACK_FENCE_META, catenary_sprite_color,
    collect_catenary_pylons_from_map, collect_catenary_sprites_from_map,
    collect_rail_sprites_for_type, collect_signal_sprite_draws, is_road_level_crossing,
    is_typed_rail_track_sprite, level_crossing_has_rail_reservation,
    level_crossing_rail_sprite_id_for_type, rail_ghost_overlay_offset,
    rail_tile_has_pbs_reservation, rail_tile_is_signals, rail_track_base_color,
    rail_trackbits_for_render, road_bits_for_render, road_flat_sprite_color,
    road_flat_sprite_index, road_tile_roadside, road_tile_snow_or_desert,
    road_tile_tram_visual_active, roadside_is_paved, signal_screen_anchor_for_side,
    signal_screen_position_for_side, track_fence_draws_for_tile, tram_flat_sprite_index,
};

#[allow(clippy::too_many_arguments)]
pub(crate) fn spawn_road_tile(
    commands: &mut Commands,
    map: &Map,
    mw: u32,
    mh: u32,
    assets: &WorldAssets,
    ctx: &TileRenderContext,
    slope_half_ground: f32,
    climate: Climate,
    show_pbs_reservations: bool,
    show_full_detail: bool,
    road_catalog: &[RoadTypeDef],
    mut road_sprites: Option<&mut NewGrfRoadSpriteCache>,
    mut images: Option<&mut Assets<Image>>,
    newgrf_stack: &[openttdrs_core::NewGrfEntry],
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
        // Ártico: tinte nieve suave; el suelo hierba/nieve lo decide `m5` vía land.rs.
        if climate.uses_snow_ground() {
            Color::srgb(0.82, 0.88, 0.98)
        } else {
            road_flat_sprite_color(t.mapt, ctx.kind, t.m7)
        }
    });
    // `GetRoadGroundSprite`: acera pavimentada (Roadside >= Paved) usa el set
    // 1313..1331 salvo nieve/desierto, que mantiene el set sobre pasto + tinte.
    let roadside = ctx.tile.and_then(|t| road_tile_roadside(t.m5, t.m6));
    let snow_or_desert = ctx
        .tile
        .is_some_and(|t| road_tile_snow_or_desert(t.mapt, ctx.kind, t.m7))
        || climate.uses_snow_ground();
    let paved = roadside.is_some_and(roadside_is_paved) && !snow_or_desert;
    if tileh != 0 {
        spawn_ground_sprite(
            commands,
            &assets.grass_slopes[tileh as usize - 1],
            Color::WHITE,
            ctx,
            slope_half_ground,
        );
    }

    // NewGRF: sustituir el sprite de suelo road por la vista OpenGFX
    // (`road_flat_sprite_index`, incl. pendientes 11–14).
    let mut used_newgrf = false;
    let view_idx = road_newgrf_view_index(tileh, rb);
    if let Some(tile) = ctx.tile
        && let Some(def) = newgrf_road_def_for_tile(road_catalog, tile)
        && let Some(view) = def.newgrf_view(view_idx)
        && let (Some(cache), Some(images)) = (road_sprites.as_mut(), images.as_mut())
    {
        let mut a2 = openttdrs_core::action2_eval_ctx_for_road_tile(
            map,
            tile,
            ctx.coord,
            climate,
            def.newgrf_type_tables.as_ref(),
            road_catalog,
        );
        a2.set_grf_params(openttdrs_core::stack_params_for_grfid(
            newgrf_stack,
            def.newgrf_grfid,
        ));
        if let Some(handle) = cache.handle_for_runtime(def, view_idx, &mut a2, images) {
            let pos3 = if tileh == 0 {
                overlay_pos(
                    ctx.iso_pos,
                    f32::from(view.x_offs),
                    f32::from(view.y_offs),
                    f32::from(view.width),
                    f32::from(view.height),
                    base_z,
                    0.02,
                    ctx.tx_i32(),
                    ctx.ty_i32(),
                )
            } else {
                tile_pos_half(ctx.tx_i32(), ctx.ty_i32(), base_z, 0.02, road_half_h)
            };
            commands.spawn((
                MapVisualLayer,
                ctx.map_tile_chunk(),
                Sprite {
                    image: handle,
                    color: Color::WHITE,
                    ..default()
                },
                Transform::from_translation(pos3),
            ));
            used_newgrf = true;
        }
    }

    if !used_newgrf {
        let road_set = if paved {
            &assets.road_paved
        } else {
            &assets.road_flat
        };
        commands.spawn((
            MapVisualLayer,
            ctx.map_tile_chunk(),
            road_set[fi].sprite_colored(road_paint),
            Transform::from_translation(tile_pos_half(
                ctx.tx_i32(),
                ctx.ty_i32(),
                base_z,
                0.02,
                road_half_h,
            )),
        ));
    }

    if let Some(tfi) = ctx.tile.and_then(|t| tram_flat_sprite_index(tileh, t.m3)) {
        let tram_half_h = if tileh == 0 {
            ROAD_FLAT_HALF_H[tfi]
        } else {
            SLOPE_HALF_H[tileh as usize]
        };
        let mut used_tram_newgrf = false;
        if let Some(tile) = ctx.tile
            && let Some(def) = newgrf_tram_def_for_tile(road_catalog, tile)
            && let Some(view) = def.newgrf_view(tfi)
            && let (Some(cache), Some(images)) = (road_sprites.as_mut(), images.as_mut())
        {
            let mut a2 = openttdrs_core::action2_eval_ctx_for_road_tile(
                map,
                tile,
                ctx.coord,
                climate,
                def.newgrf_type_tables.as_ref(),
                road_catalog,
            );
            a2.set_grf_params(openttdrs_core::stack_params_for_grfid(
                newgrf_stack,
                def.newgrf_grfid,
            ));
            if let Some(handle) = cache.handle_for_runtime(def, tfi, &mut a2, images) {
                let pos3 = if tileh == 0 {
                    overlay_pos(
                        ctx.iso_pos,
                        f32::from(view.x_offs),
                        f32::from(view.y_offs),
                        f32::from(view.width),
                        f32::from(view.height),
                        base_z,
                        TRAM_OVERLAY_LAYER_FRAC,
                        ctx.tx_i32(),
                        ctx.ty_i32(),
                    )
                } else {
                    tile_pos_half(
                        ctx.tx_i32(),
                        ctx.ty_i32(),
                        base_z,
                        TRAM_OVERLAY_LAYER_FRAC,
                        tram_half_h,
                    )
                };
                commands.spawn((
                    MapVisualLayer,
                    ctx.map_tile_chunk(),
                    Sprite {
                        image: handle,
                        color: Color::WHITE,
                        ..default()
                    },
                    Transform::from_translation(pos3),
                ));
                used_tram_newgrf = true;
            }
        }
        if !used_tram_newgrf {
            commands.spawn((
                MapVisualLayer,
                ctx.map_tile_chunk(),
                assets.tram_flat[tfi].sprite(),
                Transform::from_translation(tile_pos_half(
                    ctx.tx_i32(),
                    ctx.ty_i32(),
                    base_z,
                    TRAM_OVERLAY_LAYER_FRAC,
                    tram_half_h,
                )),
            ));
        }
    }

    // `Roadside::StreetLights` (3): faroles de `_roadside_lamps` en sus
    // subcoordenadas de mundo. Igual que upstream, solo con 2+ road bits
    // y `FullDetail` activo.
    if show_full_detail && roadside == Some(3) && rb.count_ones() > 1 {
        for &(lamp, dx, dy) in ROADSIDE_LAMPS[usize::from(rb & 0xF)] {
            let (w, h, xrel, yrel) = ROAD_STREETLIGHT_META[lamp];
            let off = remap_tile_offset(dx, dy, 0.0) * 0.5;
            let pos3 = overlay_pos(
                Vec2::new(ctx.iso_pos.x + off.x, ctx.iso_pos.y + off.y),
                xrel,
                yrel,
                w,
                h,
                base_z,
                0.2,
                ctx.tx_i32(),
                ctx.ty_i32(),
            );
            commands.spawn((
                MapVisualLayer,
                ctx.map_tile_chunk(),
                assets.road_streetlights[lamp].sprite(),
                Transform::from_translation(pos3),
            ));
        }
    }

    // `Roadside::Trees` (5): árboles de `_roadside_trees` (sprite 0x1212).
    if show_full_detail && roadside == Some(5) && rb.count_ones() > 1 {
        let (w, h, xrel, yrel) = ROADSIDE_TREE_META;
        for &(dx, dy) in ROADSIDE_TREES[usize::from(rb & 0xF)] {
            let off = remap_tile_offset(dx, dy, 0.0) * 0.5;
            let pos3 = overlay_pos(
                Vec2::new(ctx.iso_pos.x + off.x, ctx.iso_pos.y + off.y),
                xrel,
                yrel,
                w,
                h,
                base_z,
                0.25,
                ctx.tx_i32(),
                ctx.ty_i32(),
            );
            commands.spawn((
                MapVisualLayer,
                ctx.map_tile_chunk(),
                assets.roadside_tree.sprite(),
                Transform::from_translation(pos3),
            ));
        }
    }

    // Cruce a nivel: carretera + sprite de vía encima (`base_sprites.crossing + rail_axis`).
    if ctx
        .tile
        .is_some_and(|t| is_road_level_crossing(t.mapt, t.m5, ctx.kind))
    {
        let sid = ctx
            .tile
            .map(|t| {
                level_crossing_rail_sprite_id_for_type(t.m5, openttdrs_core::rail_type_from_tile(t))
            })
            .unwrap_or(1370);
        if let Some(img) = assets.rail.get(&sid) {
            let crossing_paint = ctx.tile.map_or(Color::srgb(0.88, 0.88, 0.97), |t| {
                let mut c = rail_track_base_color(t.mapt, TileKind::Rail, t.m5, t.m3);
                // Electric sigue con tinte; mono/maglev usan sprite tipado.
                if openttdrs_core::rail_type_from_tile(t) == openttdrs_core::RailType::Electric {
                    c = c.mix(&Color::srgb(0.55, 0.75, 0.95), 0.18);
                }
                if show_pbs_reservations && level_crossing_has_rail_reservation(t.m5) {
                    c = c.mix(&Color::srgb(0.95, 0.52, 0.42), 0.26);
                }
                if road_tile_tram_visual_active(t.m3, t.m8) {
                    c = c.mix(&Color::srgb(0.55, 0.88, 0.58), 0.12);
                }
                c
            });
            commands.spawn((
                MapVisualLayer,
                ctx.map_tile_chunk(),
                img.sprite_colored(crossing_paint),
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

#[allow(clippy::too_many_arguments)]
pub(crate) fn spawn_rail_tile(
    commands: &mut Commands,
    map: &Map,
    map_dims: (u32, u32),
    assets: &WorldAssets,
    ctx: &TileRenderContext,
    slope_half_ground: f32,
    rail_layers: &mut Vec<u32>,
    climate: Climate,
    show_pbs_reservations: bool,
    show_full_detail: bool,
    signals_on_right: bool,
    catenary_newgrf: &[Option<openttdrs_core::DecodedSprite>],
    mut catenary_sprites: Option<&mut crate::render::NewGrfCatenarySpriteCache>,
    rail_signal_newgrf: &[Option<openttdrs_core::RailSignalSpriteSpec>],
    mut signal_sprites: Option<&mut crate::render::NewGrfSignalSpriteCache>,
    mut images: Option<&mut Assets<Image>>,
    calendar_date: u32,
    newgrf_stack: &[openttdrs_core::NewGrfEntry],
) {
    let tileh = ctx.info.tileh;
    // Vano con puente encima: la vía la dibuja `spawn_bridge_deck` a la altura del tablero.
    if ctx
        .tile
        .is_some_and(|t| bridge_above_axis_from_mapt(t.mapt).is_some())
    {
        return;
    }
    if tileh != 0 {
        spawn_ground_sprite(
            commands,
            &assets.grass_slopes[tileh as usize - 1],
            Color::WHITE,
            ctx,
            slope_half_ground,
        );
    }
    let tb = ctx.tile.map_or(0, |t| t.m5 & 0x3F);
    let snow_ground = ctx
        .tile
        .is_some_and(|t| (t.m3 & 0x0F) == RAIL_GROUND_SNOW_OR_DESERT)
        || climate.uses_snow_ground();
    let rail_base_z = spawn_rail_foundation(commands, assets, ctx, tileh, tb);
    let rail_half_h = if tileh == 0 {
        TILE_HALF_H
    } else {
        SLOPE_HALF_H[tileh as usize]
    };
    let rail_type = ctx
        .tile
        .map(openttdrs_core::rail_type_from_tile)
        .unwrap_or_default();
    collect_rail_sprites_for_type(
        rail_trackbits_for_render(map, ctx.coord, map_dims.0, map_dims.1),
        tileh,
        snow_ground,
        rail_type,
        rail_layers,
    );
    let typed_layers = rail_layers
        .iter()
        .any(|&sid| is_typed_rail_track_sprite(sid));
    let mut rail_paint = ctx.tile.map_or(Color::srgb(0.88, 0.88, 0.97), |t| {
        let mut c = rail_track_base_color(t.mapt, ctx.kind, t.m5, t.m3);
        match openttdrs_core::rail_type_from_tile(t) {
            openttdrs_core::RailType::Electric => {
                c = c.mix(&Color::srgb(0.55, 0.75, 0.95), 0.18);
            }
            // Mono/maglev: tinte solo si caímos al sprite clásico (pendiente / sin asset).
            openttdrs_core::RailType::Monorail if !typed_layers => {
                c = c.mix(&Color::srgb(0.75, 0.55, 0.90), 0.22);
            }
            openttdrs_core::RailType::Maglev if !typed_layers => {
                c = c.mix(&Color::srgb(0.45, 0.90, 0.85), 0.22);
            }
            _ => {}
        }
        if show_pbs_reservations && rail_tile_has_pbs_reservation(t.m2_hi) {
            c = c.mix(&Color::srgb(0.95, 0.52, 0.42), 0.26);
        }
        c
    });
    if ctx.tile.is_some_and(|t| rail_tile_is_signals(t.m5)) {
        rail_paint = rail_paint.mix(&Color::srgb(0.95, 0.88, 0.55), 0.22);
    }
    for (i, sid) in rail_layers.iter().copied().enumerate() {
        let Some(img) = assets.rail.get(&sid) else {
            continue;
        };
        let z = 0.02 + i as f32 * 0.0004;
        let offset = rail_ghost_overlay_offset(sid);
        let base = tile_pos_half(ctx.tx_i32(), ctx.ty_i32(), rail_base_z, z, rail_half_h);
        commands.spawn((
            MapVisualLayer,
            ctx.map_tile_chunk(),
            img.sprite_colored(rail_paint),
            Transform::from_translation(base + Vec3::new(offset.x, offset.y, 0.0)),
        ));
    }
    // Catenaria OpenGFX: wires (PCP) + postes PPP; TO_CATENARY vía env.
    if rail_type.has_catenary() {
        let trackbits = rail_trackbits_for_render(map, ctx.coord, map_dims.0, map_dims.1);
        let tint = catenary_sprite_color();
        let mut wires = Vec::new();
        collect_catenary_sprites_from_map(
            map,
            ctx.coord,
            map_dims.0,
            map_dims.1,
            crate::sprites::OTTD_MP_RAIL,
            trackbits,
            tileh,
            &mut wires,
        );
        for (i, sid) in wires.iter().copied().enumerate() {
            let Some(sprite) = catenary_sprite_colored(
                assets,
                sid,
                tint,
                catenary_newgrf,
                catenary_sprites.as_deref_mut(),
                images.as_deref_mut(),
            ) else {
                continue;
            };
            let z = 0.035 + i as f32 * 0.0004;
            let base = tile_pos_half(ctx.tx_i32(), ctx.ty_i32(), rail_base_z, z, rail_half_h);
            commands.spawn((
                MapVisualLayer,
                ctx.map_tile_chunk(),
                sprite,
                Transform::from_translation(base),
            ));
        }
        let mut pylons = Vec::new();
        collect_catenary_pylons_from_map(
            map,
            ctx.coord,
            map_dims.0,
            map_dims.1,
            crate::sprites::OTTD_MP_RAIL,
            trackbits,
            tileh,
            &mut pylons,
        );
        for draw in pylons {
            let Some(sprite) = catenary_sprite_colored(
                assets,
                draw.sprite_id,
                tint,
                catenary_newgrf,
                catenary_sprites.as_deref_mut(),
                images.as_deref_mut(),
            ) else {
                continue;
            };
            let off = remap_tile_offset(draw.tile_dx, draw.tile_dy, 0.0) * 0.5;
            let base = tile_pos_half(
                ctx.tx_i32(),
                ctx.ty_i32(),
                rail_base_z,
                draw.z_layer,
                rail_half_h,
            );
            commands.spawn((
                MapVisualLayer,
                ctx.map_tile_chunk(),
                sprite,
                Transform::from_translation(base + Vec3::new(off.x, off.y, 0.0)),
            ));
        }
    }
    if let Some(t) = ctx.tile.filter(|t| rail_tile_is_signals(t.m5)) {
        let sig_draws = collect_signal_sprite_draws(t.m2, t.m3, t.m3hi, t.m5);
        let rail_type = openttdrs_core::rail_type_from_tile(t);
        let signal_spec = rail_signal_newgrf
            .get(usize::from(rail_type.as_u8()))
            .and_then(Option::as_ref);
        for (si, draw) in sig_draws.iter().copied().enumerate() {
            let custom = if let (Some(spec), Some(cache), Some(images)) = (
                signal_spec,
                signal_sprites.as_deref_mut(),
                images.as_deref_mut(),
            ) {
                let mut action2 = openttdrs_core::action2_eval_ctx_for_rail_tile(
                    map,
                    t,
                    ctx.coord,
                    climate,
                    calendar_date,
                    spec.type_tables.as_ref(),
                );
                action2.set_grf_params(openttdrs_core::stack_params_for_grfid(
                    newgrf_stack,
                    spec.grfid,
                ));
                cache.sprite_for(
                    spec,
                    draw.image,
                    draw.signal_type,
                    draw.variant,
                    draw.green,
                    &mut action2,
                    images,
                )
            } else {
                None
            };
            let (sprite, signal_xy) = if let Some(custom) = custom {
                let anchor = signal_screen_anchor_for_side(
                    ctx.tx_i32(),
                    ctx.ty_i32(),
                    draw.pos,
                    rail_half_h,
                    rail_base_z,
                    signals_on_right,
                );
                (custom.sprite, anchor + custom.center_offset)
            } else {
                let Some(img) = assets.rail.get(&draw.sprite_id) else {
                    continue;
                };
                (
                    img.sprite(),
                    signal_screen_position_for_side(
                        ctx.tx_i32(),
                        ctx.ty_i32(),
                        draw.pos,
                        draw.sprite_id,
                        rail_half_h,
                        rail_base_z,
                        signals_on_right,
                    ),
                )
            };
            // Misma profundidad que el fantasma de colocación (`tile_pos_half`), no z≈0.
            let layer = 0.04 + si as f32 * 0.0015;
            let depth = tile_pos_half(ctx.tx_i32(), ctx.ty_i32(), rail_base_z, layer, rail_half_h);
            commands.spawn((
                MapVisualLayer,
                ctx.map_tile_chunk(),
                sprite,
                Transform::from_translation(Vec3::new(signal_xy.x, signal_xy.y, depth.z)),
            ));
        }
    }

    // `DrawTrackDetails`: cercas de borde (FullDetail).
    if show_full_detail && tileh == 0 {
        let track_bits = ctx.tile.map_or(0, |t| t.m5 & 0x3F);
        let m3hi = ctx.tile.map_or(0, |t| t.m3hi);
        let (w, h, xrel, yrel) = TRACK_FENCE_META;
        for (sprite_i, dx, dy) in track_fence_draws_for_tile(map, ctx.coord, track_bits, m3hi) {
            let Some(img) = assets.track_fences.get(sprite_i) else {
                continue;
            };
            let off = remap_tile_offset(dx, dy, 0.0) * 0.5;
            let pos3 = overlay_pos(
                Vec2::new(ctx.iso_pos.x + off.x, ctx.iso_pos.y + off.y),
                xrel,
                yrel,
                w,
                h,
                rail_base_z,
                0.03,
                ctx.tx_i32(),
                ctx.ty_i32(),
            );
            commands.spawn((
                MapVisualLayer,
                ctx.map_tile_chunk(),
                img.sprite(),
                Transform::from_translation(pos3),
            ));
        }
    }
}
