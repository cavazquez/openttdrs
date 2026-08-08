//! Dibujo compartido de tablero de puente (rampas y vano).

use bevy::prelude::*;
use openttdrs_core::prelude::*;
use openttdrs_core::{
    BridgeType, bridge_above_axis_from_mapt, bridge_type_from_m6, calc_bridge_piece,
    rail_tile_has_pbs_reservation, rail_type_from_tile,
};

use crate::iso::{HEIGHT_PX, TILE_HALF_H, remap_tile_offset, tile_pos_half, tile_slope_and_min_z};
use crate::render::catenary_newgrf::catenary_sprite_colored;
use crate::render::{
    MapVisualLayer, NewGrfAction5SpriteCache, NewGrfCatenarySpriteCache, TileRenderContext,
    WorldAssets,
};
use crate::sprites::{
    RAIL_SPRITE_TRACK_X, RAIL_SPRITE_TRACK_Y, bridge_deck_sprite_ids, bridge_sprite_meta,
    bridge_structure_palette, catenary_sprite_color, catenary_tile_location_group,
    collect_catenary_bridge_draws, wooden_bridge_ramp_sprite_id,
};

const DECK_LAYER_FRAC: f32 = 0.08;
/// Vía sobre tablero (`DrawBridgeMiddle`: overlay entre psid\[0] y psid\[1]).
const RAIL_ON_BRIDGE_LAYER_FRAC: f32 = 0.084;
const FRONT_LAYER_FRAC: f32 = 0.088;
const PILLAR_BACK_LAYER_FRAC: f32 = 0.074;
const PILLAR_LAYER_FRAC: f32 = 0.075;
const BRIDGE_Z_START: f32 = 3.0;
const TILE_HEIGHT_PX: f32 = 8.0;

pub(crate) struct BridgeSpanInfo {
    pub deck_z: u8,
    pub rail: bool,
    /// Reserva PBS en cualquiera de las rampas ferroviarias del puente.
    pub pbs_reserved: bool,
    pub bridge_type: BridgeType,
    pub axis: usize,
    pub piece: openttdrs_core::BridgePiece,
    /// Teselas del vano entre rampas (sin contar rampas), para catenaria.
    pub middle_length: u32,
    /// Índice 1-based desde el norte en el vano (0 = rampa).
    pub middle_num: u32,
    /// ¿La rampa norte es vía eléctrica?
    pub electric: bool,
    /// Tipo de vía en la rampa norte (para Action5 bridge decks).
    pub rail_type: openttdrs_core::RailType,
}

fn ramp_tile(tile: Tile) -> bool {
    tile.is_tunnel_bridge_tile() && tile.m5 & 0x80 != 0
}

fn bridge_deck_z(ramp_tileh: u8, ramp_min_z: u8, axis: usize) -> u8 {
    let aligned = if axis == 0 {
        ramp_tileh == 12 || ramp_tileh == 3
    } else {
        ramp_tileh == 9 || ramp_tileh == 6
    };
    if ramp_tileh == 0 || aligned {
        return ramp_min_z.saturating_add(1);
    }
    let one_corner = matches!(ramp_tileh, 1 | 2 | 4 | 8);
    ramp_min_z.saturating_add(if one_corner { 1 } else { 2 })
}

fn axis_step(axis: usize) -> (i32, i32) {
    if axis == 0 { (1, 0) } else { (0, 1) }
}

fn tile_dist(a: TileCoord, b: TileCoord, axis_y: bool) -> u32 {
    if axis_y {
        a.y.abs_diff(b.y) + 1
    } else {
        a.x.abs_diff(b.x) + 1
    }
}

fn find_ramp_along(
    map: &Map,
    start: TileCoord,
    dx: i32,
    dy: i32,
    dims: (u32, u32),
    include_start: bool,
) -> Option<TileCoord> {
    let mut x = start.x;
    let mut y = start.y;
    if !include_start {
        x += dx;
        y += dy;
    }
    loop {
        if x < 0 || y < 0 || x >= dims.0 as i32 || y >= dims.1 as i32 {
            return None;
        }
        let c = TileCoord::new(x, y);
        let t = map.get(c)?;
        if ramp_tile(t) {
            return Some(c);
        }
        bridge_above_axis_from_mapt(t.mapt)?;
        x += dx;
        y += dy;
    }
}

/// Rampas en cada extremo del puente vistas desde `coord`.
fn bridge_ramp_ends(
    map: &Map,
    coord: TileCoord,
    dx: i32,
    dy: i32,
    dims: (u32, u32),
) -> Option<(TileCoord, TileCoord)> {
    let neg = find_ramp_along(map, coord, -dx, -dy, dims, false)
        .or_else(|| map.get(coord).filter(|t| ramp_tile(*t)).map(|_| coord))?;
    let pos = find_ramp_along(map, coord, dx, dy, dims, false)
        .or_else(|| map.get(coord).filter(|t| ramp_tile(*t)).map(|_| coord))?;
    Some((neg, pos))
}

fn bridge_ramp_ends_valid(
    map: &Map,
    coord: TileCoord,
    dx: i32,
    dy: i32,
    dims: (u32, u32),
) -> Option<(TileCoord, TileCoord)> {
    let (neg, pos) = bridge_ramp_ends(map, coord, dx, dy, dims)?;
    if neg == pos {
        return None;
    }
    Some((neg, pos))
}

fn detect_bridge_axis_y(map: &Map, coord: TileCoord, dims: (u32, u32)) -> Option<bool> {
    let tile = map.get(coord)?;
    if let Some(axis_y) = bridge_above_axis_from_mapt(tile.mapt) {
        return Some(axis_y);
    }
    if !ramp_tile(tile) {
        return None;
    }
    for axis_y in [false, true] {
        let (dx, dy) = axis_step(usize::from(axis_y));
        if bridge_ramp_ends_valid(map, coord, dx, dy, dims).is_some() {
            return Some(axis_y);
        }
    }
    None
}

/// Resuelve rampas, tipo y pieza de puente para una tesela (rampa o vano).
pub(crate) fn bridge_span_at(
    map: &Map,
    coord: TileCoord,
    dims: (u32, u32),
) -> Option<BridgeSpanInfo> {
    let tile = map.get(coord)?;
    let axis_y = detect_bridge_axis_y(map, coord, dims)?;
    let axis = usize::from(axis_y);
    let (dx, dy) = axis_step(axis);

    let (ramp_neg, ramp_pos) = bridge_ramp_ends_valid(map, coord, dx, dy, dims)?;

    let north = if axis_y {
        if ramp_neg.y <= ramp_pos.y {
            ramp_neg
        } else {
            ramp_pos
        }
    } else if ramp_neg.x <= ramp_pos.x {
        ramp_neg
    } else {
        ramp_pos
    };
    let south = if north == ramp_neg {
        ramp_pos
    } else {
        ramp_neg
    };

    let ramp_ref = if ramp_tile(tile) { coord } else { south };
    let ramp_tile_ref = map.get(ramp_ref)?;
    let (tileh, min_z) = tile_slope_and_min_z(map, ramp_ref.x as u32, ramp_ref.y as u32);
    let deck_z = bridge_deck_z(tileh, min_z, axis);
    let rail = (ramp_tile_ref.m5 >> 2) & 0x3 == 0;
    // La reserva del port queda asociada a la rampa recorrida por el tren. Un
    // puente se dibuja desde cualquiera de sus teselas, por lo que consultamos
    // ambos extremos y propagamos el estado a todo el tablero.
    let pbs_reserved = rail
        && [north, south].into_iter().any(|ramp| {
            map.get(ramp)
                .is_some_and(|ramp_tile| rail_tile_has_pbs_reservation(ramp_tile.m2_hi))
        });
    let bridge_type = bridge_type_from_m6(ramp_tile_ref.m6);

    let north_len = tile_dist(coord, north, axis_y);
    let south_len = tile_dist(coord, south, axis_y);
    let piece = calc_bridge_piece(north_len, south_len);
    let total = tile_dist(north, south, axis_y);
    let middle_length = total.saturating_sub(2);
    let on_ramp = ramp_tile(tile);
    let middle_num = if on_ramp || middle_length == 0 {
        0
    } else {
        north_len.saturating_sub(1).min(middle_length).max(1)
    };
    let rail_type = map
        .get(north)
        .map(rail_type_from_tile)
        .unwrap_or(openttdrs_core::RailType::Rail);
    let electric = rail && rail_type.has_catenary();

    Some(BridgeSpanInfo {
        deck_z,
        rail,
        pbs_reserved,
        bridge_type,
        axis,
        piece,
        middle_length,
        middle_num,
        electric,
        rail_type,
    })
}

/// El overlay explícito garantiza que la vía llegue hasta la casilla vecina;
/// el sprite de madera conserva además el detalle de riel de la cabeza.
fn bridge_draws_separate_rail_overlay(_bridge_type: BridgeType, _on_ramp: bool) -> bool {
    true
}

fn bridge_rail_track_sprite(axis: usize) -> u32 {
    if axis == 0 {
        RAIL_SPRITE_TRACK_X
    } else {
        RAIL_SPRITE_TRACK_Y
    }
}

fn spawn_bridge_catenary(
    commands: &mut Commands,
    assets: &WorldAssets,
    ctx: &TileRenderContext,
    span: &BridgeSpanInfo,
    catenary_newgrf: &[Option<openttdrs_core::DecodedSprite>],
    mut catenary_sprites: Option<&mut NewGrfCatenarySpriteCache>,
    mut images: Option<&mut Assets<Image>>,
) {
    let tlg = catenary_tile_location_group(ctx.tx_i32(), ctx.ty_i32());
    let mut draws = Vec::new();
    collect_catenary_bridge_draws(
        span.axis == 0,
        span.middle_num,
        span.middle_length,
        tlg,
        &mut draws,
    );
    let tint = catenary_sprite_color();
    for draw in draws {
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
        commands.spawn((
            MapVisualLayer,
            ctx.map_tile_chunk(),
            sprite,
            Transform::from_translation(
                tile_pos_half(
                    ctx.tx_i32(),
                    ctx.ty_i32(),
                    span.deck_z,
                    draw.z_layer,
                    TILE_HALF_H,
                ) + Vec3::new(off.x, off.y, 0.0),
            ),
        ));
    }
}

fn spawn_bridge_rail_overlay(
    commands: &mut Commands,
    assets: &WorldAssets,
    ctx: &TileRenderContext,
    span: &BridgeSpanInfo,
    show_pbs_reservations: bool,
) {
    let sid = bridge_rail_track_sprite(span.axis);
    let Some(img) = assets.rail.get(&sid) else {
        return;
    };
    commands.spawn((
        MapVisualLayer,
        ctx.map_tile_chunk(),
        img.sprite_colored(if show_pbs_reservations && span.pbs_reserved {
            Color::srgb(0.88, 0.88, 0.97).mix(&Color::srgb(0.95, 0.52, 0.42), 0.26)
        } else {
            Color::srgb(0.88, 0.88, 0.97)
        }),
        Transform::from_translation(tile_pos_half(
            ctx.tx_i32(),
            ctx.ty_i32(),
            span.deck_z,
            RAIL_ON_BRIDGE_LAYER_FRAC,
            TILE_HALF_H,
        )),
    ));
}

fn pillar_ground_px(tileh: u8, base_z: u8, axis: usize) -> (f32, f32) {
    let corner = |bit: u8| f32::from(base_z) + f32::from((tileh >> bit) & 1);
    let (w, s, e, n) = (corner(0), corner(1), corner(2), corner(3));
    if axis == 0 {
        (e.max(s) * TILE_HEIGHT_PX, n.max(w) * TILE_HEIGHT_PX)
    } else {
        (s.max(w) * TILE_HEIGHT_PX, n.max(e) * TILE_HEIGHT_PX)
    }
}

#[allow(clippy::too_many_arguments)]
fn spawn_layer(
    commands: &mut Commands,
    assets: &WorldAssets,
    ctx: &TileRenderContext,
    sprite_id: u32,
    shift: Vec2,
    z_px: f32,
    layer: f32,
    deck_z: u8,
    bridge_type: BridgeType,
) {
    if sprite_id == 0 {
        return;
    }
    use crate::sprites::{TransparencyOption, sprite_color};
    let palette = bridge_structure_palette(bridge_type);
    let mut sprite = if let Some(handle) = assets.bridge_palettes.handle(sprite_id, palette) {
        Sprite {
            image: handle.clone(),
            ..default()
        }
    } else if let Some(img) = assets.bridge_sprite(sprite_id) {
        img.sprite()
    } else {
        return;
    };
    sprite.color = sprite_color(TransparencyOption::Bridges);
    let (w, h, xrel, yrel) = bridge_sprite_meta(sprite_id).unwrap_or((64.0, 32.0, -32.0, -16.0));
    let pos = Vec3::new(
        ctx.iso_pos.x + shift.x + xrel + w / 2.0,
        ctx.iso_pos.y + shift.y - yrel - h / 2.0 + z_px,
        (ctx.tx_i32() + ctx.ty_i32()) as f32 * 0.01 + f32::from(deck_z) * 0.001 + layer,
    );
    commands.spawn((
        MapVisualLayer,
        ctx.map_tile_chunk(),
        sprite,
        Transform::from_translation(pos),
    ));
}

/// Dibuja tablero + barandilla + pilares para rampa o vano.
#[allow(clippy::too_many_arguments)]
pub(crate) fn spawn_bridge_deck(
    commands: &mut Commands,
    assets: &WorldAssets,
    ctx: &TileRenderContext,
    span: &BridgeSpanInfo,
    draw_pillars: bool,
    show_pbs_reservations: bool,
    catenary_newgrf: &[Option<openttdrs_core::DecodedSprite>],
    catenary_sprites: Option<&mut NewGrfCatenarySpriteCache>,
    bridge_decks_newgrf: &[Option<openttdrs_core::DecodedSprite>],
    mut action5_sprites: Option<&mut NewGrfAction5SpriteCache>,
    mut images: Option<&mut Assets<Image>>,
) {
    use crate::sprites::{TransparencyOption, is_hidden};
    if is_hidden(TransparencyOption::Bridges) {
        // Vía sobre el vano sigue visible; solo se oculta la estructura.
        if span.rail
            && bridge_draws_separate_rail_overlay(span.bridge_type, ctx.tile.is_some_and(ramp_tile))
        {
            spawn_bridge_rail_overlay(commands, assets, ctx, span, show_pbs_reservations);
        }
        return;
    }
    let ids = bridge_deck_sprite_ids(span.bridge_type, span.piece);
    let on_ramp = ctx.tile.is_some_and(ramp_tile);
    let ramp_id = ctx
        .tile
        .filter(|_| span.bridge_type == BridgeType::Wooden)
        .map(|tile| wooden_bridge_ramp_sprite_id(span.rail, ctx.info.tileh, tile.m5));
    let rear_id = ramp_id.unwrap_or_else(|| {
        // En el vano de madera dibujamos la vía como overlay para evitar que
        // el riel integrado del sprite de estructura desaparezca según el
        // atlas cargado.
        let use_road_deck = span.bridge_type == BridgeType::Wooden && span.rail;
        ids.rear(!use_road_deck && span.rail, span.axis)
    });
    let front_id = if on_ramp { 0 } else { ids.front[span.axis] };
    let pillar_id = if on_ramp { 0 } else { ids.pillar[span.axis] };
    let z_draw_px = f32::from(span.deck_z) * HEIGHT_PX - BRIDGE_Z_START;

    let front_shift = if span.axis == 0 {
        remap_tile_offset(0.0, 12.0, 0.0) * 0.5
    } else {
        remap_tile_offset(12.0, 0.0, 0.0) * 0.5
    };
    let back_shift = if span.axis == 0 {
        remap_tile_offset(0.0, 3.0, 0.0) * 0.5
    } else {
        remap_tile_offset(3.0, 0.0, 0.0) * 0.5
    };

    spawn_layer(
        commands,
        assets,
        ctx,
        rear_id,
        Vec2::ZERO,
        z_draw_px,
        DECK_LAYER_FRAC,
        span.deck_z,
        span.bridge_type,
    );
    // Superficie Action5 `0x1B` (tablero NewGRF sobre la estructura OpenGFX).
    if !on_ramp
        && let Some(slot) =
            openttdrs_core::bridge_decks_action5_slot(span.rail, span.rail_type, span.axis)
        && let (Some(cache), Some(images)) = (action5_sprites.as_mut(), images.as_mut())
        && let Some(sprite) = cache.sprite_colored(
            openttdrs_core::ACTION5_TYPE_BRIDGE_DECKS,
            slot,
            bridge_decks_newgrf,
            Color::WHITE,
            images,
        )
    {
        commands.spawn((
            MapVisualLayer,
            ctx.map_tile_chunk(),
            sprite,
            Transform::from_translation(tile_pos_half(
                ctx.tx_i32(),
                ctx.ty_i32(),
                span.deck_z,
                DECK_LAYER_FRAC + 0.001,
                TILE_HALF_H,
            )),
        ));
    }
    if span.rail && bridge_draws_separate_rail_overlay(span.bridge_type, on_ramp) {
        spawn_bridge_rail_overlay(commands, assets, ctx, span, show_pbs_reservations);
    }
    // Overlay de tranvía sobre tablero de puente de carretera.
    if !span.rail
        && let Some(tile) = ctx.tile
        && let Some(tfi) = crate::sprites::tram_flat_sprite_index(0, tile.m3)
        && let Some(tram) = assets.tram_flat.get(tfi)
    {
        commands.spawn((
            MapVisualLayer,
            ctx.map_tile_chunk(),
            tram.sprite(),
            Transform::from_translation(tile_pos_half(
                ctx.tx_i32(),
                ctx.ty_i32(),
                span.deck_z,
                RAIL_ON_BRIDGE_LAYER_FRAC,
                TILE_HALF_H,
            )),
        ));
    }
    if span.electric && span.middle_num > 0 && span.middle_length > 0 {
        spawn_bridge_catenary(
            commands,
            assets,
            ctx,
            span,
            catenary_newgrf,
            catenary_sprites,
            images,
        );
    }
    spawn_layer(
        commands,
        assets,
        ctx,
        front_id,
        front_shift,
        z_draw_px,
        FRONT_LAYER_FRAC,
        span.deck_z,
        span.bridge_type,
    );

    if !draw_pillars {
        return;
    }
    let Some(tile) = ctx.tile else {
        return;
    };
    if tile.kind == TileKind::Void {
        return;
    }
    let (front_ground_px, back_ground_px) =
        pillar_ground_px(ctx.info.tileh, ctx.info.base_z, span.axis);
    if pillar_id != 0 && bridge_sprite_meta(pillar_id).is_some() {
        let mut cur_z = z_draw_px;
        while cur_z >= front_ground_px {
            spawn_layer(
                commands,
                assets,
                ctx,
                pillar_id,
                front_shift,
                cur_z,
                PILLAR_LAYER_FRAC,
                span.deck_z,
                span.bridge_type,
            );
            cur_z -= TILE_HEIGHT_PX;
        }
        let back_top_px = z_draw_px - 2.0 * TILE_HEIGHT_PX;
        if back_ground_px <= back_top_px {
            let mut cur_z = back_top_px;
            while cur_z >= back_ground_px {
                spawn_layer(
                    commands,
                    assets,
                    ctx,
                    pillar_id,
                    back_shift,
                    cur_z,
                    PILLAR_BACK_LAYER_FRAC,
                    span.deck_z,
                    span.bridge_type,
                );
                cur_z -= TILE_HEIGHT_PX;
            }
        }
    }
}
