//! A* de **construcción** ferroviaria: teselas buildables (no red ya tendida).
//!
//! Usado por la IA TransCargo (#184). El pathfinder YAPF/`find_path` Rail exige
//! vía existente; aquí el grafo es hierba/bosque/vía reutilizable.

use std::collections::{BinaryHeap, HashMap};

use crate::map::{Map, TileCoord, TileKind};

use super::astar::{AstarNode, manhattan, reconstruct};

const COST_GRASS_OR_RAIL: u32 = 10;
const COST_FOREST: u32 = 40;

/// ¿Se puede tender vía por esta tesela (o reutilizar vía)?
#[must_use]
pub fn tile_allows_rail_build(map: &Map, c: TileCoord, is_endpoint: bool) -> bool {
    match map.get_kind(c).unwrap_or(TileKind::Void) {
        TileKind::Grass | TileKind::Forest | TileKind::Rail => true,
        TileKind::Station | TileKind::RailDepot if is_endpoint => true,
        _ => false,
    }
}

fn step_build_cost(map: &Map, c: TileCoord) -> u32 {
    match map.get_kind(c).unwrap_or(TileKind::Void) {
        TileKind::Forest => COST_FOREST,
        TileKind::Grass | TileKind::Rail | TileKind::Station | TileKind::RailDepot => {
            COST_GRASS_OR_RAIL
        }
        _ => u32::MAX / 4,
    }
}

/// Camino de construcción de `from` a `to` (ambos inclusive).
///
/// Prefiere hierba/vía sobre bosque; **no** atraviesa agua, casas, industrias, etc.
/// Si no hay camino, `None` (la IA puede caer a L Manhattan).
#[must_use]
#[allow(clippy::cast_possible_wrap)]
pub fn find_rail_build_path(map: &Map, from: TileCoord, to: TileCoord) -> Option<Vec<TileCoord>> {
    if from == to {
        return Some(vec![from]);
    }
    if !tile_allows_rail_build(map, from, true) || !tile_allows_rail_build(map, to, true) {
        return None;
    }

    let (mw, mh) = map.dimensions();
    let mut g_score: HashMap<TileCoord, u32> = HashMap::new();
    let mut parent: HashMap<TileCoord, TileCoord> = HashMap::new();
    let mut heap = BinaryHeap::new();

    g_score.insert(from, 0);
    parent.insert(from, from);
    heap.push(AstarNode {
        est_total: manhattan(from, to) * COST_GRASS_OR_RAIL,
        pos: from,
    });

    let dirs = [(-1_i32, 0_i32), (1, 0), (0, -1), (0, 1)];

    while let Some(AstarNode { pos: cur, .. }) = heap.pop() {
        if cur == to {
            let mut path = reconstruct(from, to, &parent);
            path.insert(0, from);
            return Some(path);
        }

        let cur_g = *g_score.get(&cur)?;
        for (dx, dy) in dirs {
            let next = TileCoord::new(cur.x + dx, cur.y + dy);
            if next.x < 0 || next.y < 0 || next.x >= mw as i32 || next.y >= mh as i32 {
                continue;
            }
            let is_end = next == to;
            if !tile_allows_rail_build(map, next, is_end) {
                continue;
            }
            let step = step_build_cost(map, next);
            let tentative = cur_g.saturating_add(step);
            if g_score.get(&next).is_some_and(|&g| tentative >= g) {
                continue;
            }
            g_score.insert(next, tentative);
            parent.insert(next, cur);
            heap.push(AstarNode {
                est_total: tentative + manhattan(next, to) * COST_GRASS_OR_RAIL,
                pos: next,
            });
        }
    }
    None
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::map::TileKind;

    #[test]
    fn build_path_goes_around_water() {
        let mut map = Map::new_flat(12, 8, 1);
        // Bloque de agua en el eje X entre (2,3) y (10,3).
        for x in 4..8 {
            map.set_kind(TileCoord::new(x, 3), TileKind::Water).unwrap();
        }
        let from = TileCoord::new(2, 3);
        let to = TileCoord::new(10, 3);
        let path = find_rail_build_path(&map, from, to).expect("debe rodear el agua");
        assert_eq!(path.first().copied(), Some(from));
        assert_eq!(path.last().copied(), Some(to));
        assert!(
            path.iter().all(|c| map.get_kind(*c) != Some(TileKind::Water)),
            "el path no debe pisar agua"
        );
        assert!(
            path.iter().any(|c| c.y != 3),
            "debe desviarse en Y para rodear"
        );
    }

    #[test]
    fn build_path_prefers_grass_over_forest_detour_when_equal() {
        let mut map = Map::new_flat(10, 6, 1);
        // Pasillo norte hierba; sur bosque en la línea directa.
        for x in 3..7 {
            map.set_kind(TileCoord::new(x, 2), TileKind::Forest).unwrap();
        }
        let from = TileCoord::new(2, 2);
        let to = TileCoord::new(8, 2);
        let path = find_rail_build_path(&map, from, to).expect("path");
        let forest_steps = path
            .iter()
            .filter(|c| map.get_kind(**c) == Some(TileKind::Forest))
            .count();
        // Puede atravesar bosque si es más corto, pero con coste alto suele rodear.
        assert!(
            forest_steps <= 2 || path.iter().any(|c| c.y != 2),
            "debería rodear o cruzar poco bosque; forest_steps={forest_steps} path={path:?}"
        );
    }
}
