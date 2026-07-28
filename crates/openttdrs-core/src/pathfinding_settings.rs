//! Ajustes de pathfinding / PBS (`pf.*` en `OpenTTD`).

use crate::economy::TICKS_PER_DAY;

/// Días de espera por defecto ante path signal sin reserva (`pf.wait_for_pbs_path`).
pub const DEFAULT_WAIT_FOR_PBS_PATH_DAYS: u8 = 30;

/// Intervalo de reintento de reserva (`pf.path_backoff_interval`).
pub const DEFAULT_PATH_BACKOFF_INTERVAL: u8 = 20;

/// Espera ante señal unidireccional (`pf.wait_oneway_signal`, días).
pub const DEFAULT_WAIT_ONEWAY_SIGNAL_DAYS: u8 = 15;

/// Espera ante señal bidireccional (`pf.wait_twoway_signal`, días).
pub const DEFAULT_WAIT_TWOWAY_SIGNAL_DAYS: u8 = 41;

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
    /// Días de espera ante señal unidireccional roja antes de girar (2..=255).
    #[serde(default = "default_wait_oneway")]
    pub wait_oneway_signal: u8,
    /// Días de espera ante señal bidireccional roja antes de girar (2..=255).
    #[serde(default = "default_wait_twoway")]
    pub wait_twoway_signal: u8,
    /// Forzar reserva PBS también en segmentos sin path signal (`pf.reserve_paths`).
    ///
    /// Vanilla `OpenTTD`: `false`. Con `false`, sólo se reserva cuando el segmento
    /// delante del tren es PBS (`SigSegState::Path`).
    #[serde(default = "default_reserve_paths")]
    pub reserve_paths: bool,
}

fn default_wait_oneway() -> u8 {
    DEFAULT_WAIT_ONEWAY_SIGNAL_DAYS
}

fn default_wait_twoway() -> u8 {
    DEFAULT_WAIT_TWOWAY_SIGNAL_DAYS
}

fn default_reserve_paths() -> bool {
    false
}

impl Default for PathfindingSettings {
    fn default() -> Self {
        Self {
            wait_for_pbs_path: DEFAULT_WAIT_FOR_PBS_PATH_DAYS,
            path_backoff_interval: DEFAULT_PATH_BACKOFF_INTERVAL,
            reverse_at_signals: true,
            wait_oneway_signal: DEFAULT_WAIT_ONEWAY_SIGNAL_DAYS,
            wait_twoway_signal: DEFAULT_WAIT_TWOWAY_SIGNAL_DAYS,
            reserve_paths: false,
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
        Some(u32::from(self.wait_for_pbs_path).saturating_mul(TICKS_PER_DAY))
    }

    /// Ticks de espera ante señal unidireccional (`wait_oneway_signal * DAY_TICKS * 2`).
    #[must_use]
    pub fn oneway_signal_timeout_ticks(self) -> Option<u32> {
        if self.wait_oneway_signal == PBS_WAIT_FOREVER || !self.reverse_at_signals {
            return None;
        }
        Some(
            u32::from(self.wait_oneway_signal)
                .saturating_mul(TICKS_PER_DAY)
                .saturating_mul(2),
        )
    }

    /// Ticks de espera ante señal bidireccional (`wait_twoway_signal * DAY_TICKS * 2`).
    #[must_use]
    pub fn twoway_signal_timeout_ticks(self) -> Option<u32> {
        if self.wait_twoway_signal == PBS_WAIT_FOREVER || !self.reverse_at_signals {
            return None;
        }
        Some(
            u32::from(self.wait_twoway_signal)
                .saturating_mul(TICKS_PER_DAY)
                .saturating_mul(2),
        )
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
        assert!(no_reverse.oneway_signal_timeout_ticks().is_none());
        assert_eq!(
            PathfindingSettings::default().pbs_reverse_timeout_ticks(),
            Some(30 * TICKS_PER_DAY)
        );
        assert_eq!(
            PathfindingSettings::default().oneway_signal_timeout_ticks(),
            Some(15 * TICKS_PER_DAY * 2)
        );
        assert_eq!(
            PathfindingSettings::default().twoway_signal_timeout_ticks(),
            Some(41 * TICKS_PER_DAY * 2)
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
