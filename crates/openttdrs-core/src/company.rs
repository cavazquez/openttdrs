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
        }
    }
}

/// Fracción del pago que va a la estación de primer embarque (feeder MVP).
///
/// `OpenTTD` usa `feeder_share` por packet; aquí un 25 % fijo del ingreso de
/// entrega si `first_station` ≠ destino y pertenece a otra compañía o a la misma
/// (crédito a la estación origen vía owner).
pub const FEEDER_SHARE_NUM: i64 = 1;
pub const FEEDER_SHARE_DEN: i64 = 4;

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
    fn feeder_share_is_quarter() {
        assert_eq!(feeder_share_of(100), 25);
        assert_eq!(feeder_share_of(0), 0);
        assert_eq!(feeder_share_of(-10), 0);
    }
}
