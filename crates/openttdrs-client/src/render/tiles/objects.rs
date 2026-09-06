use bevy::prelude::*;
use openttdrs_core::Climate;
use openttdrs_core::prelude::*;
use openttdrs_core::{
    RoadStopSpecDef, StationSpecDef, inclined_slope_direction, is_tunnel_entrance_slope,
    rail_type_from_tile, road_stop_spec_def, road_type_from_tile, station_at_tile,
    tram_road_type_from_tile,
};

use super::bridge_draw::{bridge_span_at, spawn_bridge_deck_with_road_types};
use super::transport::{
    catenary_local_z_delta, record_road_ground_trace, resolve_custom_rail_group_sprite,
    spawn_rail_catenary_for_surface, spawn_road_catenary_for_type,
};
use super::{
    catenary_under_low_bridge,
    helpers::{
        FLAT_WATER_LAYER_FRAC, SHORE_LAYER_FRAC, spawn_empty_bounding_box,
        spawn_forced_leveled_foundation_with_child_parent, spawn_foundation_child_sprite_at,
    },
    sloped_or_flat_image, spawn_ground_sprite,
};
use crate::iso::{
    HEIGHT_PX, TILE_HALF_H, full_tile_sprite_pos, full_tile_sprite_pos_half, ground_draw_z,
    ground_tile_pos_half, overlay_pos, remap_tile_offset, road_depot_build_sprite_center,
    road_stop_build_sprite_center, shore_png_index, shore_sprite_half_h, slope_half_h,
    slope_sprite_offset, sortable_draw_z, tile_pos_half,
};
use crate::render::catenary_newgrf::{
    catenary_sprite_anchor, catenary_sprite_center, catenary_sprite_colored,
};
use crate::render::newgrf_cache::{runtime_fingerprint, vars};
use crate::render::road_newgrf::{newgrf_road_def_for_tile, road_newgrf_view_index};
use crate::render::station_newgrf::{
    NewGrfStationSpriteCache, newgrf_station_def_for_tile, station_newgrf_view_index_for_tile,
};
use crate::render::viewport_sort::{
    ParentSprite, ParentSpriteBounds, depths_in_viewport_sort_order,
};
use crate::render::world_draw_trace::{TraceSpriteBounds, WorldDrawTrace};
use crate::render::{
    AirportRadarAnim, AtlasSprite, CompanyColoredSprites, MapVisualLayer, TileRenderContext,
    ViewportSortableParent, WaterTile, WorldAssets, sprite_from_atlas_or_company_white_colour,
    viewport_insertion_key, viewport_source_depth,
};
use crate::sprites::{
    CatenarySpriteDraw, CatenaryWireDraw, CompanyColour, DockTileLayer,
    ROAD_DEPOT_GROUND_SPRITE_ID, RailDepotLayerGfx, RailStationLayer, RoadDepotLayerGfx,
    RoadStopLayerGfx, StationTileClass, TransparencyOption, airport_station_base_for_gfx,
    airport_station_ground_layers_for_gfx, airport_station_layers_for_gfx,
    airport_station_overlay_rel_for_sprite, airport_station_sprite_for_id,
    catenary_depot_wire_draw, catenary_hidden, catenary_pylon_world_z_delta,
    catenary_reference_sprite_id, catenary_sprite_color, catenary_tunnel_wire_sprite,
    catenary_wire_world_z_delta, collect_catenary_pylons_from_map_with_pcp_override,
    collect_catenary_wire_draws_from_map, dock_tile_gfx, dock_tile_is_water_part, dock_tile_layer,
    is_hidden, log_unknown_station_type_once, rail_depot_build_layers, rail_depot_seq_gfx,
    rail_depot_visual_type_index, rail_ghost_overlay_offset, rail_pbs_reservation_offset,
    rail_station_draw_layers, rail_station_ground_track_sprite_for_type, rail_station_layer_bounds,
    rail_station_layer_for_type, rail_station_overlay_rel, rail_station_sprite_meta,
    rail_waypoint_draw_layers, rail_waypoint_layer_meta, rail_waypoint_sprite_center,
    remap_rail_sprite_id, road_depot_build_layers, road_depot_seq_gfx, road_flat_sprite_index,
    road_ground_sprite_id, road_stop_build_layers, road_stop_drive_through_layers,
    road_stop_ground_index, road_stop_ground_sprite_id, road_stop_seq_gfx,
    road_waypoint_build_layers, road_waypoint_sprite_index, roadside_is_paved, station_tile_class,
    with_to_alpha,
};

fn buildings_hidden() -> bool {
    is_hidden(TransparencyOption::Buildings)
}

fn tint_building_sprite(mut sprite: Sprite) -> Sprite {
    sprite.color = with_to_alpha(sprite.color, TransparencyOption::Buildings);
    sprite
}

/// Dibuja una vista `RTSG_TUNNEL` como `DrawGroundSprite`, conservando el
/// ancla NFO de la vista Action1/2 en lugar de imponer el rombo 64×31 del
/// baseset. El portal vanilla sigue siendo la base hasta que el grupo
/// `RTSG_TUNNEL_PORTAL` tenga su sustituto de `SPR_RAILTYPE_TUNNEL_BASE`.
#[allow(clippy::too_many_arguments)]
fn spawn_custom_rail_tunnel_surface(
    commands: &mut Commands,
    ctx: &TileRenderContext,
    resolved: crate::render::signal_newgrf::ResolvedSignalSprite,
    base_z: u8,
    layer: f32,
    trace_image: u8,
) {
    WorldDrawTrace::record_sprite_with_palette_and_geometry(
        "rail-tunnel-newgrf-ground",
        "ground",
        u32::from(trace_image),
        0,
        false,
        (0, 0, 0),
        0,
        None,
    );
    let elevation = f32::from(base_z) * HEIGHT_PX;
    let position = Vec3::new(
        ctx.iso_pos.x + resolved.center_offset.x,
        ctx.iso_pos.y + resolved.center_offset.y + elevation,
        sortable_draw_z(ctx.tx_i32(), ctx.ty_i32(), base_z, layer),
    );
    commands.spawn((
        MapVisualLayer,
        ctx.map_tile_chunk(),
        resolved.sprite,
        Transform::from_translation(position),
    ));
}

/// Ancla una vista `RTSG_TUNNEL_PORTAL` al borde sortable de la boca. El
/// `roof_bounds` de `DrawTile_TunnelBridge` agrega el mismo remap `(15,15,0)`
/// que usa la capa frontal vanilla; sólo cambia el centro NFO de la vista
/// Action1/2.
fn custom_rail_tunnel_front_translation(
    ctx: &TileRenderContext,
    center_offset: Vec2,
    base_z: u8,
    layer: f32,
) -> Vec3 {
    let elevation = f32::from(base_z) * HEIGHT_PX;
    let mut position = Vec3::new(
        ctx.iso_pos.x + center_offset.x,
        ctx.iso_pos.y + center_offset.y + elevation,
        sortable_draw_z(ctx.tx_i32(), ctx.ty_i32(), base_z, layer),
    );
    let offset = remap_tile_offset(15.0, 15.0, 0.0) * 0.5;
    position.x += offset.x;
    position.y += offset.y;
    position
}

/// `DrawRoadCatenary` usa la geometría de la entrada de la parada, no los
/// roadbits almacenados en `m3/m5`. Las vistas `RSV_*` están ordenadas
/// NE, SE, SW, NW y una bahía sólo tiene un brazo conectado.
#[must_use]
const fn road_stop_catenary_bits(view: u8) -> u8 {
    match view {
        openttdrs_core::RSV_DRIVE_THROUGH_X => 0x0A, // ROAD_X
        openttdrs_core::RSV_DRIVE_THROUGH_Y => 0x05, // ROAD_Y
        0 => 0x08,                                   // ROAD_NE
        1 => 0x04,                                   // ROAD_SE
        2 => 0x02,                                   // ROAD_SW
        3 => 0x01,                                   // ROAD_NW
        _ => 0,
    }
}

/// Un road stop `NewGRF` puede desactivar expresamente la catenaria. La
/// ausencia de spec conserva el comportamiento vanilla de OpenTTD.
#[must_use]
fn road_stop_catenary_suppressed(
    map: &Map,
    stations: &[Station],
    road_stop_catalog: &[RoadStopSpecDef],
    coord: TileCoord,
) -> bool {
    station_at_tile(map, stations, coord)
        .and_then(|station| station.road_stop_spec_at(coord))
        .and_then(|spec_id| road_stop_spec_def(road_stop_catalog, spec_id))
        .is_some_and(|spec| spec.flags & openttdrs_core::ROADSTOP_FLAG_NO_CATENARY != 0)
}

/// Dibuja ambos tipos de una tesela de parada. OpenTTD normalmente tiene sólo
/// carretera en una bahía y puede tener carretera y tranvía en una
/// drive-through/waypoint; resolverlos por separado conserva esa distinción y
/// permite que cada tipo consulte sus grupos `ROTSG_CATENARY_*`.
#[allow(clippy::needless_option_as_deref, clippy::too_many_arguments)]
fn spawn_road_stop_catenary(
    commands: &mut Commands,
    map: &Map,
    dims: (u32, u32),
    assets: &WorldAssets,
    ctx: &TileRenderContext,
    tile: Tile,
    road_bits: u8,
    surface_base_z: u8,
    climate: Climate,
    road_catalog: &[openttdrs_core::RoadTypeDef],
    mut road_sprites: Option<&mut crate::render::NewGrfRoadSpriteCache>,
    mut images: Option<&mut Assets<Image>>,
    newgrf_stack: &[openttdrs_core::NewGrfEntry],
    catenary_newgrf: &[Option<openttdrs_core::DecodedSprite>],
    mut catenary_sprites: Option<&mut crate::render::NewGrfCatenarySpriteCache>,
) {
    if road_bits == 0 {
        return;
    }
    spawn_road_catenary_for_type(
        commands,
        map,
        dims,
        assets,
        ctx,
        road_type_from_tile(&tile),
        road_bits,
        0,
        surface_base_z,
        climate,
        tile,
        road_catalog,
        road_sprites.as_deref_mut(),
        images.as_deref_mut(),
        newgrf_stack,
        catenary_newgrf,
        catenary_sprites.as_deref_mut(),
    );
    if let Some(tram_type) = tram_road_type_from_tile(&tile) {
        spawn_road_catenary_for_type(
            commands,
            map,
            dims,
            assets,
            ctx,
            tram_type,
            road_bits,
            0,
            surface_base_z,
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

/// `DrawTile_Station` llama a `DrawFoundation(Leveled)` para toda estación
/// ferroviaria inclinada antes de emitir el rail y su reserva PBS. La capa
/// resultante es hija del cimiento con el mismo `OffsetGroundSprite` que el
/// oráculo exporta en píxeles `ZOOM_BASE`.
const fn station_rail_child_offset(tileh: u8) -> Option<(i32, i32, i32)> {
    if tileh == 0 { None } else { Some((0, -32, 0)) }
}

fn record_station_pbs_trace(tileh: u8, sprite_id: u32, fallback: bool) {
    if let Some(offset) = station_rail_child_offset(tileh) {
        WorldDrawTrace::record_foundation_child_sprite_with_palette(
            "station-pbs-reservation",
            sprite_id,
            804,
            fallback,
            offset,
        );
    } else {
        WorldDrawTrace::record_sprite_with_palette_and_geometry(
            "station-pbs-reservation",
            "ground",
            sprite_id,
            804,
            fallback,
            (0, 0, 0),
            0,
            None,
        );
    }
}

fn station_company_palette(owner_colour: Option<CompanyColour>) -> u32 {
    775 + u32::from(owner_colour.unwrap_or_default().as_u8())
}

/// `SPR_FLAT_BARE_LAND`; `DrawClearLandTile(ti, 3)` suma tres bloques de 19
/// sprites y el offset de pendiente a esta base.
const SPR_FLAT_BARE_LAND: u32 = 3924;
/// `SPR_FLAT_WATER_TILE`, la base que `DrawWaterClassGround` usa para mar.
const SPR_FLAT_WATER_TILE: u32 = 4061;

/// Bounds de los dos separadores invisibles de una boca de túnel.
///
/// `DrawTile_TunnelBridge` usa `rear_sep` y `front_sep` para que el techo no
/// atraviese sprites de las teselas vecinas. No son sprites transparentes:
/// son `SPR_EMPTY_BOUNDING_BOX` y sólo existen para el sorter global.
fn tunnel_sort_separator_bounds(direction: u8, front: bool) -> TraceSpriteBounds {
    match (direction & 1, front) {
        (0, false) => TraceSpriteBounds::new(0, 0, 0, 16, 1, 8),
        (0, true) => TraceSpriteBounds::new(0, 15, 0, 16, 1, 8),
        (1, false) => TraceSpriteBounds::new(0, 0, 0, 1, 16, 8),
        (1, true) => TraceSpriteBounds::new(15, 0, 0, 1, 16, 8),
        _ => unreachable!("direction & 1 sólo puede ser 0 o 1"),
    }
}

/// Parents de una boca sin catenaria en el orden de inserción C++.
///
/// El portal delantero y los dos separadores no son tres capas Bevy
/// independientes: entran juntos a `ViewportSortParentSprites`. Modelarlos
/// como conjunto permite que la entidad visible use la misma relación de
/// orden que el oráculo; `world-draw` conserva la inserción previa al sorter.
fn tunnel_sortable_parents(
    tx: i32,
    ty: i32,
    base_z: u8,
    front_sprite_id: u32,
    front_bounds: TraceSpriteBounds,
    direction: u8,
) -> Vec<ParentSprite> {
    let parent_from_bounds = |id, sprite_id, bounds: TraceSpriteBounds| {
        tile_seq_parent_sprite(
            id, sprite_id, tx, ty, base_z, bounds.ox, bounds.oy, bounds.oz, bounds.ex, bounds.ey,
            bounds.ez,
        )
    };
    vec![
        parent_from_bounds(0, front_sprite_id, front_bounds),
        parent_from_bounds(
            1,
            crate::render::EMPTY_BOUNDING_BOX_SPRITE_ID,
            tunnel_sort_separator_bounds(direction, false),
        ),
        parent_from_bounds(
            2,
            crate::render::EMPTY_BOUNDING_BOX_SPRITE_ID,
            tunnel_sort_separator_bounds(direction, true),
        ),
    ]
}
/// `SPR_SHORE_BASE`, resuelto por Action5 del baseset OpenGFX.
const SPR_SHORE_BASE: u32 = 5936;
/// `SPR_IMG_BUOY` resuelto por el OpenGFX por defecto de la partida.
///
/// El atlas usa el nombre semántico `buoy.png`, pero el contrato world-draw
/// compara el ID global que entrega OpenTTD después de resolver el baseset.
const SPR_BUOY: u32 = 9282;

/// Caja `TILE_SEQ_LINE(4, -1, 0, 0, 0, 0, SPR_IMG_BUOY)` de
/// `station_land.h`. Las boyas no usan una caja de volumen: deben poder
/// quedar visualmente por debajo de los barcos.
const fn buoy_trace_bounds() -> TraceSpriteBounds {
    TraceSpriteBounds::new(4, -1, 0, 0, 0, 0)
}

fn dock_clear_land_sprite_id(tileh: u8) -> u32 {
    SPR_FLAT_BARE_LAND + 3 * 19 + u32::from(slope_sprite_offset(tileh))
}

/// La mitad en tierra guarda el `DiagDirection` en `StationGfx`; el agua que
/// decide si OpenTTD usa `DrawShoreTile` está exactamente en ese vecino.
fn dock_water_neighbour_is_sea(map: &Map, coord: TileCoord, m5: u8) -> bool {
    let (dx, dy) = openttdrs_core::diag_dir_offset(dock_tile_gfx(m5) as u8);
    map.get(TileCoord::new(coord.x + dx, coord.y + dy))
        .is_some_and(|tile| {
            openttdrs_core::water_class(tile) == Some(openttdrs_core::WaterClass::Sea)
        })
}

fn record_dock_layer_trace(layer: DockTileLayer, owner_colour: Option<CompanyColour>) {
    WorldDrawTrace::record_sprite_with_palette_and_geometry(
        "station-dock-layer",
        "sortable",
        layer.sprite_id,
        station_company_palette(owner_colour),
        false,
        (0, 0, 0),
        0,
        Some(TraceSpriteBounds::new(
            layer.dx as i32,
            layer.dy as i32,
            layer.dz as i32,
            layer.sx,
            layer.sy,
            layer.sz,
        )),
    );
}

/// Dibuja el suelo que `DrawTile_Station` selecciona antes de `DrawRailTileSeq`.
///
/// Un muelle nunca recibe `FOUNDATION_LEVELED`: la pieza sobre tierra conserva
/// la pendiente y usa costa sólo si la otra mitad pertenece al mar; la mitad
/// plana usa el agua de su propia clase. Reducir las seis variantes a un PNG
/// plano era la causa de los muelles recortados de Kale.
fn spawn_dock_ground(
    commands: &mut Commands,
    map: &Map,
    assets: &WorldAssets,
    ctx: &TileRenderContext,
    m5: u8,
) {
    let tileh = ctx.info.tileh;
    let base_z = ctx.info.base_z;
    if dock_tile_is_water_part(m5) {
        WorldDrawTrace::record_sprite("station-dock-water", "ground", SPR_FLAT_WATER_TILE, false);
        commands.spawn((
            MapVisualLayer,
            ctx.map_tile_chunk(),
            WaterTile::ANIMATED,
            assets.water.sprite(),
            Transform::from_translation(full_tile_sprite_pos(
                ctx.tx_i32(),
                ctx.ty_i32(),
                base_z,
                FLAT_WATER_LAYER_FRAC,
            )),
        ));
        return;
    }

    if dock_water_neighbour_is_sea(map, ctx.coord, m5) {
        let shore = shore_png_index(tileh);
        WorldDrawTrace::record_sprite(
            "station-dock-shore",
            "ground",
            SPR_SHORE_BASE + shore as u32,
            false,
        );
        commands.spawn((
            MapVisualLayer,
            ctx.map_tile_chunk(),
            assets.shore[shore].sprite(),
            Transform::from_translation(full_tile_sprite_pos_half(
                ctx.tx_i32(),
                ctx.ty_i32(),
                base_z,
                SHORE_LAYER_FRAC,
                shore_sprite_half_h(tileh),
            )),
        ));
    } else {
        let slope = usize::from(slope_sprite_offset(tileh));
        WorldDrawTrace::record_sprite(
            "station-dock-land",
            "ground",
            dock_clear_land_sprite_id(tileh),
            false,
        );
        spawn_ground_sprite(
            commands,
            &assets.grass_density[3][slope],
            Color::WHITE,
            ctx,
            slope_half_h(tileh),
        );
    }
}

/// Emite la capa `TILE_SEQ_LINE` del muelle con el ancla NFO y la caja de
/// ordenamiento de `station_land.h`. A diferencia del suelo, esta pieza se
/// oculta junto a edificios cuando aplica la transparencia global.
fn spawn_dock_layer(
    commands: &mut Commands,
    assets: &WorldAssets,
    company: Option<&CompanyColoredSprites>,
    owner_colour: Option<CompanyColour>,
    ctx: &TileRenderContext,
    m5: u8,
    map_width: u32,
) {
    if buildings_hidden() {
        return;
    }

    let gfx = dock_tile_gfx(m5);
    let layer = dock_tile_layer(m5);
    let image = if dock_tile_is_water_part(m5) {
        &assets.dock_flat[gfx - 4]
    } else {
        &assets.dock_slope[gfx]
    };
    record_dock_layer_trace(layer, owner_colour);

    let local = remap_tile_offset(layer.dx, layer.dy, layer.dz) * 0.5;
    let mut pos = overlay_pos(
        ctx.iso_pos + local,
        layer.x_offs,
        layer.y_offs,
        layer.w,
        layer.h,
        ctx.info.base_z,
        0.04,
        ctx.tx_i32(),
        ctx.ty_i32(),
    );
    let source_depth = viewport_source_depth(pos.z, ctx.tx, map_width);
    pos.z = source_depth;
    commands.spawn((
        MapVisualLayer,
        ctx.map_tile_chunk(),
        tint_building_sprite(sprite_from_atlas_or_company_white_colour(
            company,
            owner_colour,
            image,
            layer.path,
        )),
        Transform::from_translation(pos),
        ViewportSortableParent {
            sprite_id: layer.sprite_id,
            bounds: dock_parent_bounds(ctx, layer),
            insertion_key: viewport_insertion_key(ctx.tx, ctx.ty, 1),
            source_depth,
        },
    ));
}

/// Prisma `TILE_SEQ_LINE` de una mitad de muelle vanilla.
///
/// La posición NFO del PNG usa `dx`/`dy` por separado, pero el sorter recibe
/// la caja de mundo de `StationGfx` con máximos inclusivos. Reutilizar la
/// conversión común mantiene el mismo contrato que la traza `world-draw`.
fn dock_parent_bounds(ctx: &TileRenderContext, layer: DockTileLayer) -> ParentSpriteBounds {
    tile_seq_parent_sprite(
        0,
        layer.sprite_id,
        ctx.tx_i32(),
        ctx.ty_i32(),
        ctx.info.base_z,
        layer.dx as i32,
        layer.dy as i32,
        layer.dz as i32,
        layer.sx,
        layer.sy,
        layer.sz,
    )
    .bounds
}

/// `DrawTile_Station` nivela las paradas viales inclinadas antes de emitir el
/// suelo. Como para una estación ferroviaria o un depósito, el `DrawGroundSprite`
/// posterior queda colgado del cimiento con este offset de pantalla normalizado.
const fn road_stop_foundation_child_offset(tileh: u8) -> Option<(i32, i32, i32)> {
    if tileh == 0 { None } else { Some((0, -32, 0)) }
}

fn record_road_stop_ground_trace(tileh: u8, sprite_id: u32, palette: u32, fallback: bool) {
    if let Some(offset) = road_stop_foundation_child_offset(tileh) {
        WorldDrawTrace::record_foundation_child_sprite_with_palette(
            "station-road-stop-ground",
            sprite_id,
            palette,
            fallback,
            offset,
        );
    } else {
        WorldDrawTrace::record_sprite_with_palette(
            "station-road-stop-ground",
            "ground",
            sprite_id,
            palette,
            fallback,
        );
    }
}

/// Registra cada `TILE_SEQ_LINE` de una parada vial con la caja de mundo del
/// oráculo. En una pendiente, `DrawFoundation(Leveled)` actualiza `ti->z` y
/// por eso el drawable sortable suma la elevación de la superficie plana.
fn record_road_layer_trace(
    role: &'static str,
    layer: &RoadStopLayerGfx,
    owner_colour: Option<CompanyColour>,
    fallback: bool,
    world_z_delta: i32,
) {
    let (ex, ey, ez) = layer.bounds;
    WorldDrawTrace::record_sprite_with_palette_and_geometry(
        role,
        "sortable",
        layer.sprite_id,
        station_company_palette(owner_colour),
        fallback,
        (0, 0, 0),
        world_z_delta,
        Some(TraceSpriteBounds::new(
            layer.dx as i32,
            layer.dy as i32,
            layer.dz as i32,
            ex,
            ey,
            ez,
        )),
    );
}

fn record_road_stop_layer_trace(
    layer: &RoadStopLayerGfx,
    owner_colour: Option<CompanyColour>,
    fallback: bool,
    world_z_delta: i32,
) {
    record_road_layer_trace(
        "station-road-stop-layer",
        layer,
        owner_colour,
        fallback,
        world_z_delta,
    );
}

/// Construye un padre sortable a partir de una línea `TILE_SEQ_LINE`.
///
/// Los extents de OpenTTD describen cantidad de unidades y el exportador C++
/// los presenta como máximos inclusivos: una extensión 3 cubre
/// `origin..origin + 2`.
#[allow(clippy::too_many_arguments)]
fn tile_seq_parent_sprite(
    id: u64,
    sprite_id: u32,
    tx: i32,
    ty: i32,
    base_z: u8,
    dx: i32,
    dy: i32,
    dz: i32,
    ex: i32,
    ey: i32,
    ez: i32,
) -> ParentSprite {
    let xmin = tx * 16 + dx;
    let ymin = ty * 16 + dy;
    let zmin = i32::from(base_z) * 8 + dz;
    ParentSprite::sprite(
        id,
        sprite_id,
        ParentSpriteBounds::new(
            xmin,
            ymin,
            zmin,
            xmin + ex - 1,
            ymin + ey - 1,
            zmin + ez - 1,
        ),
    )
}

/// Construye las cajas que entrega `AddSortableSpriteToDraw` para las capas
/// BUILD de una parada vial.
fn road_stop_parent_sprites(
    tx: i32,
    ty: i32,
    base_z: u8,
    layers: &[RoadStopLayerGfx],
) -> Vec<ParentSprite> {
    layers
        .iter()
        .enumerate()
        .map(|(index, layer)| {
            let (ex, ey, ez) = layer.bounds;
            tile_seq_parent_sprite(
                index as u64,
                layer.sprite_id,
                tx,
                ty,
                base_z,
                layer.dx as i32,
                layer.dy as i32,
                layer.dz as i32,
                ex,
                ey,
                ez,
            )
        })
        .collect()
}

/// Centros de las capas BUILD con los mismos slots locales de Z, pero asignados
/// en el orden final de `ViewportSortParentSprites`.
///
/// No cambia su ancla ni expande la banda de profundidad de la tesela: sólo
/// corrige las inversiones como `5982 → 5983` de Kale, donde el C++ devuelve
/// `5983 → 5982` después de comparar sus bounds.
fn road_stop_sorted_layer_centers(
    ctx: &TileRenderContext,
    base_z: u8,
    layers: &[RoadStopLayerGfx],
) -> Vec<Vec3> {
    let mut centers: Vec<_> = layers
        .iter()
        .map(|layer| {
            road_stop_build_sprite_center(
                ctx.iso_pos,
                ctx.tx_i32(),
                ctx.ty_i32(),
                base_z,
                layer.z,
                road_stop_seq_gfx(layer),
                layer.w,
                layer.h,
            )
        })
        .collect();
    let parents = road_stop_parent_sprites(ctx.tx_i32(), ctx.ty_i32(), base_z, layers);
    let depths: Vec<_> = centers.iter().map(|center| center.z).collect();
    for (center, depth) in centers
        .iter_mut()
        .zip(depths_in_viewport_sort_order(&parents, &depths))
    {
        center.z = depth;
    }
    centers
}

/// PNG del suelo `PALETTE_MODIFIER_COLOUR` de las cuatro orientaciones
/// vanilla. Se mantiene junto a la selección de sprite para que el atlas y
/// el caché de recolor no vuelvan a divergir del layout de OpenTTD.
fn road_stop_ground_asset_path(class: StationTileClass, dir: usize) -> &'static str {
    const BUS: [&str; 4] = [
        "assets/opengfx/tiles/bus_stop_ne_ground.png",
        "assets/opengfx/tiles/bus_stop_se_ground.png",
        "assets/opengfx/tiles/bus_stop_sw_ground.png",
        "assets/opengfx/tiles/bus_stop_nw_ground.png",
    ];
    const TRUCK: [&str; 4] = [
        "assets/opengfx/tiles/truck_stop_ground_0.png",
        "assets/opengfx/tiles/truck_stop_ground_1.png",
        "assets/opengfx/tiles/truck_stop_ground_2.png",
        "assets/opengfx/tiles/truck_stop_ground_3.png",
    ];
    match class {
        StationTileClass::Bus => BUS[dir.min(3)],
        StationTileClass::Truck => TRUCK[dir.min(3)],
        _ => unreachable!("sólo las paradas bus/camión tienen suelo vial vanilla"),
    }
}

fn record_station_rail_ground_trace(tileh: u8, sprite_id: u32, fallback: bool) {
    if let Some(offset) = station_rail_child_offset(tileh) {
        WorldDrawTrace::record_foundation_child_sprite(
            "station-rail-track",
            sprite_id,
            fallback,
            offset,
        );
    } else {
        WorldDrawTrace::record_sprite_with_palette_and_geometry(
            "station-rail-track",
            "ground",
            sprite_id,
            0,
            fallback,
            (0, 0, 0),
            0,
            None,
        );
    }
}

fn record_station_rail_layer_trace(
    layer: &crate::sprites::RailStationLayer,
    owner_colour: Option<CompanyColour>,
    fallback: bool,
    world_z_delta: i32,
) {
    if crate::sprites::rail_station_roof_glass_sprite(layer.sprite_id) {
        WorldDrawTrace::record_foundation_child_sprite_with_palette(
            "station-rail-glass",
            layer.sprite_id,
            802,
            fallback,
            (0, 0, 0),
        );
        return;
    }
    let bounds = rail_station_layer_bounds(layer.sprite_id).map(|(ex, ey, ez)| {
        TraceSpriteBounds::new(
            layer.dx as i32,
            layer.dy as i32,
            layer.dz as i32,
            ex,
            ey,
            ez,
        )
    });
    WorldDrawTrace::record_sprite_with_palette_and_geometry(
        "station-rail-layer",
        "sortable",
        layer.sprite_id,
        station_company_palette(owner_colour),
        fallback,
        (0, 0, 0),
        world_z_delta,
        bounds,
    );
}

/// Altura de mundo de una capa BUILD de estación respecto del relieve crudo.
///
/// `DrawFoundation(FOUNDATION_LEVELED)` actualiza `ti->z` antes de
/// `DrawRailTileSeq`; los parent sprites de plataforma no son children del
/// suelo, así que deben conservar esa elevación explícita en `world-draw`.
const fn station_rail_foundation_world_z_delta(raw_base_z: u8, surface_base_z: u8) -> i32 {
    surface_base_z.saturating_sub(raw_base_z) as i32 * 8
}

/// Geometría de `_rail_catenary_sprite_data_tunnel`.
///
/// `DrawRailCatenaryOnTunnel` inicia un `SpriteCombine`; el cable queda como
/// padre sortable y el techo del túnel pasa a ser su hijo `combined`. Los
/// valores conservan `BB_Z_SEPARATOR = 7` y
/// `ELRAIL_TUNNEL_OFFSET = ELRAIL_ELEVATION - BB_Z_SEPARATOR = 3`.
const fn tunnel_catenary_trace_geometry(
    dir: u8,
) -> ((i32, i32, i32), (i32, i32, i32, i32, i32, i32)) {
    if dir & 1 == 0 {
        ((0, 7, 3), (0, 0, 7, 16, 15, 1))
    } else {
        ((7, 0, 3), (0, 0, 7, 15, 16, 1))
    }
}

fn spawn_airport_radar_overlay(
    commands: &mut Commands,
    assets: &WorldAssets,
    ctx: &TileRenderContext,
    base_z: u8,
    half_h: f32,
) {
    let m7 = ctx.tile.map(|t| t.m7).unwrap_or(0);
    let frame = usize::from(openttdrs_core::airport_radar_frame(m7));
    let Some(radar) = assets.airport_radar.get(frame) else {
        return;
    };
    commands.spawn((
        MapVisualLayer,
        ctx.map_tile_chunk(),
        AirportRadarAnim { pos: ctx.coord },
        tint_building_sprite(radar.sprite()),
        Transform::from_translation(tile_pos_half(
            ctx.tx_i32(),
            ctx.ty_i32(),
            base_z,
            0.055,
            half_h,
        )),
    ));
}

/// OpenTTD sustituye la entrada completa de `StationGfx` por su variante de
/// animación antes de recorrer `DrawTileSeq`. No alcanza con animar una torre
/// genérica: las banderas tienen cuatro PNG y los radares doce, cada uno con
/// su propio ancla NFO.
#[must_use]
fn airport_station_animation_sprite_id(gfx: u8, m7: u8, sprite_id: u32) -> u32 {
    match (gfx, sprite_id) {
        (31 | 51 | 52, 2680) => 2680 + u32::from(openttdrs_core::airport_radar_frame(m7)),
        (39 | 73, 2676) => 2676 + u32::from(m7 % 4),
        _ => sprite_id,
    }
}

fn record_airport_station_layer_trace(
    sprite_id: u32,
    layer: &crate::sprites::AirportStationLayer,
    owner_colour: Option<CompanyColour>,
    fallback: bool,
) {
    let palette = if layer.company_coloured {
        station_company_palette(owner_colour)
    } else {
        0
    };
    WorldDrawTrace::record_sprite_with_palette_and_geometry(
        "station-airport-layer",
        "sortable",
        sprite_id,
        palette,
        fallback,
        (0, 0, 0),
        0,
        Some(TraceSpriteBounds::new(
            layer.dx as i32,
            layer.dy as i32,
            layer.dz as i32,
            layer.sx,
            layer.sy,
            layer.sz,
        )),
    );
}

/// Convierte el desplazamiento `TILE_SEQ_GROUND` a los píxeles de pantalla
/// que OpenTTD pasa como `extra_offs_*` a `AddTileSpriteToDraw`.
///
/// La coordenada `world` de la traza permanece en el origen de la tesela; el
/// offset no debe aplanarse, porque una cerca puede pertenecer visualmente al
/// borde de una tesela vecina.
fn airport_station_ground_layer_trace_offset(dx: f32, dy: f32, dz: f32) -> (i32, i32, i32) {
    (((dy - dx) * 8.0) as i32, ((dx + dy - dz) * 4.0) as i32, 0)
}

/// Capas `TILE_SEQ_LINE` de cualquier `StationGfx` airport vanilla.
///
/// El sprite de suelo se emite primero mediante la tabla `StationGfx`. Estas
/// capas usan el origen TILE_SEQ, las dimensiones NFO y la paleta del
/// propietario; centrarlas en la tesela convertía el túnel peatonal y los
/// hangares del aeropuerto en piezas corridas.
fn spawn_airport_station_overlays(
    commands: &mut Commands,
    assets: &WorldAssets,
    company: Option<&CompanyColoredSprites>,
    owner_colour: Option<CompanyColour>,
    ctx: &TileRenderContext,
    base_z: u8,
    gfx: u8,
) {
    let m7 = ctx.tile.map_or(0, |tile| tile.m7);
    for layer in airport_station_layers_for_gfx(gfx) {
        let sprite_id = airport_station_animation_sprite_id(gfx, m7, layer.sprite_id);
        let Some(sprite_meta) = airport_station_sprite_for_id(sprite_id) else {
            record_airport_station_layer_trace(sprite_id, layer, owner_colour, true);
            continue;
        };
        let Some(image) = assets.airport_station_sprite(sprite_id) else {
            record_airport_station_layer_trace(sprite_id, layer, owner_colour, true);
            continue;
        };
        let (xrel, yrel) = airport_station_overlay_rel_for_sprite(layer, sprite_meta);
        let pos = overlay_pos(
            ctx.iso_pos,
            xrel,
            yrel,
            sprite_meta.w,
            sprite_meta.h,
            base_z,
            layer.z,
            ctx.tx_i32(),
            ctx.ty_i32(),
        );
        record_airport_station_layer_trace(sprite_id, layer, owner_colour, false);
        let sprite = if layer.company_coloured {
            sprite_from_atlas_or_company_white_colour(
                company,
                owner_colour,
                image,
                sprite_meta.path,
            )
        } else {
            image.sprite()
        };
        commands.spawn((
            MapVisualLayer,
            ctx.map_tile_chunk(),
            tint_building_sprite(sprite),
            Transform::from_translation(pos),
        ));
    }
}

/// Emite la base y las cercas `TILE_SEQ_GROUND` de un airport `StationGfx`.
///
/// `DrawTile_Station` no trata estas cercas como un edificio. Cada una se
/// ancla con `DrawGroundSpriteAt`, que conserva tanto el desplazamiento local
/// como el orden del pase de suelo. Dibujarlas como `tile_pos_half` dejaba el
/// apron de Kale sin borde y hacía que el portal vecino pareciera pertenecer a
/// otra tesela.
#[allow(clippy::too_many_arguments)]
fn spawn_airport_station_ground_layers(
    commands: &mut Commands,
    assets: &WorldAssets,
    company: Option<&CompanyColoredSprites>,
    owner_colour: Option<CompanyColour>,
    ctx: &TileRenderContext,
    base_z: u8,
    half_h: f32,
    gfx: u8,
) -> bool {
    let Some(base) = airport_station_base_for_gfx(gfx) else {
        return false;
    };
    let Some(image) = assets.airport_station_sprite(base.sprite_id) else {
        WorldDrawTrace::record_sprite_with_palette_and_geometry(
            "station-airport-ground",
            "ground",
            base.sprite_id,
            if base.company_coloured {
                station_company_palette(owner_colour)
            } else {
                0
            },
            true,
            (0, 0, 0),
            0,
            None,
        );
        return false;
    };
    let base_palette = if base.company_coloured {
        station_company_palette(owner_colour)
    } else {
        0
    };
    WorldDrawTrace::record_sprite_with_palette_and_geometry(
        "station-airport-ground",
        "ground",
        base.sprite_id,
        base_palette,
        false,
        (0, 0, 0),
        0,
        None,
    );
    let sprite = if base.company_coloured {
        let Some(meta) = airport_station_sprite_for_id(base.sprite_id) else {
            return false;
        };
        sprite_from_atlas_or_company_white_colour(company, owner_colour, image, meta.path)
    } else {
        image.sprite()
    };
    commands.spawn((
        MapVisualLayer,
        ctx.map_tile_chunk(),
        tint_building_sprite(sprite),
        Transform::from_translation(ground_tile_pos_half(
            ctx.tx_i32(),
            ctx.ty_i32(),
            base_z,
            0.030,
            half_h,
        )),
    ));

    for (index, layer) in airport_station_ground_layers_for_gfx(gfx)
        .iter()
        .enumerate()
    {
        let Some(image) = assets.airport_station_sprite(layer.sprite_id) else {
            WorldDrawTrace::record_sprite_with_palette_and_geometry(
                "station-airport-ground-layer",
                "ground",
                layer.sprite_id,
                if layer.company_coloured {
                    station_company_palette(owner_colour)
                } else {
                    0
                },
                true,
                airport_station_ground_layer_trace_offset(layer.dx, layer.dy, layer.dz),
                0,
                None,
            );
            continue;
        };
        // `TILE_SEQ_GROUND(dx, dy, dz, sprite)` se entrega a OpenTTD como
        // `DrawGroundSpriteAt(RemapCoords(...))`. `overlay_pos` conserva el
        // ancla NFO del PNG; sólo reemplazamos su profundidad por el pase
        // ground para no convertir la cerca en un objeto sortable.
        let local = remap_tile_offset(layer.dx, layer.dy, layer.dz) * 0.5;
        let mut pos = overlay_pos(
            ctx.iso_pos + local,
            layer.x_offs,
            layer.y_offs,
            layer.w,
            layer.h,
            base_z,
            0.0,
            ctx.tx_i32(),
            ctx.ty_i32(),
        );
        pos.z = ground_draw_z(ctx.tx_i32(), ctx.ty_i32(), 0.031 + index as f32 * 0.001);
        WorldDrawTrace::record_sprite_with_palette_and_geometry(
            "station-airport-ground-layer",
            "ground",
            layer.sprite_id,
            if layer.company_coloured {
                station_company_palette(owner_colour)
            } else {
                0
            },
            false,
            airport_station_ground_layer_trace_offset(layer.dx, layer.dy, layer.dz),
            0,
            None,
        );
        commands.spawn((
            MapVisualLayer,
            ctx.map_tile_chunk(),
            tint_building_sprite(if layer.company_coloured {
                sprite_from_atlas_or_company_white_colour(company, owner_colour, image, layer.path)
            } else {
                image.sprite()
            }),
            Transform::from_translation(pos),
        ));
    }
    true
}

/// Geometría del cable de una plataforma ferroviaria para `world-draw`.
///
/// Las estaciones son planas para `DrawRailCatenaryRailway`, pero conservan
/// la misma `SortableSpriteStruct` y altura relativa que una vía normal.
fn station_catenary_wire_trace_geometry(
    tileh: u8,
    base_z: u8,
    station_tb: u8,
    draw: crate::sprites::CatenaryWireDraw,
) -> (i32, TraceSpriteBounds) {
    let (ox, oy, oz) = draw.bounds_origin;
    let (ex, ey, ez) = draw.bounds_extent;
    (
        catenary_wire_world_z_delta(tileh, base_z, station_tb, draw),
        TraceSpriteBounds::new(ox, oy, oz, ex, ey, ez),
    )
}

/// Slots ya reasignados para la subsecuencia local que OpenTTD entrega desde
/// `DrawRailCatenary` hasta `DrawRailTileSeq` de una estación ferroviaria.
///
/// Los vidrios de techo son children del último parent de la secuencia, no
/// entradas propias del sorter. Los layouts NewGRF tampoco llegan aún con sus
/// bounds completos, por lo que ambos quedan fuera de este puente parcial.
#[derive(Debug, Default, PartialEq)]
struct RailStationLocalSortDepths {
    pylons: Vec<f32>,
    wires: Vec<f32>,
    layers: Vec<Option<f32>>,
}

/// Parent que OpenTTD crea para un poste PPP de la catenaria de estación.
/// El offset sub-tesela pertenece al origen de mundo; la caja de un poste es
/// `(-1, -1, 0; 1×1×6)` relativa a ese punto.
fn station_catenary_pylon_parent_sprite(
    id: u64,
    tx: i32,
    ty: i32,
    raw_base_z: u8,
    tileh: u8,
    station_tb: u8,
    draw: CatenarySpriteDraw,
) -> ParentSprite {
    let world_z_delta = draw.pcp_direction.map_or(0, |pcp| {
        catenary_pylon_world_z_delta(tileh, raw_base_z, station_tb, pcp)
    });
    let x = tx * 16 + draw.tile_dx as i32;
    let y = ty * 16 + draw.tile_dy as i32;
    let z = i32::from(raw_base_z) * 8 + world_z_delta;
    ParentSprite::sprite(
        id,
        catenary_reference_sprite_id(draw.sprite_id),
        ParentSpriteBounds::new(x - 1, y - 1, z, x - 1, y - 1, z + 5),
    )
}

/// Parent que OpenTTD crea para un cable de estación. El ancla Z se consulta
/// en la pendiente y puede diferir de la base plana de la plataforma.
fn station_catenary_wire_parent_sprite(
    id: u64,
    tx: i32,
    ty: i32,
    raw_base_z: u8,
    tileh: u8,
    station_tb: u8,
    draw: CatenaryWireDraw,
) -> ParentSprite {
    let (world_z_delta, bounds) =
        station_catenary_wire_trace_geometry(tileh, raw_base_z, station_tb, draw);
    let x = tx * 16 + bounds.ox;
    let y = ty * 16 + bounds.oy;
    let z = i32::from(raw_base_z) * 8 + world_z_delta + bounds.oz;
    ParentSprite::sprite(
        id,
        catenary_reference_sprite_id(draw.sprite_id),
        ParentSpriteBounds::new(
            x,
            y,
            z,
            x + bounds.ex - 1,
            y + bounds.ey - 1,
            z + bounds.ez - 1,
        ),
    )
}

/// Parent BUILD de una capa rail vanilla. `DrawFoundation(Leveled)` ya alteró
/// la altura de `TileInfo`, de modo que la caja toma el delta respecto de la
/// altura cruda, igual que la traza `world-draw`.
fn station_rail_layer_parent_sprite(
    id: u64,
    tx: i32,
    ty: i32,
    raw_base_z: u8,
    rail_base_z: u8,
    layer: RailStationLayer,
) -> Option<ParentSprite> {
    let (ex, ey, ez) = rail_station_layer_bounds(layer.sprite_id)?;
    let x = tx * 16 + layer.dx as i32;
    let y = ty * 16 + layer.dy as i32;
    let z = i32::from(raw_base_z) * 8
        + station_rail_foundation_world_z_delta(raw_base_z, rail_base_z)
        + layer.dz as i32;
    Some(ParentSprite::sprite(
        id,
        layer.sprite_id,
        ParentSpriteBounds::new(x, y, z, x + ex - 1, y + ey - 1, z + ez - 1),
    ))
}

/// Aplica el sorter C++ a los parents locales que la estación vanilla ya
/// conoce: postes, cables y capas BUILD. Conserva los slots Bevy existentes
/// para no extender la banda de profundidad a teselas vecinas.
#[allow(clippy::too_many_arguments)]
fn station_rail_local_sorted_depths(
    tx: i32,
    ty: i32,
    raw_base_z: u8,
    rail_base_z: u8,
    tileh: u8,
    station_tb: u8,
    pylons: &[CatenarySpriteDraw],
    wires: &[CatenaryWireDraw],
    layers: &[RailStationLayer],
) -> RailStationLocalSortDepths {
    let mut parents = Vec::with_capacity(pylons.len() + wires.len() + layers.len());
    let mut source_depths = Vec::with_capacity(parents.capacity());

    for draw in pylons {
        parents.push(station_catenary_pylon_parent_sprite(
            parents.len() as u64,
            tx,
            ty,
            raw_base_z,
            tileh,
            station_tb,
            *draw,
        ));
        source_depths.push(sortable_draw_z(tx, ty, rail_base_z, draw.z_layer));
    }
    for (index, draw) in wires.iter().copied().enumerate() {
        parents.push(station_catenary_wire_parent_sprite(
            parents.len() as u64,
            tx,
            ty,
            raw_base_z,
            tileh,
            station_tb,
            draw,
        ));
        source_depths.push(sortable_draw_z(
            tx,
            ty,
            rail_base_z,
            0.035 + index as f32 * 0.0004,
        ));
    }

    let mut layer_parent_indices = Vec::new();
    for (layer_index, layer) in layers.iter().copied().enumerate() {
        // El vidrio de techo no tiene bounds porque es un child del parent
        // BUILD anterior; no puede asignarse como parent independiente.
        if let Some(parent) = station_rail_layer_parent_sprite(
            parents.len() as u64,
            tx,
            ty,
            raw_base_z,
            rail_base_z,
            layer,
        ) {
            parents.push(parent);
            source_depths.push(sortable_draw_z(tx, ty, rail_base_z, layer.z));
            layer_parent_indices.push(layer_index);
        }
    }

    let sorted_depths = depths_in_viewport_sort_order(&parents, &source_depths);
    let mut next = 0;
    let pylons = sorted_depths[next..next + pylons.len()].to_vec();
    next += pylons.len();
    let wires = sorted_depths[next..next + wires.len()].to_vec();
    next += wires.len();
    let mut layers = vec![None; layers.len()];
    for layer_index in layer_parent_indices {
        layers[layer_index] = Some(sorted_depths[next]);
        next += 1;
    }
    debug_assert_eq!(next, sorted_depths.len());
    RailStationLocalSortDepths {
        pylons,
        wires,
        layers,
    }
}

/// Reúne exactamente la parte de `DrawRailCatenary` que se emite antes del
/// `DrawRailTileSeq` de estación. Separar la selección del spawn permite que
/// ambos pasen por el mismo vector local del sorter.
fn collect_station_rail_catenary_draws(
    map: &Map,
    dims: (u32, u32),
    ctx: &TileRenderContext,
    tile: Tile,
    station_tb: u8,
    tileh: u8,
) -> (Vec<CatenarySpriteDraw>, Vec<CatenaryWireDraw>) {
    if !rail_type_from_tile(tile).has_catenary() || catenary_hidden() {
        return (Vec::new(), Vec::new());
    }

    let low_bridge = catenary_under_low_bridge(map, ctx.coord, dims);
    let mut pylons = Vec::new();
    collect_catenary_pylons_from_map_with_pcp_override(
        map,
        ctx.coord,
        dims.0,
        dims.1,
        crate::sprites::OTTD_MP_RAIL,
        station_tb,
        tileh,
        low_bridge.pylon_pcp_override,
        &mut pylons,
    );
    if !openttdrs_core::station_tile_can_have_wires(tile.m3) || low_bridge.hide_wires {
        return (pylons, Vec::new());
    }

    let mut wires = Vec::new();
    collect_catenary_wire_draws_from_map(
        map,
        ctx.coord,
        dims.0,
        dims.1,
        crate::sprites::OTTD_MP_RAIL,
        station_tb,
        tileh,
        &mut wires,
    );
    (pylons, wires)
}

/// `DrawRailCatenary` dentro de `DrawTile_Station`.
///
/// OpenTTD emite postes y cables después del suelo/reserva de la plataforma,
/// pero antes de `DrawRailTileSeq` (techo, edificio y demás capas de estación).
/// Mantener ese orden evita que la catenaria quede visualmente por encima de
/// un andén y permite que `world-draw` compare los comandos reales.
#[allow(clippy::too_many_arguments)]
fn spawn_station_rail_catenary(
    commands: &mut Commands,
    assets: &WorldAssets,
    ctx: &TileRenderContext,
    station_tb: u8,
    tileh: u8,
    rail_base_z: u8,
    pylons: &[CatenarySpriteDraw],
    wires: &[CatenaryWireDraw],
    sorted_depths: Option<&RailStationLocalSortDepths>,
    catenary_newgrf: &[Option<openttdrs_core::DecodedSprite>],
    catenary_sprites: &mut Option<&mut crate::render::NewGrfCatenarySpriteCache>,
    images: &mut Option<&mut Assets<Image>>,
) {
    let tint = catenary_sprite_color();

    // `DrawRailCatenaryRailway` coloca primero los PPP. A diferencia de los
    // cables, una estación puede prohibir wire y aun así autorizar postes.
    for (index, draw) in pylons.iter().copied().enumerate() {
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
            catenary_pylon_world_z_delta(tileh, ctx.info.base_z, station_tb, pcp)
        });
        WorldDrawTrace::record_sprite_with_palette_and_world_geometry(
            "station-catenary-pylon",
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
        let local_z = catenary_local_z_delta(world_z_delta, ctx.info.base_z, rail_base_z);
        let mut position = catenary_sprite_center(
            ctx.tx_i32(),
            ctx.ty_i32(),
            rail_base_z,
            draw.z_layer,
            draw.tile_dx - 1.0,
            draw.tile_dy - 1.0,
            local_z as f32,
            anchor,
        );
        if let Some(depth) = sorted_depths
            .and_then(|depths| depths.pylons.get(index))
            .copied()
        {
            position.z = depth;
        }
        commands.spawn((
            MapVisualLayer,
            ctx.map_tile_chunk(),
            sprite,
            Transform::from_translation(position),
        ));
    }

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
        let (world_z_delta, bounds) =
            station_catenary_wire_trace_geometry(tileh, ctx.info.base_z, station_tb, draw);
        WorldDrawTrace::record_sprite_with_palette_and_geometry(
            "station-catenary-wire",
            "sortable",
            catenary_reference_sprite_id(sid),
            0,
            sprite.is_none(),
            (0, 0, 0),
            world_z_delta,
            Some(bounds),
        );
        let Some((sprite, anchor)) = sprite.zip(anchor) else {
            continue;
        };
        let z = 0.035 + i as f32 * 0.0004;
        let local_z =
            catenary_local_z_delta(world_z_delta + bounds.oz, ctx.info.base_z, rail_base_z);
        let mut position = catenary_sprite_center(
            ctx.tx_i32(),
            ctx.ty_i32(),
            rail_base_z,
            z,
            bounds.ox as f32,
            bounds.oy as f32,
            local_z as f32,
            anchor,
        );
        if let Some(depth) = sorted_depths
            .and_then(|depths| depths.wires.get(i))
            .copied()
        {
            position.z = depth;
        }
        commands.spawn((
            MapVisualLayer,
            ctx.map_tile_chunk(),
            sprite,
            Transform::from_translation(position),
        ));
    }
}

#[cfg(test)]
#[allow(clippy::too_many_arguments, clippy::needless_option_as_deref)]
pub(crate) fn spawn_station_tile(
    commands: &mut Commands,
    map: &Map,
    dims: (u32, u32),
    assets: &WorldAssets,
    company: Option<&CompanyColoredSprites>,
    owner_colour: Option<CompanyColour>,
    ctx: &TileRenderContext,
    stations: &[Station],
    slope_half_ground: f32,
    show_pbs_reservations: bool,
    station_catalog: &[StationSpecDef],
    road_stop_catalog: &[RoadStopSpecDef],
    station_sprites: Option<&mut NewGrfStationSpriteCache>,
    images: Option<&mut Assets<Image>>,
    catenary_newgrf: &[Option<openttdrs_core::DecodedSprite>],
    catenary_sprites: Option<&mut crate::render::NewGrfCatenarySpriteCache>,
    foundation_newgrf: &[Option<openttdrs_core::DecodedSprite>],
    action5_sprites: Option<&mut crate::render::NewGrfAction5SpriteCache>,
    roadstop_action5: &[Option<openttdrs_core::DecodedSprite>],
    climate: openttdrs_core::Climate,
    newgrf_stack: &[openttdrs_core::NewGrfEntry],
) {
    let road_catalog = openttdrs_core::vanilla_road_type_catalog();
    spawn_station_tile_with_world_and_road_types(
        commands,
        map,
        dims,
        assets,
        company,
        owner_colour,
        ctx,
        stations,
        slope_half_ground,
        show_pbs_reservations,
        station_catalog,
        road_stop_catalog,
        &road_catalog,
        None,
        station_sprites,
        images,
        catenary_newgrf,
        catenary_sprites,
        foundation_newgrf,
        action5_sprites,
        roadstop_action5,
        climate,
        newgrf_stack,
        None,
    );
}

/// Variante del renderer de estaciones que recibe el catálogo y la caché de
/// `RoadTypes` del mundo. Esto permite que una parada `NewGRF` comparta la
/// misma resolución de catenaria que una carretera, sin romper los callers de
/// tests/fallback que sólo disponen de los assets vanilla.
#[allow(clippy::too_many_arguments, clippy::needless_option_as_deref)]
pub(crate) fn spawn_station_tile_with_world_and_road_types(
    commands: &mut Commands,
    map: &Map,
    dims: (u32, u32),
    assets: &WorldAssets,
    company: Option<&CompanyColoredSprites>,
    owner_colour: Option<CompanyColour>,
    ctx: &TileRenderContext,
    stations: &[Station],
    slope_half_ground: f32,
    show_pbs_reservations: bool,
    station_catalog: &[StationSpecDef],
    road_stop_catalog: &[RoadStopSpecDef],
    road_catalog: &[openttdrs_core::RoadTypeDef],
    mut road_sprites: Option<&mut crate::render::NewGrfRoadSpriteCache>,
    mut station_sprites: Option<&mut NewGrfStationSpriteCache>,
    mut images: Option<&mut Assets<Image>>,
    catenary_newgrf: &[Option<openttdrs_core::DecodedSprite>],
    mut catenary_sprites: Option<&mut crate::render::NewGrfCatenarySpriteCache>,
    foundation_newgrf: &[Option<openttdrs_core::DecodedSprite>],
    mut action5_sprites: Option<&mut crate::render::NewGrfAction5SpriteCache>,
    roadstop_action5: &[Option<openttdrs_core::DecodedSprite>],
    climate: openttdrs_core::Climate,
    newgrf_stack: &[openttdrs_core::NewGrfEntry],
    world: Option<openttdrs_core::RoadStopWorldContext<'_>>,
) {
    let tileh = ctx.info.tileh;
    let base_z = ctx.info.base_z;
    let stop_kind = station_at_tile(map, stations, ctx.coord).map(|s| s.stop_kind);
    let m6 = ctx.tile.map_or(0, |t| t.m6);
    let m5 = ctx.tile.map_or(0, |t| t.m5);
    let class = station_tile_class(m6, stop_kind);
    // Las paradas de bus/camión ya tienen un suelo completo en
    // `_station_display_datas_{bus,truck}`. OpenTTD no dibuja césped bajo
    // ellas, ni siquiera en una pendiente: primero nivela y luego cuelga ese
    // suelo de la fundación. El césped genérico hacía visible una capa extra
    // alrededor de los andenes y ocultaba el desvío en la traza.
    if tileh != 0
        && !matches!(
            class,
            StationTileClass::Rail
                | StationTileClass::RailWaypoint
                | StationTileClass::Bus
                | StationTileClass::Truck
                | StationTileClass::RoadWaypoint
                | StationTileClass::Dock
        )
    {
        let grass = sloped_or_flat_image(tileh, &assets.grass, &assets.grass_slopes);
        spawn_ground_sprite(commands, &grass, Color::WHITE, ctx, slope_half_ground);
    }
    let rail_type = ctx
        .tile
        .map_or(openttdrs_core::RailType::Rail, rail_type_from_tile);

    match class {
        StationTileClass::Rail | StationTileClass::RailWaypoint => {
            if tileh == 0 {
                let grass = sloped_or_flat_image(0, &assets.grass, &assets.grass_slopes);
                spawn_ground_sprite(commands, &grass, Color::WHITE, ctx, slope_half_ground);
            }
            // `DrawTile_Station` nivela cualquier estación ferroviaria antes
            // de emitir el suelo y las capas `TILE_SEQ`. No es la fundación
            // que deriva de TrackBits: para una estación OpenTTD fuerza
            // `FOUNDATION_LEVELED`, deja el sprite de vía como child en
            // `(0, -32)` y pinta las plataformas sobre una superficie plana.
            // El césped inclinado previo desplazaba el andén y podía asomar
            // bajo su cimiento en las pendientes de Kale.
            let station_tb = if m5 & 1 != 0 { 0x02 } else { 0x01 };
            let station_foundation = spawn_forced_leveled_foundation_with_child_parent(
                commands,
                map,
                dims,
                assets,
                ctx,
                tileh,
                "station-rail",
                "station-rail-foundation",
                foundation_newgrf,
                action5_sprites.as_deref_mut(),
                images.as_deref_mut(),
            );
            let rail_base_z = station_foundation.surface_base_z;
            let foundation_child_parent = station_foundation.child_parent;
            let rail_half_h = TILE_HALF_H;
            let rail_foundation_z_delta =
                station_rail_foundation_world_z_delta(ctx.info.base_z, rail_base_z);
            // Resolver antes del suelo vanilla: un `TileLayout` completo es
            // autoritativo y puede suprimir `SPR_RAIL_TRACK_*` con
            // `DODRAW=0` o reemplazarlo por su propio sprite de ground.
            let station_layout = if !buildings_hidden() {
                resolve_station_layout_for_tile(
                    map,
                    stations,
                    ctx,
                    m5,
                    owner_colour,
                    station_catalog,
                    climate,
                    newgrf_stack,
                    world,
                )
            } else {
                None
            };
            let mut used_newgrf_layout_ground = false;
            if let Some((def, layout, runtime_fp, _view_idx)) = station_layout.as_ref()
                && let (Some(cache), Some(image_store)) =
                    (station_sprites.as_mut(), images.as_mut())
            {
                used_newgrf_layout_ground = spawn_newgrf_station_layout_ground(
                    commands,
                    ctx,
                    rail_base_z,
                    dims.0,
                    foundation_child_parent,
                    def,
                    owner_colour,
                    *runtime_fp,
                    layout,
                    cache,
                    image_store,
                );
            }
            // OpenTTD: ground SPR_RAIL_TRACK_* bajo estación y waypoint (`station_land.h`).
            let track_sid = rail_station_ground_track_sprite_for_type(m5, tileh, rail_type);
            if class == StationTileClass::Rail && !used_newgrf_layout_ground {
                record_station_rail_ground_trace(
                    tileh,
                    track_sid,
                    !assets.rail.contains_key(&track_sid),
                );
            }
            if !used_newgrf_layout_ground && let Some(img) = assets.rail.get(&track_sid) {
                let position = full_tile_sprite_pos_half(
                    ctx.tx_i32(),
                    ctx.ty_i32(),
                    rail_base_z,
                    0.02,
                    rail_half_h,
                );
                if let Some(parent) = foundation_child_parent {
                    spawn_foundation_child_sprite_at(
                        commands,
                        img.sprite(),
                        ctx,
                        position,
                        dims.0,
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
            }
            // `DrawStationTile`: una plataforma reservada no oscurece toda
            // la estación. OpenTTD vuelve a dibujar el SINGLE_X/Y de su eje
            // con PALETTE_CRASH. El bit vive en m6, no en la reserva m2 de
            // una vía normal.
            if show_pbs_reservations && m6 & 0x04 != 0 {
                let sid = remap_rail_sprite_id(1005 + u32::from(m5 & 1), rail_type);
                record_station_pbs_trace(tileh, sid, !assets.has_exact_pbs_rail_sprite(sid));
                if let Some(img) = assets.pbs_rail_sprite(sid) {
                    let base = full_tile_sprite_pos_half(
                        ctx.tx_i32(),
                        ctx.ty_i32(),
                        rail_base_z,
                        0.026,
                        rail_half_h,
                    );
                    let offset = rail_ghost_overlay_offset(sid);
                    let position = base + Vec3::new(offset.x, offset.y, 0.0);
                    if let Some(parent) = foundation_child_parent {
                        spawn_foundation_child_sprite_at(
                            commands,
                            img.sprite(),
                            ctx,
                            position,
                            dims.0,
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
                }
            }
            let overlay_layers = if class == StationTileClass::RailWaypoint {
                rail_waypoint_draw_layers(m5)
            } else {
                rail_station_draw_layers(m5)
            };
            // NewGRF: sustituir overlays OpenGFX por la vista según tiletype
            // `m5` (#46). OpenTTD ejecuta `DrawNewStationTile` después de
            // `DrawFoundation(Leveled)`, también en pendientes; conservar el
            // parent de esa fundación evita que el sprite vuelva a la banda
            // de profundidad de la tesela inclinada.
            let mut newgrf_overlay = None;
            if !station_layout
                .as_ref()
                .is_some_and(|(_, layout, _, _)| layout.complete)
                && matches!(
                    class,
                    StationTileClass::Rail | StationTileClass::RailWaypoint
                )
                && !buildings_hidden()
                && let Some(def) =
                    newgrf_station_def_for_tile(station_catalog, map, stations, ctx.coord)
                && let (Some(cache), Some(images)) = (station_sprites.as_mut(), images.as_mut())
            {
                let colour_u8 = owner_colour.map(CompanyColour::as_u8).unwrap_or(0);
                let mut a2 = world.map_or_else(
                    || {
                        openttdrs_core::action2_eval_ctx_for_station_tile_with_catalog(
                            map,
                            stations,
                            station_catalog,
                            ctx.coord,
                            colour_u8,
                            climate,
                            def.newgrf_type_tables.as_ref(),
                            def.newgrf_grf_version,
                        )
                    },
                    |world| {
                        openttdrs_core::action2_eval_ctx_for_station_tile_with_catalog_and_world(
                            map,
                            stations,
                            station_catalog,
                            ctx.coord,
                            colour_u8,
                            climate,
                            def.newgrf_type_tables.as_ref(),
                            def.newgrf_grf_version,
                            openttdrs_core::StationAction2WorldContext {
                                towns: world.towns,
                                companies: world.companies,
                                industries: world.industries,
                                cargo_spec_catalog: world.cargo_spec_catalog,
                            },
                        )
                    },
                );
                a2.set_grf_params(openttdrs_core::stack_params_for_grfid(
                    newgrf_stack,
                    def.newgrf_grfid,
                ));
                let mut callback_ctx = a2.clone();
                let view_idx = station_newgrf_view_index_for_tile(def, m5, &mut callback_ctx);
                if let Some(view) = def.newgrf_view(view_idx)
                    && let Some(handle) =
                        cache.handle_for_runtime(def, view_idx, owner_colour, &mut a2, images)
                {
                    let pos3 = crate::iso::overlay_pos(
                        ctx.iso_pos,
                        f32::from(view.x_offs),
                        f32::from(view.y_offs),
                        f32::from(view.width),
                        f32::from(view.height),
                        rail_base_z,
                        0.04,
                        ctx.tx_i32(),
                        ctx.ty_i32(),
                    );
                    newgrf_overlay = Some((handle, pos3));
                }
            }
            // La secuencia custom se emite después de la catenaria, igual que
            // `TO_BUILDINGS` en OpenTTD. El booleano se calcula antes porque
            // decide si debemos omitir las capas vanilla al ordenar.
            let has_newgrf_layout_sequence = station_layout
                .as_ref()
                .is_some_and(|(_, layout, _, _)| layout.complete && !layout.sequence.is_empty())
                && station_sprites.is_some()
                && images.is_some();
            let used_newgrf =
                newgrf_overlay.is_some() || used_newgrf_layout_ground || has_newgrf_layout_sequence;
            let (station_pylons, station_wires) = ctx.tile.map_or_else(
                || (Vec::new(), Vec::new()),
                |tile| collect_station_rail_catenary_draws(map, dims, ctx, tile, station_tb, tileh),
            );
            // La secuencia vanilla aporta todas las cajas BUILD conocidas. Un
            // layout NewGRF aún no publica sus parents/children completos, de
            // modo que no se lo mezcla con la catenaria en este paso parcial.
            let station_sort_depths = (class == StationTileClass::Rail
                && !buildings_hidden()
                && !used_newgrf)
                .then(|| {
                    station_rail_local_sorted_depths(
                        ctx.tx_i32(),
                        ctx.ty_i32(),
                        ctx.info.base_z,
                        rail_base_z,
                        tileh,
                        station_tb,
                        &station_pylons,
                        &station_wires,
                        overlay_layers,
                    )
                });
            spawn_station_rail_catenary(
                commands,
                assets,
                ctx,
                station_tb,
                tileh,
                rail_base_z,
                &station_pylons,
                &station_wires,
                station_sort_depths.as_ref(),
                catenary_newgrf,
                &mut catenary_sprites,
                &mut images,
            );
            if has_newgrf_layout_sequence
                && let Some((def, layout, runtime_fp, _view_idx)) = station_layout.as_ref()
                && let (Some(cache), Some(image_store)) =
                    (station_sprites.as_mut(), images.as_mut())
            {
                let _ = spawn_newgrf_station_layout_sequence(
                    commands,
                    ctx,
                    rail_base_z,
                    dims.0,
                    def,
                    owner_colour,
                    *runtime_fp,
                    layout,
                    cache,
                    image_store,
                );
            }
            if let Some((handle, pos3)) = newgrf_overlay {
                let sprite = tint_building_sprite(Sprite {
                    image: handle,
                    color: Color::WHITE,
                    ..default()
                });
                if let Some(parent) = foundation_child_parent {
                    spawn_foundation_child_sprite_at(commands, sprite, ctx, pos3, dims.0, parent);
                } else {
                    // En plano no existe un parent de `DrawFoundation`; la
                    // entidad conserva la ruta directa que usa OpenTTD.
                    commands.spawn((
                        MapVisualLayer,
                        ctx.map_tile_chunk(),
                        sprite,
                        Transform::from_translation(pos3),
                    ));
                }
            }
            if !buildings_hidden() && !used_newgrf {
                for (layer_index, base_layer) in overlay_layers.iter().enumerate() {
                    // `DrawStationTile` deja los waypoints vanilla sin offset,
                    // pero suma el desplazamiento de railtype a cada capa de
                    // estación normal (`DrawRailTileSeq`).
                    let layer = if class == StationTileClass::RailWaypoint {
                        *base_layer
                    } else {
                        rail_station_layer_for_type(*base_layer, rail_type)
                    };
                    if class == StationTileClass::Rail {
                        record_station_rail_layer_trace(
                            &layer,
                            owner_colour,
                            !assets.rail.contains_key(&layer.sprite_id),
                            rail_foundation_z_delta,
                        );
                    }
                    let Some(img) = assets.rail.get(&layer.sprite_id) else {
                        continue;
                    };
                    let mut pos3 = if class == StationTileClass::RailWaypoint {
                        let Some((w, h, nfo_xrel, nfo_yrel)) =
                            rail_waypoint_layer_meta(layer.sprite_id)
                        else {
                            continue;
                        };
                        rail_waypoint_sprite_center(
                            ctx.iso_pos,
                            ctx.tx_i32(),
                            ctx.ty_i32(),
                            rail_base_z,
                            layer.z,
                            &layer,
                            nfo_xrel,
                            nfo_yrel,
                            w,
                            h,
                        )
                    } else {
                        let Some((w, h, nfo_xrel, nfo_yrel)) =
                            rail_station_sprite_meta(layer.sprite_id)
                        else {
                            continue;
                        };
                        let (xrel, yrel) = rail_station_overlay_rel(&layer, nfo_xrel, nfo_yrel);
                        crate::iso::overlay_pos(
                            ctx.iso_pos,
                            xrel,
                            yrel,
                            w,
                            h,
                            rail_base_z,
                            layer.z,
                            ctx.tx_i32(),
                            ctx.ty_i32(),
                        )
                    };
                    if let Some(depth) = station_sort_depths
                        .as_ref()
                        .and_then(|depths| depths.layers.get(layer_index))
                        .copied()
                        .flatten()
                    {
                        pos3.z = depth;
                    }
                    let sprite = if crate::sprites::rail_station_roof_glass_sprite(layer.sprite_id)
                    {
                        // OpenTTD: `PALETTE_TO_TRANSPARENT` oscurece el destino (máscara),
                        // no pinta el blob CC amarillo del PNG como vidrio tintado.
                        use crate::sprites::with_to_alpha;
                        let mut s = img.sprite_colored(Color::srgba(0.0, 0.0, 0.0, 0.28));
                        s.color = with_to_alpha(s.color, TransparencyOption::Buildings);
                        s
                    } else {
                        tint_building_sprite(sprite_from_atlas_or_company_white_colour(
                            company,
                            owner_colour,
                            img,
                            &format!("rail_{}.png", layer.sprite_id),
                        ))
                    };
                    commands.spawn((
                        MapVisualLayer,
                        ctx.map_tile_chunk(),
                        sprite,
                        Transform::from_translation(pos3),
                    ));
                }
            }
        }
        StationTileClass::Bus | StationTileClass::Truck => {
            let is_drive_through = openttdrs_core::is_drive_through_orientation(m5);
            // Igual que `DrawTile_Station`: toda parada vial inclinada usa
            // una fundación nivelada, independiente de los road bits. La
            // superficie resultante es plana y las capas BUILD se ordenan a
            // su altura, no a la del relieve crudo.
            let road_stop_foundation = spawn_forced_leveled_foundation_with_child_parent(
                commands,
                map,
                dims,
                assets,
                ctx,
                tileh,
                "road-stop",
                "road-stop-foundation",
                foundation_newgrf,
                action5_sprites.as_deref_mut(),
                images.as_deref_mut(),
            );
            let road_stop_base_z = road_stop_foundation.surface_base_z;
            let foundation_child_parent = road_stop_foundation.child_parent;
            if is_drive_through {
                let road_bits = if m5 == openttdrs_core::RSV_DRIVE_THROUGH_X {
                    0x0A
                } else {
                    0x05
                };
                spawn_paved_road_stop_link(
                    commands,
                    assets,
                    ctx,
                    road_stop_base_z,
                    tileh,
                    road_bits,
                    dims.0,
                    foundation_child_parent,
                );
            }
            let view_idx = usize::from(m5.min(5));
            let custom_layout = resolve_road_stop_layout_for_tile(
                map,
                stations,
                ctx,
                view_idx,
                road_stop_catalog,
                climate,
                newgrf_stack,
                world,
            );
            let mut used_newgrf_ground = false;
            if let Some((spec_id, layout, runtime_fp, _draw_mode)) = custom_layout.as_ref()
                && let (Some(cache), Some(image_store)) =
                    (action5_sprites.as_mut(), images.as_mut())
            {
                used_newgrf_ground = spawn_newgrf_road_stop_layout_ground(
                    commands,
                    ctx,
                    road_stop_base_z,
                    dims.0,
                    foundation_child_parent,
                    *spec_id,
                    *runtime_fp,
                    layout,
                    cache,
                    image_store,
                );
            }
            if !is_drive_through && !used_newgrf_ground {
                let ground_dir = road_stop_ground_index(m5).min(3);
                let image = if class == StationTileClass::Bus {
                    assets
                        .bus_stop_grounds
                        .get(ground_dir)
                        .unwrap_or(&assets.bus_stop_grounds[0])
                } else {
                    assets
                        .station_grounds
                        .get(ground_dir)
                        .unwrap_or(&assets.station_grounds[0])
                };
                if let Some(sprite_id) = road_stop_ground_sprite_id(class, ground_dir) {
                    spawn_road_stop_ground_sprite(
                        commands,
                        image,
                        company,
                        owner_colour,
                        ctx,
                        road_stop_base_z,
                        tileh,
                        sprite_id,
                        road_stop_ground_asset_path(class, ground_dir),
                        dims.0,
                        foundation_child_parent,
                    );
                } else {
                    // La rama sólo admite Bus/Truck y `ground_dir` ya está
                    // acotado a 0..=3; conservar una degradación explícita
                    // evita que un dato malformado bloquee sus capas BUILD.
                    bevy::log::warn!(
                        "Parada vial sin ground vanilla: clase={class:?}, dirección={ground_dir}"
                    );
                }
            }
            spawn_road_stop_buildings(
                commands,
                assets,
                company,
                owner_colour,
                map,
                stations,
                ctx,
                road_stop_base_z,
                class,
                view_idx,
                road_stop_catalog,
                roadstop_action5,
                action5_sprites.as_deref_mut(),
                images.as_deref_mut(),
                climate,
                newgrf_stack,
                world,
            );
            // OpenTTD dibuja la catenaria después del suelo y del layout BUILD
            // de la parada. La entrada de una bahía es un único brazo diagonal;
            // una drive-through usa el eje completo. En pendientes la parada
            // ya fue nivelada, por eso el selector recibe SLOPE_FLAT (0) y la
            // altura de la superficie resultante de `DrawFoundation`.
            if !road_stop_catenary_suppressed(map, stations, road_stop_catalog, ctx.coord)
                && let Some(tile) = ctx.tile
            {
                spawn_road_stop_catenary(
                    commands,
                    map,
                    dims,
                    assets,
                    ctx,
                    tile,
                    road_stop_catenary_bits(m5),
                    road_stop_base_z,
                    climate,
                    road_catalog,
                    road_sprites.as_deref_mut(),
                    images.as_deref_mut(),
                    newgrf_stack,
                    catenary_newgrf,
                    catenary_sprites.as_deref_mut(),
                );
            }
        }
        StationTileClass::RoadWaypoint => {
            // `DrawTile_Station` trata el waypoint vial como un
            // drive-through: la geometría sale de `GetStationGfx` (m5), no
            // del nibble de roadside de m3. En una pendiente OpenTTD fuerza
            // `FOUNDATION_LEVELED` y todas las capas siguientes son children
            // de ese parent; el césped inclinado anterior dejaba una segunda
            // superficie visible debajo del waypoint.
            let waypoint_foundation = spawn_forced_leveled_foundation_with_child_parent(
                commands,
                map,
                dims,
                assets,
                ctx,
                tileh,
                "road-waypoint",
                "road-waypoint-foundation",
                foundation_newgrf,
                action5_sprites.as_deref_mut(),
                images.as_deref_mut(),
            );
            let waypoint_base_z = waypoint_foundation.surface_base_z;
            let foundation_child_parent = waypoint_foundation.child_parent;
            let waypoint_bits = match m5 {
                openttdrs_core::RSV_DRIVE_THROUGH_X => 0x0A,
                openttdrs_core::RSV_DRIVE_THROUGH_Y => 0x05,
                // Saves viejos y fixtures sintéticos pueden no conservar m5.
                // Sólo usar m3 como fallback cuando contiene un eje válido;
                // nunca convertir los bits de una acera en orientación.
                _ => match ctx.tile.map(|tile| tile.m3 & 0x0F) {
                    Some(0x05) => 0x05,
                    Some(0x0A) => 0x0A,
                    _ => 0x0A,
                },
            };
            let roadside = ctx.tile.map(|tile| (tile.m3 >> 2) & 0x03).unwrap_or(1);
            let snow_or_desert =
                ctx.tile.is_some_and(|tile| tile.m8 & (1 << 15) != 0) || climate.uses_snow_ground();
            let paved = roadside_is_paved(roadside) && !snow_or_desert;
            let flat_idx = road_flat_sprite_index(0, waypoint_bits);
            let foundation = if tileh == 0 {
                0
            } else {
                openttdrs_core::FOUNDATION_LEVELED
            };
            // Un waypoint puede publicar un `TileLayoutSpriteGroup` propio.
            // El grupo se selecciona con el mismo `view` que la parada
            // drive-through; sus sprites BUILD se emiten después del suelo.
            let waypoint_layout = resolve_road_stop_layout_for_tile(
                map,
                stations,
                ctx,
                usize::from(m5.min(5)),
                road_stop_catalog,
                climate,
                newgrf_stack,
                world,
            );

            // Action1/2/3 default del roadtype sustituye el suelo vanilla
            // cuando existe. Es la misma vista que usa `DrawRoadTile`, pero
            // con tileh=0 porque la fundación del waypoint ya niveló la
            // superficie.
            let mut used_newgrf_ground = false;
            if let Some(tile) = ctx.tile
                && let Some(def) = newgrf_road_def_for_tile(road_catalog, tile)
                && let Some(view) = def.newgrf_view(road_newgrf_view_index(0, waypoint_bits))
                && let (Some(cache), Some(image_store)) = (road_sprites.as_mut(), images.as_mut())
            {
                let mut action2 = openttdrs_core::action2_eval_ctx_for_road_tile(
                    map,
                    tile,
                    ctx.coord,
                    climate,
                    def.newgrf_type_tables.as_ref(),
                    road_catalog,
                );
                action2.set_grf_params(openttdrs_core::stack_params_for_grfid(
                    newgrf_stack,
                    def.newgrf_grfid,
                ));
                if let Some(handle) = cache.handle_for_runtime(
                    def,
                    road_newgrf_view_index(0, waypoint_bits),
                    &mut action2,
                    image_store,
                ) {
                    let position = overlay_pos(
                        ctx.iso_pos,
                        f32::from(view.x_offs),
                        f32::from(view.y_offs),
                        f32::from(view.width),
                        f32::from(view.height),
                        waypoint_base_z,
                        0.02,
                        ctx.tx_i32(),
                        ctx.ty_i32(),
                    );
                    spawn_waypoint_surface_sprite(
                        commands,
                        ctx,
                        Sprite {
                            image: handle,
                            color: Color::WHITE,
                            ..default()
                        },
                        position,
                        dims.0,
                        foundation_child_parent,
                    );
                    used_newgrf_ground = true;
                }
            }

            if !used_newgrf_ground {
                let sprite_id = road_ground_sprite_id(flat_idx, paved, snow_or_desert);
                record_road_ground_trace("road-waypoint-ground", sprite_id, foundation);
                let image = if paved {
                    assets.road_paved.get(flat_idx)
                } else {
                    assets.road_flat.get(flat_idx)
                };
                if let Some(image) = image {
                    let paint = if snow_or_desert {
                        Color::srgb(0.82, 0.88, 0.98)
                    } else {
                        Color::WHITE
                    };
                    let position = full_tile_sprite_pos_half(
                        ctx.tx_i32(),
                        ctx.ty_i32(),
                        waypoint_base_z,
                        0.02,
                        TILE_HALF_H,
                    );
                    spawn_waypoint_surface_sprite(
                        commands,
                        ctx,
                        image.sprite_colored(paint),
                        position,
                        dims.0,
                        foundation_child_parent,
                    );
                }
            }

            // Un waypoint vial es una parada drive-through para la catenaria,
            // pero no dibuja el overlay de acera de una parada normal. El
            // overlay NewGRF del tranvía sí se conserva si el tile declara un
            // tipo de tranvía, y sigue la misma fundación que el asfalto.
            if let Some(tile) = ctx.tile
                && tram_road_type_from_tile(&tile).is_some()
            {
                let tram_idx = road_flat_sprite_index(0, waypoint_bits);
                let mut used_tram_newgrf = false;
                if let Some(def) =
                    crate::render::road_newgrf::newgrf_tram_def_for_tile(road_catalog, tile)
                    && let Some(view) = def.newgrf_view(tram_idx)
                    && let (Some(cache), Some(image_store)) =
                        (road_sprites.as_mut(), images.as_mut())
                {
                    let mut action2 = openttdrs_core::action2_eval_ctx_for_road_tile(
                        map,
                        tile,
                        ctx.coord,
                        climate,
                        def.newgrf_type_tables.as_ref(),
                        road_catalog,
                    );
                    action2.set_grf_params(openttdrs_core::stack_params_for_grfid(
                        newgrf_stack,
                        def.newgrf_grfid,
                    ));
                    if let Some(handle) =
                        cache.handle_for_runtime(def, tram_idx, &mut action2, image_store)
                    {
                        let position = overlay_pos(
                            ctx.iso_pos,
                            f32::from(view.x_offs),
                            f32::from(view.y_offs),
                            f32::from(view.width),
                            f32::from(view.height),
                            waypoint_base_z,
                            0.028,
                            ctx.tx_i32(),
                            ctx.ty_i32(),
                        );
                        spawn_waypoint_surface_sprite(
                            commands,
                            ctx,
                            Sprite {
                                image: handle,
                                color: Color::WHITE,
                                ..default()
                            },
                            position,
                            dims.0,
                            foundation_child_parent,
                        );
                        used_tram_newgrf = true;
                    }
                }
                if !used_tram_newgrf && let Some(image) = assets.tram_flat.get(tram_idx) {
                    let position = full_tile_sprite_pos_half(
                        ctx.tx_i32(),
                        ctx.ty_i32(),
                        waypoint_base_z,
                        0.028,
                        TILE_HALF_H,
                    );
                    spawn_waypoint_surface_sprite(
                        commands,
                        ctx,
                        image.sprite(),
                        position,
                        dims.0,
                        foundation_child_parent,
                    );
                }
            }
            // `WaypGround` pertenece al draw mode de la spec. Cuando el GRF
            // usa el registro 0x100, `resolve_road_stop_layout_for_tile` ya
            // devolvió el valor efectivo después de evaluar Action2.
            if let Some((spec_id, layout, runtime_fp, draw_mode)) = waypoint_layout.as_ref()
                && *draw_mode & openttdrs_core::ROADSTOP_DRAW_MODE_WAYP_GROUND != 0
                && let (Some(cache), Some(image_store)) =
                    (action5_sprites.as_mut(), images.as_mut())
            {
                let _ = spawn_newgrf_road_stop_layout_ground(
                    commands,
                    ctx,
                    waypoint_base_z,
                    dims.0,
                    foundation_child_parent,
                    *spec_id,
                    *runtime_fp,
                    layout,
                    cache,
                    image_store,
                );
            }
            if !road_stop_catenary_suppressed(map, stations, road_stop_catalog, ctx.coord)
                && let Some(tile) = ctx.tile
            {
                spawn_road_stop_catenary(
                    commands,
                    map,
                    dims,
                    assets,
                    ctx,
                    tile,
                    road_stop_catenary_bits(m5),
                    waypoint_base_z,
                    climate,
                    road_catalog,
                    road_sprites.as_deref_mut(),
                    images.as_deref_mut(),
                    newgrf_stack,
                    catenary_newgrf,
                    catenary_sprites.as_deref_mut(),
                );
            }
            // OpenTTD emite los postes después de la catenaria mediante
            // `DrawRailTileSeq(TO_BUILDINGS, ...)`. Un layout custom conserva
            // sus parents/children y sólo si no se pudo materializar entero
            // se usa el layout vanilla de dos postes por eje.
            let mut used_newgrf_waypoint_layout = false;
            if let Some((spec_id, layout, runtime_fp, _draw_mode)) = waypoint_layout.as_ref()
                && let (Some(cache), Some(image_store)) =
                    (action5_sprites.as_mut(), images.as_mut())
            {
                used_newgrf_waypoint_layout = spawn_newgrf_road_stop_layout_sequence(
                    commands,
                    ctx,
                    waypoint_base_z,
                    dims.0,
                    *spec_id,
                    *runtime_fp,
                    layout,
                    cache,
                    image_store,
                );
            }
            if !used_newgrf_waypoint_layout {
                spawn_road_waypoint_buildings(
                    commands,
                    assets,
                    company,
                    owner_colour,
                    ctx,
                    waypoint_base_z,
                    u8::from(waypoint_bits == 0x05),
                    dims.0,
                    foundation_child_parent,
                );
            }
        }
        StationTileClass::Oilrig => {
            // `DrawTile_Station`: Oilrig usa `SPR_FLAT_WATER_TILE` como
            // suelo. Aunque su estación pueda tener servicio aéreo, su m6
            // no es `StationType::Airport`; interpretar la capacidad como
            // aeropuerto dibujaba un apron gris sobre la plataforma y hacía
            // parecer que la industria se salía del mapa.
            //
            // OpenTTD exige que un oilrig esté sobre agua plana. La aserción
            // deja visible un save corrupto en debug sin sustituirlo por un
            // aeropuerto en builds de usuario.
            debug_assert_eq!(tileh, 0, "Oilrig fuera de agua plana");
            WorldDrawTrace::record_sprite("station-oilrig-water", "ground", 4061, false);
            commands.spawn((
                MapVisualLayer,
                ctx.map_tile_chunk(),
                WaterTile::ANIMATED,
                assets.water.sprite(),
                Transform::from_translation(full_tile_sprite_pos(
                    ctx.tx_i32(),
                    ctx.ty_i32(),
                    base_z,
                    FLAT_WATER_LAYER_FRAC,
                )),
            ));
        }
        StationTileClass::Dock => {
            spawn_dock_ground(commands, map, assets, ctx, m5);
            spawn_dock_layer(commands, assets, company, owner_colour, ctx, m5, dims.0);
        }
        StationTileClass::Buoy => {
            // `DrawTile_Station` siempre llama primero a
            // `DrawWaterClassGround` para una boya. La rama anterior sólo
            // emitía el PNG de la boya; por eso sobre tierra/agua del mapa
            // veía el fondo equivocado y la auditoría Kale perdía 4061.
            WorldDrawTrace::record_sprite(
                "station-buoy-water",
                "ground",
                SPR_FLAT_WATER_TILE,
                false,
            );
            commands.spawn((
                MapVisualLayer,
                ctx.map_tile_chunk(),
                WaterTile::ANIMATED,
                assets.water.sprite(),
                Transform::from_translation(full_tile_sprite_pos(
                    ctx.tx_i32(),
                    ctx.ty_i32(),
                    base_z,
                    FLAT_WATER_LAYER_FRAC,
                )),
            ));
            if buildings_hidden() {
                return;
            }
            let half_h = if tileh == 0 {
                TILE_HALF_H
            } else {
                slope_half_h(tileh)
            };
            WorldDrawTrace::record_sprite_with_geometry(
                "station-buoy",
                "sortable",
                SPR_BUOY,
                false,
                (0, 0, 0),
                0,
                Some(buoy_trace_bounds()),
            );
            commands.spawn((
                MapVisualLayer,
                ctx.map_tile_chunk(),
                tint_building_sprite(assets.buoy.sprite()),
                Transform::from_translation(tile_pos_half(
                    ctx.tx_i32(),
                    ctx.ty_i32(),
                    base_z,
                    0.04,
                    half_h,
                )),
            ));
        }
        StationTileClass::Airport => {
            if buildings_hidden() {
                return;
            }
            let half_h = if tileh == 0 {
                TILE_HALF_H
            } else {
                slope_half_h(tileh)
            };
            // Los tiles `MP_STATION` importados conservan el índice
            // `StationGfx` vanilla completo, no el enum interno 0..7.
            if !spawn_airport_station_ground_layers(
                commands,
                assets,
                company,
                owner_colour,
                ctx,
                base_z,
                half_h,
                m5,
            ) {
                let tower_pos = tile_pos_half(ctx.tx_i32(), ctx.ty_i32(), base_z, 0.04, half_h);
                commands.spawn((
                    MapVisualLayer,
                    ctx.map_tile_chunk(),
                    tint_building_sprite(assets.airport_station_gfx_sprite(m5).sprite()),
                    Transform::from_translation(tower_pos),
                ));
            }
            spawn_airport_station_overlays(
                commands,
                assets,
                company,
                owner_colour,
                ctx,
                base_z,
                m5,
            );
        }
        StationTileClass::Other(_) => {
            // No dibujar una parada vial plausible cuando `m6` contiene un
            // tipo que todavía no implementamos. Magenta es el marcador de
            // recurso/tile desconocido usado por el renderer.
            log_unknown_station_type_once(m6);
            let warning_ground = sloped_or_flat_image(tileh, &assets.grass, &assets.grass_slopes);
            commands.spawn((
                MapVisualLayer,
                ctx.map_tile_chunk(),
                warning_ground.sprite_colored(Color::srgb(1.0, 0.0, 1.0)),
                Transform::from_translation(tile_pos_half(
                    ctx.tx_i32(),
                    ctx.ty_i32(),
                    base_z,
                    0.006,
                    if tileh == 0 {
                        TILE_HALF_H
                    } else {
                        slope_half_h(tileh)
                    },
                )),
            ));
        }
    }
}

/// Base de una parada pasante: OpenTTD usa `SPR_ROAD_PAVED_STRAIGHT_*`, no el
/// suelo de hierba/andén de una bahía convencional.
#[allow(clippy::too_many_arguments)] // Conserva el contexto de fundación del caller.
fn spawn_paved_road_stop_link(
    commands: &mut Commands,
    assets: &WorldAssets,
    ctx: &TileRenderContext,
    base_z: u8,
    tileh: u8,
    road_bits: u8,
    map_width: u32,
    foundation_child_parent: Option<Entity>,
) {
    // El layout vanilla siempre selecciona `SPR_ROAD_PAVED_STRAIGHT_*`.
    // Cuando había pendiente ya fue absorbida por `Foundation::Leveled`; no
    // debemos indexar una rampa de carretera con el `tileh` original.
    let fi = road_flat_sprite_index(0, road_bits);
    record_road_stop_ground_trace(tileh, road_ground_sprite_id(fi, true, false), 0, false);
    let position =
        full_tile_sprite_pos_half(ctx.tx_i32(), ctx.ty_i32(), base_z, 0.025, TILE_HALF_H);
    if let Some(parent) = foundation_child_parent {
        spawn_foundation_child_sprite_at(
            commands,
            assets.road_paved[fi].sprite(),
            ctx,
            position,
            map_width,
            parent,
        );
    } else {
        commands.spawn((
            MapVisualLayer,
            ctx.map_tile_chunk(),
            assets.road_paved[fi].sprite(),
            Transform::from_translation(position),
        ));
    }
}

/// Resuelve el layout `TileSeq` de la estación con el mismo contexto Action2
/// que la vista plana. El `view_idx` sólo identifica la orientación/callback;
/// las referencias del layout seleccionan su primer sprite Action1 y por eso
/// no se usa como índice adicional en la textura.
#[allow(clippy::too_many_arguments)]
fn resolve_station_layout_for_tile<'a>(
    map: &Map,
    stations: &[Station],
    ctx: &TileRenderContext,
    m5: u8,
    owner_colour: Option<CompanyColour>,
    station_catalog: &'a [StationSpecDef],
    climate: Climate,
    newgrf_stack: &[openttdrs_core::NewGrfEntry],
    world: Option<openttdrs_core::RoadStopWorldContext<'_>>,
) -> Option<(
    &'a StationSpecDef,
    openttdrs_core::newgrf_sprites::ResolvedTileLayout,
    u32,
    usize,
)> {
    let def = newgrf_station_def_for_tile(station_catalog, map, stations, ctx.coord)?;
    let colour_u8 = owner_colour.map(CompanyColour::as_u8).unwrap_or(0);
    let mut action2 = world.map_or_else(
        || {
            openttdrs_core::action2_eval_ctx_for_station_tile_with_catalog(
                map,
                stations,
                station_catalog,
                ctx.coord,
                colour_u8,
                climate,
                def.newgrf_type_tables.as_ref(),
                def.newgrf_grf_version,
            )
        },
        |world| {
            openttdrs_core::action2_eval_ctx_for_station_tile_with_catalog_and_world(
                map,
                stations,
                station_catalog,
                ctx.coord,
                colour_u8,
                climate,
                def.newgrf_type_tables.as_ref(),
                def.newgrf_grf_version,
                openttdrs_core::StationAction2WorldContext {
                    towns: world.towns,
                    companies: world.companies,
                    industries: world.industries,
                    cargo_spec_catalog: world.cargo_spec_catalog,
                },
            )
        },
    );
    action2.set_grf_params(openttdrs_core::stack_params_for_grfid(
        newgrf_stack,
        def.newgrf_grfid,
    ));
    let mut callback_ctx = action2.clone();
    let view_idx = station_newgrf_view_index_for_tile(def, m5, &mut callback_ctx);
    let layout = def.newgrf_tile_layout_runtime(view_idx, &mut action2)?;
    let runtime_fp = def
        .newgrf_runtime
        .as_ref()
        .map_or(0, |_| runtime_fingerprint(&action2, vars::STATION, false));
    Some((def, layout, runtime_fp, view_idx))
}

/// Emite el sprite de suelo de un layout de estación. Un layout completo sin
/// `ground` es intencional (`DODRAW = 0`) y suprime el suelo vanilla.
#[allow(clippy::too_many_arguments)]
fn spawn_newgrf_station_layout_ground(
    commands: &mut Commands,
    ctx: &TileRenderContext,
    base_z: u8,
    map_width: u32,
    foundation_child_parent: Option<Entity>,
    def: &StationSpecDef,
    owner_colour: Option<CompanyColour>,
    runtime_fp: u32,
    layout: &openttdrs_core::newgrf_sprites::ResolvedTileLayout,
    cache: &mut NewGrfStationSpriteCache,
    images: &mut Assets<Image>,
) -> bool {
    if !layout.complete {
        return false;
    }
    let Some(ground) = layout.ground.as_ref() else {
        return true;
    };
    let handle = cache.handle_for_layout(def, 0, owner_colour, runtime_fp, &ground.sprite, images);
    let position = overlay_pos(
        ctx.iso_pos,
        f32::from(ground.sprite.x_offs),
        f32::from(ground.sprite.y_offs),
        f32::from(ground.sprite.width),
        f32::from(ground.sprite.height),
        base_z,
        0.025,
        ctx.tx_i32(),
        ctx.ty_i32(),
    );
    let sprite = tint_building_sprite(Sprite {
        image: handle,
        color: Color::WHITE,
        ..default()
    });
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
    true
}

/// Emite parents y children de la secuencia `BUILD` de una estación NewGRF.
/// La geometría y el anclaje siguen la ruta de road stops, pero la textura se
/// obtiene del caché Action1/3 de estaciones y conserva el color de compañía.
#[allow(clippy::too_many_arguments)]
fn spawn_newgrf_station_layout_sequence(
    commands: &mut Commands,
    ctx: &TileRenderContext,
    base_z: u8,
    map_width: u32,
    def: &StationSpecDef,
    owner_colour: Option<CompanyColour>,
    runtime_fp: u32,
    layout: &openttdrs_core::newgrf_sprites::ResolvedTileLayout,
    cache: &mut NewGrfStationSpriteCache,
    images: &mut Assets<Image>,
) -> bool {
    if !layout.complete || layout.sequence.is_empty() {
        return false;
    }

    let mut last_parent: Option<(Entity, Vec2)> = None;
    let mut emitted = false;
    for (index, layer) in layout.sequence.iter().enumerate() {
        let slot = u16::try_from(index.saturating_add(1)).unwrap_or(u16::MAX);
        let handle =
            cache.handle_for_layout(def, slot, owner_colour, runtime_fp, &layer.sprite, images);
        let width = f32::from(layer.sprite.width);
        let height = f32::from(layer.sprite.height);
        let origin = crate::iso::RoadStopSeqGfx {
            dx: f32::from(layer.origin[0]),
            dy: f32::from(layer.origin[1]),
            dz: if layer.is_parent() {
                f32::from(layer.origin[2])
            } else {
                0.0
            },
            x_offs: f32::from(layer.sprite.x_offs),
            y_offs: f32::from(layer.sprite.y_offs),
            remap_x_adj: 0.0,
        };
        let layer_z = 0.05 + index as f32 * 0.0003;
        let sprite = tint_building_sprite(Sprite {
            image: handle,
            color: Color::WHITE,
            ..default()
        });

        if layer.is_parent() {
            let position = road_stop_build_sprite_center(
                ctx.iso_pos,
                ctx.tx_i32(),
                ctx.ty_i32(),
                base_z,
                layer_z,
                origin,
                width,
                height,
            );
            let source_depth = viewport_source_depth(position.z, ctx.tx, map_width);
            let sprite_id = u32::MAX.saturating_sub(u32::try_from(index).unwrap_or(u32::MAX));
            let bounds = tile_seq_parent_sprite(
                index as u64,
                sprite_id,
                ctx.tx_i32(),
                ctx.ty_i32(),
                base_z,
                i32::from(layer.origin[0]),
                i32::from(layer.origin[1]),
                i32::from(layer.origin[2]),
                i32::from(layer.extent[0]),
                i32::from(layer.extent[1]),
                i32::from(layer.extent[2]),
            )
            .bounds;
            let entity = commands
                .spawn((
                    MapVisualLayer,
                    ctx.map_tile_chunk(),
                    sprite,
                    Transform::from_translation(Vec3::new(position.x, position.y, source_depth)),
                    ViewportSortableParent {
                        sprite_id,
                        bounds,
                        insertion_key: viewport_insertion_key(
                            ctx.tx,
                            ctx.ty,
                            u8::try_from(index.saturating_add(2)).unwrap_or(u8::MAX),
                        ),
                        source_depth,
                    },
                ))
                .id();
            last_parent = Some((
                entity,
                Vec2::new(position.x - width / 2.0, position.y + height / 2.0),
            ));
        } else if let Some((parent, parent_top_left)) = last_parent {
            let position = newgrf_road_stop_child_center(
                parent_top_left,
                layer.origin,
                width,
                height,
                ctx.tx_i32(),
                ctx.ty_i32(),
                base_z,
                layer_z,
            );
            let source_depth = viewport_source_depth(position.z, ctx.tx, map_width);
            commands.spawn((
                MapVisualLayer,
                ctx.map_tile_chunk(),
                sprite,
                Transform::from_translation(Vec3::new(position.x, position.y, source_depth)),
                crate::render::ViewportSortableChild {
                    parent,
                    source_depth,
                },
            ));
        } else {
            // El formato permite un child huérfano. OpenTTD lo entrega como
            // sprite de suelo; conservarlo en el ancla de la tesela evita
            // perder una pieza visible por un GRF mal formado.
            let position = road_stop_build_sprite_center(
                ctx.iso_pos,
                ctx.tx_i32(),
                ctx.ty_i32(),
                base_z,
                layer_z,
                origin,
                width,
                height,
            );
            commands.spawn((
                MapVisualLayer,
                ctx.map_tile_chunk(),
                sprite,
                Transform::from_translation(position),
            ));
        }
        emitted = true;
    }
    emitted
}

/// Tipo sintético en la caché Action5 para vistas Action3 del catálogo `RoadStops`.
const ROADSTOP_ACTION3_CACHE_TYPE: u8 = 0x14;

/// Resuelve el grupo Action2 de una parada con el mismo contexto que usa el
/// callback/render actual. El resultado conserva el fingerprint para que todas
/// las piezas de un `TileSeq` compartan la misma entrada de caché cuando
/// cambian random bits, carga o variables del mapa.
#[allow(clippy::too_many_arguments)]
fn resolve_road_stop_layout_for_tile(
    map: &Map,
    stations: &[Station],
    ctx: &TileRenderContext,
    view: usize,
    road_stop_catalog: &[RoadStopSpecDef],
    climate: Climate,
    newgrf_stack: &[openttdrs_core::NewGrfEntry],
    world: Option<openttdrs_core::RoadStopWorldContext<'_>>,
) -> Option<(
    u16,
    openttdrs_core::newgrf_sprites::ResolvedTileLayout,
    u32,
    u8,
)> {
    let station = station_at_tile(map, stations, ctx.coord)?;
    let spec_id = station.road_stop_spec_at(ctx.coord)?;
    let def = road_stop_spec_def(road_stop_catalog, spec_id)?;
    let view_u8 = u8::try_from(view.min(5)).unwrap_or(0);
    let mut action2 = world.map_or_else(
        || {
            openttdrs_core::action2_eval_ctx_for_road_stop_tile_with_catalog(
                map,
                stations,
                road_stop_catalog,
                ctx.coord,
                view_u8,
                climate,
            )
        },
        |world| {
            openttdrs_core::action2_eval_ctx_for_road_stop_tile_with_catalog_and_world(
                map,
                stations,
                road_stop_catalog,
                world,
                ctx.coord,
                view_u8,
                climate,
            )
        },
    );
    action2.set_grf_params(openttdrs_core::stack_params_for_grfid(
        newgrf_stack,
        def.grfid,
    ));
    let layout = def.newgrf_tile_layout_runtime(view, &mut action2)?;
    // Layout register evaluation can execute Action2 STO operations. Include
    // those final values in the cache key, otherwise a changed DODRAW/offset
    // would keep the previous texture and transform alive.
    let runtime_fp = def
        .newgrf_runtime
        .as_ref()
        .map_or(0, |_| runtime_fingerprint(&action2, vars::ROAD_STOP, false));
    let draw_mode = if def.flags & openttdrs_core::ROADSTOP_FLAG_DRAW_MODE_REGISTER != 0 {
        u8::try_from(action2.registers_100.get(&0x100).copied().unwrap_or(0) & 0xFF)
            .unwrap_or_default()
    } else {
        def.draw_mode
    };
    Some((spec_id, layout, runtime_fp, draw_mode))
}

/// El renderer compacto sólo puede materializar layouts compuestos cuando
/// todos sus registros son constantes y cada entrada usa un set Action1
/// decodificado. Un layout incompleto se deja entero en manos de OpenGFX/Action5
/// para no mezclar offsets de sprites base con piezas custom.
fn road_stop_layout_is_static(layout: &openttdrs_core::newgrf_sprites::ResolvedTileLayout) -> bool {
    layout.complete
}

/// Emite el suelo custom de un `TileLayout` de road stop. OpenTTD lo dibuja
/// como ground sprite antes del asfalto/overlay; en una pendiente queda como
/// child de la fundación nivelada, igual que el suelo vanilla.
#[allow(clippy::too_many_arguments)]
fn spawn_newgrf_road_stop_layout_ground(
    commands: &mut Commands,
    ctx: &TileRenderContext,
    base_z: u8,
    map_width: u32,
    foundation_child_parent: Option<Entity>,
    spec_id: u16,
    runtime_fp: u32,
    layout: &openttdrs_core::newgrf_sprites::ResolvedTileLayout,
    cache: &mut crate::render::NewGrfAction5SpriteCache,
    images: &mut Assets<Image>,
) -> bool {
    if !road_stop_layout_is_static(layout) {
        return false;
    }
    let Some(ground) = layout.ground.as_ref() else {
        // A constant DODRAW=0 is a valid layout result: OpenTTD suppresses
        // the ground sprite instead of falling back to the vanilla road.
        return true;
    };
    let slot = spec_id.saturating_mul(64);
    let handle = cache.handle_for_variant(
        ROADSTOP_ACTION3_CACHE_TYPE,
        slot,
        runtime_fp,
        &ground.sprite,
        images,
    );
    let position = overlay_pos(
        ctx.iso_pos,
        f32::from(ground.sprite.x_offs),
        f32::from(ground.sprite.y_offs),
        f32::from(ground.sprite.width),
        f32::from(ground.sprite.height),
        base_z,
        0.025,
        ctx.tx_i32(),
        ctx.ty_i32(),
    );
    let sprite = Sprite {
        image: handle,
        color: Color::WHITE,
        ..default()
    };
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
    true
}

/// Convierte el offset de pantalla de una entrada child a su centro Bevy.
/// `DrawRailTileSeq` pasa esos bytes como píxeles firmados y la coordenada Y
/// de OpenTTD crece hacia abajo, mientras que el viewport Bevy usa Y positiva
/// hacia arriba.
// The values mirror the independent NFO register fields used by a TileSeq
// child; keeping them explicit makes the coordinate conversion auditable.
#[allow(clippy::too_many_arguments)]
fn newgrf_road_stop_child_center(
    parent_top_left: Vec2,
    origin: [i8; 3],
    width: f32,
    height: f32,
    tx: i32,
    ty: i32,
    base_z: u8,
    layer_z: f32,
) -> Vec3 {
    let child_top_left = parent_top_left + Vec2::new(f32::from(origin[0]), -f32::from(origin[1]));
    Vec3::new(
        child_top_left.x + width / 2.0,
        child_top_left.y - height / 2.0,
        sortable_draw_z(tx, ty, base_z, layer_z),
    )
}

/// Emite la secuencia BUILD de un `TileLayout` custom. Los parents usan las
/// cajas 3D de `TILE_SEQ_LINE`; los children conservan el anclaje de pantalla
/// al último parent, que es la semántica de `AddChildSpriteScreen` de OpenTTD.
#[allow(clippy::too_many_arguments)]
fn spawn_newgrf_road_stop_layout_sequence(
    commands: &mut Commands,
    ctx: &TileRenderContext,
    base_z: u8,
    map_width: u32,
    spec_id: u16,
    runtime_fp: u32,
    layout: &openttdrs_core::newgrf_sprites::ResolvedTileLayout,
    cache: &mut crate::render::NewGrfAction5SpriteCache,
    images: &mut Assets<Image>,
) -> bool {
    if !road_stop_layout_is_static(layout) || layout.sequence.is_empty() {
        return false;
    }

    let slot_base = spec_id.saturating_mul(64).saturating_add(1);
    let mut last_parent: Option<(Entity, Vec2)> = None;
    let mut emitted = false;
    for (index, layer) in layout.sequence.iter().enumerate() {
        let slot = slot_base.saturating_add(u16::try_from(index).unwrap_or(u16::MAX));
        let handle = cache.handle_for_variant(
            ROADSTOP_ACTION3_CACHE_TYPE,
            slot,
            runtime_fp,
            &layer.sprite,
            images,
        );
        let width = f32::from(layer.sprite.width);
        let height = f32::from(layer.sprite.height);
        let origin = crate::iso::RoadStopSeqGfx {
            dx: f32::from(layer.origin[0]),
            dy: f32::from(layer.origin[1]),
            dz: if layer.is_parent() {
                f32::from(layer.origin[2])
            } else {
                0.0
            },
            x_offs: f32::from(layer.sprite.x_offs),
            y_offs: f32::from(layer.sprite.y_offs),
            remap_x_adj: 0.0,
        };
        let layer_z = 0.05 + index as f32 * 0.0003;
        let sprite = tint_building_sprite(Sprite {
            image: handle,
            color: Color::WHITE,
            ..default()
        });

        if layer.is_parent() {
            let position = road_stop_build_sprite_center(
                ctx.iso_pos,
                ctx.tx_i32(),
                ctx.ty_i32(),
                base_z,
                layer_z,
                origin,
                width,
                height,
            );
            let source_depth = viewport_source_depth(position.z, ctx.tx, map_width);
            let sprite_id = u32::MAX.saturating_sub(u32::try_from(index).unwrap_or(u32::MAX));
            let bounds = tile_seq_parent_sprite(
                index as u64,
                sprite_id,
                ctx.tx_i32(),
                ctx.ty_i32(),
                base_z,
                i32::from(layer.origin[0]),
                i32::from(layer.origin[1]),
                i32::from(layer.origin[2]),
                i32::from(layer.extent[0]),
                i32::from(layer.extent[1]),
                i32::from(layer.extent[2]),
            )
            .bounds;
            let entity = commands
                .spawn((
                    MapVisualLayer,
                    ctx.map_tile_chunk(),
                    sprite,
                    Transform::from_translation(Vec3::new(position.x, position.y, source_depth)),
                    ViewportSortableParent {
                        sprite_id,
                        bounds,
                        insertion_key: viewport_insertion_key(
                            ctx.tx,
                            ctx.ty,
                            u8::try_from(index.saturating_add(2)).unwrap_or(u8::MAX),
                        ),
                        source_depth,
                    },
                ))
                .id();
            // `overlay_pos` stores the centre; converting back to the sprite's
            // upper-left gives the same anchor used by AddChildSpriteScreen.
            last_parent = Some((
                entity,
                Vec2::new(position.x - width / 2.0, position.y + height / 2.0),
            ));
        } else if let Some((parent, parent_top_left)) = last_parent {
            let position = newgrf_road_stop_child_center(
                parent_top_left,
                layer.origin,
                width,
                height,
                ctx.tx_i32(),
                ctx.ty_i32(),
                base_z,
                layer_z,
            );
            let source_depth = viewport_source_depth(position.z, ctx.tx, map_width);
            commands.spawn((
                MapVisualLayer,
                ctx.map_tile_chunk(),
                sprite,
                Transform::from_translation(Vec3::new(position.x, position.y, source_depth)),
                crate::render::ViewportSortableChild {
                    parent,
                    source_depth,
                },
            ));
        } else {
            // A child before its first parent is legal in the raw format. The
            // C++ path draws it as a ground sprite; use the tile anchor as the
            // equivalent fallback rather than dropping the decoded texture.
            let position = road_stop_build_sprite_center(
                ctx.iso_pos,
                ctx.tx_i32(),
                ctx.ty_i32(),
                base_z,
                layer_z,
                origin,
                width,
                height,
            );
            commands.spawn((
                MapVisualLayer,
                ctx.map_tile_chunk(),
                sprite,
                Transform::from_translation(position),
            ));
        }
        emitted = true;
    }
    emitted
}

#[allow(clippy::too_many_arguments)]
fn spawn_road_stop_buildings(
    commands: &mut Commands,
    assets: &WorldAssets,
    company: Option<&CompanyColoredSprites>,
    owner_colour: Option<CompanyColour>,
    map: &Map,
    stations: &[Station],
    ctx: &TileRenderContext,
    base_z: u8,
    class: StationTileClass,
    dir: usize,
    road_stop_catalog: &[RoadStopSpecDef],
    roadstop_action5: &[Option<openttdrs_core::DecodedSprite>],
    mut action5_sprites: Option<&mut crate::render::NewGrfAction5SpriteCache>,
    mut images: Option<&mut Assets<Image>>,
    climate: openttdrs_core::Climate,
    newgrf_stack: &[openttdrs_core::NewGrfEntry],
    world: Option<openttdrs_core::RoadStopWorldContext<'_>>,
) {
    if buildings_hidden() {
        return;
    }
    // Action3/2: vista NewGRF del spec persistido en la estación. La
    // randomización Action2 vive en la entidad `Station`, por lo que resolver
    // con un contexto vacío congelaba el primer sprite aun después de recibir
    // eventos de carga/llegada/salida.
    if let Some(st) = station_at_tile(map, stations, ctx.coord)
        && let Some(spec_id) = st.road_stop_spec_at(ctx.coord)
        && let Some(def) = road_stop_spec_def(road_stop_catalog, spec_id)
        && let (Some(cache), Some(images)) = (action5_sprites.as_mut(), images.as_mut())
    {
        let view_u8 = u8::try_from(dir.min(5)).unwrap_or(0);
        let mut a2 = world.map_or_else(
            || {
                openttdrs_core::action2_eval_ctx_for_road_stop_tile_with_catalog(
                    map,
                    stations,
                    road_stop_catalog,
                    ctx.coord,
                    view_u8,
                    climate,
                )
            },
            |world| {
                openttdrs_core::action2_eval_ctx_for_road_stop_tile_with_catalog_and_world(
                    map,
                    stations,
                    road_stop_catalog,
                    world,
                    ctx.coord,
                    view_u8,
                    climate,
                )
            },
        );
        a2.set_grf_params(openttdrs_core::stack_params_for_grfid(
            newgrf_stack,
            def.grfid,
        ));
        let layout = def.newgrf_tile_layout_runtime(usize::from(view_u8), &mut a2);
        let runtime_fp = def
            .newgrf_runtime
            .as_ref()
            .map_or(0, |_| runtime_fingerprint(&a2, vars::ROAD_STOP, false));
        if let Some(layout) = layout
            && spawn_newgrf_road_stop_layout_sequence(
                commands,
                ctx,
                base_z,
                map.dimensions().0,
                spec_id,
                runtime_fp,
                &layout,
                cache,
                images,
            )
        {
            return;
        }
        let view = def
            .newgrf_view_runtime(dir, &mut a2)
            .or_else(|| def.newgrf_view(dir).cloned());
        if let Some(view) = view {
            let slot = spec_id
                .saturating_mul(6)
                .saturating_add(u16::try_from(dir.min(5)).unwrap_or(0));
            let handle = cache.handle_for_variant(
                ROADSTOP_ACTION3_CACHE_TYPE,
                slot,
                runtime_fp,
                &view,
                images,
            );
            let pos3 = crate::iso::overlay_pos(
                ctx.iso_pos,
                f32::from(view.x_offs),
                f32::from(view.y_offs),
                f32::from(view.width),
                f32::from(view.height),
                base_z,
                0.05,
                ctx.tx_i32(),
                ctx.ty_i32(),
            );
            commands.spawn((
                MapVisualLayer,
                ctx.map_tile_chunk(),
                tint_building_sprite(Sprite {
                    image: handle,
                    color: Color::WHITE,
                    ..default()
                }),
                Transform::from_translation(pos3),
            ));
            return;
        }
    }
    let orientation = u8::try_from(dir).unwrap_or_default();
    let drive_through = road_stop_drive_through_layers(class, orientation);
    let world_z_delta = i32::from(base_z.saturating_sub(ctx.info.base_z)) * 8;
    if !drive_through.is_empty() {
        let handles = match class {
            StationTileClass::Bus => &assets.bus_stop_drive_through,
            StationTileClass::Truck => &assets.truck_stop_drive_through,
            _ => return,
        };
        let axis = usize::from(orientation - openttdrs_core::RSV_DRIVE_THROUGH_X);
        for spec in drive_through {
            record_road_stop_layer_trace(spec, owner_colour, false, world_z_delta);
        }
        for (layer_i, center) in road_stop_sorted_layer_centers(ctx, base_z, drive_through)
            .into_iter()
            .enumerate()
        {
            let spec = &drive_through[layer_i];
            let image = &handles[axis][layer_i];
            commands.spawn((
                MapVisualLayer,
                ctx.map_tile_chunk(),
                tint_building_sprite(sprite_from_atlas_or_company_white_colour(
                    company,
                    owner_colour,
                    image,
                    spec.path,
                )),
                Transform::from_translation(center),
            ));
        }
        return;
    }
    let handles = match class {
        StationTileClass::Bus => &assets.bus_stop_builds,
        StationTileClass::Truck => &assets.truck_stop_builds,
        _ => return,
    };
    let is_truck = class == StationTileClass::Truck;
    // OpenGFX / Action5 solo tienen bahía 0..3; DT 4/5 cae al eje.
    let build_dir = road_stop_ground_index(u8::try_from(dir).unwrap_or(0)).min(3);
    let build_layers = road_stop_build_layers(class, build_dir);
    for spec in build_layers {
        record_road_stop_layer_trace(spec, owner_colour, false, world_z_delta);
    }
    for (layer_i, center) in road_stop_sorted_layer_centers(ctx, base_z, build_layers)
        .into_iter()
        .enumerate()
    {
        let spec = &build_layers[layer_i];
        // Action5 `0x11`: sustituye la primera capa si hay sprite en el slot.
        if layer_i == 0
            && let Some(slot) = openttdrs_core::roadstop_action5_slot(is_truck, build_dir)
            && let (Some(cache), Some(images)) = (action5_sprites.as_mut(), images.as_mut())
            && let Some(sprite) = cache.sprite_colored(
                openttdrs_core::ACTION5_TYPE_ROADSTOPS,
                slot,
                roadstop_action5,
                Color::WHITE,
                images,
            )
        {
            commands.spawn((
                MapVisualLayer,
                ctx.map_tile_chunk(),
                sprite,
                Transform::from_translation(center),
            ));
            continue;
        }
        let image = &handles[build_dir][layer_i];
        commands.spawn((
            MapVisualLayer,
            ctx.map_tile_chunk(),
            tint_building_sprite(sprite_from_atlas_or_company_white_colour(
                company,
                owner_colour,
                image,
                spec.path,
            )),
            Transform::from_translation(center),
        ));
    }
}

/// Suelo de una bahía bus/camión. El `ground` vanilla lleva
/// `PALETTE_MODIFIER_COLOUR`, pero no participa de la transparencia de
/// edificios; oscurecerlo como una capa BUILD dejaba una loseta negra al
/// alternar la transparencia.
#[allow(clippy::too_many_arguments)]
fn spawn_road_stop_ground_sprite(
    commands: &mut Commands,
    image: &AtlasSprite,
    company: Option<&CompanyColoredSprites>,
    owner_colour: Option<CompanyColour>,
    ctx: &TileRenderContext,
    base_z: u8,
    original_tileh: u8,
    sprite_id: u32,
    asset_path: &str,
    map_width: u32,
    foundation_child_parent: Option<Entity>,
) {
    record_road_stop_ground_trace(
        original_tileh,
        sprite_id,
        station_company_palette(owner_colour),
        false,
    );
    let sprite =
        sprite_from_atlas_or_company_white_colour(company, owner_colour, image, asset_path);
    let position = full_tile_sprite_pos(ctx.tx_i32(), ctx.ty_i32(), base_z, 0.04);
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

/// Emite una superficie de waypoint vial y conserva el vínculo de
/// `AddChildSpriteScreen` cuando `DrawFoundation(Leveled)` creó un parent.
#[allow(clippy::too_many_arguments)]
fn spawn_waypoint_surface_sprite(
    commands: &mut Commands,
    ctx: &TileRenderContext,
    sprite: Sprite,
    position: Vec3,
    map_width: u32,
    foundation_child_parent: Option<Entity>,
) {
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

/// Dibuja las dos líneas BUILD del waypoint vial vanilla.
///
/// `station_land.h` no reutiliza las capas de una parada: los postes ocupan
/// sólo 3×16 o 16×3 unidades y se eligen por el eje de `m5`. Las cajas y los
/// offsets NFO se conservan en `road_waypoint_gfx_data_generated.rs`; al igual
/// que `AddChildSpriteScreen`, una fundación nivelada recibe ambas capas como
/// children para que sigan el parent cuando el terreno es inclinado.
#[allow(clippy::too_many_arguments)]
fn spawn_road_waypoint_buildings(
    commands: &mut Commands,
    assets: &WorldAssets,
    company: Option<&CompanyColoredSprites>,
    owner_colour: Option<CompanyColour>,
    ctx: &TileRenderContext,
    base_z: u8,
    axis: u8,
    map_width: u32,
    foundation_child_parent: Option<Entity>,
) {
    if buildings_hidden() {
        return;
    }
    let layers = road_waypoint_build_layers(axis);
    let world_z_delta = i32::from(base_z.saturating_sub(ctx.info.base_z)) * 8;
    for layer in layers {
        record_road_layer_trace(
            "station-road-waypoint-layer",
            layer,
            owner_colour,
            false,
            world_z_delta,
        );
    }
    let centers = road_stop_sorted_layer_centers(ctx, base_z, layers);
    for (layer, center) in layers.iter().zip(centers) {
        let Some(asset_index) = road_waypoint_sprite_index(layer.sprite_id) else {
            continue;
        };
        let Some(image) = assets.road_waypoint.get(asset_index) else {
            continue;
        };
        let sprite = tint_building_sprite(sprite_from_atlas_or_company_white_colour(
            company,
            owner_colour,
            image,
            layer.path,
        ));
        if let Some(parent) = foundation_child_parent {
            spawn_foundation_child_sprite_at(commands, sprite, ctx, center, map_width, parent);
        } else {
            commands.spawn((
                MapVisualLayer,
                ctx.map_tile_chunk(),
                sprite,
                Transform::from_translation(center),
            ));
        }
    }
}

#[allow(clippy::too_many_arguments)]
#[allow(dead_code)]
pub(crate) fn spawn_transport_object_tile(
    commands: &mut Commands,
    assets: &WorldAssets,
    company: Option<&CompanyColoredSprites>,
    owner_colour: Option<CompanyColour>,
    ctx: &TileRenderContext,
    slope_half_ground: f32,
    show_pbs_reservations: bool,
    map: &Map,
    dims: (u32, u32),
    stations: &[Station],
    catenary_newgrf: &[Option<openttdrs_core::DecodedSprite>],
    catenary_sprites: Option<&mut crate::render::NewGrfCatenarySpriteCache>,
    bridge_decks_newgrf: &[Option<openttdrs_core::DecodedSprite>],
    foundation_newgrf: &[Option<openttdrs_core::DecodedSprite>],
    action5_sprites: Option<&mut crate::render::NewGrfAction5SpriteCache>,
    images: Option<&mut Assets<Image>>,
) {
    spawn_transport_object_tile_with_road_types(
        commands,
        assets,
        company,
        owner_colour,
        ctx,
        slope_half_ground,
        show_pbs_reservations,
        map,
        dims,
        stations,
        &[],
        &[],
        &[],
        &[],
        &[],
        &[],
        catenary_newgrf,
        catenary_sprites,
        None,
        bridge_decks_newgrf,
        foundation_newgrf,
        Climate::Temperate,
        0,
        &[],
        None,
        &[],
        action5_sprites,
        images,
        &[],
    );
}

/// Emite el sprite `AirportTile` de un aeropuerto `NewGRF` cuando el layout
/// de construcción conservó un gfx global por tesela. El mapa sigue llevando
/// el `subst` vanilla en `m5` para FTA/compatibilidad, pero la imagen visible
/// debe venir del `Action1/3` del tile custom.
#[allow(clippy::too_many_arguments)]
fn spawn_newgrf_airport_tile(
    commands: &mut Commands,
    ctx: &TileRenderContext,
    base_z: u8,
    gfx: u16,
    map: &Map,
    map_width: u32,
    stations: &[Station],
    towns: &[openttdrs_core::Town],
    catalog: &[openttdrs_core::AirportTileSpecDef],
    climate: Climate,
    newgrf_stack: &[openttdrs_core::NewGrfEntry],
    cache: Option<&mut crate::render::NewGrfAction5SpriteCache>,
    images: Option<&mut Assets<Image>>,
    child_parent: Option<Entity>,
) -> bool {
    let Some(def) = catalog
        .iter()
        .find(|candidate| candidate.gfx.as_u16() == gfx && candidate.has_newgrf_sprites())
    else {
        return false;
    };
    let frame = usize::from(ctx.tile.map_or(0, |tile| tile.m7));
    let mut action2 = if def.newgrf_runtime.is_some() {
        let mut action2 = openttdrs_core::action2_eval_ctx_for_airport_tile_with_towns(
            map, stations, towns, ctx.coord, catalog, def, climate,
        );
        action2.set_grf_params(openttdrs_core::stack_params_for_grfid(
            newgrf_stack,
            def.newgrf_grfid,
        ));
        Some(action2)
    } else {
        None
    };
    let runtime_fp = action2.as_ref().map_or(0, |action2| {
        runtime_fingerprint(action2, vars::AIRPORT_TILE, false)
    });
    // Clone the small decoded descriptor before borrowing the image cache so
    // the catalog remains immutable while the texture is materialized.
    let Some(view) = action2
        .as_mut()
        .and_then(|action2| def.newgrf_view_runtime(frame, action2))
        .or_else(|| def.newgrf_view(frame).cloned())
    else {
        return false;
    };
    let (Some(cache), Some(images)) = (cache, images) else {
        return false;
    };
    let image = cache.handle_for_variant(0x11, gfx, runtime_fp, &view, images);
    WorldDrawTrace::record_sprite_with_palette_and_world_geometry(
        "airport-newgrf-tile",
        "sortable",
        u32::from(gfx),
        0,
        false,
        (0, 0),
        0,
        (i32::from(view.x_offs), i32::from(view.y_offs), 0),
        None,
    );
    let position = overlay_pos(
        ctx.iso_pos,
        f32::from(view.x_offs),
        f32::from(view.y_offs),
        f32::from(view.width),
        f32::from(view.height),
        base_z,
        0.04,
        ctx.tx_i32(),
        ctx.ty_i32(),
    );
    let sprite = tint_building_sprite(Sprite {
        image,
        color: Color::WHITE,
        ..default()
    });
    if let Some(parent) = child_parent {
        spawn_foundation_child_sprite_at(commands, sprite, ctx, position, map_width, parent);
    } else {
        commands.spawn((
            MapVisualLayer,
            ctx.map_tile_chunk(),
            sprite,
            Transform::from_translation(position),
        ));
    }
    true
}

/// Decide si un `AirportTile` custom conserva la fundación vanilla sobre una
/// pendiente. `CBID_AIRPTILE_DRAW_FOUNDATIONS` devuelve booleano; un callback
/// ausente/fallido conserva la conducta por defecto de `DrawNewAirportTile`.
#[allow(clippy::too_many_arguments)]
fn airport_tile_draws_default_foundation(
    def: &openttdrs_core::AirportTileSpecDef,
    map: &Map,
    stations: &[Station],
    towns: &[openttdrs_core::Town],
    coord: TileCoord,
    catalog: &[openttdrs_core::AirportTileSpecDef],
    climate: Climate,
    newgrf_stack: &[openttdrs_core::NewGrfEntry],
) -> bool {
    if !def.has_draw_foundations_callback() {
        return true;
    }
    let Some(runtime) = def.newgrf_runtime.as_ref() else {
        return true;
    };
    let mut ctx = openttdrs_core::action2_eval_ctx_for_airport_tile_with_towns(
        map, stations, towns, coord, catalog, def, climate,
    );
    ctx.set_grf_params(openttdrs_core::stack_params_for_grfid(
        newgrf_stack,
        def.newgrf_grfid,
    ));
    let result = runtime.resolve_callback_ctx(
        def.newgrf_local_id,
        openttdrs_core::CBID_AIRPTILE_DRAW_FOUNDATIONS,
        0,
        0,
        &mut ctx,
    );
    openttdrs_core::callback_draws_default_foundation(result)
}

/// Variante de [`spawn_transport_object_tile`] que conserva el estado de
/// roadtypes/NewGRF necesario para que los puentes dibujados desde el camino
/// de objetos resuelvan `ROTSG_BRIDGE` y `ROTSG_OVERLAY` igual que el mundo.
#[allow(clippy::too_many_arguments)]
pub(crate) fn spawn_transport_object_tile_with_road_types(
    commands: &mut Commands,
    assets: &WorldAssets,
    company: Option<&CompanyColoredSprites>,
    owner_colour: Option<CompanyColour>,
    ctx: &TileRenderContext,
    slope_half_ground: f32,
    show_pbs_reservations: bool,
    map: &Map,
    dims: (u32, u32),
    stations: &[Station],
    towns: &[openttdrs_core::Town],
    airport_tile_catalog: &[openttdrs_core::AirportTileSpecDef],
    rail_type_depot_newgrf: &[Option<openttdrs_core::RailSignalSpriteSpec>],
    rail_type_underlay_newgrf: &[Option<openttdrs_core::RailSignalSpriteSpec>],
    rail_type_tunnel_newgrf: &[Option<openttdrs_core::RailSignalSpriteSpec>],
    rail_type_tunnel_portal_newgrf: &[Option<openttdrs_core::RailSignalSpriteSpec>],
    catenary_newgrf: &[Option<openttdrs_core::DecodedSprite>],
    mut catenary_sprites: Option<&mut crate::render::NewGrfCatenarySpriteCache>,
    mut signal_sprites: Option<&mut crate::render::NewGrfSignalSpriteCache>,
    bridge_decks_newgrf: &[Option<openttdrs_core::DecodedSprite>],
    foundation_newgrf: &[Option<openttdrs_core::DecodedSprite>],
    climate: Climate,
    calendar_date: u32,
    road_catalog: &[openttdrs_core::RoadTypeDef],
    road_sprites: Option<&mut crate::render::NewGrfRoadSpriteCache>,
    newgrf_stack: &[openttdrs_core::NewGrfEntry],
    mut action5_sprites: Option<&mut crate::render::NewGrfAction5SpriteCache>,
    mut images: Option<&mut Assets<Image>>,
    road_stop_catalog: &[RoadStopSpecDef],
) {
    let tileh = ctx.info.tileh;
    let base_z = ctx.info.base_z;
    if ctx.kind == TileKind::ShipDepot {
        // `DrawShipDepotSprite` siempre parte de `SPR_WATER_TILE`: aunque el
        // depósito tenga un TileKind propio, en el save sigue siendo MP_WATER.
        WorldDrawTrace::record_sprite("ship-depot-water", "ground", 4061, false);
        commands.spawn((
            MapVisualLayer,
            ctx.map_tile_chunk(),
            WaterTile::ANIMATED,
            assets.water.sprite(),
            Transform::from_translation(full_tile_sprite_pos(
                ctx.tx_i32(),
                ctx.ty_i32(),
                base_z,
                FLAT_WATER_LAYER_FRAC,
            )),
        ));
    } else if !matches!(
        ctx.kind,
        TileKind::RoadTunnel
            | TileKind::RailTunnel
            | TileKind::RoadBridge
            | TileKind::RailBridge
            | TileKind::RoadDepot
            | TileKind::RailDepot
    ) {
        let ground = sloped_or_flat_image(tileh, &assets.grass, &assets.grass_slopes);
        spawn_ground_sprite(commands, &ground, Color::WHITE, ctx, slope_half_ground);
    }

    match ctx.kind {
        TileKind::RoadTunnel | TileKind::RailTunnel => {
            if !is_tunnel_entrance_slope(tileh) {
                return;
            }
            let rail = ctx.kind == TileKind::RailTunnel;
            // `DrawTile_TunnelBridge` siempre usa `GetTunnelBridgeDirection`,
            // que lee los dos bits bajos de `m5`. La pendiente sólo decide si
            // la boca es construible; no debe reemplazar la dirección
            // persistida (un save importado puede conservar una pendiente
            // efectiva distinta tras una fundación/terraformación).
            let dir = ctx.tile.map_or_else(
                || inclined_slope_direction(tileh).unwrap_or(0),
                |tile| tile.m5 & 0x03,
            );
            let rail_type = ctx
                .tile
                .map_or(openttdrs_core::RailType::Rail, rail_type_from_tile);
            // `DrawTile_TunnelBridge` consulta ambos grupos sólo cuando el
            // railtype activa `UsesOverlay()`, pero la superficie `RTSG_TUNNEL`
            // es independiente de que exista una fachada `RTSG_TUNNEL_PORTAL`.
            // Resolver cada vista por separado conserva el fallback vanilla
            // únicamente para la capa ausente.
            let (custom_tunnel_portal, custom_tunnel_surface) = if rail {
                let rail_index = usize::from(rail_type.as_u8());
                let uses_overlay = rail_type_underlay_newgrf
                    .get(rail_index)
                    .is_some_and(Option::is_some);
                if uses_overlay {
                    let portal_resolves = ctx.tile.and_then(|tile| {
                        rail_type_tunnel_portal_newgrf
                            .get(rail_index)
                            .and_then(Option::as_ref)
                            .and_then(|spec| {
                                resolve_custom_rail_group_sprite(
                                    map,
                                    tile,
                                    ctx,
                                    climate,
                                    calendar_date,
                                    newgrf_stack,
                                    spec,
                                    dir & 3,
                                    &mut signal_sprites,
                                    &mut images,
                                )
                            })
                    });
                    let tunnel_resolves = ctx.tile.and_then(|tile| {
                        rail_type_tunnel_newgrf
                            .get(rail_index)
                            .and_then(Option::as_ref)
                            .and_then(|spec| {
                                resolve_custom_rail_group_sprite(
                                    map,
                                    tile,
                                    ctx,
                                    climate,
                                    calendar_date,
                                    newgrf_stack,
                                    spec,
                                    dir & 3,
                                    &mut signal_sprites,
                                    &mut images,
                                )
                            })
                    });
                    (portal_resolves, tunnel_resolves)
                } else {
                    (None, None)
                }
            } else {
                (None, None)
            };
            let has_custom_tunnel_portal = custom_tunnel_portal.is_some();
            let snow_or_desert = rail && ctx.tile.is_some_and(|tile| (tile.m7 & 0x20) != 0);
            let climate_index = usize::from(climate.newgrf_landscape_id());
            let custom_base_rear = if rail && has_custom_tunnel_portal {
                assets.rail_tunnel_base_sprite(climate, dir, snow_or_desert, false)
            } else {
                None
            };
            let custom_base_front = if rail && has_custom_tunnel_portal {
                assets.rail_tunnel_base_sprite(climate, dir, snow_or_desert, true)
            } else {
                None
            };
            let vanilla_rear_image = if rail {
                assets.rail_tunnel_portal_sprite(rail_type, dir)
            } else {
                assets.tunnel_portal_sprite(false, dir)
            };
            let vanilla_rear_sprite_id = if rail {
                crate::sprites::rail_tunnel_rear_sprite_id(rail_type, dir)
            } else {
                crate::sprites::tunnel_rear_sprite_id(false, dir)
            };
            let rear_image = custom_base_rear.unwrap_or(vanilla_rear_image);
            let rear_sprite_id = custom_base_rear
                .and_then(|_| {
                    crate::sprites::rail_tunnel_base_sprite_id(
                        climate_index,
                        crate::sprites::rail_tunnel_base_slot(dir, snow_or_desert, false),
                    )
                })
                .unwrap_or(vanilla_rear_sprite_id);
            let vanilla_front_image = if rail {
                assets.rail_tunnel_portal_front_sprite(rail_type, dir)
            } else {
                assets.tunnel_portal_front_sprite(false, dir)
            };
            let vanilla_front_sprite_id = if rail {
                crate::sprites::rail_tunnel_front_sprite_id(rail_type, dir)
            } else {
                crate::sprites::tunnel_front_sprite_id(false, dir)
            };
            let front_image = custom_base_front.unwrap_or(vanilla_front_image);
            let front_sprite_id = custom_base_front
                .and_then(|_| {
                    crate::sprites::rail_tunnel_base_sprite_id(
                        climate_index,
                        crate::sprites::rail_tunnel_base_slot(dir, snow_or_desert, true),
                    )
                })
                .unwrap_or(vanilla_front_sprite_id);
            // OpenTTD dibuja el rear como suelo y el front como techo sortable.
            // Aunque el rear sea `DrawGroundSprite`, no es un rombo 64×31:
            // los portales mono/maglev tienen `xrel/yrel` propios. Centrar el
            // PNG como terreno desplazaba la boca hasta 20 px y dejaba la vía
            // aparentemente desconectada. Ambas capas usan su anclaje NFO.
            WorldDrawTrace::record_sprite(
                if custom_base_rear.is_some() {
                    "tunnel-railtype-base"
                } else {
                    "tunnel-rear"
                },
                "ground",
                rear_sprite_id,
                rail && has_custom_tunnel_portal && custom_base_rear.is_none(),
            );
            let rear_translation = if custom_base_rear.is_some() {
                crate::sprites::rail_tunnel_base_translation(
                    ctx.tx_i32(),
                    ctx.ty_i32(),
                    base_z,
                    climate_index,
                    dir,
                    snow_or_desert,
                    false,
                    0.0,
                )
            } else {
                crate::sprites::tunnel_portal_translation(
                    ctx.tx_i32(),
                    ctx.ty_i32(),
                    base_z,
                    vanilla_rear_sprite_id,
                    0.0,
                )
            };
            commands.spawn((
                MapVisualLayer,
                ctx.map_tile_chunk(),
                rear_image.sprite(),
                Transform::from_translation(rear_translation),
            ));
            if let Some(resolved) = custom_tunnel_surface {
                spawn_custom_rail_tunnel_surface(commands, ctx, resolved, base_z, 0.012, dir & 3);
            }
            // Igual que `DrawTunnelBridgeTile`: el bit de reserva de las
            // rampas/túneles ferroviarios está en m5 bit 4. La capa es un
            // SINGLE tipado, separada del portal, y debe ir entre el suelo y
            // el techo para no ocultar la boca del túnel.
            if rail
                && show_pbs_reservations
                && ctx
                    .tile
                    .is_some_and(openttdrs_core::tunnel_bridge_rail_reserved)
            {
                let sid = remap_rail_sprite_id(1005 + u32::from(dir & 1), rail_type);
                WorldDrawTrace::record_sprite_with_palette_and_geometry(
                    "tunnel-pbs-reservation",
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
                        0.025,
                        slope_half_ground,
                    );
                    let offset = rail_ghost_overlay_offset(sid);
                    commands.spawn((
                        MapVisualLayer,
                        ctx.map_tile_chunk(),
                        img.sprite(),
                        Transform::from_translation(base + Vec3::new(offset.x, offset.y, 0.0)),
                    ));
                }
            }
            // `DrawTile_TunnelBridge` llama a `DrawRailCatenary` antes del
            // cable especial que se combina con el techo. Esa pasada común
            // sólo aporta los postes de la boca; pedir `draw_wires=false`
            // evita dibujar encima el cable recto que OpenTTD reemplaza por
            // `_rail_catenary_sprite_data_tunnel`.
            if rail {
                let render_tb =
                    crate::sprites::rail_trackbits_for_render(map, ctx.coord, dims.0, dims.1);
                spawn_rail_catenary_for_surface(
                    commands,
                    map,
                    dims,
                    assets,
                    ctx,
                    rail_type,
                    render_tb,
                    tileh,
                    base_z,
                    false,
                    catenary_newgrf,
                    &mut catenary_sprites,
                    &mut images,
                );
            }
            let draw_tunnel_catenary = rail && !catenary_hidden() && rail_type.has_catenary();
            // El oráculo registra el cable antes del techo: es el padre del
            // `SpriteCombine` que contiene ambos. La capa Bevy conserva su
            // orden visual posterior, pero la traza modela el draw proc real.
            let tunnel_catenary_sprite = if draw_tunnel_catenary {
                let sid = catenary_tunnel_wire_sprite(dir);
                let anchor = catenary_sprite_anchor(sid, catenary_newgrf);
                let sprite = catenary_sprite_colored(
                    assets,
                    sid,
                    catenary_sprite_color(),
                    catenary_newgrf,
                    catenary_sprites.as_deref_mut(),
                    images.as_deref_mut(),
                );
                let (offset, (ox, oy, oz, ex, ey, ez)) = tunnel_catenary_trace_geometry(dir);
                WorldDrawTrace::record_sprite_with_geometry(
                    "tunnel-catenary",
                    "sortable",
                    catenary_reference_sprite_id(sid),
                    sprite.is_none(),
                    offset,
                    0,
                    Some(crate::render::world_draw_trace::TraceSpriteBounds::new(
                        ox, oy, oz, ex, ey, ez,
                    )),
                );
                sprite.zip(anchor)
            } else {
                None
            };
            let (front_offset, (ox, oy, oz, ex, ey, ez)) =
                crate::sprites::tunnel_front_trace_geometry(dir);
            let front_bounds = TraceSpriteBounds::new(ox, oy, oz, ex, ey, ez);
            let custom_front_translation = custom_tunnel_portal.as_ref().map(|resolved| {
                custom_rail_tunnel_front_translation(ctx, resolved.center_offset, base_z, 0.081)
            });
            let tunnel_parents = (!draw_tunnel_catenary).then(|| {
                tunnel_sortable_parents(
                    ctx.tx_i32(),
                    ctx.ty_i32(),
                    base_z,
                    front_sprite_id,
                    front_bounds,
                    dir,
                )
            });
            // `world-draw` conserva la inserción previa al sorter: el
            // portal se registra antes de los separadores, aunque las
            // profundidades de runtime se reasignen después.
            WorldDrawTrace::record_sprite_with_geometry(
                if custom_base_front.is_some() {
                    "tunnel-front-railtype-base"
                } else {
                    "tunnel-front"
                },
                if draw_tunnel_catenary {
                    "combined"
                } else {
                    "sortable"
                },
                front_sprite_id,
                rail && has_custom_tunnel_portal && custom_base_front.is_none(),
                front_offset,
                0,
                Some(front_bounds),
            );
            let mut front_translation = if custom_base_front.is_some() {
                crate::sprites::rail_tunnel_base_translation(
                    ctx.tx_i32(),
                    ctx.ty_i32(),
                    base_z,
                    climate_index,
                    dir,
                    snow_or_desert,
                    true,
                    0.08,
                )
            } else {
                crate::sprites::tunnel_front_translation(
                    ctx.tx_i32(),
                    ctx.ty_i32(),
                    base_z,
                    vanilla_front_sprite_id,
                    0.08,
                )
            };
            let front_sortable_parent = tunnel_parents.as_ref().map(|parents| {
                let source_depth = viewport_source_depth(
                    sortable_draw_z(ctx.tx_i32(), ctx.ty_i32(), base_z, 0.08),
                    ctx.tx,
                    dims.0,
                );
                front_translation.z = source_depth;
                ViewportSortableParent {
                    sprite_id: front_sprite_id,
                    bounds: parents[0].bounds,
                    insertion_key: viewport_insertion_key(ctx.tx, ctx.ty, 1),
                    source_depth,
                }
            });
            let mut front_entity = commands.spawn((
                MapVisualLayer,
                ctx.map_tile_chunk(),
                front_image.sprite(),
                Transform::from_translation(front_translation),
            ));
            if let Some(parent) = front_sortable_parent {
                front_entity.insert(parent);
            }
            let front_parent_entity = front_entity.id();
            if let Some(resolved) = custom_tunnel_portal {
                // OpenTTD combina la base Action5 y el overlay de portal en
                // una misma pasada sortable. Un child conserva ese vínculo
                // cuando no hay catenaria; con catenaria se usa un micro-slot
                // inmediatamente posterior al techo base.
                WorldDrawTrace::record_sprite_with_geometry(
                    "tunnel-front-newgrf",
                    if draw_tunnel_catenary {
                        "combined"
                    } else {
                        "sortable"
                    },
                    vanilla_front_sprite_id,
                    false,
                    front_offset,
                    0,
                    Some(front_bounds),
                );
                let overlay_translation = custom_front_translation.unwrap_or_else(|| {
                    custom_rail_tunnel_front_translation(ctx, resolved.center_offset, base_z, 0.081)
                });
                let mut overlay_entity = commands.spawn((
                    MapVisualLayer,
                    ctx.map_tile_chunk(),
                    resolved.sprite,
                    Transform::from_translation(overlay_translation),
                ));
                if !draw_tunnel_catenary {
                    overlay_entity.insert(crate::render::ViewportSortableChild {
                        parent: front_parent_entity,
                        source_depth: sortable_draw_z(ctx.tx_i32(), ctx.ty_i32(), base_z, 0.081),
                    });
                }
            }
            // Wire de portal (`DrawRailCatenaryOnTunnel`) si la vía es eléctrica.
            if let Some((sprite, anchor)) = tunnel_catenary_sprite {
                let (offset, (ox, oy, oz, ..)) = tunnel_catenary_trace_geometry(dir);
                commands.spawn((
                    MapVisualLayer,
                    ctx.map_tile_chunk(),
                    sprite,
                    Transform::from_translation(catenary_sprite_center(
                        ctx.tx_i32(),
                        ctx.ty_i32(),
                        base_z,
                        0.085,
                        (offset.0 + ox) as f32,
                        (offset.1 + oy) as f32,
                        (offset.2 + oz) as f32,
                        anchor,
                    )),
                ));
            }
            // Después del techo (y de su bloque combinado de catenaria),
            // OpenTTD agrega dos cajas sin imagen que separan la boca de los
            // sprites de las teselas vecinas. Conservamos tanto la traza como
            // el parent de runtime: un `Sprite` transparente no sería
            // equivalente, porque el sorter distingue explícitamente este
            // parent sin rasterizarlo.
            for (ordinal, separator_is_front) in [(2_u8, false), (3_u8, true)] {
                let bounds = tunnel_sort_separator_bounds(dir, separator_is_front);
                spawn_empty_bounding_box(
                    commands,
                    ctx,
                    "tunnel-sort-separator",
                    bounds,
                    0,
                    ordinal,
                    dims.0,
                    base_z,
                    0.09 + f32::from(ordinal) * 0.0001,
                );
            }
        }
        TileKind::RoadDepot => {
            // `DrawTile_Road` nivela los depósitos viales antes del suelo
            // 2634 y de sus capas BUILD. Dejar el césped inclinado debajo
            // desplazaba la losa y hacía que la boca pareciera desconectada.
            let depot_foundation = spawn_forced_leveled_foundation_with_child_parent(
                commands,
                map,
                dims,
                assets,
                ctx,
                tileh,
                "road-depot",
                "road-depot-foundation",
                foundation_newgrf,
                action5_sprites.as_deref_mut(),
                images.as_deref_mut(),
            );
            let depot_base_z = depot_foundation.surface_base_z;
            spawn_road_depot_tile(
                commands,
                assets,
                company,
                owner_colour,
                ctx,
                depot_base_z,
                TILE_HALF_H,
                tileh,
                dims.0,
                depot_foundation.child_parent,
            );
        }
        TileKind::RailDepot => {
            // `DrawTile_Rail` nivela cualquier depósito inclinado antes de
            // dibujar su suelo. No reutilizar el césped inclinado genérico:
            // las capas de suelo pasan a ser children de la fundación.
            let depot_foundation = spawn_forced_leveled_foundation_with_child_parent(
                commands,
                map,
                dims,
                assets,
                ctx,
                tileh,
                "rail-depot",
                "rail-depot-foundation",
                foundation_newgrf,
                action5_sprites.as_deref_mut(),
                images.as_deref_mut(),
            );
            let depot_base_z = depot_foundation.surface_base_z;
            spawn_rail_depot_tile(
                commands,
                assets,
                map,
                company,
                owner_colour,
                ctx,
                depot_base_z,
                TILE_HALF_H,
                tileh,
                dims.0,
                depot_foundation.child_parent,
                show_pbs_reservations,
                climate,
                calendar_date,
                newgrf_stack,
                rail_type_depot_newgrf,
                catenary_newgrf,
                &mut catenary_sprites,
                &mut signal_sprites,
                &mut images,
            );
        }
        TileKind::ShipDepot => {
            spawn_ship_depot_tile(commands, assets, company, owner_colour, ctx, base_z, dims.0);
        }
        TileKind::Airport => {
            let half_h = if tileh == 0 {
                TILE_HALF_H
            } else {
                slope_half_h(tileh)
            };
            let m5 = ctx.tile.map(|t| t.m5).unwrap_or(0);
            // A newly built NewGRF airport stores the vanilla `subst` in
            // `m5`, so use the per-tile global gfx retained on its Station
            // before falling back to the vanilla AirportPiece renderer.
            if let Some(gfx) = stations.iter().find_map(|station| {
                station
                    .airport_tile_gfx
                    .iter()
                    .find(|(coord, _)| *coord == ctx.coord)
                    .map(|(_, gfx)| *gfx)
            }) && let Some(def) = airport_tile_catalog
                .iter()
                .find(|candidate| candidate.gfx.as_u16() == gfx && candidate.has_newgrf_sprites())
            {
                let draws_foundation = tileh != 0
                    && airport_tile_draws_default_foundation(
                        def,
                        map,
                        stations,
                        towns,
                        ctx.coord,
                        airport_tile_catalog,
                        climate,
                        newgrf_stack,
                    );
                let (custom_base_z, child_parent) = if draws_foundation {
                    let foundation = spawn_forced_leveled_foundation_with_child_parent(
                        commands,
                        map,
                        dims,
                        assets,
                        ctx,
                        tileh,
                        "airport",
                        "airport-foundation",
                        foundation_newgrf,
                        action5_sprites.as_deref_mut(),
                        images.as_deref_mut(),
                    );
                    (foundation.surface_base_z, foundation.child_parent)
                } else {
                    (base_z, None)
                };
                if spawn_newgrf_airport_tile(
                    commands,
                    ctx,
                    custom_base_z,
                    gfx,
                    map,
                    dims.0,
                    stations,
                    towns,
                    airport_tile_catalog,
                    climate,
                    newgrf_stack,
                    action5_sprites.as_deref_mut(),
                    images.as_deref_mut(),
                    child_parent,
                ) {
                    return;
                }
            }
            let imported_station_gfx = ctx.tile.is_some_and(|tile| {
                let station_id = u32::from(tile.m2) | (u32::from(tile.m2_hi) << 8);
                stations.iter().any(|station| {
                    station.ottd_station_id == Some(station_id)
                        && station.airport_tiles.contains(&ctx.coord)
                })
            });
            let piece = if imported_station_gfx {
                openttdrs_core::AirportPiece::from_station_gfx(m5)
            } else {
                openttdrs_core::AirportPiece::from_m5(m5)
            };
            let spawned_ground = imported_station_gfx
                && spawn_airport_station_ground_layers(
                    commands,
                    assets,
                    company,
                    owner_colour,
                    ctx,
                    base_z,
                    half_h,
                    m5,
                );
            if !spawned_ground {
                let sprite = if imported_station_gfx {
                    assets.airport_station_gfx_sprite(m5).sprite()
                } else {
                    assets.airport_piece_sprite(piece).sprite()
                };
                commands.spawn((
                    MapVisualLayer,
                    ctx.map_tile_chunk(),
                    sprite,
                    Transform::from_translation(tile_pos_half(
                        ctx.tx_i32(),
                        ctx.ty_i32(),
                        base_z,
                        0.04,
                        half_h,
                    )),
                ));
            }
            if imported_station_gfx {
                spawn_airport_station_overlays(
                    commands,
                    assets,
                    company,
                    owner_colour,
                    ctx,
                    base_z,
                    m5,
                );
            }
            if !imported_station_gfx && piece == openttdrs_core::AirportPiece::Tower {
                spawn_airport_radar_overlay(commands, assets, ctx, base_z, half_h);
            }
        }
        TileKind::RoadBridge | TileKind::RailBridge => {
            if let Some(span) = bridge_span_at(map, ctx.coord, dims) {
                spawn_bridge_deck_with_road_types(
                    commands,
                    map,
                    dims,
                    assets,
                    ctx,
                    &span,
                    false,
                    show_pbs_reservations,
                    catenary_newgrf,
                    catenary_sprites,
                    bridge_decks_newgrf,
                    foundation_newgrf,
                    climate,
                    road_catalog,
                    road_sprites,
                    newgrf_stack,
                    action5_sprites,
                    images,
                    stations,
                    road_stop_catalog,
                );
            }
        }
        _ => {}
    }
}

fn spawn_ship_depot_tile(
    commands: &mut Commands,
    assets: &WorldAssets,
    company: Option<&CompanyColoredSprites>,
    owner_colour: Option<CompanyColour>,
    ctx: &TileRenderContext,
    base_z: u8,
    map_width: u32,
) {
    if buildings_hidden() {
        return;
    }

    // `water_land.h::DrawShipDepotSprite`: WaterTileType::Depot usa dos
    // teselas. m5 bit 0 = parte norte/sur, bit 1 = eje X/Y. Los offsets y
    // metadatos vienen de los sprites vanilla 4070..4075 (OpenGFX NFO).
    let m5 = ctx.tile.map_or(0, |tile| tile.m5);
    let part_south = m5 & 0x01 != 0;
    let axis_y = m5 & 0x02 != 0;
    // `TILE_SEQ_LINE` usa un borde de 16×1 para Axis::X y uno de 1×16
    // para Axis::Y. No es el tamaño del PNG: es la caja de ordenación
    // isométrica con la que OpenTTD compone ambas mitades del depósito.
    let (extent_x, extent_y) = if axis_y { (1, 16) } else { (16, 1) };
    let layers: &[(usize, f32, f32, f32, f32, f32, f32)] = match (axis_y, part_south) {
        // Eje X, norte: 4072 / ship_depot_nw.
        (false, false) => &[(2, 0.0, 15.0, -61.0, -30.0, 64.0, 47.0)],
        // Eje X, sur: 4074 (parte trasera) + 4070 (frente SE).
        (false, true) => &[
            (4, 0.0, 0.0, -31.0, 5.0, 13.0, 12.0),
            (0, 0.0, 15.0, -61.0, -31.0, 64.0, 48.0),
        ],
        // Eje Y, norte: 4073 / ship_depot_ne.
        (true, false) => &[(3, 15.0, 0.0, -1.0, -30.0, 64.0, 47.0)],
        // Eje Y, sur: 4075 (parte trasera) + 4071 (frente SW).
        (true, true) => &[
            (5, 0.0, 0.0, 20.0, 5.0, 13.0, 12.0),
            (1, 15.0, 0.0, -1.0, -31.0, 64.0, 48.0),
        ],
    };

    const SHIP_DEPOT_PATHS: [&str; 6] = [
        "assets/opengfx/tiles/ship_depot_se_front.png",
        "assets/opengfx/tiles/ship_depot_sw_front.png",
        "assets/opengfx/tiles/ship_depot_nw.png",
        "assets/opengfx/tiles/ship_depot_ne.png",
        "assets/opengfx/tiles/ship_depot_se_rear.png",
        "assets/opengfx/tiles/ship_depot_sw_rear.png",
    ];
    // `GetCompanyPalette(owner)`: `PALETTE_RECOLOUR_START + colour`. En Kale
    // el owner es DarkBlue y por eso el oráculo expone 775.
    let company_palette = 775 + u32::from(owner_colour.unwrap_or_default().as_u8());

    for (layer_i, &(sprite_i, dx, dy, xrel, yrel, width, height)) in layers.iter().enumerate() {
        let sprite_id = 4070 + sprite_i as u32;
        WorldDrawTrace::record_sprite_with_palette_and_geometry(
            "ship-depot",
            "sortable",
            sprite_id,
            company_palette,
            false,
            (0, 0, 0),
            0,
            Some(crate::render::world_draw_trace::TraceSpriteBounds::new(
                dx as i32, dy as i32, 0, extent_x, extent_y, 20,
            )),
        );
        let local = remap_tile_offset(dx, dy, 0.0) * 0.5;
        let mut pos = overlay_pos(
            ctx.iso_pos + local,
            xrel,
            yrel,
            width,
            height,
            base_z,
            0.04 + layer_i as f32 * 0.0005,
            ctx.tx_i32(),
            ctx.ty_i32(),
        );
        // Cada capa ya declara la misma `TILE_SEQ_LINE` que recibe
        // `AddSortableSpriteToDraw`. Antes estos sprites quedaban en la
        // profundidad local de la tesela y no podían cruzarse correctamente
        // con edificios, puentes ni otros depósitos. Reservamos su slot y
        // entregamos el prisma al sorter global igual que las casas vanilla.
        let source_depth = viewport_source_depth(pos.z, ctx.tx, map_width);
        pos.z = source_depth;
        commands.spawn((
            MapVisualLayer,
            ctx.map_tile_chunk(),
            tint_building_sprite(sprite_from_atlas_or_company_white_colour(
                company,
                owner_colour,
                &assets.ship_depot[sprite_i],
                SHIP_DEPOT_PATHS[sprite_i],
            )),
            Transform::from_translation(pos),
            ViewportSortableParent {
                sprite_id,
                bounds: ship_depot_parent_bounds(
                    ctx, base_z, dx as i32, dy as i32, extent_x, extent_y,
                ),
                insertion_key: viewport_insertion_key(
                    ctx.tx,
                    ctx.ty,
                    (layer_i as u8).saturating_add(1),
                ),
                source_depth,
            },
        ));
    }
}

/// Caja mundial de una capa `TILE_SEQ_LINE` del depósito naval.
///
/// Las seis piezas 4070..4075 no comparten tamaño de imagen, pero sí el
/// prisma lineal indicado por `water_land.h`: 16×1 o 1×16, siempre con altura
/// 20. Centralizar la conversión evita que la traza y el sorter runtime
/// diverjan por un máximo inclusivo o una elevación distinta.
fn ship_depot_parent_bounds(
    ctx: &TileRenderContext,
    base_z: u8,
    dx: i32,
    dy: i32,
    extent_x: i32,
    extent_y: i32,
) -> ParentSpriteBounds {
    tile_seq_parent_sprite(
        0,
        0,
        ctx.tx_i32(),
        ctx.ty_i32(),
        base_z,
        dx,
        dy,
        0,
        extent_x,
        extent_y,
        20,
    )
    .bounds
}

/// Padres BUILD del depósito vial. Aunque las cuatro variantes vanilla de
/// Kale conservan su inserción local, pasan por el mismo sorter que OpenTTD
/// para que los bounds —no el índice del PNG— sigan siendo el contrato.
fn road_depot_parent_sprites(
    tx: i32,
    ty: i32,
    base_z: u8,
    layers: &[RoadDepotLayerGfx],
) -> Vec<ParentSprite> {
    layers
        .iter()
        .enumerate()
        .map(|(index, layer)| {
            tile_seq_parent_sprite(
                index as u64,
                layer.sprite_id,
                tx,
                ty,
                base_z,
                layer.dx as i32,
                layer.dy as i32,
                layer.dz as i32,
                layer.sx,
                layer.sy,
                20,
            )
        })
        .collect()
}

fn road_depot_sorted_layer_centers(
    ctx: &TileRenderContext,
    base_z: u8,
    layers: &[RoadDepotLayerGfx],
) -> Vec<Vec3> {
    let mut centers: Vec<_> = layers
        .iter()
        .map(|layer| {
            road_depot_build_sprite_center(
                ctx.iso_pos,
                ctx.tx_i32(),
                ctx.ty_i32(),
                base_z,
                layer.z,
                road_depot_seq_gfx(layer),
                layer.w,
                layer.h,
            )
        })
        .collect();
    let parents = road_depot_parent_sprites(ctx.tx_i32(), ctx.ty_i32(), base_z, layers);
    let depths: Vec<_> = centers.iter().map(|center| center.z).collect();
    for (center, depth) in centers
        .iter_mut()
        .zip(depths_in_viewport_sort_order(&parents, &depths))
    {
        center.z = depth;
    }
    centers
}

#[allow(clippy::too_many_arguments)]
fn spawn_road_depot_tile(
    commands: &mut Commands,
    assets: &WorldAssets,
    company: Option<&CompanyColoredSprites>,
    owner_colour: Option<CompanyColour>,
    ctx: &TileRenderContext,
    base_z: u8,
    half_h: f32,
    tileh: u8,
    map_width: u32,
    foundation_child_parent: Option<Entity>,
) {
    let dir = ctx.tile.map_or(0, |t| t.m5 & 0x03).min(3) as usize;
    record_road_depot_ground_trace(tileh);
    let position = full_tile_sprite_pos_half(ctx.tx_i32(), ctx.ty_i32(), base_z, 0.02, half_h);
    if let Some(parent) = foundation_child_parent {
        spawn_foundation_child_sprite_at(
            commands,
            assets.road_depot_ground.sprite(),
            ctx,
            position,
            map_width,
            parent,
        );
    } else {
        commands.spawn((
            MapVisualLayer,
            ctx.map_tile_chunk(),
            assets.road_depot_ground.sprite(),
            Transform::from_translation(position),
        ));
    }
    // En OpenTTD, el depósito vial vanilla dibuja la losa `SPR_AIRPORT_APRON`
    // y las capas BUILD; no añade un `road_flat` normal. Ese overlay sólo
    // aparece para ciertos tipos custom/tranvías y no estaba resuelto aquí.
    // Dibujarlo siempre agregaba una vía que el oráculo no emite.
    let foundation_z_delta = (i32::from(base_z) - i32::from(ctx.info.base_z)) * 8;
    let build_layers = road_depot_build_layers(dir);
    let build_centers = road_depot_sorted_layer_centers(ctx, base_z, build_layers);
    for (layer_i, spec) in build_layers.iter().enumerate() {
        if buildings_hidden() {
            break;
        }
        let image = assets.road_depot_builds[dir].get(layer_i);
        WorldDrawTrace::record_sprite_with_palette_and_world_geometry(
            "road-depot-building",
            "sortable",
            spec.sprite_id,
            station_company_palette(owner_colour),
            image.is_none(),
            (0, 0),
            foundation_z_delta,
            (0, 0, 0),
            Some(TraceSpriteBounds::new(
                spec.dx as i32,
                spec.dy as i32,
                spec.dz as i32,
                spec.sx,
                spec.sy,
                20,
            )),
        );
        let Some(image) = image else {
            continue;
        };
        let center = build_centers[layer_i];
        commands.spawn((
            MapVisualLayer,
            ctx.map_tile_chunk(),
            tint_building_sprite(sprite_from_atlas_or_company_white_colour(
                company,
                owner_colour,
                image,
                spec.path,
            )),
            Transform::from_translation(center),
        ));
    }
}

/// `DrawFoundation(Leveled)` hace hijo el suelo del depósito mediante
/// `OffsetGroundSprite(0, -TILE_HEIGHT)`. El exportador del oráculo lo expresa
/// en píxeles de pantalla a zoom base.
const fn road_depot_foundation_child_offset(tileh: u8) -> Option<(i32, i32, i32)> {
    if tileh == 0 { None } else { Some((0, -32, 0)) }
}

fn record_road_depot_ground_trace(tileh: u8) {
    if let Some(offset) = road_depot_foundation_child_offset(tileh) {
        WorldDrawTrace::record_foundation_child_sprite(
            "road-depot-ground",
            ROAD_DEPOT_GROUND_SPRITE_ID,
            false,
            offset,
        );
    } else {
        WorldDrawTrace::record_sprite(
            "road-depot-ground",
            "ground",
            ROAD_DEPOT_GROUND_SPRITE_ID,
            false,
        );
    }
}

/// Depósito de vía según `_depot_gfx_table` (`track_land.h`): suelo de vía en
/// SE/SW (la salida mira a cámara) y capas BUILD por dirección (`m5 & 3`).
fn rail_depot_reservation_track_visible(dir: usize, buildings_are_hidden: bool) -> bool {
    buildings_are_hidden || matches!(dir, 1 | 2)
}

/// `DrawFoundation(Leveled)` desplaza los `DrawGroundSprite` posteriores 8
/// píxeles. El exportador los normaliza por `ZOOM_BASE`, de ahí -32.
const fn rail_depot_foundation_child_offset(tileh: u8) -> Option<(i32, i32, i32)> {
    if tileh == 0 { None } else { Some((0, -32, 0)) }
}

fn record_rail_depot_ground_trace(tileh: u8, role: &'static str, sprite_id: u32, fallback: bool) {
    if let Some(offset) = rail_depot_foundation_child_offset(tileh) {
        WorldDrawTrace::record_foundation_child_sprite(role, sprite_id, fallback, offset);
    } else {
        WorldDrawTrace::record_sprite(role, "ground", sprite_id, fallback);
    }
}

fn record_rail_depot_reservation_trace(tileh: u8, sprite_id: u32, fallback: bool) {
    if let Some(offset) = rail_depot_foundation_child_offset(tileh) {
        WorldDrawTrace::record_foundation_child_sprite_with_palette(
            "rail-depot-pbs-reservation",
            sprite_id,
            804,
            fallback,
            offset,
        );
    } else {
        WorldDrawTrace::record_sprite_with_palette_and_geometry(
            "rail-depot-pbs-reservation",
            "ground",
            sprite_id,
            804,
            fallback,
            (0, 0, 0),
            0,
            None,
        );
    }
}

fn rail_depot_catenary_visible(ctx: &TileRenderContext) -> bool {
    ctx.tile
        .is_some_and(|tile| rail_type_from_tile(tile).has_catenary())
        && !catenary_hidden()
}

fn rail_depot_catenary_parent_sprite(
    id: u64,
    tx: i32,
    ty: i32,
    base_z: u8,
    dir: usize,
) -> ParentSprite {
    let draw = catenary_depot_wire_draw(dir as u8);
    let (dx, dy, dz) = draw.bounds_origin;
    let (ex, ey, ez) = draw.bounds_extent;
    tile_seq_parent_sprite(
        id,
        catenary_reference_sprite_id(draw.sprite_id),
        tx,
        ty,
        base_z,
        dx,
        dy,
        dz,
        ex,
        ey,
        ez,
    )
}

fn rail_depot_build_parent_sprites(
    tx: i32,
    ty: i32,
    base_z: u8,
    first_id: u64,
    layers: &[RailDepotLayerGfx],
) -> Vec<ParentSprite> {
    layers
        .iter()
        .enumerate()
        .map(|(index, layer)| {
            tile_seq_parent_sprite(
                first_id + index as u64,
                layer.sprite_id,
                tx,
                ty,
                base_z,
                layer.dx as i32,
                layer.dy as i32,
                layer.dz as i32,
                layer.sx,
                layer.sy,
                23,
            )
        })
        .collect()
}

/// Reasigna sólo los slots locales de un depósito ferroviario según el orden
/// final de OpenTTD. El cable de entrada participa como un parent más: en
/// Kale `(195,17)` el sorter deja la puerta 1063 detrás/delante del cable
/// según sus prismas, algo que no se puede reproducir ordenando sólo las dos
/// fachadas BUILD.
fn rail_depot_sorted_layer_centers(
    ctx: &TileRenderContext,
    base_z: u8,
    half_h: f32,
    rail_type: openttdrs_core::RailType,
    dir: usize,
    include_catenary: bool,
) -> (Option<f32>, Vec<Vec3>) {
    let layers = rail_depot_build_layers(rail_type, dir);
    let mut centers: Vec<_> = layers
        .iter()
        .map(|layer| {
            road_depot_build_sprite_center(
                ctx.iso_pos,
                ctx.tx_i32(),
                ctx.ty_i32(),
                base_z,
                layer.z,
                rail_depot_seq_gfx(layer),
                layer.w,
                layer.h,
            )
        })
        .collect();

    let mut parents = Vec::with_capacity(layers.len() + usize::from(include_catenary));
    let mut source_depths = Vec::with_capacity(layers.len() + usize::from(include_catenary));
    if include_catenary {
        parents.push(rail_depot_catenary_parent_sprite(
            0,
            ctx.tx_i32(),
            ctx.ty_i32(),
            base_z,
            dir,
        ));
        source_depths.push(tile_pos_half(ctx.tx_i32(), ctx.ty_i32(), base_z, 0.035, half_h).z);
    }
    parents.extend(rail_depot_build_parent_sprites(
        ctx.tx_i32(),
        ctx.ty_i32(),
        base_z,
        parents.len() as u64,
        layers,
    ));
    source_depths.extend(centers.iter().map(|center| center.z));

    let mut sorted_depths = depths_in_viewport_sort_order(&parents, &source_depths).into_iter();
    let catenary_depth = include_catenary.then(|| sorted_depths.next()).flatten();
    debug_assert!(
        !include_catenary || catenary_depth.is_some(),
        "el parent de catenaria debe tener un slot de profundidad"
    );
    for (center, depth) in centers.iter_mut().zip(sorted_depths) {
        center.z = depth;
    }
    (catenary_depth, centers)
}

/// Índice de una fachada vanilla dentro del bloque relocatable de
/// `RTSG_DEPOT`. OpenTTD calcula el desplazamiento desde `SE_1` (1063), por lo
/// que el orden global es `SE_1`, `SE_2`, `SW_1`, `SW_2`, `NE`, `NW` aunque la
/// tabla de orientaciones se presente como NE/SE/SW/NW.
#[must_use]
const fn rail_depot_custom_sprite_index(dir: usize, layer: usize) -> Option<u8> {
    let dir = if dir > 3 { 3 } else { dir };
    match dir {
        0 if layer == 0 => Some(4),
        1 if layer < 2 => Some(layer as u8),
        2 if layer < 2 => Some(2 + layer as u8),
        3 if layer == 0 => Some(5),
        _ => None,
    }
}

/// Resuelve una capa del grupo Action3 `RailSpriteType::Depot` manteniendo el
/// mismo contexto de vía que `GetCustomRailSprite` (`0x40`–`0x45`, fecha,
/// random y parámetros del GRF).
#[allow(clippy::too_many_arguments)]
fn resolve_custom_rail_depot_sprite(
    map: &Map,
    ctx: &TileRenderContext,
    climate: Climate,
    calendar_date: u32,
    newgrf_stack: &[openttdrs_core::NewGrfEntry],
    spec: &openttdrs_core::RailSignalSpriteSpec,
    image: u8,
    signal_sprites: &mut Option<&mut crate::render::NewGrfSignalSpriteCache>,
    images: &mut Option<&mut Assets<Image>>,
) -> Option<crate::render::signal_newgrf::ResolvedSignalSprite> {
    let tile = ctx.tile?;
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

/// Cable de entrada del depósito eléctrico (`DrawRailCatenary` especial).
///
/// Esta rama va después del suelo/reserva pero antes de las capas BUILD del
/// depósito. Emitirla después de la fachada hacía que el cable apareciera
/// visualmente por delante y rompía el orden del oráculo (Kale 195,17).
#[allow(clippy::too_many_arguments)] // Comparte el contexto de recursos del draw proc del depósito.
fn spawn_rail_depot_catenary(
    commands: &mut Commands,
    assets: &WorldAssets,
    ctx: &TileRenderContext,
    base_z: u8,
    sorted_depth: Option<f32>,
    catenary_newgrf: &[Option<openttdrs_core::DecodedSprite>],
    catenary_sprites: &mut Option<&mut crate::render::NewGrfCatenarySpriteCache>,
    images: &mut Option<&mut Assets<Image>>,
) {
    let Some(tile) = ctx.tile else {
        return;
    };
    if !rail_depot_catenary_visible(ctx) {
        return;
    }

    let draw = catenary_depot_wire_draw(tile.m5 & 0x03);
    let anchor = catenary_sprite_anchor(draw.sprite_id, catenary_newgrf);
    let sprite = catenary_sprite_colored(
        assets,
        draw.sprite_id,
        catenary_sprite_color(),
        catenary_newgrf,
        catenary_sprites.as_deref_mut(),
        images.as_deref_mut(),
    );
    let (ox, oy, oz) = draw.bounds_origin;
    let (ex, ey, ez) = draw.bounds_extent;
    // `DrawRailCatenary` usa `GetTileMaxPixelZ()`: cuando el depósito recibe
    // una fundación nivelada el cable queda sobre la superficie elevada, no
    // sobre el mínimo crudo de la tesela. La transformación ya usa `base_z`;
    // conservar el mismo delta en la traza evita informar un falso desvío.
    WorldDrawTrace::record_sprite_with_palette_and_world_geometry(
        "rail-depot-catenary",
        "sortable",
        catenary_reference_sprite_id(draw.sprite_id),
        0,
        sprite.is_none(),
        (0, 0),
        (i32::from(base_z) - i32::from(ctx.info.base_z)) * 8,
        (0, 0, 0),
        Some(TraceSpriteBounds::new(ox, oy, oz, ex, ey, ez)),
    );
    let Some((sprite, anchor)) = sprite.zip(anchor) else {
        return;
    };
    let local_z = catenary_local_z_delta(
        (i32::from(base_z) - i32::from(ctx.info.base_z)) * 8 + oz,
        ctx.info.base_z,
        base_z,
    );
    let mut position = catenary_sprite_center(
        ctx.tx_i32(),
        ctx.ty_i32(),
        base_z,
        0.035,
        ox as f32,
        oy as f32,
        local_z as f32,
        anchor,
    );
    if let Some(depth) = sorted_depth {
        position.z = depth;
    }
    commands.spawn((
        MapVisualLayer,
        ctx.map_tile_chunk(),
        sprite,
        Transform::from_translation(position),
    ));
}

#[allow(clippy::too_many_arguments)] // Parámetros del spawner comparten el contexto del tile.
fn spawn_rail_depot_tile(
    commands: &mut Commands,
    assets: &WorldAssets,
    map: &Map,
    company: Option<&CompanyColoredSprites>,
    owner_colour: Option<CompanyColour>,
    ctx: &TileRenderContext,
    base_z: u8,
    half_h: f32,
    tileh: u8,
    map_width: u32,
    foundation_child_parent: Option<Entity>,
    show_pbs_reservations: bool,
    climate: Climate,
    calendar_date: u32,
    newgrf_stack: &[openttdrs_core::NewGrfEntry],
    rail_type_depot_newgrf: &[Option<openttdrs_core::RailSignalSpriteSpec>],
    catenary_newgrf: &[Option<openttdrs_core::DecodedSprite>],
    catenary_sprites: &mut Option<&mut crate::render::NewGrfCatenarySpriteCache>,
    signal_sprites: &mut Option<&mut crate::render::NewGrfSignalSpriteCache>,
    images: &mut Option<&mut Assets<Image>>,
) {
    let dir = ctx.tile.map_or(0, |t| t.m5 & 0x03).min(3) as usize;
    let rail_type = ctx
        .tile
        .map_or(openttdrs_core::RailType::Rail, rail_type_from_tile);
    if let Some(track_id) =
        crate::sprites::RAIL_DEPOT_GROUND_TRACK[dir].map(|id| remap_rail_sprite_id(id, rail_type))
    {
        let fallback = !assets.rail.contains_key(&track_id);
        record_rail_depot_ground_trace(tileh, "rail-depot-track", track_id, fallback);
        if let Some(image) = assets.rail.get(&track_id) {
            let position =
                full_tile_sprite_pos_half(ctx.tx_i32(), ctx.ty_i32(), base_z, 0.02, half_h);
            if let Some(parent) = foundation_child_parent {
                spawn_foundation_child_sprite_at(
                    commands,
                    image.sprite(),
                    ctx,
                    position,
                    map_width,
                    parent,
                );
            } else {
                commands.spawn((
                    MapVisualLayer,
                    ctx.map_tile_chunk(),
                    image.sprite(),
                    Transform::from_translation(position),
                ));
            }
        }
    } else {
        // NE/NW usan `SPR_FLAT_GRASS_TILE` en `_depot_gfx_table`; no el
        // relieve de la tesela original. En pendiente también es child del
        // mismo parent de fundación que la vía de las salidas SE/SW.
        record_rail_depot_ground_trace(tileh, "rail-depot-ground", 3981, false);
        let position = full_tile_sprite_pos_half(ctx.tx_i32(), ctx.ty_i32(), base_z, 0.02, half_h);
        if let Some(parent) = foundation_child_parent {
            spawn_foundation_child_sprite_at(
                commands,
                assets.grass.sprite(),
                ctx,
                position,
                map_width,
                parent,
            );
        } else {
            commands.spawn((
                MapVisualLayer,
                ctx.map_tile_chunk(),
                assets.grass.sprite(),
                Transform::from_translation(position),
            ));
        }
    }
    // `HasDepotReservation` vive en m5 bit 4. En el depot no se codifican
    // TrackBits: la dirección fija si el overlay es SINGLE_X o SINGLE_Y.
    // `DrawTile_Rail`: con el edificio visible, las puertas NE/NW cubren
    // la vía de acceso; OpenTTD sólo muestra su reserva si el edificio se
    // vuelve transparente. Las salidas SE/SW sí permanecen a la vista.
    let reservation_track_visible = rail_depot_reservation_track_visible(dir, buildings_hidden());
    if show_pbs_reservations
        && reservation_track_visible
        && ctx.tile.is_some_and(|tile| tile.m5 & 0x10 != 0)
    {
        let sid = remap_rail_sprite_id(1005 + u32::from(dir as u8 & 1), rail_type);
        record_rail_depot_reservation_trace(tileh, sid, !assets.has_exact_pbs_rail_sprite(sid));
        if let Some(image) = assets.pbs_rail_sprite(sid) {
            let base = full_tile_sprite_pos_half(ctx.tx_i32(), ctx.ty_i32(), base_z, 0.026, half_h);
            let offset = rail_pbs_reservation_offset(sid);
            let position = base + Vec3::new(offset.x, offset.y, 0.0);
            if let Some(parent) = foundation_child_parent {
                spawn_foundation_child_sprite_at(
                    commands,
                    image.sprite(),
                    ctx,
                    position,
                    map_width,
                    parent,
                );
            } else {
                commands.spawn((
                    MapVisualLayer,
                    ctx.map_tile_chunk(),
                    image.sprite(),
                    Transform::from_translation(position),
                ));
            }
        }
    }
    let buildings_are_hidden = buildings_hidden();
    let catenary_participates = rail_depot_catenary_visible(ctx) && !buildings_are_hidden;
    let (catenary_depth, build_centers) =
        rail_depot_sorted_layer_centers(ctx, base_z, half_h, rail_type, dir, catenary_participates);
    spawn_rail_depot_catenary(
        commands,
        assets,
        ctx,
        base_z,
        catenary_depth,
        catenary_newgrf,
        catenary_sprites,
        images,
    );
    let depot_variant = rail_depot_visual_type_index(rail_type);
    let depot_builds = &assets.rail_depot_builds[depot_variant][dir];
    let custom_depot_spec = rail_type_depot_newgrf
        .get(usize::from(rail_type.as_u8()))
        .and_then(Option::as_ref);
    // `DrawRailTileSeq`: cada fachada es un sortable con las bounds del
    // TILE_SEQ_LINE y recolor de la compañía propietaria. En una pendiente
    // la fundación altera la altura de mundo, no la caja local de la pieza.
    let company_palette = 775 + u32::from(owner_colour.unwrap_or_default().as_u8());
    let foundation_z_delta = (i32::from(base_z) - i32::from(ctx.info.base_z)) * 8;
    for (layer_i, spec) in rail_depot_build_layers(rail_type, dir).iter().enumerate() {
        if buildings_are_hidden {
            break;
        }
        let custom = custom_depot_spec.and_then(|custom_spec| {
            rail_depot_custom_sprite_index(dir, layer_i).and_then(|image| {
                resolve_custom_rail_depot_sprite(
                    map,
                    ctx,
                    climate,
                    calendar_date,
                    newgrf_stack,
                    custom_spec,
                    image,
                    &mut *signal_sprites,
                    &mut *images,
                )
            })
        });
        if let Some(resolved) = custom {
            // `DrawRailTileSeq` conserva la geometría de la línea (dx/dy/dz),
            // pero el ancla NFO pertenece al sprite GRF resuelto. Reutilizar
            // esos offsets evita centrar sprites HD como si fueran 64×31.
            let mut seq = rail_depot_seq_gfx(spec);
            seq.x_offs = resolved.center_offset.x - resolved.size.x * 0.5;
            seq.y_offs = -resolved.center_offset.y - resolved.size.y * 0.5;
            let mut center = road_depot_build_sprite_center(
                ctx.iso_pos,
                ctx.tx_i32(),
                ctx.ty_i32(),
                base_z,
                spec.z,
                seq,
                resolved.size.x,
                resolved.size.y,
            );
            // El orden de padres se calculó con los prismas TILE_SEQ vanilla;
            // conservar su profundidad mantiene la relación con la catenaria
            // y las otras fachadas aun cuando el GRF publique dimensiones HD.
            if let Some(vanilla_center) = build_centers.get(layer_i) {
                center.z = vanilla_center.z;
            }
            WorldDrawTrace::record_sprite_with_palette_and_world_geometry(
                "rail-depot-building-newgrf",
                "sortable",
                1063 + u32::from(rail_depot_custom_sprite_index(dir, layer_i).unwrap_or(0)),
                company_palette,
                false,
                (0, 0),
                foundation_z_delta,
                (0, 0, 0),
                Some(TraceSpriteBounds::new(
                    spec.dx as i32,
                    spec.dy as i32,
                    spec.dz as i32,
                    spec.sx,
                    spec.sy,
                    23,
                )),
            );
            let sprite = tint_building_sprite(resolved.sprite);
            if let Some(parent) = foundation_child_parent {
                spawn_foundation_child_sprite_at(commands, sprite, ctx, center, map_width, parent);
            } else {
                commands.spawn((
                    MapVisualLayer,
                    ctx.map_tile_chunk(),
                    sprite,
                    Transform::from_translation(center),
                ));
            }
            continue;
        }
        let Some(image) = depot_builds.get(layer_i) else {
            WorldDrawTrace::record_sprite_with_palette_and_world_geometry(
                "rail-depot-building",
                "sortable",
                spec.sprite_id,
                company_palette,
                true,
                (0, 0),
                foundation_z_delta,
                (0, 0, 0),
                Some(TraceSpriteBounds::new(
                    spec.dx as i32,
                    spec.dy as i32,
                    spec.dz as i32,
                    spec.sx,
                    spec.sy,
                    23,
                )),
            );
            continue;
        };
        WorldDrawTrace::record_sprite_with_palette_and_world_geometry(
            "rail-depot-building",
            "sortable",
            spec.sprite_id,
            company_palette,
            false,
            (0, 0),
            foundation_z_delta,
            (0, 0, 0),
            Some(TraceSpriteBounds::new(
                spec.dx as i32,
                spec.dy as i32,
                spec.dz as i32,
                spec.sx,
                spec.sy,
                23,
            )),
        );
        let center = build_centers[layer_i];
        commands.spawn((
            MapVisualLayer,
            ctx.map_tile_chunk(),
            tint_building_sprite(sprite_from_atlas_or_company_white_colour(
                company,
                owner_colour,
                image,
                spec.path,
            )),
            Transform::from_translation(center),
        ));
    }
}

#[cfg(test)]
mod tests {
    use bevy::prelude::Vec2;

    use super::{
        airport_station_ground_layer_trace_offset, buoy_trace_bounds, dock_clear_land_sprite_id,
        dock_water_neighbour_is_sea, newgrf_road_stop_child_center,
        rail_depot_build_parent_sprites, rail_depot_catenary_parent_sprite,
        rail_depot_foundation_child_offset, rail_depot_reservation_track_visible,
        road_depot_foundation_child_offset, road_depot_parent_sprites,
        road_stop_foundation_child_offset, road_stop_parent_sprites,
        station_catenary_pylon_parent_sprite, station_catenary_wire_parent_sprite,
        station_catenary_wire_trace_geometry, station_rail_child_offset,
        station_rail_foundation_world_z_delta, station_rail_layer_parent_sprite,
        station_rail_local_sorted_depths, tunnel_catenary_trace_geometry, tunnel_sortable_parents,
    };
    use openttdrs_core::{Map, TileCoord, TileKind, WaterClass, set_water_class_m1};

    use crate::render::world_draw_trace::TraceSpriteBounds;
    use crate::sprites::{
        CatenarySpriteDraw, CatenaryWireDraw, PYLON_SPRITE_BASE, StationTileClass,
        airport_station_ground_layers_for_gfx, rail_depot_build_layers, rail_station_draw_layers,
        road_depot_build_layers, road_stop_drive_through_layers,
    };

    #[test]
    fn newgrf_road_stop_child_uses_signed_screen_offsets() {
        let center = newgrf_road_stop_child_center(
            Vec2::new(100.0, 200.0),
            [4, 6, i8::MIN],
            10.0,
            20.0,
            3,
            4,
            0,
            0.05,
        );
        // OpenTTD child offsets are pixels from the previous sprite's
        // top-left; Y is inverted when entering Bevy's screen coordinates.
        assert_eq!(center.x, 109.0);
        assert_eq!(center.y, 184.0);
        assert_eq!(center.z, crate::iso::sortable_draw_z(3, 4, 0, 0.05));
    }

    #[test]
    fn dock_land_ground_uses_its_facing_water_class_and_full_clear_grass() {
        let land = TileCoord::new(2, 2);
        for (m5, water) in [
            (0, TileCoord::new(1, 2)), // NE
            (1, TileCoord::new(2, 3)), // SE
            (2, TileCoord::new(3, 2)), // SW
            (3, TileCoord::new(2, 1)), // NW
        ] {
            let mut map = Map::new_flat(5, 5, 0);
            assert!(map.set_kind(water, TileKind::Water).is_ok());
            assert!(
                map.set_m1(water, set_water_class_m1(0, WaterClass::Sea))
                    .is_ok()
            );
            assert!(
                dock_water_neighbour_is_sea(&map, land, m5),
                "m5={m5} debe consultar su mitad de agua"
            );

            assert!(
                map.set_m1(water, set_water_class_m1(0, WaterClass::Canal))
                    .is_ok()
            );
            assert!(
                !dock_water_neighbour_is_sea(&map, land, m5),
                "m5={m5} sobre canal/río usa DrawClearLandTile"
            );
        }

        // `DrawClearLandTile(ti, 3)`: bare land + 3 × 19 + slope offset.
        assert_eq!(dock_clear_land_sprite_id(0), 3981);
        assert_eq!(dock_clear_land_sprite_id(12), 3993);
        assert_eq!(dock_clear_land_sprite_id(29), 3996); // steep W → offset 15.
    }

    #[test]
    fn buoy_trace_matches_the_upstream_tile_sequence() {
        // Kale (194,149): StationType::Buoy. OpenTTD emite primero 4061 y
        // luego la línea de `station_land.h` con esta caja sin volumen.
        let bounds = buoy_trace_bounds();
        assert_eq!(
            (
                bounds.ox, bounds.oy, bounds.oz, bounds.ex, bounds.ey, bounds.ez
            ),
            (4, -1, 0, 0, 0, 0)
        );
    }

    #[test]
    fn rail_depot_reservation_is_hidden_behind_visible_ne_and_nw_buildings() {
        assert!(!rail_depot_reservation_track_visible(0, false)); // NE
        assert!(rail_depot_reservation_track_visible(1, false)); // SE
        assert!(rail_depot_reservation_track_visible(2, false)); // SW
        assert!(!rail_depot_reservation_track_visible(3, false)); // NW
        assert!(rail_depot_reservation_track_visible(0, true));
        assert!(rail_depot_reservation_track_visible(3, true));
    }

    #[test]
    fn sloped_rail_station_pbs_is_child_of_the_leveled_foundation() {
        assert_eq!(station_rail_child_offset(0), None);
        assert_eq!(station_rail_child_offset(6), Some((0, -32, 0)));
        assert_eq!(station_rail_child_offset(12), Some((0, -32, 0)));
    }

    #[test]
    fn sloped_rail_station_layers_use_the_leveled_foundation_height() {
        // Kale (16,39): raw z=1, `FOUNDATION_LEVELED` deja z=2 y los
        // sprites 1070/1072 se ordenan en z=16, no en el relieve z=8.
        assert_eq!(station_rail_foundation_world_z_delta(1, 2), 8);
        assert_eq!(station_rail_foundation_world_z_delta(0, 2), 16);
        assert_eq!(station_rail_foundation_world_z_delta(4, 4), 0);
    }

    #[test]
    fn sloped_rail_depot_ground_is_child_of_the_leveled_foundation() {
        assert_eq!(rail_depot_foundation_child_offset(0), None);
        assert_eq!(rail_depot_foundation_child_offset(11), Some((0, -32, 0)));
        assert_eq!(rail_depot_foundation_child_offset(0x17), Some((0, -32, 0)));
    }

    #[test]
    fn sloped_road_stop_ground_is_child_of_the_leveled_foundation() {
        assert_eq!(road_stop_foundation_child_offset(0), None);
        assert_eq!(road_stop_foundation_child_offset(6), Some((0, -32, 0)));
        assert_eq!(road_stop_foundation_child_offset(0x17), Some((0, -32, 0)));
    }

    #[test]
    fn road_stop_y_parents_match_kale_post_sort_bounds() {
        // Kale `(225,2)`: `DrawTile_Station` inserta 5982 y 5983 con las
        // cajas que el oráculo world-sort publica antes de invertirlas.
        let parents = road_stop_parent_sprites(
            225,
            2,
            1,
            road_stop_drive_through_layers(StationTileClass::Truck, 5),
        );
        assert_eq!(parents.len(), 2);
        assert_eq!(
            parents[0].kind,
            crate::render::viewport_sort::ParentSpriteKind::Sprite { sprite_id: 5982 }
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
            (3613, 32, 8, 3615, 47, 23)
        );
        assert_eq!(
            (
                parents[1].bounds.xmin,
                parents[1].bounds.ymin,
                parents[1].bounds.zmin,
                parents[1].bounds.xmax,
                parents[1].bounds.ymax,
                parents[1].bounds.zmax,
            ),
            (3600, 32, 8, 3602, 47, 23)
        );
        assert_eq!(
            crate::render::viewport_sort::viewport_sort_parent_sprites(&parents),
            vec![1, 0]
        );
    }

    #[test]
    fn road_depot_build_parents_keep_kale_post_sort_order() {
        // Kale `(120,8)`: el depósito SW emite 1410 y 1411. No hay
        // inversión local, pero las cajas siguen pasando por el mismo
        // contrato en vez de depender del índice de capa del atlas.
        let parents = road_depot_parent_sprites(120, 8, 1, road_depot_build_layers(2));
        assert_eq!(parents.len(), 2);
        assert_eq!(
            parents[0].kind,
            crate::render::viewport_sort::ParentSpriteKind::Sprite { sprite_id: 1410 }
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
            (1920, 128, 8, 1935, 128, 27)
        );
        assert_eq!(
            crate::render::viewport_sort::viewport_sort_parent_sprites(&parents),
            vec![0, 1]
        );
    }

    #[test]
    fn electric_rail_depot_orders_catenary_with_its_build_layers() {
        // Kale `(195,17)`: el C++ inserta 5659, 1063, 1064, pero el
        // `ViewportSortParentSprites` final los deja 1063, 5659, 1064.
        // El cable no puede quedar fuera del vector local de parents.
        let mut parents = vec![rail_depot_catenary_parent_sprite(0, 195, 17, 1, 1)];
        parents.extend(rail_depot_build_parent_sprites(
            195,
            17,
            1,
            1,
            rail_depot_build_layers(openttdrs_core::RailType::Electric, 1),
        ));
        assert_eq!(parents.len(), 3);
        assert_eq!(
            parents[0].kind,
            crate::render::viewport_sort::ParentSpriteKind::Sprite { sprite_id: 5659 }
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
            (3127, 272, 18, 3127, 286, 18)
        );
        assert_eq!(
            crate::render::viewport_sort::viewport_sort_parent_sprites(&parents),
            vec![1, 0, 2]
        );
        assert_eq!(
            crate::render::viewport_sort::depths_in_viewport_sort_order(
                &parents,
                &[0.035, 0.05, 0.06],
            ),
            vec![0.05, 0.035, 0.06]
        );
    }

    #[test]
    fn sloped_road_depot_ground_is_child_of_the_leveled_foundation() {
        assert_eq!(road_depot_foundation_child_offset(0), None);
        assert_eq!(road_depot_foundation_child_offset(6), Some((0, -32, 0)));
        assert_eq!(road_depot_foundation_child_offset(0x17), Some((0, -32, 0)));
    }

    #[test]
    fn rail_depot_newgrf_slots_follow_the_relocated_vanilla_order() {
        assert_eq!(super::rail_depot_custom_sprite_index(0, 0), Some(4));
        assert_eq!(super::rail_depot_custom_sprite_index(1, 0), Some(0));
        assert_eq!(super::rail_depot_custom_sprite_index(1, 1), Some(1));
        assert_eq!(super::rail_depot_custom_sprite_index(2, 0), Some(2));
        assert_eq!(super::rail_depot_custom_sprite_index(2, 1), Some(3));
        assert_eq!(super::rail_depot_custom_sprite_index(3, 0), Some(5));
        assert_eq!(super::rail_depot_custom_sprite_index(0, 1), None);
        assert_eq!(super::rail_depot_custom_sprite_index(3, 1), None);
    }

    #[test]
    fn tunnel_catenary_trace_uses_the_upstream_combined_parent_bounds() {
        // NE/SW: eje largo en X y cable desplazado 7 px en Y.
        assert_eq!(
            tunnel_catenary_trace_geometry(0),
            ((0, 7, 3), (0, 0, 7, 16, 15, 1))
        );
        // SE/NW: eje largo en Y y cable desplazado 7 px en X.
        assert_eq!(
            tunnel_catenary_trace_geometry(1),
            ((7, 0, 3), (0, 0, 7, 15, 16, 1))
        );
    }

    #[test]
    fn road_tunnel_portal_and_separators_keep_kale_post_sort_order() {
        // Kale `(8,5)`: el portal 2392 se inserta primero, pero el separador
        // trasero de la boca Y lo adelanta después de `ViewportSortParentSprites`.
        let (_, raw_bounds) = crate::sprites::tunnel_front_trace_geometry(1);
        let front_bounds = TraceSpriteBounds::new(
            raw_bounds.0,
            raw_bounds.1,
            raw_bounds.2,
            raw_bounds.3,
            raw_bounds.4,
            raw_bounds.5,
        );
        let parents = tunnel_sortable_parents(8, 5, 3, 2392, front_bounds, 1);
        assert_eq!(
            crate::render::viewport_sort::viewport_sort_parent_sprites(&parents),
            vec![1, 0, 2]
        );
        assert_eq!(
            (
                parents[1].bounds.xmin,
                parents[1].bounds.ymin,
                parents[1].bounds.zmin,
                parents[1].bounds.xmax,
                parents[1].bounds.ymax,
                parents[1].bounds.zmax,
            ),
            (128, 80, 24, 128, 95, 31)
        );
    }

    #[test]
    fn station_catenary_trace_matches_kale_platform_wire() {
        // Kale (194,22): plataforma Y eléctrica. OpenTTD inserta el wire
        // global 5649 entre el suelo y las capas de la estación, con la
        // `SortableSpriteStruct` de Y plano.
        let draw = CatenaryWireDraw {
            sprite_id: 1056,
            bounds_origin: (7, 0, 10),
            bounds_extent: (1, 15, 1),
        };
        assert_eq!(
            crate::sprites::catenary_reference_sprite_id(draw.sprite_id),
            5649
        );
        let (world_z_delta, bounds) = station_catenary_wire_trace_geometry(0, 1, 0x02, draw);
        assert_eq!(world_z_delta, 0);
        assert_eq!(
            (
                bounds.ox, bounds.oy, bounds.oz, bounds.ex, bounds.ey, bounds.ez
            ),
            (7, 0, 10, 1, 15, 1)
        );
    }

    #[test]
    fn electric_rail_station_orders_pylon_wire_and_platforms_like_kale() {
        // Kale `(195,21)`: la emisión local es PPP 5661, wire 5641,
        // plataforma 1071 y alero 1069. El sorter final cambia el orden a
        // 1071, 1069, 5661, 5641. Es una inversión que cruza las dos fases
        // de `DrawTile_Station`, no sólo dos capas BUILD.
        let pylon = CatenarySpriteDraw {
            sprite_id: PYLON_SPRITE_BASE + 1,
            tile_dx: 12.0,
            tile_dy: 16.0,
            z_layer: 0.036,
            pcp_direction: Some(0),
        };
        let wire = CatenaryWireDraw {
            sprite_id: 1048,
            bounds_origin: (7, 0, 10),
            bounds_extent: (1, 15, 1),
        };
        let layers = rail_station_draw_layers(1);
        let mut parents = vec![station_catenary_pylon_parent_sprite(
            0, 195, 21, 1, 0, 0x02, pylon,
        )];
        parents.push(station_catenary_wire_parent_sprite(
            1, 195, 21, 1, 0, 0x02, wire,
        ));
        parents.extend(layers.iter().enumerate().filter_map(|(index, layer)| {
            station_rail_layer_parent_sprite((index + 2) as u64, 195, 21, 1, 1, *layer)
        }));
        assert_eq!(
            parents.iter().map(|parent| parent.kind).collect::<Vec<_>>(),
            vec![
                crate::render::viewport_sort::ParentSpriteKind::Sprite { sprite_id: 5661 },
                crate::render::viewport_sort::ParentSpriteKind::Sprite { sprite_id: 5641 },
                crate::render::viewport_sort::ParentSpriteKind::Sprite { sprite_id: 1071 },
                crate::render::viewport_sort::ParentSpriteKind::Sprite { sprite_id: 1069 },
            ]
        );
        assert_eq!(
            crate::render::viewport_sort::viewport_sort_parent_sprites(&parents),
            vec![2, 3, 0, 1]
        );

        let depths =
            station_rail_local_sorted_depths(195, 21, 1, 1, 0, 0x02, &[pylon], &[wire], layers);
        assert_eq!(
            depths.pylons,
            vec![crate::iso::sortable_draw_z(195, 21, 1, 0.036)]
        );
        assert_eq!(
            depths.wires,
            vec![crate::iso::sortable_draw_z(195, 21, 1, 0.04)]
        );
        assert_eq!(
            depths.layers,
            vec![
                Some(crate::iso::sortable_draw_z(195, 21, 1, 0.03)),
                Some(crate::iso::sortable_draw_z(195, 21, 1, 0.035)),
            ]
        );
    }

    #[test]
    fn airport_ground_trace_preserves_tile_seq_ground_screen_offsets() {
        let south_west = airport_station_ground_layers_for_gfx(2);
        assert_eq!(
            airport_station_ground_layer_trace_offset(
                south_west[0].dx,
                south_west[0].dy,
                south_west[0].dz,
            ),
            (-120, 60, 0)
        );

        let south_east = airport_station_ground_layers_for_gfx(15);
        assert_eq!(
            airport_station_ground_layer_trace_offset(
                south_east[0].dx,
                south_east[0].dy,
                south_east[0].dz,
            ),
            (120, 60, 0)
        );

        let north_west_and_south_west = airport_station_ground_layers_for_gfx(57);
        assert_eq!(
            airport_station_ground_layer_trace_offset(
                north_west_and_south_west[0].dx,
                north_west_and_south_west[0].dy,
                north_west_and_south_west[0].dz,
            ),
            (0, 0, 0)
        );
        assert_eq!(
            airport_station_ground_layer_trace_offset(
                north_west_and_south_west[1].dx,
                north_west_and_south_west[1].dy,
                north_west_and_south_west[1].dz,
            ),
            (-120, 60, 0)
        );
    }
}
