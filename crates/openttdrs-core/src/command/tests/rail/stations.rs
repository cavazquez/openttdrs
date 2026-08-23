//! Tests de comandos ferroviarios — estaciones, footprints y waypoints.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::command::{Command, CommandError, apply_command};
use crate::economy::{station_build_cost, waypoint_build_cost};
use crate::{
    GameState, STATION_TYPE_RAIL_WAYPOINT, StopKind, TileCoord, TileKind, station_type_from_m6,
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
    assert!(crate::station_tile_can_have_wires(tile.m3));
    assert!(crate::station_tile_can_have_pylons(tile.m3));
    assert_eq!(s.stations[0].stop_kind, StopKind::RailStation);
}

#[test]
fn rail_station_footprint_swaps_axes() {
    assert_eq!(crate::rail_station_footprint(false, 3, 5), (5, 3));
    assert_eq!(crate::rail_station_footprint(true, 3, 5), (3, 5));
    assert_eq!(crate::rail_station_footprint(false, 1, 1), (1, 1));
    assert_eq!(crate::rail_station_footprint(true, 2, 7), (2, 7));
}

#[test]
fn rail_station_layout_matches_place_area_m5_base() {
    // 3×5 eje X: andén impar (edificio) + par techado (extremos planos si length>4).
    let layout = crate::rail_station_layout(3, 5);
    assert_eq!(layout.len(), 15);
    assert_eq!(layout[0..5], [0, 0, 2, 0, 0]); // andén 0
    assert_eq!(layout[5..10], [0, 4, 4, 4, 0]); // andén 1 techado NW
    assert_eq!(layout[10..15], [0, 6, 6, 6, 0]); // andén 2 techado SE
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
            assert!(crate::station_tile_can_have_wires(t.m3));
            assert_eq!(
                crate::station_tile_can_have_pylons(t.m3),
                t.m5 < 4,
                "solo gfx sin techo permite postes"
            );
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
    assert_eq!(
        s.stations[0].station_spec,
        crate::StationSpecId::DefaultRail
    );
    let station_cost = station_build_cost(&s.global_economy);
    assert_eq!(s.economy.money, money_before - 15 * station_cost);
}

#[test]
fn place_rail_station_area_persists_newgrf_station_spec() {
    use crate::station_class::{StationClassDef, StationClassId, StationSpecDef, StationSpecId};

    let mut s = GameState::new(16, 16);
    let class_id = StationClassId::from_u16(1);
    let spec_id = StationSpecId::from_u16(1);
    s.station_class_catalog.push(StationClassDef {
        id: class_id,
        label: "Moderna".into(),
        short_label: "MODN".into(),
        from_newgrf: true,
    });
    s.station_spec_catalog.push(StationSpecDef {
        id: spec_id,
        class: class_id,
        label: "Andén NewGRF".into(),
        short_label: "Plat".into(),
        disallowed_platforms: 0,
        disallowed_lengths: 0,
        callback_mask: 0,
        from_newgrf: true,
        newgrf_preview: None,
        newgrf_views: Vec::new(),
        newgrf_local_id: 0,
        newgrf_runtime: None,
        newgrf_grfid: 0,
        newgrf_type_tables: None,
        custom_layouts: std::collections::HashMap::new(),
    });
    s.current_station_class = class_id;
    s.current_station_spec = spec_id;
    apply_command(
        &mut s,
        &Command::PlaceRailStationArea {
            origin: TileCoord::new(3, 3),
            axis_y: false,
            platforms: 1,
            length: 2,
        },
    )
    .unwrap();
    assert_eq!(s.stations[0].station_spec, spec_id);
}

#[test]
fn place_rail_station_area_rejects_disallowed_platforms_and_lengths() {
    use crate::station_class::{StationClassDef, StationClassId, StationSpecDef, StationSpecId};

    let mut s = GameState::new(16, 16);
    let class_id = StationClassId::from_u16(1);
    let spec_id = StationSpecId::from_u16(1);
    s.station_class_catalog.push(StationClassDef {
        id: class_id,
        label: "Restringida".into(),
        short_label: "RSTR".into(),
        from_newgrf: true,
    });
    // Bit1 = 2 andenes; bit2 = longitud 3 (bit n-1).
    s.station_spec_catalog.push(StationSpecDef {
        id: spec_id,
        class: class_id,
        label: "Solo 1×2".into(),
        short_label: "Solo".into(),
        disallowed_platforms: 0b0000_0010,
        disallowed_lengths: 0b0000_0100,
        callback_mask: 0,
        from_newgrf: true,
        newgrf_preview: None,
        newgrf_views: Vec::new(),
        newgrf_local_id: 0,
        newgrf_runtime: None,
        newgrf_grfid: 0,
        newgrf_type_tables: None,
        custom_layouts: std::collections::HashMap::new(),
    });
    s.current_station_class = class_id;
    s.current_station_spec = spec_id;

    let err_plat = apply_command(
        &mut s,
        &Command::PlaceRailStationArea {
            origin: TileCoord::new(3, 3),
            axis_y: false,
            platforms: 2,
            length: 2,
        },
    );
    assert_eq!(err_plat, Err(CommandError::StationSizeNotAllowed));

    let err_len = apply_command(
        &mut s,
        &Command::PlaceRailStationArea {
            origin: TileCoord::new(3, 3),
            axis_y: false,
            platforms: 1,
            length: 3,
        },
    );
    assert_eq!(err_len, Err(CommandError::StationSizeNotAllowed));

    apply_command(
        &mut s,
        &Command::PlaceRailStationArea {
            origin: TileCoord::new(3, 3),
            axis_y: false,
            platforms: 1,
            length: 2,
        },
    )
    .unwrap();
    assert_eq!(s.stations.len(), 1);
}

#[test]
fn place_rail_station_0e_layout_writes_tiletypes_for_distinct_views() {
    use crate::newgrf_sprites::DecodedSprite;
    use crate::station_class::{
        StationClassDef, StationClassId, StationSpecDef, StationSpecId, station_newgrf_view_index,
    };

    fn solid(r: u8, g: u8, b: u8) -> DecodedSprite {
        DecodedSprite {
            width: 2,
            height: 2,
            x_offs: 0,
            y_offs: 0,
            rgba: vec![r, g, b, 255, r, g, b, 255, r, g, b, 255, r, g, b, 255],
            mask: Vec::new(),
        }
    }

    let mut s = GameState::new(16, 16);
    let class_id = StationClassId::from_u16(1);
    let spec_id = StationSpecId::from_u16(1);
    let mut layouts = std::collections::HashMap::new();
    // 1×2: tiletypes 0 y 2 → vistas NewGRF distintas tras build (axis X).
    layouts.insert((1, 2), vec![0, 2]);
    s.station_class_catalog.push(StationClassDef {
        id: class_id,
        label: "Moderna".into(),
        short_label: "MODN".into(),
        from_newgrf: true,
    });
    s.station_spec_catalog.push(StationSpecDef {
        id: spec_id,
        class: class_id,
        label: "Andén 0x0E".into(),
        short_label: "Plat".into(),
        disallowed_platforms: 0,
        disallowed_lengths: 0,
        callback_mask: 0,
        from_newgrf: true,
        newgrf_preview: None,
        newgrf_views: vec![solid(255, 0, 0), solid(0, 255, 0), solid(0, 0, 255)],
        newgrf_local_id: 0,
        newgrf_runtime: None,
        newgrf_grfid: 0,
        newgrf_type_tables: None,
        custom_layouts: layouts,
    });
    s.current_station_class = class_id;
    s.current_station_spec = spec_id;
    apply_command(
        &mut s,
        &Command::PlaceRailStationArea {
            origin: TileCoord::new(3, 3),
            axis_y: false,
            platforms: 1,
            length: 2,
        },
    )
    .unwrap();

    let m5_a = s.map.get(TileCoord::new(3, 3)).unwrap().m5;
    let m5_b = s.map.get(TileCoord::new(4, 3)).unwrap().m5;
    assert_eq!(m5_a, 0);
    assert_eq!(m5_b, 2);
    assert_eq!(station_newgrf_view_index(m5_a), 0);
    assert_eq!(station_newgrf_view_index(m5_b), 2);

    let def = crate::station_spec_def(&s.station_spec_catalog, spec_id).unwrap();
    let va = def.newgrf_view(station_newgrf_view_index(m5_a)).unwrap();
    let vb = def.newgrf_view(station_newgrf_view_index(m5_b)).unwrap();
    assert_ne!(
        va.rgba, vb.rgba,
        "tiletypes 0x0E deben elegir sprites distintos"
    );
}

#[test]
fn place_rail_station_cb24_overrides_0e_tiletype() {
    use crate::newgrf_sprites::{
        Action2VarAdjust, Action2VarEntry, Action2VarTerm, TrainSpriteAssign, TrainSpriteGraphics,
    };
    use crate::station_class::{StationClassDef, StationClassId, StationSpecDef, StationSpecId};

    let mut gfx = TrainSpriteGraphics::default();
    gfx.assigns.push(TrainSpriteAssign {
        local_id: 0,
        set_id: 5,
    });
    gfx.action2_var.insert(
        5,
        Action2VarEntry {
            first: Action2VarTerm {
                variable: 0x1A,
                param: None,
                adjust: Action2VarAdjust {
                    shift: 0,
                    and_mask: 6,
                    add_val: None,
                    divide_val: None,
                    modulo_val: None,
                },
            },
            ops: Vec::new(),
            ranges: Vec::new(),
            default: 0,
        },
    );

    let mut s = GameState::new(16, 16);
    let class_id = StationClassId::from_u16(1);
    let spec_id = StationSpecId::from_u16(1);
    let mut layouts = std::collections::HashMap::new();
    layouts.insert((1, 1), vec![0]); // 0x0E diría 0; CB24 → 6
    s.station_class_catalog.push(StationClassDef {
        id: class_id,
        label: "CB24".into(),
        short_label: "CB24".into(),
        from_newgrf: true,
    });
    s.station_spec_catalog.push(StationSpecDef {
        id: spec_id,
        class: class_id,
        label: "Callback 24".into(),
        short_label: "Cb24".into(),
        disallowed_platforms: 0,
        disallowed_lengths: 0,
        callback_mask: 0,
        from_newgrf: true,
        newgrf_preview: None,
        newgrf_views: Vec::new(),
        newgrf_local_id: 0,
        newgrf_runtime: Some(Box::new(gfx)),
        newgrf_grfid: 1,
        newgrf_type_tables: None,
        custom_layouts: layouts,
    });
    s.current_station_class = class_id;
    s.current_station_spec = spec_id;
    apply_command(
        &mut s,
        &Command::PlaceRailStationArea {
            origin: TileCoord::new(4, 4),
            axis_y: false,
            platforms: 1,
            length: 1,
        },
    )
    .unwrap();
    assert_eq!(s.map.get(TileCoord::new(4, 4)).unwrap().m5, 6);
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
fn place_rail_waypoint_on_straight_track() {
    let mut s = GameState::new(8, 8);
    let c = TileCoord::new(3, 3);
    apply_command(&mut s, &Command::SetRailBits(c, 0x01)).unwrap();
    let money = s.economy.money;
    apply_command(&mut s, &Command::PlaceRailWaypoint(c)).unwrap();
    let tile = s.map.get(c).unwrap();
    assert_eq!(tile.kind, TileKind::Station);
    assert_eq!(station_type_from_m6(tile.m6), STATION_TYPE_RAIL_WAYPOINT);
    assert!(crate::station_tile_can_have_wires(tile.m3));
    assert!(crate::station_tile_can_have_pylons(tile.m3));
    assert_eq!(s.stations.len(), 1);
    assert_eq!(s.stations[0].stop_kind, StopKind::RailWaypoint);
    assert_eq!(
        s.stations[0].station_spec,
        crate::station_class::StationSpecId::DEFAULT_RAIL
    );
    assert_eq!(
        s.economy.money,
        money - waypoint_build_cost(&s.global_economy)
    );
}

#[test]
fn place_rail_waypoint_persists_current_station_spec() {
    let mut s = GameState::new(8, 8);
    let custom = crate::station_class::StationSpecId::from_u16(42);
    s.current_station_spec = custom;
    let c = TileCoord::new(3, 3);
    apply_command(&mut s, &Command::SetRailBits(c, 0x01)).unwrap();
    apply_command(&mut s, &Command::PlaceRailWaypoint(c)).unwrap();
    assert_eq!(s.stations[0].station_spec, custom);
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
fn place_rail_rejects_overwrite_of_rail_station() {
    let mut s = GameState::new(8, 8);
    let st = TileCoord::new(3, 3);
    apply_command(&mut s, &Command::PlaceRail(TileCoord::new(2, 3))).unwrap();
    apply_command(&mut s, &Command::PlaceRailStation(st, 0)).unwrap();
    assert_eq!(s.map.get_kind(st), Some(TileKind::Station));
    assert_eq!(s.map.get(st).unwrap().mapt, 0x50);
    assert_eq!(
        apply_command(&mut s, &Command::PlaceRail(st)),
        Err(CommandError::CannotPlaceStationOnOccupiedTile)
    );
    assert_eq!(s.map.get_kind(st), Some(TileKind::Station));
    assert_eq!(s.map.get(st).unwrap().mapt, 0x50);
    assert_eq!(s.stations.len(), 1);
    assert_eq!(s.stations[0].pos, st);
}
