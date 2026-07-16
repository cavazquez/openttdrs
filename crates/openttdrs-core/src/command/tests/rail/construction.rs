//! Tests de comandos ferroviarios — vía, trackbits y cache de path.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::command::{Command, CommandError, apply_command};
use crate::{GameState, LevelMode, RAIL_BUILD_COST, TileCoord, TileKind, tile_slope_and_z};

use super::super::helpers::{
    finish_train_with_cached_path_to_depot, flat_map_for_terraform_tests, set_w_only_slope,
    train_with_cached_path_to_depot,
};

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
