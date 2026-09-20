//! V1 #591: comandos de red no pueden mutar bienes de otra compañía.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use openttdrs_core::{
    Command, CompanyId, GameState, MAX_COMPANIES, TileCoord, TileKind, VehicleKind, VehicleOrder,
};
use openttdrs_net::apply_command_as_company;

const COMPANY_A: CompanyId = CompanyId::PLAYER;
const COMPANY_B: CompanyId = CompanyId(1);

fn apply_as(state: &mut GameState, issuer: CompanyId, command: &Command) {
    apply_command_as_company(state, issuer, command)
        .unwrap_or_else(|error| panic!("{issuer:?} debe poder aplicar {command:?}: {error}"));
}

fn company_money(state: &GameState, company: CompanyId) -> i64 {
    state.companies[company.index()].economy.money
}

fn assert_rejected_atomically(
    state: &mut GameState,
    issuer: CompanyId,
    command: &Command,
    expected_error: &str,
) {
    let before_json = state.save_json().unwrap();
    let before_hash = state.canonical_hash();
    let before_random = state.random;
    let before_interactive_random = state.interactive_random;
    let before_money = state
        .companies
        .iter()
        .map(|company| company.economy.money)
        .collect::<Vec<_>>();

    let error = apply_command_as_company(state, issuer, command).unwrap_err();
    assert_eq!(
        error, expected_error,
        "rechazo productivo esperado para {command:?}"
    );
    assert_eq!(
        state.save_json().unwrap(),
        before_json,
        "un rechazo no puede cambiar mapa, vehículos, órdenes ni dinero"
    );
    assert_eq!(
        state.canonical_hash(),
        before_hash,
        "un rechazo no puede cambiar el estado persistido"
    );
    assert_eq!(state.random, before_random, "RNG de simulación inalterado");
    assert_eq!(
        state.interactive_random, before_interactive_random,
        "RNG interactivo inalterado"
    );
    assert_eq!(
        state
            .companies
            .iter()
            .map(|company| company.economy.money)
            .collect::<Vec<_>>(),
        before_money,
        "ninguna compañía puede recibir un cargo en un rechazo"
    );
}

fn two_company_fixture() -> (GameState, TileCoord, TileCoord, u32) {
    let mut state = GameState::new(16, 16);
    let road = TileCoord::new(2, 2);
    let depot = TileCoord::new(3, 2);

    apply_as(&mut state, COMPANY_A, &Command::PlaceRoad(road));
    apply_as(&mut state, COMPANY_A, &Command::PlaceRoadDepotDir(depot, 0));
    apply_as(
        &mut state,
        COMPANY_A,
        &Command::BuildRoadVehicleAtDepot(depot, VehicleKind::Bus),
    );
    let vehicle_id = state.vehicles[0].id;
    assert_eq!(state.vehicles[0].owner, COMPANY_A);

    // El mismo API que procesa un Commit materializa la compañía B y le da
    // infraestructura propia; no se simula el issuer con active_company.
    apply_as(
        &mut state,
        COMPANY_B,
        &Command::PlaceRoad(TileCoord::new(2, 6)),
    );
    assert_eq!(state.companies.len(), 2);
    assert_eq!(state.map.get(TileCoord::new(2, 6)).unwrap().m1, COMPANY_B.0);
    assert_eq!(state.active_company, COMPANY_A);

    (state, road, depot, vehicle_id)
}

#[test]
fn v1_network_issuer_rejects_foreign_assets_atomically_and_allows_owner_controls() {
    let (mut state, road, depot, vehicle_id) = two_company_fixture();

    // B intenta los cuatro gestos acotados contra bienes de A. Cada rechazo
    // se compara contra el snapshot completo, hash y ambos RNGs.
    assert_rejected_atomically(
        &mut state,
        COMPANY_B,
        &Command::BuildRoadVehicleAtDepot(depot, VehicleKind::Bus),
        "TileNotOwned",
    );
    assert_rejected_atomically(
        &mut state,
        COMPANY_B,
        &Command::ToggleVehicleRunning(vehicle_id),
        "VehicleNotOwned",
    );
    assert_rejected_atomically(
        &mut state,
        COMPANY_B,
        &Command::SetVehicleOrders(vehicle_id, vec![TileCoord::new(7, 2)]),
        "VehicleNotOwned",
    );
    assert_rejected_atomically(
        &mut state,
        COMPANY_B,
        &Command::ClearTile(road),
        "TileNotOwned",
    );
    let invalid_issuer = CompanyId(MAX_COMPANIES);
    let invalid_error = format!("compañía fuera de rango: {}", invalid_issuer.0);
    assert_rejected_atomically(
        &mut state,
        invalid_issuer,
        &Command::ToggleVehicleRunning(vehicle_id),
        &invalid_error,
    );

    // Los mismos cuatro controles emitidos por A sí operan sobre A y nunca
    // cargan dinero a B.
    let b_money = company_money(&state, COMPANY_B);
    let a_money_before_purchase = company_money(&state, COMPANY_A);
    let vehicles_before_purchase = state.vehicles.len();
    apply_as(
        &mut state,
        COMPANY_A,
        &Command::BuildRoadVehicleAtDepot(depot, VehicleKind::Bus),
    );
    assert_eq!(state.vehicles.len(), vehicles_before_purchase + 1);
    assert_eq!(state.vehicles.last().unwrap().owner, COMPANY_A);
    assert!(company_money(&state, COMPANY_A) < a_money_before_purchase);
    assert_eq!(company_money(&state, COMPANY_B), b_money);

    apply_as(
        &mut state,
        COMPANY_A,
        &Command::ToggleVehicleRunning(vehicle_id),
    );
    assert!(
        state
            .vehicles
            .iter()
            .find(|vehicle| vehicle.id == vehicle_id)
            .unwrap()
            .running
    );
    assert_eq!(company_money(&state, COMPANY_B), b_money);

    let destination = TileCoord::new(7, 2);
    apply_as(
        &mut state,
        COMPANY_A,
        &Command::SetVehicleOrders(vehicle_id, vec![destination]),
    );
    assert_eq!(
        state
            .vehicles
            .iter()
            .find(|vehicle| vehicle.id == vehicle_id)
            .unwrap()
            .orders,
        vec![VehicleOrder::Tile(destination)]
    );
    assert_eq!(company_money(&state, COMPANY_B), b_money);

    let a_money_before_demolition = company_money(&state, COMPANY_A);
    apply_as(&mut state, COMPANY_A, &Command::ClearTile(road));
    assert_eq!(state.map.get_kind(road), Some(TileKind::Grass));
    assert_eq!(
        company_money(&state, COMPANY_A),
        a_money_before_demolition - 5
    );
    assert_eq!(company_money(&state, COMPANY_B), b_money);
}
