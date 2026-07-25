use crate::airport_class::airport_spec_def;
use crate::house_spec::{STATION_ACCEPTANCE_THRESHOLD, add_accepted_cargo_of_house};
use crate::industry::{Industry, IndustryKind};
use crate::map::{Map, TileCoord, TileKind};

use super::model::{Station, StopKind};

pub const STATION_COVERAGE_RADIUS: i32 = 4;

/// Radio de cobertura efectivo (`OpenTTD` catchment por `AirportSpec`, resto = 4).
#[must_use]
pub fn station_catchment_radius(station: &Station) -> i32 {
    if station.stop_kind == StopKind::Airport
        && let Some(def) = airport_spec_def(station.airport_spec)
    {
        return def.catchment.max(0);
    }
    STATION_COVERAGE_RADIUS
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StationCoverage {
    /// Teselas `House` dentro del radio (origen de pasajeros/correo).
    pub house_tiles: u32,
    /// Aceptación de correo en octavos (`AddAcceptedCargo_Town`).
    pub accepts_mail: u32,
    /// Aceptación de mercancías/comida en octavos.
    pub accepts_goods: u32,
    /// Aceptación de agua (proxy Oil) en octavos.
    pub accepts_water: u32,
    /// Aceptación de pasajeros en octavos.
    pub accepts_passengers: u32,
    pub supplies_coal: u32,
    pub supplies_wood: u32,
    pub supplies_oil: u32,
    pub supplied_stock: u32,
}

impl StationCoverage {
    #[must_use]
    pub const fn accepts_anything(self) -> bool {
        self.accepts_mail >= STATION_ACCEPTANCE_THRESHOLD
            || self.accepts_goods >= STATION_ACCEPTANCE_THRESHOLD
            || self.accepts_water >= STATION_ACCEPTANCE_THRESHOLD
    }

    /// ¿La estación acepta mercancías urbanas? (`amt >= 8`).
    #[must_use]
    pub const fn accepts_town_goods(self) -> bool {
        self.accepts_goods >= STATION_ACCEPTANCE_THRESHOLD
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

/// Cobertura de una estación usando su catchment (aeropuerto por spec).
#[must_use]
pub fn station_coverage_for(
    map: &Map,
    industries: &[Industry],
    station: &Station,
) -> StationCoverage {
    station_coverage_at(
        map,
        industries,
        station.pos,
        station_catchment_radius(station),
    )
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
                    let house_id = tile.m8 & 0x0FFF;
                    let mut amounts = [0u32; 5];
                    add_accepted_cargo_of_house(house_id, &mut amounts);
                    coverage.accepts_passengers =
                        coverage.accepts_passengers.saturating_add(amounts[0]);
                    coverage.accepts_mail = coverage.accepts_mail.saturating_add(amounts[1]);
                    coverage.accepts_goods = coverage.accepts_goods.saturating_add(amounts[2]);
                    coverage.accepts_water = coverage.accepts_water.saturating_add(amounts[3]);
                }
                TileKind::Industry => {
                    coverage.accepts_goods = coverage
                        .accepts_goods
                        .saturating_add(STATION_ACCEPTANCE_THRESHOLD);
                }
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
            IndustryKind::Factory => {
                coverage.accepts_goods = coverage
                    .accepts_goods
                    .saturating_add(STATION_ACCEPTANCE_THRESHOLD);
            }
        }
    }

    coverage
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::airport_class::AirportSpecId;
    use crate::map::Map;
    use crate::station::Station;

    #[test]
    fn heliport_and_small_catchment_are_four() {
        let mut heli = Station::new_with_kind(TileCoord::new(10, 10), StopKind::Airport);
        heli.airport_spec = AirportSpecId::Heliport;
        assert_eq!(station_catchment_radius(&heli), 4);

        let mut small = Station::new_with_kind(TileCoord::new(10, 10), StopKind::Airport);
        small.airport_spec = AirportSpecId::Small;
        assert_eq!(station_catchment_radius(&small), 4);
    }

    #[test]
    fn intercontinental_catchment_is_ten() {
        let mut st = Station::new_with_kind(TileCoord::new(20, 20), StopKind::Airport);
        st.airport_spec = AirportSpecId::Intercontinental;
        assert_eq!(station_catchment_radius(&st), 10);
    }

    #[test]
    fn intercontinental_covers_houses_beyond_default_radius() {
        let mut map = Map::new_flat(64, 64, 1);
        let airport_pos = TileCoord::new(20, 20);
        let far_house = TileCoord::new(20, 27); // chebyshev dist 7 (> 4, < 10)
        let mut house = map.get(far_house).expect("tile");
        house.kind = TileKind::House;
        map.set_tile(far_house, house).unwrap();

        let mut heli = Station::new_with_kind(airport_pos, StopKind::Airport);
        heli.airport_spec = AirportSpecId::Heliport;
        let heli_cov = station_coverage_for(&map, &[], &heli);
        assert_eq!(heli_cov.house_tiles, 0, "Heliport r=4 no alcanza dist 7");

        let mut inter = Station::new_with_kind(airport_pos, StopKind::Airport);
        inter.airport_spec = AirportSpecId::Intercontinental;
        let inter_cov = station_coverage_for(&map, &[], &inter);
        assert_eq!(
            inter_cov.house_tiles, 1,
            "Intercontinental r=10 debe cubrir casa a dist 7"
        );
    }

    #[test]
    fn rail_station_keeps_default_catchment() {
        let st = Station::new_with_kind(TileCoord::new(5, 5), StopKind::RailStation);
        assert_eq!(station_catchment_radius(&st), STATION_COVERAGE_RADIUS);
    }

    #[test]
    fn house_spec_acceptance_feeds_catchment_goods() {
        let mut map = Map::new_flat(16, 16, 0);
        let house = TileCoord::new(8, 8);
        map.set_completed_house(house, 0, 0).unwrap(); // office: goods 4/8
        let cov = station_coverage_at(&map, &[], TileCoord::new(8, 8), 1);
        assert_eq!(cov.house_tiles, 1);
        assert_eq!(cov.accepts_passengers, 8);
        assert_eq!(cov.accepts_mail, 3);
        assert_eq!(cov.accepts_goods, 4);
        assert!(!cov.accepts_town_goods(), "hace falta ≥8/8");

        map.set_completed_house(TileCoord::new(8, 9), 0, 0).unwrap();
        let cov2 = station_coverage_at(&map, &[], TileCoord::new(8, 8), 1);
        assert_eq!(cov2.accepts_goods, 8);
        assert!(cov2.accepts_town_goods());
    }
}
