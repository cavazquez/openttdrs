//! Contexto Action2 para teselas de estación (vars de runtime).

use crate::map::{Map, TileCoord, TileKind, tile_slope_and_z};
use crate::newgrf_sprites::Action2EvalCtx;
use crate::newgrf_type_tables::{GrfTypeTranslationTables, reverse_rail_type};
use crate::rail_type::rail_type_from_tile;
use crate::station::{Station, station_at_tile, station_type_from_m6};
use crate::world_gen::Climate;

/// Contexto Action2 para dibujar / resolver sprites de una tesela de estación.
///
/// MVP: `40` (plataforma), `42` (terreno+rail), `43` (owner), `4A` (frame),
/// `5F` (random), `10` (m5/tileh), `67` (land info tesela actual, param 0).
#[must_use]
pub fn action2_eval_ctx_for_station_tile(
    map: &Map,
    stations: &[Station],
    coord: TileCoord,
    owner_colour: u8,
    climate: Climate,
    type_tables: Option<&GrfTypeTranslationTables>,
) -> Action2EvalCtx {
    let mut ctx = Action2EvalCtx::default();
    let Some(st) = station_at_tile(map, stations, coord) else {
        return ctx;
    };
    let tile = map.get(coord);
    let m5 = tile.map_or(0, |t| t.m5);
    let (tileh, z) = tile_slope_and_z(map, coord).unwrap_or((0, 0));

    let random = u32::from(st.newgrf_random_bits);
    ctx.random_bits = random;
    ctx.vars.insert(0x5F, random << 8);

    let nn_player = u32::from(st.owner.0);
    let c = u32::from(owner_colour & 0x0F);
    let var43 = nn_player | ((c | (c << 4)) << 24);
    ctx.vars.insert(0x43, var43);

    // Var 10: info adicional (m5 + tileh) para selección de sprites.
    ctx.vars
        .insert(0x10, u32::from(m5) | (u32::from(tileh) << 8));
    // `StationResolverObject::GetVariable(0x4A)`: frame persistido en MAP7.
    // También alimenta la selección Action2 del renderer después de CB140–142.
    ctx.vars.insert(0x4A, tile.map_or(0, |t| u32::from(t.m7)));

    ctx.vars
        .insert(0x40, platform_info_for_tile(map, stations, coord, m5));

    let terrain = terrain_type_for_tile(map, coord, climate, tile);
    let rail_tt = tile.map_or(0xFF_u32, |t| {
        if t.kind == TileKind::Station && station_type_from_m6(t.m6) == 0 {
            u32::from(reverse_rail_type(type_tables, rail_type_from_tile(t)))
        } else {
            0xFF
        }
    });
    ctx.vars.insert(0x42, terrain | (rail_tt << 8));

    // Var 67 param 0: land info de la tesela actual (sin offsets).
    let tile_type_byte = tile.map_or(0u32, |t| u32::from(tile_kind_as_ottd(t.kind)));
    let land = tile_type_byte << 24 | u32::from(z) << 16 | (terrain << 2) << 8 | u32::from(tileh);
    ctx.vars.insert(0x67, land);

    ctx
}

fn tile_kind_as_ottd(kind: TileKind) -> u8 {
    match kind {
        TileKind::Rail | TileKind::RailDepot | TileKind::RailTunnel | TileKind::RailBridge => 1,
        TileKind::Road | TileKind::RoadDepot | TileKind::RoadTunnel | TileKind::RoadBridge => 2,
        TileKind::House => 3,
        TileKind::Forest => 4, // MP_TREES
        TileKind::Station | TileKind::Airport => 5,
        TileKind::Water | TileKind::ShipDepot => 6,
        TileKind::Void => 7,
        TileKind::Industry => 8,
        // MP_CLEAR y desconocidos
        TileKind::Grass | TileKind::CoalField | TileKind::Unknown(_) => 0,
    }
}

fn terrain_type_for_tile(
    map: &Map,
    coord: TileCoord,
    climate: Climate,
    tile: Option<crate::map::Tile>,
) -> u32 {
    if climate.uses_snow_ground() {
        return 4;
    }
    if climate.uses_desert_patches() {
        // Aproximación: bit MAP7 nieve/desierto en road; en clear m5 desert.
        if let Some(t) = tile {
            if t.kind == TileKind::Road && (t.m7 & 0x20) != 0 {
                return 1;
            }
            if t.kind == TileKind::Grass {
                let ground = t.m5 & 0x07;
                if ground == crate::world_gen::CLEAR_GROUND_DESERT {
                    return 1;
                }
            }
        }
        // Vecino clear desert (estación suele tapar el clear).
        for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
            let n = TileCoord::new(coord.x + dx, coord.y + dy);
            if map.get(n).is_some_and(|t| {
                t.kind == TileKind::Grass && (t.m5 & 0x07) == crate::world_gen::CLEAR_GROUND_DESERT
            }) {
                return 1;
            }
        }
    }
    0
}

fn is_rail_platform_tile(map: &Map, c: TileCoord) -> bool {
    map.get(c)
        .is_some_and(|t| t.kind == TileKind::Station && station_type_from_m6(t.m6) == 0)
}

fn same_station(a: &Station, b: &Station) -> bool {
    a.pos == b.pos && a.stop_kind == b.stop_kind
}

fn find_rail_station_end(
    map: &Map,
    stations: &[Station],
    start: TileCoord,
    dx: i32,
    dy: i32,
) -> TileCoord {
    let Some(st) = station_at_tile(map, stations, start) else {
        return start;
    };
    let mut tile = start;
    loop {
        let next = TileCoord::new(tile.x + dx, tile.y + dy);
        if !is_rail_platform_tile(map, next) {
            break;
        }
        let Some(other) = station_at_tile(map, stations, next) else {
            break;
        };
        if !same_station(st, other) {
            break;
        }
        tile = next;
    }
    tile
}

fn pack_platform_info(gfx: u8, platforms: i32, length: i32, platform: i32, position: i32) -> u32 {
    let mut retval = 0u32;
    let len = length.max(1);
    let plats = platforms.max(1);
    let p = position.clamp(0, 15).cast_unsigned();
    let plat = platform.clamp(0, 15).cast_unsigned();
    retval |= p; // P
    retval |= (len - position - 1).clamp(0, 15).cast_unsigned() << 4; // p
    retval |= plat << 8; // C
    retval |= (plats - platform - 1).clamp(0, 15).cast_unsigned() << 12; // c
    retval |= len.min(15).cast_unsigned() << 16; // L
    retval |= plats.min(15).cast_unsigned() << 20; // N
    retval |= u32::from(gfx) << 24; // T
    retval
}

fn platform_info_for_tile(map: &Map, stations: &[Station], coord: TileCoord, m5: u8) -> u32 {
    if !is_rail_platform_tile(map, coord) {
        // Waypoints / no-rail: layout 1×1.
        return pack_platform_info(m5 & 0x3F, 1, 1, 0, 0);
    }
    let sx = find_rail_station_end(map, stations, coord, -1, 0).x;
    let sy = find_rail_station_end(map, stations, coord, 0, -1).y;
    let ex = find_rail_station_end(map, stations, coord, 1, 0).x + 1;
    let ey = find_rail_station_end(map, stations, coord, 0, 1).y + 1;

    let mut tx = coord.x - sx;
    let mut ty = coord.y - sy;
    let mut width = ex - sx;
    let mut height = ey - sy;

    let axis_y = m5 & 1 != 0;
    // Axis X: longitud en X, andenes en Y → swap como OpenTTD.
    if !axis_y {
        std::mem::swap(&mut width, &mut height);
        std::mem::swap(&mut tx, &mut ty);
    }

    pack_platform_info(m5 & 0x3F, width, height, tx, ty)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::company::CompanyId;
    use crate::map::{Map, Tile, TileKind};
    use crate::station::{Station, StopKind};

    fn rail_station_tile(m5: u8) -> Tile {
        Tile {
            height: 0,
            kind: TileKind::Station,
            mapt: 0,
            m5,
            m1: 0,
            m6: 0, // rail
            m8: 0,
            m3: 0,
            m2: 0,
            m2_hi: 0,
            m7: 0,
            m3hi: 0,
        }
    }

    #[test]
    fn station_dynamic_vars_share_action2_eval_ctx_for_228() {
        // #228: vars dinámicas de estación alimentan el mismo `Action2EvalCtx`
        // que el resolver variational/callback (no un camino paralelo).
        let mut map = Map::new_flat(8, 8, 0);
        let c = TileCoord::new(3, 3);
        let mut tile = rail_station_tile(2);
        tile.m7 = 9;
        map.set_tile(c, tile).unwrap();
        let mut st = Station::new_with_kind(c, StopKind::RailStation);
        st.newgrf_random_bits = 0x42;
        let ctx = action2_eval_ctx_for_station_tile(&map, &[st], c, 1, Climate::Temperate, None);
        assert!(ctx.vars.contains_key(&0x40));
        assert!(ctx.vars.contains_key(&0x42));
        assert!(ctx.vars.contains_key(&0x43));
        assert!(ctx.vars.contains_key(&0x4A));
        assert!(ctx.vars.contains_key(&0x5F));
        assert!(ctx.vars.contains_key(&0x10));
        assert!(ctx.vars.contains_key(&0x67));
        assert_eq!(ctx.random_bits, 0x42);
        assert_eq!(ctx.vars.get(&0x4A), Some(&9));
    }

    #[test]
    fn station_ctx_var40_single_tile() {
        let mut map = Map::new_flat(8, 8, 0);
        let c = TileCoord::new(3, 3);
        map.set_tile(c, rail_station_tile(0)).unwrap();
        let mut st = Station::new_with_kind(c, StopKind::RailStation);
        st.newgrf_random_bits = 0xAB;
        st.owner = CompanyId(2);
        let ctx = action2_eval_ctx_for_station_tile(&map, &[st], c, 4, Climate::Temperate, None);
        assert_eq!(ctx.random_bits, 0xAB);
        assert_eq!(ctx.vars.get(&0x5F), Some(&(0xAB << 8)));
        let v40 = *ctx.vars.get(&0x40).unwrap();
        assert_eq!(v40 & 0x0F, 0, "P=0");
        assert_eq!((v40 >> 16) & 0x0F, 1, "L=1");
        assert_eq!((v40 >> 20) & 0x0F, 1, "N=1");
        let v43 = *ctx.vars.get(&0x43).unwrap();
        assert_eq!(v43 & 0xFF, 2);
        assert_eq!((v43 >> 24) & 0xFF, 0x44);
        assert_eq!(ctx.vars.get(&0x42), Some(&0)); // grass + rail 0
        assert_eq!(ctx.vars.get(&0x4A), Some(&0)); // frame MAP7
    }

    #[test]
    fn station_ctx_var40_platform_length() {
        let mut map = Map::new_flat(10, 10, 0);
        // Eje X (m5 par): 3 teselas en X.
        for x in 2..5 {
            map.set_tile(TileCoord::new(x, 4), rail_station_tile(0))
                .unwrap();
        }
        let st = Station::new_with_kind(TileCoord::new(2, 4), StopKind::RailStation);
        let mid = TileCoord::new(3, 4);
        let ctx = action2_eval_ctx_for_station_tile(&map, &[st], mid, 0, Climate::Temperate, None);
        let v40 = *ctx.vars.get(&0x40).unwrap();
        assert_eq!(v40 & 0x0F, 1, "P=1 (medio)");
        assert_eq!((v40 >> 4) & 0x0F, 1, "p=1");
        assert_eq!((v40 >> 16) & 0x0F, 3, "L=3");
        assert_eq!((v40 >> 20) & 0x0F, 1, "N=1");
    }

    #[test]
    fn station_ctx_snow_terrain() {
        let mut map = Map::new_flat(4, 4, 0);
        let c = TileCoord::new(1, 1);
        map.set_tile(c, rail_station_tile(0)).unwrap();
        let st = Station::new_with_kind(c, StopKind::RailStation);
        let ctx = action2_eval_ctx_for_station_tile(&map, &[st], c, 0, Climate::SubArctic, None);
        assert_eq!(ctx.vars.get(&0x42).map(|v| v & 0xFF), Some(4));
    }

    #[test]
    fn station_ctx_var42_uses_rail_translation() {
        use crate::newgrf_type_tables::GrfTypeTranslationTables;
        use crate::rail_type::{RailType, set_rail_type_on_tile};
        let mut map = Map::new_flat(4, 4, 0);
        let c = TileCoord::new(1, 1);
        let mut tile = rail_station_tile(0);
        tile = set_rail_type_on_tile(tile, RailType::Electric);
        map.set_tile(c, tile).unwrap();
        let st = Station::new_with_kind(c, StopKind::RailStation);
        let tables = GrfTypeTranslationTables {
            rail: vec![*b"MONO", *b"ELRL", *b"RAIL"],
            ..Default::default()
        };
        let ctx =
            action2_eval_ctx_for_station_tile(&map, &[st], c, 0, Climate::Temperate, Some(&tables));
        let v42 = *ctx.vars.get(&0x42).unwrap();
        assert_eq!((v42 >> 8) & 0xFF, 1); // ELRL at index 1
    }
}
