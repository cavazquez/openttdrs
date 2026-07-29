//! Diálogos MVP (#272): QueryString, ErrorDialog y OSK stub.
//!
//! No pixel-perfect; chrome flotante + sync con [`ModalStack`]. Snapshots → #240.

use bevy::prelude::*;

use crate::ui::floating_window::{
    FloatingWindow, FloatingWindowClosed, FloatingWindowId, TITLE_BROWN, WINDOW_TEXT, WindowKey,
    spawn_floating_window, window_text_font,
};
use crate::ui::font::UiFontRole;
use crate::ui::modal_stack::{
    ModalKind, ModalStack, push_error_dialog, push_osk, push_query_string,
};
use crate::ui::toolbar::BuildMenuUi;
use crate::ui::window_lifecycle::{
    close_floating_window_on_message, sync_floating_window_visibility,
};

#[derive(Resource, Default)]
pub(crate) struct QueryStringWindowState {
    pub(crate) open: bool,
}

#[derive(Resource, Default)]
pub(crate) struct ErrorDialogWindowState {
    pub(crate) open: bool,
}

#[derive(Resource, Default)]
pub(crate) struct OskWindowState {
    pub(crate) open: bool,
}

#[derive(Component)]
pub(crate) struct DialogBodyText(FloatingWindowId);

pub(crate) fn setup_dialog_windows(mut commands: Commands, asset_server: Res<AssetServer>) {
    let asset_server = &*asset_server;
    for (id, title, hint) in [
        (
            FloatingWindowId::QueryString,
            "Consulta",
            "QueryString — Enter confirma, Esc cancela",
        ),
        (
            FloatingWindowId::ErrorDialog,
            "Error",
            "ErrorDialog — Enter/Esc cierra",
        ),
        (
            FloatingWindowId::OnScreenKeyboard,
            "Teclado",
            "OSK stub — edición de texto (NewGRF rename / query)",
        ),
    ] {
        let (_root, content) = spawn_floating_window(
            &mut commands,
            asset_server,
            id,
            title,
            TITLE_BROWN,
            Vec2::new(200.0, 160.0),
            320.0,
        );
        commands.entity(content).with_children(|body| {
            body.spawn((
                Text::new(hint),
                window_text_font(asset_server, UiFontRole::Caption),
                TextColor(WINDOW_TEXT),
                DialogBodyText(id),
                BuildMenuUi,
                Node {
                    width: Val::Percent(100.0),
                    ..default()
                },
            ));
        });
    }
}

pub(crate) fn sync_dialog_windows(
    stack: Res<ModalStack>,
    mut query_state: ResMut<QueryStringWindowState>,
    mut error_state: ResMut<ErrorDialogWindowState>,
    mut osk_state: ResMut<OskWindowState>,
    mut windows: Query<(&FloatingWindow, &mut Visibility)>,
    mut body_q: Query<(&DialogBodyText, &mut Text)>,
) {
    query_state.open = stack.contains_kind(ModalKind::QueryString);
    error_state.open = stack.contains_kind(ModalKind::ErrorDialog);
    osk_state.open = stack.contains_kind(ModalKind::OnScreenKeyboard);

    if let Some(top) = stack.top() {
        for (body, mut text) in &mut body_q {
            if body.0 == top.kind.window_id() {
                *text = Text::new(top.text.clone());
            }
        }
    }

    sync_floating_window_visibility(
        &mut windows,
        FloatingWindowId::QueryString,
        query_state.open,
    );
    sync_floating_window_visibility(
        &mut windows,
        FloatingWindowId::ErrorDialog,
        error_state.open,
    );
    sync_floating_window_visibility(
        &mut windows,
        FloatingWindowId::OnScreenKeyboard,
        osk_state.open,
    );
}

pub(crate) fn dialog_windows_on_closed(
    mut closed: MessageReader<FloatingWindowClosed>,
    mut stack: ResMut<ModalStack>,
    mut query_state: ResMut<QueryStringWindowState>,
    mut error_state: ResMut<ErrorDialogWindowState>,
    mut osk_state: ResMut<OskWindowState>,
) {
    close_floating_window_on_message(&mut closed, FloatingWindowId::QueryString, || {
        query_state.open = false;
        if stack
            .top()
            .is_some_and(|e| e.kind == ModalKind::QueryString)
        {
            let _ = stack.pop_cancel();
        }
    });
    close_floating_window_on_message(&mut closed, FloatingWindowId::ErrorDialog, || {
        error_state.open = false;
        if stack
            .top()
            .is_some_and(|e| e.kind == ModalKind::ErrorDialog)
        {
            let _ = stack.pop_cancel();
        }
    });
    close_floating_window_on_message(&mut closed, FloatingWindowId::OnScreenKeyboard, || {
        osk_state.open = false;
        if stack
            .top()
            .is_some_and(|e| e.kind == ModalKind::OnScreenKeyboard)
        {
            let _ = stack.pop_cancel();
        }
    });
}

/// Enter confirma query/error del modal tope.
pub(crate) fn handle_modal_enter(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut stack: ResMut<ModalStack>,
) {
    if !keyboard.just_pressed(KeyCode::Enter) && !keyboard.just_pressed(KeyCode::NumpadEnter) {
        return;
    }
    let _ = stack.handle_enter();
}

/// Helpers de prueba / NewGRF rename path.
#[allow(dead_code)]
pub(crate) fn open_query_for_newgrf_rename(stack: &mut ModalStack, initial: &str) {
    let owner = WindowKey::singleton(FloatingWindowId::NewGrf);
    push_query_string(stack, Some(owner), initial);
}

#[allow(dead_code)]
pub(crate) fn open_osk_for_query(stack: &mut ModalStack, initial: &str) {
    let owner = stack
        .top()
        .map(|e| e.key)
        .unwrap_or_else(|| WindowKey::singleton(FloatingWindowId::QueryString));
    push_osk(stack, Some(owner), initial);
}

#[allow(dead_code)]
pub(crate) fn open_error_modal(stack: &mut ModalStack, message: &str) {
    push_error_dialog(stack, None, message);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::modal_stack::MODAL_BASE_Z;

    #[test]
    fn osk_stub_pushes_over_query_owner() {
        let mut stack = ModalStack::default();
        open_query_for_newgrf_rename(&mut stack, "grf-name");
        assert!(stack.owner_is_blocked(WindowKey::singleton(FloatingWindowId::NewGrf)));
        let z = push_osk(
            &mut stack,
            Some(WindowKey::singleton(FloatingWindowId::QueryString)),
            "grf-name",
        );
        assert!(z >= MODAL_BASE_Z);
        assert_eq!(
            stack.top().map(|e| e.kind),
            Some(ModalKind::OnScreenKeyboard)
        );
        assert!(stack.handle_escape());
        assert_eq!(stack.top().map(|e| e.kind), Some(ModalKind::QueryString));
    }

    #[test]
    fn dialogs_enter_escape_path_for_query_error() {
        let mut stack = ModalStack::default();
        open_error_modal(&mut stack, "fail");
        assert!(stack.handle_enter());
        open_query_for_newgrf_rename(&mut stack, "x");
        assert!(stack.handle_escape());
        assert!(stack.is_empty());
    }
}
