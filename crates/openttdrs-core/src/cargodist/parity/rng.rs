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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct Randomizer {
    pub state: [u32; 2],
}

impl Randomizer {
    #[must_use]
    pub const fn new(seed: u32) -> Self {
        Self {
            state: [seed, seed],
        }
    }

    pub fn set_seed(&mut self, seed: u32) {
        self.state = [seed, seed];
    }

    #[must_use]
    pub fn next(&mut self) -> u32 {
        let s = self.state[0];
        let t = self.state[1];
        self.state[0] = s
            .wrapping_add((t ^ 0x1234_567F).rotate_right(7))
            .wrapping_add(1);
        let next = s.rotate_right(3).wrapping_sub(1);
        self.state[1] = next;
        next
    }

    #[must_use]
    pub const fn scale_to_limit(value: u32, limit: u32) -> u32 {
        (((value as u64) * (limit as u64)) >> 32) as u32
    }

    #[must_use]
    pub fn random_range(&mut self, limit: u32) -> u32 {
        Self::scale_to_limit(self.next(), limit)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::Randomizer;

    #[test]
    fn linkgraph_parity_randomizer_seed_one_matches_reference() {
        let mut rng = Randomizer::new(1);
        assert_eq!(rng.next(), 536_870_911);
        assert_eq!(rng.next(), 3_750_006_036);
        assert_eq!(rng.next(), 1_602_748_415);
        assert_eq!(rng.next(), 981_167_158);
    }
}
