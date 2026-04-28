use std::collections::{HashMap, VecDeque};

use crate::map::{Map, TileCoord, TileKind};

fn is_traversable(kind: TileKind) -> bool {
    matches!(kind, TileKind::Road | TileKind::Rail)
}

/// Encuentra el camino más corto entre `from` y `to` usando BFS sobre teselas Road/Rail.
///
/// Los tiles `from` y `to` pueden ser de cualquier tipo (industria, estación, etc.);
/// los tiles **intermedios** deben ser `Road` o `Rail`.
///
/// Devuelve `Some(path)` donde `path` es la secuencia de teselas desde la primera adyacente
/// a `from` hasta `to` inclusive. Si `from == to` devuelve `Some(vec![])`.
/// Devuelve `None` si no existe camino.
#[must_use]
pub fn find_path(map: &Map, from: TileCoord, to: TileCoord) -> Option<Vec<TileCoord>> {
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
            let cur_kind  = map.get_kind(cur).unwrap_or(TileKind::Grass);
            // Un tile es alcanzable si:
            // - Es Road/Rail (tile intermedio traversable), O
            // - Es el destino Y el tile actual (cur) ya es Road/Rail
            //   (el último paso viene desde la carretera, no desde un tile no-road).
            let reachable = if is_traversable(next_kind) {
                true
            } else if next == to {
                is_traversable(cur_kind)
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

fn reconstruct(from: TileCoord, to: TileCoord, parent: &HashMap<TileCoord, TileCoord>) -> Vec<TileCoord> {
    let mut path = Vec::new();
    let mut cur = to;
    while cur != from {
        path.push(cur);
        cur = parent[&cur];
    }
    path.reverse();
    path
}
