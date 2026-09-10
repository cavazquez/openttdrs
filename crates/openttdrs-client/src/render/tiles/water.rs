use bevy::prelude::*;
use openttdrs_core::DecodedSprite;
use openttdrs_core::map::{WaterClass, has_tile_water_ground, tile_slope_and_z, water_class};
use openttdrs_core::prelude::*;
use openttdrs_core::station::{
    STATION_TYPE_BUOY, STATION_TYPE_DOCK, STATION_TYPE_OILRIG, station_type_from_m6,
};
use openttdrs_core::{SLOPE_NE, SLOPE_NW, SLOPE_SE, SLOPE_SW};

use super::{SHORE_LAYER_FRAC, push_water_sprite, spawn_coast_debug_label};
use crate::iso::{
    GROUND_SPRITE_CENTER_X_OFFSET, TILE_HALF_H, ground_draw_z, overlay_pos, shore_png_index,
    shore_sprite_half_h, shore_tileh_for_draw_shore, slope_half_h, tile_pos_half,
    tile_slope_bits_from_heights,
};
use crate::render::shore_newgrf::{NEWGRF_SHORE_TILE_FLAG, NewGrfShoreSpriteCache};
use crate::render::world_draw_trace::WorldDrawTrace;
use crate::render::{MapSpriteBatches, MapVisualLayer, TileRenderContext, WaterTile, WorldAssets};
use crate::sprites::WATER_RIVER_SLOPE_SPRITE_META;

/// `SPR_FLAT_WATER_TILE` de `table/sprites.h`.
const SPR_FLAT_WATER_TILE: u32 = 4061;
/// `SPR_CANAL_DIKES_BASE` de `table/sprites.h`.
pub(crate) const SPR_CANAL_DIKES_BASE: u32 = 5380;
/// Tipo Action5 y primer slot de los diques dentro de `0x08 Canals`.
const ACTION5_CANALS_TYPE: u8 = openttdrs_core::ACTION5_TYPE_CANALS;
const CANALS_ACTION5_DIKES_OFFSET: usize = 52;
/// `SPR_CANALS_BASE` de `table/sprites.h`: las cuatro pendientes de río
/// vanilla ocupan los slots 0..3 de la hoja Action5 de canales.
pub(crate) const SPR_RIVER_SLOPE_BASE: u32 = 5328;
/// `SPR_SHORE_BASE` resuelto por Action5 canals en OpenGFX/OpenGFX2.
const SPR_SHORE_BASE: u32 = 5936;

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
#[derive(Clone, Copy)]
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

fn offset(coord: TileCoord, dx: i32, dy: i32) -> TileCoord {
    TileCoord::new(coord.x + dx, coord.y + dy)
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

/// Equivalente del `IsWateredTile` usado por `DrawWaterEdges`.
///
/// La representación importada no conserva todavía todos los pools que
/// OpenTTD consulta para una estación petrolera o una industria compuesta;
/// para esos tipos usamos la misma señal de suelo de agua que el resto del
/// renderer. Las formas explícitas de MP_WATER (clear/coast/lock/depot) sí se
/// resuelven con sus bytes y pendiente originales.
fn is_watered_tile(map: &Map, coord: TileCoord, from: WateredFrom) -> bool {
    let Some(tile) = map.get(coord) else {
        // `MP_VOID` es agua a efectos de los bordes del mapa.
        return true;
    };

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
            STATION_TYPE_OILRIG => has_tile_water_ground(tile),
            _ => false,
        },
        TileKind::Industry | TileKind::Forest => has_tile_water_ground(tile),
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

fn push_river_slope_sprite(
    batch_water: &mut Vec<(crate::render::MapTileChunk, WaterTile, Sprite, Transform)>,
    assets: &WorldAssets,
    ctx: &TileRenderContext,
    canal_action5: &[Option<DecodedSprite>],
    mut action5_sprites: Option<&mut crate::render::NewGrfAction5SpriteCache>,
    mut images: Option<&mut Assets<Image>>,
) -> bool {
    let index = match river_slope_sprite_index(ctx.info.tileh) {
        Some(index) => index,
        None => return false,
    };
    let custom = action5_canal_sprite(index, canal_action5, &mut action5_sprites, &mut images);
    let Some((_, position)) = river_slope_draw(ctx, custom.as_ref().map(|(_, sprite)| sprite))
    else {
        return false;
    };

    let sprite_id = SPR_RIVER_SLOPE_BASE + index as u32;
    WorldDrawTrace::record_sprite("water-river-slope", "ground", sprite_id, false);
    batch_water.push((
        ctx.map_tile_chunk(),
        WaterTile::STATIC,
        custom.map_or_else(|| assets.river_slopes[index].sprite(), |(sprite, _)| sprite),
        Transform::from_translation(position),
    ));
    true
}

/// Emite el ground de `DrawWaterDepot` consumiendo Action5 `Canals`.
pub(crate) fn spawn_river_slope_ground_with_action5(
    commands: &mut Commands,
    assets: &WorldAssets,
    ctx: &TileRenderContext,
    canal_action5: &[Option<DecodedSprite>],
    mut action5_sprites: Option<&mut crate::render::NewGrfAction5SpriteCache>,
    mut images: Option<&mut Assets<Image>>,
) -> bool {
    let Some(index) = river_slope_sprite_index(ctx.info.tileh) else {
        return false;
    };
    let custom = action5_canal_sprite(index, canal_action5, &mut action5_sprites, &mut images);
    let Some((_, position)) = river_slope_draw(ctx, custom.as_ref().map(|(_, sprite)| sprite))
    else {
        return false;
    };
    let sprite_id = SPR_RIVER_SLOPE_BASE + index as u32;
    WorldDrawTrace::record_sprite("ship-depot-water", "ground", sprite_id, false);
    commands.spawn((
        MapVisualLayer,
        ctx.map_tile_chunk(),
        WaterTile::STATIC,
        custom.map_or_else(|| assets.river_slopes[index].sprite(), |(sprite, _)| sprite),
        Transform::from_translation(position),
    ));
    true
}

/// Selecciona los slots `SPR_CANAL_DIKES_BASE + 0..11` de `DrawWaterEdges`.
///
/// El orden de los índices es el del C++: cuatro lados, cuatro esquinas
/// completas y cuatro esquinas cóncavas sólo cuando el diagonal intermedio no
/// está mojado. Una tesela que no sea Canal no emite diques.
#[must_use]
pub(crate) fn canal_dike_slots(map: &Map, coord: TileCoord) -> [bool; 12] {
    let mut slots = [false; 12];
    if map.get(coord).and_then(water_class) != Some(WaterClass::Canal) {
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

/// Emite `DrawWaterEdges(true, 0, tile)` consumiendo reemplazos Action5 `Canals`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn spawn_canal_dikes_with_action5(
    commands: &mut Commands,
    map: &Map,
    assets: &WorldAssets,
    ctx: &TileRenderContext,
    base_z: u8,
    role: &'static str,
    canal_action5: &[Option<DecodedSprite>],
    mut action5_sprites: Option<&mut crate::render::NewGrfAction5SpriteCache>,
    mut images: Option<&mut Assets<Image>>,
) {
    let slots = canal_dike_slots(map, ctx.coord);
    for (slot, selected) in slots.into_iter().enumerate() {
        if !selected {
            continue;
        }
        let custom = action5_canal_sprite(
            CANALS_ACTION5_DIKES_OFFSET + slot,
            canal_action5,
            &mut action5_sprites,
            &mut images,
        );
        let (sprite, width, height, xrel, yrel): (Sprite, f32, f32, f32, f32) =
            if let Some((sprite, decoded)) = custom {
                (
                    sprite,
                    f32::from(decoded.width),
                    f32::from(decoded.height),
                    f32::from(decoded.x_offs),
                    f32::from(decoded.y_offs),
                )
            } else {
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
            };
        let sprite_id = SPR_CANAL_DIKES_BASE + slot as u32;
        WorldDrawTrace::record_sprite(role, "ground", sprite_id, false);
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
        } else if ctx.tile.and_then(water_class) == Some(WaterClass::River)
            && push_river_slope_sprite(
                &mut batches.water,
                assets,
                ctx,
                canal_action5,
                action5_sprites.as_deref_mut(),
                images.as_deref_mut(),
            )
        {
            // `DrawRiverWater` already emitted the selected slope sprite.
        } else {
            // `DrawSeaWater` usa directamente `SPR_FLAT_WATER_TILE`. Un río
            // plano también cae aquí; sólo sus cuatro pendientes cardinales
            // tienen sprites vanilla específicos.
            WorldDrawTrace::record_sprite("water-ground", "ground", SPR_FLAT_WATER_TILE, false);
            push_water_sprite(&mut batches.water, &assets.water, ctx);
            if ctx.tile.and_then(water_class) == Some(WaterClass::Canal) {
                spawn_canal_dikes_with_action5(
                    commands,
                    map,
                    assets,
                    ctx,
                    ctx.info.base_z,
                    "water-canal",
                    canal_action5,
                    action5_sprites,
                    images,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        SPR_CANAL_DIKES_BASE, SPR_FLAT_WATER_TILE, action5_canal_sprite, canal_dike_slots,
        river_slope_sprite_index, shore_sprite_id,
    };
    use bevy::prelude::{Assets, Image};
    use openttdrs_core::map::{
        Map, Tile, TileCoord, TileKind, WaterClass, make_water_tile, set_water_class_m1,
    };
    use openttdrs_core::{DecodedSprite, SLOPE_NE, SLOPE_NW, SLOPE_SE, SLOPE_SW};

    fn canal_depot(map: &mut Map, coord: TileCoord) {
        let mut tile = map.get(coord).expect("canal depot tile");
        tile.kind = TileKind::ShipDepot;
        tile.m5 = 0x30;
        tile.m1 = set_water_class_m1(tile.m1, WaterClass::Canal);
        map.set_tile(coord, tile).expect("set canal depot");
    }

    #[test]
    fn water_trace_sprite_ids_follow_openttd_water_and_shore_tables() {
        assert_eq!(SPR_FLAT_WATER_TILE, 4061);
        assert_eq!(SPR_CANAL_DIKES_BASE, 5380);
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
    fn canal_dikes_emit_sides_and_outer_corners_around_isolated_depot() {
        let mut map = Map::new_flat(3, 3, 0);
        let center = TileCoord::new(1, 1);
        canal_depot(&mut map, center);

        let slots = canal_dike_slots(&map, center);
        assert_eq!(&slots[..8], &[true; 8]);
        assert_eq!(&slots[8..], &[false; 4]);
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
}
