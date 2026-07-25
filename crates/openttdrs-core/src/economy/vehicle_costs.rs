//! Costos de compra, venta y explotación de vehículos.

use super::global::GlobalEconomy;
use crate::engine::EngineDef;
use crate::train_consist::consist_unit_ids;
use crate::vehicle::{Vehicle, VehicleKind};

/// Ticks de calendario en un año (`365 * DAY_TICKS`).
pub const YEAR_TICKS: u64 = 27_010;

/// Precio de compra del motor por defecto del tipo (catálogo del original).
#[must_use]
pub fn vehicle_purchase_cost(kind: VehicleKind) -> i64 {
    crate::engine::engine_for_vehicle(kind, crate::engine::default_engine_id(kind)).price
}

/// Reembolso al vender en depósito (~50 % del precio del modelo del vehículo).
#[must_use]
pub fn vehicle_sell_refund(vehicle: &Vehicle) -> i64 {
    let base = vehicle.effective_engine().price;
    (base * 50) / 100
}

/// Valor contable del vehículo para `CalculateCompanyValue` (sin depreciación diaria aún).
#[must_use]
pub fn vehicle_asset_value(vehicle: &Vehicle) -> i64 {
    vehicle.effective_engine().price.max(1)
}

/// Coste de explotación anual del motor (`Engine::GetRunningCost` / catálogo).
#[must_use]
pub fn engine_running_cost_year(engine: &EngineDef) -> i64 {
    engine.running_cost_year.max(0)
}

/// Suma anual del consist (cada unidad con `running_cost_year`; cabeza dual ×½ en multihead).
#[must_use]
pub fn consist_running_cost_year(vehicles: &[Vehicle], head_id: u32) -> i64 {
    let mut total = 0_i64;
    for unit_id in consist_unit_ids(vehicles, head_id) {
        let Some(unit) = vehicles.iter().find(|v| v.id == unit_id) else {
            continue;
        };
        let mut yearly = engine_running_cost_year(unit.effective_engine());
        if unit.other_multiheaded_part.is_some() {
            yearly /= 2;
        }
        total = total.saturating_add(yearly);
    }
    total
}

/// ¿Este tick cuenta para `running_ticks`? Tren: no parado o aún en movimiento; resto: en servicio.
#[must_use]
pub fn vehicle_counts_running_tick(vehicle: &Vehicle) -> bool {
    match vehicle.kind {
        VehicleKind::Train => vehicle.running || vehicle.cur_speed > 0,
        _ => vehicle.running,
    }
}

/// Acumula coste fraccional y devuelve libras enteras a cobrar este tick.
pub fn accumulate_running_cost_for_head(vehicle: &mut Vehicle, yearly: i64) -> i64 {
    if !vehicle.is_consist_head() || !vehicle_counts_running_tick(vehicle) || yearly <= 0 {
        return 0;
    }
    vehicle.running_cost_accum = vehicle
        .running_cost_accum
        .saturating_add(u64::try_from(yearly).unwrap_or(u64::MAX));
    let charge = i64::try_from(vehicle.running_cost_accum / YEAR_TICKS).unwrap_or(i64::MAX);
    vehicle.running_cost_accum %= YEAR_TICKS;
    charge
}

/// Acumula coste fraccional leyendo el consist completo.
pub fn accumulate_vehicle_running_cost(vehicle: &mut Vehicle, vehicles: &[Vehicle]) -> i64 {
    let yearly = consist_running_cost_year(vehicles, vehicle.id);
    accumulate_running_cost_for_head(vehicle, yearly)
}

/// Coste por tick prorrateado (`coste * ticks / (365 * DAY_TICKS)`).
#[must_use]
pub fn vehicle_running_cost_per_tick(vehicles: &[Vehicle], vehicle: &Vehicle) -> i64 {
    if !vehicle.is_consist_head() || !vehicle_counts_running_tick(vehicle) {
        return 0;
    }
    let yearly = consist_running_cost_year(vehicles, vehicle.id);
    if yearly <= 0 {
        return 0;
    }
  yearly / i64::try_from(YEAR_TICKS).unwrap_or(1)
}

/// Coste anual vía tabla de precios + factor motor (paridad `Engine::GetRunningCost`).
#[must_use]
pub fn engine_running_cost_from_price_base(_ge: &GlobalEconomy, engine: &EngineDef) -> i64 {
    engine.running_cost_year
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::ENGINE_BUS_MPS;
    use crate::map::TileCoord;

    #[test]
    fn running_cost_prorates_yearly_catalog_cost() {
        let mut bus = Vehicle::new(1, VehicleKind::Bus, TileCoord::new(0, 0), TileCoord::new(1, 0));
        let yearly = engine_running_cost_year(bus.effective_engine());
        let mut total = 0_i64;
        for _ in 0..YEAR_TICKS {
            total += accumulate_running_cost_for_head(&mut bus, yearly);
        }
        assert_eq!(total, yearly);
    }

    #[test]
    fn stopped_bus_with_running_flag_still_costs() {
        let mut bus = Vehicle::new(2, VehicleKind::Bus, TileCoord::new(0, 0), TileCoord::new(1, 0));
        bus.running = true;
        bus.cur_speed = 0;
        let yearly = engine_running_cost_year(bus.effective_engine());
        assert!(yearly > 0);
        for _ in 0..100 {
            let _ = accumulate_running_cost_for_head(&mut bus, yearly);
        }
        assert!(bus.running_cost_accum > 0);
    }

    #[test]
    fn engine_running_cost_from_price_base_matches_catalog() {
        let ge = GlobalEconomy::new();
        let engine = crate::engine::engine_for_vehicle(VehicleKind::Bus, ENGINE_BUS_MPS);
        assert_eq!(
            engine_running_cost_from_price_base(&ge, engine),
            engine.running_cost_year
        );
    }
}
