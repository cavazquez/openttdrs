use bevy::prelude::*;
use openttdrs_core::prelude::*;
use openttdrs_core::{Climate, RoadTypeDef};

use super::{
    TRAM_OVERLAY_LAYER_FRAC, catenary_under_low_bridge, sloped_or_flat_image, spawn_ground_sprite,
    spawn_rail_foundation,
};
use crate::iso::{TILE_HALF_H, overlay_pos, remap_tile_offset, slope_half_h, tile_pos_half};
use crate::render::catenary_newgrf::catenary_sprite_colored;
use crate::render::road_newgrf::{
    NewGrfRoadSpriteCache, newgrf_road_def_for_tile, newgrf_tram_def_for_tile,
    road_newgrf_view_index,
};
use crate::render::world_draw_trace::{TraceSpriteBounds, WorldDrawTrace};
use crate::render::{MapVisualLayer, TileRenderContext, WorldAssets};
use crate::sprites::{
    RAIL_GROUND_SNOW_OR_DESERT, RAIL_TB_LEFT, RAIL_TB_LOWER, RAIL_TB_RIGHT, RAIL_TB_UPPER,
    ROAD_FLAT_HALF_H, ROAD_STREETLIGHT_META, ROADSIDE_LAMPS, ROADSIDE_TREE_META, ROADSIDE_TREES,
    TRACK_FENCE_META, catenary_pylon_world_z_delta, catenary_reference_sprite_id,
    catenary_sprite_color, catenary_wire_world_z_delta,
    collect_catenary_pylons_from_map_with_pcp_override, collect_catenary_wire_draws_from_map,
    collect_rail_pbs_reservation_draws, collect_rail_sprites_for_surface,
    collect_signal_sprite_draws, is_road_level_crossing, is_typed_rail_track_sprite,
    level_crossing_has_rail_reservation, level_crossing_rail_sprite_id_for_type,
    rail_ghost_overlay_offset, rail_pbs_reservation_offset, rail_tile_is_signals,
    rail_track_base_color, rail_trackbits_for_render, remap_rail_sprite_id, road_bits_for_render,
    road_flat_sprite_color, road_flat_sprite_index, road_tile_roadside, road_tile_snow_or_desert,
    road_tile_tram_visual_active, roadside_is_paved, signal_screen_anchor_for_side,
    signal_screen_position_for_side, signal_sprite_center_offset, track_fence_draws_for_tile,
    tram_flat_sprite_index,
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
    mut action5_sprites: Option<&mut crate::render::NewGrfAction5SpriteCache>,
) {
    let tileh = ctx.info.tileh;
    let base_z = ctx.info.base_z;
    let rb = road_bits_for_render(map, ctx.coord, mw, mh);
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
    if tileh != 0 {
        spawn_ground_sprite(
            commands,
            &sloped_or_flat_image(tileh, &assets.grass, &assets.grass_slopes),
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

    // Overlay one-way (`SPR_ONEWAY_BASE` / Action5 `0x09`).
    if let Some(tile) = ctx.tile {
        let drd = openttdrs_core::disallowed_road_directions(tile.m5);
        let road_x = (rb & 0x0F) == 0x0A;
        if let Some(slot) = openttdrs_core::oneway_action5_slot(tileh, road_x, drd)
            && let (Some(cache), Some(images)) = (action5_sprites.as_mut(), images.as_mut())
            && let Some(sprite) = cache.sprite_colored(
                openttdrs_core::ACTION5_TYPE_ONEWAY,
                slot,
                oneway_newgrf,
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
                    base_z,
                    0.025,
                    road_half_h,
                )),
            ));
        }
    }

    if let Some(tfi) = ctx.tile.and_then(|t| tram_flat_sprite_index(tileh, t.m3)) {
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
                !assets.rail.contains_key(&sid),
                (0, 0, 0),
                0,
                None,
            );
            if let Some(img) = assets.rail.get(&sid) {
                let base = tile_pos_half(ctx.tx_i32(), ctx.ty_i32(), base_z, 0.048, road_half_h);
                let offset = rail_ghost_overlay_offset(sid);
                commands.spawn((
                    MapVisualLayer,
                    ctx.map_tile_chunk(),
                    img.sprite_colored(Color::srgb(0.95, 0.52, 0.42)),
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
    let rail_base_z = spawn_rail_foundation(
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
    let rail_foundation = openttdrs_core::rail_foundation_for_trackbits(tileh, render_tb);
    let track_plan = openttdrs_core::rail_track_draw_plan(tileh, render_tb);
    rail_layers.clear();
    let mut pass_ends = [0_usize; 2];
    let mut pass_modes = [RailTrackTraceMode::Ground; 2];
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
        c
    });
    if ctx.tile.is_some_and(|t| rail_tile_is_signals(t.m5)) {
        rail_paint = rail_paint.mix(&Color::srgb(0.95, 0.88, 0.55), 0.22);
    }
    let mut pass_index = 0_usize;
    for (i, sid) in rail_layers.iter().copied().enumerate() {
        while pass_index + 1 < pass_count && i >= pass_ends[pass_index] {
            pass_index += 1;
        }
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
    // `DrawTrackBits`: una reserva PBS no recolorea toda la vía. OpenTTD
    // superpone los SINGLE_* de las pistas reservadas con PALETTE_CRASH=804.
    // La segunda capa es esencial para no confundir una reserva en un cruce
    // o túnel con una vía de otro tipo.
    if show_pbs_reservations {
        let reservation_bits = ctx.tile.map_or(0, |tile| {
            openttdrs_core::decode_rail_reservation_m2_hi(tile.m2_hi)
        });
        for (i, draw) in
            collect_rail_pbs_reservation_draws(render_tb, reservation_bits, tileh, rail_type)
                .into_iter()
                .enumerate()
        {
            let sid = draw.sprite_id;
            let mode = rail_track_trace_mode(rail_foundation, draw.halftile_corner);
            let extra_y = pbs_track_sprite_extra_y(draw.track_bit, draw.sprite_tileh);
            record_rail_pbs_trace(sid, !assets.rail.contains_key(&sid), mode, extra_y);
            let Some(img) = assets.rail.get(&sid) else {
                continue;
            };
            let offset = rail_pbs_reservation_offset(sid);
            let bevy_extra_y = pbs_extra_y_in_bevy(extra_y);
            let base = tile_pos_half(
                ctx.tx_i32(),
                ctx.ty_i32(),
                rail_base_z,
                0.026 + i as f32 * 0.0004,
                rail_half_h,
            );
            commands.spawn((
                MapVisualLayer,
                ctx.map_tile_chunk(),
                img.sprite_colored(rail_paint.mix(&Color::srgb(0.95, 0.52, 0.42), 0.26)),
                Transform::from_translation(
                    base + Vec3::new(offset.x, offset.y + bevy_extra_y, 0.0),
                ),
            ));
        }
    }
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

#[cfg(test)]
mod tests {
    use super::{
        RailTrackTraceMode, pbs_extra_y_in_bevy, pbs_track_sprite_extra_y, rail_track_trace_mode,
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
}
