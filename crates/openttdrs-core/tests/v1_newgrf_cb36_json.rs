//! V1 #592: CB36 de un camión NewGRF debe sobrevivir una recarga JSON.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use openttdrs_core::newgrf_actions::{ACTION0_FEATURE_ROAD_VEHICLES, apply_newgrf_vehicles_trains};
use openttdrs_core::newgrf_callback::effective_vehicle_max_speed_with_catalog;
use openttdrs_core::newgrf_config::{GrfContainerVersion, NewGrfEntry, scan_grf_bytes};
use openttdrs_core::newgrf_sprites::{
    build_action2_variational_payload, build_grf_v2_feature_with_action2_groups,
};
use openttdrs_core::parity::{
    FIRST_ROUTE_DELIVER_STOP, FIRST_ROUTE_DEPOT, FIRST_ROUTE_DEPOT_DIRECTION,
    FIRST_ROUTE_LOAD_STOP, FIRST_ROUTE_ROAD_END_X, FIRST_ROUTE_ROAD_START_X, FIRST_ROUTE_ROAD_Y,
    FIRST_ROUTE_VEHICLE_ID, build_scenario,
};
use openttdrs_core::{
    CargoType, Command, GameState, TileCoord, VehicleKind, VehicleOrder, apply_command,
};
use sha2::{Digest, Sha256};
use tempfile::TempDir;

const FIXTURE_FILENAME: &str = "v1-cb36-truck.grf";
const FIXTURE_GRFID_BYTES: [u8; 4] = *b"CB36";
const FIXTURE_GRFID: u32 = u32::from_be_bytes(FIXTURE_GRFID_BYTES);
const FIXTURE_GRF_VERSION: u8 = 7;
const LOCAL_ENGINE_ID: u8 = 0x2A;
const CALLBACK_DISPATCH_SET_ID: u8 = 0x0D;
const PROPERTY_DISPATCH_SET_ID: u8 = 0x0E;
const CB36_RESULT_SET_ID: u8 = 0x0F;
const ACTION0_MAX_SPEED: u8 = 91;
const CB36_MAX_SPEED: u8 = 37;
const PERSISTENT_REGISTER: u8 = 3;
const CERTIFICATION_TICKS: usize = 2_000;
const FIXTURE_SHA256: &str = "7a1d02111ed80cb702d25367f48abb49b470eda074ce2d48189c89318e80163b";

#[derive(Debug, Clone, PartialEq, Eq)]
struct RuntimeSnapshot {
    canonical_hash: u64,
    effective_max_speed: u16,
    cargo: u32,
    cargo_type: Option<CargoType>,
    orders: Vec<VehicleOrder>,
    persistent_registers: BTreeMap<u8, u32>,
}

fn action0_road_truck() -> Vec<u8> {
    vec![
        0x00,
        ACTION0_FEATURE_ROAD_VEHICLES,
        0x06,
        0x01,
        LOCAL_ENGINE_ID,
        0x0F,
        23,
        0x10,
        CargoType::Coal.cargo_id(),
        0x11,
        96,
        0x13,
        20,
        0x14,
        40,
        0x15,
        ACTION0_MAX_SPEED,
    ]
}

/// Action2 variational real: `\2psto` guarda CB36 en `7C[3]` y `\2rst`
/// devuelve ese mismo valor. No se construye ningún resolver en Rust.
fn cb36_action2_payload() -> Vec<u8> {
    vec![
        0x02,
        ACTION0_FEATURE_ROAD_VEHICLES,
        CB36_RESULT_SET_ID,
        0x81,
        0x1A,
        0x20,
        CB36_MAX_SPEED,
        0x10,
        0x1A,
        0x20,
        PERSISTENT_REGISTER,
        0x0F,
        0x1A,
        0x00,
        CB36_MAX_SPEED,
        0x00,
        0x00,
        0x00,
    ]
}

fn fixture_bytes() -> Vec<u8> {
    let action0 = action0_road_truck();
    let callback_dispatch = build_action2_variational_payload(
        ACTION0_FEATURE_ROAD_VEHICLES,
        CALLBACK_DISPATCH_SET_ID,
        0x0C,
        0,
        u8::MAX,
        &[(
            u16::from(PROPERTY_DISPATCH_SET_ID),
            openttdrs_core::CBID_VEHICLE_MODIFY_PROPERTY as u8,
            openttdrs_core::CBID_VEHICLE_MODIFY_PROPERTY as u8,
        )],
        0,
    );
    let property_dispatch = build_action2_variational_payload(
        ACTION0_FEATURE_ROAD_VEHICLES,
        PROPERTY_DISPATCH_SET_ID,
        0x10,
        0,
        u8::MAX,
        &[(u16::from(CB36_RESULT_SET_ID), 0x15, 0x15)],
        0,
    );
    let callback = cb36_action2_payload();
    build_grf_v2_feature_with_action2_groups(
        &action0,
        ACTION0_FEATURE_ROAD_VEHICLES,
        LOCAL_ENGINE_ID,
        CALLBACK_DISPATCH_SET_ID,
        &[&callback_dispatch, &property_dispatch, &callback],
        1,
        1,
        &[0],
        FIXTURE_GRFID_BYTES,
        "V1 CB36 truck",
    )
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn install_fixture() -> TempDir {
    let fixture = fixture_bytes();
    let info = scan_grf_bytes(&fixture).expect("fixture GRF V2 legible");
    assert_eq!(info.container, GrfContainerVersion::V2);
    assert_eq!(info.grfid, Some(FIXTURE_GRFID));
    assert_eq!(info.grf_version, Some(FIXTURE_GRF_VERSION));
    assert_eq!(info.name.as_deref(), Some("V1 CB36 truck"));
    assert_eq!(sha256_hex(&fixture), FIXTURE_SHA256);

    let root = tempfile::tempdir().expect("directorio temporal del GRF V1");
    fs::write(root.path().join(FIXTURE_FILENAME), fixture).expect("instalar GRF V1 temporal");
    root
}

fn add_fixture_to_stack(state: &mut GameState) {
    let mut entry = NewGrfEntry::new(FIXTURE_FILENAME, FIXTURE_GRFID);
    entry.name = "V1 CB36 truck".into();
    entry.description = "Public-domain V1 callback fixture".into();
    entry.grf_version = FIXTURE_GRF_VERSION;
    state.newgrf_stack.push(entry);
}

fn load_fixture_catalog(state: &mut GameState, search_dir: &Path) -> u16 {
    apply_newgrf_vehicles_trains(state, &[search_dir]);
    let engine = state
        .engine_catalog
        .iter()
        .find(|engine| engine.newgrf_grfid == FIXTURE_GRFID)
        .expect("Action0 del GRF debe crear un motor del catálogo");
    assert_eq!(engine.kind, VehicleKind::Truck);
    assert_eq!(engine.newgrf_local_id, u16::from(LOCAL_ENGINE_ID));
    assert_eq!(engine.newgrf_grf_version, FIXTURE_GRF_VERSION);
    assert_eq!(engine.max_speed, u16::from(ACTION0_MAX_SPEED));
    assert!(
        engine.newgrf_runtime.is_some(),
        "Action2/Action3 reales deben permanecer en el runtime del catálogo"
    );
    engine.id
}

fn configure_public_route(state: &mut GameState, engine_id: u16) {
    for x in FIRST_ROUTE_ROAD_START_X..=FIRST_ROUTE_ROAD_END_X {
        apply_command(
            state,
            &Command::PlaceRoadBits(TileCoord::new(x, FIRST_ROUTE_ROAD_Y), 0x0A),
        )
        .expect("carretera pública V1");
    }
    for command in [
        Command::PlaceRoadDepotDir(FIRST_ROUTE_DEPOT, FIRST_ROUTE_DEPOT_DIRECTION),
        Command::PlaceTruckStop(FIRST_ROUTE_LOAD_STOP, 1),
        Command::PlaceTruckStop(FIRST_ROUTE_DELIVER_STOP, 1),
        Command::BuildVehicleAtDepot(FIRST_ROUTE_DEPOT, engine_id),
        Command::RefitVehicle {
            vehicle_id: FIRST_ROUTE_VEHICLE_ID,
            cargo: CargoType::Coal,
            unit_ids: Vec::new(),
        },
        Command::SetVehicleOrderList(
            FIRST_ROUTE_VEHICLE_ID,
            vec![
                VehicleOrder::station(FIRST_ROUTE_LOAD_STOP),
                VehicleOrder::station(FIRST_ROUTE_DELIVER_STOP),
            ],
        ),
        Command::ToggleVehicleRunning(FIRST_ROUTE_VEHICLE_ID),
    ] {
        apply_command(state, &command).expect("comando público de la ruta CB36");
    }
}

fn effective_speed_and_snapshot(state: &mut GameState) -> RuntimeSnapshot {
    let vehicle_index = state
        .vehicles
        .iter()
        .position(|vehicle| vehicle.id == FIRST_ROUTE_VEHICLE_ID)
        .expect("camión NewGRF comprado por comando público");
    let effective_max_speed = effective_vehicle_max_speed_with_catalog(
        &state.engine_catalog,
        &mut state.vehicles[vehicle_index],
    );
    let vehicle = &state.vehicles[vehicle_index];
    RuntimeSnapshot {
        canonical_hash: state.canonical_hash(),
        effective_max_speed,
        cargo: vehicle.cargo,
        cargo_type: vehicle.cargo_type,
        orders: vehicle.orders.clone(),
        persistent_registers: vehicle
            .newgrf_persistent_regs
            .iter()
            .map(|(&register, &value)| (register, value))
            .collect(),
    }
}

#[test]
fn v1_cb36_road_truck_keeps_runtime_after_json_reload() {
    let fixture_root = install_fixture();
    let mut continuous = build_scenario("first_route").expect("escenario V1 de carretera");
    add_fixture_to_stack(&mut continuous);
    let engine_id = load_fixture_catalog(&mut continuous, fixture_root.path());
    configure_public_route(&mut continuous, engine_id);

    let serialized = continuous.save_json().expect("guardar ruta NewGRF a JSON");
    let mut reloaded = GameState::load_json(&serialized).expect("recargar ruta NewGRF desde JSON");
    assert_eq!(
        load_fixture_catalog(&mut reloaded, fixture_root.path()),
        engine_id,
        "la recarga debe reconstruir el catálogo desde el GRF instalado"
    );

    let initial_continuous = effective_speed_and_snapshot(&mut continuous);
    let initial_reloaded = effective_speed_and_snapshot(&mut reloaded);
    assert_eq!(
        initial_continuous.effective_max_speed,
        u16::from(CB36_MAX_SPEED)
    );
    assert_ne!(
        initial_continuous.effective_max_speed,
        u16::from(ACTION0_MAX_SPEED),
        "CB36 debe diferir de la velocidad Action0 del camión"
    );
    assert_eq!(
        initial_continuous
            .persistent_registers
            .get(&PERSISTENT_REGISTER),
        Some(&u32::from(CB36_MAX_SPEED)),
        "la evaluación real de CB36 debe dejar el registro 7C verificable"
    );
    assert_eq!(initial_continuous, initial_reloaded);

    for tick in 1..=CERTIFICATION_TICKS {
        continuous.step();
        reloaded.step();
        assert_eq!(
            effective_speed_and_snapshot(&mut continuous),
            effective_speed_and_snapshot(&mut reloaded),
            "la ruta CB36 diverge tras JSON en el tick {tick}"
        );
    }

    let vehicle = continuous
        .vehicles
        .iter()
        .find(|vehicle| vehicle.id == FIRST_ROUTE_VEHICLE_ID)
        .expect("camión V1 aún presente");
    assert!(vehicle.running);
    assert_eq!(vehicle.kind, VehicleKind::Truck);
    assert_eq!(vehicle.orders.len(), 2);
}
