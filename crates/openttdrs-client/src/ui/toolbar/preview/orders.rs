use bevy::prelude::*;
use openttdrs_core::Map;

use crate::iso::tile_pos;
use crate::ui::toolbar::OrderEditState;

use super::BuildGhostPreview;

pub(crate) fn spawn_order_route_preview(
    commands: &mut Commands,
    asset_server: &AssetServer,
    map: &Map,
    order_state: &OrderEditState,
) {
    if order_state.vehicle_id.is_none() {
        return;
    }
    let image = asset_server.load::<Image>("assets/opengfx/tiles/grass_rough.png");
    for (i, order) in order_state.orders.iter().enumerate() {
        let pos = order.destination();
        let Some(tile) = map.get(pos) else {
            continue;
        };
        let color = if i == 0 {
            Color::srgba(1.0, 0.95, 0.2, 0.62)
        } else {
            Color::srgba(0.2, 0.85, 1.0, 0.5)
        };
        commands.spawn((
            BuildGhostPreview,
            Sprite {
                image: image.clone(),
                color,
                ..default()
            },
            Transform::from_translation(tile_pos(pos.x, pos.y, tile.height, 4.0))
                .with_scale(Vec3::new(1.01, 1.01, 1.0)),
        ));
    }
}
