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

fn is_network_tile(kind: TileKind, network: PathNetwork) -> bool {
    match network {
        PathNetwork::Road => matches!(
            kind,
            TileKind::Road | TileKind::RoadDepot | TileKind::RoadTunnel | TileKind::RoadBridge
        ),
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

/// Algún vecino ortogonal tiene red de transporte (la estación debe poder «engancharse»).
#[must_use]
pub fn station_site_adjacent_to_transport(map: &Map, c: TileCoord) -> bool {
    for (dx, dy) in [(-1_i32, 0_i32), (1, 0), (0, -1), (0, 1)] {
        let n = TileCoord::new(c.x + dx, c.y + dy);
        let k = map.get_kind(n).unwrap_or(TileKind::Grass);
        if is_any_transport_tile(k) {
            return true;
        }
    }
    false
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
            let reachable = if is_network_tile(next_kind, network) {
                true
            } else if next == to {
                is_network_tile(cur_kind, network)
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
