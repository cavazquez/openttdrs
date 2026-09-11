//! Tests de construcción acuática (depósito, muelle, boya, acueducto).

use crate::economy::{ship_depot_build_cost, ship_depot_clear_cost, station_build_cost};
use crate::test_fixtures::SandboxMap;
use crate::{
    CargoType, Command, GameState, StopKind, TileCoord, TileKind, Vehicle, VehicleKind,
    VehicleOrder, WaterClass, apply_command, bridge_above_axis_from_mapt, command_would_fail,
    set_water_class_m1,
};

#[test]
fn place_ship_depot_on_water_with_water_entrance() {
    let mut s = GameState::new(12, 12);
    let depot = TileCoord::new(4, 4);
    let mouth = TileCoord::new(3, 4); // dir 0 → (-1,0)
    let other = TileCoord::new(5, 4); // segunda parte de la huella.
    for coord in [depot, mouth, other] {
        s.map.set_kind(coord, TileKind::Water).unwrap();
    }
    let money = s.economy.money;
    apply_command(&mut s, &Command::PlaceShipDepotDir(depot, 0)).unwrap();
    let tile = s.map.get(depot).expect("depósito construido");
    let other_tile = s.map.get(other).expect("segunda parte construida");
    assert_eq!(tile.kind, TileKind::ShipDepot);
    assert_eq!(tile.mapt, 0x60, "MP_WATER conserva el tipo alto canónico");
    assert_eq!(tile.m5, 0x30, "WaterTileType::Depot vigente");
    assert_eq!(other_tile.kind, TileKind::ShipDepot);
    assert_eq!(other_tile.m5, 0x31, "parte opuesta sobre el eje X");
    assert_eq!(crate::depot::depot_id_from_tile(tile), Some(0));
    assert_eq!(crate::depot::depot_id_from_tile(other_tile), Some(0));
    assert_eq!(s.depots.len(), 1);
    assert_eq!(s.depots[0].depot_id, 0);
    assert_eq!(s.depots[0].tile, depot);
    assert_eq!(
        s.depots[0].build_date,
        crate::news::openttd_date_from_calendar_day_index(u64::from(s.calendar.date))
    );
    assert_eq!(
        s.economy.money,
        money - ship_depot_build_cost(&s.global_economy)
    );
}

#[test]
fn place_ship_depot_needs_only_the_two_footprint_tiles_on_water() {
    // El resto de los vecinos queda como tierra. OpenTTD no exige una tercera
    // tesela de agua delante de la boca: sólo valida las dos teselas que el
    // depósito reemplaza.
    let cases = [
        (0u8, TileCoord::new(2, 2)),
        (1, TileCoord::new(2, 2)),
        (2, TileCoord::new(1, 2)),
        (3, TileCoord::new(2, 1)),
    ];

    for (dir, depot) in cases {
        let mut s = GameState::new(4, 4);
        let [origin, other] = crate::ship_depot_footprint(depot, dir);
        for coord in [origin, other] {
            s.map.set_kind(coord, TileKind::Water).unwrap();
        }
        let command = Command::PlaceShipDepotDir(depot, dir);

        assert_eq!(
            command_would_fail(&s, &command),
            None,
            "preview dir={dir} no debe inventar una tercera condición de agua"
        );
        apply_command(&mut s, &command).expect("huella naval sobre dos aguas");
        assert_eq!(s.map.get_kind(origin), Some(TileKind::ShipDepot));
        assert_eq!(s.map.get_kind(other), Some(TileKind::ShipDepot));
    }
}

#[test]
fn place_ship_depot_writes_current_raw_contract_and_active_owner() {
    let mut s = GameState::new(12, 12);
    s.ensure_rival_transcargo();
    let rival = crate::CompanyId(1);
    assert!(s.set_active_company(rival));

    let depot = TileCoord::new(4, 4);
    let mouth = TileCoord::new(4, 3); // dir 3 → norte en la grilla del mapa.
    let other = TileCoord::new(4, 5);
    for coord in [depot, mouth, other] {
        s.map.set_kind(coord, TileKind::Water).unwrap();
    }
    let mut original = s.map.get(depot).expect("agua del depósito");
    original.mapt = 0x60 | 0x02; // MP_WATER + zona climática persistida.
    original.m1 = set_water_class_m1(original.m1 | 0x80, WaterClass::Canal);
    original.m2 = 0xFE;
    original.m2_hi = 0xAA;
    original.m3 = 0x81;
    original.m3hi = 0x91;
    original.m6 = 0xFC;
    original.m7 = 0xAB;
    original.m8 = 0xBEEF;
    s.map.set_tile(depot, original).unwrap();
    let mut other_original = s.map.get(other).expect("agua de la parte opuesta");
    other_original.mapt = 0x60 | 0x03;
    other_original.m1 = set_water_class_m1(other_original.m1, WaterClass::River);
    other_original.m6 = 0xFD;
    s.map.set_tile(other, other_original).unwrap();

    apply_command(&mut s, &Command::PlaceShipDepotDir(depot, 3)).unwrap();

    let tile = s.map.get(depot).expect("depósito construido");
    let other_tile = s.map.get(other).expect("parte opuesta construida");
    assert_eq!(tile.kind, TileKind::ShipDepot);
    assert_eq!(tile.mapt, 0x62, "se conserva la zona climática de MAPT");
    assert_eq!(tile.m5, 0x32, "tipo Depot + parte/eje de la orientación");
    assert_eq!(crate::depot::depot_id_from_tile(tile), Some(0));
    assert_eq!(crate::depot::depot_id_from_tile(other_tile), Some(0));
    assert_eq!(tile.m3, 0);
    assert_eq!(tile.m3hi, 0);
    assert_eq!(tile.m6, 0, "MakeShipDepot limpia el contador alto");
    assert_eq!(tile.m7, 0);
    assert_eq!(tile.m8, 0);
    assert_eq!(tile.m1 & 0x80, 0, "se limpia DockingTile");
    assert_eq!(
        crate::map::water_class(tile),
        Some(WaterClass::Canal),
        "SetTileOwner no debe destruir WaterClass"
    );
    assert_eq!(
        crate::CompanyId::from_tile_m1(tile.m1, s.companies.len()),
        rival,
        "el depósito queda a nombre de la compañía activa"
    );
    assert_eq!(other_tile.kind, TileKind::ShipDepot);
    assert_eq!(
        other_tile.mapt, 0x63,
        "cada parte conserva su zona climática"
    );
    assert_eq!(other_tile.m5, 0x33, "parte opuesta del eje Y");
    assert_eq!(crate::map::water_class(other_tile), Some(WaterClass::River));
    assert_eq!(other_tile.m6, 1);
    assert_eq!(
        crate::CompanyId::from_tile_m1(other_tile.m1, s.companies.len()),
        rival
    );
}

#[test]
fn place_ship_depot_rejects_land() {
    let mut s = GameState::new(8, 8);
    let e =
        apply_command(&mut s, &Command::PlaceShipDepotDir(TileCoord::new(2, 2), 0)).unwrap_err();
    assert!(matches!(
        e,
        crate::CommandError::CannotPlaceStationOnOccupiedTile
    ));
}

#[test]
fn place_ship_depot_rejects_bridge_above_both_parts() {
    for dir in 0..4_u8 {
        for part in 0..2_usize {
            let mut s = GameState::new(8, 8);
            let depot = TileCoord::new(3, 3);
            let footprint = crate::ship_depot_footprint(depot, dir);
            for coord in footprint {
                s.map.set_kind(coord, TileKind::Water).unwrap();
            }
            let bridge_tile = footprint[part];
            let mut bridge = s.map.get(bridge_tile).unwrap();
            bridge.mapt = crate::bridge_spec::set_bridge_middle_mapt(bridge.mapt, dir & 1 != 0);
            s.map.set_tile(bridge_tile, bridge).unwrap();
            let money = s.economy.money;
            let command = Command::PlaceShipDepotDir(depot, dir);

            assert_eq!(
                command_would_fail(&s, &command),
                Some(crate::CommandError::MustDemolishBridgeFirst),
                "preview dir={dir} part={part} debe ver el puente superior"
            );
            assert_eq!(
                apply_command(&mut s, &command),
                Err(crate::CommandError::MustDemolishBridgeFirst),
                "ejecución dir={dir} part={part} debe rechazar el puente superior"
            );
            assert!(
                footprint
                    .into_iter()
                    .all(|coord| s.map.get_kind(coord) == Some(TileKind::Water))
            );
            assert!(s.depots.is_empty());
            assert_eq!(s.economy.money, money);
        }
    }
}

#[test]
fn place_ship_depot_requires_flat_water_on_both_parts() {
    for dir in 0..4_u8 {
        let mut s = GameState::new(8, 8);
        let depot = TileCoord::new(3, 3);
        let footprint = crate::ship_depot_footprint(depot, dir);
        for coord in footprint {
            s.map.set_kind(coord, TileKind::Water).unwrap();
        }
        let mut sloped = s.map.get(footprint[0]).unwrap();
        sloped.height = 2;
        s.map.set_tile(footprint[0], sloped).unwrap();
        let money = s.economy.money;
        let command = Command::PlaceShipDepotDir(depot, dir);

        assert_eq!(
            command_would_fail(&s, &command),
            Some(crate::CommandError::SiteUnsuitable),
            "preview dir={dir} debe rechazar la huella inclinada"
        );
        assert_eq!(
            apply_command(&mut s, &command),
            Err(crate::CommandError::SiteUnsuitable),
            "ejecución dir={dir} debe rechazar la huella inclinada"
        );
        assert!(
            footprint
                .into_iter()
                .all(|coord| s.map.get_kind(coord) == Some(TileKind::Water))
        );
        assert!(s.depots.is_empty());
        assert_eq!(s.economy.money, money);
    }
}

#[test]
fn place_ship_depot_rejects_water_structures_during_auto_clear() {
    for dir in 0..4_u8 {
        let mut s = GameState::new(8, 8);
        let depot = TileCoord::new(3, 3);
        let footprint = crate::ship_depot_footprint(depot, dir);
        for coord in footprint {
            s.map.set_kind(coord, TileKind::Water).unwrap();
        }
        let mut lock = s.map.get(footprint[1]).unwrap();
        lock.m5 = 0x20; // WaterTileType::Lock, eje X; sigue siendo MP_WATER.
        s.map.set_tile(footprint[1], lock).unwrap();
        let money = s.economy.money;
        let command = Command::PlaceShipDepotDir(depot, dir);

        assert_eq!(
            command_would_fail(&s, &command),
            Some(crate::CommandError::BuildingMustBeDemolished),
            "preview dir={dir} no debe limpiar una esclusa automáticamente"
        );
        assert_eq!(
            apply_command(&mut s, &command),
            Err(crate::CommandError::BuildingMustBeDemolished),
            "ejecución dir={dir} no debe sobrescribir una esclusa"
        );
        assert!(
            footprint
                .into_iter()
                .all(|coord| s.map.get_kind(coord) == Some(TileKind::Water))
        );
        assert_eq!(s.map.get(footprint[1]).unwrap().m5, 0x20);
        assert!(s.depots.is_empty());
        assert_eq!(s.economy.money, money);
    }
}

#[test]
fn place_ship_depot_classifies_water_station_blockers() {
    let cases = [
        (
            crate::station::STATION_TYPE_DOCK,
            crate::CommandError::MustDemolishDockFirst,
        ),
        (
            crate::station::STATION_TYPE_BUOY,
            crate::CommandError::BuoyInTheWay,
        ),
        (
            crate::station::STATION_TYPE_OILRIG,
            crate::CommandError::OilRigInTheWay,
        ),
    ];

    for (station_type, expected_error) in cases {
        for part in 0..2_usize {
            let mut s = GameState::new(8, 8);
            let depot = TileCoord::new(3, 3);
            let footprint = crate::ship_depot_footprint(depot, 0);
            for coord in footprint {
                s.map.set_kind(coord, TileKind::Water).unwrap();
            }
            let blocked_tile = footprint[part];
            let mut station = s.map.get(blocked_tile).unwrap();
            station.kind = TileKind::Station;
            station.mapt = 0x50; // MP_STATION.
            station.m6 = station_type << 3;
            station.m1 = set_water_class_m1(station.m1, WaterClass::Sea);
            s.map.set_tile(blocked_tile, station).unwrap();
            let before = s.map.get(blocked_tile).unwrap();
            let money = s.economy.money;
            let command = Command::PlaceShipDepotDir(depot, 0);

            assert_eq!(
                command_would_fail(&s, &command),
                Some(expected_error),
                "preview station_type={station_type} part={part}"
            );
            assert_eq!(
                apply_command(&mut s, &command),
                Err(expected_error),
                "execution station_type={station_type} part={part}"
            );
            assert_eq!(s.map.get(blocked_tile), Some(before));
            assert_eq!(s.map.get_kind(footprint[1 - part]), Some(TileKind::Water));
            assert!(s.depots.is_empty());
            assert_eq!(s.economy.money, money);
        }
    }
}

#[test]
fn place_ship_depot_rejects_water_industry_during_auto_clear() {
    for part in 0..2_usize {
        let mut s = GameState::new(8, 8);
        let depot = TileCoord::new(3, 3);
        let footprint = crate::ship_depot_footprint(depot, 0);
        for coord in footprint {
            s.map.set_kind(coord, TileKind::Water).unwrap();
        }
        let blocked_tile = footprint[part];
        let mut industry = s.map.get(blocked_tile).unwrap();
        industry.kind = TileKind::Industry;
        industry.mapt = 0x40; // MP_INDUSTRY.
        industry.m1 = set_water_class_m1(industry.m1, WaterClass::Sea);
        s.map.set_tile(blocked_tile, industry).unwrap();
        let before = s.map.get(blocked_tile).unwrap();
        let money = s.economy.money;
        let command = Command::PlaceShipDepotDir(depot, 0);

        assert_eq!(
            command_would_fail(&s, &command),
            Some(crate::CommandError::IndustryInTheWay),
            "preview part={part} no debe limpiar la industria acuática"
        );
        assert_eq!(
            apply_command(&mut s, &command),
            Err(crate::CommandError::IndustryInTheWay),
            "ejecución part={part} no debe sobrescribir la industria acuática"
        );
        assert_eq!(s.map.get(blocked_tile), Some(before));
        assert_eq!(s.map.get_kind(footprint[1 - part]), Some(TileKind::Water));
        assert!(s.depots.is_empty());
        assert_eq!(s.economy.money, money);
    }
}

#[test]
fn ship_depot_refreshes_docking_tile_for_water_neighbors() {
    for dir in 0..4_u8 {
        for part in 0..2_usize {
            let mut s = GameState::new(8, 8);
            let depot = TileCoord::new(3, 3);
            let footprint = crate::ship_depot_footprint(depot, dir);
            for coord in footprint {
                s.map.set_kind(coord, TileKind::Water).unwrap();
            }

            let side_dir = if dir & 1 == 0 { 3 } else { 0 };
            let (dx, dy) = crate::map::diag_dir_offset(side_dir);
            let dock_pos = TileCoord::new(footprint[part].x + dx, footprint[part].y + dy);
            let mut dock = s.map.get(dock_pos).unwrap();
            dock.kind = TileKind::Station;
            dock.mapt = 0x50; // MP_STATION.
            dock.m5 = 4; // GFX_DOCK_BASE_WATER_PART.
            dock.m6 = crate::station::STATION_TYPE_DOCK << 3;
            dock.m1 = set_water_class_m1(dock.m1, WaterClass::Sea);
            s.map.set_tile(dock_pos, dock).unwrap();
            s.stations
                .push(crate::Station::new_with_kind(dock_pos, StopKind::Dock));

            apply_command(&mut s, &Command::PlaceShipDepotDir(depot, dir))
                .expect("depósito junto a la parte acuática del muelle");
            assert_ne!(
                s.map.get(footprint[part]).unwrap().m1 & 0x80,
                0,
                "dir={dir} part={part} debe marcar DockingTile"
            );

            apply_command(&mut s, &Command::ClearTile(depot)).expect("limpiar depósito naval");
            assert_ne!(
                s.map.get(footprint[part]).unwrap().m1 & 0x80,
                0,
                "dir={dir} part={part} debe conservar DockingTile al limpiar"
            );
        }
    }
}

#[test]
fn ship_depot_refreshes_docking_tile_for_neutral_oil_rig_industry() {
    let mut s = GameState::new(8, 8);
    let depot = TileCoord::new(3, 3);
    let footprint = crate::ship_depot_footprint(depot, 0);
    for coord in footprint {
        s.map.set_kind(coord, TileKind::Water).unwrap();
    }
    let neighbor = TileCoord::new(depot.x, depot.y - 1);
    let mut industry = s.map.get(neighbor).unwrap();
    industry.kind = TileKind::Industry;
    industry.mapt = 0x40; // MP_INDUSTRY.
    industry.m2 = 7;
    industry.m2_hi = 0;
    industry.m1 = set_water_class_m1(industry.m1, WaterClass::Sea);
    s.map.set_tile(neighbor, industry).unwrap();
    let mut oil_rig = crate::Station::new_with_kind(neighbor, StopKind::OilRig);
    oil_rig.neutral_industry_id = Some(7);
    s.stations.push(oil_rig);

    apply_command(&mut s, &Command::PlaceShipDepotDir(depot, 0))
        .expect("depósito junto a industria con estación neutral");
    assert_ne!(
        s.map.get(depot).unwrap().m1 & 0x80,
        0,
        "la estación neutral del oil rig habilita el amarre"
    );
}

fn add_water_object(
    state: &mut GameState,
    origin: TileCoord,
    size: u8,
    flags: u16,
) -> Vec<TileCoord> {
    let object_type = 5u16;
    state.object_spec_catalog.push(crate::ObjectSpecDef {
        id: object_type,
        class_label: "WATR".into(),
        name: "Water object".into(),
        size,
        from_newgrf: true,
        local_id: 0,
        grfid: 0x5741_5452,
        newgrf_grf_version: 8,
        climate_mask: crate::DEFAULT_OBJECT_CLIMATE_MASK,
        build_cost_factor: 1,
        clear_cost_factor: 1,
        flags,
        animation_frames: 0,
        animation_status: 0xFF,
        animation_speed: 2,
        animation_triggers: 0,
        callback_mask: 0,
        views: Vec::new(),
        newgrf_runtime: None,
        associated_badges: Vec::new(),
    });
    let width = size & 0x0F;
    let height = (size >> 4) & 0x0F;
    let object_tiles = crate::map::object_footprint_tiles(origin, width, height);
    let mut tile = state.map.get(origin).expect("objeto sobre el mapa");
    tile.kind = TileKind::Unknown(crate::map::OTTD_MP_OBJECT);
    tile.mapt = crate::map::MP_OBJECT_MAPT;
    tile.m5 = u8::try_from(object_type).expect("tipo de objeto local");
    tile.m1 = set_water_class_m1(state.active_company.0, WaterClass::Canal);
    tile.m2 = 0;
    tile.m2_hi = 0;
    for &object_tile in &object_tiles {
        state.map.set_tile(object_tile, tile).unwrap();
    }
    let object_id = crate::map::object_id_from_tile(&tile).expect("ObjectID");
    state.objects.push(crate::sav::SavObject {
        object_id,
        tile: origin,
        width: u16::from(width),
        height: u16::from(height),
        town: 0,
        build_date: state.calendar.date,
        colour: state.company_colour,
        view: 0,
        object_type,
    });
    object_tiles
}

#[test]
fn place_ship_depot_auto_clears_autoremove_water_object_and_keeps_water() {
    let mut s = GameState::new(8, 8);
    let depot = TileCoord::new(3, 3);
    let other = crate::ship_depot_footprint(depot, 1)[1];
    let object_tiles = add_water_object(
        &mut s,
        depot,
        0x12,
        crate::object_spec::OBJECT_FLAG_AUTOREMOVE,
    );
    s.map.set_kind(other, TileKind::Water).unwrap();
    let money = s.economy.money;
    let clear_cost = crate::economy::object_clear_cost_factored(
        &s.global_economy,
        1,
        u32::try_from(object_tiles.len()).unwrap(),
    );
    let command = Command::PlaceShipDepotDir(depot, 1);

    assert_eq!(command_would_fail(&s, &command), None);
    apply_command(&mut s, &command).expect("el depósito puede limpiar el objeto autoremove");

    assert_eq!(s.objects.len(), 0, "se quita la instancia completa");
    assert_eq!(s.map.get_kind(object_tiles[1]), Some(TileKind::Water));
    assert_eq!(s.map.get_kind(depot), Some(TileKind::ShipDepot));
    assert_eq!(s.map.get_kind(other), Some(TileKind::ShipDepot));
    assert_eq!(
        s.economy.money,
        money - clear_cost - ship_depot_build_cost(&s.global_economy)
    );
}

#[test]
fn place_ship_depot_rejects_non_autoremove_water_object_atomically() {
    let mut s = GameState::new(8, 8);
    let depot = TileCoord::new(3, 3);
    let other = crate::ship_depot_footprint(depot, 1)[1];
    add_water_object(&mut s, depot, 0x11, 0);
    s.map.set_kind(other, TileKind::Water).unwrap();
    let before = s.map.get(depot).unwrap();
    let money = s.economy.money;
    let command = Command::PlaceShipDepotDir(depot, 1);

    assert_eq!(
        command_would_fail(&s, &command),
        Some(crate::CommandError::ObjectInTheWay)
    );
    assert_eq!(
        apply_command(&mut s, &command),
        Err(crate::CommandError::ObjectInTheWay)
    );
    assert_eq!(s.map.get(depot), Some(before));
    assert_eq!(s.map.get_kind(other), Some(TileKind::Water));
    assert_eq!(s.objects.len(), 1);
    assert_eq!(s.economy.money, money);
}

#[test]
fn place_ship_depot_writes_both_native_parts_for_each_direction() {
    let cases = [
        (
            0,
            TileCoord::new(5, 5),
            TileCoord::new(4, 5),
            TileCoord::new(6, 5),
            0x30,
            0x31,
        ),
        (
            1,
            TileCoord::new(5, 5),
            TileCoord::new(5, 6),
            TileCoord::new(5, 4),
            0x33,
            0x32,
        ),
        (
            2,
            TileCoord::new(5, 5),
            TileCoord::new(6, 5),
            TileCoord::new(4, 5),
            0x31,
            0x30,
        ),
        (
            3,
            TileCoord::new(5, 5),
            TileCoord::new(5, 4),
            TileCoord::new(5, 6),
            0x32,
            0x33,
        ),
    ];

    for (dir, depot, mouth, other, depot_m5, other_m5) in cases {
        let mut s = GameState::new(12, 12);
        for coord in [depot, mouth, other] {
            s.map.set_kind(coord, TileKind::Water).unwrap();
        }

        apply_command(&mut s, &Command::PlaceShipDepotDir(depot, dir)).unwrap();

        assert_eq!(s.map.get(depot).unwrap().kind, TileKind::ShipDepot);
        assert_eq!(s.map.get(depot).unwrap().m5, depot_m5);
        assert_eq!(s.map.get(other).unwrap().kind, TileKind::ShipDepot);
        assert_eq!(s.map.get(other).unwrap().m5, other_m5);
    }
}

#[test]
fn build_ship_uses_north_section_and_initial_depot_facing() {
    use crate::engine::ENGINE_SHIP_MPS;

    let cases = [(0u8, 1u8), (1, 7), (2, 1), (3, 7)];
    for (dir, expected_facing) in cases {
        let mut s = GameState::new(12, 12);
        let depot = TileCoord::new(5, 5);
        let [origin, other] = crate::ship_depot_footprint(depot, dir);
        s.map.set_kind(origin, TileKind::Water).unwrap();
        s.map.set_kind(other, TileKind::Water).unwrap();
        apply_command(&mut s, &Command::PlaceShipDepotDir(depot, dir)).unwrap();
        apply_command(
            &mut s,
            &Command::BuildVehicleAtDepot(origin, ENGINE_SHIP_MPS),
        )
        .unwrap();

        let north = crate::ship_depot_north_tile(&s.map, origin).unwrap();
        let ship = s.vehicles.last().expect("barco recién construido");
        assert_eq!(ship.pos, north, "dir={dir} debe usar la sección norte");
        assert_eq!(ship.direction, expected_facing, "dir={dir} rumbo físico");
        assert_eq!(
            ship.ship_rotation, expected_facing,
            "dir={dir} rumbo gráfico"
        );
        assert!(ship.ship_pos_valid, "dir={dir} debe nacer centrado");
        assert_eq!((ship.ship_x & 0xF, ship.ship_y & 0xF), (8, 8));
    }
}

#[test]
fn refit_ship_requires_native_depot_state() {
    use crate::engine::ENGINE_SHIP_MPS;

    let mut s = GameState::new(12, 12);
    let depot = TileCoord::new(5, 5);
    let [origin, other] = crate::ship_depot_footprint(depot, 0);
    for coord in [origin, other] {
        s.map.set_kind(coord, TileKind::Water).unwrap();
    }
    apply_command(&mut s, &Command::PlaceShipDepotDir(depot, 0)).unwrap();
    apply_command(
        &mut s,
        &Command::BuildVehicleAtDepot(origin, ENGINE_SHIP_MPS),
    )
    .unwrap();
    let id = s.vehicles[0].id;

    s.vehicles[0].ship_state = crate::ship_movement::SHIP_STATE_TRACK_X;
    assert_eq!(
        apply_command(
            &mut s,
            &Command::RefitVehicle {
                vehicle_id: id,
                cargo: CargoType::Oil,
                unit_ids: Vec::new(),
            },
        ),
        Err(crate::CommandError::RefitNotAllowed)
    );

    s.vehicles[0].ship_state = crate::ship_movement::SHIP_STATE_DEPOT;
    s.vehicles[0].running = true;
    assert_eq!(
        apply_command(
            &mut s,
            &Command::RefitVehicle {
                vehicle_id: id,
                cargo: CargoType::Oil,
                unit_ids: Vec::new(),
            },
        ),
        Err(crate::CommandError::RefitNotAllowed)
    );

    s.vehicles[0].running = false;
    apply_command(
        &mut s,
        &Command::RefitVehicle {
            vehicle_id: id,
            cargo: CargoType::Oil,
            unit_ids: Vec::new(),
        },
    )
    .unwrap();
    assert_eq!(s.vehicles[0].cargo_type, Some(CargoType::Oil));
}

#[test]
fn append_goto_nearest_ship_depot_uses_owned_reachable_depot() {
    let mut s = GameState::new(24, 8);
    for y in [2_i32, 3_i32] {
        for x in 0..24_i32 {
            crate::map::make_water_tile(&mut s.map, TileCoord::new(x, y), WaterClass::Sea).unwrap();
        }
    }
    let rival = TileCoord::new(5, 2);
    let own = TileCoord::new(17, 2);
    apply_command(&mut s, &Command::PlaceShipDepotDir(rival, 3)).unwrap();
    apply_command(&mut s, &Command::PlaceShipDepotDir(own, 3)).unwrap();
    for tile in crate::ship_depot_footprint(rival, 3) {
        let mut raw = s.map.get(tile).unwrap();
        raw.m1 = (raw.m1 & !0x1F) | 1;
        s.map.set_tile(tile, raw).unwrap();
    }

    let from = TileCoord::new(7, 2);
    s.vehicles
        .push(Vehicle::new(1, VehicleKind::Ship, from, from));
    apply_command(&mut s, &Command::AppendGotoNearestDepot(1)).unwrap();

    assert_eq!(s.vehicles[0].orders.len(), 1);
    assert_eq!(
        s.vehicles[0].orders[0].destination(),
        crate::ship_depot_north_tile(&s.map, own).unwrap()
    );
}

#[test]
fn depot_order_refit_requires_native_ship_depot_state() {
    use crate::engine::ENGINE_SHIP_MPS;

    let mut s = GameState::new(12, 12);
    let depot = TileCoord::new(5, 5);
    let [origin, other] = crate::ship_depot_footprint(depot, 0);
    for coord in [origin, other] {
        s.map.set_kind(coord, TileKind::Water).unwrap();
    }
    apply_command(&mut s, &Command::PlaceShipDepotDir(depot, 0)).unwrap();
    apply_command(
        &mut s,
        &Command::BuildVehicleAtDepot(origin, ENGINE_SHIP_MPS),
    )
    .unwrap();
    let ship = &mut s.vehicles[0];
    ship.cargo = 0;
    ship.running = false;
    ship.ship_state = crate::ship_movement::SHIP_STATE_TRACK_X;
    ship.pending_depot_order_refit = Some(CargoType::Oil);
    s.step();
    assert_ne!(s.vehicles[0].cargo_type, Some(CargoType::Oil));
    assert!(s.vehicles[0].pending_depot_order_refit.is_none());

    s.vehicles[0].ship_state = crate::ship_movement::SHIP_STATE_DEPOT;
    s.vehicles[0].pending_depot_order_refit = Some(CargoType::Oil);
    s.step();
    assert_eq!(s.vehicles[0].cargo_type, Some(CargoType::Oil));
}

#[test]
fn place_ship_depot_rejects_every_map_edge_atomically() {
    // Cada origen está dentro de un mapa 4×4, pero la segunda sección cae
    // fuera en uno de los cuatro bordes. El comando y su preview deben
    // rechazar la huella completa antes de escribir la primera sección.
    let cases = [
        (0u8, TileCoord::new(3, 1), TileCoord::new(2, 1)), // este
        (1, TileCoord::new(1, 0), TileCoord::new(1, 1)),   // norte
        (2, TileCoord::new(0, 1), TileCoord::new(1, 1)),   // oeste
        (3, TileCoord::new(1, 3), TileCoord::new(1, 2)),   // sur
    ];

    for (dir, depot, mouth) in cases {
        let mut s = GameState::new(4, 4);
        for coord in [depot, mouth] {
            s.map.set_kind(coord, TileKind::Water).unwrap();
        }
        let [origin, other] = crate::ship_depot_footprint(depot, dir);
        assert!(s.map.get(origin).is_some());
        assert!(s.map.get(other).is_none());
        let money = s.economy.money;
        let command = Command::PlaceShipDepotDir(depot, dir);

        assert_eq!(
            command_would_fail(&s, &command),
            Some(crate::CommandError::OutOfBounds),
            "preview dir={dir} debe rechazar la segunda sección fuera del mapa"
        );
        assert_eq!(
            apply_command(&mut s, &command),
            Err(crate::CommandError::OutOfBounds),
            "ejecución dir={dir} debe ser atómica en el borde"
        );
        assert_eq!(s.map.get_kind(origin), Some(TileKind::Water));
        assert!(s.map.get(other).is_none());
        assert!(s.depots.is_empty());
        assert_eq!(s.economy.money, money);
    }
}

#[test]
fn depot_builders_share_native_pool_ids() {
    let mut s = GameState::new(16, 16);
    let road = TileCoord::new(2, 2);
    let rail = TileCoord::new(6, 2);
    let ship = TileCoord::new(10, 8);
    let ship_other = TileCoord::new(11, 8);

    s.map
        .set_kind(TileCoord::new(2, 1), TileKind::Road)
        .unwrap();
    s.map
        .set_kind(TileCoord::new(6, 1), TileKind::Rail)
        .unwrap();
    for coord in [ship, TileCoord::new(9, 8), ship_other] {
        s.map.set_kind(coord, TileKind::Water).unwrap();
    }

    apply_command(&mut s, &Command::PlaceRoadDepotDir(road, 3)).unwrap();
    apply_command(&mut s, &Command::PlaceRailDepotDir(rail, 3)).unwrap();
    apply_command(&mut s, &Command::PlaceShipDepotDir(ship, 0)).unwrap();

    assert_eq!(
        crate::depot::depot_id_from_tile(s.map.get(road).unwrap()),
        Some(0)
    );
    assert_eq!(
        crate::depot::depot_id_from_tile(s.map.get(rail).unwrap()),
        Some(1)
    );
    assert_eq!(
        crate::depot::depot_id_from_tile(s.map.get(ship).unwrap()),
        Some(2)
    );
    assert_eq!(
        crate::depot::depot_id_from_tile(s.map.get(ship_other).unwrap()),
        Some(2)
    );
    assert_eq!(
        s.depots
            .iter()
            .map(|depot| depot.depot_id)
            .collect::<Vec<_>>(),
        vec![0, 1, 2]
    );
    assert_eq!(s.depots[0].tile, road);
    assert_eq!(s.depots[1].tile, rail);
    assert_eq!(s.depots[2].tile, ship);
}

#[test]
fn depot_registration_matches_nearest_town_and_transport_type() {
    let mut s = GameState::new(16, 16);
    s.towns.push(crate::Town {
        id: 7,
        pos: TileCoord::new(4, 4),
        name: "Villa Central".into(),
        ..Default::default()
    });

    let first = TileCoord::new(4, 4);
    let second = TileCoord::new(7, 4);
    for exit in [TileCoord::new(3, 4), TileCoord::new(6, 4)] {
        s.map.set_kind(exit, TileKind::Road).unwrap();
    }
    apply_command(&mut s, &Command::PlaceRoadDepotDir(first, 0)).unwrap();
    apply_command(&mut s, &Command::PlaceRoadDepotDir(second, 0)).unwrap();

    assert_eq!(s.depots[0].town_id, Some(7));
    assert_eq!(s.depots[0].town_cn, 0);
    assert_eq!(s.depots[1].town_id, Some(7));
    assert_eq!(s.depots[1].town_cn, 1);

    let rail = TileCoord::new(4, 8);
    s.map
        .set_kind(TileCoord::new(4, 7), TileKind::Rail)
        .unwrap();
    apply_command(&mut s, &Command::PlaceRailDepotDir(rail, 3)).unwrap();
    assert_eq!(s.depots[2].town_id, Some(7));
    assert_eq!(
        s.depots[2].town_cn, 0,
        "el ordinal se reinicia para otro tipo de Depot"
    );
}

#[test]
fn rename_depot_validates_unique_length_and_reset() {
    let mut s = GameState::new(16, 16);
    s.towns.push(crate::Town {
        id: 7,
        pos: TileCoord::new(4, 4),
        name: "Villa Central".into(),
        ..Default::default()
    });
    let first = TileCoord::new(4, 4);
    let second = TileCoord::new(8, 4);
    for exit in [TileCoord::new(3, 4), TileCoord::new(7, 4)] {
        s.map.set_kind(exit, TileKind::Road).unwrap();
    }
    apply_command(&mut s, &Command::PlaceRoadDepotDir(first, 0)).unwrap();
    apply_command(&mut s, &Command::PlaceRoadDepotDir(second, 0)).unwrap();

    apply_command(
        &mut s,
        &Command::RenameDepot {
            depot_pos: first,
            name: Some("Terminal Norte".into()),
        },
    )
    .unwrap();
    assert_eq!(s.depots[0].name, "Terminal Norte");

    assert_eq!(
        apply_command(
            &mut s,
            &Command::RenameDepot {
                depot_pos: second,
                name: Some("Terminal Norte".into()),
            },
        ),
        Err(crate::CommandError::DepotNameTaken)
    );
    assert_eq!(
        apply_command(
            &mut s,
            &Command::RenameDepot {
                depot_pos: second,
                name: Some("12345678901234567890123456789012".into()),
            },
        ),
        Err(crate::CommandError::DepotNameTooLong)
    );

    apply_command(
        &mut s,
        &Command::RenameDepot {
            depot_pos: first,
            name: None,
        },
    )
    .unwrap();
    assert!(s.depots[0].name.is_empty());
    assert_eq!(s.depots[0].town_id, Some(7));
    assert_eq!(s.depots[0].town_cn, 0);
}

#[test]
fn rename_ship_depot_from_either_section_updates_one_pool_row() {
    let mut s = GameState::new(12, 12);
    let depot = TileCoord::new(4, 4);
    let other = TileCoord::new(5, 4);
    for coord in [depot, TileCoord::new(3, 4), other] {
        s.map.set_kind(coord, TileKind::Water).unwrap();
    }
    apply_command(&mut s, &Command::PlaceShipDepotDir(depot, 0)).unwrap();

    apply_command(
        &mut s,
        &Command::RenameDepot {
            depot_pos: other,
            name: Some("Puerto Azul".into()),
        },
    )
    .unwrap();

    assert_eq!(s.depots.len(), 1);
    assert_eq!(s.depots[0].tile, depot);
    assert_eq!(s.depots[0].name, "Puerto Azul");
}

#[test]
fn place_ship_depot_rejects_second_part_without_mutating_first() {
    let mut s = GameState::new(12, 12);
    let depot = TileCoord::new(4, 4);
    let mouth = TileCoord::new(3, 4);
    let other = TileCoord::new(5, 4);
    for coord in [depot, mouth] {
        s.map.set_kind(coord, TileKind::Water).unwrap();
    }
    let money = s.economy.money;

    let error = apply_command(&mut s, &Command::PlaceShipDepotDir(depot, 0)).unwrap_err();

    assert_eq!(error, crate::CommandError::CannotPlaceStationOnOccupiedTile);
    assert_eq!(s.map.get_kind(depot), Some(TileKind::Water));
    assert_eq!(s.map.get_kind(other), Some(TileKind::Grass));
    assert_eq!(s.economy.money, money);
}

#[test]
fn clear_ship_depot_from_either_section_restores_both_water_tiles() {
    let mut s = GameState::new(12, 12);
    let depot = TileCoord::new(5, 5);
    let mouth = TileCoord::new(6, 5);
    let other = TileCoord::new(4, 5);
    for (coord, water_class) in [
        (depot, WaterClass::Canal),
        (mouth, WaterClass::Sea),
        (other, WaterClass::River),
    ] {
        s.map.set_kind(coord, TileKind::Water).unwrap();
        let mut tile = s.map.get(coord).unwrap();
        tile.m1 = set_water_class_m1(tile.m1, water_class);
        s.map.set_tile(coord, tile).unwrap();
    }
    apply_command(&mut s, &Command::PlaceShipDepotDir(depot, 2)).unwrap();
    let money = s.economy.money;

    // `other` es la sección norte; se demuele desde el extremo opuesto al
    // que recibió el comando de construcción para cubrir ambos accesos.
    apply_command(&mut s, &Command::ClearTile(other)).unwrap();

    assert_eq!(s.map.get_kind(depot), Some(TileKind::Water));
    assert_eq!(s.map.get_kind(other), Some(TileKind::Water));
    assert_eq!(
        crate::map::water_class(s.map.get(depot).unwrap()),
        Some(WaterClass::Canal)
    );
    assert_eq!(
        crate::map::water_class(s.map.get(other).unwrap()),
        Some(WaterClass::River)
    );
    assert_eq!(s.map.get(depot).unwrap().m5, 0);
    assert_eq!(s.map.get(other).unwrap().m5, 0);
    assert!(s.depots.is_empty());
    assert_eq!(
        s.economy.money,
        money - ship_depot_clear_cost(&s.global_economy)
    );
}

#[test]
fn clear_ship_depot_rejects_vehicle_on_the_other_section() {
    let mut s = GameState::new(12, 12);
    let depot = TileCoord::new(4, 4);
    let other = TileCoord::new(5, 4);
    for coord in [depot, TileCoord::new(3, 4), other] {
        s.map.set_kind(coord, TileKind::Water).unwrap();
    }
    apply_command(&mut s, &Command::PlaceShipDepotDir(depot, 0)).unwrap();
    s.vehicles
        .push(Vehicle::new(1, VehicleKind::Ship, other, other));
    let money = s.economy.money;

    assert_eq!(
        command_would_fail(&s, &Command::ClearTile(depot)),
        Some(crate::CommandError::VehicleInTheWay)
    );
    assert_eq!(
        apply_command(&mut s, &Command::ClearTile(depot)),
        Err(crate::CommandError::VehicleInTheWay)
    );
    assert_eq!(s.map.get_kind(depot), Some(TileKind::ShipDepot));
    assert_eq!(s.map.get_kind(other), Some(TileKind::ShipDepot));
    assert_eq!(s.economy.money, money);
}

#[test]
fn place_dock_on_coast_and_serves_ship() {
    let mut s = GameState::new(12, 12);
    let land = TileCoord::new(5, 4);
    let water = TileCoord::new(5, 5);
    let approach = TileCoord::new(5, 6);
    s.map.set_kind(land, TileKind::Grass).unwrap();
    s.map.set_kind(water, TileKind::Water).unwrap();
    s.map.set_kind(approach, TileKind::Water).unwrap();
    set_dock_land_slope(&mut s.map, land, 1, 1);
    let money = s.economy.money;
    apply_command(&mut s, &Command::PlaceDock(land, 1)).unwrap();
    assert_eq!(s.map.get_kind(land), Some(TileKind::Station));
    assert_eq!(
        crate::station::station_type_from_m6(s.map.get(land).unwrap().m6),
        crate::station::STATION_TYPE_DOCK
    );
    assert_eq!(s.map.get_kind(water), Some(TileKind::Station));
    assert_eq!(s.map.get(water).unwrap().m5, 5);
    assert!(!crate::ship_movement::is_water_network_tile_at(
        &s.map, land
    ));
    assert!(crate::ship_movement::is_water_network_tile_at(
        &s.map, water
    ));
    assert_eq!(s.stations.len(), 1);
    assert_eq!(s.stations[0].stop_kind, StopKind::Dock);
    assert_eq!(s.stations[0].pos, land);
    assert_eq!(s.stations[0].ottd_station_id, Some(0));
    assert!(s.stations[0].can_service_vehicle(VehicleKind::Ship));
    assert_eq!(
        s.economy.money,
        money - station_build_cost(&s.global_economy)
    );
}

#[test]
fn dock_uses_shared_native_id_and_clears_from_water_part() {
    let mut s = GameState::new(12, 12);
    let land = TileCoord::new(6, 5);
    let water = TileCoord::new(5, 5);
    let approach = TileCoord::new(4, 5);
    s.map.set_kind(land, TileKind::Grass).unwrap();
    s.map.set_kind(water, TileKind::Water).unwrap();
    s.map.set_kind(approach, TileKind::Water).unwrap();
    set_dock_land_slope(&mut s.map, land, 0, 1);
    let money = s.economy.money;

    apply_command(&mut s, &Command::PlaceDock(land, 0)).unwrap();

    let land_tile = s.map.get(land).unwrap();
    let water_tile = s.map.get(water).unwrap();
    assert_eq!(land_tile.kind, TileKind::Station);
    assert_eq!(water_tile.kind, TileKind::Station);
    assert_eq!(land_tile.m5, 0);
    assert_eq!(water_tile.m5, crate::station::DOCK_WATER_PART_GFX);
    assert_eq!(
        crate::depot::depot_id_from_tile(land_tile),
        None,
        "los docks no deben entrar al pool de depósitos"
    );
    assert_eq!(
        u16::from(land_tile.m2) | (u16::from(land_tile.m2_hi) << 8),
        u16::from(water_tile.m2) | (u16::from(water_tile.m2_hi) << 8)
    );
    assert!(crate::ship_movement::is_water_network_tile_at(
        &s.map, water
    ));
    assert!(!crate::ship_movement::is_water_network_tile_at(
        &s.map, land
    ));
    assert_eq!(s.stations[0].pos, land);
    assert_eq!(
        command_would_fail(&s, &Command::ClearTile(water)),
        None,
        "la pieza acuática debe ser un cursor válido de demolición"
    );

    apply_command(&mut s, &Command::ClearTile(water)).unwrap();

    assert_eq!(s.map.get_kind(land), Some(TileKind::Grass));
    assert_eq!(s.map.get_kind(water), Some(TileKind::Water));
    assert_eq!(s.map.get_kind(approach), Some(TileKind::Water));
    let cleared_land = s.map.get(land).unwrap();
    assert_eq!(cleared_land.m2, 0);
    assert_eq!(cleared_land.m2_hi, 0);
    assert_eq!(cleared_land.m6, 0);
    assert!(s.stations.is_empty());
    assert_eq!(
        s.economy.money,
        money - station_build_cost(&s.global_economy) - crate::CLEAR_TILE_COST
    );
}

#[test]
fn place_buoy_on_water_is_ship_waypoint() {
    let mut s = GameState::new(12, 12);
    let buoy = TileCoord::new(4, 4);
    s.map.set_kind(buoy, TileKind::Water).unwrap();
    let money = s.economy.money;
    apply_command(&mut s, &Command::PlaceBuoy(buoy)).unwrap();
    assert_eq!(s.map.get_kind(buoy), Some(TileKind::Station));
    assert_eq!(s.stations.len(), 1);
    assert_eq!(s.stations[0].stop_kind, StopKind::Buoy);
    assert!(s.stations[0].is_waypoint());
    assert!(s.stations[0].can_service_vehicle(VehicleKind::Ship));
    assert!(!s.stations[0].accepts_cargo(crate::CargoType::Goods));
    assert_eq!(
        s.economy.money,
        money - station_build_cost(&s.global_economy) / 2
    );
}

#[test]
fn clearing_buoy_restores_underlying_canal_water() {
    use crate::map::is_canal_tile;

    let mut s = GameState::new(8, 8);
    let buoy = TileCoord::new(4, 4);
    apply_command(&mut s, &Command::PlaceCanal(buoy)).unwrap();
    apply_command(&mut s, &Command::PlaceBuoy(buoy)).unwrap();
    assert_eq!(s.map.get_kind(buoy), Some(TileKind::Station));

    apply_command(&mut s, &Command::ClearTile(buoy)).unwrap();

    assert_eq!(s.map.get_kind(buoy), Some(TileKind::Water));
    assert!(s.map.get(buoy).is_some_and(is_canal_tile));
    assert_eq!(s.map.get(buoy).map(|tile| tile.m6), Some(0));
    assert!(s.stations.is_empty());
}

#[test]
fn place_buoy_under_bridge_keeps_waterway_available() {
    use crate::BridgeType;

    let mut s = GameState::new(8, 8);
    let c = |x: i32| TileCoord::new(x, 4);
    for x in 2..=5 {
        s.map.set_kind(c(x), TileKind::Water).unwrap();
    }
    apply_command(
        &mut s,
        &Command::PlaceRoadBridge(c(1), c(6), BridgeType::Wooden),
    )
    .unwrap();

    let buoy = c(3);
    assert_eq!(s.map.get_kind(buoy), Some(TileKind::Water));
    apply_command(&mut s, &Command::PlaceBuoy(buoy)).unwrap();
    assert_eq!(s.map.get_kind(buoy), Some(TileKind::Station));
    assert_eq!(s.stations[0].stop_kind, StopKind::Buoy);
}

#[test]
fn place_buoy_rejects_land() {
    let mut s = GameState::new(8, 8);
    let e = apply_command(&mut s, &Command::PlaceBuoy(TileCoord::new(2, 2))).unwrap_err();
    assert!(matches!(
        e,
        crate::CommandError::CannotPlaceStationOnOccupiedTile
    ));
}

fn set_ne_slope(map: &mut crate::Map, tx: i32, ty: i32, base: u8) {
    map.set_height(TileCoord::new(tx, ty), base + 1).unwrap();
    map.set_height(TileCoord::new(tx, ty + 1), base + 1)
        .unwrap();
    map.set_height(TileCoord::new(tx + 1, ty), base).unwrap();
    map.set_height(TileCoord::new(tx + 1, ty + 1), base)
        .unwrap();
}

fn set_sw_slope(map: &mut crate::Map, tx: i32, ty: i32, base: u8) {
    map.set_height(TileCoord::new(tx, ty), base).unwrap();
    map.set_height(TileCoord::new(tx, ty + 1), base).unwrap();
    map.set_height(TileCoord::new(tx + 1, ty), base + 1)
        .unwrap();
    map.set_height(TileCoord::new(tx + 1, ty + 1), base + 1)
        .unwrap();
}

fn set_dock_land_slope(map: &mut crate::Map, land: TileCoord, dir: u8, base: u8) {
    for corner in [
        TileCoord::new(land.x, land.y),
        TileCoord::new(land.x + 1, land.y),
        TileCoord::new(land.x, land.y + 1),
        TileCoord::new(land.x + 1, land.y + 1),
    ] {
        map.set_height(corner, base).unwrap();
    }
    let high = match crate::map::opposite_diag_dir(dir) {
        0 => [
            TileCoord::new(land.x, land.y),
            TileCoord::new(land.x, land.y + 1),
        ],
        1 => [
            TileCoord::new(land.x, land.y + 1),
            TileCoord::new(land.x + 1, land.y + 1),
        ],
        2 => [
            TileCoord::new(land.x + 1, land.y),
            TileCoord::new(land.x + 1, land.y + 1),
        ],
        3 => [
            TileCoord::new(land.x, land.y),
            TileCoord::new(land.x + 1, land.y),
        ],
        _ => unreachable!("opposite_diag_dir siempre devuelve 0..=3"),
    };
    for corner in high {
        map.set_height(corner, base + 1).unwrap();
    }
}

#[test]
fn dock_requires_an_inclined_coast_matching_direction() {
    let mut s = GameState::new(12, 12);
    let land = TileCoord::new(5, 4);
    let water = TileCoord::new(5, 5);
    let approach = TileCoord::new(5, 6);
    s.map.set_kind(land, TileKind::Grass).unwrap();
    s.map.set_kind(water, TileKind::Water).unwrap();
    s.map.set_kind(approach, TileKind::Water).unwrap();

    assert_eq!(
        command_would_fail(&s, &Command::PlaceDock(land, 1)),
        Some(crate::CommandError::SiteUnsuitable)
    );
    set_dock_land_slope(&mut s.map, land, 1, 1);
    assert_eq!(command_would_fail(&s, &Command::PlaceDock(land, 1)), None);
    assert_eq!(
        command_would_fail(&s, &Command::PlaceDock(land, 0)),
        Some(crate::CommandError::SiteUnsuitable)
    );
}

#[test]
fn dock_accepts_each_native_slope_direction() {
    for dir in 0..4 {
        let mut s = GameState::new(12, 12);
        let land = TileCoord::new(5, 5);
        let water = crate::station::dock_water_tile(land, dir);
        let approach = crate::station::dock_water_tile(water, dir);
        s.map.set_kind(land, TileKind::Grass).unwrap();
        s.map.set_kind(water, TileKind::Water).unwrap();
        s.map.set_kind(approach, TileKind::Water).unwrap();
        set_dock_land_slope(&mut s.map, land, dir, 1);

        assert_eq!(
            command_would_fail(&s, &Command::PlaceDock(land, dir)),
            None,
            "orientación nativa {dir}"
        );
    }
}

#[test]
fn place_dock_auto_clears_autoremove_water_object_and_keeps_water() {
    let mut s = GameState::new(12, 12);
    let land = TileCoord::new(5, 4);
    let water = crate::station::dock_water_tile(land, 1);
    let approach = crate::station::dock_water_tile(water, 1);
    s.map.set_kind(land, TileKind::Grass).unwrap();
    let object_tiles = add_water_object(
        &mut s,
        water,
        0x11,
        crate::object_spec::OBJECT_FLAG_AUTOREMOVE,
    );
    s.map.set_kind(approach, TileKind::Water).unwrap();
    set_dock_land_slope(&mut s.map, land, 1, 1);
    let money = s.economy.money;
    let clear_cost = crate::economy::object_clear_cost_factored(
        &s.global_economy,
        1,
        u32::try_from(object_tiles.len()).unwrap(),
    );
    let command = Command::PlaceDock(land, 1);

    assert_eq!(command_would_fail(&s, &command), None);
    apply_command(&mut s, &command).expect("el muelle puede limpiar el objeto autoremove");

    assert!(s.objects.is_empty(), "se quita la instancia completa");
    assert_eq!(s.map.get_kind(water), Some(TileKind::Station));
    assert_eq!(s.map.get_kind(approach), Some(TileKind::Water));
    assert_eq!(
        s.economy.money,
        money - clear_cost - station_build_cost(&s.global_economy)
    );
}

#[test]
fn place_dock_rejects_non_autoremove_water_object_atomically() {
    let mut s = GameState::new(12, 12);
    let land = TileCoord::new(5, 4);
    let water = crate::station::dock_water_tile(land, 1);
    let approach = crate::station::dock_water_tile(water, 1);
    s.map.set_kind(land, TileKind::Grass).unwrap();
    add_water_object(&mut s, water, 0x11, 0);
    s.map.set_kind(approach, TileKind::Water).unwrap();
    set_dock_land_slope(&mut s.map, land, 1, 1);
    let before = s.map.get(water).unwrap();
    let money = s.economy.money;
    let command = Command::PlaceDock(land, 1);

    assert_eq!(
        command_would_fail(&s, &command),
        Some(crate::CommandError::ObjectInTheWay)
    );
    assert_eq!(
        apply_command(&mut s, &command),
        Err(crate::CommandError::ObjectInTheWay)
    );
    assert_eq!(s.map.get(water), Some(before));
    assert_eq!(s.map.get_kind(approach), Some(TileKind::Water));
    assert_eq!(s.objects.len(), 1);
    assert_eq!(s.economy.money, money);
}

#[test]
fn place_adjacent_dock_reuses_station_identity_and_footprint() {
    let mut s = GameState::new(16, 12);
    let first = TileCoord::new(5, 4);
    let second = TileCoord::new(6, 4);
    for land in [first, second] {
        let water = crate::station::dock_water_tile(land, 1);
        let approach = crate::station::dock_water_tile(water, 1);
        s.map.set_kind(land, TileKind::Grass).unwrap();
        s.map.set_kind(water, TileKind::Water).unwrap();
        s.map.set_kind(approach, TileKind::Water).unwrap();
        set_dock_land_slope(&mut s.map, land, 1, 1);
    }

    apply_command(&mut s, &Command::PlaceDock(first, 1)).unwrap();
    apply_command(&mut s, &Command::PlaceDock(second, 1)).unwrap();

    assert_eq!(s.stations.len(), 1);
    let station = &s.stations[0];
    assert_eq!(station.pos, first);
    assert!(
        station
            .joined_tiles
            .contains(&crate::station::dock_water_tile(first, 1))
    );
    assert!(station.joined_tiles.contains(&second));
    assert!(
        station
            .joined_tiles
            .contains(&crate::station::dock_water_tile(second, 1))
    );
    let first_id = crate::depot::depot_id_from_tile(s.map.get(first).unwrap());
    assert_eq!(first_id, None, "un muelle no consume DepotID");
    let station_id = |tile: TileCoord| {
        let raw = s.map.get(tile).unwrap();
        u16::from(raw.m2) | (u16::from(raw.m2_hi) << 8)
    };
    assert_eq!(
        station_id(first),
        station_id(crate::station::dock_water_tile(first, 1))
    );
    assert_eq!(station_id(first), station_id(second));
    assert_eq!(
        station_id(first),
        station_id(crate::station::dock_water_tile(second, 1))
    );
}

#[test]
fn joining_docks_unifies_native_id_and_promotes_anchor_after_clear() {
    let mut s = GameState::new(18, 12);
    let first = TileCoord::new(4, 4);
    let second = TileCoord::new(8, 4);
    for land in [first, second] {
        let water = crate::station::dock_water_tile(land, 1);
        let approach = crate::station::dock_water_tile(water, 1);
        s.map.set_kind(land, TileKind::Grass).unwrap();
        s.map.set_kind(water, TileKind::Water).unwrap();
        s.map.set_kind(approach, TileKind::Water).unwrap();
        set_dock_land_slope(&mut s.map, land, 1, 1);
    }

    apply_command(&mut s, &Command::PlaceDock(first, 1)).unwrap();
    apply_command(&mut s, &Command::PlaceDock(second, 1)).unwrap();
    assert_eq!(s.stations.len(), 2);
    let second_id_before = {
        let raw = s.map.get(second).unwrap();
        u16::from(raw.m2) | (u16::from(raw.m2_hi) << 8)
    };

    apply_command(
        &mut s,
        &Command::JoinStations {
            keep: first,
            merge: second,
        },
    )
    .unwrap();

    assert_eq!(s.stations.len(), 1);
    assert_eq!(s.stations[0].pos, first);
    assert!(s.stations[0].covers_tile(crate::station::dock_water_tile(second, 1)));
    let first_id = {
        let raw = s.map.get(first).unwrap();
        u16::from(raw.m2) | (u16::from(raw.m2_hi) << 8)
    };
    assert_ne!(first_id, second_id_before);
    assert_eq!(first_id, {
        let raw = s.map.get(second).unwrap();
        u16::from(raw.m2) | (u16::from(raw.m2_hi) << 8)
    });

    apply_command(&mut s, &Command::ClearTile(first)).unwrap();

    assert_eq!(s.stations.len(), 1);
    assert_eq!(s.stations[0].pos, second);
    assert_eq!(s.map.get_kind(first), Some(TileKind::Grass));
    assert_eq!(
        s.map.get_kind(crate::station::dock_water_tile(first, 1)),
        Some(TileKind::Water)
    );
    assert!(s.stations[0].covers_tile(second));
    assert!(!s.stations[0].covers_tile(first));
}

#[test]
fn place_dock_reuses_explicit_station_to_join_at_distance() {
    let mut s = GameState::new(20, 12);
    s.construction.distant_join_stations = true;
    let first = TileCoord::new(4, 4);
    let second = TileCoord::new(12, 4);
    for land in [first, second] {
        let water = crate::station::dock_water_tile(land, 1);
        let approach = crate::station::dock_water_tile(water, 1);
        s.map.set_kind(land, TileKind::Grass).unwrap();
        s.map.set_kind(water, TileKind::Water).unwrap();
        s.map.set_kind(approach, TileKind::Water).unwrap();
        set_dock_land_slope(&mut s.map, land, 1, 1);
    }

    apply_command(&mut s, &Command::PlaceDock(first, 1)).unwrap();
    let station_id = s.stations[0].ottd_station_id.unwrap() as u16;

    apply_command(
        &mut s,
        &Command::PlaceDockAtStation {
            origin: second,
            dir: 1,
            station_to_join: station_id,
        },
    )
    .unwrap();

    assert_eq!(s.stations.len(), 1);
    assert_eq!(s.stations[0].pos, first);
    assert!(s.stations[0].covers_tile(second));
    assert!(s.stations[0].covers_tile(crate::station::dock_water_tile(second, 1)));
    for tile in [
        first,
        crate::station::dock_water_tile(first, 1),
        second,
        crate::station::dock_water_tile(second, 1),
    ] {
        let raw = s.map.get(tile).unwrap();
        assert_eq!(u16::from(raw.m2) | (u16::from(raw.m2_hi) << 8), station_id);
    }
}

#[test]
fn place_dock_rejects_unknown_explicit_station_before_mutation() {
    let mut s = GameState::new(12, 12);
    let land = TileCoord::new(5, 4);
    let water = crate::station::dock_water_tile(land, 1);
    let approach = crate::station::dock_water_tile(water, 1);
    s.map.set_kind(land, TileKind::Grass).unwrap();
    s.map.set_kind(water, TileKind::Water).unwrap();
    s.map.set_kind(approach, TileKind::Water).unwrap();
    set_dock_land_slope(&mut s.map, land, 1, 1);
    let before_land = s.map.get(land).unwrap();
    let before_water = s.map.get(water).unwrap();
    let money = s.economy.money;

    let command = Command::PlaceDockAtStation {
        origin: land,
        dir: 1,
        station_to_join: 77,
    };
    assert_eq!(
        command_would_fail(&s, &command),
        Some(crate::CommandError::CannotJoinStations)
    );
    assert_eq!(
        apply_command(&mut s, &command),
        Err(crate::CommandError::CannotJoinStations)
    );
    assert_eq!(s.map.get(land), Some(before_land));
    assert_eq!(s.map.get(water), Some(before_water));
    assert_eq!(s.economy.money, money);
}

#[test]
fn place_aqueduct_between_facing_slopes() {
    let mut s = SandboxMap::flat_rich(16, 12, 1);
    // Oeste → este: rampa SW en (3,5), rampa NE en (7,5).
    let west = TileCoord::new(3, 5);
    let east = TileCoord::new(7, 5);
    set_sw_slope(&mut s.map, west.x, west.y, 1);
    set_ne_slope(&mut s.map, east.x, east.y, 1);
    apply_command(&mut s, &Command::PlaceAqueduct(west, east)).unwrap();
    assert_eq!(s.map.get_kind(west), Some(TileKind::Water));
    assert_eq!(s.map.get_kind(east), Some(TileKind::Water));
    let mid = s.map.get(TileCoord::new(5, 5)).unwrap();
    assert_eq!(mid.kind, TileKind::Water);
    assert!(bridge_above_axis_from_mapt(mid.mapt).is_some());
    assert!(crate::ship_movement::is_water_network_tile_at(
        &s.map,
        TileCoord::new(5, 5)
    ));
}

#[test]
fn place_aqueduct_rejects_flat_endpoints() {
    let mut s = SandboxMap::flat_rich(12, 12, 1);
    let e = apply_command(
        &mut s,
        &Command::PlaceAqueduct(TileCoord::new(2, 4), TileCoord::new(6, 4)),
    )
    .unwrap_err();
    assert!(matches!(e, crate::CommandError::InvalidBridgeSpan));
}

#[test]
fn ship_buys_at_depot_and_paths_to_dock() {
    use crate::engine::ENGINE_SHIP_MPS;
    use crate::pathfinder::{PathNetwork, find_path};
    use crate::vehicle::VehicleOrder;

    let mut s = GameState::new(16, 10);
    for x in 1..=10 {
        s.map
            .set_kind(TileCoord::new(x, 4), TileKind::Water)
            .unwrap();
    }
    let dock_land = TileCoord::new(10, 3);
    s.map.set_kind(dock_land, TileKind::Grass).unwrap();
    s.map
        .set_kind(TileCoord::new(10, 5), TileKind::Water)
        .unwrap();
    set_dock_land_slope(&mut s.map, dock_land, 1, 1);
    apply_command(&mut s, &Command::PlaceShipDepotDir(TileCoord::new(2, 4), 2)).unwrap(); // boca +x hacia agua
    apply_command(&mut s, &Command::PlaceDock(dock_land, 1)).unwrap();
    apply_command(
        &mut s,
        &Command::BuildVehicleAtDepot(TileCoord::new(2, 4), ENGINE_SHIP_MPS),
    )
    .unwrap();
    let ship = s
        .vehicles
        .iter_mut()
        .find(|v| v.kind == VehicleKind::Ship)
        .unwrap();
    ship.running = true;
    ship.set_vehicle_orders(vec![VehicleOrder::station(dock_land)]);
    ship.sync_order_destination(&s.map);
    let path = find_path(&s.map, ship.pos, ship.dest, PathNetwork::Water);
    assert!(path.is_some(), "ruta agua depósito → muelle");
}

#[test]
fn ship_depot_order_from_south_section_is_stored_at_north_anchor() {
    let mut s = GameState::new(16, 10);
    let depot = TileCoord::new(4, 4);
    let [origin, other] = crate::ship_depot_footprint(depot, 0);
    for tile in [origin, other, TileCoord::new(3, 4)] {
        s.map.set_kind(tile, TileKind::Water).unwrap();
    }
    apply_command(&mut s, &Command::PlaceShipDepotDir(depot, 0)).unwrap();
    let north = crate::ship_depot_north_tile(&s.map, depot).unwrap();
    let south = crate::ship_depot_other_tile(&s.map, north).unwrap();
    apply_command(
        &mut s,
        &Command::BuildVehicleAtDepot(north, crate::engine::ENGINE_SHIP_MPS),
    )
    .unwrap();
    let id = s.vehicles[0].id;

    apply_command(
        &mut s,
        &Command::SetVehicleOrderList(id, vec![VehicleOrder::depot(south)]),
    )
    .unwrap();

    assert_eq!(s.vehicles[0].orders[0].destination(), north);
    assert_eq!(s.vehicles[0].dest, north);
}

#[test]
fn ship_depot_commands_from_south_section_use_north_anchor() {
    use crate::engine::ENGINE_SHIP_MPS;

    let mut s = GameState::new(16, 10);
    let origin = TileCoord::new(5, 4);
    let [first, second] = crate::ship_depot_footprint(origin, 0);
    for tile in [first, second] {
        s.map.set_kind(tile, TileKind::Water).unwrap();
    }
    apply_command(&mut s, &Command::PlaceShipDepotDir(origin, 0)).unwrap();
    let north = crate::ship_depot_north_tile(&s.map, origin).unwrap();
    let south = crate::ship_depot_other_tile(&s.map, north).unwrap();

    apply_command(
        &mut s,
        &Command::BuildVehicleAtDepot(north, ENGINE_SHIP_MPS),
    )
    .unwrap();
    let source_id = s.vehicles[0].id;

    apply_command(
        &mut s,
        &Command::CloneVehicleAtDepot {
            source_vehicle_id: source_id,
            depot_pos: south,
        },
    )
    .unwrap();
    assert_eq!(s.vehicles.len(), 2);
    assert!(s.vehicles.iter().all(|vehicle| vehicle.pos == north));

    apply_command(
        &mut s,
        &Command::BuildVehicleAtDepot(south, ENGINE_SHIP_MPS),
    )
    .unwrap();
    assert_eq!(s.vehicles.len(), 3);
    for vehicle in &mut s.vehicles {
        vehicle.running = true;
        vehicle.autoreplace_attempted_this_stop = true;
    }

    apply_command(
        &mut s,
        &Command::SetDepotVehiclesRunning {
            depot_pos: south,
            running: false,
        },
    )
    .unwrap();
    assert!(s.vehicles.iter().all(|vehicle| !vehicle.running));

    apply_command(
        &mut s,
        &Command::DepotReorderVehicleSlot {
            depot_pos: south,
            from_slot: 0,
            to_slot: 2,
        },
    )
    .unwrap();
    assert_eq!(s.vehicles[0].depot_display_slot, Some(2));

    apply_command(&mut s, &Command::DepotMassAutoreplace { depot_pos: south }).unwrap();
    assert!(
        s.vehicles
            .iter()
            .all(|vehicle| !vehicle.autoreplace_attempted_this_stop)
    );

    apply_command(&mut s, &Command::SellAllVehiclesAtDepot(south)).unwrap();
    assert!(s.vehicles.is_empty());
}

#[test]
fn ship_paths_via_buoy() {
    use crate::pathfinder::{PathNetwork, find_path};

    let mut s = GameState::new(16, 10);
    for x in 2..=10 {
        s.map
            .set_kind(TileCoord::new(x, 4), TileKind::Water)
            .unwrap();
    }
    apply_command(&mut s, &Command::PlaceBuoy(TileCoord::new(6, 4))).unwrap();
    let path = find_path(
        &s.map,
        TileCoord::new(2, 4),
        TileCoord::new(10, 4),
        PathNetwork::Water,
    );
    assert!(path.is_some(), "ruta agua atraviesa boya");
    assert!(
        path.unwrap().contains(&TileCoord::new(6, 4)),
        "la ruta incluye la boya"
    );
}

#[test]
fn place_river_on_flat_and_inclined() {
    use crate::map::is_river_tile;

    let mut s = SandboxMap::flat_rich(12, 12, 1);
    let flat = TileCoord::new(4, 4);
    apply_command(&mut s, &Command::PlaceRiver(flat)).unwrap();
    assert!(s.map.get(flat).is_some_and(is_river_tile));

    // Pendiente NE en (6,4): río permitido.
    s.map.set_height(TileCoord::new(6, 4), 2).unwrap();
    s.map.set_height(TileCoord::new(6, 5), 2).unwrap();
    s.map.set_height(TileCoord::new(7, 4), 1).unwrap();
    s.map.set_height(TileCoord::new(7, 5), 1).unwrap();
    let slope = TileCoord::new(6, 4);
    apply_command(&mut s, &Command::PlaceRiver(slope)).unwrap();
    assert!(s.map.get(slope).is_some_and(is_river_tile));
    // Río en pendiente no es navegable.
    assert!(!crate::ship_movement::is_water_network_tile_at(
        &s.map, slope
    ));

    assert!(
        apply_command(&mut s, &Command::PlaceCanal(slope)).is_err(),
        "canal rechaza pendiente"
    );
}

#[test]
fn place_canal_sets_water_class_canal() {
    use crate::map::is_canal_tile;

    let mut s = GameState::new(8, 8);
    let c = TileCoord::new(3, 3);
    apply_command(&mut s, &Command::PlaceCanal(c)).unwrap();
    assert!(s.map.get(c).is_some_and(is_canal_tile));
}

#[test]
fn ship_paths_on_flat_river() {
    use crate::pathfinder::{PathNetwork, find_path};

    let mut s = SandboxMap::flat_rich(16, 10, 1);
    for x in 2..=10 {
        apply_command(&mut s, &Command::PlaceRiver(TileCoord::new(x, 4))).unwrap();
    }
    let path = find_path(
        &s.map,
        TileCoord::new(2, 4),
        TileCoord::new(10, 4),
        PathNetwork::Water,
    );
    assert!(path.is_some(), "río plano navegable");
}
