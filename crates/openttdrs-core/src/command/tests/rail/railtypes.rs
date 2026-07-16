//! Tests de comandos ferroviarios — tipos de vía y engines.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::command::{Command, CommandError, apply_command};
use crate::test_fixtures::SandboxMap;
use crate::{GameState, TileCoord};

#[test]
fn convert_rail_preserves_trackbits_and_sets_electric() {
    use crate::rail_type::{RailType, rail_type_from_tile};

    let mut s = GameState::new(12, 8);
    s.economy.money = 100_000;
    let c = TileCoord::new(4, 4);
    apply_command(&mut s, &Command::PlaceRail(c)).unwrap();
    let before = s.map.get(c).unwrap();
    let bits = before.m5 & 0x3F;
    assert_eq!(rail_type_from_tile(before), RailType::Rail);

    apply_command(&mut s, &Command::ConvertRail(c, RailType::Electric.as_u8())).unwrap();
    let after = s.map.get(c).unwrap();
    assert_eq!(after.m5 & 0x3F, bits, "trackbits intactos");
    assert_eq!(rail_type_from_tile(after), RailType::Electric);

    apply_command(&mut s, &Command::ConvertRail(c, RailType::Rail.as_u8())).unwrap();
    assert_eq!(rail_type_from_tile(s.map.get(c).unwrap()), RailType::Rail);
}

#[test]
fn electric_engine_requires_electrified_neighbor() {
    use crate::rail_type::RailType;

    let mut s = SandboxMap::flat_rich(12, 8, 1);
    for x in 2..=6_i32 {
        apply_command(&mut s, &Command::PlaceRail(TileCoord::new(x, 4))).unwrap();
    }
    let depot = TileCoord::new(4, 5);
    apply_command(&mut s, &Command::PlaceRailDepotDir(depot, 3)).unwrap();

    let err = apply_command(
        &mut s,
        &Command::BuildVehicleAtDepot(depot, crate::engine::ENGINE_TRAIN_ASIASTAR),
    )
    .unwrap_err();
    assert_eq!(err, CommandError::EngineRequiresElectricRail);

    apply_command(
        &mut s,
        &Command::ConvertRail(TileCoord::new(4, 4), RailType::Electric.as_u8()),
    )
    .unwrap();
    apply_command(
        &mut s,
        &Command::BuildVehicleAtDepot(depot, crate::engine::ENGINE_TRAIN_ASIASTAR),
    )
    .unwrap();
    assert_eq!(
        s.vehicles[0].engine_id,
        Some(crate::engine::ENGINE_TRAIN_ASIASTAR)
    );
}

#[test]
fn place_rail_uses_current_rail_type() {
    use crate::rail_type::{RailType, rail_type_from_tile};

    let mut s = GameState::new(8, 8);
    s.economy.money = 50_000;
    s.current_rail_type = RailType::Electric;
    let c = TileCoord::new(3, 3);
    apply_command(&mut s, &Command::PlaceRail(c)).unwrap();
    assert_eq!(
        rail_type_from_tile(s.map.get(c).unwrap()),
        RailType::Electric
    );
}

#[test]
fn convert_rail_cycles_through_mono_and_maglev() {
    use crate::rail_type::{RailType, rail_type_from_tile};

    let mut s = GameState::new(8, 8);
    s.economy.money = 100_000;
    let c = TileCoord::new(3, 3);
    apply_command(&mut s, &Command::PlaceRail(c)).unwrap();
    for expected in [
        RailType::Electric,
        RailType::Monorail,
        RailType::Maglev,
        RailType::Rail,
    ] {
        apply_command(&mut s, &Command::ConvertRail(c, expected.as_u8())).unwrap();
        assert_eq!(rail_type_from_tile(s.map.get(c).unwrap()), expected);
    }
}

#[test]
fn monorail_engine_requires_monorail_neighbor() {
    use crate::rail_type::RailType;

    let mut s = SandboxMap::flat_rich(12, 8, 1);
    for x in 2..=6_i32 {
        apply_command(&mut s, &Command::PlaceRail(TileCoord::new(x, 4))).unwrap();
    }
    let depot = TileCoord::new(4, 5);
    apply_command(&mut s, &Command::PlaceRailDepotDir(depot, 3)).unwrap();

    let err = apply_command(
        &mut s,
        &Command::BuildVehicleAtDepot(depot, crate::engine::ENGINE_TRAIN_X2001),
    )
    .unwrap_err();
    assert_eq!(err, CommandError::EngineRequiresMonorail);

    apply_command(
        &mut s,
        &Command::ConvertRail(TileCoord::new(4, 4), RailType::Monorail.as_u8()),
    )
    .unwrap();
    apply_command(
        &mut s,
        &Command::BuildVehicleAtDepot(depot, crate::engine::ENGINE_TRAIN_X2001),
    )
    .unwrap();
}

#[test]
fn maglev_engine_requires_maglev_neighbor() {
    use crate::rail_type::RailType;

    let mut s = SandboxMap::flat_rich(12, 8, 1);
    for x in 2..=6_i32 {
        apply_command(&mut s, &Command::PlaceRail(TileCoord::new(x, 4))).unwrap();
    }
    let depot = TileCoord::new(4, 5);
    apply_command(&mut s, &Command::PlaceRailDepotDir(depot, 3)).unwrap();

    let err = apply_command(
        &mut s,
        &Command::BuildVehicleAtDepot(depot, crate::engine::ENGINE_TRAIN_LEV1),
    )
    .unwrap_err();
    assert_eq!(err, CommandError::EngineRequiresMaglev);

    apply_command(
        &mut s,
        &Command::ConvertRail(TileCoord::new(4, 4), RailType::Maglev.as_u8()),
    )
    .unwrap();
    apply_command(
        &mut s,
        &Command::BuildVehicleAtDepot(depot, crate::engine::ENGINE_TRAIN_LEV1),
    )
    .unwrap();
}

#[test]
fn monorail_path_does_not_cross_normal_rail() {
    use crate::pathfinder::find_rail_path_for_engine;
    use crate::rail_type::RailType;

    let mut s = SandboxMap::flat_rich(16, 8, 1);
    // Tramo mono 2..5 y tramo normal 6..10 (sin solape de tipo).
    for x in 2..=5_i32 {
        apply_command(&mut s, &Command::PlaceRail(TileCoord::new(x, 4))).unwrap();
        apply_command(
            &mut s,
            &Command::ConvertRail(TileCoord::new(x, 4), RailType::Monorail.as_u8()),
        )
        .unwrap();
    }
    for x in 6..=10_i32 {
        apply_command(&mut s, &Command::PlaceRail(TileCoord::new(x, 4))).unwrap();
    }
    let from = TileCoord::new(2, 4);
    let to = TileCoord::new(10, 4);
    assert!(
        find_rail_path_for_engine(&s.map, from, to, None, None).is_some(),
        "sin filtro debería cruzar"
    );
    assert!(
        find_rail_path_for_engine(
            &s.map,
            from,
            to,
            None,
            Some(crate::engine::ENGINE_TRAIN_X2001)
        )
        .is_none(),
        "X2001 no puede salir de la red monorail"
    );
}
