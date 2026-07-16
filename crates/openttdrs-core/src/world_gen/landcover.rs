//! Cobertura de terreno: densidad de hierba, bosques y desiertos.

use super::config::Climate;

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

pub(super) fn grass_density(x: i32, y: i32, seed: u64) -> u8 {
    let n = hash_u64(seed.wrapping_add(i64_pair_hash(x.wrapping_mul(3), y.wrapping_mul(5))));
    // Variación suave: mayoría hierba completa; sin `bare` (m5==0) para no confundir con default.
    match n % 10 {
        0..=1 => 1,
        2..=4 => 2,
        _ => 3,
    }
}

pub fn forest_patch(x: i32, y: i32, seed: u64, climate: Climate) -> bool {
    if !matches!(climate, Climate::Temperate | Climate::SubArctic) {
        return false;
    }
    hash_u64(seed.wrapping_add(i64_pair_hash(x.wrapping_mul(7), y.wrapping_mul(11)))) % 9 == 0
}

pub fn desert_patch(x: i32, y: i32, seed: u64) -> bool {
    hash_u64(seed.wrapping_add(i64_pair_hash(x.wrapping_mul(13), y.wrapping_mul(17)))) % 5 == 0
}
