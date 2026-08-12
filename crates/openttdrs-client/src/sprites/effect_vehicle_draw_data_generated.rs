// Generado por scripts/gen_effect_vehicle_sprites.py — NO EDITAR A MANO.
//
// EffectVehicle: humo, chispas, explosión, avería y burbujas Toyland.

pub const DIESEL_SMOKE_FRAMES: usize = 6;

/// (w, h, xrel, yrel) de `diesel_smoke_{i}.png`.
pub static DIESEL_SMOKE_META: [(f32, f32, f32, f32); 6] = [
    (4.0, 4.0, -2.0, -2.0),
    (5.0, 5.0, -2.0, -2.0),
    (6.0, 6.0, -3.0, -3.0),
    (7.0, 7.0, -3.0, -3.0),
    (8.0, 8.0, -4.0, -4.0),
    (9.0, 9.0, -4.0, -4.0),
];

pub const STEAM_SMOKE_FRAMES: usize = 5;

/// (w, h, xrel, yrel) de `steam_smoke_{i}.png`.
pub static STEAM_SMOKE_META: [(f32, f32, f32, f32); 5] = [
    (8.0, 8.0, -4.0, -4.0),
    (10.0, 10.0, -5.0, -5.0),
    (12.0, 12.0, -6.0, -6.0),
    (14.0, 14.0, -7.0, -7.0),
    (16.0, 16.0, -8.0, -8.0),
];

pub const ELECTRIC_SPARK_FRAMES: usize = 6;

/// (w, h, xrel, yrel) de `electric_spark_{i}.png`.
pub static ELECTRIC_SPARK_META: [(f32, f32, f32, f32); 6] = [
    (3.0, 4.0, -1.0, -2.0),
    (7.0, 7.0, -2.0, -4.0),
    (11.0, 10.0, -4.0, -5.0),
    (14.0, 13.0, -5.0, -6.0),
    (23.0, 17.0, -9.0, -5.0),
    (26.0, 25.0, -10.0, -10.0),
];

pub const EXPLOSION_LARGE_FRAMES: usize = 16;

/// (w, h, xrel, yrel) de `explosion_large_{i}.png`.
pub static EXPLOSION_LARGE_META: [(f32, f32, f32, f32); 16] = [
    (43.0, 39.0, -21.0, -20.0),
    (46.0, 43.0, -23.0, -21.0),
    (46.0, 46.0, -23.0, -22.0),
    (45.0, 47.0, -23.0, -23.0),
    (43.0, 44.0, -23.0, -22.0),
    (40.0, 43.0, -21.0, -21.0),
    (38.0, 41.0, -19.0, -21.0),
    (37.0, 41.0, -19.0, -21.0),
    (34.0, 29.0, -19.0, -16.0),
    (33.0, 27.0, -19.0, -15.0),
    (28.0, 27.0, -14.0, -15.0),
    (26.0, 20.0, -14.0, -15.0),
    (15.0, 5.0, -3.0, -3.0),
    (5.0, 3.0, -3.0, -3.0),
    (1.0, 1.0, 23.0, 23.0),
    (1.0, 1.0, 23.0, 23.0),
];

pub const BREAKDOWN_SMOKE_FRAMES: usize = 4;

/// (w, h, xrel, yrel) de `breakdown_smoke_{i}.png`.
pub static BREAKDOWN_SMOKE_META: [(f32, f32, f32, f32); 4] = [
    (32.0, 32.0, 0.0, -31.0),
    (26.0, 30.0, 0.0, -29.0),
    (32.0, 32.0, 0.0, -31.0),
    (27.0, 30.0, 0.0, -29.0),
];

pub const BUBBLE_FRAMES: usize = 15;

/// (w, h, xrel, yrel) de `bubble_{i}.png`.
pub static BUBBLE_META: [(f32, f32, f32, f32); 15] = [
    (15.0, 15.0, -7.0, -15.0),
    (15.0, 16.0, -7.0, -16.0),
    (16.0, 15.0, -7.0, -15.0),
    (17.0, 4.0, -8.0, -2.0),
    (17.0, 8.0, -8.0, -6.0),
    (17.0, 12.0, -8.0, -10.0),
    (17.0, 15.0, -8.0, -13.0),
    (20.0, 21.0, -9.0, -18.0),
    (28.0, 28.0, -13.0, -21.0),
    (37.0, 36.0, -17.0, -25.0),
    (15.0, 16.0, -7.0, -15.0),
    (15.0, 18.0, -7.0, -14.0),
    (13.0, 21.0, -6.0, -12.0),
    (11.0, 25.0, -5.0, -6.0),
    (7.0, 35.0, -3.0, 1.0),
];
