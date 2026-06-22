//! Prueba saves con más vía que `stationlist-test.sav` (sintético y partida real).

#![allow(clippy::expect_used)]

use std::path::PathBuf;

use openttdrs_core::{GameState, TileKind, VehicleKind, sav};

fn sav_bytes(rel: &str) -> Vec<u8> {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.pop();
    path.pop();
    path.push(rel);
    std::fs::read(&path).unwrap_or_else(|e| panic!("no se encontró {}: {e}", path.display()))
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

fn simulate_movement(label: &str, rel: &str, min_rail: usize) {
    let raw = sav_bytes(rel);
    let sav = sav::load(&raw).unwrap_or_else(|e| panic!("{label}: load: {e:?}"));
    let rail = count_rail(&sav.map);
    eprintln!(
        "{label}: SLV={} map={:?} rail_tiles={rail} veh={} stations={}",
        sav.version,
        sav.map.dimensions(),
        sav.vehicles.len(),
        sav.stations.len()
    );

    assert!(
        rail >= min_rail,
        "{label}: esperábamos al menos {min_rail} teselas de vía (got {rail})"
    );

    let mut state = GameState::from_sav_game(sav);
    let movers: Vec<usize> = state
        .vehicles
        .iter()
        .enumerate()
        .filter(|(_, v)| !v.orders.is_empty())
        .map(|(i, _)| i)
        .collect();

    let snapshots: Vec<_> = movers
        .iter()
        .map(|&i| {
            let v = &state.vehicles[i];
            (v.kind, v.pos, v.progress, v.running)
        })
        .collect();

    for _ in 0..500 {
        state.step();
    }

    let mut train_moved = false;
    let mut any_moved = false;
    for (idx, &(kind, start_pos, start_progress, running)) in movers.iter().zip(snapshots.iter()) {
        let v = &state.vehicles[*idx];
        if v.pos != start_pos || v.progress != start_progress {
            any_moved = true;
            if kind == VehicleKind::Train {
                train_moved = true;
            }
            eprintln!(
                "  veh[{idx}] {kind:?} running={running}: {:?} -> {:?} progress {} -> {}",
                start_pos, v.pos, start_progress, v.progress
            );
        }
    }

    eprintln!(
        "{label}: {} vehículos con órdenes, moved={any_moved} train_moved={train_moved}",
        movers.len()
    );

    assert!(
        any_moved,
        "{label}: al menos un vehículo con órdenes debería moverse"
    );
}

#[test]
fn demo_openttd_sav_trains_move() {
    simulate_movement("demo_openttd", "save/demo_openttd.sav", 30);
}

#[test]
fn grinnway_sav_has_rail_network() {
    // Partida real: poca vía explícita en MAPT pero más que stationlist; puede no tener trenes con órdenes.
    let raw = sav_bytes("save/Grinnway Transport, 1955-08-01.sav");
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
