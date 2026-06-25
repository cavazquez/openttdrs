//! Tile loop de industrias vanilla — avance de `m3hi` (frame) y `gfx` (pozos).
//!
//! Paridad parcial con `AnimateTile_Industry` / `TileLoop_Industry` de OpenTTD:
//! torres de mina (`anim_state`) y pozos de petróleo (gfx 30–32).

use super::{Map, Tile, TileCoord, TileKind};

/// `GFX_COAL_MINE_TOWER_ANIMATED`
pub const GFX_COAL_MINE_TOWER_ANIMATED: u16 = 1;
/// `GFX_OILWELL_ANIMATED_1`
pub const GFX_OILWELL_ANIMATED_1: u16 = 30;
/// `GFX_OILWELL_ANIMATED_2`
pub const GFX_OILWELL_ANIMATED_2: u16 = 31;
/// `GFX_OILWELL_ANIMATED_3`
pub const GFX_OILWELL_ANIMATED_3: u16 = 32;
/// `GFX_COPPER_MINE_TOWER_ANIMATED`
pub const GFX_COPPER_MINE_TOWER_ANIMATED: u16 = 48;
/// `GFX_GOLD_MINE_TOWER_ANIMATED`
pub const GFX_GOLD_MINE_TOWER_ANIMATED: u16 = 88;

const TOWER_ANIM_GFX: [u16; 3] = [
    GFX_COAL_MINE_TOWER_ANIMATED,
    GFX_COPPER_MINE_TOWER_ANIMATED,
    GFX_GOLD_MINE_TOWER_ANIMATED,
];

/// gfx de industria de 9 bits (`GetCleanIndustryGfx`).
#[must_use]
pub fn industry_gfx(tile: &Tile) -> u16 {
    u16::from(tile.m5) | (u16::from((tile.m6 >> 2) & 1) << 8)
}

/// Escribe gfx de 9 bits en `m5` / bit 8 en `m6`.
pub fn set_industry_gfx(tile: &mut Tile, gfx: u16) {
    tile.m5 = u8::try_from(gfx & 0xFF).unwrap_or(0);
    tile.m6 = (tile.m6 & !0x04) | (((gfx >> 8) & 1) as u8) << 2;
}

/// Industria terminada (`IsIndustryCompleted`).
#[must_use]
pub fn is_industry_completed(tile: &Tile) -> bool {
    tile.m1 & 0x80 != 0
}

/// `IndustryTileSpec.anim_state` (subconjunto vanilla en tabla de 131 gfx).
#[must_use]
pub fn industry_tile_anim_state(gfx: u16) -> bool {
    matches!(
        gfx,
        GFX_COAL_MINE_TOWER_ANIMATED
            | GFX_OILWELL_ANIMATED_1
            | GFX_OILWELL_ANIMATED_2
            | GFX_OILWELL_ANIMATED_3
            | GFX_COPPER_MINE_TOWER_ANIMATED
            | GFX_GOLD_MINE_TOWER_ANIMATED
    )
}

/// Frame de animación (`GetAnimationFrame & 3`).
#[must_use]
pub fn industry_animation_frame(m3hi: u8) -> u8 {
    m3hi & 3
}

fn tile_anim_phase(x: i32, y: i32) -> u64 {
    (u64::try_from(x.max(0)).unwrap_or(0))
        .wrapping_mul(17)
        .wrapping_add(u64::try_from(y.max(0)).unwrap_or(0).wrapping_mul(31))
}

fn advance_tower(tile: &mut Tile, tick: u64, x: i32, y: i32) -> bool {
    if !tick.is_multiple_of(2) {
        return false;
    }
    let phase = tile_anim_phase(x, y);
    if !tick.wrapping_add(phase).is_multiple_of(8) {
        return false;
    }
    let frame = industry_animation_frame(tile.m3hi);
    tile.m3hi = (tile.m3hi & !3) | ((frame + 1) & 3);
    true
}

fn advance_oil_well(tile: &mut Tile, gfx: u16, tick: u64) -> bool {
    if !tick.is_multiple_of(3) {
        return false;
    }
    let frame = industry_animation_frame(tile.m3hi);
    if frame + 1 >= 4 {
        tile.m3hi &= !3;
        let next = if gfx >= GFX_OILWELL_ANIMATED_3 {
            GFX_OILWELL_ANIMATED_1
        } else {
            gfx + 1
        };
        set_industry_gfx(tile, next);
    } else {
        tile.m3hi = (tile.m3hi & !3) | ((frame + 1) & 3);
    }
    true
}

fn industry_draw_proc_gfx(gfx: u16, m1: u8) -> u8 {
    let stage = if m1 & 0x80 != 0 {
        3usize
    } else {
        usize::from((m1 & 0x60) >> 5)
    }
    .min(3);
    match gfx {
        10 if stage == 3 => 5,
        143 if stage == 3 => 4,
        162 if stage >= 1 => 3,
        165 => 2,
        174 => 1,
        _ => 0,
    }
}

fn advance_draw_proc(tile: &mut Tile, proc: u8, tick: u64) -> bool {
    match proc {
        1 if is_industry_completed(tile) && tick.is_multiple_of(2) => {
            let m = u16::from(tile.m3hi).saturating_add(1);
            tile.m3hi = u8::try_from(if m >= 96 { 0 } else { m }).unwrap_or(0);
            true
        }
        2 if tick.is_multiple_of(4) => {
            tile.m3hi = tile.m3hi.wrapping_add(1) % 70;
            true
        }
        3 if tick.is_multiple_of(2) => {
            tile.m3hi = tile.m3hi.wrapping_add(1) % 40;
            true
        }
        4 if tick.is_multiple_of(2) => {
            tile.m3hi = tile.m3hi.wrapping_add(1) % 50;
            true
        }
        5 if is_industry_completed(tile) && tick.is_multiple_of(4) => {
            if tile.m3hi == 6 {
                tile.m3hi = 0;
            } else {
                tile.m3hi = tile.m3hi.saturating_add(1);
            }
            true
        }
        _ => false,
    }
}

fn advance_industry_tile(tile: &mut Tile, tick: u64, x: i32, y: i32) -> bool {
    if tile.kind != TileKind::Industry {
        return false;
    }
    let gfx = industry_gfx(tile);
    let proc = industry_draw_proc_gfx(gfx, tile.m1);
    if proc > 0 && advance_draw_proc(tile, proc, tick) {
        return true;
    }
    if !is_industry_completed(tile) {
        return false;
    }
    if TOWER_ANIM_GFX.contains(&gfx) {
        return advance_tower(tile, tick, x, y);
    }
    if (GFX_OILWELL_ANIMATED_1..=GFX_OILWELL_ANIMATED_3).contains(&gfx) {
        return advance_oil_well(tile, gfx, tick);
    }
    false
}

/// Avanza animaciones de teselas `MP_INDUSTRY` terminadas. Devuelve teselas mutadas.
pub fn advance_industry_tile_animations(map: &mut Map, tick: u64) -> u32 {
    let (w, h) = map.dimensions();
    let mut changed = 0u32;
    for uy in 0..h {
        for ux in 0..w {
            let coord = TileCoord::new(
                i32::try_from(ux).unwrap_or(0),
                i32::try_from(uy).unwrap_or(0),
            );
            let Some(mut tile) = map.get(coord) else {
                continue;
            };
            if advance_industry_tile(&mut tile, tick, coord.x, coord.y)
                && map.set_tile(coord, tile).is_ok()
            {
                changed += 1;
            }
        }
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    fn industry_tile(gfx: u16, m1: u8, m3hi: u8) -> Tile {
        let mut tile = Tile {
            height: 0,
            kind: TileKind::Industry,
            mapt: 0,
            m5: 0,
            m1,
            m6: 0,
            m8: 0,
            m3: 0,
            m2: 0,
            m2_hi: 0,
            m7: 0,
            m3hi,
        };
        set_industry_gfx(&mut tile, gfx);
        tile
    }

    #[test]
    fn tower_cycles_animation_frame() {
        let mut map = Map::new_flat(1, 1, 0);
        map.set_tile(
            TileCoord::new(0, 0),
            industry_tile(GFX_COAL_MINE_TOWER_ANIMATED, 0x80, 0),
        )
        .unwrap();
        let mut saw_change = false;
        let mut prev = map.get(TileCoord::new(0, 0)).unwrap().m3hi & 3;
        for tick in 1..=64 {
            advance_industry_tile_animations(&mut map, tick);
            let frame = map.get(TileCoord::new(0, 0)).unwrap().m3hi & 3;
            if frame != prev {
                saw_change = true;
            }
            prev = frame;
        }
        assert!(saw_change);
    }

    #[test]
    fn oil_well_advances_frame_or_gfx() {
        let mut map = Map::new_flat(1, 1, 0);
        map.set_tile(
            TileCoord::new(0, 0),
            industry_tile(GFX_OILWELL_ANIMATED_1, 0x80, 0),
        )
        .unwrap();
        for tick in 1..=24 {
            advance_industry_tile_animations(&mut map, tick);
        }
        let tile = map.get(TileCoord::new(0, 0)).unwrap();
        assert!(
            industry_animation_frame(tile.m3hi) > 0
                || industry_gfx(&tile) != GFX_OILWELL_ANIMATED_1
        );
    }

    #[test]
    fn construction_tiles_do_not_animate_tower() {
        let mut map = Map::new_flat(1, 1, 0);
        map.set_tile(
            TileCoord::new(0, 0),
            industry_tile(GFX_COAL_MINE_TOWER_ANIMATED, 0x01, 0),
        )
        .unwrap();
        advance_industry_tile_animations(&mut map, 8);
        assert_eq!(map.get(TileCoord::new(0, 0)).unwrap().m3hi, 0);
    }

    #[test]
    fn power_plant_sparks_cycle() {
        let mut map = Map::new_flat(1, 1, 0);
        map.set_tile(TileCoord::new(0, 0), industry_tile(10, 0x80, 0))
            .unwrap();
        advance_industry_tile_animations(&mut map, 4);
        assert_eq!(map.get(TileCoord::new(0, 0)).unwrap().m3hi, 1);
    }
}
