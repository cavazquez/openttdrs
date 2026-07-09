use crate::command::{Command, CommandError, apply_command};
use crate::{
    CargoType, GameState, LevelMode, OrderConditionKind, RAIL_BUILD_COST, STATION_BUILD_COST,
    STATION_TYPE_RAIL_WAYPOINT, StopKind, TileCoord, TileKind, Vehicle, VehicleKind, VehicleOrder,
    WAYPOINT_BUILD_COST, pathfinder, station_type_from_m6, tile_slope_and_z,
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
fn train_paths_to_platform_stop_tile_of_long_station() {
    use crate::{PathNetwork, find_path, rail_station_approach_tile, rail_station_stop_tile};
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
    // Vía pegada al extremo este del andén y tramo hasta (12,5).
    for x in 9..=12 {
        apply_command(&mut s, &Command::PlaceRail(TileCoord::new(x, 5))).unwrap();
    }
    let anchor = s.stations[0].pos;
    assert_eq!(anchor, TileCoord::new(6, 5));
    let approach = rail_station_approach_tile(&s.map, anchor).unwrap();
    assert_eq!(approach, TileCoord::new(9, 5), "vía junto al extremo este");
    let stop = rail_station_stop_tile(&s.map, anchor).unwrap();
    assert_eq!(
        stop,
        TileCoord::new(6, 5),
        "parada Middle en plataforma de 5"
    );
    let path = find_path(&s.map, TileCoord::new(12, 5), stop, PathNetwork::Rail).unwrap();
    assert_eq!(path.last(), Some(&stop));
    assert!(
        s.map.get_kind(stop) == Some(TileKind::Station),
        "el destino es la plataforma, no la vía de acceso"
    );
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
        0x14,
        "empalme T: carril paralelo une la vía E-O existente"
    );
}

#[test]
fn place_rail_bits_links_perpendicular_neighbor() {
    let mut s = GameState::new(8, 8);
    apply_command(&mut s, &Command::PlaceRailBits(TileCoord::new(3, 4), 0x01)).unwrap();
    apply_command(&mut s, &Command::PlaceRailBits(TileCoord::new(3, 3), 0x02)).unwrap();
    assert_eq!(s.map.get(TileCoord::new(3, 3)).unwrap().m5 & 0x3F, 0x02);
    assert_eq!(
        s.map.get(TileCoord::new(3, 4)).unwrap().m5 & 0x3F,
        0x03,
        "la X existente forma cruce al colocar Y perpendicular en vecino"
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
    apply_command(
        &mut s,
        &Command::PlaceRailSignal(c, 0, 128, 128, crate::rail_signals::SIGTYPE_BLOCK),
    )
    .unwrap();
    let tile = s.map.get(c).unwrap();
    assert!(crate::rail_signals::rail_tile_is_signals(tile.m5));
    assert_eq!(
        s.economy.money,
        money - crate::rail_signals::SIGNAL_BUILD_COST
    );
}

#[test]
fn place_rail_signal_cycles_side_full_on_x() {
    let mut s = GameState::new(8, 8);
    let c = TileCoord::new(2, 2);
    apply_command(&mut s, &Command::SetRailBits(c, 0x01)).unwrap();
    apply_command(
        &mut s,
        &Command::PlaceRailSignal(c, 0, 128, 128, crate::rail_signals::SIGTYPE_BLOCK),
    )
    .unwrap();
    assert_eq!(
        crate::rail_signals::rail_signal_present_mask(s.map.get(c).unwrap().m3).count_ones(),
        1
    );
    // 2.º clic → two-way
    apply_command(
        &mut s,
        &Command::PlaceRailSignal(c, 0, 128, 128, crate::rail_signals::SIGTYPE_BLOCK),
    )
    .unwrap();
    assert_eq!(
        crate::rail_signals::rail_signal_present_mask(s.map.get(c).unwrap().m3).count_ones(),
        2
    );
    // 3.º clic → one-way sentido opuesto
    apply_command(
        &mut s,
        &Command::PlaceRailSignal(c, 0, 128, 128, crate::rail_signals::SIGTYPE_BLOCK),
    )
    .unwrap();
    assert_eq!(
        crate::rail_signals::rail_signal_present_mask(s.map.get(c).unwrap().m3).count_ones(),
        1
    );
    // 4.º clic → vuelve al one-way inicial (ciclo completo)
    apply_command(
        &mut s,
        &Command::PlaceRailSignal(c, 0, 128, 128, crate::rail_signals::SIGTYPE_BLOCK),
    )
    .unwrap();
    assert_eq!(
        crate::rail_signals::rail_signal_present_mask(s.map.get(c).unwrap().m3).count_ones(),
        1
    );
}

#[test]
fn place_rail_signal_cycles_side_on_same_track() {
    let mut s = GameState::new(8, 8);
    let c = TileCoord::new(2, 2);
    apply_command(&mut s, &Command::SetRailBits(c, 0x01)).unwrap();
    apply_command(
        &mut s,
        &Command::PlaceRailSignal(c, 0, 128, 128, crate::rail_signals::SIGTYPE_BLOCK),
    )
    .unwrap();
    let present_one = crate::rail_signals::rail_signal_present_mask(s.map.get(c).unwrap().m3);
    assert_eq!(present_one.count_ones(), 1);
    let money = s.economy.money;
    apply_command(
        &mut s,
        &Command::PlaceRailSignal(c, 0, 128, 128, crate::rail_signals::SIGTYPE_BLOCK),
    )
    .unwrap();
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
fn cycle_rail_signal_type_block_path_oneway() {
    use crate::rail_signals::{
        SIGTYPE_BLOCK, SIGTYPE_PATH, SIGTYPE_PATH_ONEWAY, SignalTrack, signal_type_for_track,
    };

    let mut s = GameState::new(8, 8);
    let c = TileCoord::new(2, 2);
    apply_command(&mut s, &Command::SetRailBits(c, 0x01)).unwrap();
    apply_command(
        &mut s,
        &Command::PlaceRailSignal(c, 0, 128, 128, SIGTYPE_BLOCK),
    )
    .unwrap();
    let track = SignalTrack::X;
    assert_eq!(
        signal_type_for_track(s.map.get(c).unwrap().m2, track),
        SIGTYPE_BLOCK
    );
    apply_command(&mut s, &Command::CycleRailSignalType(c, 128, 128)).unwrap();
    assert_eq!(
        signal_type_for_track(s.map.get(c).unwrap().m2, track),
        SIGTYPE_PATH
    );
    apply_command(&mut s, &Command::CycleRailSignalType(c, 128, 128)).unwrap();
    assert_eq!(
        signal_type_for_track(s.map.get(c).unwrap().m2, track),
        SIGTYPE_PATH_ONEWAY
    );
    apply_command(&mut s, &Command::CycleRailSignalType(c, 128, 128)).unwrap();
    assert_eq!(
        signal_type_for_track(s.map.get(c).unwrap().m2, track),
        SIGTYPE_BLOCK
    );
}

#[test]
fn place_path_signal_with_explicit_type() {
    use crate::rail_signals::{SIGTYPE_PATH, SignalTrack, signal_type_for_track};

    let mut s = GameState::new(8, 8);
    let c = TileCoord::new(2, 2);
    apply_command(&mut s, &Command::SetRailBits(c, 0x01)).unwrap();
    apply_command(
        &mut s,
        &Command::PlaceRailSignal(c, 0, 128, 128, SIGTYPE_PATH),
    )
    .unwrap();
    assert_eq!(
        signal_type_for_track(s.map.get(c).unwrap().m2, SignalTrack::X),
        SIGTYPE_PATH
    );
}

#[test]
fn clear_tile_removes_rail_signal() {
    let mut s = GameState::new(8, 8);
    let c = TileCoord::new(2, 2);
    apply_command(&mut s, &Command::SetRailBits(c, 0x01)).unwrap();
    apply_command(
        &mut s,
        &Command::PlaceRailSignal(c, 0, 128, 128, crate::rail_signals::SIGTYPE_BLOCK),
    )
    .unwrap();
    apply_command(&mut s, &Command::ClearTile(c)).unwrap();
    let tile = s.map.get(c).unwrap();
    assert!(!crate::rail_signals::rail_tile_is_signals(tile.m5));
}

#[test]
fn remove_rail_signal_keeps_track() {
    let mut s = GameState::new(8, 8);
    let c = TileCoord::new(2, 2);
    apply_command(&mut s, &Command::SetRailBits(c, 0x01)).unwrap();
    apply_command(
        &mut s,
        &Command::PlaceRailSignal(c, 0, 128, 128, crate::rail_signals::SIGTYPE_BLOCK),
    )
    .unwrap();
    let money_before = s.economy.money;
    apply_command(&mut s, &Command::RemoveRailSignal(c, 128, 128)).unwrap();
    let tile = s.map.get(c).unwrap();
    assert_eq!(tile.kind, TileKind::Rail);
    assert!(!crate::rail_signals::rail_tile_is_signals(tile.m5));
    assert_eq!(tile.m5 & 0x3F, 0x01);
    assert!(s.economy.money > money_before);
}

#[test]
fn remove_rail_signal_one_lane_on_horz_keeps_other() {
    let mut s = GameState::new(8, 8);
    let c = TileCoord::new(2, 2);
    apply_command(&mut s, &Command::SetRailBits(c, 0x0C)).unwrap(); // HORZ
    apply_command(
        &mut s,
        &Command::PlaceRailSignal(c, 0, 64, 64, crate::rail_signals::SIGTYPE_BLOCK),
    )
    .unwrap();
    apply_command(
        &mut s,
        &Command::PlaceRailSignal(c, 1, 200, 200, crate::rail_signals::SIGTYPE_BLOCK),
    )
    .unwrap();
    assert!(crate::rail_signals::rail_tile_is_signals(
        s.map.get(c).unwrap().m5
    ));
    apply_command(&mut s, &Command::RemoveRailSignal(c, 64, 64)).unwrap();
    let tile = s.map.get(c).unwrap();
    assert!(crate::rail_signals::rail_tile_is_signals(tile.m5));
    assert_eq!(tile.m5 & 0x3F, 0x0C);
    let present = crate::rail_signals::rail_signal_present_mask(tile.m3);
    assert_ne!(present, 0);
}

#[test]
fn place_second_signal_on_horz_merges_m2() {
    let mut s = GameState::new(8, 8);
    let c = TileCoord::new(2, 2);
    apply_command(&mut s, &Command::SetRailBits(c, 0x0C)).unwrap();
    apply_command(
        &mut s,
        &Command::PlaceRailSignal(c, 0, 64, 64, crate::rail_signals::SIGTYPE_BLOCK),
    )
    .unwrap();
    let m2_upper = s.map.get(c).unwrap().m2;
    assert_ne!(m2_upper, 0);
    apply_command(
        &mut s,
        &Command::PlaceRailSignal(c, 1, 200, 200, crate::rail_signals::SIGTYPE_BLOCK),
    )
    .unwrap();
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
    apply_command(
        &mut s,
        &Command::PlaceRailSignal(c, 0, 128, 128, crate::rail_signals::SIGTYPE_BLOCK),
    )
    .unwrap();
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
fn remove_vehicle_order_at_adjusts_current_order() {
    let mut s = GameState::new(8, 8);
    let a = TileCoord::new(2, 2);
    let b = TileCoord::new(4, 2);
    let c = TileCoord::new(6, 2);
    for x in 2..=6 {
        apply_command(&mut s, &Command::SetRailBits(TileCoord::new(x, 2), 0x01)).unwrap();
    }
    s.vehicles.push(Vehicle::new(1, VehicleKind::Train, a, a));
    apply_command(
        &mut s,
        &Command::SetVehicleOrderList(
            1,
            vec![
                VehicleOrder::tile(a),
                VehicleOrder::tile(b),
                VehicleOrder::tile(c),
            ],
        ),
    )
    .unwrap();
    s.vehicles[0].current_order = 2;
    apply_command(
        &mut s,
        &Command::RemoveVehicleOrderAt {
            vehicle_id: 1,
            index: 1,
        },
    )
    .unwrap();
    assert_eq!(s.vehicles[0].orders.len(), 2);
    assert_eq!(s.vehicles[0].current_order, 1);
}

#[test]
fn skip_vehicle_order_advances_current() {
    let mut s = GameState::new(8, 8);
    let a = TileCoord::new(2, 2);
    let b = TileCoord::new(4, 2);
    s.vehicles.push(Vehicle::new(1, VehicleKind::Bus, a, a));
    apply_command(
        &mut s,
        &Command::SetVehicleOrderList(1, vec![VehicleOrder::tile(a), VehicleOrder::tile(b)]),
    )
    .unwrap();
    assert_eq!(s.vehicles[0].current_order, 0);
    apply_command(&mut s, &Command::SkipVehicleOrder(1)).unwrap();
    assert_eq!(s.vehicles[0].current_order, 1);
}

#[test]
fn toggle_full_load_on_station_order() {
    let mut s = GameState::new(8, 8);
    let stop = TileCoord::new(3, 3);
    let road = TileCoord::new(3, 2);
    apply_command(&mut s, &Command::PlaceRoad(road)).unwrap();
    apply_command(&mut s, &Command::PlaceBusStop(stop, 3)).unwrap();
    s.vehicles
        .push(Vehicle::new(1, VehicleKind::Bus, stop, stop));
    apply_command(
        &mut s,
        &Command::SetVehicleOrderList(1, vec![VehicleOrder::station(stop)]),
    )
    .unwrap();
    apply_command(
        &mut s,
        &Command::ToggleVehicleOrderFullLoad {
            vehicle_id: 1,
            index: 0,
        },
    )
    .unwrap();
    assert!(s.vehicles[0].orders[0].full_load());
    apply_command(
        &mut s,
        &Command::ToggleVehicleOrderNoUnload {
            vehicle_id: 1,
            index: 0,
        },
    )
    .unwrap();
    assert!(s.vehicles[0].orders[0].no_unload());
}

#[test]
fn append_goto_nearest_depot_adds_depot_order() {
    let mut s = GameState::new(10, 10);
    let depot = TileCoord::new(5, 5);
    let bus_pos = TileCoord::new(2, 2);
    apply_command(&mut s, &Command::PlaceRoad(bus_pos)).unwrap();
    apply_command(&mut s, &Command::PlaceRoad(TileCoord::new(5, 4))).unwrap();
    apply_command(&mut s, &Command::PlaceRoadDepotDir(depot, 3)).unwrap();
    s.vehicles
        .push(Vehicle::new(1, VehicleKind::Bus, bus_pos, bus_pos));
    apply_command(&mut s, &Command::AppendGotoNearestDepot(1)).unwrap();
    assert_eq!(s.vehicles[0].orders.len(), 1);
    assert_eq!(s.vehicles[0].orders[0].destination(), depot);
}

#[test]
fn rename_vehicle_stores_trimmed_name() {
    let mut s = GameState::new(4, 4);
    s.vehicles.push(Vehicle::new(
        1,
        VehicleKind::Bus,
        TileCoord::new(1, 1),
        TileCoord::new(1, 1),
    ));
    apply_command(
        &mut s,
        &Command::RenameVehicle {
            vehicle_id: 1,
            name: Some("  Ruta 42  ".to_string()),
        },
    )
    .unwrap();
    assert_eq!(s.vehicles[0].name.as_deref(), Some("Ruta 42"));
}

#[test]
fn set_depot_vehicles_running_toggles_all_in_tile() {
    let mut s = GameState::new(8, 8);
    let depot = TileCoord::new(3, 3);
    apply_command(&mut s, &Command::PlaceRoad(TileCoord::new(3, 2))).unwrap();
    apply_command(&mut s, &Command::PlaceRoadDepotDir(depot, 3)).unwrap();
    s.vehicles
        .push(Vehicle::new(1, VehicleKind::Bus, depot, depot));
    s.vehicles[0].running = true;
    s.vehicles
        .push(Vehicle::new(2, VehicleKind::Truck, depot, depot));
    s.vehicles[1].running = true;
    apply_command(
        &mut s,
        &Command::SetDepotVehiclesRunning {
            depot_pos: depot,
            running: false,
        },
    )
    .unwrap();
    assert!(!s.vehicles[0].running);
    assert!(!s.vehicles[1].running);
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
fn two_trains_can_leave_same_rail_depot() {
    let mut s = GameState::new(12, 12);
    s.economy.money = 1_000_000;
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
    let id1 = s.vehicles[0].id;
    apply_command(
        &mut s,
        &Command::BuildVehicleAtDepot(depot, crate::engine::ENGINE_TRAIN_GINZU_A4),
    )
    .unwrap();
    let id2 = s.vehicles[1].id;

    let orders = vec![TileCoord::new(8, 4)];
    apply_command(&mut s, &Command::SetVehicleOrders(id1, orders.clone())).unwrap();
    apply_command(&mut s, &Command::SetVehicleOrders(id2, orders)).unwrap();
    apply_command(&mut s, &Command::ToggleVehicleRunning(id1)).unwrap();
    apply_command(&mut s, &Command::ToggleVehicleRunning(id2)).unwrap();

    let mut any_left = false;
    for _ in 0..10_000 {
        s.step();
        if s.vehicles[0].pos != depot || s.vehicles[1].pos != depot {
            any_left = true;
            break;
        }
    }
    assert!(
        any_left,
        "al menos un tren debe salir del depósito compartido"
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
    assert_eq!(s.vehicles[0].direction, crate::DIR_NE);
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

#[test]
fn move_vehicle_order_swaps_and_tracks_current() {
    use crate::command::OrderMoveDirection;

    let mut s = GameState::new(8, 8);
    let a = TileCoord::new(2, 2);
    let b = TileCoord::new(4, 2);
    let c = TileCoord::new(6, 2);
    s.vehicles.push(Vehicle::new(1, VehicleKind::Bus, a, a));
    apply_command(
        &mut s,
        &Command::SetVehicleOrderList(
            1,
            vec![
                VehicleOrder::tile(a),
                VehicleOrder::tile(b),
                VehicleOrder::tile(c),
            ],
        ),
    )
    .unwrap();
    s.vehicles[0].current_order = 1;
    apply_command(
        &mut s,
        &Command::MoveVehicleOrder {
            vehicle_id: 1,
            index: 1,
            direction: OrderMoveDirection::Up,
        },
    )
    .unwrap();
    assert_eq!(s.vehicles[0].orders[0].destination(), b);
    assert_eq!(s.vehicles[0].orders[1].destination(), a);
    assert_eq!(s.vehicles[0].current_order, 0);
}

#[test]
fn toggle_depot_stop_on_depot_order() {
    let mut s = GameState::new(8, 8);
    let depot = TileCoord::new(2, 2);
    apply_command(&mut s, &Command::PlaceRoad(TileCoord::new(1, 2))).unwrap();
    apply_command(&mut s, &Command::PlaceRoadDepotDir(depot, 0)).unwrap();
    s.vehicles
        .push(Vehicle::new(1, VehicleKind::Bus, depot, depot));
    apply_command(
        &mut s,
        &Command::SetVehicleOrderList(1, vec![VehicleOrder::depot(depot)]),
    )
    .unwrap();
    assert!(s.vehicles[0].orders[0].depot_stops());
    apply_command(
        &mut s,
        &Command::ToggleVehicleOrderDepotStop {
            vehicle_id: 1,
            index: 0,
        },
    )
    .unwrap();
    assert!(!s.vehicles[0].orders[0].depot_stops());
}

#[test]
fn turn_around_vehicle_reverses_train_heading() {
    use crate::vehicle::{DIR_N, DIR_S};

    let mut s = GameState::new(8, 8);
    let pos = TileCoord::new(2, 2);
    let mut train = Vehicle::new(1, VehicleKind::Train, pos, pos);
    train.direction = DIR_N;
    s.vehicles.push(train);
    apply_command(&mut s, &Command::TurnAroundVehicle(1)).unwrap();
    assert_eq!(s.vehicles[0].direction, DIR_S);
}

#[test]
fn clone_vehicle_at_depot_copies_engine_and_orders() {
    let mut s = GameState::new(8, 8);
    let depot = TileCoord::new(2, 2);
    apply_command(&mut s, &Command::PlaceRoad(TileCoord::new(1, 2))).unwrap();
    apply_command(&mut s, &Command::PlaceRoadDepotDir(depot, 0)).unwrap();
    apply_command(
        &mut s,
        &Command::BuildVehicleAtDepot(depot, crate::engine::ENGINE_BUS_FOSTER),
    )
    .unwrap();
    let source_id = s.vehicles[0].id;
    apply_command(
        &mut s,
        &Command::SetVehicleOrderList(source_id, vec![VehicleOrder::tile(TileCoord::new(3, 3))]),
    )
    .unwrap();
    let money_before = s.economy.money;
    apply_command(
        &mut s,
        &Command::CloneVehicleAtDepot {
            source_vehicle_id: source_id,
            depot_pos: depot,
        },
    )
    .unwrap();
    assert_eq!(s.vehicles.len(), 2);
    assert_eq!(
        s.vehicles[1].engine_id,
        Some(crate::engine::ENGINE_BUS_FOSTER)
    );
    assert_eq!(s.vehicles[1].orders, s.vehicles[0].orders);
    assert!(s.economy.money < money_before);
}

#[test]
fn sell_all_vehicles_at_depot_empties_depot() {
    let mut s = GameState::new(8, 8);
    let depot = TileCoord::new(2, 2);
    apply_command(&mut s, &Command::PlaceRoad(TileCoord::new(1, 2))).unwrap();
    apply_command(&mut s, &Command::PlaceRoadDepotDir(depot, 0)).unwrap();
    apply_command(
        &mut s,
        &Command::BuildVehicleAtDepot(depot, crate::engine::ENGINE_BUS_MPS),
    )
    .unwrap();
    apply_command(
        &mut s,
        &Command::BuildVehicleAtDepot(depot, crate::engine::ENGINE_BUS_MPS),
    )
    .unwrap();
    assert_eq!(s.vehicles.len(), 2);
    apply_command(&mut s, &Command::SellAllVehiclesAtDepot(depot)).unwrap();
    assert!(s.vehicles.is_empty());
}

#[test]
fn refit_truck_in_depot_changes_cargo_type() {
    let mut s = GameState::new(8, 8);
    let depot = TileCoord::new(2, 2);
    apply_command(&mut s, &Command::PlaceRoad(TileCoord::new(1, 2))).unwrap();
    apply_command(&mut s, &Command::PlaceRoadDepotDir(depot, 0)).unwrap();
    apply_command(
        &mut s,
        &Command::BuildVehicleAtDepot(depot, crate::engine::ENGINE_TRUCK_MPS),
    )
    .unwrap();
    let id = s.vehicles[0].id;
    apply_command(
        &mut s,
        &Command::RefitVehicle {
            vehicle_id: id,
            cargo: CargoType::Coal,
        },
    )
    .unwrap();
    assert_eq!(s.vehicles[0].cargo_type, Some(CargoType::Coal));
}

#[test]
fn refit_rejects_with_cargo_on_board() {
    let mut s = GameState::new(8, 8);
    let depot = TileCoord::new(2, 2);
    apply_command(&mut s, &Command::PlaceRoad(TileCoord::new(1, 2))).unwrap();
    apply_command(&mut s, &Command::PlaceRoadDepotDir(depot, 0)).unwrap();
    apply_command(
        &mut s,
        &Command::BuildVehicleAtDepot(depot, crate::engine::ENGINE_TRUCK_MPS),
    )
    .unwrap();
    s.vehicles[0].cargo = 5;
    let id = s.vehicles[0].id;
    assert_eq!(
        apply_command(
            &mut s,
            &Command::RefitVehicle {
                vehicle_id: id,
                cargo: CargoType::Coal,
            },
        ),
        Err(CommandError::RefitNotAllowed)
    );
}

#[test]
fn force_vehicle_proceed_sets_flag_on_train() {
    let mut s = GameState::new(8, 8);
    let pos = TileCoord::new(2, 2);
    s.vehicles
        .push(Vehicle::new(1, VehicleKind::Train, pos, pos));
    apply_command(&mut s, &Command::ForceVehicleProceed(1)).unwrap();
    assert!(s.vehicles[0].force_proceed);
    apply_command(&mut s, &Command::ForceVehicleProceed(2)).unwrap_err();
}

#[test]
fn autoreplace_upgrades_truck_in_depot() {
    let mut s = GameState::new(8, 8);
    let depot = TileCoord::new(2, 2);
    apply_command(&mut s, &Command::PlaceRoad(TileCoord::new(1, 2))).unwrap();
    apply_command(&mut s, &Command::PlaceRoadDepotDir(depot, 0)).unwrap();
    apply_command(
        &mut s,
        &Command::BuildVehicleAtDepot(depot, crate::engine::ENGINE_TRUCK_MPS),
    )
    .unwrap();
    let id = s.vehicles[0].id;
    apply_command(
        &mut s,
        &Command::SetAutoReplaceRule {
            from_engine_id: crate::engine::ENGINE_TRUCK_MPS,
            to_engine_id: crate::engine::ENGINE_TRUCK_BALOGH_GOODS,
        },
    )
    .unwrap();
    assert!(crate::autoreplace::try_autoreplace_vehicle(&mut s, id).unwrap());
    assert_eq!(
        s.vehicles[0].engine_id,
        Some(crate::engine::ENGINE_TRUCK_BALOGH_GOODS)
    );
}

#[test]
fn vehicle_group_assign_and_save_v8_fields() {
    let mut s = GameState::new(8, 8);
    apply_command(
        &mut s,
        &Command::CreateVehicleGroup {
            name: "Buses centro".into(),
        },
    )
    .unwrap();
    let group_id = s.vehicle_groups[0].id;
    let depot = TileCoord::new(2, 2);
    apply_command(&mut s, &Command::PlaceRoad(TileCoord::new(1, 2))).unwrap();
    apply_command(&mut s, &Command::PlaceRoadDepotDir(depot, 0)).unwrap();
    apply_command(
        &mut s,
        &Command::BuildVehicleAtDepot(depot, crate::engine::ENGINE_BUS_MPS),
    )
    .unwrap();
    let id = s.vehicles[0].id;
    assert_eq!(s.vehicles[0].build_tick, s.tick.get());
    apply_command(
        &mut s,
        &Command::AssignVehicleToGroup {
            vehicle_id: id,
            group_id: Some(group_id),
        },
    )
    .unwrap();
    assert_eq!(s.vehicles[0].group_id, Some(group_id));
    assert!(s.vehicles[0].vehicle_age_years(s.tick.get()) == 0);
}

#[test]
fn timetable_lateness_clear_command() {
    let mut s = GameState::new(4, 4);
    let mut v = Vehicle::new(
        1,
        VehicleKind::Bus,
        TileCoord::new(0, 0),
        TileCoord::new(1, 1),
    );
    v.timetable_lateness = 42;
    s.vehicles.push(v);
    apply_command(&mut s, &Command::ClearVehicleTimetableLateness(1)).unwrap();
    assert_eq!(s.vehicles[0].timetable_lateness, 0);
}

#[test]
fn autoreplace_only_when_old_skips_young_vehicle() {
    let mut s = GameState::new(8, 8);
    let depot = TileCoord::new(2, 2);
    apply_command(&mut s, &Command::PlaceRoad(TileCoord::new(1, 2))).unwrap();
    apply_command(&mut s, &Command::PlaceRoadDepotDir(depot, 0)).unwrap();
    apply_command(
        &mut s,
        &Command::BuildVehicleAtDepot(depot, crate::engine::ENGINE_TRUCK_MPS),
    )
    .unwrap();
    let id = s.vehicles[0].id;
    apply_command(
        &mut s,
        &Command::SetAutoReplaceRule {
            from_engine_id: crate::engine::ENGINE_TRUCK_MPS,
            to_engine_id: crate::engine::ENGINE_TRUCK_BALOGH_GOODS,
        },
    )
    .unwrap();
    apply_command(
        &mut s,
        &Command::ToggleAutoReplaceOnlyWhenOld {
            from_engine_id: crate::engine::ENGINE_TRUCK_MPS,
        },
    )
    .unwrap();
    assert!(!crate::autoreplace::try_autoreplace_vehicle(&mut s, id).unwrap());
}

#[test]
fn shared_orders_sync_linked_vehicles() {
    let mut s = GameState::new(8, 8);
    let depot = TileCoord::new(2, 2);
    apply_command(&mut s, &Command::PlaceRoad(TileCoord::new(1, 2))).unwrap();
    apply_command(&mut s, &Command::PlaceRoadDepotDir(depot, 0)).unwrap();
    apply_command(
        &mut s,
        &Command::BuildVehicleAtDepot(depot, crate::engine::ENGINE_BUS_MPS),
    )
    .unwrap();
    apply_command(
        &mut s,
        &Command::BuildVehicleAtDepot(depot, crate::engine::ENGINE_BUS_MPS),
    )
    .unwrap();
    let a = s.vehicles[0].id;
    let b = s.vehicles[1].id;
    s.vehicles[0].orders = vec![VehicleOrder::tile(depot)];
    apply_command(&mut s, &Command::CreateSharedOrdersFromVehicle(a)).unwrap();
    let shared_id = s.vehicles[0].shared_order_id.unwrap();
    apply_command(
        &mut s,
        &Command::LinkVehicleToSharedOrders {
            vehicle_id: b,
            shared_id,
        },
    )
    .unwrap();
    s.shared_order_lists[0].orders = vec![
        VehicleOrder::tile(depot),
        VehicleOrder::tile(TileCoord::new(3, 2)),
    ];
    crate::shared_orders::sync_shared_orders_to_vehicles(&mut s, shared_id);
    assert_eq!(s.vehicles[0].orders.len(), 2);
    assert_eq!(s.vehicles[1].orders.len(), 2);
}

#[test]
fn conditional_order_jumps_when_cargo_above_threshold() {
    let pos = TileCoord::new(1, 1);
    let mut v = Vehicle::new(1, VehicleKind::Truck, pos, pos);
    v.cargo = 60;
    v.capacity = 100;
    v.orders = vec![
        VehicleOrder::conditional(OrderConditionKind::CargoLoadAbove, 50, 2),
        VehicleOrder::tile(TileCoord::new(0, 0)),
        VehicleOrder::tile(TileCoord::new(2, 2)),
    ];
    v.current_order = 0;
    v.resolve_conditional_orders();
    assert_eq!(v.current_order, 2);
}

#[test]
fn depot_reorder_vehicle_slot_updates_display_order() {
    let mut s = GameState::new(8, 8);
    let depot = TileCoord::new(2, 2);
    apply_command(&mut s, &Command::PlaceRoad(TileCoord::new(1, 2))).unwrap();
    apply_command(&mut s, &Command::PlaceRoadDepotDir(depot, 0)).unwrap();
    apply_command(
        &mut s,
        &Command::BuildVehicleAtDepot(depot, crate::engine::ENGINE_BUS_MPS),
    )
    .unwrap();
    apply_command(
        &mut s,
        &Command::BuildVehicleAtDepot(depot, crate::engine::ENGINE_BUS_MPS),
    )
    .unwrap();
    apply_command(
        &mut s,
        &Command::DepotReorderVehicleSlot {
            depot_pos: depot,
            from_slot: 0,
            to_slot: 1,
        },
    )
    .unwrap();
    assert_eq!(s.vehicles[0].depot_display_slot, Some(1));
    assert_eq!(s.vehicles[1].depot_display_slot, Some(0));
}
