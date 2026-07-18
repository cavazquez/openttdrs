//! Ventana flotante «Historial de noticias» (N4 — Message history).

use bevy::prelude::*;
use openttdrs_core::NewsDisplayMode;

use crate::state::SimWorld;
use crate::ui::floating_window::{
    FloatingWindow, FloatingWindowClosed, FloatingWindowId, TITLE_BROWN, WINDOW_TEXT,
    spawn_floating_window, window_text_font,
};
use crate::ui::font::UiFontRole;
use crate::ui::toolbar::BuildMenuUi;

use super::NewsUiState;
use super::sync::focus_news_reference;

const HISTORY_WIDTH: f32 = 420.0;
const HISTORY_LIST_HEIGHT: f32 = 300.0;
const ROW_HEIGHT: f32 = 34.0;
const HEADLINE_MAX_CHARS: usize = 52;

#[derive(Resource, Default)]
pub(crate) struct NewsHistoryState {
    pub(crate) open: bool,
}

#[derive(Component)]
pub(crate) struct NewsHistoryScrollArea;

#[derive(Component)]
pub(crate) struct NewsHistoryListRoot;

#[derive(Component, Clone, Copy)]
pub(crate) struct NewsHistoryRow {
    pub(crate) item_id: u64,
}

#[derive(Default)]
pub(crate) struct NewsHistoryListCache {
    ids: Vec<u64>,
}

pub(crate) fn setup_news_history_window(mut commands: Commands, asset_server: Res<AssetServer>) {
    let asset_server = &*asset_server;
    let (_root, content) = spawn_floating_window(
        &mut commands,
        asset_server,
        FloatingWindowId::NewsHistory,
        "Historial de noticias",
        TITLE_BROWN,
        Vec2::new(12.0, 480.0),
        HISTORY_WIDTH,
    );
    commands.entity(content).insert(Node {
        height: Val::Px(HISTORY_LIST_HEIGHT + 12.0),
        ..default()
    });
    commands.entity(content).with_children(|body| {
        body.spawn((
            NewsHistoryScrollArea,
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(HISTORY_LIST_HEIGHT),
                overflow: Overflow::scroll_y(),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(Color::srgb(0.22, 0.18, 0.12)),
            BorderColor::all(Color::srgb(0.45, 0.39, 0.27)),
            BuildMenuUi,
        ))
        .with_children(|scroll| {
            scroll.spawn((
                NewsHistoryListRoot,
                Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(2.0),
                    padding: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                BuildMenuUi,
            ));
        });
    });
}

pub(crate) fn handle_open_news_history(
    mut history: ResMut<NewsHistoryState>,
    interaction_q: Query<&Interaction, (Changed<Interaction>, With<super::StatusBarDateButton>)>,
) {
    for interaction in &interaction_q {
        if *interaction == Interaction::Pressed {
            history.open = true;
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn sync_news_history_window(
    history: Res<NewsHistoryState>,
    sim: Res<SimWorld>,
    mut root_q: Query<(&FloatingWindow, &mut Visibility)>,
    list_roots: Query<Entity, With<NewsHistoryListRoot>>,
    children_q: Query<&Children>,
    mut commands: Commands,
    mut cache: Local<NewsHistoryListCache>,
    asset_server: Res<AssetServer>,
) {
    let Some((_, mut vis)) = root_q
        .iter_mut()
        .find(|(w, _)| w.id == FloatingWindowId::NewsHistory)
    else {
        return;
    };
    if !history.open {
        *vis = Visibility::Hidden;
        cache.ids.clear();
        return;
    }
    *vis = Visibility::Visible;

    let ids: Vec<u64> = sim.state.news.items.iter().map(|item| item.id).collect();
    if ids == cache.ids {
        return;
    }
    cache.ids.clone_from(&ids);

    let Ok(list_root) = list_roots.single() else {
        return;
    };
    if let Ok(children) = children_q.get(list_root) {
        for child in children.iter() {
            commands.entity(child).despawn();
        }
    }

    if ids.is_empty() {
        commands.entity(list_root).with_children(|list| {
            list.spawn((
                Text::new("No hay noticias todavía."),
                window_text_font(&asset_server, UiFontRole::Caption),
                TextColor(WINDOW_TEXT),
                Node {
                    padding: UiRect::all(Val::Px(8.0)),
                    ..default()
                },
            ));
        });
        return;
    }

    commands.entity(list_root).with_children(|list| {
        for id in ids {
            let Some(item) = sim.state.news.get(id) else {
                continue;
            };
            let date = item.date_label();
            let headline = truncate_headline(&item.headline);
            let mode = display_mode_tag(item.display);
            list.spawn((
                Button,
                NewsHistoryRow { item_id: id },
                Node {
                    width: Val::Percent(100.0),
                    min_height: Val::Px(ROW_HEIGHT),
                    padding: UiRect::horizontal(Val::Px(6.0)),
                    justify_content: JustifyContent::FlexStart,
                    align_items: AlignItems::Center,
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.30, 0.25, 0.17)),
                BorderColor::all(Color::srgb(0.50, 0.44, 0.30)),
                Interaction::default(),
                BuildMenuUi,
                children![(
                    Text::new(format!("{date}  {mode}  {headline}")),
                    window_text_font(&asset_server, UiFontRole::Caption),
                    TextColor(WINDOW_TEXT),
                )],
            ));
        }
    });
}

pub(crate) fn handle_news_history_row_click(
    history: Res<NewsHistoryState>,
    sim: Res<SimWorld>,
    mut news_ui: ResMut<NewsUiState>,
    mut focus: ResMut<crate::camera::CameraFocusRequest>,
    mut selected: ResMut<crate::ui::hud::SelectedTileInfo>,
    mut feedback: ResMut<crate::ui::hud::HudBuildFeedback>,
    rows: Query<(&Interaction, &NewsHistoryRow), (Changed<Interaction>, With<Button>)>,
) {
    if !history.open {
        return;
    }
    for (interaction, row) in &rows {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let Some(item) = sim.state.news.get(row.item_id).cloned() else {
            continue;
        };
        focus_news_reference(item.reference, &sim, &mut focus, &mut selected);
        if item.display == NewsDisplayMode::Full {
            news_ui.shown_full.remove(&item.id);
            news_ui.waiting_full.push_front(item.id);
            feedback.pending_news_chime = true;
        }
    }
}

pub(crate) fn news_history_on_closed(
    mut closed: MessageReader<FloatingWindowClosed>,
    mut history: ResMut<NewsHistoryState>,
) {
    for msg in closed.read() {
        if msg.0 == FloatingWindowId::NewsHistory {
            history.open = false;
        }
    }
}

fn truncate_headline(headline: &str) -> String {
    if headline.chars().count() <= HEADLINE_MAX_CHARS {
        return headline.to_string();
    }
    let mut out: String = headline.chars().take(HEADLINE_MAX_CHARS).collect();
    out.push('…');
    out
}

fn display_mode_tag(display: NewsDisplayMode) -> &'static str {
    match display {
        NewsDisplayMode::Full => "■",
        NewsDisplayMode::Summary => "›",
        NewsDisplayMode::Off => "·",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_headline_adds_ellipsis_when_long() {
        let long = "a".repeat(HEADLINE_MAX_CHARS + 10);
        let out = truncate_headline(&long);
        assert!(out.ends_with('…'));
        assert!(out.chars().count() <= HEADLINE_MAX_CHARS + 1);
    }
}
