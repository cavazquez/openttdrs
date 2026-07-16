use crate::cargo::{ALL_CARGO_TYPES, CargoType};
use crate::company::CompanyId;
use crate::industry::Industry;
use crate::map::{Map, TileCoord};

use super::Station;
use super::coverage::{STATION_COVERAGE_RADIUS, station_coverage_at};

/// Máximo de días sin recogida antes de truncar (`station_cmd.cpp`).
pub const MAX_TIME_SINCE_PICKUP_DAYS: u8 = 255;
/// Rating mínimo del dueño para generar pax/correo en parada bus (`station_cmd.cpp` ≈ 130).
pub const TOWN_CARGO_MIN_OWNER_RATING: u8 = 130;

/// Rating 0–255 para un tipo de carga (255 = recién servido).
///
/// Combina días sin recogida con la edad del packet más viejo en cola.
#[must_use]
pub fn station_rating_for_cargo(station: &Station, cargo: CargoType) -> u8 {
    let from_pickup = station.time_since_pickup.get(cargo);
    let from_packets = station.cargo_packets.oldest_waiting_days(cargo);
    255u8.saturating_sub(from_pickup.max(from_packets))
}

/// Rating 0–255 para la compañía que carga (competencia multi-compañía).
#[must_use]
pub fn station_rating_for_company_cargo(
    station: &Station,
    company: CompanyId,
    cargo: CargoType,
) -> u8 {
    let from_pickup = station.company_pickup_days(company, cargo);
    let from_packets = station.cargo_packets.oldest_waiting_days(cargo);
    255u8.saturating_sub(from_pickup.max(from_packets))
}

/// Recalcula el rating global como mínimo entre cargas con stock en espera.
pub fn recompute_station_rating(station: &mut Station) {
    let mut min_rating = 255u8;
    let mut any_waiting = false;
    for cargo in ALL_CARGO_TYPES {
        if station.cargo_stock.get(cargo) == 0 {
            continue;
        }
        any_waiting = true;
        min_rating = min_rating.min(station_rating_for_cargo(station, cargo));
    }
    station.rating = if any_waiting { min_rating } else { 255 };
}

/// Incrementa antigüedad de carga en espera (una vez por día simulado).
///
/// Si `time_since_pickup` satura en 255 (`MAX_TIME_SINCE_PICKUP_DAYS`) y
/// `selectgoods` está activo, se descarta la carga (`TruncateCargo` en `OpenTTD`).
pub fn tick_station_cargo_age(stations: &mut [Station], selectgoods: bool) {
    for station in stations {
        station.ensure_packets_from_stock();
        if !station.cargo_packets.is_empty() {
            station.cargo_packets.age_waiting_one_day();
        }
        for cargo in ALL_CARGO_TYPES {
            if station.cargo_stock.get(cargo) == 0 {
                continue;
            }
            station.time_since_pickup.increment_waiting(cargo);
            for (_, company_tsp) in &mut station.company_time_since_pickup {
                company_tsp.increment_waiting(cargo);
            }
            if selectgoods && station.time_since_pickup.get(cargo) == MAX_TIME_SINCE_PICKUP_DAYS {
                station.cargo_packets.truncate_cargo(cargo);
                station.time_since_pickup.set(cargo, 0);
            }
        }
        station.sync_stock_from_packets();
        recompute_station_rating(station);
    }
}

/// Marca recogida reciente de un tipo de carga por una compañía.
pub fn on_station_cargo_pickup(station: &mut Station, cargo: CargoType, company: CompanyId) {
    station.time_since_pickup.set(cargo, 0);
    station.company_pickup_slot_mut(company).set(cargo, 0);
    for p in &mut station.cargo_packets.packets {
        if p.cargo == cargo {
            p.periods_in_transit = 0;
        }
    }
    recompute_station_rating(station);
}

/// Factor 0–255 para limitar cantidad cargable según rating.
#[must_use]
pub fn load_amount_for_rating(requested: u32, rating: u8) -> u32 {
    if requested == 0 {
        return 0;
    }
    let scaled = (u64::from(requested) * u64::from(rating)) / 255;
    u32::try_from(scaled).unwrap_or(u32::MAX)
}

/// Parada donde el vehículo puede recoger mercancía primaria (mina, bosque, pozo).
#[must_use]
pub fn station_is_freight_pickup_stop(
    map: &Map,
    industries: &[Industry],
    station_pos: TileCoord,
    cargo: CargoType,
) -> bool {
    let coverage = station_coverage_at(map, industries, station_pos, STATION_COVERAGE_RADIUS);
    match cargo {
        CargoType::Coal | CargoType::IronOre => coverage.supplies_coal > 0,
        CargoType::Wood | CargoType::Grain | CargoType::Livestock => coverage.supplies_wood > 0,
        CargoType::Oil => coverage.supplies_oil > 0,
        _ => false,
    }
}
