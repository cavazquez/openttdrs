//! Pagos de transporte inspirados en `GetTransportedGoodsIncome` de OpenTTD (Camino B).

use crate::cargo::CargoType;
use crate::map::TileCoord;
use crate::vehicle::{Vehicle, VehicleKind};

/// Ticks de simulación ≈ un día de calendario (alineado con el HUD del cliente).
pub const TICKS_PER_TRANSIT_DAY: u32 = 74;
/// Año simulado en ticks (365 días).
pub const TICKS_PER_YEAR: u64 = TICKS_PER_TRANSIT_DAY as u64 * 365;

/// Tasas base del clima templado de OpenTTD (`cargo_const.h`), sin inflación.
#[derive(Debug, Clone, Copy)]
pub struct CargoPaymentSpec {
    /// `CargoSpec::initial_payment`.
    pub base_rate: i32,
    /// `transit_periods[0]`: entrega rápida sin penalización.
    pub transit_fast_days: u16,
    /// `transit_periods[1]`: tramo lineal extra antes del mínimo.
    pub transit_slow_days: u16,
}

impl CargoType {
    #[must_use]
    pub const fn payment_spec(self) -> CargoPaymentSpec {
        match self {
            Self::Passengers => CargoPaymentSpec {
                base_rate: 3185,
                transit_fast_days: 0,
                transit_slow_days: 24,
            },
            Self::Mail => CargoPaymentSpec {
                base_rate: 4550,
                transit_fast_days: 20,
                transit_slow_days: 90,
            },
            Self::Coal => CargoPaymentSpec {
                base_rate: 5916,
                transit_fast_days: 7,
                transit_slow_days: 255,
            },
            Self::Wood => CargoPaymentSpec {
                base_rate: 5005,
                transit_fast_days: 15,
                transit_slow_days: 255,
            },
            Self::Oil => CargoPaymentSpec {
                base_rate: 4437,
                transit_fast_days: 25,
                transit_slow_days: 255,
            },
            Self::Goods => CargoPaymentSpec {
                base_rate: 6144,
                transit_fast_days: 5,
                transit_slow_days: 28,
            },
        }
    }
}

#[must_use]
pub const fn manhattan_distance(a: TileCoord, b: TileCoord) -> u32 {
    (a.x - b.x).unsigned_abs() + (a.y - b.y).unsigned_abs()
}

#[must_use]
pub const fn ticks_to_transit_days(ticks: u32) -> u16 {
    let days = ticks / TICKS_PER_TRANSIT_DAY;
    if days > u16::MAX as u32 {
        u16::MAX
    } else {
        days as u16
    }
}

/// Factor de inflación de ingresos (punto fijo /1024). Crece ~0,4 % por año simulado.
#[must_use]
pub fn inflation_income_factor(tick: u64) -> u32 {
    let years = tick / TICKS_PER_YEAR;
    1024 + (years as u32).saturating_mul(4)
}

const MIN_TIME_FACTOR: i32 = 31;
const MAX_TIME_FACTOR: i32 = 255;

#[must_use]
pub fn cargo_time_factor(transit_days: u16, spec: CargoPaymentSpec) -> i32 {
    let tp = i32::from(transit_days);
    let periods1 = i32::from(spec.transit_fast_days);
    let periods2 = i32::from(spec.transit_slow_days);
    let over1 = (tp - periods1).max(0);
    let over2 = (over1 - periods2).max(0);
    (MAX_TIME_FACTOR - over1 - over2).max(MIN_TIME_FACTOR)
}

/// Ingreso por entrega final (simplificación de OpenTTD `GetTransportedGoodsIncome`).
#[must_use]
pub fn transported_goods_income(
    count: u32,
    distance: u32,
    transit_days: u16,
    cargo: CargoType,
    tick: u64,
) -> i64 {
    if count == 0 {
        return 0;
    }
    let dist = distance.max(1);
    let spec = cargo.payment_spec();
    let time_factor = cargo_time_factor(transit_days, spec);
    let mut income =
        i64::from(dist) * i64::from(time_factor) * i64::from(count) * i64::from(spec.base_rate);
    income >>= 21;
    income = income.saturating_mul(i64::from(inflation_income_factor(tick))) / 1024;
    income.max(1)
}

/// Precio de compra en depósito (orden de magnitud TTD original).
#[must_use]
pub const fn vehicle_purchase_cost(kind: VehicleKind) -> i64 {
    match kind {
        VehicleKind::Bus => 8_192,
        VehicleKind::Truck => 11_939,
        VehicleKind::Train => 25_000,
    }
}

/// Reembolso al vender en depósito (~50 % del precio de compra).
#[must_use]
pub fn vehicle_sell_refund(vehicle: &Vehicle) -> i64 {
    let base = vehicle_purchase_cost(vehicle.kind);
    (base * 50) / 100
}

/// Coste de explotación por tick (solo en movimiento, como corrida en OpenTTD).
#[must_use]
pub const fn vehicle_running_cost_per_tick(kind: VehicleKind, running: bool, moving: bool) -> i64 {
    if !running || !moving {
        return 0;
    }
    match kind {
        VehicleKind::Bus => 2,
        VehicleKind::Truck => 3,
        VehicleKind::Train => 8,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coal_pays_more_than_passengers_on_same_route() {
        let dist = 12;
        let days = 3;
        let count = 10;
        let coal = transported_goods_income(count, dist, days, CargoType::Coal, 0);
        let pax = transported_goods_income(count, dist, days, CargoType::Passengers, 0);
        assert!(coal > pax, "carbón {coal} vs pasajeros {pax}");
    }

    #[test]
    fn longer_distance_pays_more() {
        let short = transported_goods_income(5, 4, 2, CargoType::Wood, 0);
        let long = transported_goods_income(5, 20, 2, CargoType::Wood, 0);
        assert!(long > short);
    }

    #[test]
    fn slow_transit_reduces_income() {
        let fast = transported_goods_income(8, 10, 2, CargoType::Goods, 0);
        let slow = transported_goods_income(8, 10, 80, CargoType::Goods, 0);
        assert!(fast > slow);
    }

    #[test]
    fn inflation_increases_income_over_years() {
        let base = transported_goods_income(10, 8, 4, CargoType::Coal, 0);
        let later = transported_goods_income(10, 8, 4, CargoType::Coal, TICKS_PER_YEAR * 10);
        assert!(later > base);
    }

    #[test]
    fn manhattan_distance_matches_expectation() {
        let a = TileCoord::new(3, 2);
        let b = TileCoord::new(6, 2);
        assert_eq!(manhattan_distance(a, b), 3);
    }
}
