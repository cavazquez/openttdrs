//! Lista global de estaciones y waypoints.

use bevy::prelude::*;
use openttdrs_core::{StopKind, TileCoord};

use crate::state::SimWorld;
use crate::ui::floating_window::{
    FloatingWindow, FloatingWindowClosed, FloatingWindowId, TITLE_BROWN, WINDOW_TEXT,
    spawn_floating_window, window_text_font,
};
use crate::ui::font::UiFontRole;
use crate::ui::navigation::{OpenUiRoute, UiRoute};
use crate::ui::toolbar::{BuildMenuUi, StationCargoPanelState};

const LIST_HEIGHT: f32 = 330.0;
const BTN_BG: Color = Color::srgb(0.36, 0.31, 0.21);
const BTN_HOVER: Color = Color::srgb(0.47, 0.41, 0.28);
const BTN_ACTIVE: Color = Color::srgb(0.58, 0.50, 0.31);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum StationDirectorySort {
    #[default]
    Name,
    Rating,
    Waiting,
}

#[derive(Resource, Default)]
pub(crate) struct StationDirectoryState {
    pub(crate) open: bool,
    pub(crate) sort: StationDirectorySort,
}

#[derive(Component)]
pub(crate) struct StationDirectoryListRoot;

#[derive(Component, Clone, Copy)]
pub(crate) struct StationDirectoryRow {
    pos: TileCoord,
}

#[derive(Component, Clone, Copy)]
pub(crate) struct StationDirectorySortButton(StationDirectorySort);

#[derive(Default)]
pub(crate) struct StationDirectoryCache {
    sort: StationDirectorySort,
    rows: Vec<(TileCoord, String, StopKind, u8, u32)>,
}

pub(crate) fn setup_station_directory(mut commands: Commands, asset_server: Res<AssetServer>) {
    let asset_server = &*asset_server;
    let (_root, content) = spawn_floating_window(
        &mut commands,
        asset_server,
        FloatingWindowId::StationDirectory,
        "Lista de estaciones",
        TITLE_BROWN,
        Vec2::new(490.0, 140.0),
        460.0,
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
            spawn_sort_button(row, asset_server, "Nombre", StationDirectorySort::Name);
            spawn_sort_button(row, asset_server, "Rating", StationDirectorySort::Rating);
            spawn_sort_button(
                row,
                asset_server,
                "En espera",
                StationDirectorySort::Waiting,
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
                StationDirectoryListRoot,
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
    sort: StationDirectorySort,
) {
    parent.spawn((
        Button,
        StationDirectorySortButton(sort),
        Node {
            min_width: Val::Px(94.0),
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

pub(crate) fn open_station_directory_from_routes(
    mut routes: MessageReader<OpenUiRoute>,
    mut state: ResMut<StationDirectoryState>,
) {
    for route in routes.read() {
        if route.0 == UiRoute::Stations {
            state.open = true;
        }
    }
}

pub(crate) fn handle_station_directory_buttons(
    mut state: ResMut<StationDirectoryState>,
    sort_buttons: Query<
        (&Interaction, &StationDirectorySortButton),
        (Changed<Interaction>, With<Button>),
    >,
    rows: Query<(&Interaction, &StationDirectoryRow), (Changed<Interaction>, With<Button>)>,
    mut station_panel: ResMut<StationCargoPanelState>,
) {
    for (interaction, button) in &sort_buttons {
        if *interaction == Interaction::Pressed {
            state.sort = button.0;
        }
    }
    for (interaction, row) in &rows {
        if *interaction == Interaction::Pressed {
            station_panel.station_pos = Some(row.pos);
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn sync_station_directory(
    state: Res<StationDirectoryState>,
    sim: Res<SimWorld>,
    mut root_q: Query<(&FloatingWindow, &mut Visibility)>,
    list_roots: Query<Entity, With<StationDirectoryListRoot>>,
    children_q: Query<&Children>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut cache: Local<StationDirectoryCache>,
    mut sort_buttons: Query<
        (
            &StationDirectorySortButton,
            &Interaction,
            &mut BackgroundColor,
        ),
        With<Button>,
    >,
) {
    let Some((_, mut visibility)) = root_q
        .iter_mut()
        .find(|(window, _)| window.id == FloatingWindowId::StationDirectory)
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
        .stations
        .iter()
        .map(|station| {
            let name = station
                .name
                .clone()
                .filter(|name| !name.is_empty())
                .unwrap_or_else(|| {
                    format!(
                        "{} ({}, {})",
                        station_kind_label(station.stop_kind),
                        station.pos.x,
                        station.pos.y
                    )
                });
            let stock_total = station
                .cargo_stock
                .passengers
                .saturating_add(station.cargo_stock.mail)
                .saturating_add(station.cargo_stock.goods)
                .saturating_add(station.cargo_stock.coal)
                .saturating_add(station.cargo_stock.wood)
                .saturating_add(station.cargo_stock.oil);
            let waiting = station.stock.max(stock_total).max(
                station
                    .cargo_packets
                    .packets
                    .iter()
                    .map(|packet| u32::from(packet.count))
                    .fold(0, u32::saturating_add),
            );
            (
                station.pos,
                name,
                station.stop_kind,
                station.rating,
                waiting,
            )
        })
        .collect();
    match state.sort {
        StationDirectorySort::Name => {
            rows.sort_by(|a, b| a.1.cmp(&b.1).then_with(|| a.0.x.cmp(&b.0.x)));
        }
        StationDirectorySort::Rating => {
            rows.sort_by(|a, b| b.3.cmp(&a.3).then_with(|| a.1.cmp(&b.1)));
        }
        StationDirectorySort::Waiting => {
            rows.sort_by(|a, b| b.4.cmp(&a.4).then_with(|| a.1.cmp(&b.1)));
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
                Text::new("No hay estaciones."),
                window_text_font(&asset_server, UiFontRole::Caption),
                TextColor(WINDOW_TEXT),
                Node {
                    padding: UiRect::all(Val::Px(8.0)),
                    ..default()
                },
            ));
            return;
        }
        for (pos, name, kind, rating, waiting) in rows {
            list.spawn((
                Button,
                StationDirectoryRow { pos },
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
                        "{name}  ·  {}  ·  rating {rating}  ·  espera {waiting}",
                        station_kind_label(kind)
                    )),
                    window_text_font(&asset_server, UiFontRole::Caption),
                    TextColor(WINDOW_TEXT),
                )],
            ));
        }
    });
}

fn station_kind_label(kind: StopKind) -> &'static str {
    match kind {
        StopKind::BusStop => "Bus",
        StopKind::TruckStop => "Camión",
        StopKind::RailStation => "Tren",
        StopKind::Dock => "Muelle",
        StopKind::Airport => "Aeropuerto",
        StopKind::RailWaypoint => "Waypoint",
    }
}

pub(crate) fn station_directory_on_closed(
    mut closed: MessageReader<FloatingWindowClosed>,
    mut state: ResMut<StationDirectoryState>,
) {
    for message in closed.read() {
        if message.0 == FloatingWindowId::StationDirectory {
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
    fn route_opens_station_directory() {
        let mut world = World::new();
        world.init_resource::<StationDirectoryState>();
        world.init_resource::<Messages<OpenUiRoute>>();
        world.write_message(OpenUiRoute(UiRoute::Stations));
        world
            .run_system_once(open_station_directory_from_routes)
            .unwrap();
        assert!(world.resource::<StationDirectoryState>().open);
    }

    #[test]
    fn row_opens_station_panel() {
        let mut world = World::new();
        world.init_resource::<StationDirectoryState>();
        world.init_resource::<StationCargoPanelState>();
        let pos = TileCoord::new(6, 7);
        world.spawn((Button, StationDirectoryRow { pos }, Interaction::Pressed));
        world
            .run_system_once(handle_station_directory_buttons)
            .unwrap();
        assert_eq!(
            world.resource::<StationCargoPanelState>().station_pos,
            Some(pos)
        );
    }
}
