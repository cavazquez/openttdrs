//! Colocación de industrias (MVP de `GenerateIndustries`).

use crate::command::{Command, apply_command, check_place_industry_spec};
use crate::company::OWNER_NONE_M1;
use crate::industry::IndustrySpec;
use crate::map::tree_tile_loop::{clear_ground_type, with_clear_counter};
use crate::map::{TileCoord, TileKind, WaterClass, set_water_class_m1};
use crate::world_gen::{
    CLEAR_GROUND_DESERT, CLEAR_GROUND_FIELDS, CLEAR_GROUND_ROCKY, CLEAR_GROUND_SNOW,
    clear_ground_m5,
};

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
        // `DoCreateNewIndustry` planta 50 campos alrededor de una granja con
        // `PlantRandomFarmField`. El mapa generado debe conservar el contrato
        // MP_CLEAR/CLEAR_FIELDS (no un TileKind inventado): el renderer y el
        // save de OpenTTD leen m5=0x0f, m2=IndustryID y m3=estado.
        if matches!(spec, IndustrySpec::Farm | IndustrySpec::FarmTropic) {
            let industry_id = ctx
                .state
                .industries
                .last()
                .map_or(0, |industry| industry.instance_id);
            plant_farm_fields(ctx, origin, industry_id);
        }
        industry_origins.push(origin);
    }
    industry_origins.len()
}

const FARM_FIELD_ATTEMPTS: usize = 50;

fn farm_field_suitable(tile: crate::map::Tile, allow_fields: bool, allow_rough: bool) -> bool {
    match tile.kind {
        TileKind::Grass => match clear_ground_type(tile.m5) {
            CLEAR_GROUND_SNOW | CLEAR_GROUND_DESERT => false,
            CLEAR_GROUND_ROCKY => allow_rough,
            CLEAR_GROUND_FIELDS => allow_fields,
            _ => true,
        },
        // OpenTTD permits ordinary trees as a field substrate, but not shore
        // trees. `tree_ground` is stored in m2 bits 6..8 for MP_TREES.
        TileKind::Forest => ((tile.m2 >> 6) & 0x07) != 3 && allow_rough,
        _ => false,
    }
}

fn plant_farm_fields(ctx: &mut PopCtx<'_>, origin: TileCoord, industry_id: u8) {
    let map_w = i32::try_from(ctx.mw).unwrap_or(i32::MAX);
    let map_h = i32::try_from(ctx.mh).unwrap_or(i32::MAX);
    if map_w == 0 || map_h == 0 {
        return;
    }
    // La RNG global de OpenTTD sí intercalará estos draws, pero el generador
    // procedural de esta etapa todavía no porta `GenerateIndustries` completo.
    // Mantener un stream derivado evita que añadir la representación correcta
    // de campos cambie qué industrias se colocan, y permite medir este gap de
    // forma aislada.
    let field_seed = ctx
        .state
        .world_seed
        .wrapping_add(u64::from(industry_id) << 32)
        .wrapping_add(u64::from(origin.x.cast_unsigned()) << 16)
        .wrapping_add(u64::from(origin.y.cast_unsigned()));
    let mut rng = super::SeededRng::new(field_seed);

    for _ in 0..FARM_FIELD_ATTEMPTS {
        // `PlantFarmField`: width/height are 4..7 in temperate and are
        // derived from the same 0x303 random mask as upstream.
        let size_random = (rng.next_u32() & 0x303).wrapping_add(0x404);
        let size_x = i32::try_from(size_random & 0xFF).unwrap_or(4).max(1);
        let size_y = i32::try_from((size_random >> 8) & 0xFF).unwrap_or(4).max(1);
        let center_x = origin.x + i32::try_from(rng.next_range(31)).unwrap_or(0) - 16;
        let center_y = origin.y + i32::try_from(rng.next_range(31)).unwrap_or(0) - 16;
        let min_x = (center_x - size_x / 2).clamp(0, map_w.saturating_sub(1));
        let min_y = (center_y - size_y / 2).clamp(0, map_h.saturating_sub(1));
        let max_x = (min_x + size_x).min(map_w);
        let max_y = (min_y + size_y).min(map_h);
        if max_x <= min_x || max_y <= min_y {
            continue;
        }

        let mut suitable = 0usize;
        let mut total = 0usize;
        for y in min_y..max_y {
            for x in min_x..max_x {
                total += 1;
                if ctx
                    .state
                    .map
                    .get(TileCoord::new(x, y))
                    .is_some_and(|tile| farm_field_suitable(tile, false, false))
                {
                    suitable += 1;
                }
            }
        }
        if suitable * 2 < total {
            continue;
        }

        let field_random = rng.next_u32();
        let counter = u8::try_from((field_random >> 5) & 7).unwrap_or(0);
        let field_type = u8::try_from((((field_random >> 8) & 0xFF) * 9) >> 8).unwrap_or(0);
        for y in min_y..max_y {
            for x in min_x..max_x {
                let c = TileCoord::new(x, y);
                let Some(tile) = ctx.state.map.get(c) else {
                    continue;
                };
                if !farm_field_suitable(tile, true, true) || in_preserve(ctx.preserve, x, y) {
                    continue;
                }
                let mut field = tile;
                field.kind = TileKind::Grass;
                field.mapt = 0;
                field.m1 = set_water_class_m1(OWNER_NONE_M1, WaterClass::Invalid);
                field.m2 = industry_id;
                field.m3 = field_type;
                field.m5 = with_clear_counter(clear_ground_m5(CLEAR_GROUND_FIELDS, 3), counter);
                field.m6 = 0;
                field.m7 = 0;
                field.m3hi = 0;
                let _ = ctx.state.map.set_tile(c, field);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_state::GameState;
    use crate::map::tree_tile_loop::clear_density;

    #[test]
    fn farm_fields_use_openttd_clear_tile_contract() {
        let mut state = GameState::new(64, 64);
        state.world_seed = 0xCAFE;
        for y in 0..64_i32 {
            for x in 0..64_i32 {
                let c = TileCoord::new(x, y);
                let mut tile = state.map.get(c).expect("flat map tile");
                tile.mapt = 0;
                tile.m5 = clear_ground_m5(0, 3);
                tile.m1 = OWNER_NONE_M1;
                state.map.set_tile(c, tile).expect("set flat tile");
            }
        }
        let mut rng = super::super::SeededRng::new(7);
        let mut ctx = PopCtx {
            state: &mut state,
            preserve: &[],
            rng: &mut rng,
            mw: 64,
            mh: 64,
        };
        plant_farm_fields(&mut ctx, TileCoord::new(32, 32), 7);

        let fields: Vec<_> = ctx
            .state
            .map
            .tiles()
            .iter()
            .filter(|tile| clear_ground_type(tile.m5) == CLEAR_GROUND_FIELDS)
            .collect();
        assert!(
            !fields.is_empty(),
            "a flat map must accept at least one field"
        );
        assert!(fields.iter().all(|tile| {
            tile.kind == TileKind::Grass
                && tile.mapt == 0
                && tile.m2 == 7
                && clear_density(tile.m5) == 3
                && (tile.m3 & 0x0F) <= 8
                && tile.m1 == set_water_class_m1(OWNER_NONE_M1, WaterClass::Invalid)
        }));
    }
}
