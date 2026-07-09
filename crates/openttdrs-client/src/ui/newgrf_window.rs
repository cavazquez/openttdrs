//! Ventana NewGRF (Fase 7 MVP): lista de solo lectura del stack activo.

use bevy::prelude::*;
use openttdrs_core::format_grfid;

use crate::state::SimWorld;
use crate::ui::floating_window::{
    FloatingWindow, FloatingWindowClosed, FloatingWindowId, TITLE_BROWN, WINDOW_TEXT,
    spawn_floating_window, window_text_font,
};
use crate::ui::font::UiFontRole;

#[derive(Resource, Default)]
pub(crate) struct NewGrfWindowState {
    pub(crate) open: bool,
}

#[derive(Component)]
pub(crate) struct NewGrfListText;

pub(crate) fn setup_newgrf_window(mut commands: Commands, asset_server: Res<AssetServer>) {
    let asset_server = &*asset_server;
    let (_root, content) = spawn_floating_window(
        &mut commands,
        asset_server,
        FloatingWindowId::NewGrf,
        "NewGRF",
        TITLE_BROWN,
        Vec2::new(320.0, 120.0),
        420.0,
    );
    commands.entity(content).with_children(|body| {
        body.spawn((
            Text::new(
                "Stack activo (solo lectura). Runtime Action0–14 pendiente; sprites OpenGFX pre-bakeados.",
            ),
            window_text_font(asset_server, UiFontRole::Caption),
            TextColor(Color::srgb(0.82, 0.78, 0.68)),
            Node {
                margin: UiRect::bottom(Val::Px(8.0)),
                ..default()
            },
        ));
        body.spawn((
            NewGrfListText,
            Text::new(""),
            window_text_font(asset_server, UiFontRole::Body),
            TextColor(WINDOW_TEXT),
            Node {
                width: Val::Percent(100.0),
                ..default()
            },
        ));
    });
}

pub(crate) fn sync_newgrf_window(
    state: Res<NewGrfWindowState>,
    sim: Option<Res<SimWorld>>,
    mut windows: Query<(&FloatingWindow, &mut Visibility)>,
    mut list: Query<&mut Text, With<NewGrfListText>>,
) {
    let Some(sim) = sim else {
        return;
    };
    let visible = state.open;
    for (w, mut vis) in &mut windows {
        if w.id == FloatingWindowId::NewGrf {
            *vis = if visible {
                Visibility::Visible
            } else {
                Visibility::Hidden
            };
        }
    }
    if !visible {
        return;
    }
    let Ok(mut text) = list.single_mut() else {
        return;
    };
    if sim.state.newgrf_stack.is_empty() {
        **text = "(stack vacío)".into();
        return;
    }
    let mut lines = String::new();
    for (i, e) in sim.state.newgrf_stack.iter().enumerate() {
        let flag = if e.enabled { "ON" } else { "off" };
        let static_mark = if e.is_static { " [base]" } else { "" };
        let name = if e.name.is_empty() {
            e.filename.as_str()
        } else {
            e.name.as_str()
        };
        lines.push_str(&format!(
            "{}. [{flag}] {name}{static_mark}\n   {}  {}\n",
            i + 1,
            format_grfid(e.grfid),
            e.filename
        ));
    }
    **text = lines;
}

pub(crate) fn newgrf_window_on_closed(
    mut closed: MessageReader<FloatingWindowClosed>,
    mut state: ResMut<NewGrfWindowState>,
) {
    for msg in closed.read() {
        if msg.0 == FloatingWindowId::NewGrf {
            state.open = false;
        }
    }
}
