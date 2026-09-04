//! Costos de compra, venta y explotación de vehículos.

use super::global::GlobalEconomy;
use crate::engine::EngineDef;
use crate::train_consist::consist_unit_ids;
use crate::vehicle::{Vehicle, VehicleKind};
use crate::{CargoType, Climate};

/// Ticks de calendario en un año (`365 * DAY_TICKS`).
pub const YEAR_TICKS: u64 = 27_010;

/// Precio de compra del motor por defecto del tipo (catálogo del original).
#[must_use]
pub fn vehicle_purchase_cost(kind: VehicleKind) -> i64 {
    crate::engine::engine_for_vehicle(kind, crate::engine::default_engine_id(kind)).price
}

fn purchase_price_base(engine: &EngineDef) -> i64 {
    match engine.kind {
        VehicleKind::Train if crate::train_consist::engine_is_wagon(engine) => 2_000,
        VehicleKind::Train => 400_000,
        VehicleKind::Bus | VehicleKind::Truck | VehicleKind::Tram => 14_000,
        VehicleKind::Ship => 65_000,
        VehicleKind::Aircraft => 700_000,
    }
}

fn running_price_base(engine: &EngineDef) -> i64 {
    match engine.kind {
        // NewGRF trains use the diesel running-cost class in the current
        // catalogue; vanilla engines without a runtime callback retain their
        // precomputed value through the fallback below.
        VehicleKind::Train => 5_200,
        VehicleKind::Bus | VehicleKind::Truck | VehicleKind::Tram => 1_600,
        VehicleKind::Ship => 5_600,
        VehicleKind::Aircraft => 9_600,
    }
}

fn refit_price_index(engine: &EngineDef) -> super::pricebase::PriceIndex {
    match engine.kind {
        VehicleKind::Train if crate::train_consist::engine_is_wagon(engine) => {
            super::pricebase::PriceIndex::BuildVehicleWagon
        }
        VehicleKind::Train => super::pricebase::PriceIndex::BuildVehicleTrain,
        VehicleKind::Bus | VehicleKind::Truck | VehicleKind::Tram => {
            super::pricebase::PriceIndex::BuildVehicleRoad
        }
        VehicleKind::Ship => super::pricebase::PriceIndex::BuildVehicleShip,
        VehicleKind::Aircraft => super::pricebase::PriceIndex::BuildVehicleAircraft,
    }
}

/// Coste/factor de conversión de carga de una unidad.
///
/// `CBID_VEHICLE_REFIT_COST` devuelve un factor signed de 14 bits y el bit 14
/// permite autorefit. Sin callback se conserva `EngineDef::refit_cost`; el
/// coste de trenes usa el doble del factor, como `GetRefitCost` upstream.
#[must_use]
pub fn vehicle_refit_cost_with_callbacks(
    ge: &GlobalEconomy,
    engine: &EngineDef,
    vehicle: &mut Vehicle,
    cargo: CargoType,
    new_subtype: u8,
    climate: Climate,
    cargo_catalog: &[crate::cargo_spec::CargoSpecDef],
) -> (i64, bool) {
    let (factor, auto_refit_allowed) = crate::newgrf_callback::resolve_vehicle_refit_cost_callback(
        engine,
        vehicle,
        cargo,
        new_subtype,
        climate,
        cargo_catalog,
    )
    .map_or_else(
        || {
            (
                if vehicle.cargo_type == Some(cargo) {
                    0
                } else {
                    i16::from(engine.refit_cost)
                },
                engine.refit_cost == 0,
            )
        },
        |result| (result.factor, result.auto_refit_allowed),
    );
    let mut factor = i64::from(factor);
    if engine.kind == VehicleKind::Train {
        factor = factor.saturating_mul(2);
    }
    let cost = super::pricebase::get_price(ge, refit_price_index(engine), factor, -10);
    (cost, auto_refit_allowed)
}

/// Precio de compra aplicando CB36 (`0x17`, `0x11`, `0x0A` o `0x0B`).
///
/// El callback devuelve el `cost_factor` BYTE, que se vuelve a combinar con
/// el precio base de `pricebase.h`. Si falla, se mantiene exactamente el
/// precio ya calculado del catálogo.
#[must_use]
pub fn vehicle_purchase_cost_with_callbacks(engine: &EngineDef, vehicle: &mut Vehicle) -> i64 {
    crate::newgrf_callback::vehicle_cost_factor(engine, vehicle, false)
        .map_or(engine.price, |factor| {
            (purchase_price_base(engine) * i64::from(factor)) >> 8
        })
}

/// Reembolso al vender en depósito (~50 % del precio del modelo del vehículo).
#[must_use]
pub fn vehicle_sell_refund(vehicle: &Vehicle) -> i64 {
    let base = vehicle.effective_engine().price;
    (base * 50) / 100
}

/// Reembolso usando el motor runtime y CB36. El callback se evalúa sobre una
/// copia: vender no debe mutar registros persistentes de la unidad que se va a
/// eliminar.
#[must_use]
pub fn vehicle_sell_refund_with_catalog(vehicle: &Vehicle, engine_catalog: &[EngineDef]) -> i64 {
    let Some(engine) = vehicle
        .engine_id
        .and_then(|id| crate::engine::engine_in_catalog(engine_catalog, id))
        .cloned()
    else {
        return vehicle_sell_refund(vehicle);
    };
    let mut snapshot = vehicle.clone();
    let base = vehicle_purchase_cost_with_callbacks(&engine, &mut snapshot);
    (base * 50) / 100
}

/// Valor contable del vehículo para `CalculateCompanyValue` (sin depreciación diaria aún).
#[must_use]
pub fn vehicle_asset_value(vehicle: &Vehicle) -> i64 {
    vehicle.effective_engine().price.max(1)
}

/// Valor contable con catálogo activo y CB36 de coste de compra.
#[must_use]
pub fn vehicle_asset_value_with_catalog(vehicle: &Vehicle, engine_catalog: &[EngineDef]) -> i64 {
    let Some(engine) = vehicle
        .engine_id
        .and_then(|id| crate::engine::engine_in_catalog(engine_catalog, id))
        .cloned()
    else {
        return vehicle_asset_value(vehicle);
    };
    let mut snapshot = vehicle.clone();
    vehicle_purchase_cost_with_callbacks(&engine, &mut snapshot).max(1)
}

/// Coste de explotación anual del motor (`Engine::GetRunningCost` / catálogo).
#[must_use]
pub fn engine_running_cost_year(engine: &EngineDef) -> i64 {
    engine.running_cost_year.max(0)
}

/// Coste anual aplicando CB36 de factor de explotación.
#[must_use]
pub fn engine_running_cost_year_with_callbacks(engine: &EngineDef, vehicle: &mut Vehicle) -> i64 {
    crate::newgrf_callback::vehicle_cost_factor(engine, vehicle, true).map_or_else(
        || engine_running_cost_year(engine),
        |factor| (running_price_base(engine) * i64::from(factor)) >> 8,
    )
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

/// Suma anual de un consist usando el catálogo activo y los factores CB36.
///
/// Se conserva una variante separada para que las APIs públicas históricas
/// sigan siendo inmutables y determininistas con el catálogo vanilla.
pub fn consist_running_cost_year_with_catalog(
    vehicles: &mut [Vehicle],
    head_id: u32,
    engine_catalog: &[EngineDef],
) -> i64 {
    let mut total = 0_i64;
    for unit_id in consist_unit_ids(vehicles, head_id) {
        let Some(unit) = vehicles.iter_mut().find(|v| v.id == unit_id) else {
            continue;
        };
        let Some(engine) = unit
            .engine_id
            .and_then(|id| crate::engine::engine_in_catalog(engine_catalog, id))
            .cloned()
        else {
            let mut yearly = engine_running_cost_year(unit.effective_engine());
            if unit.other_multiheaded_part.is_some() {
                yearly /= 2;
            }
            total = total.saturating_add(yearly);
            continue;
        };
        let mut yearly = engine_running_cost_year_with_callbacks(&engine, unit);
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

    fn callback_runtime_literal(value: u8) -> crate::newgrf_sprites::TrainSpriteGraphics {
        use crate::newgrf_sprites::{
            Action2VarAdjust, Action2VarEntry, Action2VarTerm, TrainSpriteAssign,
        };

        let mut gfx = crate::newgrf_sprites::TrainSpriteGraphics::default();
        gfx.assigns.push(TrainSpriteAssign {
            local_id: 0,
            set_id: 2,
        });
        gfx.action2_var.insert(
            2,
            Action2VarEntry {
                first: Action2VarTerm {
                    variable: 0x1A,
                    param: None,
                    adjust: Action2VarAdjust {
                        and_mask: u32::from(value),
                        ..Action2VarAdjust::default()
                    },
                },
                ops: Vec::new(),
                ranges: Vec::new(),
                default: 0,
            },
        );
        gfx
    }

    fn callback_runtime_literal_u16(value: u16) -> crate::newgrf_sprites::TrainSpriteGraphics {
        use crate::newgrf_sprites::{
            Action2VarAdjust, Action2VarEntry, Action2VarTerm, TrainSpriteAssign,
        };

        let mut gfx = crate::newgrf_sprites::TrainSpriteGraphics::default();
        gfx.assigns.push(TrainSpriteAssign {
            local_id: 0,
            set_id: 2,
        });
        gfx.action2_var.insert(
            2,
            Action2VarEntry {
                first: Action2VarTerm {
                    variable: 0x1A,
                    param: None,
                    adjust: Action2VarAdjust {
                        and_mask: u32::from(value),
                        ..Action2VarAdjust::default()
                    },
                },
                ops: Vec::new(),
                ranges: Vec::new(),
                default: 0,
            },
        );
        gfx
    }

    #[test]
    fn running_cost_prorates_yearly_catalog_cost() {
        let mut bus = Vehicle::new(
            1,
            VehicleKind::Bus,
            TileCoord::new(0, 0),
            TileCoord::new(1, 0),
        );
        let yearly = engine_running_cost_year(bus.effective_engine());
        let mut total = 0_i64;
        for _ in 0..YEAR_TICKS {
            total += accumulate_running_cost_for_head(&mut bus, yearly);
        }
        assert_eq!(total, yearly);
    }

    #[test]
    fn stopped_bus_with_running_flag_still_costs() {
        let mut bus = Vehicle::new(
            2,
            VehicleKind::Bus,
            TileCoord::new(0, 0),
            TileCoord::new(1, 0),
        );
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

    #[test]
    #[allow(clippy::unwrap_used)]
    fn cb36_cost_factors_override_purchase_and_running_costs() {
        let mut engine = crate::engine::engines_table()
            .iter()
            .find(|candidate| candidate.kind == VehicleKind::Bus)
            .cloned()
            .unwrap();
        engine.id = 65_102;
        engine.newgrf_grfid = 0x434F_5354;
        engine.newgrf_local_id = 0;
        engine.newgrf_runtime = Some(Box::new(callback_runtime_literal(64)));
        let mut vehicle = Vehicle::new(
            3,
            VehicleKind::Bus,
            TileCoord::new(0, 0),
            TileCoord::new(1, 0),
        );
        vehicle.engine_id = Some(engine.id);

        // `0x40 / 0x100` is applied to the OpenTTD road bases 14,000 and
        // 1,600, independently of the Action0 factors stored in `engine`.
        assert_eq!(
            vehicle_purchase_cost_with_callbacks(&engine, &mut vehicle),
            (14_000_i64 * 64) >> 8
        );
        assert_eq!(
            engine_running_cost_year_with_callbacks(&engine, &mut vehicle),
            (1_600_i64 * 64) >> 8
        );

        engine.newgrf_runtime = None;
        assert_eq!(
            vehicle_purchase_cost_with_callbacks(&engine, &mut vehicle),
            engine.price
        );
        assert_eq!(
            engine_running_cost_year_with_callbacks(&engine, &mut vehicle),
            engine.running_cost_year
        );
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn refit_cost_uses_action0_fallback_and_cb15e_autorefit_flag() {
        let ge = GlobalEconomy::new();
        let mut engine = crate::engine::engines_table()
            .iter()
            .find(|candidate| candidate.kind == VehicleKind::Bus)
            .cloned()
            .unwrap();
        engine.id = 65_103;
        engine.refit_cost = 4;
        let mut vehicle = Vehicle::new(
            4,
            VehicleKind::Bus,
            TileCoord::new(0, 0),
            TileCoord::new(1, 0),
        );
        vehicle.engine_id = Some(engine.id);
        vehicle.cargo_type = Some(CargoType::Passengers);

        let (fallback_cost, fallback_auto) = vehicle_refit_cost_with_callbacks(
            &ge,
            &engine,
            &mut vehicle,
            CargoType::Coal,
            0,
            Climate::Temperate,
            &[],
        );
        assert_eq!(
            fallback_cost,
            super::super::pricebase::get_price(
                &ge,
                super::super::pricebase::PriceIndex::BuildVehicleRoad,
                4,
                -10
            )
        );
        assert!(!fallback_auto);

        engine.newgrf_grfid = 0x5243_4F53;
        engine.newgrf_runtime = Some(Box::new(callback_runtime_literal_u16(0x4000 | 6)));
        let (callback_cost, callback_auto) = vehicle_refit_cost_with_callbacks(
            &ge,
            &engine,
            &mut vehicle,
            CargoType::Coal,
            0,
            Climate::Temperate,
            &[],
        );
        assert_eq!(
            callback_cost,
            super::super::pricebase::get_price(
                &ge,
                super::super::pricebase::PriceIndex::BuildVehicleRoad,
                6,
                -10
            )
        );
        assert!(callback_auto);
    }
}
