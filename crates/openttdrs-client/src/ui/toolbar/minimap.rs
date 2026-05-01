use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use openttdrs_core::{Map, TileCoord, TileKind};

use crate::iso::{ISO_HW, ISO_QH, tile_pos, world_pos_to_tile_coord};
use crate::render::{IndustryPreviewCamera, PrimaryGameCamera};
use crate::state::SimWorld;
use crate::ui::hud::SimHudControls;

use super::BuildMenuUi;

const MINIMAP_COLS: u32 = 64;
const MINIMAP_ROWS: u32 = 40;
const MINIMAP_CELL: f32 = 3.0;
const MINIMAP_PAD: f32 = 6.0;
const MINIMAP_RIGHT: f32 = 10.0;
const MINIMAP_BOTTOM: f32 = 10.0;

#[derive(Component)]
pub(crate) struct MinimapRoot;

#[derive(Component)]
pub(crate) struct MinimapCell {
    col: u32,
    row: u32,
}

#[derive(Component)]
pub(crate) struct MinimapViewport;

pub(crate) fn setup_minimap(mut commands: Commands) {
    let root = commands
        .spawn((
            Button,
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(MINIMAP_RIGHT),
                bottom: Val::Px(MINIMAP_BOTTOM),
                width: Val::Px(MINIMAP_COLS as f32 * MINIMAP_CELL + 12.0),
                height: Val::Px(MINIMAP_ROWS as f32 * MINIMAP_CELL + 12.0),
                padding: UiRect::all(Val::Px(MINIMAP_PAD)),
                border: UiRect::all(Val::Px(2.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(0.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.05, 0.05, 0.04, 0.82)),
            BorderColor::all(Color::srgb(0.55, 0.5, 0.34)),
            BuildMenuUi,
            MinimapRoot,
            Interaction::default(),
        ))
        .id();

    commands.entity(root).with_children(|root| {
        for row in 0..MINIMAP_ROWS {
            root.spawn(Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(0.0),
                ..default()
            })
            .with_children(|line| {
                for col in 0..MINIMAP_COLS {
                    line.spawn((
                        MinimapCell { col, row },
                        Node {
                            width: Val::Px(MINIMAP_CELL),
                            height: Val::Px(MINIMAP_CELL),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.12, 0.2, 0.09)),
                    ));
                }
            });
        }
        root.spawn((
            MinimapViewport,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(MINIMAP_PAD),
                top: Val::Px(MINIMAP_PAD),
                width: Val::Px(12.0),
                height: Val::Px(8.0),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            BorderColor::all(Color::srgb(1.0, 1.0, 0.9)),
        ));
    });
}

pub(crate) fn sync_minimap(
    sim: Res<SimWorld>,
    hud: Res<SimHudControls>,
    mut root_q: Query<&mut Visibility, With<MinimapRoot>>,
    mut cells: Query<(&MinimapCell, &mut BackgroundColor)>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cam_q: Query<
        (&Transform, &Projection),
        (With<PrimaryGameCamera>, Without<IndustryPreviewCamera>),
    >,
    mut viewport_q: Query<&mut Node, With<MinimapViewport>>,
) {
    if let Ok(mut vis) = root_q.single_mut() {
        *vis = if hud.minimap_visible {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
    if !hud.minimap_visible {
        return;
    }
    let (mw, mh) = sim.state.map.dimensions();
    if mw == 0 || mh == 0 {
        return;
    }
    for (cell, mut bg) in &mut cells {
        let x = (MINIMAP_COLS.saturating_sub(1).saturating_sub(cell.col)) * mw / MINIMAP_COLS;
        let y = cell.row * mh / MINIMAP_ROWS;
        let c = TileCoord::new(x as i32, y as i32);
        *bg = BackgroundColor(minimap_color(
            sim.state.map.get_kind(c).unwrap_or(TileKind::Void),
        ));
    }

    update_minimap_viewport(&sim.state.map, &windows, &cam_q, &mut viewport_q);
}

pub(crate) fn handle_minimap_click(
    mouse: Res<ButtonInput<MouseButton>>,
    hud: Res<SimHudControls>,
    windows: Query<&Window, With<PrimaryWindow>>,
    sim: Res<SimWorld>,
    mut cam_q: Query<&mut Transform, (With<PrimaryGameCamera>, Without<IndustryPreviewCamera>)>,
    menu_pointer: Query<&Interaction, With<BuildMenuUi>>,
) {
    if !hud.minimap_visible || !mouse.just_pressed(MouseButton::Left) {
        return;
    }
    if menu_pointer.iter().any(|i| *i != Interaction::None) {
        return;
    }
    let Ok(window) = windows.single() else {
        return;
    };
    let Some(cursor) = window.cursor_position() else {
        return;
    };
    let Some((tile_x, tile_y)) = cursor_to_minimap_tile(cursor, window, sim.state.map.dimensions())
    else {
        return;
    };
    let coord = TileCoord::new(tile_x, tile_y);
    let height = sim.state.map.get(coord).map_or(0, |tile| tile.height);
    let pos = tile_pos(tile_x, tile_y, height, 0.0);
    let Ok(mut tf) = cam_q.single_mut() else {
        return;
    };
    tf.translation.x = pos.x;
    tf.translation.y = pos.y;
}

fn cursor_to_minimap_tile(
    cursor: Vec2,
    window: &Window,
    dimensions: (u32, u32),
) -> Option<(i32, i32)> {
    let (mw, mh) = dimensions;
    if mw == 0 || mh == 0 {
        return None;
    }
    let total_w = MINIMAP_COLS as f32 * MINIMAP_CELL + MINIMAP_PAD * 2.0;
    let left = window.width() - MINIMAP_RIGHT - total_w;
    let total_h = MINIMAP_ROWS as f32 * MINIMAP_CELL + MINIMAP_PAD * 2.0;
    let top = window.height() - MINIMAP_BOTTOM - total_h;
    let local_x = cursor.x - left - MINIMAP_PAD;
    let local_y_from_top = cursor.y - top - MINIMAP_PAD;
    if local_x < 0.0
        || local_y_from_top < 0.0
        || local_x >= MINIMAP_COLS as f32 * MINIMAP_CELL
        || local_y_from_top >= MINIMAP_ROWS as f32 * MINIMAP_CELL
    {
        return None;
    }
    let col = (local_x / MINIMAP_CELL).floor() as u32;
    let row = (local_y_from_top / MINIMAP_CELL).floor() as u32;
    let x = ((MINIMAP_COLS.saturating_sub(1).saturating_sub(col)) * mw / MINIMAP_COLS) as i32;
    let y = (row * mh / MINIMAP_ROWS) as i32;
    Some((x, y))
}

fn update_minimap_viewport(
    map: &Map,
    windows: &Query<&Window, With<PrimaryWindow>>,
    cam_q: &Query<
        (&Transform, &Projection),
        (With<PrimaryGameCamera>, Without<IndustryPreviewCamera>),
    >,
    viewport_q: &mut Query<&mut Node, With<MinimapViewport>>,
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
    let left_min = MINIMAP_PAD
        + ((mw as f32 - 1.0 - max_x).max(0.0) / mw as f32 * MINIMAP_COLS as f32 * MINIMAP_CELL);
    let left_max = MINIMAP_PAD
        + ((mw as f32 - 1.0 - min_x).max(0.0) / mw as f32 * MINIMAP_COLS as f32 * MINIMAP_CELL);
    let top_min = MINIMAP_PAD + (min_y / mh as f32 * MINIMAP_ROWS as f32 * MINIMAP_CELL);
    let top_max = MINIMAP_PAD + (max_y / mh as f32 * MINIMAP_ROWS as f32 * MINIMAP_CELL);
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

pub(crate) fn minimap_color(kind: TileKind) -> Color {
    match kind {
        TileKind::Water => Color::srgb(0.08, 0.25, 0.55),
        TileKind::Road | TileKind::RoadDepot | TileKind::RoadBridge | TileKind::RoadTunnel => {
            Color::srgb(0.48, 0.42, 0.32)
        }
        TileKind::Rail | TileKind::RailDepot | TileKind::RailBridge | TileKind::RailTunnel => {
            Color::srgb(0.68, 0.68, 0.62)
        }
        TileKind::House => Color::srgb(0.72, 0.28, 0.2),
        TileKind::Industry | TileKind::CoalField => Color::srgb(0.78, 0.64, 0.2),
        TileKind::Station => Color::srgb(0.95, 0.95, 0.86),
        TileKind::Forest => Color::srgb(0.05, 0.34, 0.1),
        TileKind::Grass => Color::srgb(0.16, 0.48, 0.12),
        TileKind::Void => Color::srgb(0.02, 0.02, 0.02),
        TileKind::Unknown(_) => Color::srgb(0.38, 0.12, 0.45),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cursor_to_minimap_tile_top_left_maps_to_small_coords() {
        let window = Window {
            resolution: (200_u32, 150_u32).into(),
            ..default()
        };
        let total_w = MINIMAP_COLS as f32 * MINIMAP_CELL + MINIMAP_PAD * 2.0;
        let left = window.width() - MINIMAP_RIGHT - total_w;
        let total_h = MINIMAP_ROWS as f32 * MINIMAP_CELL + MINIMAP_PAD * 2.0;
        let top = window.height() - MINIMAP_BOTTOM - total_h;
        let cursor = Vec2::new(left + MINIMAP_PAD + 1.0, top + MINIMAP_PAD + 1.0);
        assert!(cursor_to_minimap_tile(cursor, &window, (64, 40)).is_some());
    }
}
