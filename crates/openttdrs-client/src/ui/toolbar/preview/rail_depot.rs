//! Fantasma de depósito de vía: vía de salida + capas BUILD (misma lógica que el mapa).

use bevy::prelude::*;
use openttdrs_core::{Climate, Map, NewGrfEntry, RailSignalSpriteSpec, RailType, TileCoord};

use crate::iso::{iso, road_depot_build_sprite_center, tile_pos_half};
use crate::render::viewport_sort::{ParentSpriteBounds, tile_seq_parent_bounds};
use crate::render::{
    CompanyColoredSprites, NewGrfCatenarySpriteCache, NewGrfSignalSpriteCache,
    ViewportSortableParent, WorldAssets, catenary_sprite_anchor, catenary_sprite_center,
    sprite_from_atlas_or_company_colour, sprite_from_company_or_asset, viewport_insertion_key,
    viewport_source_depth,
};
use crate::sprites::{
    RAIL_DEPOT_GROUND_TRACK, catenary_depot_wire_draw, catenary_hidden,
    catenary_reference_sprite_id, catenary_sprite_atlas_key, catenary_sprite_color,
    rail_depot_build_layers, rail_depot_custom_sprite_index, rail_depot_seq_gfx,
    rail_depot_visual_type_index, remap_rail_sprite_id,
};
use crate::ui::toolbar::preview::BuildGhostPreview;

const PREVIEW_Z_BASE: f32 = 3.0;
const PREVIEW_SCALE: f32 = 1.002;

pub(crate) struct RailDepotPreviewSpawn<'a> {
    pub px: i32,
    pub py: i32,
    pub base_z: u8,
    pub half_h: f32,
    pub dir: usize,
    pub rail_type: RailType,
    pub tint: Color,
    pub asset_server: &'a AssetServer,
    pub company: Option<&'a CompanyColoredSprites>,
    pub world_assets: Option<&'a WorldAssets>,
    pub map_width: u32,
    pub catenary_newgrf: &'a [Option<openttdrs_core::DecodedSprite>],
    pub catenary_sprites: &'a mut NewGrfCatenarySpriteCache,
    pub images: &'a mut Assets<Image>,
    pub map: &'a Map,
    pub climate: Climate,
    pub calendar_date: u32,
    pub newgrf_stack: &'a [NewGrfEntry],
    pub rail_type_depot_newgrf: &'a [Option<RailSignalSpriteSpec>],
    pub signal_sprites: &'a mut NewGrfSignalSpriteCache,
}

pub(crate) fn spawn_rail_depot_preview(commands: &mut Commands, spawn: RailDepotPreviewSpawn<'_>) {
    let RailDepotPreviewSpawn {
        px,
        py,
        base_z,
        half_h,
        dir,
        rail_type,
        tint,
        asset_server,
        company,
        world_assets,
        map_width,
        catenary_newgrf,
        catenary_sprites,
        images,
        map,
        climate,
        calendar_date,
        newgrf_stack,
        rail_type_depot_newgrf,
        signal_sprites,
    } = spawn;
    let dir = dir.min(3);

    spawn_rail_depot_catenary(
        commands,
        asset_server,
        world_assets,
        px,
        py,
        base_z,
        dir,
        rail_type,
        tint,
        map_width,
        catenary_newgrf,
        catenary_sprites,
        images,
    );

    if let Some(track_id) =
        RAIL_DEPOT_GROUND_TRACK[dir].map(|id| remap_rail_sprite_id(id, rail_type))
    {
        let track_sprite = world_assets
            .and_then(|assets| assets.rail.get(&track_id))
            .map_or_else(
                || Sprite {
                    image: asset_server
                        .load::<Image>(format!("assets/opengfx/tiles/rail_{track_id}.png")),
                    color: tint,
                    ..default()
                },
                |asset| asset.sprite_colored(tint),
            );
        commands.spawn((
            BuildGhostPreview,
            track_sprite,
            Transform::from_translation(tile_pos_half(
                px,
                py,
                base_z,
                PREVIEW_Z_BASE + 0.02,
                half_h,
            ))
            .with_scale(Vec3::splat(PREVIEW_SCALE)),
        ));
    }

    for (layer_index, spec) in rail_depot_build_layers(rail_type, dir).iter().enumerate() {
        let custom_slot = rail_depot_custom_sprite_index(dir, layer_index);
        let custom_layer = rail_type_depot_newgrf
            .get(usize::from(rail_type.as_u8()))
            .and_then(Option::as_ref)
            .and_then(|custom_spec| {
                custom_slot.and_then(|image| {
                    custom_rail_depot_preview_layer(
                        map,
                        TileCoord::new(px, py),
                        rail_type,
                        climate,
                        calendar_date,
                        newgrf_stack,
                        custom_spec,
                        image,
                        signal_sprites,
                        images,
                        spec,
                        tint,
                    )
                })
            });
        let custom_sprite_id = custom_layer
            .as_ref()
            .and(custom_slot)
            .map(|image| 1063 + u32::from(image));
        let (sprite, seq, width, height) = custom_layer.unwrap_or_else(|| {
            let sprite = world_assets
                .and_then(|assets| {
                    assets.rail_depot_builds[rail_depot_visual_type_index(rail_type)][dir]
                        .get(layer_index)
                })
                .map_or_else(
                    || sprite_from_company_or_asset(company, asset_server, spec.path, tint),
                    |asset| {
                        // El runtime usa el atlas de la variante rail/mono/maglev
                        // y aplica el recolor antes de la transparencia de
                        // construcción; el fallback mantiene aislados los tests
                        // que no montan WorldAssets.
                        sprite_from_atlas_or_company_colour(company, None, asset, spec.path, tint)
                    },
                );
            (sprite, rail_depot_seq_gfx(spec), spec.w, spec.h)
        });
        let layer_z = PREVIEW_Z_BASE + spec.z;
        let center = road_depot_build_sprite_center(
            iso(px, py),
            px,
            py,
            base_z,
            layer_z,
            seq,
            width,
            height,
        );
        let source_depth = viewport_source_depth(center.z, px as u32, map_width);
        let mut position = center;
        position.z = source_depth;
        commands.spawn((
            BuildGhostPreview,
            sprite,
            Transform::from_translation(position).with_scale(Vec3::splat(PREVIEW_SCALE)),
            ViewportSortableParent {
                sprite_id: custom_sprite_id.unwrap_or(spec.sprite_id),
                bounds: rail_depot_parent_bounds(px, py, base_z, spec),
                insertion_key: viewport_insertion_key(
                    px as u32,
                    py as u32,
                    u8::try_from(layer_index + 1).unwrap_or(u8::MAX),
                ),
                source_depth,
            },
        ));
    }
}

/// Resuelve una fachada `RTSG_DEPOT` con las variables de tesela que usa el
/// renderer materializado. El tile existente sólo aporta terreno/random; el
/// tipo de vía se sustituye por el seleccionado en la herramienta, como hace
/// el comando antes de crear el depósito.
#[allow(clippy::too_many_arguments)]
fn custom_rail_depot_preview_layer(
    map: &Map,
    coord: TileCoord,
    rail_type: RailType,
    climate: Climate,
    calendar_date: u32,
    newgrf_stack: &[NewGrfEntry],
    spec: &RailSignalSpriteSpec,
    image: u8,
    signal_sprites: &mut NewGrfSignalSpriteCache,
    images: &mut Assets<Image>,
    layer: &crate::sprites::RailDepotLayerGfx,
    tint: Color,
) -> Option<(Sprite, crate::iso::RoadStopSeqGfx, f32, f32)> {
    let tile = map.get(coord)?;
    let tile = openttdrs_core::set_rail_type_on_tile(tile, rail_type);
    let mut action2 = openttdrs_core::action2_eval_ctx_for_rail_tile(
        map,
        tile,
        coord,
        climate,
        calendar_date,
        spec.type_tables.as_ref(),
    );
    action2.set_grf_params(openttdrs_core::stack_params_for_grfid(
        newgrf_stack,
        spec.grfid,
    ));
    let mut resolved = signal_sprites.sprite_for_group(spec, image, &mut action2, images)?;
    resolved.sprite.color = tint;
    let mut seq = rail_depot_seq_gfx(layer);
    seq.x_offs = resolved.center_offset.x - resolved.size.x * 0.5;
    seq.y_offs = -resolved.center_offset.y - resolved.size.y * 0.5;
    Some((resolved.sprite, seq, resolved.size.x, resolved.size.y))
}

/// Resuelve la entrada de catenaria que consumiría el mapa: Action5 local si
/// existe, atlas canónico en OpenGFX y PNG directo sólo cuando el atlas aún no
/// está montado (tests o arranque temprano del cliente).
fn preview_rail_depot_catenary_sprite(
    asset_server: &AssetServer,
    world_assets: Option<&WorldAssets>,
    sprite_id: u32,
    tint: Color,
    catenary_newgrf: &[Option<openttdrs_core::DecodedSprite>],
    catenary_sprites: &mut NewGrfCatenarySpriteCache,
    images: &mut Assets<Image>,
) -> Option<(Sprite, crate::render::CatenarySpriteAnchor)> {
    let anchor = catenary_sprite_anchor(sprite_id, catenary_newgrf)?;
    if let Some(slot) = openttdrs_core::catenary_action5_local_slot(sprite_id)
        && let Some(decoded) = catenary_newgrf.get(slot).and_then(Option::as_ref)
    {
        let image =
            catenary_sprites.handle_for(u8::try_from(slot).unwrap_or(u8::MAX), decoded, images);
        return Some((
            Sprite {
                image,
                color: tint,
                ..default()
            },
            anchor,
        ));
    }
    if let Some(asset) = world_assets.and_then(|assets| assets.rail.get(&sprite_id)) {
        return Some((asset.sprite_colored(tint), anchor));
    }
    let path = catenary_sprite_atlas_key(sprite_id)?;
    Some((
        Sprite {
            image: asset_server.load::<Image>(format!("assets/opengfx/tiles/{path}")),
            color: tint,
            ..default()
        },
        anchor,
    ))
}

/// Fantasma del cable especial de entrada de un depósito eléctrico.
///
/// Los depósitos no pasan por PCP/PPP: `DrawRailCatenary` usa directamente
/// `_rail_catenary_sprite_data_depot[dir]`. Mantener ese selector, sus bounds
/// y el ordinal sortable permite que la preview coincida con el mapa aun al
/// cruzarse con fachadas de otros tiles.
#[allow(clippy::too_many_arguments)]
fn spawn_rail_depot_catenary(
    commands: &mut Commands,
    asset_server: &AssetServer,
    world_assets: Option<&WorldAssets>,
    px: i32,
    py: i32,
    base_z: u8,
    dir: usize,
    rail_type: RailType,
    tint: Color,
    map_width: u32,
    catenary_newgrf: &[Option<openttdrs_core::DecodedSprite>],
    catenary_sprites: &mut NewGrfCatenarySpriteCache,
    images: &mut Assets<Image>,
) {
    if !rail_type.has_catenary() || catenary_hidden() {
        return;
    }
    let draw = catenary_depot_wire_draw(dir as u8);
    let tint = tint.with_alpha(tint.alpha() * catenary_sprite_color().alpha());
    let Some((sprite, anchor)) = preview_rail_depot_catenary_sprite(
        asset_server,
        world_assets,
        draw.sprite_id,
        tint,
        catenary_newgrf,
        catenary_sprites,
        images,
    ) else {
        return;
    };
    let (ox, oy, oz) = draw.bounds_origin;
    let mut position = catenary_sprite_center(
        px, py, base_z, 0.035, ox as f32, oy as f32, oz as f32, anchor,
    );
    let source_depth = viewport_source_depth(position.z, px as u32, map_width);
    position.z = source_depth;
    commands.spawn((
        BuildGhostPreview,
        sprite,
        Transform::from_translation(position),
        ViewportSortableParent {
            sprite_id: catenary_reference_sprite_id(draw.sprite_id),
            bounds: rail_depot_catenary_parent_bounds(px, py, base_z, draw),
            insertion_key: viewport_insertion_key(px as u32, py as u32, 1),
            source_depth,
        },
    ));
}

/// Caja `TILE_SEQ_LINE` que `DrawRailTileSeq` entrega al sorter runtime.
///
/// El tamaño de la imagen puede cambiar por `RTSG_DEPOT`, pero el preview
/// vanilla conserva el prisma de la capa declarada por `track_land.h`.
fn rail_depot_parent_bounds(
    px: i32,
    py: i32,
    base_z: u8,
    layer: &crate::sprites::RailDepotLayerGfx,
) -> ParentSpriteBounds {
    tile_seq_parent_bounds(
        px,
        py,
        base_z,
        layer.dx as i32,
        layer.dy as i32,
        layer.dz as i32,
        layer.sx,
        layer.sy,
        23,
    )
}

fn rail_depot_catenary_parent_bounds(
    px: i32,
    py: i32,
    base_z: u8,
    draw: crate::sprites::CatenaryWireDraw,
) -> ParentSpriteBounds {
    let (ox, oy, oz) = draw.bounds_origin;
    let (ex, ey, ez) = draw.bounds_extent;
    tile_seq_parent_bounds(px, py, base_z, ox, oy, oz, ex, ey, ez)
}

#[cfg(test)]
mod tests {
    use super::{
        NewGrfSignalSpriteCache, custom_rail_depot_preview_layer, rail_depot_build_layers,
        rail_depot_catenary_parent_bounds, rail_depot_parent_bounds,
    };
    use crate::render::viewport_sort::ParentSpriteBounds;
    use crate::sprites::catenary_depot_wire_draw;
    use bevy::prelude::{Assets, Color, Image};
    use openttdrs_core::{
        Climate, DecodedSprite, Map, RailSignalSpriteSpec, RailType, TileCoord, TrainSpriteGraphics,
    };
    use std::collections::HashMap;

    #[test]
    fn preview_parent_bounds_match_runtime_tile_seq_line() {
        let layer = rail_depot_build_layers(RailType::Rail, 0)[0];
        assert_eq!(
            rail_depot_parent_bounds(1, 1, 2, &layer),
            ParentSpriteBounds::new(18, 29, 16, 30, 29, 38)
        );
    }

    #[test]
    fn preview_catenary_bounds_follow_depot_wire_axis() {
        assert_eq!(
            rail_depot_catenary_parent_bounds(2, 3, 4, catenary_depot_wire_draw(0)),
            ParentSpriteBounds::new(32, 55, 42, 46, 55, 42)
        );
        assert_eq!(
            rail_depot_catenary_parent_bounds(2, 3, 4, catenary_depot_wire_draw(1)),
            ParentSpriteBounds::new(39, 48, 42, 39, 62, 42)
        );
    }

    #[test]
    fn custom_depot_preview_uses_relocated_slot_and_nfo_geometry() {
        let map = Map::new_flat(4, 4, 0);
        let view = DecodedSprite {
            width: 7,
            height: 9,
            x_offs: -3,
            y_offs: -5,
            rgba: vec![255, 0, 0, 255].repeat(7 * 9),
            mask: Vec::new(),
        };
        let graphics = TrainSpriteGraphics {
            sets: vec![vec![view; 6]],
            specific_assigns: HashMap::from([((0, openttdrs_core::RAIL_SPRITE_TYPE_DEPOT), 0)]),
            ..Default::default()
        };
        let spec = RailSignalSpriteSpec {
            rail_type: RailType::Rail,
            local_id: 0,
            sprite_type: openttdrs_core::RAIL_SPRITE_TYPE_DEPOT,
            grfid: 0x4445_504F,
            type_tables: None,
            graphics,
        };
        let layer = rail_depot_build_layers(RailType::Rail, 1)[1];
        let mut images = Assets::<Image>::default();
        let mut signal_sprites = NewGrfSignalSpriteCache::default();
        let (_, seq, width, height) = custom_rail_depot_preview_layer(
            &map,
            TileCoord::new(1, 1),
            RailType::Rail,
            Climate::Temperate,
            0,
            &[],
            &spec,
            1,
            &mut signal_sprites,
            &mut images,
            &layer,
            Color::WHITE,
        )
        .expect("RTSG_DEPOT preview view");

        assert_eq!((width, height), (7.0, 9.0));
        assert_eq!((seq.x_offs, seq.y_offs), (-3.0, -5.0));
        assert_eq!(images.len(), 1);
    }
}
