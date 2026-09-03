//! Historiales mensuales de pueblos e industrias (UI-3 gráficos).

use crate::game_state::ECONOMY_HISTORY_MONTHS;

/// Misma profundidad que los gráficos económicos.
pub const ENTITY_HISTORY_MONTHS: usize = ECONOMY_HISTORY_MONTHS;

/// Número de registros que `OpenTTD` reserva para los historiales de
/// industrias (`HISTORY_RECORDS`: mes actual + meses/trimestres/años).
///
/// La ventana nativa contiene el mes actual, 24 meses, 17 agregados
/// trimestrales y 19 agregados anuales: 61 posiciones (`misc/history_type.hpp`).
/// El runtime reducido todavía no expone las vistas trimestrales/anuales, pero
/// conserva la longitud completa para que un `INDY` mutado no trunque el save.
pub const INDUSTRY_HISTORY_RECORDS: usize = 61;

/// Muestra mensual de un cargo aceptado por una industria.
///
/// El orden es el nativo de `OpenTTD`: índice cero es el mes actual y los
/// siguientes son meses anteriores. `waiting` es el promedio diario del mes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct IndustryAcceptedHistorySample {
    pub accepted: u16,
    pub waiting: u16,
}

/// Muestra mensual de un cargo producido por una industria.
///
/// El orden es el nativo de `OpenTTD`: índice cero es el mes actual y los
/// siguientes son meses anteriores. Ambos contadores son unidades de carga
/// representables por los campos `uint16` de `SlIndustryProducedHistory`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct IndustryProducedHistorySample {
    pub production: u16,
    pub transported: u16,
}

/// Muestra mensual de un pueblo.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TownHistorySample {
    /// Población al cierre del mes.
    pub population: u32,
    /// Pasajeros entregados durante el mes (delta).
    pub passengers_served: u32,
    /// Correo entregado durante el mes (delta).
    pub mail_served: u32,
    /// Valoración de autoridad al cierre.
    pub rating: i16,
}

/// Ring buffer de muestras mensuales de pueblo.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TownHistory {
    pub samples: Vec<TownHistorySample>,
    #[serde(default)]
    pub last_passengers_served: u32,
    #[serde(default)]
    pub last_mail_served: u32,
}

impl TownHistory {
    /// Registra el mes a partir de totales actuales del pueblo.
    pub fn push_month(
        &mut self,
        population: u32,
        passengers_served_total: u32,
        mail_served_total: u32,
        rating: i16,
    ) {
        let sample = TownHistorySample {
            population,
            passengers_served: passengers_served_total.saturating_sub(self.last_passengers_served),
            mail_served: mail_served_total.saturating_sub(self.last_mail_served),
            rating,
        };
        self.last_passengers_served = passengers_served_total;
        self.last_mail_served = mail_served_total;
        self.samples.push(sample);
        if self.samples.len() > ENTITY_HISTORY_MONTHS {
            let drop = self.samples.len() - ENTITY_HISTORY_MONTHS;
            self.samples.drain(0..drop);
        }
    }
}

/// Muestra mensual de una industria.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct IndustryHistorySample {
    /// Stock al cierre del mes.
    pub stock: u32,
    /// Unidades producidas durante el mes (delta).
    pub produced: u32,
    /// Unidades cargadas/transportadas durante el mes (delta).
    pub transported: u32,
}

/// Ring buffer de muestras mensuales de industria.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct IndustryHistory {
    pub samples: Vec<IndustryHistorySample>,
    #[serde(default)]
    pub last_produced_total: u64,
    #[serde(default)]
    pub last_transported_total: u64,
}

impl IndustryHistory {
    /// Registra el mes a partir de totales actuales de la industria.
    pub fn push_month(&mut self, stock: u32, produced_total: u64, transported_total: u64) {
        let sample = IndustryHistorySample {
            stock,
            produced: u32::try_from(produced_total.saturating_sub(self.last_produced_total))
                .unwrap_or(u32::MAX),
            transported: u32::try_from(
                transported_total.saturating_sub(self.last_transported_total),
            )
            .unwrap_or(u32::MAX),
        };
        self.last_produced_total = produced_total;
        self.last_transported_total = transported_total;
        self.samples.push(sample);
        if self.samples.len() > ENTITY_HISTORY_MONTHS {
            let drop = self.samples.len() - ENTITY_HISTORY_MONTHS;
            self.samples.drain(0..drop);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn town_history_stores_deltas_and_caps() {
        let mut history = TownHistory::default();
        history.push_month(100, 5, 2, 10);
        assert_eq!(history.samples.len(), 1);
        assert_eq!(history.samples[0].passengers_served, 5);
        assert_eq!(history.samples[0].population, 100);

        history.push_month(110, 12, 2, 10);
        assert_eq!(history.samples[1].passengers_served, 7);
        assert_eq!(history.samples[1].population, 110);

        for i in 0..ENTITY_HISTORY_MONTHS {
            history.push_month(
                110 + u32::try_from(i).unwrap_or(0),
                12 + u32::try_from(i).unwrap_or(0) + 1,
                2,
                10,
            );
        }
        assert_eq!(history.samples.len(), ENTITY_HISTORY_MONTHS);
    }

    #[test]
    fn industry_history_tracks_produced_and_transported() {
        let mut history = IndustryHistory::default();
        history.push_month(12, 16, 4);
        assert_eq!(history.samples[0].produced, 16);
        assert_eq!(history.samples[0].transported, 4);
        assert_eq!(history.samples[0].stock, 12);

        history.push_month(8, 24, 10);
        assert_eq!(history.samples[1].produced, 8);
        assert_eq!(history.samples[1].transported, 6);
        assert_eq!(history.samples[1].stock, 8);
    }
}
