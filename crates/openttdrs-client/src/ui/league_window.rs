//! Tabla de liga / ranking de compañías (#43).

use bevy::prelude::*;
use openttdrs_core::{format_money, league_rows};

use crate::state::SimWorld;
use crate::ui::floating_window::{
    FloatingWindow, FloatingWindowClosed, FloatingWindowId, TITLE_CREAM, spawn_floating_window,
    window_text_font,
};
use crate::ui::font::UiFontRole;
use crate::ui::list_window::{
    LIST_DEFAULT_HEIGHT, clear_list_children, spawn_list_empty_label, spawn_list_row_button,
    spawn_list_scroll_area,
};
use crate::ui::navigation::{OpenUiRoute, UiRoute};
use crate::ui::toolbar::BuildMenuUi;

#[derive(Resource, Default)]
pub(crate) struct LeagueWindowState {
    pub(crate) open: bool,
}

#[derive(Component)]
pub(crate) struct LeagueListRoot;

#[derive(Component)]
struct LeagueListRow;

#[derive(Default)]
pub(crate) struct LeagueCache {
    fingerprint: u64,
}

pub(crate) fn setup_league_window(mut commands: Commands, asset_server: Res<AssetServer>) {
    let asset_server = &*asset_server;
    let (_root, content) = spawn_floating_window(
        &mut commands,
        asset_server,
        FloatingWindowId::League,
        "Liga",
        TITLE_CREAM,
        Vec2::new(440.0, 110.0),
        460.0,
    );
    commands.entity(content).with_children(|body| {
        body.spawn((
            Text::new("Compañías ordenadas por valor neto · performance trimestral"),
            window_text_font(asset_server, UiFontRole::Caption),
            TextColor(Color::srgb(0.82, 0.78, 0.68)),
            Node {
                margin: UiRect::bottom(Val::Px(6.0)),
                ..default()
            },
            BuildMenuUi,
        ));
        spawn_list_scroll_area(body, asset_server, LeagueListRoot, LIST_DEFAULT_HEIGHT);
    });
}

pub(crate) fn open_league_from_routes(
    mut routes: MessageReader<OpenUiRoute>,
    mut state: ResMut<LeagueWindowState>,
) {
    for route in routes.read() {
        if route.0 == UiRoute::League {
            state.open = true;
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn sync_league_window(
    state: Res<LeagueWindowState>,
    sim: Option<Res<SimWorld>>,
    mut windows: Query<(&FloatingWindow, &mut Visibility)>,
    list_roots: Query<Entity, With<LeagueListRoot>>,
    children_q: Query<&Children>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut cache: Local<LeagueCache>,
) {
    for (w, mut vis) in &mut windows {
        if w.id == FloatingWindowId::League {
            *vis = if state.open {
                Visibility::Visible
            } else {
                Visibility::Hidden
            };
        }
    }
    if !state.open {
        cache.fingerprint = 0;
        return;
    }
    let Some(sim) = sim.as_deref() else {
        return;
    };
    let rows = league_rows(&sim.state);
    let fingerprint = rows.iter().fold(rows.len() as u64, |acc, r| {
        acc.wrapping_mul(31)
            .wrapping_add(u64::from(r.company_id))
            .wrapping_add(r.net_value as u64)
            .wrapping_add(r.performance as u64)
    });
    if fingerprint == cache.fingerprint {
        return;
    }
    cache.fingerprint = fingerprint;
    let Ok(list_root) = list_roots.single() else {
        return;
    };
    clear_list_children(&mut commands, list_root, &children_q);
    commands.entity(list_root).with_children(|list| {
        if rows.is_empty() {
            spawn_list_empty_label(list, &asset_server, "Sin compañías");
            return;
        }
        for (rank, row) in rows.iter().enumerate() {
            let kind = if row.is_ai { "IA" } else { "Humana" };
            spawn_list_row_button(
                list,
                &asset_server,
                format!(
                    "{}. {} ({})  {}  perf {}",
                    rank + 1,
                    row.name,
                    kind,
                    format_money(row.net_value),
                    row.performance
                ),
                LeagueListRow,
                rank == 0,
            );
        }
    });
}

pub(crate) fn league_window_on_closed(
    mut closed: MessageReader<FloatingWindowClosed>,
    mut state: ResMut<LeagueWindowState>,
) {
    for msg in closed.read() {
        if msg.0.class == FloatingWindowId::League {
            state.open = false;
        }
    }
}
