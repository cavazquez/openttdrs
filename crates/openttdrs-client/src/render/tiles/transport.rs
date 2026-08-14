use bevy::prelude::*;
use openttdrs_core::prelude::*;
use openttdrs_core::{Climate, RoadTypeDef, partial_pixel_z};

use super::{
    TRAM_OVERLAY_LAYER_FRAC, catenary_under_low_bridge, roadside_detail_visible_under_bridge,
    sloped_or_flat_image, spawn_forced_leveled_foundation, spawn_ground_sprite,
    spawn_rail_foundation, spawn_road_foundation,
};
use crate::iso::{TILE_HALF_H, overlay_pos, remap_tile_offset, slope_half_h, tile_pos_half};
use crate::render::catenary_newgrf::catenary_sprite_colored;
use crate::render::road_newgrf::{
    NewGrfRoadSpriteCache, newgrf_road_def_for_tile, newgrf_tram_def_for_tile,
    road_newgrf_view_index,
};
use crate::render::world_draw_trace::{TraceSpriteBounds, WorldDrawTrace};
use crate::render::{
    CompanyColoredSprites, MapVisualLayer, TileRenderContext, WorldAssets,
    sprite_from_atlas_or_company_white_colour,
};
use crate::sprites::{
    CompanyColour, ONEWAY_ROAD_SPRITE_META, RAIL_GROUND_SNOW_OR_DESERT, RAIL_TB_LEFT,
    RAIL_TB_LOWER, RAIL_TB_RIGHT, RAIL_TB_UPPER, ROAD_FLAT_HALF_H, ROAD_STREETLIGHT_META,
    ROADSIDE_LAMPS, ROADSIDE_TREE_META, ROADSIDE_TREES, SPR_ROADSIDE_TREE,
    catenary_pylon_world_z_delta, catenary_reference_sprite_id, catenary_sprite_color,
    catenary_wire_world_z_delta, collect_catenary_pylons_from_map_with_pcp_override,
    collect_catenary_wire_draws_from_map, collect_rail_pbs_reservation_draws,
    collect_rail_sprites_for_surface, collect_signal_sprite_draws, is_road_level_crossing,
    is_typed_rail_track_sprite, level_crossing_ground_sprite_id_for_type,
    level_crossing_has_rail_reservation, oneway_road_sprite_id, rail_ghost_overlay_offset,
    rail_pbs_reservation_offset, rail_tile_is_signals, rail_trackbits_for_render,
    remap_rail_sprite_id, road_bits_for_render, road_flat_sprite_color, road_flat_sprite_index,
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

/// Convierte el mismo desplazamiento de `OffsetGroundSprite` a las
/// coordenadas del renderer. El oráculo lo serializa multiplicado por
/// `ZOOM_BASE=4`, mientras que nuestras teselas 8bpp ya usan píxeles finales
/// de 64×31 y Bevy tiene Y hacia arriba.
const fn halftile_foundation_child_visual_offset(corner: Option<u8>) -> Vec2 {
    match corner {
        Some(corner) => {
            let (x, y, _) = halftile_foundation_child_offset(corner);
            Vec2::new(x as f32 / 4.0, -(y as f32) / 4.0)
        }
        None => Vec2::ZERO,
    }
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
    oneway_newgrf: &[Option<openttdrs_core::DecodedSprite>],
    foundation_newgrf: &[Option<openttdrs_core::DecodedSprite>],
    mut action5_sprites: Option<&mut crate::render::NewGrfAction5SpriteCache>,
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
    let (tileh, base_z, road_foundation) = if is_level_crossing {
        let crossing_base_z = spawn_forced_leveled_foundation(
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
            crossing_base_z,
            if raw_tileh == 0 {
                0
            } else {
                openttdrs_core::FOUNDATION_LEVELED
            },
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
        // `DrawRoadGroundSprites` ocurre después de `DrawFoundation`; el
        // contrato es hijo del cimiento cuando éste existe, no un nuevo suelo
        // absoluto. La traza conserva esa relación para que el oráculo detecte
        // tanto el sprite como la transición de pendiente.
        record_road_ground_trace(
            "road-ground",
            road_ground_sprite_id(fi, paved, snow_or_desert),
            road_foundation,
        );
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
            commands.spawn((
                MapVisualLayer,
                ctx.map_tile_chunk(),
                sprite,
                Transform::from_translation(road_oneway_overlay_pos(ctx, tileh, base_z, meta)),
            ));
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
    if !is_level_crossing
        && show_full_detail
        && roadside == Some(3)
        && rb.count_ones() > 1
        && roadside_detail_visible_under_bridge(map, ctx.coord, (mw, mh), false)
    {
        for &(lamp, dx, dy) in ROADSIDE_LAMPS[usize::from(rb & 0xF)] {
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
        if let Some(img) = assets.rail.get(&sid) {
            // `DrawRoadTile` pinta el bloque de vía del cruce con `PAL_NONE`.
            // La identidad visual (rail/electric/mono/maglev/tram) ya viene en
            // el sprite seleccionado; recolorearlo lo alejaba de OpenTTD.
            let crossing_paint = Color::WHITE;
            record_road_ground_trace("crossing-ground", sid, road_foundation);
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
                let base = tile_pos_half(ctx.tx_i32(), ctx.ty_i32(), base_z, 0.048, road_half_h);
                let offset = rail_ghost_overlay_offset(sid);
                commands.spawn((
                    MapVisualLayer,
                    ctx.map_tile_chunk(),
                    img.sprite(),
                    Transform::from_translation(base + Vec3::new(offset.x, offset.y, 0.0)),
                ));
            }
        }
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
    mut signal_sprites: Option<&mut crate::render::NewGrfSignalSpriteCache>,
    signal_action5: &[Option<openttdrs_core::DecodedSprite>],
    foundation_newgrf: &[Option<openttdrs_core::DecodedSprite>],
    mut action5_sprites: Option<&mut crate::render::NewGrfAction5SpriteCache>,
    mut images: Option<&mut Assets<Image>>,
    calendar_date: u32,
    newgrf_stack: &[openttdrs_core::NewGrfEntry],
) {
    let tileh = ctx.info.tileh;
    // `IsBridgeAbove` no reemplaza el contenido de la tesela: OpenTTD pinta
    // primero la vía inferior y después el tablero elevado. El tablero se
    // agrega separadamente por `spawn_bridge_middle` en `tile_spawn.rs`.
    // Saltar esta rama hacía desaparecer vías reales bajo puentes y dejaba
    // sus reservas PBS, túneles y conexiones aparentemente cortados.
    if tileh != 0 {
        spawn_ground_sprite(
            commands,
            &sloped_or_flat_image(tileh, &assets.grass, &assets.grass_slopes),
            Color::WHITE,
            ctx,
            slope_half_ground,
        );
    }
    let render_tb = rail_trackbits_for_render(map, ctx.coord, map_dims.0, map_dims.1);
    let snow_ground = ctx
        .tile
        .is_some_and(|t| (t.m3 & 0x0F) == RAIL_GROUND_SNOW_OR_DESERT)
        || climate.uses_snow_ground();
    let rail_foundation = openttdrs_core::rail_foundation_for_trackbits(tileh, render_tb);
    let track_plan = openttdrs_core::rail_track_draw_plan(tileh, render_tb);
    let foundation_after_pass = rail_foundation_after_pass(track_plan);
    // En una fundación no continua el primer `DrawFoundation` es NONE o
    // STEEP_LOWER; el cimiento visible se crea recién entre las dos pasadas.
    // Aun así catenaria y señales posteriores necesitan conocer desde ahora
    // la altura final de la superficie.
    let rail_base_z = if foundation_after_pass.is_some() {
        let (_, z_delta) = openttdrs_core::rail_surface_slope_and_z(tileh, render_tb);
        ctx.info.base_z.saturating_add(z_delta)
    } else {
        spawn_rail_foundation(
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
        )
    };
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
    let rail_type = ctx
        .tile
        .map(openttdrs_core::rail_type_from_tile)
        .unwrap_or_default();
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
        let start = if pass_index == 0 {
            0
        } else {
            pass_ends[pass_index - 1]
        };
        let end = pass_ends[pass_index];
        let halftile_offset =
            halftile_foundation_child_visual_offset(pass_halftile_corner[pass_index]);
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
            let base = tile_pos_half(
                ctx.tx_i32(),
                ctx.ty_i32(),
                pass_base_z[pass_index],
                z,
                pass_half_h[pass_index],
            );
            commands.spawn((
                MapVisualLayer,
                ctx.map_tile_chunk(),
                img.sprite_colored(rail_paint),
                Transform::from_translation(
                    base + Vec3::new(
                        offset.x + halftile_offset.x,
                        offset.y + halftile_offset.y,
                        0.0,
                    ),
                ),
            ));
            track_layer_index += 1;
        }
        if let Some(draws) = reservation_draws.as_ref() {
            for draw in draws
                .iter()
                .filter(|draw| draw.halftile_corner == pass_halftile_corner[pass_index])
            {
                let sid = draw.sprite_id;
                let mode = rail_track_trace_mode(rail_foundation, draw.halftile_corner);
                let extra_y = pbs_track_sprite_extra_y(draw.track_bit, draw.sprite_tileh);
                record_rail_pbs_trace(sid, !assets.has_exact_pbs_rail_sprite(sid), mode, extra_y);
                let Some(img) = assets.pbs_rail_sprite(sid) else {
                    pbs_layer_index += 1;
                    continue;
                };
                let offset = rail_pbs_reservation_offset(sid);
                let bevy_extra_y = pbs_extra_y_in_bevy(extra_y);
                let base = tile_pos_half(
                    ctx.tx_i32(),
                    ctx.ty_i32(),
                    pass_base_z[pass_index],
                    0.026 + pbs_layer_index as f32 * 0.0004,
                    pass_half_h[pass_index],
                );
                commands.spawn((
                    MapVisualLayer,
                    ctx.map_tile_chunk(),
                    img.sprite(),
                    Transform::from_translation(
                        base + Vec3::new(
                            offset.x + halftile_offset.x,
                            offset.y + bevy_extra_y + halftile_offset.y,
                            0.0,
                        ),
                    ),
                ));
                pbs_layer_index += 1;
            }
        }
        if foundation_after_pass == Some(pass_index) {
            let _ = spawn_rail_foundation(
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
    // Catenaria OpenGFX: wires (PCP) + postes PPP; TO_CATENARY vía env.
    if rail_type.has_catenary() {
        let trackbits = rail_trackbits_for_render(map, ctx.coord, map_dims.0, map_dims.1);
        let low_bridge = catenary_under_low_bridge(map, ctx.coord, map_dims);
        let tint = catenary_sprite_color();
        let mut wires = Vec::new();
        if !low_bridge.hide_wires {
            collect_catenary_wire_draws_from_map(
                map,
                ctx.coord,
                map_dims.0,
                map_dims.1,
                crate::sprites::OTTD_MP_RAIL,
                trackbits,
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
            trackbits,
            tileh,
            low_bridge.pylon_pcp_override,
            &mut pylons,
        );
        for draw in pylons {
            let sprite = catenary_sprite_colored(
                assets,
                draw.sprite_id,
                tint,
                catenary_newgrf,
                catenary_sprites.as_deref_mut(),
                images.as_deref_mut(),
            );
            WorldDrawTrace::record_sprite_with_palette_and_world_geometry(
                "catenary-pylon",
                "sortable",
                catenary_reference_sprite_id(draw.sprite_id),
                0,
                sprite.is_none(),
                (draw.tile_dx as i32, draw.tile_dy as i32),
                draw.pcp_direction.map_or(0, |pcp| {
                    catenary_pylon_world_z_delta(tileh, ctx.info.base_z, render_tb, pcp)
                }),
                (1, 1, 0),
                Some(TraceSpriteBounds::new(-1, -1, 0, 1, 1, 6)),
            );
            let Some(sprite) = sprite else {
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
        // OpenTTD emite primero los postes PPP y después los cables PCP. El
        // z visual mantiene al poste sobre el cable, pero preservar la misma
        // secuencia también hace comparable el stream sortable del oráculo.
        for (i, draw) in wires.iter().copied().enumerate() {
            let sid = draw.sprite_id;
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
            WorldDrawTrace::record_sprite_with_palette_and_geometry(
                "catenary-wire",
                "sortable",
                catenary_reference_sprite_id(sid),
                0,
                sprite.is_none(),
                (0, 0, 0),
                catenary_wire_world_z_delta(tileh, ctx.info.base_z, render_tb, draw),
                Some(TraceSpriteBounds::new(ox, oy, oz, ex, ey, ez)),
            );
            let Some(sprite) = sprite else {
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
    }
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
    use bevy::prelude::Vec2;

    use super::{
        RailTrackTraceMode, halftile_foundation_child_visual_offset, pbs_extra_y_in_bevy,
        pbs_track_sprite_extra_y, rail_foundation_after_pass, rail_track_trace_mode,
        road_detail_world_z_delta, road_foundation_child_offset, signal_trace_geometry,
    };
    use crate::sprites::{RAIL_TB_LEFT, RAIL_TB_LOWER, RAIL_TB_RIGHT, RAIL_TB_UPPER};
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
    fn halftile_rail_passes_defer_the_foundation_and_keep_its_screen_offset() {
        // Kale_TitleGame (160,65): hay una pasada baja y otra alta; el
        // cimiento Action5 debe emitirse entre ambas, no antes de las dos.
        let plan = openttdrs_core::rail_track_draw_plan(0x02, 0x0C);
        assert_eq!(rail_foundation_after_pass(plan), Some(0));

        // `OffsetGroundSprite(0, -16)` se serializa como (0,-64) en el
        // oráculo (ZOOM_BASE=4). El renderer usa Y positivo hacia arriba.
        assert_eq!(
            halftile_foundation_child_visual_offset(Some(1)),
            Vec2::new(0.0, 16.0)
        );
        assert_eq!(
            halftile_foundation_child_visual_offset(Some(0)),
            Vec2::new(16.0, 8.0)
        );
        assert_eq!(halftile_foundation_child_visual_offset(None), Vec2::ZERO);
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
