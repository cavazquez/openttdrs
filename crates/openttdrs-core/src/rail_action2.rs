//! Contexto Action2 para gráficos de `RailType` (`RailTypeScopeResolver`).

use crate::map::{Map, Tile, TileCoord};
use crate::newgrf_sprites::Action2EvalCtx;
use crate::newgrf_type_tables::{GrfTypeTranslationTables, reverse_rail_type_for_var45};
use crate::rail_type::rail_type_from_tile;
use crate::world_gen::Climate;

/// Variables de tesela usadas por `RailType` Action2 al resolver señales custom.
#[must_use]
pub fn action2_eval_ctx_for_rail_tile(
    map: &Map,
    tile: Tile,
    coord: TileCoord,
    climate: Climate,
    calendar_date: u32,
    type_tables: Option<&GrfTypeTranslationTables>,
) -> Action2EvalCtx {
    let mut ctx = Action2EvalCtx::default();
    let bits = rail_tile_random_bits(map, coord);
    ctx.random_bits = u32::from(bits);
    ctx.vars.insert(0x5F, u32::from(bits) << 8);
    ctx.vars.insert(0x40, terrain_type(tile, climate));
    ctx.vars.insert(0x41, 0);
    ctx.vars.insert(0x42, 0); // una tesela ferroviaria con señal no es cruce vial
    ctx.vars.insert(0x43, calendar_date);
    ctx.vars.insert(0x44, 0); // HouseZone::TownEdge
    let local = reverse_rail_type_for_var45(type_tables, rail_type_from_tile(tile));
    ctx.vars.insert(0x45, 0xFFFF | (u32::from(local) << 16));
    ctx
}

fn rail_tile_random_bits(map: &Map, coord: TileCoord) -> u8 {
    let (mw, _) = map.dimensions();
    let x = coord.x.cast_unsigned();
    let y = coord.y.cast_unsigned();
    let tile_index = y
        .wrapping_mul(mw)
        .wrapping_add(x)
        .wrapping_add(x.wrapping_add(y).wrapping_mul(16));
    u8::try_from(tile_index.count_ones() & 0x03).unwrap_or(0)
}

fn terrain_type(tile: Tile, climate: Climate) -> u32 {
    if climate.uses_desert_patches() && (tile.m7 & 0x20) != 0 {
        return 1;
    }
    if climate.uses_snow_ground() || (tile.m7 & 0x20) != 0 {
        return 4;
    }
    0
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::map::TileKind;
    use crate::rail_type::{RailType, set_rail_type_on_tile};

    #[test]
    fn rail_ctx_exposes_date_terrain_and_local_track_type() {
        let map = Map::new_flat(8, 8, 0);
        let mut tile = map.get(TileCoord::new(2, 3)).unwrap();
        tile.kind = TileKind::Rail;
        tile = set_rail_type_on_tile(tile, RailType::Electric);
        let ctx = action2_eval_ctx_for_rail_tile(
            &map,
            tile,
            TileCoord::new(2, 3),
            Climate::Temperate,
            12_345,
            None,
        );
        assert_eq!(ctx.vars.get(&0x40), Some(&0));
        assert_eq!(ctx.vars.get(&0x43), Some(&12_345));
        assert_eq!(ctx.vars.get(&0x45), Some(&0x01_FFFF));
    }
}
