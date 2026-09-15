//! A* sobre teselas de agua.

use std::collections::{BinaryHeap, HashMap};

use crate::engine::EngineDef;
use crate::map::{Map, TileCoord, TileKind, WaterClass, effective_water_class_for_ship};
use crate::rail_pbs::YAPF_TILE_LENGTH;
use crate::ship_movement::{water_tile_is_lock, water_tiles_connected};

use super::astar::{AstarNode, manhattan, reconstruct};
use super::network::{PathNetwork, is_network_tile};

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
        let reduction = match water_class {
            Some(WaterClass::Sea) => self.ocean_speed_frac,
            // `GetEffectiveWaterClass` treats every non-Sea class as canal
            // for this YAPF property, including River and Invalid/fallback.
            _ => self.canal_speed_frac,
        };
        let reduction = u32::from(reduction);
        YAPF_TILE_LENGTH + YAPF_TILE_LENGTH.saturating_mul(reduction) / (256 - reduction)
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
        let mut total: u32 = 0;
        let mut current = from;
        for &next in path {
            total = total.saturating_add(self.step_cost(map, current, next));
            current = next;
        }
        total
    }

    #[must_use]
    fn step_cost(self, map: &Map, current: TileCoord, next: TileCoord) -> u32 {
        self.tile_cost(ship_water_class(map, current, next))
            .saturating_add(self.lock_penalty(map, next))
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
    find_water_path_with_cost(map, from, to, Some(cost))
}

#[must_use]
fn find_water_path_with_cost(
    map: &Map,
    from: TileCoord,
    to: TileCoord,
    ship_cost: Option<ShipPathCost>,
) -> Option<Vec<TileCoord>> {
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
    }
    None
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
