use crate::command::{Command, CommandError, apply_command, command_error_message};
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
    let mut s = GameState::new(8, 8);
    for y in 0..8 {
        for x in 0..8 {
            s.map.set_height(TileCoord::new(x, y), 4).unwrap();
        }
    }
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
    let mut s = GameState::new(8, 8);
    let c = TileCoord::new(3, 4);
    for y in 0..8 {
        for x in 0..8 {
            s.map.set_height(TileCoord::new(x, y), 4).unwrap();
        }
    }
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
    let mut s = GameState::new(8, 8);
    let c = TileCoord::new(3, 4);
    for y in 0..8 {
        for x in 0..8 {
            s.map.set_height(TileCoord::new(x, y), 4).unwrap();
        }
    }
    let money_before = s.economy.money;
    apply_command(&mut s, &Command::RaiseLand(c)).unwrap();
    assert_eq!(s.economy.money, money_before - TERRAFORM_COST);
    let (tileh, _) = tile_slope_and_z(&s.map, c).unwrap();
    assert_ne!(tileh, 0);
    assert_eq!(
        command_error_message(CommandError::TileNotTerraformable),
        "Solo se puede modificar el terreno en hierba o bosque libre."
    );
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
    for _ in 0..128 {
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
    const ERRORS: [CommandError; 25] = [
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
        let msg = command_error_message(err);
        assert!(!msg.is_empty(), "{err:?}");
        assert!(
            msg.chars().any(char::is_alphabetic),
            "mensaje sin letras para {err:?}: {msg}"
        );
    }
}
