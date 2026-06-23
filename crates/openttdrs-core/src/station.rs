use crate::cargo::{CargoStock, CargoType};
use crate::industry::{Industry, IndustryKind};
use crate::map::{Map, TileCoord, TileKind};
use crate::pathfinder::diag_dir_offset;
use crate::vehicle::{VehicleKind, VehicleOrder};

pub const STATION_COVERAGE_RADIUS: i32 = 4;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Station {
    pub pos: TileCoord,
    #[serde(default)]
    pub stop_kind: StopKind,
    /// Nombre de la estación (saves de `OpenTTD` con nombre custom).
    #[serde(default)]
    pub name: Option<String>,
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
    /// Punto de paso ferroviario (`StationType::RailWaypoint`); sin carga ni parada.
    RailWaypoint,
}

impl Station {
    #[must_use]
    pub fn new(pos: TileCoord) -> Self {
        Self::new_with_kind(pos, StopKind::TruckStop)
    }

    #[must_use]
    pub fn new_with_kind(pos: TileCoord, stop_kind: StopKind) -> Self {
        Self {
            pos,
            stop_kind,
            name: None,
            stock: 0,
            cargo_stock: CargoStock::default(),
            income: 0,
        }
    }

    #[must_use]
    pub fn can_service_vehicle(&self, vehicle_kind: VehicleKind) -> bool {
        matches!(
            (vehicle_kind, self.stop_kind),
            (
                VehicleKind::Train,
                StopKind::RailStation | StopKind::RailWaypoint
            ) | (VehicleKind::Bus, StopKind::BusStop)
                | (VehicleKind::Truck, StopKind::TruckStop)
        )
    }

    #[must_use]
    pub fn is_waypoint(&self) -> bool {
        self.stop_kind == StopKind::RailWaypoint
    }

    #[must_use]
    pub fn accepts_cargo(&self, cargo: CargoType) -> bool {
        if self.stop_kind == StopKind::RailWaypoint {
            return false;
        }
        match self.stop_kind {
            StopKind::BusStop => matches!(cargo, CargoType::Passengers | CargoType::Mail),
            StopKind::TruckStop => !matches!(cargo, CargoType::Passengers | CargoType::Mail),
            StopKind::RailStation => !matches!(cargo, CargoType::Passengers | CargoType::Mail),
            StopKind::RailWaypoint => false,
        }
    }
}

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

/// `StationType::RailWaypoint` en bits 3–6 de `m6` (`station_type.h`).
pub const STATION_TYPE_RAIL_WAYPOINT: u8 = 7;

#[must_use]
pub fn station_type_from_m6(m6: u8) -> u8 {
    (m6 >> 3) & 0x0F
}

#[must_use]
pub fn is_rail_waypoint_tile(tile: &crate::map::Tile) -> bool {
    tile.kind == TileKind::Station && station_type_from_m6(tile.m6) == STATION_TYPE_RAIL_WAYPOINT
}

#[must_use]
pub fn is_rail_waypoint_at(map: &Map, c: TileCoord) -> bool {
    map.get(c).is_some_and(|t| is_rail_waypoint_tile(&t))
}

#[must_use]
fn is_rail_track_kind(kind: TileKind) -> bool {
    matches!(
        kind,
        TileKind::Rail | TileKind::RailDepot | TileKind::RailTunnel | TileKind::RailBridge
    )
}

/// Teselas `Station` contiguas al ancla (huella de una estación multi-tesela).
#[must_use]
fn station_footprint_tiles(map: &Map, anchor: TileCoord) -> Vec<TileCoord> {
    const MAX_FOOTPRINT: usize = 64;
    let mut tiles = vec![anchor];
    let mut seen = std::collections::HashSet::from([anchor]);
    let mut i = 0;
    while i < tiles.len() && tiles.len() < MAX_FOOTPRINT {
        let c = tiles[i];
        i += 1;
        for dir in 0..4u8 {
            let (dx, dy) = diag_dir_offset(dir);
            let n = TileCoord::new(c.x + dx, c.y + dy);
            if map.get_kind(n) == Some(TileKind::Station) && seen.insert(n) {
                tiles.push(n);
            }
        }
    }
    tiles
}

/// Tesela de vía donde el tren debe detenerse junto a una estación de tren (no
/// sobre la plataforma). Usa la plataforma si ya es vía (`StationType::Rail`)
/// o la vía adyacente más cercana. Prefiere vía adyacente; si no hay, plataforma rail.
#[must_use]
pub fn rail_station_approach_tile(map: &Map, station_pos: TileCoord) -> Option<TileCoord> {
    let is_rail_platform = |c: TileCoord| {
        map.get(c)
            .is_some_and(|t| t.kind == TileKind::Station && station_type_from_m6(t.m6) == 0)
    };
    let mut adjacent: Option<(i32, TileCoord)> = None;
    let mut platform: Option<(i32, TileCoord)> = None;
    for c in station_footprint_tiles(map, station_pos) {
        if is_rail_platform(c) {
            let d = (c.x - station_pos.x).abs() + (c.y - station_pos.y).abs();
            if platform.is_none_or(|(bd, _)| d < bd) {
                platform = Some((d, c));
            }
        }
        for dir in 0..4u8 {
            let (dx, dy) = diag_dir_offset(dir);
            let track = TileCoord::new(c.x + dx, c.y + dy);
            if !map.get_kind(track).is_some_and(is_rail_track_kind) {
                continue;
            }
            let d = (track.x - station_pos.x).abs() + (track.y - station_pos.y).abs();
            if adjacent.is_none_or(|(bd, _)| d < bd) {
                adjacent = Some((d, track));
            }
        }
    }
    adjacent.or(platform).map(|(_, track)| track)
}

/// Destino de movimiento según tipo de vehículo y orden (trenes paran en la vía adyacente).
#[must_use]
pub fn resolve_order_destination(map: &Map, kind: VehicleKind, order: VehicleOrder) -> TileCoord {
    match (kind, order) {
        (VehicleKind::Train, VehicleOrder::Station { station, .. }) => {
            rail_station_approach_tile(map, station).unwrap_or(station)
        }
        (VehicleKind::Train, VehicleOrder::Waypoint { waypoint }) => waypoint,
        (_, order) => order.destination(),
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
