//! Fantasma de parada bus/camión: GROUND + BUILD_A/B/C (misma lógica que el mapa).

use bevy::prelude::*;

use crate::iso::{iso, road_stop_build_sprite_center, tile_pos_half};
use crate::sprites::{StationTileClass, road_stop_build_layers, road_stop_seq_gfx};
use crate::ui::toolbar::preview::BuildGhostPreview;

const PREVIEW_Z_BASE: f32 = 3.0;
const PREVIEW_SCALE: f32 = 1.002;

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
            Sprite {
                image: asset_server.load::<Image>(spec.path),
                color: tint,
                ..default()
            },
            Transform::from_translation(center).with_scale(Vec3::splat(PREVIEW_SCALE)),
        ));
    }
}

#[must_use]
pub(crate) fn road_stop_preview_dir(orientation: u8) -> usize {
    usize::from(orientation.min(3))
}

#[must_use]
pub(crate) fn bus_stop_ground_path(dir: usize) -> &'static str {
    const PATHS: [&str; 4] = [
        "assets/opengfx/tiles/bus_stop_ne_ground.png",
        "assets/opengfx/tiles/bus_stop_se_ground.png",
        "assets/opengfx/tiles/bus_stop_sw_ground.png",
        "assets/opengfx/tiles/bus_stop_nw_ground.png",
    ];
    PATHS[dir.min(3)]
}

#[must_use]
pub(crate) fn truck_stop_ground_path(dir: usize) -> &'static str {
    const PATHS: [&str; 4] = [
        "assets/opengfx/tiles/truck_stop_ground_0.png",
        "assets/opengfx/tiles/truck_stop_ground_1.png",
        "assets/opengfx/tiles/truck_stop_ground_2.png",
        "assets/opengfx/tiles/truck_stop_ground_3.png",
    ];
    PATHS[dir.min(3)]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn road_stop_preview_dir_clamps() {
        assert_eq!(road_stop_preview_dir(9), 3);
    }

    #[test]
    fn bus_stop_ground_paths_cover_four_dirs() {
        for d in 0..4 {
            assert!(bus_stop_ground_path(d).contains("bus_stop"));
        }
    }
}
