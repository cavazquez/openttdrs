use crate::command::{Command, CommandError, apply_command};
use crate::test_fixtures::SandboxMap;
use crate::{
    GameState, IndustryKind, IndustrySpec, LevelMode, TERRAFORM_COST, TileCoord, TileKind,
    industry_template, tile_slope_and_z,
};

#[test]
fn command_sequence_is_deterministic() {
    let cmds = [
        Command::PlaceRoad(TileCoord::new(0, 0)),
        Command::PlaceRail(TileCoord::new(0, 1)),
        Command::PlaceRoad(TileCoord::new(1, 0)),
        Command::PlaceStation(TileCoord::new(2, 0)),
        Command::ClearTile(TileCoord::new(1, 0)),
    ];
    let mut a = GameState::new(8, 8);
    let mut b = GameState::new(8, 8);
    for cmd in &cmds {
        apply_command(&mut a, cmd).unwrap();
        apply_command(&mut b, cmd).unwrap();
    }
    let ja = a.save_json().unwrap();
    let jb = b.save_json().unwrap();
    assert_eq!(ja, jb);
}

#[test]
fn level_land_drag_flattens_area() {
    let mut s = SandboxMap::flat(8, 8, 4);
    apply_command(&mut s, &Command::RaiseLand(TileCoord::new(3, 3))).unwrap();
    apply_command(
        &mut s,
        &Command::LevelLand {
            from: TileCoord::new(2, 2),
            to: TileCoord::new(4, 4),
            mode: LevelMode::Level,
        },
    )
    .unwrap();
    for y in 2..=4 {
        for x in 2..=4 {
            assert_eq!(s.map.get(TileCoord::new(x, y)).unwrap().height, 4);
        }
    }
}

#[test]
fn raise_then_lower_restores_flat_grass() {
    let mut s = SandboxMap::flat(8, 8, 4);
    let c = TileCoord::new(3, 4);
    apply_command(&mut s, &Command::RaiseLand(c)).unwrap();
    assert_ne!(tile_slope_and_z(&s.map, c).unwrap().0, 0);
    apply_command(&mut s, &Command::LowerLand(c)).unwrap();
    assert_eq!(tile_slope_and_z(&s.map, c).unwrap().0, 0);
    assert_eq!(s.map.get(c).unwrap().height, 4);
}

#[test]
fn lower_land_rejects_sea_level() {
    let mut s = GameState::new(8, 8);
    let c = TileCoord::new(1, 1);
    s.map.set_height(c, 0).unwrap();
    assert_eq!(
        apply_command(&mut s, &Command::LowerLand(c)),
        Err(CommandError::TerrainTooLow)
    );
}

#[test]
fn raise_land_on_grass_costs_and_creates_slope() {
    let mut s = SandboxMap::flat(8, 8, 4);
    let c = TileCoord::new(3, 4);
    let money_before = s.economy.money;
    apply_command(&mut s, &Command::RaiseLand(c)).unwrap();
    assert_eq!(s.economy.money, money_before - TERRAFORM_COST);
    let (tileh, _) = tile_slope_and_z(&s.map, c).unwrap();
    assert_ne!(tileh, 0);
    // CommandError::Display ahora retorna el nombre técnico del variant (sin mensajes UI)
    assert!(!CommandError::TileNotTerraformable.to_string().is_empty());
}

#[test]
fn set_vehicle_orders_assigns_circular_route() {
    let mut s = GameState::new(8, 8);
    s.vehicles.push(crate::Vehicle::new(
        7,
        crate::VehicleKind::Truck,
        TileCoord::new(0, 0),
        TileCoord::new(1, 0),
    ));
    apply_command(
        &mut s,
        &Command::SetVehicleOrders(7, vec![TileCoord::new(2, 0), TileCoord::new(2, 2)]),
    )
    .unwrap();
    assert_eq!(s.vehicles[0].dest, TileCoord::new(2, 0));
    assert_eq!(s.vehicles[0].orders.len(), 2);
}

#[test]
fn sandbox_commands_place_visible_tile_kinds() {
    let mut s = GameState::new(8, 8);
    apply_command(&mut s, &Command::PlaceHouse(TileCoord::new(1, 1))).unwrap();
    apply_command(&mut s, &Command::PlaceIndustry(TileCoord::new(2, 1))).unwrap();
    apply_command(&mut s, &Command::PlaceForest(TileCoord::new(3, 1))).unwrap();
    apply_command(
        &mut s,
        &Command::PlaceIndustryKind(TileCoord::new(4, 1), IndustryKind::CoalMine),
    )
    .unwrap();
    assert_eq!(s.map.get_kind(TileCoord::new(1, 1)), Some(TileKind::House));
    assert_eq!(
        s.map.get_kind(TileCoord::new(2, 1)),
        Some(TileKind::Industry)
    );
    assert_eq!(s.map.get_kind(TileCoord::new(3, 1)), Some(TileKind::Forest));
    assert_eq!(
        s.map.get_kind(TileCoord::new(4, 1)),
        Some(TileKind::Industry)
    );
    // CoalMine ahora ocupa múltiples tiles (2x2).
    assert_eq!(
        s.map.get_kind(TileCoord::new(5, 1)),
        Some(TileKind::Industry)
    );
    assert_eq!(
        s.map.get_kind(TileCoord::new(4, 2)),
        Some(TileKind::Industry)
    );
    assert_eq!(
        s.map.get_kind(TileCoord::new(5, 2)),
        Some(TileKind::Industry)
    );
    assert!(s.industries.iter().any(|industry| {
        industry.pos == TileCoord::new(4, 1) && industry.kind == IndustryKind::CoalMine
    }));
}

#[test]
fn toyland_industry_rejected_on_temperate_map() {
    let mut s = GameState::new(16, 16);
    assert_eq!(
        apply_command(
            &mut s,
            &Command::PlaceIndustrySpec(TileCoord::new(5, 5), IndustrySpec::FizzyDrinkFactory),
        )
        .unwrap_err(),
        CommandError::IndustryNotAvailableInClimate,
    );
}

#[test]
fn place_industry_spec_starts_construction_in_progress() {
    let mut s = GameState::new(16, 16);
    let origin = TileCoord::new(5, 5);
    apply_command(
        &mut s,
        &Command::PlaceIndustrySpec(origin, IndustrySpec::Sawmill),
    )
    .unwrap();
    for (coord, _) in industry_template(origin, IndustrySpec::Sawmill) {
        let Some(tile) = s.map.get(coord) else {
            panic!("tesela del footprint {coord:?}");
        };
        assert_eq!(tile.kind, TileKind::Industry);
        assert_eq!(tile.m1 & 0x80, 0, "obra en curso en {coord:?}");
        assert_eq!(tile.m2, 1);
        // P7: MakeIndustry siembra m3 y deja triggers (m6 bits 3–5) a 0.
        assert_eq!(
            crate::industry_random_triggers(&tile),
            0,
            "triggers limpios en {coord:?}"
        );
        // Determinista con world_seed=0; al menos el byte queda escrito (puede ser 0).
        let _ = crate::industry_random_bits(&tile);
    }
}

#[test]
fn industry_construction_completes_over_sim_ticks() {
    let mut s = GameState::new(16, 16);
    let origin = TileCoord::new(5, 5);
    apply_command(
        &mut s,
        &Command::PlaceIndustrySpec(origin, IndustrySpec::Sawmill),
    )
    .unwrap();
    // RunTileLoop: ~12 visitas/tesela × 256 ticks (counter×stages hasta completed).
    for _ in 0..4_096 {
        s.step();
    }
    for (coord, _) in industry_template(origin, IndustrySpec::Sawmill) {
        let Some(tile) = s.map.get(coord) else {
            panic!("footprint {coord:?}");
        };
        assert_ne!(tile.m1 & 0x80, 0, "terminada en {coord:?}");
    }
}

#[test]
fn clear_any_industry_tile_removes_whole_industry_footprint() {
    let mut s = GameState::new(10, 10);
    let origin = TileCoord::new(2, 2);
    apply_command(
        &mut s,
        &Command::PlaceIndustryKind(origin, IndustryKind::Factory),
    )
    .unwrap();
    assert_eq!(s.industries.len(), 1);
    let target_inside = TileCoord::new(3, 2);
    apply_command(&mut s, &Command::ClearTile(target_inside)).unwrap();
    assert!(s.industries.is_empty());
    // Factory template cubre también (4,3).
    assert_eq!(s.map.get_kind(TileCoord::new(4, 3)), Some(TileKind::Grass));
}

#[test]
fn every_command_error_has_user_message() {
    const ERRORS: [CommandError; 27] = [
        CommandError::OutOfBounds,
        CommandError::CannotPlaceRoadOnWater,
        CommandError::CannotPlaceRoadOnVoid,
        CommandError::CannotPlaceRailOnWater,
        CommandError::CannotPlaceRailOnVoid,
        CommandError::CannotPlaceStationOnWater,
        CommandError::CannotPlaceStationOnVoid,
        CommandError::CannotPlaceStationOnOccupiedTile,
        CommandError::StationNotAdjacentToTransport,
        CommandError::StationAlreadyExists,
        CommandError::StationNotFound,
        CommandError::VehicleNotFound,
        CommandError::VehicleNotOwned,
        CommandError::TileNotOwned,
        CommandError::VehicleNotInDepot,
        CommandError::InvalidDepotTile,
        CommandError::VehicleKindNotAllowed,
        CommandError::EngineNotFound,
        CommandError::InsufficientFunds,
        CommandError::IncompatibleStopForVehicle,
        CommandError::InvalidTunnelEndpoints,
        CommandError::InvalidBridgeSpan,
        CommandError::InvalidRailOnSlope,
        CommandError::CannotPlaceWaypointOnTrack,
        CommandError::NoRailToRemove,
        CommandError::CannotPlaceSignalOnTrack,
        CommandError::SignalAlreadyPresent,
    ];
    for err in ERRORS {
        let msg = err.to_string();
        assert!(!msg.is_empty(), "{err:?}");
        assert!(
            msg.chars().any(char::is_alphabetic),
            "mensaje sin letras para {err:?}: {msg}"
        );
    }
}

#[test]
fn place_rail_and_road_write_active_company_owner_m1() {
    let mut s = GameState::new(8, 8);
    s.ensure_rival_transcargo();
    let rival = crate::CompanyId(1);
    assert!(s.set_active_company(rival));
    let rail = TileCoord::new(2, 2);
    let road = TileCoord::new(3, 3);
    apply_command(&mut s, &Command::PlaceRail(rail)).unwrap();
    apply_command(&mut s, &Command::PlaceRoad(road)).unwrap();
    assert_eq!(s.map.get(rail).unwrap().m1, rival.0);
    assert_eq!(s.map.get(road).unwrap().m1, rival.0);
    assert_eq!(
        crate::CompanyId::from_tile_m1(s.map.get(rail).unwrap().m1, s.companies.len()),
        rival
    );
}

#[test]
fn place_rail_tunnel_and_bridge_write_active_company_owner_m1() {
    use crate::{BridgeType, TileKind};

    let mut s = SandboxMap::flat_rich(16, 16, 1);
    s.ensure_rival_transcargo();
    let rival = crate::CompanyId(1);
    assert!(s.set_active_company(rival));

    let c = |x: i32, y: i32| TileCoord::new(x, y);
    // Huella de túnel del golden / station tests.
    s.map.set_height(c(5, 5), 2).unwrap();
    s.map.set_height(c(5, 6), 2).unwrap();
    s.map.set_height(c(6, 5), 1).unwrap();
    s.map.set_height(c(6, 6), 1).unwrap();
    s.map.set_height(c(3, 5), 1).unwrap();
    s.map.set_height(c(3, 6), 1).unwrap();
    s.map.set_height(c(4, 5), 2).unwrap();
    s.map.set_height(c(4, 6), 2).unwrap();
    apply_command(&mut s, &Command::PlaceRailTunnel(c(5, 5), c(3, 5))).unwrap();
    for pos in [c(5, 5), c(4, 5), c(3, 5)] {
        assert_eq!(
            s.map.get(pos).unwrap().m1,
            rival.0,
            "túnel {pos:?} debe ser de la compañía activa"
        );
        assert_eq!(s.map.get_kind(pos), Some(TileKind::RailTunnel));
    }

    for x in 2..=5 {
        s.map.set_kind(c(x, 10), TileKind::Water).unwrap();
    }
    let west = c(1, 10);
    let east = c(6, 10);
    apply_command(
        &mut s,
        &Command::PlaceRailBridge(west, east, BridgeType::Wooden),
    )
    .unwrap();
    assert_eq!(s.map.get(west).unwrap().m1, rival.0);
    assert_eq!(s.map.get(east).unwrap().m1, rival.0);

    // Rival no deja demoler al jugador.
    assert!(s.set_active_company(crate::CompanyId::PLAYER));
    assert_eq!(
        apply_command(&mut s, &Command::ClearTile(c(5, 5))).unwrap_err(),
        CommandError::TileNotOwned
    );
    assert_eq!(
        apply_command(&mut s, &Command::ClearTile(west)).unwrap_err(),
        CommandError::TileNotOwned
    );
}

#[test]
fn toggle_vehicle_running_rejects_other_company_owner() {
    let mut s = SandboxMap::flat_rich(8, 8, 1);
    s.ensure_rival_transcargo();
    let depot = TileCoord::new(2, 2);
    apply_command(&mut s, &Command::PlaceRoad(TileCoord::new(1, 2))).unwrap();
    apply_command(&mut s, &Command::PlaceRoadDepotDir(depot, 0)).unwrap();
    apply_command(
        &mut s,
        &Command::BuildRoadVehicleAtDepot(depot, crate::VehicleKind::Bus),
    )
    .unwrap();
    let id = s.vehicles[0].id;
    assert_eq!(s.vehicles[0].owner, crate::CompanyId::PLAYER);
    assert!(s.set_active_company(crate::CompanyId(1)));
    let err = apply_command(&mut s, &Command::ToggleVehicleRunning(id)).unwrap_err();
    assert_eq!(err, CommandError::VehicleNotOwned);
    assert!(!s.vehicles[0].running);
}

#[test]
fn clear_and_remove_rail_reject_foreign_owned_infra() {
    let mut s = SandboxMap::flat_rich(8, 8, 1);
    s.ensure_rival_transcargo();
    let rival = crate::CompanyId(1);
    assert!(s.set_active_company(rival));
    let rail = TileCoord::new(3, 3);
    let road = TileCoord::new(4, 4);
    apply_command(&mut s, &Command::PlaceRail(rail)).unwrap();
    apply_command(&mut s, &Command::PlaceRoad(road)).unwrap();
    assert!(s.set_active_company(crate::CompanyId::PLAYER));
    assert_eq!(
        apply_command(&mut s, &Command::ClearTile(rail)).unwrap_err(),
        CommandError::TileNotOwned
    );
    assert_eq!(
        apply_command(&mut s, &Command::RemoveRail(rail)).unwrap_err(),
        CommandError::TileNotOwned
    );
    assert_eq!(
        apply_command(&mut s, &Command::ClearTile(road)).unwrap_err(),
        CommandError::TileNotOwned
    );
    assert_eq!(s.map.get_kind(rail), Some(TileKind::Rail));
    assert_eq!(s.map.get_kind(road), Some(TileKind::Road));
}

#[test]
fn place_rail_bits_rejects_overwrite_of_foreign_rail() {
    let mut s = SandboxMap::flat_rich(8, 8, 1);
    s.ensure_rival_transcargo();
    assert!(s.set_active_company(crate::CompanyId(1)));
    let c = TileCoord::new(2, 2);
    apply_command(&mut s, &Command::PlaceRail(c)).unwrap();
    assert!(s.set_active_company(crate::CompanyId::PLAYER));
    assert_eq!(
        apply_command(&mut s, &Command::PlaceRailBits(c, 0x02)).unwrap_err(),
        CommandError::TileNotOwned
    );
    assert_eq!(s.map.get(c).unwrap().m1, 1);
}

#[test]
fn cheat_set_year_and_switch_company_require_enabled() {
    let mut s = GameState::new(8, 8);
    s.ensure_rival_transcargo();
    assert_eq!(
        apply_command(&mut s, &Command::CheatSetYear(2000)).unwrap_err(),
        CommandError::CheatsDisabled
    );
    apply_command(&mut s, &Command::CheatSetEnabled(true)).unwrap();
    apply_command(&mut s, &Command::CheatSetYear(2000)).unwrap();
    let (year, _) = crate::calendar_year_day(crate::calendar_day_index(s.tick));
    assert_eq!(year, 2000);
    assert_eq!(
        apply_command(&mut s, &Command::CheatSetYear(1000)).unwrap_err(),
        CommandError::InvalidCheatYear
    );
    apply_command(&mut s, &Command::CheatSwitchCompany(crate::CompanyId(1))).unwrap();
    assert_eq!(s.active_company, crate::CompanyId(1));
    assert_eq!(
        apply_command(&mut s, &Command::CheatSwitchCompany(crate::CompanyId(99))).unwrap_err(),
        CommandError::CompanyNotFound
    );
}

#[test]
fn magic_bulldozer_clears_foreign_tile() {
    let mut s = SandboxMap::flat_rich(8, 8, 1);
    s.ensure_rival_transcargo();
    assert!(s.set_active_company(crate::CompanyId(1)));
    let rail = TileCoord::new(3, 3);
    apply_command(&mut s, &Command::PlaceRail(rail)).unwrap();
    assert!(s.set_active_company(crate::CompanyId::PLAYER));
    apply_command(&mut s, &Command::CheatSetEnabled(true)).unwrap();
    apply_command(&mut s, &Command::CheatToggleMagicBulldozer).unwrap();
    apply_command(&mut s, &Command::ClearTile(rail)).unwrap();
    assert_eq!(s.map.get_kind(rail), Some(TileKind::Grass));
}

#[test]
fn set_pathfinding_settings_clamps_and_is_idempotent() {
    let mut s = GameState::new(8, 8);
    let custom = crate::PathfindingSettings {
        wait_for_pbs_path: 10,
        path_backoff_interval: 60,
        reverse_at_signals: false,
    };
    apply_command(&mut s, &Command::SetPathfindingSettings(custom)).unwrap();
    assert_eq!(s.pathfinding, custom);

    apply_command(&mut s, &Command::SetPathfindingSettings(custom)).unwrap();
    assert_eq!(s.pathfinding, custom);

    let too_low = crate::PathfindingSettings {
        wait_for_pbs_path: 1,
        path_backoff_interval: 0,
        reverse_at_signals: true,
    };
    apply_command(&mut s, &Command::SetPathfindingSettings(too_low)).unwrap();
    assert_eq!(s.pathfinding.wait_for_pbs_path, 2);
    assert_eq!(s.pathfinding.path_backoff_interval, 1);
    assert!(s.pathfinding.reverse_at_signals);

    apply_command(
        &mut s,
        &Command::SetPathfindingSettings(crate::PathfindingSettings::default()),
    )
    .unwrap();
    assert_eq!(s.pathfinding, crate::PathfindingSettings::default());
}

#[test]
fn set_cargo_dist_distribution_rebuilds_flows() {
    use crate::flow_stat::DistributionType;

    let mut s = GameState::new(8, 8);
    assert_eq!(s.cargo_dist.distribution, DistributionType::Manual);
    apply_command(
        &mut s,
        &Command::SetCargoDistDistribution(DistributionType::Asymmetric),
    )
    .unwrap();
    assert_eq!(s.cargo_dist.distribution, DistributionType::Asymmetric);
    // Mapa vacío: rebuild no debe panicar.
    apply_command(
        &mut s,
        &Command::SetCargoDistDistribution(DistributionType::Asymmetric),
    )
    .unwrap();
    apply_command(
        &mut s,
        &Command::SetCargoDistDistribution(DistributionType::Symmetric),
    )
    .unwrap();
    assert_eq!(s.cargo_dist.distribution, DistributionType::Symmetric);
}
