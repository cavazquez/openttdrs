//! Reparto de producción entre estaciones competidoras (`MoveGoodsToStation`).
//!
//! Cuando varias estaciones cubren la misma industria o el mismo pueblo, la carga no va
//! entera a la primera que aparece: se pondera por `rating + 1` y se reparte entre
//! compañías y estaciones. El rating también decide cuánto de lo producido llega de
//! verdad (el resto se pierde en el camino al andén).

use crate::cargo::CargoType;
use crate::company::CompanyId;
use crate::map::TileCoord;

use super::Station;
use super::model::StopKind;

/// ¿Puede esta estación recibir este tipo de carga? (`CanMoveGoodsToStation`).
#[must_use]
pub fn can_move_goods_to_station(station: &Station, cargo: CargoType, selectgoods: bool) -> bool {
    if station.goods.rating(cargo) == 0 {
        return false;
    }
    if selectgoods && !station.goods.get(cargo).has_vehicle_ever_tried_loading() {
        return false;
    }
    if cargo.is_town_cargo() {
        // Los pasajeros no van a una parada que solo sea de camiones.
        if station.stop_kind == StopKind::TruckStop {
            return false;
        }
    } else if station.stop_kind == StopKind::BusStop {
        // La mercancía no va a una parada que solo sea de buses.
        return false;
    }
    if station.is_waypoint() {
        return false;
    }
    station.accepts_cargo(cargo)
}

/// Añade carga en espera aplicando la parte fraccionaria (`UpdateStationWaiting`).
///
/// `amount` ya viene escalado por `rating + 1` (o el `best_rating + 1` del reparto).
/// Devuelve las unidades enteras que acabaron en el andén.
pub fn update_station_waiting(
    station: &mut Station,
    cargo: CargoType,
    amount: u32,
    source: TileCoord,
) -> u32 {
    if amount == 0 {
        return 0;
    }
    let entry = station.goods.get_mut(cargo);
    let total = amount.saturating_add(u32::from(entry.amount_fract));
    entry.amount_fract = (total & 0xFF) as u8;
    let whole = total >> 8;
    if whole == 0 {
        return 0;
    }
    if !entry.has_rating {
        entry.has_rating = true;
    }
    station.ensure_packets_from_stock();
    station.cargo_packets.add_amount(cargo, whole, source);
    station.sync_stock_from_packets();
    whole
}

/// Reparte `amount` unidades entre las estaciones candidatas (`MoveGoodsToStation`).
///
/// `station_indices` son índices en `stations`. Devuelve cuánto acabó realmente en andenes
/// (tras el escalado por rating y la parte fraccionaria).
pub fn move_goods_to_station(
    stations: &mut [Station],
    station_indices: &[usize],
    cargo: CargoType,
    amount: u32,
    source: TileCoord,
    selectgoods: bool,
    exclusivity: Option<CompanyId>,
) -> u32 {
    if amount == 0 || station_indices.is_empty() {
        return 0;
    }

    let mut eligible: Vec<usize> = Vec::new();
    for &idx in station_indices {
        let Some(station) = stations.get(idx) else {
            continue;
        };
        if let Some(owner) = exclusivity
            && station.owner != owner
        {
            continue;
        }
        if can_move_goods_to_station(station, cargo, selectgoods) {
            eligible.push(idx);
        }
    }

    if eligible.is_empty() {
        return 0;
    }

    if eligible.len() == 1 {
        let idx = eligible[0];
        let rating = u32::from(stations[idx].goods.rating(cargo));
        let scaled = amount.saturating_mul(rating + 1);
        return update_station_waiting(&mut stations[idx], cargo, scaled, source);
    }

    // Mejor rating y suma de ratings por compañía; suma de mejores entre compañías.
    let mut company_best = [0_u32; 256];
    let mut company_sum = [0_u32; 256];
    let mut best_rating = 0_u32;
    let mut best_sum = 0_u32;

    for &idx in &eligible {
        let owner = stations[idx].owner.0 as usize;
        let rating = u32::from(stations[idx].goods.rating(cargo));
        if rating > company_best[owner] {
            best_sum += rating - company_best[owner];
            company_best[owner] = rating;
            if rating > best_rating {
                best_rating = rating;
            }
        }
        company_sum[owner] += rating;
    }

    if best_sum == 0 {
        return 0;
    }

    // Unidades fraccionarias: amount × (mejor rating + 1).
    let fractional = u64::from(amount).saturating_mul(u64::from(best_rating) + 1);
    let mut shares: Vec<(usize, u64)> = Vec::with_capacity(eligible.len());
    let mut moving = 0_u64;

    for &idx in &eligible {
        let owner = stations[idx].owner.0 as usize;
        let rating = u64::from(stations[idx].goods.rating(cargo));
        let share = fractional
            .saturating_mul(u64::from(company_best[owner]))
            .saturating_mul(rating)
            / u64::from(best_sum)
            / u64::from(company_sum[owner]).max(1);
        shares.push((idx, share));
        moving += share;
    }

    // El resto por redondeo va a las de mejor rating.
    if fractional > moving {
        shares.sort_by(|a, b| {
            let ra = stations[a.0].goods.rating(cargo);
            let rb = stations[b.0].goods.rating(cargo);
            rb.cmp(&ra).then_with(|| a.0.cmp(&b.0))
        });
        let mut left = fractional - moving;
        for share in &mut shares {
            if left == 0 {
                break;
            }
            share.1 += 1;
            left -= 1;
        }
    }

    let mut moved = 0_u32;
    for (idx, share) in shares {
        let give = u32::try_from(share).unwrap_or(u32::MAX);
        moved = moved.saturating_add(update_station_waiting(
            &mut stations[idx],
            cargo,
            give,
            source,
        ));
    }
    moved
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::TileCoord;
    use crate::station::INITIAL_STATION_RATING;
    use crate::station::Station;
    use crate::station::StopKind;

    fn truck_stop(pos: TileCoord) -> Station {
        let mut st = Station::new_with_kind(pos, StopKind::TruckStop);
        // Sin esto, selectgoods bloquearía el reparto (nadie ha intentado cargar).
        st.goods.get_mut(CargoType::Coal).last_speed = 1;
        st
    }

    #[test]
    fn single_station_receives_rating_fraction() {
        let mut stations = vec![truck_stop(TileCoord::new(0, 0))];
        let moved = move_goods_to_station(
            &mut stations,
            &[0],
            CargoType::Coal,
            256,
            TileCoord::new(1, 0),
            true,
            None,
        );
        // 256 × (175 + 1) >> 8 = 176.
        assert_eq!(moved, 176);
        assert_eq!(stations[0].cargo_stock.coal, 176);
        assert!(stations[0].goods.get(CargoType::Coal).has_rating);
    }

    #[test]
    fn better_rated_station_gets_more_cargo() {
        let mut good = truck_stop(TileCoord::new(0, 0));
        let mut bad = truck_stop(TileCoord::new(2, 0));
        good.goods.get_mut(CargoType::Coal).rating = 200;
        bad.goods.get_mut(CargoType::Coal).rating = 50;
        let mut stations = vec![good, bad];

        let moved = move_goods_to_station(
            &mut stations,
            &[0, 1],
            CargoType::Coal,
            256,
            TileCoord::new(1, 0),
            true,
            None,
        );
        assert!(moved > 0);
        assert!(
            stations[0].cargo_stock.coal > stations[1].cargo_stock.coal,
            "buena {} vs mala {}",
            stations[0].cargo_stock.coal,
            stations[1].cargo_stock.coal
        );
    }

    #[test]
    fn companies_compete_by_best_rating() {
        let mut a = truck_stop(TileCoord::new(0, 0));
        let mut b = truck_stop(TileCoord::new(2, 0));
        a.owner = CompanyId::PLAYER;
        b.owner = CompanyId(1);
        a.goods.get_mut(CargoType::Coal).rating = 200;
        b.goods.get_mut(CargoType::Coal).rating = 100;
        let mut stations = vec![a, b];

        move_goods_to_station(
            &mut stations,
            &[0, 1],
            CargoType::Coal,
            256,
            TileCoord::new(1, 0),
            true,
            None,
        );
        assert!(
            stations[0].cargo_stock.coal > stations[1].cargo_stock.coal,
            "la compañía con mejor rating se lleva más"
        );
    }

    #[test]
    fn selectgoods_blocks_unvisited_station() {
        let st = Station::new_with_kind(TileCoord::new(0, 0), StopKind::TruckStop);
        assert_eq!(st.goods.rating(CargoType::Coal), INITIAL_STATION_RATING);
        let mut stations = vec![st];
        let moved = move_goods_to_station(
            &mut stations,
            &[0],
            CargoType::Coal,
            100,
            TileCoord::new(1, 0),
            true,
            None,
        );
        assert_eq!(moved, 0);
        assert_eq!(stations[0].cargo_stock.coal, 0);
    }

    #[test]
    fn rating_zero_rejects_cargo() {
        let mut st = truck_stop(TileCoord::new(0, 0));
        st.goods.get_mut(CargoType::Coal).rating = 0;
        let mut stations = vec![st];
        assert_eq!(
            move_goods_to_station(
                &mut stations,
                &[0],
                CargoType::Coal,
                100,
                TileCoord::new(1, 0),
                true,
                None,
            ),
            0
        );
    }
}
