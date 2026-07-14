//! Lista global de subvenciones (ofertas y contratos activos).

use bevy::prelude::*;
use openttdrs_core::{TICKS_PER_MONTH, TileCoord, cargo_display_name};

use crate::iso::tile_pos;
use crate::render::{MapPreviewCamera, PrimaryGameCamera};
use crate::state::SimWorld;
use crate::ui::floating_window::{
    FloatingWindow, FloatingWindowClosed, FloatingWindowId, TITLE_CREAM, WINDOW_TEXT,
    spawn_floating_window, window_text_font,
};
use crate::ui::font::UiFontRole;
use crate::ui::industry_panel::{IndustryPanelState, kind_label, spec_label};
use crate::ui::navigation::{OpenUiRoute, UiRoute};
use crate::ui::toolbar::{BuildMenuUi, StationCargoPanelState};

const LIST_HEIGHT: f32 = 330.0;
const BTN_BG: Color = Color::srgb(0.36, 0.31, 0.21);

#[derive(Resource, Default)]
pub(crate) struct SubsidyListState {
    pub(crate) open: bool,
    pub(crate) selected: Option<u32>,
}

#[derive(Component)]
pub(crate) struct SubsidyListRoot;

#[derive(Component, Clone, Copy)]
pub(crate) struct SubsidyListRow {
    subsidy_id: u32,
}

#[derive(Component, Clone, Copy)]
pub(crate) enum SubsidyListAction {
    CenterSource,
    CenterDest,
    OpenRelated,
}

#[derive(Default)]
pub(crate) struct SubsidyListCache {
    tick: u64,
    rows: Vec<(u32, bool, String)>,
}

pub(crate) fn setup_subsidy_list(mut commands: Commands, asset_server: Res<AssetServer>) {
    let asset_server = &*asset_server;
    let (_root, content) = spawn_floating_window(
        &mut commands,
        asset_server,
        FloatingWindowId::SubsidyList,
        "Subvenciones",
        TITLE_CREAM,
        Vec2::new(480.0, 120.0),
        520.0,
    );
    commands.entity(content).with_children(|body| {
        body.spawn((
            Text::new("Clic en fila: seleccionar y centrar origen"),
            window_text_font(asset_server, UiFontRole::Caption),
            TextColor(WINDOW_TEXT),
            Node {
                margin: UiRect::bottom(Val::Px(4.0)),
                ..default()
            },
        ));
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
                "Centrar origen",
                SubsidyListAction::CenterSource,
            );
            spawn_action_button(
                row,
                asset_server,
                "Centrar destino",
                SubsidyListAction::CenterDest,
            );
            spawn_action_button(
                row,
                asset_server,
                "Abrir entidad",
                SubsidyListAction::OpenRelated,
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
                SubsidyListRoot,
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

fn spawn_action_button(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    label: &str,
    action: SubsidyListAction,
) {
    parent.spawn((
        Button,
        action,
        Node {
            min_width: Val::Px(110.0),
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

fn months_remaining(expires_tick: u64, now: u64) -> u32 {
    if expires_tick <= now {
        return 0;
    }
    u32::try_from((expires_tick - now).div_ceil(TICKS_PER_MONTH)).unwrap_or(u32::MAX)
}

fn industry_label(sim: &SimWorld, pos: TileCoord) -> String {
    if let Some(industry) = sim.state.industries.iter().find(|i| i.pos == pos) {
        let name = industry
            .spec
            .map_or_else(|| kind_label(industry.kind), spec_label);
        format!("{name} ({}, {})", pos.x, pos.y)
    } else {
        format!("Industria ({}, {})", pos.x, pos.y)
    }
}

fn station_label(sim: &SimWorld, pos: TileCoord) -> String {
    if let Some(station) = sim.state.stations.iter().find(|s| s.pos == pos) {
        station
            .name
            .clone()
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| format!("Estación ({}, {})", pos.x, pos.y))
    } else {
        format!("Estación ({}, {})", pos.x, pos.y)
    }
}

fn format_subsidy_row(sim: &SimWorld, subsidy: &openttdrs_core::Subsidy, tick: u64) -> String {
    let cargo = cargo_display_name(subsidy.cargo);
    let source = industry_label(sim, subsidy.source_industry_pos);
    let dest = station_label(sim, subsidy.dest_station_pos);
    if subsidy.is_award_active(tick) {
        let months = months_remaining(subsidy.award_expires_tick, tick);
        let winner = subsidy
            .awarded_company
            .and_then(|id| {
                sim.state
                    .companies
                    .iter()
                    .find(|c| c.id == id)
                    .map(|c| c.name.as_str())
            })
            .unwrap_or("?");
        format!("[Activo ×2 · {winner}] {cargo}: {source} → {dest} · {months} meses")
    } else {
        let months = months_remaining(subsidy.offer_expires_tick, tick);
        format!("[Oferta] {cargo}: {source} → {dest} · {months} meses")
    }
}

pub(crate) fn open_subsidy_list_from_routes(
    mut routes: MessageReader<OpenUiRoute>,
    mut state: ResMut<SubsidyListState>,
) {
    for route in routes.read() {
        if route.0 == UiRoute::Subsidies {
            state.open = true;
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn handle_subsidy_list_buttons(
    mut state: ResMut<SubsidyListState>,
    rows: Query<(&Interaction, &SubsidyListRow), (Changed<Interaction>, With<Button>)>,
    actions: Query<(&Interaction, &SubsidyListAction), (Changed<Interaction>, With<Button>)>,
    sim: Res<SimWorld>,
    mut cam_q: Query<&mut Transform, (With<PrimaryGameCamera>, Without<MapPreviewCamera>)>,
    mut industry_panel: ResMut<IndustryPanelState>,
    mut station_panel: ResMut<StationCargoPanelState>,
) {
    for (interaction, row) in &rows {
        if *interaction != Interaction::Pressed {
            continue;
        }
        state.selected = Some(row.subsidy_id);
        if let Some(subsidy) = sim.state.subsidies.iter().find(|s| s.id == row.subsidy_id) {
            center_on_tile(&sim, &mut cam_q, subsidy.source_industry_pos);
        }
    }
    for (interaction, action) in &actions {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let Some(id) = state.selected else {
            continue;
        };
        let Some(subsidy) = sim.state.subsidies.iter().find(|s| s.id == id) else {
            continue;
        };
        match action {
            SubsidyListAction::CenterSource => {
                center_on_tile(&sim, &mut cam_q, subsidy.source_industry_pos);
            }
            SubsidyListAction::CenterDest => {
                center_on_tile(&sim, &mut cam_q, subsidy.dest_station_pos);
            }
            SubsidyListAction::OpenRelated => {
                industry_panel.open = true;
                industry_panel.focus_tile = Some(subsidy.source_industry_pos);
                station_panel.station_pos = Some(subsidy.dest_station_pos);
                center_on_tile(&sim, &mut cam_q, subsidy.dest_station_pos);
            }
        }
    }
}

fn center_on_tile(
    sim: &SimWorld,
    cam_q: &mut Query<&mut Transform, (With<PrimaryGameCamera>, Without<MapPreviewCamera>)>,
    pos: TileCoord,
) {
    let height = sim.state.map.get(pos).map_or(0, |tile| tile.height);
    let world = tile_pos(pos.x, pos.y, height, 0.0);
    if let Ok(mut transform) = cam_q.single_mut() {
        transform.translation.x = world.x;
        transform.translation.y = world.y;
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn sync_subsidy_list(
    state: Res<SubsidyListState>,
    sim: Res<SimWorld>,
    mut root_q: Query<(&FloatingWindow, &mut Visibility)>,
    list_roots: Query<Entity, With<SubsidyListRoot>>,
    children_q: Query<&Children>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut cache: Local<SubsidyListCache>,
) {
    let Some((_, mut visibility)) = root_q
        .iter_mut()
        .find(|(window, _)| window.id == FloatingWindowId::SubsidyList)
    else {
        return;
    };
    if !state.open {
        *visibility = Visibility::Hidden;
        cache.rows.clear();
        return;
    }
    *visibility = Visibility::Visible;

    let tick = sim.state.tick.get();
    let mut rows: Vec<(u32, bool, String)> = sim
        .state
        .subsidies
        .iter()
        .filter(|s| s.is_offer_active(tick) || s.is_award_active(tick))
        .map(|s| {
            (
                s.id,
                s.is_award_active(tick),
                format_subsidy_row(&sim, s, tick),
            )
        })
        .collect();
    rows.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

    if cache.tick == tick && cache.rows == rows {
        return;
    }
    cache.tick = tick;
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
                Text::new("No hay subvenciones activas ni ofertas."),
                window_text_font(&asset_server, UiFontRole::Caption),
                TextColor(WINDOW_TEXT),
                Node {
                    padding: UiRect::all(Val::Px(8.0)),
                    ..default()
                },
            ));
            return;
        }
        for (subsidy_id, _awarded, label) in rows {
            let selected = Some(subsidy_id) == state.selected;
            list.spawn((
                Button,
                SubsidyListRow { subsidy_id },
                Node {
                    width: Val::Percent(100.0),
                    min_height: Val::Px(28.0),
                    padding: UiRect::horizontal(Val::Px(7.0)),
                    justify_content: JustifyContent::FlexStart,
                    align_items: AlignItems::Center,
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(if selected {
                    Color::srgb(0.58, 0.50, 0.31)
                } else {
                    BTN_BG
                }),
                BorderColor::all(Color::srgb(0.50, 0.44, 0.30)),
                Interaction::default(),
                BuildMenuUi,
                children![(
                    Text::new(label),
                    window_text_font(&asset_server, UiFontRole::Caption),
                    TextColor(WINDOW_TEXT),
                )],
            ));
        }
    });
}

pub(crate) fn subsidy_list_on_closed(
    mut closed: MessageReader<FloatingWindowClosed>,
    mut state: ResMut<SubsidyListState>,
) {
    for message in closed.read() {
        if message.0 == FloatingWindowId::SubsidyList {
            state.open = false;
            state.selected = None;
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use openttdrs_core::{
        CargoType, GameState, Industry, IndustryKind, Station, StopKind, Subsidy,
    };

    #[test]
    fn route_opens_subsidy_list() {
        let mut world = World::new();
        world.init_resource::<SubsidyListState>();
        world.init_resource::<Messages<OpenUiRoute>>();
        world.write_message(OpenUiRoute(UiRoute::Subsidies));
        world
            .run_system_once(open_subsidy_list_from_routes)
            .unwrap();
        assert!(world.resource::<SubsidyListState>().open);
    }

    #[test]
    fn row_centers_on_source_industry() {
        let mut world = World::new();
        world.init_resource::<SubsidyListState>();
        world.init_resource::<IndustryPanelState>();
        world.init_resource::<StationCargoPanelState>();
        let mut state = GameState::new(16, 16);
        let src = TileCoord::new(4, 5);
        let dest = TileCoord::new(8, 5);
        state
            .industries
            .push(Industry::new(src, IndustryKind::CoalMine));
        state
            .stations
            .push(Station::new_with_kind(dest, StopKind::TruckStop));
        state.subsidies.push(Subsidy {
            id: 1,
            cargo: CargoType::Coal,
            source_industry_pos: src,
            dest_station_pos: dest,
            offer_expires_tick: 10_000,
            awarded: false,
            award_expires_tick: 0,
            awarded_company: None,
        });
        let height = state.map.get(src).map_or(0, |t| t.height);
        let expected = tile_pos(src.x, src.y, height, 0.0);
        world.insert_resource(SimWorld {
            state,
            ..SimWorld::default()
        });
        world.spawn((Transform::from_xyz(0.0, 0.0, 0.0), PrimaryGameCamera));
        world.spawn((
            Button,
            SubsidyListRow { subsidy_id: 1 },
            Interaction::Pressed,
        ));
        world.run_system_once(handle_subsidy_list_buttons).unwrap();
        assert_eq!(world.resource::<SubsidyListState>().selected, Some(1));
        let cam = world
            .query_filtered::<&Transform, With<PrimaryGameCamera>>()
            .single(&world)
            .unwrap();
        assert!((cam.translation.x - expected.x).abs() < 0.01);
        assert!((cam.translation.y - expected.y).abs() < 0.01);
    }

    #[test]
    fn months_remaining_rounds_up() {
        assert_eq!(months_remaining(TICKS_PER_MONTH, 0), 1);
        assert_eq!(months_remaining(TICKS_PER_MONTH * 3, TICKS_PER_MONTH), 2);
        assert_eq!(months_remaining(10, 10), 0);
    }
}
