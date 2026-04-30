use crate::industry::{Industry, IndustryKind};
use crate::map::{Map, TileCoord, TileKind};

pub const STATION_COVERAGE_RADIUS: i32 = 4;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Station {
    pub pos: TileCoord,
    /// Cargo acumulado en el almacén de la estación.
    pub stock: u32,
    /// Contador histórico total de unidades entregadas (análogo a `income` simplificado).
    pub income: u64,
}

impl Station {
    #[must_use]
    pub fn new(pos: TileCoord) -> Self {
        Self {
            pos,
            stock: 0,
            income: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StationCoverage {
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
                TileKind::House => coverage.accepts_mail += 1,
                TileKind::Industry => coverage.accepts_goods += 1,
                _ => {}
            }
        }
    }

    for industry in industries {
        if (industry.pos.x - pos.x).abs() > radius || (industry.pos.y - pos.y).abs() > radius {
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
