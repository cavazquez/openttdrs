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

fn state_with_custom_road_stop_bridge_height(min_height: u8) -> (GameState, TileCoord) {
    let mut state = GameState::new(10, 8);
    let stop = TileCoord::new(3, 3);
    state
        .map
        .set_kind(TileCoord::new(4, 3), TileKind::Water)
        .unwrap();
    state.road_stop_class_catalog.push(crate::RoadStopClassDef {
        id: 0,
        label: "Bridge class".into(),
        short_label: "BRDG".into(),
        from_newgrf: true,
    });
    let mut bridgeable_info =
        [crate::RoadStopBridgeableInfo::default(); crate::ROADSTOP_LAYOUT_COUNT];
    for info in &mut bridgeable_info {
        info.min_height = min_height;
    }
    state.road_stop_spec_catalog.push(crate::RoadStopSpecDef {
        id: 0,
        class: 0,
        label: "Bridge stop".into(),
        short_label: "BRDG".into(),
        stop_type: crate::ROADSTOP_TYPE_BUS,
        from_newgrf: true,
        grfid: 0x4252_4447,
        newgrf_local_id: 0,
        newgrf_grf_version: 8,
        draw_mode: crate::ROADSTOP_DRAW_MODE_DEFAULT,
        random_cargo_triggers: 0,
        flags: 0,
        build_cost_multiplier: 16,
        clear_cost_multiplier: 16,
        bridgeable_info,
        callback_mask: 0,
        animation_status: 0xFF,
        animation_frames: 0,
        animation_speed: 2,
        animation_triggers: 0,
        newgrf_views: Vec::new(),
        newgrf_runtime: None,
        newgrf_type_tables: None,
        associated_badges: Vec::new(),
        newgrf_badge_translation: Vec::new(),
    });
    crate::apply_command(&mut state, &Command::SetCurrentRoadStopSpec(0)).unwrap();
    crate::apply_command(&mut state, &Command::PlaceRoad(TileCoord::new(3, 2))).unwrap();
    crate::apply_command(&mut state, &Command::PlaceBusStop(stop, 3)).unwrap();
    (state, stop)
}

#[test]
fn bridge_rejects_custom_road_stop_without_enough_bridge_height() {
    let (mut state, stop) = state_with_custom_road_stop_bridge_height(2);
    let start = TileCoord::new(1, 3);
    let end = TileCoord::new(5, 3);
    let command = Command::PlaceRoadBridge(start, end, BridgeType::Wooden);
    assert_eq!(
        command_would_fail(&state, &command),
        Some(CommandError::BridgeTooLowForRoadStop)
    );
    let before = state.map.get(stop);
    assert_eq!(
        apply_command(&mut state, &command),
        Err(CommandError::BridgeTooLowForRoadStop)
    );
    assert_eq!(state.map.get(stop), before);
    assert_eq!(state.map.get_kind(start), Some(TileKind::Grass));
}

#[test]
fn bridge_rejects_custom_road_stop_with_zero_bridgeable_height() {
    let (state, _) = state_with_custom_road_stop_bridge_height(0);
    assert_eq!(
        command_would_fail(
            &state,
            &Command::PlaceRoadBridge(
                TileCoord::new(1, 3),
                TileCoord::new(5, 3),
                BridgeType::Wooden,
            )
        ),
        Some(CommandError::BridgeTooLowForRoadStop)
    );
}

#[test]
fn bridge_accepts_custom_road_stop_at_declared_minimum_height() {
    let (mut state, stop) = state_with_custom_road_stop_bridge_height(1);
    let start = TileCoord::new(1, 3);
    let end = TileCoord::new(5, 3);
    let command = Command::PlaceRoadBridge(start, end, BridgeType::Wooden);
    assert_eq!(command_would_fail(&state, &command), None);
    apply_command(&mut state, &command).unwrap();
    assert_eq!(state.map.get_kind(stop), Some(TileKind::Station));
    assert_eq!(state.map.get_kind(start), Some(TileKind::RoadBridge));
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
