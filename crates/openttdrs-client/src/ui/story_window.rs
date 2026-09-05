//! Libro de historia GameScript-lite (#43).

use bevy::prelude::*;

use crate::i18n::localized_text;
use crate::settings::ClientPreferences;
use crate::state::SimWorld;
use crate::ui::floating_window::{
    FloatingWindow, FloatingWindowClosed, FloatingWindowId, FloatingWindowTitleText, TITLE_BROWN,
    WINDOW_TEXT, spawn_floating_window, window_text_font,
};
use crate::ui::font::UiFontRole;
use crate::ui::navigation::{OpenUiRoute, UiRoute};
use crate::ui::toolbar::BuildMenuUi;

const BTN_BG: Color = Color::srgb(0.36, 0.31, 0.21);
const BTN_BORDER: Color = Color::srgb(0.66, 0.58, 0.38);

#[derive(Resource, Default)]
pub(crate) struct StoryWindowState {
    pub(crate) open: bool,
    /// Índice de página local (no muta `GameState.gs`; cada cliente navega solo).
    pub(crate) page_index: usize,
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

/// Etiqueta traducible de una acción de navegación; la acción propiamente dicha
/// queda separada para que cambiar el locale no afecte la navegación local.
#[derive(Component, Clone, Copy)]
pub(crate) struct StoryNavText(pub(crate) StoryNavAction);

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
                StoryNavText(action),
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
    mut state: ResMut<StoryWindowState>,
    sim: Option<Res<SimWorld>>,
    buttons: Query<
        (&Interaction, &StoryNavAction),
        (Changed<Interaction>, With<Button>, With<StoryNavAction>),
    >,
) {
    if !state.open {
        return;
    }
    let Some(sim) = sim.as_deref() else {
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
                state.page_index = state.page_index.saturating_sub(1);
            }
            StoryNavAction::Next => {
                if state.page_index + 1 < n {
                    state.page_index += 1;
                }
            }
        }
    }
}

pub(crate) fn sync_story_window(
    state: Res<StoryWindowState>,
    sim: Option<Res<SimWorld>>,
    prefs: Res<ClientPreferences>,
    mut windows: Query<(&FloatingWindow, &mut Visibility)>,
    mut title_q: Query<
        &mut Text,
        (
            With<StoryTitleText>,
            Without<StoryBodyText>,
            Without<StoryPageLabel>,
            Without<StoryNavText>,
            Without<FloatingWindowTitleText>,
        ),
    >,
    mut body_q: Query<
        &mut Text,
        (
            With<StoryBodyText>,
            Without<StoryTitleText>,
            Without<StoryPageLabel>,
            Without<StoryNavText>,
            Without<FloatingWindowTitleText>,
        ),
    >,
    mut page_q: Query<
        &mut Text,
        (
            With<StoryPageLabel>,
            Without<StoryTitleText>,
            Without<StoryBodyText>,
            Without<StoryNavText>,
            Without<FloatingWindowTitleText>,
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
            localized_text(prefs.locale(), "Sin historia"),
            localized_text(
                prefs.locale(),
                "Este escenario no tiene páginas Story (GS demo desactivado).",
            ),
            "0 / 0".into(),
        )
    } else {
        let idx = state.page_index.min(gs.story_pages.len() - 1);
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

/// Sincroniza el chrome propio de Story. Se mantiene separado de la página
/// dinámica para no convertir el sistema de contenido en una query ECS amplia.
pub(crate) fn sync_story_window_chrome(
    prefs: Res<ClientPreferences>,
    mut window_title_q: Query<
        (&FloatingWindowTitleText, &mut Text),
        (
            Without<StoryTitleText>,
            Without<StoryBodyText>,
            Without<StoryPageLabel>,
            Without<StoryNavText>,
        ),
    >,
    mut nav_q: Query<
        (&StoryNavText, &mut Text),
        (
            Without<StoryTitleText>,
            Without<StoryBodyText>,
            Without<StoryPageLabel>,
            Without<FloatingWindowTitleText>,
        ),
    >,
) {
    for (window_title, mut text) in &mut window_title_q {
        if window_title.0 == FloatingWindowId::Story {
            **text = localized_text(prefs.locale(), "Historia");
        }
    }
    for (nav, mut text) in &mut nav_q {
        let source = match nav.0 {
            StoryNavAction::Prev => "Anterior",
            StoryNavAction::Next => "Siguiente",
        };
        **text = localized_text(prefs.locale(), source);
    }
}

pub(crate) fn story_window_on_closed(
    mut closed: MessageReader<FloatingWindowClosed>,
    mut state: ResMut<StoryWindowState>,
) {
    for msg in closed.read() {
        if msg.0.class == FloatingWindowId::Story {
            state.open = false;
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use bevy::prelude::*;

    use super::{
        StoryBodyText, StoryNavAction, StoryNavText, StoryPageLabel, StoryTitleText,
        StoryWindowState, sync_story_window, sync_story_window_chrome,
    };
    use crate::settings::ClientPreferences;
    use crate::state::SimWorld;
    use crate::ui::floating_window::{FloatingWindowId, FloatingWindowTitleText};
    use openttdrs_core::gs::GsStoryPage;

    #[test]
    fn story_fallback_follows_the_live_locale_without_touching_game_script_pages() {
        let mut world = World::new();
        world.insert_resource(StoryWindowState {
            open: true,
            ..StoryWindowState::default()
        });
        world.insert_resource(SimWorld::default());
        {
            let mut sim = world.resource_mut::<SimWorld>();
            sim.state.gs.enabled = false;
            sim.state.gs.story_pages.clear();
        }
        world.insert_resource(ClientPreferences {
            language: "en".into(),
            ..ClientPreferences::default()
        });
        let title = world.spawn((StoryTitleText, Text::new("—"))).id();
        let body = world.spawn((StoryBodyText, Text::new("—"))).id();
        world.spawn((StoryPageLabel, Text::new("—")));
        let window_title = world
            .spawn((
                FloatingWindowTitleText(FloatingWindowId::Story),
                Text::new("—"),
            ))
            .id();
        let previous = world
            .spawn((StoryNavText(StoryNavAction::Prev), Text::new("—")))
            .id();
        let next = world
            .spawn((StoryNavText(StoryNavAction::Next), Text::new("—")))
            .id();

        let mut schedule = Schedule::default();
        schedule.add_systems((sync_story_window, sync_story_window_chrome));
        schedule.run(&mut world);
        assert_eq!(
            world.entity(title).get::<Text>().unwrap().as_str(),
            "No story"
        );
        assert_eq!(
            world.entity(body).get::<Text>().unwrap().as_str(),
            "This scenario has no Story pages (GS demo disabled)."
        );
        assert_eq!(
            world.entity(window_title).get::<Text>().unwrap().as_str(),
            "Story"
        );
        assert_eq!(
            world.entity(previous).get::<Text>().unwrap().as_str(),
            "Previous"
        );
        assert_eq!(world.entity(next).get::<Text>().unwrap().as_str(), "Next");

        world.resource_mut::<ClientPreferences>().language = "es-AR".into();
        schedule.run(&mut world);
        assert_eq!(
            world.entity(title).get::<Text>().unwrap().as_str(),
            "Sin historia"
        );
        assert_eq!(
            world.entity(body).get::<Text>().unwrap().as_str(),
            "Este escenario no tiene páginas Story (GS demo desactivado)."
        );
        assert_eq!(
            world.entity(window_title).get::<Text>().unwrap().as_str(),
            "Historia"
        );
        assert_eq!(
            world.entity(previous).get::<Text>().unwrap().as_str(),
            "Anterior"
        );
        assert_eq!(
            world.entity(next).get::<Text>().unwrap().as_str(),
            "Siguiente"
        );

        {
            let mut sim = world.resource_mut::<SimWorld>();
            sim.state.gs.enabled = true;
            sim.state.gs.story_pages.push(GsStoryPage {
                id: 42,
                title: "Título del GameScript".into(),
                body: "Cuerpo que debe conservar el escenario".into(),
            });
        }
        world.resource_mut::<ClientPreferences>().language = "en".into();
        schedule.run(&mut world);
        assert_eq!(
            world.entity(title).get::<Text>().unwrap().as_str(),
            "Título del GameScript"
        );
        assert_eq!(
            world.entity(body).get::<Text>().unwrap().as_str(),
            "Cuerpo que debe conservar el escenario"
        );
    }
}
