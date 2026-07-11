//! Cercas de vía (`DrawTrackDetails` / `SPR_TRACK_FENCE_*`).

use openttdrs_core::{Map, TileCoord, TileKind};

/// `RailGroundType` fence values (`rail_map.h`) — nibble bajo de `m3hi` (= m4 OTTD).
pub const RAIL_GROUND_FENCE_NW: u8 = 2;
pub const RAIL_GROUND_FENCE_SE: u8 = 3;
pub const RAIL_GROUND_FENCE_SENW: u8 = 4;
pub const RAIL_GROUND_FENCE_NE: u8 = 5;
pub const RAIL_GROUND_FENCE_SW: u8 = 6;
pub const RAIL_GROUND_FENCE_NESW: u8 = 7;
pub const RAIL_GROUND_FENCE_VERT1: u8 = 8;
pub const RAIL_GROUND_FENCE_VERT2: u8 = 9;
pub const RAIL_GROUND_FENCE_HORIZ1: u8 = 10;
pub const RAIL_GROUND_FENCE_HORIZ2: u8 = 11;

/// (sprite_index 0..7, dx, dy) para colocar la cerca en la tesela.
pub type TrackFenceDraw = (usize, f32, f32);

/// Bits de vía que tocan cada borde (análogo a `TRACK_BIT_3WAY_*`).
const EDGE_NE: u8 = 0x01 | 0x04 | 0x20; // X | UPPER | RIGHT
const EDGE_SE: u8 = 0x02 | 0x08 | 0x20; // Y | LOWER | RIGHT
const EDGE_SW: u8 = 0x01 | 0x08 | 0x10; // X | LOWER | LEFT
const EDGE_NW: u8 = 0x02 | 0x04 | 0x10; // Y | UPPER | LEFT

fn neighbor_wants_fence(map: &Map, c: TileCoord, dx: i32, dy: i32) -> bool {
    let n = TileCoord::new(c.x + dx, c.y + dy);
    let Some(t) = map.get(n) else {
        return true;
    };
    // Misma compañía de vía contigua: sin cerca. Resto (hierba, casa, carretera…): cerca.
    !matches!(t.kind, TileKind::Rail | TileKind::RailDepot)
}

/// Inferencia de cercas al estilo `TileLoop_Rail` (sin persistir en el mapa).
#[must_use]
pub fn infer_track_fence_draws(map: &Map, c: TileCoord, track_bits: u8) -> Vec<TrackFenceDraw> {
    let mut fences = Vec::new();
    // DiagDirection NE/SE/SW/NW → offsets de tesela del port.
    let edges = [
        ((1, 0), EDGE_NE, 1usize, 1.0_f32, 8.0_f32),
        ((0, 1), EDGE_SE, 0usize, 8.0_f32, 15.0_f32),
        ((-1, 0), EDGE_SW, 1usize, 15.0_f32, 8.0_f32),
        ((0, -1), EDGE_NW, 0usize, 8.0_f32, 1.0_f32),
    ];
    for &((dx, dy), edge_bits, sprite, fx, fy) in &edges {
        if track_bits & edge_bits != 0 {
            continue;
        }
        if neighbor_wants_fence(map, c, dx, dy) {
            fences.push((sprite, fx, fy));
        }
    }
    fences
}

/// Cercas desde `RailGroundType` almacenado en `m3hi` (saves OTTD).
#[must_use]
pub fn track_fence_draws_from_ground(ground: u8) -> Vec<TrackFenceDraw> {
    match ground {
        RAIL_GROUND_FENCE_NW => vec![(0, 8.0, 1.0)],
        RAIL_GROUND_FENCE_SE => vec![(0, 8.0, 15.0)],
        RAIL_GROUND_FENCE_SENW => vec![(0, 8.0, 1.0), (0, 8.0, 15.0)],
        RAIL_GROUND_FENCE_NE => vec![(1, 1.0, 8.0)],
        RAIL_GROUND_FENCE_SW => vec![(1, 15.0, 8.0)],
        RAIL_GROUND_FENCE_NESW => vec![(1, 1.0, 8.0), (1, 15.0, 8.0)],
        RAIL_GROUND_FENCE_VERT1 | RAIL_GROUND_FENCE_VERT2 => vec![(2, 8.0, 8.0)],
        RAIL_GROUND_FENCE_HORIZ1 | RAIL_GROUND_FENCE_HORIZ2 => vec![(3, 8.0, 8.0)],
        _ => Vec::new(),
    }
}

/// Combina ground almacenado (si hay cercas) o inferencia por vecinos.
#[must_use]
pub fn track_fence_draws_for_tile(
    map: &Map,
    c: TileCoord,
    track_bits: u8,
    m3hi: u8,
) -> Vec<TrackFenceDraw> {
    let ground = m3hi & 0x0F;
    let stored = track_fence_draws_from_ground(ground);
    if stored.is_empty() {
        infer_track_fence_draws(map, c, track_bits)
    } else {
        stored
    }
}

/// (w, h, xrel, yrel) aproximados para `track_fence_*.png` (33×21 en OpenGFX2).
pub const TRACK_FENCE_META: (f32, f32, f32, f32) = (33.0, 21.0, -16.0, -18.0);

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use openttdrs_core::GameState;

    #[test]
    fn fence_nw_ground_emits_one_draw() {
        let d = track_fence_draws_from_ground(RAIL_GROUND_FENCE_NW);
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].0, 0);
    }

    #[test]
    fn infer_fence_against_grass() {
        let mut state = GameState::new(8, 8);
        let c = TileCoord::new(2, 2);
        let mut tile = state.map.get(c).unwrap();
        tile.kind = TileKind::Rail;
        tile.m5 = 0x01;
        state.map.set_tile(c, tile).unwrap();
        let draws = infer_track_fence_draws(&state.map, c, 0x01);
        assert!(!draws.is_empty(), "vía aislada debería tener cercas");
    }
}
