use std::collections::HashMap;

use crate::map::TileCoord;

use super::{PathNetwork, water::ShipPathCost};

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
    origin_trackdir: u8,
}

/// Caché de rutas por tick (no se serializa; se invalida al avanzar la simulación).
#[derive(Debug, Default, Clone)]
pub struct PathCache {
    tick: u64,
    entries: HashMap<PathCacheKey, Vec<TileCoord>>,
}

impl PathCache {
    const MAX_ENTRIES: usize = 256;

    pub fn begin_tick(&mut self, tick: u64) {
        if self.tick != tick {
            self.entries.clear();
            self.tick = tick;
        }
    }

    #[must_use]
    pub fn get(
        &self,
        from: TileCoord,
        to: TileCoord,
        network: PathNetwork,
    ) -> Option<&Vec<TileCoord>> {
        let key = cache_key(from, to, network, None, None);
        self.entries.get(&key)
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
        let key = cache_key(from, to, PathNetwork::Water, Some(cost), origin_trackdir);
        self.entries.get(&key)
    }

    pub fn insert(
        &mut self,
        from: TileCoord,
        to: TileCoord,
        network: PathNetwork,
        path: Vec<TileCoord>,
    ) {
        if self.entries.len() >= Self::MAX_ENTRIES {
            self.entries.clear();
        }
        self.entries
            .insert(cache_key(from, to, network, None, None), path);
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
        if self.entries.len() >= Self::MAX_ENTRIES {
            self.entries.clear();
        }
        self.entries.insert(
            cache_key(from, to, PathNetwork::Water, Some(cost), origin_trackdir),
            path,
        );
    }
}

#[must_use]
fn cache_key(
    from: TileCoord,
    to: TileCoord,
    network: PathNetwork,
    ship_cost: Option<ShipPathCost>,
    origin_trackdir: Option<u8>,
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
    let (origin_trackdir_present, origin_trackdir) =
        origin_trackdir.map_or((0, 0), |trackdir| (1, trackdir));
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
        origin_trackdir_present,
        origin_trackdir,
    }
}
