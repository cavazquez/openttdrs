//! Prueba saves con más vía que `stationlist-test.sav` (sintético y partida real opcional).

#![allow(clippy::expect_used)]

use std::path::{Path, PathBuf};

use openttdrs_core::{GameState, TileKind, VehicleKind, sav};

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

fn workspace_save_path(rel: &str) -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.pop();
    path.pop();
    path.push(rel);
    path
}

fn read_bytes(path: &Path, label: &str) -> Vec<u8> {
    std::fs::read(path)
        .unwrap_or_else(|e| panic!("no se encontró {label} ({}): {e}", path.display()))
}

fn count_rail(map: &openttdrs_core::Map) -> usize {
    let (w, h) = map.dimensions();
    let mut n = 0usize;
    for y in 0..h as i32 {
        for x in 0..w as i32 {
            let k = map.get_kind(openttdrs_core::TileCoord::new(x, y));
            if matches!(
                k,
                Some(
                    TileKind::Rail
                        | TileKind::RailDepot
                        | TileKind::RailTunnel
                        | TileKind::RailBridge
                )
            ) {
                n += 1;
            }
        }
    }
    n
}

#[test]
fn demo_openttd_sav_has_train_orders_and_rail() {
    // #226: VEHS export oficial es tren-only (ROAD omitido). El smoke de
    // movimiento ya no depende de un bus; verificamos forma + órdenes.
    let path = fixture_path("demo_openttd.sav");
    let raw = read_bytes(&path, "demo_openttd.sav");
    let sav = sav::load(&raw).expect("demo load");
    let rail = count_rail(&sav.map);
    assert!(rail >= 30, "demo: vía insuficiente ({rail})");
    assert!(
        sav.vehicles
            .iter()
            .any(|v| v.kind == sav::SavVehicleKind::Train && !v.orders.is_empty()),
        "demo: falta tren con órdenes"
    );
    let state = GameState::from_sav_game(sav);
    assert!(
        state
            .vehicles
            .iter()
            .any(|v| v.kind == VehicleKind::Train && !v.orders.is_empty() && v.running),
        "demo: tren importado con órdenes"
    );
}

#[test]
fn grinnway_sav_has_rail_network() {
    // Partida real opcional (solo en desarrollo local bajo save/, no versionada).
    let path = workspace_save_path("save/Grinnway Transport, 1955-08-01.sav");
    let Ok(raw) = std::fs::read(&path) else {
        eprintln!(
            "skip grinnway: {} no presente (partida local opcional)",
            path.display()
        );
        return;
    };

    let sav = sav::load(&raw).expect("grinnway load");
    let rail = count_rail(&sav.map);
    eprintln!(
        "grinnway: SLV={} rail={rail} veh={} with_orders={}",
        sav.version,
        sav.vehicles.len(),
        sav.vehicles.iter().filter(|v| !v.orders.is_empty()).count()
    );
    assert!(
        rail >= 15,
        "Grinnway debería tener más vía que stationlist (got {rail})"
    );
}
