use crate::map::{TileCoord, TileKind};
use crate::{GameState, Industry, IndustryKind, IndustrySpec};

use super::transport::{build_error_for_kind, transport_tile_is_buildable};
use super::{CommandError, in_bounds};

pub(super) fn place_industry_sandbox(
    state: &mut GameState,
    c: TileCoord,
) -> Result<(), CommandError> {
    place_industry_spec_sandbox(state, c, IndustrySpec::Factory)
}

pub(super) fn place_industry_kind_sandbox(
    state: &mut GameState,
    c: TileCoord,
    kind: IndustryKind,
) -> Result<(), CommandError> {
    let spec = match kind {
        IndustryKind::CoalMine => IndustrySpec::CoalMine,
        IndustryKind::Forest => IndustrySpec::Forest,
        IndustryKind::OilWell => IndustrySpec::OilWells,
        IndustryKind::Factory => IndustrySpec::Factory,
    };
    place_industry_spec_sandbox(state, c, spec)
}

pub(super) fn place_industry_spec_sandbox(
    state: &mut GameState,
    c: TileCoord,
    spec: IndustrySpec,
) -> Result<(), CommandError> {
    let template = industry_template(c, spec);
    for (tile, _) in &template {
        in_bounds(&state.map, *tile)?;
        let existing_kind = state.map.get_kind(*tile).unwrap_or(TileKind::Grass);
        if !transport_tile_is_buildable(existing_kind) {
            return Err(build_error_for_kind(existing_kind));
        }
    }
    let footprint: Vec<TileCoord> = template.iter().map(|(tile, _)| *tile).collect();
    for (tile, m5) in &template {
        state
            .map
            .set_kind(*tile, TileKind::Industry)
            .map_err(|_| CommandError::OutOfBounds)?;
        state
            .map
            .set_mapt_m5(*tile, 0x80, *m5)
            .map_err(|_| CommandError::OutOfBounds)?;
    }
    state
        .industries
        .retain(|industry| !industry.contains_tile(c));
    state
        .industries
        .push(Industry::with_tiles_spec(c, spec.kind(), spec, footprint));
    state.economy.money -= 250;
    Ok(())
}

#[must_use]
pub fn industry_template(c: TileCoord, spec: IndustrySpec) -> Vec<(TileCoord, u8)> {
    const COAL_MINE_LAYOUTS: [&[(i32, i32, u8)]; 4] = [
        // OpenTTD _tile_table_coal_mine_0.
        &[
            (1, 1, 0),
            (1, 2, 2),
            (0, 0, 5),
            (1, 0, 6),
            (2, 0, 3),
            (2, 2, 3),
        ],
        // OpenTTD _tile_table_coal_mine_1.
        &[
            (1, 1, 0),
            (1, 2, 2),
            (2, 0, 0),
            (2, 1, 2),
            (1, 0, 3),
            (0, 0, 3),
            (0, 1, 4),
            (0, 2, 4),
            (2, 2, 4),
        ],
        // OpenTTD _tile_table_coal_mine_2.
        &[
            (0, 0, 0),
            (0, 1, 2),
            (0, 2, 5),
            (1, 0, 3),
            (1, 1, 3),
            (1, 2, 6),
        ],
        // OpenTTD _tile_table_coal_mine_3.
        &[
            (0, 1, 0),
            (0, 2, 2),
            (0, 3, 4),
            (1, 0, 5),
            (1, 1, 0),
            (1, 2, 2),
            (1, 3, 3),
            (2, 0, 6),
            (2, 1, 4),
            (2, 2, 3),
        ],
    ];
    const METAL_MINE_LAYOUTS: [&[(i32, i32, u8)]; 2] = [
        // OpenTTD _tile_table_copper_mine_0.
        &[
            (0, 0, 47),
            (0, 1, 49),
            (0, 2, 51),
            (1, 0, 47),
            (1, 1, 49),
            (1, 2, 50),
            (2, 0, 51),
            (2, 1, 51),
        ],
        // OpenTTD _tile_table_copper_mine_1.
        &[
            (0, 0, 50),
            (0, 1, 47),
            (0, 2, 49),
            (1, 0, 47),
            (1, 1, 49),
            (1, 2, 51),
            (2, 0, 51),
            (2, 1, 47),
            (2, 2, 49),
        ],
    ];
    const GOLD_MINE_LAYOUTS: [&[(i32, i32, u8)]; 1] = [
        // OpenTTD _tile_table_gold_mine_0.
        &[
            (0, 0, 72),
            (0, 1, 73),
            (0, 2, 74),
            (0, 3, 75),
            (1, 0, 76),
            (1, 1, 77),
            (1, 2, 78),
            (1, 3, 79),
            (2, 0, 80),
            (2, 1, 81),
            (2, 2, 82),
            (2, 3, 83),
            (3, 0, 84),
            (3, 1, 85),
            (3, 2, 86),
            (3, 3, 87),
        ],
    ];
    const FOREST_LAYOUTS: [&[(i32, i32, u8)]; 2] = [
        // OpenTTD _tile_table_forest_0.
        &[
            (0, 0, 16),
            (0, 1, 16),
            (0, 2, 16),
            (0, 3, 16),
            (1, 0, 16),
            (1, 1, 16),
            (1, 2, 16),
            (1, 3, 16),
            (2, 0, 16),
            (2, 1, 16),
            (2, 2, 16),
            (2, 3, 16),
            (3, 0, 16),
            (3, 1, 16),
            (3, 2, 16),
            (3, 3, 16),
            (1, 4, 16),
            (2, 4, 16),
        ],
        // OpenTTD _tile_table_forest_1.
        &[
            (0, 0, 16),
            (1, 0, 16),
            (2, 0, 16),
            (3, 0, 16),
            (4, 0, 16),
            (0, 1, 16),
            (1, 1, 16),
            (2, 1, 16),
            (3, 1, 16),
            (4, 1, 16),
            (0, 2, 16),
            (1, 2, 16),
            (2, 2, 16),
            (3, 2, 16),
            (4, 2, 16),
            (0, 3, 16),
            (1, 3, 16),
            (2, 3, 16),
            (3, 3, 16),
            (4, 3, 16),
            (1, 4, 16),
            (2, 4, 16),
            (3, 4, 16),
        ],
    ];
    const FARM_LAYOUTS: [&[(i32, i32, u8)]; 3] = [
        // OpenTTD _tile_table_farm_0.
        &[
            (1, 0, 33),
            (1, 1, 34),
            (1, 2, 36),
            (0, 0, 37),
            (0, 1, 37),
            (0, 2, 36),
            (2, 0, 35),
            (2, 1, 38),
            (2, 2, 38),
        ],
        // OpenTTD _tile_table_farm_1.
        &[
            (1, 1, 33),
            (1, 2, 34),
            (0, 0, 35),
            (0, 1, 36),
            (0, 2, 36),
            (0, 3, 35),
            (1, 0, 37),
            (1, 3, 38),
            (2, 0, 37),
            (2, 1, 37),
            (2, 2, 38),
            (2, 3, 38),
        ],
        // OpenTTD _tile_table_farm_2.
        &[
            (2, 0, 33),
            (2, 1, 34),
            (0, 0, 36),
            (0, 1, 36),
            (0, 2, 37),
            (0, 3, 37),
            (1, 0, 35),
            (1, 1, 38),
            (1, 2, 38),
            (1, 3, 37),
            (2, 2, 37),
            (2, 3, 35),
        ],
    ];
    const OIL_LAYOUTS: [&[(i32, i32, u8)]; 2] = [
        // OpenTTD _tile_table_oil_well_0.
        &[(0, 0, 29), (1, 0, 29), (2, 0, 29), (0, 1, 29), (0, 2, 29)],
        // OpenTTD _tile_table_oil_well_1.
        &[(0, 0, 29), (1, 0, 29), (1, 1, 29), (2, 2, 29), (2, 3, 29)],
    ];
    const REFINERY_LAYOUTS: [&[(i32, i32, u8)]; 2] = [
        // OpenTTD _tile_table_oil_refinery_0.
        &[
            (0, 0, 20),
            (0, 1, 21),
            (0, 2, 22),
            (0, 3, 21),
            (1, 0, 20),
            (1, 1, 19),
            (1, 2, 22),
            (1, 3, 20),
            (2, 1, 18),
            (2, 2, 18),
            (2, 3, 18),
            (3, 2, 18),
            (3, 3, 18),
            (2, 0, 23),
            (3, 1, 23),
        ],
        // OpenTTD _tile_table_oil_refinery_1.
        &[
            (0, 0, 18),
            (0, 1, 18),
            (0, 2, 21),
            (0, 3, 22),
            (0, 4, 20),
            (1, 0, 18),
            (1, 1, 18),
            (1, 2, 19),
            (1, 3, 20),
            (2, 0, 18),
            (2, 1, 18),
            (2, 2, 19),
            (2, 3, 22),
            (1, 4, 23),
            (2, 4, 23),
        ],
    ];
    const FACTORY_LAYOUTS: [&[(i32, i32, u8)]; 2] = [
        // OpenTTD _tile_table_factory_0.
        &[
            (0, 0, 39),
            (0, 1, 40),
            (1, 0, 41),
            (1, 1, 42),
            (0, 2, 39),
            (0, 3, 40),
            (1, 2, 41),
            (1, 3, 42),
            (2, 1, 39),
            (2, 2, 40),
            (3, 1, 41),
            (3, 2, 42),
        ],
        // OpenTTD _tile_table_factory_1.
        &[
            (0, 0, 39),
            (0, 1, 40),
            (1, 0, 41),
            (1, 1, 42),
            (2, 0, 39),
            (2, 1, 40),
            (3, 0, 41),
            (3, 1, 42),
            (1, 2, 39),
            (1, 3, 40),
            (2, 2, 41),
            (2, 3, 42),
        ],
    ];
    const SAWMILL_LAYOUTS: [&[(i32, i32, u8)]; 2] = [
        // OpenTTD _tile_table_sawmill_0.
        &[
            (1, 0, 14),
            (1, 1, 12),
            (1, 2, 11),
            (2, 0, 14),
            (2, 1, 13),
            (0, 0, 15),
            (0, 1, 15),
            (0, 2, 12),
        ],
        // OpenTTD _tile_table_sawmill_1.
        &[
            (0, 0, 15),
            (0, 1, 11),
            (0, 2, 14),
            (1, 0, 15),
            (1, 1, 13),
            (1, 2, 12),
            (2, 0, 11),
            (2, 1, 13),
        ],
    ];
    const IRON_MINE_LAYOUTS: [&[(i32, i32, u8)]; 1] = [
        // OpenTTD _tile_table_iron_mine_0.
        &[
            (0, 0, 100),
            (0, 1, 101),
            (0, 2, 102),
            (0, 3, 103),
            (1, 0, 104),
            (1, 1, 105),
            (1, 2, 106),
            (1, 3, 107),
            (2, 0, 108),
            (2, 1, 109),
            (2, 2, 110),
            (2, 3, 111),
            (3, 0, 112),
            (3, 1, 113),
            (3, 2, 114),
            (3, 3, 115),
        ],
    ];

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
