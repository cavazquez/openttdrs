//! Fantasma de puente: eje según el tramo y sprite distinto en rampas vs vano.

use bevy::prelude::*;
use openttdrs_core::{
    BridgeType, Climate, Map, NewGrfEntry, RoadTramType, RoadType, RoadTypeDef, Tile, TileCoord,
    TileKind, action2_eval_ctx_for_road_tile, calc_bridge_piece, set_road_type_on_tile,
    set_tram_road_type_on_tile, stack_params_for_grfid,
};

use crate::iso::{HEIGHT_PX, iso, overlay_pos, remap_tile_offset, tile_slope_and_min_z};
use crate::render::{
    CatenarySpriteAnchor, NewGrfAction5SpriteCache, NewGrfCatenarySpriteCache,
    NewGrfRoadSpriteCache, bridge_foundation_decision_at, bridge_ramp_catenary_slope,
    bridge_ramp_catenary_world_z_delta, catenary_local_z_delta, catenary_sprite_anchor,
    catenary_sprite_center,
};
use crate::sprites::{
    BridgeDeckSpriteIds, OTTD_MP_RAIL, RAIL_TB_X, RAIL_TB_Y, bridge_deck_sprite_ids,
    bridge_ramp_sprite_id, bridge_sprite_meta, catenary_sprite_atlas_key, catenary_sprite_color,
    catenary_tile_location_group, collect_catenary_bridge_draws,
    collect_catenary_ramp_draws_from_map, foundation_asset_path, foundation_gfx_for_tileh,
};
use crate::ui::toolbar::BuildMenuAction;

use super::BuildGhostPreview;

const DECK_LAYER: f32 = 0.04;
const FRONT_LAYER: f32 = 0.045;
/// La preview conserva la misma secuencia local que el mapa: fundación debajo
/// del suelo efectivo y del tablero. El ordinal evita invertir las dos piezas
/// de una pendiente empinada cuando comparten tesela.
const FOUNDATION_LAYER: f32 = DECK_LAYER - 0.006;
const CUSTOM_BRIDGE_LAYER: f32 = DECK_LAYER + 0.001;
const CUSTOM_OVERLAY_LAYER: f32 = DECK_LAYER + 0.002;
const CUSTOM_CATENARY_BACK_LAYER: f32 = DECK_LAYER + 0.003;
const CUSTOM_CATENARY_FRONT_LAYER: f32 = FRONT_LAYER + 0.002;
const ROTSG_BRIDGE: u8 = 6;
const ROTSG_OVERLAY: u8 = 1;
const ROTSG_CATENARY_FRONT: u8 = 4;
const ROTSG_CATENARY_BACK: u8 = 5;
const BRIDGE_ROAD_OVERLAY_OFFSETS: [usize; 6] = [0, 1, 11, 12, 13, 14];
const BRIDGE_ROAD_CATENARY_BACK_OFFSETS: [usize; 6] = [95, 96, 99, 102, 100, 101];
const BRIDGE_ROAD_CATENARY_FRONT_OFFSETS: [usize; 6] = [97, 98, 103, 106, 104, 105];

pub(crate) struct BridgeSpanPreviewSpawn<'a> {
    pub asset_server: &'a AssetServer,
    pub action: BuildMenuAction,
    pub tiles: &'a [(i32, i32)],
    pub map: &'a Map,
    pub valid: bool,
    pub rail_type: openttdrs_core::RailType,
    pub road_def: Option<&'a RoadTypeDef>,
    pub road_catalog: &'a [RoadTypeDef],
    pub climate: Climate,
    pub newgrf_stack: &'a [NewGrfEntry],
    pub road_sprites: &'a mut NewGrfRoadSpriteCache,
    pub catenary_newgrf: &'a [Option<openttdrs_core::DecodedSprite>],
    pub catenary_sprites: &'a mut NewGrfCatenarySpriteCache,
    pub foundation_newgrf: &'a [Option<openttdrs_core::DecodedSprite>],
    pub action5_sprites: &'a mut NewGrfAction5SpriteCache,
    pub images: &'a mut Assets<Image>,
}

/// Eje Y del puente (vía vertical en mapa) a partir del tramo de teselas.
#[must_use]
pub fn bridge_span_axis_y(tiles: &[(i32, i32)]) -> bool {
    let Some(&(sx, sy)) = tiles.first() else {
        return false;
    };
    let Some(&(ex, ey)) = tiles.last() else {
        return false;
    };
    if sx == ex && sy == ey {
        return false;
    }
    (ex - sx).abs() < (ey - sy).abs()
}

/// Desplazamiento del sprite front (misma lógica que `spawn_bridge_deck`).
fn bridge_front_shift(axis: usize) -> Vec2 {
    if axis == 0 {
        remap_tile_offset(0.0, 12.0, 0.0) * 0.5
    } else {
        remap_tile_offset(12.0, 0.0, 0.0) * 0.5
    }
}

/// Orden canónico de las piezas del puente: norte → sur según el eje del
/// tramo. El drag de la UI puede empezar en cualquiera de las dos cabezas,
/// pero `calc_bridge_piece` y las tablas de OpenTTD siempre reciben ese orden.
fn bridge_preview_render_order(tiles: &[(i32, i32)]) -> Vec<(i32, i32)> {
    let axis_y = bridge_span_axis_y(tiles);
    let mut ordered = tiles.to_vec();
    ordered.sort_unstable_by_key(|&(px, py)| if axis_y { py } else { px });
    ordered
}

/// Misma selección de altura que `DrawTile_TunnelBridge::GetBridgeDeckZ`.
/// El vano usa la cota del tablero de la cabeza sur, no la cota local del
/// agua o del terreno que todavía ocupa esa tesela durante el drag.
fn bridge_preview_deck_z(ramp_tileh: u8, ramp_min_z: u8, axis: usize) -> u8 {
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

/// Índice que `DrawBridgeRoadBits` entrega al grupo `ROTSG_BRIDGE`.
fn bridge_preview_road_sprite_offset(axis_y: bool, index: usize, total: usize, tileh: u8) -> usize {
    let axis = usize::from(axis_y);
    if index != 0 && index + 1 < total {
        return axis ^ 1;
    }
    let south_dir = u8::from(!axis_y) + 1;
    let dir = if index == 0 { south_dir } else { 2 ^ south_dir };
    if tileh == 0 {
        usize::from(dir) + 2
    } else {
        usize::from(dir.wrapping_add(1) & 1)
    }
}

/// Crea la vista de la rampa sur que todavía no existe durante la preview.
/// El renderer del mapa lee el tipo desde la rampa materializada; aquí se
/// escribe el tipo seleccionado sobre una copia para que las variables
/// `RoadTypeSpriteGroup` vean el mismo contrato sin modificar el mapa.
fn bridge_preview_source_tile(
    map: &Map,
    ordered_tiles: &[(i32, i32)],
    def: &RoadTypeDef,
) -> Option<(TileCoord, Tile)> {
    let (px, py) = *ordered_tiles
        .iter()
        .rev()
        .find(|&&(px, py)| map.get(TileCoord::new(px, py)).is_some())?;
    let source_coord = TileCoord::new(px, py);
    let mut source_tile = map.get(source_coord)?;
    source_tile.kind = TileKind::RoadBridge;
    if def.class == RoadTramType::Tram {
        source_tile = set_road_type_on_tile(source_tile, RoadType::from_u8(0x3F));
        source_tile = set_tram_road_type_on_tile(source_tile, Some(def.id));
    } else {
        source_tile = set_road_type_on_tile(source_tile, def.id);
        source_tile = set_tram_road_type_on_tile(source_tile, None);
    }
    Some((source_coord, source_tile))
}

/// Resuelve una vista específica con el mismo contexto de Action2 que usa el
/// mapa, pero conserva la textura en la caché del preview para no recrearla en
/// cada frame del cursor.
#[allow(clippy::too_many_arguments)]
fn custom_bridge_preview_layer(
    def: &RoadTypeDef,
    map: &Map,
    source_coord: TileCoord,
    source_tile: Tile,
    selector: u8,
    view_idx: usize,
    climate: Climate,
    road_catalog: &[RoadTypeDef],
    newgrf_stack: &[NewGrfEntry],
    road_sprites: &mut NewGrfRoadSpriteCache,
    images: &mut Assets<Image>,
    tint: Color,
) -> Option<(Sprite, openttdrs_core::DecodedSprite)> {
    let mut action2 = action2_eval_ctx_for_road_tile(
        map,
        source_tile,
        source_coord,
        climate,
        def.newgrf_type_tables.as_ref(),
        road_catalog,
    );
    action2.set_grf_params(stack_params_for_grfid(newgrf_stack, def.newgrf_grfid));
    let view = def.newgrf_specific_view_runtime(selector, view_idx, &mut action2)?;
    let image = road_sprites
        .handle_for_resolved_specific_view(def, selector, view_idx, None, &action2, &view, images);
    Some((
        Sprite {
            image,
            color: tint,
            ..default()
        },
        view,
    ))
}

/// Fallback vanilla de `GetBridgeRoadCatenary`: ambos sprites pertenecen al
/// bloque Action5 `SPR_TRAMWAY`, pero mantienen cajas NFO distintas.
fn bridge_road_catenary_sprite_ids(offset: usize) -> (u32, u32) {
    let offset = offset.min(5);
    (
        crate::sprites::TRAMWAY_SPRITE_BASE
            + u32::try_from(BRIDGE_ROAD_CATENARY_BACK_OFFSETS[offset]).unwrap_or(0),
        crate::sprites::TRAMWAY_SPRITE_BASE
            + u32::try_from(BRIDGE_ROAD_CATENARY_FRONT_OFFSETS[offset]).unwrap_or(0),
    )
}

#[allow(clippy::too_many_arguments)]
fn spawn_vanilla_bridge_catenary(
    commands: &mut Commands,
    asset_server: &AssetServer,
    px: i32,
    py: i32,
    surface_z: u8,
    layer: f32,
    sprite_id: u32,
    tint: Color,
) {
    let (Some(path), Some(gfx)) = (
        crate::sprites::tramway_sprite_atlas_key(sprite_id),
        crate::sprites::catenary_sprite_gfx(sprite_id),
    ) else {
        return;
    };
    let catenary_tint = crate::sprites::catenary_sprite_color();
    commands.spawn((
        BuildGhostPreview,
        Sprite {
            image: asset_server.load::<Image>(format!("assets/opengfx/tiles/{path}")),
            color: tint.with_alpha(tint.alpha() * catenary_tint.alpha()),
            ..default()
        },
        Transform::from_translation(overlay_pos(
            iso(px, py),
            gfx.x_offs,
            gfx.y_offs,
            gfx.width,
            gfx.height,
            surface_z,
            layer,
            px,
            py,
        ))
        .with_scale(Vec3::splat(1.002)),
    ));
}

/// Carga una pieza de catenaria con la misma prioridad que el renderer del
/// mapa: Action5 activa primero y OpenGFX vanilla como fallback.
fn preview_catenary_sprite(
    asset_server: &AssetServer,
    sprite_id: u32,
    tint: Color,
    catenary_newgrf: &[Option<openttdrs_core::DecodedSprite>],
    catenary_sprites: &mut NewGrfCatenarySpriteCache,
    images: &mut Assets<Image>,
) -> Option<(Sprite, CatenarySpriteAnchor)> {
    let anchor = catenary_sprite_anchor(sprite_id, catenary_newgrf)?;
    let image = if let Some(slot) = openttdrs_core::catenary_action5_local_slot(sprite_id)
        && let Some(decoded) = catenary_newgrf.get(slot).and_then(Option::as_ref)
    {
        catenary_sprites.handle_for(u8::try_from(slot).unwrap_or(u8::MAX), decoded, images)
    } else {
        let path = catenary_sprite_atlas_key(sprite_id)?;
        asset_server.load::<Image>(format!("assets/opengfx/tiles/{path}"))
    };
    Some((
        Sprite {
            image,
            color: tint,
            ..default()
        },
        anchor,
    ))
}

/// Catenaria del vano ferroviario durante la construcción.
///
/// Usa `collect_catenary_bridge_draws`, el mismo port de
/// `DrawRailCatenaryOnBridge` que consume el mapa. Esto conserva el cable
/// corto/largo, la paridad del primer poste y el cambio de eje sin inventar
/// una geometría específica para el cursor.
#[allow(clippy::too_many_arguments)]
fn spawn_rail_bridge_catenary(
    commands: &mut Commands,
    asset_server: &AssetServer,
    px: i32,
    py: i32,
    surface_z: u8,
    axis_y: bool,
    middle_num: usize,
    middle_length: usize,
    tint: Color,
    catenary_newgrf: &[Option<openttdrs_core::DecodedSprite>],
    catenary_sprites: &mut NewGrfCatenarySpriteCache,
    images: &mut Assets<Image>,
) {
    if crate::sprites::catenary_hidden() || middle_length == 0 || middle_num == 0 {
        return;
    }
    let Ok(num) = u32::try_from(middle_num) else {
        return;
    };
    let Ok(length) = u32::try_from(middle_length) else {
        return;
    };
    let mut draws = Vec::new();
    collect_catenary_bridge_draws(
        !axis_y,
        num,
        length,
        catenary_tile_location_group(px, py),
        &mut draws,
    );
    let catenary_tint = catenary_sprite_color();
    let tint = tint.with_alpha(tint.alpha() * catenary_tint.alpha());
    let (wire_ox, wire_oy, wire_oz) = if axis_y {
        (7.0, 0.0, 10.0)
    } else {
        (0.0, 7.0, 10.0)
    };
    for draw in draws {
        let Some((sprite, anchor)) = preview_catenary_sprite(
            asset_server,
            draw.sprite_id,
            tint,
            catenary_newgrf,
            catenary_sprites,
            images,
        ) else {
            continue;
        };
        let (tile_dx, tile_dy, local_z) = if draw.pcp_direction.is_some() {
            (draw.tile_dx - 1.0, draw.tile_dy - 1.0, 0.0)
        } else {
            (wire_ox, wire_oy, wire_oz)
        };
        let position = catenary_sprite_center(
            px,
            py,
            surface_z,
            draw.z_layer,
            tile_dx,
            tile_dy,
            local_z,
            anchor,
        );
        commands.spawn((
            BuildGhostPreview,
            sprite,
            Transform::from_translation(position),
        ));
    }
}

/// Dirección persistida en `m5` para la rampa de la preview. El orden
/// canónico de las teselas es norte → sur; el extremo sur apunta de vuelta al
/// norte, como las dos cabezas que materializa `PlaceRailBridge`.
fn bridge_preview_ramp_direction(axis_y: bool, south_ramp: bool) -> u8 {
    let north_to_south = u8::from(!axis_y) + 1;
    if south_ramp {
        (north_to_south + 2) & 3
    } else {
        north_to_south
    }
}

/// Catenaria de una rampa ferroviaria durante la construcción.
///
/// La rampa aún no existe en `Map`, por eso la decisión de fundación recibe
/// la pendiente/base crudas por separado y el recolector recibe la pendiente
/// efectiva. La colocación de wires, PCP y PPP es la misma que en el renderer
/// materializado; sólo se omite el parent sortable propio del mapa.
#[allow(clippy::too_many_arguments)]
fn spawn_rail_bridge_ramp_catenary(
    commands: &mut Commands,
    asset_server: &AssetServer,
    map: &Map,
    map_dims: (u32, u32),
    px: i32,
    py: i32,
    axis_y: bool,
    south_ramp: bool,
    raw_tileh: u8,
    raw_base_z: u8,
    tint: Color,
    catenary_newgrf: &[Option<openttdrs_core::DecodedSprite>],
    catenary_sprites: &mut NewGrfCatenarySpriteCache,
    images: &mut Assets<Image>,
) {
    if crate::sprites::catenary_hidden() {
        return;
    }
    let direction = bridge_preview_ramp_direction(axis_y, south_ramp);
    let coord = TileCoord::new(px, py);
    let foundation =
        bridge_foundation_decision_at(map, coord, map_dims, raw_tileh, raw_base_z, direction);
    let foundation_tileh = foundation.surface_tileh;
    let foundation_base_z = foundation.surface_base_z;
    let ramp_slope = bridge_ramp_catenary_slope(foundation_tileh, direction);
    let trackbits = if axis_y { RAIL_TB_Y } else { RAIL_TB_X };
    let mut wires = Vec::new();
    let mut pylons = Vec::new();
    collect_catenary_ramp_draws_from_map(
        map,
        coord,
        map_dims.0,
        map_dims.1,
        OTTD_MP_RAIL,
        trackbits,
        ramp_slope,
        1 << (direction & 3),
        &mut wires,
        &mut pylons,
    );
    let catenary_tint = catenary_sprite_color();
    let tint = tint.with_alpha(tint.alpha() * catenary_tint.alpha());

    for draw in pylons {
        let Some((sprite, anchor)) = preview_catenary_sprite(
            asset_server,
            draw.sprite_id,
            tint,
            catenary_newgrf,
            catenary_sprites,
            images,
        ) else {
            continue;
        };
        let Some(pcp) = draw.pcp_direction else {
            continue;
        };
        let index = usize::from(pcp & 3);
        let world_z_delta = bridge_ramp_catenary_world_z_delta(
            foundation_tileh,
            foundation_base_z,
            raw_base_z,
            direction,
            [0, 8, 16, 8][index].min(15),
            [8, 16, 8, 0][index].min(15),
            4,
        );
        let local_z = catenary_local_z_delta(world_z_delta, raw_base_z, foundation_base_z);
        let position = catenary_sprite_center(
            px,
            py,
            foundation_base_z,
            draw.z_layer + 0.055,
            draw.tile_dx - 1.0,
            draw.tile_dy - 1.0,
            local_z as f32,
            anchor,
        );
        commands.spawn((
            BuildGhostPreview,
            sprite,
            Transform::from_translation(position),
        ));
    }

    for (index, draw) in wires.into_iter().enumerate() {
        let Some((sprite, anchor)) = preview_catenary_sprite(
            asset_server,
            draw.sprite_id,
            tint,
            catenary_newgrf,
            catenary_sprites,
            images,
        ) else {
            continue;
        };
        let (ox, oy, oz) = draw.bounds_origin;
        let world_z_delta = bridge_ramp_catenary_world_z_delta(
            foundation_tileh,
            foundation_base_z,
            raw_base_z,
            direction,
            ox,
            oy,
            8,
        );
        let position = catenary_sprite_center(
            px,
            py,
            foundation_base_z,
            0.09 + index as f32 * 0.0004,
            ox as f32,
            oy as f32,
            (world_z_delta + oz) as f32,
            anchor,
        );
        commands.spawn((
            BuildGhostPreview,
            sprite,
            Transform::from_translation(position),
        ));
    }
}

/// Dibuja una pieza de `DrawFoundation` en la preview usando el mismo anclaje
/// NFO que `spawn_foundation_sprite`. Los 14 sprites clásicos se cargan del
/// atlas OpenGFX; las fundaciones virtuales se resuelven desde el bloque
/// Action5 vigente del save/NewGRF.
#[allow(clippy::too_many_arguments)]
fn spawn_bridge_ramp_foundations(
    commands: &mut Commands,
    asset_server: &AssetServer,
    px: i32,
    py: i32,
    raw_base_z: u8,
    plan: openttdrs_core::RailFoundationDrawPlan,
    tint: Color,
    foundation_newgrf: &[Option<openttdrs_core::DecodedSprite>],
    action5_sprites: &mut NewGrfAction5SpriteCache,
    images: &mut Assets<Image>,
) {
    let foundation_tint = tint.with_alpha(tint.alpha() * 0.84);
    for (ordinal, draw) in plan.sprites.into_iter().flatten().enumerate() {
        let (image, xrel, yrel, width, height) = if let Some(tileh) = draw
            .sprite_id
            .checked_sub(openttdrs_core::FOUNDATION_ORIGINAL_SPRITE_BASE)
            .and_then(|value| u8::try_from(value).ok())
            .filter(|tileh| (1..=14).contains(tileh))
        {
            let (Some(path), Some(gfx)) = (
                foundation_asset_path(tileh),
                foundation_gfx_for_tileh(tileh),
            ) else {
                continue;
            };
            (
                asset_server.load::<Image>(path),
                gfx.xrel,
                gfx.yrel,
                gfx.w,
                gfx.h,
            )
        } else {
            let Some(slot) = openttdrs_core::foundation_action5_slot_for_sprite_id(draw.sprite_id)
            else {
                continue;
            };
            let Some(decoded) = foundation_newgrf.get(slot).and_then(Option::as_ref) else {
                continue;
            };
            let Some(slot_u16) = u16::try_from(slot).ok() else {
                continue;
            };
            (
                action5_sprites.handle_for(
                    openttdrs_core::ACTION5_TYPE_FOUNDATIONS,
                    slot_u16,
                    decoded,
                    images,
                ),
                f32::from(decoded.x_offs),
                f32::from(decoded.y_offs),
                f32::from(decoded.width),
                f32::from(decoded.height),
            )
        };

        let mut position = overlay_pos(
            iso(px, py),
            xrel,
            yrel,
            width,
            height,
            raw_base_z.saturating_add(draw.z_delta),
            FOUNDATION_LAYER + ordinal as f32 * 0.0005,
            px,
            py,
        );
        // `AddSortableSpriteToDraw` remapea también el origen de SpriteBounds;
        // omitirlo desalinearía las mitades de una fundación de media tesela.
        let bounds_offset = remap_tile_offset(
            f32::from(draw.bounds.ox),
            f32::from(draw.bounds.oy),
            f32::from(draw.bounds.oz),
        ) * 0.5;
        position += Vec3::new(bounds_offset.x, bounds_offset.y, 0.0);
        commands.spawn((
            BuildGhostPreview,
            Sprite {
                image,
                color: foundation_tint,
                ..default()
            },
            Transform::from_translation(position).with_scale(Vec3::splat(1.002)),
        ));
    }
}

/// Posición en pantalla con offsets NFO, como `spawn_layer` en `bridge_draw.rs`.
fn bridge_ghost_translation(
    px: i32,
    py: i32,
    base_z: u8,
    sprite_id: u32,
    shift: Vec2,
    layer: f32,
) -> Vec3 {
    let (w, h, xrel, yrel) = bridge_sprite_meta(sprite_id).unwrap_or((64.0, 32.0, -32.0, -16.0));
    let iso_pos = iso(px, py);
    let z_px = f32::from(base_z) * HEIGHT_PX;
    Vec3::new(
        iso_pos.x + shift.x + xrel + w / 2.0,
        iso_pos.y + shift.y - yrel - h / 2.0 + z_px,
        crate::iso::sortable_draw_z(px, py, base_z, layer),
    )
}

pub(crate) fn spawn_bridge_span_preview(
    commands: &mut Commands,
    spawn: BridgeSpanPreviewSpawn<'_>,
) {
    let BridgeSpanPreviewSpawn {
        asset_server,
        action,
        tiles,
        map,
        valid,
        rail_type,
        road_def,
        road_catalog,
        climate,
        newgrf_stack,
        road_sprites,
        catenary_newgrf,
        catenary_sprites,
        foundation_newgrf,
        action5_sprites,
        images,
    } = spawn;
    let is_rail = action == BuildMenuAction::RailBridge;
    let ordered_tiles = bridge_preview_render_order(tiles);
    let axis_y = bridge_span_axis_y(&ordered_tiles);
    let axis = usize::from(axis_y);
    let total = ordered_tiles.len();
    let bridge_type = BridgeType::Wooden;
    let tint = if valid {
        Color::srgba(1.0, 1.0, 1.0, 0.58)
    } else {
        Color::srgba(1.0, 0.28, 0.22, 0.58)
    };
    let map_dims = map.dimensions();
    let deck_z = ordered_tiles.last().map(|&(px, py)| {
        let (tileh, min_z) = tile_slope_and_min_z(map, px as u32, py as u32);
        bridge_preview_deck_z(tileh, min_z, axis)
    });
    let road_source = road_def.and_then(|def| {
        bridge_preview_source_tile(map, &ordered_tiles, def).map(|(coord, tile)| (def, coord, tile))
    });
    for (index, &(px, py)) in ordered_tiles.iter().enumerate() {
        let coord = openttdrs_core::TileCoord::new(px, py);
        if map.get(coord).is_none() {
            continue;
        }
        let (tileh, base_z) = tile_slope_and_min_z(map, px as u32, py as u32);
        let north_len = u32::try_from(index + 1).unwrap_or(u32::MAX);
        let south_len = u32::try_from(total.saturating_sub(index)).unwrap_or(u32::MAX);
        let piece = calc_bridge_piece(north_len, south_len);
        let is_middle = total > 2 && index > 0 && index + 1 < total;
        let bridge_offset = bridge_preview_road_sprite_offset(axis_y, index, total, tileh);
        let ids = bridge_deck_sprite_ids(bridge_type, piece);
        let ramp_direction = (!is_middle && total >= 2)
            .then(|| bridge_preview_ramp_direction(axis_y, index + 1 == total));
        let ramp_foundation = ramp_direction.map(|direction| {
            bridge_foundation_decision_at(map, coord, map_dims, tileh, base_z, direction)
        });
        if !is_middle
            && let Some(foundation) = ramp_foundation
            && foundation.foundation != 0
        {
            let plan = openttdrs_core::foundation_draw_plan(
                tileh,
                foundation.foundation,
                foundation.sprite_block,
            );
            debug_assert_eq!(plan.surface_tileh, foundation.surface_tileh);
            debug_assert_eq!(
                plan.surface_z_delta,
                foundation.surface_base_z.saturating_sub(base_z)
            );
            spawn_bridge_ramp_foundations(
                commands,
                asset_server,
                px,
                py,
                base_z,
                plan,
                tint,
                foundation_newgrf,
                action5_sprites,
                images,
            );
        }
        let (sprite_id, shift, layer) = if is_middle {
            (ids.front[axis], bridge_front_shift(axis), FRONT_LAYER)
        } else {
            (
                ramp_foundation
                    .zip(ramp_direction)
                    .map(|(foundation, direction)| {
                        bridge_ramp_sprite_id(
                            bridge_type,
                            is_rail,
                            rail_type,
                            foundation.surface_tileh,
                            direction,
                        )
                    })
                    .unwrap_or_else(|| ids.rear(is_rail, axis)),
                Vec2::ZERO,
                DECK_LAYER,
            )
        };
        let surface_z = if is_middle {
            deck_z.unwrap_or(base_z)
        } else {
            ramp_foundation
                .map(|foundation| foundation.surface_base_z)
                .unwrap_or(base_z)
        };
        let path = format!(
            "assets/opengfx/tiles/{}",
            BridgeDeckSpriteIds::atlas_name(sprite_id)
        );
        commands.spawn((
            BuildGhostPreview,
            Sprite {
                image: asset_server.load(path),
                color: tint,
                ..default()
            },
            Transform::from_translation(bridge_ghost_translation(
                px, py, surface_z, sprite_id, shift, layer,
            ))
            .with_scale(Vec3::new(1.002, 1.002, 1.0)),
        ));

        if let Some((def, source_coord, source_tile)) = road_source
            && let Some((sprite, view)) = custom_bridge_preview_layer(
                def,
                map,
                source_coord,
                source_tile,
                ROTSG_BRIDGE,
                bridge_offset,
                climate,
                road_catalog,
                newgrf_stack,
                road_sprites,
                images,
                tint,
            )
        {
            commands.spawn((
                BuildGhostPreview,
                sprite,
                Transform::from_translation(overlay_pos(
                    iso(px, py),
                    f32::from(view.x_offs),
                    f32::from(view.y_offs),
                    f32::from(view.width),
                    f32::from(view.height),
                    surface_z,
                    CUSTOM_BRIDGE_LAYER,
                    px,
                    py,
                ))
                .with_scale(Vec3::splat(1.002)),
            ));
        }

        if is_rail && rail_type.has_catenary() {
            if is_middle {
                if let Some(middle_length) = total.checked_sub(2) {
                    spawn_rail_bridge_catenary(
                        commands,
                        asset_server,
                        px,
                        py,
                        surface_z,
                        axis_y,
                        index,
                        middle_length,
                        tint,
                        catenary_newgrf,
                        catenary_sprites,
                        images,
                    );
                }
            } else if total >= 2 {
                spawn_rail_bridge_ramp_catenary(
                    commands,
                    asset_server,
                    map,
                    map_dims,
                    px,
                    py,
                    axis_y,
                    index + 1 == total,
                    tileh,
                    base_z,
                    tint,
                    catenary_newgrf,
                    catenary_sprites,
                    images,
                );
            }
        }

        if let Some((def, source_coord, source_tile)) = road_source
            && def.has_newgrf_specific_group(ROTSG_OVERLAY)
            && (def.class == RoadTramType::Road || source_tile.m3 & 0x0F != 0)
            && let Some((sprite, view)) = custom_bridge_preview_layer(
                def,
                map,
                source_coord,
                source_tile,
                ROTSG_OVERLAY,
                BRIDGE_ROAD_OVERLAY_OFFSETS[bridge_offset.min(5)],
                climate,
                road_catalog,
                newgrf_stack,
                road_sprites,
                images,
                tint,
            )
        {
            let custom_z = if is_middle {
                deck_z.unwrap_or(base_z)
            } else {
                base_z
            };
            commands.spawn((
                BuildGhostPreview,
                sprite,
                Transform::from_translation(overlay_pos(
                    iso(px, py),
                    f32::from(view.x_offs),
                    f32::from(view.y_offs),
                    f32::from(view.width),
                    f32::from(view.height),
                    custom_z,
                    CUSTOM_OVERLAY_LAYER,
                    px,
                    py,
                ))
                .with_scale(Vec3::splat(1.002)),
            ));
        }

        if let Some((def, source_coord, source_tile)) = road_source
            && def.has_catenary()
            && !crate::sprites::catenary_hidden()
        {
            let offset = bridge_offset.min(BRIDGE_ROAD_CATENARY_BACK_OFFSETS.len() - 1);
            let has_custom_catenary = def.has_newgrf_specific_group(ROTSG_CATENARY_BACK)
                || def.has_newgrf_specific_group(ROTSG_CATENARY_FRONT);
            if has_custom_catenary {
                if def.has_newgrf_specific_group(ROTSG_CATENARY_BACK)
                    && let Some((sprite, view)) = custom_bridge_preview_layer(
                        def,
                        map,
                        source_coord,
                        source_tile,
                        ROTSG_CATENARY_BACK,
                        23 + BRIDGE_ROAD_CATENARY_BACK_OFFSETS[offset],
                        climate,
                        road_catalog,
                        newgrf_stack,
                        road_sprites,
                        images,
                        tint,
                    )
                {
                    commands.spawn((
                        BuildGhostPreview,
                        sprite,
                        Transform::from_translation(overlay_pos(
                            iso(px, py),
                            f32::from(view.x_offs),
                            f32::from(view.y_offs),
                            f32::from(view.width),
                            f32::from(view.height),
                            surface_z,
                            CUSTOM_CATENARY_BACK_LAYER,
                            px,
                            py,
                        ))
                        .with_scale(Vec3::splat(1.002)),
                    ));
                }
                if def.has_newgrf_specific_group(ROTSG_CATENARY_FRONT)
                    && let Some((sprite, view)) = custom_bridge_preview_layer(
                        def,
                        map,
                        source_coord,
                        source_tile,
                        ROTSG_CATENARY_FRONT,
                        23 + BRIDGE_ROAD_CATENARY_FRONT_OFFSETS[offset],
                        climate,
                        road_catalog,
                        newgrf_stack,
                        road_sprites,
                        images,
                        tint,
                    )
                {
                    commands.spawn((
                        BuildGhostPreview,
                        sprite,
                        Transform::from_translation(overlay_pos(
                            iso(px, py),
                            f32::from(view.x_offs),
                            f32::from(view.y_offs),
                            f32::from(view.width),
                            f32::from(view.height),
                            surface_z,
                            CUSTOM_CATENARY_FRONT_LAYER,
                            px,
                            py,
                        ))
                        .with_scale(Vec3::splat(1.002)),
                    ));
                }
            } else {
                let (back_id, front_id) = bridge_road_catenary_sprite_ids(bridge_offset);
                spawn_vanilla_bridge_catenary(
                    commands,
                    asset_server,
                    px,
                    py,
                    surface_z,
                    CUSTOM_CATENARY_BACK_LAYER,
                    back_id,
                    tint,
                );
                spawn_vanilla_bridge_catenary(
                    commands,
                    asset_server,
                    px,
                    py,
                    surface_z,
                    CUSTOM_CATENARY_FRONT_LAYER,
                    front_id,
                    tint,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bridge_axis_y_when_span_runs_north_south() {
        let tiles = vec![(3, 2), (3, 3), (3, 4), (3, 5)];
        assert!(bridge_span_axis_y(&tiles));
    }

    #[test]
    fn bridge_axis_x_when_span_runs_east_west() {
        let tiles = vec![(2, 4), (3, 4), (4, 4), (5, 4)];
        assert!(!bridge_span_axis_y(&tiles));
    }

    #[test]
    fn bridge_preview_reorders_a_drag_started_at_the_south_head() {
        let tiles = vec![(3, 5), (3, 4), (3, 3), (3, 2)];
        assert_eq!(
            bridge_preview_render_order(&tiles),
            vec![(3, 2), (3, 3), (3, 4), (3, 5)]
        );
    }

    #[test]
    fn bridge_preview_ramp_directions_follow_axis_and_endpoint() {
        assert_eq!(bridge_preview_ramp_direction(false, false), 2);
        assert_eq!(bridge_preview_ramp_direction(false, true), 0);
        assert_eq!(bridge_preview_ramp_direction(true, false), 1);
        assert_eq!(bridge_preview_ramp_direction(true, true), 3);
    }

    #[test]
    fn bridge_preview_deck_z_matches_flat_and_corner_ramps() {
        assert_eq!(bridge_preview_deck_z(0, 4, 0), 5);
        assert_eq!(bridge_preview_deck_z(1, 4, 0), 5);
        assert_eq!(bridge_preview_deck_z(5, 4, 0), 6);
    }

    #[test]
    fn custom_bridge_layer_uses_specific_view_geometry() {
        use std::collections::HashMap;

        let view = openttdrs_core::DecodedSprite {
            width: 11,
            height: 13,
            x_offs: -4,
            y_offs: -7,
            rgba: vec![255, 0, 0, 255].repeat(11 * 13),
            mask: Vec::new(),
        };
        let graphics = openttdrs_core::TrainSpriteGraphics {
            sets: vec![vec![view.clone()]],
            assigns: vec![openttdrs_core::TrainSpriteAssign {
                local_id: 0,
                set_id: 0,
            }],
            specific_assigns: HashMap::from([((0, ROTSG_BRIDGE), 0), ((0, ROTSG_OVERLAY), 0)]),
            ..default()
        };
        let def = RoadTypeDef {
            id: RoadType::from_u8(2),
            class: RoadTramType::Road,
            label: "Preview bridge".into(),
            short_label: "PBRG".into(),
            intro_year: 0,
            max_speed: 0,
            cost_multiplier: 0,
            maintenance_multiplier: 0,
            flags: 0,
            powered_mask: 0,
            badges: Vec::new(),
            from_tramtypes_feature: false,
            from_newgrf: true,
            newgrf_preview: None,
            newgrf_views: Vec::new(),
            newgrf_local_id: 0,
            newgrf_runtime: Some(Box::new(graphics)),
            newgrf_grfid: 0,
            newgrf_type_tables: None,
        };
        let map = Map::new_flat(4, 4, 0);
        let coord = TileCoord::new(2, 2);
        let tile = map.get(coord).expect("preview source tile");
        let mut images = Assets::<Image>::default();
        let mut road_sprites = NewGrfRoadSpriteCache::default();

        let (_, resolved) = custom_bridge_preview_layer(
            &def,
            &map,
            coord,
            tile,
            ROTSG_BRIDGE,
            0,
            Climate::Temperate,
            &[def.clone()],
            &[],
            &mut road_sprites,
            &mut images,
            Color::WHITE,
        )
        .expect("ROTSG_BRIDGE preview view");

        assert_eq!((resolved.width, resolved.height), (11, 13));
        assert_eq!((resolved.x_offs, resolved.y_offs), (-4, -7));
        assert_eq!(images.len(), 1);
    }

    #[test]
    fn custom_bridge_layer_keeps_overlay_selector_separate_in_cache() {
        use std::collections::HashMap;

        let view = openttdrs_core::DecodedSprite {
            width: 5,
            height: 6,
            x_offs: 1,
            y_offs: -2,
            rgba: vec![0, 255, 0, 255].repeat(5 * 6),
            mask: Vec::new(),
        };
        let graphics = openttdrs_core::TrainSpriteGraphics {
            sets: vec![vec![view.clone()]],
            assigns: vec![openttdrs_core::TrainSpriteAssign {
                local_id: 0,
                set_id: 0,
            }],
            specific_assigns: HashMap::from([((0, ROTSG_OVERLAY), 0)]),
            ..default()
        };
        let def = RoadTypeDef {
            id: RoadType::from_u8(3),
            class: RoadTramType::Road,
            label: "Preview overlay".into(),
            short_label: "POVL".into(),
            intro_year: 0,
            max_speed: 0,
            cost_multiplier: 0,
            maintenance_multiplier: 0,
            flags: 0,
            powered_mask: 0,
            badges: Vec::new(),
            from_tramtypes_feature: false,
            from_newgrf: true,
            newgrf_preview: None,
            newgrf_views: Vec::new(),
            newgrf_local_id: 0,
            newgrf_runtime: Some(Box::new(graphics)),
            newgrf_grfid: 0,
            newgrf_type_tables: None,
        };
        let map = Map::new_flat(4, 4, 0);
        let coord = TileCoord::new(1, 1);
        let tile = map.get(coord).expect("preview source tile");
        let mut images = Assets::<Image>::default();
        let mut road_sprites = NewGrfRoadSpriteCache::default();

        let (_, resolved) = custom_bridge_preview_layer(
            &def,
            &map,
            coord,
            tile,
            ROTSG_OVERLAY,
            BRIDGE_ROAD_OVERLAY_OFFSETS[0],
            Climate::Temperate,
            &[def.clone()],
            &[],
            &mut road_sprites,
            &mut images,
            Color::WHITE,
        )
        .expect("ROTSG_OVERLAY preview view");

        assert_eq!((resolved.width, resolved.height), (5, 6));
        assert_eq!((resolved.x_offs, resolved.y_offs), (1, -2));
        assert_eq!(images.len(), 1);
    }

    #[test]
    fn bridge_preview_catenary_uses_distinct_front_and_back_offsets() {
        assert_eq!(23 + BRIDGE_ROAD_CATENARY_BACK_OFFSETS[0], 118);
        assert_eq!(23 + BRIDGE_ROAD_CATENARY_FRONT_OFFSETS[0], 120);
        assert_eq!(23 + BRIDGE_ROAD_CATENARY_BACK_OFFSETS[5], 124);
        assert_eq!(23 + BRIDGE_ROAD_CATENARY_FRONT_OFFSETS[5], 128);
    }

    #[test]
    fn vanilla_bridge_catenary_uses_tramway_assets_and_native_geometry() {
        let (back, front) = bridge_road_catenary_sprite_ids(0);
        assert_eq!((back, front), (6081, 6083));
        assert_eq!(
            crate::sprites::tramway_sprite_atlas_key(back).as_deref(),
            Some("tramway_095.png")
        );
        assert!(crate::sprites::catenary_sprite_gfx(front).is_some());
    }

    #[test]
    fn rail_bridge_preview_uses_the_world_van_geometry_contract() {
        let mut draws = Vec::new();
        collect_catenary_bridge_draws(true, 1, 2, catenary_tile_location_group(8, 6), &mut draws);
        assert_eq!(draws[0].sprite_id, crate::sprites::WIRE_SPRITE_BASE + 16);
        assert_eq!(draws[0].tile_dx, 8.0);
        assert!(draws[1].pcp_direction.is_some());
        assert!(crate::sprites::catenary_sprite_gfx(draws[1].sprite_id).is_some());
    }

    #[test]
    fn rail_bridge_preview_resolves_virtual_catenary_aliases() {
        assert_eq!(
            crate::sprites::catenary_sprite_atlas_key(crate::sprites::WIRE_SPRITE_BASE),
            Some("rail_1039.png".into())
        );
        assert_eq!(
            crate::sprites::catenary_sprite_atlas_key(crate::sprites::PYLON_SPRITE_BASE),
            Some("rail_pylon_0.png".into())
        );
    }
}
