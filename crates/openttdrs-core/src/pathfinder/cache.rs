use std::collections::HashMap;

use crate::map::TileCoord;

use super::PathNetwork;

/// Caché de rutas por tick (no se serializa; se invalida al avanzar la simulación).
#[derive(Debug, Default, Clone)]
pub struct PathCache {
    tick: u64,
    entries: HashMap<(i32, i32, i32, i32, u8), Vec<TileCoord>>,
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
        let key = cache_key(from, to, network);
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
        self.entries.insert(cache_key(from, to, network), path);
    }
}

#[must_use]
fn cache_key(from: TileCoord, to: TileCoord, network: PathNetwork) -> (i32, i32, i32, i32, u8) {
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
    )
}
