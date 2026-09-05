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
use crate::ui::window_lifecycle::{
    close_floating_window_on_message, sync_floating_window_visibility,
};

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
    // `0` es el fingerprint válido del GameScript desactivado y sin goals.
    // `None` distingue la primera apertura de una lista ya vacía.
    fingerprint: Option<u64>,
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
        spawn_list_scroll_area(body, asset_server, GoalListRoot, LIST_DEFAULT_HEIGHT);
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
    sync_floating_window_visibility(&mut windows, FloatingWindowId::Goals, state.open);
    if !state.open {
        cache.fingerprint = None;
        return;
    }
    let Some(sim) = sim.as_deref() else {
        return;
    };
    if !refresh_goal_list_cache(&mut cache, &sim.state.gs) {
        return;
    }
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

/// Registra el contenido visto y conserva la primera lista vacía como cambio.
fn refresh_goal_list_cache(cache: &mut GoalListCache, gs: &openttdrs_core::GsState) -> bool {
    let fingerprint = goal_fingerprint(gs);
    if cache.fingerprint == Some(fingerprint) {
        return false;
    }
    cache.fingerprint = Some(fingerprint);
    true
}

pub(crate) fn goal_list_window_on_closed(
    mut closed: MessageReader<FloatingWindowClosed>,
    mut state: ResMut<GoalListWindowState>,
) {
    close_floating_window_on_message(&mut closed, FloatingWindowId::Goals, || {
        state.open = false;
    });
}

#[cfg(test)]
mod tests {
    use super::{GoalListCache, refresh_goal_list_cache};

    #[test]
    fn disabled_empty_goal_list_refreshes_when_opened_for_the_first_time() {
        let mut cache = GoalListCache::default();
        let gs = openttdrs_core::GsState::default();

        assert!(refresh_goal_list_cache(&mut cache, &gs));
        assert!(!refresh_goal_list_cache(&mut cache, &gs));
    }
}
