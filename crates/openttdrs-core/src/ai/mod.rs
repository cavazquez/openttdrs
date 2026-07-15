//! Hooks para decisión automática de compañías (rivales CPU).
//!
//! Ver `docs/epics/ai_rivals.md`. La medición headless vive en [`crate::dev_metrics`].

mod rule_based;
mod settings;

pub use rule_based::{AI_BUILD_MONEY_THRESHOLD, MAX_AI_ROUTES, TransCargoAi};
pub use settings::{AiSettings, DEFAULT_AI_BUILD_MONEY_THRESHOLD, DEFAULT_AI_MAX_ROUTES};

use crate::GameState;
use crate::command::Command;
use crate::economy::TICKS_PER_MONTH;
use crate::format_money;
use crate::vehicle::{VehicleKind, VehicleOrder};

/// Política que propone comandos para una compañía (rival o bot de pruebas).
pub trait CompanyAi {
    fn decide(&self, state: &GameState) -> Vec<Command>;
}

/// Ciclo de IA: mantenimiento de vehículos cada tick; construcción mensual.
pub fn tick_ai_companies(state: &mut GameState, tick: u64) {
    if !state.ai.enabled {
        return;
    }
    if !state.companies.iter().any(|c| c.is_ai) {
        return;
    }
    rule_based::maintain_transcargo_vehicles(state);
    if tick == 0 || !tick.is_multiple_of(TICKS_PER_MONTH) {
        return;
    }
    rule_based::tick_transcargo(state);
}

/// Texto de debug para la ventana AI settings (#44).
#[must_use]
pub fn format_ai_debug_status(state: &GameState) -> String {
    let ai = state.ai.clamped();
    let mut lines = vec![format!(
        "IA: {} · umbral {} · máx. rutas {}",
        if ai.enabled { "ON" } else { "OFF" },
        format_money(ai.build_money_threshold),
        ai.max_routes
    )];
    let Some(company) = state.companies.iter().find(|c| c.is_ai) else {
        lines.push("Sin compañía IA en la partida.".into());
        return lines.join("\n");
    };
    let econ = state.company_economy(company.id);
    lines.push(format!(
        "{} · color {} · {}",
        company.name,
        company.colour,
        format_money(econ.money)
    ));
    let trains: Vec<_> = state
        .vehicles
        .iter()
        .filter(|v| v.owner == company.id && v.kind == VehicleKind::Train && v.is_consist_head())
        .collect();
    lines.push(format!(
        "Rutas / trenes: {} / {}",
        trains.len(),
        ai.max_routes
    ));
    if trains.is_empty() {
        lines.push("(sin trenes IA)".into());
    }
    for v in trains {
        let stations: Vec<String> = v
            .orders
            .iter()
            .filter_map(|o| match o {
                VehicleOrder::Station { station, .. } => {
                    Some(format!("({},{})", station.x, station.y))
                }
                _ => None,
            })
            .collect();
        let cargo = v
            .cargo_type
            .map_or_else(|| "?".into(), |c| format!("{c:?}"));
        let route = if stations.is_empty() {
            "sin órdenes".into()
        } else {
            stations.join(" → ")
        };
        lines.push(format!(
            "  tren #{} · {} · {} · {}",
            v.id,
            cargo,
            if v.running { "marcha" } else { "parado" },
            route
        ));
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_ai_skips_construction() {
        let mut state = GameState::new(32, 32);
        state.ensure_rival_transcargo();
        state.ai.enabled = false;
        let before = state.vehicles.len();
        for _ in 0..(TICKS_PER_MONTH + 2) {
            let tick = state.tick.get();
            tick_ai_companies(&mut state, tick);
            state.tick.advance();
        }
        assert_eq!(state.vehicles.len(), before);
    }

    #[test]
    fn debug_status_lists_rival_name() {
        let mut state = GameState::new(16, 16);
        state.ensure_rival_transcargo();
        let text = format_ai_debug_status(&state);
        assert!(text.contains("TransCargo"));
        assert!(text.contains("ON"));
    }
}
