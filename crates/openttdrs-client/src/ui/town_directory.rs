//! Directorio global de pueblos — primer consumidor de `list_window`.

use bevy::input::keyboard::KeyboardInput;
use bevy::prelude::*;
use bevy::text::EditableText;

use crate::iso::tile_pos;
use crate::render::{MapPreviewCamera, PrimaryGameCamera};
use crate::state::SimWorld;
use crate::ui::floating_window::{
    FloatingWindow, FloatingWindowClosed, FloatingWindowId, TITLE_CREAM, spawn_floating_window,
};
use crate::ui::list_window::{
    LIST_DEFAULT_HEIGHT, SortDir, apply_list_search_keyboard, clear_list_children,
    spawn_list_empty_label, spawn_list_filter_input, spawn_list_row_button, spawn_list_scroll_area,
    spawn_list_sort_button, sync_list_sort_colors, text_filter_matches,
};
use crate::ui::navigation::{OpenUiRoute, UiRoute};
use crate::ui::toolbar::BuildMenuUi;
use crate::ui::town_window::TownWindowState;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum TownDirectorySort {
    #[default]
    Name,
    Population,
    Rating,
}

#[derive(Resource, Default)]
pub(crate) struct TownDirectoryState {
    pub(crate) open: bool,
    pub(crate) sort: TownDirectorySort,
    pub(crate) sort_dir: SortDir,
    pub(crate) filter_text: String,
    pub(crate) selected: Option<u32>,
}

#[derive(Component)]
pub(crate) struct TownDirectoryListRoot;

#[derive(Component)]
pub(crate) struct TownDirectorySearchInput;

#[derive(Component, Clone, Copy)]
pub(crate) struct TownDirectoryRow {
    town_id: u32,
}

#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TownDirectorySortButton(TownDirectorySort);

#[derive(Default)]
pub(crate) struct TownDirectoryCache {
    sort: TownDirectorySort,
    sort_dir: SortDir,
    filter: String,
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
        spawn_list_filter_input(
            body,
            asset_server,
            TownDirectorySearchInput,
            "buscar pueblo…",
        );
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
            spawn_list_sort_button(
                row,
                asset_server,
                "Nombre",
                TownDirectorySortButton(TownDirectorySort::Name),
                96.0,
            );
            spawn_list_sort_button(
                row,
                asset_server,
                "Población",
                TownDirectorySortButton(TownDirectorySort::Population),
                96.0,
            );
            spawn_list_sort_button(
                row,
                asset_server,
                "Rating",
                TownDirectorySortButton(TownDirectorySort::Rating),
                96.0,
            );
        });
        spawn_list_scroll_area(body, TownDirectoryListRoot, LIST_DEFAULT_HEIGHT);
    });
}

pub(crate) fn open_town_directory_from_routes(
    mut routes: MessageReader<OpenUiRoute>,
    mut state: ResMut<TownDirectoryState>,
) {
    for route in routes.read() {
        if route.0 == UiRoute::Towns {
            state.open = true;
        }
    }
}

pub(crate) fn town_directory_search_keyboard(
    mut key_events: MessageReader<KeyboardInput>,
    mut state: ResMut<TownDirectoryState>,
    mut inputs: Query<(&mut EditableText, &mut Text), With<TownDirectorySearchInput>>,
) {
    if !state.open {
        key_events.clear();
        return;
    }
    let Ok((mut editable, mut text)) = inputs.single_mut() else {
        key_events.clear();
        return;
    };
    apply_list_search_keyboard(
        &mut key_events,
        &mut editable,
        &mut text,
        &mut state.filter_text,
        32,
        "buscar pueblo…",
    );
}

pub(crate) fn handle_town_directory_buttons(
    mut state: ResMut<TownDirectoryState>,
    sort_buttons: Query<
        (&Interaction, &TownDirectorySortButton),
        (Changed<Interaction>, With<Button>),
    >,
    town_rows: Query<(&Interaction, &TownDirectoryRow), (Changed<Interaction>, With<Button>)>,
    mut town_window: ResMut<TownWindowState>,
    sim: Res<SimWorld>,
    mut cam_q: Query<&mut Transform, (With<PrimaryGameCamera>, Without<MapPreviewCamera>)>,
) {
    for (interaction, button) in &sort_buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        if state.sort == button.0 {
            state.sort_dir = state.sort_dir.toggle();
        } else {
            state.sort = button.0;
            state.sort_dir = SortDir::Asc;
        }
    }
    for (interaction, row) in &town_rows {
        if *interaction != Interaction::Pressed {
            continue;
        }
        state.selected = Some(row.town_id);
        town_window.town_id = Some(row.town_id);
        if let Some(town) = sim.state.towns.iter().find(|town| town.id == row.town_id) {
            let height = sim.state.map.get(town.pos).map_or(0, |tile| tile.height);
            let center = tile_pos(town.pos.x, town.pos.y, height, 0.0);
            if let Ok(mut transform) = cam_q.single_mut() {
                transform.translation.x = center.x;
                transform.translation.y = center.y;
            }
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

    sync_list_sort_colors(&mut sort_buttons, TownDirectorySortButton(state.sort));

    let mut rows: Vec<_> = sim
        .state
        .towns
        .iter()
        .filter(|town| text_filter_matches(&state.filter_text, &town.name))
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
            rows.sort_by(|a, b| {
                state
                    .sort_dir
                    .apply(a.1.cmp(&b.1).then_with(|| a.0.cmp(&b.0)))
            });
        }
        TownDirectorySort::Population => {
            rows.sort_by(|a, b| {
                state
                    .sort_dir
                    .apply(a.2.cmp(&b.2).then_with(|| a.1.cmp(&b.1)))
            });
        }
        TownDirectorySort::Rating => {
            rows.sort_by(|a, b| {
                state
                    .sort_dir
                    .apply(a.3.cmp(&b.3).then_with(|| a.1.cmp(&b.1)))
            });
        }
    }
    if cache.sort == state.sort
        && cache.sort_dir == state.sort_dir
        && cache.filter == state.filter_text
        && cache.rows == rows
    {
        return;
    }
    cache.sort = state.sort;
    cache.sort_dir = state.sort_dir;
    cache.filter.clone_from(&state.filter_text);
    cache.rows.clone_from(&rows);

    let Ok(list_root) = list_roots.single() else {
        return;
    };
    clear_list_children(&mut commands, list_root, &children_q);
    let selected = state.selected;
    commands.entity(list_root).with_children(|list| {
        if rows.is_empty() {
            spawn_list_empty_label(
                list,
                &asset_server,
                if state.filter_text.trim().is_empty() {
                    "No hay pueblos."
                } else {
                    "Ningún pueblo coincide con el filtro."
                },
            );
            return;
        }
        for (town_id, name, population, rating) in rows {
            spawn_list_row_button(
                list,
                &asset_server,
                format!("{name}  ·  {population} hab.  ·  autoridad {rating}"),
                TownDirectoryRow { town_id },
                selected == Some(town_id),
            );
        }
    });
}

pub(crate) fn town_directory_on_closed(
    mut closed: MessageReader<FloatingWindowClosed>,
    mut state: ResMut<TownDirectoryState>,
    mut search_q: Query<(&mut EditableText, &mut Text), With<TownDirectorySearchInput>>,
) {
    for message in closed.read() {
        if message.0 == FloatingWindowId::TownDirectory {
            state.open = false;
            state.filter_text.clear();
            state.selected = None;
            if let Ok((mut editable, mut text)) = search_q.single_mut() {
                *editable = EditableText::new("");
                **text = "buscar pueblo…".into();
            }
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
        world.write_message(OpenUiRoute(UiRoute::Towns));
        world
            .run_system_once(open_town_directory_from_routes)
            .unwrap();
        assert!(world.resource::<TownDirectoryState>().open);
    }

    #[test]
    fn sort_toggle_flips_direction() {
        let mut state = TownDirectoryState {
            sort: TownDirectorySort::Name,
            sort_dir: SortDir::Asc,
            ..Default::default()
        };
        if state.sort == TownDirectorySort::Name {
            state.sort_dir = state.sort_dir.toggle();
        }
        assert_eq!(state.sort_dir, SortDir::Desc);
    }

    #[test]
    fn town_row_opens_existing_town_window() {
        let mut world = World::new();
        world.init_resource::<TownDirectoryState>();
        world.init_resource::<TownWindowState>();
        world.insert_resource(SimWorld {
            state: openttdrs_core::GameState::new(8, 8),
            ..SimWorld::default()
        });
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

    #[test]
    fn town_row_centers_camera_on_town() {
        let mut world = World::new();
        world.init_resource::<TownDirectoryState>();
        world.init_resource::<TownWindowState>();
        let mut state = openttdrs_core::GameState::new(16, 16);
        state.towns.push(openttdrs_core::Town {
            id: 3,
            pos: openttdrs_core::TileCoord::new(6, 7),
            name: "Norte".into(),
            population: 200,
            local_authority_rating: 40,
            ..Default::default()
        });
        let height = state
            .map
            .get(openttdrs_core::TileCoord::new(6, 7))
            .map_or(0, |tile| tile.height);
        let expected = tile_pos(6, 7, height, 0.0);
        world.insert_resource(SimWorld {
            state,
            ..SimWorld::default()
        });
        world.spawn((Transform::from_xyz(0.0, 0.0, 0.0), PrimaryGameCamera));
        world.spawn((
            Button,
            TownDirectoryRow { town_id: 3 },
            Interaction::Pressed,
        ));
        world
            .run_system_once(handle_town_directory_buttons)
            .unwrap();
        let cam = world
            .query_filtered::<&Transform, With<PrimaryGameCamera>>()
            .single(&world)
            .unwrap();
        assert!((cam.translation.x - expected.x).abs() < 0.01);
        assert!((cam.translation.y - expected.y).abs() < 0.01);
    }
}
