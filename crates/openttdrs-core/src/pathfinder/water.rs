//! A* sobre teselas de agua.

use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};

use crate::bridge_spec::{bridge_middle_length, water_aqueduct_other_end};
use crate::engine::EngineDef;
use crate::map::{
    Map, TileCoord, TileKind, WaterClass, diag_dir_offset, effective_water_class_for_ship,
};
use crate::rail_pbs::YAPF_TILE_LENGTH;
use crate::ship_movement::{
    ship_subcoord, ship_track_exit_diagdir, ship_trackdir, water_tile_is_lock,
    water_tiles_connected,
};

use super::astar::{AstarNode, manhattan, reconstruct};
use super::network::{PathNetwork, is_network_tile};

/// Penalización vanilla de YAPF por una curva de 45 grados (`1 * tile`).
pub const SHIP_CURVE45_PENALTY: u32 = YAPF_TILE_LENGTH;
/// Penalización vanilla de YAPF por una curva de 90 grados (`6 * tile`).
pub const SHIP_CURVE90_PENALTY: u32 = 6 * YAPF_TILE_LENGTH;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct ShipNodeKey {
    tile: TileCoord,
    trackdir: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ShipAstarNode {
    est_total: u32,
    key: ShipNodeKey,
}

impl Ord for ShipAstarNode {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .est_total
            .cmp(&self.est_total)
            .then_with(|| other.key.tile.x.cmp(&self.key.tile.x))
            .then_with(|| other.key.tile.y.cmp(&self.key.tile.y))
            .then_with(|| other.key.trackdir.cmp(&self.key.trackdir))
    }
}

impl PartialOrd for ShipAstarNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Propiedades del coste naval que YAPF obtiene de `ShipVehicleInfo`.
///
/// Las dos propiedades son reducciones (`0` = sin reducción), no fracciones
/// multiplicativas. El coste de un tile usa la misma conversión que
/// `yapf_ship.cpp`: `base + base * reduction / (256 - reduction)`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ShipPathCost {
    pub ocean_speed_frac: u8,
    pub canal_speed_frac: u8,
    /// Velocidad máxima estática del `ShipVehicleInfo`, usada por la
    /// penalización del centro de una esclusa.
    pub max_speed: u16,
}

impl ShipPathCost {
    #[must_use]
    pub const fn from_engine(engine: &EngineDef) -> Self {
        Self {
            ocean_speed_frac: engine.ocean_speed_frac,
            canal_speed_frac: engine.canal_speed_frac,
            max_speed: engine.max_speed,
        }
    }

    #[must_use]
    pub fn tile_cost(self, water_class: Option<WaterClass>) -> u32 {
        self.track_tile_cost(YAPF_TILE_LENGTH, water_class, 0)
    }

    #[must_use]
    pub fn minimum_tile_cost(self) -> u32 {
        self.tile_cost(Some(WaterClass::Sea))
            .min(self.tile_cost(Some(WaterClass::Canal)))
    }

    /// Coste acumulado de una secuencia de tiles usando la misma unidad que
    /// `YapfShip::PfCalcCost` para su componente agua/clase.
    #[must_use]
    pub fn path_cost(self, map: &Map, from: TileCoord, path: &[TileCoord]) -> u32 {
        let Some(&first) = path.first() else {
            return 0;
        };
        let Some((first_entry, _)) = water_step_geometry(map, from, first) else {
            return self.fallback_path_cost(map, from, path);
        };
        let Some(mut previous_trackdir) = straight_trackdir_for_exit(first_entry) else {
            return self.fallback_path_cost(map, from, path);
        };

        let mut total: u32 = 0;
        let mut previous_tile = from;
        for (index, &current) in path.iter().enumerate() {
            let Some((entry, skipped)) = water_step_geometry(map, previous_tile, current) else {
                return self.fallback_path_cost(map, from, path);
            };
            if trackdir_exit_diagdir(previous_trackdir) != Some(entry) {
                return self.fallback_path_cost(map, from, path);
            }
            let node_trackdir = path
                .get(index + 1)
                .and_then(|&next| {
                    water_step_geometry(map, current, next)
                        .and_then(|(exit, _)| trackdir_for_entry_exit(entry, exit))
                })
                .or_else(|| straight_trackdir_for_exit(entry))
                .unwrap_or(previous_trackdir);
            total = total.saturating_add(self.node_cost(
                map,
                previous_tile,
                current,
                previous_trackdir,
                node_trackdir,
                skipped,
            ));
            previous_trackdir = node_trackdir;
            previous_tile = current;
        }
        total
    }

    #[must_use]
    fn fallback_path_cost(self, map: &Map, from: TileCoord, path: &[TileCoord]) -> u32 {
        let mut total: u32 = 0;
        let mut current = from;
        for &next in path {
            let step = if water_aqueduct_other_end(map, current) == Some(next) {
                self.aqueduct_step_cost(map, current, next)
            } else {
                self.step_cost(map, current, next)
            };
            total = total.saturating_add(step);
            current = next;
        }
        total
    }

    #[must_use]
    fn step_cost(self, map: &Map, current: TileCoord, next: TileCoord) -> u32 {
        self.tile_cost(ship_water_class(map, current, next))
            .saturating_add(self.lock_penalty(map, next))
    }

    #[must_use]
    fn aqueduct_step_cost(self, map: &Map, current: TileCoord, next: TileCoord) -> u32 {
        let span_tiles = u32::from(bridge_middle_length(current, next));
        self.step_cost(map, current, next).saturating_add(
            self.tile_cost(Some(WaterClass::Canal))
                .saturating_mul(span_tiles),
        )
    }

    #[must_use]
    fn node_cost(
        self,
        map: &Map,
        previous: TileCoord,
        current: TileCoord,
        previous_trackdir: u8,
        trackdir: u8,
        skipped: u32,
    ) -> u32 {
        let base = if matches!(trackdir & 0x07, 0 | 1) {
            YAPF_TILE_LENGTH
        } else {
            crate::rail_pbs::YAPF_TILE_CORNER_LENGTH
        };
        self.track_tile_cost(base, ship_water_class(map, previous, current), skipped)
            .saturating_add(Self::curve_penalty(previous_trackdir, trackdir))
            .saturating_add(preferred_direction_penalty(current, trackdir))
            .saturating_add(self.lock_penalty(map, current))
    }

    #[must_use]
    fn track_tile_cost(self, base: u32, water_class: Option<WaterClass>, skipped: u32) -> u32 {
        let reduction = self.speed_reduction(water_class);
        let speed_penalty = base
            .saturating_mul((1_u32.saturating_add(skipped)).saturating_mul(reduction))
            / (256 - reduction);
        base.saturating_add(YAPF_TILE_LENGTH.saturating_mul(skipped))
            .saturating_add(speed_penalty)
    }

    #[must_use]
    fn speed_reduction(self, water_class: Option<WaterClass>) -> u32 {
        match water_class {
            Some(WaterClass::Sea) => u32::from(self.ocean_speed_frac),
            // `GetEffectiveWaterClass` treats every non-Sea class as canal
            // for this YAPF property, including River and Invalid/fallback.
            _ => u32::from(self.canal_speed_frac),
        }
    }

    #[must_use]
    fn curve_penalty(previous: u8, current: u8) -> u32 {
        if trackdir_crosses_trackdir(previous, current) {
            SHIP_CURVE90_PENALTY
        } else if next_trackdir(previous) != Some(current) {
            SHIP_CURVE45_PENALTY
        } else {
            0
        }
    }

    /// Penalización de `YapfShip` para el centro de una esclusa.
    #[must_use]
    fn lock_penalty(self, map: &Map, tile: TileCoord) -> u32 {
        let Some(raw) = map.get(tile) else {
            return 0;
        };
        // `LockPart::Middle == 0`; lower/upper no detienen el barco en el
        // sentido del coste YAPF.
        if raw.kind != TileKind::Water
            || !water_tile_is_lock(map, tile)
            || (raw.m5 >> 2) & 0x03 != 0
        {
            return 0;
        }
        let reduction = u32::from(self.canal_speed_frac);
        let canal_speed = u32::from(self.max_speed).saturating_mul(256 - reduction) / 256;
        // `TILE_HEIGHT` es 8 en OpenTTD; el barco queda detenido al cruzar
        // un nivel y YAPF lo expresa en unidades de `YAPF_TILE_LENGTH`.
        8 * YAPF_TILE_LENGTH * canal_speed / 128
    }
}

/// A* sobre teselas de agua (vecinos ortogonales conectados).
#[must_use]
#[allow(clippy::cast_possible_wrap)]
pub(super) fn find_water_path(map: &Map, from: TileCoord, to: TileCoord) -> Option<Vec<TileCoord>> {
    find_water_path_with_cost(map, from, to, None)
}

/// A* naval con el coste de clase de agua de `YapfShip`.
#[must_use]
#[allow(clippy::cast_possible_wrap)]
pub(super) fn find_ship_path(
    map: &Map,
    from: TileCoord,
    to: TileCoord,
    cost: ShipPathCost,
) -> Option<Vec<TileCoord>> {
    find_ship_path_with_trackdirs(map, from, to, cost)
}

/// A* naval que conserva el `Trackdir` de cada nodo.
///
/// El camino público sigue exponiendo teselas, pero YAPF calcula el coste del
/// nodo como una función de `(tile, trackdir)`: así una curva no puede perderse
/// al fusionar dos llegadas a la misma coordenada. Los estados de origen usan
/// los cuatro trackdirs rectos posibles porque esta API no recibe la orientación
/// física del barco; el controlador sí resuelve el trackdir concreto al entrar.
#[must_use]
fn find_ship_path_with_trackdirs(
    map: &Map,
    from: TileCoord,
    to: TileCoord,
    cost: ShipPathCost,
) -> Option<Vec<TileCoord>> {
    if from == to {
        return Some(Vec::new());
    }

    let mut g_score: HashMap<ShipNodeKey, u32> = HashMap::new();
    let mut parent: HashMap<ShipNodeKey, Option<ShipNodeKey>> = HashMap::new();
    let mut heap = BinaryHeap::new();

    for trackdir in straight_trackdirs() {
        let Some(exit) = trackdir_exit_diagdir(trackdir) else {
            continue;
        };
        if water_follow_step(map, from, exit, to).is_none() {
            continue;
        }
        let key = ShipNodeKey {
            tile: from,
            trackdir,
        };
        g_score.insert(key, 0);
        parent.insert(key, None);
        heap.push(ShipAstarNode {
            est_total: heuristic(from, to, Some(cost)),
            key,
        });
    }

    while let Some(ShipAstarNode { key, .. }) = heap.pop() {
        if key.tile == to {
            return Some(reconstruct_ship_path(key, &parent, from));
        }
        let cur_g = g_score[&key];
        let Some(exit) = trackdir_exit_diagdir(key.trackdir) else {
            continue;
        };
        let Some((next, skipped)) = water_follow_step(map, key.tile, exit, to) else {
            continue;
        };

        for track in 0..6_u8 {
            let Some(subcoord) = ship_subcoord(exit, track) else {
                continue;
            };
            let next_trackdir = ship_trackdir(track, subcoord.dir);
            if next != to
                && trackdir_exit_diagdir(next_trackdir)
                    .and_then(|next_exit| water_follow_step(map, next, next_exit, to))
                    .is_none()
            {
                continue;
            }

            let next_key = ShipNodeKey {
                tile: next,
                trackdir: next_trackdir,
            };
            let tentative = cur_g.saturating_add(cost.node_cost(
                map,
                key.tile,
                next,
                key.trackdir,
                next_trackdir,
                skipped,
            ));
            if g_score
                .get(&next_key)
                .is_some_and(|&score| tentative >= score)
            {
                continue;
            }
            g_score.insert(next_key, tentative);
            parent.insert(next_key, Some(key));
            heap.push(ShipAstarNode {
                est_total: tentative.saturating_add(heuristic(next, to, Some(cost))),
                key: next_key,
            });
        }
    }
    None
}

#[must_use]
fn find_water_path_with_cost(
    map: &Map,
    from: TileCoord,
    to: TileCoord,
    ship_cost: Option<ShipPathCost>,
) -> Option<Vec<TileCoord>> {
    if let Some(cost) = ship_cost {
        return find_ship_path_with_trackdirs(map, from, to, cost);
    }
    let (mw, mh) = map.dimensions();
    let mut g_score: HashMap<TileCoord, u32> = HashMap::new();
    let mut parent: HashMap<TileCoord, TileCoord> = HashMap::new();
    let mut heap = BinaryHeap::new();

    g_score.insert(from, 0);
    parent.insert(from, from);
    heap.push(AstarNode {
        est_total: heuristic(from, to, ship_cost),
        pos: from,
    });

    let dirs = [(-1_i32, 0_i32), (1, 0), (0, -1), (0, 1)];

    while let Some(AstarNode {
        est_total: _,
        pos: cur,
    }) = heap.pop()
    {
        if cur == to {
            return Some(reconstruct(from, to, &parent));
        }

        let cur_g = g_score[&cur];
        for (dx, dy) in dirs {
            let next = TileCoord::new(cur.x + dx, cur.y + dy);
            if next.x < 0 || next.y < 0 || next.x >= mw.cast_signed() || next.y >= mh.cast_signed()
            {
                continue;
            }
            let next_kind = map.get_kind(next).unwrap_or(TileKind::Grass);
            let cur_kind = map.get_kind(cur).unwrap_or(TileKind::Grass);
            let reachable = if is_network_tile(map, next, next_kind, PathNetwork::Water) {
                water_tiles_connected(map, cur, next)
            } else if next == to {
                is_network_tile(map, cur, cur_kind, PathNetwork::Water)
            } else {
                false
            };
            if !reachable {
                continue;
            }

            let step = ship_cost.map_or(1, |cost| cost.step_cost(map, cur, next));
            let tentative = cur_g.saturating_add(step);
            if g_score.get(&next).is_some_and(|&g| tentative >= g) {
                continue;
            }
            g_score.insert(next, tentative);
            parent.insert(next, cur);
            heap.push(AstarNode {
                est_total: tentative.saturating_add(heuristic(next, to, ship_cost)),
                pos: next,
            });
        }

        // Un acueducto es un wormhole naval: las rampas son las únicas
        // teselas `MP_TUNNELBRIDGE` y el tramo intermedio puede conservar
        // tierra, agua u otra capa. `FollowTrackWater` salta al otro extremo
        // al salir por la dirección de la rampa; no debe recorrer el terreno
        // de abajo como una secuencia de teselas de agua.
        if let Some(other) = water_aqueduct_other_end(map, cur)
            && (other == to
                || map
                    .get_kind(other)
                    .is_some_and(|kind| is_network_tile(map, other, kind, PathNetwork::Water)))
        {
            let step = ship_cost.map_or(1, |cost| cost.aqueduct_step_cost(map, cur, other));
            let tentative = cur_g.saturating_add(step);
            if g_score.get(&other).is_none_or(|&g| tentative < g) {
                g_score.insert(other, tentative);
                parent.insert(other, cur);
                heap.push(AstarNode {
                    est_total: tentative.saturating_add(heuristic(other, to, ship_cost)),
                    pos: other,
                });
            }
        }
    }
    None
}

#[must_use]
const fn straight_trackdirs() -> [u8; 4] {
    [0, 1, 8, 9]
}

#[must_use]
const fn straight_trackdir_for_exit(exit: u8) -> Option<u8> {
    match exit & 0x03 {
        0 => Some(0), // TRACKDIR_X_NE
        1 => Some(1), // TRACKDIR_Y_SE
        2 => Some(8), // TRACKDIR_X_SW
        3 => Some(9), // TRACKDIR_Y_NW
        _ => None,
    }
}

/// `TrackdirToExitdir` reducido a los seis tracks de barcos.
#[must_use]
const fn trackdir_exit_diagdir(trackdir: u8) -> Option<u8> {
    match trackdir {
        0 | 2 | 13 => Some(0),  // NE
        1 | 3 | 5 => Some(1),   // SE
        4 | 8 | 11 => Some(2),  // SW
        9 | 10 | 12 => Some(3), // NW
        _ => None,
    }
}

#[must_use]
const fn next_trackdir(trackdir: u8) -> Option<u8> {
    match trackdir {
        0 => Some(0),
        1 => Some(1),
        2 => Some(3),
        3 => Some(2),
        4 => Some(5),
        5 => Some(4),
        8 => Some(8),
        9 => Some(9),
        10 => Some(11),
        11 => Some(10),
        12 => Some(13),
        13 => Some(12),
        _ => None,
    }
}

#[must_use]
fn trackdir_crosses_trackdir(previous: u8, current: u8) -> bool {
    if current >= 16 {
        return false;
    }
    let mask = match previous & 0x07 {
        0 => (1_u16 << 1) | (1_u16 << 9),
        1 => (1_u16 << 0) | (1_u16 << 8),
        2 | 3 => (1_u16 << 4) | (1_u16 << 5) | (1_u16 << 12) | (1_u16 << 13),
        4 | 5 => (1_u16 << 2) | (1_u16 << 3) | (1_u16 << 10) | (1_u16 << 11),
        _ => 0,
    };
    mask & (1_u16 << current) != 0
}

/// Replica `IsPreferredShipDirection`: alterna los trackdirs por coordenada
/// para separar barcos que recorren la misma red en sentidos opuestos.
#[must_use]
fn is_preferred_ship_direction(tile: TileCoord, trackdir: u8) -> bool {
    let odd_x = (tile.x & 1) != 0;
    let odd_y = (tile.y & 1) != 0;
    match trackdir {
        0 => odd_y,  // TRACKDIR_X_NE
        8 => !odd_y, // TRACKDIR_X_SW
        9 => odd_x,  // TRACKDIR_Y_NW
        1 => !odd_x, // TRACKDIR_Y_SE
        2 | 5 | 11 | 12 => odd_x ^ odd_y,
        3 | 4 | 10 | 13 => !(odd_x ^ odd_y),
        _ => false,
    }
}

#[must_use]
fn preferred_direction_penalty(tile: TileCoord, trackdir: u8) -> u32 {
    u32::from(!is_preferred_ship_direction(tile, trackdir)) * YAPF_TILE_LENGTH
}

#[must_use]
fn trackdir_for_entry_exit(entry: u8, exit: u8) -> Option<u8> {
    (0..6_u8).find_map(|track| {
        let subcoord = ship_subcoord(entry, track)?;
        (ship_track_exit_diagdir(entry, track) == Some(exit))
            .then_some(ship_trackdir(track, subcoord.dir))
    })
}

/// Geometría de un paso del path tile-based: dirección de entrada al segundo
/// tile y cantidad de teselas omitidas por un acueducto.
#[must_use]
fn water_step_geometry(map: &Map, from: TileCoord, to: TileCoord) -> Option<(u8, u32)> {
    if water_aqueduct_other_end(map, from) == Some(to) {
        let ramp = map.get(from)?;
        return Some((ramp.m5 & 0x03, u32::from(bridge_middle_length(from, to))));
    }
    match (to.x - from.x, to.y - from.y) {
        (-1, 0) => Some((0, 0)),
        (0, 1) => Some((1, 0)),
        (1, 0) => Some((2, 0)),
        (0, -1) => Some((3, 0)),
        _ => None,
    }
}

/// Sigue un `Trackdir` y resuelve el salto de un acueducto cuando su salida
/// coincide con el lado interior de la rampa.
#[must_use]
fn water_follow_step(
    map: &Map,
    current: TileCoord,
    exit: u8,
    destination: TileCoord,
) -> Option<(TileCoord, u32)> {
    if let Some(other) = water_aqueduct_other_end(map, current) {
        let ramp = map.get(current)?;
        if ramp.m5 & 0x03 == exit {
            let reachable = other == destination
                || map
                    .get_kind(other)
                    .is_some_and(|kind| is_network_tile(map, other, kind, PathNetwork::Water));
            return reachable.then_some((other, u32::from(bridge_middle_length(current, other))));
        }
    }

    let (dx, dy) = diag_dir_offset(exit);
    let next = TileCoord::new(current.x + dx, current.y + dy);
    let next_kind = map.get_kind(next)?;
    let current_kind = map.get_kind(current)?;
    if is_network_tile(map, next, next_kind, PathNetwork::Water) {
        water_tiles_connected(map, current, next).then_some((next, 0))
    } else if next == destination && is_network_tile(map, current, current_kind, PathNetwork::Water)
    {
        Some((next, 0))
    } else {
        None
    }
}

#[must_use]
fn reconstruct_ship_path(
    goal: ShipNodeKey,
    parent: &HashMap<ShipNodeKey, Option<ShipNodeKey>>,
    from: TileCoord,
) -> Vec<TileCoord> {
    let mut path = Vec::new();
    let mut current = goal;
    while current.tile != from {
        path.push(current.tile);
        let Some(Some(previous)) = parent.get(&current) else {
            break;
        };
        current = *previous;
    }
    path.reverse();
    path
}

#[must_use]
fn ship_water_class(map: &Map, current: TileCoord, next: TileCoord) -> Option<WaterClass> {
    map.get(next)
        .and_then(effective_water_class_for_ship)
        .or_else(|| map.get(current).and_then(effective_water_class_for_ship))
}

#[must_use]
fn heuristic(from: TileCoord, to: TileCoord, ship_cost: Option<ShipPathCost>) -> u32 {
    let distance = manhattan(from, to);
    ship_cost.map_or(distance, |cost| {
        distance.saturating_mul(cost.minimum_tile_cost())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preferred_ship_direction_matches_native_parity_table() {
        let even = TileCoord::new(0, 0);
        assert!(!is_preferred_ship_direction(even, 0));
        assert!(is_preferred_ship_direction(even, 8));
        assert!(!is_preferred_ship_direction(even, 9));
        assert!(is_preferred_ship_direction(even, 1));
        assert!(!is_preferred_ship_direction(even, 2));
        assert!(is_preferred_ship_direction(even, 3));
        assert!(is_preferred_ship_direction(even, 4));
        assert!(!is_preferred_ship_direction(even, 5));
        assert!(is_preferred_ship_direction(even, 10));
        assert!(!is_preferred_ship_direction(even, 11));
        assert!(!is_preferred_ship_direction(even, 12));
        assert!(is_preferred_ship_direction(even, 13));

        let odd_y = TileCoord::new(0, 1);
        assert!(is_preferred_ship_direction(odd_y, 0));
        assert!(!is_preferred_ship_direction(odd_y, 8));
    }
}
