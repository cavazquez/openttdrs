use bevy::prelude::*;
use openttdrs_core::prelude::*;
use openttdrs_core::{Climate, RoadTypeDef, partial_pixel_z};

use super::helpers::{
    spawn_forced_leveled_foundation_with_child_parent, spawn_foundation_child_ground_sprite_at,
    spawn_foundation_child_sprite_at,
};
use super::{
    FLAT_WATER_LAYER_FRAC, SHORE_LAYER_FRAC, TRAM_OVERLAY_LAYER_FRAC, catenary_under_low_bridge,
    roadside_detail_visible_under_bridge, sloped_or_flat_image, spawn_ground_sprite,
    spawn_rail_foundation, spawn_road_foundation,
};
use crate::iso::{
    GROUND_SPRITE_CENTER_X_OFFSET, HEIGHT_PX, TILE_HALF_H, full_tile_sprite_pos,
    full_tile_sprite_pos_half, ground_draw_z, ground_tile_pos_half, overlay_pos, remap_tile_offset,
    shore_png_index, shore_sprite_half_h, slope_half_h, slope_sprite_offset, sortable_draw_z,
    tile_pos_half,
};
use crate::render::catenary_newgrf::{
    CatenarySpriteAnchor, catenary_sprite_anchor, catenary_sprite_center, catenary_sprite_colored,
    catenary_sprite_horizontal_crop,
};
use crate::render::road_newgrf::{
    NewGrfRoadSpriteCache, newgrf_road_def_for_tile, newgrf_tram_def_for_tile,
    road_newgrf_view_index,
};
use crate::render::viewport_sort::{
    ParentSprite, ParentSpriteBounds, depths_in_viewport_sort_order,
};
use crate::render::world_draw_trace::{TraceSpriteBounds, WorldDrawTrace};
use crate::render::{
    CompanyColoredSprites, MapVisualLayer, TileRenderContext, ViewportSortableChild, WaterTile,
    WorldAssets, sprite_from_atlas_or_company_white_colour, viewport_source_depth,
};
use crate::sprites::{
    CompanyColour, ONEWAY_ROAD_SPRITE_META, RAIL_GROUND_HALF_TILE_SNOW,
    RAIL_GROUND_HALF_TILE_WATER, RAIL_GROUND_SNOW_OR_DESERT, RAIL_TB_CROSS, RAIL_TB_HORZ,
    RAIL_TB_LEFT, RAIL_TB_LOWER, RAIL_TB_RIGHT, RAIL_TB_UPPER, RAIL_TB_VERT, RAIL_TB_X, RAIL_TB_Y,
    ROAD_FLAT_HALF_H, ROAD_STREETLIGHT_META, ROADSIDE_LAMPS, ROADSIDE_TREE_META, ROADSIDE_TREES,
    SPR_ROADSIDE_TREE, catenary_hidden, catenary_pylon_world_z_delta, catenary_reference_sprite_id,
    catenary_sprite_color, catenary_tunnel_exterior_pcp, catenary_wire_world_z_delta,
    collect_catenary_pylons_from_map_with_pcp_override, collect_catenary_wire_draws_from_map,
    collect_rail_pbs_reservation_draws, collect_rail_sprites_for_surface,
    collect_signal_sprite_draws, is_road_level_crossing, is_typed_rail_track_sprite,
    level_crossing_ground_sprite_id_for_type, level_crossing_has_rail_reservation,
    oneway_road_sprite_id, rail_ghost_overlay_offset, rail_pbs_reservation_offset,
    rail_tile_is_signals, rail_trackbits_for_render, remap_rail_sprite_id, road_bits_for_render,
    road_catenary_sprite_ids, road_flat_sprite_color, road_flat_sprite_index,
    road_ground_sprite_id, road_streetlight_sprite_id, road_tile_roadside,
    road_tile_snow_or_desert, roadside_is_paved, signal_safe_slope_position_for_side,
    signal_screen_anchor_for_side, signal_screen_position_for_side, signal_sprite_center_offset,
    signal_world_position_for_side, track_fence_draws_for_tile, track_fence_height_px,
    track_fence_sprite_meta, tram_flat_sprite_index,
};

/// Contexto de `DrawGroundSprite` para una pasada de vía. Una fundación crea
/// un padre sortable y las vías siguientes pasan a ser `child`; sin ella, el
/// mismo draw proc emite una primitiva `ground` anclada al mundo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RailTrackTraceMode {
    Ground,
    FoundationChild((i32, i32, i32)),
}

// Discriminantes de `Foundation` que todavía no forman parte de la API pública
// de core. Se mantienen aquí porque solo describen la relación padre/hijo del
// stream de trazas, no la selección ni el dibujo de los cimientos.
const FOUNDATION_STEEP_LOWER: u8 = 4;
const FOUNDATION_STEEP_BOTH: u8 = 5;
const FOUNDATION_HALFTILE_W: u8 = 6;
const FOUNDATION_HALFTILE_N: u8 = 9;
const FOUNDATION_RAIL_W: u8 = 10;
const FOUNDATION_RAIL_N: u8 = 13;

/// Bases de `DrawTrackBits` / `DrawTrackBitsOverlay` en `rail_cmd.cpp`.
///
/// Estos IDs son de la semántica OpenTTD; la imagen concreta sigue saliendo
/// del atlas activo, por lo que el selector se comparte entre OpenGFX 8bpp y
/// OpenGFX2 32bpp.
const SPR_FLAT_BARE_LAND: u32 = 3924;
const SPR_FLAT_GRASS_TILE: u32 = 3981;
const SPR_FLAT_WATER_TILE: u32 = 4061;
const SPR_FLAT_SNOW_DESERT_TILE: u32 = 4550;
const SPR_SHORE_BASE: u32 = 5936;
const RAIL_SLOPE_STEEP: u8 = 0x10;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RailGroundKind {
    Barren,
    Grass,
    SnowOrDesert,
    Water,
    Shore,
}

/// Una llamada explícita a `DrawGroundSprite` que acompaña a una vía.
///
/// Las vías clásicas normalmente ya llevan su suelo incluido en el sprite de
/// riel. Sólo un railtype con `Underlay` Action3 usa
/// `DrawTrackBitsOverlay`, que dibuja el suelo antes del overlay; las vías
/// junto a `HalfTileWater` también requieren costa o agua independientes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct RailGroundDraw {
    kind: RailGroundKind,
    tileh: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct RailInitialGroundDraw {
    draw: RailGroundDraw,
    trace_mode: RailTrackTraceMode,
    surface_z_delta: u8,
}

const fn rail_ground_kind_for_land(ground_type: u8, upper_halftile: bool) -> RailGroundKind {
    match ground_type {
        0 => RailGroundKind::Barren,
        RAIL_GROUND_SNOW_OR_DESERT => RailGroundKind::SnowOrDesert,
        // `DrawTrackBitsOverlay` trata HalfTileSnow como nieve sólo en la
        // segunda pasada de una fundación no continua.
        RAIL_GROUND_HALF_TILE_SNOW if upper_halftile => RailGroundKind::SnowOrDesert,
        _ => RailGroundKind::Grass,
    }
}

fn rail_ground_sprite_id(draw: RailGroundDraw) -> u32 {
    match draw.kind {
        RailGroundKind::Barren => SPR_FLAT_BARE_LAND + u32::from(slope_sprite_offset(draw.tileh)),
        RailGroundKind::Grass => SPR_FLAT_GRASS_TILE + u32::from(slope_sprite_offset(draw.tileh)),
        RailGroundKind::SnowOrDesert => {
            SPR_FLAT_SNOW_DESERT_TILE + u32::from(slope_sprite_offset(draw.tileh))
        }
        RailGroundKind::Water => SPR_FLAT_WATER_TILE,
        RailGroundKind::Shore => SPR_SHORE_BASE + shore_png_index(draw.tileh) as u32,
    }
}

/// Track bits de la primera llamada a `DrawTrackBits`.
///
/// `RailTrackDrawPlan` omite deliberadamente una pasada sin vías. Para la
/// paridad del suelo esa ausencia importa: OpenTTD todavía pinta agua o
/// terreno antes de crear la fundación de la mitad elevada.
const fn rail_initial_track_bits(track_plan: openttdrs_core::RailTrackDrawPlan) -> u8 {
    match track_plan.passes[0] {
        Some(pass) if pass.halftile_corner.is_none() => pass.track_bits,
        _ => 0,
    }
}

/// Fundación aplicada antes del primer suelo de `DrawTrackBits`.
///
/// Las fundaciones de media tesela se difieren; `SteepBoth` aplica primero
/// solamente `SteepLower`. Así no se adelanta visualmente la mitad elevada.
const fn rail_initial_foundation(foundation: u8) -> u8 {
    match foundation {
        FOUNDATION_STEEP_BOTH => FOUNDATION_STEEP_LOWER,
        FOUNDATION_HALFTILE_W..=FOUNDATION_HALFTILE_N => 0,
        _ => foundation,
    }
}

/// En `SteepLower` no se crea un parent sortable: `OffsetGroundSprite` sólo
/// modifica la posición de un eventual parent, que sigue inexistente. Por
/// eso el primer `DrawGroundSprite` mantiene coordenadas de mundo.
const fn rail_initial_ground_trace_mode(foundation: u8) -> RailTrackTraceMode {
    match rail_initial_foundation(foundation) {
        openttdrs_core::FOUNDATION_LEVELED => RailTrackTraceMode::FoundationChild((0, -32, 0)),
        openttdrs_core::FOUNDATION_INCLINED_X
        | openttdrs_core::FOUNDATION_INCLINED_Y
        | FOUNDATION_RAIL_W..=FOUNDATION_RAIL_N => RailTrackTraceMode::FoundationChild((0, 0, 0)),
        _ => RailTrackTraceMode::Ground,
    }
}

fn rail_initial_ground_draw(
    tileh: u8,
    foundation: u8,
    track_plan: openttdrs_core::RailTrackDrawPlan,
    rail_uses_overlay: bool,
    ground_type: u8,
) -> Option<RailInitialGroundDraw> {
    let initial_track = rail_initial_track_bits(track_plan);
    let initial_foundation = rail_initial_foundation(foundation);
    let surface = openttdrs_core::foundation_draw_plan(tileh, initial_foundation, 0);
    let surface_tileh = surface.surface_tileh;

    let kind = if rail_uses_overlay {
        // `DrawTrackBitsOverlay` siempre emite una base, incluso con vía
        // plana. Para HalfTileWater el primer pase usa costa si contiene vía
        // o si la pendiente es empinada; de otro modo conserva agua plana.
        if ground_type == RAIL_GROUND_HALF_TILE_WATER {
            if initial_track != 0 || surface_tileh & RAIL_SLOPE_STEEP != 0 {
                RailGroundKind::Shore
            } else {
                RailGroundKind::Water
            }
        } else {
            rail_ground_kind_for_land(ground_type, false)
        }
    } else if ground_type == RAIL_GROUND_HALF_TILE_WATER {
        // La vía clásica sólo añade un suelo separado para HalfTileWater.
        // Con vía presente es costa; la mitad inferior vacía conserva agua
        // plana salvo la pendiente empinada.
        if initial_track != 0 || surface_tileh & RAIL_SLOPE_STEEP != 0 {
            RailGroundKind::Shore
        } else {
            RailGroundKind::Water
        }
    } else if initial_track == 0 {
        // Caso aislado por el oráculo en Kale (158,65): la mitad baja sin
        // riel sigue necesitando su `DrawGroundSprite` antes de Foundation.
        rail_ground_kind_for_land(ground_type, false)
    } else {
        return None;
    };

    Some(RailInitialGroundDraw {
        draw: RailGroundDraw {
            kind,
            tileh: surface_tileh,
        },
        trace_mode: rail_initial_ground_trace_mode(foundation),
        surface_z_delta: surface.surface_z_delta,
    })
}

fn rail_upper_halftile_ground_draw(
    rail_uses_overlay: bool,
    ground_type: u8,
    upper_tileh: u8,
) -> Option<RailGroundDraw> {
    rail_uses_overlay.then_some(RailGroundDraw {
        kind: rail_ground_kind_for_land(ground_type, true),
        tileh: upper_tileh,
    })
}

fn record_rail_ground_trace(draw: RailGroundDraw, mode: RailTrackTraceMode, world_z_delta: i32) {
    let sprite_id = rail_ground_sprite_id(draw);
    match mode {
        RailTrackTraceMode::Ground => WorldDrawTrace::record_sprite_with_geometry(
            "rail-ground",
            "ground",
            sprite_id,
            false,
            (0, 0, 0),
            world_z_delta,
            None,
        ),
        RailTrackTraceMode::FoundationChild(offset) => {
            WorldDrawTrace::record_foundation_child_sprite("rail-ground", sprite_id, false, offset);
        }
    }
}

/// Posición de una base de vía emitida con `DrawGroundSprite` sin foundation.
///
/// Aunque la tesela lógica sea `Rail`, OpenTTD la coloca en el pase previo de
/// suelo. Mantener este pequeño helper separado evita que las variantes agua y
/// costa vuelvan accidentalmente al pase sortable al compartir atlas.
#[inline]
fn rail_ground_pass_pos(tx: i32, ty: i32, base_z: u8, layer: f32, half_h: f32) -> Vec3 {
    ground_tile_pos_half(tx, ty, base_z, layer, half_h)
}

/// Materializa una base ferroviaria que OpenTTD dibuja como suelo separado.
///
/// Las variantes se resuelven desde [`WorldAssets`], no desde PNGs fijos,
/// para que la misma selección semántica use 8bpp o 32bpp según el baseset
/// activo. Las medias teselas altas se trazan pero no se redibujan completas:
/// OpenTTD las recorta con `SubSprite`, mientras que el suelo ya existente
/// mantiene la parte baja visible en el renderer actual.
fn spawn_rail_ground_draw(
    commands: &mut Commands,
    assets: &WorldAssets,
    ctx: &TileRenderContext,
    draw: RailGroundDraw,
    base_z: u8,
    foundation_child_parent: Option<Entity>,
    map_width: u32,
) {
    let slope = usize::from(slope_sprite_offset(draw.tileh));
    let image = match draw.kind {
        RailGroundKind::Barren => &assets.grass_density[0][slope],
        RailGroundKind::Grass => &assets.grass_density[3][slope],
        RailGroundKind::SnowOrDesert => &assets.snow_desert[3][slope],
        RailGroundKind::Water => &assets.water,
        RailGroundKind::Shore => &assets.shore[shore_png_index(draw.tileh)],
    };

    if let Some(parent) = foundation_child_parent {
        // Los hijos de Foundation conservan el orden local de su padre. No
        // aparece agua hija en Kale hoy, pero conservar WaterTile evita que
        // un SAV posterior con esa rama pierda su animación.
        if draw.kind == RailGroundKind::Water {
            let mut position = full_tile_sprite_pos(ctx.tx_i32(), ctx.ty_i32(), base_z, 0.001);
            let source_depth = viewport_source_depth(position.z, ctx.tx, map_width);
            position.z = source_depth;
            commands.spawn((
                MapVisualLayer,
                ctx.map_tile_chunk(),
                WaterTile::ANIMATED,
                image.sprite(),
                Transform::from_translation(position),
                ViewportSortableChild {
                    parent,
                    source_depth,
                },
            ));
            return;
        }
        let half_h = if draw.kind == RailGroundKind::Shore {
            shore_sprite_half_h(draw.tileh)
        } else if draw.tileh == 0 {
            TILE_HALF_H
        } else {
            slope_half_h(draw.tileh)
        };
        spawn_foundation_child_ground_sprite_at(
            commands,
            image,
            Color::WHITE,
            ctx,
            base_z,
            0.001,
            half_h,
            map_width,
            parent,
        );
        return;
    }

    match draw.kind {
        RailGroundKind::Water => {
            commands.spawn((
                MapVisualLayer,
                ctx.map_tile_chunk(),
                WaterTile::ANIMATED,
                image.sprite(),
                // `DrawGroundSprite(SPR_FLAT_WATER_TILE)` entra en el pase
                // ground aun cuando la tesela sea ferroviaria. La altura
                // sigue moviendo la imagen en pantalla, pero no altera el
                // orden del pase ground (igual que OpenTTD).
                Transform::from_translation(rail_ground_pass_pos(
                    ctx.tx_i32(),
                    ctx.ty_i32(),
                    base_z,
                    FLAT_WATER_LAYER_FRAC,
                    TILE_HALF_H,
                )),
            ));
        }
        RailGroundKind::Shore => {
            commands.spawn((
                MapVisualLayer,
                ctx.map_tile_chunk(),
                image.sprite(),
                // `DrawGroundSprite` de la costa usa el mismo pase que el
                // agua plana. Mantener ambas mitades en la banda ground evita
                // que una foundation vecina abra una fisura entre ellas.
                Transform::from_translation(rail_ground_pass_pos(
                    ctx.tx_i32(),
                    ctx.ty_i32(),
                    base_z,
                    SHORE_LAYER_FRAC,
                    shore_sprite_half_h(draw.tileh),
                )),
            ));
        }
        RailGroundKind::Barren | RailGroundKind::Grass | RailGroundKind::SnowOrDesert => {
            let half_h = if draw.tileh == 0 {
                TILE_HALF_H
            } else {
                slope_half_h(draw.tileh)
            };
            commands.spawn((
                MapVisualLayer,
                ctx.map_tile_chunk(),
                image.sprite(),
                Transform::from_translation(ground_tile_pos_half(
                    ctx.tx_i32(),
                    ctx.ty_i32(),
                    base_z,
                    0.0,
                    half_h,
                )),
            ));
        }
    }
}

/// Offset de `OffsetGroundSprite` para `HalftileFoundation(corner)`, ya
/// normalizado por `ZOOM_BASE` como lo exporta el oráculo C++.
const fn halftile_foundation_child_offset(corner: u8) -> (i32, i32, i32) {
    match corner {
        // Corner::W, ::S, ::E, ::N respectivamente.
        0 => (64, -32, 0),
        1 => (0, -64, 0),
        2 => (-64, -32, 0),
        _ => (0, 0, 0),
    }
}

/// Convierte el `SubSprite` de la parte alta de una foundation de media
/// tesela en un rectángulo de un `Sprite` de Bevy.
///
/// `DrawFoundation(Halftile...)` desplaza el parent por el origen de sus
/// bounds y luego `OffsetGroundSprite` aplica el desplazamiento inverso al
/// child. Por eso el ancla del PNG de vía ya es la de la tesela cruda; sólo
/// hay que recortar la mitad que `GfxBlitter` deja visible. El centro de un
/// `Sprite::rect` cambia al recortarlo, de modo que devolvemos también la
/// compensación necesaria para mantener los píxeles que sobreviven en su
/// posición OpenTTD.
fn halftile_track_subsprite(corner: Option<u8>, size: Vec2, half_h: f32) -> Option<(Rect, Vec2)> {
    halftile_track_subsprite_with_center(
        corner,
        size,
        Vec2::new(GROUND_SPRITE_CENTER_X_OFFSET, -half_h),
        half_h,
    )
}

/// Variante del recorte que conserva un ancla NFO arbitraria (sprites HD de
/// underlay/overlay). `center_offset` es el centro del PNG respecto de
/// `ctx.iso_pos`, con Y ya convertido al eje de Bevy.
fn halftile_track_subsprite_with_center(
    corner: Option<u8>,
    size: Vec2,
    center_offset: Vec2,
    _half_h: f32,
) -> Option<(Rect, Vec2)> {
    let corner = corner?;
    if size.x <= 0.0 || size.y <= 0.0 {
        return None;
    }

    // `DrawGroundSprite` expresa la posición como centro del PNG. Recuperar
    // xrel/yrel desde ese centro permite reutilizar exactamente el recorte
    // aun cuando el NewGRF publique offsets distintos de OpenGFX.
    let xrel = center_offset.x - size.x / 2.0;
    let yrel = -center_offset.y - size.y / 2.0;
    let rect = match corner {
        // `{ -INF, -INF, 32 - 33, INF }`: derecha inclusiva = -1.
        0 => Rect::new(0.0, 0.0, (-xrel).clamp(0.0, size.x), size.y),
        // `{ -INF, 0 + 7, INF, INF }`.
        1 => Rect::new(0.0, (7.0 - yrel).clamp(0.0, size.y), size.x, size.y),
        // `{ -31 + 33, -INF, INF, INF }`: izquierda inclusiva = 2.
        2 => Rect::new((2.0 - xrel).clamp(0.0, size.x), 0.0, size.x, size.y),
        // `{ -INF, -INF, INF, 30 - 23 }`: abajo inclusivo = 7.
        _ => Rect::new(0.0, 0.0, size.x, (8.0 - yrel).clamp(0.0, size.y)),
    };
    if rect.width() <= 0.0 || rect.height() <= 0.0 {
        return None;
    }

    // El eje Y de la textura crece hacia abajo pero Bevy posiciona sprites
    // en Y hacia arriba. La componente Y de la compensación invierte por eso
    // el desplazamiento del centro del rectángulo fuente.
    let shift = Vec2::new(
        rect.center().x - size.x / 2.0,
        size.y / 2.0 - rect.center().y,
    );
    Some((rect, shift))
}

/// `DrawTrackBits` sólo difiere la fundación cuando hay una pasada baja y una
/// alta. La primera debe quedar antes del cimiento; la segunda, después. Si
/// sólo existe la pasada alta, el cimiento sigue yendo al comienzo.
const fn rail_foundation_after_pass(
    track_plan: openttdrs_core::RailTrackDrawPlan,
) -> Option<usize> {
    if track_plan.passes[1].is_some() {
        Some(0)
    } else {
        None
    }
}

/// Replica el padre activo que deja cada llamada a `DrawFoundation` antes de
/// una pasada de `DrawTrackBits`. En las fundaciones no continuas, la pasada
/// baja se pinta antes de crear la fundación de media tesela y la alta después.
const fn rail_track_trace_mode(foundation: u8, halftile_corner: Option<u8>) -> RailTrackTraceMode {
    if let Some(corner) = halftile_corner {
        return RailTrackTraceMode::FoundationChild(halftile_foundation_child_offset(corner));
    }

    match foundation {
        // `DrawFoundation` no se invoca: DrawGroundSpriteAt conserva mundo.
        0 | u8::MAX => RailTrackTraceMode::Ground,
        // Leveled y SteepLower aplican OffsetGroundSprite(0, -TILE_HEIGHT).
        openttdrs_core::FOUNDATION_LEVELED | FOUNDATION_STEEP_LOWER | FOUNDATION_STEEP_BOTH => {
            RailTrackTraceMode::FoundationChild((0, -32, 0))
        }
        // En la pasada baja de una fundación de media tesela no se llamó aún
        // a DrawFoundation. La superior entra por `halftile_corner` arriba.
        FOUNDATION_HALFTILE_W..=FOUNDATION_HALFTILE_N => RailTrackTraceMode::Ground,
        // InclinedX/Y y las fundaciones anti-zig-zag mantienen offset cero.
        openttdrs_core::FOUNDATION_INCLINED_X
        | openttdrs_core::FOUNDATION_INCLINED_Y
        | FOUNDATION_RAIL_W..=FOUNDATION_RAIL_N => RailTrackTraceMode::FoundationChild((0, 0, 0)),
        // Un valor nuevo/desconocido no debe inventar una relación padre.
        _ => RailTrackTraceMode::Ground,
    }
}

fn record_rail_track_trace(
    role: &'static str,
    sprite_id: u32,
    fallback: bool,
    mode: RailTrackTraceMode,
) {
    match mode {
        RailTrackTraceMode::Ground => WorldDrawTrace::record_sprite_with_geometry(
            role,
            "ground",
            sprite_id,
            fallback,
            (0, 0, 0),
            0,
            None,
        ),
        RailTrackTraceMode::FoundationChild(offset) => {
            WorldDrawTrace::record_foundation_child_sprite(role, sprite_id, fallback, offset);
        }
    }
}

/// `DrawRoadBits` dibuja el suelo después de `DrawFoundation`. Las
/// fundaciones viales posibles son las mismas tres continuas de una rampa de
/// puente: nivelada usa `OffsetGroundSprite(0, -TILE_HEIGHT)` e inclinadas
/// conservan el offset cero.
const fn road_foundation_child_offset(foundation: u8) -> Option<(i32, i32, i32)> {
    match foundation {
        0 => None,
        openttdrs_core::FOUNDATION_LEVELED => Some((0, -32, 0)),
        openttdrs_core::FOUNDATION_INCLINED_X | openttdrs_core::FOUNDATION_INCLINED_Y => {
            Some((0, 0, 0))
        }
        _ => None,
    }
}

fn record_road_ground_trace(role: &'static str, sprite_id: u32, foundation: u8) {
    if let Some(offset) = road_foundation_child_offset(foundation) {
        WorldDrawTrace::record_foundation_child_sprite(role, sprite_id, false, offset);
    } else {
        WorldDrawTrace::record_sprite_with_geometry(
            role,
            "ground",
            sprite_id,
            false,
            (0, 0, 0),
            0,
            None,
        );
    }
}

/// Traza de `DrawGroundSpriteAt(oneway, ..., 8, 8, GetPartialPixelZ(...))`.
///
/// Las flechas no son un nuevo roadtype: son el bloque Action5 0x09 que
/// OpenTTD carga siempre desde `openttd.grf`. En una fundación el viewport C++
/// las cuelga del mismo padre que el asfalto; el punto `(8, 8)` añade 64 px
/// normalizados y cada unidad de Z resta `ZOOM_BASE` (=4) px de pantalla.
fn record_road_oneway_trace(sprite_id: u32, tileh: u8, foundation: u8) {
    let local_z = i32::from(partial_pixel_z(8.0, 8.0, tileh));
    if let Some((x, y, z)) = road_foundation_child_offset(foundation) {
        WorldDrawTrace::record_foundation_child_sprite(
            "road-oneway",
            sprite_id,
            false,
            (x, y + 64 - local_z * 4, z),
        );
    } else {
        WorldDrawTrace::record_sprite_with_palette_and_world_geometry(
            "road-oneway",
            "ground",
            sprite_id,
            0,
            false,
            (8, 8),
            local_z,
            (0, 0, 0),
            None,
        );
    }
}

/// Centro Bevy de la flecha Action5. Conserva el ancla NFO y el subpunto
/// `(8, 8)` que usa `DrawRoadBits`, en vez de centrar el PNG 24×16 sobre toda
/// la tesela (que la desplazaba visiblemente en pendientes).
fn road_oneway_overlay_pos(
    ctx: &TileRenderContext,
    tileh: u8,
    base_z: u8,
    (w, h, xrel, yrel): (f32, f32, f32, f32),
) -> Vec3 {
    let local_z = f32::from(partial_pixel_z(8.0, 8.0, tileh));
    let local = remap_tile_offset(8.0, 8.0, local_z) * 0.5;
    overlay_pos(
        ctx.iso_pos + local,
        xrel,
        yrel,
        w,
        h,
        base_z,
        0.025,
        ctx.tx_i32(),
        ctx.ty_i32(),
    )
}

/// Mantiene el ancla NFO de una capa vial, pero la entrega al pase global de
/// `DrawGroundSprite` cuando no hay un `DrawFoundation` activo.
///
/// La altura sigue desplazando el PNG en pantalla; nunca debe promover el
/// suelo vial a la banda de parents, donde puede cubrir la transparencia de
/// una casa que OpenTTD dibuja después.
#[inline]
fn road_ground_pass_pos(mut position: Vec3, ctx: &TileRenderContext, layer: f32) -> Vec3 {
    position.z = ground_draw_z(ctx.tx_i32(), ctx.ty_i32(), layer);
    position
}

/// Altura de mundo del `DrawRoadDetail` respecto de la base cruda de la
/// tesela. Después de una fundación `ti->z` y `ti->tileh` ya son los
/// efectivos, por lo que ambos componentes son necesarios.
fn road_detail_world_z_delta(
    raw_base_z: u8,
    surface_base_z: u8,
    surface_tileh: u8,
    dx: f32,
    dy: f32,
) -> i32 {
    (i32::from(surface_base_z) - i32::from(raw_base_z)) * 8
        + i32::from(partial_pixel_z(dx, dy, surface_tileh))
}

/// Capa local histórica de los faroles de acera. OpenTTD los inserta como
/// parents separados; Bevy necesita dos slots distintos para poder reflejar
/// una inversión del sorter aunque ambos PNG compartan la misma capa visual.
const ROADSIDE_STREETLIGHT_LAYER: f32 = 0.2;
/// El salto se mantiene dentro de la franja local (`×0.001` en
/// `sortable_draw_z`) y es mayor que el ULP de una fila incluso en mapas de
/// 4096×4096. Nunca coincide con el modo `Roadside::Trees`, que usa 0.25.
const ROADSIDE_STREETLIGHT_SLOT_STEP: f32 = 0.02;

/// Parents que `DrawRoadDetail` entrega para los faroles de una misma tesela.
///
/// El ancla Z no siempre es `base_z * 8`: `GetSlopePixelZ` evalúa la esquina
/// concreta del farol, y después de una fundación parte de la altura cruda.
/// Usar esa misma altura es necesario para que la caja del sorter no convierta
/// las pendientes en una coincidencia accidental.
#[allow(clippy::too_many_arguments)]
fn roadside_streetlight_parent_sprites(
    tx: i32,
    ty: i32,
    raw_base_z: u8,
    surface_base_z: u8,
    surface_tileh: u8,
    lamps: &[(usize, f32, f32)],
) -> Vec<ParentSprite> {
    lamps
        .iter()
        .enumerate()
        .map(|(index, &(lamp, dx, dy))| {
            let xmin = tx * 16 + dx as i32;
            let ymin = ty * 16 + dy as i32;
            let zmin = i32::from(raw_base_z) * 8
                + road_detail_world_z_delta(raw_base_z, surface_base_z, surface_tileh, dx, dy);
            ParentSprite::sprite(
                index as u64,
                road_streetlight_sprite_id(lamp),
                ParentSpriteBounds::new(xmin, ymin, zmin, xmin + 1, ymin + 1, zmin + 15),
            )
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn roadside_streetlight_sorted_depths(
    tx: i32,
    ty: i32,
    raw_base_z: u8,
    surface_base_z: u8,
    surface_tileh: u8,
    lamps: &[(usize, f32, f32)],
) -> Vec<f32> {
    let parents = roadside_streetlight_parent_sprites(
        tx,
        ty,
        raw_base_z,
        surface_base_z,
        surface_tileh,
        lamps,
    );
    let source_depths: Vec<_> = (0..lamps.len())
        .map(|index| {
            sortable_draw_z(
                tx,
                ty,
                surface_base_z,
                ROADSIDE_STREETLIGHT_LAYER + index as f32 * ROADSIDE_STREETLIGHT_SLOT_STEP,
            )
        })
        .collect();
    depths_in_viewport_sort_order(&parents, &source_depths)
}

/// Convierte el ancla Z absoluta que informa `AddSortableSpriteToDraw` en el
/// desplazamiento local que falta después de posicionar la superficie Bevy.
///
/// Los helpers de catenaria devuelven el delta respecto de `TileInfo::z`
/// crudo, pero [`tile_pos_half`] ya incorpora `surface_base_z`. Aplicar el
/// delta crudo dos veces sobre una fundación elevada mueve el poste/cable una
/// tesela de altura de más; omitirlo los deja pegados al mínimo de una
/// pendiente. El resultado se pasa como `dz` a [`remap_tile_offset`].
#[inline]
pub(crate) const fn catenary_local_z_delta(
    world_z_delta: i32,
    raw_base_z: u8,
    surface_base_z: u8,
) -> i32 {
    world_z_delta - (surface_base_z as i32 - raw_base_z as i32) * 8
}

/// Offset extra de las pistas de esquina PBS en `DrawTrackBits`, ya
/// multiplicado por `ZOOM_BASE`. X/Y usan el banco inclinado sin offset;
/// `Upper/Lower/Right/Left` pasan `-TILE_HEIGHT` a `DrawGroundSprite` cuando
/// la pendiente efectiva contiene su dirección.
const fn pbs_track_sprite_extra_y(track_bit: u8, surface_tileh: u8) -> i32 {
    let slope_bit = match track_bit {
        RAIL_TB_UPPER => 0x08,
        RAIL_TB_LOWER => 0x02,
        RAIL_TB_RIGHT => 0x04,
        RAIL_TB_LEFT => 0x01,
        _ => 0,
    };
    if slope_bit != 0 && surface_tileh & slope_bit != 0 {
        -32
    } else {
        0
    }
}

/// Convierte el desplazamiento de pantalla de OpenTTD (Y hacia abajo) a la
/// coordenada de Bevy (Y hacia arriba). Ambos ya están en píxeles del sprite
/// OpenGFX, por lo que sólo cambia el signo.
const fn pbs_extra_y_in_bevy(extra_y: i32) -> f32 {
    -(extra_y as f32)
}

fn record_rail_pbs_trace(sprite_id: u32, fallback: bool, mode: RailTrackTraceMode, extra_y: i32) {
    match mode {
        RailTrackTraceMode::Ground => {
            WorldDrawTrace::record_sprite_with_palette_and_geometry(
                "rail-pbs-reservation",
                "ground",
                sprite_id,
                804,
                fallback,
                (0, extra_y, 0),
                0,
                None,
            );
        }
        RailTrackTraceMode::FoundationChild((x, y, z)) => {
            WorldDrawTrace::record_foundation_child_sprite_with_palette(
                "rail-pbs-reservation",
                sprite_id,
                804,
                fallback,
                (x, y + extra_y, z),
            );
        }
    }
}

/// Geometría que recibe `AddSortableSpriteToDraw` para una señal.
///
/// `DrawSingleSignal` conserva el punto visual de `SignalPositions`, pero
/// evalúa el Z con `GetSafeSlopeZ`: en una media fundación los carriles
/// ortogonales deben leer la esquina estable y no la posición del poste.
fn signal_trace_geometry(
    tileh: u8,
    raw_base_z: u8,
    trackbits: u8,
    pos: u8,
    track: u8,
    signals_on_right: bool,
) -> ((i32, i32), i32, TraceSpriteBounds) {
    let (world_x, world_y) = signal_world_position_for_side(pos, signals_on_right);
    let (slope_x, slope_y) = signal_safe_slope_position_for_side(pos, track, signals_on_right);
    // `GetSafeSlopeZ` delega en `GetSlopePixelZ_Rail`, que aplica
    // `GetRailFoundation` antes de evaluar `GetPartialPixelZ`. Usar la
    // pendiente cruda aquí baja los postes sobre una fundación nivelada o de
    // media tesela, aunque `DrawTrackBits` ya haya elegido el sprite correcto.
    let (surface_tileh, surface_z_delta) =
        openttdrs_core::rail_surface_slope_and_z(tileh, trackbits);
    let safe_z = (i32::from(raw_base_z) + i32::from(surface_z_delta)) * 8
        + i32::from(partial_pixel_z(
            f32::from(slope_x),
            f32::from(slope_y),
            surface_tileh,
        ));
    (
        (i32::from(world_x), i32::from(world_y)),
        safe_z - i32::from(raw_base_z) * 8,
        TraceSpriteBounds::new(0, 0, 0, 1, 1, 6),
    )
}

#[allow(clippy::needless_option_as_deref, clippy::too_many_arguments)]
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
    oneway_newgrf: &[Option<openttdrs_core::DecodedSprite>],
    foundation_newgrf: &[Option<openttdrs_core::DecodedSprite>],
    mut action5_sprites: Option<&mut crate::render::NewGrfAction5SpriteCache>,
    catenary_newgrf: &[Option<openttdrs_core::DecodedSprite>],
    mut catenary_sprites: Option<&mut crate::render::NewGrfCatenarySpriteCache>,
) {
    let raw_tileh = ctx.info.tileh;
    let raw_base_z = ctx.info.base_z;
    let rb = road_bits_for_render(map, ctx.coord, mw, mh);
    let is_level_crossing = ctx
        .tile
        .is_some_and(|tile| is_road_level_crossing(tile.mapt, tile.m5, ctx.kind));
    // OpenTTD primero conserva el terreno original y luego deja que
    // `DrawFoundation` reemplace el TileInfo para todos los dibujos de la
    // carretera. Hacerlo en ese orden impide que los muros oculten el suelo
    // vecino y, a la vez, usa el sprite correcto sobre la superficie nivelada.
    if raw_tileh != 0 {
        spawn_ground_sprite(
            commands,
            &sloped_or_flat_image(raw_tileh, &assets.grass, &assets.grass_slopes),
            Color::WHITE,
            ctx,
            slope_half_ground,
        );
    }
    // El cruce a nivel no entra por `DrawRoadBits`: `DrawTile_Road` fuerza
    // una fundación nivelada y usa un único sprite de cruce, mientras que una
    // carretera normal decide su fundación a partir de RoadBits.
    let (tileh, base_z, road_foundation, foundation_child_parent) = if is_level_crossing {
        let crossing_foundation = spawn_forced_leveled_foundation_with_child_parent(
            commands,
            map,
            (mw, mh),
            assets,
            ctx,
            raw_tileh,
            "road-crossing",
            "road-crossing-foundation",
            foundation_newgrf,
            action5_sprites.as_deref_mut(),
            images.as_deref_mut(),
        );
        (
            0,
            crossing_foundation.surface_base_z,
            if raw_tileh == 0 {
                0
            } else {
                openttdrs_core::FOUNDATION_LEVELED
            },
            crossing_foundation.child_parent,
        )
    } else {
        let road_surface = spawn_road_foundation(
            commands,
            map,
            (mw, mh),
            assets,
            ctx,
            raw_tileh,
            rb,
            foundation_newgrf,
            action5_sprites.as_deref_mut(),
            images.as_deref_mut(),
        );
        (
            road_surface.surface_tileh,
            road_surface.surface_base_z,
            road_surface.foundation,
            road_surface.child_parent,
        )
    };
    let fi = road_flat_sprite_index(tileh, rb);
    let road_half_h = if tileh == 0 {
        ROAD_FLAT_HALF_H[fi]
    } else {
        slope_half_h(tileh)
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

    // NewGRF: sustituir el sprite de suelo road por la vista OpenGFX
    // (`road_flat_sprite_index`, incl. pendientes 11–14).
    let mut used_newgrf = is_level_crossing;
    let view_idx = road_newgrf_view_index(tileh, rb);
    if !is_level_crossing
        && let Some(tile) = ctx.tile
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
            let position = if tileh == 0 {
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
            let sprite = Sprite {
                image: handle,
                color: Color::WHITE,
                ..default()
            };
            if let Some(parent) = foundation_child_parent {
                spawn_foundation_child_sprite_at(commands, sprite, ctx, position, mw, parent);
            } else {
                commands.spawn((
                    MapVisualLayer,
                    ctx.map_tile_chunk(),
                    sprite,
                    Transform::from_translation(road_ground_pass_pos(position, ctx, 0.02)),
                ));
            }
            used_newgrf = true;
        }
    }

    if !used_newgrf {
        let road_set = if paved {
            &assets.road_paved
        } else {
            &assets.road_flat
        };
        // `DrawRoadGroundSprites` ocurre después de `DrawFoundation`; el
        // contrato es hijo del cimiento cuando éste existe, no un nuevo suelo
        // absoluto. La traza conserva esa relación para que el oráculo detecte
        // tanto el sprite como la transición de pendiente.
        record_road_ground_trace(
            "road-ground",
            road_ground_sprite_id(fi, paved, snow_or_desert),
            road_foundation,
        );
        let sprite = road_set[fi].sprite_colored(road_paint);
        let position =
            full_tile_sprite_pos_half(ctx.tx_i32(), ctx.ty_i32(), base_z, 0.02, road_half_h);
        if let Some(parent) = foundation_child_parent {
            spawn_foundation_child_sprite_at(commands, sprite, ctx, position, mw, parent);
        } else {
            commands.spawn((
                MapVisualLayer,
                ctx.map_tile_chunk(),
                sprite,
                Transform::from_translation(road_ground_pass_pos(position, ctx, 0.02)),
            ));
        }
    }

    // Overlay one-way (`SPR_ONEWAY_BASE` / Action5 `0x09`). El bloque base
    // vive en `openttd.grf`; no depende de que la partida tenga NewGRFs.
    if !is_level_crossing && let Some(tile) = ctx.tile {
        let drd = openttdrs_core::disallowed_road_directions(tile.m5);
        let road_x = (rb & 0x0F) == 0x0A;
        if let Some(slot) = openttdrs_core::oneway_action5_slot(tileh, road_x, drd)
            && let Some(sprite_id) = oneway_road_sprite_id(slot)
        {
            record_road_oneway_trace(sprite_id, tileh, road_foundation);
            // Un Action5 de un NewGRF puede reemplazar el fallback oficial
            // con otro recorte y otras anclas. La imagen y su metadata deben
            // viajar juntas: usar los 24x16/-12,-8 del fallback para un
            // reemplazo lo desplazaría aunque el ID de la traza sea correcto.
            let (sprite, meta) = oneway_newgrf
                .get(slot)
                .and_then(Option::as_ref)
                .and_then(|decoded| {
                    let cache = action5_sprites.as_mut()?;
                    let images = images.as_mut()?;
                    let slot = u16::try_from(slot).ok()?;
                    Some((
                        Sprite {
                            image: cache.handle_for(
                                openttdrs_core::ACTION5_TYPE_ONEWAY,
                                slot,
                                decoded,
                                images,
                            ),
                            color: Color::WHITE,
                            ..default()
                        },
                        (
                            f32::from(decoded.width),
                            f32::from(decoded.height),
                            f32::from(decoded.x_offs),
                            f32::from(decoded.y_offs),
                        ),
                    ))
                })
                .unwrap_or_else(|| {
                    (
                        assets.oneway_roads[slot].sprite(),
                        ONEWAY_ROAD_SPRITE_META[slot],
                    )
                });
            let position = road_oneway_overlay_pos(ctx, tileh, base_z, meta);
            if let Some(parent) = foundation_child_parent {
                spawn_foundation_child_sprite_at(commands, sprite, ctx, position, mw, parent);
            } else {
                commands.spawn((
                    MapVisualLayer,
                    ctx.map_tile_chunk(),
                    sprite,
                    Transform::from_translation(road_ground_pass_pos(position, ctx, 0.025)),
                ));
            }
        }
    }

    if !is_level_crossing
        && let Some(tfi) = ctx.tile.and_then(|t| tram_flat_sprite_index(tileh, t.m3))
    {
        let tram_half_h = if tileh == 0 {
            ROAD_FLAT_HALF_H[tfi]
        } else {
            slope_half_h(tileh)
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
                let sprite = Sprite {
                    image: handle,
                    color: Color::WHITE,
                    ..default()
                };
                if let Some(parent) = foundation_child_parent {
                    // El overlay de tranvía sigue a `DrawFoundation` igual que
                    // el asfalto: una vista NewGRF no puede quedar como parent
                    // independiente en una pendiente.
                    spawn_foundation_child_sprite_at(commands, sprite, ctx, pos3, mw, parent);
                } else {
                    commands.spawn((
                        MapVisualLayer,
                        ctx.map_tile_chunk(),
                        sprite,
                        Transform::from_translation(pos3),
                    ));
                }
                used_tram_newgrf = true;
            }
        }
        if !used_tram_newgrf {
            let position = tile_pos_half(
                ctx.tx_i32(),
                ctx.ty_i32(),
                base_z,
                TRAM_OVERLAY_LAYER_FRAC,
                tram_half_h,
            );
            if let Some(parent) = foundation_child_parent {
                spawn_foundation_child_sprite_at(
                    commands,
                    assets.tram_flat[tfi].sprite(),
                    ctx,
                    position,
                    mw,
                    parent,
                );
            } else {
                commands.spawn((
                    MapVisualLayer,
                    ctx.map_tile_chunk(),
                    assets.tram_flat[tfi].sprite(),
                    Transform::from_translation(position),
                ));
            }
        }
    }

    // `Roadside::StreetLights` (3): faroles de `_roadside_lamps` en sus
    // subcoordenadas de mundo. Igual que upstream, solo con 2+ road bits
    // y `FullDetail` activo.
    if !is_level_crossing
        && show_full_detail
        && roadside == Some(3)
        && rb.count_ones() > 1
        && roadside_detail_visible_under_bridge(map, ctx.coord, (mw, mh), false)
    {
        let lamps = ROADSIDE_LAMPS[usize::from(rb & 0xF)];
        let sorted_depths = roadside_streetlight_sorted_depths(
            ctx.tx_i32(),
            ctx.ty_i32(),
            raw_base_z,
            base_z,
            tileh,
            lamps,
        );
        for (lamp_index, &(lamp, dx, dy)) in lamps.iter().enumerate() {
            let (w, h, xrel, yrel) = ROAD_STREETLIGHT_META[lamp];
            let detail_z = f32::from(partial_pixel_z(dx, dy, tileh));
            WorldDrawTrace::record_sprite_with_geometry(
                "roadside-streetlight",
                "sortable",
                road_streetlight_sprite_id(lamp),
                false,
                (0, 0, 0),
                road_detail_world_z_delta(raw_base_z, base_z, tileh, dx, dy),
                Some(TraceSpriteBounds::new(dx as i32, dy as i32, 0, 2, 2, 16)),
            );
            let off = remap_tile_offset(dx, dy, detail_z) * 0.5;
            let mut pos3 = overlay_pos(
                Vec2::new(ctx.iso_pos.x + off.x, ctx.iso_pos.y + off.y),
                xrel,
                yrel,
                w,
                h,
                base_z,
                ROADSIDE_STREETLIGHT_LAYER + lamp_index as f32 * ROADSIDE_STREETLIGHT_SLOT_STEP,
                ctx.tx_i32(),
                ctx.ty_i32(),
            );
            pos3.z = sorted_depths[lamp_index];
            commands.spawn((
                MapVisualLayer,
                ctx.map_tile_chunk(),
                assets.road_streetlights[lamp].sprite(),
                Transform::from_translation(pos3),
            ));
        }
    }

    // `DrawRoadCatenary` se ejecuta para las carreteras normales antes de los
    // detalles de roadside. El bloque vanilla de tranvía incluye tanto los
    // sprites planos como los cuatro pares inclinados; hasta ahora el cliente
    // sólo los tenía en el atlas y por eso una calle electrificada quedaba sin
    // hilo/postes aunque el overlay de riel sí estuviera presente.
    if !is_level_crossing && let Some(tile) = ctx.tile.filter(|tile| tile.kind == TileKind::Road) {
        let road_bits = rb;
        let road_type = openttdrs_core::road_type_from_tile(&tile);
        spawn_road_catenary_for_type(
            commands,
            map,
            (mw, mh),
            assets,
            ctx,
            road_type,
            road_bits,
            tileh,
            base_z,
            climate,
            tile,
            road_catalog,
            road_sprites.as_deref_mut(),
            images.as_deref_mut(),
            newgrf_stack,
            catenary_newgrf,
            catenary_sprites.as_deref_mut(),
        );
        let tram_type = openttdrs_core::tram_road_type_from_tile(&tile).or_else(|| {
            (openttdrs_core::tram_track_bits(&tile) != 0).then_some(openttdrs_core::RoadType::TRAM)
        });
        if let Some(tram_type) = tram_type {
            spawn_road_catenary_for_type(
                commands,
                map,
                (mw, mh),
                assets,
                ctx,
                tram_type,
                tile.m3 & 0x0F,
                tileh,
                base_z,
                climate,
                tile,
                road_catalog,
                road_sprites.as_deref_mut(),
                images.as_deref_mut(),
                newgrf_stack,
                catenary_newgrf,
                catenary_sprites.as_deref_mut(),
            );
        }
    }

    // `Roadside::Trees` (5): árboles de `_roadside_trees` (sprite 0x1212).
    if !is_level_crossing
        && show_full_detail
        && roadside == Some(5)
        && rb.count_ones() > 1
        && roadside_detail_visible_under_bridge(map, ctx.coord, (mw, mh), true)
    {
        let (w, h, xrel, yrel) = ROADSIDE_TREE_META;
        for &(dx, dy) in ROADSIDE_TREES[usize::from(rb & 0xF)] {
            let detail_z = f32::from(partial_pixel_z(dx, dy, tileh));
            WorldDrawTrace::record_sprite_with_geometry(
                "roadside-tree",
                "sortable",
                SPR_ROADSIDE_TREE,
                false,
                (0, 0, 0),
                road_detail_world_z_delta(raw_base_z, base_z, tileh, dx, dy),
                Some(TraceSpriteBounds::new(dx as i32, dy as i32, 0, 2, 2, 16)),
            );
            let off = remap_tile_offset(dx, dy, detail_z) * 0.5;
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
    if is_level_crossing {
        let sid = ctx
            .tile
            .map(|t| {
                level_crossing_ground_sprite_id_for_type(
                    t.m5,
                    openttdrs_core::rail_type_from_tile(t),
                    paved,
                    snow_or_desert,
                )
            })
            .unwrap_or(1370);
        if let Some(img) = assets.level_crossing_ground_sprite(sid) {
            // `DrawRoadTile` pinta el bloque de vía del cruce con `PAL_NONE`.
            // La identidad visual (rail/electric/mono/maglev/tram) ya viene en
            // el sprite seleccionado; recolorearlo lo alejaba de OpenTTD.
            let crossing_paint = Color::WHITE;
            record_road_ground_trace("crossing-ground", sid, road_foundation);
            let sprite = img.sprite_colored(crossing_paint);
            let position =
                full_tile_sprite_pos_half(ctx.tx_i32(), ctx.ty_i32(), base_z, 0.045, road_half_h);
            if let Some(parent) = foundation_child_parent {
                spawn_foundation_child_sprite_at(commands, sprite, ctx, position, mw, parent);
            } else {
                commands.spawn((
                    MapVisualLayer,
                    ctx.map_tile_chunk(),
                    sprite,
                    Transform::from_translation(road_ground_pass_pos(position, ctx, 0.045)),
                ));
            }
        }
        // `DrawRoadTile`: una reserva PBS de cruce tiene su propio
        // SINGLE_X/Y con PALETTE_CRASH. Teñir la vía base hacía que todo el
        // cruce pareciera reservado y omitía la selección tipada mono/maglev.
        if show_pbs_reservations
            && let Some(t) = ctx
                .tile
                .filter(|tile| level_crossing_has_rail_reservation(tile.m5))
        {
            let rail_axis = 1 - (t.m5 & 1);
            let sid = remap_rail_sprite_id(
                1005 + u32::from(rail_axis),
                openttdrs_core::rail_type_from_tile(t),
            );
            WorldDrawTrace::record_sprite_with_palette_and_geometry(
                "crossing-pbs-reservation",
                "ground",
                sid,
                804,
                !assets.has_exact_pbs_rail_sprite(sid),
                (0, 0, 0),
                0,
                None,
            );
            if let Some(img) = assets.pbs_rail_sprite(sid) {
                let base = full_tile_sprite_pos_half(
                    ctx.tx_i32(),
                    ctx.ty_i32(),
                    base_z,
                    0.048,
                    road_half_h,
                );
                let offset = rail_ghost_overlay_offset(sid);
                commands.spawn((
                    MapVisualLayer,
                    ctx.map_tile_chunk(),
                    img.sprite(),
                    Transform::from_translation(road_ground_pass_pos(
                        base + Vec3::new(offset.x, offset.y, 0.0),
                        ctx,
                        0.048,
                    )),
                ));
            }
        }

        // `DrawTile_Road` termina el cruce con `DrawRailCatenary`. Aunque la
        // tesela sea MP_ROAD, `GetRailTrackBitsUniversal` la trata como una
        // vía especial: conserva el cable y los postes del railtype, no la
        // catenaria vial. Reusar el selector PCP/PPP también cubre mono y
        // maglev que tengan un railtype electrificado definido por NewGRF.
        if let Some(tile) = ctx.tile {
            let rail_type = openttdrs_core::rail_type_from_tile(tile);
            let render_tb = rail_trackbits_for_render(map, ctx.coord, mw, mh);
            spawn_rail_catenary_for_surface(
                commands,
                map,
                (mw, mh),
                assets,
                ctx,
                rail_type,
                render_tb,
                tileh,
                base_z,
                true,
                catenary_newgrf,
                &mut catenary_sprites,
                &mut images,
            );
        }
    }
}

/// Comprueba la regla especial de `DrawRoadTypeCatenary`: en una unión de más
/// de dos brazos sólo se conserva el extremo cuyo vecino también tiene algún
/// tipo de carretera/tranvía electrificado. Si quedan menos de dos extremos,
/// OpenTTD mantiene la máscara original para no borrar una catenaria corta.
fn road_catenary_bits_for_render(
    map: &Map,
    coord: TileCoord,
    dims: (u32, u32),
    bits: u8,
    road_catalog: &[RoadTypeDef],
) -> u8 {
    if bits.count_ones() <= 2 {
        return bits & 0x0F;
    }
    let mut filtered = 0;
    for (bit, (dx, dy)) in [
        (0x01, (0, -1)), // NW
        (0x02, (1, 0)),  // SW
        (0x04, (0, 1)),  // SE
        (0x08, (-1, 0)), // NE
    ] {
        if bits & bit == 0 {
            continue;
        }
        let neighbour = TileCoord::new(coord.x + dx, coord.y + dy);
        if neighbour.x < 0
            || neighbour.y < 0
            || neighbour.x >= dims.0 as i32
            || neighbour.y >= dims.1 as i32
        {
            continue;
        }
        let Some(tile) = map.get(neighbour) else {
            continue;
        };
        if !matches!(tile.kind, TileKind::Road | TileKind::Station) {
            continue;
        }
        let road_electric =
            openttdrs_core::road_type_def(road_catalog, openttdrs_core::road_type_from_tile(&tile))
                .is_some_and(RoadTypeDef::has_catenary);
        let tram_electric = openttdrs_core::tram_road_type_from_tile(&tile)
            .or_else(|| {
                (openttdrs_core::tram_track_bits(&tile) != 0)
                    .then_some(openttdrs_core::RoadType::TRAM)
            })
            .and_then(|rt| openttdrs_core::road_type_def(road_catalog, rt))
            .is_some_and(RoadTypeDef::has_catenary);
        if road_electric || tram_electric {
            filtered |= bit;
        }
    }
    if filtered.count_ones() >= 2 {
        filtered
    } else {
        bits & 0x0F
    }
}

#[allow(clippy::too_many_arguments)]
fn custom_road_catenary_sprite(
    def: &RoadTypeDef,
    selector: u8,
    view_idx: usize,
    map: &Map,
    coord: TileCoord,
    tile: Tile,
    climate: Climate,
    road_catalog: &[RoadTypeDef],
    newgrf_stack: &[openttdrs_core::NewGrfEntry],
    road_sprites: &mut Option<&mut NewGrfRoadSpriteCache>,
    images: &mut Option<&mut Assets<Image>>,
    tint: Color,
) -> Option<(Sprite, CatenarySpriteAnchor)> {
    let cache = road_sprites.as_deref_mut()?;
    let image_store = images.as_deref_mut()?;
    let mut action2 = openttdrs_core::action2_eval_ctx_for_road_tile(
        map,
        tile,
        coord,
        climate,
        def.newgrf_type_tables.as_ref(),
        road_catalog,
    );
    action2.set_grf_params(openttdrs_core::stack_params_for_grfid(
        newgrf_stack,
        def.newgrf_grfid,
    ));
    let view = def.newgrf_specific_view_runtime(selector, view_idx, &mut action2)?;
    let handle =
        cache.handle_for_specific_runtime(def, selector, view_idx, &mut action2, image_store)?;
    let anchor = CatenarySpriteAnchor::from_decoded(&view);
    Some((
        Sprite {
            image: handle,
            color: tint,
            ..default()
        },
        anchor,
    ))
}

#[allow(clippy::needless_option_as_deref, clippy::too_many_arguments)]
fn spawn_road_catenary_for_type(
    commands: &mut Commands,
    map: &Map,
    dims: (u32, u32),
    assets: &WorldAssets,
    ctx: &TileRenderContext,
    road_type: openttdrs_core::RoadType,
    road_bits: u8,
    tileh: u8,
    surface_base_z: u8,
    climate: Climate,
    tile: Tile,
    road_catalog: &[RoadTypeDef],
    mut road_sprites: Option<&mut NewGrfRoadSpriteCache>,
    mut images: Option<&mut Assets<Image>>,
    newgrf_stack: &[openttdrs_core::NewGrfEntry],
    catenary_newgrf: &[Option<openttdrs_core::DecodedSprite>],
    mut catenary_sprites: Option<&mut crate::render::NewGrfCatenarySpriteCache>,
) {
    if catenary_hidden() {
        return;
    }
    let Some(def) = openttdrs_core::road_type_def(road_catalog, road_type) else {
        return;
    };
    if !def.has_catenary() {
        return;
    }
    let road_bits = road_catenary_bits_for_render(map, ctx.coord, dims, road_bits, road_catalog);
    let Some((fallback_back, fallback_front)) = road_catenary_sprite_ids(tileh, road_bits) else {
        return;
    };
    let view_idx = road_newgrf_view_index(tileh, road_bits);
    let tint = catenary_sprite_color();
    let custom_back = def.has_newgrf_specific_group(5);
    let custom_front = def.has_newgrf_specific_group(4);
    let custom_any = custom_back || custom_front;
    let (back_resolved, back_fallback) = if custom_any {
        let resolved = custom_back.then(|| {
            custom_road_catenary_sprite(
                def,
                5,
                view_idx,
                map,
                ctx.coord,
                tile,
                climate,
                road_catalog,
                newgrf_stack,
                &mut road_sprites,
                &mut images,
                tint,
            )
        });
        (resolved.flatten(), false)
    } else {
        (
            catenary_sprite_colored(
                assets,
                fallback_back,
                tint,
                catenary_newgrf,
                catenary_sprites.as_deref_mut(),
                images.as_deref_mut(),
            )
            .zip(catenary_sprite_anchor(fallback_back, catenary_newgrf)),
            true,
        )
    };
    let (front_resolved, front_fallback) = if custom_any {
        let resolved = custom_front.then(|| {
            custom_road_catenary_sprite(
                def,
                4,
                view_idx,
                map,
                ctx.coord,
                tile,
                climate,
                road_catalog,
                newgrf_stack,
                &mut road_sprites,
                &mut images,
                tint,
            )
        });
        (resolved.flatten(), false)
    } else {
        (
            catenary_sprite_colored(
                assets,
                fallback_front,
                tint,
                catenary_newgrf,
                catenary_sprites.as_deref_mut(),
                images.as_deref_mut(),
            )
            .zip(catenary_sprite_anchor(fallback_front, catenary_newgrf)),
            true,
        )
    };

    let z_wires = if tileh == 0 { 0 } else { 8 } + 2;
    let west_z = i32::from(partial_pixel_z(15.0, 0.0, tileh));
    let north_z = i32::from(partial_pixel_z(0.0, 0.0, tileh));
    let east_z = i32::from(partial_pixel_z(0.0, 15.0, tileh));
    let base_z_delta = (i32::from(surface_base_z) - i32::from(ctx.info.base_z)) * 8;

    if let Some((sprite, anchor)) = back_resolved {
        for (index, (left, right, bounds, offset)) in [
            (
                None,
                Some(-12.0),
                TraceSpriteBounds::new(15, 0, west_z, 1, 1, z_wires),
                (-15, 0, -west_z),
            ),
            (
                Some(-12.0),
                Some(12.0),
                TraceSpriteBounds::new(0, 0, north_z, 1, 1, z_wires),
                (0, 0, -north_z),
            ),
            (
                Some(12.0),
                None,
                TraceSpriteBounds::new(0, 15, east_z, 1, 1, z_wires),
                (0, -15, -east_z),
            ),
        ]
        .into_iter()
        .enumerate()
        {
            let Some((sprite, x_shift)) =
                catenary_sprite_horizontal_crop(sprite.clone(), anchor, left, right)
            else {
                continue;
            };
            WorldDrawTrace::record_sprite_with_palette_and_world_geometry(
                "road-catenary-back",
                "sortable",
                fallback_back,
                0,
                back_fallback,
                (0, 0),
                base_z_delta,
                offset,
                Some(bounds),
            );
            let mut position = catenary_sprite_center(
                ctx.tx_i32(),
                ctx.ty_i32(),
                surface_base_z,
                0.034 + index as f32 * 0.0001,
                0.0,
                0.0,
                0.0,
                anchor,
            );
            position.x += x_shift;
            commands.spawn((
                MapVisualLayer,
                ctx.map_tile_chunk(),
                sprite,
                Transform::from_translation(position),
            ));
        }
    }
    if let Some((sprite, anchor)) = front_resolved {
        WorldDrawTrace::record_sprite_with_palette_and_world_geometry(
            "road-catenary-front",
            "sortable",
            fallback_front,
            0,
            front_fallback,
            (0, 0),
            base_z_delta,
            (0, 0, -z_wires),
            Some(TraceSpriteBounds::new(0, 0, z_wires, 16, 16, 1)),
        );
        let position = catenary_sprite_center(
            ctx.tx_i32(),
            ctx.ty_i32(),
            surface_base_z,
            0.04,
            0.0,
            0.0,
            0.0,
            anchor,
        );
        commands.spawn((
            MapVisualLayer,
            ctx.map_tile_chunk(),
            sprite,
            Transform::from_translation(position),
        ));
    }
}

/// Emite la catenaria ferroviaria común de `DrawRailCatenaryRailway`.
///
/// Las vías normales, los cruces a nivel y las bocas de túnel comparten el
/// algoritmo de PCP/PPP de OpenTTD. El último caso usa su cable de entrada
/// especial, por lo que puede pedir sólo los postes con `draw_wires=false`.
/// Centralizar la rama evita que un cruce eléctrico vuelva a verse como un
/// simple cruce sin cable mientras la misma topología sí funciona en vía
/// normal.
#[allow(clippy::too_many_arguments)]
pub(crate) fn spawn_rail_catenary_for_surface(
    commands: &mut Commands,
    map: &Map,
    map_dims: (u32, u32),
    assets: &WorldAssets,
    ctx: &TileRenderContext,
    rail_type: openttdrs_core::RailType,
    render_tb: u8,
    tileh: u8,
    surface_base_z: u8,
    draw_wires: bool,
    catenary_newgrf: &[Option<openttdrs_core::DecodedSprite>],
    catenary_sprites: &mut Option<&mut crate::render::NewGrfCatenarySpriteCache>,
    images: &mut Option<&mut Assets<Image>>,
) {
    if !rail_type.has_catenary() {
        return;
    }

    let low_bridge = catenary_under_low_bridge(map, ctx.coord, map_dims);
    let tint = catenary_sprite_color();
    let mut wires = Vec::new();
    if draw_wires && !low_bridge.hide_wires {
        collect_catenary_wire_draws_from_map(
            map,
            ctx.coord,
            map_dims.0,
            map_dims.1,
            crate::sprites::OTTD_MP_RAIL,
            render_tb,
            tileh,
            &mut wires,
        );
    }
    let mut pylons = Vec::new();
    collect_catenary_pylons_from_map_with_pcp_override(
        map,
        ctx.coord,
        map_dims.0,
        map_dims.1,
        crate::sprites::OTTD_MP_RAIL,
        render_tb,
        tileh,
        low_bridge.pylon_pcp_override,
        &mut pylons,
    );
    // Para una boca de túnel el algoritmo C++ considera el borde que mira
    // hacia el interior como `TRACK_BIT_NONE`. El colector genérico conserva
    // ambos extremos para vías y cruces, así que filtramos aquí —único
    // consumidor de túneles— el PPP opuesto a `m5`. Sin esto aparecían postes
    // dobles dentro de la boca (Kale: 170,81; 180,127).
    if let Some(exterior_pcp) = ctx
        .tile
        .filter(|tile| tile.kind == TileKind::RailTunnel)
        .map(|tile| catenary_tunnel_exterior_pcp(tile.m5))
    {
        pylons.retain(|draw| draw.pcp_direction == Some(exterior_pcp));
    }
    for draw in pylons {
        let anchor = catenary_sprite_anchor(draw.sprite_id, catenary_newgrf);
        let sprite = catenary_sprite_colored(
            assets,
            draw.sprite_id,
            tint,
            catenary_newgrf,
            catenary_sprites.as_deref_mut(),
            images.as_deref_mut(),
        );
        let world_z_delta = draw.pcp_direction.map_or(0, |pcp| {
            catenary_pylon_world_z_delta(tileh, ctx.info.base_z, render_tb, pcp)
        });
        WorldDrawTrace::record_sprite_with_palette_and_world_geometry(
            "catenary-pylon",
            "sortable",
            catenary_reference_sprite_id(draw.sprite_id),
            0,
            sprite.is_none(),
            (draw.tile_dx as i32, draw.tile_dy as i32),
            world_z_delta,
            (1, 1, 0),
            Some(TraceSpriteBounds::new(-1, -1, 0, 1, 1, 6)),
        );
        let Some((sprite, anchor)) = sprite.zip(anchor) else {
            continue;
        };
        // `AddSortableSpriteToDraw` traslada primero el origen del sprite por
        // el `SpriteBounds` del poste. Aunque su caja sea diminuta, `(-1,-1)`
        // cambia el píxel de anclaje; omitirlo desplazaba todos los PPP.
        let local_z = catenary_local_z_delta(world_z_delta, ctx.info.base_z, surface_base_z);
        let position = catenary_sprite_center(
            ctx.tx_i32(),
            ctx.ty_i32(),
            surface_base_z,
            draw.z_layer,
            draw.tile_dx - 1.0,
            draw.tile_dy - 1.0,
            local_z as f32,
            anchor,
        );
        commands.spawn((
            MapVisualLayer,
            ctx.map_tile_chunk(),
            sprite,
            Transform::from_translation(position),
        ));
    }
    // OpenTTD emite primero los postes PPP y después los cables PCP. El
    // orden estable también permite comparar el stream sortable del oráculo.
    for (i, draw) in wires.iter().copied().enumerate() {
        let sid = draw.sprite_id;
        let anchor = catenary_sprite_anchor(sid, catenary_newgrf);
        let sprite = catenary_sprite_colored(
            assets,
            sid,
            tint,
            catenary_newgrf,
            catenary_sprites.as_deref_mut(),
            images.as_deref_mut(),
        );
        let (ox, oy, oz) = draw.bounds_origin;
        let (ex, ey, ez) = draw.bounds_extent;
        let world_z_delta = catenary_wire_world_z_delta(tileh, ctx.info.base_z, render_tb, draw);
        WorldDrawTrace::record_sprite_with_palette_and_geometry(
            "catenary-wire",
            "sortable",
            catenary_reference_sprite_id(sid),
            0,
            sprite.is_none(),
            (0, 0, 0),
            world_z_delta,
            Some(TraceSpriteBounds::new(ox, oy, oz, ex, ey, ez)),
        );
        let Some((sprite, anchor)) = sprite.zip(anchor) else {
            continue;
        };
        let z = 0.035 + i as f32 * 0.0004;
        // El `SortableSpriteStruct` de cada cable aporta un origen 3D. Es
        // parte del ancla del PNG (no sólo del sorter): en vía plana `oz=10`
        // eleva el hilo exactamente sobre la vía.
        let local_z = catenary_local_z_delta(world_z_delta + oz, ctx.info.base_z, surface_base_z);
        let position = catenary_sprite_center(
            ctx.tx_i32(),
            ctx.ty_i32(),
            surface_base_z,
            z,
            ox as f32,
            oy as f32,
            local_z as f32,
            anchor,
        );
        commands.spawn((
            MapVisualLayer,
            ctx.map_tile_chunk(),
            sprite,
            Transform::from_translation(position),
        ));
    }
}

// Offsets `RailTrackOffset` de OpenTTD. El grupo Action3 de underlay/overlay
// comparte esta tabla: los cinco últimos índices son las variantes de cruce.
const RTO_X: u8 = 0;
const RTO_Y: u8 = 1;
const RTO_N: u8 = 2;
const RTO_S: u8 = 3;
const RTO_E: u8 = 4;
const RTO_W: u8 = 5;
const RTO_CROSSING_XY: u8 = 10;
const RTO_JUNCTION_SW: u8 = 11;
const RTO_JUNCTION_NE: u8 = 12;
const RTO_JUNCTION_SE: u8 = 13;
const RTO_JUNCTION_NW: u8 = 14;
const RTO_JUNCTION_NSEW: u8 = 15;

const RAIL_3WAY_NE: u8 = RAIL_TB_X | RAIL_TB_UPPER | RAIL_TB_RIGHT;
const RAIL_3WAY_SW: u8 = RAIL_TB_X | RAIL_TB_LOWER | RAIL_TB_LEFT;
const RAIL_3WAY_NW: u8 = RAIL_TB_Y | RAIL_TB_UPPER | RAIL_TB_LEFT;
const RAIL_3WAY_SE: u8 = RAIL_TB_Y | RAIL_TB_LOWER | RAIL_TB_RIGHT;

/// Devuelve el offset de underlay para una pasada de `DrawTrackBitsOverlay`.
///
/// Para una recta/corner se emite una imagen que ya contiene suelo y vía. En
/// cruces el underlay sólo contiene el lastre y las piezas se agregan con el
/// grupo overlay, exactamente como en `rail_cmd.cpp`.
fn rail_custom_underlay_offsets(track_bits: u8) -> Vec<u8> {
    let bits = track_bits & 0x3F;
    if bits == 0 {
        return Vec::new();
    }
    match bits {
        RAIL_TB_X => vec![RTO_X],
        RAIL_TB_Y => vec![RTO_Y],
        RAIL_TB_UPPER => vec![RTO_N],
        RAIL_TB_LOWER => vec![RTO_S],
        RAIL_TB_RIGHT => vec![RTO_E],
        RAIL_TB_LEFT => vec![RTO_W],
        RAIL_TB_CROSS => vec![RTO_CROSSING_XY],
        RAIL_TB_HORZ => vec![RTO_N, RTO_S],
        RAIL_TB_VERT => vec![RTO_E, RTO_W],
        bits => {
            let offset = if bits & RAIL_3WAY_NE == 0 {
                RTO_JUNCTION_SW
            } else if bits & RAIL_3WAY_SW == 0 {
                RTO_JUNCTION_NE
            } else if bits & RAIL_3WAY_NW == 0 {
                RTO_JUNCTION_SE
            } else if bits & RAIL_3WAY_SE == 0 {
                RTO_JUNCTION_NW
            } else {
                RTO_JUNCTION_NSEW
            };
            vec![offset]
        }
    }
}

/// Piezas que OpenTTD dibuja con `RTSG_OVERLAY` en una pasada normal. Sólo
/// las uniones necesitan la capa de vía; las rectas ya vienen completas en el
/// underlay. PBS se agrega aparte y usa siempre estos mismos índices.
fn rail_custom_overlay_offsets(track_bits: u8) -> Vec<(u8, u8)> {
    let bits = track_bits & 0x3F;
    if matches!(
        bits,
        RAIL_TB_X
            | RAIL_TB_Y
            | RAIL_TB_UPPER
            | RAIL_TB_LOWER
            | RAIL_TB_RIGHT
            | RAIL_TB_LEFT
            | RAIL_TB_CROSS
            | RAIL_TB_HORZ
            | RAIL_TB_VERT
    ) {
        return Vec::new();
    }
    [
        (RAIL_TB_X, RTO_X),
        (RAIL_TB_Y, RTO_Y),
        (RAIL_TB_UPPER, RTO_N),
        (RAIL_TB_LOWER, RTO_S),
        (RAIL_TB_RIGHT, RTO_E),
        (RAIL_TB_LEFT, RTO_W),
    ]
    .into_iter()
    .filter(|(track, _)| bits & track != 0)
    .collect()
}

/// Offset de `DrawTrackSprite`: las piezas de esquina se desplazan una
/// altura de vía cuando la pendiente efectiva contiene su dirección.
fn rail_custom_track_extra_y(offset: u8, surface_tileh: u8) -> i32 {
    let track_bit = match offset {
        RTO_N => RAIL_TB_UPPER,
        RTO_S => RAIL_TB_LOWER,
        RTO_E => RAIL_TB_RIGHT,
        RTO_W => RAIL_TB_LEFT,
        _ => 0,
    };
    pbs_track_sprite_extra_y(track_bit, surface_tileh)
}

const fn rail_custom_offset_for_track_bit(track_bit: u8) -> Option<u8> {
    match track_bit {
        RAIL_TB_X => Some(RTO_X),
        RAIL_TB_Y => Some(RTO_Y),
        RAIL_TB_UPPER => Some(RTO_N),
        RAIL_TB_LOWER => Some(RTO_S),
        RAIL_TB_RIGHT => Some(RTO_E),
        RAIL_TB_LEFT => Some(RTO_W),
        _ => None,
    }
}

/// Índice del grupo `RTSG_GROUND_COMPLETE` para una combinación de vías.
/// OpenTTD usa el bitmask directamente y deja libre el índice cero para
/// `TRACK_BIT_NONE`.
const fn rail_ground_complete_offset(track_bits: u8) -> Option<u8> {
    let bits = track_bits & 0x3F;
    if bits == 0 { None } else { Some(bits - 1) }
}

#[allow(clippy::too_many_arguments)]
fn resolve_custom_rail_group_sprite(
    map: &Map,
    tile: openttdrs_core::Tile,
    ctx: &TileRenderContext,
    climate: Climate,
    calendar_date: u32,
    newgrf_stack: &[openttdrs_core::NewGrfEntry],
    spec: &openttdrs_core::RailSignalSpriteSpec,
    image: u8,
    signal_sprites: &mut Option<&mut crate::render::NewGrfSignalSpriteCache>,
    images: &mut Option<&mut Assets<Image>>,
) -> Option<crate::render::signal_newgrf::ResolvedSignalSprite> {
    let cache = signal_sprites.as_deref_mut()?;
    let images = images.as_deref_mut()?;
    let mut action2 = openttdrs_core::action2_eval_ctx_for_rail_tile(
        map,
        tile,
        ctx.coord,
        climate,
        calendar_date,
        spec.type_tables.as_ref(),
    );
    action2.set_grf_params(openttdrs_core::stack_params_for_grfid(
        newgrf_stack,
        spec.grfid,
    ));
    cache.sprite_for_group(spec, image, &mut action2, images)
}

/// Emite una vista Action3 de rail con el ancla NFO original y la relación
/// parent/child de la fundación activa. `center_offset` se calcula desde
/// `(x_offs, y_offs, width, height)`, por lo que no se impone el 64×31 de
/// OpenGFX a un sprite NewGRF HD.
#[allow(clippy::too_many_arguments)]
fn spawn_custom_rail_sprite(
    commands: &mut Commands,
    ctx: &TileRenderContext,
    resolved: crate::render::signal_newgrf::ResolvedSignalSprite,
    base_z: u8,
    layer: f32,
    extra_y: i32,
    halftile_corner: Option<u8>,
    half_h: f32,
    foundation_child_parent: Option<Entity>,
    map_width: u32,
    role: &'static str,
    trace_image: u8,
) {
    let mut sprite = resolved.sprite;
    let crop_shift = halftile_track_subsprite_with_center(
        halftile_corner,
        resolved.size,
        resolved.center_offset,
        half_h,
    )
    .map_or(Vec2::ZERO, |(rect, shift)| {
        sprite.rect = Some(rect);
        shift
    });
    let elevation = f32::from(base_z) * HEIGHT_PX;
    let mut position = Vec3::new(
        ctx.iso_pos.x + resolved.center_offset.x,
        ctx.iso_pos.y + resolved.center_offset.y + elevation,
        sortable_draw_z(ctx.tx_i32(), ctx.ty_i32(), base_z, layer),
    );
    position.y += pbs_extra_y_in_bevy(extra_y) + crop_shift.y;
    position.x += crop_shift.x;
    WorldDrawTrace::record_sprite_with_palette_and_geometry(
        role,
        "sortable",
        u32::from(trace_image),
        0,
        false,
        (0, extra_y, 0),
        0,
        None,
    );
    if let Some(parent) = foundation_child_parent {
        spawn_foundation_child_sprite_at(commands, sprite, ctx, position, map_width, parent);
    } else {
        commands.spawn((
            MapVisualLayer,
            ctx.map_tile_chunk(),
            sprite,
            Transform::from_translation(position),
        ));
    }
}

#[allow(clippy::too_many_arguments, clippy::needless_option_as_deref)]
pub(crate) fn spawn_rail_tile(
    commands: &mut Commands,
    map: &Map,
    map_dims: (u32, u32),
    assets: &WorldAssets,
    company: Option<&CompanyColoredSprites>,
    owner_colour: Option<CompanyColour>,
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
    rail_type_underlay_newgrf: &[Option<openttdrs_core::RailSignalSpriteSpec>],
    rail_type_overlay_newgrf: &[Option<openttdrs_core::RailSignalSpriteSpec>],
    rail_type_ground_complete_newgrf: &[Option<openttdrs_core::RailSignalSpriteSpec>],
    rail_type_props: &[openttdrs_core::RailTypeRuntimeProps; 4],
    mut signal_sprites: Option<&mut crate::render::NewGrfSignalSpriteCache>,
    signal_action5: &[Option<openttdrs_core::DecodedSprite>],
    foundation_newgrf: &[Option<openttdrs_core::DecodedSprite>],
    mut action5_sprites: Option<&mut crate::render::NewGrfAction5SpriteCache>,
    mut images: Option<&mut Assets<Image>>,
    calendar_date: u32,
    newgrf_stack: &[openttdrs_core::NewGrfEntry],
) {
    let tileh = ctx.info.tileh;
    let render_tb = rail_trackbits_for_render(map, ctx.coord, map_dims.0, map_dims.1);
    let rail_type = ctx
        .tile
        .map(openttdrs_core::rail_type_from_tile)
        .unwrap_or_default();
    let rail_type_index = usize::from(rail_type.as_u8());
    let underlay_spec = rail_type_underlay_newgrf
        .get(rail_type_index)
        .and_then(Option::as_ref);
    let overlay_spec = rail_type_overlay_newgrf
        .get(rail_type_index)
        .and_then(Option::as_ref);
    let ground_complete_spec = rail_type_ground_complete_newgrf
        .get(rail_type_index)
        .and_then(Option::as_ref);
    let no_sprite_combine = tileh == 0
        && rail_type_props
            .get(rail_type_index)
            .is_some_and(|props| props.no_sprite_combine());
    // `RailTypeInfo::UsesOverlay()` no depende de que el tipo sea mono o
    // maglev: se activa cuando el railtype publica su grupo `Ground`. El
    // selector 2 es el valor upstream; el selector 0 se conserva como
    // compatibilidad con fixtures antiguos del parser local. `GroundComplete`
    // sólo sustituye el sprite combinado en la rama plana y no debe forzar un
    // segundo rombo de suelo en pendientes.
    let rail_uses_overlay = underlay_spec.is_some();
    let rail_has_custom_overlay =
        rail_uses_overlay || (no_sprite_combine && ground_complete_spec.is_some());
    // `GetRailGroundType` lee los cuatro bits bajos de m4. En el mapa Rust
    // m4 se llama `m3hi`; leer `m3` confundía el tipo de vía/señales con
    // nieve y dejaba el suelo de mono/maglev desalineado del oráculo.
    let rail_ground_type = ctx.tile.map_or_else(
        || {
            if climate.uses_snow_ground() {
                RAIL_GROUND_SNOW_OR_DESERT
            } else {
                0
            }
        },
        |t| t.m3hi & 0x0F,
    );
    let snow_ground = rail_ground_type == RAIL_GROUND_SNOW_OR_DESERT;
    let rail_foundation = openttdrs_core::rail_foundation_for_trackbits(tileh, render_tb);
    let track_plan = openttdrs_core::rail_track_draw_plan(tileh, render_tb);
    let initial_ground = rail_initial_ground_draw(
        tileh,
        rail_foundation,
        track_plan,
        rail_uses_overlay,
        rail_ground_type,
    );

    // `IsBridgeAbove` no reemplaza el contenido de la tesela: OpenTTD pinta
    // primero la vía inferior y después el tablero elevado. El tablero se
    // agrega separadamente por `spawn_bridge_middle` en `tile_spawn.rs`.
    // Saltar esta rama hacía desaparecer vías reales bajo puentes y dejaba
    // sus reservas PBS, túneles y conexiones aparentemente cortados.
    if let Some(initial) = initial_ground
        && initial.trace_mode == RailTrackTraceMode::Ground
    {
        record_rail_ground_trace(
            initial.draw,
            initial.trace_mode,
            i32::from(initial.surface_z_delta) * 8,
        );
        spawn_rail_ground_draw(
            commands,
            assets,
            ctx,
            initial.draw,
            ctx.info.base_z.saturating_add(initial.surface_z_delta),
            None,
            map_dims.0,
        );
    } else if tileh != 0 {
        // El sprite de vía clásica ya incluye suelo; esta base visual antigua
        // llena sus transparencias inclinadas. Las ramas con suelo explícito
        // (water/shore, mono/maglev y mitad vacía) se manejan arriba con su
        // selección exacta, para no dejarlas cubiertas por pasto.
        spawn_ground_sprite(
            commands,
            &sloped_or_flat_image(tileh, &assets.grass, &assets.grass_slopes),
            Color::WHITE,
            ctx,
            slope_half_ground,
        );
    }
    let foundation_after_pass = rail_foundation_after_pass(track_plan);
    // En una fundación no continua el primer `DrawFoundation` es NONE o
    // STEEP_LOWER; el cimiento visible se crea recién entre las dos pasadas.
    // Aun así catenaria y señales posteriores necesitan conocer desde ahora
    // la altura final de la superficie.
    let mut foundation_child_parent = None;
    let rail_base_z = if foundation_after_pass.is_some() {
        let (_, z_delta) = openttdrs_core::rail_surface_slope_and_z(tileh, render_tb);
        ctx.info.base_z.saturating_add(z_delta)
    } else {
        let foundation = spawn_rail_foundation(
            commands,
            map,
            map_dims,
            assets,
            ctx,
            tileh,
            render_tb,
            foundation_newgrf,
            action5_sprites.as_deref_mut(),
            images.as_deref_mut(),
        );
        foundation_child_parent = foundation.child_parent;
        foundation.surface_base_z
    };
    if let Some(initial) = initial_ground
        && let RailTrackTraceMode::FoundationChild(_) = initial.trace_mode
    {
        // La fundación ya dejó el parent sortable activo. Emitir después el
        // suelo coincide con `DrawFoundation` seguido de `DrawGroundSprite`.
        record_rail_ground_trace(initial.draw, initial.trace_mode, 0);
        spawn_rail_ground_draw(
            commands,
            assets,
            ctx,
            initial.draw,
            rail_base_z,
            foundation_child_parent,
            map_dims.0,
        );
    }
    let (surface_tileh, _) = openttdrs_core::rail_surface_slope_and_z(tileh, render_tb);
    let render_tileh = if surface_tileh & 0x20 != 0 {
        tileh
    } else {
        surface_tileh
    };
    let rail_half_h = if render_tileh == 0 {
        TILE_HALF_H
    } else {
        slope_half_h(render_tileh)
    };
    // Conservamos el límite entre las dos pasadas de `DrawTrackBits`: para
    // una media fundación cambia el padre activo entre la mitad baja y alta.
    // Los dos arrays son de tamaño fijo porque core garantiza como máximo dos
    // pasadas; así la traza no agrega una asignación por tesela.
    rail_layers.clear();
    let mut pass_ends = [0_usize; 2];
    let mut pass_modes = [RailTrackTraceMode::Ground; 2];
    let mut pass_base_z = [rail_base_z; 2];
    let mut pass_half_h = [rail_half_h; 2];
    let mut pass_halftile_corner = [None; 2];
    let mut pass_tileh = [render_tileh; 2];
    let mut pass_count = 0_usize;
    for pass in track_plan.passes.into_iter().flatten() {
        collect_rail_sprites_for_surface(
            pass.track_bits,
            pass.sprite_tileh,
            snow_ground,
            rail_type,
            rail_layers,
        );
        pass_modes[pass_count] = rail_track_trace_mode(rail_foundation, pass.halftile_corner);
        pass_base_z[pass_count] = ctx.info.base_z.saturating_add(pass.z_delta);
        pass_half_h[pass_count] = if pass.sprite_tileh == 0 {
            TILE_HALF_H
        } else {
            slope_half_h(pass.sprite_tileh)
        };
        pass_halftile_corner[pass_count] = pass.halftile_corner;
        pass_tileh[pass_count] = pass.sprite_tileh;
        pass_ends[pass_count] = rail_layers.len();
        pass_count += 1;
    }
    let typed_layers = rail_layers
        .iter()
        .any(|&sid| is_typed_rail_track_sprite(sid));
    // En mono/maglev todo el bloque de vía vanilla tiene una variante tipada.
    // Si el selector no produjo ninguna, no debemos ocultarlo detrás de un
    // sprite de rail normal: la traza lo marca con railtype y coordenada.
    let typed_selection_fallback = matches!(
        rail_type,
        openttdrs_core::RailType::Monorail | openttdrs_core::RailType::Maglev
    ) && !typed_layers;
    // `DrawTrackBits` usa `PAL_NONE` para la vía base, inclusive en vías
    // electrificadas, mono/maglev y teselas con señales. El tipo está en el
    // ID del sprite: el tinte sintético lo ocultaba tras azul/violeta.
    let rail_paint = Color::WHITE;
    let reservation_draws = show_pbs_reservations.then(|| {
        let reservation_bits = ctx.tile.map_or(0, |tile| {
            openttdrs_core::decode_rail_reservation_m2_hi(tile.m2_hi)
        });
        collect_rail_pbs_reservation_draws(render_tb, reservation_bits, tileh, rail_type)
    });
    let mut track_layer_index = 0_usize;
    let mut pbs_layer_index = 0_usize;
    for pass_index in 0..pass_count {
        if let Some(upper_ground) = rail_upper_halftile_ground_draw(
            rail_uses_overlay,
            rail_ground_type,
            pass_tileh[pass_index],
        )
        .filter(|_| pass_halftile_corner[pass_index].is_some())
        {
            // `DrawTrackBitsOverlay` recorta este suelo al triángulo alto con
            // SubSprite. La traza conserva la selección y su relación con la
            // fundación; visualmente ya mantenemos el backing de pendiente
            // para no dibujar un rombo completo sobre la mitad baja.
            record_rail_ground_trace(upper_ground, pass_modes[pass_index], 0);
        }
        let start = if pass_index == 0 {
            0
        } else {
            pass_ends[pass_index - 1]
        };
        let end = pass_ends[pass_index];
        // `DrawTrackBitsOverlay` no reutiliza los sprites combinados del
        // baseset cuando el railtype publica `Underlay`: resuelve el grupo
        // Action3 para cada `RailTrackOffset` y agrega la vía como una capa
        // separada. Antes sólo se usaba el underlay como booleano, por lo que
        // un railtype NewGRF terminaba mostrando la vía vanilla completa.
        let mut custom_ground_complete = false;
        let mut custom_track_drawn = false;
        if no_sprite_combine {
            // `DrawTrackBits` selecciona `RTSG_GROUND_COMPLETE` con el
            // bitmask de vías como índice directo (sin sprite para `NONE`).
            // Sólo la tesela plana entra en esta rama, igual que OpenTTD;
            // si el GRF no entrega el grupo o la vista, se conserva el
            // fallback vanilla en lugar de ocultar la vía.
            if let Some(spec) = ground_complete_spec
                && let Some(tile) = ctx.tile
            {
                let track_bits = track_plan.passes[pass_index].map_or(0, |pass| pass.track_bits);
                if let Some(image) = rail_ground_complete_offset(track_bits)
                    && let Some(resolved) = resolve_custom_rail_group_sprite(
                        map,
                        tile,
                        ctx,
                        climate,
                        calendar_date,
                        newgrf_stack,
                        spec,
                        image,
                        &mut signal_sprites,
                        &mut images,
                    )
                {
                    spawn_custom_rail_sprite(
                        commands,
                        ctx,
                        resolved,
                        pass_base_z[pass_index],
                        0.02,
                        0,
                        pass_halftile_corner[pass_index],
                        pass_half_h[pass_index],
                        foundation_child_parent,
                        map_dims.0,
                        "rail-newgrf-ground-complete",
                        image,
                    );
                    custom_track_drawn = true;
                }
            }
        }
        if rail_uses_overlay && !no_sprite_combine {
            let mut custom_draws = 0_usize;
            if let Some(spec) = underlay_spec {
                custom_ground_complete = true;
                for offset in rail_custom_underlay_offsets(
                    track_plan.passes[pass_index].map_or(0, |pass| pass.track_bits),
                ) {
                    let Some(tile) = ctx.tile else {
                        custom_ground_complete = false;
                        continue;
                    };
                    let resolved = resolve_custom_rail_group_sprite(
                        map,
                        tile,
                        ctx,
                        climate,
                        calendar_date,
                        newgrf_stack,
                        spec,
                        offset,
                        &mut signal_sprites,
                        &mut images,
                    );
                    let Some(resolved) = resolved else {
                        custom_ground_complete = false;
                        continue;
                    };
                    spawn_custom_rail_sprite(
                        commands,
                        ctx,
                        resolved,
                        pass_base_z[pass_index],
                        0.02 + custom_draws as f32 * 0.0004,
                        rail_custom_track_extra_y(offset, pass_tileh[pass_index]),
                        pass_halftile_corner[pass_index],
                        pass_half_h[pass_index],
                        foundation_child_parent,
                        map_dims.0,
                        "rail-newgrf-underlay",
                        offset,
                    );
                    custom_draws += 1;
                }
            }
            if custom_ground_complete && let Some(spec) = overlay_spec {
                for (_, offset) in rail_custom_overlay_offsets(
                    track_plan.passes[pass_index].map_or(0, |pass| pass.track_bits),
                ) {
                    let Some(tile) = ctx.tile else {
                        continue;
                    };
                    let Some(resolved) = resolve_custom_rail_group_sprite(
                        map,
                        tile,
                        ctx,
                        climate,
                        calendar_date,
                        newgrf_stack,
                        spec,
                        offset,
                        &mut signal_sprites,
                        &mut images,
                    ) else {
                        continue;
                    };
                    spawn_custom_rail_sprite(
                        commands,
                        ctx,
                        resolved,
                        pass_base_z[pass_index],
                        0.02 + custom_draws as f32 * 0.0004,
                        rail_custom_track_extra_y(offset, pass_tileh[pass_index]),
                        pass_halftile_corner[pass_index],
                        pass_half_h[pass_index],
                        foundation_child_parent,
                        map_dims.0,
                        "rail-newgrf-overlay",
                        offset,
                    );
                    custom_draws += 1;
                }
            }
        }
        if !custom_track_drawn && (!rail_uses_overlay || !custom_ground_complete) {
            for sid in rail_layers[start..end].iter().copied() {
                let missing_asset = !assets.rail.contains_key(&sid);
                let fallback = typed_selection_fallback || missing_asset;
                let role = if fallback {
                    match rail_type {
                        openttdrs_core::RailType::Rail => "rail-track-fallback-rail",
                        openttdrs_core::RailType::Electric => "rail-track-fallback-electric",
                        openttdrs_core::RailType::Monorail => "rail-track-fallback-monorail",
                        openttdrs_core::RailType::Maglev => "rail-track-fallback-maglev",
                    }
                } else {
                    "rail-track"
                };
                record_rail_track_trace(role, sid, fallback, pass_modes[pass_index]);
                let Some(img) = assets.rail.get(&sid) else {
                    track_layer_index += 1;
                    continue;
                };
                let z = 0.02 + track_layer_index as f32 * 0.0004;
                let offset = rail_ghost_overlay_offset(sid);
                let base = full_tile_sprite_pos_half(
                    ctx.tx_i32(),
                    ctx.ty_i32(),
                    pass_base_z[pass_index],
                    z,
                    pass_half_h[pass_index],
                );
                let mut sprite = img.sprite_colored(rail_paint);
                let crop_shift = if let Some((rect, shift)) = halftile_track_subsprite(
                    pass_halftile_corner[pass_index],
                    img.size,
                    pass_half_h[pass_index],
                ) {
                    sprite.rect = Some(rect);
                    shift
                } else {
                    Vec2::ZERO
                };
                // `OffsetGroundSprite` ya queda incorporado por el origen del
                // parent y el ancla NFO de la pendiente falsa. Sumárselo de nuevo
                // desplaza el child 16 px y separa la vía del backing de agua.
                let position =
                    base + Vec3::new(offset.x + crop_shift.x, offset.y + crop_shift.y, 0.0);
                if matches!(
                    pass_modes[pass_index],
                    RailTrackTraceMode::FoundationChild(_)
                ) && let Some(parent) = foundation_child_parent
                {
                    spawn_foundation_child_sprite_at(
                        commands, sprite, ctx, position, map_dims.0, parent,
                    );
                } else {
                    commands.spawn((
                        MapVisualLayer,
                        ctx.map_tile_chunk(),
                        sprite,
                        Transform::from_translation(position),
                    ));
                }
                track_layer_index += 1;
            }
        }
        if let Some(draws) = reservation_draws.as_ref() {
            for draw in draws
                .iter()
                .filter(|draw| draw.halftile_corner == pass_halftile_corner[pass_index])
            {
                let sid = draw.sprite_id;
                let mode = rail_track_trace_mode(rail_foundation, draw.halftile_corner);
                let extra_y = pbs_track_sprite_extra_y(draw.track_bit, draw.sprite_tileh);
                if rail_has_custom_overlay
                    && let Some(spec) = overlay_spec
                    && let Some(offset) = rail_custom_offset_for_track_bit(draw.track_bit)
                    && let Some(tile) = ctx.tile
                    && let Some(resolved) = resolve_custom_rail_group_sprite(
                        map,
                        tile,
                        ctx,
                        climate,
                        calendar_date,
                        newgrf_stack,
                        spec,
                        offset,
                        &mut signal_sprites,
                        &mut images,
                    )
                {
                    spawn_custom_rail_sprite(
                        commands,
                        ctx,
                        resolved,
                        pass_base_z[pass_index],
                        0.026 + pbs_layer_index as f32 * 0.0004,
                        extra_y,
                        pass_halftile_corner[pass_index],
                        pass_half_h[pass_index],
                        foundation_child_parent,
                        map_dims.0,
                        "rail-newgrf-pbs",
                        offset,
                    );
                    pbs_layer_index += 1;
                    continue;
                }
                record_rail_pbs_trace(sid, !assets.has_exact_pbs_rail_sprite(sid), mode, extra_y);
                let Some(img) = assets.pbs_rail_sprite(sid) else {
                    pbs_layer_index += 1;
                    continue;
                };
                let offset = rail_pbs_reservation_offset(sid);
                let bevy_extra_y = pbs_extra_y_in_bevy(extra_y);
                let base = full_tile_sprite_pos_half(
                    ctx.tx_i32(),
                    ctx.ty_i32(),
                    pass_base_z[pass_index],
                    0.026 + pbs_layer_index as f32 * 0.0004,
                    pass_half_h[pass_index],
                );
                let position = base + Vec3::new(offset.x, offset.y + bevy_extra_y, 0.0);
                if matches!(mode, RailTrackTraceMode::FoundationChild(_))
                    && let Some(parent) = foundation_child_parent
                {
                    spawn_foundation_child_sprite_at(
                        commands,
                        img.sprite(),
                        ctx,
                        position,
                        map_dims.0,
                        parent,
                    );
                } else {
                    commands.spawn((
                        MapVisualLayer,
                        ctx.map_tile_chunk(),
                        img.sprite(),
                        Transform::from_translation(position),
                    ));
                }
                pbs_layer_index += 1;
            }
        }
        if foundation_after_pass == Some(pass_index) {
            let foundation = spawn_rail_foundation(
                commands,
                map,
                map_dims,
                assets,
                ctx,
                tileh,
                render_tb,
                foundation_newgrf,
                action5_sprites.as_deref_mut(),
                images.as_deref_mut(),
            );
            foundation_child_parent = foundation.child_parent;
        }
    }
    // `DrawTrackDetails` se emite inmediatamente después de `DrawTrackBits`,
    // antes de catenaria y señales. Usar la pendiente/altura que dejó la
    // fundación es esencial: este bloque incluye los sprites 1305..1308 de
    // las laderas y las cercas verticales que se apoyan en una esquina.
    if show_full_detail {
        let m3hi = ctx.tile.map_or(0, |t| t.m3hi);
        let palette = 775 + u32::from(owner_colour.unwrap_or_default().as_u8());
        let base_z_delta = (i32::from(rail_base_z) - i32::from(ctx.info.base_z)) * 8;
        for (index, draw) in track_fence_draws_for_tile(m3hi, surface_tileh)
            .into_iter()
            .enumerate()
        {
            let sprite_id = 1301 + draw.sprite_index as u32;
            let corner_z = track_fence_height_px(draw, surface_tileh);
            let Some(meta) = track_fence_sprite_meta(draw.sprite_index) else {
                WorldDrawTrace::record_sprite_with_palette_and_geometry(
                    "rail-track-fence",
                    "sortable",
                    sprite_id,
                    palette,
                    true,
                    (0, 0, 0),
                    base_z_delta + corner_z,
                    Some(TraceSpriteBounds::new(
                        draw.bounds_origin.0,
                        draw.bounds_origin.1,
                        draw.bounds_origin.2,
                        draw.bounds_extent.0,
                        draw.bounds_extent.1,
                        draw.bounds_extent.2,
                    )),
                );
                continue;
            };
            let Some(img) = assets.track_fences.get(draw.sprite_index) else {
                WorldDrawTrace::record_sprite_with_palette_and_geometry(
                    "rail-track-fence",
                    "sortable",
                    sprite_id,
                    palette,
                    true,
                    (0, 0, 0),
                    base_z_delta + corner_z,
                    Some(TraceSpriteBounds::new(
                        draw.bounds_origin.0,
                        draw.bounds_origin.1,
                        draw.bounds_origin.2,
                        draw.bounds_extent.0,
                        draw.bounds_extent.1,
                        draw.bounds_extent.2,
                    )),
                );
                continue;
            };
            WorldDrawTrace::record_sprite_with_palette_and_geometry(
                "rail-track-fence",
                "sortable",
                sprite_id,
                palette,
                false,
                (0, 0, 0),
                base_z_delta + corner_z,
                Some(TraceSpriteBounds::new(
                    draw.bounds_origin.0,
                    draw.bounds_origin.1,
                    draw.bounds_origin.2,
                    draw.bounds_extent.0,
                    draw.bounds_extent.1,
                    draw.bounds_extent.2,
                )),
            );

            let filename = format!("track_fence_{}.png", draw.sprite_index);
            let mut pos3 = overlay_pos(
                ctx.iso_pos,
                f32::from(meta.xrel),
                f32::from(meta.yrel),
                f32::from(meta.width),
                f32::from(meta.height),
                rail_base_z,
                0.03 + index as f32 * 0.0001,
                ctx.tx_i32(),
                ctx.ty_i32(),
            );
            // `DrawTrackFence` desplaza únicamente las cercas con referencia
            // de esquina. El pequeño ajuste de profundidad evita que una
            // cerca elevada quede detrás de la misma tesela en Bevy.
            pos3.y += corner_z as f32;
            pos3.z += corner_z as f32 * 0.0001;
            commands.spawn((
                MapVisualLayer,
                ctx.map_tile_chunk(),
                sprite_from_atlas_or_company_white_colour(company, owner_colour, img, &filename),
                Transform::from_translation(pos3),
            ));
        }
    }

    // `DrawTrackBits`: una reserva PBS no recolorea toda la vía. OpenTTD
    // superpone los SINGLE_* de las pistas reservadas con PALETTE_CRASH=804.
    // La segunda capa es esencial para no confundir una reserva en un cruce
    // o túnel con una vía de otro tipo.
    spawn_rail_catenary_for_surface(
        commands,
        map,
        map_dims,
        assets,
        ctx,
        rail_type,
        render_tb,
        tileh,
        rail_base_z,
        true,
        catenary_newgrf,
        &mut catenary_sprites,
        &mut images,
    );
    if let Some(t) = ctx.tile.filter(|t| rail_tile_is_signals(t.m5)) {
        let sig_draws = collect_signal_sprite_draws(t.m2, t.m3, t.m3hi, t.m5);
        let rail_type = openttdrs_core::rail_type_from_tile(t);
        let signal_spec = rail_signal_newgrf
            .get(usize::from(rail_type.as_u8()))
            .and_then(Option::as_ref);
        for (si, draw) in sig_draws.iter().copied().enumerate() {
            let (world_xy, signal_z_delta, signal_bounds) = signal_trace_geometry(
                ctx.info.tileh,
                ctx.info.base_z,
                render_tb,
                draw.pos,
                draw.track,
                signals_on_right,
            );
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
            let (sprite, mut signal_xy) = if let Some(custom) = custom {
                let anchor = signal_screen_anchor_for_side(
                    ctx.tx_i32(),
                    ctx.ty_i32(),
                    draw.pos,
                    rail_half_h,
                    rail_base_z,
                    signals_on_right,
                );
                (custom.sprite, anchor + custom.center_offset)
            } else if let Some(slot) = openttdrs_core::signal_action5_slot(draw.sprite_id)
                && let (Some(cache), Some(images)) = (action5_sprites.as_mut(), images.as_mut())
                && let Some(sprite) = cache.sprite_colored(
                    openttdrs_core::ACTION5_TYPE_SIGNALS,
                    slot,
                    signal_action5,
                    Color::WHITE,
                    images,
                )
            {
                let anchor = signal_screen_anchor_for_side(
                    ctx.tx_i32(),
                    ctx.ty_i32(),
                    draw.pos,
                    rail_half_h,
                    rail_base_z,
                    signals_on_right,
                );
                let offset = signal_sprite_center_offset(draw.sprite_id);
                (sprite, anchor + offset)
            } else {
                let Some(img) = assets.rail.get(&draw.sprite_id) else {
                    WorldDrawTrace::record_sprite_with_palette_and_world_geometry(
                        "rail-signal",
                        "sortable",
                        draw.sprite_id,
                        0,
                        true,
                        world_xy,
                        signal_z_delta,
                        (0, 0, 0),
                        Some(signal_bounds),
                    );
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
            WorldDrawTrace::record_sprite_with_palette_and_world_geometry(
                "rail-signal",
                "sortable",
                draw.sprite_id,
                0,
                false,
                world_xy,
                signal_z_delta,
                (0, 0, 0),
                Some(signal_bounds),
            );
            // `signal_xy` ya contiene la elevación de la superficie de vía.
            // Agregar únicamente la diferencia de `GetSafeSlopeZ` preserva
            // los cimientos y sube el poste sobre la pendiente exacta.
            signal_xy.y +=
                (signal_z_delta - (i32::from(rail_base_z) - i32::from(ctx.info.base_z)) * 8) as f32;
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
}

#[cfg(test)]
mod tests {
    use bevy::prelude::{Rect, Vec2};

    use super::{
        RTO_CROSSING_XY, RTO_E, RTO_JUNCTION_SE, RTO_N, RTO_S, RTO_W, RTO_X, RTO_Y, RailGroundKind,
        RailTrackTraceMode, catenary_local_z_delta, halftile_track_subsprite, pbs_extra_y_in_bevy,
        pbs_track_sprite_extra_y, rail_custom_overlay_offsets, rail_custom_underlay_offsets,
        rail_foundation_after_pass, rail_ground_complete_offset, rail_ground_sprite_id,
        rail_initial_ground_draw, rail_track_trace_mode, rail_upper_halftile_ground_draw,
        road_detail_world_z_delta, road_foundation_child_offset,
        roadside_streetlight_parent_sprites, roadside_streetlight_sorted_depths,
        signal_trace_geometry,
    };
    use crate::sprites::{
        RAIL_GROUND_HALF_TILE_SNOW, RAIL_GROUND_HALF_TILE_WATER, RAIL_TB_CROSS, RAIL_TB_HORZ,
        RAIL_TB_LEFT, RAIL_TB_LOWER, RAIL_TB_RIGHT, RAIL_TB_UPPER, RAIL_TB_VERT, RAIL_TB_X,
        RAIL_TB_Y, ROADSIDE_LAMPS,
    };
    use openttdrs_core::{FOUNDATION_INCLINED_X, FOUNDATION_LEVELED};

    #[test]
    fn rail_track_trace_mode_matches_draw_ground_sprite_foundation_context() {
        // Sin cimiento (o con una combinación inválida) el proc C++ conserva
        // una coordenada de mundo. El resto verifica las ramas de
        // `DrawFoundation` que cambian el padre activo.
        assert_eq!(rail_track_trace_mode(0, None), RailTrackTraceMode::Ground);
        assert_eq!(
            rail_track_trace_mode(u8::MAX, None),
            RailTrackTraceMode::Ground
        );
        assert_eq!(
            rail_track_trace_mode(FOUNDATION_LEVELED, None),
            RailTrackTraceMode::FoundationChild((0, -32, 0))
        );
        assert_eq!(
            rail_track_trace_mode(FOUNDATION_INCLINED_X, None),
            RailTrackTraceMode::FoundationChild((0, 0, 0))
        );

        // Fundación de media tesela: baja sin padre, alta en el padre nuevo.
        assert_eq!(rail_track_trace_mode(7, None), RailTrackTraceMode::Ground);
        assert_eq!(
            rail_track_trace_mode(7, Some(1)),
            RailTrackTraceMode::FoundationChild((0, -64, 0))
        );
        // SteepBoth primero deja el padre SteepLower y después crea el
        // padre de media tesela de la esquina alta.
        assert_eq!(
            rail_track_trace_mode(5, None),
            RailTrackTraceMode::FoundationChild((0, -32, 0))
        );
        assert_eq!(
            rail_track_trace_mode(5, Some(0)),
            RailTrackTraceMode::FoundationChild((64, -32, 0))
        );
    }

    #[test]
    fn rail_newgrf_offsets_match_openttd_track_overlay_table() {
        assert_eq!(rail_custom_underlay_offsets(RAIL_TB_X), vec![RTO_X]);
        assert_eq!(rail_custom_underlay_offsets(RAIL_TB_Y), vec![RTO_Y]);
        assert_eq!(
            rail_custom_underlay_offsets(RAIL_TB_HORZ),
            vec![RTO_N, RTO_S]
        );
        assert_eq!(
            rail_custom_underlay_offsets(RAIL_TB_VERT),
            vec![RTO_E, RTO_W]
        );
        assert_eq!(
            rail_custom_underlay_offsets(RAIL_TB_CROSS),
            vec![RTO_CROSSING_XY]
        );
        // X + LOWER + RIGHT deja libre la esquina NW → RTO_JUNCTION_SE.
        assert_eq!(rail_custom_underlay_offsets(0x29), vec![RTO_JUNCTION_SE]);
        assert_eq!(
            rail_custom_overlay_offsets(RAIL_TB_X | RAIL_TB_LOWER | RAIL_TB_LEFT),
            vec![
                (RAIL_TB_X, RTO_X),
                (RAIL_TB_LOWER, RTO_S),
                (RAIL_TB_LEFT, RTO_W)
            ]
        );
        assert!(rail_custom_overlay_offsets(RAIL_TB_X).is_empty());
    }

    #[test]
    fn rail_ground_complete_uses_trackbits_minus_one() {
        assert_eq!(rail_ground_complete_offset(0), None);
        assert_eq!(rail_ground_complete_offset(RAIL_TB_X), Some(0));
        assert_eq!(rail_ground_complete_offset(RAIL_TB_CROSS), Some(2));
        assert_eq!(rail_ground_complete_offset(0xFF), Some(0x3E));
    }

    #[test]
    fn roadside_streetlights_match_kale_post_sort_order() {
        // Kale `(119,9)`, road bits 10: OpenTTD inserta 1407 y 1406, pero
        // `ViewportSortParentSprites` los pinta como 1406 y luego 1407.
        let lamps = ROADSIDE_LAMPS[10];
        let parents = roadside_streetlight_parent_sprites(119, 9, 1, 1, 0, lamps);
        assert_eq!(parents.len(), 2);
        assert_eq!(
            parents[0].kind,
            crate::render::viewport_sort::ParentSpriteKind::Sprite { sprite_id: 1407 }
        );
        assert_eq!(
            (
                parents[0].bounds.xmin,
                parents[0].bounds.ymin,
                parents[0].bounds.zmin,
                parents[0].bounds.xmax,
                parents[0].bounds.ymax,
                parents[0].bounds.zmax,
            ),
            (1912, 158, 8, 1913, 159, 23)
        );
        assert_eq!(
            crate::render::viewport_sort::viewport_sort_parent_sprites(&parents),
            vec![1, 0]
        );

        let depths = roadside_streetlight_sorted_depths(119, 9, 1, 1, 0, lamps);
        assert_eq!(
            depths,
            vec![
                crate::iso::sortable_draw_z(119, 9, 1, 0.22),
                crate::iso::sortable_draw_z(119, 9, 1, 0.2),
            ]
        );
    }

    #[test]
    fn rail_full_sprite_center_preserves_opengfx_xrel_minus_31() {
        let pos = crate::iso::full_tile_sprite_pos_half(7, 3, 0, 0.02, crate::iso::TILE_HALF_H);
        let flat = crate::iso::full_tile_sprite_pos(7, 3, 0, 0.02);
        assert_eq!(
            pos.x,
            crate::iso::iso(7, 3).x + crate::iso::GROUND_SPRITE_CENTER_X_OFFSET
        );
        assert_eq!(flat.x, pos.x);
    }

    #[test]
    fn rail_water_ground_stays_in_the_ground_pass() {
        // `DrawGroundSprite(SPR_FLAT_WATER_TILE)` de una vía no es un
        // sortable sprite. Esta es la forma que usa
        // `spawn_rail_ground_draw` para la rama sin parent.
        let expected = super::rail_ground_pass_pos(
            230,
            150,
            0,
            super::FLAT_WATER_LAYER_FRAC,
            crate::iso::TILE_HALF_H,
        );
        assert!(
            expected.z
                < crate::iso::full_tile_sprite_pos(230, 150, 0, super::FLAT_WATER_LAYER_FRAC,).z
        );
    }

    #[allow(clippy::expect_used)] // Fixture del oráculo: el fallo debe mostrar el caso exacto.
    #[test]
    fn rail_ground_selection_matches_kale_openttd_oracle_cases() {
        // Estos cinco casos se exportaron con `world_draw_export` del C++ de
        // referencia. Los m5 son TrackBits ya normalizados por el mapa Kale.
        // Así se protege tanto la selección de m4 como la relación ground /
        // child que dejó DrawFoundation.
        let case = |tileh, bits, ground_type| {
            let foundation = openttdrs_core::rail_foundation_for_trackbits(tileh, bits);
            rail_initial_ground_draw(
                tileh,
                foundation,
                openttdrs_core::rail_track_draw_plan(tileh, bits),
                false, // RailTypeInfo::UsesOverlay() = false en los vanilla.
                ground_type,
            )
            .expect("el oráculo emitió un suelo ferroviario")
        };

        // Kale (182,28): HalfTileWater sin vía baja → agua plana 4061.
        let water = case(0x02, 0x08, RAIL_GROUND_HALF_TILE_WATER);
        assert_eq!(water.draw.kind, RailGroundKind::Water);
        assert_eq!(rail_ground_sprite_id(water.draw), 4061);
        assert_eq!(water.trace_mode, RailTrackTraceMode::Ground);

        // Kale (116,79): monorail vanilla no usa Overlay; la mitad baja
        // vacía sigue siendo pasto 3985, antes de Foundation(8).
        let mono = case(0x04, 0x20, 9);
        assert_eq!(mono.draw.kind, RailGroundKind::Grass);
        assert_eq!(rail_ground_sprite_id(mono.draw), 3985);
        assert_eq!(mono.trace_mode, RailTrackTraceMode::Ground);

        // Kale (158,65): la misma rama clásica para Foundation(9).
        let grass = case(0x08, 0x04, 1);
        assert_eq!(rail_ground_sprite_id(grass.draw), 3989);
        assert_eq!(grass.trace_mode, RailTrackTraceMode::Ground);

        // Kale (191,137): vía eléctrica sobre HalfTileWater → costa 5949.
        let shore = case(0x0D, 0x04, RAIL_GROUND_HALF_TILE_WATER);
        assert_eq!(shore.draw.kind, RailGroundKind::Shore);
        assert_eq!(rail_ground_sprite_id(shore.draw), 5949);
        assert_eq!(shore.trace_mode, RailTrackTraceMode::Ground);

        // Kale (229,149): la anti-zig-zag Foundation(10) modifica la
        // pendiente a 11; la costa se convierte en child 5947 del muro.
        let child_shore = case(0x09, 0x10, RAIL_GROUND_HALF_TILE_WATER);
        assert_eq!(child_shore.draw.kind, RailGroundKind::Shore);
        assert_eq!(child_shore.draw.tileh, 0x0B);
        assert_eq!(rail_ground_sprite_id(child_shore.draw), 5947);
        assert_eq!(
            child_shore.trace_mode,
            RailTrackTraceMode::FoundationChild((0, 0, 0))
        );
    }

    #[allow(clippy::expect_used)] // Fixture del oráculo: el fallo debe mostrar el caso exacto.
    #[test]
    fn rail_underlay_ground_is_opt_in_and_half_tile_snow_is_upper_only() {
        // `RailTypeInfo::UsesOverlay()` depende del grupo Action3 Underlay,
        // no del enum monorail/maglev. Un baseset vanilla debe conservar la
        // rama clásica sin un segundo rombo de suelo.
        assert!(rail_upper_halftile_ground_draw(false, 1, 0x0B).is_none());

        let upper = rail_upper_halftile_ground_draw(true, RAIL_GROUND_HALF_TILE_SNOW, 0x0B)
            .expect("un Underlay NewGRF dibuja el suelo elevado");
        assert_eq!(upper.kind, RailGroundKind::SnowOrDesert);
        assert_eq!(rail_ground_sprite_id(upper), 4561);

        let lower = rail_initial_ground_draw(
            0x04,
            0,
            openttdrs_core::rail_track_draw_plan(0x04, 0x20),
            true,
            RAIL_GROUND_HALF_TILE_SNOW,
        )
        .expect("DrawTrackBitsOverlay siempre dibuja base");
        assert_eq!(lower.draw.kind, RailGroundKind::Grass);
    }

    #[test]
    fn pbs_corner_tracks_follow_the_effective_surface_slope() {
        // `DrawTrackBits` desplaza sólo la reserva de la esquina que está
        // elevada. `sprite_tileh` ya es la pendiente posterior a
        // `DrawFoundation`, no necesariamente el `tileh` almacenado en SAV.
        assert_eq!(pbs_track_sprite_extra_y(RAIL_TB_LEFT, 0x0B), -32);
        assert_eq!(pbs_track_sprite_extra_y(RAIL_TB_RIGHT, 0x0E), -32);
        assert_eq!(pbs_track_sprite_extra_y(RAIL_TB_UPPER, 0x0D), -32);
        assert_eq!(pbs_track_sprite_extra_y(RAIL_TB_LOWER, 0x07), -32);

        assert_eq!(pbs_track_sprite_extra_y(RAIL_TB_LEFT, 0x0E), 0);
        assert_eq!(pbs_track_sprite_extra_y(RAIL_TB_RIGHT, 0x0B), 0);
        assert_eq!(pbs_track_sprite_extra_y(RAIL_TB_UPPER, 0x07), 0);
        assert_eq!(pbs_track_sprite_extra_y(RAIL_TB_LOWER, 0x0D), 0);

        assert_eq!(pbs_extra_y_in_bevy(-32), 32.0);
        assert_eq!(pbs_extra_y_in_bevy(0), 0.0);
    }

    #[test]
    fn halftile_rail_passes_defer_the_foundation_and_clip_like_openttd() {
        // Kale_TitleGame (160,65): hay una pasada baja y otra alta; el
        // cimiento Action5 debe emitirse entre ambas, no antes de las dos.
        let plan = openttdrs_core::rail_track_draw_plan(0x02, 0x0C);
        assert_eq!(rail_foundation_after_pass(plan), Some(0));

        // `_halftile_sub_sprite` se aplica al PNG de la pendiente falsa. La
        // compensación conserva el borde que OpenTTD dibuja antes de recortar
        // el centro del sprite de Bevy.
        assert_eq!(
            halftile_track_subsprite(Some(0), Vec2::new(64.0, 31.0), 7.5),
            Some((Rect::new(0.0, 0.0, 31.0, 31.0), Vec2::new(-16.5, 0.0)))
        );
        assert_eq!(
            halftile_track_subsprite(Some(1), Vec2::new(64.0, 23.0), 11.5),
            Some((Rect::new(0.0, 7.0, 64.0, 23.0), Vec2::new(0.0, -3.5)))
        );
        assert_eq!(
            halftile_track_subsprite(Some(2), Vec2::new(64.0, 31.0), 7.5),
            Some((Rect::new(33.0, 0.0, 64.0, 31.0), Vec2::new(16.5, 0.0)))
        );
        assert_eq!(
            halftile_track_subsprite(Some(3), Vec2::new(64.0, 39.0), 11.5),
            Some((Rect::new(0.0, 0.0, 64.0, 16.0), Vec2::new(0.0, 11.5)))
        );
        assert_eq!(halftile_track_subsprite(None, Vec2::ONE, 0.5), None);
    }

    #[test]
    fn road_foundation_ground_relation_matches_draw_foundation() {
        // La carretera posterior a una fundación nivelada es child del muro;
        // las dos fundaciones inclinadas conservan el mismo origen de padre.
        assert_eq!(road_foundation_child_offset(0), None);
        assert_eq!(
            road_foundation_child_offset(FOUNDATION_LEVELED),
            Some((0, -32, 0))
        );
        assert_eq!(
            road_foundation_child_offset(FOUNDATION_INCLINED_X),
            Some((0, 0, 0))
        );
    }

    #[test]
    fn road_details_follow_effective_slope_and_foundation_height() {
        // Valores capturados por el oráculo en Kale: h=9, base Z=2, faroles
        // en (1,8)/(14,8) se dibujan a Z=20 (delta=4), no en la base Z=16.
        assert_eq!(road_detail_world_z_delta(2, 2, 0x09, 1.0, 8.0), 4);
        assert_eq!(road_detail_world_z_delta(2, 2, 0x09, 14.0, 8.0), 4);

        // Árboles en la misma pendiente pero con base Z=1: las posiciones
        // (0,2) y (0,10) dan 7 y 3 píxeles por encima de la base.
        assert_eq!(road_detail_world_z_delta(1, 1, 0x09, 0.0, 2.0), 7);
        assert_eq!(road_detail_world_z_delta(1, 1, 0x09, 0.0, 10.0), 3);

        // Con fundación nivelada el detalle sube toda la altura de tesela aun
        // cuando la superficie posterior sea plana.
        assert_eq!(road_detail_world_z_delta(0, 1, 0, 8.0, 8.0), 8);
    }

    #[test]
    fn catenary_local_z_keeps_the_upstream_anchor_after_surface_placement() {
        // El ancla de un PCP/cable es absoluta respecto de TileInfo::z
        // crudo. En una pendiente sin fundación debe subir los 8 px completos
        // dentro de la tesela.
        assert_eq!(catenary_local_z_delta(8, 3, 3), 8);

        // Si `tile_pos_half` ya quedó sobre una fundación nivelada (z=2 en
        // lugar de z=1), sólo queda el tramo local: anchor absoluto 16 menos
        // los 8 px que ya aportó la base = 8. Esto impide contar dos veces la
        // elevación de la fundación.
        assert_eq!(catenary_local_z_delta(16, 1, 2), 8);

        // El caso complementario aparece en estaciones niveladas: el ancla
        // C++ sigue en z crudo, por lo que debe compensar la base visual
        // elevada en vez de quedarse suspendido otros 8 px.
        assert_eq!(catenary_local_z_delta(8, 1, 2), 0);

        let screen_delta =
            crate::iso::remap_tile_offset(0.0, 0.0, catenary_local_z_delta(16, 1, 2) as f32) * 0.5;
        assert_eq!(screen_delta, Vec2::new(0.0, 8.0));
    }

    #[test]
    fn signal_trace_keeps_kale_anchor_and_uses_safe_slope_z() {
        // Kale (85,7): la señal X usa `pos=8` con señales a la derecha:
        // `DrawSingleSignal` ancla el sortable en (11,13), no en el centro
        // de la tesela, y su caja es siempre 1×1×6.
        let (world_xy, z_delta, bounds) = signal_trace_geometry(
            0, 1, 1, 8, 0, // Track::X
            true,
        );
        assert_eq!(world_xy, (11, 13));
        assert_eq!(z_delta, 0);
        assert_eq!(
            (
                bounds.ox, bounds.oy, bounds.oz, bounds.ex, bounds.ey, bounds.ez
            ),
            (0, 0, 0, 1, 1, 6)
        );

        // Kale (183,28): la vía X sobre SLOPE_SE requiere fundación
        // nivelada. La señal usa `pos=9` en el lado derecho;
        // `GetSlopePixelZ_Rail` sube primero la superficie una altura de
        // tesela. Medir la pendiente cruda daba Z=2 en vez de Z=8.
        assert_eq!(signal_trace_geometry(0x06, 0, 0x01, 9, 0, true).1, 8);

        // En una media fundación la esquina segura elegida por el carril
        // importa: UPPER sobre SLOPE_N queda en la mitad elevada (Z=8), aun
        // cuando el poste no esté justo en (0,0).
        assert_eq!(signal_trace_geometry(0x08, 0, 0x04, 4, 4, true).1, 8);
    }
}
