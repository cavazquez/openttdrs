//! Directorio global de pueblos, primer consumidor de `UiRoute`/menú reusable.

use bevy::prelude::*;

use crate::state::SimWorld;
use crate::ui::floating_window::{
    FloatingWindow, FloatingWindowClosed, FloatingWindowId, TITLE_CREAM, WINDOW_TEXT,
    spawn_floating_window, window_text_font,
};
use crate::ui::font::UiFontRole;
use crate::ui::navigation::{OpenUiRoute, UiRoute};
use crate::ui::toolbar::BuildMenuUi;
use crate::ui::town_window::TownWindowState;

const LIST_HEIGHT: f32 = 330.0;
const BTN_BG: Color = Color::srgb(0.36, 0.31, 0.21);
const BTN_HOVER: Color = Color::srgb(0.47, 0.41, 0.28);
const BTN_ACTIVE: Color = Color::srgb(0.58, 0.50, 0.31);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum TownDirectorySort {
    #[default]
    Name,
    Population,
}

#[derive(Resource, Default)]
pub(crate) struct TownDirectoryState {
    pub(crate) open: bool,
    pub(crate) sort: TownDirectorySort,
}

#[derive(Component)]
pub(crate) struct TownDirectoryListRoot;

#[derive(Component, Clone, Copy)]
pub(crate) struct TownDirectoryRow {
    town_id: u32,
}

#[derive(Component, Clone, Copy)]
pub(crate) struct TownDirectorySortButton(TownDirectorySort);

#[derive(Default)]
pub(crate) struct TownDirectoryCache {
    sort: TownDirectorySort,
    rows: Vec<(u32, String, u32, i16)>,
}

pub(crate) fn setup_town_directory(mut commands: Commands, asset_server: Res<AssetServer>) {
    let asset_server = &*asset_server;
    let (_root, content) = spawn_floating_window(
        &mut commands,
        asset_server,
        FloatingWindowId::TownDirectory,
        "Directorio de pueblos",
        TITLE_CREAM,
        Vec2::new(420.0, 90.0),
        390.0,
    );
    commands.entity(content).with_children(|body| {
        body.spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(4.0),
                margin: UiRect::bottom(Val::Px(5.0)),
                ..default()
            },
            BuildMenuUi,
        ))
        .with_children(|row| {
            spawn_sort_button(row, asset_server, "Nombre", TownDirectorySort::Name);
            spawn_sort_button(
                row,
                asset_server,
                "Población",
                TownDirectorySort::Population,
            );
        });
        body.spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(LIST_HEIGHT),
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
                TownDirectoryListRoot,
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

fn spawn_sort_button(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    label: &str,
    sort: TownDirectorySort,
) {
    parent.spawn((
        Button,
        TownDirectorySortButton(sort),
        Node {
            min_width: Val::Px(96.0),
            height: Val::Px(24.0),
            padding: UiRect::horizontal(Val::Px(6.0)),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        BackgroundColor(BTN_BG),
        BorderColor::all(Color::srgb(0.58, 0.50, 0.33)),
        Interaction::default(),
        BuildMenuUi,
        children![(
            Text::new(label),
            window_text_font(asset_server, UiFontRole::Caption),
            TextColor(WINDOW_TEXT),
        )],
    ));
}

pub(crate) fn open_town_directory_from_routes(
    mut routes: MessageReader<OpenUiRoute>,
    mut state: ResMut<TownDirectoryState>,
) {
    for route in routes.read() {
        if route.0 == UiRoute::TownDirectory {
            state.open = true;
        }
    }
}

pub(crate) fn handle_town_directory_buttons(
    mut state: ResMut<TownDirectoryState>,
    sort_buttons: Query<
        (&Interaction, &TownDirectorySortButton),
        (Changed<Interaction>, With<Button>),
    >,
    town_rows: Query<(&Interaction, &TownDirectoryRow), (Changed<Interaction>, With<Button>)>,
    mut town_window: ResMut<TownWindowState>,
) {
    for (interaction, button) in &sort_buttons {
        if *interaction == Interaction::Pressed {
            state.sort = button.0;
        }
    }
    for (interaction, row) in &town_rows {
        if *interaction == Interaction::Pressed {
            town_window.town_id = Some(row.town_id);
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn sync_town_directory(
    state: Res<TownDirectoryState>,
    sim: Res<SimWorld>,
    mut root_q: Query<(&FloatingWindow, &mut Visibility)>,
    list_roots: Query<Entity, With<TownDirectoryListRoot>>,
    children_q: Query<&Children>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut cache: Local<TownDirectoryCache>,
    mut sort_buttons: Query<
        (&TownDirectorySortButton, &Interaction, &mut BackgroundColor),
        With<Button>,
    >,
) {
    let Some((_, mut visibility)) = root_q
        .iter_mut()
        .find(|(window, _)| window.id == FloatingWindowId::TownDirectory)
    else {
        return;
    };
    if !state.open {
        *visibility = Visibility::Hidden;
        cache.rows.clear();
        return;
    }
    *visibility = Visibility::Visible;

    for (button, interaction, mut bg) in &mut sort_buttons {
        *bg = if button.0 == state.sort {
            BackgroundColor(BTN_ACTIVE)
        } else if *interaction == Interaction::Hovered {
            BackgroundColor(BTN_HOVER)
        } else {
            BackgroundColor(BTN_BG)
        };
    }

    let mut rows: Vec<_> = sim
        .state
        .towns
        .iter()
        .map(|town| {
            (
                town.id,
                town.name.clone(),
                town.population,
                town.local_authority_rating,
            )
        })
        .collect();
    match state.sort {
        TownDirectorySort::Name => {
            rows.sort_by(|a, b| a.1.cmp(&b.1).then_with(|| a.0.cmp(&b.0)));
        }
        TownDirectorySort::Population => {
            rows.sort_by(|a, b| b.2.cmp(&a.2).then_with(|| a.1.cmp(&b.1)));
        }
    }
    if cache.sort == state.sort && cache.rows == rows {
        return;
    }
    cache.sort = state.sort;
    cache.rows.clone_from(&rows);

    let Ok(list_root) = list_roots.single() else {
        return;
    };
    if let Ok(children) = children_q.get(list_root) {
        for child in children.iter() {
            commands.entity(child).despawn();
        }
    }
    commands.entity(list_root).with_children(|list| {
        if rows.is_empty() {
            list.spawn((
                Text::new("No hay pueblos."),
                window_text_font(&asset_server, UiFontRole::Caption),
                TextColor(WINDOW_TEXT),
                Node {
                    padding: UiRect::all(Val::Px(8.0)),
                    ..default()
                },
            ));
            return;
        }
        for (town_id, name, population, rating) in rows {
            list.spawn((
                Button,
                TownDirectoryRow { town_id },
                Node {
                    width: Val::Percent(100.0),
                    min_height: Val::Px(28.0),
                    padding: UiRect::horizontal(Val::Px(7.0)),
                    justify_content: JustifyContent::FlexStart,
                    align_items: AlignItems::Center,
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(BTN_BG),
                BorderColor::all(Color::srgb(0.50, 0.44, 0.30)),
                Interaction::default(),
                BuildMenuUi,
                children![(
                    Text::new(format!(
                        "{name}  ·  {population} hab.  ·  autoridad {rating}"
                    )),
                    window_text_font(&asset_server, UiFontRole::Caption),
                    TextColor(WINDOW_TEXT),
                )],
            ));
        }
    });
}

pub(crate) fn town_directory_on_closed(
    mut closed: MessageReader<FloatingWindowClosed>,
    mut state: ResMut<TownDirectoryState>,
) {
    for message in closed.read() {
        if message.0 == FloatingWindowId::TownDirectory {
            state.open = false;
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    #[test]
    fn route_opens_town_directory() {
        let mut world = World::new();
        world.init_resource::<TownDirectoryState>();
        world.init_resource::<Messages<OpenUiRoute>>();
        world.write_message(OpenUiRoute(UiRoute::TownDirectory));
        world
            .run_system_once(open_town_directory_from_routes)
            .unwrap();
        assert!(world.resource::<TownDirectoryState>().open);
    }

    #[test]
    fn town_row_opens_existing_town_window() {
        let mut world = World::new();
        world.init_resource::<TownDirectoryState>();
        world.init_resource::<TownWindowState>();
        world.spawn((
            Button,
            TownDirectoryRow { town_id: 7 },
            Interaction::Pressed,
        ));
        world
            .run_system_once(handle_town_directory_buttons)
            .unwrap();
        assert_eq!(world.resource::<TownWindowState>().town_id, Some(7));
    }
}
