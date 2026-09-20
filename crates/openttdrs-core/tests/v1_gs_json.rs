//! V1 #599: objetivo GS-lite de carbón y noticia única tras JSON.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use openttdrs_core::parity::{TRUCK_BAY_LOAD_STOP, TRUCK_BAY_VEHICLE_ID, build_truck_bay};
use openttdrs_core::{
    CargoType, Command, CompanyId, GameState, GsGoal, GsGoalKind, GsState, Industry, IndustryKind,
    IndustrySpec, OrderLoadType, OrderNonStop, OrderUnloadType, TileCoord, VehicleOrder,
    apply_command,
};
use serde_json::Value;

const GOAL_ID: u32 = 599;
const GOAL_UNITS: u64 = 10;
const FIRST_DELIVERY_UNITS: u64 = 5;
const CONTINUATION_TICKS: usize = 2_000;
const DELIVERY_TIMEOUT_TICKS: usize = 8_000;
const SHORT_DELIVER_STOP: TileCoord = TileCoord::new(8, 5);
const POWER_STATION: TileCoord = TileCoord::new(11, 3);
const COMPLETION_HEADLINE: &str = "Objetivos del escenario cumplidos";

#[derive(Debug, Clone, PartialEq, Eq)]
struct GsJsonReport {
    source_sha: String,
    save_tick: u64,
    completion_tick: u64,
    recipient: CompanyId,
    delivered_at_save: u64,
    delivered_at_completion: u64,
    continuation_ticks: usize,
    completion_news: usize,
    canonical_hash: u64,
}

fn source_sha() -> String {
    std::env::var("OPENTTDRS_SOURCE_SHA")
        .or_else(|_| std::env::var("GITHUB_SHA"))
        .unwrap_or_else(|_| "local-worktree".into())
}

fn coal_delivered(state: &GameState, company: CompanyId) -> u64 {
    state.companies[company.index()]
        .cargo_units_delivered_by_type
        .units_for(CargoType::Coal)
}

fn goal(state: &GameState) -> &GsGoal {
    state
        .gs
        .goals
        .iter()
        .find(|goal| goal.id == GOAL_ID)
        .expect("meta GS V1")
}

fn completion_news_count(state: &GameState) -> usize {
    state
        .news
        .items
        .iter()
        .filter(|item| item.headline == COMPLETION_HEADLINE)
        .count()
}

fn first_json_difference(left: &Value, right: &Value, path: &str) -> Option<String> {
    match (left, right) {
        (Value::Object(left), Value::Object(right)) => {
            for key in left.keys().chain(right.keys()) {
                let next_path = format!("{path}.{key}");
                match (left.get(key), right.get(key)) {
                    (Some(left), Some(right)) => {
                        if let Some(difference) = first_json_difference(left, right, &next_path) {
                            return Some(difference);
                        }
                    }
                    (left, right) => return Some(format!("{next_path}: {left:?} != {right:?}")),
                }
            }
            None
        }
        (Value::Array(left), Value::Array(right)) => {
            if left.len() != right.len() {
                return Some(format!("{path}.len: {} != {}", left.len(), right.len()));
            }
            for (index, (left, right)) in left.iter().zip(right).enumerate() {
                let next_path = format!("{path}[{index}]");
                if let Some(difference) = first_json_difference(left, right, &next_path) {
                    return Some(difference);
                }
            }
            None
        }
        _ if left != right => Some(format!("{path}: {left:?} != {right:?}")),
        _ => None,
    }
}

/// Reutiliza el recorrido corto `truck_bay`, pero convierte su camión en el
/// rival objetivo y limita cada viaje real a cinco unidades. La mina ya tiene
/// diez unidades; se desactiva su producción para que el 5/10 de guardado y
/// el 10/10 final no dependan de una generación posterior.
fn gs_fixture() -> (GameState, CompanyId) {
    let mut state = build_truck_bay();
    state.ensure_rival_transcargo();
    state.ai.enabled = false;
    let rival = state
        .companies
        .iter()
        .find(|company| company.is_ai)
        .expect("TransCargo rival")
        .id;
    assert_eq!(state.companies.len(), 2, "dos compañías en el fixture V1");
    assert!(state.set_active_company(rival));
    apply_command(&mut state, &Command::PlaceTruckStop(SHORT_DELIVER_STOP, 1))
        .expect("parada de descarga corta V1");

    for station in &mut state.stations {
        station.owner = rival;
    }
    state
        .vehicles
        .iter_mut()
        .find(|vehicle| vehicle.id == TRUCK_BAY_VEHICLE_ID)
        .expect("camión truck_bay")
        .owner = rival;
    let truck = state
        .vehicles
        .iter_mut()
        .find(|vehicle| vehicle.id == TRUCK_BAY_VEHICLE_ID)
        .expect("camión truck_bay");
    truck.cargo_type = Some(CargoType::Coal);
    truck.capacity = u32::try_from(FIRST_DELIVERY_UNITS).expect("capacidad V1");
    truck.refit_capacity = u16::try_from(FIRST_DELIVERY_UNITS).expect("capacidad refit V1");
    truck.set_vehicle_orders(vec![
        VehicleOrder::station_with_types(
            TRUCK_BAY_LOAD_STOP,
            OrderLoadType::LoadIfPossible,
            OrderUnloadType::NoUnload,
            OrderNonStop::NonStopDestination,
        ),
        VehicleOrder::station_with_types(
            SHORT_DELIVER_STOP,
            OrderLoadType::NoLoad,
            OrderUnloadType::UnloadIfPossible,
            OrderNonStop::NonStopDestination,
        ),
    ]);
    truck.sync_order_destination(&state.map);

    let mine = state.industries.first_mut().expect("mina truck_bay");
    assert_eq!(mine.kind, IndustryKind::CoalMine);
    mine.stock = u32::try_from(GOAL_UNITS).expect("stock V1");
    mine.newgrf_production_rate = Some(0);
    let power_station = Industry::with_tiles_spec(
        POWER_STATION,
        IndustryKind::Factory,
        IndustrySpec::PowerStation,
        vec![POWER_STATION],
        0,
    );
    assert!(power_station.accepts_cargo(CargoType::Coal));
    state.industries.push(power_station);

    state.gs = GsState {
        enabled: true,
        goals: vec![GsGoal {
            id: GOAL_ID,
            title: "Entregá 10 unidades de carbón".into(),
            progress_num: 0,
            progress_den: GOAL_UNITS,
            completed: false,
            kind: GsGoalKind::CargoDelivered {
                cargo: CargoType::Coal,
                min: GOAL_UNITS,
            },
        }],
        story_pages: Vec::new(),
        story_index: 0,
        all_complete: false,
        victory_news_sent: false,
        rival_goal_news_sent: Vec::new(),
    };
    (state, rival)
}

/// Avanza hasta una entrega física exacta. Mientras el vehículo carga o viaja,
/// la meta no puede adelantar el ledger de entregas finales.
fn advance_until_delivered(state: &mut GameState, recipient: CompanyId, expected: u64) {
    let mut saw_loaded_without_delivery = false;
    for _ in 0..DELIVERY_TIMEOUT_TICKS {
        let delivered_before = coal_delivered(state, recipient);
        state.step();
        let delivered_after = coal_delivered(state, recipient);
        let current_goal = goal(state);
        assert!(
            current_goal.progress_num <= delivered_after,
            "la meta no puede avanzar por una carga no entregada: progreso={} entregado={delivered_after}",
            current_goal.progress_num,
        );
        if delivered_after == delivered_before
            && state.vehicles.iter().any(|vehicle| {
                vehicle.owner == recipient
                    && vehicle
                        .cargo_packets
                        .packets
                        .iter()
                        .any(|packet| packet.cargo == CargoType::Coal && packet.count > 0)
            })
        {
            saw_loaded_without_delivery = true;
            assert_eq!(
                current_goal.progress_num, delivered_after,
                "cargar carbón no puede acreditar la meta antes de una entrega final"
            );
        }
        if delivered_after == expected && current_goal.progress_num == expected {
            assert!(
                saw_loaded_without_delivery,
                "el fixture debe observar carbón cargado antes de acreditarlo"
            );
            return;
        }
        assert!(
            delivered_after <= expected,
            "la capacidad fija debe entregar exactamente de a cinco, no {delivered_after}"
        );
    }
    panic!(
        "la compañía receptora no llegó a {expected} unidades de carbón en {DELIVERY_TIMEOUT_TICKS} ticks; entregó {}",
        coal_delivered(state, recipient)
    );
}

fn assert_same_v1_state(continuous: &GameState, restored: &GameState, recipient: CompanyId) {
    assert_eq!(continuous.active_company, recipient);
    assert_eq!(restored.active_company, recipient);
    assert_eq!(continuous.companies.len(), 2);
    assert_eq!(restored.companies.len(), 2);
    assert_eq!(
        coal_delivered(continuous, recipient),
        coal_delivered(restored, recipient)
    );
    assert_eq!(goal(continuous), goal(restored));
    assert_eq!(
        completion_news_count(continuous),
        completion_news_count(restored),
        "la rama JSON debe conservar la deduplicación de noticia"
    );
    let continuous_hash = continuous.canonical_hash();
    let restored_hash = restored.canonical_hash();
    assert_eq!(
        continuous_hash,
        restored_hash,
        "la rama JSON y la continua deben conservar el mismo estado persistido; primera diferencia: {:?}",
        first_json_difference(
            &serde_json::to_value(continuous).expect("serializar rama continua"),
            &serde_json::to_value(restored).expect("serializar rama JSON"),
            "$",
        )
    );
}

#[test]
fn v1_gs_cargo_goal_and_completion_news_survive_json() {
    let (mut continuous, recipient) = gs_fixture();
    advance_until_delivered(&mut continuous, recipient, FIRST_DELIVERY_UNITS);
    assert_eq!(coal_delivered(&continuous, CompanyId::PLAYER), 0);
    assert_eq!(continuous.active_company, recipient);
    assert_eq!(goal(&continuous).progress_num, FIRST_DELIVERY_UNITS);
    assert!(!goal(&continuous).completed);
    assert!(!continuous.gs.victory_news_sent);
    assert_eq!(completion_news_count(&continuous), 0);

    let save_tick = continuous.tick.get();
    let saved = continuous.save_json().expect("guardar progreso 5/10");
    let mut restored = GameState::load_json(&saved).expect("recargar progreso 5/10");
    assert_same_v1_state(&continuous, &restored, recipient);

    let mut completion_tick = None;
    for _ in 0..CONTINUATION_TICKS {
        continuous.step();
        restored.step();
        assert_same_v1_state(&continuous, &restored, recipient);
        if goal(&continuous).completed {
            completion_tick.get_or_insert(continuous.tick.get());
        }
    }

    let completion_tick = completion_tick.unwrap_or_else(|| {
        panic!(
            "la segunda entrega completa el objetivo: tick={} entregado={} camión={:?} espera={:?}",
            continuous.tick.get(),
            coal_delivered(&continuous, recipient),
            continuous
                .vehicles
                .iter()
                .find(|vehicle| vehicle.owner == recipient)
                .map(|vehicle| (
                    vehicle.pos,
                    vehicle.dest,
                    vehicle.current_order,
                    vehicle.cargo,
                    vehicle.cargo_packets.packets.clone(),
                )),
            continuous
                .stations
                .iter()
                .map(|station| (station.pos, station.cargo_stock.get(CargoType::Coal)))
                .collect::<Vec<_>>(),
        )
    });
    assert_eq!(coal_delivered(&continuous, recipient), GOAL_UNITS);
    assert_eq!(goal(&continuous).progress_num, GOAL_UNITS);
    assert!(goal(&continuous).completed);
    assert!(continuous.gs.all_complete);
    assert!(continuous.gs.victory_news_sent);
    assert_eq!(completion_news_count(&continuous), 1);

    let completed_json = restored.save_json().expect("guardar objetivo completado");
    let mut after_second_reload =
        GameState::load_json(&completed_json).expect("recargar objetivo completado");
    assert_same_v1_state(&restored, &after_second_reload, recipient);
    for _ in 0..16 {
        after_second_reload.step();
        assert_eq!(
            completion_news_count(&after_second_reload),
            1,
            "la segunda recarga no puede duplicar la noticia de finalización"
        );
    }

    let report = GsJsonReport {
        source_sha: source_sha(),
        save_tick,
        completion_tick,
        recipient,
        delivered_at_save: FIRST_DELIVERY_UNITS,
        delivered_at_completion: coal_delivered(&continuous, recipient),
        continuation_ticks: CONTINUATION_TICKS,
        completion_news: completion_news_count(&after_second_reload),
        canonical_hash: continuous.canonical_hash(),
    };
    println!("V1-GS JSON: {report:?}");
}
