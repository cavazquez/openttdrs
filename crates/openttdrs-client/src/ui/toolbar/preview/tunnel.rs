//! Fantasma de boca de túnel: sprite y anclaje según la pendiente de la tesela.

use bevy::prelude::*;
use openttdrs_core::prelude::*;
use openttdrs_core::{inclined_slope_direction, is_tunnel_entrance_slope};

use crate::sprites::{tunnel_portal_translation, tunnel_rear_sprite_id};
use crate::ui::toolbar::BuildMenuAction;

use super::BuildGhostPreview;

pub(crate) fn spawn_tunnel_entrance_preview(
    commands: &mut Commands,
    asset_server: &AssetServer,
    map: &Map,
    action: BuildMenuAction,
    coord: TileCoord,
    valid: bool,
) {
    let Some((tileh, base_z)) = openttdrs_core::tile_slope_and_z(map, coord) else {
        return;
    };
    if !is_tunnel_entrance_slope(tileh) {
        return;
    }
    let Some(dir) = inclined_slope_direction(tileh) else {
        return;
    };
    let rail = action == BuildMenuAction::RailTunnel;
    let sprite_id = tunnel_rear_sprite_id(rail, dir);
    let path = format!(
        "assets/opengfx/tiles/{}",
        crate::sprites::tunnel_rear_atlas_name(rail, dir)
    );
    let tint = if valid {
        Color::srgba(1.0, 1.0, 1.0, 0.58)
    } else {
        Color::srgba(1.0, 0.28, 0.22, 0.58)
    };
    commands.spawn((
        BuildGhostPreview,
        Sprite {
            image: asset_server.load(path),
            color: tint,
            ..default()
        },
        Transform::from_translation(tunnel_portal_translation(
            coord.x, coord.y, base_z, sprite_id, 0.04,
        ))
        .with_scale(Vec3::new(1.002, 1.002, 1.0)),
    ));
}
