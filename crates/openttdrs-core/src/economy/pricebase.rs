//! Tabla de precios base (`table/pricebase.h` + `RecomputePrices`, `economy.cpp:733-785`).
//!
//! Pendiente de migrar a `GetPrice`: depósitos, túneles/puentes, señales, clear-tile,
//! industrias, acciones de pueblo, infraestructura de mantenimiento y compra de vehículos
//! en comandos que aún usan constantes fijas o `engine.price` sin índice.

use super::global::GlobalEconomy;

/// Índices de `_price` usados en el port (orden = `pricebase.h`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum PriceIndex {
    StationValue = 0,
    BuildRail = 1,
    BuildRoad = 2,
    BuildSignals = 3,
    BuildBridge = 4,
    BuildDepotTrain = 5,
    BuildDepotRoad = 6,
    BuildDepotShip = 7,
    BuildTunnel = 8,
    BuildStationRail = 9,
    BuildStationRailLength = 10,
    BuildStationAirport = 11,
    BuildStationBus = 12,
    BuildStationTruck = 13,
    BuildStationDock = 14,
    BuildVehicleTrain = 15,
    BuildVehicleWagon = 16,
    BuildVehicleAircraft = 17,
    BuildVehicleRoad = 18,
    BuildVehicleShip = 19,
    Terraform = 21,
    ClearStationBus = 36,
    ClearStationTruck = 37,
    BuildObject = 50,
    RunningTrainSteam = 41,
    RunningTrainDiesel = 42,
    RunningTrainElectric = 43,
    RunningAircraft = 44,
    RunningRoadveh = 45,
    RunningShip = 46,
    BuildWaypointRail = 56,
}

impl PriceIndex {
    const fn spec_index(self) -> usize {
        self as usize
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PriceCategory {
    None,
    Construction,
    Running,
}

#[derive(Clone, Copy)]
struct PriceBaseSpec {
    start_price: i32,
    category: PriceCategory,
}

/// Tabla vanilla (`pricebase.h`); entradas omitidas usan precio 0 y no se consultan.
const PRICE_TABLE_LEN: usize = 82;

const fn spec(start_price: i32, category: PriceCategory) -> PriceBaseSpec {
    PriceBaseSpec {
        start_price,
        category,
    }
}

/// Specs indexadas por valor numérico de [`PriceIndex`] y huecos intermedios.
const PRICE_BASE_SPECS: [PriceBaseSpec; PRICE_TABLE_LEN] = {
    let none = spec(0, PriceCategory::None);
    let mut table = [none; PRICE_TABLE_LEN];
    table[0] = spec(100, PriceCategory::None);
    table[1] = spec(100, PriceCategory::Construction);
    table[2] = spec(95, PriceCategory::Construction);
    table[3] = spec(65, PriceCategory::Construction);
    table[4] = spec(275, PriceCategory::Construction);
    table[5] = spec(600, PriceCategory::Construction);
    table[6] = spec(500, PriceCategory::Construction);
    table[7] = spec(700, PriceCategory::Construction);
    table[8] = spec(450, PriceCategory::Construction);
    table[9] = spec(200, PriceCategory::Construction);
    table[10] = spec(180, PriceCategory::Construction);
    table[11] = spec(600, PriceCategory::Construction);
    table[12] = spec(200, PriceCategory::Construction);
    table[13] = spec(200, PriceCategory::Construction);
    table[14] = spec(350, PriceCategory::Construction);
    table[15] = spec(400_000, PriceCategory::Construction);
    table[16] = spec(2_000, PriceCategory::Construction);
    table[17] = spec(700_000, PriceCategory::Construction);
    table[18] = spec(14_000, PriceCategory::Construction);
    table[19] = spec(65_000, PriceCategory::Construction);
    table[21] = spec(250, PriceCategory::Construction);
    table[36] = spec(50, PriceCategory::Construction);
    table[37] = spec(50, PriceCategory::Construction);
    table[41] = spec(5_600, PriceCategory::Running);
    table[42] = spec(5_200, PriceCategory::Running);
    table[43] = spec(4_800, PriceCategory::Running);
    table[44] = spec(9_600, PriceCategory::Running);
    table[45] = spec(1_600, PriceCategory::Running);
    table[46] = spec(5_600, PriceCategory::Running);
    table[50] = spec(40, PriceCategory::Construction);
    table[56] = spec(600, PriceCategory::Construction);
    table
};

/// Multiplicador de dificultad: 0 → ×6, 1 → ×8 (media), 2 → ×9.
#[must_use]
pub const fn difficulty_multiplier(mod_setting: u8) -> i32 {
    match mod_setting {
        0 => 6,
        2 => 9,
        _ => 8,
    }
}

/// Precio base tras `RecomputePrices` (sin `cost_factor` ni `shift` de `GetPrice`).
#[must_use]
pub fn base_price_at(
    index: PriceIndex,
    inflation_prices: u64,
    construction_cost: u8,
    vehicle_costs: u8,
) -> i64 {
    let spec_idx = index.spec_index();
    if spec_idx >= PRICE_BASE_SPECS.len() {
        return 0;
    }
    let spec = PRICE_BASE_SPECS[spec_idx];
    if spec.start_price == 0 {
        return 0;
    }

    let mut price = i64::from(spec.start_price);
    let mod_setting = match spec.category {
        PriceCategory::Running => vehicle_costs,
        PriceCategory::Construction => construction_cost,
        PriceCategory::None => super::global::DEFAULT_DIFFICULTY_MOD,
    };
    price = price.saturating_mul(i64::from(difficulty_multiplier(mod_setting)));

    price = price.saturating_mul(i64::try_from(inflation_prices).unwrap_or(i64::MAX));

    // Sin multiplicadores NewGRF (`_price_base_multiplier` = 0): shift = -16 - 3.
    let shift = -19_i32;
    if shift >= 0 {
        price <<= shift;
    } else {
        price >>= (-shift).cast_unsigned();
    }

    if price == 0 {
        price = i64::from(spec.start_price.clamp(-1, 1));
    }
    price
}

/// `GetPrice` (`economy.cpp:936-949`).
#[must_use]
pub fn get_price(ge: &GlobalEconomy, index: PriceIndex, cost_factor: i64, shift: i32) -> i64 {
    let base = base_price_at(
        index,
        ge.inflation_prices,
        ge.construction_cost,
        ge.vehicle_costs,
    );
    let mut cost = base.saturating_mul(cost_factor);
    if shift >= 0 {
        cost <<= shift.cast_unsigned();
    } else {
        cost >>= (-shift).cast_unsigned();
    }
    cost
}

/// Precio de referencia en dificultad media sin inflación (tests y alias legacy).
#[must_use]
pub const fn medium_default_price(index: PriceIndex) -> i64 {
    let spec_idx = index as usize;
    if spec_idx >= PRICE_BASE_SPECS.len() {
        return 0;
    }
    PRICE_BASE_SPECS[spec_idx].start_price as i64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::economy::global::INFLATION_FRAC_ONE;
    use crate::linkgraph_parity::Randomizer;
    use crate::news::CALENDAR_BASE_YEAR;

    #[test]
    fn price_scales_with_difficulty_mod() {
        let ge_low = GlobalEconomy {
            construction_cost: 0,
            ..GlobalEconomy::new()
        };
        let ge_high = GlobalEconomy {
            construction_cost: 2,
            ..GlobalEconomy::new()
        };
        let low = get_price(&ge_low, PriceIndex::BuildRail, 1, 0);
        let mid = get_price(&GlobalEconomy::new(), PriceIndex::BuildRail, 1, 0);
        let high = get_price(&ge_high, PriceIndex::BuildRail, 1, 0);
        assert_eq!(mid, 100);
        assert!(low < mid);
        assert!(high > mid);
        assert_eq!(low * 8, mid * 6);
        assert!((high * 8).abs_diff(mid * 9) <= 8);
    }

    #[test]
    fn price_inflation_scales_build_rail() {
        let mut ge = GlobalEconomy::new();
        for _ in 0..24 {
            ge.add_monthly_inflation(CALENDAR_BASE_YEAR, true);
        }
        let base = get_price(&GlobalEconomy::new(), PriceIndex::BuildRail, 1, 0);
        let inflated = get_price(&ge, PriceIndex::BuildRail, 1, 0);
        assert!(inflated > base);
    }

    #[test]
    fn station_value_matches_original_table() {
        let ge = GlobalEconomy::new();
        assert_eq!(get_price(&ge, PriceIndex::StationValue, 1, 0), 100);
        assert_eq!(get_price(&ge, PriceIndex::StationValue, 1, 0) >> 2, 25);
    }

    #[test]
    fn terraform_and_road_use_price_base() {
        let ge = GlobalEconomy::new();
        assert_eq!(get_price(&ge, PriceIndex::Terraform, 1, 0), 250);
        assert_eq!(get_price(&ge, PriceIndex::BuildRoad, 1, 0), 95);
    }

    #[test]
    fn get_price_applies_cost_factor_and_shift() {
        let ge = GlobalEconomy::new();
        let base = get_price(&ge, PriceIndex::RunningRoadveh, 1, 0);
        assert_eq!(base, 1_600);
        assert_eq!(get_price(&ge, PriceIndex::RunningRoadveh, 91, -8), 568);
    }

    #[test]
    fn startup_inflation_increases_prices() {
        let mut ge = GlobalEconomy::new();
        ge.startup(&mut Randomizer::new(1), CALENDAR_BASE_YEAR);
        let fresh = get_price(&GlobalEconomy::new(), PriceIndex::BuildStationRail, 1, 0);
        let warmed = get_price(&ge, PriceIndex::BuildStationRail, 1, 0);
        assert!(warmed > fresh);
        assert_eq!(fresh, 200);
        let _ = INFLATION_FRAC_ONE;
    }
}
