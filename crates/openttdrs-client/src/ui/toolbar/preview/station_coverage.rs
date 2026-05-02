use bevy::prelude::*;
use openttdrs_core::{Map, STATION_COVERAGE_RADIUS, TileCoord, station_coverage_at};

use crate::iso::tile_pos;

use super::BuildGhostPreview;

pub(crate) fn spawn_station_coverage_preview(
    commands: &mut Commands,
    asset_server: &AssetServer,
    map: &Map,
    preview_tiles: &[(i32, i32)],
    has_coverage: bool,
) {
    let Some(&(tx, ty)) = preview_tiles.first() else {
        return;
    };
    let image = asset_server.load::<Image>("assets/opengfx/tiles/grass_rough.png");
    let tint = if has_coverage {
        Color::srgba(1.0, 0.95, 0.25, 0.22)
    } else {
        Color::srgba(1.0, 0.25, 0.15, 0.2)
    };
    for y in ty - STATION_COVERAGE_RADIUS..=ty + STATION_COVERAGE_RADIUS {
        for x in tx - STATION_COVERAGE_RADIUS..=tx + STATION_COVERAGE_RADIUS {
            let Some(tile) = map.get(TileCoord::new(x, y)) else {
                continue;
            };
            commands.spawn((
                BuildGhostPreview,
                Sprite {
                    image: image.clone(),
                    color: tint,
                    ..default()
                },
                Transform::from_translation(tile_pos(x, y, tile.height, 2.5))
                    .with_scale(Vec3::new(1.002, 1.002, 1.0)),
            ));
        }
    }
}

pub(crate) fn station_preview_has_coverage(
    map: &Map,
    industries: &[openttdrs_core::Industry],
    tx: i32,
    ty: i32,
) -> bool {
    let coverage = station_coverage_at(
        map,
        industries,
        TileCoord::new(tx, ty),
        STATION_COVERAGE_RADIUS,
    );
    coverage.accepts_anything() || coverage.supplies_anything()
}
