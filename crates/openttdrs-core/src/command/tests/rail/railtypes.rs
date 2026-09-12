//! Tests de comandos ferroviarios — tipos de vía y engines.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::command::{Command, CommandError, apply_command, command_would_fail};
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
fn convert_rail_bridge_updates_both_ramps_and_scales_cost() {
    use crate::bridge_spec::{BridgeType, bridge_total_length};
    use crate::rail_type::{RailType, rail_type_from_tile};

    let mut s = GameState::new(10, 6);
    s.economy.money = 100_000;
    for x in 2..=3 {
        s.map
            .set_kind(TileCoord::new(x, 2), crate::TileKind::Water)
            .unwrap();
    }
    let start = TileCoord::new(1, 2);
    let end = TileCoord::new(4, 2);
    apply_command(
        &mut s,
        &Command::PlaceRailBridge(start, end, BridgeType::Wooden),
    )
    .unwrap();

    let money_before = s.economy.money;
    let per_trackbit = crate::economy::rail_convert_cost(
        &s.global_economy,
        RailType::Rail,
        RailType::Electric,
        &s.runtime.rail_type_props,
    );
    apply_command(
        &mut s,
        &Command::ConvertRail(start, RailType::Electric.as_u8()),
    )
    .unwrap();

    assert_eq!(
        rail_type_from_tile(s.map.get(start).unwrap()),
        RailType::Electric
    );
    assert_eq!(
        rail_type_from_tile(s.map.get(end).unwrap()),
        RailType::Electric
    );
    assert_eq!(
        s.economy.money,
        money_before - per_trackbit * i64::from(bridge_total_length(start, end))
    );
}

#[test]
fn convert_rail_bridge_rejects_endpoint_vehicle_atomically() {
    use crate::bridge_spec::BridgeType;
    use crate::rail_type::RailType;

    let mut s = GameState::new(10, 6);
    s.economy.money = 100_000;
    for x in 2..=3 {
        s.map
            .set_kind(TileCoord::new(x, 2), crate::TileKind::Water)
            .unwrap();
    }
    let start = TileCoord::new(1, 2);
    let end = TileCoord::new(4, 2);
    apply_command(
        &mut s,
        &Command::PlaceRailBridge(start, end, BridgeType::Wooden),
    )
    .unwrap();
    let before = [s.map.get(start).unwrap(), s.map.get(end).unwrap()];
    let money_before = s.economy.money;
    s.vehicles.push(crate::Vehicle::new(
        1,
        crate::VehicleKind::Train,
        start,
        end,
    ));
    let command = Command::ConvertRail(start, RailType::Monorail.as_u8());

    assert_eq!(
        command_would_fail(&s, &command),
        Some(CommandError::VehicleInTheWay)
    );
    assert_eq!(
        apply_command(&mut s, &command),
        Err(CommandError::VehicleInTheWay)
    );
    assert_eq!([s.map.get(start).unwrap(), s.map.get(end).unwrap()], before);
    assert_eq!(s.economy.money, money_before);
}

#[test]
fn convert_rail_tunnel_updates_both_mouths_and_keeps_middle_terrain() {
    use crate::rail_type::{RailType, rail_type_from_tile};

    let mut s = GameState::new(16, 16);
    s.economy.money = 100_000;
    let c = |x: i32, y: i32| TileCoord::new(x, y);
    s.map.set_height(c(5, 5), 2).unwrap();
    s.map.set_height(c(5, 6), 2).unwrap();
    s.map.set_height(c(6, 5), 1).unwrap();
    s.map.set_height(c(6, 6), 1).unwrap();
    s.map.set_height(c(3, 5), 1).unwrap();
    s.map.set_height(c(3, 6), 1).unwrap();
    s.map.set_height(c(4, 5), 2).unwrap();
    s.map.set_height(c(4, 6), 2).unwrap();
    let start = c(5, 5);
    let middle = c(4, 5);
    let end = c(3, 5);
    apply_command(&mut s, &Command::PlaceRailTunnel(start, end)).unwrap();

    let money_before = s.economy.money;
    let per_trackbit = crate::economy::rail_convert_cost(
        &s.global_economy,
        RailType::Rail,
        RailType::Electric,
        &s.runtime.rail_type_props,
    );
    apply_command(
        &mut s,
        &Command::ConvertRail(start, RailType::Electric.as_u8()),
    )
    .unwrap();

    assert_eq!(
        rail_type_from_tile(s.map.get(start).unwrap()),
        RailType::Electric
    );
    assert_eq!(
        rail_type_from_tile(s.map.get(end).unwrap()),
        RailType::Electric
    );
    assert_eq!(
        rail_type_from_tile(s.map.get(middle).unwrap()),
        RailType::Rail,
        "el vano conserva el terreno y no se convierte dos veces"
    );
    assert_eq!(s.economy.money, money_before - per_trackbit * 3);
}

#[test]
fn convert_rail_plain_costs_each_trackbit() {
    use crate::rail_type::{RailType, rail_type_from_tile};

    let mut s = GameState::new(8, 8);
    s.economy.money = 100_000;
    let c = TileCoord::new(3, 3);
    apply_command(&mut s, &Command::PlaceRailBits(c, crate::RAIL_TB_CROSS)).unwrap();
    let money_before = s.economy.money;
    let per_trackbit = crate::economy::rail_convert_cost(
        &s.global_economy,
        RailType::Rail,
        RailType::Monorail,
        &s.runtime.rail_type_props,
    );

    apply_command(&mut s, &Command::ConvertRail(c, RailType::Monorail.as_u8())).unwrap();

    assert_eq!(
        rail_type_from_tile(s.map.get(c).unwrap()),
        RailType::Monorail
    );
    assert_eq!(s.economy.money, money_before - per_trackbit * 2);
}

#[test]
fn convert_rail_uses_runtime_target_cost_multiplier() {
    use crate::rail_type::{RailType, rail_type_from_tile};

    let mut s = GameState::new(8, 8);
    s.economy.money = 100_000;
    s.global_economy = crate::economy::GlobalEconomy::new();
    s.runtime.rail_type_props[usize::from(RailType::Electric.as_u8())].cost_multiplier = 24;
    let c = TileCoord::new(3, 3);
    apply_command(&mut s, &Command::PlaceRail(c)).unwrap();
    let money_before = s.economy.money;
    let expected = crate::economy::rail_convert_cost(
        &s.global_economy,
        RailType::Rail,
        RailType::Electric,
        &s.runtime.rail_type_props,
    );

    apply_command(&mut s, &Command::ConvertRail(c, RailType::Electric.as_u8())).unwrap();

    assert_eq!(
        rail_type_from_tile(s.map.get(c).unwrap()),
        RailType::Electric
    );
    assert_eq!(s.economy.money, money_before - expected);
    assert_eq!(expected, 230, "RailConvertCost runtime con límite nativo");
}

#[test]
fn convert_rail_station_and_depot_updates_their_railtype_byte() {
    use crate::rail_type::{RailType, rail_type_from_tile};

    let mut s = GameState::new(12, 8);
    s.economy.money = 100_000;
    let rail = TileCoord::new(2, 3);
    let station = TileCoord::new(3, 3);
    apply_command(&mut s, &Command::PlaceRail(rail)).unwrap();
    apply_command(&mut s, &Command::PlaceRailStation(station, 0)).unwrap();
    let station_m6 = s.map.get(station).unwrap().m6;
    apply_command(
        &mut s,
        &Command::ConvertRail(station, RailType::Electric.as_u8()),
    )
    .unwrap();
    assert_eq!(
        rail_type_from_tile(s.map.get(station).unwrap()),
        RailType::Electric
    );
    assert_eq!(s.map.get(station).unwrap().m6, station_m6);

    let depot_rail = TileCoord::new(7, 3);
    let depot = TileCoord::new(7, 4);
    apply_command(&mut s, &Command::PlaceRail(depot_rail)).unwrap();
    apply_command(&mut s, &Command::PlaceRailDepotDir(depot, 3)).unwrap();
    apply_command(
        &mut s,
        &Command::ConvertRail(depot, RailType::Electric.as_u8()),
    )
    .unwrap();
    assert_eq!(
        rail_type_from_tile(s.map.get(depot).unwrap()),
        RailType::Electric
    );
}

#[test]
fn convert_rail_crossing_updates_railtype_without_touching_road_bits() {
    use crate::rail_type::{RailType, rail_type_from_tile};

    let mut s = GameState::new(8, 8);
    s.economy.money = 100_000;
    let crossing = TileCoord::new(3, 3);
    s.map.set_kind(crossing, crate::TileKind::Road).unwrap();
    let mut tile = s.map.get(crossing).unwrap();
    tile.mapt = crate::map::OTTD_MP_ROAD << 4;
    tile.m3 = 0x0A;
    tile.m5 = 0x01 | (1 << 6);
    tile.m8 = 0;
    s.map.set_tile(crossing, tile).unwrap();
    let m3_before = tile.m3;
    let m5_before = tile.m5;

    apply_command(
        &mut s,
        &Command::ConvertRail(crossing, RailType::Electric.as_u8()),
    )
    .unwrap();

    let after = s.map.get(crossing).unwrap();
    assert_eq!(rail_type_from_tile(after), RailType::Electric);
    assert_eq!(after.m3, m3_before);
    assert_eq!(after.m5, m5_before);
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
fn disabling_elrails_allows_electric_engine_on_normal_rail() {
    use crate::rail_type::{RailType, rail_type_from_tile};

    let mut s = SandboxMap::flat_rich(12, 8, 1);
    s.construction.disable_elrails = true;
    for x in 2..=6_i32 {
        apply_command(&mut s, &Command::PlaceRail(TileCoord::new(x, 4))).unwrap();
    }
    let depot = TileCoord::new(4, 5);
    apply_command(&mut s, &Command::PlaceRailDepotDir(depot, 3)).unwrap();

    apply_command(
        &mut s,
        &Command::BuildVehicleAtDepot(depot, crate::engine::ENGINE_TRAIN_ASIASTAR),
    )
    .unwrap();
    assert_eq!(
        rail_type_from_tile(s.map.get(TileCoord::new(4, 4)).unwrap()),
        RailType::Rail
    );
}

#[test]
fn disabling_elrails_makes_electric_to_rail_conversion_a_noop() {
    use crate::rail_type::{RailType, rail_type_from_tile};

    let mut s = GameState::new(8, 8);
    s.economy.money = 100_000;
    s.construction.disable_elrails = true;
    let c = TileCoord::new(3, 3);
    apply_command(&mut s, &Command::PlaceRail(c)).unwrap();
    apply_command(&mut s, &Command::ConvertRail(c, RailType::Electric.as_u8())).unwrap();
    let money_before = s.economy.money;
    apply_command(&mut s, &Command::ConvertRail(c, RailType::Rail.as_u8())).unwrap();
    assert_eq!(
        rail_type_from_tile(s.map.get(c).unwrap()),
        RailType::Electric
    );
    assert_eq!(s.economy.money, money_before);
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
fn place_rail_depot_uses_current_rail_type() {
    use crate::rail_type::{RailType, rail_type_from_tile};

    let mut s = GameState::new(10, 8);
    s.economy.money = 100_000;
    s.current_rail_type = RailType::Monorail;
    let rail = TileCoord::new(4, 4);
    let depot = TileCoord::new(4, 5);
    apply_command(&mut s, &Command::PlaceRail(rail)).unwrap();
    apply_command(&mut s, &Command::PlaceRailDepotDir(depot, 3)).unwrap();

    assert_eq!(
        rail_type_from_tile(s.map.get(depot).unwrap()),
        RailType::Monorail
    );
}

#[test]
fn place_rail_tunnel_uses_current_rail_type_on_both_mouths() {
    use crate::rail_type::{RailType, rail_type_from_tile};

    let mut s = GameState::new(16, 16);
    s.economy.money = 100_000;
    s.current_rail_type = RailType::Electric;
    let c = |x: i32, y: i32| TileCoord::new(x, y);
    s.map.set_height(c(5, 5), 2).unwrap();
    s.map.set_height(c(5, 6), 2).unwrap();
    s.map.set_height(c(6, 5), 1).unwrap();
    s.map.set_height(c(6, 6), 1).unwrap();
    s.map.set_height(c(3, 5), 1).unwrap();
    s.map.set_height(c(3, 6), 1).unwrap();
    s.map.set_height(c(4, 5), 2).unwrap();
    s.map.set_height(c(4, 6), 2).unwrap();

    let start = c(5, 5);
    let end = c(3, 5);
    apply_command(&mut s, &Command::PlaceRailTunnel(start, end)).unwrap();

    for mouth in [start, end] {
        assert_eq!(
            rail_type_from_tile(s.map.get(mouth).unwrap()),
            RailType::Electric
        );
    }
    assert_eq!(
        rail_type_from_tile(s.map.get(c(4, 5)).unwrap()),
        RailType::Rail,
        "la representación sintética del vano no es una boca nativa"
    );
}

#[test]
fn place_rail_bridge_uses_current_rail_type_on_both_ramps() {
    use crate::bridge_spec::BridgeType;
    use crate::rail_type::{RailType, rail_type_from_tile};

    let mut s = GameState::new(10, 6);
    s.economy.money = 100_000;
    s.current_rail_type = RailType::Maglev;
    for x in 2..=3 {
        s.map
            .set_kind(TileCoord::new(x, 2), crate::TileKind::Water)
            .unwrap();
    }
    let start = TileCoord::new(1, 2);
    let end = TileCoord::new(4, 2);
    apply_command(
        &mut s,
        &Command::PlaceRailBridge(start, end, BridgeType::Wooden),
    )
    .unwrap();

    for ramp in [start, end] {
        assert_eq!(
            rail_type_from_tile(s.map.get(ramp).unwrap()),
            RailType::Maglev
        );
    }
}

#[test]
fn rail_build_and_depot_costs_use_vanilla_railtype_multipliers() {
    use crate::economy::{rail_build_cost_factored, train_depot_build_cost};
    use crate::rail_type::{RailType, rail_type_from_tile};

    let mut s = GameState::new(10, 8);
    s.global_economy = crate::economy::GlobalEconomy::new();
    s.economy.money = 100_000;
    s.current_rail_type = RailType::Electric;
    let electric = TileCoord::new(3, 3);
    let money_before_electric = s.economy.money;
    apply_command(&mut s, &Command::PlaceRail(electric)).unwrap();
    assert_eq!(
        s.economy.money,
        money_before_electric - rail_build_cost_factored(&s.global_economy, 12)
    );
    assert_eq!(
        rail_type_from_tile(s.map.get(electric).unwrap()),
        RailType::Electric
    );

    s.current_rail_type = RailType::Monorail;
    let mono_rail = TileCoord::new(6, 4);
    let depot = TileCoord::new(6, 5);
    apply_command(&mut s, &Command::PlaceRail(mono_rail)).unwrap();
    let money_before_depot = s.economy.money;
    apply_command(&mut s, &Command::PlaceRailDepotDir(depot, 3)).unwrap();
    assert_eq!(
        s.economy.money,
        money_before_depot - train_depot_build_cost(&s.global_economy, 16)
    );
    assert_eq!(
        rail_type_from_tile(s.map.get(depot).unwrap()),
        RailType::Monorail
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
