// Generado por scripts/gen_shore_full_set.py — NO EDITAR A MANO.
//
// Set completo de orillas (SPR_SHORE_BASE + 0..17) de OpenGFX. El GRF
// extra aporta Action5 0x0D y el base los ocho sprites clásicos.
// `SHORE_META` son los offsets NFO (w, h, xrel, yrel) y
// `TILEH_TO_SHORE_SPRITE` es la tabla
// `tileh_to_shoresprite` de `water_cmd.cpp` (tabla completa 0..31).

/// Sprites del set de orillas (`SHORE_SPRITE_COUNT` en upstream).
pub const SHORE_SPRITE_COUNT: usize = 18;

/// (w, h, xrel, yrel) NFO por slot de `SPR_SHORE_BASE`.
pub static SHORE_META: [(f32, f32, f32, f32); 18] = [
    (64.0, 15.0, -31.0, 0.0),   // slot 0
    (64.0, 31.0, -31.0, 0.0),   // slot 1
    (64.0, 23.0, -31.0, 0.0),   // slot 2
    (64.0, 23.0, -31.0, 0.0),   // slot 3
    (64.0, 31.0, -31.0, 0.0),   // slot 4
    (64.0, 31.0, -31.0, -8.0),  // slot 5
    (64.0, 23.0, -31.0, 0.0),   // slot 6
    (64.0, 23.0, -31.0, 0.0),   // slot 7
    (64.0, 39.0, -31.0, -8.0),  // slot 8
    (64.0, 39.0, -31.0, -8.0),  // slot 9
    (64.0, 47.0, -31.0, -16.0), // slot 10
    (64.0, 31.0, -31.0, -8.0),  // slot 11
    (64.0, 39.0, -31.0, -8.0),  // slot 12
    (64.0, 39.0, -31.0, -8.0),  // slot 13
    (64.0, 31.0, -31.0, -8.0),  // slot 14
    (64.0, 31.0, -31.0, -8.0),  // slot 15
    (64.0, 31.0, -31.0, 0.0),   // slot 16
    (64.0, 31.0, -31.0, -8.0),  // slot 17
];

/// `tileh` (0..31) → slot de sprite de orilla (`tileh_to_shoresprite`).
pub static TILEH_TO_SHORE_SPRITE: [u8; 32] = [
    0, 1, 2, 3, 4, 16, 6, 7, 8, 9, 17, 11, 12, 13, 14, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 5, 0,
    10, 15, 0,
];
