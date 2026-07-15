//! Ajustes in-game de la IA rival (`AiSettings`, UI-8 / #44).

/// Umbral de efectivo por defecto antes de abrir una ruta nueva.
pub const DEFAULT_AI_BUILD_MONEY_THRESHOLD: i64 = 80_000;
/// Máximo de líneas (trenes head) por defecto.
pub const DEFAULT_AI_MAX_ROUTES: u8 = 2;

/// Ajustes de IA persistidos en la partida.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AiSettings {
    /// Si `false`, no construye ni mantiene decisiones de `TransCargo`.
    pub enabled: bool,
    /// Efectivo mínimo de la IA antes de abrir una ruta nueva.
    pub build_money_threshold: i64,
    /// Máximo de líneas (1..=4).
    pub max_routes: u8,
}

impl Default for AiSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            build_money_threshold: DEFAULT_AI_BUILD_MONEY_THRESHOLD,
            max_routes: DEFAULT_AI_MAX_ROUTES,
        }
    }
}

impl AiSettings {
    /// Normaliza umbral y tope de rutas a rangos jugables.
    #[must_use]
    pub fn clamped(self) -> Self {
        Self {
            enabled: self.enabled,
            build_money_threshold: self.build_money_threshold.clamp(10_000, 500_000),
            max_routes: self.max_routes.clamp(1, 4),
        }
    }

    #[must_use]
    pub fn max_routes_usize(self) -> usize {
        usize::from(self.clamped().max_routes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamp_bounds_threshold_and_routes() {
        let s = AiSettings {
            enabled: true,
            build_money_threshold: 1,
            max_routes: 99,
        }
        .clamped();
        assert_eq!(s.build_money_threshold, 10_000);
        assert_eq!(s.max_routes, 4);
        assert_eq!(s.max_routes_usize(), 4);
    }
}
