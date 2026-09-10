use bevy::prelude::*;
use openttdrs_core::DecodedSprite;
use openttdrs_core::map::{
    WaterClass, has_tile_water_ground, industry_tiles_mergeable, is_map_object_tile,
    tile_slope_and_z, water_class,
};
use openttdrs_core::newgrf_sprites::Action2EvalCtx;
use openttdrs_core::prelude::*;
use openttdrs_core::station::{
    STATION_TYPE_BUOY, STATION_TYPE_DOCK, STATION_TYPE_OILRIG, station_type_from_m6,
};
use openttdrs_core::{Climate, SLOPE_NE, SLOPE_NW, SLOPE_SE, SLOPE_SW};

use super::{FLAT_WATER_LAYER_FRAC, SHORE_LAYER_FRAC, push_water_sprite, spawn_coast_debug_label};
use crate::iso::{
    GROUND_SPRITE_CENTER_X_OFFSET, TILE_HALF_H, ground_draw_z, overlay_pos, shore_png_index,
    shore_sprite_half_h, shore_tileh_for_draw_shore, slope_half_h, tile_pos_half,
    tile_slope_bits_from_heights,
};
use crate::render::newgrf_cache::{runtime_fingerprint, vars};
use crate::render::shore_newgrf::{NEWGRF_SHORE_TILE_FLAG, NewGrfShoreSpriteCache};
use crate::render::viewport_sort::ParentSpriteBounds;
use crate::render::world_draw_trace::WorldDrawTrace;
use crate::render::{
    MapSpriteBatches, MapVisualLayer, TileRenderContext, ViewportSortableParent, WaterTile,
    WorldAssets, viewport_insertion_key, viewport_source_depth,
};
use crate::sprites::{WATER_LOCK_SPRITE_META, WATER_RIVER_SLOPE_SPRITE_META};

/// `SPR_FLAT_WATER_TILE` de `table/sprites.h`.
const SPR_FLAT_WATER_TILE: u32 = 4061;
/// `SPR_CANAL_DIKES_BASE` de `table/sprites.h`.
pub(crate) const SPR_CANAL_DIKES_BASE: u32 = 5380;
/// Tipo Action5 y primer slot de los diques dentro de `0x08 Canals`.
const ACTION5_CANALS_TYPE: u8 = openttdrs_core::ACTION5_TYPE_CANALS;
const CANALS_ACTION5_DIKES_OFFSET: usize = 52;
/// Namespace fuera del rango de tipos Action5 para vistas Action1/3 de canal.
const CANAL_FEATURE_CACHE_TYPE_BASE: u8 = 0x80;
/// `SPR_CANALS_BASE` de `table/sprites.h`: las cuatro pendientes de río
/// vanilla ocupan los slots 0..3 de la hoja Action5 de canales.
pub(crate) const SPR_RIVER_SLOPE_BASE: u32 = 5328;
/// Primer sprite de esclusa relativo a `SPR_CANALS_BASE`.
const SPR_LOCK_WATER_BASE: u32 = SPR_RIVER_SLOPE_BASE + 4;
/// `SPR_SHORE_BASE` resuelto por Action5 canals en OpenGFX/OpenGFX2.
const SPR_SHORE_BASE: u32 = 5936;

/// Convierte el slot local de una vista de agua en el ID lógico que usa la
/// traza `world-draw`. Las vistas Action1/3 no tienen un ID OpenGFX propio en
/// el atlas candidato; se comparan en el mismo namespace de `SPR_CANALS_BASE`
/// que usa `GetCanalSprite` en OpenTTD.
#[must_use]
pub(crate) fn canal_feature_trace_sprite_id(feature_id: u8, slot: usize) -> u32 {
    let base = if feature_id == openttdrs_core::CF_DIKES {
        SPR_CANAL_DIKES_BASE
    } else {
        SPR_RIVER_SLOPE_BASE
    };
    base.saturating_add(u32::try_from(slot).unwrap_or(u32::MAX))
}

/// Variables de `CanalScopeResolver` disponibles para un sprite de agua.
///
/// El mapa conserva `m3hi` como `m4()` (random del agua), el nibble bajo de
/// `mapt` como `TropicZone` y la altura de la tesela para el clima ártico.
fn canal_scope_height(tile: Tile) -> u8 {
    // `CanalScopeResolver::GetVariable(0x80)` mantiene la continuidad de una
    // esclusa: la parte superior usa la cota de la cámara inferior (`z - 1`).
    // El ajuste sólo aplica a MP_WATER/LockPart::Upper; un depósito naval con
    // bits parecidos no debe recibirlo.
    let is_upper_lock =
        tile.kind == TileKind::Water && (tile.m5 >> 4) & 0x0F == 2 && (tile.m5 >> 2) & 0x03 == 2;
    tile.height.saturating_sub(u8::from(is_upper_lock))
}

fn canal_action2_context(
    tile: Option<Tile>,
    connectivity: u8,
    climate: Climate,
    snow_line_height: u8,
) -> Action2EvalCtx {
    let mut action2 = Action2EvalCtx::default();
    let Some(tile) = tile else {
        return action2;
    };
    // `CanalScopeResolver::GetRandomBits` and var 0x83 expose m4 only for
    // MP_WATER. Ship depots are represented as a separate semantic kind in
    // this renderer, but still originate from MP_WATER; stations, industries,
    // objects and trees must receive zero even when they retain m3hi bytes.
    let random = match tile.kind {
        TileKind::Water | TileKind::ShipDepot => u32::from(tile.m3hi),
        _ => 0,
    };
    let height = canal_scope_height(tile);
    let terrain = match climate {
        Climate::SubTropical => u32::from(tile.mapt & 0x03),
        Climate::SubArctic => u32::from(tile.height > snow_line_height) * 4,
        Climate::Temperate | Climate::Toyland => 0,
    };
    action2.random_bits = random;
    action2.vars.insert(0x80, u32::from(height));
    action2.vars.insert(0x81, terrain);
    action2.vars.insert(0x82, u32::from(connectivity));
    action2.vars.insert(0x83, random);
    action2
}

/// Máscara `0x82` de `CanalScopeResolver`, con la misma orientación que
/// `DrawWaterEdges`: lados NE/SE/SW/NW y luego E/S/W/N.
fn canal_connectivity_mask(map: &Map, coord: TileCoord) -> u8 {
    let checks = [
        (offset(coord, -1, 0), WateredFrom::Sw),
        (offset(coord, 0, 1), WateredFrom::Nw),
        (offset(coord, 1, 0), WateredFrom::Ne),
        (offset(coord, 0, -1), WateredFrom::Se),
        (offset(coord, -1, 1), WateredFrom::W),
        (offset(coord, 1, 1), WateredFrom::N),
        (offset(coord, 1, -1), WateredFrom::E),
        (offset(coord, -1, -1), WateredFrom::S),
    ];
    checks
        .into_iter()
        .enumerate()
        .fold(0, |mask, (bit, (coord, from))| {
            mask | u8::from(!is_watered_tile(map, coord, from)) << bit
        })
}

fn canal_action2_context_for_tile(map: &Map, ctx: &TileRenderContext) -> Action2EvalCtx {
    canal_action2_context(
        ctx.tile,
        canal_connectivity_mask(map, ctx.coord),
        ctx.climate,
        ctx.snow_line_height,
    )
}

fn shore_sprite_id(tileh: u8) -> u32 {
    SPR_SHORE_BASE + shore_png_index(tileh) as u32
}

/// Materializa un reemplazo Action5 `Canals` y conserva su ancla NFO.
///
/// Los cuatro sprites de pendiente ocupan los slots 0..3 y los doce diques
/// empiezan en el slot 52. Si el caller no tiene cache/`Assets<Image>` (por
/// ejemplo, un preview mínimo), el caller puede usar el fallback vanilla.
fn action5_canal_sprite(
    slot: usize,
    table: &[Option<DecodedSprite>],
    cache: &mut Option<&mut crate::render::NewGrfAction5SpriteCache>,
    images: &mut Option<&mut Assets<Image>>,
) -> Option<(Sprite, DecodedSprite)> {
    let decoded = table.get(slot).and_then(Option::as_ref)?.clone();
    let (Some(cache), Some(images)) = (cache.as_deref_mut(), images.as_deref_mut()) else {
        return None;
    };
    let sprite = cache.sprite_colored(ACTION5_CANALS_TYPE, slot, table, Color::WHITE, images)?;
    Some((sprite, decoded))
}

/// Materializa una vista Action1/3 de `CanalFeature` en el cache compartido.
///
/// Las vistas de features no tienen un tipo Action5 propio. Se usa un
/// namespace reservado en la clave del cache para que una vista `CF_DIKES`
/// no pueda reutilizar accidentalmente el handle de un slot `0x08 Canals`.
#[cfg(test)]
fn canal_feature_sprite(
    canal_features: &[openttdrs_core::CanalFeatureDef],
    feature_id: u8,
    slot: usize,
    cache: &mut Option<&mut crate::render::NewGrfAction5SpriteCache>,
    images: &mut Option<&mut Assets<Image>>,
) -> Option<(Sprite, DecodedSprite)> {
    let mut action2 = Action2EvalCtx::default();
    canal_feature_sprite_with_context(
        canal_features,
        feature_id,
        slot,
        cache,
        images,
        &mut action2,
    )
}

fn canal_feature_sprite_with_context(
    canal_features: &[openttdrs_core::CanalFeatureDef],
    feature_id: u8,
    slot: usize,
    cache: &mut Option<&mut crate::render::NewGrfAction5SpriteCache>,
    images: &mut Option<&mut Assets<Image>>,
    action2: &mut Action2EvalCtx,
) -> Option<(Sprite, DecodedSprite)> {
    canal_feature_sprite_with_context_and_slot(
        canal_features,
        feature_id,
        slot,
        cache,
        images,
        action2,
    )
    .map(|(sprite, decoded, _)| (sprite, decoded))
}

fn canal_feature_sprite_with_context_and_slot(
    canal_features: &[openttdrs_core::CanalFeatureDef],
    feature_id: u8,
    slot: usize,
    cache: &mut Option<&mut crate::render::NewGrfAction5SpriteCache>,
    images: &mut Option<&mut Assets<Image>>,
    action2: &mut Action2EvalCtx,
) -> Option<(Sprite, DecodedSprite, usize)> {
    let feature = openttdrs_core::canal_feature_def(canal_features, feature_id)?;
    let selected_slot = feature.newgrf_sprite_offset(slot, action2);
    let decoded = feature
        .newgrf_view_runtime(selected_slot, action2)
        .or_else(|| feature.newgrf_views.get(selected_slot).cloned())?;
    let (Some(cache), Some(images)) = (cache.as_deref_mut(), images.as_deref_mut()) else {
        return None;
    };
    let slot = u16::try_from(selected_slot).ok()?;
    let runtime_fp = feature
        .newgrf_runtime
        .as_ref()
        .map_or(0, |_| runtime_fingerprint(action2, vars::CANAL, false));
    let handle = cache.handle_for_variant(
        CANAL_FEATURE_CACHE_TYPE_BASE + feature_id,
        slot,
        runtime_fp,
        &decoded,
        images,
    );
    Some((
        Sprite {
            image: handle,
            color: Color::WHITE,
            ..default()
        },
        decoded,
        selected_slot,
    ))
}

/// Resuelve el primer sprite plano de un feature de canal que declara
/// `CFF_HAS_FLAT_SPRITE`, conservando la geometría NFO en el pase sortable.
pub(crate) fn canal_feature_surface(
    ctx: &TileRenderContext,
    map: &Map,
    feature_id: u8,
    canal_features: &[openttdrs_core::CanalFeatureDef],
    mut action5_sprites: Option<&mut crate::render::NewGrfAction5SpriteCache>,
    mut images: Option<&mut Assets<Image>>,
) -> Option<(Sprite, Transform, usize)> {
    let feature = openttdrs_core::canal_feature_def(canal_features, feature_id)?;
    if feature.flags & openttdrs_core::CFF_HAS_FLAT_SPRITE == 0 {
        return None;
    }
    let mut action2 = canal_action2_context_for_tile(map, ctx);
    let (sprite, decoded, selected_slot) = canal_feature_sprite_with_context_and_slot(
        canal_features,
        feature_id,
        0,
        &mut action5_sprites,
        &mut images,
        &mut action2,
    )?;
    let position = overlay_pos(
        ctx.iso_pos,
        f32::from(decoded.x_offs),
        f32::from(decoded.y_offs),
        f32::from(decoded.width),
        f32::from(decoded.height),
        ctx.info.base_z,
        FLAT_WATER_LAYER_FRAC,
        ctx.tx_i32(),
        ctx.ty_i32(),
    );
    Some((sprite, Transform::from_translation(position), selected_slot))
}

/// Igual que [`canal_feature_surface`], pero conserva la profundidad del
/// `DrawGroundSprite` que usa `DrawWaterClassGround` antes del depósito naval.
/// El agua normal mantiene su sesgo de costa; sólo el ground del depósito debe
/// entrar en la banda común del pase de suelo.
pub(crate) fn canal_feature_surface_ground(
    ctx: &TileRenderContext,
    map: &Map,
    feature_id: u8,
    canal_features: &[openttdrs_core::CanalFeatureDef],
    action5_sprites: Option<&mut crate::render::NewGrfAction5SpriteCache>,
    images: Option<&mut Assets<Image>>,
) -> Option<(Sprite, Transform, usize)> {
    let (sprite, mut transform, selected_slot) = canal_feature_surface(
        ctx,
        map,
        feature_id,
        canal_features,
        action5_sprites,
        images,
    )?;
    transform.translation.z = ground_draw_z(ctx.tx_i32(), ctx.ty_i32(), 0.0);
    Some((sprite, transform, selected_slot))
}

/// Resuelve el ground que `DrawWaterLock` obtiene de `CF_WATERSLOPE`.
///
/// El layout vanilla usa cuatro índices distintos para la tesela central de
/// la esclusa (`NE, SE, SW, NW`) y `SPR_FLAT_WATER_TILE` para las partes
/// inferior/superior. Cuando el feature declara `CFF_HAS_FLAT_SPRITE`, el
/// sprite plano ocupa el slot 0 y desplaza los cuatro índices centrales una
/// posición; sin esa bandera las partes no centrales siguen usando el agua
/// vanilla, exactamente como en `water_cmd.cpp`.
fn lock_water_ground_sprite(
    map: &Map,
    ctx: &TileRenderContext,
    canal_features: &[openttdrs_core::CanalFeatureDef],
    mut action5_sprites: Option<&mut crate::render::NewGrfAction5SpriteCache>,
    mut images: Option<&mut Assets<Image>>,
) -> Option<(Sprite, Transform, usize)> {
    let tile = ctx.tile?;
    if tile.kind != TileKind::Water || (tile.m5 >> 4) & 0x0F != 2 {
        return None;
    }
    let feature = openttdrs_core::canal_feature_def(canal_features, openttdrs_core::CF_WATERSLOPE)?;
    let has_flat_sprite = feature.flags & openttdrs_core::CFF_HAS_FLAT_SPRITE != 0;
    let part = (tile.m5 >> 2) & 0x03;
    let offset = match part {
        0 => {
            // `_lock_display_middle_*_seq` uses 1, 0, 2, 3 for NE, SE, SW,
            // NW respectively. A flat custom sprite is prepended at zero.
            let middle_offset = [1usize, 0, 2, 3][usize::from(tile.m5 & 0x03)];
            middle_offset + usize::from(has_flat_sprite)
        }
        1 | 2 if has_flat_sprite => 0,
        1 | 2 => return None,
        _ => return None,
    };
    let mut action2 = canal_action2_context_for_tile(map, ctx);
    let (sprite, decoded) = canal_feature_sprite_with_context(
        canal_features,
        openttdrs_core::CF_WATERSLOPE,
        offset,
        &mut action5_sprites,
        &mut images,
        &mut action2,
    )?;
    let mut position = overlay_pos(
        ctx.iso_pos,
        f32::from(decoded.x_offs),
        f32::from(decoded.y_offs),
        f32::from(decoded.width),
        f32::from(decoded.height),
        ctx.info.base_z,
        0.02,
        ctx.tx_i32(),
        ctx.ty_i32(),
    );
    // Lock water is a DrawGroundSprite, not a sortable BUILD layer.
    position.z = ground_draw_z(ctx.tx_i32(), ctx.ty_i32(), 0.02);
    Some((sprite, Transform::from_translation(position), offset))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct LockStructureLayer {
    dx: i32,
    dy: i32,
    image_offset: usize,
    extent_x: i32,
    extent_y: i32,
    extent_z: i32,
}

/// Devuelve las dos líneas `TILE_SEQ` de una esclusa vanilla.
fn lock_structure_layer(part: u8, direction: usize, layer: usize) -> Option<LockStructureLayer> {
    let (dx, dy, extent_x, extent_y) = match (direction, layer) {
        (0, 0) | (2, 0) => (0, 0, 16, 1),
        (0, 1) | (2, 1) => (0, 15, 16, 1),
        (1, 0) | (3, 0) => (0, 0, 1, 16),
        (1, 1) => (15, 0, 1, 16),
        (3, 1) => (15, 0, 1, 16),
        _ => return None,
    };
    let direction = direction.min(3);
    let image_offset = match part {
        0 => [[1, 5], [0, 4], [2, 6], [3, 7]][direction][layer],
        1 => [[9, 13], [8, 12], [10, 14], [11, 15]][direction][layer],
        2 => [[17, 21], [16, 20], [18, 22], [19, 23]][direction][layer],
        _ => return None,
    };
    let extent_z = if part == 2 || layer == 0 { 6 } else { 10 };
    Some(LockStructureLayer {
        dx,
        dy,
        image_offset,
        extent_x,
        extent_y,
        extent_z,
    })
}

/// Emite las estructuras de `DrawWaterLock` cuando el ground `CF_WATERSLOPE`
/// reemplazó el sprite compuesto vanilla.
#[allow(clippy::too_many_arguments)]
fn spawn_lock_structures(
    commands: &mut Commands,
    map: &Map,
    assets: &WorldAssets,
    ctx: &TileRenderContext,
    map_width: u32,
    canal_features: &[openttdrs_core::CanalFeatureDef],
    canal_action5: &[Option<DecodedSprite>],
    mut action5_sprites: Option<&mut crate::render::NewGrfAction5SpriteCache>,
    mut images: Option<&mut Assets<Image>>,
) {
    let Some(tile) = ctx.tile else {
        return;
    };
    let part = (tile.m5 >> 2) & 0x03;
    let direction = usize::from(tile.m5 & 0x03);
    let z_offset = usize::from(part == 2 && ctx.info.base_z > 8) * 24;

    for layer_index in 0..2 {
        let Some(layer) = lock_structure_layer(part, direction, layer_index) else {
            continue;
        };
        let action5_slot = layer.image_offset + 4 + z_offset;
        let mut action2 = canal_action2_context_for_tile(map, ctx);
        let custom = canal_feature_sprite_with_context(
            canal_features,
            openttdrs_core::CF_LOCKS,
            layer.image_offset,
            &mut action5_sprites,
            &mut images,
            &mut action2,
        );
        let action5 = custom.is_none().then(|| {
            action5_canal_sprite(
                action5_slot,
                canal_action5,
                &mut action5_sprites,
                &mut images,
            )
        });
        let (sprite, width, height, xrel, yrel, fallback, logical_offset) =
            if let Some((sprite, decoded)) = custom {
                (
                    sprite,
                    f32::from(decoded.width),
                    f32::from(decoded.height),
                    f32::from(decoded.x_offs),
                    f32::from(decoded.y_offs),
                    false,
                    layer.image_offset,
                )
            } else if let Some(Some((sprite, decoded))) = action5 {
                (
                    sprite,
                    f32::from(decoded.width),
                    f32::from(decoded.height),
                    f32::from(decoded.x_offs),
                    f32::from(decoded.y_offs),
                    false,
                    layer.image_offset + z_offset,
                )
            } else {
                let Some(meta) = WATER_LOCK_SPRITE_META.get(action5_slot.saturating_sub(4)) else {
                    continue;
                };
                (
                    assets.water_lock_structures[action5_slot.saturating_sub(4)].sprite(),
                    f32::from(meta.0),
                    f32::from(meta.1),
                    f32::from(meta.2),
                    f32::from(meta.3),
                    true,
                    layer.image_offset + z_offset,
                )
            };
        let sprite_id = SPR_LOCK_WATER_BASE + logical_offset as u32;
        let bounds = ParentSpriteBounds::new(
            ctx.tx_i32() * 16 + layer.dx,
            ctx.ty_i32() * 16 + layer.dy,
            i32::from(ctx.info.base_z) * 8,
            ctx.tx_i32() * 16 + layer.dx + layer.extent_x - 1,
            ctx.ty_i32() * 16 + layer.dy + layer.extent_y - 1,
            i32::from(ctx.info.base_z) * 8 + layer.extent_z - 1,
        );
        WorldDrawTrace::record_sprite_with_palette_and_geometry(
            "water-lock-structure",
            "sortable",
            sprite_id,
            0,
            fallback,
            (layer.dx, layer.dy, 0),
            0,
            Some(crate::render::world_draw_trace::TraceSpriteBounds::new(
                layer.dx,
                layer.dy,
                0,
                layer.extent_x,
                layer.extent_y,
                layer.extent_z,
            )),
        );
        let mut position = overlay_pos(
            ctx.iso_pos,
            xrel,
            yrel,
            width,
            height,
            ctx.info.base_z,
            0.04 + layer_index as f32 * 0.0005,
            ctx.tx_i32(),
            ctx.ty_i32(),
        );
        let source_depth = viewport_source_depth(position.z, ctx.tx, map_width);
        position.z = source_depth;
        commands.spawn((
            MapVisualLayer,
            ctx.map_tile_chunk(),
            sprite,
            Transform::from_translation(position),
            ViewportSortableParent {
                sprite_id,
                bounds,
                insertion_key: viewport_insertion_key(
                    ctx.tx,
                    ctx.ty,
                    u8::try_from(layer_index + 1).unwrap_or(u8::MAX),
                ),
                source_depth,
            },
        ));
    }
}

/// Índice de `SPR_CANALS_BASE + offset` que selecciona `DrawRiverWater` cuando
/// no hay un callback NewGRF que reemplace la pendiente.
#[must_use]
pub(crate) const fn river_slope_sprite_index(tileh: u8) -> Option<usize> {
    match tileh {
        SLOPE_SE => Some(0), // SPR_WATER_SLOPE_Y_UP
        SLOPE_NE => Some(1), // SPR_WATER_SLOPE_X_DOWN
        SLOPE_SW => Some(2), // SPR_WATER_SLOPE_X_UP
        SLOPE_NW => Some(3), // SPR_WATER_SLOPE_Y_DOWN
        _ => None,
    }
}

/// Direcciones que `IsWateredTile` recibe desde `DrawWaterEdges`.
///
/// No usamos `VehicleDirection` directamente porque aquí también aparecen los
/// cuatro cardinales de las esquinas (`DIR_W/N/E/S`), y mantener la tabla
/// explícita evita que una conversión de dirección oculte el eje del lock.
#[derive(Clone, Copy, Debug)]
enum WateredFrom {
    Sw,
    Nw,
    Ne,
    Se,
    W,
    N,
    E,
    S,
}

/// Offset de `TileOffsByDir(from)` en el plano de coordenadas del mapa.
///
/// `DrawWaterEdges` consulta una tesela vecina y le pasa la dirección que
/// apunta hacia el lado opuesto. Para `MP_STATION`/`MP_INDUSTRY`,
/// `IsWateredTile` vuelve a avanzar con este offset para saber si está dentro
/// de la misma estructura. Mantener la tabla aquí evita confundir las
/// direcciones de la vista con los ejes X/Y del mapa.
const fn watered_source_offset(from: WateredFrom) -> (i32, i32) {
    match from {
        WateredFrom::Sw => (1, 0),  // DIR_SW
        WateredFrom::Nw => (0, -1), // DIR_NW
        WateredFrom::Ne => (-1, 0), // DIR_NE
        WateredFrom::Se => (0, 1),  // DIR_SE
        WateredFrom::W => (1, -1),  // DIR_W
        WateredFrom::N => (-1, -1), // DIR_N
        WateredFrom::E => (-1, 1),  // DIR_E
        WateredFrom::S => (1, 1),   // DIR_S
    }
}

fn offset(coord: TileCoord, dx: i32, dy: i32) -> TileCoord {
    TileCoord::new(coord.x + dx, coord.y + dy)
}

fn watered_source_tile(map: &Map, coord: TileCoord, from: WateredFrom) -> Option<Tile> {
    let (dx, dy) = watered_source_offset(from);
    map.get(offset(coord, dx, dy))
}

fn coast_is_watered(map: &Map, coord: TileCoord, from: WateredFrom) -> bool {
    let Some((tileh, _)) = tile_slope_and_z(map, coord) else {
        return false;
    };
    match tileh {
        0x01 => matches!(from, WateredFrom::Se | WateredFrom::E | WateredFrom::Ne),
        0x02 => matches!(from, WateredFrom::Ne | WateredFrom::N | WateredFrom::Nw),
        0x04 => matches!(from, WateredFrom::Nw | WateredFrom::W | WateredFrom::Sw),
        0x08 => matches!(from, WateredFrom::Sw | WateredFrom::S | WateredFrom::Se),
        _ => false,
    }
}

fn lock_is_watered(m5: u8, from: WateredFrom) -> bool {
    // `place_lock` stores Axis::Y in bit 0. `DiagDirToAxis(DirToDiagDir())`
    // maps SW/NE/N/S to Axis::X and NW/SE/E/W to Axis::Y.
    let axis_y = m5 & 1 != 0;
    if axis_y {
        matches!(
            from,
            WateredFrom::Nw | WateredFrom::Se | WateredFrom::E | WateredFrom::W
        )
    } else {
        matches!(
            from,
            WateredFrom::Sw | WateredFrom::Ne | WateredFrom::N | WateredFrom::S
        )
    }
}

const fn watered_from_diag_direction(from: WateredFrom) -> u8 {
    // `DirToDiagDir` redondea cada cardinal hacia el siguiente eje horario:
    // N/NE → NE, E/SE → SE, S/SW → SW y W/NW → NW.
    match from {
        WateredFrom::Ne | WateredFrom::N => 0,
        WateredFrom::Se | WateredFrom::E => 1,
        WateredFrom::Sw | WateredFrom::S => 2,
        WateredFrom::Nw | WateredFrom::W => 3,
    }
}

fn tunnel_bridge_is_watered(tile: Tile, from: WateredFrom) -> bool {
    const TRANSPORT_WATER: u8 = 2;
    let transport = (tile.m5 >> 2) & 0x03;
    let direction = tile.m5 & 0x03;
    transport == TRANSPORT_WATER && (direction ^ 0x02) == watered_from_diag_direction(from)
}

/// Equivalente del `IsWateredTile` usado por `DrawWaterEdges`.
///
/// Las estaciones petroleras y las industrias suprimen los bordes internos de
/// una estructura compuesta, igual que `IsWateredTile` en OpenTTD. El mapa
/// conserva el `IndustryID` en MAP2 y el vínculo legacy de MAP1, por lo que
/// podemos mantener esa separación sin hacer flood-fill durante el render.
fn is_watered_tile(map: &Map, coord: TileCoord, from: WateredFrom) -> bool {
    let Some(tile) = map.get(coord) else {
        // `MP_VOID` es agua a efectos de los bordes del mapa.
        return true;
    };

    // `IsWateredTile(MP_OBJECT)` delega en `IsTileOnWater`. El tipo semántico
    // local de un objeto puede ser `Unknown(10)`, así que la clase válida se
    // debe leer de MAPT/M1 antes de entrar al match de TileKind.
    if is_map_object_tile(tile.mapt) {
        return water_class(tile).is_some_and(|class| class != WaterClass::Invalid);
    }

    // `MP_TUNNELBRIDGE` también representa las rampas de un acueducto. En
    // OpenTTD sólo el lado opuesto a `GetTunnelBridgeDirection` se considera
    // mojado, y únicamente cuando el transporte codificado es agua.
    if tile.is_tunnel_bridge_tile() {
        return tunnel_bridge_is_watered(tile, from);
    }

    match tile.kind {
        TileKind::Water => match (tile.m5 >> 4) & 0x0F {
            0 | 3 => true, // Clear / Depot.
            1 => coast_is_watered(map, coord, from),
            2 => lock_is_watered(tile.m5, from),
            _ => false,
        },
        TileKind::ShipDepot | TileKind::Void => true,
        TileKind::Rail if tile.m3hi & 0x0F == 13 => coast_is_watered(map, coord, from),
        TileKind::Station => match station_type_from_m6(tile.m6) {
            STATION_TYPE_DOCK => tile_slope_and_z(map, coord).is_some_and(|(tileh, _)| tileh == 0),
            STATION_TYPE_BUOY => true,
            STATION_TYPE_OILRIG => {
                // Oil rigs are represented as stations after construction, but
                // their outer tiles still border an industry or another rig.
                watered_source_tile(map, coord, from).is_some_and(|source| {
                    (source.kind == TileKind::Station
                        && station_type_from_m6(source.m6) == STATION_TYPE_OILRIG)
                        || source.kind == TileKind::Industry
                }) || has_tile_water_ground(tile)
            }
            _ => false,
        },
        TileKind::Industry => {
            // An industry tile is water-facing unless the source tile in this
            // direction belongs to the same industry or to its oil-rig station.
            watered_source_tile(map, coord, from).is_some_and(|source| {
                (source.kind == TileKind::Station
                    && station_type_from_m6(source.m6) == STATION_TYPE_OILRIG)
                    || (source.kind == TileKind::Industry
                        && industry_tiles_mergeable(&tile, &source, false))
            }) || has_tile_water_ground(tile)
        }
        _ => false,
    }
}

/// Agrega la pendiente fluvial vanilla como `DrawGroundSprite`.
///
/// A diferencia del agua plana, estas cuatro imágenes no forman parte del
/// ciclo de paleta animada: `DrawRiverWater` emite el sprite estático y sólo
/// después consulta los bordes NewGRF. El atlas conserva el `xrel/yrel` real
/// de cada fila Action5, incluidas las dos variantes de 39 px de alto.
fn river_slope_draw(
    ctx: &TileRenderContext,
    custom_sprite: Option<&DecodedSprite>,
) -> Option<(usize, Vec3)> {
    let index = river_slope_sprite_index(ctx.info.tileh)?;
    let (width, height, xrel, yrel) = custom_sprite.map_or_else(
        || {
            WATER_RIVER_SLOPE_SPRITE_META
                .get(index)
                .map(|&(width, height, xrel, yrel)| {
                    (
                        f32::from(width),
                        f32::from(height),
                        f32::from(xrel),
                        f32::from(yrel),
                    )
                })
        },
        |sprite| {
            Some((
                f32::from(sprite.width),
                f32::from(sprite.height),
                f32::from(sprite.x_offs),
                f32::from(sprite.y_offs),
            ))
        },
    )?;
    let mut position = overlay_pos(
        ctx.iso_pos,
        xrel,
        yrel,
        width,
        height,
        ctx.info.base_z,
        0.0,
        ctx.tx_i32(),
        ctx.ty_i32(),
    );
    // OpenTTD emits this through DrawGroundSprite: elevation changes the
    // screen position, but not the diagonal ground-pass ordering.
    position.z = ground_draw_z(ctx.tx_i32(), ctx.ty_i32(), 0.0);
    Some((index, position))
}

#[allow(clippy::too_many_arguments)]
fn push_river_slope_sprite(
    batch_water: &mut Vec<(crate::render::MapTileChunk, WaterTile, Sprite, Transform)>,
    map: &Map,
    assets: &WorldAssets,
    ctx: &TileRenderContext,
    canal_features: &[openttdrs_core::CanalFeatureDef],
    canal_action5: &[Option<DecodedSprite>],
    mut action5_sprites: Option<&mut crate::render::NewGrfAction5SpriteCache>,
    mut images: Option<&mut Assets<Image>>,
) -> bool {
    let index = match river_slope_sprite_index(ctx.info.tileh) {
        Some(index) => index,
        None => return false,
    };
    let feature_custom = openttdrs_core::canal_feature_def(
        canal_features,
        openttdrs_core::CF_RIVER_SLOPE,
    )
    .and_then(|feature| {
        let flat_offset = usize::from(feature.flags & openttdrs_core::CFF_HAS_FLAT_SPRITE != 0);
        let mut action2 = canal_action2_context_for_tile(map, ctx);
        canal_feature_sprite_with_context_and_slot(
            canal_features,
            openttdrs_core::CF_RIVER_SLOPE,
            flat_offset + index,
            &mut action5_sprites,
            &mut images,
            &mut action2,
        )
    });
    let custom = feature_custom.or_else(|| {
        action5_canal_sprite(index, canal_action5, &mut action5_sprites, &mut images)
            .map(|(sprite, decoded)| (sprite, decoded, index))
    });
    let Some((_, position)) = river_slope_draw(ctx, custom.as_ref().map(|(_, sprite, _)| sprite))
    else {
        return false;
    };

    let sprite_id = custom.as_ref().map_or_else(
        || canal_feature_trace_sprite_id(openttdrs_core::CF_RIVER_SLOPE, index),
        |(_, _, selected_slot)| {
            canal_feature_trace_sprite_id(openttdrs_core::CF_RIVER_SLOPE, *selected_slot)
        },
    );
    WorldDrawTrace::record_sprite("water-river-slope", "ground", sprite_id, false);
    batch_water.push((
        ctx.map_tile_chunk(),
        WaterTile::STATIC,
        custom.map_or_else(
            || assets.river_slopes[index].sprite(),
            |(sprite, _, _)| sprite,
        ),
        Transform::from_translation(position),
    ));
    true
}

/// Emite el ground de `DrawWaterDepot` consumiendo Action5 `Canals`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn spawn_river_slope_ground_with_action5(
    commands: &mut Commands,
    map: &Map,
    assets: &WorldAssets,
    ctx: &TileRenderContext,
    canal_features: &[openttdrs_core::CanalFeatureDef],
    canal_action5: &[Option<DecodedSprite>],
    mut action5_sprites: Option<&mut crate::render::NewGrfAction5SpriteCache>,
    mut images: Option<&mut Assets<Image>>,
) -> bool {
    let Some(index) = river_slope_sprite_index(ctx.info.tileh) else {
        return false;
    };
    let feature_custom = openttdrs_core::canal_feature_def(
        canal_features,
        openttdrs_core::CF_RIVER_SLOPE,
    )
    .and_then(|feature| {
        let flat_offset = usize::from(feature.flags & openttdrs_core::CFF_HAS_FLAT_SPRITE != 0);
        let mut action2 = canal_action2_context_for_tile(map, ctx);
        canal_feature_sprite_with_context_and_slot(
            canal_features,
            openttdrs_core::CF_RIVER_SLOPE,
            flat_offset + index,
            &mut action5_sprites,
            &mut images,
            &mut action2,
        )
    });
    let custom = feature_custom.or_else(|| {
        action5_canal_sprite(index, canal_action5, &mut action5_sprites, &mut images)
            .map(|(sprite, decoded)| (sprite, decoded, index))
    });
    let Some((_, position)) = river_slope_draw(ctx, custom.as_ref().map(|(_, sprite, _)| sprite))
    else {
        return false;
    };
    let sprite_id = custom.as_ref().map_or_else(
        || canal_feature_trace_sprite_id(openttdrs_core::CF_RIVER_SLOPE, index),
        |(_, _, selected_slot)| {
            canal_feature_trace_sprite_id(openttdrs_core::CF_RIVER_SLOPE, *selected_slot)
        },
    );
    WorldDrawTrace::record_sprite("ship-depot-water", "ground", sprite_id, false);
    commands.spawn((
        MapVisualLayer,
        ctx.map_tile_chunk(),
        WaterTile::STATIC,
        custom.map_or_else(
            || assets.river_slopes[index].sprite(),
            |(sprite, _, _)| sprite,
        ),
        Transform::from_translation(position),
    ));
    true
}

/// Selecciona los doce slots de `DrawWaterEdges` para una clase de agua.
///
/// El orden de los índices es el del C++: cuatro lados, cuatro esquinas
/// completas y cuatro esquinas cóncavas sólo cuando el diagonal intermedio no
/// está mojado.
#[must_use]
fn water_edge_slots(map: &Map, coord: TileCoord, expected_class: WaterClass) -> [bool; 12] {
    let mut slots = [false; 12];
    if map.get(coord).and_then(water_class) != Some(expected_class) {
        return slots;
    }

    let watered = [
        is_watered_tile(map, offset(coord, -1, 0), WateredFrom::Sw),
        is_watered_tile(map, offset(coord, 0, 1), WateredFrom::Nw),
        is_watered_tile(map, offset(coord, 1, 0), WateredFrom::Ne),
        is_watered_tile(map, offset(coord, 0, -1), WateredFrom::Se),
    ];
    slots[0] = !watered[0];
    slots[1] = !watered[1];
    slots[2] = !watered[2];
    slots[3] = !watered[3];

    if !watered[0] && !watered[1] {
        slots[4] = true;
    } else if watered[0]
        && watered[1]
        && !is_watered_tile(map, offset(coord, -1, 1), WateredFrom::W)
    {
        slots[8] = true;
    }

    if !watered[1] && !watered[2] {
        slots[5] = true;
    } else if watered[1] && watered[2] && !is_watered_tile(map, offset(coord, 1, 1), WateredFrom::N)
    {
        slots[9] = true;
    }

    if !watered[2] && !watered[3] {
        slots[6] = true;
    } else if watered[2]
        && watered[3]
        && !is_watered_tile(map, offset(coord, 1, -1), WateredFrom::E)
    {
        slots[10] = true;
    }

    if !watered[3] && !watered[0] {
        slots[7] = true;
    } else if watered[3]
        && watered[0]
        && !is_watered_tile(map, offset(coord, -1, -1), WateredFrom::S)
    {
        slots[11] = true;
    }

    slots
}

/// Selecciona los slots de dique de `DrawWaterEdges(true, 0, tile)`.
#[must_use]
#[cfg(test)]
pub(crate) fn canal_dike_slots(map: &Map, coord: TileCoord) -> [bool; 12] {
    water_edge_slots(map, coord, WaterClass::Canal)
}

/// Selecciona los slots de borde de `DrawWaterEdges(false, offset, tile)`.
#[must_use]
#[cfg(test)]
pub(crate) fn river_edge_slots(map: &Map, coord: TileCoord) -> [bool; 12] {
    water_edge_slots(map, coord, WaterClass::River)
}

/// Offset de los bordes River para la pendiente activa de `DrawRiverWater`.
///
/// OpenTTD sólo avanza a los bloques de 12 sprites cuando la pendiente
/// proviene de un feature `CF_RIVER_SLOPE` con una vista materializable. Un
/// reemplazo Action5 de `SPR_CANALS_BASE` conserva el bloque plano de bordes.
#[must_use]
pub(crate) fn river_edge_sprite_offset(
    map: &Map,
    ctx: &TileRenderContext,
    canal_features: &[openttdrs_core::CanalFeatureDef],
) -> usize {
    let Some(_) = river_slope_sprite_index(ctx.info.tileh) else {
        return 0;
    };
    let Some(feature) =
        openttdrs_core::canal_feature_def(canal_features, openttdrs_core::CF_RIVER_SLOPE)
    else {
        return 0;
    };
    // `DrawRiverWater` advances the edge block when `GetCanalSprite` finds a
    // custom base, not only when the preview table happens to contain the
    // selected slope. Runtime Action2 can expose that base only for the
    // current tile, so evaluate slot zero with the same Canal scope used by
    // the ground pass.
    let mut action2 = canal_action2_context_for_tile(map, ctx);
    let custom_base = feature
        .newgrf_view_runtime(0, &mut action2)
        .or_else(|| feature.newgrf_views.first().cloned())
        .is_some();
    if !custom_base {
        return 0;
    }
    match ctx.info.tileh {
        SLOPE_SE => 12,
        SLOPE_NE => 24,
        SLOPE_SW => 36,
        SLOPE_NW => 48,
        _ => 0,
    }
}

/// Emite `DrawWaterEdges` con vistas Action1/3 y fallback Action5/vanilla.
#[allow(clippy::too_many_arguments)]
fn spawn_water_edges_with_action5(
    commands: &mut Commands,
    map: &Map,
    assets: &WorldAssets,
    ctx: &TileRenderContext,
    base_z: u8,
    role: &'static str,
    expected_class: WaterClass,
    feature_id: u8,
    feature_offset: usize,
    fallback_action5_offset: Option<usize>,
    canal_features: &[openttdrs_core::CanalFeatureDef],
    canal_action5: &[Option<DecodedSprite>],
    mut action5_sprites: Option<&mut crate::render::NewGrfAction5SpriteCache>,
    mut images: Option<&mut Assets<Image>>,
) {
    let slots = water_edge_slots(map, ctx.coord, expected_class);
    for (slot, selected) in slots.into_iter().enumerate() {
        if !selected {
            continue;
        }
        let mut action2 = canal_action2_context_for_tile(map, ctx);
        let feature_custom = canal_feature_sprite_with_context_and_slot(
            canal_features,
            feature_id,
            feature_offset + slot,
            &mut action5_sprites,
            &mut images,
            &mut action2,
        );
        let custom = feature_custom.or_else(|| {
            fallback_action5_offset.and_then(|offset| {
                action5_canal_sprite(
                    offset + slot,
                    canal_action5,
                    &mut action5_sprites,
                    &mut images,
                )
                .map(|(sprite, decoded)| (sprite, decoded, feature_offset + slot))
            })
        });
        let trace_sprite_id = custom.as_ref().map_or_else(
            || canal_feature_trace_sprite_id(feature_id, feature_offset + slot),
            |(_, _, selected_slot)| canal_feature_trace_sprite_id(feature_id, *selected_slot),
        );
        let (sprite, width, height, xrel, yrel): (Sprite, f32, f32, f32, f32) =
            if let Some((sprite, decoded, _)) = custom {
                (
                    sprite,
                    f32::from(decoded.width),
                    f32::from(decoded.height),
                    f32::from(decoded.x_offs),
                    f32::from(decoded.y_offs),
                )
            } else if fallback_action5_offset.is_some() {
                let Some(&(width, height, xrel, yrel)) =
                    crate::sprites::WATER_CANAL_DIKE_SPRITE_META.get(slot)
                else {
                    continue;
                };
                (
                    assets.canal_dikes[slot].sprite(),
                    f32::from(width),
                    f32::from(height),
                    f32::from(xrel),
                    f32::from(yrel),
                )
            } else {
                continue;
            };
        WorldDrawTrace::record_sprite(role, "ground", trace_sprite_id, false);
        let layer = 0.010 + slot as f32 * 0.0001;
        let mut position = overlay_pos(
            ctx.iso_pos,
            xrel,
            yrel,
            width,
            height,
            base_z,
            layer,
            ctx.tx_i32(),
            ctx.ty_i32(),
        );
        position.z = ground_draw_z(ctx.tx_i32(), ctx.ty_i32(), layer);
        commands.spawn((
            MapVisualLayer,
            ctx.map_tile_chunk(),
            sprite,
            Transform::from_translation(position),
        ));
    }
}

/// Emite `DrawWaterEdges(true, 0, tile)` consumiendo reemplazos Action5
/// `Canals` y vistas Action1/3 de `CF_DIKES`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn spawn_canal_dikes_with_action5(
    commands: &mut Commands,
    map: &Map,
    assets: &WorldAssets,
    ctx: &TileRenderContext,
    base_z: u8,
    role: &'static str,
    canal_features: &[openttdrs_core::CanalFeatureDef],
    canal_action5: &[Option<DecodedSprite>],
    action5_sprites: Option<&mut crate::render::NewGrfAction5SpriteCache>,
    images: Option<&mut Assets<Image>>,
) {
    spawn_water_edges_with_action5(
        commands,
        map,
        assets,
        ctx,
        base_z,
        role,
        WaterClass::Canal,
        openttdrs_core::CF_DIKES,
        0,
        Some(CANALS_ACTION5_DIKES_OFFSET),
        canal_features,
        canal_action5,
        action5_sprites,
        images,
    );
}

/// Emite `DrawWaterEdges(false, offset, tile)` usando `CF_RIVER_EDGE`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn spawn_river_edges(
    commands: &mut Commands,
    map: &Map,
    assets: &WorldAssets,
    ctx: &TileRenderContext,
    base_z: u8,
    role: &'static str,
    feature_offset: usize,
    canal_features: &[openttdrs_core::CanalFeatureDef],
    action5_sprites: Option<&mut crate::render::NewGrfAction5SpriteCache>,
    images: Option<&mut Assets<Image>>,
) {
    spawn_water_edges_with_action5(
        commands,
        map,
        assets,
        ctx,
        base_z,
        role,
        WaterClass::River,
        openttdrs_core::CF_RIVER_EDGE,
        feature_offset,
        None,
        canal_features,
        &[],
        action5_sprites,
        images,
    );
}

#[cfg(test)]
#[allow(clippy::too_many_arguments)]
pub(crate) fn push_water_tile(
    commands: &mut Commands,
    map: &Map,
    map_dims: (u32, u32),
    assets: &WorldAssets,
    ctx: &TileRenderContext,
    debug_coast: bool,
    batches: &mut MapSpriteBatches,
    shore_newgrf: &[Option<DecodedSprite>],
    shore_sprites: Option<&mut NewGrfShoreSpriteCache>,
    images: Option<&mut Assets<Image>>,
) {
    push_water_tile_with_action5(
        commands,
        map,
        map_dims,
        assets,
        ctx,
        debug_coast,
        batches,
        shore_newgrf,
        shore_sprites,
        images,
        &[],
        &[],
        None,
    );
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn push_water_tile_with_action5(
    commands: &mut Commands,
    map: &Map,
    map_dims: (u32, u32),
    assets: &WorldAssets,
    ctx: &TileRenderContext,
    debug_coast: bool,
    batches: &mut MapSpriteBatches,
    shore_newgrf: &[Option<DecodedSprite>],
    shore_sprites: Option<&mut NewGrfShoreSpriteCache>,
    mut images: Option<&mut Assets<Image>>,
    canal_features: &[openttdrs_core::CanalFeatureDef],
    canal_action5: &[Option<DecodedSprite>],
    mut action5_sprites: Option<&mut crate::render::NewGrfAction5SpriteCache>,
) {
    if ctx.info.use_shore {
        // `DrawShoreTile(tileh)` — igual que OpenTTD: pendiente real del 2×2
        // cuando no es plana; si no, vecinos de tierra (`infer_coast`).
        let th = shore_tileh_for_draw_shore(map, ctx.tx, ctx.ty, map_dims.0, map_dims.1);
        if th != 0 {
            let si = shore_png_index(th);
            // `DrawShoreTile` siempre entrega el slot Action5 global
            // `SPR_SHORE_BASE + tileh_to_shoresprite[tileh]`. Aunque el
            // cache lo materialice como NewGRF, éste continúa siendo el ID
            // lógico que expone el oráculo C++.
            WorldDrawTrace::record_sprite("water-shore", "ground", shore_sprite_id(th), false);
            let mut position = tile_pos_half(
                ctx.tx_i32(),
                ctx.ty_i32(),
                ctx.info.base_z,
                SHORE_LAYER_FRAC,
                shore_sprite_half_h(th),
            );
            // `DrawShoreTile` comparte el xrel=-31 de los ground sprites;
            // no hereda el centro geométrico -32 del Sprite de Bevy.
            position.x += GROUND_SPRITE_CENTER_X_OFFSET;
            let transform = Transform::from_translation(position);
            let mut used_newgrf = false;
            let sprite = if let (Some(cache), Some(images), Some(decoded)) = (
                shore_sprites,
                images,
                shore_newgrf.get(si).and_then(|s| s.as_ref()),
            ) {
                used_newgrf = true;
                let handle = cache.handle_for(si as u8, decoded, images);
                Sprite {
                    image: handle,
                    color: Color::WHITE,
                    ..default()
                }
            } else {
                assets.shore[si].sprite()
            };
            let shore_marker = if used_newgrf {
                crate::render::ShoreTile(si as u8 | NEWGRF_SHORE_TILE_FLAG)
            } else {
                crate::render::ShoreTile(si as u8)
            };
            // Coast en OpenTTD dibuja solo `DrawShoreTile`: el PNG del set
            // completo ya incluye agua/tierra del rombo, con transparencia
            // solo fuera de él.
            batches
                .shore
                .push((ctx.map_tile_chunk(), shore_marker, sprite, transform));
            if debug_coast {
                let (raw, _) = tile_slope_bits_from_heights(map, ctx.tx, ctx.ty);
                spawn_coast_debug_label(commands, ctx, raw, th, si);
            }
        } else {
            // Datos inválidos: OpenTTD asertea que Coast no es flat. Evitamos un hueco.
            WorldDrawTrace::record_sprite(
                "water-ground-fallback",
                "ground",
                SPR_FLAT_WATER_TILE,
                true,
            );
            push_water_sprite(&mut batches.water, &assets.water, ctx);
        }
    } else {
        // Agua libre (Clear) o esclusa (m5 subtype Lock = 2).
        let m5 = ctx.tile.map(|t| t.m5).unwrap_or(0);
        if (m5 >> 4) & 0x0F == 2 {
            if let Some((sprite, transform, offset)) = lock_water_ground_sprite(
                map,
                ctx,
                canal_features,
                action5_sprites.as_deref_mut(),
                images.as_deref_mut(),
            ) {
                WorldDrawTrace::record_sprite(
                    "water-lock-ground",
                    "ground",
                    SPR_LOCK_WATER_BASE + offset as u32,
                    false,
                );
                batches.water.push((
                    ctx.map_tile_chunk(),
                    crate::render::WaterTile::STATIC,
                    sprite,
                    transform,
                ));
                spawn_lock_structures(
                    commands,
                    map,
                    assets,
                    ctx,
                    map_dims.0,
                    canal_features,
                    canal_action5,
                    action5_sprites.as_deref_mut(),
                    images.as_deref_mut(),
                );
            } else {
                let axis = usize::from(m5 & 1).min(1);
                let level = openttdrs_core::lock_sprite_level(map, ctx.coord).min(2);
                let half_h = if ctx.info.tileh == 0 {
                    TILE_HALF_H
                } else {
                    slope_half_h(ctx.info.tileh)
                };
                batches.water.push((
                    ctx.map_tile_chunk(),
                    crate::render::WaterTile::STATIC,
                    assets.water_lock[axis][level].sprite(),
                    Transform::from_translation(tile_pos_half(
                        ctx.tx_i32(),
                        ctx.ty_i32(),
                        ctx.info.base_z,
                        0.02,
                        half_h,
                    )),
                ));
            }
        } else if ctx.tile.and_then(water_class) == Some(WaterClass::River) {
            let river_slope = push_river_slope_sprite(
                &mut batches.water,
                map,
                assets,
                ctx,
                canal_features,
                canal_action5,
                action5_sprites.as_deref_mut(),
                images.as_deref_mut(),
            );
            if !river_slope {
                if let Some((sprite, transform, selected_slot)) = canal_feature_surface(
                    ctx,
                    map,
                    openttdrs_core::CF_RIVER_SLOPE,
                    canal_features,
                    action5_sprites.as_deref_mut(),
                    images.as_deref_mut(),
                ) {
                    WorldDrawTrace::record_sprite(
                        "water-ground",
                        "ground",
                        canal_feature_trace_sprite_id(
                            openttdrs_core::CF_RIVER_SLOPE,
                            selected_slot,
                        ),
                        false,
                    );
                    batches.water.push((
                        ctx.map_tile_chunk(),
                        WaterTile::STATIC,
                        sprite,
                        transform,
                    ));
                } else {
                    // `DrawRiverWater` usa `SPR_FLAT_WATER_TILE` en un río
                    // plano si CF_RIVER_SLOPE no publica el sprite plano.
                    WorldDrawTrace::record_sprite(
                        "water-ground",
                        "ground",
                        SPR_FLAT_WATER_TILE,
                        false,
                    );
                    push_water_sprite(&mut batches.water, &assets.water, ctx);
                }
            }
            spawn_river_edges(
                commands,
                map,
                assets,
                ctx,
                ctx.info.base_z,
                "water-river-edge",
                river_edge_sprite_offset(map, ctx, canal_features),
                canal_features,
                action5_sprites.as_deref_mut(),
                images.as_deref_mut(),
            );
        } else {
            let canal_surface = if ctx.tile.and_then(water_class) == Some(WaterClass::Canal) {
                canal_feature_surface(
                    ctx,
                    map,
                    openttdrs_core::CF_WATERSLOPE,
                    canal_features,
                    action5_sprites.as_deref_mut(),
                    images.as_deref_mut(),
                )
            } else {
                None
            };
            if let Some((sprite, transform, selected_slot)) = canal_surface {
                WorldDrawTrace::record_sprite(
                    "water-ground",
                    "ground",
                    canal_feature_trace_sprite_id(openttdrs_core::CF_WATERSLOPE, selected_slot),
                    false,
                );
                batches
                    .water
                    .push((ctx.map_tile_chunk(), WaterTile::STATIC, sprite, transform));
            } else {
                // `DrawSeaWater` usa directamente `SPR_FLAT_WATER_TILE` y el
                // canal cae al mismo fallback si no publica CF_WATERSLOPE.
                WorldDrawTrace::record_sprite("water-ground", "ground", SPR_FLAT_WATER_TILE, false);
                push_water_sprite(&mut batches.water, &assets.water, ctx);
            }
            if ctx.tile.and_then(water_class) == Some(WaterClass::Canal) {
                spawn_canal_dikes_with_action5(
                    commands,
                    map,
                    assets,
                    ctx,
                    ctx.info.base_z,
                    "water-canal",
                    canal_features,
                    canal_action5,
                    action5_sprites,
                    images,
                );
            }
        }
    }
}

/// Emite `DrawWaterClassGround` para el ground de un objeto NewGRF construido
/// sobre agua.
///
/// `MP_OBJECT` no entra por el dispatcher normal de `MP_WATER`, pero conserva
/// la clase en `M1`. Reutilizar esta ruta mantiene la superficie, los doce
/// slots de borde y los reemplazos Action2/Action5 alineados con el agua que
/// OpenTTD dibuja para una industria, estación o depósito naval.
#[allow(clippy::too_many_arguments)]
pub(crate) fn push_object_water_ground_with_action5(
    commands: &mut Commands,
    map: &Map,
    assets: &WorldAssets,
    ctx: &TileRenderContext,
    batches: &mut MapSpriteBatches,
    canal_features: &[openttdrs_core::CanalFeatureDef],
    canal_action5: &[Option<DecodedSprite>],
    action5_sprites: Option<&mut crate::render::NewGrfAction5SpriteCache>,
    images: Option<&mut Assets<Image>>,
) -> bool {
    let Some(tile) = ctx.tile else {
        return false;
    };
    if !has_tile_water_ground(tile) {
        return false;
    }
    push_water_tile_with_action5(
        commands,
        map,
        map.dimensions(),
        assets,
        ctx,
        false,
        batches,
        &[],
        None,
        images,
        canal_features,
        canal_action5,
        action5_sprites,
    );
    true
}

#[cfg(test)]
mod tests {
    use super::{
        SPR_CANAL_DIKES_BASE, SPR_FLAT_WATER_TILE, WateredFrom, action5_canal_sprite,
        canal_action2_context, canal_dike_slots, canal_feature_sprite,
        canal_feature_sprite_with_context, canal_feature_trace_sprite_id, is_watered_tile,
        lock_structure_layer, lock_water_ground_sprite, river_edge_slots, river_edge_sprite_offset,
        river_slope_sprite_index, shore_sprite_id,
    };
    use bevy::prelude::{Assets, Image};
    use openttdrs_core::map::{
        MP_OBJECT_MAPT, Map, Tile, TileCoord, TileKind, WaterClass, make_water_tile,
        set_water_class_m1,
    };
    use openttdrs_core::newgrf_sprites::{
        Action2EvalCtx, Action2VarAdjust, Action2VarEntry, Action2VarTerm, TrainSpriteAssign,
        TrainSpriteGraphics,
    };
    use openttdrs_core::station::STATION_TYPE_OILRIG;
    use openttdrs_core::{
        CanalFeatureDef, Climate, DecodedSprite, SLOPE_NE, SLOPE_NW, SLOPE_SE, SLOPE_SW,
    };

    fn canal_depot(map: &mut Map, coord: TileCoord) {
        let mut tile = map.get(coord).expect("canal depot tile");
        tile.kind = TileKind::ShipDepot;
        tile.m5 = 0x30;
        tile.m1 = set_water_class_m1(tile.m1, WaterClass::Canal);
        map.set_tile(coord, tile).expect("set canal depot");
    }

    #[test]
    fn canal_context_uses_persisted_climate_terrain() {
        let mut map = Map::new_flat(1, 1, 12);
        let coord = TileCoord::new(0, 0);
        let mut tile = map.get(coord).expect("fixture tile");
        tile.mapt = 2;
        map.set_tile(coord, tile).expect("set tropic zone");

        let tropical = canal_action2_context(
            map.get(coord),
            0,
            Climate::SubTropical,
            openttdrs_core::DEF_SNOW_LINE_HEIGHT,
        );
        assert_eq!(tropical.vars.get(&0x81), Some(&2));

        let arctic = canal_action2_context(map.get(coord), 0, Climate::SubArctic, 10);
        assert_eq!(arctic.vars.get(&0x81), Some(&4));

        tile.height = 10;
        map.set_tile(coord, tile).expect("set below snow line");
        let below_snow = canal_action2_context(map.get(coord), 0, Climate::SubArctic, 10);
        assert_eq!(below_snow.vars.get(&0x81), Some(&0));
    }

    #[test]
    fn canal_context_exposes_random_only_for_mp_water() {
        let mut map = Map::new_flat(1, 1, 0);
        let coord = TileCoord::new(0, 0);
        let mut tile = map.get(coord).expect("fixture tile");
        tile.m3hi = 0xA5;

        for kind in [TileKind::Water, TileKind::ShipDepot] {
            tile.kind = kind;
            map.set_tile(coord, tile).expect("set water semantic kind");
            let context = canal_action2_context(
                map.get(coord),
                0,
                Climate::Temperate,
                openttdrs_core::DEF_SNOW_LINE_HEIGHT,
            );
            assert_eq!(context.random_bits, 0xA5, "{kind:?} is backed by MP_WATER");
            assert_eq!(context.vars.get(&0x83), Some(&0xA5));
        }

        for kind in [
            TileKind::Station,
            TileKind::Industry,
            TileKind::Forest,
            TileKind::Unknown(10),
        ] {
            tile.kind = kind;
            map.set_tile(coord, tile)
                .expect("set non-water semantic kind");
            let context = canal_action2_context(
                map.get(coord),
                0,
                Climate::Temperate,
                openttdrs_core::DEF_SNOW_LINE_HEIGHT,
            );
            assert_eq!(context.random_bits, 0, "{kind:?} is not MP_WATER");
            assert_eq!(context.vars.get(&0x83), Some(&0));
        }
    }

    #[test]
    fn canal_context_lowers_only_upper_lock_height() {
        let mut map = Map::new_flat(1, 1, 12);
        let coord = TileCoord::new(0, 0);
        let mut tile = map.get(coord).expect("fixture tile");
        tile.kind = TileKind::Water;
        tile.m5 = 0x28; // WaterTileType::Lock + LockPart::Upper.
        map.set_tile(coord, tile).expect("set upper lock");

        let upper = canal_action2_context(map.get(coord), 0, Climate::Temperate, 12);
        assert_eq!(upper.vars.get(&0x80), Some(&11));

        tile.m5 = 0x24; // WaterTileType::Lock + LockPart::Lower.
        map.set_tile(coord, tile).expect("set lower lock");
        let lower = canal_action2_context(map.get(coord), 0, Climate::Temperate, 12);
        assert_eq!(lower.vars.get(&0x80), Some(&12));
    }

    #[test]
    fn lock_ground_consumes_water_slope_slots_and_legacy_fallback() {
        let mut map = Map::new_flat(1, 1, 12);
        let coord = TileCoord::new(0, 0);
        let mut tile = map.get(coord).expect("fixture lock tile");
        tile.kind = TileKind::Water;
        tile.m5 = 0x20; // middle, NE sequence.
        map.set_tile(coord, tile).expect("set middle lock");
        let grid = crate::render::RenderGrid::from_map(&map, 1, 1);

        let mut features = openttdrs_core::vanilla_canal_feature_catalog();
        features[usize::from(openttdrs_core::CF_WATERSLOPE)].flags =
            openttdrs_core::CFF_HAS_FLAT_SPRITE;
        features[usize::from(openttdrs_core::CF_WATERSLOPE)].newgrf_views = (0..5)
            .map(|slot| DecodedSprite {
                width: 1,
                height: 1,
                x_offs: 0,
                y_offs: 0,
                rgba: vec![slot as u8, 0, 0, 255],
                mask: Vec::new(),
            })
            .collect();

        let mut cache = crate::render::NewGrfAction5SpriteCache::default();
        let mut images = Assets::<Image>::default();

        let ctx = crate::render::TileRenderContext::new(&map, &grid, 0, 0);
        let (sprite, transform, offset) =
            lock_water_ground_sprite(&map, &ctx, &features, Some(&mut cache), Some(&mut images))
                .expect("middle lock consumes CF_WATERSLOPE");
        assert_eq!(offset, 2, "flat slot precedes the NE middle slot");
        assert_eq!(
            images
                .get(&sprite.image)
                .and_then(|image| image.data.as_deref()),
            Some(&[2, 0, 0, 255][..])
        );
        assert_eq!(
            transform.translation.z,
            crate::iso::ground_draw_z(0, 0, 0.02)
        );

        tile.m5 = 0x24; // lower, NE sequence.
        map.set_tile(coord, tile).expect("set lower lock");
        let ctx = crate::render::TileRenderContext::new(&map, &grid, 0, 0);
        let (sprite, _, offset) =
            lock_water_ground_sprite(&map, &ctx, &features, Some(&mut cache), Some(&mut images))
                .expect("lower lock consumes the flat CF_WATERSLOPE slot");
        assert_eq!(offset, 0);
        assert_eq!(
            images
                .get(&sprite.image)
                .and_then(|image| image.data.as_deref()),
            Some(&[0, 0, 0, 255][..])
        );

        features[usize::from(openttdrs_core::CF_WATERSLOPE)].flags = 0;
        features[usize::from(openttdrs_core::CF_WATERSLOPE)]
            .newgrf_views
            .truncate(4);
        tile.m5 = 0x20;
        map.set_tile(coord, tile).expect("restore middle lock");
        let ctx = crate::render::TileRenderContext::new(&map, &grid, 0, 0);
        let (_, _, offset) =
            lock_water_ground_sprite(&map, &ctx, &features, Some(&mut cache), Some(&mut images))
                .expect("legacy middle lock consumes the four slope slots");
        assert_eq!(offset, 1);

        tile.m5 = 0x24;
        map.set_tile(coord, tile).expect("set legacy lower lock");
        let ctx = crate::render::TileRenderContext::new(&map, &grid, 0, 0);
        assert!(
            lock_water_ground_sprite(&map, &ctx, &features, Some(&mut cache), Some(&mut images),)
                .is_none()
        );
    }

    #[test]
    fn lock_structure_layers_match_water_land_sequences() {
        let expected = [
            [(0, 0, 1, 16, 1, 6), (0, 15, 5, 16, 1, 10)],
            [(0, 0, 0, 1, 16, 6), (15, 0, 4, 1, 16, 10)],
            [(0, 0, 2, 16, 1, 6), (0, 15, 6, 16, 1, 10)],
            [(0, 0, 3, 1, 16, 6), (15, 0, 7, 1, 16, 10)],
        ];
        for (direction, expected_layers) in expected.into_iter().enumerate() {
            for (layer, expected) in expected_layers.into_iter().enumerate() {
                let actual = lock_structure_layer(0, direction, layer).expect("middle layer");
                assert_eq!(
                    (
                        actual.dx,
                        actual.dy,
                        actual.image_offset,
                        actual.extent_x,
                        actual.extent_y,
                        actual.extent_z,
                    ),
                    expected
                );
            }
        }
        assert_eq!(
            lock_structure_layer(1, 0, 0)
                .expect("lower NE layer")
                .image_offset,
            9
        );
        assert_eq!(
            lock_structure_layer(2, 3, 1)
                .expect("upper NW layer")
                .image_offset,
            23
        );
    }

    #[test]
    fn water_trace_sprite_ids_follow_openttd_water_and_shore_tables() {
        assert_eq!(SPR_FLAT_WATER_TILE, 4061);
        assert_eq!(SPR_CANAL_DIKES_BASE, 5380);
        assert_eq!(
            canal_feature_trace_sprite_id(openttdrs_core::CF_RIVER_SLOPE, 3),
            5331
        );
        assert_eq!(
            canal_feature_trace_sprite_id(openttdrs_core::CF_RIVER_EDGE, 24),
            5352
        );
        assert_eq!(
            canal_feature_trace_sprite_id(openttdrs_core::CF_DIKES, 7),
            5387
        );
        assert_eq!(shore_sprite_id(1), 5937); // SLOPE_W.
        assert_eq!(shore_sprite_id(23), 5936); // SLOPE_STEEP_S -> slot 0.
        assert_eq!(shore_sprite_id(27), 5941); // SLOPE_STEEP_N -> slot 5.
        assert_eq!(shore_sprite_id(30), 5951); // SLOPE_STEEP_E -> slot 15.
    }

    #[test]
    fn river_slopes_follow_draw_river_water_sprite_order() {
        assert_eq!(river_slope_sprite_index(SLOPE_SE), Some(0)); // Y_UP.
        assert_eq!(river_slope_sprite_index(SLOPE_NE), Some(1)); // X_DOWN.
        assert_eq!(river_slope_sprite_index(SLOPE_SW), Some(2)); // X_UP.
        assert_eq!(river_slope_sprite_index(SLOPE_NW), Some(3)); // Y_DOWN.
        assert_eq!(river_slope_sprite_index(0), None);
        assert_eq!(river_slope_sprite_index(0x0F), None);
    }

    #[test]
    fn river_edges_emit_the_same_sides_and_outer_corners_as_canals() {
        let mut map = Map::new_flat(3, 3, 0);
        let center = TileCoord::new(1, 1);
        make_water_tile(&mut map, center, WaterClass::River).expect("river tile");

        let slots = river_edge_slots(&map, center);
        assert_eq!(&slots[..8], &[true; 8]);
        assert_eq!(&slots[8..], &[false; 4]);
    }

    #[test]
    fn watered_industry_source_offsets_match_openttd_directions() {
        let directions = [
            (WateredFrom::Sw, (1, 0)),
            (WateredFrom::Nw, (0, -1)),
            (WateredFrom::Ne, (-1, 0)),
            (WateredFrom::Se, (0, 1)),
            (WateredFrom::W, (1, -1)),
            (WateredFrom::N, (-1, -1)),
            (WateredFrom::E, (-1, 1)),
            (WateredFrom::S, (1, 1)),
        ];
        let target = TileCoord::new(2, 2);

        for (from, (dx, dy)) in directions {
            let mut map = Map::new_flat(5, 5, 0);
            let mut target_tile = map.get(target).expect("industry target");
            target_tile.kind = TileKind::Industry;
            target_tile.m1 = set_water_class_m1(target_tile.m1, WaterClass::Invalid);
            target_tile.m2 = 7;
            map.set_tile(target, target_tile)
                .expect("set industry target");

            let source_coord = TileCoord::new(target.x + dx, target.y + dy);
            let mut source_tile = map.get(source_coord).expect("industry source");
            source_tile.kind = TileKind::Industry;
            source_tile.m1 = set_water_class_m1(source_tile.m1, WaterClass::Invalid);
            source_tile.m2 = 7;
            map.set_tile(source_coord, source_tile)
                .expect("set industry source");

            assert!(
                is_watered_tile(&map, target, from),
                "same-industry source was not found for {from:?}"
            );

            source_tile.m2 = 8;
            map.set_tile(source_coord, source_tile)
                .expect("set unrelated industry source");
            assert!(
                !is_watered_tile(&map, target, from),
                "unrelated industry suppressed the border for {from:?}"
            );
        }
    }

    #[test]
    fn watered_industry_and_oilrig_sources_hide_internal_borders() {
        let target = TileCoord::new(2, 2);
        let from = WateredFrom::Sw;
        let source_coord = TileCoord::new(3, 2);
        let mut map = Map::new_flat(5, 5, 0);

        let mut target_tile = map.get(target).expect("industry target");
        target_tile.kind = TileKind::Industry;
        target_tile.m1 = set_water_class_m1(target_tile.m1, WaterClass::Invalid);
        map.set_tile(target, target_tile)
            .expect("set industry target");

        let mut oilrig = map.get(source_coord).expect("oil-rig source");
        oilrig.kind = TileKind::Station;
        oilrig.m1 = set_water_class_m1(oilrig.m1, WaterClass::Invalid);
        oilrig.m6 = STATION_TYPE_OILRIG << 3;
        map.set_tile(source_coord, oilrig)
            .expect("set oil-rig source");

        assert!(is_watered_tile(&map, target, from));

        target_tile.kind = TileKind::Station;
        target_tile.m6 = STATION_TYPE_OILRIG << 3;
        map.set_tile(target, target_tile)
            .expect("set oil-rig target");
        oilrig.kind = TileKind::Industry;
        oilrig.m1 = set_water_class_m1(oilrig.m1, WaterClass::Invalid);
        map.set_tile(source_coord, oilrig)
            .expect("set industry source");

        assert!(is_watered_tile(&map, target, from));
    }

    #[test]
    fn river_edge_offset_follows_custom_slope_blocks() {
        let sprite = DecodedSprite {
            width: 1,
            height: 1,
            x_offs: 0,
            y_offs: 0,
            rgba: vec![0xFF; 4],
            mask: Vec::new(),
        };
        let mut features = openttdrs_core::vanilla_canal_feature_catalog();
        features[usize::from(openttdrs_core::CF_RIVER_SLOPE)].newgrf_views = vec![sprite; 4];
        let map = Map::new_flat(3, 3, 0);
        let grid = crate::render::RenderGrid::from_map(&map, 3, 3);
        let mut ctx = crate::render::TileRenderContext::new(&map, &grid, 1, 1);

        ctx.info.tileh = SLOPE_SE;
        assert_eq!(river_edge_sprite_offset(&map, &ctx, &features), 12);
        ctx.info.tileh = SLOPE_NE;
        assert_eq!(river_edge_sprite_offset(&map, &ctx, &features), 24);
        ctx.info.tileh = SLOPE_SW;
        assert_eq!(river_edge_sprite_offset(&map, &ctx, &features), 36);
        ctx.info.tileh = SLOPE_NW;
        assert_eq!(river_edge_sprite_offset(&map, &ctx, &features), 48);
        ctx.info.tileh = SLOPE_SE;
        assert_eq!(river_edge_sprite_offset(&map, &ctx, &[]), 0);
    }

    #[test]
    fn river_edge_offset_accepts_runtime_only_slope_base() {
        let sprite = DecodedSprite {
            width: 1,
            height: 1,
            x_offs: 0,
            y_offs: 0,
            rgba: vec![0xFF; 4],
            mask: Vec::new(),
        };
        let runtime = TrainSpriteGraphics {
            sets: vec![vec![sprite]],
            assigns: vec![TrainSpriteAssign {
                local_id: openttdrs_core::CF_RIVER_SLOPE,
                set_id: 0,
            }],
            ..TrainSpriteGraphics::default()
        };
        let mut features = openttdrs_core::vanilla_canal_feature_catalog();
        features[usize::from(openttdrs_core::CF_RIVER_SLOPE)] = CanalFeatureDef {
            id: openttdrs_core::CF_RIVER_SLOPE,
            callback_mask: 0,
            flags: 0,
            from_newgrf: true,
            grfid: 0xCAFE,
            newgrf_views: Vec::new(),
            newgrf_runtime: Some(Box::new(runtime)),
        };
        let map = Map::new_flat(3, 3, 0);
        let grid = crate::render::RenderGrid::from_map(&map, 3, 3);
        let mut ctx = crate::render::TileRenderContext::new(&map, &grid, 1, 1);
        ctx.info.tileh = SLOPE_SE;

        assert_eq!(river_edge_sprite_offset(&map, &ctx, &features), 12);
    }

    #[test]
    fn canal_dikes_emit_sides_and_outer_corners_around_isolated_depot() {
        let mut map = Map::new_flat(3, 3, 0);
        let center = TileCoord::new(1, 1);
        canal_depot(&mut map, center);

        let slots = canal_dike_slots(&map, center);
        assert_eq!(&slots[..8], &[true; 8]);
        assert_eq!(&slots[8..], &[false; 4]);
    }

    #[test]
    fn object_water_class_suppresses_canal_border_like_openttd() {
        let mut map = Map::new_flat(3, 3, 0);
        let center = TileCoord::new(1, 1);
        let neighbour = TileCoord::new(0, 1);
        canal_depot(&mut map, center);

        let mut object = map.get(neighbour).expect("object neighbour");
        object.kind = TileKind::Unknown(10);
        object.mapt = MP_OBJECT_MAPT;
        for class in [WaterClass::Sea, WaterClass::Canal, WaterClass::River] {
            object.m1 = set_water_class_m1(object.m1, class);
            map.set_tile(neighbour, object).expect("set water object");
            assert!(
                !canal_dike_slots(&map, center)[0],
                "valid object water class {class:?} keeps the shared side internal"
            );
        }

        object.m1 = set_water_class_m1(object.m1, WaterClass::Invalid);
        map.set_tile(neighbour, object).expect("set dry object");
        assert!(canal_dike_slots(&map, center)[0]);
    }

    #[test]
    fn forest_water_class_does_not_suppress_canal_border() {
        let mut map = Map::new_flat(3, 3, 0);
        let center = TileCoord::new(1, 1);
        let neighbour = TileCoord::new(0, 1);
        canal_depot(&mut map, center);

        let mut forest = map.get(neighbour).expect("forest neighbour");
        forest.kind = TileKind::Forest;
        forest.mapt = 0x40; // MP_TREES.
        for class in [WaterClass::Sea, WaterClass::Canal, WaterClass::River] {
            forest.m1 = set_water_class_m1(forest.m1, class);
            map.set_tile(neighbour, forest)
                .expect("set water-class forest");
            assert!(
                !is_watered_tile(&map, neighbour, WateredFrom::Sw),
                "MP_TREES with {class:?} must follow OpenTTD's default branch"
            );
            assert!(
                canal_dike_slots(&map, center)[0],
                "a water-class forest must not hide the depot's dike"
            );
        }
    }

    #[test]
    fn aqueduct_ramp_is_watered_only_on_its_opposite_direction() {
        let mut map = Map::new_flat(1, 1, 0);
        let coord = TileCoord::new(0, 0);
        let mut ramp = map.get(coord).expect("aqueduct ramp");
        ramp.kind = TileKind::Water;
        ramp.mapt = 0x90;
        ramp.m5 = 0x8A; // bridge + water transport + direction SW.
        map.set_tile(coord, ramp).expect("set aqueduct ramp");

        for (from, expected) in [
            (WateredFrom::Sw, false),
            (WateredFrom::Nw, false),
            (WateredFrom::Ne, true),
            (WateredFrom::Se, false),
            (WateredFrom::W, false),
            (WateredFrom::N, true),
            (WateredFrom::E, false),
            (WateredFrom::S, false),
        ] {
            assert_eq!(is_watered_tile(&map, coord, from), expected, "{from:?}");
        }

        ramp.m5 = 0x86; // same direction, road transport.
        map.set_tile(coord, ramp).expect("set road bridge ramp");
        assert!(!is_watered_tile(&map, coord, WateredFrom::Ne));
    }

    #[test]
    fn canal_dikes_use_inner_corner_when_cardinal_and_diagonal_water_differ() {
        let mut map = Map::new_flat(3, 3, 0);
        for x in 0..3 {
            for y in 0..3 {
                make_water_tile(&mut map, TileCoord::new(x, y), WaterClass::Sea)
                    .expect("water neighbour");
            }
        }
        let center = TileCoord::new(1, 1);
        canal_depot(&mut map, center);
        map.set_kind(TileCoord::new(0, 2), TileKind::Grass)
            .expect("dry diagonal");

        let slots = canal_dike_slots(&map, center);
        assert!(slots[8], "right concave corner uses slot 8");
        assert!(slots[8..].iter().skip(1).all(|slot| !slot));
        assert!(slots[..8].iter().all(|slot| !slot));
    }

    #[test]
    fn sea_depot_does_not_emit_canal_dikes() {
        let mut map = Map::new_flat(3, 3, 0);
        let center = TileCoord::new(1, 1);
        let mut tile: Tile = map.get(center).expect("sea depot tile");
        tile.kind = TileKind::ShipDepot;
        tile.m5 = 0x30;
        tile.m1 = set_water_class_m1(tile.m1, WaterClass::Sea);
        map.set_tile(center, tile).expect("set sea depot");

        assert_eq!(canal_dike_slots(&map, center), [false; 12]);
    }

    #[test]
    fn canal_action5_cache_materializes_slope_and_dike_slots_with_nfo_geometry() {
        let slope = DecodedSprite {
            width: 9,
            height: 7,
            x_offs: -4,
            y_offs: -6,
            rgba: vec![0xFF; 9 * 7 * 4],
            mask: Vec::new(),
        };
        let dike = DecodedSprite {
            width: 13,
            height: 5,
            x_offs: 8,
            y_offs: -3,
            rgba: vec![0x80; 13 * 5 * 4],
            mask: Vec::new(),
        };
        let mut table = vec![None; 64];
        table[0] = Some(slope.clone());
        table[52] = Some(dike.clone());
        let mut cache = crate::render::NewGrfAction5SpriteCache::default();
        let mut images = Assets::<Image>::default();
        let mut cache_ref = Some(&mut cache);
        let mut images_ref = Some(&mut images);

        let (_, selected_slope) = action5_canal_sprite(0, &table, &mut cache_ref, &mut images_ref)
            .expect("pendiente Action5");
        assert_eq!(selected_slope, slope);

        let (_, selected_dike) = action5_canal_sprite(52, &table, &mut cache_ref, &mut images_ref)
            .expect("dique Action5");
        assert_eq!(selected_dike, dike);
        assert_eq!(images.len(), 2, "cada slot conserva su textura cacheada");
    }

    #[test]
    fn canal_action1_3_view_uses_separate_cache_namespace_and_nfo_geometry() {
        let dike = DecodedSprite {
            width: 13,
            height: 5,
            x_offs: 8,
            y_offs: -3,
            rgba: vec![0x80; 13 * 5 * 4],
            mask: Vec::new(),
        };
        let mut features = openttdrs_core::vanilla_canal_feature_catalog();
        features[usize::from(openttdrs_core::CF_DIKES)]
            .newgrf_views
            .push(dike.clone());
        let mut action5 = vec![None; 53];
        action5[52] = Some(dike.clone());
        let mut cache = crate::render::NewGrfAction5SpriteCache::default();
        let mut images = Assets::<Image>::default();
        let mut cache_ref = Some(&mut cache);
        let mut images_ref = Some(&mut images);

        let (_, selected_action5) =
            action5_canal_sprite(52, &action5, &mut cache_ref, &mut images_ref)
                .expect("dique Action5");
        assert_eq!(selected_action5, dike);

        let (_, selected_feature) = canal_feature_sprite(
            &features,
            openttdrs_core::CF_DIKES,
            0,
            &mut cache_ref,
            &mut images_ref,
        )
        .expect("dique Action1/3");
        assert_eq!(selected_feature, dike);
        assert_eq!(images.len(), 2, "Action5 y Action1/3 no comparten handle");
    }

    #[test]
    fn canal_runtime_view_and_cache_follow_tile_context() {
        let red = DecodedSprite {
            width: 1,
            height: 1,
            x_offs: 0,
            y_offs: 0,
            rgba: vec![255, 0, 0, 255],
            mask: Vec::new(),
        };
        let green = DecodedSprite {
            width: 1,
            height: 1,
            x_offs: 0,
            y_offs: 0,
            rgba: vec![0, 255, 0, 255],
            mask: Vec::new(),
        };
        let mut runtime = TrainSpriteGraphics {
            sets: vec![vec![red.clone()], vec![green.clone()]],
            assigns: vec![TrainSpriteAssign {
                local_id: openttdrs_core::CF_DIKES,
                set_id: 0,
            }],
            ..TrainSpriteGraphics::default()
        };
        runtime.action2_var.insert(
            0,
            Action2VarEntry {
                first: Action2VarTerm {
                    variable: 0x80,
                    param: None,
                    adjust: Action2VarAdjust {
                        and_mask: 1,
                        ..Action2VarAdjust::default()
                    },
                },
                ops: Vec::new(),
                ranges: vec![(1, 1, 1)],
                default: 0,
            },
        );
        runtime.action2_to_action1.extend([(0, 0), (1, 1)]);

        let mut features = openttdrs_core::vanilla_canal_feature_catalog();
        features[usize::from(openttdrs_core::CF_DIKES)] = CanalFeatureDef {
            id: openttdrs_core::CF_DIKES,
            callback_mask: 0,
            flags: 0,
            from_newgrf: true,
            grfid: 0xCAFE,
            newgrf_views: Vec::new(),
            newgrf_runtime: Some(Box::new(runtime)),
        };
        let mut cache = crate::render::NewGrfAction5SpriteCache::default();
        let mut images = Assets::<Image>::default();
        let mut cache_ref = Some(&mut cache);
        let mut images_ref = Some(&mut images);
        let mut red_ctx = Action2EvalCtx::default();
        red_ctx.vars.insert(0x80, 0);
        let (_, selected_red) = canal_feature_sprite_with_context(
            &features,
            openttdrs_core::CF_DIKES,
            0,
            &mut cache_ref,
            &mut images_ref,
            &mut red_ctx,
        )
        .expect("vista runtime roja");
        assert_eq!(selected_red, red);

        let mut green_ctx = Action2EvalCtx::default();
        green_ctx.vars.insert(0x80, 1);
        let (_, selected_green) = canal_feature_sprite_with_context(
            &features,
            openttdrs_core::CF_DIKES,
            0,
            &mut cache_ref,
            &mut images_ref,
            &mut green_ctx,
        )
        .expect("vista runtime verde");
        assert_eq!(selected_green, green);
        assert_eq!(images.len(), 2, "cada variante conserva su textura runtime");
    }
}
