use crate::map::TileCoord;
use crate::IndustrySpec;

use super::layout_tables::{
    COAL_MINE_LAYOUTS, FACTORY_LAYOUTS, FARM_LAYOUTS, FOREST_LAYOUTS, GOLD_MINE_LAYOUTS,
    IRON_MINE_LAYOUTS, METAL_MINE_LAYOUTS, OIL_LAYOUTS, REFINERY_LAYOUTS, SAWMILL_LAYOUTS,
};

#[must_use]
pub fn industry_template(c: TileCoord, spec: IndustrySpec) -> Vec<(TileCoord, u8)> {
    let offsets_and_gfx = match spec {
        IndustrySpec::CoalMine => choose_layout(c, &COAL_MINE_LAYOUTS),
        IndustrySpec::IronOreMine => choose_layout(c, &IRON_MINE_LAYOUTS),
        IndustrySpec::CopperOreMine => choose_layout(c, &METAL_MINE_LAYOUTS),
        IndustrySpec::GoldMine => choose_layout(c, &GOLD_MINE_LAYOUTS),
        IndustrySpec::Forest => choose_layout(c, &FOREST_LAYOUTS),
        IndustrySpec::Farm => choose_layout(c, &FARM_LAYOUTS),
        IndustrySpec::OilWells => choose_layout(c, &OIL_LAYOUTS),
        IndustrySpec::OilRefinery => choose_layout(c, &REFINERY_LAYOUTS),
        IndustrySpec::Factory => choose_layout(c, &FACTORY_LAYOUTS),
        IndustrySpec::Sawmill => choose_layout(c, &SAWMILL_LAYOUTS),
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
