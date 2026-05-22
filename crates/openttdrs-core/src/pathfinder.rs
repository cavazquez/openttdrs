use std::collections::{HashMap, VecDeque};

use crate::map::{Map, TileCoord, TileKind};
use crate::vehicle::VehicleKind;

fn is_any_transport_tile(kind: TileKind) -> bool {
    matches!(
        kind,
        TileKind::Road
            | TileKind::Rail
            | TileKind::RoadDepot
            | TileKind::RailDepot
            | TileKind::RoadTunnel
            | TileKind::RailTunnel
            | TileKind::RoadBridge
            | TileKind::RailBridge
    )
}

fn is_network_tile(map: &Map, c: TileCoord, kind: TileKind, network: PathNetwork) -> bool {
    match network {
        PathNetwork::Road => is_road_network_tile(kind) || is_road_stop_station_tile(map, c),
        PathNetwork::Rail => matches!(
            kind,
            TileKind::Rail | TileKind::RailDepot | TileKind::RailTunnel | TileKind::RailBridge
        ),
    }
}

/// Red de transporte para BFS: carretera (bus/camión) o vía (tren).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathNetwork {
    Road,
    Rail,
}

#[must_use]
pub const fn path_network_for_vehicle(kind: VehicleKind) -> PathNetwork {
    match kind {
        VehicleKind::Train => PathNetwork::Rail,
        VehicleKind::Truck | VehicleKind::Bus => PathNetwork::Road,
    }
}

/// Misma condición que los tiles intermedios del pathfinder (carretera/vía/túnel/puente).
#[must_use]
pub fn tile_is_path_traversable(kind: TileKind) -> bool {
    is_any_transport_tile(kind)
}

/// Offset de tesela vecina hacia donde apunta la entrada (`OpenTTD` `TileOffsByDiagDir`).
#[must_use]
pub const fn diag_dir_offset(dir: u8) -> (i32, i32) {
    const OFFSETS: [(i32, i32); 4] = [
        (-1, 0), // DIAGDIR_NE
        (0, 1),  // DIAGDIR_SE
        (1, 0),  // DIAGDIR_SW
        (0, -1), // DIAGDIR_NW
    ];
    OFFSETS[dir as usize & 3]
}

/// Algún vecino ortogonal tiene red de transporte (la estación debe poder «engancharse»).
#[must_use]
#[inline]
fn is_road_network_tile(kind: TileKind) -> bool {
    matches!(
        kind,
        TileKind::Road | TileKind::RoadDepot | TileKind::RoadTunnel | TileKind::RoadBridge
    )
}

/// Parada bus/camión con boca a carretera (`m3` = road bits de acceso).
#[must_use]
fn is_road_stop_station_tile(map: &Map, c: TileCoord) -> bool {
    let Some(t) = map.get(c) else {
        return false;
    };
    if t.kind != TileKind::Station {
        return false;
    }
    matches!((t.m6 >> 3) & 0x0F, 2 | 3) && (t.m3 & 0x0F) != 0
}

/// Algún vecino ortogonal tiene red de transporte (la estación debe poder «engancharse»).
#[must_use]
#[inline]
fn is_rail_network_tile(kind: TileKind) -> bool {
    matches!(
        kind,
        TileKind::Rail | TileKind::RailDepot | TileKind::RailBridge | TileKind::RailTunnel
    )
}

/// Tesela adyacente a vía férrea (para estaciones de tren).
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

/// Tesela donde puede construirse la estación (hierba o bosque limpiable; no sobre la red).
#[must_use]
pub fn station_site_tile_allows_build(kind: TileKind) -> bool {
    matches!(kind, TileKind::Grass | TileKind::Forest)
}

/// Como `OpenTTD` `LandscapeClear` antes de parada: bosque → hierba con coste extra.
#[must_use]
pub fn station_site_tile_needs_clear(kind: TileKind) -> bool {
    matches!(kind, TileKind::Forest)
}

/// La entrada de la parada (carretera) mira hacia una tesela con red de carretera.
#[must_use]
pub fn station_entrance_faces_road(map: &Map, c: TileCoord, dir: u8) -> bool {
    let (dx, dy) = diag_dir_offset(dir);
    let n = TileCoord::new(c.x + dx, c.y + dy);
    map.get_kind(n).is_some_and(is_road_network_tile)
}

/// La entrada de la estación de tren mira hacia vía férrea (o estación compatible).
#[must_use]
pub fn station_entrance_faces_rail(map: &Map, c: TileCoord, dir: u8) -> bool {
    let (dx, dy) = diag_dir_offset(dir);
    let n = TileCoord::new(c.x + dx, c.y + dy);
    map.get_kind(n)
        .is_some_and(|k| is_rail_network_tile(k) || k == TileKind::Station)
}

/// Encuentra el camino más corto entre `from` y `to` usando BFS sobre una sola red (`Road…` o `Rail…`).
///
/// Los tiles `from` y `to` pueden ser de cualquier tipo (industria, estación, etc.);
/// los tiles **intermedios** deben pertenecer a la red elegida.
///
/// Devuelve `Some(path)` donde `path` es la secuencia de teselas desde la primera adyacente
/// a `from` hasta `to` inclusive. Si `from == to` devuelve `Some(vec![])`.
/// Devuelve `None` si no existe camino.
#[must_use]
#[allow(clippy::cast_possible_wrap)]
pub fn find_path(
    map: &Map,
    from: TileCoord,
    to: TileCoord,
    network: PathNetwork,
) -> Option<Vec<TileCoord>> {
    if from == to {
        return Some(vec![]);
    }

    let (mw, mh) = map.dimensions();
    // parent[tile] = tile que lo descubrió
    let mut parent: HashMap<TileCoord, TileCoord> = HashMap::new();
    let mut queue: VecDeque<TileCoord> = VecDeque::new();

    parent.insert(from, from);
    queue.push_back(from);

    let dirs = [(-1_i32, 0_i32), (1, 0), (0, -1), (0, 1)];

    while let Some(cur) = queue.pop_front() {
        if cur == to {
            return Some(reconstruct(from, to, &parent));
        }
        for (dx, dy) in dirs {
            let next = TileCoord::new(cur.x + dx, cur.y + dy);
            if parent.contains_key(&next) {
                continue;
            }
            if next.x < 0 || next.y < 0 || next.x >= mw as i32 || next.y >= mh as i32 {
                continue;
            }
            let next_kind = map.get_kind(next).unwrap_or(TileKind::Grass);
            let cur_kind = map.get_kind(cur).unwrap_or(TileKind::Grass);
            // Un tile es alcanzable si:
            // - Pertenece a la red y es transitable, O
            // - Es el destino Y el tile actual (cur) ya está en la red (último paso desde red).
            let reachable = if is_network_tile(map, next, next_kind, network) {
                true
            } else if next == to {
                is_network_tile(map, cur, cur_kind, network)
            } else {
                false
            };
            if reachable {
                parent.insert(next, cur);
                queue.push_back(next);
            }
        }
    }
    None
}

fn reconstruct(
    from: TileCoord,
    to: TileCoord,
    parent: &HashMap<TileCoord, TileCoord>,
) -> Vec<TileCoord> {
    let mut path = Vec::new();
    let mut cur = to;
    while cur != from {
        path.push(cur);
        cur = parent[&cur];
    }
    path.reverse();
    path
}
