use crate::bridge_spec::{BridgeType, bridge_build_cost};
use crate::command::{Command, CommandError, apply_command, command_would_fail};
use crate::{
    GameState, PathNetwork, RAIL_TB_X, RAIL_TB_Y, TileCoord, TileKind, bridge_above_axis_from_mapt,
    rail_bridge_other_end,
};

#[test]
fn bridge_cost_scales_with_line_length() {
    let mut s = GameState::new(8, 8);
    let c = |x: i32, y: i32| TileCoord::new(x, y);
    for x in 2..=3 {
        s.map.set_kind(c(x, 1), TileKind::Water).unwrap();
    }
    let a = c(1, 1);
    let b = c(4, 1);
    let money_before = s.economy.money;
    apply_command(&mut s, &Command::PlaceRoadBridge(a, b, BridgeType::Wooden)).unwrap();
    assert_eq!(
        s.economy.money,
        money_before - bridge_build_cost(BridgeType::Wooden, a, b)
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
    apply_command(&mut s, &Command::PlaceRoadBridge(a, b, BridgeType::Wooden)).unwrap();
    assert_eq!(s.map.get(a).unwrap().m5 & 0x03, 1); // SE en eje Y
    let mut s2 = GameState::new(8, 8);
    for x in 1..=5 {
        s2.map.set_kind(c(x, 2), TileKind::Water).unwrap();
    }
    let a2 = TileCoord::new(0, 2);
    let b2 = TileCoord::new(6, 2);
    apply_command(
        &mut s2,
        &Command::PlaceRoadBridge(a2, b2, BridgeType::Wooden),
    )
    .unwrap();
    assert_eq!(s2.map.get(a2).unwrap().m5 & 0x03, 2); // SW en eje X
}

#[test]
fn bridge_rejects_flat_grass_without_gap() {
    let s = GameState::new(8, 8);
    let a = TileCoord::new(1, 1);
    let b = TileCoord::new(4, 1);
    assert_eq!(
        command_would_fail(&s, &Command::PlaceRoadBridge(a, b, BridgeType::Wooden)),
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
    assert!(
        command_would_fail(
            &s,
            &Command::PlaceRoadBridge(c(1, 4), c(6, 4), BridgeType::Wooden)
        )
        .is_none()
    );
}

fn assert_rail_bridge_connects_axis(axis_y: bool) {
    let mut state = GameState::new(12, 12);
    let c = TileCoord::new;
    let (start, end, before, after, expected_dir, expected_reverse, expected_track, middle) =
        if axis_y {
            let start = c(5, 2);
            let end = c(5, 7);
            for y in 3..=6 {
                state.map.set_kind(c(5, y), TileKind::Water).unwrap();
            }
            (start, end, c(5, 1), c(5, 8), 1, 3, RAIL_TB_Y, c(5, 4))
        } else {
            let start = c(2, 5);
            let end = c(7, 5);
            for x in 3..=6 {
                state.map.set_kind(c(x, 5), TileKind::Water).unwrap();
            }
            (start, end, c(1, 5), c(8, 5), 2, 0, RAIL_TB_X, c(4, 5))
        };

    apply_command(
        &mut state,
        &Command::PlaceRailBridge(start, end, BridgeType::Wooden),
    )
    .unwrap();
    for access in [before, after] {
        apply_command(&mut state, &Command::PlaceRail(access)).unwrap();
    }

    // `axis_to_diag_dir` / `ReverseDiagDir` de OpenTTD codifican las dos
    // rampas opuestas; de esto dependen el sprite y el salto lógico del vano.
    assert_eq!(state.map.get(start).unwrap().m5 & 0x03, expected_dir);
    assert_eq!(state.map.get(end).unwrap().m5 & 0x03, expected_reverse);
    assert_eq!(rail_bridge_other_end(&state.map, start), Some(end));
    assert_eq!(rail_bridge_other_end(&state.map, end), Some(start));
    assert_eq!(
        bridge_above_axis_from_mapt(state.map.get(middle).unwrap().mapt),
        Some(axis_y)
    );
    assert_eq!(
        crate::rail_pbs::track_for_rail_step(&state.map, start, end),
        Some(expected_track)
    );
    assert_eq!(
        crate::rail_pbs::track_for_rail_step(&state.map, end, start),
        Some(expected_track)
    );

    for (from, to) in [(before, after), (after, before)] {
        let Some(path) = crate::find_path(&state.map, from, to, PathNetwork::Rail) else {
            panic!("las vías deben cruzar el puente de {from:?} a {to:?}");
        };
        assert!(path.contains(&start), "ruta sin rampa inicial: {path:?}");
        assert!(path.contains(&end), "ruta sin rampa final: {path:?}");
    }
}

#[test]
fn rail_bridge_routes_through_both_axes_and_both_ramps() {
    // Regresión de los puentes visualmente desconectados: el mismo m5 que
    // selecciona la rampa debe producir continuidad real para X e Y.
    assert_rail_bridge_connects_axis(false);
    assert_rail_bridge_connects_axis(true);
}
