//! Lista de carteles del mapa (SignList) con centrar / renombrar / borrar.

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use openttdrs_core::Command;
use openttdrs_core::MAX_SIGN_NAME_CHARS;

use crate::iso::tile_pos;
use crate::render::{
    MapPreviewCamera, PrimaryGameCamera, RemapMapVisualsPending,
    request_map_visual_remap_with_labels,
};
use crate::state::SimWorld;
use crate::ui::floating_window::{
    FloatingWindow, FloatingWindowClosed, FloatingWindowId, TITLE_BROWN, WINDOW_TEXT,
    spawn_floating_window, window_text_font,
};
use crate::ui::font::UiFontRole;
use crate::ui::navigation::{OpenUiRoute, UiRoute};
use crate::ui::toolbar::BuildMenuUi;

const BTN_BG: Color = Color::srgb(0.36, 0.31, 0.21);

#[derive(Resource, Default)]
pub(crate) struct SignListWindowState {
    pub(crate) open: bool,
    pub(crate) selected: Option<u32>,
    pub(crate) rename_editing: bool,
}

#[derive(Component)]
pub(crate) struct SignListBodyText;

#[derive(Component)]
pub(crate) struct SignListRenameInput;

#[derive(Component, Clone, Copy)]
pub(crate) enum SignListAction {
    Center,
    Rename,
    Delete,
    ApplyRename,
    CancelRename,
}

pub(crate) fn setup_sign_list_window(mut commands: Commands, asset_server: Res<AssetServer>) {
    let asset_server = &*asset_server;
    let (_root, content) = spawn_floating_window(
        &mut commands,
        asset_server,
        FloatingWindowId::SignList,
        "Carteles",
        TITLE_BROWN,
        Vec2::new(420.0, 140.0),
        380.0,
    );
    commands.entity(content).with_children(|panel| {
        panel.spawn((
            Button,
            SignListBodyText,
            Text::new(""),
            window_text_font(asset_server, UiFontRole::Caption),
            TextColor(WINDOW_TEXT),
            BuildMenuUi,
            Interaction::default(),
            Node {
                width: Val::Percent(100.0),
                min_height: Val::Px(120.0),
                ..default()
            },
            BackgroundColor(Color::NONE),
        ));
        panel
            .spawn(Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(4.0),
                margin: UiRect::top(Val::Px(6.0)),
                ..default()
            })
            .with_children(|row| {
                for (label, action) in [
                    ("Centrar", SignListAction::Center),
                    ("Renombrar", SignListAction::Rename),
                    ("Borrar", SignListAction::Delete),
                ] {
                    spawn_action_btn(row, asset_server, label, action);
                }
            });
        panel.spawn((
            SignListRenameInput,
            Text::new(""),
            window_text_font(asset_server, UiFontRole::Body),
            TextColor(WINDOW_TEXT),
            BuildMenuUi,
            Node {
                width: Val::Percent(100.0),
                min_height: Val::Px(22.0),
                margin: UiRect::top(Val::Px(6.0)),
                padding: UiRect::all(Val::Px(4.0)),
                border: UiRect::all(Val::Px(1.0)),
                display: Display::None,
                ..default()
            },
            BackgroundColor(Color::srgb(0.28, 0.22, 0.16)),
            BorderColor::all(Color::srgb(0.66, 0.58, 0.38)),
        ));
        panel
            .spawn(Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(4.0),
                margin: UiRect::top(Val::Px(4.0)),
                ..default()
            })
            .with_children(|row| {
                spawn_action_btn(row, asset_server, "Aplicar", SignListAction::ApplyRename);
                spawn_action_btn(row, asset_server, "Cancelar", SignListAction::CancelRename);
            });
    });
}

fn spawn_action_btn(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    label: &str,
    action: SignListAction,
) {
    parent.spawn((
        Button,
        action,
        BuildMenuUi,
        Node {
            min_width: Val::Px(72.0),
            height: Val::Px(24.0),
            padding: UiRect::horizontal(Val::Px(6.0)),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        BackgroundColor(BTN_BG),
        BorderColor::all(Color::srgb(0.66, 0.58, 0.38)),
        Interaction::default(),
        children![(
            Text::new(label),
            window_text_font(asset_server, UiFontRole::Caption),
            TextColor(WINDOW_TEXT),
        )],
    ));
}

pub(crate) fn open_sign_list_from_routes(
    mut routes: MessageReader<OpenUiRoute>,
    mut state: ResMut<SignListWindowState>,
) {
    for route in routes.read() {
        if route.0 == UiRoute::SignList {
            state.open = true;
        }
    }
}

pub(crate) fn sync_sign_list_window(
    sim: Res<SimWorld>,
    mut state: ResMut<SignListWindowState>,
    mut root_q: Query<(&FloatingWindow, &mut Visibility)>,
    mut body_q: Query<&mut Text, (With<SignListBodyText>, Without<SignListRenameInput>)>,
    mut rename_q: Query<
        (&mut Text, &mut Node),
        (With<SignListRenameInput>, Without<SignListBodyText>),
    >,
) {
    let Some((_, mut vis)) = root_q
        .iter_mut()
        .find(|(w, _)| w.id == FloatingWindowId::SignList)
    else {
        return;
    };
    *vis = if state.open {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
    if !state.open {
        return;
    }
    if state
        .selected
        .is_some_and(|id| !sim.state.signs.iter().any(|s| s.id == id))
    {
        state.selected = None;
        state.rename_editing = false;
    }
    if let Ok(mut body) = body_q.single_mut() {
        if sim.state.signs.is_empty() {
            **body = "Sin carteles.\n\nUsa Paisaje → Cartel para colocar uno.".into();
        } else {
            let mut lines = String::from("Clic en una fila para seleccionar:\n");
            for sign in &sim.state.signs {
                let mark = if state.selected == Some(sign.id) {
                    ">"
                } else {
                    " "
                };
                lines.push_str(&format!(
                    "{mark} #{:<3} ({}, {})  {}\n",
                    sign.id, sign.pos.x, sign.pos.y, sign.name
                ));
            }
            **body = lines;
        }
    }
    if let Ok((mut rename_text, mut node)) = rename_q.single_mut() {
        node.display = if state.rename_editing {
            Display::Flex
        } else {
            Display::None
        };
        if !state.rename_editing {
            **rename_text = String::new();
        }
    }
}

pub(crate) fn handle_sign_list_buttons(
    buttons: Query<(&Interaction, &SignListAction), (Changed<Interaction>, With<Button>)>,
    mut state: ResMut<SignListWindowState>,
    mut sim: ResMut<SimWorld>,
    mut pending: ResMut<RemapMapVisualsPending>,
    mut cam_q: Query<&mut Transform, (With<PrimaryGameCamera>, Without<MapPreviewCamera>)>,
    mut rename_q: Query<&mut Text, With<SignListRenameInput>>,
) {
    for (interaction, action) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match *action {
            SignListAction::Center => {
                let Some(id) = state.selected else {
                    continue;
                };
                let Some(sign) = sim.state.signs.iter().find(|s| s.id == id) else {
                    continue;
                };
                if let Ok(mut tf) = cam_q.single_mut() {
                    let ground = tile_pos(sign.pos.x, sign.pos.y, 1, 0.0);
                    tf.translation.x = ground.x;
                    tf.translation.y = ground.y;
                }
            }
            SignListAction::Rename => {
                let Some(id) = state.selected else {
                    continue;
                };
                let Some(name) = sim
                    .state
                    .signs
                    .iter()
                    .find(|s| s.id == id)
                    .map(|s| s.name.clone())
                else {
                    continue;
                };
                state.rename_editing = true;
                if let Ok(mut text) = rename_q.single_mut() {
                    **text = name;
                }
            }
            SignListAction::Delete => {
                let Some(id) = state.selected else {
                    continue;
                };
                if crate::network::apply_player_command(
                    &mut sim.state,
                    &Command::RemoveSign { sign_id: id },
                )
                .is_ok()
                {
                    state.selected = None;
                    state.rename_editing = false;
                    let (mw, mh) = sim.state.map.dimensions();
                    request_map_visual_remap_with_labels(&mut pending, mw, mh, &[]);
                }
            }
            SignListAction::ApplyRename => {
                let Some(id) = state.selected else {
                    continue;
                };
                let name = rename_q
                    .single()
                    .ok()
                    .map(|t| t.to_string())
                    .unwrap_or_default();
                if crate::network::apply_player_command(
                    &mut sim.state,
                    &Command::RenameSign {
                        sign_id: id,
                        name: Some(name),
                    },
                )
                .is_ok()
                {
                    state.rename_editing = false;
                    let (mw, mh) = sim.state.map.dimensions();
                    request_map_visual_remap_with_labels(&mut pending, mw, mh, &[]);
                }
            }
            SignListAction::CancelRename => {
                state.rename_editing = false;
            }
        }
    }
}

/// Selección por clic en el cuerpo de texto (parsea la línea bajo el cursor de forma simple:
/// cicla al siguiente cartel al pulsar el cuerpo).
pub(crate) fn handle_sign_list_body_click(
    body: Query<&Interaction, (Changed<Interaction>, With<SignListBodyText>)>,
    sim: Res<SimWorld>,
    mut state: ResMut<SignListWindowState>,
    mut rename_q: Query<&mut Text, With<SignListRenameInput>>,
) {
    for interaction in &body {
        if *interaction != Interaction::Pressed || sim.state.signs.is_empty() {
            continue;
        }
        let ids: Vec<u32> = sim.state.signs.iter().map(|s| s.id).collect();
        let next = match state
            .selected
            .and_then(|cur| ids.iter().position(|id| *id == cur))
        {
            Some(i) => ids[(i + 1) % ids.len()],
            None => ids[0],
        };
        state.selected = Some(next);
        if let Some(sign) = sim.state.signs.iter().find(|s| s.id == next)
            && let Ok(mut text) = rename_q.single_mut()
        {
            **text = sign.name.clone();
        }
    }
}

pub(crate) fn sign_list_rename_keyboard(
    mut events: MessageReader<KeyboardInput>,
    state: Res<SignListWindowState>,
    mut rename_q: Query<&mut Text, With<SignListRenameInput>>,
) {
    if !state.rename_editing {
        return;
    }
    let Ok(mut text) = rename_q.single_mut() else {
        return;
    };
    for ev in events.read() {
        if ev.state != ButtonState::Pressed {
            continue;
        }
        match &ev.logical_key {
            Key::Backspace => {
                let mut s = text.to_string();
                s.pop();
                **text = s;
            }
            Key::Character(c) => {
                let mut s = text.to_string();
                if s.chars().count() < MAX_SIGN_NAME_CHARS {
                    s.push_str(c);
                    **text = s;
                }
            }
            _ => {}
        }
    }
}

pub(crate) fn sign_list_window_on_closed(
    mut closed: MessageReader<FloatingWindowClosed>,
    mut state: ResMut<SignListWindowState>,
) {
    for msg in closed.read() {
        if msg.0.class == FloatingWindowId::SignList {
            state.open = false;
            state.rename_editing = false;
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    #[test]
    fn route_opens_sign_list() {
        let mut world = World::new();
        world.init_resource::<SignListWindowState>();
        world.init_resource::<Messages<OpenUiRoute>>();
        world.write_message(OpenUiRoute(UiRoute::SignList));
        world.run_system_once(open_sign_list_from_routes).unwrap();
        assert!(world.resource::<SignListWindowState>().open);
    }
}
