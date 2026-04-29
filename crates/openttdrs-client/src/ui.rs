//! UI de información de tile seleccionado.

use bevy::prelude::*;
use bevy::sprite::Anchor;
use bevy::window::PrimaryWindow;
use openttdrs_core::{TileCoord, TileKind};

use crate::iso::{compute_tileh, slope_label, world_to_tile};
use crate::sprites::{
    is_road_level_crossing, level_crossing_rail_sprite_id, rail_tile_is_signals,
    road_bits_for_render,
};
use crate::state::SimWorld;

/// Pausa simulación y ruta del JSON de **F5/F9** (alternativa a variable de entorno al arranque).
#[derive(Resource)]
pub struct SimHudControls {
    pub paused: bool,
    pub json_save_path: String,
}

impl Default for SimHudControls {
    fn default() -> Self {
        Self {
            paused: false,
            json_save_path: std::env::var("OPENTTDRS_JSON_SAVE")
                .unwrap_or_else(|_| "openttdrs_sim.json".into()),
        }
    }
}

/// Información del tile actualmente seleccionado (click izquierdo).
#[derive(Resource, Default)]
pub struct SelectedTileInfo {
    pub pos: Option<TileCoord>,
}

/// Marcador para el texto de información del tile.
#[derive(Component)]
pub struct TileInfoText;

/// Crea el texto de información del tile.
pub fn setup_tile_info_ui(mut commands: Commands) {
    commands.spawn((
        TileInfoText,
        Text2d::new("Click en tile para ver info"),
        TextFont {
            font_size: 14.0,
            ..default()
        },
        TextColor(Color::srgb(1.0, 1.0, 0.8)),
        Transform::from_xyz(0.0, 0.0, 1000.0),
        Anchor::TOP_LEFT,
    ));
}

/// Detecta click izquierdo y actualiza el tile seleccionado.
pub fn handle_tile_click(
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cam_q: Query<(&Transform, &Projection), With<Camera2d>>,
    mut selected: ResMut<SelectedTileInfo>,
    sim: Res<SimWorld>,
) {
    if !mouse.just_pressed(MouseButton::Left) {
        return;
    }

    let Ok(window) = windows.single() else {
        return;
    };
    let Some(cursor_pos) = window.cursor_position() else {
        return;
    };
    let Ok((cam_transform, projection)) = cam_q.single() else {
        return;
    };
    let Projection::Orthographic(proj) = projection else {
        return;
    };

    let window_size = Vec2::new(window.width(), window.height());
    let cursor_offset = cursor_pos - window_size / 2.0;
    let cursor_offset_world = Vec2::new(cursor_offset.x, -cursor_offset.y);
    let world_pos = Vec2::new(cam_transform.translation.x, cam_transform.translation.y)
        + cursor_offset_world * proj.scale;

    let (tx, ty) = world_to_tile(world_pos);
    let (mw, mh) = sim.state.map.dimensions();

    if tx >= 0 && ty >= 0 && tx < mw as i32 && ty < mh as i32 {
        selected.pos = Some(TileCoord::new(tx, ty));
    } else {
        selected.pos = None;
    }
}

/// Actualiza el texto de información del tile seleccionado.
#[allow(clippy::type_complexity)]
pub fn update_tile_info_text(
    selected: Res<SelectedTileInfo>,
    sim: Res<SimWorld>,
    hud: Res<SimHudControls>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cam_q: Query<(&Transform, &Projection), With<Camera2d>>,
    mut text_q: Query<(&mut Text2d, &mut Transform), (With<TileInfoText>, Without<Camera2d>)>,
) {
    let Ok((mut text, mut text_transform)) = text_q.single_mut() else {
        return;
    };
    let Ok((cam_transform, projection)) = cam_q.single() else {
        return;
    };
    let Projection::Orthographic(proj) = projection else {
        return;
    };
    let Ok(window) = windows.single() else {
        return;
    };

    // Posicionar en esquina superior izquierda de la ventana (en coordenadas del mundo)
    let half_w = window.width() / 2.0 * proj.scale;
    let half_h = window.height() / 2.0 * proj.scale;
    text_transform.translation.x = cam_transform.translation.x - half_w + 10.0 * proj.scale;
    text_transform.translation.y = cam_transform.translation.y + half_h - 10.0 * proj.scale;
    text_transform.scale = Vec3::splat(proj.scale);

    let zoom_label = format!("Zoom {:.2}×", proj.scale);
    let pause_l = if hud.paused {
        "Pausa ON (P)"
    } else {
        "Pausa off (P)"
    };
    let hud_footer = format!("{pause_l} | JSON: {} | F4 otra ruta", hud.json_save_path);

    let Some(pos) = selected.pos else {
        **text = format!("{zoom_label}\n{hud_footer}\nClick en tile para ver info");
        return;
    };

    let Some(tile) = sim.state.map.get(pos) else {
        **text = format!(
            "{zoom_label}\n{hud_footer}\n({}, {}): fuera del mapa",
            pos.x, pos.y
        );
        return;
    };

    let kind_str = match tile.kind {
        TileKind::Void => "Void",
        TileKind::Grass => "Grass",
        TileKind::Water => "Water",
        TileKind::Road => "Road",
        TileKind::Rail => "Rail",
        TileKind::House => "House",
        TileKind::Industry => "Industry",
        TileKind::Station => "Station",
        TileKind::Forest => "Forest",
        TileKind::CoalField => "CoalField",
        TileKind::Unknown(n) => {
            **text = format!(
                "{zoom_label}\n{hud_footer}\n({}, {}): Unknown({})",
                pos.x, pos.y, n
            );
            return;
        }
    };

    let extra = if tile.kind == TileKind::Road {
        let rb = road_bits_for_render(
            &sim.state.map,
            pos,
            sim.state.map.dimensions().0,
            sim.state.map.dimensions().1,
        );
        let mut s = format!(" rb:0x{rb:02X}");
        if is_road_level_crossing(tile.mapt, tile.m5, tile.kind) {
            s.push_str(&format!(
                " Xing rail:{}",
                level_crossing_rail_sprite_id(tile.m5)
            ));
        }
        s
    } else if tile.kind == TileKind::Rail && rail_tile_is_signals(tile.m5) {
        format!(
            " signals present:0x{:X} m2:0x{:02X}",
            (tile.m3 >> 4) & 0xF,
            tile.m2
        )
    } else if tile.kind == TileKind::Industry {
        format!(" gfx:{} ind:{}", tile.m5, tile.m1 & 0x7F)
    } else {
        String::new()
    };

    let mw = sim.state.map.dimensions().0;
    let mh = sim.state.map.dimensions().1;
    let tileh = if pos.x >= 0 && pos.y >= 0 && (pos.x as u32) < mw && (pos.y as u32) < mh {
        compute_tileh(&sim.state.map, pos.x as u32, pos.y as u32)
    } else {
        0
    };
    let slope_str = slope_label(tileh);

    **text = format!(
        "{zoom_label}\n{hud_footer}\nTile ({},{}) {}\nh:{} slope:{} ({}) mapt:0x{:02X} m5:0x{:02X} m1:0x{:02X} m2:0x{:02X} m7:0x{:02X} m3:0x{:02X} m3hi:0x{:02X}{}",
        pos.x,
        pos.y,
        kind_str,
        tile.height,
        tileh,
        slope_str,
        tile.mapt,
        tile.m5,
        tile.m1,
        tile.m2,
        tile.m7,
        tile.m3,
        tile.m3hi,
        extra
    );
}

/// **P** alterna pausa del tick de simulación (`GameState::step`).
pub fn handle_pause_toggle(keyboard: Res<ButtonInput<KeyCode>>, mut hud: ResMut<SimHudControls>) {
    if keyboard.just_pressed(KeyCode::KeyP) {
        hud.paused = !hud.paused;
        if hud.paused {
            info!("Pausa: ON");
        } else {
            info!("Pausa: OFF");
        }
    }
}

/// **F4** alterna entre dos rutas de archivo predefinidas para F5/F9.
pub fn cycle_json_save_path_hotkey(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut hud: ResMut<SimHudControls>,
) {
    if keyboard.just_pressed(KeyCode::F4) {
        hud.json_save_path = if hud.json_save_path.ends_with("autosave.json") {
            "openttdrs_sim.json".into()
        } else {
            "openttdrs_autosave.json".into()
        };
        info!("Ruta JSON (F5/F9): {}", hud.json_save_path);
    }
}
