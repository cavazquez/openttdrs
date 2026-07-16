use crate::command::{Command, apply_command};
use crate::test_fixtures::SandboxMap;
use crate::{GameState, TileCoord};

pub(crate) fn set_w_only_slope(map: &mut crate::Map, tx: i32, ty: i32, base: u8) {
    let c = |x: i32, y: i32| TileCoord::new(x, y);
    map.set_height(c(tx, ty), base).unwrap();
    map.set_height(c(tx + 1, ty), base + 1).unwrap();
    map.set_height(c(tx, ty + 1), base).unwrap();
    map.set_height(c(tx + 1, ty + 1), base).unwrap();
}

/// Tren en `(4, 4)` con camino cacheado hacia el depósito en `(5, 5)`.
pub(crate) fn finish_train_with_cached_path_to_depot(mut s: GameState) -> GameState {
    for x in 2..=8_i32 {
        apply_command(&mut s, &Command::PlaceRail(TileCoord::new(x, 4))).unwrap();
    }
    let depot = TileCoord::new(5, 5);
    apply_command(&mut s, &Command::PlaceRailDepotDir(depot, 3)).unwrap();
    apply_command(
        &mut s,
        &Command::BuildVehicleAtDepot(depot, crate::engine::ENGINE_TRAIN_GINZU_A4),
    )
    .unwrap();
    let id = s.vehicles[0].id;
    apply_command(&mut s, &Command::ToggleVehicleRunning(id)).unwrap();
    apply_command(
        &mut s,
        &Command::SetVehicleOrders(id, vec![TileCoord::new(2, 4)]),
    )
    .unwrap();
    for _ in 0..5_000 {
        s.step();
        if s.vehicles[0].pos == TileCoord::new(2, 4) {
            break;
        }
    }
    assert_eq!(
        s.vehicles[0].pos,
        TileCoord::new(2, 4),
        "no llegó al extremo"
    );
    apply_command(&mut s, &Command::SetVehicleOrders(id, vec![depot])).unwrap();
    for _ in 0..5_000 {
        s.step();
        if s.vehicles[0].pos == TileCoord::new(4, 4) {
            break;
        }
    }
    assert_eq!(s.vehicles[0].pos, TileCoord::new(4, 4), "no quedó en ruta");
    assert!(
        !s.vehicles[0].path.is_empty(),
        "debería tener camino cacheado"
    );
    s
}

pub(crate) fn train_with_cached_path_to_depot() -> GameState {
    finish_train_with_cached_path_to_depot(SandboxMap::flat_rich(12, 12, 4))
}

pub(crate) fn flat_map_for_terraform_tests() -> GameState {
    SandboxMap::flat_rich(12, 12, 4)
}
