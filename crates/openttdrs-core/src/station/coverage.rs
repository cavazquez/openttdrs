use crate::industry::{Industry, IndustryKind};
use crate::map::{Map, TileCoord, TileKind};

pub const STATION_COVERAGE_RADIUS: i32 = 4;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StationCoverage {
    /// Teselas `House` dentro del radio (origen de pasajeros/correo).
    pub house_tiles: u32,
    pub accepts_mail: u32,
    pub accepts_goods: u32,
    pub supplies_coal: u32,
    pub supplies_wood: u32,
    pub supplies_oil: u32,
    pub supplied_stock: u32,
}

impl StationCoverage {
    #[must_use]
    pub const fn accepts_anything(self) -> bool {
        self.accepts_mail > 0 || self.accepts_goods > 0
    }

    #[must_use]
    pub const fn supplies_anything(self) -> bool {
        self.supplies_coal > 0 || self.supplies_wood > 0 || self.supplies_oil > 0
    }
}

/// Desajustes entre teselas `MP_STATION` y entradas en [`crate::game_state::GameState::stations`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StationMapCoherenceReport {
    pub tiles_without_station: Vec<TileCoord>,
    pub stations_without_tile: Vec<TileCoord>,
}

/// Comprueba que cada tesela `Station` tenga entrada en `state.stations` y viceversa.
#[must_use]
pub fn station_map_coherence(state: &crate::GameState) -> StationMapCoherenceReport {
    use std::collections::HashSet;

    let mut report = StationMapCoherenceReport::default();
    let state_positions: HashSet<(i32, i32)> =
        state.stations.iter().map(|s| (s.pos.x, s.pos.y)).collect();

    let (mw, mh) = state.map.dimensions();
    let mut tile_positions = HashSet::new();
    for y in 0..mh {
        for x in 0..mw {
            let c = TileCoord::new(x.cast_signed(), y.cast_signed());
            if state.map.get_kind(c) == Some(TileKind::Station) {
                tile_positions.insert((c.x, c.y));
                if !state_positions.contains(&(c.x, c.y)) {
                    report.tiles_without_station.push(c);
                }
            }
        }
    }

    for station in &state.stations {
        let key = (station.pos.x, station.pos.y);
        if !tile_positions.contains(&key) {
            report.stations_without_tile.push(station.pos);
        }
    }

    report
}

#[must_use]
pub const fn station_covers_tile(station_pos: TileCoord, tile: TileCoord, radius: i32) -> bool {
    (tile.x - station_pos.x).abs() <= radius && (tile.y - station_pos.y).abs() <= radius
}

#[must_use]
pub fn industry_in_station_coverage(
    industry: &Industry,
    station_pos: TileCoord,
    radius: i32,
) -> bool {
    industry
        .tiles
        .iter()
        .copied()
        .chain(std::iter::once(industry.pos))
        .any(|tile| station_covers_tile(station_pos, tile, radius))
}

#[must_use]
pub fn industry_in_station_coverage_by_pos(
    industry_pos: TileCoord,
    station_or_source: TileCoord,
    radius: i32,
) -> bool {
    station_covers_tile(station_or_source, industry_pos, radius)
}

#[must_use]
pub fn station_coverage_at(
    map: &Map,
    industries: &[Industry],
    pos: TileCoord,
    radius: i32,
) -> StationCoverage {
    let mut coverage = StationCoverage::default();
    for y in pos.y - radius..=pos.y + radius {
        for x in pos.x - radius..=pos.x + radius {
            let c = TileCoord::new(x, y);
            let Some(tile) = map.get(c) else {
                continue;
            };
            match tile.kind {
                TileKind::House => {
                    coverage.house_tiles += 1;
                    coverage.accepts_mail += 1;
                }
                TileKind::Industry => coverage.accepts_goods += 1,
                _ => {}
            }
        }
    }

    for industry in industries {
        if !industry_in_station_coverage(industry, pos, radius) {
            continue;
        }
        coverage.supplied_stock = coverage.supplied_stock.saturating_add(industry.stock);
        match industry.kind {
            IndustryKind::CoalMine => coverage.supplies_coal += 1,
            IndustryKind::Forest => coverage.supplies_wood += 1,
            IndustryKind::OilWell => coverage.supplies_oil += 1,
            IndustryKind::Factory => coverage.accepts_goods += 1,
        }
    }

    coverage
}
