//! Cheats / sandbox formales (UI-7). Solo afectan partida si `enabled`.

/// Estado de cheats de la partida (no afecta saves antiguos: `#[serde(default)]`).
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct CheatsState {
    /// Maestro: sin esto, el resto se ignora.
    pub enabled: bool,
    /// La compañía activa no gasta dinero (saldo se rellena al aplicar comandos).
    pub infinite_money: bool,
    /// `ClearTile` ignora propiedad de tesela.
    pub magic_bulldozer: bool,
}

impl CheatsState {
    #[must_use]
    pub const fn infinite_money_active(&self) -> bool {
        self.enabled && self.infinite_money
    }

    #[must_use]
    pub const fn magic_bulldozer_active(&self) -> bool {
        self.enabled && self.magic_bulldozer
    }
}
