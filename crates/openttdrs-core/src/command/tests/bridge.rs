use crate::command::{Command, CommandError, apply_command, command_would_fail};
use crate::{BRIDGE_BUILD_COST_PER_TILE, GameState, TileCoord, TileKind};

#[test]
fn bridge_cost_scales_with_line_length() {
    let mut s = GameState::new(8, 8);
    let c = |x: i32, y: i32| TileCoord::new(x, y);
    for x in 2..=3 {
        s.map.set_kind(c(x, 1), TileKind::Water).unwrap();
    }
    let money_before = s.economy.money;
    apply_command(&mut s, &Command::PlaceRoadBridge(c(1, 1), c(4, 1))).unwrap();
    assert_eq!(
        s.economy.money,
        money_before - BRIDGE_BUILD_COST_PER_TILE * 4
    );
}

#[test]
fn bridge_axis_y_sets_m5_flag() {
    let mut s = GameState::new(8, 8);
    let c = |x: i32, y: i32| TileCoord::new(x, y);
    for y in 2..=4 {
        s.map.set_kind(c(2, y), TileKind::Water).unwrap();
    }
    let a = TileCoord::new(2, 1);
    let b = TileCoord::new(2, 5);
    apply_command(&mut s, &Command::PlaceRoadBridge(a, b)).unwrap();
    assert_eq!(s.map.get(a).unwrap().m5 & 0x10, 0x10);
    let mut s2 = GameState::new(8, 8);
    for x in 1..=5 {
        s2.map.set_kind(c(x, 2), TileKind::Water).unwrap();
    }
    let a2 = TileCoord::new(0, 2);
    let b2 = TileCoord::new(6, 2);
    apply_command(&mut s2, &Command::PlaceRoadBridge(a2, b2)).unwrap();
    assert_eq!(s2.map.get(a2).unwrap().m5 & 0x10, 0);
}

#[test]
fn bridge_rejects_flat_grass_without_gap() {
    let s = GameState::new(8, 8);
    let a = TileCoord::new(1, 1);
    let b = TileCoord::new(4, 1);
    assert_eq!(
        command_would_fail(&s, &Command::PlaceRoadBridge(a, b)),
        Some(CommandError::InvalidBridgeSpan)
    );
}

#[test]
fn bridge_accepts_span_over_water() {
    let mut s = GameState::new(16, 8);
    let c = |x: i32, y: i32| TileCoord::new(x, y);
    for x in 2..=5 {
        s.map.set_kind(c(x, 4), TileKind::Water).unwrap();
    }
    assert!(command_would_fail(&s, &Command::PlaceRoadBridge(c(1, 4), c(6, 4))).is_none());
}
