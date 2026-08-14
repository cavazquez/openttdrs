//! GENERADO por scripts/extract_oneway_road_sprites.py — no editar a mano.
//!
//! Fallback Action5 0x09 de `openttd.grf` (OpenTTD 15.3). El GRF
//! oficial es 8bpp también cuando la base seleccionada es OpenGFX2 32bpp.

pub const SPR_ONEWAY_BASE: u32 = 6105;
pub const ONEWAY_ROAD_SPRITE_COUNT: usize = 18;

/// `(w, h, xrel, yrel)` NFO de `oneway_{00..17}.png`.
pub static ONEWAY_ROAD_SPRITE_META: [(f32, f32, f32, f32); ONEWAY_ROAD_SPRITE_COUNT] = [
    (24.0, 16.0, -10.0, -9.0),
    (24.0, 16.0, -13.0, -7.0),
    (24.0, 16.0, -12.0, -8.0),
    (24.0, 16.0, -15.0, -10.0),
    (24.0, 16.0, -12.0, -9.0),
    (24.0, 16.0, -11.0, -8.0),
    (24.0, 16.0, -13.0, -10.0),
    (24.0, 16.0, -12.0, -8.0),
    (24.0, 16.0, -12.0, -9.0),
    (24.0, 16.0, -11.0, -8.0),
    (24.0, 16.0, -9.0, -10.0),
    (24.0, 16.0, -10.0, -9.0),
    (24.0, 16.0, -8.0, -11.0),
    (24.0, 16.0, -11.0, -5.0),
    (24.0, 16.0, -12.0, -8.0),
    (24.0, 16.0, -12.0, -5.0),
    (24.0, 16.0, -14.0, -10.0),
    (24.0, 16.0, -12.0, -8.0),
];

/// ID global que el draw proc de OpenTTD ve para un slot Action5 0x09.
#[must_use]
pub const fn oneway_road_sprite_id(slot: usize) -> Option<u32> {
    if slot < ONEWAY_ROAD_SPRITE_COUNT {
        Some(SPR_ONEWAY_BASE + slot as u32)
    } else {
        None
    }
}
