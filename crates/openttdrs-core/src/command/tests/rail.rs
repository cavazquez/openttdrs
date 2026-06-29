use crate::command::{Command, CommandError, apply_command};
use crate::{
    GameState, LevelMode, RAIL_BUILD_COST, STATION_BUILD_COST, STATION_TYPE_RAIL_WAYPOINT,
    StopKind, TileCoord, TileKind, Vehicle, VehicleKind, VehicleOrder, WAYPOINT_BUILD_COST,
    pathfinder, station_type_from_m6, tile_slope_and_z,
};

use super::helpers::{
    finish_train_with_cached_path_to_depot, flat_map_for_terraform_tests, set_w_only_slope,
    train_with_cached_path_to_depot,
};

#[test]
fn place_rail_station_sets_m6_and_axis_in_m5() {
    let mut s = GameState::new(8, 8);
    let c = TileCoord::new(2, 2);
    apply_command(&mut s, &Command::PlaceRail(TileCoord::new(1, 2))).unwrap();
    apply_command(&mut s, &Command::PlaceRailStation(c, 0)).unwrap();
    let tile = s.map.get(c).unwrap();
    assert_eq!(tile.kind, TileKind::Station);
    assert_eq!((tile.m6 >> 3) & 0x0F, 0);
    assert_eq!(
        tile.m5, 3,
        "vía vecina aislada es eje Y → gfx 3 con edificio"
    );
    assert_eq!(s.stations[0].stop_kind, StopKind::RailStation);
}

#[test]
fn place_rail_station_area_writes_layout_and_anchors_center() {
    let mut s = GameState::new(16, 16);
    let origin = TileCoord::new(3, 4);
    let money_before = s.economy.money;
    // Eje X, 3 andenes, longitud 5 → huella 5×3.
    apply_command(
        &mut s,
        &Command::PlaceRailStationArea {
            origin,
            axis_y: false,
            platforms: 3,
            length: 5,
        },
    )
    .unwrap();
    for dy in 0..3 {
        for dx in 0..5 {
            let t = s.map.get(TileCoord::new(3 + dx, 4 + dy)).unwrap();
            assert_eq!(t.kind, TileKind::Station, "tesela ({dx},{dy}) de la huella");
            assert_eq!((t.m6 >> 3) & 0x0F, 0, "tipo rail en m6");
            assert!(t.m5.is_multiple_of(2), "eje X → gfx par");
        }
    }
    // Layout estándar: andén impar primero (edificio al centro), luego par techado.
    assert_eq!(s.map.get(TileCoord::new(5, 4)).unwrap().m5, 2, "edificio");
    // Con longitud > 4 los extremos del andén techado quedan planos (gfx 0).
    assert_eq!(s.map.get(TileCoord::new(3, 5)).unwrap().m5, 0, "extremo");
    assert_eq!(s.map.get(TileCoord::new(4, 5)).unwrap().m5, 4, "techo NW");
    assert_eq!(s.map.get(TileCoord::new(4, 6)).unwrap().m5, 6, "techo SE");
    assert_eq!(s.stations.len(), 1, "una sola estación para toda la huella");
    assert_eq!(s.stations[0].pos, TileCoord::new(5, 5), "ancla al centro");
    assert_eq!(s.economy.money, money_before - 15 * STATION_BUILD_COST);
}

#[test]
fn place_rail_station_area_axis_y_uses_odd_gfx() {
    let mut s = GameState::new(16, 16);
    apply_command(
        &mut s,
        &Command::PlaceRailStationArea {
            origin: TileCoord::new(2, 2),
            axis_y: true,
            platforms: 1,
            length: 3,
        },
    )
    .unwrap();
    assert_eq!(s.map.get(TileCoord::new(2, 2)).unwrap().m5, 1, "plano Y");
    assert_eq!(s.map.get(TileCoord::new(2, 3)).unwrap().m5, 3, "edificio Y");
    assert_eq!(s.map.get(TileCoord::new(2, 4)).unwrap().m5, 1);
    assert_eq!(s.stations[0].pos, TileCoord::new(2, 3));
}

#[test]
fn place_rail_station_area_rejects_occupied_and_out_of_bounds() {
    let mut s = GameState::new(8, 8);
    apply_command(&mut s, &Command::PlaceRoad(TileCoord::new(4, 2))).unwrap();
    let e = apply_command(
        &mut s,
        &Command::PlaceRailStationArea {
            origin: TileCoord::new(2, 2),
            axis_y: false,
            platforms: 2,
            length: 4,
        },
    )
    .unwrap_err();
    assert_eq!(e, CommandError::CannotPlaceStationOnOccupiedTile);
    assert!(s.stations.is_empty());

    let e = apply_command(
        &mut s,
        &Command::PlaceRailStationArea {
            origin: TileCoord::new(6, 6),
            axis_y: false,
            platforms: 2,
            length: 4,
        },
    )
    .unwrap_err();
    assert_eq!(e, CommandError::OutOfBounds);
}

#[test]
fn train_paths_to_track_at_platform_end_of_long_station() {
    use crate::{PathNetwork, find_path, rail_station_approach_tile};
    let mut s = GameState::new(20, 20);
    // Estación eje X de longitud 5 en y=5, andén único: x 4..=8.
    apply_command(
        &mut s,
        &Command::PlaceRailStationArea {
            origin: TileCoord::new(4, 5),
            axis_y: false,
            platforms: 1,
            length: 5,
        },
    )
    .unwrap();
    // Vía pegada al extremo SW del andén y tramo hasta (12,5).
    for x in 9..=12 {
        apply_command(&mut s, &Command::PlaceRail(TileCoord::new(x, 5))).unwrap();
    }
    let anchor = s.stations[0].pos;
    assert_eq!(anchor, TileCoord::new(6, 5));
    let approach = rail_station_approach_tile(&s.map, anchor).unwrap();
    assert_eq!(approach, TileCoord::new(9, 5), "vía junto al extremo");
    let path = find_path(&s.map, TileCoord::new(12, 5), approach, PathNetwork::Rail).unwrap();
    assert_eq!(path.last(), Some(&approach));
}

#[test]
fn rail_path_traverses_station_platform_along_axis() {
    use crate::{PathNetwork, find_path};
    let mut s = GameState::new(20, 20);
    apply_command(
        &mut s,
        &Command::PlaceRailStationArea {
            origin: TileCoord::new(6, 5),
            axis_y: false,
            platforms: 1,
            length: 3,
        },
    )
    .unwrap();
    apply_command(&mut s, &Command::PlaceRail(TileCoord::new(5, 5))).unwrap();
    apply_command(&mut s, &Command::PlaceRail(TileCoord::new(9, 5))).unwrap();
    // El andén actúa como vía X: se puede cruzar de un lado al otro.
    let path = find_path(
        &s.map,
        TileCoord::new(5, 5),
        TileCoord::new(9, 5),
        PathNetwork::Rail,
    )
    .unwrap();
    assert_eq!(
        path,
        vec![
            TileCoord::new(6, 5),
            TileCoord::new(7, 5),
            TileCoord::new(8, 5),
            TileCoord::new(9, 5)
        ]
    );
}

#[test]
fn place_rail_station_rejects_entrance_away_from_rail() {
    let mut s = GameState::new(8, 8);
    let c = TileCoord::new(2, 2);
    apply_command(&mut s, &Command::PlaceRail(TileCoord::new(1, 2))).unwrap();
    let e = apply_command(&mut s, &Command::PlaceRailStation(c, 2)).unwrap_err();
    assert_eq!(e, CommandError::StationNotAdjacentToTransport);
}

#[test]
fn place_rail_mutates_tile_kind() {
    let mut s = GameState::new(8, 8);
    let c = TileCoord::new(1, 3);
    assert_eq!(s.map.get_kind(c), Some(TileKind::Grass));
    apply_command(&mut s, &Command::PlaceRail(c)).unwrap();
    assert_eq!(s.map.get_kind(c), Some(TileKind::Rail));
}

#[test]
fn place_rail_sets_mapt_and_trackbits_for_horizontal_line() {
    let mut s = GameState::new(12, 8);
    for x in 2..=5 {
        apply_command(&mut s, &Command::PlaceRail(TileCoord::new(x, 4))).unwrap();
    }
    let mid = s.map.get(TileCoord::new(3, 4)).unwrap();
    assert_eq!(mid.mapt, 0x10);
    assert_eq!(mid.m5 & 0x3F, 0x01, "tramo horizontal: Track X");
    assert_eq!((mid.m5 >> 6) & 0x3, 0);
}

#[test]
fn set_rail_bits_places_horz_and_vert() {
    let mut s = GameState::new(8, 8);
    apply_command(&mut s, &Command::SetRailBits(TileCoord::new(2, 2), 0x0C)).unwrap();
    apply_command(&mut s, &Command::SetRailBits(TileCoord::new(4, 2), 0x30)).unwrap();
    assert_eq!(s.map.get_kind(TileCoord::new(2, 2)), Some(TileKind::Rail));
    assert_eq!(s.map.get(TileCoord::new(2, 2)).unwrap().m5 & 0x3F, 0x0C);
    assert_eq!(s.map.get(TileCoord::new(4, 2)).unwrap().m5 & 0x3F, 0x30);
}

#[test]
fn autorail_crossing_two_lines_yields_clean_x_y_cross() {
    use crate::{PathNetwork, find_path};

    let mut s = GameState::new(9, 9);
    // Recta X (y=4) y recta Y (x=4) que se cruzan en (4,4).
    for x in 2..=6_i32 {
        apply_command(&mut s, &Command::PlaceRail(TileCoord::new(x, 4))).unwrap();
    }
    for y in 2..=6_i32 {
        apply_command(&mut s, &Command::PlaceRail(TileCoord::new(4, y))).unwrap();
    }
    let mid = s.map.get(TileCoord::new(4, 4)).unwrap();
    assert_eq!(
        mid.m5 & 0x3F,
        0x03,
        "intersección de dos rectas = cruce X|Y sin curvas: m5={:#04x}",
        mid.m5
    );
    // Sin pieza de giro: un tren que viene por X no puede doblar hacia Y.
    assert!(
        find_path(
            &s.map,
            TileCoord::new(2, 4),
            TileCoord::new(6, 4),
            PathNetwork::Rail
        )
        .is_some(),
        "recta X pasa por la diagonal"
    );
    assert!(
        find_path(
            &s.map,
            TileCoord::new(2, 4),
            TileCoord::new(4, 6),
            PathNetwork::Rail
        )
        .is_none(),
        "sin curva no se puede doblar en el cruce"
    );
}

#[test]
fn place_rail_bits_merges_trackbits_on_existing_rail() {
    let mut s = GameState::new(8, 8);
    apply_command(&mut s, &Command::SetRailBits(TileCoord::new(3, 3), 0x01)).unwrap();
    apply_command(&mut s, &Command::PlaceRailBits(TileCoord::new(3, 3), 0x02)).unwrap();
    assert_eq!(s.map.get(TileCoord::new(3, 3)).unwrap().m5 & 0x3F, 0x03);
}

#[test]
fn place_rail_parallel_lanes_merge_on_same_tile() {
    use crate::rail_lane::{rail_horz_lane_bit, rail_vert_lane_bit};

    let mut s = GameState::new(8, 8);
    let c = TileCoord::new(3, 3);
    apply_command(
        &mut s,
        &Command::PlaceRailBits(c, rail_vert_lane_bit(200, 100)),
    )
    .unwrap();
    apply_command(
        &mut s,
        &Command::PlaceRailBits(c, rail_vert_lane_bit(50, 150)),
    )
    .unwrap();
    assert_eq!(s.map.get(c).unwrap().m5 & 0x3F, 0x30);

    let h = TileCoord::new(6, 6);
    apply_command(&mut s, &Command::PlaceRailBits(h, rail_horz_lane_bit(0, 0))).unwrap();
    apply_command(
        &mut s,
        &Command::PlaceRailBits(h, rail_horz_lane_bit(200, 100)),
    )
    .unwrap();
    assert_eq!(s.map.get(h).unwrap().m5 & 0x3F, 0x0C);
}

#[test]
fn parallel_horz_line_keeps_lane_bits_after_neighbor_refresh() {
    use crate::rail_lane::rail_horz_lane_bit;

    let mut s = GameState::new(16, 16);
    for x in 2..=6 {
        apply_command(
            &mut s,
            &Command::PlaceRailBits(TileCoord::new(x, 4), rail_horz_lane_bit(64, 64)),
        )
        .unwrap();
    }
    for x in 2..=6 {
        assert_eq!(
            s.map.get(TileCoord::new(x, 4)).unwrap().m5 & 0x3F,
            0x04,
            "carril UPPER no debe convertirse a X/Y al refrescar vecinos"
        );
    }
}

#[test]
fn parallel_lane_clicks_place_vert_on_clicked_tile_only() {
    use crate::rail_lane::rail_vert_lane_bit;

    let mut s = GameState::new(16, 16);
    apply_command(&mut s, &Command::SetRailBits(TileCoord::new(5, 4), 0x01)).unwrap();
    apply_command(
        &mut s,
        &Command::PlaceRailBits(TileCoord::new(5, 5), rail_vert_lane_bit(200, 100)),
    )
    .unwrap();
    assert_eq!(
        s.map.get(TileCoord::new(5, 5)).unwrap().m5 & 0x3F,
        0x10,
        "LEFT en la tesela del clic"
    );
    assert_eq!(
        s.map.get(TileCoord::new(5, 4)).unwrap().m5 & 0x3F,
        0x01,
        "la X existente no debe ganar piezas extra"
    );
}

#[test]
fn parallel_vert_beside_y_track_stays_on_clicked_tile() {
    use crate::rail_lane::rail_vert_lane_bit;

    let mut s = GameState::new(16, 16);
    apply_command(&mut s, &Command::SetRailBits(TileCoord::new(5, 5), 0x02)).unwrap();
    apply_command(
        &mut s,
        &Command::PlaceRailBits(TileCoord::new(6, 5), rail_vert_lane_bit(200, 100)),
    )
    .unwrap();
    assert_eq!(
        s.map.get(TileCoord::new(6, 5)).unwrap().m5 & 0x3F,
        0x10,
        "carril en la tesela del clic (este de la Y)"
    );
    assert_eq!(
        s.map.get(TileCoord::new(5, 5)).unwrap().m5 & 0x3F,
        0x02,
        "la Y no debe absorber el carril paralelo"
    );
}

#[test]
fn parallel_horz_branch_places_vert_only_on_clicked_tile() {
    use crate::rail_lane::{rail_horz_lane_bit, rail_vert_lane_bit};

    let mut s = GameState::new(16, 16);
    for x in 4..=6 {
        apply_command(
            &mut s,
            &Command::PlaceRailBits(TileCoord::new(x, 5), rail_horz_lane_bit(64, 64)),
        )
        .unwrap();
    }
    apply_command(
        &mut s,
        &Command::PlaceRailBits(TileCoord::new(5, 6), rail_vert_lane_bit(200, 100)),
    )
    .unwrap();
    assert_eq!(
        s.map.get(TileCoord::new(5, 6)).unwrap().m5 & 0x3F,
        0x10,
        "LEFT solo en la tesela del clic"
    );
    assert_eq!(
        s.map.get(TileCoord::new(5, 5)).unwrap().m5 & 0x3F,
        0x04,
        "la vía E-O existente no debe ganar piezas extra"
    );
}

#[test]
fn parallel_horz_extension_stays_on_clicked_tile() {
    use crate::rail_lane::rail_horz_lane_bit;

    let mut s = GameState::new(16, 16);
    apply_command(
        &mut s,
        &Command::PlaceRailBits(TileCoord::new(5, 5), rail_horz_lane_bit(64, 64)),
    )
    .unwrap();
    apply_command(
        &mut s,
        &Command::PlaceRailBits(TileCoord::new(6, 5), rail_horz_lane_bit(64, 64)),
    )
    .unwrap();
    assert_eq!(s.map.get(TileCoord::new(5, 5)).unwrap().m5 & 0x3F, 0x04);
    assert_eq!(s.map.get(TileCoord::new(6, 5)).unwrap().m5 & 0x3F, 0x04);
}

#[test]
fn parallel_horz_second_lane_extends_without_merging_neighbor() {
    use crate::rail_lane::rail_horz_lane_bit;

    let mut s = GameState::new(16, 16);
    apply_command(
        &mut s,
        &Command::PlaceRailBits(TileCoord::new(5, 5), rail_horz_lane_bit(64, 64)),
    )
    .unwrap();
    apply_command(
        &mut s,
        &Command::PlaceRailBits(TileCoord::new(6, 5), rail_horz_lane_bit(200, 100)),
    )
    .unwrap();
    assert_eq!(s.map.get(TileCoord::new(5, 5)).unwrap().m5 & 0x3F, 0x04);
    assert_eq!(s.map.get(TileCoord::new(6, 5)).unwrap().m5 & 0x3F, 0x08);
}

#[test]
fn place_rail_autoslopes_slope_before_invalid_trackbits() {
    let mut s = GameState::new(8, 8);
    set_w_only_slope(&mut s.map, 4, 4, 1);
    let c = TileCoord::new(4, 4);
    assert_eq!(tile_slope_and_z(&s.map, c).unwrap().0, 1);
    let money = s.economy.money;
    apply_command(&mut s, &Command::SetRailBits(c, 0x0C)).unwrap();
    assert_eq!(tile_slope_and_z(&s.map, c).unwrap().0, 0);
    assert_eq!(s.map.get(c).unwrap().kind, TileKind::Rail);
    assert!(s.economy.money < money - RAIL_BUILD_COST);
}

#[test]
fn place_rail_waypoint_on_straight_track() {
    let mut s = GameState::new(8, 8);
    let c = TileCoord::new(3, 3);
    apply_command(&mut s, &Command::SetRailBits(c, 0x01)).unwrap();
    let money = s.economy.money;
    apply_command(&mut s, &Command::PlaceRailWaypoint(c)).unwrap();
    let tile = s.map.get(c).unwrap();
    assert_eq!(tile.kind, TileKind::Station);
    assert_eq!(station_type_from_m6(tile.m6), STATION_TYPE_RAIL_WAYPOINT);
    assert_eq!(s.stations.len(), 1);
    assert_eq!(s.stations[0].stop_kind, StopKind::RailWaypoint);
    assert_eq!(s.economy.money, money - WAYPOINT_BUILD_COST);
}

#[test]
fn place_rail_waypoint_rejects_curved_track() {
    let mut s = GameState::new(8, 8);
    let c = TileCoord::new(2, 2);
    apply_command(&mut s, &Command::SetRailBits(c, 0x03)).unwrap();
    assert_eq!(
        apply_command(&mut s, &Command::PlaceRailWaypoint(c)),
        Err(CommandError::CannotPlaceWaypointOnTrack)
    );
}

#[test]
fn remove_rail_clears_tile_and_refreshes_neighbors() {
    let mut s = GameState::new(8, 8);
    let c = TileCoord::new(3, 3);
    apply_command(&mut s, &Command::SetRailBits(c, 0x01)).unwrap();
    apply_command(&mut s, &Command::SetRailBits(TileCoord::new(4, 3), 0x01)).unwrap();
    let money = s.economy.money;
    apply_command(&mut s, &Command::RemoveRail(c)).unwrap();
    assert_eq!(s.map.get_kind(c), Some(TileKind::Grass));
    assert_eq!(
        s.economy.money,
        money + crate::rail_signals::RAIL_REMOVE_REFUND
    );
}

#[test]
fn place_rail_signal_on_straight_track() {
    let mut s = GameState::new(8, 8);
    let c = TileCoord::new(2, 2);
    apply_command(&mut s, &Command::SetRailBits(c, 0x01)).unwrap();
    let money = s.economy.money;
    apply_command(&mut s, &Command::PlaceRailSignal(c, 0, 128, 128)).unwrap();
    let tile = s.map.get(c).unwrap();
    assert!(crate::rail_signals::rail_tile_is_signals(tile.m5));
    assert_eq!(
        s.economy.money,
        money - crate::rail_signals::SIGNAL_BUILD_COST
    );
}

#[test]
fn place_rail_signal_cycles_side_on_same_track() {
    let mut s = GameState::new(8, 8);
    let c = TileCoord::new(2, 2);
    apply_command(&mut s, &Command::SetRailBits(c, 0x01)).unwrap();
    apply_command(&mut s, &Command::PlaceRailSignal(c, 0, 128, 128)).unwrap();
    let present_one = crate::rail_signals::rail_signal_present_mask(s.map.get(c).unwrap().m3);
    assert_eq!(present_one.count_ones(), 1);
    let money = s.economy.money;
    apply_command(&mut s, &Command::PlaceRailSignal(c, 0, 128, 128)).unwrap();
    let tile = s.map.get(c).unwrap();
    assert!(crate::rail_signals::rail_tile_is_signals(tile.m5));
    assert_eq!(
        crate::rail_signals::rail_signal_present_mask(tile.m3).count_ones(),
        2,
        "CycleSignalSide añade la segunda dirección"
    );
    assert_eq!(s.economy.money, money, "ciclar lado es gratis");
}

#[test]
fn clear_tile_removes_rail_signal() {
    let mut s = GameState::new(8, 8);
    let c = TileCoord::new(2, 2);
    apply_command(&mut s, &Command::SetRailBits(c, 0x01)).unwrap();
    apply_command(&mut s, &Command::PlaceRailSignal(c, 0, 128, 128)).unwrap();
    apply_command(&mut s, &Command::ClearTile(c)).unwrap();
    let tile = s.map.get(c).unwrap();
    assert!(!crate::rail_signals::rail_tile_is_signals(tile.m5));
}

#[test]
fn place_second_signal_on_horz_merges_m2() {
    let mut s = GameState::new(8, 8);
    let c = TileCoord::new(2, 2);
    apply_command(&mut s, &Command::SetRailBits(c, 0x0C)).unwrap();
    apply_command(&mut s, &Command::PlaceRailSignal(c, 0, 64, 64)).unwrap();
    let m2_upper = s.map.get(c).unwrap().m2;
    assert_ne!(m2_upper, 0);
    apply_command(&mut s, &Command::PlaceRailSignal(c, 1, 200, 200)).unwrap();
    let tile = s.map.get(c).unwrap();
    assert!(crate::rail_signals::rail_tile_is_signals(tile.m5));
    assert_eq!(
        tile.m2 & m2_upper,
        m2_upper,
        "m2 del carril superior conservado"
    );
    assert_ne!(tile.m2 & 0xF0, 0, "m2 del carril inferior codificado");
    assert_eq!(
        crate::rail_signals::rail_signal_present_mask(tile.m3),
        0b0110,
        "dos señales en carriles distintos de Horz"
    );
}

#[test]
fn place_rail_bits_preserves_signal_when_merging_diagonals_to_cross() {
    let mut s = GameState::new(8, 8);
    let c = TileCoord::new(3, 3);
    apply_command(&mut s, &Command::PlaceRailBits(c, 0x02)).unwrap();
    apply_command(&mut s, &Command::PlaceRailSignal(c, 0, 128, 128)).unwrap();
    let present_before = crate::rail_signals::rail_signal_present_mask(s.map.get(c).unwrap().m3);
    assert_ne!(present_before, 0);
    apply_command(&mut s, &Command::PlaceRailBits(c, 0x01)).unwrap();
    let tile = s.map.get(c).unwrap();
    assert_eq!(tile.m5 & 0x3F, 0x03, "Y + X = cruce");
    assert!(crate::rail_signals::rail_tile_is_signals(tile.m5));
    assert_eq!(
        crate::rail_signals::rail_signal_present_mask(tile.m3),
        present_before
    );
}

#[test]
fn train_order_through_waypoint_advances_without_full_stop() {
    let mut s = GameState::new(12, 12);
    let wp = TileCoord::new(5, 5);
    let end = TileCoord::new(8, 5);
    for x in 4..=8 {
        apply_command(&mut s, &Command::SetRailBits(TileCoord::new(x, 5), 0x01)).unwrap();
    }
    apply_command(&mut s, &Command::PlaceRailWaypoint(wp)).unwrap();
    s.vehicles.push(Vehicle::new(
        1,
        VehicleKind::Train,
        TileCoord::new(4, 5),
        TileCoord::new(4, 5),
    ));
    apply_command(
        &mut s,
        &Command::SetVehicleOrderList(1, vec![VehicleOrder::waypoint(wp), VehicleOrder::tile(end)]),
    )
    .unwrap();
    s.vehicles[0].set_cruise_speed();
    s.vehicles[0].sync_order_destination(&s.map);
    let path = pathfinder::find_path(
        &s.map,
        TileCoord::new(4, 5),
        wp,
        pathfinder::PathNetwork::Rail,
    );
    assert!(path.is_some());
    assert!(path.unwrap().contains(&wp));
}

#[test]
fn sell_vehicle_requires_depot_tile() {
    let mut s = GameState::new(8, 8);
    let road = TileCoord::new(2, 2);
    s.map.set_kind(road, TileKind::Road).unwrap();
    s.vehicles
        .push(Vehicle::new(1, VehicleKind::Truck, road, road));
    assert_eq!(
        apply_command(&mut s, &Command::SellVehicle(1)),
        Err(CommandError::VehicleNotInDepot)
    );
}

#[test]
fn rail_depot_beside_x_line_connects_exit_tile() {
    use crate::pathfinder::{PathNetwork, find_path};

    let mut s = GameState::new(12, 12);
    // Línea recta en eje X (y=4) y depósito al sur con la boca hacia la vía (NW).
    for x in 2..=8_i32 {
        apply_command(&mut s, &Command::PlaceRail(TileCoord::new(x, 4))).unwrap();
    }
    let depot = TileCoord::new(5, 5);
    apply_command(&mut s, &Command::PlaceRailDepotDir(depot, 3)).unwrap();

    // La tesela de salida gana las curvas de empalme hacia la boca del depósito:
    // X (recta NE↔SW) + LOWER (SE↔SW) + RIGHT (NE↔SE) = 0x29.
    let exit = s.map.get(TileCoord::new(5, 4)).unwrap();
    assert_eq!(
        exit.m5 & 0x3F,
        0x29,
        "empalme esperado X|LOWER|RIGHT: m5={:#04x}",
        exit.m5
    );

    // Un tren en la línea puede llegar al depósito y salir de él.
    assert!(
        find_path(&s.map, TileCoord::new(2, 4), depot, PathNetwork::Rail).is_some(),
        "línea → depósito"
    );
    assert!(
        find_path(&s.map, depot, TileCoord::new(8, 4), PathNetwork::Rail).is_some(),
        "depósito → línea"
    );
}

#[test]
fn disconnecting_rail_stops_train_with_cached_path() {
    let mut s = train_with_cached_path_to_depot();
    // Se desconecta el empalme: la tesela de salida pierde las curvas al depósito.
    apply_command(&mut s, &Command::SetRailBits(TileCoord::new(5, 4), 0x01)).unwrap();
    assert!(
        s.vehicles[0].path.is_empty(),
        "editar el mapa debe invalidar el camino cacheado"
    );
    for _ in 0..5_000 {
        s.step();
        assert_ne!(
            s.vehicles[0].pos,
            TileCoord::new(5, 5),
            "el tren no debe entrar al depósito desconectado"
        );
    }
    assert!(
        s.vehicles[0].no_network_route_to_order,
        "debe marcar que no hay ruta por red"
    );
}

#[test]
fn terraform_raise_clears_cached_train_path() {
    let mut s = train_with_cached_path_to_depot();
    apply_command(&mut s, &Command::RaiseLand(TileCoord::new(10, 10))).unwrap();
    assert!(
        s.vehicles[0].path.is_empty(),
        "RaiseLand debe invalidar el camino cacheado"
    );
}

#[test]
fn terraform_lower_clears_cached_train_path() {
    let mut s = flat_map_for_terraform_tests();
    let hill = TileCoord::new(10, 10);
    apply_command(&mut s, &Command::RaiseLand(hill)).unwrap();
    s = finish_train_with_cached_path_to_depot(s);
    apply_command(&mut s, &Command::LowerLand(hill)).unwrap();
    assert!(
        s.vehicles[0].path.is_empty(),
        "LowerLand debe invalidar el camino cacheado"
    );
}

#[test]
fn terraform_level_clears_cached_train_path() {
    let mut s = train_with_cached_path_to_depot();
    apply_command(
        &mut s,
        &Command::LevelLand {
            from: TileCoord::new(10, 10),
            to: TileCoord::new(10, 10),
            mode: LevelMode::Raise,
        },
    )
    .unwrap();
    assert!(
        s.vehicles[0].path.is_empty(),
        "LevelLand debe invalidar el camino cacheado"
    );
}

#[test]
fn failed_terraform_keeps_cached_train_path() {
    let mut s = train_with_cached_path_to_depot();
    let rail = TileCoord::new(4, 4);
    assert_eq!(
        apply_command(&mut s, &Command::RaiseLand(rail)),
        Err(CommandError::TileNotTerraformable)
    );
    assert!(
        !s.vehicles[0].path.is_empty(),
        "terraform fallido no debe borrar el camino"
    );
}

#[test]
fn build_vehicle_at_rail_depot_creates_train_with_engine() {
    let mut s = GameState::new(8, 8);
    let depot = TileCoord::new(2, 2);
    apply_command(&mut s, &Command::PlaceRail(TileCoord::new(1, 2))).unwrap();
    apply_command(&mut s, &Command::PlaceRailDepotDir(depot, 0)).unwrap();
    let money_before = s.economy.money;
    let engine = crate::engine_by_id(crate::engine::ENGINE_TRAIN_GINZU_A4).unwrap();
    apply_command(
        &mut s,
        &Command::BuildVehicleAtDepot(depot, crate::engine::ENGINE_TRAIN_GINZU_A4),
    )
    .unwrap();
    assert_eq!(s.vehicles.len(), 1);
    assert_eq!(s.vehicles[0].kind, VehicleKind::Train);
    assert_eq!(
        s.vehicles[0].engine_id,
        Some(crate::engine::ENGINE_TRAIN_GINZU_A4)
    );
    assert!(!s.vehicles[0].running);
    assert_eq!(s.economy.money, money_before - engine.price);
}

#[test]
fn build_vehicle_at_depot_rejects_insufficient_funds() {
    let mut s = GameState::new(8, 8);
    let depot = TileCoord::new(2, 2);
    apply_command(&mut s, &Command::PlaceRoad(TileCoord::new(1, 2))).unwrap();
    apply_command(&mut s, &Command::PlaceRoadDepotDir(depot, 0)).unwrap();
    s.economy.money = 10;
    let e = apply_command(
        &mut s,
        &Command::BuildVehicleAtDepot(depot, crate::engine::ENGINE_BUS_MPS),
    )
    .unwrap_err();
    assert_eq!(e, CommandError::InsufficientFunds);
    assert!(s.vehicles.is_empty());
    assert_eq!(s.economy.money, 10, "sin cobro al fallar");
}

#[test]
fn build_vehicle_at_depot_charges_model_price() {
    let mut s = GameState::new(8, 8);
    let depot = TileCoord::new(2, 2);
    apply_command(&mut s, &Command::PlaceRoad(TileCoord::new(1, 2))).unwrap();
    apply_command(&mut s, &Command::PlaceRoadDepotDir(depot, 0)).unwrap();
    let money_before = s.economy.money;
    let engine = crate::engine_by_id(crate::engine::ENGINE_BUS_FOSTER).unwrap();
    apply_command(
        &mut s,
        &Command::BuildVehicleAtDepot(depot, crate::engine::ENGINE_BUS_FOSTER),
    )
    .unwrap();
    assert_eq!(s.economy.money, money_before - engine.price);
    assert_eq!(s.vehicles[0].capacity, engine.capacity);
}

#[test]
fn build_vehicle_at_depot_rejects_unknown_engine() {
    let mut s = GameState::new(8, 8);
    let depot = TileCoord::new(2, 2);
    apply_command(&mut s, &Command::PlaceRoad(TileCoord::new(1, 2))).unwrap();
    apply_command(&mut s, &Command::PlaceRoadDepotDir(depot, 0)).unwrap();
    let e = apply_command(&mut s, &Command::BuildVehicleAtDepot(depot, 9_999)).unwrap_err();
    assert_eq!(e, CommandError::EngineNotFound);
}
