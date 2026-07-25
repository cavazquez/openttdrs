//! Generación de altura legado (pre-TGP): ruido por capas.
//! Conservado por si hace falta comparar o reactivar el backend antiguo.
#![allow(dead_code)]

/// Ruido grueso que marca cuencas de lagos interiores (0 = sin lago, 1 = depresión máxima).
pub(super) fn lake_depression(cx: i32, cy: i32, seed: u64) -> f32 {
    const LAKE_SEED: u64 = 0xA11C_E000;
    const THRESHOLD: f32 = 0.52;
    let n = value_noise(cx / 6, cy / 6, seed.wrapping_add(LAKE_SEED));
    if n <= THRESHOLD {
        return 0.0;
    }
    ((n - THRESHOLD) / (1.0 - THRESHOLD)).min(1.0)
}

pub(super) fn smooth_corners(corners: &mut [f32], w: i32, h: i32) {
    let mut next = corners.to_vec();
    for y in 0..h {
        for x in 0..w {
            let mut sum = 0.0;
            let mut count = 0.0;
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let nx = x + dx;
                    let ny = y + dy;
                    if nx >= 0 && ny >= 0 && nx < w && ny < h {
                        sum += corners[(ny * w + nx) as usize];
                        count += 1.0;
                    }
                }
            }
            next[(y * w + x) as usize] = sum / count;
        }
    }
    corners.copy_from_slice(&next);
}

pub(super) fn layered_noise(x: i32, y: i32, seed: u64) -> f32 {
    let n0 = value_noise(x, y, seed);
    let n1 = value_noise(x / 2, y / 2, seed.wrapping_add(1));
    let n2 = value_noise(x / 4, y / 4, seed.wrapping_add(2));
    (n0 * 0.5 + n1 * 0.35 + n2 * 0.15).clamp(0.0, 1.0)
}

pub(super) fn island_falloff(x: i32, y: i32, map_w: i32, map_h: i32) -> f32 {
    if map_w <= 1 || map_h <= 1 {
        return 1.0;
    }
    let fx = x as f32 / map_w as f32;
    let fy = y as f32 / map_h as f32;
    let edge = (fx - 0.5).abs().max((fy - 0.5).abs()) * 2.0;
    (1.0 - edge.powf(1.4)).clamp(0.0, 1.0)
}

fn value_noise(x: i32, y: i32, seed: u64) -> f32 {
    let h = hash_u64(seed.wrapping_add(i64_pair_hash(x, y)));
    (h % 10_000) as f32 / 10_000.0
}

fn hash_u64(mut x: u64) -> u64 {
    x ^= x >> 30;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 27;
    x = x.wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

fn i64_pair_hash(x: i32, y: i32) -> u64 {
    u64::from(x as u32)
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .wrapping_add(u64::from(y as u32).wrapping_mul(0x6C62_272E_07BB_0142))
}

pub(super) fn corner_height_from_grid(
    corners: &[f32],
    corners_w: i32,
    x: i32,
    y: i32,
    sea_level: u8,
    height_span: u8,
) -> u8 {
    let idx = (y * corners_w + x) as usize;
    let n = corners.get(idx).copied().unwrap_or(0.5);
    // `n`≈0 → nivel del mar / lagos; `n`≈1 → colinas.
    let span = f32::from(height_span.max(1));
    let base = f32::from(sea_level) + n * span;
    base.round().clamp(0.0, 15.0) as u8
}
