// Generado por scripts/gen_field_draw_data.py — NO EDITAR A MANO.
//
// Campos de cultivo (`SPR_FARMLAND_*`, 9 estados × 19 pendientes) y
// cercas (`SPR_HEDGE_*`, 6 tipos × 6 variantes) de `table/clear_land.h`.

/// Metadatos NFO de un sprite de cerca (`fence_{tipo}_{var}.png`).
#[derive(Debug, Clone, Copy)]
pub struct FenceSpriteMeta {
    pub w: f32,
    pub h: f32,
    pub xrel: f32,
    pub yrel: f32,
}

pub const FIELD_STATES: usize = 9;

pub static FENCE_SPRITE_META: [[FenceSpriteMeta; 6]; 6] = [
    [
        FenceSpriteMeta {
            w: 33.0,
            h: 21.0,
            xrel: -32.0,
            yrel: 10.0,
        },
        FenceSpriteMeta {
            w: 33.0,
            h: 21.0,
            xrel: 0.0,
            yrel: 10.0,
        },
        FenceSpriteMeta {
            w: 33.0,
            h: 29.0,
            xrel: -32.0,
            yrel: 2.0,
        },
        FenceSpriteMeta {
            w: 33.0,
            h: 29.0,
            xrel: 0.0,
            yrel: 2.0,
        },
        FenceSpriteMeta {
            w: 33.0,
            h: 13.0,
            xrel: -32.0,
            yrel: 18.0,
        },
        FenceSpriteMeta {
            w: 33.0,
            h: 13.0,
            xrel: 0.0,
            yrel: 18.0,
        },
    ],
    [
        FenceSpriteMeta {
            w: 33.0,
            h: 21.0,
            xrel: -32.0,
            yrel: 10.0,
        },
        FenceSpriteMeta {
            w: 33.0,
            h: 21.0,
            xrel: 0.0,
            yrel: 10.0,
        },
        FenceSpriteMeta {
            w: 33.0,
            h: 29.0,
            xrel: -32.0,
            yrel: 2.0,
        },
        FenceSpriteMeta {
            w: 33.0,
            h: 29.0,
            xrel: 0.0,
            yrel: 2.0,
        },
        FenceSpriteMeta {
            w: 33.0,
            h: 13.0,
            xrel: -32.0,
            yrel: 18.0,
        },
        FenceSpriteMeta {
            w: 33.0,
            h: 13.0,
            xrel: 0.0,
            yrel: 18.0,
        },
    ],
    [
        FenceSpriteMeta {
            w: 33.0,
            h: 19.0,
            xrel: -32.0,
            yrel: 12.0,
        },
        FenceSpriteMeta {
            w: 33.0,
            h: 19.0,
            xrel: 0.0,
            yrel: 12.0,
        },
        FenceSpriteMeta {
            w: 33.0,
            h: 27.0,
            xrel: -32.0,
            yrel: 4.0,
        },
        FenceSpriteMeta {
            w: 33.0,
            h: 27.0,
            xrel: 0.0,
            yrel: 4.0,
        },
        FenceSpriteMeta {
            w: 33.0,
            h: 11.0,
            xrel: -32.0,
            yrel: 20.0,
        },
        FenceSpriteMeta {
            w: 33.0,
            h: 11.0,
            xrel: 0.0,
            yrel: 20.0,
        },
    ],
    [
        FenceSpriteMeta {
            w: 33.0,
            h: 21.0,
            xrel: -32.0,
            yrel: 10.0,
        },
        FenceSpriteMeta {
            w: 33.0,
            h: 21.0,
            xrel: 0.0,
            yrel: 10.0,
        },
        FenceSpriteMeta {
            w: 33.0,
            h: 29.0,
            xrel: -32.0,
            yrel: 2.0,
        },
        FenceSpriteMeta {
            w: 33.0,
            h: 29.0,
            xrel: 0.0,
            yrel: 2.0,
        },
        FenceSpriteMeta {
            w: 33.0,
            h: 13.0,
            xrel: -32.0,
            yrel: 18.0,
        },
        FenceSpriteMeta {
            w: 33.0,
            h: 13.0,
            xrel: 0.0,
            yrel: 18.0,
        },
    ],
    [
        FenceSpriteMeta {
            w: 33.0,
            h: 21.0,
            xrel: -32.0,
            yrel: 10.0,
        },
        FenceSpriteMeta {
            w: 33.0,
            h: 21.0,
            xrel: 0.0,
            yrel: 10.0,
        },
        FenceSpriteMeta {
            w: 33.0,
            h: 29.0,
            xrel: -32.0,
            yrel: 2.0,
        },
        FenceSpriteMeta {
            w: 33.0,
            h: 29.0,
            xrel: 0.0,
            yrel: 2.0,
        },
        FenceSpriteMeta {
            w: 33.0,
            h: 13.0,
            xrel: -32.0,
            yrel: 18.0,
        },
        FenceSpriteMeta {
            w: 33.0,
            h: 13.0,
            xrel: 0.0,
            yrel: 18.0,
        },
    ],
    [
        FenceSpriteMeta {
            w: 33.0,
            h: 21.0,
            xrel: -32.0,
            yrel: 10.0,
        },
        FenceSpriteMeta {
            w: 33.0,
            h: 21.0,
            xrel: 0.0,
            yrel: 10.0,
        },
        FenceSpriteMeta {
            w: 33.0,
            h: 29.0,
            xrel: -32.0,
            yrel: 2.0,
        },
        FenceSpriteMeta {
            w: 33.0,
            h: 29.0,
            xrel: 0.0,
            yrel: 2.0,
        },
        FenceSpriteMeta {
            w: 33.0,
            h: 13.0,
            xrel: -32.0,
            yrel: 18.0,
        },
        FenceSpriteMeta {
            w: 33.0,
            h: 13.0,
            xrel: 0.0,
            yrel: 18.0,
        },
    ],
];

/// `_fence_mod_by_tileh_sw`: variante de sprite por pendiente.
pub static FENCE_MOD_BY_TILEH_SW: [u8; 32] = [
    0, 2, 4, 0, 0, 2, 4, 0, 0, 2, 4, 0, 0, 2, 4, 0, 0, 2, 4, 0, 0, 2, 4, 4, 0, 2, 4, 2, 0, 2, 4, 0,
];

/// `_fence_mod_by_tileh_se`: variante de sprite por pendiente.
pub static FENCE_MOD_BY_TILEH_SE: [u8; 32] = [
    1, 1, 5, 5, 3, 3, 1, 1, 1, 1, 5, 5, 3, 3, 1, 1, 1, 1, 5, 5, 3, 3, 1, 5, 1, 1, 5, 5, 3, 3, 3, 1,
];

/// `_fence_mod_by_tileh_ne`: variante de sprite por pendiente.
pub static FENCE_MOD_BY_TILEH_NE: [u8; 32] = [
    0, 0, 0, 0, 4, 4, 4, 4, 2, 2, 2, 2, 0, 0, 0, 0, 0, 0, 0, 0, 4, 4, 4, 4, 2, 2, 2, 2, 0, 2, 4, 0,
];

/// `_fence_mod_by_tileh_nw`: variante de sprite por pendiente.
pub static FENCE_MOD_BY_TILEH_NW: [u8; 32] = [
    1, 5, 1, 5, 1, 5, 1, 5, 3, 1, 3, 1, 3, 1, 3, 1, 1, 5, 1, 5, 1, 5, 1, 5, 3, 1, 3, 5, 3, 3, 3, 1,
];
