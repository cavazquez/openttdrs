use crate::IndustrySpec;
use crate::map::TileCoord;

use super::layout_tables::{
    COAL_MINE_LAYOUTS, FACTORY_LAYOUTS, FARM_LAYOUTS, FOREST_LAYOUTS, GOLD_MINE_LAYOUTS,
    IRON_MINE_LAYOUTS, METAL_MINE_LAYOUTS, OIL_LAYOUTS, POWER_STATION_LAYOUTS, REFINERY_LAYOUTS,
    SAWMILL_LAYOUTS, STEEL_MILL_LAYOUTS,
};
use super::toyland_layout_tables::{
    BATTERY_FARM_LAYOUTS, BUBBLE_GENERATOR_LAYOUTS, CANDY_FACTORY_LAYOUTS, COLA_WELLS_LAYOUTS,
    COTTON_CANDY_LAYOUTS, FIZZY_DRINK_FACTORY_LAYOUTS, PLASTIC_FOUNTAIN_LAYOUTS,
    SUGAR_MINE_LAYOUTS, TOFFEE_QUARRY_LAYOUTS, TOY_FACTORY_LAYOUTS,
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
        IndustrySpec::GoldMine | IndustrySpec::DiamondMine => &GOLD_MINE_LAYOUTS,
        IndustrySpec::Forest | IndustrySpec::FruitPlantation | IndustrySpec::RubberPlantation => {
            &FOREST_LAYOUTS
        }
        IndustrySpec::Farm | IndustrySpec::FarmTropic => &FARM_LAYOUTS,
        IndustrySpec::OilWells | IndustrySpec::WaterSupply => &OIL_LAYOUTS,
        IndustrySpec::OilRefinery => &REFINERY_LAYOUTS,
        IndustrySpec::Factory
        | IndustrySpec::FactoryTropic
        | IndustrySpec::Bank
        | IndustrySpec::BankArcticTropic
        | IndustrySpec::PrintingWorks
        | IndustrySpec::FoodProcessingPlant
        | IndustrySpec::WaterTower
        | IndustrySpec::ToyShop => &FACTORY_LAYOUTS,
        IndustrySpec::SteelMill => &STEEL_MILL_LAYOUTS,
        IndustrySpec::Sawmill | IndustrySpec::PaperMill | IndustrySpec::LumberMill => {
            &SAWMILL_LAYOUTS
        }
        IndustrySpec::CottonCandy => &COTTON_CANDY_LAYOUTS,
        IndustrySpec::CandyFactory => &CANDY_FACTORY_LAYOUTS,
        IndustrySpec::BatteryFarm => &BATTERY_FARM_LAYOUTS,
        IndustrySpec::ColaWells => &COLA_WELLS_LAYOUTS,
        IndustrySpec::ToyFactory => &TOY_FACTORY_LAYOUTS,
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
    }
}
