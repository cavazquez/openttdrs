use crate::IndustrySpec;
use crate::map::TileCoord;

use super::layout_tables::{
    BANK_LAYOUTS, BANK2_LAYOUTS, COAL_MINE_LAYOUTS, DIAMOND_MINE_LAYOUTS, FACTORY_LAYOUTS,
    FACTORY_TROPIC_LAYOUTS, FARM_LAYOUTS, FARM_TROPIC_LAYOUTS, FOOD_PROCESS_LAYOUTS,
    FOREST_LAYOUTS, FRUIT_PLANTATION_LAYOUTS, GOLD_MINE_LAYOUTS, IRON_MINE_LAYOUTS,
    LUMBER_MILL_LAYOUTS, METAL_MINE_LAYOUTS, OIL_LAYOUTS, PAPER_MILL_LAYOUTS,
    POWER_STATION_LAYOUTS, PRINTING_WORKS_LAYOUTS, REFINERY_LAYOUTS, RUBBER_PLANTATION_LAYOUTS,
    SAWMILL_LAYOUTS, STEEL_MILL_LAYOUTS, WATER_SUPPLY_LAYOUTS, WATER_TOWER_LAYOUTS,
};
use super::toyland_layout_tables::{
    BATTERY_FARM_LAYOUTS, BUBBLE_GENERATOR_LAYOUTS, CANDY_FACTORY_LAYOUTS, COLA_WELLS_LAYOUTS,
    COTTON_CANDY_LAYOUTS, FIZZY_DRINK_FACTORY_LAYOUTS, PLASTIC_FOUNTAIN_LAYOUTS,
    SUGAR_MINE_LAYOUTS, TOFFEE_QUARRY_LAYOUTS, TOY_FACTORY_LAYOUTS, TOY_SHOP_LAYOUTS,
};

#[must_use]
pub fn industry_template(c: TileCoord, spec: IndustrySpec) -> Vec<(TileCoord, u8)> {
    let layouts = layouts_for_spec(spec);
    let offsets_and_gfx = choose_layout(c, layouts);

    offsets_and_gfx
        .iter()
        .map(|(dx, dy, m5)| (TileCoord::new(c.x + dx, c.y + dy), *m5))
        .collect()
}

/// Materializa exactamente el layout ya sorteado por `CreateNewIndustry`.
///
/// La interfaz manual conserva [`industry_template`], pero el generador de
/// mundo debe usar esta variante para no sustituir `RandomRange(layouts)` por
/// una función de la coordenada.
#[must_use]
pub fn industry_template_with_layout(
    c: TileCoord,
    spec: IndustrySpec,
    layout_index: usize,
) -> Option<Vec<(TileCoord, u8)>> {
    let offsets_and_gfx = layouts_for_spec(spec).get(layout_index)?;

    Some(
        offsets_and_gfx
            .iter()
            .map(|(dx, dy, m5)| (TileCoord::new(c.x + dx, c.y + dy), *m5))
            .collect(),
    )
}

/// Número de layouts vanilla habilitados para la especie.
#[must_use]
pub fn industry_template_layout_count(spec: IndustrySpec) -> usize {
    layouts_for_spec(spec).len()
}

type IndustryLayout = &'static [(i32, i32, u8)];

fn layouts_for_spec(spec: IndustrySpec) -> &'static [IndustryLayout] {
    match spec {
        IndustrySpec::CoalMine => &COAL_MINE_LAYOUTS,
        IndustrySpec::PowerStation => &POWER_STATION_LAYOUTS,
        IndustrySpec::IronOreMine => &IRON_MINE_LAYOUTS,
        IndustrySpec::CopperOreMine => &METAL_MINE_LAYOUTS,
        IndustrySpec::GoldMine => &GOLD_MINE_LAYOUTS,
        IndustrySpec::DiamondMine => &DIAMOND_MINE_LAYOUTS,
        IndustrySpec::Forest => &FOREST_LAYOUTS,
        IndustrySpec::FruitPlantation => &FRUIT_PLANTATION_LAYOUTS,
        IndustrySpec::RubberPlantation => &RUBBER_PLANTATION_LAYOUTS,
        IndustrySpec::Farm => &FARM_LAYOUTS,
        IndustrySpec::FarmTropic => &FARM_TROPIC_LAYOUTS,
        IndustrySpec::OilWells => &OIL_LAYOUTS,
        IndustrySpec::WaterSupply => &WATER_SUPPLY_LAYOUTS,
        IndustrySpec::OilRefinery => &REFINERY_LAYOUTS,
        IndustrySpec::Factory => &FACTORY_LAYOUTS,
        IndustrySpec::FactoryTropic => &FACTORY_TROPIC_LAYOUTS,
        IndustrySpec::Bank => &BANK_LAYOUTS,
        IndustrySpec::BankArcticTropic => &BANK2_LAYOUTS,
        IndustrySpec::PrintingWorks => &PRINTING_WORKS_LAYOUTS,
        IndustrySpec::FoodProcessingPlant => &FOOD_PROCESS_LAYOUTS,
        IndustrySpec::WaterTower => &WATER_TOWER_LAYOUTS,
        IndustrySpec::SteelMill => &STEEL_MILL_LAYOUTS,
        IndustrySpec::Sawmill => &SAWMILL_LAYOUTS,
        IndustrySpec::PaperMill => &PAPER_MILL_LAYOUTS,
        IndustrySpec::LumberMill => &LUMBER_MILL_LAYOUTS,
        IndustrySpec::CottonCandy => &COTTON_CANDY_LAYOUTS,
        IndustrySpec::CandyFactory => &CANDY_FACTORY_LAYOUTS,
        IndustrySpec::BatteryFarm => &BATTERY_FARM_LAYOUTS,
        IndustrySpec::ColaWells => &COLA_WELLS_LAYOUTS,
        IndustrySpec::ToyFactory => &TOY_FACTORY_LAYOUTS,
        IndustrySpec::ToyShop => &TOY_SHOP_LAYOUTS,
        IndustrySpec::PlasticFountain => &PLASTIC_FOUNTAIN_LAYOUTS,
        IndustrySpec::FizzyDrinkFactory => &FIZZY_DRINK_FACTORY_LAYOUTS,
        IndustrySpec::BubbleGenerator => &BUBBLE_GENERATOR_LAYOUTS,
        IndustrySpec::ToffeeQuarry => &TOFFEE_QUARRY_LAYOUTS,
        IndustrySpec::SugarMine => &SUGAR_MINE_LAYOUTS,
    }
}

fn choose_layout<'a>(c: TileCoord, layouts: &'a [&'a [(i32, i32, u8)]]) -> &'a [(i32, i32, u8)] {
    let seed = i64::from(c.x)
        .wrapping_mul(31)
        .wrapping_add(i64::from(c.y).wrapping_mul(17));
    let idx = usize::try_from(seed.unsigned_abs()).unwrap_or(0) % layouts.len();
    layouts[idx]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selected_vanilla_layouts_keep_native_counts_and_tiles() {
        let origin = TileCoord::new(20, 30);
        assert_eq!(industry_template_layout_count(IndustrySpec::CoalMine), 4);
        assert_eq!(
            industry_template_layout_count(IndustrySpec::PowerStation),
            3
        );
        assert_eq!(industry_template_layout_count(IndustrySpec::SteelMill), 2);
        assert_eq!(
            industry_template_layout_count(IndustrySpec::PrintingWorks),
            2
        );
        assert_eq!(
            industry_template_layout_count(IndustrySpec::FoodProcessingPlant),
            2
        );
        assert_eq!(industry_template_layout_count(IndustrySpec::PaperMill), 1);
        assert_eq!(
            industry_template_layout_count(IndustrySpec::BankArcticTropic),
            1
        );
        assert_eq!(industry_template_layout_count(IndustrySpec::DiamondMine), 1);
        assert_eq!(
            industry_template_layout_count(IndustrySpec::FactoryTropic),
            2
        );
        assert_eq!(industry_template_layout_count(IndustrySpec::CottonCandy), 2);
        assert_eq!(
            industry_template_layout_count(IndustrySpec::CandyFactory),
            2
        );
        assert_eq!(industry_template_layout_count(IndustrySpec::ColaWells), 2);
        assert_eq!(industry_template_layout_count(IndustrySpec::ToyShop), 1);
        assert_eq!(
            industry_template_layout_count(IndustrySpec::PlasticFountain),
            2
        );

        let coal =
            industry_template_with_layout(origin, IndustrySpec::CoalMine, 0).unwrap_or_default();
        assert_eq!(coal.len(), 6);
        assert_eq!(coal[0], (TileCoord::new(21, 31), 0));
        assert_eq!(coal[2], (origin, 5));

        let power = industry_template_with_layout(origin, IndustrySpec::PowerStation, 2)
            .unwrap_or_default();
        assert_eq!(power.len(), 6);
        assert_eq!(power[4], (TileCoord::new(22, 30), 10));

        let steel =
            industry_template_with_layout(origin, IndustrySpec::SteelMill, 1).unwrap_or_default();
        assert_eq!(steel.len(), 14);
        assert_eq!(steel[13], (TileCoord::new(22, 33), 57));
        assert!(industry_template_with_layout(origin, IndustrySpec::CoalMine, 4).is_none());

        let printing = industry_template_with_layout(origin, IndustrySpec::PrintingWorks, 0)
            .unwrap_or_default();
        assert_eq!(printing.len(), 12);
        assert_eq!(printing[0], (origin, 43));
        assert_eq!(printing[11], (TileCoord::new(23, 32), 46));

        let food = industry_template_with_layout(origin, IndustrySpec::FoodProcessingPlant, 1)
            .unwrap_or_default();
        assert_eq!(food.len(), 14);
        assert_eq!(food[0], (origin, 61));
        assert_eq!(food[13], (TileCoord::new(21, 33), 62));

        let paper =
            industry_template_with_layout(origin, IndustrySpec::PaperMill, 0).unwrap_or_default();
        assert_eq!(paper.len(), 12);
        assert_eq!(paper[11], (TileCoord::new(23, 32), 70));

        let bank = industry_template_with_layout(origin, IndustrySpec::BankArcticTropic, 0)
            .unwrap_or_default();
        assert_eq!(bank, vec![(origin, 89), (TileCoord::new(21, 30), 90)]);

        let diamond =
            industry_template_with_layout(origin, IndustrySpec::DiamondMine, 0).unwrap_or_default();
        assert_eq!(diamond.len(), 9);
        assert_eq!(diamond[8], (TileCoord::new(22, 32), 99));

        let factory_tropic = industry_template_with_layout(origin, IndustrySpec::FactoryTropic, 0)
            .unwrap_or_default();
        assert_eq!(factory_tropic.len(), 8);
        assert_eq!(factory_tropic[7], (TileCoord::new(21, 33), 124));

        let toy_shop =
            industry_template_with_layout(origin, IndustrySpec::ToyShop, 0).unwrap_or_default();
        assert_eq!(
            toy_shop,
            vec![
                (origin, 138),
                (TileCoord::new(20, 31), 139),
                (TileCoord::new(21, 30), 140),
                (TileCoord::new(21, 31), 141)
            ]
        );

        let plastic_horizontal =
            industry_template_with_layout(origin, IndustrySpec::PlasticFountain, 1)
                .unwrap_or_default();
        assert_eq!(plastic_horizontal.len(), 3);
        assert_eq!(plastic_horizontal[2], (TileCoord::new(22, 30), 154));
    }
}
