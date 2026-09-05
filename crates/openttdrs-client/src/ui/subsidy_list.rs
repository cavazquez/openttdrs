//! Lista global de subvenciones (ofertas y contratos activos).

use bevy::prelude::*;
use openttdrs_core::prelude::*;
use openttdrs_core::{TICKS_PER_MONTH, cargo_display_name};

use crate::i18n::{Locale, localized_text};
use crate::iso::tile_pos;
use crate::render::{MapPreviewCamera, PrimaryGameCamera};
use crate::settings::ClientPreferences;
use crate::state::SimWorld;
use crate::ui::floating_window::{
    FloatingWindow, FloatingWindowClosed, FloatingWindowId, TITLE_CREAM, WINDOW_TEXT,
    spawn_floating_window, window_text_font,
};
use crate::ui::font::UiFontRole;
use crate::ui::industry_panel::{IndustryPanelState, kind_label, spec_label};
use crate::ui::navigation::{OpenUiRoute, UiRoute};
use crate::ui::scrollbar::spawn_classic_scroll_area;
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
    locale: Option<Locale>,
    rows: Vec<(u32, bool, String)>,
}

impl SubsidyListCache {
    fn needs_refresh(&self, tick: u64, locale: Locale, rows: &[(u32, bool, String)]) -> bool {
        self.tick != tick || self.locale != Some(locale) || self.rows != rows
    }

    fn reset(&mut self) {
        self.tick = 0;
        self.locale = None;
        self.rows.clear();
    }
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
        spawn_classic_scroll_area(
            body,
            asset_server,
            SubsidyListRoot,
            LIST_HEIGHT,
            Color::srgb(0.22, 0.18, 0.12),
            Color::srgb(0.45, 0.39, 0.27),
        );
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

fn industry_label(locale: Locale, sim: &SimWorld, pos: TileCoord) -> String {
    if let Some(industry) = sim.state.industries.iter().find(|i| i.pos == pos) {
        let name = industry
            .spec
            .map_or_else(|| kind_label(industry.kind), spec_label);
        format!("{name} ({}, {})", pos.x, pos.y)
    } else {
        format!(
            "{} ({}, {})",
            localized_text(locale, "Industria"),
            pos.x,
            pos.y
        )
    }
}

fn station_label(locale: Locale, sim: &SimWorld, pos: TileCoord) -> String {
    if let Some(station) = sim.state.stations.iter().find(|s| s.pos == pos) {
        station
            .name
            .clone()
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| {
                format!(
                    "{} ({}, {})",
                    localized_text(locale, "Estación"),
                    pos.x,
                    pos.y
                )
            })
    } else {
        format!(
            "{} ({}, {})",
            localized_text(locale, "Estación"),
            pos.x,
            pos.y
        )
    }
}

fn format_subsidy_row(
    locale: Locale,
    sim: &SimWorld,
    subsidy: &openttdrs_core::Subsidy,
    tick: u64,
) -> String {
    let cargo = cargo_display_name(subsidy.cargo);
    let source = industry_label(locale, sim, subsidy.source_industry_pos);
    let dest = station_label(locale, sim, subsidy.dest_station_pos);
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
        match locale {
            Locale::Es => {
                format!("[Activo ×2 · {winner}] {cargo}: {source} → {dest} · {months} meses")
            }
            Locale::En => {
                format!("[Active ×2 · {winner}] {cargo}: {source} → {dest} · {months} months")
            }
        }
    } else {
        let months = months_remaining(subsidy.offer_expires_tick, tick);
        match locale {
            Locale::Es => format!("[Oferta] {cargo}: {source} → {dest} · {months} meses"),
            Locale::En => format!("[Offer] {cargo}: {source} → {dest} · {months} months"),
        }
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
                station_panel.selected_tile = Some(subsidy.dest_station_pos);
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
    prefs: Res<ClientPreferences>,
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
        cache.reset();
        return;
    }
    *visibility = Visibility::Visible;

    let tick = sim.state.tick.get();
    let locale = prefs.locale();
    let mut rows: Vec<(u32, bool, String)> = sim
        .state
        .subsidies
        .iter()
        .filter(|s| s.is_offer_active(tick) || s.is_award_active(tick))
        .map(|s| {
            (
                s.id,
                s.is_award_active(tick),
                format_subsidy_row(locale, &sim, s, tick),
            )
        })
        .collect();
    rows.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

    if !cache.needs_refresh(tick, locale, &rows) {
        return;
    }
    cache.tick = tick;
    cache.locale = Some(locale);
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
        if message.0.class == FloatingWindowId::SubsidyList {
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

    use openttdrs_core::{CargoType, Industry, IndustryKind, Subsidy};

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
            source_town_pos: None,
            dest_town_pos: None,
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

    #[test]
    fn subsidy_rows_follow_locale_without_translating_cargo_data() {
        let state = GameState::new(8, 8);
        let sim = SimWorld {
            state,
            ..SimWorld::default()
        };
        let subsidy = Subsidy {
            id: 4,
            cargo: CargoType::Coal,
            source_industry_pos: TileCoord::new(1, 2),
            dest_station_pos: TileCoord::new(5, 6),
            source_town_pos: None,
            dest_town_pos: None,
            offer_expires_tick: TICKS_PER_MONTH * 2,
            awarded: false,
            award_expires_tick: 0,
            awarded_company: None,
        };

        let english = format_subsidy_row(Locale::En, &sim, &subsidy, 0);
        assert!(english.starts_with("[Offer]"));
        assert!(english.contains(cargo_display_name(CargoType::Coal)));
        assert!(english.contains("Industry (1, 2)"));
        assert!(english.contains("Station (5, 6)"));
        assert!(english.ends_with("2 months"));

        let spanish = format_subsidy_row(Locale::Es, &sim, &subsidy, 0);
        assert!(spanish.starts_with("[Oferta]"));
        assert!(spanish.contains(cargo_display_name(CargoType::Coal)));
        assert!(spanish.contains("Industria (1, 2)"));
        assert!(spanish.contains("Estación (5, 6)"));
        assert!(spanish.ends_with("2 meses"));
    }

    #[test]
    fn subsidy_cache_invalidates_when_only_locale_changes() {
        let rows = vec![(4, false, "[Oferta] Coal".to_owned())];
        let mut cache = SubsidyListCache {
            tick: 12,
            locale: Some(Locale::Es),
            rows: rows.clone(),
        };
        assert!(!cache.needs_refresh(12, Locale::Es, &rows));
        assert!(cache.needs_refresh(12, Locale::En, &rows));
        assert!(cache.needs_refresh(13, Locale::Es, &rows));
        cache.reset();
        assert!(cache.needs_refresh(0, Locale::Es, &[]));
    }
}
