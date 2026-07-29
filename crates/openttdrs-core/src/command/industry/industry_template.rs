use crate::IndustrySpec;
use crate::map::TileCoord;

use super::layout_tables::{
    COAL_MINE_LAYOUTS, FACTORY_LAYOUTS, FARM_LAYOUTS, FOREST_LAYOUTS, GOLD_MINE_LAYOUTS,
    IRON_MINE_LAYOUTS, METAL_MINE_LAYOUTS, OIL_LAYOUTS, POWER_STATION_LAYOUTS, REFINERY_LAYOUTS,
    SAWMILL_LAYOUTS,
};
use super::toyland_layout_tables::{
    BATTERY_FARM_LAYOUTS, BUBBLE_GENERATOR_LAYOUTS, CANDY_FACTORY_LAYOUTS, COLA_WELLS_LAYOUTS,
    COTTON_CANDY_LAYOUTS, FIZZY_DRINK_FACTORY_LAYOUTS, PLASTIC_FOUNTAIN_LAYOUTS,
    SUGAR_MINE_LAYOUTS, TOFFEE_QUARRY_LAYOUTS, TOY_FACTORY_LAYOUTS,
};

#[must_use]
pub fn industry_template(c: TileCoord, spec: IndustrySpec) -> Vec<(TileCoord, u8)> {
    let offsets_and_gfx = match spec {
        IndustrySpec::CoalMine => choose_layout(c, &COAL_MINE_LAYOUTS),
        IndustrySpec::PowerStation => choose_layout(c, &POWER_STATION_LAYOUTS),
        IndustrySpec::IronOreMine => choose_layout(c, &IRON_MINE_LAYOUTS),
        IndustrySpec::CopperOreMine => choose_layout(c, &METAL_MINE_LAYOUTS),
        IndustrySpec::GoldMine | IndustrySpec::DiamondMine => choose_layout(c, &GOLD_MINE_LAYOUTS),
        IndustrySpec::Forest
        | IndustrySpec::FruitPlantation
        | IndustrySpec::RubberPlantation => choose_layout(c, &FOREST_LAYOUTS),
        IndustrySpec::Farm | IndustrySpec::FarmTropic => choose_layout(c, &FARM_LAYOUTS),
        IndustrySpec::OilWells | IndustrySpec::WaterSupply => choose_layout(c, &OIL_LAYOUTS),
        IndustrySpec::OilRefinery => choose_layout(c, &REFINERY_LAYOUTS),
        IndustrySpec::Factory
        | IndustrySpec::FactoryTropic
        | IndustrySpec::SteelMill
        | IndustrySpec::Bank
        | IndustrySpec::BankArcticTropic
        | IndustrySpec::PrintingWorks
        | IndustrySpec::FoodProcessingPlant
        | IndustrySpec::WaterTower
        | IndustrySpec::ToyShop => choose_layout(c, &FACTORY_LAYOUTS),
        IndustrySpec::Sawmill | IndustrySpec::PaperMill | IndustrySpec::LumberMill => {
            choose_layout(c, &SAWMILL_LAYOUTS)
        }
        IndustrySpec::CottonCandy => choose_layout(c, &COTTON_CANDY_LAYOUTS),
        IndustrySpec::CandyFactory => choose_layout(c, &CANDY_FACTORY_LAYOUTS),
        IndustrySpec::BatteryFarm => choose_layout(c, &BATTERY_FARM_LAYOUTS),
        IndustrySpec::ColaWells => choose_layout(c, &COLA_WELLS_LAYOUTS),
        IndustrySpec::ToyFactory => choose_layout(c, &TOY_FACTORY_LAYOUTS),
        IndustrySpec::PlasticFountain => choose_layout(c, &PLASTIC_FOUNTAIN_LAYOUTS),
        IndustrySpec::FizzyDrinkFactory => choose_layout(c, &FIZZY_DRINK_FACTORY_LAYOUTS),
        IndustrySpec::BubbleGenerator => choose_layout(c, &BUBBLE_GENERATOR_LAYOUTS),
        IndustrySpec::ToffeeQuarry => choose_layout(c, &TOFFEE_QUARRY_LAYOUTS),
        IndustrySpec::SugarMine => choose_layout(c, &SUGAR_MINE_LAYOUTS),
    };

    offsets_and_gfx
        .iter()
        .map(|(dx, dy, m5)| (TileCoord::new(c.x + dx, c.y + dy), *m5))
        .collect()
}

fn choose_layout<'a>(c: TileCoord, layouts: &'a [&'a [(i32, i32, u8)]]) -> &'a [(i32, i32, u8)] {
    let seed = i64::from(c.x)
        .wrapping_mul(31)
        .wrapping_add(i64::from(c.y).wrapping_mul(17));
    let idx = usize::try_from(seed.unsigned_abs()).unwrap_or(0) % layouts.len();
    layouts[idx]
}
