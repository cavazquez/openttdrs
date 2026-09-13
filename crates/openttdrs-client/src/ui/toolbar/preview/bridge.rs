//! Fantasma de puente: eje según el tramo y sprite distinto en rampas vs vano.

use bevy::prelude::*;
use openttdrs_core::{
    BridgeType, Climate, Map, NewGrfEntry, RoadTramType, RoadType, RoadTypeDef, Tile, TileCoord,
    TileKind, action2_eval_ctx_for_road_tile, calc_bridge_piece, set_road_type_on_tile,
    set_tram_road_type_on_tile, stack_params_for_grfid,
};

use crate::iso::{HEIGHT_PX, iso, overlay_pos, remap_tile_offset, tile_slope_and_min_z};
use crate::render::NewGrfRoadSpriteCache;
use crate::sprites::{BridgeDeckSpriteIds, bridge_deck_sprite_ids, bridge_sprite_meta};
use crate::ui::toolbar::BuildMenuAction;

use super::BuildGhostPreview;

const DECK_LAYER: f32 = 0.04;
const FRONT_LAYER: f32 = 0.045;
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
    pub custom_road_def: Option<&'a RoadTypeDef>,
    pub road_catalog: &'a [RoadTypeDef],
    pub climate: Climate,
    pub newgrf_stack: &'a [NewGrfEntry],
    pub road_sprites: &'a mut NewGrfRoadSpriteCache,
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
        custom_road_def,
        road_catalog,
        climate,
        newgrf_stack,
        road_sprites,
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
    let deck_z = ordered_tiles.last().map(|&(px, py)| {
        let (tileh, min_z) = tile_slope_and_min_z(map, px as u32, py as u32);
        bridge_preview_deck_z(tileh, min_z, axis)
    });
    let custom_source = custom_road_def.and_then(|def| {
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
        let (sprite_id, shift, layer) = if is_middle {
            (ids.front[axis], bridge_front_shift(axis), FRONT_LAYER)
        } else {
            (ids.rear(is_rail, axis), Vec2::ZERO, DECK_LAYER)
        };
        let draw_z = if is_middle {
            deck_z.unwrap_or(base_z)
        } else {
            base_z
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
                px, py, draw_z, sprite_id, shift, layer,
            ))
            .with_scale(Vec3::new(1.002, 1.002, 1.0)),
        ));

        if let Some((def, source_coord, source_tile)) = custom_source
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
                    CUSTOM_BRIDGE_LAYER,
                    px,
                    py,
                ))
                .with_scale(Vec3::splat(1.002)),
            ));
        }

        if let Some((def, source_coord, source_tile)) = custom_source
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

        if let Some((def, source_coord, source_tile)) = custom_source
            && def.has_catenary()
            && !crate::sprites::catenary_hidden()
        {
            let offset = bridge_offset.min(BRIDGE_ROAD_CATENARY_BACK_OFFSETS.len() - 1);
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
                        CUSTOM_CATENARY_FRONT_LAYER,
                        px,
                        py,
                    ))
                    .with_scale(Vec3::splat(1.002)),
                ));
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
}
