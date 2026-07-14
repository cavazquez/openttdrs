//! Pool de compañías (Fase 4 estructural).
//!
//! `GameState::economy` / `company_colour` siguen siendo el espejo de la compañía
//! activa (jugador) para no romper comandos/UI. El pool `companies` es la fuente
//! de verdad multi-compañía; [`sync_company_mirrors`] mantiene ambos alineados.

use serde::{Deserialize, Serialize};

use crate::game_state::CompanyEconomy;

/// Identificador de compañía (índice en `GameState::companies`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub struct CompanyId(pub u8);

impl CompanyId {
    pub const PLAYER: Self = Self(0);

    #[must_use]
    pub const fn index(self) -> usize {
        self.0 as usize
    }

    /// Owner de tesela desde byte `m1` (MAPO), acotado a compañías existentes.
    #[must_use]
    pub fn from_tile_m1(m1: u8, company_count: usize) -> Self {
        let idx = usize::from(m1);
        if company_count == 0 || idx >= company_count {
            Self::PLAYER
        } else {
            Self(m1)
        }
    }
}

/// Escribe el owner de infraestructura en `m1` (vía / carretera / depósitos).
#[must_use]
pub fn tile_with_owner(mut tile: crate::map::Tile, owner: CompanyId) -> crate::map::Tile {
    tile.m1 = owner.0;
    tile
}

/// Compañía jugable o IA.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Company {
    pub id: CompanyId,
    pub name: String,
    pub colour: u8,
    pub economy: CompanyEconomy,
    /// `true` = controlada por [`crate::ai::CompanyAi`].
    #[serde(default)]
    pub is_ai: bool,
    /// Ingresos acumulados por entregas (esta compañía).
    #[serde(default)]
    pub cargo_income_earned: u64,
    /// Costes de explotación de vehículos acumulados.
    #[serde(default)]
    pub vehicle_running_costs: u64,
    /// Entregas de carga acumuladas.
    #[serde(default)]
    pub cargo_deliveries: u64,
    /// Series mensuales para gráficos (Income / Operating Profit / Value).
    #[serde(default)]
    pub economy_history: crate::game_state::EconomyHistory,
    /// Series trimestrales (`CompaniesGenStatistics` / rating + valoración con activos).
    #[serde(default)]
    pub quarterly_economy: crate::economy_quarterly::QuarterlyEconomyHistory,
    /// Meses consecutivos en quiebra (rivales; el jugador usa `GameState::bankruptcy_streak`).
    #[serde(default)]
    pub bankruptcy_months: u8,
}

impl Company {
    #[must_use]
    pub fn player(economy: CompanyEconomy, colour: u8) -> Self {
        Self {
            id: CompanyId::PLAYER,
            name: "Jugador".to_string(),
            colour,
            economy,
            is_ai: false,
            cargo_income_earned: 0,
            vehicle_running_costs: 0,
            cargo_deliveries: 0,
            economy_history: crate::game_state::EconomyHistory::default(),
            quarterly_economy: crate::economy_quarterly::QuarterlyEconomyHistory::default(),
            bankruptcy_months: 0,
        }
    }

    #[must_use]
    pub fn rival_transcargo(economy: CompanyEconomy, colour: u8) -> Self {
        Self {
            id: CompanyId(1),
            name: "TransCargo".to_string(),
            colour,
            economy,
            is_ai: true,
            cargo_income_earned: 0,
            vehicle_running_costs: 0,
            cargo_deliveries: 0,
            economy_history: crate::game_state::EconomyHistory::default(),
            quarterly_economy: crate::economy_quarterly::QuarterlyEconomyHistory::default(),
            bankruptcy_months: 0,
        }
    }
}

/// Fracción del pago feeder (`_settings_game.economy.feeder_payment_share`, default 75 %).
///
/// `OpenTTD` acumula `feeder_share` por packet; aquí se acredita al owner de
/// `first_station` si difiere del destino de descarga.
pub const FEEDER_SHARE_NUM: i64 = 75;
pub const FEEDER_SHARE_DEN: i64 = 100;

/// Parte del pago que corresponde al feeder (`first_station`).
#[must_use]
pub fn feeder_share_of(payment: i64) -> i64 {
    if payment <= 0 {
        return 0;
    }
    payment.saturating_mul(FEEDER_SHARE_NUM) / FEEDER_SHARE_DEN
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn feeder_share_is_three_quarters() {
        assert_eq!(feeder_share_of(100), 75);
        assert_eq!(feeder_share_of(0), 0);
        assert_eq!(feeder_share_of(-10), 0);
    }
}
