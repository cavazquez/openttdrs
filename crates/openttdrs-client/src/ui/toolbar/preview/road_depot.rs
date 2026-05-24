//! Fantasma de depósito de carretera: losa + carretera + capas BUILD (como el mapa).

use bevy::prelude::*;

use crate::iso::{iso, road_stop_build_sprite_center, tile_pos_half};
use crate::sprites::{
    ROAD_DEPOT_GROUND_PATH, road_depot_build_layers, road_depot_entrance_road_bits,
    road_depot_seq_gfx, road_flat_sprite_index,
};
use crate::ui::toolbar::preview::BuildGhostPreview;

const PREVIEW_Z_BASE: f32 = 3.0;
const PREVIEW_SCALE: f32 = 1.002;

pub(crate) struct RoadDepotPreviewSpawn<'a> {
    pub px: i32,
    pub py: i32,
    pub base_z: u8,
    pub half_h: f32,
    pub dir: usize,
    pub tint: Color,
    pub asset_server: &'a AssetServer,
}

pub(crate) fn spawn_road_depot_preview(commands: &mut Commands, spawn: RoadDepotPreviewSpawn<'_>) {
    let RoadDepotPreviewSpawn {
        px,
        py,
        base_z,
        half_h,
        dir,
        tint,
        asset_server,
    } = spawn;
    let dir = dir.min(3);

    commands.spawn((
        BuildGhostPreview,
        Sprite {
            image: asset_server.load::<Image>(ROAD_DEPOT_GROUND_PATH),
            color: tint,
            ..default()
        },
        Transform::from_translation(tile_pos_half(px, py, base_z, PREVIEW_Z_BASE, half_h))
            .with_scale(Vec3::splat(PREVIEW_SCALE)),
    ));

    let road_bits = road_depot_entrance_road_bits(dir as u8);
    let fi = road_flat_sprite_index(0, road_bits);
    commands.spawn((
        BuildGhostPreview,
        Sprite {
            image: asset_server
                .load::<Image>(format!("assets/opengfx/tiles/road_flat_{fi:02}.png")),
            color: tint,
            ..default()
        },
        Transform::from_translation(tile_pos_half(
            px,
            py,
            base_z,
            PREVIEW_Z_BASE + 0.025,
            half_h,
        ))
        .with_scale(Vec3::splat(PREVIEW_SCALE)),
    ));

    for spec in road_depot_build_layers(dir) {
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
            Sprite {
                image: asset_server.load::<Image>(spec.path),
                color: tint,
                ..default()
            },
            Transform::from_translation(center).with_scale(Vec3::splat(PREVIEW_SCALE)),
        ));
    }
}
