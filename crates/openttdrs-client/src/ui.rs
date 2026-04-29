//! UI de información de tile seleccionado y menú de construcción (I6).

use bevy::prelude::*;
use bevy::sprite::Anchor;
use bevy::ui::FocusPolicy;
use bevy::window::PrimaryWindow;
use openttdrs_core::{Command, TileCoord, TileKind, apply_command};

use crate::RemapMapVisualsPending;
use crate::iso::{
    compute_tileh, shore_png_index, shore_tileh_for_draw_shore, slope_label,
    tile_slope_bits_from_heights, world_pos_to_tile_coord,
};
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

/// Marca nodos del menú “Construir” para ignorar clics en el mapa cuando el cursor está encima.
#[derive(Component)]
pub(crate) struct BuildMenuUi;

/// Acción del botón del menú de construcción.
#[derive(Component, Clone, Copy)]
pub(crate) enum BuildMenuAction {
    Road,
    Station,
}

/// Crea el texto de información del tile.
pub fn setup_tile_info_ui(mut commands: Commands) {
    commands.spawn((
        TileInfoText,
        Text2d::new("Clic en mapa: seleccionar tile · Construir: panel inferior izquierdo"),
        TextFont {
            font_size: 14.0,
            ..default()
        },
        TextColor(Color::srgb(1.0, 1.0, 0.8)),
        Transform::from_xyz(0.0, 0.0, 1000.0),
        Anchor::TOP_LEFT,
    ));
}

/// Panel flotante: carretera / estación sobre el tile seleccionado.
pub fn setup_build_menu(mut commands: Commands) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(12.0),
                bottom: Val::Px(12.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                padding: UiRect::all(Val::Px(10.0)),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.1, 0.12, 0.16, 0.94)),
            BorderColor::all(Color::srgb(0.35, 0.4, 0.48)),
            GlobalZIndex(2000),
            FocusPolicy::Block,
            BuildMenuUi,
            Interaction::default(),
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("Construir (tile activo)"),
                TextFont {
                    font_size: 13.0,
                    ..default()
                },
                TextColor(Color::srgb(0.92, 0.88, 0.78)),
            ));
            for (label, action) in [
                ("Carretera", BuildMenuAction::Road),
                ("Estación", BuildMenuAction::Station),
            ] {
                parent
                    .spawn((
                        Button,
                        action,
                        BuildMenuUi,
                        Node {
                            width: Val::Px(148.0),
                            height: Val::Px(34.0),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.22, 0.25, 0.3)),
                        BorderColor::all(Color::srgb(0.42, 0.46, 0.52)),
                        Interaction::default(),
                    ))
                    .with_children(|p| {
                        p.spawn((
                            Text::new(label),
                            TextFont {
                                font_size: 14.0,
                                ..default()
                            },
                            TextColor(Color::srgb(0.93, 0.9, 0.84)),
                        ));
                    });
            }
        });
}

/// Aplica comando según botón del menú (tile [`SelectedTileInfo::pos`]).
#[allow(clippy::type_complexity)]
pub(crate) fn build_menu_interaction(
    mut q: Query<(&Interaction, &BuildMenuAction), (Changed<Interaction>, With<Button>)>,
    selected: Res<SelectedTileInfo>,
    mut sim: ResMut<SimWorld>,
    mut pending: ResMut<RemapMapVisualsPending>,
) {
    for (interaction, action) in &mut q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let Some(pos) = selected.pos else {
            continue;
        };
        let cmd = match *action {
            BuildMenuAction::Road => Command::PlaceRoad(pos),
            BuildMenuAction::Station => Command::PlaceStation(pos),
        };
        if apply_command(&mut sim.state, &cmd).is_ok() {
            pending.pending = true;
        }
    }
}

/// Clic izquierdo: solo selecciona tile (no construye). Ignora si el cursor está sobre el menú.
pub fn handle_tile_click(
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cam_q: Query<(&Camera, &Transform), With<Camera2d>>,
    mut selected: ResMut<SelectedTileInfo>,
    sim: Res<SimWorld>,
    menu_pointer: Query<&Interaction, With<BuildMenuUi>>,
) {
    if !mouse.just_pressed(MouseButton::Left) {
        return;
    }

    if menu_pointer.iter().any(|i| *i != Interaction::None) {
        return;
    }

    let Ok(window) = windows.single() else {
        return;
    };
    let Some(cursor_pos) = window.cursor_position() else {
        return;
    };
    let Ok((camera, cam_tf)) = cam_q.single() else {
        return;
    };
    // Misma proyección que el renderer (el cálculo manual centro+scale*delta no coincide con Orthographic).
    let cam_global = GlobalTransform::from(*cam_tf);
    let Ok(world_pos) = camera.viewport_to_world_2d(&cam_global, cursor_pos) else {
        return;
    };

    let Some((tx, ty)) = world_pos_to_tile_coord(world_pos, &sim.state.map) else {
        selected.pos = None;
        return;
    };
    selected.pos = Some(TileCoord::new(tx, ty));
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
        **text =
            format!("{zoom_label}\n{hud_footer}\nClic mapa: elegir tile · panel Construir abajo");
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
    let coast_dbg = if std::env::var("OPENTTDRS_DEBUG_COAST")
        .ok()
        .is_some_and(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        && tile.kind == TileKind::Water
        && pos.x >= 0
        && pos.y >= 0
    {
        let ux = pos.x as u32;
        let uy = pos.y as u32;
        let (mw, mh) = sim.state.map.dimensions();
        let (raw, _) = tile_slope_bits_from_heights(&sim.state.map, ux, uy);
        let th = shore_tileh_for_draw_shore(&sim.state.map, ux, uy, mw, mh);
        let si = shore_png_index(th);
        format!("\ncoast dbg raw:{raw} th:{th} si:{si}")
    } else {
        String::new()
    };

    **text = format!(
        "{zoom_label}\n{hud_footer}\nTile ({},{}) {}\nh:{} slope:{} ({}) mapt:0x{:02X} m5:0x{:02X} m1:0x{:02X} m2:0x{:02X} m7:0x{:02X} m3:0x{:02X} m3hi:0x{:02X}{}{}",
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
        extra,
        coast_dbg
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
