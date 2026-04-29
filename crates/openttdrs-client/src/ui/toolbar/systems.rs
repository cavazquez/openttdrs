use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use openttdrs_core::{Command, TileCoord, apply_command};

use crate::iso::world_pos_to_tile_coord;
use crate::state::SimWorld;
use crate::world_render::RemapMapVisualsPending;

use super::super::hud::SelectedTileInfo;
use super::{
    BuildMenuAction, BuildMenuUi, ToolButtonGroup, ToolSelectButton, ToolbarGroup, ToolbarGroupButton,
    ToolbarState, ToolbarTooltipTarget, TooltipBox, TooltipText, UiToolState,
};

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

/// El boton del menu selecciona la herramienta activa para aplicar en el mapa.
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

/// Resalta el boton de herramienta actualmente activo.
pub(crate) fn update_tool_button_visuals(
    tool_state: Res<UiToolState>,
    mut q: Query<(&BuildMenuAction, &Interaction, &mut BackgroundColor), With<ToolSelectButton>>,
) {
    for (action, interaction, mut bg) in &mut q {
        let is_active = tool_state.active_tool.is_some_and(|active| active == *action);
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
pub(crate) fn handle_tile_click(
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
