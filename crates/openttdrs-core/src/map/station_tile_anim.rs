//! Animación de teselas de estación / aeropuerto (`AnimateTile_Airport`).

use crate::airport::{AirportPiece, airport_station_gfx_animation_frames};
use crate::map::{Map, TileCoord, TileKind};
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
}
