//! Tests de comandos ferroviarios — señales.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::command::{Command, apply_command};
use crate::{GameState, TileCoord, TileKind};

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
fn cycle_rail_signal_type_full_openttd_order() {
    use crate::rail_signals::{
        SIGTYPE_BLOCK, SIGTYPE_COMBO, SIGTYPE_ENTRY, SIGTYPE_EXIT, SIGTYPE_PATH,
        SIGTYPE_PATH_ONEWAY, SignalTrack, signal_type_for_track,
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
    let expected = [
        SIGTYPE_ENTRY,
        SIGTYPE_EXIT,
        SIGTYPE_COMBO,
        SIGTYPE_PATH,
        SIGTYPE_PATH_ONEWAY,
        SIGTYPE_BLOCK,
    ];
    for want in expected {
        apply_command(&mut s, &Command::CycleRailSignalType(c, 128, 128)).unwrap();
        assert_eq!(signal_type_for_track(s.map.get(c).unwrap().m2, track), want);
    }
}

#[test]
fn explicit_and_cycled_signal_variant_preserve_signal_type() {
    use crate::rail_signals::{
        SIGTYPE_PATH, SignalTrack, signal_type_for_track, signal_variant_for_track,
    };

    let mut s = GameState::new(8, 8);
    let c = TileCoord::new(2, 2);
    apply_command(&mut s, &Command::SetRailBits(c, 0x01)).unwrap();
    apply_command(
        &mut s,
        &Command::PlaceRailSignalWithVariant(c, 0, 128, 128, SIGTYPE_PATH, 1),
    )
    .unwrap();
    let tile = s.map.get(c).unwrap();
    assert_eq!(signal_type_for_track(tile.m2, SignalTrack::X), SIGTYPE_PATH);
    assert_eq!(signal_variant_for_track(tile.m2, SignalTrack::X), 1);

    apply_command(&mut s, &Command::CycleRailSignalVariant(c, 128, 128)).unwrap();
    let tile = s.map.get(c).unwrap();
    assert_eq!(signal_type_for_track(tile.m2, SignalTrack::X), SIGTYPE_PATH);
    assert_eq!(signal_variant_for_track(tile.m2, SignalTrack::X), 0);
}

#[test]
fn place_presignal_types_write_m2() {
    use crate::rail_signals::{
        SIGTYPE_COMBO, SIGTYPE_ENTRY, SIGTYPE_EXIT, SignalTrack, signal_type_for_track,
    };

    for sig_type in [SIGTYPE_ENTRY, SIGTYPE_EXIT, SIGTYPE_COMBO] {
        let mut s = GameState::new(8, 8);
        let c = TileCoord::new(2, 2);
        apply_command(&mut s, &Command::SetRailBits(c, 0x01)).unwrap();
        apply_command(&mut s, &Command::PlaceRailSignal(c, 0, 128, 128, sig_type)).unwrap();
        assert_eq!(
            signal_type_for_track(s.map.get(c).unwrap().m2, SignalTrack::X),
            sig_type
        );
    }
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
        &Command::PlaceRailSignalWithVariant(c, 0, 64, 64, crate::rail_signals::SIGTYPE_BLOCK, 1),
    )
    .unwrap();
    apply_command(
        &mut s,
        &Command::PlaceRailSignalWithVariant(c, 1, 200, 200, crate::rail_signals::SIGTYPE_BLOCK, 1),
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
        &Command::PlaceRailSignalWithVariant(c, 0, 64, 64, crate::rail_signals::SIGTYPE_BLOCK, 1),
    )
    .unwrap();
    let m2_upper = s.map.get(c).unwrap().m2;
    assert_ne!(m2_upper, 0);
    apply_command(
        &mut s,
        &Command::PlaceRailSignalWithVariant(c, 1, 200, 200, crate::rail_signals::SIGTYPE_BLOCK, 1),
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
