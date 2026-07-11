//! Directorio global de industrias.

use bevy::prelude::*;
use openttdrs_core::{IndustryKind, TileCoord};

use crate::state::SimWorld;
use crate::ui::floating_window::{
    FloatingWindow, FloatingWindowClosed, FloatingWindowId, TITLE_BROWN, WINDOW_TEXT,
    spawn_floating_window, window_text_font,
};
use crate::ui::font::UiFontRole;
use crate::ui::industry_panel::{IndustryPanelState, kind_label};
use crate::ui::navigation::{OpenUiRoute, UiRoute};
use crate::ui::toolbar::BuildMenuUi;

const LIST_HEIGHT: f32 = 330.0;
const BTN_BG: Color = Color::srgb(0.36, 0.31, 0.21);
const BTN_HOVER: Color = Color::srgb(0.47, 0.41, 0.28);
const BTN_ACTIVE: Color = Color::srgb(0.58, 0.50, 0.31);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum IndustryDirectorySort {
    #[default]
    Type,
    Stock,
}

#[derive(Resource, Default)]
pub(crate) struct IndustryDirectoryState {
    pub(crate) open: bool,
    pub(crate) sort: IndustryDirectorySort,
}

#[derive(Component)]
pub(crate) struct IndustryDirectoryListRoot;

#[derive(Component, Clone, Copy)]
pub(crate) struct IndustryDirectoryRow {
    pos: TileCoord,
}

#[derive(Component, Clone, Copy)]
pub(crate) struct IndustryDirectorySortButton(IndustryDirectorySort);

#[derive(Default)]
pub(crate) struct IndustryDirectoryCache {
    sort: IndustryDirectorySort,
    rows: Vec<(TileCoord, IndustryKind, u32, u32)>,
}

pub(crate) fn setup_industry_directory(mut commands: Commands, asset_server: Res<AssetServer>) {
    let asset_server = &*asset_server;
    let (_root, content) = spawn_floating_window(
        &mut commands,
        asset_server,
        FloatingWindowId::IndustryDirectory,
        "Directorio de industrias",
        TITLE_BROWN,
        Vec2::new(455.0, 115.0),
        410.0,
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
            spawn_sort_button(row, asset_server, "Tipo", IndustryDirectorySort::Type);
            spawn_sort_button(row, asset_server, "Stock", IndustryDirectorySort::Stock);
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
                IndustryDirectoryListRoot,
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
    sort: IndustryDirectorySort,
) {
    parent.spawn((
        Button,
        IndustryDirectorySortButton(sort),
        Node {
            min_width: Val::Px(90.0),
            height: Val::Px(24.0),
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

pub(crate) fn open_industry_directory_from_routes(
    mut routes: MessageReader<OpenUiRoute>,
    mut state: ResMut<IndustryDirectoryState>,
) {
    for route in routes.read() {
        if route.0 == UiRoute::Industries {
            state.open = true;
        }
    }
}

pub(crate) fn handle_industry_directory_buttons(
    mut state: ResMut<IndustryDirectoryState>,
    sort_buttons: Query<
        (&Interaction, &IndustryDirectorySortButton),
        (Changed<Interaction>, With<Button>),
    >,
    rows: Query<(&Interaction, &IndustryDirectoryRow), (Changed<Interaction>, With<Button>)>,
    mut panel: ResMut<IndustryPanelState>,
) {
    for (interaction, button) in &sort_buttons {
        if *interaction == Interaction::Pressed {
            state.sort = button.0;
        }
    }
    for (interaction, row) in &rows {
        if *interaction == Interaction::Pressed {
            panel.open = true;
            panel.focus_tile = Some(row.pos);
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn sync_industry_directory(
    state: Res<IndustryDirectoryState>,
    sim: Res<SimWorld>,
    mut root_q: Query<(&FloatingWindow, &mut Visibility)>,
    list_roots: Query<Entity, With<IndustryDirectoryListRoot>>,
    children_q: Query<&Children>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut cache: Local<IndustryDirectoryCache>,
    mut sort_buttons: Query<
        (
            &IndustryDirectorySortButton,
            &Interaction,
            &mut BackgroundColor,
        ),
        With<Button>,
    >,
) {
    let Some((_, mut visibility)) = root_q
        .iter_mut()
        .find(|(window, _)| window.id == FloatingWindowId::IndustryDirectory)
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
        .industries
        .iter()
        .map(|industry| {
            (
                industry.pos,
                industry.kind,
                industry.stock,
                industry.capacity,
            )
        })
        .collect();
    match state.sort {
        IndustryDirectorySort::Type => rows.sort_by(|a, b| {
            kind_label(a.1)
                .cmp(kind_label(b.1))
                .then_with(|| a.0.x.cmp(&b.0.x))
                .then_with(|| a.0.y.cmp(&b.0.y))
        }),
        IndustryDirectorySort::Stock => {
            rows.sort_by(|a, b| {
                b.2.cmp(&a.2)
                    .then_with(|| kind_label(a.1).cmp(kind_label(b.1)))
            });
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
                Text::new("No hay industrias."),
                window_text_font(&asset_server, UiFontRole::Caption),
                TextColor(WINDOW_TEXT),
                Node {
                    padding: UiRect::all(Val::Px(8.0)),
                    ..default()
                },
            ));
            return;
        }
        for (pos, kind, stock, capacity) in rows {
            list.spawn((
                Button,
                IndustryDirectoryRow { pos },
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
                        "{}  ·  stock {stock}/{capacity}  ·  ({}, {})",
                        kind_label(kind),
                        pos.x,
                        pos.y
                    )),
                    window_text_font(&asset_server, UiFontRole::Caption),
                    TextColor(WINDOW_TEXT),
                )],
            ));
        }
    });
}

pub(crate) fn industry_directory_on_closed(
    mut closed: MessageReader<FloatingWindowClosed>,
    mut state: ResMut<IndustryDirectoryState>,
) {
    for message in closed.read() {
        if message.0 == FloatingWindowId::IndustryDirectory {
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
    fn route_opens_industry_directory() {
        let mut world = World::new();
        world.init_resource::<IndustryDirectoryState>();
        world.init_resource::<Messages<OpenUiRoute>>();
        world.write_message(OpenUiRoute(UiRoute::Industries));
        world
            .run_system_once(open_industry_directory_from_routes)
            .unwrap();
        assert!(world.resource::<IndustryDirectoryState>().open);
    }

    #[test]
    fn row_opens_industry_panel() {
        let mut world = World::new();
        world.init_resource::<IndustryDirectoryState>();
        world.init_resource::<IndustryPanelState>();
        let pos = TileCoord::new(3, 4);
        world.spawn((Button, IndustryDirectoryRow { pos }, Interaction::Pressed));
        world
            .run_system_once(handle_industry_directory_buttons)
            .unwrap();
        let panel = world.resource::<IndustryPanelState>();
        assert!(panel.open);
        assert_eq!(panel.focus_tile, Some(pos));
    }
}
