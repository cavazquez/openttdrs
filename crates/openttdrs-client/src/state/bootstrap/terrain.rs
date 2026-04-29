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

#[cfg(test)]
mod tests {
    use super::distribute_tile_kinds;
    use openttdrs_core::{GameState, TileCoord, TileKind};

    fn snapshot(state: &GameState) -> Vec<TileKind> {
        let (mw, mh) = state.map.dimensions();
        let mut out = Vec::with_capacity((mw * mh) as usize);
        for y in 0..mh {
            for x in 0..mw {
                out.push(
                    state
                        .map
                        .get_kind(TileCoord::new(x as i32, y as i32))
                        .unwrap_or(TileKind::Void),
                );
            }
        }
        out
    }

    #[test]
    fn distribute_tile_kinds_is_deterministic_for_same_seed() {
        let mut a = GameState::new(8, 8);
        let mut b = GameState::new(8, 8);
        distribute_tile_kinds(&mut a, 42);
        distribute_tile_kinds(&mut b, 42);
        assert_eq!(snapshot(&a), snapshot(&b));
    }

    #[test]
    fn distribute_tile_kinds_only_generates_expected_kinds() {
        let mut state = GameState::new(12, 12);
        distribute_tile_kinds(&mut state, 1234);
        for kind in snapshot(&state) {
            assert!(matches!(
                kind,
                TileKind::Water | TileKind::Forest | TileKind::CoalField | TileKind::Grass
            ));
        }
    }
}
