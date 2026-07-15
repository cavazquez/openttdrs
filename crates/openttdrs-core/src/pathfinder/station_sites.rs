use crate::map::{Map, TileCoord, TileKind};

use super::{diag_dir_offset, is_rail_network_tile, is_road_network_tile};

#[must_use]
pub fn station_site_adjacent_to_rail(map: &Map, c: TileCoord) -> bool {
    for (dx, dy) in [(-1_i32, 0_i32), (1, 0), (0, -1), (0, 1)] {
        let n = TileCoord::new(c.x + dx, c.y + dy);
        if map
            .get_kind(n)
            .is_some_and(|k| is_rail_network_tile(k) || k == TileKind::Station)
        {
            return true;
        }
    }
    false
}

#[must_use]
pub fn station_site_adjacent_to_transport(map: &Map, c: TileCoord) -> bool {
    for (dx, dy) in [(-1_i32, 0_i32), (1, 0), (0, -1), (0, 1)] {
        let n = TileCoord::new(c.x + dx, c.y + dy);
        let k = map.get_kind(n).unwrap_or(TileKind::Grass);
        if is_road_network_tile(k) {
            return true;
        }
    }
    false
}

#[must_use]
pub fn station_site_tile_allows_build(kind: TileKind) -> bool {
    matches!(kind, TileKind::Grass | TileKind::Forest)
}

#[must_use]
pub fn station_site_tile_needs_clear(kind: TileKind) -> bool {
    matches!(kind, TileKind::Forest)
}

#[must_use]
pub fn station_entrance_faces_road(map: &Map, c: TileCoord, dir: u8) -> bool {
    let (dx, dy) = diag_dir_offset(dir);
    let n = TileCoord::new(c.x + dx, c.y + dy);
    map.get_kind(n).is_some_and(is_road_network_tile)
}

#[must_use]
pub fn station_entrance_faces_rail(map: &Map, c: TileCoord, dir: u8) -> bool {
    let (dx, dy) = diag_dir_offset(dir);
    let n = TileCoord::new(c.x + dx, c.y + dy);
    map.get_kind(n)
        .is_some_and(|k| is_rail_network_tile(k) || k == TileKind::Station)
}
