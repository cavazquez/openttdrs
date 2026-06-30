//! Tramos intermedios de puente sobre teselas con `IsBridgeAbove` en `mapt`.

use bevy::prelude::*;
use openttdrs_core::{Map, bridge_above_axis_from_mapt};

use crate::render::{TileRenderContext, WorldAssets};

use super::bridge_draw::{bridge_span_at, spawn_bridge_deck};

/// Dibuja el tablero si la tesela tiene un puente por encima (no rampa).
pub(crate) fn spawn_bridge_middle(
    commands: &mut Commands,
    map: &Map,
    dims: (u32, u32),
    assets: &WorldAssets,
    ctx: &TileRenderContext,
) {
    let Some(tile) = ctx.tile else {
        return;
    };
    if bridge_above_axis_from_mapt(tile.mapt).is_none() {
        return;
    }
    let Some(span) = bridge_span_at(map, ctx.coord, dims) else {
        return;
    };
    spawn_bridge_deck(commands, assets, ctx, &span, true);
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use openttdrs_core::{
        BridgeType, Map, Tile, TileCoord, TileKind, bridge_above_axis_from_mapt,
        set_bridge_middle_mapt, set_bridge_type_m6,
    };

    fn ramp_tile_template(m5: u8) -> Tile {
        Tile {
            height: 0,
            kind: TileKind::RoadBridge,
            mapt: 0x90,
            m5,
            m1: 0,
            m6: set_bridge_type_m6(0, BridgeType::CantileverRed),
            m8: 0,
            m3: 0,
            m2: 0,
            m2_hi: 0,
            m7: 0,
            m3hi: 0,
        }
    }

    #[test]
    fn span_at_resolves_on_ramp_endpoints() {
        let mut map = Map::new_flat(8, 8, 0);
        let c = |x: i32, y: i32| TileCoord::new(x, y);
        map.set_tile(c(1, 2), ramp_tile_template(0x86))
            .expect("ramp");
        map.set_tile(c(5, 2), ramp_tile_template(0x84))
            .expect("ramp");
        for x in 2..=4 {
            let water = Tile {
                height: 0,
                kind: TileKind::Water,
                mapt: set_bridge_middle_mapt(0x60, false),
                m5: 0,
                m1: 0,
                m6: set_bridge_type_m6(0, BridgeType::CantileverRed),
                m8: 0,
                m3: 0,
                m2: 0,
                m2_hi: 0,
                m7: 0,
                m3hi: 0,
            };
            map.set_tile(c(x, 2), water).expect("span");
        }
        let dims = map.dimensions();
        for x in 1..=5 {
            let span = bridge_span_at(&map, c(x, 2), dims).expect("span");
            assert_eq!(span.bridge_type, BridgeType::CantileverRed);
            assert_eq!(span.axis, 0);
        }
        assert_eq!(
            bridge_span_at(&map, c(1, 2), dims).unwrap().piece,
            openttdrs_core::BridgePiece::North
        );
        assert_eq!(
            bridge_span_at(&map, c(5, 2), dims).unwrap().piece,
            openttdrs_core::BridgePiece::South
        );
    }

    #[test]
    fn bridge_above_reads_mapt_bits() {
        assert_eq!(bridge_above_axis_from_mapt(0x64), Some(false));
        assert_eq!(bridge_above_axis_from_mapt(0x68), Some(true));
        assert_eq!(bridge_above_axis_from_mapt(0x60), None);
    }
}
