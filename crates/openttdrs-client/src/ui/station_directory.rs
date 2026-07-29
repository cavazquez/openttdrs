//! Lista global de estaciones: filtros por compañía, facility y carga.

use bevy::prelude::*;
use openttdrs_core::CargoType;
use openttdrs_core::prelude::*;

use crate::iso::tile_pos;
use crate::render::{MapPreviewCamera, PrimaryGameCamera};
use crate::state::SimWorld;
use crate::ui::floating_window::{
    FloatingWindow, FloatingWindowClosed, FloatingWindowId, TITLE_BROWN, spawn_floating_window,
};
use crate::ui::list_window::{
    SortDir, clear_list_children, list_chip_bg, spawn_list_empty_label, spawn_list_row_button,
    spawn_list_scroll_area, spawn_list_sort_button,
};
use crate::ui::navigation::{OpenUiRoute, UiRoute};
use crate::ui::toolbar::{BuildMenuUi, StationCargoPanelState};

const LIST_HEIGHT: f32 = 270.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum StationDirectorySort {
    #[default]
    Name,
    Rating,
    Waiting,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum StationCompanyFilter {
    #[default]
    All,
    Active,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum StationFacilityFilter {
    #[default]
    All,
    Bus,
    Truck,
    Rail,
    Dock,
    Airport,
    Waypoint,
}

impl StationFacilityFilter {
    fn matches(self, kind: StopKind) -> bool {
        match self {
            Self::All => true,
            Self::Bus => kind == StopKind::BusStop,
            Self::Truck => kind == StopKind::TruckStop,
            Self::Rail => kind == StopKind::RailStation,
            Self::Dock => matches!(kind, StopKind::Dock | StopKind::Buoy),
            Self::Airport => kind == StopKind::Airport,
            Self::Waypoint => matches!(kind, StopKind::RailWaypoint | StopKind::RoadWaypoint),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum StationCargoFilter {
    #[default]
    All,
    Passengers,
    Mail,
    Goods,
    Coal,
    Wood,
    Oil,
    Livestock,
    Grain,
    IronOre,
    Steel,
    Valuables,
}

impl StationCargoFilter {
    fn as_cargo(self) -> Option<CargoType> {
        match self {
            Self::All => None,
            Self::Passengers => Some(CargoType::Passengers),
            Self::Mail => Some(CargoType::Mail),
            Self::Goods => Some(CargoType::Goods),
            Self::Coal => Some(CargoType::Coal),
            Self::Wood => Some(CargoType::Wood),
            Self::Oil => Some(CargoType::Oil),
            Self::Livestock => Some(CargoType::Livestock),
            Self::Grain => Some(CargoType::Grain),
            Self::IronOre => Some(CargoType::IronOre),
            Self::Steel => Some(CargoType::Steel),
            Self::Valuables => Some(CargoType::Valuables),
        }
    }
}

#[derive(Resource, Default)]
pub(crate) struct StationDirectoryState {
    pub(crate) open: bool,
    pub(crate) sort: StationDirectorySort,
    pub(crate) sort_dir: SortDir,
    pub(crate) company: StationCompanyFilter,
    pub(crate) facility: StationFacilityFilter,
    pub(crate) cargo: StationCargoFilter,
}

#[derive(Component)]
pub(crate) struct StationDirectoryListRoot;

#[derive(Component, Clone, Copy)]
pub(crate) struct StationDirectoryRow {
    pos: TileCoord,
}

#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub(crate) struct StationDirectorySortButton(StationDirectorySort);

#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub(crate) struct StationCompanyFilterButton(StationCompanyFilter);

#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub(crate) struct StationFacilityFilterButton(StationFacilityFilter);

#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub(crate) struct StationCargoFilterButton(StationCargoFilter);

#[derive(Default)]
pub(crate) struct StationDirectoryCache {
    sort: StationDirectorySort,
    sort_dir: SortDir,
    company: StationCompanyFilter,
    facility: StationFacilityFilter,
    cargo: StationCargoFilter,
    active_company: CompanyId,
    rows: Vec<(TileCoord, String, StopKind, u8, u32, CompanyId)>,
}

pub(crate) fn setup_station_directory(mut commands: Commands, asset_server: Res<AssetServer>) {
    let asset_server = &*asset_server;
    let (_root, content) = spawn_floating_window(
        &mut commands,
        asset_server,
        FloatingWindowId::StationDirectory,
        "Lista de estaciones",
        TITLE_BROWN,
        Vec2::new(540.0, 80.0),
        520.0,
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
            spawn_sort_button(row, asset_server, "Nombre", StationDirectorySort::Name);
            spawn_sort_button(row, asset_server, "Rating", StationDirectorySort::Rating);
            spawn_sort_button(row, asset_server, "Espera", StationDirectorySort::Waiting);
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
            spawn_company_filter(row, asset_server, "Todas", StationCompanyFilter::All);
            spawn_company_filter(row, asset_server, "Mía", StationCompanyFilter::Active);
        });
        body.spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                column_gap: Val::Px(4.0),
                row_gap: Val::Px(3.0),
                margin: UiRect::bottom(Val::Px(4.0)),
                ..default()
            },
            BuildMenuUi,
        ))
        .with_children(|row| {
            spawn_facility_filter(row, asset_server, "Tipo*", StationFacilityFilter::All);
            spawn_facility_filter(row, asset_server, "Bus", StationFacilityFilter::Bus);
            spawn_facility_filter(row, asset_server, "Camión", StationFacilityFilter::Truck);
            spawn_facility_filter(row, asset_server, "Tren", StationFacilityFilter::Rail);
            spawn_facility_filter(row, asset_server, "Muelle", StationFacilityFilter::Dock);
            spawn_facility_filter(row, asset_server, "Aero", StationFacilityFilter::Airport);
            spawn_facility_filter(row, asset_server, "WP", StationFacilityFilter::Waypoint);
        });
        body.spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                column_gap: Val::Px(4.0),
                row_gap: Val::Px(3.0),
                margin: UiRect::bottom(Val::Px(5.0)),
                ..default()
            },
            BuildMenuUi,
        ))
        .with_children(|row| {
            spawn_cargo_filter(row, asset_server, "Carga*", StationCargoFilter::All);
            spawn_cargo_filter(row, asset_server, "Pax", StationCargoFilter::Passengers);
            spawn_cargo_filter(row, asset_server, "Mail", StationCargoFilter::Mail);
            spawn_cargo_filter(row, asset_server, "Goods", StationCargoFilter::Goods);
            spawn_cargo_filter(row, asset_server, "Carbón", StationCargoFilter::Coal);
            spawn_cargo_filter(row, asset_server, "Madera", StationCargoFilter::Wood);
            spawn_cargo_filter(row, asset_server, "Petróleo", StationCargoFilter::Oil);
            spawn_cargo_filter(row, asset_server, "Grano", StationCargoFilter::Grain);
            spawn_cargo_filter(row, asset_server, "Hierro", StationCargoFilter::IronOre);
            spawn_cargo_filter(row, asset_server, "Acero", StationCargoFilter::Steel);
            spawn_cargo_filter(row, asset_server, "Ganado", StationCargoFilter::Livestock);
            spawn_cargo_filter(row, asset_server, "Valor", StationCargoFilter::Valuables);
        });
        spawn_list_scroll_area(body, asset_server, StationDirectoryListRoot, LIST_HEIGHT);
    });
}

fn spawn_sort_button(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    label: &str,
    sort: StationDirectorySort,
) {
    spawn_list_sort_button(
        parent,
        asset_server,
        label,
        StationDirectorySortButton(sort),
        84.0,
    );
}

fn spawn_company_filter(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    label: &str,
    filter: StationCompanyFilter,
) {
    spawn_list_sort_button(
        parent,
        asset_server,
        label,
        StationCompanyFilterButton(filter),
        72.0,
    );
}

fn spawn_facility_filter(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    label: &str,
    filter: StationFacilityFilter,
) {
    spawn_list_sort_button(
        parent,
        asset_server,
        label,
        StationFacilityFilterButton(filter),
        64.0,
    );
}

fn spawn_cargo_filter(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    label: &str,
    filter: StationCargoFilter,
) {
    spawn_list_sort_button(
        parent,
        asset_server,
        label,
        StationCargoFilterButton(filter),
        72.0,
    );
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

#[allow(clippy::too_many_arguments)]
pub(crate) fn handle_station_directory_buttons(
    mut state: ResMut<StationDirectoryState>,
    sort_buttons: Query<
        (&Interaction, &StationDirectorySortButton),
        (Changed<Interaction>, With<Button>),
    >,
    company_buttons: Query<
        (&Interaction, &StationCompanyFilterButton),
        (Changed<Interaction>, With<Button>),
    >,
    facility_buttons: Query<
        (&Interaction, &StationFacilityFilterButton),
        (Changed<Interaction>, With<Button>),
    >,
    cargo_buttons: Query<
        (&Interaction, &StationCargoFilterButton),
        (Changed<Interaction>, With<Button>),
    >,
    rows: Query<(&Interaction, &StationDirectoryRow), (Changed<Interaction>, With<Button>)>,
    mut station_panel: ResMut<StationCargoPanelState>,
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
            state.sort_dir = match button.0 {
                StationDirectorySort::Name => SortDir::Asc,
                StationDirectorySort::Rating | StationDirectorySort::Waiting => SortDir::Desc,
            };
        }
    }
    for (interaction, button) in &company_buttons {
        if *interaction == Interaction::Pressed {
            state.company = button.0;
        }
    }
    for (interaction, button) in &facility_buttons {
        if *interaction == Interaction::Pressed {
            state.facility = button.0;
        }
    }
    for (interaction, button) in &cargo_buttons {
        if *interaction == Interaction::Pressed {
            state.cargo = button.0;
        }
    }
    for (interaction, row) in &rows {
        if *interaction != Interaction::Pressed {
            continue;
        }
        station_panel.station_pos = Some(row.pos);
        let height = sim.state.map.get(row.pos).map_or(0, |tile| tile.height);
        let center = tile_pos(row.pos.x, row.pos.y, height, 0.0);
        if let Ok(mut transform) = cam_q.single_mut() {
            transform.translation.x = center.x;
            transform.translation.y = center.y;
        }
    }
}

fn station_waiting_total(station: &openttdrs_core::Station) -> u32 {
    let stock_total = station
        .cargo_stock
        .passengers
        .saturating_add(station.cargo_stock.mail)
        .saturating_add(station.cargo_stock.goods)
        .saturating_add(station.cargo_stock.coal)
        .saturating_add(station.cargo_stock.wood)
        .saturating_add(station.cargo_stock.oil);
    station.stock.max(stock_total).max(
        station
            .cargo_packets
            .packets()
            .map(|packet| u32::from(packet.count))
            .fold(0, u32::saturating_add),
    )
}

fn station_matches_cargo_filter(
    station: &openttdrs_core::Station,
    filter: StationCargoFilter,
) -> bool {
    let Some(cargo) = filter.as_cargo() else {
        return true;
    };
    station.accepts_cargo(cargo) || station.cargo_stock.get(cargo) > 0
}

fn company_short_name(sim: &SimWorld, owner: CompanyId) -> String {
    sim.state
        .companies
        .iter()
        .find(|company| company.id == owner)
        .map(|company| company.name.clone())
        .unwrap_or_else(|| format!("C{}", owner.0))
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
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
        (
            With<Button>,
            Without<StationCompanyFilterButton>,
            Without<StationFacilityFilterButton>,
            Without<StationCargoFilterButton>,
        ),
    >,
    mut company_buttons: Query<
        (
            &StationCompanyFilterButton,
            &Interaction,
            &mut BackgroundColor,
        ),
        (
            With<Button>,
            Without<StationDirectorySortButton>,
            Without<StationFacilityFilterButton>,
            Without<StationCargoFilterButton>,
        ),
    >,
    mut facility_buttons: Query<
        (
            &StationFacilityFilterButton,
            &Interaction,
            &mut BackgroundColor,
        ),
        (
            With<Button>,
            Without<StationDirectorySortButton>,
            Without<StationCompanyFilterButton>,
            Without<StationCargoFilterButton>,
        ),
    >,
    mut cargo_buttons: Query<
        (
            &StationCargoFilterButton,
            &Interaction,
            &mut BackgroundColor,
        ),
        (
            With<Button>,
            Without<StationDirectorySortButton>,
            Without<StationCompanyFilterButton>,
            Without<StationFacilityFilterButton>,
        ),
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
        *bg = list_chip_bg(button.0 == state.sort, *interaction);
    }
    for (button, interaction, mut bg) in &mut company_buttons {
        *bg = list_chip_bg(button.0 == state.company, *interaction);
    }
    for (button, interaction, mut bg) in &mut facility_buttons {
        *bg = list_chip_bg(button.0 == state.facility, *interaction);
    }
    for (button, interaction, mut bg) in &mut cargo_buttons {
        *bg = list_chip_bg(button.0 == state.cargo, *interaction);
    }

    let active_company = sim.state.active_company;
    let mut rows: Vec<_> = sim
        .state
        .stations
        .iter()
        .filter(|station| match state.company {
            StationCompanyFilter::All => true,
            StationCompanyFilter::Active => station.owner == active_company,
        })
        .filter(|station| state.facility.matches(station.stop_kind))
        .filter(|station| station_matches_cargo_filter(station, state.cargo))
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
            (
                station.pos,
                name,
                station.stop_kind,
                station.rating,
                station_waiting_total(station),
                station.owner,
            )
        })
        .collect();
    match state.sort {
        StationDirectorySort::Name => {
            rows.sort_by(|a, b| {
                state
                    .sort_dir
                    .apply(a.1.cmp(&b.1).then_with(|| a.0.x.cmp(&b.0.x)))
            });
        }
        StationDirectorySort::Rating => {
            rows.sort_by(|a, b| {
                state
                    .sort_dir
                    .apply(a.3.cmp(&b.3).then_with(|| a.1.cmp(&b.1)))
            });
        }
        StationDirectorySort::Waiting => {
            rows.sort_by(|a, b| {
                state
                    .sort_dir
                    .apply(a.4.cmp(&b.4).then_with(|| a.1.cmp(&b.1)))
            });
        }
    }
    if cache.sort == state.sort
        && cache.sort_dir == state.sort_dir
        && cache.company == state.company
        && cache.facility == state.facility
        && cache.cargo == state.cargo
        && cache.active_company == active_company
        && cache.rows == rows
    {
        return;
    }
    cache.sort = state.sort;
    cache.sort_dir = state.sort_dir;
    cache.company = state.company;
    cache.facility = state.facility;
    cache.cargo = state.cargo;
    cache.active_company = active_company;
    cache.rows.clone_from(&rows);

    let Ok(list_root) = list_roots.single() else {
        return;
    };
    clear_list_children(&mut commands, list_root, &children_q);
    commands.entity(list_root).with_children(|list| {
        if rows.is_empty() {
            spawn_list_empty_label(list, &asset_server, "No hay estaciones con estos filtros.");
            return;
        }
        for (pos, name, kind, rating, waiting, owner) in rows {
            let owner_name = company_short_name(&sim, owner);
            spawn_list_row_button(
                list,
                &asset_server,
                format!(
                    "{name}  ·  {}  ·  {owner_name}  ·  rating {rating}  ·  espera {waiting}",
                    station_kind_label(kind)
                ),
                StationDirectoryRow { pos },
                false,
            );
        }
    });
}

fn station_kind_label(kind: StopKind) -> &'static str {
    match kind {
        StopKind::BusStop => "Bus",
        StopKind::TruckStop => "Camión",
        StopKind::RailStation => "Tren",
        StopKind::Dock => "Muelle",
        StopKind::Buoy => "Boya",
        StopKind::Airport => "Aeropuerto",
        StopKind::RailWaypoint => "Waypoint",
        StopKind::RoadWaypoint => "WP road",
    }
}

pub(crate) fn station_directory_on_closed(
    mut closed: MessageReader<FloatingWindowClosed>,
    mut state: ResMut<StationDirectoryState>,
) {
    for message in closed.read() {
        if message.0.class == FloatingWindowId::StationDirectory {
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
        world.insert_resource(SimWorld {
            state: GameState::new(16, 16),
            ..SimWorld::default()
        });
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

    #[test]
    fn company_and_facility_filters_update_state() {
        let mut world = World::new();
        world.init_resource::<StationDirectoryState>();
        world.init_resource::<StationCargoPanelState>();
        world.insert_resource(SimWorld {
            state: GameState::new(16, 16),
            ..SimWorld::default()
        });
        world.spawn((
            Button,
            StationCompanyFilterButton(StationCompanyFilter::Active),
            Interaction::Pressed,
        ));
        world.spawn((
            Button,
            StationFacilityFilterButton(StationFacilityFilter::Rail),
            Interaction::None,
        ));
        world
            .run_system_once(handle_station_directory_buttons)
            .unwrap();
        assert_eq!(
            world.resource::<StationDirectoryState>().company,
            StationCompanyFilter::Active
        );

        // Second press for facility in a fresh interaction frame.
        let mut world = World::new();
        world.init_resource::<StationDirectoryState>();
        world.init_resource::<StationCargoPanelState>();
        world.insert_resource(SimWorld {
            state: GameState::new(16, 16),
            ..SimWorld::default()
        });
        world.spawn((
            Button,
            StationFacilityFilterButton(StationFacilityFilter::Rail),
            Interaction::Pressed,
        ));
        world.spawn((
            Button,
            StationCargoFilterButton(StationCargoFilter::Coal),
            Interaction::Pressed,
        ));
        world
            .run_system_once(handle_station_directory_buttons)
            .unwrap();
        let state = world.resource::<StationDirectoryState>();
        assert_eq!(state.facility, StationFacilityFilter::Rail);
        assert_eq!(state.cargo, StationCargoFilter::Coal);
    }

    #[test]
    fn cargo_filter_matches_accepting_or_stocked_station() {
        let mut station = Station::new_with_kind(TileCoord::new(1, 1), StopKind::TruckStop);
        station.cargo_stock.coal = 5;
        assert!(station_matches_cargo_filter(
            &station,
            StationCargoFilter::Coal
        ));
        assert!(!station_matches_cargo_filter(
            &station,
            StationCargoFilter::Passengers
        ));
        let bus = Station::new_with_kind(TileCoord::new(2, 2), StopKind::BusStop);
        assert!(station_matches_cargo_filter(
            &bus,
            StationCargoFilter::Passengers
        ));
        let _ = GameState::new(4, 4);
    }
}
