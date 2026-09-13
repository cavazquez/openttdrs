//! Fantasma de depósito de vía: vía de salida + capas BUILD (misma lógica que el mapa).

use bevy::prelude::*;
use openttdrs_core::RailType;

use crate::iso::{iso, road_depot_build_sprite_center, tile_pos_half};
use crate::render::{
    CompanyColoredSprites, WorldAssets, sprite_from_atlas_or_company_colour,
    sprite_from_company_or_asset,
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
        commands.spawn((
            BuildGhostPreview,
            sprite,
            Transform::from_translation(center).with_scale(Vec3::splat(PREVIEW_SCALE)),
        ));
    }
}
