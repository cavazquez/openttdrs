//! Guía acotada para la sesión determinista «Primera ruta».
//!
//! No es un sistema de tutoriales ni un GameScript: esta UI sólo se materializa
//! cuando el estado persistido sigue siendo el escenario de la primera ruta.
//! Cada marca se calcula de las entidades y contadores del mundo, por lo que
//! una carga JSON recompone exactamente el mismo avance.

use bevy::prelude::*;
use bevy::ui::FocusPolicy;
use openttdrs_core::parity::{
    FIRST_ROUTE_COAL_MINE, FIRST_ROUTE_POWER_STATION, FIRST_ROUTE_WORLD_SEED,
};
use openttdrs_core::{
    CargoType, Climate, GameState, Industry, IndustrySpec, PathNetwork, STATION_COVERAGE_RADIUS,
    StopKind, TileCoord, TileKind, Vehicle, VehicleKind, VehicleOrder, find_path,
    industry_in_station_coverage, station_catchment_radius,
};

use crate::i18n::Locale;
use crate::settings::ClientPreferences;
use crate::state::SimWorld;
use crate::state::ingame_lifecycle::InGameUi;
use crate::ui::floating_window::window_text_font;
use crate::ui::font::UiFontRole;

const GUIDE_WIDTH: f32 = 420.0;
const GUIDE_TOP: f32 = 74.0;
const GUIDE_RIGHT: f32 = 16.0;
const GUIDE_Z: i32 = 2050;

const PANEL_BG: Color = Color::srgba(0.08, 0.075, 0.055, 0.94);
const PANEL_BORDER: Color = Color::srgb(0.72, 0.64, 0.39);
const TITLE: Color = Color::srgb(0.98, 0.91, 0.67);
const BODY: Color = Color::srgb(0.94, 0.9, 0.8);
const NEXT: Color = Color::srgb(0.98, 0.83, 0.4);

/// Progreso reconstruible de la primera ruta.
///
/// Los campos no se guardan: son una vista sobre [`GameState`], para que no
/// haya progreso accionado por clics, temporizadores o estado de UI efímero.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FirstRouteProgress {
    pub(crate) road_connected: bool,
    pub(crate) stops_and_depot: bool,
    pub(crate) coal_truck_ready: bool,
    pub(crate) orders_assigned: bool,
    pub(crate) vehicle_running: bool,
    pub(crate) first_delivery: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FirstRouteStep {
    ConnectRoad,
    StopsAndDepot,
    CoalTruck,
    Orders,
    StartVehicle,
    FirstDelivery,
}

impl FirstRouteProgress {
    fn next_step(self) -> Option<FirstRouteStep> {
        (!self.road_connected)
            .then_some(FirstRouteStep::ConnectRoad)
            .or_else(|| (!self.stops_and_depot).then_some(FirstRouteStep::StopsAndDepot))
            .or_else(|| (!self.coal_truck_ready).then_some(FirstRouteStep::CoalTruck))
            .or_else(|| (!self.orders_assigned).then_some(FirstRouteStep::Orders))
            .or_else(|| (!self.vehicle_running).then_some(FirstRouteStep::StartVehicle))
            .or_else(|| (!self.first_delivery).then_some(FirstRouteStep::FirstDelivery))
    }
}

impl FirstRouteStep {
    const fn instruction(self, locale: Locale) -> &'static str {
        match (locale, self) {
            (Locale::Es, Self::ConnectRoad) => {
                "Carreteras: conectá la mina y la central con una carretera."
            }
            (Locale::Es, Self::StopsAndDepot) => {
                "Carreteras: colocá una parada de camión junto a cada industria y un depósito."
            }
            (Locale::Es, Self::CoalTruck) => {
                "Depósito: comprá un camión y refitalo para transportar carbón."
            }
            (Locale::Es, Self::Orders) => {
                "Camión → Órdenes: añadí mina de carbón → central eléctrica."
            }
            (Locale::Es, Self::StartVehicle) => "Camión: iniciálo y esperá la primera entrega.",
            (Locale::Es, Self::FirstDelivery) => {
                "Esperá a que el camión entregue carbón en la central."
            }
            (Locale::En, Self::ConnectRoad) => {
                "Roads: connect the mine and power station with a road."
            }
            (Locale::En, Self::StopsAndDepot) => {
                "Roads: place a truck stop by each industry and a depot."
            }
            (Locale::En, Self::CoalTruck) => "Depot: buy a truck and refit it to carry coal.",
            (Locale::En, Self::Orders) => "Truck → Orders: add coal mine → power station.",
            (Locale::En, Self::StartVehicle) => "Truck: start it and wait for the first delivery.",
            (Locale::En, Self::FirstDelivery) => {
                "Wait for the truck to deliver coal to the power station."
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FirstRouteGuideContent {
    title: &'static str,
    objective: &'static str,
    checklist: String,
    next: String,
}

fn first_route_guide_content(
    locale: Locale,
    progress: FirstRouteProgress,
) -> FirstRouteGuideContent {
    let title = match locale {
        Locale::Es => "Primera ruta",
        Locale::En => "First route",
    };
    let objective = match locale {
        Locale::Es => "Objetivo: llevá carbón de la mina a la central eléctrica.",
        Locale::En => "Goal: move coal from the mine to the power station.",
    };
    let steps = [
        (progress.road_connected, FirstRouteStep::ConnectRoad),
        (progress.stops_and_depot, FirstRouteStep::StopsAndDepot),
        (progress.coal_truck_ready, FirstRouteStep::CoalTruck),
        (progress.orders_assigned, FirstRouteStep::Orders),
        (progress.vehicle_running, FirstRouteStep::StartVehicle),
        (progress.first_delivery, FirstRouteStep::FirstDelivery),
    ];
    let checklist = steps
        .iter()
        .enumerate()
        .map(|(index, (done, step))| {
            let mark = if *done { "✓" } else { "○" };
            format!("{mark} {}. {}", index + 1, step.instruction(locale))
        })
        .collect::<Vec<_>>()
        .join("\n");
    let next = match progress.next_step() {
        Some(step) => match locale {
            Locale::Es => format!("Siguiente: {}", step.instruction(locale)),
            Locale::En => format!("Next: {}", step.instruction(locale)),
        },
        None => match locale {
            Locale::Es => {
                "✓ Primera entrega lograda: la central recibió carbón y cobraste.".to_owned()
            }
            Locale::En => {
                "✓ First delivery complete: the power station received coal and you were paid."
                    .to_owned()
            }
        },
    };

    FirstRouteGuideContent {
        title,
        objective,
        checklist,
        next,
    }
}

/// Calcula la guía sólo para el fixture de «Primera ruta».
///
/// La identificación evita usar el año, porque una partida guardada puede
/// avanzar el calendario antes de volver a abrirse. Las etapas se verifican
/// contra el mapa, estaciones, órdenes, vehículo e ingresos reales.
#[must_use]
pub(crate) fn first_route_progress(state: &GameState) -> Option<FirstRouteProgress> {
    let (coal_mine, power_station) = first_route_industries(state)?;
    let road_connected = road_connects_industries(state, coal_mine, power_station);
    let mine_stops = truck_stops_covering(state, coal_mine);
    let power_stops = truck_stops_covering(state, power_station);
    let stops_and_depot =
        !mine_stops.is_empty() && !power_stops.is_empty() && has_road_depot(state);

    let coal_truck_ready = state.vehicles.iter().any(is_coal_truck);
    let orders_assigned = state.vehicles.iter().any(|vehicle| {
        is_coal_truck(vehicle)
            && vehicle_visits_any_stop(vehicle, &mine_stops)
            && vehicle_visits_any_stop(vehicle, &power_stops)
    });
    let vehicle_running = state.vehicles.iter().any(|vehicle| {
        is_coal_truck(vehicle)
            && vehicle.running
            && vehicle_visits_any_stop(vehicle, &mine_stops)
            && vehicle_visits_any_stop(vehicle, &power_stops)
    });
    let first_delivery = power_station.was_cargo_delivered
        && power_station.last_accepted_date(CargoType::Coal) > 0
        && state.stats.cargo_income_earned > 0
        && state
            .companies
            .iter()
            .any(|company| company.id == state.active_company && company.cargo_income_earned > 0);

    Some(FirstRouteProgress {
        road_connected,
        stops_and_depot,
        coal_truck_ready,
        orders_assigned,
        vehicle_running,
        first_delivery,
    })
}

fn first_route_industries(state: &GameState) -> Option<(&Industry, &Industry)> {
    if state.map.dimensions() != (64, 64)
        || state.world_seed != FIRST_ROUTE_WORLD_SEED
        || state.climate != Climate::Temperate
    {
        return None;
    }
    let coal_mine = state.industries.iter().find(|industry| {
        industry.pos == FIRST_ROUTE_COAL_MINE && industry.spec == Some(IndustrySpec::CoalMine)
    })?;
    let power_station = state.industries.iter().find(|industry| {
        industry.pos == FIRST_ROUTE_POWER_STATION
            && industry.spec == Some(IndustrySpec::PowerStation)
    })?;
    Some((coal_mine, power_station))
}

/// Encuentra teselas de carretera dentro del radio donde una parada puede
/// servir a cada industria. Así la etapa de carretera no depende de colocar
/// antes las paradas, ni obliga a usar las coordenadas de la ruta de referencia.
fn road_connects_industries(
    state: &GameState,
    coal_mine: &Industry,
    power_station: &Industry,
) -> bool {
    let mine_roads = road_tiles_serving_industry(state, coal_mine);
    let power_roads = road_tiles_serving_industry(state, power_station);
    mine_roads.iter().any(|from| {
        power_roads
            .iter()
            .any(|to| find_path(&state.map, *from, *to, PathNetwork::Road).is_some())
    })
}

fn road_tiles_serving_industry(state: &GameState, industry: &Industry) -> Vec<TileCoord> {
    let (width, height) = state.map.dimensions();
    let mut roads = Vec::new();
    for y in 0..height {
        for x in 0..width {
            let coord = TileCoord::new(x.cast_signed(), y.cast_signed());
            if is_road_network_tile(state, coord)
                && industry_in_station_coverage(industry, coord, STATION_COVERAGE_RADIUS)
            {
                roads.push(coord);
            }
        }
    }
    roads
}

fn is_road_network_tile(state: &GameState, coord: TileCoord) -> bool {
    matches!(
        state.map.get_kind(coord),
        Some(TileKind::Road | TileKind::RoadDepot | TileKind::RoadTunnel | TileKind::RoadBridge)
    )
}

fn truck_stops_covering(state: &GameState, industry: &Industry) -> Vec<TileCoord> {
    state
        .stations
        .iter()
        .filter(|station| {
            station.stop_kind == StopKind::TruckStop
                && industry_in_station_coverage(
                    industry,
                    station.pos,
                    station_catchment_radius(station),
                )
        })
        .map(|station| station.pos)
        .collect()
}

fn has_road_depot(state: &GameState) -> bool {
    let (width, height) = state.map.dimensions();
    (0..height).any(|y| {
        (0..width).any(|x| {
            state
                .map
                .get_kind(TileCoord::new(x.cast_signed(), y.cast_signed()))
                == Some(TileKind::RoadDepot)
        })
    })
}

fn is_coal_truck(vehicle: &Vehicle) -> bool {
    vehicle.kind == VehicleKind::Truck && vehicle.cargo_type == Some(CargoType::Coal)
}

fn vehicle_visits_any_stop(vehicle: &Vehicle, stops: &[TileCoord]) -> bool {
    vehicle.orders.iter().any(
        |order| matches!(order, VehicleOrder::Station { station, .. } if stops.contains(station)),
    )
}

#[derive(Component)]
pub(crate) struct FirstRouteGuideRoot;

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FirstRouteGuideText {
    Title,
    Objective,
    Checklist,
    Next,
}

/// Crea el panel una vez por sesión InGame. Arranca oculto porque una carga
/// normal también puede entrar a InGame; `sync_first_route_guide` decide si el
/// estado cargado es el escenario objetivo.
pub(crate) fn setup_first_route_guide(mut commands: Commands, asset_server: Res<AssetServer>) {
    let asset_server = &*asset_server;
    commands
        .spawn((
            InGameUi,
            FirstRouteGuideRoot,
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(GUIDE_TOP),
                right: Val::Px(GUIDE_RIGHT),
                width: Val::Px(GUIDE_WIDTH),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(7.0),
                padding: UiRect::all(Val::Px(12.0)),
                border: UiRect::all(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(PANEL_BG),
            BorderColor::all(PANEL_BORDER),
            FocusPolicy::Pass,
            GlobalZIndex(GUIDE_Z),
            Visibility::Hidden,
        ))
        .with_children(|panel| {
            panel.spawn((
                FirstRouteGuideText::Title,
                Text::new(""),
                window_text_font(asset_server, UiFontRole::Title),
                TextColor(TITLE),
                Node {
                    width: Val::Percent(100.0),
                    ..default()
                },
            ));
            panel.spawn((
                FirstRouteGuideText::Objective,
                Text::new(""),
                window_text_font(asset_server, UiFontRole::Body),
                TextColor(BODY),
                Node {
                    width: Val::Percent(100.0),
                    ..default()
                },
            ));
            panel.spawn((
                FirstRouteGuideText::Checklist,
                Text::new(""),
                window_text_font(asset_server, UiFontRole::Body),
                TextColor(BODY),
                Node {
                    width: Val::Percent(100.0),
                    ..default()
                },
            ));
            panel.spawn((
                FirstRouteGuideText::Next,
                Text::new(""),
                window_text_font(asset_server, UiFontRole::Body),
                TextColor(NEXT),
                Node {
                    width: Val::Percent(100.0),
                    margin: UiRect::top(Val::Px(3.0)),
                    ..default()
                },
            ));
        });
}

/// Sincroniza contenido y visibilidad desde el mundo real, no desde eventos
/// de UI. Esto también cubre el primer frame posterior a cargar un JSON.
pub(crate) fn sync_first_route_guide(
    sim: Option<Res<SimWorld>>,
    prefs: Res<ClientPreferences>,
    mut roots: Query<&mut Visibility, With<FirstRouteGuideRoot>>,
    mut texts: Query<(&FirstRouteGuideText, &mut Text)>,
) {
    let progress = sim
        .as_deref()
        .and_then(|sim| first_route_progress(&sim.state));
    for mut visibility in &mut roots {
        *visibility = if progress.is_some() {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
    let Some(progress) = progress else {
        return;
    };
    let content = first_route_guide_content(prefs.locale(), progress);
    for (kind, mut text) in &mut texts {
        let value = match kind {
            FirstRouteGuideText::Title => content.title,
            FirstRouteGuideText::Objective => content.objective,
            FirstRouteGuideText::Checklist => &content.checklist,
            FirstRouteGuideText::Next => &content.next,
        };
        if text.as_str() != value {
            *text = Text::new(value);
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use bevy::ecs::system::RunSystemOnce;
    use bevy::prelude::*;
    use openttdrs_core::command::{Command, apply_command};
    use openttdrs_core::parity::{
        FIRST_ROUTE_DELIVER_STOP, FIRST_ROUTE_DEPOT, FIRST_ROUTE_DEPOT_DIRECTION,
        FIRST_ROUTE_LOAD_STOP, FIRST_ROUTE_POWER_STATION, FIRST_ROUTE_ROAD_END_X,
        FIRST_ROUTE_ROAD_START_X, FIRST_ROUTE_ROAD_Y, FIRST_ROUTE_VEHICLE_ID, build_first_route,
    };
    use openttdrs_core::save::{load, save};
    use openttdrs_core::{CargoType, GameState, TileCoord, VehicleKind, VehicleOrder};

    use super::{
        FirstRouteGuideRoot, FirstRouteGuideText, FirstRouteStep, first_route_guide_content,
        first_route_progress, sync_first_route_guide,
    };
    use crate::settings::ClientPreferences;
    use crate::state::SimWorld;

    const MAX_ROUTE_TICKS: usize = 40_000;

    fn configured_first_route() -> GameState {
        let mut state = build_first_route();
        for x in FIRST_ROUTE_ROAD_START_X..=FIRST_ROUTE_ROAD_END_X {
            apply_command(
                &mut state,
                &Command::PlaceRoadBits(TileCoord::new(x, FIRST_ROUTE_ROAD_Y), 0x0A),
            )
            .expect("carretera de la primera ruta");
        }
        for command in [
            Command::PlaceRoadDepotDir(FIRST_ROUTE_DEPOT, FIRST_ROUTE_DEPOT_DIRECTION),
            Command::PlaceTruckStop(FIRST_ROUTE_LOAD_STOP, 1),
            Command::PlaceTruckStop(FIRST_ROUTE_DELIVER_STOP, 1),
            Command::BuildRoadVehicleAtDepot(FIRST_ROUTE_DEPOT, VehicleKind::Truck),
            Command::RefitVehicle {
                vehicle_id: FIRST_ROUTE_VEHICLE_ID,
                cargo: CargoType::Coal,
                unit_ids: Vec::new(),
            },
            Command::SetVehicleOrderList(
                FIRST_ROUTE_VEHICLE_ID,
                vec![
                    VehicleOrder::station_with_flags(FIRST_ROUTE_LOAD_STOP, true, false),
                    VehicleOrder::station(FIRST_ROUTE_DELIVER_STOP),
                ],
            ),
            Command::ToggleVehicleRunning(FIRST_ROUTE_VEHICLE_ID),
        ] {
            apply_command(&mut state, &command).expect("comando de la primera ruta");
        }
        state
    }

    #[test]
    fn first_route_progress_is_incomplete_then_complete_and_rebuilds_from_json() {
        let initial = first_route_progress(&build_first_route()).expect("fixture identificado");
        assert_eq!(
            initial.next_step(),
            Some(FirstRouteStep::ConnectRoad),
            "el fixture vacío empieza por construir la carretera"
        );
        assert!(!initial.road_connected);
        assert!(!initial.stops_and_depot);
        assert!(!initial.coal_truck_ready);
        assert!(!initial.orders_assigned);
        assert!(!initial.vehicle_running);
        assert!(!initial.first_delivery);

        let mut state = configured_first_route();
        let ready = first_route_progress(&state).expect("ruta configurada identificada");
        assert!(ready.road_connected);
        assert!(ready.stops_and_depot);
        assert!(ready.coal_truck_ready);
        assert!(ready.orders_assigned);
        assert!(ready.vehicle_running);
        assert!(!ready.first_delivery);
        assert_eq!(ready.next_step(), Some(FirstRouteStep::FirstDelivery));

        for _ in 0..MAX_ROUTE_TICKS {
            state.step();
            if first_route_progress(&state).is_some_and(|progress| progress.first_delivery) {
                break;
            }
        }
        let complete = first_route_progress(&state).expect("ruta terminada identificada");
        assert!(
            complete.first_delivery,
            "la central recibió carbón con ingreso"
        );
        assert_eq!(complete.next_step(), None);

        assert!(
            first_route_guide_content(crate::i18n::Locale::Es, complete)
                .next
                .contains("Primera entrega lograda"),
            "la UI sólo anuncia el éxito cuando el estado lo confirma"
        );

        let mut without_income = state.clone();
        without_income.stats.cargo_income_earned = 0;
        for company in &mut without_income.companies {
            if company.id == without_income.active_company {
                company.cargo_income_earned = 0;
            }
        }
        assert!(
            !first_route_progress(&without_income)
                .expect("fixture sin ingreso identificado")
                .first_delivery,
            "recibir carbón sin ingreso no completa el objetivo"
        );

        let mut without_coal_receipt = state.clone();
        without_coal_receipt
            .industries
            .iter_mut()
            .find(|industry| industry.pos == FIRST_ROUTE_POWER_STATION)
            .expect("central de la primera ruta")
            .was_cargo_delivered = false;
        assert!(
            !first_route_progress(&without_coal_receipt)
                .expect("fixture sin recepción identificado")
                .first_delivery,
            "un ingreso sin recepción de carbón en la central no completa el objetivo"
        );

        let directory = tempfile::tempdir().expect("directorio temporal");
        let path = directory.path().join("first-route.json");
        save(&state, &path).expect("guardar JSON");
        let loaded = load(&path).expect("cargar JSON");
        assert_eq!(
            first_route_progress(&loaded),
            Some(complete),
            "el avance se reconstruye desde el JSON, no desde UI"
        );
    }

    #[test]
    fn guide_is_visible_only_for_first_route_and_uses_the_selected_locale() {
        let mut world = World::new();
        world.insert_resource(SimWorld::first_route());
        world.insert_resource(ClientPreferences::default());
        let root = world.spawn((FirstRouteGuideRoot, Visibility::Hidden)).id();
        world.spawn((FirstRouteGuideText::Title, Text::new("")));
        world.spawn((FirstRouteGuideText::Objective, Text::new("")));
        world.spawn((FirstRouteGuideText::Checklist, Text::new("")));
        world.spawn((FirstRouteGuideText::Next, Text::new("")));

        world.run_system_once(sync_first_route_guide).unwrap();
        assert_eq!(world.get::<Visibility>(root), Some(&Visibility::Visible));
        let mut texts = world.query::<(&FirstRouteGuideText, &Text)>();
        assert!(texts.iter(&world).any(|(kind, text)| {
            *kind == FirstRouteGuideText::Next && text.as_str().starts_with("Siguiente:")
        }));

        world.resource_mut::<SimWorld>().state = GameState::new(64, 64);
        world.run_system_once(sync_first_route_guide).unwrap();
        assert_eq!(world.get::<Visibility>(root), Some(&Visibility::Hidden));

        let english = first_route_guide_content(
            crate::i18n::Locale::En,
            first_route_progress(&build_first_route()).expect("fixture identificado"),
        );
        assert!(english.objective.contains("move coal"));
        assert!(english.next.starts_with("Next:"));
    }
}
