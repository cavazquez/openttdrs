//! Dibujo compartido de tablero de puente (rampas y vano).

use bevy::prelude::*;
use openttdrs_core::prelude::*;
use openttdrs_core::{
    BridgeType, bridge_above_axis_from_mapt, bridge_type_from_m6, calc_bridge_piece,
    foundation_draw_plan, rail_bridge_other_end, rail_type_from_tile, road_bridge_other_end,
    tunnel_bridge_rail_reserved,
};

use crate::iso::{
    HEIGHT_PX, TILE_HALF_H, full_tile_sprite_pos_half, remap_tile_offset, slope_half_h,
    slope_sprite_offset, tile_pos_half, tile_slope_and_min_z,
};
use crate::render::catenary_newgrf::{
    catenary_sprite_anchor, catenary_sprite_center, catenary_sprite_colored,
};
use crate::render::viewport_sort::ParentSpriteBounds;
use crate::render::world_draw_trace::{TraceSpriteBounds, WorldDrawTrace};
use crate::render::{
    MapVisualLayer, NewGrfAction5SpriteCache, NewGrfCatenarySpriteCache, TileRenderContext,
    ViewportSortableChild, ViewportSortableParent, WorldAssets, viewport_insertion_key,
    viewport_source_depth,
};
use crate::sprites::{
    OTTD_MP_RAIL, RAIL_TB_X, RAIL_TB_Y, bridge_deck_sprite_ids, bridge_ramp_sprite_id,
    bridge_sprite_meta, bridge_structure_palette_for_sprite, catenary_reference_sprite_id,
    catenary_sprite_color, catenary_tile_location_group, collect_catenary_bridge_draws,
    collect_catenary_pylons_from_map, collect_catenary_wire_draws_from_map,
};

use super::helpers::{
    bridge_foundation_decision, foundation_surface_at, sloped_or_flat_image,
    spawn_empty_bounding_box, spawn_foundation_child_ground_sprite_at, spawn_foundation_sprite,
    spawn_ground_sprite_at,
};
use super::transport::catenary_local_z_delta;

const DECK_LAYER_FRAC: f32 = 0.08;
/// Vía sobre tablero (`DrawBridgeMiddle`: overlay entre psid\[0] y psid\[1]).
const RAIL_ON_BRIDGE_LAYER_FRAC: f32 = 0.084;
/// El exportador de OpenTTD guarda los offsets de `AddChildSpriteScreen`
/// multiplicados por `ZOOM_BASE` (=4). Una fundación nivelada llama
/// `OffsetGroundSprite(0, -TILE_HEIGHT)`.
const FOUNDATION_LEVELED_CHILD_OFFSET: (i32, i32, i32) = (0, -32, 0);
const FRONT_LAYER_FRAC: f32 = 0.088;
const PILLAR_BACK_LAYER_FRAC: f32 = 0.074;
const PILLAR_LAYER_FRAC: f32 = 0.075;
/// Deja espacio para los parents de `DrawFoundation` (ordinales 0..=3) antes
/// de insertar las piezas estructurales del puente en la misma tesela. Los
/// subrangos conservan el orden observable rear → catenaria → front → pilares.
const BRIDGE_REAR_ORDINAL: u8 = 32;
const BRIDGE_CATENARY_ORDINAL_BASE: u8 = 48;
const BRIDGE_FRONT_ORDINAL: u8 = 64;
const BRIDGE_PILLAR_ORDINAL_BASE: u8 = 80;
const BRIDGE_Z_START: f32 = 3.0;
const TILE_HEIGHT_PX: f32 = 8.0;
const TILE_HEIGHT_WORLD: i32 = 8;
/// `BB_Z_SEPARATOR` de `tunnelbridge_cmd.cpp`: separa el tablero del mundo
/// que se dibuja por debajo sin añadir píxeles propios.
const BRIDGE_SORT_SEPARATOR_Z: i32 = 7;
/// `SPR_FLAT_GRASS_TILE` de OpenTTD.
const SPR_FLAT_GRASS_TILE: u32 = 3981;
/// `SPR_FLAT_SNOW_DESERT_TILE` de OpenTTD.
const SPR_FLAT_SNOW_DESERT_TILE: u32 = 4550;
/// `SPR_SHORE_BASE` de OpenTTD.
const SPR_SHORE_BASE: u32 = 5936;
/// `SPR_RAIL_SINGLE_X`; usado por OpenTTD para resaltar una reserva PBS en
/// un puente que no tiene overlay NewGRF.
const SPR_RAIL_SINGLE_BASE: u32 = 1005;
/// `SPR_MONO_SINGLE_X`; análogo para monorriel.
const SPR_MONO_SINGLE_BASE: u32 = 1087;
/// `SPR_MGLV_SINGLE_X`; análogo para maglev.
const SPR_MAGLEV_SINGLE_BASE: u32 = 1169;
/// `SPR_TRAMWAY_OVERLAY` de la tabla vanilla (base 5986 + 4).
const SPR_TRAMWAY_OVERLAY_BASE: u32 = 5990;
/// `SPR_BRIDGE_DECKS_BASE` de Action5 `0x1B` (base 6240).
const SPR_BRIDGE_DECKS_BASE: u32 = 6240;
/// `SPR_TRACKS_FOR_SLOPES_RAIL_BASE` de OpenTTD.
const SPR_RAIL_SLOPED_RESERVATION_BASE: u32 = 5401;
/// `SPR_TRACKS_FOR_SLOPES_MONO_BASE` de OpenTTD.
const SPR_MONO_SLOPED_RESERVATION_BASE: u32 = 5405;
/// `SPR_TRACKS_FOR_SLOPES_MAGLEV_BASE` de OpenTTD.
const SPR_MAGLEV_SLOPED_RESERVATION_BASE: u32 = 5409;

/// Suelo que `DrawTile_TunnelBridge` compone bajo una rampa, una vez aplicada
/// su fundación. Es importante que esta decisión use `tileh` y `z` *efectivos*:
/// OpenTTD llama primero a `DrawFoundation` y después decide entre costa,
/// césped y nieve/desierto.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BridgeRampGround {
    Grass,
    Shore,
    SnowOrDesert,
}

pub(crate) struct BridgeSpanInfo {
    pub deck_z: u8,
    pub rail: bool,
    /// Reserva PBS de las rampas ferroviarias del puente. OpenTTD reutiliza
    /// los sprites `SINGLE_*` únicamente como overlay de esta reserva.
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

/// Decisión de `DrawRailCatenaryRailway` cuando una vía inferior pasa bajo un
/// puente bajo visible. El cable no se dibuja bajo el tablero y los PCP del
/// eje del puente tampoco pueden generar postes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct CatenaryUnderLowBridge {
    pub(crate) hide_wires: bool,
    pub(crate) pylon_pcp_override: u8,
}

/// Replica el bloque `IsBridgeAbove` de `DrawRailCatenaryRailway`.
///
/// `GetBridgeHeight <= GetTileMaxZ + 1` significa que el tablero está a una
/// altura insuficiente para ver el cable inferior. Los dos extremos PCP de
/// la pista paralela al puente se marcan además como override, tal como hace
/// OpenTTD antes de decidir sus postes.
pub(crate) fn catenary_under_low_bridge(
    map: &Map,
    coord: TileCoord,
    dims: (u32, u32),
) -> CatenaryUnderLowBridge {
    let Some(tile) = map.get(coord) else {
        return CatenaryUnderLowBridge::default();
    };
    let Some(axis_y) = bridge_above_axis_from_mapt(tile.mapt) else {
        return CatenaryUnderLowBridge::default();
    };
    let Some(span) = bridge_span_at(map, coord, dims) else {
        return CatenaryUnderLowBridge::default();
    };
    let (tileh, base_z) = tile_slope_and_min_z(map, coord.x as u32, coord.y as u32);
    let max_z = base_z.saturating_add(if tileh & 0x10 != 0 {
        2
    } else if tileh & 0x0F != 0 {
        1
    } else {
        0
    });
    if span.deck_z > max_z.saturating_add(1) {
        return CatenaryUnderLowBridge::default();
    }

    // `_pcp_positions[AxisToTrack(axis)]`: X = NE/SW, Y = SE/NW.
    let pylon_pcp_override = if axis_y { 0b1010 } else { 0b0101 };
    CatenaryUnderLowBridge {
        hide_wires: true,
        pylon_pcp_override,
    }
}

/// Decide si un detalle lateral de una carretera puede verse bajo el tablero
/// elevado. Replica el bloque de `DrawRoadBits`: faroles requieren dos niveles
/// de despeje sobre el máximo del terreno y árboles, tres.
///
/// Los detalles no deben consultar la pendiente de la rampa del puente: como
/// `GetBridgeHeight(GetNorthernBridgeEnd(...))`, se comparan contra el tablero
/// completo y el `GetTileMaxZ` de la tesela inferior actual.
pub(crate) fn roadside_detail_visible_under_bridge(
    map: &Map,
    coord: TileCoord,
    dims: (u32, u32),
    trees: bool,
) -> bool {
    let Some(tile) = map.get(coord) else {
        return true;
    };
    if bridge_above_axis_from_mapt(tile.mapt).is_none() {
        return true;
    }
    let Some(span) = bridge_span_at(map, coord, dims) else {
        return true;
    };
    let (tileh, base_z) = tile_slope_and_min_z(map, coord.x as u32, coord.y as u32);
    let terrain_max_z = base_z.saturating_add(if tileh & 0x10 != 0 {
        2
    } else if tileh & 0x0F != 0 {
        1
    } else {
        0
    });
    let minimum_deck_z = terrain_max_z
        .saturating_add(2)
        .saturating_add(u8::from(trees));
    span.deck_z >= minimum_deck_z
}

fn ramp_tile(tile: Tile) -> bool {
    tile.is_tunnel_bridge_tile() && tile.m5 & 0x80 != 0
}

/// Offset normalizado de `DrawGroundSprite` cuando la rampa ya tiene padre
/// de fundación. `GetBridgeFoundation` sólo produce Leveled o Inclined{X,Y};
/// las inclinadas usan `(0, 0)`.
const fn bridge_foundation_child_offset(foundation: u8) -> (i32, i32, i32) {
    if foundation == openttdrs_core::FOUNDATION_LEVELED {
        FOUNDATION_LEVELED_CHILD_OFFSET
    } else {
        (0, 0, 0)
    }
}

fn bridge_ramp_ground_kind(
    map: &Map,
    coord: TileCoord,
    tile: Tile,
    foundation_tileh: u8,
    foundation_base_z: u8,
) -> BridgeRampGround {
    // `HasTunnelBridgeSnowOrDesert`: bit 5 de MAP7.
    if tile.m7 & 0x20 != 0 {
        return BridgeRampGround::SnowOrDesert;
    }

    // `DrawShoreTile` sólo se usa en una rampa inclinada a nivel del mar y
    // cuando la tesela inmediatamente delante de la rampa tiene clase Sea.
    // La dirección es `GetTunnelBridgeDirection` (los dos bits bajos de m5).
    if foundation_tileh != 0 && foundation_base_z == 0 {
        let (dx, dy) = openttdrs_core::diag_dir_offset(tile.m5 & 0x03);
        let next = TileCoord::new(coord.x + dx, coord.y + dy);
        if map.get(next).is_some_and(|next_tile| {
            openttdrs_core::water_class(next_tile) == Some(openttdrs_core::WaterClass::Sea)
        }) {
            return BridgeRampGround::Shore;
        }
    }

    BridgeRampGround::Grass
}

fn bridge_ramp_ground_sprite_id(kind: BridgeRampGround, tileh: u8) -> u32 {
    match kind {
        BridgeRampGround::Grass => SPR_FLAT_GRASS_TILE + u32::from(slope_sprite_offset(tileh)),
        BridgeRampGround::Shore => {
            let shore = crate::sprites::TILEH_TO_SHORE_SPRITE[usize::from(tileh)];
            SPR_SHORE_BASE + u32::from(shore)
        }
        BridgeRampGround::SnowOrDesert => {
            SPR_FLAT_SNOW_DESERT_TILE + u32::from(slope_sprite_offset(tileh))
        }
    }
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

/// La cabeza contiene la subida en su propio sprite: se dibuja desde el suelo.
/// El vano se dibuja a la altura elevada del tablero.
fn bridge_surface_z(base_z: u8, deck_z: u8, on_ramp: bool) -> u8 {
    if on_ramp { base_z } else { deck_z }
}

/// Caja invisible que `DrawBridgeMiddle` agrega antes de sus barandillas.
///
/// La estructura de puente no puede depender sólo de sus sprites visibles:
/// OpenTTD inserta este parent `SPR_EMPTY_BOUNDING_BOX` a una unidad de píxel
/// por debajo del tablero para que el sorter separe correctamente todo lo que
/// pasa debajo. El renderer la materializa como constraint sin `Sprite`.
fn bridge_sort_separator_bounds() -> TraceSpriteBounds {
    TraceSpriteBounds::new(0, 0, 0, 16, 16, 1)
}

fn bridge_sort_separator_world_z_delta(base_z: u8, deck_z: u8) -> i32 {
    (i32::from(deck_z) - i32::from(base_z)) * TILE_HEIGHT_WORLD - TILE_HEIGHT_WORLD
        + BRIDGE_SORT_SEPARATOR_Z
}

/// Sprite del overlay de reserva PBS de un puente vanilla.
///
/// No es una capa estructural permanente: OpenTTD dibuja la estructura
/// tipada mono/maglev en `psid[0]` y sólo agrega estos `SINGLE_*` con
/// `PALETTE_CRASH` cuando la reserva está visible. Para una rampa plana, la
/// subida ocurre dentro de la tesela y se usa el bloque
/// `SPR_TRACKS_FOR_SLOPES_*`; en cualquier otra pendiente efectiva se usa
/// el sprite recto X/Y.
fn bridge_pbs_reservation_sprite_id(
    rail_type: openttdrs_core::RailType,
    axis: usize,
    on_ramp: bool,
    ramp_effective_tileh: u8,
    ramp_direction: u8,
) -> u32 {
    let (single_base, sloped_base) = match rail_type {
        openttdrs_core::RailType::Rail | openttdrs_core::RailType::Electric => {
            (SPR_RAIL_SINGLE_BASE, SPR_RAIL_SLOPED_RESERVATION_BASE)
        }
        openttdrs_core::RailType::Monorail => {
            (SPR_MONO_SINGLE_BASE, SPR_MONO_SLOPED_RESERVATION_BASE)
        }
        openttdrs_core::RailType::Maglev => {
            (SPR_MAGLEV_SINGLE_BASE, SPR_MAGLEV_SLOPED_RESERVATION_BASE)
        }
    };

    if on_ramp && ramp_effective_tileh == 0 {
        sloped_base + u32::from(ramp_direction & 3)
    } else {
        single_base + (axis & 1) as u32
    }
}

/// Compensa el origen NFO de los overlays PBS recortados del GRF extra.
///
/// Las imágenes de `5401..=5412` son una porción muy pequeña del sprite
/// original; a diferencia de los PNG compuestos de vía, el atlas no conserva
/// `xrel/yrel`. Estos offsets las vuelven a anclar al centro 64×31 que usa el
/// renderer de vía común. Los `SINGLE_*` reutilizan la compensación ya
/// establecida para rail/monorail/maglev.
fn bridge_pbs_reservation_offset(sprite_id: u32) -> Vec2 {
    crate::sprites::rail_pbs_reservation_offset(sprite_id)
}

/// Caja de ordenación de la reserva PBS emitida por `DrawTile_TunnelBridge`.
///
/// La reserva no se agrega como otro sprite sortable independiente: el
/// `StartSpriteCombine` de la cabeza o del vano la convierte en un `combined`
/// hijo de la estructura. En rampas, `HasBridgeFlatRamp` vuelve a aplicar la
/// fundación a la pendiente ya efectiva antes de escoger entre ambas cajas.
fn bridge_pbs_trace_bounds(on_ramp: bool, foundation_tileh: u8, axis: usize) -> TraceSpriteBounds {
    if !on_ramp {
        // `DrawBridgeMiddle`: el tablero ya está en `deck_z` y la reserva
        // ocupa la superficie plana de la tesela.
        return TraceSpriteBounds::new(0, 0, 0, 16, 16, 0);
    }

    let (ramp_tileh, _) = openttdrs_core::bridge_surface_slope_and_z(foundation_tileh, axis == 0);
    if ramp_tileh != 0 {
        // `HasBridgeFlatRamp == true`: vía plana sobre una rampa del terreno.
        TraceSpriteBounds::new(0, 0, 8, 16, 16, 0)
    } else {
        // Vía inclinada que parte de una superficie de fundación plana.
        TraceSpriteBounds::new(0, 0, 0, 16, 16, 8)
    }
}

/// Materializa un `AddSortableSpriteToDraw` emitido dentro de
/// `StartSpriteCombine` como child del primer sprite del bloque.
///
/// Los overlays del tablero y las reservas PBS no son parents independientes:
/// OpenTTD los mueve junto con la barandilla trasera cuando el `ViewportSort`
/// intercambia el puente con una casa, una fundación o una tesela vecina. El
/// `source_depth` conserva el orden local del bloque y el sistema global de
/// children lo encierra entre este parent y el siguiente.
fn spawn_bridge_combined_child(
    commands: &mut Commands,
    ctx: &TileRenderContext,
    map_width: u32,
    parent: Entity,
    sprite: Sprite,
    position: Vec3,
) {
    let source_depth = viewport_source_depth(position.z, ctx.tx, map_width);
    commands.spawn((
        MapVisualLayer,
        ctx.map_tile_chunk(),
        sprite,
        Transform::from_translation(position.with_z(source_depth)),
        ViewportSortableChild {
            parent,
            source_depth,
        },
    ));
}

#[allow(clippy::too_many_arguments)]
fn spawn_bridge_pbs_reservation(
    commands: &mut Commands,
    assets: &WorldAssets,
    ctx: &TileRenderContext,
    span: &BridgeSpanInfo,
    on_ramp: bool,
    foundation_tileh: u8,
    surface_z: u8,
    map_width: u32,
    parent: Option<Entity>,
) {
    let ramp_direction = ctx.tile.map_or(0, |tile| tile.m5);
    let sprite_id = bridge_pbs_reservation_sprite_id(
        span.rail_type,
        span.axis,
        on_ramp,
        foundation_tileh,
        ramp_direction,
    );
    WorldDrawTrace::record_sprite_with_palette_and_geometry(
        "bridge-pbs-reservation",
        "combined",
        sprite_id,
        // `PALETTE_CRASH`: el overlay se remapea por índice con la tabla 804
        // de OpenTTD; no es una mezcla de color sobre el tablero.
        804,
        !assets.has_exact_pbs_rail_sprite(sprite_id),
        (0, 0, 0),
        (i32::from(surface_z) - i32::from(ctx.info.base_z)) * 8,
        Some(bridge_pbs_trace_bounds(
            on_ramp,
            foundation_tileh,
            span.axis,
        )),
    );
    let Some(sprite) = assets.pbs_rail_sprite(sprite_id) else {
        return;
    };
    let offset = bridge_pbs_reservation_offset(sprite_id);
    let position = full_tile_sprite_pos_half(
        ctx.tx_i32(),
        ctx.ty_i32(),
        surface_z,
        RAIL_ON_BRIDGE_LAYER_FRAC,
        TILE_HALF_H,
    ) + Vec3::new(offset.x, offset.y, 0.0);
    if let Some(parent) = parent {
        spawn_bridge_combined_child(commands, ctx, map_width, parent, sprite.sprite(), position);
    } else {
        commands.spawn((
            MapVisualLayer,
            ctx.map_tile_chunk(),
            sprite.sprite(),
            Transform::from_translation(position),
        ));
    }
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

/// La dirección de una rampa codifica de forma inequívoca el eje del puente:
/// NE/SW recorren X; SE/NW recorren Y. No se puede inferir probando ambos
/// ejes: dos rampas de puentes vecinos pueden quedar alineadas por casualidad
/// (caso real de Kale en 133,152) y forman un falso puente transversal.
fn ramp_axis_y(tile: Tile) -> bool {
    tile.m5 & 1 != 0
}

/// Otra rampa del mismo puente, validando también el transporte. El resolvedor
/// de core sigue la dirección persistida, pero la validación adicional evita
/// unir una rampa ferroviaria a una de carretera superpuesta.
fn paired_bridge_ramp(map: &Map, ramp: TileCoord) -> Option<TileCoord> {
    let tile = map.get(ramp)?;
    if !ramp_tile(tile) {
        return None;
    }
    let other = match tile.kind {
        TileKind::RailBridge => rail_bridge_other_end(map, ramp),
        TileKind::RoadBridge => road_bridge_other_end(map, ramp),
        _ => None,
    }?;
    let other_tile = map.get(other)?;
    (ramp_tile(other_tile)
        && other_tile.kind == tile.kind
        && (other_tile.m5 & 3) == ((tile.m5 + 2) & 3))
        .then_some(other)
}

/// Busca una rampa desde una tesela bajo el vano. Sólo atraviesa el vano con
/// el mismo eje y exige que la rampa encontrada mire hacia el centro.
fn find_ramp_from_middle(
    map: &Map,
    start: TileCoord,
    dx: i32,
    dy: i32,
    axis_y: bool,
    dims: (u32, u32),
) -> Option<TileCoord> {
    let expected_direction = match (-dx, -dy) {
        (-1, 0) => 0,
        (0, 1) => 1,
        (1, 0) => 2,
        (0, -1) => 3,
        _ => return None,
    };
    let mut x = start.x + dx;
    let mut y = start.y + dy;
    loop {
        if x < 0 || y < 0 || x >= dims.0 as i32 || y >= dims.1 as i32 {
            return None;
        }
        let c = TileCoord::new(x, y);
        let t = map.get(c)?;
        if ramp_tile(t) {
            return ((t.m5 & 3) == expected_direction).then_some(c);
        }
        if bridge_above_axis_from_mapt(t.mapt) != Some(axis_y) {
            return None;
        }
        x += dx;
        y += dy;
    }
}

fn order_bridge_ramps(first: TileCoord, second: TileCoord, axis_y: bool) -> (TileCoord, TileCoord) {
    if (axis_y && first.y <= second.y) || (!axis_y && first.x <= second.x) {
        (first, second)
    } else {
        (second, first)
    }
}

/// Resuelve el eje y las dos rampas del puente que realmente contiene
/// `coord`. Las rampas usan su dirección codificada; los vanos usan `mapt`.
fn bridge_span_endpoints(
    map: &Map,
    coord: TileCoord,
    dims: (u32, u32),
) -> Option<(bool, TileCoord, TileCoord)> {
    let tile = map.get(coord)?;
    if ramp_tile(tile) {
        let axis_y = ramp_axis_y(tile);
        let other = paired_bridge_ramp(map, coord)?;
        // Una rampa SE/NW nunca puede terminar a izquierda/derecha, ni una
        // NE/SW arriba/abajo. Esta guarda convierte una corrupción de mapa en
        // ausencia de render, no en un tablero cruzado visible.
        if (axis_y && other.x != coord.x) || (!axis_y && other.y != coord.y) {
            return None;
        }
        let (north, south) = order_bridge_ramps(coord, other, axis_y);
        return Some((axis_y, north, south));
    }

    let axis_y = bridge_above_axis_from_mapt(tile.mapt)?;
    let (dx, dy) = axis_step(usize::from(axis_y));
    let neg = find_ramp_from_middle(map, coord, -dx, -dy, axis_y, dims)?;
    let pos = find_ramp_from_middle(map, coord, dx, dy, axis_y, dims)?;
    if paired_bridge_ramp(map, neg) != Some(pos) {
        return None;
    }
    let (north, south) = order_bridge_ramps(neg, pos, axis_y);
    Some((axis_y, north, south))
}

/// Resuelve rampas, tipo y pieza de puente para una tesela (rampa o vano).
pub(crate) fn bridge_span_at(
    map: &Map,
    coord: TileCoord,
    dims: (u32, u32),
) -> Option<BridgeSpanInfo> {
    let tile = map.get(coord)?;
    let (axis_y, north, south) = bridge_span_endpoints(map, coord, dims)?;
    let axis = usize::from(axis_y);

    let ramp_ref = if ramp_tile(tile) { coord } else { south };
    let ramp_tile_ref = map.get(ramp_ref)?;
    let (tileh, min_z) = tile_slope_and_min_z(map, ramp_ref.x as u32, ramp_ref.y as u32);
    let deck_z = bridge_deck_z(tileh, min_z, axis);
    let rail = (ramp_tile_ref.m5 >> 2) & 0x3 == 0;
    // `HasTunnelBridgeReservation` usa m5 bit 4, no el formato MAP2 de una
    // tesela ferroviaria común. Un puente se dibuja desde cualquiera de sus
    // teselas, por lo que propagamos la reserva de sus rampas al tablero.
    let pbs_reserved = rail
        && [north, south]
            .into_iter()
            .any(|ramp| map.get(ramp).is_some_and(tunnel_bridge_rail_reserved));
    let bridge_type = bridge_type_from_m6(ramp_tile_ref.m6);

    // `tile_dist` cuenta ambas teselas. Para una tesela intermedia OpenTTD
    // calcula la pieza desde su distancia hasta cada rampa, sin contar la
    // rampa misma (`GetTunnelBridgeLength(...) + 1`). Conservamos la medida
    // inclusiva para las rampas (también la exponen los tests), pero quitamos
    // ese extremo al clasificar el vano.
    let north_tiles = tile_dist(coord, north, axis_y);
    let south_tiles = tile_dist(coord, south, axis_y);
    let on_ramp = ramp_tile(tile);
    let piece = if on_ramp {
        calc_bridge_piece(north_tiles, south_tiles)
    } else {
        calc_bridge_piece(north_tiles.saturating_sub(1), south_tiles.saturating_sub(1))
    };
    let total = tile_dist(north, south, axis_y);
    let middle_length = total.saturating_sub(2);
    let middle_num = if on_ramp || middle_length == 0 {
        0
    } else {
        north_tiles.saturating_sub(1).min(middle_length).max(1)
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

/// Caja sortable de un cable recto de catenaria en el vano de un puente.
///
/// `DrawRailCatenaryOnBridge` sólo alterna el sprite largo/corto y el
/// remate; ambos comparten estos bounds por eje en
/// `_rail_catenary_sprite_data`.
fn bridge_catenary_wire_trace_bounds(axis: usize) -> TraceSpriteBounds {
    if axis == 0 {
        TraceSpriteBounds::new(0, 7, 10, 15, 1, 1)
    } else {
        TraceSpriteBounds::new(7, 0, 10, 1, 15, 1)
    }
}

#[allow(clippy::too_many_arguments)]
fn spawn_bridge_catenary(
    commands: &mut Commands,
    assets: &WorldAssets,
    ctx: &TileRenderContext,
    span: &BridgeSpanInfo,
    map_width: u32,
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
    for (draw_index, draw) in draws.into_iter().enumerate() {
        let anchor = catenary_sprite_anchor(draw.sprite_id, catenary_newgrf);
        let sprite = catenary_sprite_colored(
            assets,
            draw.sprite_id,
            tint,
            catenary_newgrf,
            catenary_sprites.as_deref_mut(),
            images.as_deref_mut(),
        );
        let world_z_delta = (i32::from(span.deck_z) - i32::from(ctx.info.base_z)) * 8;
        let bounds = if draw.pcp_direction.is_some() {
            // `DrawRailCatenaryOnBridge` ancla los postes directamente a la
            // altura del tablero. A diferencia de una rampa no consulta la
            // pendiente del terreno: el pilar vive completamente sobre el
            // vano plano.
            WorldDrawTrace::record_sprite_with_palette_and_world_geometry(
                "bridge-catenary-pylon",
                "sortable",
                catenary_reference_sprite_id(draw.sprite_id),
                0,
                sprite.is_none(),
                (draw.tile_dx as i32, draw.tile_dy as i32),
                world_z_delta,
                (1, 1, 0),
                Some(TraceSpriteBounds::new(-1, -1, 0, 1, 1, 6)),
            );
            TraceSpriteBounds::new(-1, -1, 0, 1, 1, 6)
        } else {
            // Los cables del vano son siempre rectos. Los WSO largo/corto
            // cambian el PNG y el remate, pero comparten la misma caja del
            // eje correspondiente en `_rail_catenary_sprite_data`.
            let bounds = bridge_catenary_wire_trace_bounds(span.axis);
            WorldDrawTrace::record_sprite_with_palette_and_geometry(
                "bridge-catenary-wire",
                "sortable",
                catenary_reference_sprite_id(draw.sprite_id),
                0,
                sprite.is_none(),
                (0, 0, 0),
                world_z_delta,
                Some(bounds),
            );
            bounds
        };
        let Some((sprite, anchor)) = sprite.zip(anchor) else {
            continue;
        };
        let (tile_dx, tile_dy, local_z) = if draw.pcp_direction.is_some() {
            (
                draw.tile_dx - 1.0,
                draw.tile_dy - 1.0,
                catenary_local_z_delta(world_z_delta, ctx.info.base_z, span.deck_z),
            )
        } else {
            (
                bounds.ox as f32,
                bounds.oy as f32,
                catenary_local_z_delta(world_z_delta + bounds.oz, ctx.info.base_z, span.deck_z),
            )
        };
        let position = catenary_sprite_center(
            ctx.tx_i32(),
            ctx.ty_i32(),
            span.deck_z,
            draw.z_layer,
            tile_dx,
            tile_dy,
            local_z as f32,
            anchor,
        );
        let placement = BridgeTracePlacement {
            world_xy_delta: if draw.pcp_direction.is_some() {
                (draw.tile_dx as i32, draw.tile_dy as i32)
            } else {
                (0, 0)
            },
            world_z_delta,
            offset: (0, 0, 0),
            bounds,
        };
        let parent = bridge_sortable_parent(
            ctx,
            map_width,
            catenary_reference_sprite_id(draw.sprite_id),
            span.deck_z,
            draw.z_layer,
            BRIDGE_CATENARY_ORDINAL_BASE.saturating_add(draw_index as u8),
            placement,
        );
        commands.spawn((
            MapVisualLayer,
            ctx.map_tile_chunk(),
            sprite,
            Transform::from_translation(position),
            parent,
        ));
    }
}

/// Emula el `GetSlopePixelZ_TunnelBridge` de OpenTTD para una rampa sobre
/// terreno plano: aunque `tileh` sea plano, el cable asciende dentro de la
/// tesela hacia el tablero. En una rampa sobre pendiente, la fundación deja
/// una superficie plana al nivel del tablero.
fn bridge_ramp_catenary_slope(tileh: u8, dir: u8) -> u8 {
    if tileh != 0 {
        return 0;
    }
    match dir & 3 {
        0 => openttdrs_core::SLOPE_NE,
        1 => openttdrs_core::SLOPE_SE,
        2 => openttdrs_core::SLOPE_SW,
        _ => openttdrs_core::SLOPE_NW,
    }
}

/// Ancla vertical de la catenaria sobre una rampa de puente.
///
/// `DrawRailCatenaryRailway` escoge cable/poste a partir del `TileInfo` ya
/// modificado por `DrawFoundation` y `AdjustTileh`, pero al registrarlo llama
/// a `GetSlopePixelZ(tile, ..., true)`. Para `TunnelBridge` esa consulta no
/// usa la pendiente de la vía normal: aplica de nuevo la fundación y, sobre
/// una rampa que quedó plana, interpola la inclinación en la dirección del
/// puente. Si la fundación dejó una pendiente, devuelve directamente un nivel
/// de terreno por encima de ella. Esta es la traducción de
/// `GetSlopePixelZ_TunnelBridge` más el redondeo específico de cables (8 px)
/// o PCPs (4 px).
fn bridge_ramp_catenary_world_z_delta(
    foundation_tileh: u8,
    foundation_base_z: u8,
    raw_base_z: u8,
    dir: u8,
    x: i32,
    y: i32,
    rounding: i32,
) -> i32 {
    let raw_base_px = i32::from(raw_base_z) * 8;
    let foundation_base_px = i32::from(foundation_base_z) * 8;
    let slope_px = if foundation_tileh != 0 {
        foundation_base_px + 8
    } else {
        foundation_base_px
            + i32::from(openttdrs_core::partial_pixel_z(
                x as f32,
                y as f32,
                bridge_ramp_catenary_slope(0, dir),
            ))
    };
    ((slope_px + rounding / 2) / rounding) * rounding - raw_base_px
}

#[allow(clippy::too_many_arguments)]
fn spawn_bridge_ramp_catenary(
    commands: &mut Commands,
    map: &Map,
    dims: (u32, u32),
    assets: &WorldAssets,
    ctx: &TileRenderContext,
    span: &BridgeSpanInfo,
    map_width: u32,
    foundation_tileh: u8,
    foundation_base_z: u8,
    catenary_newgrf: &[Option<openttdrs_core::DecodedSprite>],
    mut catenary_sprites: Option<&mut NewGrfCatenarySpriteCache>,
    mut images: Option<&mut Assets<Image>>,
) {
    let Some(tile) = ctx.tile else {
        return;
    };
    let trackbits = if span.axis == 0 { RAIL_TB_X } else { RAIL_TB_Y };
    // `DrawFoundation` modifica `TileInfo` antes de que OpenTTD dibuje la
    // catenaria. Usar la pendiente cruda deja los cables una altura por
    // debajo del tablero en rampas con fundación nivelada.
    let ramp_slope = bridge_ramp_catenary_slope(foundation_tileh, tile.m5);
    let base_z = foundation_base_z;
    let tint = catenary_sprite_color();

    let mut wires = Vec::new();
    collect_catenary_wire_draws_from_map(
        map,
        ctx.coord,
        dims.0,
        dims.1,
        OTTD_MP_RAIL,
        trackbits,
        ramp_slope,
        &mut wires,
    );
    let mut pylons = Vec::new();
    collect_catenary_pylons_from_map(
        map,
        ctx.coord,
        dims.0,
        dims.1,
        OTTD_MP_RAIL,
        trackbits,
        ramp_slope,
        &mut pylons,
    );
    let pylon_count = pylons.len();
    for (pylon_index, draw) in pylons.into_iter().enumerate() {
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
            let index = usize::from(pcp & 3);
            bridge_ramp_catenary_world_z_delta(
                foundation_tileh,
                foundation_base_z,
                ctx.info.base_z,
                tile.m5,
                [0, 8, 16, 8][index].min(15),
                [8, 16, 8, 0][index].min(15),
                4,
            )
        });
        WorldDrawTrace::record_sprite_with_palette_and_world_geometry(
            "bridge-ramp-catenary-pylon",
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
        let local_z = catenary_local_z_delta(world_z_delta, ctx.info.base_z, base_z);
        let position = catenary_sprite_center(
            ctx.tx_i32(),
            ctx.ty_i32(),
            base_z,
            draw.z_layer + 0.055,
            draw.tile_dx - 1.0,
            draw.tile_dy - 1.0,
            local_z as f32,
            anchor,
        );
        let parent = bridge_sortable_parent(
            ctx,
            map_width,
            catenary_reference_sprite_id(draw.sprite_id),
            base_z,
            draw.z_layer + 0.055,
            BRIDGE_CATENARY_ORDINAL_BASE.saturating_add(pylon_index as u8),
            BridgeTracePlacement {
                world_xy_delta: (draw.tile_dx as i32, draw.tile_dy as i32),
                world_z_delta,
                offset: (1, 1, 0),
                bounds: TraceSpriteBounds::new(-1, -1, 0, 1, 1, 6),
            },
        );
        commands.spawn((
            MapVisualLayer,
            ctx.map_tile_chunk(),
            sprite,
            Transform::from_translation(position),
            parent,
        ));
    }

    // Aunque `DrawRailCatenaryOnBridge` llama primero al cable, el stream
    // sortable de OpenTTD coloca el PCP antes por su caja/ancla. Emitirlos en
    // este orden conserva el stream observable y el orden visual ya queda
    // protegido por las capas `z_layer` (poste por encima del cable).
    for (i, draw) in wires.into_iter().enumerate() {
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
        let world_z_delta = bridge_ramp_catenary_world_z_delta(
            foundation_tileh,
            foundation_base_z,
            ctx.info.base_z,
            tile.m5,
            ox,
            oy,
            8,
        );
        WorldDrawTrace::record_sprite_with_palette_and_geometry(
            "bridge-ramp-catenary-wire",
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
        let local_z = catenary_local_z_delta(world_z_delta + oz, ctx.info.base_z, base_z);
        let position = catenary_sprite_center(
            ctx.tx_i32(),
            ctx.ty_i32(),
            base_z,
            0.09 + i as f32 * 0.0004,
            ox as f32,
            oy as f32,
            local_z as f32,
            anchor,
        );
        let parent = bridge_sortable_parent(
            ctx,
            map_width,
            catenary_reference_sprite_id(sid),
            base_z,
            0.09 + i as f32 * 0.0004,
            BRIDGE_CATENARY_ORDINAL_BASE
                .saturating_add(pylon_count as u8)
                .saturating_add(i as u8),
            BridgeTracePlacement {
                world_xy_delta: (0, 0),
                world_z_delta,
                offset: (0, 0, 0),
                bounds: TraceSpriteBounds::new(ox, oy, oz, ex, ey, ez),
            },
        );
        commands.spawn((
            MapVisualLayer,
            ctx.map_tile_chunk(),
            sprite,
            Transform::from_translation(position),
            parent,
        ));
    }
}

/// Mitad visible de un pilar sobre una arista inclinada. La distinción no es
/// meramente estética: OpenTTD corta el mismo PNG por la mitad en lugar de
/// dibujar una columna adicional completa.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PillarHalf {
    North,
    South,
}

/// Un tramo que `DrawBridgePillars` entrega a `DrawPillar`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PillarSegment {
    z_px: i32,
    half: Option<PillarHalf>,
}

/// Las cuatro alturas que OpenTTD obtiene mediante dos llamadas a
/// `GetSlopePixelZOnEdge`: norte/sur del pilar frontal y del trasero.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PillarGroundHeights {
    front_north: i32,
    front_south: i32,
    back_north: i32,
    back_south: i32,
}

const PILLAR_SLOPE_HALFTILE: u8 = 0x20;
const PILLAR_SLOPE_W: u8 = 0x01;
const PILLAR_SLOPE_S: u8 = 0x02;
const PILLAR_SLOPE_E: u8 = 0x04;
const PILLAR_SLOPE_N: u8 = 0x08;
const PILLAR_SLOPE_STEEP_W: u8 = 0x10 | PILLAR_SLOPE_N | PILLAR_SLOPE_W | PILLAR_SLOPE_S;
const PILLAR_SLOPE_STEEP_S: u8 = 0x10 | PILLAR_SLOPE_W | PILLAR_SLOPE_S | PILLAR_SLOPE_E;
const PILLAR_SLOPE_STEEP_E: u8 = 0x10 | PILLAR_SLOPE_S | PILLAR_SLOPE_E | PILLAR_SLOPE_N;
const PILLAR_SLOPE_STEEP_N: u8 = 0x10 | PILLAR_SLOPE_E | PILLAR_SLOPE_N | PILLAR_SLOPE_W;

/// Traducción directa de `GetSlopePixelZOnEdge` para el cálculo de pilares.
///
/// `tileh` puede contener la codificación interna de media tesela; aunque no
/// aparece en el terreno natural, sí puede llegar tras una fundación. Mantener
/// ese caso hace que este helper preserve la semántica de OpenTTD completa.
fn pillar_edge_heights(
    tileh: u8,
    base_z: u8,
    first: u8,
    second: u8,
    steep_first: u8,
    steep_second: u8,
) -> (i32, i32) {
    let base = i32::from(base_z) * TILE_HEIGHT_PX as i32;
    let mut z_first = base;
    let mut z_second = base;

    if tileh & PILLAR_SLOPE_HALFTILE != 0 {
        let halftile_corner = 1 << ((tileh >> 6) & 0x03);
        if halftile_corner == first {
            z_second += TILE_HEIGHT_PX as i32;
        }
        if halftile_corner == second {
            z_first += TILE_HEIGHT_PX as i32;
        }
    }
    if tileh & first != 0 {
        z_first += TILE_HEIGHT_PX as i32;
    }
    if tileh & second != 0 {
        z_second += TILE_HEIGHT_PX as i32;
    }
    let without_halftile = tileh & !0xE0;
    if without_halftile == steep_first {
        z_first += TILE_HEIGHT_PX as i32;
    }
    if without_halftile == steep_second {
        z_second += TILE_HEIGHT_PX as i32;
    }
    (z_first, z_second)
}

/// Alturas bajo ambos pilares, con el mismo eje que `DrawBridgePillars`.
fn pillar_ground_heights(tileh: u8, base_z: u8, axis: usize) -> PillarGroundHeights {
    match axis {
        // `AxisToDiagDir(Axis::X) == SW`; la arista inversa es NE.
        0 => {
            let (front_south, back_south) = pillar_edge_heights(
                tileh,
                base_z,
                PILLAR_SLOPE_S,
                PILLAR_SLOPE_W,
                PILLAR_SLOPE_STEEP_S,
                PILLAR_SLOPE_STEEP_W,
            );
            let (front_north, back_north) = pillar_edge_heights(
                tileh,
                base_z,
                PILLAR_SLOPE_E,
                PILLAR_SLOPE_N,
                PILLAR_SLOPE_STEEP_E,
                PILLAR_SLOPE_STEEP_N,
            );
            PillarGroundHeights {
                front_north,
                front_south,
                back_north,
                back_south,
            }
        }
        // `AxisToDiagDir(Axis::Y) == SE`; la arista inversa es NW.
        1 => {
            let (front_south, back_south) = pillar_edge_heights(
                tileh,
                base_z,
                PILLAR_SLOPE_S,
                PILLAR_SLOPE_E,
                PILLAR_SLOPE_STEEP_S,
                PILLAR_SLOPE_STEEP_E,
            );
            let (front_north, back_north) = pillar_edge_heights(
                tileh,
                base_z,
                PILLAR_SLOPE_W,
                PILLAR_SLOPE_N,
                PILLAR_SLOPE_STEEP_W,
                PILLAR_SLOPE_STEEP_N,
            );
            PillarGroundHeights {
                front_north,
                front_south,
                back_north,
                back_south,
            }
        }
        _ => unreachable!("un puente sólo puede usar el eje X o Y"),
    }
}

/// Devuelve las columnas completas y las medias columnas terminales que
/// OpenTTD dibuja para un pilar. Incluso si el extremo superior queda bajo la
/// arista más alta, puede haber una media columna sobre la arista baja.
fn pillar_segments(z_top: i32, z_north: i32, z_south: i32) -> Vec<PillarSegment> {
    let ground = z_north.max(z_south);
    let mut z = z_top;
    let mut segments = Vec::new();
    while z >= ground {
        segments.push(PillarSegment {
            z_px: z,
            half: None,
        });
        z -= TILE_HEIGHT_PX as i32;
    }
    if z_north < ground {
        segments.push(PillarSegment {
            z_px: z,
            half: Some(PillarHalf::North),
        });
    }
    if z_south < ground {
        segments.push(PillarSegment {
            z_px: z,
            half: Some(PillarHalf::South),
        });
    }
    segments
}

/// Rectángulo de textura equivalente a los cuatro `SubSprite` de
/// `half_pillar_sub_sprite` en `tunnelbridge_cmd.cpp`.
///
/// El rectángulo es relativo al sprite, por lo que Bevy lo puede aplicar tanto
/// a una entrada del atlas como a la imagen recoloreada de un puente.
fn pillar_half_crop(
    axis: usize,
    half: PillarHalf,
    width: f32,
    height: f32,
    xrel: f32,
) -> Option<(Rect, f32)> {
    let (left, right) = match (axis, half) {
        // `{ -14, -INF, INF, INF }` y `{ -INF, -INF, -15, INF }`.
        (0, PillarHalf::North) => (Some(-14.0), None),
        (0, PillarHalf::South) => (None, Some(-15.0)),
        // `{ -INF, -INF, 15, INF }` y `{ 16, -INF, INF, INF }`.
        (1, PillarHalf::North) => (None, Some(15.0)),
        (1, PillarHalf::South) => (Some(16.0), None),
        _ => unreachable!("un puente sólo puede usar el eje X o Y"),
    };
    // `SubSprite::right` es inclusivo. Estos cálculos corresponden a
    // `clip_left` / `clip_right` de `GfxBlitter` después de aplicar `x_offs`.
    let min_x = left.map_or(0.0, |x| (x - xrel).clamp(0.0, width));
    let max_x = right.map_or(width, |x| (x - xrel + 1.0).clamp(0.0, width));
    if max_x <= min_x {
        return None;
    }
    let rect = Rect::new(min_x, 0.0, max_x, height);
    // El ancla de Bevy es el centro del recorte; OpenTTD conserva el ancla del
    // PNG completo y desplaza el blitter por `clip_left`. Compensamos esa
    // diferencia para que ambos píxeles caigan en la misma pantalla.
    let x_shift = rect.center().x - width / 2.0;
    Some((rect, x_shift))
}

/// Geometría que OpenTTD entrega a `AddSortableSpriteToDraw` para una pieza
/// estructural de puente. Mantenerla en la traza hace visible si el error está
/// en la altura, el desplazamiento frontal/trasero o los límites de sorting;
/// los PNG por sí solos no permiten distinguir esas causas.
#[derive(Clone, Copy)]
struct BridgeTracePlacement {
    world_xy_delta: (i32, i32),
    world_z_delta: i32,
    offset: (i32, i32, i32),
    bounds: TraceSpriteBounds,
}

/// Convierte la misma geometría de `AddSortableSpriteToDraw` que se exporta a
/// `world-draw` en la caja que consume el sorter runtime de Bevy.
fn bridge_parent_bounds(
    tx: i32,
    ty: i32,
    base_z: u8,
    placement: BridgeTracePlacement,
) -> ParentSpriteBounds {
    let (world_dx, world_dy) = placement.world_xy_delta;
    let world_x = tx * 16 + world_dx;
    let world_y = ty * 16 + world_dy;
    let world_z = i32::from(base_z) * 8 + placement.world_z_delta;
    let bounds = placement.bounds;
    ParentSpriteBounds::new(
        world_x + bounds.ox,
        world_y + bounds.oy,
        world_z + bounds.oz,
        world_x + bounds.ox + bounds.ex - 1,
        world_y + bounds.oy + bounds.ey - 1,
        world_z + bounds.oz + bounds.ez - 1,
    )
}

fn bridge_sortable_parent(
    ctx: &TileRenderContext,
    map_width: u32,
    sprite_id: u32,
    deck_z: u8,
    layer: f32,
    draw_ordinal: u8,
    placement: BridgeTracePlacement,
) -> ViewportSortableParent {
    let source_depth = viewport_source_depth(
        crate::iso::sortable_draw_z(ctx.tx_i32(), ctx.ty_i32(), deck_z, layer),
        ctx.tx,
        map_width,
    );
    ViewportSortableParent {
        sprite_id,
        bounds: bridge_parent_bounds(ctx.tx_i32(), ctx.ty_i32(), ctx.info.base_z, placement),
        insertion_key: viewport_insertion_key(ctx.tx, ctx.ty, draw_ordinal),
        source_depth,
    }
}

fn pillar_trace_placement(
    ctx: &TileRenderContext,
    axis: usize,
    back: bool,
    z_px: f32,
) -> BridgeTracePlacement {
    // `DrawBridgePillars`: el frente se desplaza 12 px sobre el eje que no
    // recorre el puente; el pilar trasero queda en 3 px (12 - 9). El sprite
    // usa además `DrawPillar(..., {origin={0,0,-5}, extent={w,h,6},
    // offset={0,0,5}})`.
    let along = if back { 3 } else { 12 };
    let (x, y) = if axis == 0 { (0, along) } else { (along, 0) };
    let (ex, ey) = if axis == 0 { (16, 2) } else { (2, 16) };
    BridgeTracePlacement {
        world_xy_delta: (x, y),
        world_z_delta: z_px.round() as i32 - i32::from(ctx.info.base_z) * 8,
        offset: (0, 0, 5),
        bounds: TraceSpriteBounds::new(0, 0, -5, ex, ey, 6),
    }
}

/// Geometría de las dos barandillas de `DrawBridgeMiddle`.
///
/// `psid[0]` se ancla en el borde lejano y `psid[1]` en el cercano. Ambos
/// usan `bridge_z - BRIDGE_Z_START` como origen, una caja que comienza tres
/// píxeles arriba y un offset de pantalla inverso. El frente avanza 12 px por
/// el eje transversal antes de aplicar su offset interno de 3 px.
fn bridge_middle_structure_trace_placement(
    base_z: u8,
    axis: usize,
    front: bool,
    z_px: f32,
) -> BridgeTracePlacement {
    let (world_xy_delta, bounds, offset) = match (axis, front) {
        (0, false) => (
            (0, 0),
            TraceSpriteBounds::new(0, 0, 3, 16, 1, 40),
            (0, 0, -3),
        ),
        (0, true) => (
            (0, 12),
            TraceSpriteBounds::new(0, 3, 3, 16, 1, 40),
            (0, -3, -3),
        ),
        (1, false) => (
            (0, 0),
            TraceSpriteBounds::new(0, 0, 3, 1, 16, 40),
            (0, 0, -3),
        ),
        (1, true) => (
            (12, 0),
            TraceSpriteBounds::new(3, 0, 3, 1, 16, 40),
            (-3, 0, -3),
        ),
        _ => unreachable!("un puente sólo puede usar el eje X o Y"),
    };
    BridgeTracePlacement {
        world_xy_delta,
        world_z_delta: z_px.round() as i32 - i32::from(base_z) * 8,
        offset,
        bounds,
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
    trace_bounds: Option<TraceSpriteBounds>,
    trace_placement: Option<BridgeTracePlacement>,
    map_width: u32,
    draw_ordinal: u8,
    pillar_half: Option<(usize, PillarHalf)>,
) -> Option<Entity> {
    if sprite_id == 0 {
        return None;
    }
    use crate::sprites::{TransparencyOption, sprite_color};
    let palette = bridge_structure_palette_for_sprite(bridge_type, sprite_id);
    let mut sprite = if let Some(handle) = assets.bridge_palettes.handle(sprite_id, palette) {
        Sprite {
            image: handle.clone(),
            ..default()
        }
    } else if let Some(img) = assets.bridge_sprite(sprite_id) {
        img.sprite()
    } else {
        record_bridge_structure_trace(
            ctx,
            sprite_id,
            palette.openttd_palette_id(),
            true,
            deck_z,
            trace_bounds,
            trace_placement,
        );
        return None;
    };
    // Las cabezas de rampa no traen un `M(...)` separado en el call site,
    // pero su `TraceSpriteBounds` sigue siendo la caja que OpenTTD entrega al
    // sorter. Normalizar ambos caminos a una colocación permite que también
    // las rampas participen del orden global, no sólo los vanos y pilares.
    let effective_placement = trace_placement.or_else(|| {
        trace_bounds.map(|bounds| BridgeTracePlacement {
            world_xy_delta: (0, 0),
            world_z_delta: (i32::from(deck_z) - i32::from(ctx.info.base_z)) * 8,
            offset: (0, 0, 0),
            bounds,
        })
    });
    record_bridge_structure_trace(
        ctx,
        sprite_id,
        palette.openttd_palette_id(),
        false,
        deck_z,
        trace_bounds,
        effective_placement,
    );
    sprite.color = sprite_color(TransparencyOption::Bridges);
    let (w, h, xrel, yrel) = bridge_sprite_meta(sprite_id).unwrap_or((64.0, 32.0, -32.0, -16.0));
    let crop_x_shift = if let Some((axis, half)) = pillar_half {
        let Some((rect, x_shift)) = pillar_half_crop(axis, half, w, h, xrel) else {
            // Igual que `GfxBlitter`: una subsprite vacía conserva la decisión
            // de dibujo en la traza, pero no llega a rasterizar píxeles.
            return None;
        };
        sprite.rect = Some(rect);
        x_shift
    } else {
        0.0
    };
    let pos = Vec3::new(
        ctx.iso_pos.x + shift.x + xrel + w / 2.0 + crop_x_shift,
        ctx.iso_pos.y + shift.y - yrel - h / 2.0 + z_px,
        crate::iso::sortable_draw_z(ctx.tx_i32(), ctx.ty_i32(), deck_z, layer),
    );
    let sortable_parent = effective_placement.map(|placement| {
        bridge_sortable_parent(
            ctx,
            map_width,
            sprite_id,
            deck_z,
            layer,
            draw_ordinal,
            placement,
        )
    });
    let mut entity = commands.spawn((
        MapVisualLayer,
        ctx.map_tile_chunk(),
        sprite,
        Transform::from_translation(pos),
    ));
    if let Some(parent) = sortable_parent {
        entity.insert(parent);
    }
    Some(entity.id())
}

/// Las cabezas de puente se comparan con el `AddSortableSpriteToDraw` de
/// OpenTTD. La traza debe usar la Z posterior a `DrawFoundation`, no el
/// `base_z` crudo que conserva el contexto de la tesela.
fn record_bridge_structure_trace(
    ctx: &TileRenderContext,
    sprite_id: u32,
    palette: u32,
    fallback: bool,
    surface_z: u8,
    bounds: Option<TraceSpriteBounds>,
    placement: Option<BridgeTracePlacement>,
) {
    if let Some(placement) = placement {
        WorldDrawTrace::record_sprite_with_palette_and_world_geometry(
            "bridge-structure",
            "sortable",
            sprite_id,
            palette,
            fallback,
            placement.world_xy_delta,
            placement.world_z_delta,
            placement.offset,
            Some(placement.bounds),
        );
        return;
    }
    let world_z_delta = (i32::from(surface_z) - i32::from(ctx.info.base_z)) * 8;
    WorldDrawTrace::record_sprite_with_palette_and_geometry(
        "bridge-structure",
        "sortable",
        sprite_id,
        palette,
        fallback,
        (0, 0, 0),
        world_z_delta,
        bounds,
    );
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    clippy::items_after_test_module,
    clippy::unwrap_used
)]
mod tests {
    use bevy::prelude::{Rect, Vec2};
    use openttdrs_core::{
        BridgePiece, BridgeType, Map, RailType, Tile, TileCoord, TileKind, WaterClass,
        set_bridge_middle_mapt, set_bridge_type_m6, set_water_class_m1,
    };

    use super::{
        BridgeRampGround, PILLAR_SLOPE_STEEP_W, PillarHalf, PillarSegment, RAIL_TB_X,
        bridge_catenary_wire_trace_bounds, bridge_foundation_child_offset,
        bridge_middle_structure_trace_placement, bridge_parent_bounds,
        bridge_pbs_reservation_offset, bridge_pbs_reservation_sprite_id, bridge_pbs_trace_bounds,
        bridge_ramp_catenary_slope, bridge_ramp_catenary_world_z_delta, bridge_ramp_ground_kind,
        bridge_ramp_ground_sprite_id, bridge_span_at, bridge_surface_z, catenary_under_low_bridge,
        pillar_ground_heights, pillar_half_crop, pillar_segments,
        roadside_detail_visible_under_bridge,
    };
    use crate::sprites::bridge_deck_sprite_ids;

    #[test]
    fn bridge_ramp_uses_ground_height_while_span_uses_deck_height() {
        assert_eq!(bridge_surface_z(1, 2, true), 1);
        assert_eq!(bridge_surface_z(1, 2, false), 2);
    }

    #[test]
    fn bridge_ramp_catenary_uses_tunnel_bridge_slope_query_and_rounding() {
        // Rampa plana hacia NE: `GetSlopePixelZ_TunnelBridge` convierte la
        // superficie plana en SLOPE_NE antes de ubicar cable/poste.
        assert_eq!(bridge_ramp_catenary_world_z_delta(0, 0, 0, 0, 0, 0, 8), 8);
        assert_eq!(bridge_ramp_catenary_world_z_delta(0, 0, 0, 0, 15, 0, 8), 0);
        // Los PCP se redondean a media altura de tesela, no a una completa.
        assert_eq!(bridge_ramp_catenary_world_z_delta(0, 0, 0, 0, 7, 0, 4), 4);

        // Si la fundación efectiva conserva una pendiente, la callback de
        // puente devuelve exactamente un nivel por encima de esa superficie.
        assert_eq!(bridge_ramp_catenary_world_z_delta(9, 2, 1, 0, 0, 0, 8), 16);

        // Una fundación nivelada (surface=flat, base elevada) aún asciende
        // dentro de la rampa según la dirección del puente.
        assert_eq!(bridge_ramp_catenary_world_z_delta(0, 2, 1, 2, 0, 0, 8), 8);
        assert_eq!(bridge_ramp_catenary_world_z_delta(0, 2, 1, 2, 15, 0, 8), 16);
    }

    #[test]
    fn bridge_catenary_wire_bounds_follow_the_bridge_axis() {
        let x = bridge_catenary_wire_trace_bounds(0);
        assert_eq!((x.ox, x.oy, x.oz, x.ex, x.ey, x.ez), (0, 7, 10, 15, 1, 1));

        let y = bridge_catenary_wire_trace_bounds(1);
        assert_eq!((y.ox, y.oy, y.oz, y.ex, y.ey, y.ez), (7, 0, 10, 1, 15, 1));
    }

    #[test]
    fn bridge_foundation_child_offset_matches_drawfoundation() {
        // Kale (92,148): Foundation::Leveled llama
        // OffsetGroundSprite(0, -TILE_HEIGHT), que el exportador serializa
        // multiplicado por ZOOM_BASE = 4.
        assert_eq!(
            bridge_foundation_child_offset(openttdrs_core::FOUNDATION_LEVELED),
            (0, -32, 0)
        );
        // Las únicas otras fundaciones de rampas son InclinedX/Y.
        assert_eq!(
            bridge_foundation_child_offset(openttdrs_core::FOUNDATION_INCLINED_X),
            (0, 0, 0)
        );
        assert_eq!(
            bridge_foundation_child_offset(openttdrs_core::FOUNDATION_INCLINED_Y),
            (0, 0, 0)
        );
    }

    #[test]
    fn bridge_pillars_keep_the_raw_water_slope_and_split_the_low_edge() {
        // Kale (85,140): agua con `SLOPE_STEEP_W` bajo un puente X. El suelo
        // visible se aplana, pero DrawBridgePillars conserva estas cuatro
        // alturas de GetTileSlopeZ.
        let ground = pillar_ground_heights(PILLAR_SLOPE_STEEP_W, 0, 0);
        assert_eq!(ground.front_north, 0);
        assert_eq!(ground.front_south, 8);
        assert_eq!(ground.back_north, 8);
        assert_eq!(ground.back_south, 16);

        assert_eq!(
            pillar_segments(21, ground.front_north, ground.front_south),
            vec![
                PillarSegment {
                    z_px: 21,
                    half: None,
                },
                PillarSegment {
                    z_px: 13,
                    half: None,
                },
                PillarSegment {
                    z_px: 5,
                    half: Some(PillarHalf::North),
                },
            ]
        );

        // El pilar trasero comienza dos niveles más abajo (z=5); ambas
        // aristas quedan por encima, por eso OpenTTD no lo llama.
        assert!(ground.back_north > 5 && ground.back_south > 5);
    }

    #[test]
    fn bridge_pillar_half_crops_match_openttd_subsprite_bounds() {
        // 2566 es el pilar del eje X: xrel=-31, ancho=36.
        let (north, north_shift) =
            pillar_half_crop(0, PillarHalf::North, 36.0, 25.0, -31.0).unwrap();
        assert_eq!(north, Rect::new(17.0, 0.0, 36.0, 25.0));
        assert_eq!(north_shift, 8.5);
        let (south, south_shift) =
            pillar_half_crop(0, PillarHalf::South, 36.0, 25.0, -31.0).unwrap();
        assert_eq!(south, Rect::new(0.0, 0.0, 17.0, 25.0));
        assert_eq!(south_shift, -9.5);

        // 2567 es el pilar del eje Y: xrel=-3, ancho=36.
        let (north, north_shift) =
            pillar_half_crop(1, PillarHalf::North, 36.0, 25.0, -3.0).unwrap();
        assert_eq!(north, Rect::new(0.0, 0.0, 19.0, 25.0));
        assert_eq!(north_shift, -8.5);
        let (south, south_shift) =
            pillar_half_crop(1, PillarHalf::South, 36.0, 25.0, -3.0).unwrap();
        assert_eq!(south, Rect::new(19.0, 0.0, 36.0, 25.0));
        assert_eq!(south_shift, 9.5);
    }

    #[test]
    fn bridge_rear_uses_the_transport_specific_vanilla_layer() {
        let girder = bridge_deck_sprite_ids(BridgeType::GirderSteelAlt, BridgePiece::MiddleOdd);
        assert_eq!(girder.rear_for_transport(true, RailType::Monorail, 1), 4363);
        assert_eq!(girder.rear_for_transport(true, RailType::Maglev, 0), 4402);

        let suspension = bridge_deck_sprite_ids(BridgeType::SuspensionConcrete, BridgePiece::North);
        assert_eq!(
            suspension.rear_for_transport(true, RailType::Monorail, 0),
            4338
        );
        assert_eq!(
            suspension.rear_for_transport(true, RailType::Maglev, 1),
            4374
        );
    }

    #[test]
    fn cantilever_red_rows_match_the_openttd_bridge_table() {
        // `_bridge_sprite_table_cantilever_red_{north,middle,south}`. Kale
        // usa el eje Y (índice 1), pero comprobamos ambos ejes y los tres
        // medios ferroviarios para que una variante no vuelva a degradarse a
        // rail normal o a perder el frente/pilar.
        let north = bridge_deck_sprite_ids(BridgeType::CantileverRed, BridgePiece::North);
        assert_eq!(north.rear_rail, [2509, 2510]);
        assert_eq!(north.rear_mono, [4348, 4349]);
        assert_eq!(north.rear_maglev, [4388, 4389]);
        assert_eq!(north.front, [2521, 2522]);
        assert_eq!(north.pillar, [0, 0]);

        let middle = bridge_deck_sprite_ids(BridgeType::CantileverRed, BridgePiece::MiddleOdd);
        assert_eq!(middle.rear_rail, [2508, 2511]);
        assert_eq!(middle.rear_mono, [4347, 4350]);
        assert_eq!(middle.rear_maglev, [4387, 4390]);
        assert_eq!(middle.front, [2520, 2523]);
        assert_eq!(middle.pillar, [2526, 2527]);

        let south = bridge_deck_sprite_ids(BridgeType::CantileverRed, BridgePiece::South);
        assert_eq!(south.rear_rail, [2507, 2512]);
        assert_eq!(south.rear_mono, [4346, 4351]);
        assert_eq!(south.rear_maglev, [4386, 4391]);
        assert_eq!(south.front, [2519, 2524]);
        assert_eq!(south.pillar, [2525, 2528]);
    }

    #[test]
    fn bridge_pbs_reservation_uses_the_upstream_sprite_layout() {
        // Kale (127,123): monorriel X sobre rampa con terreno efectivo 12;
        // `HasBridgeFlatRamp` es true y OpenTTD compone SPR_MONO_SINGLE_X,
        // pero exclusivamente porque la reserva PBS está activa.
        assert_eq!(
            bridge_pbs_reservation_sprite_id(RailType::Monorail, 0, true, 12, 2),
            1087
        );
        // Un vano sólo selecciona por eje.
        assert_eq!(
            bridge_pbs_reservation_sprite_id(RailType::Maglev, 1, false, 0, 0),
            1170
        );
        // Terreno plano: usa `single_sloped + dirección`, no los cuatro
        // `SINGLE_*` de monorriel. `SPR_TRACKS_FOR_SLOPES_MONO_BASE = 5405`.
        assert_eq!(
            bridge_pbs_reservation_sprite_id(RailType::Monorail, 1, true, 0, 3),
            5408
        );
        assert_eq!(
            bridge_pbs_reservation_sprite_id(RailType::Electric, 0, false, 0, 0),
            1005
        );
    }

    #[test]
    fn bridge_pbs_trace_uses_combined_bounds_from_the_effective_ramp() {
        // Vano de Kale (160,47): `DrawBridgeMiddle` compone el overlay en la
        // superficie del tablero, sin volumen vertical.
        let span = bridge_pbs_trace_bounds(false, 0, 1);
        assert_eq!(
            (span.ox, span.oy, span.oz, span.ex, span.ey, span.ez),
            (0, 0, 0, 16, 16, 0)
        );

        // Cabeza con pendiente efectiva 9: `HasBridgeFlatRamp` es true y la
        // caja empieza a una altura de tesela (`origin.z = TILE_HEIGHT`).
        let flat_ramp = bridge_pbs_trace_bounds(true, 9, 1);
        assert_eq!(
            (
                flat_ramp.ox,
                flat_ramp.oy,
                flat_ramp.oz,
                flat_ramp.ex,
                flat_ramp.ey,
                flat_ramp.ez,
            ),
            (0, 0, 8, 16, 16, 0)
        );

        // Cabeza plana: la vía asciende y ocupa el volumen vertical.
        let sloped_ramp = bridge_pbs_trace_bounds(true, 0, 0);
        assert_eq!(
            (
                sloped_ramp.ox,
                sloped_ramp.oy,
                sloped_ramp.oz,
                sloped_ramp.ex,
                sloped_ramp.ey,
                sloped_ramp.ez,
            ),
            (0, 0, 0, 16, 16, 8)
        );
    }

    #[test]
    fn bridge_middle_structure_trace_preserves_front_back_sorting_geometry() {
        // `bridge_z = 16`, `z = bridge_z - BRIDGE_Z_START = 13`; con el
        // terreno en z=8, el oráculo recibe delta de mundo +5.
        let far_x = bridge_middle_structure_trace_placement(1, 0, false, 13.0);
        assert_eq!(far_x.world_xy_delta, (0, 0));
        assert_eq!(far_x.world_z_delta, 5);
        assert_eq!(far_x.offset, (0, 0, -3));
        assert_eq!(
            (
                far_x.bounds.ox,
                far_x.bounds.oy,
                far_x.bounds.oz,
                far_x.bounds.ex,
                far_x.bounds.ey,
                far_x.bounds.ez,
            ),
            (0, 0, 3, 16, 1, 40)
        );

        let front_y = bridge_middle_structure_trace_placement(1, 1, true, 13.0);
        assert_eq!(front_y.world_xy_delta, (12, 0));
        assert_eq!(front_y.world_z_delta, 5);
        assert_eq!(front_y.offset, (-3, 0, -3));
        assert_eq!(
            (
                front_y.bounds.ox,
                front_y.bounds.oy,
                front_y.bounds.oz,
                front_y.bounds.ex,
                front_y.bounds.ey,
                front_y.bounds.ez,
            ),
            (3, 0, 3, 1, 16, 40)
        );
    }

    #[test]
    fn bridge_parent_bounds_keep_trace_world_anchor_and_extent() {
        let placement = bridge_middle_structure_trace_placement(1, 0, false, 13.0);
        let bounds = bridge_parent_bounds(10, 20, 1, placement);
        assert_eq!(
            (
                bounds.xmin,
                bounds.ymin,
                bounds.zmin,
                bounds.xmax,
                bounds.ymax,
                bounds.zmax,
            ),
            (160, 320, 16, 175, 320, 55)
        );

        let front = bridge_middle_structure_trace_placement(1, 1, true, 13.0);
        let front_bounds = bridge_parent_bounds(10, 20, 1, front);
        assert_eq!(
            (
                front_bounds.xmin,
                front_bounds.ymin,
                front_bounds.zmin,
                front_bounds.xmax,
                front_bounds.ymax,
                front_bounds.zmax,
            ),
            (175, 320, 16, 175, 335, 55)
        );
    }

    #[test]
    fn bridge_pbs_slope_overlay_restores_its_nfo_anchor() {
        // Sprite 5404: xrel=-5, yrel=0, 12×5. Frente al compuesto 64×31
        // (centro 1, 15.5) queda centrado en X y 13 px hacia arriba.
        assert_eq!(bridge_pbs_reservation_offset(5404), Vec2::new(0.0, 13.0));
        // Mono y maglev se extraen del mismo GRF, con sus propios rectángulos.
        assert_eq!(bridge_pbs_reservation_offset(5408), Vec2::new(-13.0, 6.5));
        assert_eq!(bridge_pbs_reservation_offset(5409), Vec2::new(23.0, 0.0));
        // Los SINGLE tipados se delegan al ancla que ya usa la vía plana.
        assert_eq!(bridge_pbs_reservation_offset(1087), Vec2::ZERO);
    }

    #[test]
    fn flat_bridge_ramp_uses_directional_catenary_slope() {
        assert_eq!(bridge_ramp_catenary_slope(0, 0), openttdrs_core::SLOPE_NE);
        assert_eq!(bridge_ramp_catenary_slope(0, 2), openttdrs_core::SLOPE_SW);
        assert_eq!(bridge_ramp_catenary_slope(6, 0), 0);
    }

    #[test]
    fn bridge_ramp_ground_uses_the_effective_foundation_surface_and_sea_ahead() {
        // Caso real Kale_TitleGame.sav (130,106): rampa hacia el este sobre
        // tileh 12; la fundación deja la misma pendiente a z=0 y la tesela
        // siguiente es mar. OpenTTD llama DrawShoreTile(12) = 5936 + 12.
        let ramp = TileCoord::new(1, 1);
        let sea = TileCoord::new(2, 1);
        let mut map = Map::new_flat(3, 3, 0);
        map.set_kind(sea, TileKind::Water).expect("sea kind");
        map.set_m1(sea, set_water_class_m1(0, WaterClass::Sea))
            .expect("sea class");
        let mut tile = map.get(ramp).expect("ramp tile");
        tile.m5 = 2; // dirección este (`TileOffsByDiagDir(2)`).

        assert_eq!(
            bridge_ramp_ground_kind(&map, ramp, tile, 12, 0),
            BridgeRampGround::Shore
        );
        assert_eq!(
            bridge_ramp_ground_sprite_id(BridgeRampGround::Shore, 12),
            5948
        );

        // La misma pendiente elevada ya no toca costa según la condición
        // `ti->z == 0` de OpenTTD.
        assert_eq!(
            bridge_ramp_ground_kind(&map, ramp, tile, 12, 1),
            BridgeRampGround::Grass
        );

        // MAP7 bit 5 tiene precedencia incluso si hay mar delante.
        tile.m7 = 0x20;
        assert_eq!(
            bridge_ramp_ground_kind(&map, ramp, tile, 12, 0),
            BridgeRampGround::SnowOrDesert
        );
        assert_eq!(
            bridge_ramp_ground_sprite_id(BridgeRampGround::SnowOrDesert, 12),
            4562
        );
    }

    #[test]
    fn bridge_span_uses_the_encoded_ramp_axis_not_nearby_parallel_heads() {
        // Reproduce la geometría de Kale (133,152): un puente ferroviario
        // eléctrico vertical comparte fila con dos cabezas de un puente vial.
        // Probar ambos ejes encontraba antes las cabezas viales y convertía la
        // rampa eléctrica en un falso puente horizontal, perdiendo además la
        // catenaria de la rampa.
        let mut map = Map::new_flat(7, 7, 0);
        let c = TileCoord::new;
        let bridge_type = BridgeType::SuspensionConcrete;
        let rail_ramp = |m5| Tile {
            height: 0,
            kind: TileKind::RailBridge,
            mapt: 0x90,
            m5,
            m1: 0,
            m6: set_bridge_type_m6(0, bridge_type),
            m8: RailType::Electric as u16,
            m3: 0,
            m2: 0,
            m2_hi: 0,
            m7: 0,
            m3hi: 0,
        };
        let road_ramp = |m5| Tile {
            height: 0,
            kind: TileKind::RoadBridge,
            mapt: 0x90,
            m5,
            m1: 0,
            m6: set_bridge_type_m6(0, bridge_type),
            m8: 0,
            m3: 0,
            m2: 0,
            m2_hi: 0,
            m7: 0,
            m3hi: 0,
        };
        let middle = Tile {
            height: 0,
            kind: TileKind::Water,
            mapt: set_bridge_middle_mapt(0, true),
            m5: 0,
            m1: 0,
            m6: 0,
            m8: 0,
            m3: 0,
            m2: 0,
            m2_hi: 0,
            m7: 0,
            m3hi: 0,
        };

        // Dirección 1/3 = eje Y: (3,1) → (3,4).
        map.set_tile(c(3, 1), rail_ramp(0x81)).expect("rampa norte");
        map.set_tile(c(3, 4), rail_ramp(0x83)).expect("rampa sur");
        map.set_tile(c(3, 2), middle).expect("vano uno");
        map.set_tile(c(3, 3), middle).expect("vano dos");
        // Cabezas viales adyacentes en X que no pertenecen al puente.
        map.set_tile(c(2, 1), road_ramp(0x86))
            .expect("señuelo oeste");
        map.set_tile(c(4, 1), road_ramp(0x84))
            .expect("señuelo este");

        for pos in [c(3, 1), c(3, 2), c(3, 3), c(3, 4)] {
            let span = bridge_span_at(&map, pos, map.dimensions()).expect("puente vertical");
            assert_eq!(span.axis, 1, "{pos:?} debe conservar el eje Y");
            assert!(span.rail, "{pos:?} debe conservar transporte ferroviario");
            assert!(span.electric, "{pos:?} debe conservar catenaria eléctrica");
            assert_eq!(span.rail_type, RailType::Electric);
        }
    }

    #[test]
    fn low_bridge_hides_lower_catenary_and_overrides_parallel_pcps() {
        // Un puente ferroviario horizontal a z=1 sobre una vía eléctrica a
        // nivel del suelo. OpenTTD no dibuja el cable bajo el tablero y marca
        // los PCP NE/SW (los paralelos al eje X) como override antes de elegir
        // postes. Es el caso que antes dejaba cables magenta atravesando el
        // puente en Kale_TitleGame.sav.
        let mut map = Map::new_flat(8, 8, 0);
        let c = TileCoord::new;
        let rail_ramp = |m5| Tile {
            height: 0,
            kind: TileKind::RailBridge,
            mapt: 0x90,
            m5,
            m1: 0,
            m6: set_bridge_type_m6(0, BridgeType::SuspensionConcrete),
            m8: RailType::Electric as u16,
            m3: 0,
            m2: 0,
            m2_hi: 0,
            m7: 0,
            m3hi: 0,
        };
        // Oeste hacia el este y este hacia el oeste: eje X.
        map.set_tile(c(1, 3), rail_ramp(0x82)).expect("rampa oeste");
        map.set_tile(c(4, 3), rail_ramp(0x80)).expect("rampa este");
        for x in 2..=3 {
            let mut lower_rail = map.get(c(x, 3)).expect("vía inferior");
            lower_rail.kind = TileKind::Rail;
            lower_rail.mapt = set_bridge_middle_mapt(lower_rail.mapt, false);
            lower_rail.m5 = RAIL_TB_X;
            map.set_tile(c(x, 3), lower_rail)
                .expect("vano sobre vía inferior");
        }

        let decision = catenary_under_low_bridge(&map, c(2, 3), map.dimensions());
        assert!(decision.hide_wires);
        assert_eq!(decision.pylon_pcp_override, 0b0101);

        // `DrawRoadBits` requiere una altura aún mayor que la catenaria: el
        // mismo tablero a z=1 oculta faroles y árboles de la carretera
        // inferior. Este era el origen de árboles/faroles atravesando puentes
        // bajos en la paridad de Kale.
        assert!(!roadside_detail_visible_under_bridge(
            &map,
            c(2, 3),
            map.dimensions(),
            false,
        ));
        assert!(!roadside_detail_visible_under_bridge(
            &map,
            c(2, 3),
            map.dimensions(),
            true,
        ));

        // Sin puente elevado no se cambia la decisión de catenaria.
        assert_eq!(
            catenary_under_low_bridge(&map, c(0, 0), map.dimensions()),
            super::CatenaryUnderLowBridge::default()
        );
        assert!(roadside_detail_visible_under_bridge(
            &map,
            c(0, 0),
            map.dimensions(),
            false,
        ));
    }
}

/// Dibuja tablero + barandilla + pilares para rampa o vano.
#[allow(clippy::expect_used, clippy::too_many_arguments)]
pub(crate) fn spawn_bridge_deck(
    commands: &mut Commands,
    map: &Map,
    dims: (u32, u32),
    assets: &WorldAssets,
    ctx: &TileRenderContext,
    span: &BridgeSpanInfo,
    draw_pillars: bool,
    show_pbs_reservations: bool,
    catenary_newgrf: &[Option<openttdrs_core::DecodedSprite>],
    catenary_sprites: Option<&mut NewGrfCatenarySpriteCache>,
    bridge_decks_newgrf: &[Option<openttdrs_core::DecodedSprite>],
    foundation_newgrf: &[Option<openttdrs_core::DecodedSprite>],
    mut action5_sprites: Option<&mut NewGrfAction5SpriteCache>,
    mut images: Option<&mut Assets<Image>>,
) {
    use crate::sprites::{TransparencyOption, is_hidden};
    let ids = bridge_deck_sprite_ids(span.bridge_type, span.piece);
    let on_ramp = ctx.tile.is_some_and(ramp_tile);
    // OpenTTD llama a `DrawFoundation` antes de elegir la cabeza de puente.
    // Por eso tanto el sprite como la altura de una rampa deben tomar la
    // pendiente/base efectivas y no el relieve crudo del mapa.
    let foundation_decision = ctx
        .tile
        .filter(|_| on_ramp)
        .map(|tile| bridge_foundation_decision(map, ctx, dims, tile.m5 & 0x03));
    let (foundation_tileh, foundation_z_delta) = foundation_decision.map_or_else(
        || (ctx.info.tileh, 0),
        |decision| {
            (
                decision.surface_tileh,
                decision.surface_base_z.saturating_sub(ctx.info.base_z),
            )
        },
    );
    let foundation_base_z = ctx.info.base_z.saturating_add(foundation_z_delta);

    // `DrawTile_TunnelBridge` llama a `DrawFoundation`, dibuja el suelo
    // resultante como child y recién después compone la rampa. Antes sólo
    // cambiábamos el selector de la cabeza (2450) y conservábamos el pasto de
    // la pendiente cruda: faltaba la pared 5478 y quedaba un corte visible.
    let mut foundation_child_parent = None;
    if let Some(decision) = foundation_decision {
        if decision.foundation != 0 {
            WorldDrawTrace::record_foundation(
                "bridge",
                decision.foundation,
                decision.surface_tileh,
                decision.surface_base_z,
                decision.sprite_block,
                decision.nw_edge.visible,
                decision.ne_edge.visible,
                (
                    decision.nw_edge.here.0,
                    decision.nw_edge.here.1,
                    decision.nw_edge.neighbour.0,
                    decision.nw_edge.neighbour.1,
                ),
                (
                    decision.ne_edge.here.0,
                    decision.ne_edge.here.1,
                    decision.ne_edge.neighbour.0,
                    decision.ne_edge.neighbour.1,
                ),
            );
            let plan =
                foundation_draw_plan(ctx.info.tileh, decision.foundation, decision.sprite_block);
            debug_assert_eq!(plan.surface_tileh, decision.surface_tileh);
            debug_assert_eq!(
                plan.surface_z_delta,
                decision.surface_base_z.saturating_sub(ctx.info.base_z)
            );
            for (index, draw) in plan.sprites.into_iter().flatten().enumerate() {
                if let Some(parent) = spawn_foundation_sprite(
                    commands,
                    assets,
                    ctx,
                    "bridge-foundation",
                    draw,
                    index as u8,
                    dims.0,
                    0.36 + index as f32 * 0.0005,
                    foundation_newgrf,
                    action5_sprites.as_deref_mut(),
                    images.as_deref_mut(),
                ) {
                    // DrawFoundation deja activo su último sortable; el
                    // DrawGroundSprite siguiente debe ser su screen child.
                    foundation_child_parent = Some(parent);
                }
            }
        }

        let ground_kind = bridge_ramp_ground_kind(
            map,
            ctx.coord,
            ctx.tile.expect("bridge foundation only exists on a ramp"),
            foundation_tileh,
            foundation_base_z,
        );
        let ground_id = bridge_ramp_ground_sprite_id(ground_kind, foundation_tileh);
        if decision.foundation == 0 {
            // Sin `DrawFoundation`, OpenTTD emite un ground con ancla world.
            WorldDrawTrace::record_sprite_with_geometry(
                "bridge-ramp-ground",
                "ground",
                ground_id,
                false,
                (0, 0, 0),
                0,
                None,
            );
        } else {
            // `DrawGroundSprite` se transforma en `AddChildSpriteScreen`.
            // La posición Bevy ya equivale a la superficie efectiva; la traza
            // conserva el offset relativo al padre para contrastarlo.
            WorldDrawTrace::record_foundation_child_sprite(
                "bridge-foundation-ground",
                ground_id,
                false,
                bridge_foundation_child_offset(decision.foundation),
            );
        }
        let ground = match ground_kind {
            BridgeRampGround::Grass => {
                sloped_or_flat_image(foundation_tileh, &assets.grass, &assets.grass_slopes)
            }
            BridgeRampGround::Shore => {
                let shore = crate::sprites::TILEH_TO_SHORE_SPRITE[usize::from(foundation_tileh)];
                assets.shore[usize::from(shore)].clone()
            }
            // `SPR_FLAT_SNOW_DESERT_TILE + SlopeToSpriteOffset(tileh)`.
            BridgeRampGround::SnowOrDesert => {
                assets.snow_desert[3][usize::from(slope_sprite_offset(foundation_tileh))].clone()
            }
        };
        let half_h = if foundation_tileh == 0 {
            TILE_HALF_H
        } else {
            slope_half_h(foundation_tileh)
        };
        if let Some(parent) = foundation_child_parent {
            spawn_foundation_child_ground_sprite_at(
                commands,
                &ground,
                Color::WHITE,
                ctx,
                foundation_base_z,
                DECK_LAYER_FRAC - 0.001,
                half_h,
                dims.0,
                parent,
            );
        } else {
            spawn_ground_sprite_at(
                commands,
                &ground,
                Color::WHITE,
                ctx,
                foundation_base_z,
                DECK_LAYER_FRAC - 0.001,
                half_h,
            );
        }
    }

    // Aunque el usuario oculte los puentes, el suelo y la fundación siguen
    // perteneciendo a la tesela. La transparencia sólo debe omitir estructura,
    // barandilla, pilares y catenaria.
    if is_hidden(TransparencyOption::Bridges) {
        return;
    }
    let ramp_id = ctx.tile.filter(|_| on_ramp).map(|tile| {
        bridge_ramp_sprite_id(
            span.bridge_type,
            span.rail,
            span.rail_type,
            foundation_tileh,
            tile.m5,
        )
    });
    let rear_id =
        ramp_id.unwrap_or_else(|| ids.rear_for_transport(span.rail, span.rail_type, span.axis));
    let front_id = if on_ramp { 0 } else { ids.front[span.axis] };
    let pillar_id = if on_ramp { 0 } else { ids.pillar[span.axis] };
    let surface_z = bridge_surface_z(foundation_base_z, span.deck_z, on_ramp);
    let z_draw_px = f32::from(surface_z) * HEIGHT_PX - BRIDGE_Z_START;

    if !on_ramp {
        // `DrawBridgeMiddle` inserta el helper antes de comenzar el bloque
        // combinado del tablero. Debe existir aun sin píxeles propios, porque
        // vecinos (casas, foundations, señales) sí participan del sorter.
        spawn_empty_bounding_box(
            commands,
            ctx,
            "bridge-sort-separator",
            bridge_sort_separator_bounds(),
            bridge_sort_separator_world_z_delta(ctx.info.base_z, span.deck_z),
            0,
            dims.0,
            span.deck_z,
            DECK_LAYER_FRAC - 0.0005,
        );
    }

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

    let rear_parent = spawn_layer(
        commands,
        assets,
        ctx,
        rear_id,
        Vec2::ZERO,
        z_draw_px,
        DECK_LAYER_FRAC,
        surface_z,
        span.bridge_type,
        on_ramp.then(|| {
            TraceSpriteBounds::new(
                0,
                0,
                0,
                16,
                16,
                if foundation_tileh == 0 {
                    0
                } else {
                    TILE_HEIGHT_PX as i32
                },
            )
        }),
        (!on_ramp).then(|| {
            bridge_middle_structure_trace_placement(ctx.info.base_z, span.axis, false, z_draw_px)
        }),
        dims.0,
        BRIDGE_REAR_ORDINAL,
        None,
    );
    // Una superficie Action5 `0x1B` se compone sobre la estructura en un
    // vano. Las vías vanilla no agregan una capa de tablero separada: los
    // `SINGLE_*` de rail/mono/maglev se reservan para PBS más abajo.
    let action5_surface_slot = (!on_ramp)
        .then(|| openttdrs_core::bridge_decks_action5_slot(span.rail, span.rail_type, span.axis))
        .flatten();
    // Superficie Action5 `0x1B` (tablero NewGRF sobre la estructura OpenGFX).
    if let Some(slot) = action5_surface_slot
        && let (Some(cache), Some(images)) = (action5_sprites.as_mut(), images.as_mut())
        && let Some(sprite) = cache.sprite_colored(
            openttdrs_core::ACTION5_TYPE_BRIDGE_DECKS,
            slot,
            bridge_decks_newgrf,
            Color::WHITE,
            images,
        )
    {
        let position = tile_pos_half(
            ctx.tx_i32(),
            ctx.ty_i32(),
            surface_z,
            DECK_LAYER_FRAC + 0.001,
            TILE_HALF_H,
        );
        WorldDrawTrace::record_sprite_with_palette_and_geometry(
            "bridge-deck-overlay",
            "combined",
            SPR_BRIDGE_DECKS_BASE.saturating_add(u32::try_from(slot).unwrap_or(0)),
            0,
            false,
            (0, 0, 0),
            (i32::from(surface_z) - i32::from(ctx.info.base_z)) * 8,
            Some(TraceSpriteBounds::new(0, 0, 0, 16, 16, 0)),
        );
        if let Some(parent) = rear_parent {
            spawn_bridge_combined_child(commands, ctx, dims.0, parent, sprite, position);
        } else {
            commands.spawn((
                MapVisualLayer,
                ctx.map_tile_chunk(),
                sprite,
                Transform::from_translation(position),
            ));
        }
    }
    if span.rail && show_pbs_reservations && span.pbs_reserved {
        spawn_bridge_pbs_reservation(
            commands,
            assets,
            ctx,
            span,
            on_ramp,
            foundation_tileh,
            surface_z,
            dims.0,
            rear_parent,
        );
    }
    // Overlay de tranvía sobre tablero de puente de carretera.
    if !span.rail
        && let Some(tile) = ctx.tile
        && let Some(tfi) = crate::sprites::tram_flat_sprite_index(0, tile.m3)
        && let Some(tram) = assets.tram_flat.get(tfi)
    {
        let position = tile_pos_half(
            ctx.tx_i32(),
            ctx.ty_i32(),
            surface_z,
            RAIL_ON_BRIDGE_LAYER_FRAC,
            TILE_HALF_H,
        );
        WorldDrawTrace::record_sprite_with_geometry(
            "bridge-tram-overlay",
            "combined",
            SPR_TRAMWAY_OVERLAY_BASE.saturating_add(u32::try_from(tfi).unwrap_or(0)),
            false,
            (0, 0, 0),
            (i32::from(surface_z) - i32::from(ctx.info.base_z)) * 8,
            Some(TraceSpriteBounds::new(0, 0, 0, 16, 16, 0)),
        );
        if let Some(parent) = rear_parent {
            spawn_bridge_combined_child(commands, ctx, dims.0, parent, tram.sprite(), position);
        } else {
            commands.spawn((
                MapVisualLayer,
                ctx.map_tile_chunk(),
                tram.sprite(),
                Transform::from_translation(position),
            ));
        }
    }
    if span.electric {
        if on_ramp {
            spawn_bridge_ramp_catenary(
                commands,
                map,
                dims,
                assets,
                ctx,
                span,
                dims.0,
                foundation_tileh,
                foundation_base_z,
                catenary_newgrf,
                catenary_sprites,
                images,
            );
        } else if span.middle_num > 0 && span.middle_length > 0 {
            spawn_bridge_catenary(
                commands,
                assets,
                ctx,
                span,
                dims.0,
                catenary_newgrf,
                catenary_sprites,
                images,
            );
        }
    }
    spawn_layer(
        commands,
        assets,
        ctx,
        front_id,
        front_shift,
        z_draw_px,
        FRONT_LAYER_FRAC,
        surface_z,
        span.bridge_type,
        None,
        (!on_ramp).then(|| {
            bridge_middle_structure_trace_placement(ctx.info.base_z, span.axis, true, z_draw_px)
        }),
        dims.0,
        BRIDGE_FRONT_ORDINAL,
        None,
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
    // `DrawBridgePillars` recibe el `TileInfo` después de `DrawFoundation`.
    // La misma superficie efectiva preserva las fundaciones ferroviarias y,
    // para agua sin fundación, la pendiente cruda que el suelo visual aplana.
    let (pillar_tileh, pillar_base_z) =
        foundation_surface_at(map, ctx.coord, dims).unwrap_or((ctx.info.tileh, ctx.info.base_z));
    let ground = pillar_ground_heights(pillar_tileh, pillar_base_z, span.axis);
    if pillar_id != 0 && bridge_sprite_meta(pillar_id).is_some() {
        for segment in pillar_segments(
            z_draw_px.round() as i32,
            ground.front_north,
            ground.front_south,
        ) {
            spawn_layer(
                commands,
                assets,
                ctx,
                pillar_id,
                front_shift,
                segment.z_px as f32,
                PILLAR_LAYER_FRAC,
                span.deck_z,
                span.bridge_type,
                None,
                Some(pillar_trace_placement(
                    ctx,
                    span.axis,
                    false,
                    segment.z_px as f32,
                )),
                dims.0,
                BRIDGE_PILLAR_ORDINAL_BASE,
                segment.half.map(|half| (span.axis, half)),
            );
        }
        let back_top_px = z_draw_px.round() as i32 - 2 * TILE_HEIGHT_PX as i32;
        if ground.back_north <= back_top_px || ground.back_south <= back_top_px {
            for segment in pillar_segments(back_top_px, ground.back_north, ground.back_south) {
                spawn_layer(
                    commands,
                    assets,
                    ctx,
                    pillar_id,
                    back_shift,
                    segment.z_px as f32,
                    PILLAR_BACK_LAYER_FRAC,
                    span.deck_z,
                    span.bridge_type,
                    None,
                    Some(pillar_trace_placement(
                        ctx,
                        span.axis,
                        true,
                        segment.z_px as f32,
                    )),
                    dims.0,
                    BRIDGE_PILLAR_ORDINAL_BASE + 1,
                    segment.half.map(|half| (span.axis, half)),
                );
            }
        }
    }
}
