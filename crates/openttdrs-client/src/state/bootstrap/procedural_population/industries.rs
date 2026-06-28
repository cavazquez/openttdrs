use openttdrs_core::{
    Climate, Command, IndustrySpec, TileCoord, apply_command, check_place_industry_spec,
};

use super::{PopCtx, in_preserve, min_distance_sq};

fn climate_industry_specs(climate: Climate) -> &'static [IndustrySpec] {
    match climate {
        Climate::Temperate => &[
            IndustrySpec::CoalMine,
            IndustrySpec::Forest,
            IndustrySpec::Sawmill,
            IndustrySpec::Factory,
            IndustrySpec::Farm,
            IndustrySpec::IronOreMine,
        ],
        Climate::SubArctic => &[
            IndustrySpec::CoalMine,
            IndustrySpec::Forest,
            IndustrySpec::Sawmill,
            IndustrySpec::Factory,
            IndustrySpec::GoldMine,
            IndustrySpec::IronOreMine,
        ],
        Climate::SubTropical => &[
            IndustrySpec::OilWells,
            IndustrySpec::OilRefinery,
            IndustrySpec::Farm,
            IndustrySpec::Factory,
            IndustrySpec::CopperOreMine,
        ],
        Climate::Toyland => &[
            IndustrySpec::Factory,
            IndustrySpec::Farm,
            IndustrySpec::Forest,
            IndustrySpec::CoalMine,
        ],
    }
}

pub(super) fn place_industries(
    ctx: &mut PopCtx<'_>,
    climate: Climate,
    target: usize,
    town_centers: &[TileCoord],
) {
    let specs = climate_industry_specs(climate);
    let margin = 3_u32;
    let span_w = ctx.mw.saturating_sub(margin * 2).max(1);
    let span_h = ctx.mh.saturating_sub(margin * 2).max(1);
    let min_town_dist_sq = 10_i32 * 10;
    let min_industry_dist_sq = 8_i32 * 8;
    let max_attempts = target.saturating_mul(120);
    let mut industry_origins: Vec<TileCoord> = Vec::with_capacity(target);

    for _ in 0..max_attempts {
        if industry_origins.len() >= target {
            break;
        }
        let x = i32::try_from(margin + ctx.rng.next_range(span_w)).unwrap_or(5);
        let y = i32::try_from(margin + ctx.rng.next_range(span_h)).unwrap_or(5);
        let origin = TileCoord::new(x, y);
        if in_preserve(ctx.preserve, x, y) {
            continue;
        }
        if town_centers
            .iter()
            .any(|&t| min_distance_sq(origin, t) < min_town_dist_sq)
        {
            continue;
        }
        if industry_origins
            .iter()
            .any(|&o| min_distance_sq(origin, o) < min_industry_dist_sq)
        {
            continue;
        }

        let spec =
            specs[usize::try_from(ctx.rng.next_range(u32::try_from(specs.len()).unwrap_or(1)))
                .unwrap_or(0)];
        if check_place_industry_spec(&ctx.state.map, origin, spec).is_err() {
            continue;
        }
        if apply_command(ctx.state, &Command::PlaceIndustrySpec(origin, spec)).is_err() {
            continue;
        }
        industry_origins.push(origin);
    }
}
