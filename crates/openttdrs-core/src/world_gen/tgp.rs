//! Generador TGP (`TerraGenesis` Perlin), port de `OpenTTD/src/tgp.cpp`.
#![allow(
    clippy::cast_lossless,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    clippy::manual_midpoint,
    clippy::too_many_lines,
    clippy::unreadable_literal
)]

use super::config::{Climate, TerrainType, TgenSmoothness, WorldGenConfig};
use crate::cargodist::parity::rng::Randomizer;

/// Altura en punto fijo (4 bits fraccionarios).
type Height = i16;
const HEIGHT_DECIMAL_BITS: i32 = 4;
/// Amplitud en punto fijo (10 bits fraccionarios).
type Amplitude = i32;
const AMPLITUDE_DECIMAL_BITS: i32 = 10;
const MAX_TGP_FREQUENCIES: i32 = 10;
const WATER_PERCENT_FACTOR: i64 = 1024;
const MIN_MAP_SIZE_BITS: u32 = 6;

#[inline]
const fn i2h(i: i32) -> Height {
    (i << HEIGHT_DECIMAL_BITS) as Height
}

#[inline]
fn h2i(h: Height) -> i32 {
    i32::from(h) >> HEIGHT_DECIMAL_BITS
}

#[inline]
const fn a2h(a: Amplitude) -> Height {
    (a >> (AMPLITUDE_DECIMAL_BITS - HEIGHT_DECIMAL_BITS)) as Height
}

#[derive(Clone, Copy)]
struct BorderFlags(u8);

impl BorderFlags {
    const NORTH_EAST: u8 = 1 << 0;
    const SOUTH_EAST: u8 = 1 << 1;
    const SOUTH_WEST: u8 = 1 << 2;
    const NORTH_WEST: u8 = 1 << 3;
    const ALL: Self =
        Self(Self::NORTH_EAST | Self::SOUTH_EAST | Self::SOUTH_WEST | Self::NORTH_WEST);
    const NONE: Self = Self(0);

    fn test(self, bit: u8) -> bool {
        self.0 & bit != 0
    }
}

struct HeightMap {
    h: Vec<Height>,
    dim_x: i32,
    size_x: i32,
    size_y: i32,
}

impl HeightMap {
    fn new(size_x: i32, size_y: i32) -> Self {
        let dim_x = size_x + 1;
        let dim_y = size_y + 1;
        Self {
            h: vec![0; (dim_x * dim_y) as usize],
            dim_x,
            size_x,
            size_y,
        }
    }

    #[inline]
    fn idx(&self, x: i32, y: i32) -> usize {
        (x + y * self.dim_x) as usize
    }

    #[inline]
    fn height(&self, x: i32, y: i32) -> Height {
        self.h[self.idx(x, y)]
    }

    #[inline]
    fn height_mut(&mut self, x: i32, y: i32) -> &mut Height {
        let i = self.idx(x, y);
        &mut self.h[i]
    }

    fn is_valid_xy(&self, x: i32, y: i32) -> bool {
        x >= 0 && x < self.size_x && y >= 0 && y < self.size_y
    }
}

fn map_log2(size: i32) -> u32 {
    let s = size.max(1) as u32;
    // OpenTTD exige potencias de 2; aquí usamos floor(log2) para mapas arbitrarios.
    s.ilog2()
}

fn tgp_get_max_height(terrain: TerrainType, map_w: i32, map_h: i32) -> Height {
    // filas: VeryFlat..Alpinist; columnas: 64..4096
    const MAX_HEIGHT: [[i32; 7]; 5] = [
        [3, 3, 3, 3, 4, 5, 7],
        [5, 7, 8, 9, 14, 19, 31],
        [8, 9, 10, 15, 23, 37, 61],
        [10, 11, 17, 19, 49, 63, 73],
        [12, 19, 25, 31, 67, 75, 87],
    ];
    let log_min = map_log2(map_w.min(map_h));
    let bucket = log_min.saturating_sub(MIN_MAP_SIZE_BITS).min(6) as usize;
    let row = terrain.as_index().min(4);
    i2h(MAX_HEIGHT[row][bucket])
}

fn get_amplitude(frequency: i32, smoothness: TgenSmoothness, max_height: Height) -> Amplitude {
    const AMPLITUDES: [[Amplitude; 7]; 4] = [
        [16000, 5600, 1968, 688, 240, 16, 16],
        [24000, 12800, 6400, 2700, 1024, 128, 16],
        [32000, 19200, 12800, 8000, 3200, 256, 64],
        [48000, 24000, 19200, 16000, 8000, 512, 320],
    ];
    const EXTRAPOLATION: [f64; 4] = [3.3, 2.8, 2.3, 1.8];

    let sm = smoothness.as_index().min(3);
    let table_len = 7i32;
    let mut index = frequency - MAX_TGP_FREQUENCIES + table_len;
    let mut amplitude = AMPLITUDES[sm][index.max(0) as usize];
    if index >= 0 {
        return amplitude;
    }

    let factor = EXTRAPOLATION[sm];
    let mut height_range = i2h(16);
    while index < 0 {
        amplitude = (factor * f64::from(amplitude)) as Amplitude;
        height_range <<= 1;
        index += 1;
    }
    let scale =
        ((i32::from(max_height) - i32::from(height_range)) / i32::from(height_range)).clamp(0, 1);
    scale * amplitude
}

fn random_height(rng: &mut Randomizer, r_max: Amplitude) -> Height {
    let span = (2 * r_max + 1) as u32;
    let v = rng.random_range(span) as i32 - r_max;
    a2h(v)
}

fn height_map_generate(hm: &mut HeightMap, cfg: &WorldGenConfig, rng: &mut Randomizer) {
    let max_h = tgp_get_max_height(cfg.terrain_type, hm.size_x, hm.size_y);
    let start = (MAX_TGP_FREQUENCIES - map_log2(hm.size_x.min(hm.size_y)) as i32).max(0);
    let mut first = true;

    for frequency in start..MAX_TGP_FREQUENCIES {
        let amplitude = get_amplitude(frequency, cfg.tgen_smoothness, max_h);
        if amplitude == 0 {
            continue;
        }
        let step = 1 << (MAX_TGP_FREQUENCIES - frequency - 1);

        if first {
            let mut y = 0;
            while y <= hm.size_y {
                let mut x = 0;
                while x <= hm.size_x {
                    *hm.height_mut(x, y) = if amplitude > 0 {
                        random_height(rng, amplitude)
                    } else {
                        0
                    };
                    x += step;
                }
                y += step;
            }
            first = false;
            continue;
        }

        let mut y = 0;
        while y <= hm.size_y {
            let mut x = 0;
            while x <= hm.size_x - 2 * step {
                let h00 = hm.height(x, y);
                let h02 = hm.height(x + 2 * step, y);
                *hm.height_mut(x + step, y) = ((i32::from(h00) + i32::from(h02)) / 2) as Height;
                x += 2 * step;
            }
            y += 2 * step;
        }

        let mut y = 0;
        while y <= hm.size_y - 2 * step {
            let mut x = 0;
            while x <= hm.size_x {
                let h00 = hm.height(x, y);
                let h20 = hm.height(x, y + 2 * step);
                *hm.height_mut(x, y + step) = ((i32::from(h00) + i32::from(h20)) / 2) as Height;
                x += step;
            }
            y += 2 * step;
        }

        let mut y = 0;
        while y <= hm.size_y {
            let mut x = 0;
            while x <= hm.size_x {
                *hm.height_mut(x, y) = hm.height(x, y).wrapping_add(random_height(rng, amplitude));
                x += step;
            }
            y += step;
        }
    }
}

fn height_map_adjust_water_level(hm: &mut HeightMap, water_percent: i64, h_max_new: Height) {
    let (h_min, h_max) =
        hm.h.iter()
            .copied()
            .fold((Height::MAX, Height::MIN), |(lo, hi), h| {
                (lo.min(h), hi.max(h))
            });
    if h_max <= h_min {
        return;
    }

    let hist_len = (i32::from(h_max) - i32::from(h_min) + 1) as usize;
    let mut hist_buf = vec![0i32; hist_len];
    for &h in &hm.h {
        hist_buf[(i32::from(h) - i32::from(h_min)) as usize] += 1;
    }

    let desired =
        water_percent * i64::from(hm.size_x) * i64::from(hm.size_y) / WATER_PERCENT_FACTOR;
    let mut water_tiles = 0i64;
    let mut h_water_level = h_min;
    while h_water_level < h_max {
        water_tiles += i64::from(hist_buf[(i32::from(h_water_level) - i32::from(h_min)) as usize]);
        if water_tiles >= desired {
            break;
        }
        h_water_level += 1;
    }

    let denom = i32::from(h_max) - i32::from(h_water_level);
    if denom == 0 {
        return;
    }
    let h_max_new_i = i32::from(h_max_new);
    for h in &mut hm.h {
        let mut nh =
            h_max_new_i * (i32::from(*h) - i32::from(h_water_level)) / denom + i32::from(i2h(1));
        if nh < 0 {
            nh = i32::from(i2h(0));
        }
        if nh >= h_max_new_i {
            nh = h_max_new_i - 1;
        }
        *h = nh as Height;
    }
}

fn sine_transform_lowlands(height: f64) -> f64 {
    let sine_lower_limit = 0.5;
    let linear_compression = 2.0;
    if height <= sine_lower_limit {
        height / linear_compression
    } else {
        let m = sine_lower_limit / linear_compression;
        let mut h = 2.0 * ((height - sine_lower_limit) / (1.0 - sine_lower_limit)) - 1.0;
        h = (h * std::f64::consts::FRAC_PI_2).sin();
        0.5 * ((1.0 - m) * h + (1.0 + m))
    }
}

fn sine_transform_normal(height: f64) -> f64 {
    let h = 2.0 * height - 1.0;
    let h = (h * std::f64::consts::FRAC_PI_2).sin();
    0.5 * (h + 1.0)
}

fn sine_transform_plateaus(height: f64) -> f64 {
    let sine_upper_limit = 0.75;
    let linear_compression = 2.0;
    if height >= sine_upper_limit {
        1.0 - (1.0 - height) / linear_compression
    } else {
        let m = 1.0 - (1.0 - sine_upper_limit) / linear_compression;
        let mut h = 2.0 * height / sine_upper_limit - 1.0;
        h = (h * std::f64::consts::FRAC_PI_2).sin();
        0.5 * (h + 1.0) * m
    }
}

fn height_map_sine_transform(hm: &mut HeightMap, h_min: Height, h_max: Height, climate: Climate) {
    let span = i32::from(h_max) - i32::from(h_min);
    if span <= 0 {
        return;
    }
    for h in &mut hm.h {
        if *h < h_min {
            continue;
        }
        let mut fheight = f64::from(i32::from(*h) - i32::from(h_min)) / f64::from(span);
        fheight = match climate {
            Climate::SubTropical => sine_transform_lowlands(fheight),
            Climate::SubArctic => sine_transform_plateaus(fheight),
            Climate::Temperate | Climate::Toyland => sine_transform_normal(fheight),
        };
        let mut nh = (fheight * f64::from(span) + f64::from(h_min)) as Height;
        if nh < 0 {
            nh = i2h(0);
        }
        if nh >= h_max {
            nh = h_max - 1;
        }
        *h = nh;
    }
}

fn int_noise(x: i32, y: i32, prime: i32, seed: u32) -> f64 {
    let mut n = x
        .wrapping_add(y.wrapping_mul(prime))
        .wrapping_add(seed as i32);
    n = (n << 13) ^ n;
    let r = n
        .wrapping_mul(n.wrapping_mul(n).wrapping_mul(15731).wrapping_add(789221))
        .wrapping_add(1_376_312_589);
    1.0 - f64::from(r & 0x7fff_ffff) / 1_073_741_824.0
}

fn linear_interpolate(a: f64, b: f64, x: f64) -> f64 {
    a + x * (b - a)
}

fn interpolated_noise(x: f64, y: f64, prime: i32, seed: u32) -> f64 {
    let integer_x = x.floor() as i32;
    let integer_y = y.floor() as i32;
    let fractional_x = x - f64::from(integer_x);
    let fractional_y = y - f64::from(integer_y);
    let v1 = int_noise(integer_x, integer_y, prime, seed);
    let v2 = int_noise(integer_x + 1, integer_y, prime, seed);
    let v3 = int_noise(integer_x, integer_y + 1, prime, seed);
    let v4 = int_noise(integer_x + 1, integer_y + 1, prime, seed);
    let i1 = linear_interpolate(v1, v2, fractional_x);
    let i2 = linear_interpolate(v3, v4, fractional_x);
    linear_interpolate(i1, i2, fractional_y)
}

fn perlin_coast_noise_2d(x: f64, y: f64, p: f64, prime: i32, seed: u32) -> f64 {
    const OCTAVES: i32 = 6;
    const INITIAL_FREQUENCY: f64 = (1 << OCTAVES) as f64;
    let mut total = 0.0;
    let mut max_value = 0.0;
    let mut frequency = 1.0 / INITIAL_FREQUENCY;
    let mut amplitude = 1.0;
    for _ in 0..OCTAVES {
        total += interpolated_noise(x * frequency, y * frequency, prime, seed) * amplitude;
        max_value += amplitude;
        frequency *= 2.0;
        amplitude *= p;
    }
    total / max_value
}

fn height_map_coast_lines(hm: &mut HeightMap, water_borders: BorderFlags, seed: u32) {
    let smallest = hm.size_x.min(hm.size_y);
    let map_ratio = hm.size_x.max(hm.size_y) / smallest.max(1);
    let jagged_distance = (12 + (smallest * smallest / 4096) + map_ratio.min(16)).min(64);
    let smooth_distance = (smallest / 32).min(32);

    let get_depth = |x: i32, p1: i32, p2: i32, p3: i32| -> i32 {
        let xf = f64::from(x);
        2 + (smooth_distance as f64 * (1.0 + perlin_coast_noise_2d(xf, xf, 0.2, p1, seed))) as i32
            + (jagged_distance as f64 * perlin_coast_noise_2d(xf, xf, 0.5, p2, seed).abs()) as i32
            + (8.0 * perlin_coast_noise_2d(xf, xf, 0.8, p3, seed).abs()) as i32
    };

    for y in 0..=hm.size_y {
        if water_borders.test(BorderFlags::NORTH_EAST) {
            let depth = get_depth(y, 67, 179, 53);
            for x in 0..depth {
                if x <= hm.size_x {
                    *hm.height_mut(x, y) = 0;
                }
            }
        }
        if water_borders.test(BorderFlags::SOUTH_WEST) {
            let depth = get_depth(y, 199, 67, 101);
            let mut x = hm.size_x;
            while x > (hm.size_x - 1 - depth) {
                *hm.height_mut(x, y) = 0;
                if x == 0 {
                    break;
                }
                x -= 1;
            }
        }
    }

    for x in 0..=hm.size_x {
        if water_borders.test(BorderFlags::NORTH_WEST) {
            let depth = get_depth(x, 179, 211, 167);
            for y in 0..depth {
                if y <= hm.size_y {
                    *hm.height_mut(x, y) = 0;
                }
            }
        }
        if water_borders.test(BorderFlags::SOUTH_EAST) {
            let depth = get_depth(x, 101, 193, 71);
            let mut y = hm.size_y;
            while y > (hm.size_y - 1 - depth) {
                *hm.height_mut(x, y) = 0;
                if y == 0 {
                    break;
                }
                y -= 1;
            }
        }
    }
}

fn height_map_smooth_coast_in_direction(
    hm: &mut HeightMap,
    org_x: i32,
    org_y: i32,
    dir_x: i32,
    dir_y: i32,
) {
    const MAX_COAST_DIST_FROM_EDGE: i32 = 100;
    const MAX_COAST_SMOOTH_DEPTH: i32 = 35;

    let mut x = org_x;
    let mut y = org_y;
    let mut ed = 0;
    while hm.is_valid_xy(x, y) && ed < MAX_COAST_DIST_FROM_EDGE {
        if hm.height(x, y) >= i2h(1) {
            break;
        }
        if hm.is_valid_xy(x + dir_y, y + dir_x) && hm.height(x + dir_y, y + dir_x) > 0 {
            break;
        }
        if hm.is_valid_xy(x - dir_y, y - dir_x) && hm.height(x - dir_y, y - dir_x) > 0 {
            break;
        }
        x += dir_x;
        y += dir_y;
        ed += 1;
    }

    let mut h_prev = i2h(1);
    let mut depth = 0;
    while hm.is_valid_xy(x, y) && depth <= MAX_COAST_SMOOTH_DEPTH {
        let h = hm.height(x, y);
        let capped = i32::from(h).min(i32::from(h_prev) + (4 + depth)) as Height;
        *hm.height_mut(x, y) = capped;
        h_prev = capped;
        depth += 1;
        x += dir_x;
        y += dir_y;
    }
}

fn height_map_smooth_coasts(hm: &mut HeightMap, water_borders: BorderFlags) {
    for x in 0..hm.size_x {
        if water_borders.test(BorderFlags::NORTH_WEST) {
            height_map_smooth_coast_in_direction(hm, x, 0, 0, 1);
        }
        if water_borders.test(BorderFlags::SOUTH_EAST) {
            height_map_smooth_coast_in_direction(hm, x, hm.size_y - 1, 0, -1);
        }
    }
    for y in 0..hm.size_y {
        if water_borders.test(BorderFlags::NORTH_EAST) {
            height_map_smooth_coast_in_direction(hm, 0, y, 1, 0);
        }
        if water_borders.test(BorderFlags::SOUTH_WEST) {
            height_map_smooth_coast_in_direction(hm, hm.size_x - 1, y, -1, 0);
        }
    }
}

fn height_map_smooth_slopes(hm: &mut HeightMap, dh_max: Height) {
    for y in 0..=hm.size_y {
        for x in 0..=hm.size_x {
            let left = hm.height(if x > 0 { x - 1 } else { x }, y);
            let up = hm.height(x, if y > 0 { y - 1 } else { y });
            let h_max = left.min(up).wrapping_add(dh_max);
            if hm.height(x, y) > h_max {
                *hm.height_mut(x, y) = h_max;
            }
        }
    }
    for y in (0..=hm.size_y).rev() {
        for x in (0..=hm.size_x).rev() {
            let right = hm.height(if x < hm.size_x { x + 1 } else { x }, y);
            let down = hm.height(x, if y < hm.size_y { y + 1 } else { y });
            let h_max = right.min(down).wrapping_add(dh_max);
            if hm.height(x, y) > h_max {
                *hm.height_mut(x, y) = h_max;
            }
        }
    }
}

fn height_map_curves(hm: &mut HeightMap, level: u8, max_height: Height, rng: &mut Randomizer) {
    let mh = i32::from(max_height) - i32::from(i2h(1));
    if mh <= 0 || level == 0 {
        return;
    }
    let f = |fraction: f64| -> Height { (fraction * f64::from(mh)) as Height };

    let curve_maps: [&[(Height, Height)]; 4] = [
        &[(f(0.0), f(0.0)), (f(0.8), f(0.13)), (f(1.0), f(0.4))],
        &[
            (f(0.0), f(0.0)),
            (f(0.53), f(0.13)),
            (f(0.8), f(0.27)),
            (f(1.0), f(0.6)),
        ],
        &[
            (f(0.0), f(0.0)),
            (f(0.53), f(0.27)),
            (f(0.8), f(0.57)),
            (f(1.0), f(0.8)),
        ],
        &[
            (f(0.0), f(0.0)),
            (f(0.4), f(0.3)),
            (f(0.7), f(0.8)),
            (f(0.92), f(0.99)),
            (f(1.0), f(0.99)),
        ],
    ];

    let factor = (f64::from(hm.size_x) / f64::from(hm.size_y.max(1))).sqrt();
    let sx = ((((1 << level) as f64) * factor) + 0.5).clamp(1.0, 128.0) as usize;
    let sy = ((((1 << level) as f64) / factor) + 0.5).clamp(1.0, 128.0) as usize;
    let mut c = vec![0u8; sx * sy];
    for cell in &mut c {
        *cell = rng.random_range(curve_maps.len() as u32) as u8;
    }

    for x in 0..hm.size_x {
        let fx = (sx as f64) * f64::from(x) / f64::from(hm.size_x) + 1.0;
        let mut x1 = fx as usize;
        let mut x2 = x1;
        let mut xr = 2.0 * (fx - x1 as f64) - 1.0;
        xr = (xr * std::f64::consts::FRAC_PI_2).sin();
        xr = (xr * std::f64::consts::FRAC_PI_2).sin();
        xr = 0.5 * (xr + 1.0);
        let xri = 1.0 - xr;
        if x1 > 0 {
            x1 -= 1;
            if x2 >= sx {
                x2 -= 1;
            }
        }

        for y in 0..hm.size_y {
            let fy = (sy as f64) * f64::from(y) / f64::from(hm.size_y) + 1.0;
            let mut y1 = fy as usize;
            let mut y2 = y1;
            let mut yr = 2.0 * (fy - y1 as f64) - 1.0;
            yr = (yr * std::f64::consts::FRAC_PI_2).sin();
            yr = (yr * std::f64::consts::FRAC_PI_2).sin();
            yr = 0.5 * (yr + 1.0);
            let yri = 1.0 - yr;
            if y1 > 0 {
                y1 -= 1;
                if y2 >= sy {
                    y2 -= 1;
                }
            }

            let corner_a = c[x1 + sx * y1] as usize;
            let corner_b = c[x1 + sx * y2] as usize;
            let corner_c = c[x2 + sx * y1] as usize;
            let corner_d = c[x2 + sx * y2] as usize;
            let corner_bits =
                (1u8 << corner_a) | (1u8 << corner_b) | (1u8 << corner_c) | (1u8 << corner_d);

            let mut h = hm.height(x, y);
            if h < i2h(1) {
                continue;
            }
            h -= i2h(1);

            let mut ht = [0i16; 4];
            for (t, cm) in curve_maps.iter().enumerate() {
                if corner_bits & (1 << t) == 0 {
                    continue;
                }
                for i in 0..cm.len() - 1 {
                    let (p1x, p1y) = cm[i];
                    let (p2x, p2y) = cm[i + 1];
                    if h >= p1x && h < p2x {
                        let denom = i32::from(p2x) - i32::from(p1x);
                        ht[t] = if denom == 0 {
                            p1y
                        } else {
                            (i32::from(p1y)
                                + (i32::from(h) - i32::from(p1x))
                                    * (i32::from(p2y) - i32::from(p1y))
                                    / denom) as Height
                        };
                        break;
                    }
                }
            }

            let blended = (f64::from(ht[corner_a]) * yri + f64::from(ht[corner_b]) * yr) * xri
                + (f64::from(ht[corner_c]) * yri + f64::from(ht[corner_d]) * yr) * xr;
            *hm.height_mut(x, y) = (blended as Height).wrapping_add(i2h(1));
        }
    }
}

fn height_map_normalize(hm: &mut HeightMap, cfg: &WorldGenConfig, rng: &mut Randomizer) {
    let water_percent = cfg.quantity_sea_lakes.water_percent_x1024();
    let h_max_new = tgp_get_max_height(cfg.terrain_type, hm.size_x, hm.size_y);
    let roughness = i2h(0) + (7 + 3 * cfg.tgen_smoothness.as_index() as i32) as Height;

    height_map_adjust_water_level(hm, water_percent, h_max_new);

    let water_borders = if cfg.island {
        BorderFlags::ALL
    } else {
        BorderFlags::NONE
    };
    let seed = cfg.seed as u32;

    if water_borders.0 != 0 {
        height_map_coast_lines(hm, water_borders, seed);
    }
    height_map_smooth_slopes(hm, roughness);
    if water_borders.0 != 0 {
        height_map_smooth_coasts(hm, water_borders);
    }
    height_map_smooth_slopes(hm, roughness);
    height_map_sine_transform(hm, i2h(1), h_max_new, cfg.climate);

    if cfg.variety > 0 {
        height_map_curves(hm, cfg.variety.min(5), h_max_new, rng);
    }
}

/// Genera alturas de tesela (una por celda) con TGP; determinista para `config.seed`.
#[must_use]
pub(super) fn generate_tgp_heights(map_w: i32, map_h: i32, config: &WorldGenConfig) -> Vec<u8> {
    let mut hm = HeightMap::new(map_w, map_h);
    let mut rng = Randomizer::new(config.seed as u32);
    height_map_generate(&mut hm, config, &mut rng);
    height_map_normalize(&mut hm, config, &mut rng);

    let max_height = h2i(tgp_get_max_height(config.terrain_type, map_w, map_h));
    let mut out = vec![0u8; (map_w * map_h) as usize];
    for y in 0..map_h {
        for x in 0..map_w {
            let h = h2i(hm.height(x, y)).clamp(0, max_height).clamp(0, 255) as u8;
            out[(y * map_w + x) as usize] = h;
        }
    }
    out
}

/// Estima la línea de cobertura (nieve/desierto) como `CalculateCoverageLine` en `landscape.cpp`.
#[must_use]
pub(super) fn calculate_coverage_line(
    heights: &[u8],
    map_w: i32,
    map_h: i32,
    coverage: u8,
    edge_multiplier: u32,
) -> u8 {
    const MAX_TILE_HEIGHT: usize = 255;
    let mut histogram = [0i32; MAX_TILE_HEIGHT + 1];
    let mut edge_histogram = [0i32; MAX_TILE_HEIGHT + 1];
    let size = (map_w * map_h) as usize;

    for y in 0..map_h {
        for x in 0..map_w {
            let idx = (y * map_w + x) as usize;
            let h = heights.get(idx).copied().unwrap_or(0) as usize;
            histogram[h] += 1;
            if edge_multiplier != 0 {
                for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                    let nx = x + dx;
                    let ny = y + dy;
                    if nx >= 0 && ny >= 0 && nx < map_w && ny < map_h {
                        let nh = heights[(ny * map_w + nx) as usize] as usize;
                        if nh < h {
                            edge_histogram[h] += 1;
                        }
                    }
                }
            }
        }
    }

    let land_tiles = size as i32 - histogram[0];
    if land_tiles <= 0 {
        return 2;
    }
    let goal_tiles = land_tiles * i32::from(coverage) / 100;
    let mut best_score = land_tiles;
    let mut best_h = MAX_TILE_HEIGHT as u8;
    let mut current_tiles = 0i32;
    for h in (1..=MAX_TILE_HEIGHT).rev() {
        current_tiles += histogram[h];
        let mut current_score = goal_tiles - current_tiles;
        if edge_multiplier != 0 && h > 1 {
            current_score -= edge_histogram[1] * edge_multiplier as i32;
            current_score -= edge_histogram[h] * edge_multiplier as i32;
        }
        if current_score.abs() < best_score.abs() {
            best_score = current_score;
            best_h = h as u8;
        }
    }
    best_h
}

#[cfg(test)]
#[allow(clippy::naive_bytecount)]
mod tests {
    use super::*;
    use crate::world_gen::config::{QuantitySeaLakes, WorldGenConfig};

    #[test]
    fn tgp_is_deterministic_for_seed() {
        let cfg = WorldGenConfig {
            seed: 0x00C0_FFEE,
            island: true,
            ..WorldGenConfig::default()
        };
        let a = generate_tgp_heights(32, 32, &cfg);
        let b = generate_tgp_heights(32, 32, &cfg);
        assert_eq!(a, b);
    }

    #[test]
    fn tgp_terrain_type_changes_max_height() {
        let flat = WorldGenConfig::default().with_terrain_type(TerrainType::VeryFlat);
        let alpine = WorldGenConfig::default().with_terrain_type(TerrainType::Alpinist);
        let hf = generate_tgp_heights(64, 64, &WorldGenConfig { seed: 7, ..flat });
        let ha = generate_tgp_heights(64, 64, &WorldGenConfig { seed: 7, ..alpine });
        let max_f = *hf.iter().max().unwrap_or(&0);
        let max_a = *ha.iter().max().unwrap_or(&0);
        assert!(max_a >= max_f, "alpinist max {max_a} vs very flat {max_f}");
    }

    #[test]
    fn tgp_more_sea_increases_water_tiles() {
        let low = WorldGenConfig {
            seed: 99,
            quantity_sea_lakes: QuantitySeaLakes::VeryLow,
            island: false,
            ..WorldGenConfig::default()
        };
        let high = WorldGenConfig {
            quantity_sea_lakes: QuantitySeaLakes::High,
            ..low
        };
        let water = |h: &[u8]| h.iter().filter(|&&z| z == 0).count();
        assert!(
            water(&generate_tgp_heights(48, 48, &high))
                >= water(&generate_tgp_heights(48, 48, &low))
        );
    }
}
