use bevy::prelude::*;
use openttdrs_core::{Map, Tile, TileCoord, TileKind};

use crate::iso::{iso, tile_slope_and_min_z};

/// `true` si algún vecino ortogonal no es agua ni vacío (borde mar/tierra o río).
///
/// Los exports `.ottdmap` a veces dejan `m5=0` en toda el agua y se pierde
/// `WaterTileType::Coast` en bits 4–7; sin esto solo se pinta agua plana en la orilla.
fn water_tile_touches_land(map: &Map, tx: u32, ty: u32, mw: u32, mh: u32) -> bool {
    let is_land = |x: i32, y: i32| -> bool {
        if x < 0 || y < 0 || x >= mw as i32 || y >= mh as i32 {
            return false;
        }
        map.get(TileCoord::new(x, y))
            .is_some_and(|t| t.kind != TileKind::Water && t.kind != TileKind::Void)
    };
    let x = tx as i32;
    let y = ty as i32;
    is_land(x - 1, y) || is_land(x + 1, y) || is_land(x, y - 1) || is_land(x, y + 1)
}

#[derive(Clone, Copy)]
pub(crate) struct TileRenderInfo {
    pub(crate) tileh: u8,
    pub(crate) base_z: u8,
    pub(crate) use_shore: bool,
}

pub(crate) struct RenderGrid {
    width: u32,
    tiles: Vec<TileRenderInfo>,
}

impl RenderGrid {
    pub(crate) fn from_map(map: &Map, mw: u32, mh: u32) -> Self {
        let mut tiles = vec![
            TileRenderInfo {
                tileh: 0,
                base_z: 0,
                use_shore: false,
            };
            (mw * mh) as usize
        ];

        for ty in 0..mh {
            for tx in 0..mw {
                let idx = (ty * mw + tx) as usize;
                let (tileh, base_z) = tile_slope_and_min_z(map, tx, ty);
                let c = TileCoord::new(tx as i32, ty as i32);
                let tile = map.get(c);
                let kind = tile.map_or(TileKind::Grass, |t| t.kind);
                let use_shore = if kind == TileKind::Water {
                    let m5_w = tile.map_or(0u8, |t| t.m5);
                    let water_tile_type = (m5_w >> 4) & 0x0F;
                    water_tile_type == 1
                        || (water_tile_type == 0 && water_tile_touches_land(map, tx, ty, mw, mh))
                } else {
                    false
                };
                tiles[idx] = TileRenderInfo {
                    tileh,
                    base_z,
                    use_shore,
                };
            }
        }

        Self { width: mw, tiles }
    }

    fn get(&self, tx: u32, ty: u32) -> TileRenderInfo {
        self.tiles[(ty * self.width + tx) as usize]
    }
}

pub(crate) struct TileRenderContext {
    pub(crate) tx: u32,
    pub(crate) ty: u32,
    pub(crate) coord: TileCoord,
    pub(crate) tile: Option<Tile>,
    pub(crate) kind: TileKind,
    pub(crate) info: TileRenderInfo,
    pub(crate) iso_pos: Vec2,
}

impl TileRenderContext {
    pub(crate) fn new(map: &Map, grid: &RenderGrid, tx: u32, ty: u32) -> Self {
        let coord = TileCoord::new(tx as i32, ty as i32);
        let tile = map.get(coord);
        let kind = tile.map_or(TileKind::Grass, |t| t.kind);
        Self {
            tx,
            ty,
            coord,
            tile,
            kind,
            info: grid.get(tx, ty),
            iso_pos: iso(tx as i32, ty as i32),
        }
    }

    pub(crate) fn tx_i32(&self) -> i32 {
        self.tx as i32
    }

    pub(crate) fn ty_i32(&self) -> i32 {
        self.ty as i32
    }
}
