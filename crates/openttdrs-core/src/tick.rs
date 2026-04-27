/// Contador monotónico de ticks de simulación (similar al reloj lógico de `OpenTTD`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct GameTick(u64);

impl GameTick {
    #[must_use]
    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }

    pub fn advance(&mut self) {
        self.0 = self.0.wrapping_add(1);
    }
}
