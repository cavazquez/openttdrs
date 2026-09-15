use std::collections::HashMap;

use crate::map::TileCoord;

use super::{PathNetwork, water::ShipPathCost};

type PathCacheKey = (i32, i32, i32, i32, u8, u8, u8, u8, u16, u32, u32);

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
        let key = cache_key(from, to, network, None);
        self.entries.get(&key)
    }

    #[must_use]
    pub fn get_ship(
        &self,
        from: TileCoord,
        to: TileCoord,
        cost: ShipPathCost,
    ) -> Option<&Vec<TileCoord>> {
        let key = cache_key(from, to, PathNetwork::Water, Some(cost));
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
            .insert(cache_key(from, to, network, None), path);
    }

    pub fn insert_ship(
        &mut self,
        from: TileCoord,
        to: TileCoord,
        cost: ShipPathCost,
        path: Vec<TileCoord>,
    ) {
        if self.entries.len() >= Self::MAX_ENTRIES {
            self.entries.clear();
        }
        self.entries
            .insert(cache_key(from, to, PathNetwork::Water, Some(cost)), path);
    }
}

#[must_use]
fn cache_key(
    from: TileCoord,
    to: TileCoord,
    network: PathNetwork,
    ship_cost: Option<ShipPathCost>,
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
    (
        from.x,
        from.y,
        to.x,
        to.y,
        match network {
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
    )
}
