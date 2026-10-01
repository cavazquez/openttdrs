use std::collections::{HashMap, VecDeque};

use crate::map::{Map, TileCoord};

use super::{PathNetwork, TunnelWormholes, water::ShipPathCost};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct RouteRequest {
    pub from: TileCoord,
    pub to: TileCoord,
    pub network: PathNetwork,
    pub ship_cost: Option<ShipPathCost>,
    pub origin_trackdir_mask: Option<u16>,
}

impl RouteRequest {
    fn key(self) -> PathCacheKey {
        cache_key(
            self.from,
            self.to,
            self.network,
            self.ship_cost,
            self.origin_trackdir_mask,
        )
    }
}

pub(crate) enum CachedRoute {
    Miss,
    Hit(Option<Vec<TileCoord>>),
}

#[derive(Debug, Default, Clone, Copy)]
pub struct PathCacheStats {
    pub hits: u64,
    pub misses: u64,
    pub negative_hits: u64,
    pub topology_invalidations: u64,
    pub computations: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct PathCacheKey {
    from_x: i32,
    from_y: i32,
    to_x: i32,
    to_y: i32,
    network: u8,
    ship_cost_present: u8,
    ocean_speed_frac: u8,
    canal_speed_frac: u8,
    max_speed: u16,
    curve45_penalty: u32,
    curve90_penalty: u32,
    origin_trackdir_present: u8,
    origin_trackdir_mask: u16,
}

/// Road/water searches persist until topology changes; rail routes expire
/// each tick because their costs include live signals and reservations.
/// No derived cache contents or statistics are serialized.
#[derive(Debug, Default, Clone)]
pub struct PathCache {
    tick: u64,
    entries: HashMap<PathCacheKey, Option<Vec<TileCoord>>>,
    insertion_order: VecDeque<PathCacheKey>,
    topology: Option<(u64, u64)>,
    rail_revision: u64,
    wormholes: Option<TunnelWormholes>,
    stats: PathCacheStats,
}

impl PathCache {
    const MAX_ENTRIES: usize = 256;

    pub fn begin_tick(&mut self, tick: u64) {
        if self.tick != tick {
            self.clear_rail();
            self.tick = tick;
        }
    }

    fn clear_rail(&mut self) {
        self.entries.retain(|key, _| key.network != 1);
        self.insertion_order.retain(|key| key.network != 1);
    }

    pub(crate) fn prepare_map(&mut self, map: &Map) {
        let topology = map.navigation_topology_version();
        if self.topology != Some(topology) {
            if self.topology.is_some() {
                self.stats.topology_invalidations += 1;
            }
            self.entries.clear();
            self.insertion_order.clear();
            self.topology = Some(topology);
        } else if self.rail_revision != map.mutation_revision() {
            self.clear_rail();
        }
        self.rail_revision = map.mutation_revision();
    }

    pub(crate) fn prepare(&mut self, map: &Map, wormholes: Option<&TunnelWormholes>) {
        self.prepare_map(map);
        let wormholes = wormholes.filter(|links| !links.is_empty());
        if self.wormholes.as_ref() != wormholes {
            if !self.entries.is_empty() {
                self.stats.topology_invalidations += 1;
            }
            self.entries.retain(|key, _| matches!(key.network, 2 | 3));
            self.insertion_order
                .retain(|key| matches!(key.network, 2 | 3));
            self.wormholes = wormholes.cloned();
        }
    }

    #[must_use]
    pub const fn stats(&self) -> PathCacheStats {
        self.stats
    }

    pub(crate) fn lookup(&mut self, request: RouteRequest) -> CachedRoute {
        let Some(path) = self.entries.get(&request.key()) else {
            self.stats.misses += 1;
            return CachedRoute::Miss;
        };
        self.stats.hits += 1;
        if path.is_none() {
            self.stats.negative_hits += 1;
        }
        CachedRoute::Hit(path.clone())
    }

    pub(crate) fn insert_result(&mut self, request: RouteRequest, path: Option<Vec<TileCoord>>) {
        self.stats.computations += 1;
        self.insert_key(request.key(), path);
    }

    fn insert_key(&mut self, key: PathCacheKey, path: Option<Vec<TileCoord>>) {
        if !self.entries.contains_key(&key) {
            if self.entries.len() >= Self::MAX_ENTRIES
                && let Some(oldest) = self.insertion_order.pop_front()
            {
                self.entries.remove(&oldest);
            }
            self.insertion_order.push_back(key);
        }
        self.entries.insert(key, path);
    }

    #[must_use]
    pub fn get(
        &self,
        from: TileCoord,
        to: TileCoord,
        network: PathNetwork,
    ) -> Option<&Vec<TileCoord>> {
        let key = cache_key(from, to, network, None, None);
        self.entries.get(&key).and_then(Option::as_ref)
    }

    #[must_use]
    pub fn get_ship(
        &self,
        from: TileCoord,
        to: TileCoord,
        cost: ShipPathCost,
    ) -> Option<&Vec<TileCoord>> {
        self.get_ship_with_trackdir(from, to, cost, None)
    }

    #[must_use]
    pub fn get_ship_with_trackdir(
        &self,
        from: TileCoord,
        to: TileCoord,
        cost: ShipPathCost,
        origin_trackdir: Option<u8>,
    ) -> Option<&Vec<TileCoord>> {
        let key = cache_key(
            from,
            to,
            PathNetwork::Water,
            Some(cost),
            origin_trackdir.map(trackdir_mask),
        );
        self.entries.get(&key).and_then(Option::as_ref)
    }

    #[must_use]
    pub fn get_ship_with_trackdirs(
        &self,
        from: TileCoord,
        to: TileCoord,
        cost: ShipPathCost,
        origin_trackdir_mask: u16,
    ) -> Option<&Vec<TileCoord>> {
        let key = cache_key(
            from,
            to,
            PathNetwork::Water,
            Some(cost),
            Some(origin_trackdir_mask),
        );
        self.entries.get(&key).and_then(Option::as_ref)
    }

    pub fn insert(
        &mut self,
        from: TileCoord,
        to: TileCoord,
        network: PathNetwork,
        path: Vec<TileCoord>,
    ) {
        self.insert_key(cache_key(from, to, network, None, None), Some(path));
    }

    pub fn insert_ship(
        &mut self,
        from: TileCoord,
        to: TileCoord,
        cost: ShipPathCost,
        path: Vec<TileCoord>,
    ) {
        self.insert_ship_with_trackdir(from, to, cost, None, path);
    }

    pub fn insert_ship_with_trackdir(
        &mut self,
        from: TileCoord,
        to: TileCoord,
        cost: ShipPathCost,
        origin_trackdir: Option<u8>,
        path: Vec<TileCoord>,
    ) {
        self.insert_key(
            cache_key(
                from,
                to,
                PathNetwork::Water,
                Some(cost),
                origin_trackdir.map(trackdir_mask),
            ),
            Some(path),
        );
    }

    pub fn insert_ship_with_trackdirs(
        &mut self,
        from: TileCoord,
        to: TileCoord,
        cost: ShipPathCost,
        origin_trackdir_mask: u16,
        path: Vec<TileCoord>,
    ) {
        self.insert_key(
            cache_key(
                from,
                to,
                PathNetwork::Water,
                Some(cost),
                Some(origin_trackdir_mask),
            ),
            Some(path),
        );
    }
}

#[must_use]
fn cache_key(
    from: TileCoord,
    to: TileCoord,
    network: PathNetwork,
    ship_cost: Option<ShipPathCost>,
    origin_trackdir_mask: Option<u16>,
) -> PathCacheKey {
    let (
        ship_cost_present,
        ocean_speed_frac,
        canal_speed_frac,
        max_speed,
        curve45_penalty,
        curve90_penalty,
    ) = ship_cost.map_or((0, 0, 0, 0, 0, 0), |cost| {
        (
            1,
            cost.ocean_speed_frac,
            cost.canal_speed_frac,
            cost.max_speed,
            cost.curve45_penalty,
            cost.curve90_penalty,
        )
    });
    PathCacheKey {
        from_x: from.x,
        from_y: from.y,
        to_x: to.x,
        to_y: to.y,
        network: match network {
            PathNetwork::Road => 0,
            PathNetwork::Rail => 1,
            PathNetwork::Water => 2,
            PathNetwork::Air => 3,
            PathNetwork::Tram => 4,
        },
        ship_cost_present,
        ocean_speed_frac,
        canal_speed_frac,
        max_speed,
        curve45_penalty,
        curve90_penalty,
        origin_trackdir_present: u8::from(origin_trackdir_mask.is_some()),
        origin_trackdir_mask: origin_trackdir_mask.unwrap_or(0),
    }
}

#[must_use]
fn trackdir_mask(trackdir: u8) -> u16 {
    1_u16 << trackdir
}
