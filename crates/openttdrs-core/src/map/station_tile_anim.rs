//! Animación de teselas de estación / aeropuerto (`AnimateTile_Airport`).

use crate::airport::{AirportPiece, airport_station_gfx_animation_frames};
use crate::map::{Map, TileCoord, TileKind};
use crate::newgrf_callback::{advance_road_stop_animation, trigger_road_stop_animation};
use crate::road_stop_spec::{
    ROADSTOP_ANIMATION_TRIGGER_TILE_LOOP, RoadStopSpecDef, road_stop_spec_def,
};
use crate::station::{Station, StopKind};

/// Frames del radar vanilla (`SPR_AIRPORT_RADAR_1` … `_12`).
pub const AIRPORT_RADAR_FRAMES: u8 = 12;

/// Avanza `m7` en las teselas airport animadas; coste O(aeropuertos), no O(mapa).
pub fn step_airport_tiles(map: &mut Map, tick: u64, stations: &[Station]) -> Vec<TileCoord> {
    // Un frame cada 3 ticks ≈ ritmo visual cercano a OpenTTD.
    if !tick.is_multiple_of(3) {
        return Vec::new();
    }
    let mut dirty = Vec::new();
    for station in stations {
        // Los saves importados pueden mezclar instalaciones bajo el mismo
        // StationID. En ese caso `ottd_station_id` identifica que `m5` es el
        // StationGfx airport real, aun si `stop_kind` no quedó Airport.
        let imported_station_gfx = station.ottd_station_id.is_some();
        if !imported_station_gfx && station.stop_kind != StopKind::Airport {
            continue;
        }
        let tiles = if station.airport_tiles.is_empty() {
            std::slice::from_ref(&station.pos)
        } else {
            station.airport_tiles.as_slice()
        };
        for &pos in tiles {
            let Some(mut tile) = map.get(pos) else {
                continue;
            };
            let frames = if imported_station_gfx {
                airport_station_gfx_animation_frames(tile.m5)
            } else if is_airport_tower_tile(tile.kind, tile.m5) {
                Some(AIRPORT_RADAR_FRAMES)
            } else {
                None
            };
            let Some(frames) = frames else {
                continue;
            };
            tile.m7 = tile.m7.wrapping_add(1) % frames;
            let _ = map.set_tile(pos, tile);
            dirty.push(pos);
        }
    }
    dirty.sort_by_key(|c| (c.x, c.y));
    dirty.dedup();
    dirty
}

/// Ejecuta el subconjunto runtime de animación de paradas viales NewGRF.
///
/// La instancia `Station` contiene el frame y el registro activo equivalentes
/// a `roadstoptiledata`. Por ahora los call sites de trigger cubiertos son
/// `Built` (en el comando de construcción) y `TileLoop`; carga, llegada y
/// salida de vehículo permanecen fuera de este scheduler.
pub fn step_newgrf_road_stop_tiles(
    map: &Map,
    tick: u64,
    stations: &mut [Station],
    catalog: &[RoadStopSpecDef],
    tile_loop_visits: &[(TileCoord, crate::map::Tile)],
) -> Vec<TileCoord> {
    let mut dirty = Vec::new();

    for (coord, tile) in tile_loop_visits {
        if tile.kind != TileKind::Station {
            continue;
        }
        let Some(index) = stations.iter().position(|station| station.pos == *coord) else {
            continue;
        };
        let Some(spec_id) = stations[index].road_stop_spec else {
            continue;
        };
        let Some(def) = road_stop_spec_def(catalog, spec_id) else {
            continue;
        };
        if trigger_road_stop_animation(
            def,
            &mut stations[index],
            tile.m5,
            ROADSTOP_ANIMATION_TRIGGER_TILE_LOOP,
            tick,
        ) {
            dirty.push(*coord);
        }
    }

    for station in stations {
        if !matches!(station.stop_kind, StopKind::BusStop | StopKind::TruckStop) {
            continue;
        }
        let Some(spec_id) = station.road_stop_spec else {
            continue;
        };
        let Some(def) = road_stop_spec_def(catalog, spec_id) else {
            continue;
        };
        let Some(tile) = map.get(station.pos) else {
            continue;
        };
        if advance_road_stop_animation(def, station, tile.m5, tick) {
            dirty.push(station.pos);
        }
    }

    dirty.sort_by_key(|coord| (coord.x, coord.y));
    dirty.dedup();
    dirty
}

/// Frame de radar 0..11 desde `m7`.
#[must_use]
pub const fn airport_radar_frame(m7: u8) -> u8 {
    m7 % AIRPORT_RADAR_FRAMES
}

#[must_use]
pub fn is_airport_tower_tile(kind: TileKind, m5: u8) -> bool {
    kind == TileKind::Airport && AirportPiece::from_m5(m5) == AirportPiece::Tower
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::newgrf_sprites::{
        Action2VarAdjust, Action2VarEntry, Action2VarTerm, TrainSpriteAssign, TrainSpriteGraphics,
    };
    use crate::road_stop_spec::{
        ROADSTOP_ANIMATION_TRIGGER_TILE_LOOP, ROADSTOP_CALLBACK_MASK_ANIMATION_NEXT_FRAME,
        ROADSTOP_CALLBACK_MASK_ANIMATION_SPEED, ROADSTOP_DRAW_MODE_DEFAULT, RoadStopSpecDef,
    };

    fn callback_literal(value: u8) -> Action2VarEntry {
        Action2VarEntry {
            first: Action2VarTerm {
                variable: 0x1A,
                param: None,
                adjust: Action2VarAdjust {
                    shift: 0,
                    and_mask: value,
                    ..Action2VarAdjust::default()
                },
            },
            ops: Vec::new(),
            ranges: Vec::new(),
            default: 0,
        }
    }

    /// Runtime sintético que distingue 0x140, 0x141 y 0x142 por el byte bajo
    /// de `var 0x0C`, como hacen los callbacks de 15 bits en Action2.
    fn road_stop_animation_callbacks() -> TrainSpriteGraphics {
        let mut gfx = TrainSpriteGraphics::default();
        gfx.assigns.push(TrainSpriteAssign {
            local_id: 0,
            set_id: 2,
        });
        gfx.action2_var.insert(
            2,
            Action2VarEntry {
                first: Action2VarTerm {
                    variable: 0x0C,
                    param: None,
                    adjust: Action2VarAdjust {
                        shift: 0,
                        and_mask: 0xFF,
                        ..Action2VarAdjust::default()
                    },
                },
                ops: Vec::new(),
                ranges: vec![(4, 0x40, 0x40), (5, 0x41, 0x41), (6, 0x42, 0x42)],
                default: 0,
            },
        );
        // CB140 inicia; CB141 fija el frame 3; CB142 espera 2^2 ticks.
        gfx.action2_var.insert(4, callback_literal(0xFE));
        gfx.action2_var.insert(5, callback_literal(3));
        gfx.action2_var.insert(6, callback_literal(2));
        gfx
    }

    fn animated_road_stop_spec() -> RoadStopSpecDef {
        RoadStopSpecDef {
            id: 7,
            class: 0,
            label: "Animada".into(),
            short_label: "ANIM".into(),
            stop_type: crate::ROADSTOP_TYPE_BUS,
            from_newgrf: true,
            grfid: 0x414E_494D,
            newgrf_local_id: 0,
            draw_mode: ROADSTOP_DRAW_MODE_DEFAULT,
            flags: 0,
            callback_mask: ROADSTOP_CALLBACK_MASK_ANIMATION_NEXT_FRAME
                | ROADSTOP_CALLBACK_MASK_ANIMATION_SPEED,
            animation_status: 1,
            animation_frames: 5,
            animation_speed: 0,
            animation_triggers: ROADSTOP_ANIMATION_TRIGGER_TILE_LOOP,
            newgrf_views: Vec::new(),
            newgrf_runtime: Some(Box::new(road_stop_animation_callbacks())),
            newgrf_type_tables: None,
            associated_badges: Vec::new(),
        }
    }

    #[test]
    fn radar_frame_cycles_on_tower() {
        let mut map = Map::new_flat(4, 4, 1);
        let pos = TileCoord::new(1, 1);
        let mut tile = map.get(pos).unwrap();
        tile.kind = TileKind::Airport;
        tile.m5 = AirportPiece::Tower as u8;
        map.set_tile(pos, tile).unwrap();

        let mut station = Station::new_with_kind(pos, StopKind::Airport);
        station.airport_tiles = vec![pos];

        assert!(is_airport_tower_tile(
            TileKind::Airport,
            AirportPiece::Tower as u8
        ));
        assert!(!is_airport_tower_tile(
            TileKind::Airport,
            AirportPiece::Apron as u8
        ));

        let dirty = step_airport_tiles(&mut map, 3, &[station.clone()]);
        assert_eq!(dirty, vec![pos]);
        assert_eq!(map.get(pos).unwrap().m7, 1);
        assert_eq!(airport_radar_frame(1), 1);

        let _ = step_airport_tiles(&mut map, 6, &[station.clone()]);
        let _ = step_airport_tiles(&mut map, 9, &[station]);
        assert_eq!(map.get(pos).unwrap().m7, 3);

        assert!(step_airport_tiles(&mut map, 4, &[]).is_empty());
    }

    #[test]
    fn radar_ignores_map_tiles_not_listed_in_stations() {
        let mut map = Map::new_flat(8, 8, 1);
        let pos = TileCoord::new(3, 3);
        let mut tile = map.get(pos).unwrap();
        tile.kind = TileKind::Airport;
        tile.m5 = AirportPiece::Tower as u8;
        map.set_tile(pos, tile).unwrap();
        assert!(step_airport_tiles(&mut map, 3, &[]).is_empty());
        assert_eq!(map.get(pos).unwrap().m7, 0);
    }

    #[test]
    fn imported_airport_animates_only_the_explicit_station_gfx_variants() {
        let mut map = Map::new_flat(8, 8, 1);
        let radar = TileCoord::new(1, 1);
        let flag = TileCoord::new(2, 1);
        let static_tower = TileCoord::new(3, 1);

        for (pos, gfx) in [(radar, 51), (flag, 39), (static_tower, 47)] {
            let mut tile = map.get(pos).unwrap();
            tile.kind = TileKind::Airport;
            tile.m5 = gfx;
            map.set_tile(pos, tile).unwrap();
        }

        let mut station = Station::new_with_kind(radar, StopKind::RailStation);
        station.ottd_station_id = Some(77);
        station.airport_tiles = vec![radar, flag, static_tower];

        let dirty = step_airport_tiles(&mut map, 3, &[station.clone()]);
        assert_eq!(dirty, vec![radar, flag]);
        assert_eq!(map.get(radar).unwrap().m7, 1);
        assert_eq!(map.get(flag).unwrap().m7, 1);
        assert_eq!(map.get(static_tower).unwrap().m7, 0);

        for tick in [6, 9, 12] {
            let _ = step_airport_tiles(&mut map, tick, &[station.clone()]);
        }
        assert_eq!(map.get(flag).unwrap().m7, 0, "flag has four frames");
        assert_eq!(map.get(radar).unwrap().m7, 4, "radar has twelve frames");
    }

    #[test]
    #[allow(clippy::unwrap_used)] // Fixtures y JSON son locales a esta regresión.
    fn newgrf_road_stop_animation_runs_trigger_speed_next_frame_and_roundtrips() {
        let coord = TileCoord::new(1, 1);
        let mut map = Map::new_flat(4, 4, 0);
        let mut tile = map.get(coord).unwrap();
        tile.kind = TileKind::Station;
        tile.m5 = crate::RSV_BAY_NW;
        map.set_tile(coord, tile).unwrap();

        let mut station = Station::new_with_kind(coord, StopKind::BusStop);
        station.road_stop_spec = Some(7);
        let mut stations = vec![station];
        let catalog = vec![animated_road_stop_spec()];

        let dirty = step_newgrf_road_stop_tiles(
            &map,
            1,
            &mut stations,
            &catalog,
            &[(coord, map.get(coord).unwrap())],
        );
        assert_eq!(dirty, vec![coord]);
        assert!(stations[0].road_stop_animation_active);
        // CB142 = 2: en el tick 1 todavía no se consulta el frame siguiente.
        assert_eq!(stations[0].road_stop_animation_frame, 0);

        let dirty = step_newgrf_road_stop_tiles(&map, 4, &mut stations, &catalog, &[]);
        assert_eq!(dirty, vec![coord]);
        // CB141 = 3: el frame no es el avance lineal de fallback.
        assert_eq!(stations[0].road_stop_animation_frame, 3);

        let mut state = crate::GameState::from_map(map);
        state.stations = stations;
        let json = state.save_json().unwrap();
        let loaded = crate::GameState::load_json(&json).unwrap();
        assert_eq!(loaded.stations[0].road_stop_animation_frame, 3);
        assert!(loaded.stations[0].road_stop_animation_active);
    }
}
