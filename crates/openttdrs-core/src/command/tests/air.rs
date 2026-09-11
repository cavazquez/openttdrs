//! Tests de aeropuerto, canal y esclusa.

use crate::economy::station_build_cost;
use crate::{
    AircraftPhase, Command, CommandError, DEPOT_BUILD_COST, ENGINE_AIRCRAFT_DAKOTA,
    ENGINE_AIRCRAFT_TRICARIO, ENGINE_SHIP_FERRY, GameState, StopKind, TileCoord, TileKind,
    VehicleKind, airport_tile_is_hangar, airport_tile_is_heliport, apply_command,
    command_would_fail,
};
use crate::{AirportClassId, AirportLayoutTile, AirportTileLayout, NewgrfAirportSpecDef};

#[test]
fn place_heliport_and_buy_helicopter() {
    let mut s = GameState::new(12, 12);
    let c = TileCoord::new(4, 4);
    let money = s.economy.money;
    apply_command(&mut s, &Command::PlaceAirport(c)).unwrap();
    assert_eq!(s.map.get_kind(c), Some(TileKind::Airport));
    assert!(airport_tile_is_heliport(&s.map, c));
    assert_eq!(s.stations.len(), 1);
    assert_eq!(s.stations[0].stop_kind, StopKind::Airport);
    assert!(s.stations[0].can_service_vehicle(VehicleKind::Aircraft));
    assert_eq!(s.economy.money, money - DEPOT_BUILD_COST);

    // Aviones no se compran en helipuerto.
    let err = apply_command(
        &mut s,
        &Command::BuildVehicleAtDepot(c, ENGINE_AIRCRAFT_DAKOTA),
    )
    .unwrap_err();
    assert!(matches!(err, crate::CommandError::VehicleKindNotAllowed));

    apply_command(
        &mut s,
        &Command::BuildVehicleAtDepot(c, ENGINE_AIRCRAFT_TRICARIO),
    )
    .unwrap();
    assert!(
        s.vehicles
            .iter()
            .any(|v| v.kind == VehicleKind::Aircraft && v.pos == c)
    );
}

#[test]
fn airport_cannot_overwrite_nonremovable_object() {
    let mut s = GameState::new(12, 12);
    let c = TileCoord::new(4, 4);
    apply_command(
        &mut s,
        &Command::BuildObject {
            pos: c,
            object_type: crate::OBJECT_TYPE_LIGHTHOUSE,
        },
    )
    .unwrap();

    assert_eq!(
        command_would_fail(&s, &Command::PlaceAirport(c)),
        Some(CommandError::ObjectCannotBeRemoved)
    );
    assert_eq!(
        apply_command(&mut s, &Command::PlaceAirport(c)),
        Err(CommandError::ObjectCannotBeRemoved)
    );
    assert!(crate::is_map_object_tile(s.map.get(c).unwrap().mapt));
}

#[test]
fn place_airport_small_footprint_and_hangar_buy() {
    let mut s = GameState::new(20, 20);
    let origin = TileCoord::new(2, 2);
    apply_command(
        &mut s,
        &Command::PlaceAirportArea {
            origin,
            axis_y: false,
            spec: crate::AirportSpecId::Small,
        },
    )
    .unwrap();
    assert_eq!(s.stations.len(), 1);
    assert_eq!(s.stations[0].airport_tiles.len(), 12);
    let hangar = s.stations[0].pos;
    assert!(airport_tile_is_hangar(&s.map, hangar));
    // Compra solo en hangar.
    apply_command(
        &mut s,
        &Command::BuildVehicleAtDepot(hangar, ENGINE_AIRCRAFT_DAKOTA),
    )
    .unwrap();
    // Apron no es hangar.
    let apron = s.stations[0]
        .airport_tiles
        .iter()
        .copied()
        .find(|&c| !airport_tile_is_hangar(&s.map, c))
        .unwrap();
    let err = apply_command(
        &mut s,
        &Command::BuildVehicleAtDepot(apron, ENGINE_AIRCRAFT_DAKOTA),
    )
    .unwrap_err();
    assert!(matches!(err, crate::CommandError::InvalidDepotTile));
}

#[test]
fn aircraft_purchase_keeps_secondary_mail_capacity_on_primary() {
    let mut s = GameState::new(20, 20);
    let origin = TileCoord::new(2, 2);
    apply_command(
        &mut s,
        &Command::PlaceAirportArea {
            origin,
            axis_y: false,
            spec: crate::AirportSpecId::Small,
        },
    )
    .unwrap();

    let mut engine =
        crate::engine::engine_for_vehicle(VehicleKind::Aircraft, ENGINE_AIRCRAFT_DAKOTA).clone();
    engine.id = 0x7E01;
    engine.name = "Mail shadow test aircraft".into();
    engine.mail_capacity = 7;
    s.engine_catalog.push(engine);

    let hangar = s.stations[0].pos;
    apply_command(&mut s, &Command::BuildVehicleAtDepot(hangar, 0x7E01)).unwrap();
    let aircraft = s
        .vehicles
        .iter()
        .find(|vehicle| vehicle.kind == VehicleKind::Aircraft)
        .expect("avión comprado");
    let aircraft_id = aircraft.id;
    assert_eq!(aircraft.aircraft_mail_capacity, Some(7));

    apply_command(
        &mut s,
        &Command::RefitVehicle {
            vehicle_id: aircraft_id,
            cargo: crate::CargoType::Mail,
            unit_ids: Vec::new(),
        },
    )
    .unwrap();
    let aircraft = s
        .vehicles
        .iter()
        .find(|vehicle| vehicle.id == aircraft_id)
        .expect("avión refitado a correo");
    assert_eq!(aircraft.cargo_type, Some(crate::CargoType::Mail));
    assert_eq!(aircraft.aircraft_mail_capacity, Some(0));

    apply_command(
        &mut s,
        &Command::RefitVehicle {
            vehicle_id: aircraft_id,
            cargo: crate::CargoType::Passengers,
            unit_ids: Vec::new(),
        },
    )
    .unwrap();
    let aircraft = s
        .vehicles
        .iter()
        .find(|vehicle| vehicle.id == aircraft_id)
        .expect("avión refitado a pasajeros");
    assert_eq!(aircraft.cargo_type, Some(crate::CargoType::Passengers));
    assert_eq!(aircraft.aircraft_mail_capacity, Some(7));
}

#[test]
fn newgrf_airport_build_uses_declared_east_layout_without_transposing_tiles() {
    // `AirportTileTableIterator` usa los offsets Action0 tal cual. La segunda
    // variante no es la transposición de la primera: además de ocupar 2×4,
    // publica un gfx distinto para verificar que se eligió el layout E real.
    let mut s = GameState::new(16, 16);
    s.airport_spec_catalog.push(NewgrfAirportSpecDef {
        id: 10,
        class: AirportClassId::Small,
        label: "Rotaciones".into(),
        short_label: "Rot".into(),
        size_x: 4,
        size_y: 2,
        catchment: 4,
        noise_level: 1,
        subst_id: crate::AirportSpecId::Small,
        ttd_airport_type: 0,
        layouts: vec![
            AirportTileLayout {
                rotation: 0,
                tiles: vec![
                    AirportLayoutTile {
                        x: 0,
                        y: 0,
                        gfx: 24,
                    },
                    AirportLayoutTile {
                        x: 3,
                        y: 1,
                        gfx: 14,
                    },
                ],
            },
            AirportTileLayout {
                rotation: 2,
                tiles: vec![
                    AirportLayoutTile {
                        x: 0,
                        y: 0,
                        gfx: 24,
                    },
                    AirportLayoutTile {
                        x: 1,
                        y: 3,
                        gfx: 18,
                    },
                ],
            },
        ],
        enabled: true,
        min_year: 0,
        max_year: u16::MAX,
        maintenance_cost: 0,
        associated_badges: Vec::new(),
        newgrf_local_id: 0,
        newgrf_grfid: 0,
        newgrf_views: Vec::new(),
        newgrf_purchase_views: Vec::new(),
    });
    s.current_airport_newgrf_id = Some(10);
    let origin = TileCoord::new(2, 2);
    assert_eq!(
        crate::airport::newgrf_airport_footprint(&s.airport_spec_catalog[0], true),
        (2, 4),
        "el área E intercambia sólo las dimensiones, no los offsets Action0"
    );

    assert!(
        apply_command(
            &mut s,
            &Command::PlaceAirportArea {
                origin,
                axis_y: true,
                spec: crate::AirportSpecId::Small,
            },
        )
        .is_ok(),
        "airport NewGRF este"
    );

    let station = &s.stations[0];
    assert_eq!(station.airport_layout, 1);
    assert_eq!(station.airport_rotation, 2);
    assert_eq!(
        station.airport_tiles,
        vec![TileCoord::new(2, 2), TileCoord::new(3, 5)],
        "los offsets E de Action0 no se transponen"
    );
    assert_eq!(
        station.airport_tile_gfx,
        vec![(TileCoord::new(2, 2), 24), (TileCoord::new(3, 5), 18)],
        "se conserva el gfx de la variante E seleccionada"
    );
    assert_eq!(
        s.map.get_kind(TileCoord::new(3, 5)),
        Some(TileKind::Airport)
    );
    assert_ne!(
        s.map.get_kind(TileCoord::new(5, 3)),
        Some(TileKind::Airport)
    );
}

#[test]
fn explicit_newgrf_layout_uses_only_declared_tiles_and_rejects_unknown_index() {
    // `CheckFlatLandAirport` itera `AirportTileTableIterator`: una tesela
    // ocupada dentro del rectángulo declarado, pero fuera del layout, no puede
    // bloquear ni ser limpiada por la construcción.
    let mut s = GameState::new(12, 12);
    s.airport_spec_catalog.push(NewgrfAirportSpecDef {
        id: 10,
        class: AirportClassId::Small,
        label: "Layout sparse".into(),
        short_label: "Sparse".into(),
        size_x: 4,
        size_y: 3,
        catchment: 4,
        noise_level: 1,
        subst_id: crate::AirportSpecId::Small,
        ttd_airport_type: 0,
        layouts: vec![AirportTileLayout {
            rotation: 4,
            tiles: vec![
                AirportLayoutTile {
                    x: 0,
                    y: 0,
                    gfx: 24,
                },
                AirportLayoutTile {
                    x: 3,
                    y: 2,
                    gfx: 14,
                },
            ],
        }],
        enabled: true,
        min_year: 0,
        max_year: u16::MAX,
        maintenance_cost: 0,
        associated_badges: Vec::new(),
        newgrf_local_id: 0,
        newgrf_grfid: 0,
        newgrf_views: Vec::new(),
        newgrf_purchase_views: Vec::new(),
    });
    // El comando explícito lleva el id; no depende de una selección de picker
    // que pudo cambiar antes de aplicarse (red/replay).
    s.current_airport_newgrf_id = None;
    let origin = TileCoord::new(2, 2);
    let undeclared = TileCoord::new(3, 3);
    s.map.set_kind(undeclared, TileKind::Rail).unwrap();

    apply_command(
        &mut s,
        &Command::PlaceAirportAreaWithLayout {
            origin,
            newgrf_spec_id: 10,
            layout: 0,
            spec: crate::AirportSpecId::Small,
        },
    )
    .unwrap();

    let station = &s.stations[0];
    assert_eq!(station.airport_layout, 0);
    assert_eq!(station.airport_rotation, 4);
    assert_eq!(
        station.airport_tiles,
        vec![TileCoord::new(2, 2), TileCoord::new(5, 4)]
    );
    assert_eq!(s.map.get_kind(undeclared), Some(TileKind::Rail));

    let mut invalid = GameState::new(12, 12);
    invalid.airport_spec_catalog = s.airport_spec_catalog.clone();
    invalid.current_airport_newgrf_id = None;
    let err = apply_command(
        &mut invalid,
        &Command::PlaceAirportAreaWithLayout {
            origin,
            newgrf_spec_id: 10,
            layout: 1,
            spec: crate::AirportSpecId::Small,
        },
    )
    .unwrap_err();
    assert_eq!(err, crate::CommandError::InvalidAirportLayout);
}

#[test]
fn place_canal_converts_grass_to_water() {
    let mut s = GameState::new(8, 8);
    let c = TileCoord::new(3, 3);
    let money = s.economy.money;
    apply_command(&mut s, &Command::PlaceCanal(c)).unwrap();
    assert_eq!(s.map.get_kind(c), Some(TileKind::Water));
    assert_eq!(
        s.economy.money,
        money - station_build_cost(&s.global_economy) / 2
    );
}

#[test]
fn place_lock_requires_height_delta() {
    let mut s = GameState::new(8, 8);
    let c = TileCoord::new(2, 2);
    let a = TileCoord::new(1, 2);
    let b = TileCoord::new(3, 2);
    for t in [a, c, b] {
        s.map.set_kind(t, TileKind::Water).unwrap();
    }
    // Misma altura → rechazo.
    assert!(apply_command(&mut s, &Command::PlaceLock(c, false)).is_err());
    s.map.set_height(a, 1).unwrap();
    s.map.set_height(c, 1).unwrap();
    s.map.set_height(b, 2).unwrap();
    apply_command(&mut s, &Command::PlaceLock(c, false)).unwrap();
    let tile = s.map.get(c).unwrap();
    assert_eq!(tile.m5 >> 4, 2);
}

#[test]
fn place_city_airport_footprint() {
    let mut s = GameState::new(24, 24);
    let origin = TileCoord::new(2, 2);
    apply_command(
        &mut s,
        &Command::PlaceAirportArea {
            origin,
            axis_y: false,
            spec: crate::AirportSpecId::City,
        },
    )
    .unwrap();
    assert_eq!(s.stations[0].airport_tiles.len(), 36);
    assert!(airport_tile_is_hangar(&s.map, s.stations[0].pos));
}

#[test]
fn place_international_airport_footprint() {
    let mut s = GameState::new(24, 24);
    let origin = TileCoord::new(1, 1);
    apply_command(
        &mut s,
        &Command::PlaceAirportArea {
            origin,
            axis_y: false,
            spec: crate::AirportSpecId::International,
        },
    )
    .unwrap();
    assert_eq!(s.stations[0].airport_tiles.len(), 49);
    let hangar = s.stations[0].pos;
    assert!(airport_tile_is_hangar(&s.map, hangar));
    apply_command(
        &mut s,
        &Command::BuildVehicleAtDepot(hangar, ENGINE_AIRCRAFT_DAKOTA),
    )
    .unwrap();
}

#[test]
fn ferry_engine_is_passenger_ship() {
    let eng = crate::engine_by_id(ENGINE_SHIP_FERRY).unwrap();
    assert_eq!(eng.kind, VehicleKind::Ship);
    assert_eq!(eng.cargo, Some(crate::CargoType::Passengers));
}

#[test]
fn aircraft_phase_starts_on_heliport_pad() {
    let mut s = GameState::new(12, 12);
    let c = TileCoord::new(4, 4);
    apply_command(&mut s, &Command::PlaceAirport(c)).unwrap();
    apply_command(
        &mut s,
        &Command::BuildVehicleAtDepot(c, ENGINE_AIRCRAFT_TRICARIO),
    )
    .unwrap();
    // Heliport/Oilrig FTA: sin hangar; arranca en pad (Taxi + Helipad1).
    assert_eq!(s.vehicles[0].aircraft_phase, AircraftPhase::Taxi);
    assert!(s.vehicles[0].airport_fta_active);
    assert_eq!(s.vehicles[0].altitude, 0);
}

#[test]
fn small_airport_accepts_helicopter_and_airplane() {
    let mut s = GameState::new(20, 20);
    let origin = TileCoord::new(2, 2);
    apply_command(
        &mut s,
        &Command::PlaceAirportArea {
            origin,
            axis_y: false,
            spec: crate::AirportSpecId::Small,
        },
    )
    .unwrap();
    let hangar = s.stations[0].pos;
    assert!(!airport_tile_is_heliport(&s.map, hangar));
    // Country: Airplanes + Helicopters (+ ShortStrip).
    apply_command(
        &mut s,
        &Command::BuildVehicleAtDepot(hangar, ENGINE_AIRCRAFT_TRICARIO),
    )
    .unwrap();
    apply_command(
        &mut s,
        &Command::BuildVehicleAtDepot(hangar, ENGINE_AIRCRAFT_DAKOTA),
    )
    .unwrap();
    assert_eq!(
        s.vehicles
            .iter()
            .filter(|v| v.kind == VehicleKind::Aircraft)
            .count(),
        2
    );
}

#[test]
fn airplane_order_to_heliport_rejected() {
    let mut s = GameState::new(24, 24);
    let heliport = TileCoord::new(2, 2);
    apply_command(&mut s, &Command::PlaceAirport(heliport)).unwrap();
    let small_origin = TileCoord::new(8, 2);
    apply_command(
        &mut s,
        &Command::PlaceAirportArea {
            origin: small_origin,
            axis_y: false,
            spec: crate::AirportSpecId::Small,
        },
    )
    .unwrap();
    let hangar = s.stations.iter().find(|st| st.pos != heliport).unwrap().pos;
    apply_command(
        &mut s,
        &Command::BuildVehicleAtDepot(hangar, ENGINE_AIRCRAFT_DAKOTA),
    )
    .unwrap();
    let plane_id = s.vehicles[0].id;
    let err = apply_command(
        &mut s,
        &Command::SetVehicleOrderList(
            plane_id,
            vec![crate::vehicle::VehicleOrder::station(heliport)],
        ),
    )
    .unwrap_err();
    assert!(matches!(
        err,
        crate::CommandError::IncompatibleStopForVehicle
    ));

    // Hélico sí puede ordenar al helipuerto.
    apply_command(
        &mut s,
        &Command::BuildVehicleAtDepot(heliport, ENGINE_AIRCRAFT_TRICARIO),
    )
    .unwrap();
    let heli_id = s.vehicles.iter().find(|v| v.id != plane_id).unwrap().id;
    apply_command(
        &mut s,
        &Command::SetVehicleOrderList(
            heli_id,
            vec![crate::vehicle::VehicleOrder::station(heliport)],
        ),
    )
    .unwrap();
}

#[test]
fn airport_noise_rejects_intercontinental_near_small_town() {
    let mut s = GameState::new(40, 40);
    s.station_noise_level = true;
    s.towns.push(crate::Town {
        id: 1,
        pos: TileCoord::new(5, 5),
        name: "Villa".into(),
        population: 100, // MaxTownNoise = 100/800+3 = 3
        authority_ratings: vec![500],
        ..crate::Town::default()
    });
    let err = apply_command(
        &mut s,
        &Command::PlaceAirportArea {
            origin: TileCoord::new(4, 4),
            axis_y: false,
            spec: crate::AirportSpecId::Intercontinental, // noise 25
        },
    )
    .unwrap_err();
    assert!(matches!(err, crate::CommandError::AirportNoiseTooHigh));

    // Helipuerto (noise 1) cabe en max 3.
    apply_command(&mut s, &Command::PlaceAirport(TileCoord::new(8, 8))).unwrap();
    assert_eq!(s.towns[0].noise_reached, 1);
}

#[test]
fn airplane_order_to_small_and_intercon_ok() {
    let mut s = GameState::new(40, 40);
    apply_command(
        &mut s,
        &Command::PlaceAirportArea {
            origin: TileCoord::new(2, 2),
            axis_y: false,
            spec: crate::AirportSpecId::Small,
        },
    )
    .unwrap();
    apply_command(
        &mut s,
        &Command::PlaceAirportArea {
            origin: TileCoord::new(12, 2),
            axis_y: false,
            spec: crate::AirportSpecId::Intercontinental,
        },
    )
    .unwrap();
    let small = s
        .stations
        .iter()
        .find(|st| st.airport_spec == crate::AirportSpecId::Small)
        .unwrap()
        .pos;
    let inter = s
        .stations
        .iter()
        .find(|st| st.airport_spec == crate::AirportSpecId::Intercontinental)
        .unwrap()
        .pos;
    apply_command(
        &mut s,
        &Command::BuildVehicleAtDepot(small, ENGINE_AIRCRAFT_DAKOTA),
    )
    .unwrap();
    let id = s.vehicles[0].id;
    apply_command(
        &mut s,
        &Command::SetVehicleOrderList(
            id,
            vec![
                crate::vehicle::VehicleOrder::station(small),
                crate::vehicle::VehicleOrder::station(inter),
            ],
        ),
    )
    .unwrap();
}
