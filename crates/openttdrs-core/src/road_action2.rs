//! Contexto Action2 para teselas de carretera / tram (vars de runtime).

use crate::map::{Map, Tile, TileCoord, TileKind, is_road_level_crossing};
use crate::newgrf_sprites::Action2EvalCtx;
use crate::newgrf_type_tables::{
    GrfTypeTranslationTables, reverse_rail_type_for_var45, reverse_road_type_for_var45,
};
use crate::rail_type::rail_type_from_tile;
use crate::road_type::{RoadTypeDef, road_type_from_tile, tram_road_type_from_tile};
use crate::world_gen::Climate;

/// Contexto Action2 para dibujar / resolver sprites de una tesela road/tram.
///
/// MVP: `40` (terreno), `42` (cruce cerrado), `45` (tipos track en tesela).
#[must_use]
pub fn action2_eval_ctx_for_road_tile(
    _map: &Map,
    tile: Tile,
    coord: TileCoord,
    climate: Climate,
    type_tables: Option<&GrfTypeTranslationTables>,
    road_catalog: &[RoadTypeDef],
) -> Action2EvalCtx {
    let mut ctx = Action2EvalCtx::default();

    // Bits pseudoaleatorios por posición de tesela (como OpenTTD road random).
    let bits = seed_road_tile_random(coord);
    ctx.random_bits = u32::from(bits);
    ctx.vars.insert(0x5F, u32::from(bits) << 8);

    ctx.vars
        .insert(0x40, terrain_type_for_road_tile(tile, climate));

    let crossing_closed = if is_road_level_crossing(tile.mapt, tile.m5, tile.kind) {
        // `IsCrossingBarred`: bit 5 de m5.
        u32::from((tile.m5 & 0x20) != 0)
    } else {
        0
    };
    ctx.vars.insert(0x42, crossing_closed);

    ctx.vars
        .insert(0x45, track_types_on_tile(tile, type_tables, road_catalog));

    ctx
}

fn seed_road_tile_random(coord: TileCoord) -> u8 {
    let x = coord.x.cast_unsigned();
    let y = coord.y.cast_unsigned();
    ((x.wrapping_mul(0x9E37_79B9) ^ y.wrapping_mul(0x85EB_CA6B)) >> 24) as u8
}

fn terrain_type_for_road_tile(tile: Tile, climate: Climate) -> u32 {
    if climate.uses_desert_patches() && (tile.m7 & 0x20) != 0 {
        return 1;
    }
    if climate.uses_snow_ground() || (tile.m7 & 0x20) != 0 {
        return 4;
    }
    0
}

/// Formato `__RRttrr` (`GetTrackTypes`); `0xFF` ausente, `0xFE` sin match en tabla.
fn track_types_on_tile(
    tile: Tile,
    tables: Option<&GrfTypeTranslationTables>,
    catalog: &[RoadTypeDef],
) -> u32 {
    let rr = if tile.kind == TileKind::Road || tile.kind == TileKind::RoadDepot {
        u32::from(reverse_road_type_for_var45(
            tables,
            catalog,
            road_type_from_tile(&tile),
        ))
    } else {
        0xFF
    };
    let tt = tram_road_type_from_tile(&tile).map_or(0xFF_u32, |t| {
        u32::from(reverse_road_type_for_var45(tables, catalog, t))
    });
    let rail = if is_road_level_crossing(tile.mapt, tile.m5, tile.kind) {
        u32::from(reverse_rail_type_for_var45(
            tables,
            rail_type_from_tile(tile),
        ))
    } else {
        0xFF
    };
    rr | (tt << 8) | (rail << 16)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::map::{Map, OTTD_MP_ROAD};
    use crate::road_type::{RoadType, set_road_type_on_tile, set_tram_road_type_on_tile};

    fn plain_road() -> Tile {
        Tile {
            height: 0,
            kind: TileKind::Road,
            mapt: OTTD_MP_ROAD << 4,
            m5: 0x05,
            m1: 0,
            m6: 0,
            m8: 0,
            m3: 0,
            m2: 0,
            m2_hi: 0,
            m7: 0,
            m3hi: 0,
        }
    }

    #[test]
    fn road_ctx_terrain_and_types() {
        let map = Map::new_flat(4, 4, 0);
        let c = TileCoord::new(1, 1);
        let tile = set_road_type_on_tile(plain_road(), RoadType::Road);
        let catalog = crate::road_type::vanilla_road_type_catalog();
        let ctx = action2_eval_ctx_for_road_tile(&map, tile, c, Climate::Temperate, None, &catalog);
        assert_eq!(ctx.vars.get(&0x40), Some(&0));
        assert_eq!(ctx.vars.get(&0x42), Some(&0));
        let v45 = *ctx.vars.get(&0x45).unwrap();
        assert_eq!(v45 & 0xFF, 0); // road
        assert_eq!((v45 >> 8) & 0xFF, 0xFF); // no tram
        assert_eq!((v45 >> 16) & 0xFF, 0xFF); // no rail
    }

    #[test]
    fn road_ctx_crossing_closed() {
        let map = Map::new_flat(4, 4, 0);
        let c = TileCoord::new(2, 2);
        let mut tile = plain_road();
        tile.m5 = (1 << 6) | 0x20; // crossing + barred
        let catalog = crate::road_type::vanilla_road_type_catalog();
        let ctx = action2_eval_ctx_for_road_tile(&map, tile, c, Climate::Temperate, None, &catalog);
        assert_eq!(ctx.vars.get(&0x42), Some(&1));
        let v45 = *ctx.vars.get(&0x45).unwrap();
        assert_eq!((v45 >> 16) & 0xFF, 0); // rail type present on crossing
    }

    #[test]
    fn road_ctx_snow_and_tram() {
        let map = Map::new_flat(4, 4, 0);
        let c = TileCoord::new(0, 0);
        let mut tile = set_tram_road_type_on_tile(plain_road(), Some(RoadType::Tram));
        tile.m7 |= 0x20;
        let catalog = crate::road_type::vanilla_road_type_catalog();
        let ctx = action2_eval_ctx_for_road_tile(&map, tile, c, Climate::SubArctic, None, &catalog);
        assert_eq!(ctx.vars.get(&0x40), Some(&4));
        let v45 = *ctx.vars.get(&0x45).unwrap();
        assert_eq!((v45 >> 8) & 0xFF, 1); // tram id
    }

    #[test]
    fn road_ctx_var45_uses_translation_table() {
        use crate::newgrf_type_tables::GrfTypeTranslationTables;
        let map = Map::new_flat(4, 4, 0);
        let c = TileCoord::new(1, 1);
        let tile = set_road_type_on_tile(plain_road(), RoadType::Road);
        let catalog = crate::road_type::vanilla_road_type_catalog();
        let tables = GrfTypeTranslationTables {
            road: vec![*b"COBB", *b"ROAD"],
            ..Default::default()
        };
        let ctx = action2_eval_ctx_for_road_tile(
            &map,
            tile,
            c,
            Climate::Temperate,
            Some(&tables),
            &catalog,
        );
        let v45 = *ctx.vars.get(&0x45).unwrap();
        assert_eq!(v45 & 0xFF, 1); // ROAD at index 1
    }
}
