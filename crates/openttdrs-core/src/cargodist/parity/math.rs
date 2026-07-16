//! Port alineado a `OpenTTD`; casts/bucles intencionales.
#![allow(
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::double_must_use,
    clippy::if_not_else,
    clippy::items_after_statements,
    clippy::mut_range_bound,
    clippy::needless_range_loop,
    clippy::should_implement_trait
)]

#[must_use]
pub fn int_sqrt(mut num: u32) -> u32 {
    let mut res = 0_u32;
    let mut bit = 1_u32 << 30;

    while bit > num {
        bit >>= 2;
    }

    while bit != 0 {
        if num >= res.saturating_add(bit) {
            num -= res + bit;
            res = (res >> 1) + bit;
        } else {
            res >>= 1;
        }
        bit >>= 2;
    }

    if num > res {
        res += 1;
    }

    res
}

#[must_use]
pub fn distance_max_plus_manhattan(x0: u32, y0: u32, x1: u32, y1: u32) -> u32 {
    let dx = x0.abs_diff(x1);
    let dy = y0.abs_diff(y1);
    if dx > dy {
        dx.saturating_mul(2).saturating_add(dy)
    } else {
        dy.saturating_mul(2).saturating_add(dx)
    }
}

#[cfg(test)]
mod tests {
    use super::{distance_max_plus_manhattan, int_sqrt};

    #[test]
    fn linkgraph_parity_int_sqrt_matches_reference() {
        assert_eq!(int_sqrt(25), 5);
        assert_eq!(int_sqrt(88), 9);
    }

    #[test]
    fn linkgraph_parity_distance_matches_formula() {
        assert_eq!(distance_max_plus_manhattan(0, 0, 7, 3), 17);
        assert_eq!(distance_max_plus_manhattan(4, 1, 2, 9), 18);
    }
}
