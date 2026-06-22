//! Fantasma de parada bus/camión: GROUND + BUILD_A/B/C (misma lógica que el mapa).

use bevy::prelude::*;

use crate::iso::{iso, road_stop_build_sprite_center, tile_pos_half};
use crate::render::{CompanyColoredSprites, sprite_from_company_or_asset};
use crate::sprites::{StationTileClass, road_stop_build_layers, road_stop_seq_gfx};
use crate::ui::toolbar::preview::BuildGhostPreview;

const PREVIEW_Z_BASE: f32 = 3.0;
const PREVIEW_SCALE: f32 = 1.002;

const BUS_STOP_GROUND_PATHS: [&str; 4] = [
    "assets/opengfx/tiles/bus_stop_ne_ground.png",
    "assets/opengfx/tiles/bus_stop_se_ground.png",
    "assets/opengfx/tiles/bus_stop_sw_ground.png",
    "assets/opengfx/tiles/bus_stop_nw_ground.png",
];

const TRUCK_STOP_GROUND_PATHS: [&str; 4] = [
    "assets/opengfx/tiles/truck_stop_ground_0.png",
    "assets/opengfx/tiles/truck_stop_ground_1.png",
    "assets/opengfx/tiles/truck_stop_ground_2.png",
    "assets/opengfx/tiles/truck_stop_ground_3.png",
];

/// Orientación de construcción (0=NE … 3=NW) → índice de suelo/capas BUILD.
#[must_use]
pub(crate) fn road_stop_preview_dir(orientation: u8) -> usize {
    usize::from(orientation.min(3))
}

#[must_use]
pub(crate) fn bus_stop_ground_path(dir: usize) -> &'static str {
    BUS_STOP_GROUND_PATHS[dir.min(3)]
}

#[must_use]
pub(crate) fn truck_stop_ground_path(dir: usize) -> &'static str {
    TRUCK_STOP_GROUND_PATHS[dir.min(3)]
}

pub(crate) struct RoadStopPreviewSpawn<'a> {
    pub px: i32,
    pub py: i32,
    pub base_z: u8,
    pub half_h: f32,
    pub class: StationTileClass,
    pub dir: usize,
    pub ground_path: &'static str,
    pub tint: Color,
    pub asset_server: &'a AssetServer,
    pub company: Option<&'a CompanyColoredSprites>,
}

pub(crate) fn spawn_road_stop_preview(commands: &mut Commands, spawn: RoadStopPreviewSpawn<'_>) {
    let RoadStopPreviewSpawn {
        px,
        py,
        base_z,
        half_h,
        class,
        dir,
        ground_path,
        tint,
        asset_server,
        company,
    } = spawn;
    commands.spawn((
        BuildGhostPreview,
        Sprite {
            image: asset_server.load::<Image>(ground_path),
            color: tint,
            ..default()
        },
        Transform::from_translation(tile_pos_half(px, py, base_z, PREVIEW_Z_BASE, half_h))
            .with_scale(Vec3::splat(PREVIEW_SCALE)),
    ));

    let dir = dir.min(3);
    for spec in road_stop_build_layers(class, dir) {
        let layer_z = PREVIEW_Z_BASE + spec.z;
        let center = road_stop_build_sprite_center(
            iso(px, py),
            px,
            py,
            base_z,
            layer_z,
            road_stop_seq_gfx(spec),
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
