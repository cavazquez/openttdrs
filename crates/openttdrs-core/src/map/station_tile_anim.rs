//! Animación de teselas de estación / aeropuerto (`AnimateTile_Airport`).

use crate::airport::AirportPiece;
use crate::map::{Map, TileCoord, TileKind};

/// Frames del radar vanilla (`SPR_AIRPORT_RADAR_1` … `_12`).
pub const AIRPORT_RADAR_FRAMES: u8 = 12;

/// Avanza `m7` en torres de aeropuerto; devuelve teselas dirty.
pub fn step_airport_tiles(map: &mut Map, tick: u64) -> Vec<TileCoord> {
    // Un frame cada 3 ticks ≈ ritmo visual cercano a OpenTTD.
    if !tick.is_multiple_of(3) {
        return Vec::new();
    }
    let (w, h) = map.dimensions();
    let mut dirty = Vec::new();
    for y in 0..h {
        for x in 0..w {
            let pos = TileCoord::new(x.cast_signed(), y.cast_signed());
            let Some(mut tile) = map.get(pos) else {
                continue;
            };
            if !is_airport_tower_tile(tile.kind, tile.m5) {
                continue;
            }
            tile.m7 = tile.m7.wrapping_add(1) % AIRPORT_RADAR_FRAMES;
            let _ = map.set_tile(pos, tile);
            dirty.push(pos);
        }
    }
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

        assert!(is_airport_tower_tile(
            TileKind::Airport,
            AirportPiece::Tower as u8
        ));
        assert!(!is_airport_tower_tile(
            TileKind::Airport,
            AirportPiece::Apron as u8
        ));

        let dirty = step_airport_tiles(&mut map, 3);
        assert_eq!(dirty, vec![pos]);
        assert_eq!(map.get(pos).unwrap().m7, 1);
        assert_eq!(airport_radar_frame(1), 1);

        let _ = step_airport_tiles(&mut map, 6);
        let _ = step_airport_tiles(&mut map, 9);
        assert_eq!(map.get(pos).unwrap().m7, 3);

        assert!(step_airport_tiles(&mut map, 4).is_empty());
    }
}
