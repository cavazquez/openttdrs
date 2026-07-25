//! Colocación de industrias (MVP de `GenerateIndustries`).

use crate::command::{Command, apply_command, check_place_industry_spec};
use crate::industry::IndustrySpec;
use crate::map::TileCoord;

use super::{PopCtx, in_preserve, min_distance_sq};

/// Intenta colocar hasta `target` industrias; devuelve cuántas se crearon.
pub(super) fn place_industries(
    ctx: &mut PopCtx<'_>,
    target: usize,
    town_centers: &[TileCoord],
) -> usize {
    if target == 0 {
        return 0;
    }
    let specs = IndustrySpec::specs_for_climate(ctx.state.climate);
    if specs.is_empty() {
        return 0;
    }
    let margin = 3_u32;
    let span_w = ctx.mw.saturating_sub(margin * 2).max(1);
    let span_h = ctx.mh.saturating_sub(margin * 2).max(1);
    let min_town_dist_sq = 10_i32 * 10;
    let min_industry_dist_sq = 8_i32 * 8;
    let max_attempts = target.saturating_mul(200).max(4_000);
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
    industry_origins.len()
}
