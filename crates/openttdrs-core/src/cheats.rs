//! Cheats / sandbox formales (UI-7 / #45). Solo afectan partida si `enabled`.

use crate::news::CALENDAR_BASE_YEAR;

/// Año máximo admitido por `CheatSetYear` (MVP; `OpenTTD` permite más rango).
pub const CHEAT_YEAR_MAX: u32 = CALENDAR_BASE_YEAR + 500;

/// `true` si el año está en el rango admitido por cheats.
#[must_use]
pub const fn year_in_range(year: u32) -> bool {
    year >= CALENDAR_BASE_YEAR && year <= CHEAT_YEAR_MAX
}

/// Estado de cheats de la partida (no afecta saves antiguos: `#[serde(default)]`).
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[allow(clippy::struct_excessive_bools)]
pub struct CheatsState {
    /// Maestro: sin esto, el resto se ignora.
    pub enabled: bool,
    /// La compañía activa no gasta dinero (saldo se rellena al aplicar comandos).
    pub infinite_money: bool,
    /// `ClearTile` ignora propiedad de tesela.
    pub magic_bulldozer: bool,
    /// Desactiva crash de jets en pista corta (`_cheats.no_jetcrash`).
    #[serde(default)]
    pub no_jetcrash: bool,
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

    #[must_use]
    pub const fn no_jetcrash_active(&self) -> bool {
        self.enabled && self.no_jetcrash
    }
}
