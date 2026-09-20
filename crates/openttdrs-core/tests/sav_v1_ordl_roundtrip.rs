//! Contrato V1-SAV (#588): una edición pública de ORDL debe sobrevivir al
//! re-guardado de OpenTTD sin afectar vehículos, estaciones ni otras órdenes.

#![allow(clippy::expect_used)]

use std::path::{Path, PathBuf};

use openttdrs_core::{Command, GameState, command::apply_command, sav, vehicle::VehicleOrder};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const V1_ENABLED: &str = "OPENTTDRS_V1_SAV_ORDL";
const EVIDENCE_SCHEMA: u8 = 1;
const MVP_RICH_SHA256: &str = "9461411d07d4c4b770a7e79ea599465db26820c5930592191a4eb584a31383de";
const TARGET_VEHICLE_ID: u32 = 0;
const TARGET_ORDER_INDEX: usize = 0;
const TARGET_STATION: (i32, i32) = (28, 39);

#[derive(Debug)]
struct V1Paths {
    input: PathBuf,
    edited: PathBuf,
    evidence: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct OrderEvidence {
    kind: String,
    destination: (i32, i32),
    /// Representación serializada completa: incluye los flags de carga,
    /// descarga, non-stop, timetable y refit cuando apliquen.
    encoded: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct VehicleEvidence {
    vehicle_id: u32,
    kind: String,
    /// `true` acredita que el vehículo sigue referido desde ORDL, aunque el
    /// índice interno del pool se renumere al guardar.
    shared_order_linked: bool,
    /// Miembros del mismo pool ORDL, ordenados por ID lógico de vehículo.
    shared_order_members: Vec<u32>,
    orders: Vec<OrderEvidence>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct StateEvidence {
    vehicles: Vec<VehicleEvidence>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct TargetEvidence {
    vehicle_id: u32,
    shared_id_before_save: u32,
    order_index: usize,
    shared_order_members: Vec<u32>,
    before: OrderEvidence,
    after: OrderEvidence,
}

#[derive(Debug, Serialize, Deserialize)]
struct Evidence {
    schema: u8,
    source_git_sha: String,
    input_sha256: String,
    edited_sha256: String,
    vehicle_ids: Vec<u32>,
    station_ids: Vec<u32>,
    target: TargetEvidence,
    before: StateEvidence,
    expected_after: StateEvidence,
}

fn enabled_paths() -> Option<V1Paths> {
    if std::env::var(V1_ENABLED).as_deref() != Ok("1") {
        return None;
    }
    Some(V1Paths {
        input: PathBuf::from(
            std::env::var("OPENTTDRS_V1_SAV_ORDL_INPUT")
                .expect("OPENTTDRS_V1_SAV_ORDL_INPUT requerido"),
        ),
        edited: PathBuf::from(
            std::env::var("OPENTTDRS_V1_SAV_ORDL_EDITED")
                .expect("OPENTTDRS_V1_SAV_ORDL_EDITED requerido"),
        ),
        evidence: PathBuf::from(
            std::env::var("OPENTTDRS_V1_SAV_ORDL_EVIDENCE")
                .expect("OPENTTDRS_V1_SAV_ORDL_EVIDENCE requerido"),
        ),
    })
}

fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn sorted_ids<I>(ids: I) -> Vec<u32>
where
    I: IntoIterator<Item = u32>,
{
    let mut ids: Vec<_> = ids.into_iter().collect();
    ids.sort_unstable();
    ids
}

fn order_evidence(order: VehicleOrder) -> OrderEvidence {
    let kind = match order {
        VehicleOrder::Station { .. } => "station",
        VehicleOrder::Waypoint { .. } => "waypoint",
        VehicleOrder::Depot { .. } => "depot",
        VehicleOrder::Tile(_) => "tile",
        VehicleOrder::Conditional { .. } => "conditional",
    };
    let destination = order.destination();
    OrderEvidence {
        kind: kind.into(),
        destination: (destination.x, destination.y),
        encoded: serde_json::to_value(order).expect("serializar orden para evidencia"),
    }
}

fn shared_members(state: &GameState, vehicle_id: u32, shared_id: Option<u32>) -> Vec<u32> {
    let mut members = match shared_id {
        Some(shared_id) => state
            .vehicles
            .iter()
            .filter(|vehicle| vehicle.shared_order_id == Some(shared_id))
            .map(|vehicle| vehicle.id)
            .collect(),
        None => vec![vehicle_id],
    };
    members.sort_unstable();
    members
}

fn state_evidence(state: &GameState) -> StateEvidence {
    let mut vehicles: Vec<_> = state
        .vehicles
        .iter()
        .map(|vehicle| VehicleEvidence {
            vehicle_id: vehicle.id,
            kind: format!("{:?}", vehicle.kind),
            shared_order_linked: vehicle.shared_order_id.is_some(),
            shared_order_members: shared_members(state, vehicle.id, vehicle.shared_order_id),
            orders: vehicle.orders.iter().copied().map(order_evidence).collect(),
        })
        .collect();
    vehicles.sort_by_key(|vehicle| vehicle.vehicle_id);
    StateEvidence { vehicles }
}

fn vehicle_evidence(state: &StateEvidence, vehicle_id: u32) -> &VehicleEvidence {
    state
        .vehicles
        .iter()
        .find(|vehicle| vehicle.vehicle_id == vehicle_id)
        .unwrap_or_else(|| panic!("vehículo {vehicle_id} ausente de evidencia"))
}

fn choose_station_order(state: &GameState) -> (u32, u32, usize, VehicleOrder) {
    let mut candidates = Vec::new();
    for list in &state.shared_order_lists {
        let Some(vehicle_id) = state
            .vehicles
            .iter()
            .filter(|vehicle| vehicle.shared_order_id == Some(list.id))
            .map(|vehicle| vehicle.id)
            .min()
        else {
            continue;
        };
        for (index, order) in list.orders.iter().copied().enumerate() {
            if matches!(order, VehicleOrder::Station { .. }) {
                candidates.push((list.id, vehicle_id, index, order));
            }
        }
    }
    candidates.sort_by_key(|(shared_id, vehicle_id, index, _)| (*shared_id, *vehicle_id, *index));
    let (shared_id, vehicle_id, index, order) = candidates
        .into_iter()
        .next()
        .expect("mvp_openttd_rich.sav debe aportar una orden Station enlazada a ORDL para V1-SAV");
    (vehicle_id, shared_id, index, order)
}

fn assert_only_declared_order_changed(
    before: &StateEvidence,
    expected_after: &StateEvidence,
    target: &TargetEvidence,
) {
    assert_eq!(
        before.vehicles.len(),
        expected_after.vehicles.len(),
        "la edición no agrega ni quita vehículos"
    );
    let previous_target = vehicle_evidence(before, target.vehicle_id);
    let changed_target = vehicle_evidence(expected_after, target.vehicle_id);
    assert!(previous_target.shared_order_linked, "ORDL inicial ausente");
    assert!(changed_target.shared_order_linked, "ORDL editado ausente");
    assert_eq!(
        changed_target.shared_order_members, target.shared_order_members,
        "la lista ORDL de destino conserva sus vehículos"
    );
    assert_eq!(
        previous_target.orders[target.order_index], target.before,
        "la evidencia inicial apunta a la orden elegida"
    );
    assert_eq!(
        changed_target.orders[target.order_index], target.after,
        "la evidencia posterior apunta a la orden editada"
    );
    assert_eq!(target.before.kind, "station");
    assert_eq!(target.after.kind, "station");
    assert_eq!(
        target.before.destination, target.after.destination,
        "la edición de flags no cambia el destino de estación"
    );
    assert_ne!(
        target.before.encoded, target.after.encoded,
        "la orden debe cambiar flags de forma observable"
    );
    assert_ne!(
        target.before.encoded["load_type"], target.after.encoded["load_type"],
        "la operación pública debe cambiar el tipo de carga"
    );

    for prior in &before.vehicles {
        let current = vehicle_evidence(expected_after, prior.vehicle_id);
        assert_eq!(
            prior.kind, current.kind,
            "kind de vehículo {}",
            prior.vehicle_id
        );
        assert_eq!(
            prior.shared_order_linked, current.shared_order_linked,
            "vínculo ORDL de vehículo {}",
            prior.vehicle_id
        );
        assert_eq!(
            prior.shared_order_members, current.shared_order_members,
            "miembros ORDL de vehículo {}",
            prior.vehicle_id
        );
        if target.shared_order_members.contains(&prior.vehicle_id) {
            assert_eq!(
                prior.orders.len(),
                current.orders.len(),
                "cantidad de órdenes compartidas de vehículo {}",
                prior.vehicle_id
            );
            for (index, (old_order, new_order)) in
                prior.orders.iter().zip(&current.orders).enumerate()
            {
                if index == target.order_index {
                    assert_eq!(new_order, &target.after, "orden editada compartida");
                } else {
                    assert_eq!(
                        old_order, new_order,
                        "orden compartida no editada de vehículo {} índice {index}",
                        prior.vehicle_id
                    );
                }
            }
        } else {
            assert_eq!(
                prior.orders, current.orders,
                "órdenes ajenas a ORDL no deben cambiar (vehículo {})",
                prior.vehicle_id
            );
        }
    }
}

fn write_evidence(path: &Path, evidence: &Evidence) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("crear directorio de evidencia ORDL");
    }
    let json = serde_json::to_vec_pretty(evidence).expect("serializar evidencia ORDL");
    std::fs::write(path, json).expect("escribir evidencia ORDL");
}

/// Prepara el SAV de entrada para el dedicated: la única mutación pasa por
/// `Command::SetSharedOrderAt`, no por una edición directa del serializador.
#[test]
fn prepares_v1_edited_station_order_for_native_resave() {
    let Some(paths) = enabled_paths() else {
        return;
    };
    let input = std::fs::read(&paths.input).expect("leer fixture V1-SAV");
    assert_eq!(
        sha256(&input),
        MVP_RICH_SHA256,
        "el contrato V1-SAV fija el fixture mvp_openttd_rich.sav"
    );
    let source_game = sav::load(&input).expect("importar fixture V1-SAV");
    let vehicle_ids = sorted_ids(source_game.vehicles.iter().map(|vehicle| vehicle.sav_id));
    let station_ids = sorted_ids(
        source_game
            .stations
            .iter()
            .map(|station| station.station_id),
    );
    let mut state = GameState::from_sav_game(source_game);
    let before = state_evidence(&state);
    let (vehicle_id, shared_id, order_index, original_order) = choose_station_order(&state);
    assert_eq!(vehicle_id, TARGET_VEHICLE_ID, "vehículo V1-SAV fijado");
    assert_eq!(order_index, TARGET_ORDER_INDEX, "índice ORDL V1-SAV fijado");
    let original_evidence = order_evidence(original_order);
    assert_eq!(original_evidence.kind, "station");
    assert_eq!(original_evidence.destination, TARGET_STATION);
    assert_eq!(
        original_evidence.encoded["load_type"], "load_if_possible",
        "el punto de partida V1-SAV está fijado"
    );
    let updated_order = original_order
        .with_toggled_full_load()
        .expect("la orden seleccionada es Station");
    let updated_evidence = order_evidence(updated_order);
    assert_eq!(updated_evidence.destination, TARGET_STATION);
    assert_eq!(updated_evidence.encoded["load_type"], "full_load");

    apply_command(
        &mut state,
        &Command::SetSharedOrderAt {
            shared_id,
            index: order_index,
            order: updated_order,
        },
    )
    .expect("edición pública de ORDL");

    let expected_after = state_evidence(&state);
    let target_after = vehicle_evidence(&expected_after, vehicle_id);
    let target = TargetEvidence {
        vehicle_id,
        shared_id_before_save: shared_id,
        order_index,
        shared_order_members: target_after.shared_order_members.clone(),
        before: original_evidence,
        after: updated_evidence,
    };
    assert_only_declared_order_changed(&before, &expected_after, &target);

    if let Some(parent) = paths.edited.parent() {
        std::fs::create_dir_all(parent).expect("crear directorio de SAV editado");
    }
    sav::write::save(&state, &paths.edited).expect("exportar SAV ORDL editado");
    let edited = std::fs::read(&paths.edited).expect("leer SAV ORDL editado");
    let edited_game = sav::load(&edited).expect("reimportar SAV ORDL editado");
    assert_eq!(
        sorted_ids(edited_game.vehicles.iter().map(|vehicle| vehicle.sav_id)),
        vehicle_ids,
        "la exportación propia no agrega ni quita vehículos"
    );
    assert_eq!(
        sorted_ids(
            edited_game
                .stations
                .iter()
                .map(|station| station.station_id)
        ),
        station_ids,
        "la exportación propia no agrega ni quita estaciones"
    );

    write_evidence(
        &paths.evidence,
        &Evidence {
            schema: EVIDENCE_SCHEMA,
            source_git_sha: std::env::var("OPENTTDRS_V1_CANDIDATE_SHA")
                .unwrap_or_else(|_| "local-unpinned".into()),
            input_sha256: sha256(&input),
            edited_sha256: sha256(&edited),
            vehicle_ids,
            station_ids,
            target,
            before,
            expected_after,
        },
    );
}

/// Comprueba el output del dedicated contra el manifiesto generado antes de
/// abrir OpenTTD. Si falta output/evidencia, el modo V1 falla en vez de omitir.
#[test]
fn native_resave_preserves_v1_edited_station_order() {
    let Some(paths) = enabled_paths() else {
        return;
    };
    let evidence: Evidence =
        serde_json::from_slice(&std::fs::read(&paths.evidence).expect("leer evidencia V1-SAV"))
            .expect("parsear evidencia V1-SAV");
    assert_eq!(evidence.schema, EVIDENCE_SCHEMA, "schema de evidencia");
    assert_ne!(
        evidence.source_git_sha, "local-unpinned",
        "la corrida obligatoria debe quedar fijada a un SHA candidato"
    );
    assert!(
        evidence.source_git_sha.len() == 40
            && evidence
                .source_git_sha
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit()),
        "el candidato debe usar un SHA Git completo: {}",
        evidence.source_git_sha
    );
    let input = std::fs::read(&paths.input).expect("leer fixture original ORDL");
    assert_eq!(
        sha256(&input),
        evidence.input_sha256,
        "el manifiesto debe fijar el fixture original"
    );
    assert_eq!(evidence.input_sha256, MVP_RICH_SHA256);
    let edited = std::fs::read(&paths.edited).expect("leer input ORDL editado");
    assert_eq!(
        sha256(&edited),
        evidence.edited_sha256,
        "el native resave debe partir del input certificado"
    );

    let native_path = std::env::var("OPENTTDRS_ROUNDTRIP_SAV")
        .expect("OPENTTDRS_ROUNDTRIP_SAV requerido tras el dedicated");
    let native = std::fs::read(&native_path).expect("leer SAV re-guardado por OpenTTD");
    let native_game = sav::load(&native).expect("importar SAV re-guardado por OpenTTD");
    assert_eq!(
        sorted_ids(native_game.vehicles.iter().map(|vehicle| vehicle.sav_id)),
        evidence.vehicle_ids,
        "OpenTTD no agrega ni quita vehículos"
    );
    assert_eq!(
        sorted_ids(
            native_game
                .stations
                .iter()
                .map(|station| station.station_id)
        ),
        evidence.station_ids,
        "OpenTTD no agrega ni quita estaciones"
    );
    let observed = state_evidence(&GameState::from_sav_game(native_game));
    assert_eq!(
        observed, evidence.expected_after,
        "OpenTTD debe preservar exactamente el orden, tipo, destino, flags y enlace ORDL"
    );
    assert_only_declared_order_changed(&evidence.before, &observed, &evidence.target);
}
