use openttdrs_core::{GameState, TileCoord, TileKind};

pub(crate) fn distribute_tile_kinds(state: &mut GameState, seed: u64) {
    let (mw, mh) = state.map.dimensions();
    for y in 0..mh {
        for x in 0..mw {
            let kind = tile_kind_hash(x, y, seed);
            let c = TileCoord::new(x as i32, y as i32);
            let _ = state.map.set_kind(c, kind);
        }
    }
}

fn tile_kind_hash(x: u32, y: u32, seed: u64) -> TileKind {
    let mut h = seed
        .wrapping_add(u64::from(x).wrapping_mul(0x9E37_79B9_7F4A_7C15))
        .wrapping_add(u64::from(y).wrapping_mul(0x6C62_272E_07BB_0142));
    h ^= h >> 30;
    h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h ^= h >> 27;
    h = h.wrapping_mul(0x94D0_49BB_1331_11EB);
    h ^= h >> 31;
    match h % 10 {
        0 | 1 => TileKind::Water,
        2 | 3 => TileKind::Forest,
        4 => TileKind::CoalField,
        _ => TileKind::Grass,
    }
}
