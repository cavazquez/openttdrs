/// Hash de Wang para generar variación determinista (sin RNG en el core).
pub fn wang_hash(x: u32, y: u32, seed: u32) -> u32 {
    let mut h = seed
        .wrapping_add(x.wrapping_mul(0x9E37_79B9))
        .wrapping_add(y.wrapping_mul(0x6C62_272E));
    h ^= h >> 16;
    h = h.wrapping_mul(0x045D_9F3B);
    h ^= h >> 16;
    h
}
