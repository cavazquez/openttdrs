//! Autoreemplazo de motores en depósito (paridad reducida con `OpenTTD` autoreplace).

use crate::engine::{EngineDef, engine_available_in_year, engine_by_id};
use crate::refit::{refittable_cargo_types, vehicle_in_depot};
use crate::vehicle::Vehicle;
use crate::{GameState, economy};

/// Regla: al entrar en depósito, sustituir `from_engine_id` por `to_engine_id`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AutoReplaceRule {
    pub from_engine_id: u16,
    pub to_engine_id: u16,
    #[serde(default = "default_rule_enabled")]
    pub enabled: bool,
    #[serde(default)]
    pub only_when_old: bool,
    /// `None` = regla global; `Some(id)` = solo vehículos del grupo.
    #[serde(default)]
    pub group_id: Option<u32>,
}

const fn default_rule_enabled() -> bool {
    true
}

impl AutoReplaceRule {
    #[must_use]
    pub const fn new(from_engine_id: u16, to_engine_id: u16) -> Self {
        Self {
            from_engine_id,
            to_engine_id,
            enabled: true,
            only_when_old: false,
            group_id: None,
        }
    }
}

#[must_use]
pub fn resolve_rule(
    rules: &[AutoReplaceRule],
    from_engine_id: u16,
    vehicle_group: Option<u32>,
) -> Option<&AutoReplaceRule> {
    if let Some(gid) = vehicle_group
        && let Some(r) = rules
            .iter()
            .find(|r| r.enabled && r.from_engine_id == from_engine_id && r.group_id == Some(gid))
    {
        return Some(r);
    }
    rules
        .iter()
        .find(|r| r.enabled && r.from_engine_id == from_engine_id && r.group_id.is_none())
}

fn autoreplace_cost(vehicle: &Vehicle, new_engine: &EngineDef) -> i64 {
    new_engine.price - economy::vehicle_sell_refund(vehicle)
}

fn apply_engine_with_refit(vehicle: &mut Vehicle, new_engine: &EngineDef) {
    vehicle.engine_id = Some(new_engine.id);
    if new_engine.capacity > 0 {
        vehicle.capacity = new_engine.capacity;
    }
    if let Some(c) = new_engine.cargo {
        vehicle.cargo_type = Some(c);
    } else if let Some(current) = vehicle.cargo_type {
        let mut probe = vehicle.clone();
        probe.engine_id = Some(new_engine.id);
        if !refittable_cargo_types(&probe).contains(&current)
            && let Some(&first) = refittable_cargo_types(&probe).first()
        {
            vehicle.cargo_type = Some(first);
        }
    }
}

/// Intenta reemplazar el motor del vehículo si hay regla activa y fondos.
pub fn try_autoreplace_vehicle(
    state: &mut GameState,
    vehicle_id: u32,
) -> Result<bool, crate::CommandError> {
    use crate::CommandError;

    let calendar_year = crate::rail_signals::calendar_year_at_tick(state.tick);
    let current_tick = state.tick.get();
    let Some(vehicle_idx) = state.vehicles.iter().position(|v| v.id == vehicle_id) else {
        return Err(CommandError::VehicleNotFound);
    };

    let vehicle = &state.vehicles[vehicle_idx];
    if vehicle.cargo > 0 || !vehicle_in_depot(&state.map, vehicle.pos) {
        return Ok(false);
    }
    let from_engine_id = vehicle
        .engine_id
        .unwrap_or(crate::engine::default_engine_id(vehicle.kind));
    let Some(rule) =
        resolve_rule(&state.autoreplace_rules, from_engine_id, vehicle.group_id).copied()
    else {
        return Ok(false);
    };
    if rule.only_when_old && !vehicle.needs_autorenewing(current_tick) {
        return Ok(false);
    }
    if rule.from_engine_id == rule.to_engine_id {
        return Ok(false);
    }
    let Some(new_engine) = engine_by_id(rule.to_engine_id) else {
        return Err(CommandError::EngineNotFound);
    };
    if new_engine.kind != vehicle.kind {
        return Err(CommandError::AutoreplaceNotAllowed);
    }
    if !engine_available_in_year(new_engine, calendar_year) {
        crate::news::push_autoreplace_failed_news(state, vehicle_id, CommandError::EngineNotFound);
        return Ok(false);
    }

    let vehicle = &state.vehicles[vehicle_idx];
    let cost = autoreplace_cost(vehicle, new_engine);
    if state.economy.money < cost {
        crate::news::push_autoreplace_failed_news(
            state,
            vehicle_id,
            CommandError::InsufficientFunds,
        );
        return Ok(false);
    }

    let vehicle = &mut state.vehicles[vehicle_idx];
    apply_engine_with_refit(vehicle, new_engine);
    state.economy.money -= cost;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn group_rule_wins_over_global() {
        let rules = vec![
            AutoReplaceRule {
                from_engine_id: 10,
                to_engine_id: 11,
                enabled: true,
                only_when_old: false,
                group_id: None,
            },
            AutoReplaceRule {
                from_engine_id: 10,
                to_engine_id: 12,
                enabled: true,
                only_when_old: false,
                group_id: Some(1),
            },
        ];
        assert_eq!(
            resolve_rule(&rules, 10, Some(1)).map(|r| r.to_engine_id),
            Some(12)
        );
        assert_eq!(
            resolve_rule(&rules, 10, None).map(|r| r.to_engine_id),
            Some(11)
        );
    }
}
