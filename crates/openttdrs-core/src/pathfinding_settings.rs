//! Ajustes de pathfinding / PBS (`pf.*` en `OpenTTD`).

use crate::economy::TICKS_PER_TRANSIT_DAY;

/// Días de espera por defecto ante path signal sin reserva (`pf.wait_for_pbs_path`).
pub const DEFAULT_WAIT_FOR_PBS_PATH_DAYS: u8 = 30;

/// Intervalo de reintento de reserva (`pf.path_backoff_interval`).
pub const DEFAULT_PATH_BACKOFF_INTERVAL: u8 = 20;

/// Valor especial: no girar nunca / no hacer look-ahead.
pub const PBS_WAIT_FOREVER: u8 = 255;

/// Ajustes de pathfinding persistidos en la partida.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PathfindingSettings {
    /// Días de espera ante path sin reserva antes de girar (2..=255; 255 = nunca).
    pub wait_for_pbs_path: u8,
    /// Ticks entre reintentos de reserva / look-ahead (1..=255; 255 = desactivar look-ahead).
    pub path_backoff_interval: u8,
    /// Si `false`, no girar automáticamente en señales (incluye path stuck).
    pub reverse_at_signals: bool,
}

impl Default for PathfindingSettings {
    fn default() -> Self {
        Self {
            wait_for_pbs_path: DEFAULT_WAIT_FOR_PBS_PATH_DAYS,
            path_backoff_interval: DEFAULT_PATH_BACKOFF_INTERVAL,
            reverse_at_signals: true,
        }
    }
}

impl PathfindingSettings {
    /// Ticks de espera antes de girar en path (`wait_for_pbs_path * DAY_TICKS`).
    #[must_use]
    pub fn pbs_reverse_timeout_ticks(self) -> Option<u32> {
        if self.wait_for_pbs_path == PBS_WAIT_FOREVER || !self.reverse_at_signals {
            return None;
        }
        Some(u32::from(self.wait_for_pbs_path).saturating_mul(TICKS_PER_TRANSIT_DAY))
    }

    /// `true` si este tick debe reintentar look-ahead / reserva (`path_backoff_interval`).
    #[must_use]
    pub fn should_retry_reservation(self, wait_counter: u32) -> bool {
        if self.path_backoff_interval == PBS_WAIT_FOREVER {
            return false;
        }
        let interval = u32::from(self.path_backoff_interval.max(1));
        wait_counter.is_multiple_of(interval)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forever_disables_reverse_timeout() {
        let forever = PathfindingSettings {
            wait_for_pbs_path: PBS_WAIT_FOREVER,
            ..Default::default()
        };
        assert!(forever.pbs_reverse_timeout_ticks().is_none());
        let no_reverse = PathfindingSettings {
            reverse_at_signals: false,
            ..Default::default()
        };
        assert!(no_reverse.pbs_reverse_timeout_ticks().is_none());
        assert_eq!(
            PathfindingSettings::default().pbs_reverse_timeout_ticks(),
            Some(30 * TICKS_PER_TRANSIT_DAY)
        );
    }

    #[test]
    fn backoff_255_never_retries() {
        let off = PathfindingSettings {
            path_backoff_interval: PBS_WAIT_FOREVER,
            ..Default::default()
        };
        assert!(!off.should_retry_reservation(20));
        let on = PathfindingSettings {
            path_backoff_interval: 20,
            ..Default::default()
        };
        assert!(on.should_retry_reservation(40));
        assert!(!on.should_retry_reservation(41));
    }
}
