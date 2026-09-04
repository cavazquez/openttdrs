use crate::cargo::{ALL_CARGO_TYPES, CUSTOM_CARGO_COUNT, CargoType};
use crate::cargo_spec::{CargoSpecDef, cargo_spec_for_type};
use crate::cargodist::parity::Randomizer;
use crate::company::CompanyId;
use crate::industry::Industry;
use crate::map::{Map, TileCoord};
use crate::newgrf_callback::resolve_cargo_station_rating_callback;
use crate::vehicle::VehicleKind;

use super::Station;
use super::coverage::{STATION_COVERAGE_RADIUS, station_coverage_at};
use super::goods_entry::INITIAL_STATION_RATING;

/// Barridos sin recogida tras los que la carga se descarta (`time_since_pickup == 255`).
pub const MAX_TIME_SINCE_PICKUP_DAYS: u8 = 255;
/// Rating mínimo del dueño para generar pax/correo en parada bus (`station_cmd.cpp` ≈ 130).
pub const TOWN_CARGO_MIN_OWNER_RATING: u8 = 130;

/// Boost de rating por campaña publicitaria mediana (`TownActionAdvertiseMedium`, +0x70).
pub const TOWN_ADVERTISE_MEDIUM_RATING_BOOST: u8 = 0x70;
/// Radio manhattan de la publicidad mediana (`TownActionAdvertiseMedium`, 15 teselas).
pub const TOWN_ADVERTISE_MEDIUM_RADIUS: i32 = 15;

/// A partir de esta cantidad en espera empieza el recorte progresivo
/// (`WAITING_CARGO_THRESHOLD`, `station_cmd.cpp:4101`).
const WAITING_CARGO_THRESHOLD: u32 = 1 << 12;
/// Divisor del exceso recortado por barrido (`WAITING_CARGO_CUT_FACTOR`).
const WAITING_CARGO_CUT_FACTOR: u32 = 1 << 6;
/// Tope duro de carga en espera (`MAX_WAITING_CARGO`).
const MAX_WAITING_CARGO: u32 = 1 << 15;

/// Rating persistente 0–255 de un tipo de carga en la estación (`GoodsEntry::rating`).
#[must_use]
pub fn station_rating_for_cargo(station: &Station, cargo: CargoType) -> u8 {
    station.goods.rating(cargo)
}

/// Rating de la carga tal como lo ve una compañía al cargar.
///
/// En `OpenTTD` el rating es de la estación, igual para todos: la competencia entre compañías
/// se resuelve al repartir la producción (`MoveGoodsToStation`), no aquí. El port sigue
/// midiendo `company_time_since_pickup` por compañía porque ese reparto lo necesitará.
#[must_use]
pub fn station_rating_for_company_cargo(
    station: &Station,
    _company: CompanyId,
    cargo: CargoType,
) -> u8 {
    station.goods.rating(cargo)
}

/// Objetivo de rating para una carga según el servicio prestado (`UpdateStationRating`).
///
/// Devuelve el valor crudo, sin acotar ni suavizar: la convergencia ±2 la aplica
/// [`super::goods_entry::GoodsEntry::converge_rating_towards`].
#[must_use]
fn station_rating_target(station: &Station, cargo: CargoType, cargo_specs: &[CargoSpecDef]) -> i16 {
    let entry = station.goods.get(cargo);
    if let Some(rating) = cargo_spec_for_type(cargo_specs, cargo).and_then(|def| {
        resolve_cargo_station_rating_callback(
            def,
            station.time_since_pickup.get(cargo),
            entry.max_waiting_cargo,
            entry.has_vehicle_ever_tried_loading(),
            entry.last_speed,
            station.last_vehicle_type,
        )
    }) {
        return rating;
    }
    let mut rating: i16 = 0;

    let speed_bonus = i16::from(entry.last_speed) - 85;
    if speed_bonus >= 0 {
        rating += speed_bonus >> 2;
    }

    let mut waittime = station.time_since_pickup.get(cargo);
    if station.last_vehicle_type == Some(VehicleKind::Ship) {
        waittime >>= 2;
    }
    if waittime <= 21 {
        rating += 25;
    }
    if waittime <= 12 {
        rating += 25;
    }
    if waittime <= 6 {
        rating += 45;
    }
    if waittime <= 3 {
        rating += 35;
    }

    rating -= 90;
    if entry.max_waiting_cargo <= 1500 {
        rating += 55;
    }
    if entry.max_waiting_cargo <= 1000 {
        rating += 35;
    }
    if entry.max_waiting_cargo <= 600 {
        rating += 10;
    }
    if entry.max_waiting_cargo <= 300 {
        rating += 20;
    }
    if entry.max_waiting_cargo <= 100 {
        rating += 10;
    }

    // Falta el +26 por estatua: el port todavía no tiene acciones de ayuntamiento.

    if entry.last_age < 3 {
        rating += 10;
    }
    if entry.last_age < 2 {
        rating += 10;
    }
    if entry.last_age < 1 {
        rating += 13;
    }

    rating
}

/// Suma `amount` al rating de cada carga activa en estaciones del `owner` dentro de `radius`
/// teselas manhattan de `center` (`ModifyStationRatingAround`, `station_cmd.cpp:4398`).
///
/// Una entrada cuenta como activa si `has_rating`, si algún vehículo intentó cargar o si hay
/// stock en espera — alineado a `GoodsEntry::status.Any()` del original.
#[must_use]
pub fn modify_station_rating_around(
    stations: &mut [Station],
    center: TileCoord,
    owner: CompanyId,
    radius: i32,
    amount: u8,
) -> usize {
    let mut touched = 0usize;
    for station in stations.iter_mut() {
        if station.owner != owner {
            continue;
        }
        let dx = (station.pos.x - center.x).abs();
        let dy = (station.pos.y - center.y).abs();
        if dx.saturating_add(dy) > radius {
            continue;
        }
        station.ensure_packets_from_stock();
        let mut station_changed = false;
        for cargo in ALL_CARGO_TYPES {
            let entry = station.goods.get(cargo);
            let active = entry.has_rating
                || entry.has_vehicle_ever_tried_loading()
                || station.cargo_stock.get(cargo) > 0;
            if !active {
                continue;
            }
            let entry = station.goods.get_mut(cargo);
            entry.rating = entry.rating.saturating_add(amount);
            entry.has_rating = true;
            station_changed = true;
            touched += 1;
        }
        for slot in 0..CUSTOM_CARGO_COUNT {
            let cargo = crate::cargo::custom_cargo(slot);
            let entry = station.goods.get(cargo);
            let active = entry.has_rating
                || entry.has_vehicle_ever_tried_loading()
                || station.cargo_stock.get(cargo) > 0;
            if !active {
                continue;
            }
            let entry = station.goods.get_mut(cargo);
            entry.rating = entry.rating.saturating_add(amount);
            entry.has_rating = true;
            station_changed = true;
            touched += 1;
        }
        if station_changed {
            recompute_station_rating(station);
        }
    }
    touched
}

/// Resumen para la UI: el peor rating entre las cargas que la estación llegó a mover.
pub fn recompute_station_rating(station: &mut Station) {
    let mut min_rating = 255u8;
    let mut any_rated = false;
    for cargo in ALL_CARGO_TYPES {
        if !station.goods.get(cargo).has_rating {
            continue;
        }
        any_rated = true;
        min_rating = min_rating.min(station.goods.rating(cargo));
    }
    for slot in 0..CUSTOM_CARGO_COUNT {
        let cargo = crate::cargo::custom_cargo(slot);
        if !station.goods.get(cargo).has_rating {
            continue;
        }
        any_rated = true;
        min_rating = min_rating.min(station.goods.rating(cargo));
    }
    station.rating = if any_rated {
        min_rating
    } else {
        INITIAL_STATION_RATING
    };
}

/// Barrido de rating de estaciones (`UpdateStationRating`), cada `STATION_RATING_TICKS`.
///
/// Envejece la carga en espera, mueve el rating de cada tipo hacia el objetivo que merece el
/// servicio prestado y descarta carga cuando la estación va mal y hay mucha acumulada.
pub fn update_station_ratings(stations: &mut [Station], selectgoods: bool, rng: &mut Randomizer) {
    update_station_ratings_with_cargo_callbacks(stations, &[], selectgoods, rng);
}

/// Barrido de rating que ejecuta CB145 para los cargos `NewGRF` del catálogo.
///
/// Conserva [`update_station_ratings`] como API sin catálogo para callers legacy;
/// el loop de `GameState` usa esta variante y por eso el callback se ejecuta en
/// el mismo punto periódico que el algoritmo estándar de `OpenTTD`.
pub fn update_station_ratings_with_cargo_callbacks(
    stations: &mut [Station],
    cargo_specs: &[CargoSpecDef],
    selectgoods: bool,
    rng: &mut Randomizer,
) {
    for station in stations {
        station.ensure_packets_from_stock();
        if !station.cargo_packets.is_empty() {
            station.cargo_packets.age_waiting_one_period();
        }
        for cargo in ALL_CARGO_TYPES {
            update_cargo_rating(station, cargo, cargo_specs, selectgoods, rng);
        }
        for slot in 0..CUSTOM_CARGO_COUNT {
            update_cargo_rating(
                station,
                crate::cargo::custom_cargo(slot),
                cargo_specs,
                selectgoods,
                rng,
            );
        }
        station.sync_stock_from_packets();
        recompute_station_rating(station);
    }
}

fn update_cargo_rating(
    station: &mut Station,
    cargo: CargoType,
    cargo_specs: &[CargoSpecDef],
    selectgoods: bool,
    rng: &mut Randomizer,
) {
    let waiting = station.cargo_stock.get(cargo);
    if waiting > 0 {
        // La carga llegó a la estación: a partir de aquí este tipo tiene rating propio.
        station.goods.get_mut(cargo).has_rating = true;
    }

    if !station.goods.get(cargo).has_rating {
        // Nunca movió esta carga: el rating se recupera de uno en uno hacia el inicial.
        let entry = station.goods.get_mut(cargo);
        if entry.rating < INITIAL_STATION_RATING {
            entry.rating += 1;
        }
        return;
    }

    station.time_since_pickup.increment_waiting(cargo);
    for (_, company_tsp) in &mut station.company_time_since_pickup {
        company_tsp.increment_waiting(cargo);
    }

    if selectgoods && station.time_since_pickup.get(cargo) == MAX_TIME_SINCE_PICKUP_DAYS {
        let entry = station.goods.get_mut(cargo);
        entry.has_rating = false;
        entry.last_speed = 0;
        station.cargo_packets.truncate_cargo(cargo);
        station.time_since_pickup.set(cargo, 0);
        return;
    }

    let target = station_rating_target(station, cargo, cargo_specs);
    let rating = station.goods.get_mut(cargo).converge_rating_towards(target);
    truncate_waiting_cargo(station, cargo, rating, waiting, rng);
}

/// Recorta la carga en espera cuando la estación está mal valorada o acumula de más.
///
/// Con un solo destino (`NUM_DESTS = 1`) el reparto equivale al modo manual del original,
/// donde `waiting_avg = waiting / 2`.
fn truncate_waiting_cargo(
    station: &mut Station,
    cargo: CargoType,
    rating: u8,
    waiting: u32,
    rng: &mut Randomizer,
) {
    const NUM_DESTS: u32 = 1;
    let waiting_avg = waiting / (NUM_DESTS + 1);
    let mut left = waiting;
    let mut changed = false;

    if rating <= 64 && waiting_avg >= 100 {
        let mut dec = rng.next() & 0x1F;
        if waiting_avg < 200 {
            dec &= 7;
        }
        left = left.saturating_sub((dec + 1) * NUM_DESTS);
        changed = true;
    }

    if rating <= 127 && left != 0 {
        let r = rng.next();
        if u32::from(rating) <= (r & 0x7F) {
            left = left.saturating_sub((((r >> 8) & 0x03) + 1) * NUM_DESTS);
            changed = true;
        }
    }

    if left > WAITING_CARGO_THRESHOLD {
        let excess = left - WAITING_CARGO_THRESHOLD;
        left = (left - excess / WAITING_CARGO_CUT_FACTOR).min(MAX_WAITING_CARGO);
        changed = true;
    }

    let available = station.cargo_packets.total_of(cargo);
    if changed && left < available {
        station.goods.get_mut(cargo).max_waiting_cargo = 0;
        let (_moved, per_source) =
            station
                .cargo_packets
                .truncate_cargo_amount(cargo, available - left, rng);
        // OpenTTD castiga `max_waiting_cargo` del origen; aquí el origen suele ser
        // esta estación (`first_station`).
        for (src, amount) in per_source {
            if amount == 0 || src != Some(station.pos) {
                continue;
            }
            let entry = station.goods.get_mut(cargo);
            entry.max_waiting_cargo = entry.max_waiting_cargo.max(amount);
        }
    } else {
        station.goods.get_mut(cargo).max_waiting_cargo = waiting_avg;
    }
}

/// Datos del vehículo que la estación recuerda tras un intento de carga.
///
/// `OpenTTD` los guarda en `GoodsEntry` al cargar (`economy.cpp:1745-1765`) y los usa como
/// términos del rating: servir con material rápido y nuevo puntúa.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StationVisit {
    pub vehicle_kind: VehicleKind,
    /// Velocidad máxima en las unidades de rating del original.
    pub last_speed: u8,
    /// Edad del vehículo en años.
    pub last_age: u8,
}

/// Anota que un vehículo intentó cargar aquí, aunque el andén estuviera vacío.
///
/// Con `selectgoods`, `MoveGoodsToStation` no entrega a estaciones que nadie ha visitado.
/// Sin este registro, un bus en una parada nueva nunca vería pasajeros: no hay carga → no
/// hay recogida → no hay `last_speed` → no llega carga.
pub fn note_station_load_attempt(station: &mut Station, cargo: CargoType, visit: StationVisit) {
    station.last_vehicle_type = Some(visit.vehicle_kind);
    let entry = station.goods.get_mut(cargo);
    // `last_speed == 0` significa «nunca»; un intento real siempre deja al menos 1.
    entry.last_speed = visit.last_speed.max(1);
    entry.last_age = visit.last_age;
}

/// Marca recogida reciente de un tipo de carga por una compañía.
pub fn on_station_cargo_pickup(
    station: &mut Station,
    cargo: CargoType,
    company: CompanyId,
    visit: StationVisit,
) {
    note_station_load_attempt(station, cargo, visit);
    station.time_since_pickup.set(cargo, 0);
    station.company_pickup_slot_mut(company).set(cargo, 0);
    // La carga que sigue en el andén no rejuvenece: su antigüedad es la que cobrará al
    // entregarse. El rating ya no depende de ella, sino de `time_since_pickup`.
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
        CargoType::Coal
        | CargoType::IronOre
        | CargoType::CopperOre
        | CargoType::Gold
        | CargoType::Diamonds
        | CargoType::Sugar
        | CargoType::Batteries
        | CargoType::Plastic
        | CargoType::Toffee
        | CargoType::Water => coverage.supplies_coal > 0,
        CargoType::Wood
        | CargoType::Grain
        | CargoType::Wheat
        | CargoType::Maize
        | CargoType::Livestock
        | CargoType::Fruit
        | CargoType::Rubber
        | CargoType::CottonCandy
        | CargoType::Bubbles => coverage.supplies_wood > 0,
        CargoType::Oil | CargoType::Cola => coverage.supplies_oil > 0,
        _ => false,
    }
}
