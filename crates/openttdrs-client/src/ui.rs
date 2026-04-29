//! UI de información de tile seleccionado y menú de construcción (I6).

use bevy::prelude::*;
use bevy::sprite::Anchor;
use bevy::ui::FocusPolicy;
use bevy::window::PrimaryWindow;
use openttdrs_core::{Command, TileCoord, TileKind, apply_command};

use crate::bevy_app::{StartupSet, UpdateSet};
use crate::config;
use crate::iso::{
    compute_tileh, shore_png_index, shore_tileh_for_draw_shore, slope_label,
    tile_slope_bits_from_heights, world_pos_to_tile_coord,
};
use crate::sprites::{
    is_road_level_crossing, level_crossing_rail_sprite_id, rail_tile_is_signals,
    road_bits_for_render,
};
use crate::state::SimWorld;
use crate::world_render::RemapMapVisualsPending;

pub(crate) struct ClientUiPlugin;

impl Plugin for ClientUiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SelectedTileInfo>()
            .init_resource::<SimHudControls>()
            .init_resource::<UiToolState>()
            .init_resource::<ToolbarState>()
            .add_systems(
                Startup,
                (setup_tile_info_ui, setup_top_toolbar, setup_build_menu).in_set(StartupSet::Ui),
            )
            .add_systems(
                Update,
                (
                    handle_pause_toggle,
                    cycle_json_save_path_hotkey,
                    handle_tool_hotkeys,
                )
                    .in_set(UpdateSet::Input),
            )
            .add_systems(
                Update,
                (
                    toolbar_group_interaction,
                    build_menu_interaction,
                    update_toolbar_group_visuals,
                    update_toolbar_tool_visibility,
                    update_tool_button_visuals,
                    update_toolbar_tooltip,
                    handle_tile_click,
                    update_tile_info_text,
                )
                    .in_set(UpdateSet::Ui),
            );
    }
}

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
            json_save_path: config::json_save_path(),
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
#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BuildMenuAction {
    Road,
    Rail,
    Station,
    Clear,
}

#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ToolbarGroup {
    Transport,
    Build,
    Economy,
    Info,
    Settings,
}

/// Marca botones que seleccionan herramienta de construcción.
#[derive(Component)]
pub(crate) struct ToolSelectButton;

#[derive(Component)]
pub(crate) struct ToolbarGroupButton;

#[derive(Component)]
pub(crate) struct ToolButtonGroup(pub ToolbarGroup);

#[derive(Component)]
pub(crate) struct TooltipText;

#[derive(Component)]
pub(crate) struct TooltipBox;

#[derive(Component)]
pub(crate) struct ToolbarTooltipTarget {
    pub text: &'static str,
}

/// Herramienta de construcción activa elegida desde la UI.
#[derive(Resource, Default)]
pub(crate) struct UiToolState {
    pub active_tool: Option<BuildMenuAction>,
}

#[derive(Resource)]
pub(crate) struct ToolbarState {
    pub active_group: ToolbarGroup,
}

impl Default for ToolbarState {
    fn default() -> Self {
        Self {
            active_group: ToolbarGroup::Build,
        }
    }
}

/// Crea el texto de información del tile.
pub fn setup_tile_info_ui(mut commands: Commands) {
    commands.spawn((
        TileInfoText,
        Text2d::new(
            "Clic en mapa: seleccionar tile · Toolbar: 1/2/3/C para tool, Esc cancela",
        ),
        TextFont {
            font_size: 14.0,
            ..default()
        },
        TextColor(Color::srgb(1.0, 1.0, 0.8)),
        Transform::from_xyz(0.0, 0.0, 1000.0),
        Anchor::TOP_LEFT,
    ));
}

/// Barra superior compacta tipo toolbar para selección rápida de herramienta.
pub fn setup_top_toolbar(mut commands: Commands, asset_server: Res<AssetServer>) {
    let root = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(12.0),
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(2.0),
                ..default()
            },
            BuildMenuUi,
            GlobalZIndex(2100),
        ))
        .id();

    commands.entity(root).with_children(|root| {
        root.spawn((
            Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(2.0),
                padding: UiRect::all(Val::Px(4.0)),
                border: UiRect::all(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.22, 0.2, 0.16, 0.95)),
            BorderColor::all(Color::srgb(0.55, 0.5, 0.36)),
            FocusPolicy::Block,
            BuildMenuUi,
            Interaction::default(),
        ))
        .with_children(|parent| {
            for (i, icon_path, group) in [
                (0_u8, "opengfx/tiles/rail_1005.png", ToolbarGroup::Transport),
                (1, "opengfx/tiles/road_flat_00.png", ToolbarGroup::Build),
                (2, "opengfx/tiles/house_church_build.png", ToolbarGroup::Economy),
                (3, "opengfx/tiles/object_lighthouse.png", ToolbarGroup::Info),
                (4, "opengfx/tiles/object_transmitter.png", ToolbarGroup::Settings),
            ] {
                parent
                    .spawn((
                        Button,
                        group,
                        ToolbarGroupButton,
                        ToolbarTooltipTarget {
                            text: match group {
                                ToolbarGroup::Transport => "Transportes",
                                ToolbarGroup::Build => "Construccion",
                                ToolbarGroup::Economy => "Economia",
                                ToolbarGroup::Info => "Informacion",
                                ToolbarGroup::Settings => "Ajustes",
                            },
                        },
                        BuildMenuUi,
                        Node {
                            width: Val::Px(22.0),
                            height: Val::Px(22.0),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            border: UiRect::all(Val::Px(1.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.36, 0.33, 0.24)),
                        BorderColor::all(Color::srgb(0.62, 0.58, 0.44)),
                        Interaction::default(),
                    ))
                    .with_children(|p| {
                        p.spawn((
                            ImageNode::new(asset_server.load::<Image>(icon_path)),
                            Node {
                                width: Val::Px(16.0),
                                height: Val::Px(16.0),
                                ..default()
                            },
                        ));
                    });
                if i < 4 {
                    parent.spawn((
                        Node {
                            width: Val::Px(1.0),
                            height: Val::Px(18.0),
                            margin: UiRect::horizontal(Val::Px(1.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.5, 0.46, 0.34)),
                        BuildMenuUi,
                    ));
                }
            }
        });

        root.spawn((
            Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(2.0),
                padding: UiRect::all(Val::Px(4.0)),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.2, 0.18, 0.14, 0.94)),
            BorderColor::all(Color::srgb(0.62, 0.58, 0.44)),
            FocusPolicy::Block,
            BuildMenuUi,
            ToolButtonGroup(ToolbarGroup::Build),
            Interaction::default(),
        ))
        .with_children(|buttons| {
            for (label, icon_path, action) in [
                ("Road", "opengfx/tiles/road_flat_00.png", BuildMenuAction::Road),
                (
                    "Station",
                    "opengfx/tiles/truck_stop_ground_0.png",
                    BuildMenuAction::Station,
                ),
                ("Clear", "opengfx/tiles/grass_rough.png", BuildMenuAction::Clear),
            ] {
                buttons
                    .spawn((
                        Button,
                        action,
                        ToolSelectButton,
                        ToolbarTooltipTarget {
                            text: match action {
                                BuildMenuAction::Road => "Construir carretera (1)",
                                BuildMenuAction::Rail => "Construir via ferrea (3)",
                                BuildMenuAction::Station => "Construir estacion (2)",
                                BuildMenuAction::Clear => "Limpiar tesela (C)",
                            },
                        },
                        BuildMenuUi,
                        Node {
                            width: Val::Px(86.0),
                            height: Val::Px(22.0),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            border: UiRect::all(Val::Px(1.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.3, 0.28, 0.2)),
                        BorderColor::all(Color::srgb(0.62, 0.58, 0.44)),
                        Interaction::default(),
                    ))
                    .with_children(|p| {
                        p.spawn((
                            ImageNode::new(asset_server.load::<Image>(icon_path)),
                            Node {
                                width: Val::Px(16.0),
                                height: Val::Px(16.0),
                                margin: UiRect::right(Val::Px(4.0)),
                                ..default()
                            },
                        ));
                        p.spawn((
                            Text::new(label),
                            TextFont {
                                font_size: 11.0,
                                ..default()
                            },
                            TextColor(Color::srgb(0.12, 0.12, 0.1)),
                        ));
                    });
            }
        });

        root.spawn((
            Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(2.0),
                padding: UiRect::all(Val::Px(4.0)),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.2, 0.18, 0.14, 0.94)),
            BorderColor::all(Color::srgb(0.62, 0.58, 0.44)),
            FocusPolicy::Block,
            BuildMenuUi,
            ToolButtonGroup(ToolbarGroup::Transport),
            Interaction::default(),
        ))
        .with_children(|buttons| {
            for (label, icon_path, action) in [
                ("Rail", "opengfx/tiles/rail_1005.png", BuildMenuAction::Rail),
                ("Road", "opengfx/tiles/road_flat_00.png", BuildMenuAction::Road),
                ("Clear", "opengfx/tiles/grass_rough.png", BuildMenuAction::Clear),
            ] {
                buttons
                    .spawn((
                        Button,
                        action,
                        ToolSelectButton,
                        ToolbarTooltipTarget {
                            text: match action {
                                BuildMenuAction::Road => "Construir carretera (1)",
                                BuildMenuAction::Rail => "Construir via ferrea (3)",
                                BuildMenuAction::Station => "Construir estacion (2)",
                                BuildMenuAction::Clear => "Limpiar tesela (C)",
                            },
                        },
                        BuildMenuUi,
                        Node {
                            width: Val::Px(86.0),
                            height: Val::Px(22.0),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            border: UiRect::all(Val::Px(1.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.3, 0.28, 0.2)),
                        BorderColor::all(Color::srgb(0.62, 0.58, 0.44)),
                        Interaction::default(),
                    ))
                    .with_children(|p| {
                        p.spawn((
                            ImageNode::new(asset_server.load::<Image>(icon_path)),
                            Node {
                                width: Val::Px(16.0),
                                height: Val::Px(16.0),
                                margin: UiRect::right(Val::Px(4.0)),
                                ..default()
                            },
                        ));
                        p.spawn((
                            Text::new(label),
                            TextFont {
                                font_size: 11.0,
                                ..default()
                            },
                            TextColor(Color::srgb(0.12, 0.12, 0.1)),
                        ));
                    });
            }
        });

        for (label, group) in [
            ("Economia: pronto", ToolbarGroup::Economy),
            ("Info: pronto", ToolbarGroup::Info),
            ("Ajustes: pronto", ToolbarGroup::Settings),
        ] {
            root.spawn((
                Node {
                    padding: UiRect::all(Val::Px(6.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.2, 0.18, 0.14, 0.94)),
                BorderColor::all(Color::srgb(0.62, 0.58, 0.44)),
                FocusPolicy::Block,
                BuildMenuUi,
                ToolButtonGroup(group),
                children![(
                    Text::new(label),
                    TextFont {
                        font_size: 11.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.9, 0.86, 0.72)),
                )],
            ));
        }

        root.spawn((
            Node {
                padding: UiRect::axes(Val::Px(6.0), Val::Px(3.0)),
                border: UiRect::all(Val::Px(1.0)),
                display: Display::None,
                ..default()
            },
            BackgroundColor(Color::srgba(0.12, 0.11, 0.08, 0.95)),
            BorderColor::all(Color::srgb(0.76, 0.7, 0.52)),
            BuildMenuUi,
            TooltipBox,
            children![(
                TooltipText,
                Text::new(""),
                TextFont {
                    font_size: 11.0,
                    ..default()
                },
                TextColor(Color::srgb(0.94, 0.9, 0.76)),
            )],
        ));
    });
}

/// Conservado por compatibilidad del pipeline startup; la UI vive en la toolbar superior.
pub fn setup_build_menu(_commands: Commands) {
}

pub(crate) fn toolbar_group_interaction(
    mut q: Query<(&Interaction, &ToolbarGroup), (Changed<Interaction>, With<ToolbarGroupButton>)>,
    mut toolbar_state: ResMut<ToolbarState>,
) {
    for (interaction, group) in &mut q {
        if *interaction == Interaction::Pressed {
            toolbar_state.active_group = *group;
        }
    }
}

pub(crate) fn update_toolbar_group_visuals(
    toolbar_state: Res<ToolbarState>,
    mut q: Query<(&ToolbarGroup, &Interaction, &mut BackgroundColor), With<ToolbarGroupButton>>,
) {
    for (group, interaction, mut bg) in &mut q {
        *bg = if *group == toolbar_state.active_group && *interaction == Interaction::Pressed {
            BackgroundColor(Color::srgb(0.7, 0.64, 0.44))
        } else if *group == toolbar_state.active_group {
            BackgroundColor(Color::srgb(0.58, 0.52, 0.34))
        } else if *interaction == Interaction::Hovered {
            BackgroundColor(Color::srgb(0.44, 0.4, 0.28))
        } else {
            BackgroundColor(Color::srgb(0.36, 0.33, 0.24))
        };
    }
}

pub(crate) fn update_toolbar_tool_visibility(
    toolbar_state: Res<ToolbarState>,
    mut q: Query<(&ToolButtonGroup, &mut Node)>,
) {
    if !toolbar_state.is_changed() {
        return;
    }
    for (tool_group, mut node) in &mut q {
        node.display = if tool_group.0 == toolbar_state.active_group {
            Display::Flex
        } else {
            Display::None
        };
        // Desplaza el popup para anclarlo debajo del tab activo.
        let offset = match toolbar_state.active_group {
            ToolbarGroup::Transport => -56.0,
            ToolbarGroup::Build => -28.0,
            ToolbarGroup::Economy => 0.0,
            ToolbarGroup::Info => 28.0,
            ToolbarGroup::Settings => 56.0,
        };
        node.margin.left = Val::Px(offset);
    }
}

/// El botón del menú selecciona la herramienta activa para aplicar en el mapa.
#[allow(clippy::type_complexity)]
pub(crate) fn build_menu_interaction(
    mut q: Query<(&Interaction, &BuildMenuAction), (Changed<Interaction>, With<Button>)>,
    mut tool_state: ResMut<UiToolState>,
) {
    for (interaction, action) in &mut q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        tool_state.active_tool = Some(*action);
    }
}

/// Resalta el botón de herramienta actualmente activo.
pub(crate) fn update_tool_button_visuals(
    tool_state: Res<UiToolState>,
    mut q: Query<(&BuildMenuAction, &Interaction, &mut BackgroundColor), With<ToolSelectButton>>,
) {
    for (action, interaction, mut bg) in &mut q {
        let is_active = tool_state
            .active_tool
            .is_some_and(|active| active == *action);
        *bg = if is_active && *interaction == Interaction::Pressed {
            BackgroundColor(Color::srgb(0.64, 0.58, 0.4))
        } else if is_active {
            BackgroundColor(Color::srgb(0.54, 0.48, 0.33))
        } else if *interaction == Interaction::Hovered {
            BackgroundColor(Color::srgb(0.42, 0.38, 0.27))
        } else {
            BackgroundColor(Color::srgb(0.3, 0.28, 0.2))
        };
    }
}

pub(crate) fn update_toolbar_tooltip(
    mut tooltip_q: Query<&mut Node, With<TooltipBox>>,
    mut text_q: Query<&mut Text, With<TooltipText>>,
    target_q: Query<(&Interaction, &ToolbarTooltipTarget)>,
) {
    let mut hovered: Option<&'static str> = None;
    for (interaction, tip) in &target_q {
        if *interaction == Interaction::Hovered {
            hovered = Some(tip.text);
            break;
        }
    }

    let Ok(mut tooltip_text) = text_q.single_mut() else {
        return;
    };
    let Ok(mut node) = tooltip_q.single_mut() else {
        return;
    };

    if let Some(text) = hovered {
        **tooltip_text = text.to_string();
        node.display = Display::Flex;
    } else {
        node.display = Display::None;
    }
}

/// Clic izquierdo: selecciona tile y aplica herramienta activa (si existe).
pub fn handle_tile_click(
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cam_q: Query<(&Camera, &Transform), With<Camera2d>>,
    mut selected: ResMut<SelectedTileInfo>,
    mut sim: ResMut<SimWorld>,
    tool_state: Res<UiToolState>,
    mut pending: ResMut<RemapMapVisualsPending>,
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
    let pos = TileCoord::new(tx, ty);
    selected.pos = Some(pos);

    let Some(action) = tool_state.active_tool else {
        return;
    };
    let cmd = match action {
        BuildMenuAction::Road => Command::PlaceRoad(pos),
        BuildMenuAction::Rail => Command::PlaceRail(pos),
        BuildMenuAction::Station => Command::PlaceStation(pos),
        BuildMenuAction::Clear => Command::ClearTile(pos),
    };
    if apply_command(&mut sim.state, &cmd).is_ok() {
        pending.pending = true;
    }
}

/// Actualiza el texto de información del tile seleccionado.
#[allow(clippy::type_complexity)]
pub fn update_tile_info_text(
    selected: Res<SelectedTileInfo>,
    sim: Res<SimWorld>,
    hud: Res<SimHudControls>,
    tool_state: Res<UiToolState>,
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
    let tool_l = match tool_state.active_tool {
        Some(BuildMenuAction::Road) => "Road",
        Some(BuildMenuAction::Rail) => "Rail",
        Some(BuildMenuAction::Station) => "Station",
        Some(BuildMenuAction::Clear) => "Clear",
        None => "None",
    };
    let hud_footer = format!(
        "{pause_l} | Tool: {tool_l} | JSON: {} | F4 otra ruta",
        hud.json_save_path
    );

    let Some(pos) = selected.pos else {
        **text =
            format!(
                "{zoom_label}\n{hud_footer}\nClic mapa: elegir tile · toolbar tools · 1/2/3/C/Esc"
            );
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
    let coast_dbg = if config::env_flag("OPENTTDRS_DEBUG_COAST")
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

/// Hotkeys de herramienta: 1 carretera, 2 estación, C limpiar, Esc desactivar.
pub fn handle_tool_hotkeys(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut tool_state: ResMut<UiToolState>,
) {
    if keyboard.just_pressed(KeyCode::Digit1) {
        tool_state.active_tool = Some(BuildMenuAction::Road);
    } else if keyboard.just_pressed(KeyCode::Digit2) {
        tool_state.active_tool = Some(BuildMenuAction::Station);
    } else if keyboard.just_pressed(KeyCode::Digit3) {
        tool_state.active_tool = Some(BuildMenuAction::Rail);
    } else if keyboard.just_pressed(KeyCode::KeyC) {
        tool_state.active_tool = Some(BuildMenuAction::Clear);
    } else if keyboard.just_pressed(KeyCode::Escape) {
        tool_state.active_tool = None;
    }
}
