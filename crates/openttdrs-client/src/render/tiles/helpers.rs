use bevy::prelude::*;

use crate::iso::{
    HEIGHT_PX, TILE_HALF_H, ground_tile_pos_half, overlay_pos, slope_sprite_offset, tile_pos,
    tile_pos_half,
};
use crate::render::world_draw_trace::WorldDrawTrace;
use crate::render::{
    AtlasSprite, MapTileChunk, MapVisualLayer, TileRenderContext, WaterTile, WorldAssets,
};
use crate::sprites::{foundation_gfx_for_tileh, rail_trackbits_for_render};
use openttdrs_core::{
    FOUNDATION_ORIGINAL_SPRITE_BASE, Map, RailFoundationSpriteDraw, TileCoord, TileKind,
    foundation_draw_plan, rail_foundation_draw_plan, rail_foundation_for_trackbits,
    rail_surface_slope_and_z, tile_slope_and_z,
};

/// Sesgo en la componente Z de **solo** el agua animada (sin sprite `shore_*`).
/// El orden de dibujo usa `(tx+ty)`; el mar al **este/sur** tiene suma mayor y acaba
/// encima del borde costero del vecino NO/NE → sierra y rectángulos azules oscuros.
pub(crate) const FLAT_WATER_LAYER_FRAC: f32 = -0.030;
/// Costa entre tierra y agua: debe tapar agua vecina, pero no pintar su parte azul
/// encima de la tierra que queda del lado interior de la orilla.
pub(crate) const SHORE_LAYER_FRAC: f32 = -0.015;
/// Capa de tranvía (`tram_flat_*`, SPR_TRAMWAY_OVERLAY) por encima del asfalto.
pub(crate) const TRAM_OVERLAY_LAYER_FRAC: f32 = 0.028;

pub(crate) fn sloped_or_flat_image(
    tileh: u8,
    flat: &AtlasSprite,
    slopes: &[AtlasSprite],
) -> AtlasSprite {
    let offset = slope_sprite_offset(tileh);
    offset
        .checked_sub(1)
        .and_then(|index| slopes.get(usize::from(index)))
        .cloned()
        .unwrap_or_else(|| flat.clone())
}

/// Posición de overlay tras `DrawFoundation(FOUNDATION_LEVELED)` + `OffsetGroundSprite(0, -8)`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn leveled_foundation_overlay_pos(
    ref_pos: Vec2,
    xrel: f32,
    yrel: f32,
    w: f32,
    h: f32,
    base_z: u8,
    layer: f32,
    tx: i32,
    ty: i32,
) -> Vec3 {
    let mut pos = overlay_pos(
        ref_pos,
        xrel,
        yrel,
        w,
        h,
        base_z.saturating_add(1),
        layer,
        tx,
        ty,
    );
    pos.y -= HEIGHT_PX;
    pos
}

pub(crate) fn spawn_leveled_foundation(
    commands: &mut Commands,
    assets: &WorldAssets,
    ctx: &TileRenderContext,
    tileh: u8,
    _foundation_newgrf: &[Option<openttdrs_core::DecodedSprite>],
    _action5_sprites: Option<&mut crate::render::NewGrfAction5SpriteCache>,
    _images: Option<&mut Assets<Image>>,
) {
    let Some(gfx) = foundation_gfx_for_tileh(tileh) else {
        return;
    };
    let pos = overlay_pos(
        ctx.iso_pos,
        gfx.xrel,
        gfx.yrel,
        gfx.w,
        gfx.h,
        ctx.info.base_z,
        0.36,
        ctx.tx_i32(),
        ctx.ty_i32(),
    );
    let Some(img) = assets.foundations.get((tileh - 1) as usize) else {
        return;
    };
    commands.spawn((
        MapVisualLayer,
        ctx.map_tile_chunk(),
        img.sprite(),
        Transform::from_translation(pos),
    ));
}

const SLOPE_STEEP: u8 = 0x10;
const SLOPE_HALFTILE: u8 = 0x20;
const SLOPE_W: u8 = 0x01;
const SLOPE_S: u8 = 0x02;
const SLOPE_E: u8 = 0x04;
const SLOPE_N: u8 = 0x08;
const SLOPE_STEEP_W: u8 = SLOPE_STEEP | SLOPE_N | SLOPE_W | SLOPE_S;
const SLOPE_STEEP_S: u8 = SLOPE_STEEP | SLOPE_W | SLOPE_S | SLOPE_E;
const SLOPE_STEEP_E: u8 = SLOPE_STEEP | SLOPE_S | SLOPE_E | SLOPE_N;
const SLOPE_STEEP_N: u8 = SLOPE_STEEP | SLOPE_E | SLOPE_N | SLOPE_W;

/// `_invalid_tileh_slopes_road[0]` de `road_cmd.cpp`.
const ROAD_INVALID_ON_LEVELED: [u8; 15] = [
    0x00, 0x0C, 0x09, 0x08, 0x03, 0x00, 0x01, 0x00, 0x06, 0x04, 0x00, 0x00, 0x02, 0x00, 0x00,
];
/// `_invalid_tileh_slopes_road[1]` de `road_cmd.cpp`.
const ROAD_INVALID_STRAIGHT: [u8; 15] = [
    0x00, 0x00, 0x00, 0x05, 0x00, 0x0F, 0x0A, 0x0F, 0x00, 0x0A, 0x0F, 0x0F, 0x05, 0x0F, 0x0F,
];
const ROAD_X: u8 = 0x0A;

#[derive(Clone, Copy)]
enum FoundationEdge {
    Ne,
    Se,
    Sw,
    Nw,
}

/// Resultado completo de la comparación que `HasFoundationNW/NE` reduce a
/// un booleano. Mantener las cuatro alturas es crucial para diagnosticar una
/// orientación invertida o una fundación vecina equivocada.
#[derive(Clone, Copy, Debug)]
pub(crate) struct FoundationEdgeComparison {
    pub(crate) visible: bool,
    pub(crate) here: (i32, i32),
    pub(crate) neighbour: (i32, i32),
}

impl FoundationEdge {
    const fn opposite(self) -> Self {
        match self {
            Self::Ne => Self::Sw,
            Self::Se => Self::Nw,
            Self::Sw => Self::Ne,
            Self::Nw => Self::Se,
        }
    }
}

/// Réplica de `GetSlopePixelZOnEdge`. El par conserva el orden de OpenTTD:
/// la esquina cercana a cámara primero, la lejana después.
fn foundation_edge_heights(tileh: u8, base_z: u8, edge: FoundationEdge) -> (i32, i32) {
    // `GetHalftileSlopeCorner` devuelve el índice de esquina (W, S, E, N),
    // no la máscara de pendiente. Conservamos ambos porque el resto del
    // algoritmo sí opera con máscaras de `Slope`.
    let (first, second, first_corner, second_corner) = match edge {
        FoundationEdge::Ne => (0x04, 0x08, 2, 3), // E, N
        FoundationEdge::Se => (0x02, 0x04, 1, 2), // S, E
        FoundationEdge::Sw => (0x02, 0x01, 1, 0), // S, W
        FoundationEdge::Nw => (0x01, 0x08, 0, 3), // W, N
    };
    let mut z_first = i32::from(base_z) * 8;
    let mut z_second = z_first;
    if tileh & SLOPE_HALFTILE != 0 {
        let halftile_corner = (tileh >> 6) & 0x03;
        if halftile_corner == first_corner {
            z_second += 8;
        }
        if halftile_corner == second_corner {
            z_first += 8;
        }
    }
    if tileh & first != 0 {
        z_first += 8;
    }
    if tileh & second != 0 {
        z_second += 8;
    }
    let steep_first = match first {
        SLOPE_W => SLOPE_STEEP_W,
        SLOPE_S => SLOPE_STEEP_S,
        SLOPE_E => SLOPE_STEEP_E,
        SLOPE_N => SLOPE_STEEP_N,
        _ => unreachable!("una arista sólo contiene una esquina válida"),
    };
    let steep_second = match second {
        SLOPE_W => SLOPE_STEEP_W,
        SLOPE_S => SLOPE_STEEP_S,
        SLOPE_E => SLOPE_STEEP_E,
        SLOPE_N => SLOPE_STEEP_N,
        _ => unreachable!("una arista sólo contiene una esquina válida"),
    };
    let without_halftile = tileh & !0xE0;
    if without_halftile == steep_first {
        z_first += 8;
    }
    if without_halftile == steep_second {
        z_second += 8;
    }
    (z_first, z_second)
}

/// Réplica de `GetBridgeFoundation` + `ApplyFoundationToSlope` para una
/// rampa de puente. `direction` es el `DiagDirection` persistido en `m5`.
///
/// `GetFoundationPixelSlope` usa esta superficie, no el terreno crudo, al
/// decidir qué paredes de una fundación vecina deben ser visibles. Ignorarla
/// hacía que una rampa inclinada pareciera más baja y seleccionáramos el
/// bloque clásico (`996`) donde OpenTTD usa el bloque Action5 (`5427`).
pub(crate) fn bridge_foundation_kind(tileh: u8, direction: u8) -> u8 {
    openttdrs_core::bridge_foundation_for_axis(tileh, direction & 1 == 0)
}

/// Réplica de `ApplyFoundationToSlope(GetBridgeFoundation(...))` para una
/// rampa de puente. `direction` es el `DiagDirection` persistido en `m5`.
pub(crate) fn bridge_foundation_surface(tileh: u8, direction: u8) -> (u8, u8) {
    openttdrs_core::bridge_surface_slope_and_z(tileh, direction & 1 == 0)
}

/// Máscara de la esquina más alta en las únicas pendientes que la tienen de
/// forma inequívoca: una sola esquina o una pendiente `SLOPE_STEEP`.
///
/// Esto replica `HasSlopeHighestCorner` / `GetHighestSlopeCorner`: una
/// pendiente de tres esquinas elevadas (por ejemplo `SLOPE_ENW` = `0x0E`)
/// tiene una esquina *baja*, no una alta, y debe recibir una fundación
/// nivelada.
fn highest_slope_corner_mask(tileh: u8) -> Option<u8> {
    match tileh & !0xE0 {
        SLOPE_W | SLOPE_STEEP_W => Some(SLOPE_W),
        SLOPE_S | SLOPE_STEEP_S => Some(SLOPE_S),
        SLOPE_E | SLOPE_STEEP_E => Some(SLOPE_E),
        SLOPE_N | SLOPE_STEEP_N => Some(SLOPE_N),
        _ => None,
    }
}

/// Réplica de `GetRoadFoundation` para una vía normal.
///
/// `road_bits` debe incluir carretera y tranvía (`GetAllRoadBits`). Mantener
/// esta selección separada de la superficie efectiva permite que el renderer
/// dibuje el mismo `DrawFoundation` que OpenTTD, no sólo su resultado lógico.
pub(crate) fn road_foundation_kind(tileh: u8, road_bits: u8) -> u8 {
    let road_bits = road_bits & 0x0F;
    if tileh == 0 || road_bits == 0 {
        return 0;
    }

    // Para decidir el tipo de fundación OpenTTD reduce una pendiente empinada
    // a su esquina alta; al aplicarla conserva el incremento de altura extra.
    let rule_tileh = if tileh & SLOPE_STEEP != 0 {
        highest_slope_corner_mask(tileh).unwrap_or(0)
    } else {
        tileh
    };
    let index = usize::from(rule_tileh.min(14));
    if road_bits & ROAD_INVALID_ON_LEVELED[index] == 0 {
        return openttdrs_core::FOUNDATION_LEVELED;
    }
    if highest_slope_corner_mask(rule_tileh).is_none()
        && road_bits & ROAD_INVALID_STRAIGHT[index] == 0
    {
        return 0;
    }

    if road_bits == ROAD_X {
        openttdrs_core::FOUNDATION_INCLINED_X
    } else {
        // Cualquier combinación que no sea la recta X se trata como la recta
        // Y en `GetRoadFoundation`.
        openttdrs_core::FOUNDATION_INCLINED_Y
    }
}

/// Réplica de `GetRoadFoundation` + `ApplyFoundationToSlope` para una vía
/// normal. La misma rutina genérica de core que usa `DrawFoundation` evita
/// que la pendiente efectiva diverja de los sprites de fundación.
pub(crate) fn road_foundation_surface(tileh: u8, road_bits: u8) -> (u8, u8) {
    let plan = foundation_draw_plan(tileh, road_foundation_kind(tileh, road_bits), 0);
    (plan.surface_tileh, plan.surface_z_delta)
}

/// `FlatteningFoundation(tileh)` + `ApplyFoundationToSlope`.
///
/// Casas, estaciones, industrias y depósitos no exponen el relieve crudo a
/// `GetFoundationPixelSlope`: nivelan la tesela completa, incluyendo el nivel
/// adicional de una pendiente empinada.
const fn flattening_foundation_surface(tileh: u8) -> (u8, u8) {
    if tileh == 0 {
        (0, 0)
    } else {
        let steep_extra = if tileh & SLOPE_STEEP != 0 { 1 } else { 0 };
        (0, 1 + steep_extra)
    }
}

/// Pendiente efectiva sobre la fundación de una tesela.
///
/// Además de `HasFoundationNW/NE`, la usan elementos superpuestos —como los
/// pilares de un puente elevado— porque OpenTTD recibe un `TileInfo` ya
/// modificado por `DrawFoundation`. Las estaciones vanilla son niveladas;
/// custom foundations se dejan para la traza de estaciones separada.
pub(crate) fn foundation_surface_at(
    map: &Map,
    coord: TileCoord,
    map_dims: (u32, u32),
) -> Option<(u8, u8)> {
    let tile = map.get(coord)?;
    let (tileh, base_z) = tile_slope_and_z(map, coord)?;
    match tile.kind {
        TileKind::Rail => {
            let trackbits = rail_trackbits_for_render(map, coord, map_dims.0, map_dims.1);
            let (surface, z_delta) = rail_surface_slope_and_z(tileh, trackbits);
            Some((surface, base_z.saturating_add(z_delta)))
        }
        TileKind::RailBridge | TileKind::RoadBridge => {
            let (surface, z_delta) = bridge_foundation_surface(tileh, tile.m5 & 0x03);
            Some((surface, base_z.saturating_add(z_delta)))
        }
        // `GetFoundation_Rail/Road` usa `FlatteningFoundation` para depósitos
        // y cruces. Las procs vanilla de casas, estaciones e industrias hacen
        // lo mismo. Para NewGRF con callback que omite fundación queda una
        // aproximación conservadora; los saves de OpenGFX no tienen ese caso.
        TileKind::RailDepot
        | TileKind::RoadDepot
        | TileKind::House
        | TileKind::Station
        | TileKind::Industry
        | TileKind::Airport => {
            let (surface, z_delta) = flattening_foundation_surface(tileh);
            Some((surface, base_z.saturating_add(z_delta)))
        }
        // `RoadTileType::Crossing` no es una carretera normal y también se
        // aplana.
        TileKind::Road if (tile.m5 >> 6) & 0x03 != 0 => {
            let (surface, z_delta) = flattening_foundation_surface(tileh);
            Some((surface, base_z.saturating_add(z_delta)))
        }
        TileKind::Road => {
            let road_bits = (tile.m5 | tile.m3) & 0x0F;
            let (surface, z_delta) = road_foundation_surface(tileh, road_bits);
            Some((surface, base_z.saturating_add(z_delta)))
        }
        _ => Some((tileh, base_z)),
    }
}

fn has_foundation_edge(
    map: &Map,
    coord: TileCoord,
    map_dims: (u32, u32),
    current_slope: u8,
    current_z: u8,
    edge: FoundationEdge,
) -> FoundationEdgeComparison {
    let neighbour = match edge {
        FoundationEdge::Nw => TileCoord::new(coord.x, coord.y - 1),
        FoundationEdge::Ne => TileCoord::new(coord.x - 1, coord.y),
        FoundationEdge::Se => TileCoord::new(coord.x, coord.y + 1),
        FoundationEdge::Sw => TileCoord::new(coord.x + 1, coord.y),
    };
    let (neighbour_slope, neighbour_z) =
        foundation_surface_at(map, neighbour, map_dims).unwrap_or((0, 0));
    let here = foundation_edge_heights(current_slope, current_z, edge);
    let there = foundation_edge_heights(neighbour_slope, neighbour_z, edge.opposite());
    FoundationEdgeComparison {
        visible: here.0 > there.0 || here.1 > there.1,
        here,
        neighbour: there,
    }
}

/// Selecciona el bloque de cimientos 0..3 exactamente como `DrawFoundation`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct FoundationDecision {
    pub(crate) foundation: u8,
    pub(crate) surface_tileh: u8,
    pub(crate) surface_base_z: u8,
    pub(crate) sprite_block: u8,
    pub(crate) nw_edge: FoundationEdgeComparison,
    pub(crate) ne_edge: FoundationEdgeComparison,
}

/// Calcula el mismo bloque que `DrawFoundation` y conserva sus dos
/// comparaciones de borde para la traza de paridad. Mantener la decisión como
/// dato evita que la selección de sprite y el diagnóstico recalculen caminos
/// distintos.
fn foundation_decision(
    map: &Map,
    ctx: &TileRenderContext,
    map_dims: (u32, u32),
    foundation: u8,
    surface: u8,
    z_delta: u8,
) -> FoundationDecision {
    let current_z = ctx.info.base_z.saturating_add(z_delta);
    let nw_edge = has_foundation_edge(
        map,
        ctx.coord,
        map_dims,
        surface,
        current_z,
        FoundationEdge::Nw,
    );
    let ne_edge = has_foundation_edge(
        map,
        ctx.coord,
        map_dims,
        surface,
        current_z,
        FoundationEdge::Ne,
    );
    FoundationDecision {
        foundation,
        surface_tileh: surface,
        surface_base_z: current_z,
        sprite_block: u8::from(!nw_edge.visible) + 2 * u8::from(!ne_edge.visible),
        nw_edge,
        ne_edge,
    }
}

fn rail_foundation_decision(
    map: &Map,
    ctx: &TileRenderContext,
    map_dims: (u32, u32),
    tileh: u8,
    trackbits: u8,
) -> FoundationDecision {
    let (surface, z_delta) = rail_surface_slope_and_z(tileh, trackbits);
    foundation_decision(
        map,
        ctx,
        map_dims,
        rail_foundation_for_trackbits(tileh, trackbits),
        surface,
        z_delta,
    )
}

/// Decisión de `DrawFoundation(GetRoadFoundation(...))` para una carretera
/// normal. `DrawRoadBits` modifica el `TileInfo` antes de elegir los sprites
/// de carretera, tranvía, flechas y detalles; por eso este resultado se
/// comparte entre el cimiento y todas las capas posteriores.
pub(crate) fn road_foundation_decision(
    map: &Map,
    ctx: &TileRenderContext,
    map_dims: (u32, u32),
    tileh: u8,
    road_bits: u8,
) -> FoundationDecision {
    let foundation = road_foundation_kind(tileh, road_bits);
    let (surface, z_delta) = road_foundation_surface(tileh, road_bits);
    foundation_decision(map, ctx, map_dims, foundation, surface, z_delta)
}

/// Decisión de `DrawFoundation(GetBridgeFoundation(...))` para una rampa.
///
/// La usan tanto el render como la traza: la elección de bloque 0..3 debe
/// depender de las mismas superficies vecinas efectivas que OpenTTD.
pub(crate) fn bridge_foundation_decision(
    map: &Map,
    ctx: &TileRenderContext,
    map_dims: (u32, u32),
    direction: u8,
) -> FoundationDecision {
    let foundation = bridge_foundation_kind(ctx.info.tileh, direction);
    let (surface, z_delta) = bridge_foundation_surface(ctx.info.tileh, direction);
    foundation_decision(map, ctx, map_dims, foundation, surface, z_delta)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn spawn_foundation_sprite(
    commands: &mut Commands,
    assets: &WorldAssets,
    ctx: &TileRenderContext,
    role: &'static str,
    draw: RailFoundationSpriteDraw,
    layer: f32,
    foundation_newgrf: &[Option<openttdrs_core::DecodedSprite>],
    action5_sprites: Option<&mut crate::render::NewGrfAction5SpriteCache>,
    images: Option<&mut Assets<Image>>,
) {
    if let Some(tileh) = draw
        .sprite_id
        .checked_sub(FOUNDATION_ORIGINAL_SPRITE_BASE)
        .and_then(|value| u8::try_from(value).ok())
        .filter(|tileh| (1..=14).contains(tileh))
    {
        let missing = foundation_gfx_for_tileh(tileh).is_none()
            || assets.foundations.get(usize::from(tileh - 1)).is_none();
        WorldDrawTrace::record_sprite(role, "sortable", draw.sprite_id, missing);
        let (Some(gfx), Some(image)) = (
            foundation_gfx_for_tileh(tileh),
            assets.foundations.get(usize::from(tileh - 1)),
        ) else {
            return;
        };
        let pos = overlay_pos(
            ctx.iso_pos,
            gfx.xrel,
            gfx.yrel,
            gfx.w,
            gfx.h,
            ctx.info.base_z.saturating_add(draw.z_delta),
            layer,
            ctx.tx_i32(),
            ctx.ty_i32(),
        );
        commands.spawn((
            MapVisualLayer,
            ctx.map_tile_chunk(),
            image.sprite(),
            Transform::from_translation(pos),
        ));
        return;
    }

    let Some(slot) = openttdrs_core::foundation_action5_slot_for_sprite_id(draw.sprite_id) else {
        WorldDrawTrace::record_sprite(role, "sortable", draw.sprite_id, true);
        return;
    };
    let Some(decoded) = foundation_newgrf.get(slot).and_then(Option::as_ref) else {
        WorldDrawTrace::record_sprite(role, "sortable", draw.sprite_id, true);
        return;
    };
    let pos = overlay_pos(
        ctx.iso_pos,
        f32::from(decoded.x_offs),
        f32::from(decoded.y_offs),
        f32::from(decoded.width),
        f32::from(decoded.height),
        ctx.info.base_z.saturating_add(draw.z_delta),
        layer,
        ctx.tx_i32(),
        ctx.ty_i32(),
    );
    let sprite = action5_sprites.zip(images).and_then(|(cache, images)| {
        cache.sprite_colored(
            openttdrs_core::ACTION5_TYPE_FOUNDATIONS,
            slot,
            foundation_newgrf,
            Color::WHITE,
            images,
        )
    });
    WorldDrawTrace::record_sprite(role, "sortable", draw.sprite_id, sprite.is_none());
    let Some(sprite) = sprite else {
        return;
    };
    commands.spawn((
        MapVisualLayer,
        ctx.map_tile_chunk(),
        sprite,
        Transform::from_translation(pos),
    ));
}

/// Cimiento bajo vía/estación en pendiente. Replica la selección de sprites
/// de `DrawTrackBits` / `DrawFoundation` y devuelve el `base_z` que deben usar
/// las capas ferroviarias encima del cimiento.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub(crate) fn spawn_rail_foundation(
    commands: &mut Commands,
    map: &Map,
    map_dims: (u32, u32),
    assets: &WorldAssets,
    ctx: &TileRenderContext,
    tileh: u8,
    trackbits: u8,
    foundation_newgrf: &[Option<openttdrs_core::DecodedSprite>],
    action5_sprites: Option<&mut crate::render::NewGrfAction5SpriteCache>,
    images: Option<&mut Assets<Image>>,
) -> u8 {
    if tileh == 0 {
        return ctx.info.base_z;
    }
    let decision = rail_foundation_decision(map, ctx, map_dims, tileh, trackbits);
    if decision.foundation != 0 && decision.foundation != u8::MAX {
        WorldDrawTrace::record_foundation(
            "rail",
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
    }
    let plan = rail_foundation_draw_plan(tileh, trackbits, decision.sprite_block);
    let mut action5_sprites = action5_sprites;
    let mut images = images;
    for (index, draw) in plan.sprites.into_iter().flatten().enumerate() {
        spawn_foundation_sprite(
            commands,
            assets,
            ctx,
            "rail-foundation",
            draw,
            0.36 + index as f32 * 0.0005,
            foundation_newgrf,
            action5_sprites.as_deref_mut(),
            images.as_deref_mut(),
        );
    }
    ctx.info.base_z.saturating_add(plan.surface_z_delta)
}

/// Superficie que deja `DrawFoundation(GetRoadFoundation(...))` para una
/// carretera. Además del tipo de fundación conserva la pendiente y Z
/// efectivas que deben usar los draws posteriores de `DrawRoadBits`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RoadFoundationRender {
    pub(crate) foundation: u8,
    pub(crate) surface_tileh: u8,
    pub(crate) surface_base_z: u8,
}

/// Dibuja el cimiento de una carretera normal y devuelve su superficie
/// efectiva. Es el análogo vial de [`spawn_rail_foundation`].
#[must_use]
#[allow(clippy::too_many_arguments)]
pub(crate) fn spawn_road_foundation(
    commands: &mut Commands,
    map: &Map,
    map_dims: (u32, u32),
    assets: &WorldAssets,
    ctx: &TileRenderContext,
    tileh: u8,
    road_bits: u8,
    foundation_newgrf: &[Option<openttdrs_core::DecodedSprite>],
    action5_sprites: Option<&mut crate::render::NewGrfAction5SpriteCache>,
    images: Option<&mut Assets<Image>>,
) -> RoadFoundationRender {
    let decision = road_foundation_decision(map, ctx, map_dims, tileh, road_bits);
    let plan = foundation_draw_plan(tileh, decision.foundation, decision.sprite_block);
    debug_assert_eq!(plan.surface_tileh, decision.surface_tileh);
    debug_assert_eq!(
        plan.surface_z_delta,
        decision.surface_base_z.saturating_sub(ctx.info.base_z)
    );

    if decision.foundation != 0 {
        WorldDrawTrace::record_foundation(
            "road",
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
    }

    let mut action5_sprites = action5_sprites;
    let mut images = images;
    for (index, draw) in plan.sprites.into_iter().flatten().enumerate() {
        spawn_foundation_sprite(
            commands,
            assets,
            ctx,
            "road-foundation",
            draw,
            0.36 + index as f32 * 0.0005,
            foundation_newgrf,
            action5_sprites.as_deref_mut(),
            images.as_deref_mut(),
        );
    }

    RoadFoundationRender {
        foundation: decision.foundation,
        surface_tileh: plan.surface_tileh,
        surface_base_z: decision.surface_base_z,
    }
}

/// Dibuja la fundación nivelada que OpenTTD fuerza para construcciones que no
/// pueden conservar la pendiente de la tesela, como un depósito ferroviario.
///
/// A diferencia de una vía normal, la elección no depende de `TrackBits`:
/// `DrawTile_Rail` llama explícitamente a `DrawFoundation(Leveled)`. Devuelve
/// la Z de la superficie plana que deben usar las capas posteriores.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub(crate) fn spawn_forced_leveled_foundation(
    commands: &mut Commands,
    map: &Map,
    map_dims: (u32, u32),
    assets: &WorldAssets,
    ctx: &TileRenderContext,
    tileh: u8,
    source: &'static str,
    role: &'static str,
    foundation_newgrf: &[Option<openttdrs_core::DecodedSprite>],
    action5_sprites: Option<&mut crate::render::NewGrfAction5SpriteCache>,
    images: Option<&mut Assets<Image>>,
) -> u8 {
    if tileh == 0 {
        return ctx.info.base_z;
    }

    // `ApplyPixelFoundationToSlope(Leveled)` eleva una unidad normal y dos
    // cuando la pendiente es empinada. La superficie resultante siempre es
    // plana, por lo que las capas posteriores se cuelgan del parent creado
    // por `DrawFoundation` con `OffsetGroundSprite(0, -TILE_HEIGHT)`.
    let z_delta = 1 + u8::from(tileh & SLOPE_STEEP != 0);
    let decision = foundation_decision(
        map,
        ctx,
        map_dims,
        openttdrs_core::FOUNDATION_LEVELED,
        0,
        z_delta,
    );
    WorldDrawTrace::record_foundation(
        source,
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

    let plan = openttdrs_core::foundation_draw_plan(
        tileh,
        openttdrs_core::FOUNDATION_LEVELED,
        decision.sprite_block,
    );
    debug_assert_eq!(plan.surface_tileh, decision.surface_tileh);
    debug_assert_eq!(
        plan.surface_z_delta,
        decision.surface_base_z.saturating_sub(ctx.info.base_z)
    );
    let mut action5_sprites = action5_sprites;
    let mut images = images;
    for (index, draw) in plan.sprites.into_iter().flatten().enumerate() {
        spawn_foundation_sprite(
            commands,
            assets,
            ctx,
            role,
            draw,
            0.36 + index as f32 * 0.0005,
            foundation_newgrf,
            action5_sprites.as_deref_mut(),
            images.as_deref_mut(),
        );
    }
    decision.surface_base_z
}

pub(crate) fn spawn_ground_sprite(
    commands: &mut Commands,
    image: &AtlasSprite,
    color: Color,
    ctx: &TileRenderContext,
    half_h: f32,
) {
    commands.spawn((
        MapVisualLayer,
        ctx.map_tile_chunk(),
        image.sprite_colored(color),
        Transform::from_translation(ground_tile_pos_half(
            ctx.tx_i32(),
            ctx.ty_i32(),
            ctx.info.base_z,
            0.0,
            half_h,
        )),
    ));
}

/// Variante para una superficie modificada por `DrawFoundation`.
///
/// El suelo de una rampa de puente comparte el orden local del deck y puede
/// llevar una capa explícita. Se conserva su profundidad histórica de overlay:
/// el pase diagonal sin altura de [`spawn_ground_sprite`] es para suelo natural
/// independiente (campos, césped y terreno bajo tiles), no para ese hijo de
/// fundación.
pub(crate) fn spawn_ground_sprite_at(
    commands: &mut Commands,
    image: &AtlasSprite,
    color: Color,
    ctx: &TileRenderContext,
    base_z: u8,
    layer: f32,
    half_h: f32,
) {
    commands.spawn((
        MapVisualLayer,
        ctx.map_tile_chunk(),
        image.sprite_colored(color),
        Transform::from_translation(tile_pos_half(
            ctx.tx_i32(),
            ctx.ty_i32(),
            base_z,
            layer,
            half_h,
        )),
    ));
}

pub(crate) fn push_water_sprite(
    batch_water: &mut Vec<(MapTileChunk, WaterTile, Sprite, Transform)>,
    h_water: &AtlasSprite,
    ctx: &TileRenderContext,
) {
    batch_water.push((
        ctx.map_tile_chunk(),
        WaterTile::ANIMATED,
        h_water.sprite(),
        Transform::from_translation(tile_pos(
            ctx.tx_i32(),
            ctx.ty_i32(),
            ctx.info.base_z,
            FLAT_WATER_LAYER_FRAC,
        )),
    ));
}

pub(crate) fn spawn_coast_debug_label(
    commands: &mut Commands,
    ctx: &TileRenderContext,
    raw: u8,
    tileh: u8,
    shore_index: usize,
) {
    let label = format!("r{raw}/t{tileh}/s{shore_index}");
    commands.spawn((
        MapVisualLayer,
        ctx.map_tile_chunk(),
        Text2d::new(label),
        TextFont {
            font_size: FontSize::Px(9.0),
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

#[cfg(test)]
mod tests {
    use bevy::prelude::Vec2;
    use openttdrs_core::{FOUNDATION_INCLINED_X, FOUNDATION_INCLINED_Y, FOUNDATION_LEVELED};

    use super::{
        FLAT_WATER_LAYER_FRAC, SHORE_LAYER_FRAC, bridge_foundation_kind, bridge_foundation_surface,
        flattening_foundation_surface, leveled_foundation_overlay_pos, road_foundation_kind,
        road_foundation_surface,
    };
    use crate::iso::{TILE_HALF_H, overlay_pos, tile_pos, tile_pos_half};

    #[test]
    fn leveled_overlay_matches_flat_elevation() {
        let flat = overlay_pos(Vec2::ZERO, 0.0, 0.0, 64.0, 40.0, 2, 0.5, 3, 4);
        let leveled =
            leveled_foundation_overlay_pos(Vec2::ZERO, 0.0, 0.0, 64.0, 40.0, 2, 0.5, 3, 4);
        assert!((flat.y - leveled.y).abs() < 0.01);
    }

    #[test]
    fn shore_z_sits_between_neighbor_land_and_water() {
        let tx = 10;
        let ty = 10;
        let shore = tile_pos_half(tx, ty, 0, SHORE_LAYER_FRAC, TILE_HALF_H).z;
        let inner_land = tile_pos(tx - 1, ty, 0, 0.0).z;
        let outer_water = tile_pos(tx + 1, ty, 0, FLAT_WATER_LAYER_FRAC).z;

        assert!(shore < inner_land);
        assert!(shore > outer_water);
    }

    #[test]
    fn bridge_foundation_surface_matches_openttd_bridge_ramp_rules() {
        // Kale_TitleGame (109,28): NW-facing rail bridge (axis Y) sobre
        // SLOPE_S. `GetBridgeFoundation` elige InclinedY → SLOPE_SE.
        assert_eq!(bridge_foundation_surface(0x02, 3), (0x06, 0));

        // Las pendientes paralelas a la rampa no requieren fundación.
        assert_eq!(bridge_foundation_surface(0x0C, 0), (0x0C, 0));
        assert_eq!(bridge_foundation_surface(0x09, 1), (0x09, 0));

        // Un lomo se nivela; una pendiente empinada conserva una elevación
        // extra antes de su superficie inclinada.
        assert_eq!(bridge_foundation_surface(0x05, 0), (0, 1));
        assert_eq!(bridge_foundation_surface(0x17, 1), (0x06, 1));

        // Kale_TitleGame (92,148): la pendiente de tres esquinas no tiene una
        // esquina alta única;
        // OpenTTD elige `Foundation::Leveled`, por lo que la cabeza se dibuja
        // sobre suelo plano a Z+1 (`SPR_BTGEN_ROAD_RAMP_X_DOWN`, 2450).
        assert_eq!(bridge_foundation_kind(0x0E, 2), FOUNDATION_LEVELED);
        assert_eq!(bridge_foundation_surface(0x0E, 2), (0, 1));
    }

    #[test]
    fn flattening_foundation_keeps_steep_height() {
        assert_eq!(flattening_foundation_surface(0), (0, 0));
        assert_eq!(flattening_foundation_surface(0x07), (0, 1));
        assert_eq!(flattening_foundation_surface(0x17), (0, 2));
    }

    #[test]
    fn road_foundation_surface_matches_get_road_foundation() {
        // Kale_TitleGame (108,79): ROAD_Y sobre SLOPE_S → InclinedY → SE.
        assert_eq!(road_foundation_kind(0x02, 0x05), FOUNDATION_INCLINED_Y);
        assert_eq!(road_foundation_surface(0x02, 0x05), (0x06, 0));
        // ROAD_X es la única recta que usa InclinedX.
        assert_eq!(road_foundation_kind(0x01, 0x0A), FOUNDATION_INCLINED_X);
        assert_eq!(road_foundation_surface(0x01, 0x0A), (0x03, 0));
        // Una combinación compatible cabe sobre una fundación nivelada.
        assert_eq!(road_foundation_kind(0x05, 0x0A), FOUNDATION_LEVELED);
        assert_eq!(road_foundation_surface(0x05, 0x0A), (0, 1));
        // En una pendiente diagonal, la recta compatible no necesita muro.
        assert_eq!(road_foundation_kind(0x03, 0x0A), 0);
        assert_eq!(road_foundation_surface(0x03, 0x0A), (0x03, 0));
    }
}
