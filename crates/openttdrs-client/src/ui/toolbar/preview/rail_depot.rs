//! Fantasma de depósito de vía: vía de salida + capas BUILD (misma lógica que el mapa).

use bevy::prelude::*;

use crate::iso::{iso, road_stop_build_sprite_center, tile_pos_half};
use crate::render::{CompanyColoredSprites, sprite_from_company_or_asset};
use crate::sprites::{RAIL_DEPOT_GROUND_TRACK, rail_depot_build_layers, road_depot_seq_gfx};
use crate::ui::toolbar::preview::BuildGhostPreview;

const PREVIEW_Z_BASE: f32 = 3.0;
const PREVIEW_SCALE: f32 = 1.002;

pub(crate) struct RailDepotPreviewSpawn<'a> {
    pub px: i32,
    pub py: i32,
    pub base_z: u8,
    pub half_h: f32,
    pub dir: usize,
    pub tint: Color,
    pub asset_server: &'a AssetServer,
    pub company: Option<&'a CompanyColoredSprites>,
}

pub(crate) fn spawn_rail_depot_preview(commands: &mut Commands, spawn: RailDepotPreviewSpawn<'_>) {
    let RailDepotPreviewSpawn {
        px,
        py,
        base_z,
        half_h,
        dir,
        tint,
        asset_server,
        company,
    } = spawn;
    let dir = dir.min(3);

    if let Some(track_id) = RAIL_DEPOT_GROUND_TRACK[dir] {
        commands.spawn((
            BuildGhostPreview,
            Sprite {
                image: asset_server
                    .load::<Image>(format!("assets/opengfx/tiles/rail_{track_id}.png")),
                color: tint,
                ..default()
            },
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

    for spec in rail_depot_build_layers(dir) {
        let layer_z = PREVIEW_Z_BASE + spec.z;
        let center = road_stop_build_sprite_center(
            iso(px, py),
            px,
            py,
            base_z,
            layer_z,
            road_depot_seq_gfx(spec),
            spec.w,
            spec.h,
        );
        commands.spawn((
            BuildGhostPreview,
            sprite_from_company_or_asset(company, asset_server, spec.path, tint),
            Transform::from_translation(center).with_scale(Vec3::splat(PREVIEW_SCALE)),
        ));
    }
}
