//! Lista global de flota (tren / carretera / barco / avión).

use bevy::prelude::*;
use bevy::ui::widget::ImageNode;
use openttdrs_core::Command;
use openttdrs_core::prelude::*;

use crate::render::{
    MapPreviewCamera, PrimaryGameCamera, RemapMapVisualsPending, TruckHandles,
    vehicle_world_position,
};
use crate::state::SimWorld;
use crate::ui::floating_window::{
    FloatingWindow, FloatingWindowClosed, FloatingWindowId, FloatingWindowTitleText, TITLE_CREAM,
    WINDOW_TEXT, spawn_floating_window, window_text_font,
};
use crate::ui::font::UiFontRole;
use crate::ui::hud::{HudBuildFeedback, push_build_command_error};
use crate::ui::list_window::{
    LIST_BTN_ACTIVE, LIST_BTN_BG, LIST_BTN_HOVER, SortDir, clear_list_children, list_chip_bg,
    spawn_list_empty_label, spawn_list_scroll_area, spawn_list_sort_button,
};
use crate::ui::navigation::{OpenUiRoute, UiRoute};
use crate::ui::toolbar::BuildMenuUi;
use crate::ui::vehicle_window::{
    CONSIST_UNIT_SPRITE_H, CONSIST_UNIT_SPRITE_W, VehicleWindowState, vehicle_side_sprite,
};

const LIST_HEIGHT: f32 = 300.0;
const PLACEHOLDER_SPRITE: &str = "assets/opengfx/tiles/vehicle_train_e.png";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum VehicleListKind {
    #[default]
    Train,
    Road,
    Ship,
    Aircraft,
}

impl VehicleListKind {
    fn title(self) -> &'static str {
        match self {
            Self::Train => "Lista de trenes",
            Self::Road => "Lista de vehículos de carretera",
            Self::Ship => "Lista de barcos",
            Self::Aircraft => "Lista de aviones",
        }
    }

    fn matches(self, kind: VehicleKind) -> bool {
        match self {
            Self::Train => kind == VehicleKind::Train,
            Self::Road => matches!(
                kind,
                VehicleKind::Bus | VehicleKind::Truck | VehicleKind::Tram
            ),
            Self::Ship => kind == VehicleKind::Ship,
            Self::Aircraft => kind == VehicleKind::Aircraft,
        }
    }

    fn empty_label(self) -> &'static str {
        match self {
            Self::Train => "No hay trenes.",
            Self::Road => "No hay vehículos de carretera.",
            Self::Ship => "No hay barcos.",
            Self::Aircraft => "No hay aviones.",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum VehicleListSort {
    #[default]
    Name,
    Age,
    Speed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum VehicleListAction {
    ToggleRunning,
    GotoDepot,
    CenterCamera,
    ClearStationFilter,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum VehicleCompanyFilter {
    #[default]
    Active,
    All,
}

#[derive(Resource, Default)]
pub(crate) struct VehicleListState {
    pub(crate) open: bool,
    pub(crate) kind: VehicleListKind,
    pub(crate) sort: VehicleListSort,
    pub(crate) sort_dir: SortDir,
    pub(crate) selected: Option<u32>,
    /// Si está definido, solo muestra vehículos con orden a esa estación.
    pub(crate) station_filter: Option<TileCoord>,
    pub(crate) company: VehicleCompanyFilter,
}

impl VehicleListState {
    /// Abre la lista filtrada a vehículos que visitan `station_pos`.
    pub(crate) fn open_for_station(
        &mut self,
        station_pos: TileCoord,
        stop_kind: openttdrs_core::StopKind,
    ) {
        self.open = true;
        self.station_filter = Some(station_pos);
        self.selected = None;
        self.kind = match stop_kind {
            openttdrs_core::StopKind::BusStop | openttdrs_core::StopKind::TruckStop => {
                VehicleListKind::Road
            }
            openttdrs_core::StopKind::RailStation | openttdrs_core::StopKind::RailWaypoint => {
                VehicleListKind::Train
            }
            openttdrs_core::StopKind::RoadWaypoint => VehicleListKind::Road,
            openttdrs_core::StopKind::Dock | openttdrs_core::StopKind::Buoy => {
                VehicleListKind::Ship
            }
            openttdrs_core::StopKind::Airport => VehicleListKind::Aircraft,
        };
    }
}

#[derive(Component)]
pub(crate) struct VehicleListRoot;

#[derive(Component, Clone, Copy)]
pub(crate) struct VehicleListRow {
    vehicle_id: u32,
}

#[derive(Component)]
pub(crate) struct VehicleListRowSprite;

#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub(crate) struct VehicleListSortButton(VehicleListSort);

#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub(crate) struct VehicleListKindButton(VehicleListKind);

#[derive(Component, Clone, Copy)]
pub(crate) struct VehicleListActionButton(VehicleListAction);

#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub(crate) struct VehicleCompanyFilterButton(VehicleCompanyFilter);

#[derive(Component)]
pub(crate) struct VehicleListToggleLabel;

#[derive(Default)]
pub(crate) struct VehicleListCache {
    kind: VehicleListKind,
    sort: VehicleListSort,
    sort_dir: SortDir,
    selected: Option<u32>,
    station_filter: Option<TileCoord>,
    company: VehicleCompanyFilter,
    rows: Vec<(u32, String, u32, u16, i32, i32, String)>,
}

pub(crate) fn setup_vehicle_list(mut commands: Commands, asset_server: Res<AssetServer>) {
    let asset_server = &*asset_server;
    let (_root, content) = spawn_floating_window(
        &mut commands,
        asset_server,
        FloatingWindowId::VehicleList,
        VehicleListKind::Train.title(),
        TITLE_CREAM,
        Vec2::new(500.0, 110.0),
        500.0,
    );
    commands.entity(content).with_children(|body| {
        body.spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(4.0),
                margin: UiRect::bottom(Val::Px(4.0)),
                ..default()
            },
            BuildMenuUi,
        ))
        .with_children(|row| {
            spawn_list_sort_button(
                row,
                asset_server,
                "Trenes",
                VehicleListKindButton(VehicleListKind::Train),
                84.0,
            );
            spawn_list_sort_button(
                row,
                asset_server,
                "Carretera",
                VehicleListKindButton(VehicleListKind::Road),
                84.0,
            );
            spawn_list_sort_button(
                row,
                asset_server,
                "Barcos",
                VehicleListKindButton(VehicleListKind::Ship),
                84.0,
            );
            spawn_list_sort_button(
                row,
                asset_server,
                "Aviones",
                VehicleListKindButton(VehicleListKind::Aircraft),
                84.0,
            );
            spawn_list_sort_button(
                row,
                asset_server,
                "Mía",
                VehicleCompanyFilterButton(VehicleCompanyFilter::Active),
                48.0,
            );
            spawn_list_sort_button(
                row,
                asset_server,
                "Todas",
                VehicleCompanyFilterButton(VehicleCompanyFilter::All),
                56.0,
            );
        });
        body.spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(4.0),
                margin: UiRect::bottom(Val::Px(4.0)),
                ..default()
            },
            BuildMenuUi,
        ))
        .with_children(|row| {
            spawn_list_sort_button(
                row,
                asset_server,
                "Nombre",
                VehicleListSortButton(VehicleListSort::Name),
                84.0,
            );
            spawn_list_sort_button(
                row,
                asset_server,
                "Edad",
                VehicleListSortButton(VehicleListSort::Age),
                84.0,
            );
            spawn_list_sort_button(
                row,
                asset_server,
                "Velocidad",
                VehicleListSortButton(VehicleListSort::Speed),
                84.0,
            );
        });
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
            spawn_action_button(
                row,
                asset_server,
                "Iniciar",
                VehicleListAction::ToggleRunning,
                true,
            );
            spawn_action_button(
                row,
                asset_server,
                "Depósito",
                VehicleListAction::GotoDepot,
                false,
            );
            spawn_action_button(
                row,
                asset_server,
                "Centrar",
                VehicleListAction::CenterCamera,
                false,
            );
            spawn_action_button(
                row,
                asset_server,
                "Quitar filtro",
                VehicleListAction::ClearStationFilter,
                false,
            );
        });
        spawn_list_scroll_area(body, VehicleListRoot, LIST_HEIGHT);
    });
}

fn spawn_action_button(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    label: &str,
    action: VehicleListAction,
    toggle_label: bool,
) {
    let mut entity = parent.spawn((
        Button,
        VehicleListActionButton(action),
        Node {
            min_width: Val::Px(88.0),
            height: Val::Px(24.0),
            padding: UiRect::horizontal(Val::Px(6.0)),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        BackgroundColor(LIST_BTN_BG),
        BorderColor::all(Color::srgb(0.58, 0.50, 0.33)),
        Interaction::default(),
        BuildMenuUi,
    ));
    entity.with_children(|btn| {
        let mut text = btn.spawn((
            Text::new(label),
            window_text_font(asset_server, UiFontRole::Caption),
            TextColor(WINDOW_TEXT),
        ));
        if toggle_label {
            text.insert(VehicleListToggleLabel);
        }
    });
}

fn speed_to_kmh(kind: VehicleKind, units: u16) -> u16 {
    match kind {
        VehicleKind::Train | VehicleKind::Aircraft => units,
        VehicleKind::Bus | VehicleKind::Truck | VehicleKind::Tram | VehicleKind::Ship => units / 2,
    }
}

fn vehicle_status_label(vehicle: &openttdrs_core::Vehicle) -> String {
    if vehicle.running {
        if vehicle.no_network_route_to_order {
            "Sin ruta".to_string()
        } else {
            "En marcha".to_string()
        }
    } else {
        "Detenido".to_string()
    }
}

pub(crate) fn open_vehicle_list_from_routes(
    mut routes: MessageReader<OpenUiRoute>,
    mut state: ResMut<VehicleListState>,
) {
    for route in routes.read() {
        if let UiRoute::Vehicles(kind) = route.0 {
            state.kind = kind;
            state.open = true;
            state.station_filter = None;
        }
    }
}

fn vehicle_visits_station(vehicle: &openttdrs_core::Vehicle, station_pos: TileCoord) -> bool {
    vehicle.orders.iter().any(
        |order| matches!(order, VehicleOrder::Station { station, .. } if *station == station_pos),
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn handle_vehicle_list_buttons(
    mut state: ResMut<VehicleListState>,
    kind_buttons: Query<
        (&Interaction, &VehicleListKindButton),
        (Changed<Interaction>, With<Button>),
    >,
    company_filter_buttons: Query<
        (&Interaction, &VehicleCompanyFilterButton),
        (Changed<Interaction>, With<Button>),
    >,
    sort_buttons: Query<
        (&Interaction, &VehicleListSortButton),
        (Changed<Interaction>, With<Button>),
    >,
    action_buttons: Query<
        (&Interaction, &VehicleListActionButton),
        (Changed<Interaction>, With<Button>),
    >,
    rows: Query<(&Interaction, &VehicleListRow), (Changed<Interaction>, With<Button>)>,
    mut vehicle_window: ResMut<VehicleWindowState>,
    mut sim: ResMut<SimWorld>,
    mut pending: ResMut<RemapMapVisualsPending>,
    mut hud_feedback: ResMut<HudBuildFeedback>,
    time: Res<Time>,
    mut cam_q: Query<&mut Transform, (With<PrimaryGameCamera>, Without<MapPreviewCamera>)>,
) {
    for (interaction, button) in &kind_buttons {
        if *interaction == Interaction::Pressed {
            state.kind = button.0;
            state.selected = None;
        }
    }
    for (interaction, button) in &company_filter_buttons {
        if *interaction == Interaction::Pressed {
            state.company = button.0;
        }
    }
    for (interaction, button) in &sort_buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        if state.sort == button.0 {
            state.sort_dir = state.sort_dir.toggle();
        } else {
            state.sort = button.0;
            state.sort_dir = match button.0 {
                VehicleListSort::Name => SortDir::Asc,
                VehicleListSort::Age | VehicleListSort::Speed => SortDir::Desc,
            };
        }
    }
    for (interaction, row) in &rows {
        if *interaction == Interaction::Pressed {
            state.selected = Some(row.vehicle_id);
            vehicle_window.vehicle_id = Some(row.vehicle_id);
            vehicle_window.rename_editing = false;
        }
    }
    for (interaction, action) in &action_buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        if matches!(action.0, VehicleListAction::ClearStationFilter) {
            state.station_filter = None;
            continue;
        }
        let Some(vehicle_id) = state.selected.or(vehicle_window.vehicle_id) else {
            continue;
        };
        match action.0 {
            VehicleListAction::ToggleRunning => {
                match crate::network::apply_player_command(
                    &mut sim.state,
                    &Command::ToggleVehicleRunning(vehicle_id),
                ) {
                    Ok(()) => pending.pending = true,
                    Err(e) => {
                        push_build_command_error(&mut hud_feedback, e, time.elapsed_secs());
                    }
                }
            }
            VehicleListAction::GotoDepot => {
                match crate::network::apply_player_command(
                    &mut sim.state,
                    &Command::AppendGotoNearestDepot(vehicle_id),
                ) {
                    Ok(()) => pending.pending = true,
                    Err(e) => {
                        push_build_command_error(&mut hud_feedback, e, time.elapsed_secs());
                    }
                }
            }
            VehicleListAction::CenterCamera => {
                if let Some(vehicle) = sim.state.vehicles.iter().find(|v| v.id == vehicle_id) {
                    let world_pos = vehicle_world_position(vehicle, &sim.state.map);
                    if let Ok(mut transform) = cam_q.single_mut() {
                        transform.translation.x = world_pos.x;
                        transform.translation.y = world_pos.y;
                    }
                }
            }
            VehicleListAction::ClearStationFilter => {}
        }
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(crate) fn sync_vehicle_list(
    state: Res<VehicleListState>,
    sim: Res<SimWorld>,
    trucks: Option<Res<TruckHandles>>,
    mut root_q: Query<(&FloatingWindow, &mut Visibility)>,
    mut title_q: Query<(&FloatingWindowTitleText, &mut Text), Without<VehicleListToggleLabel>>,
    list_roots: Query<Entity, With<VehicleListRoot>>,
    children_q: Query<&Children>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut cache: Local<VehicleListCache>,
    mut kind_buttons: Query<
        (&VehicleListKindButton, &Interaction, &mut BackgroundColor),
        (
            With<Button>,
            Without<VehicleListSortButton>,
            Without<VehicleListActionButton>,
        ),
    >,
    mut sort_buttons: Query<
        (&VehicleListSortButton, &Interaction, &mut BackgroundColor),
        (
            With<Button>,
            Without<VehicleListKindButton>,
            Without<VehicleListActionButton>,
        ),
    >,
    mut action_buttons: Query<
        (&VehicleListActionButton, &Interaction, &mut BackgroundColor),
        (
            With<Button>,
            Without<VehicleListKindButton>,
            Without<VehicleListSortButton>,
        ),
    >,
    mut toggle_label_q: Query<
        &mut Text,
        (
            With<VehicleListToggleLabel>,
            Without<FloatingWindowTitleText>,
        ),
    >,
    mut row_buttons: Query<
        (&VehicleListRow, &mut BackgroundColor),
        (
            With<Button>,
            Without<VehicleListKindButton>,
            Without<VehicleListSortButton>,
            Without<VehicleListActionButton>,
        ),
    >,
) {
    let Some((_, mut visibility)) = root_q
        .iter_mut()
        .find(|(window, _)| window.id == FloatingWindowId::VehicleList)
    else {
        return;
    };
    if !state.open {
        *visibility = Visibility::Hidden;
        cache.rows.clear();
        return;
    }
    *visibility = Visibility::Visible;

    if let Some((_, mut title)) = title_q
        .iter_mut()
        .find(|(text, _)| text.0 == FloatingWindowId::VehicleList)
    {
        **title = if let Some(pos) = state.station_filter {
            format!("{} · estación ({}, {})", state.kind.title(), pos.x, pos.y)
        } else {
            state.kind.title().to_string()
        };
    }

    for (button, interaction, mut bg) in &mut kind_buttons {
        *bg = list_chip_bg(button.0 == state.kind, *interaction);
    }
    for (button, interaction, mut bg) in &mut sort_buttons {
        *bg = list_chip_bg(button.0 == state.sort, *interaction);
    }
    let has_selection = state.selected.is_some();
    let has_station_filter = state.station_filter.is_some();
    for (action, interaction, mut bg) in &mut action_buttons {
        let enabled = match action.0 {
            VehicleListAction::ClearStationFilter => has_station_filter,
            _ => has_selection,
        };
        *bg = if !enabled {
            BackgroundColor(Color::srgb(0.28, 0.24, 0.17))
        } else if *interaction == Interaction::Hovered {
            BackgroundColor(LIST_BTN_HOVER)
        } else {
            BackgroundColor(LIST_BTN_BG)
        };
    }
    if let Ok(mut toggle) = toggle_label_q.single_mut() {
        let running = state
            .selected
            .and_then(|id| sim.state.vehicles.iter().find(|v| v.id == id))
            .is_some_and(|v| v.running);
        **toggle = if running {
            "Detener".to_string()
        } else {
            "Iniciar".to_string()
        };
    }

    let tick = sim.state.tick.get();
    let company = sim.state.active_company;
    let mut rows: Vec<_> = sim
        .state
        .vehicles
        .iter()
        .filter(|vehicle| vehicle.is_consist_head())
        .filter(|vehicle| match state.company {
            VehicleCompanyFilter::Active => vehicle.owner == company,
            VehicleCompanyFilter::All => true,
        })
        .filter(|vehicle| state.kind.matches(vehicle.kind))
        .filter(|vehicle| {
            state
                .station_filter
                .is_none_or(|pos| vehicle_visits_station(vehicle, pos))
        })
        .map(|vehicle| {
            let mut name = vehicle.display_name();
            if state.company == VehicleCompanyFilter::All && vehicle.owner != company {
                name = format!("[{}] {name}", vehicle.owner.0);
            }
            (
                vehicle.id,
                name,
                vehicle.vehicle_age_years(tick),
                speed_to_kmh(vehicle.kind, vehicle.effective_speed()),
                vehicle.pos.x,
                vehicle.pos.y,
                vehicle_status_label(vehicle),
            )
        })
        .collect();
    match state.sort {
        VehicleListSort::Name => {
            rows.sort_by(|a, b| {
                state
                    .sort_dir
                    .apply(a.1.cmp(&b.1).then_with(|| a.0.cmp(&b.0)))
            });
        }
        VehicleListSort::Age => {
            rows.sort_by(|a, b| {
                state
                    .sort_dir
                    .apply(a.2.cmp(&b.2).then_with(|| a.1.cmp(&b.1)))
            });
        }
        VehicleListSort::Speed => {
            rows.sort_by(|a, b| {
                state
                    .sort_dir
                    .apply(a.3.cmp(&b.3).then_with(|| a.1.cmp(&b.1)))
            });
        }
    }

    if cache.kind == state.kind
        && cache.sort == state.sort
        && cache.sort_dir == state.sort_dir
        && cache.station_filter == state.station_filter
        && cache.company == state.company
        && cache.rows == rows
    {
        if cache.selected != state.selected {
            cache.selected = state.selected;
            for (row, mut bg) in &mut row_buttons {
                *bg = if Some(row.vehicle_id) == state.selected {
                    BackgroundColor(LIST_BTN_ACTIVE)
                } else {
                    BackgroundColor(LIST_BTN_BG)
                };
            }
        }
        return;
    }
    cache.kind = state.kind;
    cache.sort = state.sort;
    cache.sort_dir = state.sort_dir;
    cache.station_filter = state.station_filter;
    cache.company = state.company;
    cache.selected = state.selected;
    cache.rows.clone_from(&rows);

    let Ok(list_root) = list_roots.single() else {
        return;
    };
    clear_list_children(&mut commands, list_root, &children_q);
    commands.entity(list_root).with_children(|list| {
        if rows.is_empty() {
            let empty = if state.station_filter.is_some() {
                "Ningún vehículo visita esta estación."
            } else {
                state.kind.empty_label()
            };
            spawn_list_empty_label(list, &asset_server, empty);
            return;
        }
        for (vehicle_id, name, age, speed, x, y, status) in rows {
            spawn_vehicle_list_row(
                list,
                &asset_server,
                trucks.as_deref(),
                &sim,
                vehicle_id,
                format!("{name}  ·  {age}a  ·  {speed} km/h  ·  ({x},{y})  ·  {status}"),
                Some(vehicle_id) == state.selected,
            );
        }
    });
}

fn spawn_vehicle_list_row(
    list: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    trucks: Option<&TruckHandles>,
    sim: &SimWorld,
    vehicle_id: u32,
    label: String,
    selected: bool,
) {
    let sprite = trucks
        .and_then(|t| {
            sim.state
                .vehicles
                .iter()
                .find(|v| v.id == vehicle_id)
                .map(|v| vehicle_side_sprite(t, v))
        })
        .unwrap_or_else(|| asset_server.load::<Image>(PLACEHOLDER_SPRITE));
    list.spawn((
        Button,
        VehicleListRow { vehicle_id },
        Node {
            width: Val::Percent(100.0),
            min_height: Val::Px(24.0),
            padding: UiRect::horizontal(Val::Px(4.0)),
            column_gap: Val::Px(6.0),
            justify_content: JustifyContent::FlexStart,
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        BackgroundColor(if selected {
            LIST_BTN_ACTIVE
        } else {
            LIST_BTN_BG
        }),
        BorderColor::all(Color::srgb(0.50, 0.44, 0.30)),
        Interaction::default(),
        BuildMenuUi,
    ))
    .with_children(|row| {
        row.spawn((
            VehicleListRowSprite,
            ImageNode::new(sprite),
            Node {
                width: Val::Px(CONSIST_UNIT_SPRITE_W),
                height: Val::Px(CONSIST_UNIT_SPRITE_H),
                flex_shrink: 0.0,
                ..default()
            },
        ));
        row.spawn((
            Text::new(label),
            window_text_font(asset_server, UiFontRole::Caption),
            TextColor(WINDOW_TEXT),
        ));
    });
}

pub(crate) fn vehicle_list_on_closed(
    mut closed: MessageReader<FloatingWindowClosed>,
    mut state: ResMut<VehicleListState>,
) {
    for message in closed.read() {
        if message.0 == FloatingWindowId::VehicleList {
            state.open = false;
            state.selected = None;
            state.station_filter = None;
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    fn fixture_resources(world: &mut World) {
        world.init_resource::<VehicleListState>();
        world.init_resource::<VehicleWindowState>();
        world.init_resource::<RemapMapVisualsPending>();
        world.init_resource::<HudBuildFeedback>();
        world.insert_resource(Time::<()>::default());
    }

    fn sim_with(state: GameState) -> SimWorld {
        SimWorld {
            state,
            ..SimWorld::default()
        }
    }

    #[test]
    fn route_opens_vehicle_list_with_kind() {
        let mut world = World::new();
        world.init_resource::<VehicleListState>();
        world.init_resource::<Messages<OpenUiRoute>>();
        world.write_message(OpenUiRoute(UiRoute::Vehicles(VehicleListKind::Ship)));
        world
            .run_system_once(open_vehicle_list_from_routes)
            .unwrap();
        let state = world.resource::<VehicleListState>();
        assert!(state.open);
        assert_eq!(state.kind, VehicleListKind::Ship);
    }

    #[test]
    fn vehicle_row_opens_vehicle_window_and_selects() {
        let mut world = World::new();
        fixture_resources(&mut world);
        world.insert_resource(sim_with(GameState::new(8, 8)));
        world.spawn((
            Button,
            VehicleListRow { vehicle_id: 42 },
            Interaction::Pressed,
        ));
        world.run_system_once(handle_vehicle_list_buttons).unwrap();
        assert_eq!(world.resource::<VehicleWindowState>().vehicle_id, Some(42));
        assert_eq!(world.resource::<VehicleListState>().selected, Some(42));
    }

    #[test]
    fn kind_button_switches_filter() {
        let mut world = World::new();
        fixture_resources(&mut world);
        world.insert_resource(sim_with(GameState::new(8, 8)));
        world.spawn((
            Button,
            VehicleListKindButton(VehicleListKind::Aircraft),
            Interaction::Pressed,
        ));
        world.run_system_once(handle_vehicle_list_buttons).unwrap();
        assert_eq!(
            world.resource::<VehicleListState>().kind,
            VehicleListKind::Aircraft
        );
    }

    #[test]
    fn toggle_action_starts_selected_vehicle() {
        let mut world = World::new();
        fixture_resources(&mut world);
        let mut state = GameState::new(16, 16);
        let mut vehicle = Vehicle::new(
            7,
            VehicleKind::Train,
            TileCoord::new(1, 1),
            TileCoord::new(1, 1),
        );
        vehicle.running = false;
        vehicle.owner = state.active_company;
        state.vehicles.push(vehicle);
        world.insert_resource(sim_with(state));
        world.resource_mut::<VehicleListState>().selected = Some(7);
        world.spawn((
            Button,
            VehicleListActionButton(VehicleListAction::ToggleRunning),
            Interaction::Pressed,
        ));
        world.run_system_once(handle_vehicle_list_buttons).unwrap();
        let sim = world.resource::<SimWorld>();
        assert!(
            sim.state
                .vehicles
                .iter()
                .find(|v| v.id == 7)
                .unwrap()
                .running
        );
    }

    #[test]
    fn center_action_moves_camera_to_vehicle() {
        let mut world = World::new();
        fixture_resources(&mut world);
        let mut state = GameState::new(16, 16);
        let mut vehicle = Vehicle::new(
            3,
            VehicleKind::Bus,
            TileCoord::new(4, 5),
            TileCoord::new(4, 5),
        );
        vehicle.owner = state.active_company;
        let expected = vehicle_world_position(&vehicle, &state.map);
        state.vehicles.push(vehicle);
        world.insert_resource(sim_with(state));
        world.resource_mut::<VehicleListState>().selected = Some(3);
        world.spawn((Transform::from_xyz(0.0, 0.0, 0.0), PrimaryGameCamera));
        world.spawn((
            Button,
            VehicleListActionButton(VehicleListAction::CenterCamera),
            Interaction::Pressed,
        ));
        world.run_system_once(handle_vehicle_list_buttons).unwrap();
        let cam = world
            .query_filtered::<&Transform, With<PrimaryGameCamera>>()
            .single(&world)
            .unwrap();
        assert!((cam.translation.x - expected.x).abs() < 0.01);
        assert!((cam.translation.y - expected.y).abs() < 0.01);
    }

    #[test]
    fn open_for_station_sets_kind_and_filter() {
        let mut state = VehicleListState::default();
        state.open_for_station(TileCoord::new(2, 3), openttdrs_core::StopKind::Dock);
        assert!(state.open);
        assert_eq!(state.kind, VehicleListKind::Ship);
        assert_eq!(state.station_filter, Some(TileCoord::new(2, 3)));
    }

    #[test]
    fn clear_station_filter_does_not_need_selection() {
        let mut world = World::new();
        fixture_resources(&mut world);
        world.insert_resource(sim_with(GameState::new(8, 8)));
        world.resource_mut::<VehicleListState>().station_filter = Some(TileCoord::new(1, 1));
        world.spawn((
            Button,
            VehicleListActionButton(VehicleListAction::ClearStationFilter),
            Interaction::Pressed,
        ));
        world.run_system_once(handle_vehicle_list_buttons).unwrap();
        assert!(
            world
                .resource::<VehicleListState>()
                .station_filter
                .is_none()
        );
    }

    #[test]
    fn route_clears_station_filter() {
        let mut world = World::new();
        world.init_resource::<VehicleListState>();
        world.init_resource::<Messages<OpenUiRoute>>();
        world.resource_mut::<VehicleListState>().station_filter = Some(TileCoord::new(0, 0));
        world.write_message(OpenUiRoute(UiRoute::Vehicles(VehicleListKind::Train)));
        world
            .run_system_once(open_vehicle_list_from_routes)
            .unwrap();
        assert!(
            world
                .resource::<VehicleListState>()
                .station_filter
                .is_none()
        );
    }
}
