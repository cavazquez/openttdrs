//! Fantasma de depósito de vía: vía de salida + capas BUILD (misma lógica que el mapa).

use bevy::prelude::*;
use openttdrs_core::RailType;

use crate::iso::{iso, road_depot_build_sprite_center, tile_pos_half};
use crate::render::viewport_sort::{ParentSpriteBounds, tile_seq_parent_bounds};
use crate::render::{
    CompanyColoredSprites, ViewportSortableParent, WorldAssets,
    sprite_from_atlas_or_company_colour, sprite_from_company_or_asset, viewport_insertion_key,
    viewport_source_depth,
};
use crate::sprites::{
    RAIL_DEPOT_GROUND_TRACK, rail_depot_build_layers, rail_depot_seq_gfx,
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
    } = spawn;
    let dir = dir.min(3);

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
        let layer_z = PREVIEW_Z_BASE + spec.z;
        let center = road_depot_build_sprite_center(
            iso(px, py),
            px,
            py,
            base_z,
            layer_z,
            rail_depot_seq_gfx(spec),
            spec.w,
            spec.h,
        );
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
        let source_depth = viewport_source_depth(center.z, px as u32, map_width);
        let mut position = center;
        position.z = source_depth;
        commands.spawn((
            BuildGhostPreview,
            sprite,
            Transform::from_translation(position).with_scale(Vec3::splat(PREVIEW_SCALE)),
            ViewportSortableParent {
                sprite_id: spec.sprite_id,
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

#[cfg(test)]
mod tests {
    use super::{rail_depot_build_layers, rail_depot_parent_bounds};
    use crate::render::viewport_sort::ParentSpriteBounds;
    use openttdrs_core::RailType;

    #[test]
    fn preview_parent_bounds_match_runtime_tile_seq_line() {
        let layer = rail_depot_build_layers(RailType::Rail, 0)[0];
        assert_eq!(
            rail_depot_parent_bounds(1, 1, 2, &layer),
            ParentSpriteBounds::new(18, 29, 16, 30, 29, 38)
        );
    }
}
