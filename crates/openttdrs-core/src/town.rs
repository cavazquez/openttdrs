//! Demanda urbana mínima: casas en cobertura de parada generan pasajeros y correo.

use crate::cargo::CargoType;
use crate::industry::Industry;
use crate::map::Map;
use crate::station::{self, STATION_COVERAGE_RADIUS, Station, StopKind};

/// Ciudad (importada de saves de `OpenTTD` o creada por el juego).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Town {
    pub id: u32,
    pub pos: crate::map::TileCoord,
    pub name: String,
    pub population: u32,
}

/// Periodo de generación (mismo orden de magnitud que [`crate::INDUSTRY_PRODUCE_TICKS`]).
pub const TOWN_PRODUCE_TICKS: u64 = 256;

pub const PASSENGERS_PER_HOUSE: u32 = 2;
pub const MAIL_PER_HOUSE: u32 = 1;

/// Tope de espera en parada bus (análogo al stock de industria).
pub const STATION_TOWN_CARGO_CAPACITY: u32 = 500;

/// Añade pasajeros/correo en paradas bus según casas dentro del radio de cobertura.
pub fn produce_town_cargo(
    map: &Map,
    industries: &[Industry],
    stations: &mut [Station],
    tick: u64,
) -> (u64, u64) {
    if tick == 0 || !tick.is_multiple_of(TOWN_PRODUCE_TICKS) {
        return (0, 0);
    }

    let mut passengers = 0_u64;
    let mut mail = 0_u64;

    for station in stations {
        if station.stop_kind != StopKind::BusStop {
            continue;
        }
        let coverage =
            station::station_coverage_at(map, industries, station.pos, STATION_COVERAGE_RADIUS);
        if coverage.house_tiles == 0 {
            continue;
        }

        let pax_amount = (coverage.house_tiles * PASSENGERS_PER_HOUSE)
            .min(STATION_TOWN_CARGO_CAPACITY.saturating_sub(station.cargo_stock.passengers));
        let mail_amount = (coverage.house_tiles * MAIL_PER_HOUSE)
            .min(STATION_TOWN_CARGO_CAPACITY.saturating_sub(station.cargo_stock.mail));

        station.cargo_stock.add(CargoType::Passengers, pax_amount);
        station.cargo_stock.add(CargoType::Mail, mail_amount);
        passengers += u64::from(pax_amount);
        mail += u64::from(mail_amount);
    }

    (passengers, mail)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::map::{TileCoord, TileKind};

    #[test]
    fn produce_adds_cargo_when_houses_in_coverage() {
        let mut map = Map::new_flat(16, 16, 0);
        let stop_pos = TileCoord::new(8, 8);
        map.set_kind(TileCoord::new(7, 8), TileKind::House).unwrap();
        map.set_kind(TileCoord::new(8, 7), TileKind::House).unwrap();

        let mut stations = vec![Station::new_with_kind(stop_pos, StopKind::BusStop)];

        let (pax, mail) = produce_town_cargo(&map, &[], &mut stations, TOWN_PRODUCE_TICKS);
        assert_eq!(pax, u64::from(2 * PASSENGERS_PER_HOUSE));
        assert_eq!(mail, u64::from(2 * MAIL_PER_HOUSE));
        assert_eq!(stations[0].cargo_stock.passengers, 2 * PASSENGERS_PER_HOUSE);
        assert_eq!(stations[0].cargo_stock.mail, 2 * MAIL_PER_HOUSE);
    }

    #[test]
    fn produce_skips_non_bus_stops() {
        let mut map = Map::new_flat(8, 8, 0);
        let pos = TileCoord::new(2, 2);
        map.set_kind(TileCoord::new(2, 1), TileKind::House).unwrap();
        let mut stations = vec![Station::new_with_kind(pos, StopKind::TruckStop)];

        let (pax, mail) = produce_town_cargo(&map, &[], &mut stations, TOWN_PRODUCE_TICKS);
        assert_eq!(pax, 0);
        assert_eq!(mail, 0);
    }
}
