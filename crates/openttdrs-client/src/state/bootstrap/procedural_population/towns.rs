use openttdrs_core::Command;
use openttdrs_core::prelude::*;
use openttdrs_core::{Town, generate_town_name, tile_slope_and_z};

use super::{
    PROCEDURAL_HOUSE_STYLE_SPREAD, PopCtx, in_preserve, min_distance_sq, procedural_house_choices,
    tile_is_flat_grass, tile_ok_for_house,
};

/// Bits de carretera recta (eje X / eje Y).
const ROAD_BITS_AXIS_X: u8 = 0x0A;
const ROAD_BITS_AXIS_Y: u8 = 0x05;

#[derive(Clone, Copy, PartialEq, Eq)]
enum StreetAxis {
    EastWest,
    NorthSouth,
}

struct StreetTownPlan {
    axis: StreetAxis,
    roads: Vec<TileCoord>,
    houses: Vec<TileCoord>,
    town_pos: TileCoord,
}

pub(super) fn place_towns(ctx: &mut PopCtx<'_>, target: usize, town_centers: &mut Vec<TileCoord>) {
    let margin = 5_u32;
    let span_w = ctx.mw.saturating_sub(margin * 2).max(1);
    let span_h = ctx.mh.saturating_sub(margin * 2).max(1);
    let min_town_dist_sq = 14_i32 * 14;
    let max_attempts = target.saturating_mul(80);
    let map_w = i32::try_from(ctx.mw).unwrap_or(i32::MAX);
    let map_h = i32::try_from(ctx.mh).unwrap_or(i32::MAX);

    for _attempt in 0..max_attempts {
        if town_centers.len() >= target {
            break;
        }
        let x = i32::try_from(margin + ctx.rng.next_range(span_w)).unwrap_or(5);
        let y = i32::try_from(margin + ctx.rng.next_range(span_h)).unwrap_or(5);
        let center = TileCoord::new(x, y);
        if in_preserve(ctx.preserve, x, y) {
            continue;
        }
        if !tile_is_flat_grass(&ctx.state.map, center) {
            continue;
        }
        if town_centers
            .iter()
            .any(|&other| min_distance_sq(center, other) < min_town_dist_sq)
        {
            continue;
        }

        let axis = if ctx.rng.next_range(2) == 0 {
            StreetAxis::EastWest
        } else {
            StreetAxis::NorthSouth
        };
        let half_len = i32::try_from(2 + ctx.rng.next_range(3)).unwrap_or(2);
        let south_row = ctx.rng.next_range(3) != 0;
        let Some(plan) = plan_street_town(center, axis, half_len, south_row, map_w, map_h) else {
            continue;
        };
        if !plan_fits_terrain(ctx, &plan) {
            continue;
        }

        let choices = procedural_house_choices();
        if choices.is_empty() {
            continue;
        }
        let town_house_base = ctx
            .rng
            .next_range(u32::try_from(choices.len()).unwrap_or(1));
        let (placed_houses, population) = build_street_town(ctx, &plan, town_house_base);
        if placed_houses < 3 {
            continue;
        }

        let name_seed = ctx.rng.next_u32();
        let name = generate_town_name(4, name_seed).unwrap_or_else(|| format!("Pueblo {x},{y}"));
        let town_id = u32::try_from(ctx.state.towns.len().saturating_add(1)).unwrap_or(1);
        let mut town = Town {
            id: town_id,
            pos: plan.town_pos,
            name,
            population,
            local_authority_rating: 0,
            passengers_served: 0,
            mail_served: 0,
            growth_funded: 0,
            ..Default::default()
        };
        town.init_growth_goals(ctx.state.climate);
        ctx.state.towns.push(town);
        town_centers.push(plan.town_pos);
    }
}

fn plan_street_town(
    center: TileCoord,
    axis: StreetAxis,
    half_len: i32,
    south_row: bool,
    map_w: i32,
    map_h: i32,
) -> Option<StreetTownPlan> {
    let mut roads = Vec::new();
    let mut houses = Vec::new();
    let town_pos;

    match axis {
        StreetAxis::EastWest => {
            let road_y = center.y;
            town_pos = TileCoord::new(center.x, road_y.saturating_sub(1));
            for dx in -half_len..=half_len {
                let x = center.x + dx;
                if !coord_in_map(x, road_y, map_w, map_h) {
                    return None;
                }
                roads.push(TileCoord::new(x, road_y));
                for row in house_rows_beside_road(south_row) {
                    let hy = road_y + row;
                    if coord_in_map(x, hy, map_w, map_h) {
                        houses.push(TileCoord::new(x, hy));
                    }
                }
            }
        }
        StreetAxis::NorthSouth => {
            let road_x = center.x;
            town_pos = TileCoord::new(road_x.saturating_sub(1), center.y);
            for dy in -half_len..=half_len {
                let y = center.y + dy;
                if !coord_in_map(road_x, y, map_w, map_h) {
                    return None;
                }
                roads.push(TileCoord::new(road_x, y));
                for col in house_cols_beside_road(south_row) {
                    let hx = road_x + col;
                    if coord_in_map(hx, y, map_w, map_h) {
                        houses.push(TileCoord::new(hx, y));
                    }
                }
            }
        }
    }

    if roads.is_empty() || houses.len() < 3 {
        return None;
    }
    Some(StreetTownPlan {
        axis,
        roads,
        houses,
        town_pos,
    })
}

/// Filas de casas en la acera (±1 tesela respecto a la calle).
fn house_rows_beside_road(second_side: bool) -> Vec<i32> {
    let mut rows = vec![-1];
    if second_side {
        rows.push(1);
    }
    rows
}

fn house_cols_beside_road(east_side: bool) -> Vec<i32> {
    let mut cols = vec![-1];
    if east_side {
        cols.push(1);
    }
    cols
}

fn coord_in_map(x: i32, y: i32, map_w: i32, map_h: i32) -> bool {
    x >= 0 && y >= 0 && x < map_w && y < map_h
}

fn plan_fits_terrain(ctx: &PopCtx<'_>, plan: &StreetTownPlan) -> bool {
    if plan
        .roads
        .iter()
        .any(|&c| in_preserve(ctx.preserve, c.x, c.y))
    {
        return false;
    }
    if !street_roads_are_flat_and_level(ctx.state, &plan.roads) {
        return false;
    }
    let road_bits = match plan.axis {
        StreetAxis::EastWest => ROAD_BITS_AXIS_X,
        StreetAxis::NorthSouth => ROAD_BITS_AXIS_Y,
    };
    if plan
        .roads
        .iter()
        .any(|&c| command_would_fail(ctx.state, &Command::SetRoadBits(c, road_bits)).is_some())
    {
        return false;
    }
    plan.houses
        .iter()
        .filter(|&&c| tile_ok_for_house(ctx.state, c, ctx.preserve))
        .count()
        >= 3
}

fn street_roads_are_flat_and_level(state: &GameState, roads: &[TileCoord]) -> bool {
    let mut base_z = None;
    for &c in roads {
        if !tile_is_flat_grass(&state.map, c) {
            return false;
        }
        let Some((tileh, z)) = tile_slope_and_z(&state.map, c) else {
            return false;
        };
        if tileh != 0 {
            return false;
        }
        match base_z {
            None => base_z = Some(z),
            Some(b) if b != z => return false,
            Some(_) => {}
        }
    }
    true
}

/// Coloca calles/casas; devuelve `(casas_colocadas, población HouseSpec)`.
fn build_street_town(
    ctx: &mut PopCtx<'_>,
    plan: &StreetTownPlan,
    town_house_base: u32,
) -> (usize, u32) {
    let road_bits = match plan.axis {
        StreetAxis::EastWest => ROAD_BITS_AXIS_X,
        StreetAxis::NorthSouth => ROAD_BITS_AXIS_Y,
    };

    if !plan_fits_terrain(ctx, plan) {
        return (0, 0);
    }

    for &c in &plan.roads {
        if apply_command(ctx.state, &Command::SetRoadBits(c, road_bits)).is_err() {
            rollback_road_tiles(ctx.state, &plan.roads);
            return (0, 0);
        }
    }

    let choices = procedural_house_choices();
    let n_choices = u32::try_from(choices.len()).unwrap_or(0);
    if n_choices == 0 {
        return (0, 0);
    }

    let mut placed = 0_usize;
    let mut population = 0_u32;
    for &c in &plan.houses {
        if !tile_ok_for_house(ctx.state, c, ctx.preserve) {
            continue;
        }
        let idx = (town_house_base + ctx.rng.next_range(PROCEDURAL_HOUSE_STYLE_SPREAD)) % n_choices;
        let house_id = choices[usize::try_from(idx).unwrap_or(0)];
        let age = u8::try_from(ctx.rng.next_u32() % 200).unwrap_or(0);
        if ctx.state.map.set_completed_house(c, house_id, age).is_ok() {
            placed += 1;
            population = population
                .saturating_add(u32::from(openttdrs_core::house_spec_population(house_id)));
        }
    }
    (placed, population)
}

fn rollback_road_tiles(state: &mut GameState, roads: &[TileCoord]) {
    for &c in roads {
        if state.map.get_kind(c) == Some(TileKind::Road) {
            let _ = state.map.set_kind(c, TileKind::Grass);
        }
    }
}

#[cfg(test)]
pub(super) fn road_tiles_are_flat(map: &openttdrs_core::Map, roads: &[TileCoord]) -> bool {
    roads.iter().all(|&c| {
        map.get_kind(c) == Some(TileKind::Road)
            && tile_slope_and_z(map, c).is_some_and(|(tileh, _)| tileh == 0)
    })
}

#[cfg(test)]
pub(super) fn house_beside_road(map: &openttdrs_core::Map, house: TileCoord) -> bool {
    for (dx, dy) in [(0, 1), (0, -1), (1, 0), (-1, 0)] {
        let n = TileCoord::new(house.x + dx, house.y + dy);
        if map.get_kind(n) == Some(TileKind::Road) {
            return true;
        }
    }
    false
}
