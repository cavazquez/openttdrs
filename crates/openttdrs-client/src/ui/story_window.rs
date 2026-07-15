//! Libro de historia GameScript-lite (#43).

use bevy::prelude::*;

use crate::state::SimWorld;
use crate::ui::floating_window::{
    FloatingWindow, FloatingWindowClosed, FloatingWindowId, TITLE_BROWN, WINDOW_TEXT,
    spawn_floating_window, window_text_font,
};
use crate::ui::font::UiFontRole;
use crate::ui::navigation::{OpenUiRoute, UiRoute};
use crate::ui::toolbar::BuildMenuUi;

const BTN_BG: Color = Color::srgb(0.36, 0.31, 0.21);
const BTN_BORDER: Color = Color::srgb(0.66, 0.58, 0.38);

#[derive(Resource, Default)]
pub(crate) struct StoryWindowState {
    pub(crate) open: bool,
}

#[derive(Component)]
pub(crate) struct StoryTitleText;

#[derive(Component)]
pub(crate) struct StoryBodyText;

#[derive(Component)]
pub(crate) struct StoryPageLabel;

#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StoryNavAction {
    Prev,
    Next,
}

pub(crate) fn setup_story_window(mut commands: Commands, asset_server: Res<AssetServer>) {
    let asset_server = &*asset_server;
    let (_root, content) = spawn_floating_window(
        &mut commands,
        asset_server,
        FloatingWindowId::Story,
        "Historia",
        TITLE_BROWN,
        Vec2::new(380.0, 90.0),
        440.0,
    );
    commands.entity(content).with_children(|body| {
        body.spawn((
            StoryTitleText,
            Text::new("—"),
            window_text_font(asset_server, UiFontRole::Caption),
            TextColor(WINDOW_TEXT),
            Node {
                width: Val::Percent(100.0),
                margin: UiRect::bottom(Val::Px(6.0)),
                ..default()
            },
            BuildMenuUi,
        ));
        body.spawn((
            StoryBodyText,
            Text::new("Sin páginas de historia."),
            window_text_font(asset_server, UiFontRole::Caption),
            TextColor(Color::srgb(0.88, 0.84, 0.72)),
            Node {
                width: Val::Percent(100.0),
                min_height: Val::Px(120.0),
                margin: UiRect::bottom(Val::Px(8.0)),
                ..default()
            },
            BuildMenuUi,
        ));
        body.spawn((
            StoryPageLabel,
            Text::new("0 / 0"),
            window_text_font(asset_server, UiFontRole::Caption),
            TextColor(WINDOW_TEXT),
            Node {
                margin: UiRect::bottom(Val::Px(6.0)),
                ..default()
            },
            BuildMenuUi,
        ));
        body.spawn(Node {
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(6.0),
            ..default()
        })
        .with_children(|row| {
            spawn_nav(row, asset_server, StoryNavAction::Prev, "Anterior");
            spawn_nav(row, asset_server, StoryNavAction::Next, "Siguiente");
        });
    });
}

fn spawn_nav(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    action: StoryNavAction,
    label: &'static str,
) {
    parent
        .spawn((
            Button,
            action,
            Node {
                min_width: Val::Px(96.0),
                height: Val::Px(26.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border: UiRect::all(Val::Px(1.0)),
                padding: UiRect::horizontal(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(BTN_BG),
            BorderColor::all(BTN_BORDER),
            Interaction::default(),
            BuildMenuUi,
        ))
        .with_children(|btn| {
            btn.spawn((
                Text::new(label),
                window_text_font(asset_server, UiFontRole::Caption),
                TextColor(WINDOW_TEXT),
            ));
        });
}

pub(crate) fn open_story_from_routes(
    mut routes: MessageReader<OpenUiRoute>,
    mut state: ResMut<StoryWindowState>,
) {
    for route in routes.read() {
        if route.0 == UiRoute::Story {
            state.open = true;
        }
    }
}

pub(crate) fn handle_story_nav_buttons(
    state: Res<StoryWindowState>,
    mut sim: Option<ResMut<SimWorld>>,
    buttons: Query<
        (&Interaction, &StoryNavAction),
        (Changed<Interaction>, With<Button>, With<StoryNavAction>),
    >,
) {
    if !state.open {
        return;
    }
    let Some(sim) = sim.as_deref_mut() else {
        return;
    };
    if !sim.state.gs.enabled || sim.state.gs.story_pages.is_empty() {
        return;
    }
    let n = sim.state.gs.story_pages.len();
    for (interaction, action) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match *action {
            StoryNavAction::Prev => {
                sim.state.gs.story_index = sim.state.gs.story_index.saturating_sub(1);
            }
            StoryNavAction::Next => {
                if sim.state.gs.story_index + 1 < n {
                    sim.state.gs.story_index += 1;
                }
            }
        }
    }
}

pub(crate) fn sync_story_window(
    state: Res<StoryWindowState>,
    sim: Option<Res<SimWorld>>,
    mut windows: Query<(&FloatingWindow, &mut Visibility)>,
    mut title_q: Query<
        &mut Text,
        (
            With<StoryTitleText>,
            Without<StoryBodyText>,
            Without<StoryPageLabel>,
        ),
    >,
    mut body_q: Query<
        &mut Text,
        (
            With<StoryBodyText>,
            Without<StoryTitleText>,
            Without<StoryPageLabel>,
        ),
    >,
    mut page_q: Query<
        &mut Text,
        (
            With<StoryPageLabel>,
            Without<StoryTitleText>,
            Without<StoryBodyText>,
        ),
    >,
) {
    for (w, mut vis) in &mut windows {
        if w.id == FloatingWindowId::Story {
            *vis = if state.open {
                Visibility::Visible
            } else {
                Visibility::Hidden
            };
        }
    }
    if !state.open {
        return;
    }
    let Some(sim) = sim.as_deref() else {
        return;
    };
    let gs = &sim.state.gs;
    let (title, body, page) = if !gs.enabled || gs.story_pages.is_empty() {
        (
            "Sin historia".into(),
            "Este escenario no tiene páginas Story (GS demo desactivado).".into(),
            "0 / 0".into(),
        )
    } else {
        let idx = gs.story_index.min(gs.story_pages.len() - 1);
        let page = &gs.story_pages[idx];
        (
            page.title.clone(),
            page.body.clone(),
            format!("{} / {}", idx + 1, gs.story_pages.len()),
        )
    };
    for mut text in &mut title_q {
        **text = title.clone();
    }
    for mut text in &mut body_q {
        **text = body.clone();
    }
    for mut text in &mut page_q {
        **text = page.clone();
    }
}

pub(crate) fn story_window_on_closed(
    mut closed: MessageReader<FloatingWindowClosed>,
    mut state: ResMut<StoryWindowState>,
) {
    for msg in closed.read() {
        if msg.0 == FloatingWindowId::Story {
            state.open = false;
        }
    }
}
