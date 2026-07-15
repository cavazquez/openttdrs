//! Lista de objetivos GameScript-lite (#43).

use bevy::prelude::*;

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
pub(crate) struct GoalListWindowState {
    pub(crate) open: bool,
}

#[derive(Component)]
pub(crate) struct GoalListRoot;

#[derive(Component)]
struct GoalListRow;

#[derive(Default)]
pub(crate) struct GoalListCache {
    fingerprint: u64,
}

pub(crate) fn setup_goal_list_window(mut commands: Commands, asset_server: Res<AssetServer>) {
    let asset_server = &*asset_server;
    let (_root, content) = spawn_floating_window(
        &mut commands,
        asset_server,
        FloatingWindowId::Goals,
        "Objetivos",
        TITLE_CREAM,
        Vec2::new(400.0, 100.0),
        420.0,
    );
    commands.entity(content).with_children(|body| {
        body.spawn((
            Text::new("GameScript-lite · progreso de goals del escenario"),
            window_text_font(asset_server, UiFontRole::Caption),
            TextColor(Color::srgb(0.82, 0.78, 0.68)),
            Node {
                margin: UiRect::bottom(Val::Px(6.0)),
                ..default()
            },
            BuildMenuUi,
        ));
        spawn_list_scroll_area(body, GoalListRoot, LIST_DEFAULT_HEIGHT);
    });
}

pub(crate) fn open_goal_list_from_routes(
    mut routes: MessageReader<OpenUiRoute>,
    mut state: ResMut<GoalListWindowState>,
) {
    for route in routes.read() {
        if route.0 == UiRoute::Goals {
            state.open = true;
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn sync_goal_list_window(
    state: Res<GoalListWindowState>,
    sim: Option<Res<SimWorld>>,
    mut windows: Query<(&FloatingWindow, &mut Visibility)>,
    list_roots: Query<Entity, With<GoalListRoot>>,
    children_q: Query<&Children>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut cache: Local<GoalListCache>,
) {
    for (w, mut vis) in &mut windows {
        if w.id == FloatingWindowId::Goals {
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
    let fingerprint = goal_fingerprint(&sim.state.gs);
    if fingerprint == cache.fingerprint {
        return;
    }
    cache.fingerprint = fingerprint;
    let Ok(list_root) = list_roots.single() else {
        return;
    };
    clear_list_children(&mut commands, list_root, &children_q);
    let goals = sim.state.gs.goals.clone();
    let enabled = sim.state.gs.enabled;
    commands.entity(list_root).with_children(|list| {
        if !enabled || goals.is_empty() {
            spawn_list_empty_label(list, &asset_server, "Sin escenario GS activo");
            return;
        }
        for goal in &goals {
            let mark = if goal.completed { "✓" } else { "·" };
            spawn_list_row_button(
                list,
                &asset_server,
                format!(
                    "{mark} {}  ({}/{})",
                    goal.title, goal.progress_num, goal.progress_den
                ),
                GoalListRow,
                goal.completed,
            );
        }
    });
}

fn goal_fingerprint(gs: &openttdrs_core::GsState) -> u64 {
    let mut h = u64::from(gs.enabled) << 63;
    h ^= u64::from(gs.all_complete) << 62;
    h ^= (gs.goals.len() as u64).wrapping_mul(0x9E37);
    for g in &gs.goals {
        h = h
            .wrapping_mul(31)
            .wrapping_add(u64::from(g.id))
            .wrapping_add(g.progress_num.wrapping_mul(17))
            .wrapping_add(u64::from(g.completed));
    }
    h
}

pub(crate) fn goal_list_window_on_closed(
    mut closed: MessageReader<FloatingWindowClosed>,
    mut state: ResMut<GoalListWindowState>,
) {
    for msg in closed.read() {
        if msg.0 == FloatingWindowId::Goals {
            state.open = false;
        }
    }
}
