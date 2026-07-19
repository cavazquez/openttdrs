//! Regresión con el save real `tests/fixtures/stationlist-test.sav` (SLV 211,
//! ORDR + ORDL.first). Comprueba mapa, entidades, órdenes y simulación breve.

#![allow(clippy::expect_used, clippy::cast_possible_truncation)]

use std::path::PathBuf;

use openttdrs_core::{GameState, SavVehicleKind, VehicleOrder, sav};

fn stationlist_sav_bytes() -> Vec<u8> {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.pop(); // crates/openttdrs-core
    path.pop(); // crates
    path.push("tests/fixtures/stationlist-test.sav");
    std::fs::read(&path).unwrap_or_else(|e| {
        panic!(
            "no se encontró {}: {e} (fixture de regresión OpenTTD)",
            path.display()
        )
    })
}

#[test]
fn loads_stationlist_test_sav() {
    let raw = stationlist_sav_bytes();
    let sav = sav::load(&raw).expect("cargar stationlist-test.sav");

    assert_eq!(sav.version, 211);
    assert_eq!(sav.map.dimensions(), (256, 256));
    assert!(!sav.stations.is_empty(), "lista de estaciones jugables");
    assert!(!sav.towns.is_empty());
    assert!(!sav.industries.is_empty());
    assert!(sav.money.is_some());
    assert!(sav.game_time.is_some(), "chunk DATE");
    assert!(
        !sav.vehicles.is_empty(),
        "al menos un vehículo cabeza de convoy"
    );

    let with_sav_orders = sav.vehicles.iter().filter(|v| !v.orders.is_empty()).count();
    assert!(
        with_sav_orders > 0,
        "ORDR/ORDL.first debería resolver órdenes (got {with_sav_orders})"
    );

    let state = GameState::from_sav_game(sav);
    assert!(state.tick.get() > 0, "tick importado desde DATE");
    let with_orders = state
        .vehicles
        .iter()
        .filter(|v| !v.orders.is_empty())
        .count();
    assert!(
        with_orders > 0,
        "GameState debería tener vehículos con órdenes jugables"
    );

    eprintln!(
        "stationlist: {} estaciones, {} vehículos ({} con órdenes), tick={}",
        state.stations.len(),
        state.vehicles.len(),
        with_orders,
        state.tick.get()
    );
}

#[test]
fn stationlist_depot_row_connects_to_rail() {
    use openttdrs_core::{PathNetwork, TileKind, pathfinder};

    let state = GameState::from_sav_game(sav::load(&stationlist_sav_bytes()).expect("load"));
    // Save real: vía (21,39) — huecos — depósito (24,39); tras import debe haber continuidad.
    assert_eq!(
        state.map.get_kind(openttdrs_core::TileCoord::new(22, 39)),
        Some(TileKind::Rail)
    );
    assert_eq!(
        state.map.get_kind(openttdrs_core::TileCoord::new(23, 39)),
        Some(TileKind::Rail)
    );
    let path = pathfinder::find_path(
        &state.map,
        openttdrs_core::TileCoord::new(24, 39),
        openttdrs_core::TileCoord::new(21, 39),
        PathNetwork::Rail,
    );
    assert!(
        path.is_some(),
        "depósito y vía colindante deberían quedar unidos"
    );
}

/// El save SLV 211 mezcla órdenes tren→parada de bus y redes sin ruta YAPF;
/// no es un oráculo de movimiento. La paridad de marcha está en
/// `pbs_openttd_oracle` / `pbs_dual_curve_oracle`.
#[test]
#[ignore = "fixture stationlist-test.sav: órdenes importadas sin ruta YAPF usable"]
fn stationlist_vehicles_move_with_imported_orders() {
    let raw = stationlist_sav_bytes();
    let mut state = GameState::from_sav_game(sav::load(&raw).expect("load"));

    let movers: Vec<usize> = state
        .vehicles
        .iter()
        .enumerate()
        .filter(|(_, v)| !v.orders.is_empty())
        .map(|(i, _)| i)
        .collect();
    assert!(
        !movers.is_empty(),
        "necesitamos al menos un vehículo con órdenes importadas"
    );

    let snapshots: Vec<_> = movers
        .iter()
        .map(|&i| {
            let v = &state.vehicles[i];
            (v.pos, v.progress)
        })
        .collect();

    for _ in 0..500 {
        state.step();
    }

    let any_moved =
        movers
            .iter()
            .zip(snapshots.iter())
            .any(|(idx, &(start_pos, start_progress))| {
                let v = &state.vehicles[*idx];
                v.pos != start_pos || v.progress != start_progress
            });
    assert!(
        any_moved,
        "al menos un vehículo con órdenes debería moverse en 500 ticks tras importar el save"
    );
}

#[test]
fn stationlist_orders_resolve_to_stations_or_waypoints() {
    let raw = stationlist_sav_bytes();
    let state = GameState::from_sav_game(sav::load(&raw).expect("load"));

    let mut goto_orders = 0usize;
    for v in &state.vehicles {
        for o in &v.orders {
            match o {
                VehicleOrder::Station { .. } | VehicleOrder::Waypoint { .. } => {
                    goto_orders += 1;
                }
                _ => {}
            }
        }
    }
    assert!(
        goto_orders > 0,
        "las órdenes goto deberían mapear a estación/waypoint del STNN"
    );

    let trains = state
        .vehicles
        .iter()
        .filter(|v| matches!(v.kind, openttdrs_core::VehicleKind::Train))
        .count();
    let road = state.vehicles.len().saturating_sub(trains);
    eprintln!("stationlist vehículos: {trains} trenes, {road} carretera");
    let _ = SavVehicleKind::Train;
}
