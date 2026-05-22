use crate::cargo::{CargoStock, CargoType};
use crate::industry::{Industry, IndustryKind};
use crate::map::{Map, TileCoord, TileKind};
use crate::vehicle::VehicleKind;

pub const STATION_COVERAGE_RADIUS: i32 = 4;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Station {
    pub pos: TileCoord,
    #[serde(default)]
    pub stop_kind: StopKind,
    /// Cargo acumulado en el almacén de la estación.
    pub stock: u32,
    #[serde(default)]
    pub cargo_stock: CargoStock,
    /// Contador histórico total de unidades entregadas (análogo a `income` simplificado).
    pub income: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
pub enum StopKind {
    #[default]
    TruckStop,
    BusStop,
    RailStation,
}

impl Station {
    #[must_use]
    pub fn new(pos: TileCoord) -> Self {
        Self {
            pos,
            stop_kind: StopKind::TruckStop,
            stock: 0,
            cargo_stock: CargoStock::default(),
            income: 0,
        }
    }

    #[must_use]
    pub fn new_with_kind(pos: TileCoord, stop_kind: StopKind) -> Self {
        Self {
            pos,
            stop_kind,
            stock: 0,
            cargo_stock: CargoStock::default(),
            income: 0,
        }
    }

    #[must_use]
    pub fn can_service_vehicle(&self, vehicle_kind: VehicleKind) -> bool {
        match vehicle_kind {
            VehicleKind::Bus => self.stop_kind == StopKind::BusStop,
            VehicleKind::Truck => self.stop_kind == StopKind::TruckStop,
            VehicleKind::Train => self.stop_kind == StopKind::RailStation,
        }
    }

    #[must_use]
    pub fn accepts_cargo(&self, cargo: CargoType) -> bool {
        match self.stop_kind {
            StopKind::BusStop => matches!(cargo, CargoType::Passengers | CargoType::Mail),
            StopKind::TruckStop => !matches!(cargo, CargoType::Passengers | CargoType::Mail),
            StopKind::RailStation => !matches!(cargo, CargoType::Passengers | CargoType::Mail),
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
