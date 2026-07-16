//! Serialización de planos del mapa (MAPT, MAPH, etc.).

use crate::map::{Map, Tile, TileCoord, TileKind};

pub(super) struct MapPlanes {
    pub mapt: Vec<u8>,
    pub maph: Vec<u8>,
    pub mapo: Vec<u8>,
    pub map2: Vec<u8>,
    pub m3lo: Vec<u8>,
    pub m3hi: Vec<u8>,
    pub map5: Vec<u8>,
    pub mape: Vec<u8>,
    pub map7: Vec<u8>,
    pub map8: Vec<u8>,
}

pub(super) fn collect_planes(map: &Map, w: u32, h: u32, n: usize) -> MapPlanes {
    let mut planes = MapPlanes {
        mapt: vec![0; n],
        maph: vec![0; n],
        mapo: vec![0; n],
        map2: vec![0; n * 2],
        m3lo: vec![0; n],
        m3hi: vec![0; n],
        map5: vec![0; n],
        mape: vec![0; n],
        map7: vec![0; n],
        map8: vec![0; n * 2],
    };
    for y in 0..h {
        for x in 0..w {
            let i = (y * w + x) as usize;
            let Some(tile) = map.get(TileCoord::new(x.cast_signed(), y.cast_signed())) else {
                continue;
            };
            planes.mapt[i] = tile_mapt(tile);
            planes.maph[i] = tile.height;
            planes.mapo[i] = tile.m1;
            // MAP2 en el save: u16 big-endian (byte alto = m2_hi, bajo = m2).
            planes.map2[i * 2] = tile.m2_hi;
            planes.map2[i * 2 + 1] = tile.m2;
            planes.m3lo[i] = tile.m3;
            planes.m3hi[i] = tile.m3hi;
            planes.map5[i] = tile.m5;
            planes.mape[i] = tile.m6;
            planes.map7[i] = tile.m7;
            // MAP8 en el save: u16 big-endian; en memoria `Tile.m8` es LE.
            let m8 = tile.m8.to_be_bytes();
            planes.map8[i * 2] = m8[0];
            planes.map8[i * 2 + 1] = m8[1];
        }
    }
    planes
}

/// Byte MAPT: conserva el del tile si está; si no, deriva del [`TileKind`].
fn tile_mapt(tile: Tile) -> u8 {
    if tile.mapt != 0 {
        return tile.mapt;
    }
    match tile.kind {
        TileKind::Grass | TileKind::CoalField => 0x00,
        TileKind::Rail | TileKind::RailDepot => 0x10,
        TileKind::Road | TileKind::RoadDepot => 0x20,
        TileKind::House => 0x30,
        TileKind::Forest => 0x40,
        TileKind::Station | TileKind::Airport => 0x50,
        TileKind::Water | TileKind::ShipDepot => 0x60,
        TileKind::Void => 0x70,
        TileKind::Industry => 0x80,
        TileKind::RailTunnel
        | TileKind::RoadTunnel
        | TileKind::RailBridge
        | TileKind::RoadBridge => 0x90,
        TileKind::Unknown(t) => (t & 0x0F) << 4,
    }
}
