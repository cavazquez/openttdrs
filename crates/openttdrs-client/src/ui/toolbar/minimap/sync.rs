use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use openttdrs_core::prelude::*;

use crate::iso::{ISO_HW, ISO_QH, world_pos_to_tile_coord};
use crate::render::{MapPreviewCamera, PrimaryGameCamera};
use crate::state::SimWorld;
use crate::ui::hud::SimHudControls;

use super::palette::minimap_cell_color;
use super::{
    MINIMAP_BOTTOM, MINIMAP_COLS, MINIMAP_PAD, MINIMAP_RIGHT, MINIMAP_ROWS, MinimapCell,
    MinimapLayerState, MinimapLayerToggle, MinimapLegendText, MinimapRoot, MinimapViewport,
};

/// Filtros disjuntos para evitar B0001 entre root / celdas / viewport / toggles.
type MinimapRootFilter = (
    With<MinimapRoot>,
    Without<MinimapCell>,
    Without<MinimapViewport>,
    Without<MinimapLayerToggle>,
);
type MinimapCellFilter = (
    With<MinimapCell>,
    Without<MinimapRoot>,
    Without<MinimapViewport>,
    Without<MinimapLayerToggle>,
);
type MinimapToggleFilter = (
    With<MinimapLayerToggle>,
    With<Button>,
    Without<MinimapCell>,
    Without<MinimapRoot>,
    Without<MinimapViewport>,
);
type MinimapViewportFilter = (
    With<MinimapViewport>,
    Without<MinimapRoot>,
    Without<MinimapCell>,
    Without<MinimapLayerToggle>,
);

#[allow(clippy::too_many_arguments)]
pub(crate) fn sync_minimap(
    sim: Res<SimWorld>,
    hud: Res<SimHudControls>,
    layers: Res<MinimapLayerState>,
    mut root_q: Query<(&mut Visibility, &mut Node, &mut GlobalZIndex), MinimapRootFilter>,
    mut cells: Query<(&MinimapCell, &mut BackgroundColor, &mut Node), MinimapCellFilter>,
    mut toggles: Query<(&MinimapLayerToggle, &mut BackgroundColor), MinimapToggleFilter>,
    mut legend: Query<&mut Text, With<MinimapLegendText>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cam_q: Query<(&Transform, &Projection), (With<PrimaryGameCamera>, Without<MapPreviewCamera>)>,
    mut viewport_q: Query<&mut Node, MinimapViewportFilter>,
) {
    let Ok((mut vis, mut root_node, mut z)) = root_q.single_mut() else {
        return;
    };
    *vis = if hud.minimap_visible {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
    if !hud.minimap_visible {
        return;
    }

    let cell = layers.cell_px();
    let (root_w, root_h) = layers.root_size();
    root_node.width = Val::Px(root_w);
    root_node.height = Val::Px(root_h);
    if layers.expanded {
        let Ok(window) = windows.single() else {
            return;
        };
        let left = ((window.width() - root_w) * 0.5).max(8.0);
        let top = ((window.height() - root_h) * 0.5).max(8.0);
        root_node.left = Val::Px(left);
        root_node.top = Val::Px(top);
        root_node.right = Val::Auto;
        root_node.bottom = Val::Auto;
        *z = GlobalZIndex(2400);
    } else {
        root_node.left = Val::Auto;
        root_node.top = Val::Auto;
        root_node.right = Val::Px(MINIMAP_RIGHT);
        root_node.bottom = Val::Px(MINIMAP_BOTTOM);
        *z = GlobalZIndex(1200);
    }

    let (mw, mh) = sim.state.map.dimensions();
    if mw == 0 || mh == 0 {
        return;
    }
    for (map_cell, mut bg, mut node) in &mut cells {
        node.width = Val::Px(cell);
        node.height = Val::Px(cell);
        let x = (MINIMAP_COLS.saturating_sub(1).saturating_sub(map_cell.col)) * mw / MINIMAP_COLS;
        let y = map_cell.row * mh / MINIMAP_ROWS;
        let c = TileCoord::new(x as i32, y as i32);
        let kind = sim.state.map.get_kind(c).unwrap_or(TileKind::Void);
        *bg = BackgroundColor(minimap_cell_color(&sim.state, &layers, c, kind));
    }

    for (toggle, mut bg) in &mut toggles {
        let on = match *toggle {
            MinimapLayerToggle::Industries => layers.industries,
            MinimapLayerToggle::Owners => layers.owners,
            MinimapLayerToggle::Vehicles => layers.vehicles,
            MinimapLayerToggle::Expand => layers.expanded,
        };
        *bg = BackgroundColor(if on {
            Color::srgb(0.58, 0.50, 0.31)
        } else {
            Color::srgb(0.28, 0.24, 0.16)
        });
    }
    if let Ok(mut text) = legend.single_mut() {
        **text = format!(
            "{}{}{}{}",
            if layers.industries { "I" } else { "-" },
            if layers.owners { "D" } else { "-" },
            if layers.vehicles { "V" } else { "-" },
            if layers.expanded { "+" } else { "" },
        );
    }

    update_minimap_viewport(cell, &sim.state.map, &windows, &cam_q, &mut viewport_q);
}

fn update_minimap_viewport(
    cell: f32,
    map: &Map,
    windows: &Query<&Window, With<PrimaryWindow>>,
    cam_q: &Query<(&Transform, &Projection), (With<PrimaryGameCamera>, Without<MapPreviewCamera>)>,
    viewport_q: &mut Query<&mut Node, MinimapViewportFilter>,
) {
    let Ok(window) = windows.single() else {
        return;
    };
    let Ok((cam_tf, projection)) = cam_q.single() else {
        return;
    };
    let Projection::Orthographic(proj) = projection else {
        return;
    };
    let (mw, mh) = map.dimensions();
    if mw == 0 || mh == 0 {
        return;
    }
    let center_world = Vec2::new(cam_tf.translation.x, cam_tf.translation.y);
    let Some((cx, cy)) = world_pos_to_tile_coord(center_world, map) else {
        return;
    };

    let half_w = window.width() * proj.scale * 0.5;
    let half_h = window.height() * proj.scale * 0.5;
    let half_tiles = (0.5 * (half_w / ISO_HW + half_h / ISO_QH)).max(2.0);
    let half_tiles_x = half_tiles.clamp(2.0, mw as f32);
    let half_tiles_y = half_tiles.clamp(2.0, mh as f32);

    let min_x = (cx as f32 - half_tiles_x).clamp(0.0, mw.saturating_sub(1) as f32);
    let max_x = (cx as f32 + half_tiles_x).clamp(0.0, mw.saturating_sub(1) as f32);
    let min_y = (cy as f32 - half_tiles_y).clamp(0.0, mh.saturating_sub(1) as f32);
    let max_y = (cy as f32 + half_tiles_y).clamp(0.0, mh.saturating_sub(1) as f32);
    let left_min =
        MINIMAP_PAD + ((mw as f32 - 1.0 - max_x).max(0.0) / mw as f32 * MINIMAP_COLS as f32 * cell);
    let left_max =
        MINIMAP_PAD + ((mw as f32 - 1.0 - min_x).max(0.0) / mw as f32 * MINIMAP_COLS as f32 * cell);
    let top_min = MINIMAP_PAD + (min_y / mh as f32 * MINIMAP_ROWS as f32 * cell);
    let top_max = MINIMAP_PAD + (max_y / mh as f32 * MINIMAP_ROWS as f32 * cell);
    let width = (left_max - left_min).max(3.0);
    let height = (top_max - top_min).max(3.0);
    let Ok(mut node) = viewport_q.single_mut() else {
        return;
    };
    node.left = Val::Px(left_min);
    node.top = Val::Px(top_min);
    node.width = Val::Px(width);
    node.height = Val::Px(height);
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use openttdrs_core::GameState;

    use crate::ui::hud::SimHudControls;
    use crate::ui::toolbar::minimap::setup_minimap;

    #[test]
    fn sync_minimap_accepts_spawned_ui_without_b0001() {
        let mut world = World::new();
        world.insert_resource(SimWorld {
            state: GameState::new(16, 16),
            loaded_file: false,
            ottdmap_extras: None,
        });
        world.insert_resource(SimHudControls {
            minimap_visible: true,
            ..Default::default()
        });
        world.init_resource::<MinimapLayerState>();
        world.spawn((
            Window {
                resolution: (800_u32, 600_u32).into(),
                ..default()
            },
            PrimaryWindow,
        ));
        world.spawn((
            PrimaryGameCamera,
            Camera2d,
            Transform::default(),
            Projection::Orthographic(OrthographicProjection::default_2d()),
        ));

        world.run_system_once(setup_minimap).unwrap();
        world.run_system_once(sync_minimap).unwrap();
        assert!(
            world
                .query_filtered::<Entity, With<MinimapRoot>>()
                .iter(&world)
                .next()
                .is_some()
        );
    }
}
