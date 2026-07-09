//! Fantasma de estación de tren multi-tesela (vía + plataformas reales).

use bevy::prelude::*;
use openttdrs_core::{Map, TileCoord, rail_station_footprint, rail_station_layout};

use crate::iso::{TILE_HALF_H, iso, overlay_pos, tile_pos_half, tile_slope_and_min_z};
use crate::render::{CompanyColoredSprites, TileAtlas};
use crate::sprites::{
    rail_station_draw_layers, rail_station_ground_track_sprite, rail_station_overlay_rel,
    rail_station_sprite_meta,
};

use super::BuildGhostPreview;

const PREVIEW_Z_BASE: f32 = 2.5;
const PREVIEW_SCALE: f32 = 1.002;

#[allow(clippy::too_many_arguments)]
pub(crate) fn spawn_rail_station_area_sprite_preview(
    commands: &mut Commands,
    atlas: Option<&TileAtlas>,
    company: Option<&CompanyColoredSprites>,
    map: &Map,
    origin: TileCoord,
    axis_y: bool,
    platforms: u8,
    length: u8,
    valid: bool,
) {
    let Some(atlas) = atlas else {
        return;
    };
    let platforms = platforms.clamp(1, 7);
    let length = length.clamp(1, 7);
    let _footprint = rail_station_footprint(axis_y, platforms, length);
    let layout = rail_station_layout(usize::from(platforms), usize::from(length));
    let tint = if valid {
        Color::srgba(1.0, 1.0, 1.0, 0.7)
    } else {
        Color::srgba(1.0, 0.35, 0.3, 0.7)
    };

    for n in 0..platforms {
        for l in 0..length {
            let c = if axis_y {
                TileCoord::new(origin.x + i32::from(n), origin.y + i32::from(l))
            } else {
                TileCoord::new(origin.x + i32::from(l), origin.y + i32::from(n))
            };
            if map.get(c).is_none() {
                continue;
            }
            let idx = usize::from(n) * usize::from(length) + usize::from(l);
            let m5 = layout[idx] + u8::from(axis_y);
            spawn_one_tile(commands, atlas, company, map, c, m5, tint);
        }
    }
}

fn spawn_one_tile(
    commands: &mut Commands,
    atlas: &TileAtlas,
    company: Option<&CompanyColoredSprites>,
    map: &Map,
    coord: TileCoord,
    m5: u8,
    tint: Color,
) {
    let (_, base_z) = tile_slope_and_min_z(map, coord.x as u32, coord.y as u32);
    let origin = iso(coord.x, coord.y);
    let track_sid = rail_station_ground_track_sprite(m5, 0);
    if let Some(img) = atlas.try_get(&format!("rail_{track_sid}.png")) {
        commands.spawn((
            BuildGhostPreview,
            img.sprite_colored(tint),
            Transform::from_translation(tile_pos_half(
                coord.x,
                coord.y,
                base_z,
                PREVIEW_Z_BASE,
                TILE_HALF_H,
            ))
            .with_scale(Vec3::splat(PREVIEW_SCALE)),
        ));
    }
    for layer in rail_station_draw_layers(m5) {
        let Some(img) = atlas.try_get(&format!("rail_{}.png", layer.sprite_id)) else {
            continue;
        };
        let Some((w, h, nfo_xrel, nfo_yrel)) = rail_station_sprite_meta(layer.sprite_id) else {
            continue;
        };
        let (xrel, yrel) = rail_station_overlay_rel(layer, nfo_xrel, nfo_yrel);
        let pos3 = overlay_pos(
            origin,
            xrel,
            yrel,
            w,
            h,
            base_z,
            layer.z + PREVIEW_Z_BASE,
            coord.x,
            coord.y,
        );
        let sprite = company
            .and_then(|c| c.rail_handle(layer.sprite_id))
            .map(|handle| Sprite {
                image: handle.clone(),
                color: tint,
                ..default()
            })
            .unwrap_or_else(|| img.sprite_colored(tint));
        commands.spawn((
            BuildGhostPreview,
            sprite,
            Transform::from_translation(pos3).with_scale(Vec3::splat(PREVIEW_SCALE)),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openttdrs_core::rail_station_layout;

    #[test]
    fn preview_layout_gfx_matches_place_area() {
        let layout = rail_station_layout(1, 3);
        assert_eq!(layout, vec![0, 2, 0]);
        let m5: Vec<u8> = layout.into_iter().map(|g| g + 1).collect();
        assert_eq!(m5, vec![1, 3, 1]);
        for &gfx in &m5 {
            assert!(!rail_station_draw_layers(gfx).is_empty());
        }
    }
}
