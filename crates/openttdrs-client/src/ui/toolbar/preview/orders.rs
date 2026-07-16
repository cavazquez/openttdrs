use bevy::prelude::*;
use openttdrs_core::prelude::*;

use crate::iso::tile_pos;
use crate::state::SimWorld;
use crate::ui::toolbar::OrderEditState;
use crate::ui::toolbar::build_input::orders::order_pick_valid;

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

/// Resalta la parada bajo el cursor cuando el destino es válido.
pub(crate) fn spawn_order_pick_target_preview(
    commands: &mut Commands,
    asset_server: &AssetServer,
    sim: &SimWorld,
    order_state: &OrderEditState,
    hover: TileCoord,
) {
    let Some(vehicle_id) = order_state.vehicle_id else {
        return;
    };
    if !order_pick_valid(sim, vehicle_id, hover) {
        return;
    };
    let Some(tile) = sim.state.map.get(hover) else {
        return;
    };
    let image = asset_server.load::<Image>("assets/opengfx/tiles/grass_rough.png");
    commands.spawn((
        BuildGhostPreview,
        Sprite {
            image,
            color: Color::srgba(0.35, 1.0, 0.45, 0.55),
            ..default()
        },
        Transform::from_translation(tile_pos(hover.x, hover.y, tile.height, 5.0))
            .with_scale(Vec3::new(1.04, 1.04, 1.0)),
    ));
}
