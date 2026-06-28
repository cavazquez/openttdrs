//! Pueblos e industrias en mapas procedurales (≥64²), fuera del demo 24×18.

use openttdrs_core::{
    Climate, Command, GameState, IndustrySpec, PreserveRect, TileCoord, TileKind, Town,
    apply_command, check_place_industry_spec, command_would_fail, generate_town_name,
    tile_slope_and_z,
};

use super::world::{MapSizePreset, NewGameSettings, PopulationDensity};

/// HouseID originales OpenTTD (0..=109); evitamos NewGRF.
const PROCEDURAL_HOUSE_ID_MAX: u32 = 110;
/// Variación de estilo dentro de un mismo pueblo.
const PROCEDURAL_HOUSE_STYLE_SPREAD: u32 = 16;

/// Generador determinista para colocación (mismo seed → mismo mundo).
struct SeededRng {
    state: u64,
}

impl SeededRng {
    fn new(seed: u64) -> Self {
        Self { state: seed.max(1) }
    }

    fn next_u32(&mut self) -> u32 {
        self.state = self.state.wrapping_mul(6364136223846793005).wrapping_add(1);
        (self.state >> 32) as u32
    }

    fn next_range(&mut self, max_exclusive: u32) -> u32 {
        if max_exclusive <= 1 {
            return 0;
        }
        self.next_u32() % max_exclusive
    }
}

/// El demo compacto ya coloca pueblo e industrias en `gameplay_showcase`.
#[must_use]
pub(crate) fn should_populate_procedurally(settings: &NewGameSettings) -> bool {
    let s = settings.sanitized();
    !(s.preserve_demo && s.map_size == MapSizePreset::Compact)
}

pub(crate) fn populate_procedural_world(
    state: &mut GameState,
    settings: &NewGameSettings,
    preserve: &[PreserveRect],
) {
    let settings = settings.sanitized();
    let (mw, mh) = state.map.dimensions();
    let area = u64::from(mw).saturating_mul(u64::from(mh));
    let seed = procedural_seed(state.world_seed, settings.seed, mw, mh);
    let mut rng = SeededRng::new(seed);

    let town_target = scaled_population_count((area / 512).clamp(4, 40), settings.town_density, 2);
    let industry_target =
        scaled_population_count((area / 768).clamp(6, 48), settings.industry_density, 3);

    let mut town_centers: Vec<TileCoord> = Vec::with_capacity(town_target);
    let mut ctx = PopCtx {
        state,
        preserve,
        rng: &mut rng,
        mw,
        mh,
    };
    place_towns(&mut ctx, town_target, &mut town_centers);
    place_industries(&mut ctx, settings.climate, industry_target, &town_centers);
}

fn scaled_population_count(base: u64, density: PopulationDensity, min_count: u64) -> usize {
    let scaled = base
        .saturating_mul(u64::from(density.multiplier_bps()))
        .saturating_div(100);
    usize::try_from(scaled.max(min_count)).unwrap_or(min_count as usize)
}

fn procedural_seed(world_seed: u64, settings_seed: u64, mw: u32, mh: u32) -> u64 {
    if world_seed != 0 {
        return world_seed;
    }
    if settings_seed != 0 {
        return settings_seed;
    }
    u64::from(mw)
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .wrapping_add(u64::from(mh).wrapping_mul(0x6C62_272E_07BB_0142))
        .wrapping_add(0x5055_4C41_5449_4F4E)
}

fn in_preserve(preserve: &[PreserveRect], x: i32, y: i32) -> bool {
    preserve.iter().any(|r| r.contains(x, y))
}

fn tile_ok_for_house(state: &GameState, c: TileCoord, preserve: &[PreserveRect]) -> bool {
    if in_preserve(preserve, c.x, c.y) {
        return false;
    }
    tile_is_flat_grass(&state.map, c)
}

fn tile_is_flat_grass(map: &openttdrs_core::Map, c: TileCoord) -> bool {
    if map.get_kind(c) != Some(TileKind::Grass) {
        return false;
    }
    tile_slope_and_z(map, c).is_some_and(|(tileh, _)| tileh == 0)
}

fn min_distance_sq(a: TileCoord, b: TileCoord) -> i32 {
    let dx = a.x - b.x;
    let dy = a.y - b.y;
    dx * dx + dy * dy
}

struct PopCtx<'a> {
    state: &'a mut GameState,
    preserve: &'a [PreserveRect],
    rng: &'a mut SeededRng,
    mw: u32,
    mh: u32,
}

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

fn place_towns(ctx: &mut PopCtx<'_>, target: usize, town_centers: &mut Vec<TileCoord>) {
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

        let town_house_base = ctx.rng.next_range(PROCEDURAL_HOUSE_ID_MAX);
        let placed_houses = build_street_town(ctx, &plan, town_house_base);
        if placed_houses < 3 {
            continue;
        }

        let name_seed = ctx.rng.next_u32();
        let name = generate_town_name(4, name_seed).unwrap_or_else(|| format!("Pueblo {x},{y}"));
        let town_id = u32::try_from(ctx.state.towns.len().saturating_add(1)).unwrap_or(1);
        ctx.state.towns.push(Town {
            id: town_id,
            pos: plan.town_pos,
            name,
            population: u32::try_from(placed_houses.saturating_mul(8)).unwrap_or(8),
        });
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

fn build_street_town(ctx: &mut PopCtx<'_>, plan: &StreetTownPlan, town_house_base: u32) -> usize {
    let road_bits = match plan.axis {
        StreetAxis::EastWest => ROAD_BITS_AXIS_X,
        StreetAxis::NorthSouth => ROAD_BITS_AXIS_Y,
    };

    if !plan_fits_terrain(ctx, plan) {
        return 0;
    }

    for &c in &plan.roads {
        if apply_command(ctx.state, &Command::SetRoadBits(c, road_bits)).is_err() {
            rollback_road_tiles(ctx.state, &plan.roads);
            return 0;
        }
    }

    let mut placed = 0_usize;
    for &c in &plan.houses {
        if !tile_ok_for_house(ctx.state, c, ctx.preserve) {
            continue;
        }
        let house_id = u16::try_from(
            (town_house_base + ctx.rng.next_range(PROCEDURAL_HOUSE_STYLE_SPREAD))
                % PROCEDURAL_HOUSE_ID_MAX,
        )
        .unwrap_or(1);
        let age = u8::try_from(ctx.rng.next_u32() % 200).unwrap_or(0);
        if ctx.state.map.set_completed_house(c, house_id, age).is_ok() {
            placed += 1;
        }
    }
    placed
}

fn rollback_road_tiles(state: &mut GameState, roads: &[TileCoord]) {
    for &c in roads {
        if state.map.get_kind(c) == Some(TileKind::Road) {
            let _ = state.map.set_kind(c, TileKind::Grass);
        }
    }
}

#[cfg(test)]
fn road_tiles_are_flat(map: &openttdrs_core::Map, roads: &[TileCoord]) -> bool {
    roads.iter().all(|&c| {
        map.get_kind(c) == Some(TileKind::Road)
            && tile_slope_and_z(map, c).is_some_and(|(tileh, _)| tileh == 0)
    })
}

#[cfg(test)]
fn house_beside_road(map: &openttdrs_core::Map, house: TileCoord) -> bool {
    for (dx, dy) in [(0, 1), (0, -1), (1, 0), (-1, 0)] {
        let n = TileCoord::new(house.x + dx, house.y + dy);
        if map.get_kind(n) == Some(TileKind::Road) {
            return true;
        }
    }
    false
}

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

fn place_industries(
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::bootstrap::build_procedural_demo_world;

    #[test]
    fn skips_population_on_compact_demo() {
        assert!(!should_populate_procedurally(&NewGameSettings::default()));
    }

    #[test]
    fn populates_large_procedural_island() {
        let settings = NewGameSettings::procedural_island(openttdrs_core::Climate::Temperate, 99);
        assert!(should_populate_procedurally(&settings));
        let state = build_procedural_demo_world(&settings);
        assert!(!state.towns.is_empty(), "debe haber al menos un pueblo");
        assert!(
            !state.industries.is_empty(),
            "debe haber al menos una industria"
        );
    }

    #[test]
    fn dense_population_places_more_towns_than_sparse() {
        let base = NewGameSettings::procedural_island(openttdrs_core::Climate::Temperate, 1234);
        let sparse = build_procedural_demo_world(&NewGameSettings {
            town_density: PopulationDensity::Sparse,
            industry_density: PopulationDensity::Sparse,
            ..base
        });
        let dense = build_procedural_demo_world(&NewGameSettings {
            town_density: PopulationDensity::Dense,
            industry_density: PopulationDensity::Dense,
            ..base
        });
        assert!(dense.towns.len() >= sparse.towns.len());
        assert!(dense.industries.len() >= sparse.industries.len());
    }

    #[test]
    fn procedural_houses_are_completed_with_varied_ids() {
        let settings = NewGameSettings::procedural_island(openttdrs_core::Climate::Temperate, 555);
        let state = build_procedural_demo_world(&settings);
        let (mw, mh) = state.map.dimensions();
        let mut house_tiles = Vec::new();
        for y in 0..mh {
            for x in 0..mw {
                let c = TileCoord::new(x as i32, y as i32);
                if state.map.get_kind(c) == Some(TileKind::House)
                    && let Some(tile) = state.map.get(c)
                {
                    house_tiles.push(tile);
                }
            }
        }
        assert!(house_tiles.len() >= 3);
        assert!(house_tiles.iter().all(|t| t.m3 & 0x80 != 0));
        let distinct_ids: std::collections::HashSet<u16> =
            house_tiles.iter().map(|t| t.m8).collect();
        assert!(distinct_ids.len() > 1, "debe haber más de un HouseID");
    }

    #[test]
    fn procedural_towns_place_houses_beside_roads() {
        let settings = NewGameSettings::procedural_island(openttdrs_core::Climate::Temperate, 888);
        let state = build_procedural_demo_world(&settings);
        let (mw, mh) = state.map.dimensions();
        let mut houses = Vec::new();
        let mut road_tiles = 0_u32;
        for y in 0..mh {
            for x in 0..mw {
                let c = TileCoord::new(x as i32, y as i32);
                match state.map.get_kind(c) {
                    Some(TileKind::House) => houses.push(c),
                    Some(TileKind::Road) => road_tiles += 1,
                    _ => {}
                }
            }
        }
        assert!(road_tiles > 0, "debe haber calles en pueblos procedurales");
        assert!(!houses.is_empty());
        assert!(
            houses.iter().all(|&c| house_beside_road(&state.map, c)),
            "cada casa debe tener calle adyacente"
        );
    }

    #[test]
    fn procedural_town_roads_stay_on_flat_terrain() {
        let settings =
            NewGameSettings::procedural_island(openttdrs_core::Climate::Temperate, 12345);
        let state = build_procedural_demo_world(&settings);
        let (mw, mh) = state.map.dimensions();
        let mut road_tiles = Vec::new();
        for y in 0..mh {
            for x in 0..mw {
                let c = TileCoord::new(x as i32, y as i32);
                if state.map.get_kind(c) == Some(TileKind::Road) {
                    road_tiles.push(c);
                }
            }
        }
        assert!(!road_tiles.is_empty());
        assert!(
            road_tiles_are_flat(&state.map, &road_tiles),
            "todas las calles procedurales deben estar en terreno plano"
        );
    }
}
