//! Autoreemplazo de motores en depósito (paridad reducida con `OpenTTD` autoreplace).
//!
//! Incluye `ReplaceChain` (P3.12): reconstrucción de consist con dual-head y
//! wagon removal (`renew_keep_length`).

use crate::engine::{EngineDef, engine_available_in_year, engine_by_id};
use crate::refit::{refittable_cargo_types, vehicle_in_depot};
use crate::train_consist::{consist_changed, consist_unit_ids, detach_unit, engine_is_wagon};
use crate::vehicle::{Vehicle, VehicleKind};
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

fn apply_engine_with_refit(vehicle: &mut Vehicle, new_engine: &EngineDef, current_tick: u64) {
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
    vehicle.build_tick = current_tick;
    crate::vehicle::init_vehicle_reliability_from_engine(vehicle, new_engine);
}

fn company_for_vehicle(
    state: &GameState,
    owner: crate::company::CompanyId,
) -> Option<&crate::company::Company> {
    state.companies.get(owner.index())
}

fn can_afford_replacement(money: i64, cost: i64, engine_renew_money: i64) -> bool {
    money >= cost.saturating_add(engine_renew_money)
}

/// Evalúa si hay un reemplazo/autorenovación pendiente con fondos (`NeedsServicing` autoreplace).
#[must_use]
pub fn pending_autoreplace_for_service(state: &GameState, vehicle: &Vehicle) -> bool {
    let Some(company) = company_for_vehicle(state, vehicle.owner) else {
        return false;
    };
    if company.economy.money < company.engine_renew_money {
        return false;
    }
    let mut needed_money = company.engine_renew_money;
    let from_engine_id = vehicle
        .engine_id
        .unwrap_or(crate::engine::default_engine_id(vehicle.kind));
    let calendar_year = crate::rail_signals::calendar_year_at_tick(state.tick);
    let current_tick = state.tick.get();

    if let Some(rule) = resolve_rule(&state.autoreplace_rules, from_engine_id, vehicle.group_id)
        && rule.from_engine_id != rule.to_engine_id
    {
        if rule.only_when_old
            && !vehicle.needs_autorenewing(current_tick, company.engine_renew_months)
        {
            return false;
        }
        if let Some(new_engine) = engine_by_id(rule.to_engine_id)
            && new_engine.kind == vehicle.kind
            && engine_available_in_year(new_engine, calendar_year)
        {
            needed_money = needed_money.saturating_add(autoreplace_cost(vehicle, new_engine));
            return needed_money <= company.economy.money;
        }
    }

    if !company.engine_renew
        || !vehicle.needs_autorenewing(current_tick, company.engine_renew_months)
    {
        return false;
    }
    let Some(engine) = engine_by_id(from_engine_id) else {
        return false;
    };
    if !engine_available_in_year(engine, calendar_year) {
        return false;
    }
    needed_money = needed_money.saturating_add(autoreplace_cost(vehicle, engine));
    needed_money <= company.economy.money
}

/// Intenta reemplazar el motor del vehículo si hay regla activa o autorenovación y fondos.
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
    let owner = vehicle.owner;
    let from_engine_id = vehicle
        .engine_id
        .unwrap_or(crate::engine::default_engine_id(vehicle.kind));
    let Some(company) = company_for_vehicle(state, owner) else {
        return Ok(false);
    };

    if let Some(rule) =
        resolve_rule(&state.autoreplace_rules, from_engine_id, vehicle.group_id).copied()
    {
        if rule.only_when_old
            && !vehicle.needs_autorenewing(current_tick, company.engine_renew_months)
        {
            return Ok(false);
        }
        if rule.from_engine_id != rule.to_engine_id {
            let Some(new_engine) = engine_by_id(rule.to_engine_id) else {
                return Err(CommandError::EngineNotFound);
            };
            if new_engine.kind != vehicle.kind {
                return Err(CommandError::AutoreplaceNotAllowed);
            }
            if !engine_available_in_year(new_engine, calendar_year) {
                crate::news::push_autoreplace_failed_news(
                    state,
                    vehicle_id,
                    CommandError::EngineNotFound,
                );
                return Ok(false);
            }
            let wagon_removal = company.renew_keep_length;
            let cost = autoreplace_cost(&state.vehicles[vehicle_idx], new_engine);
            if !can_afford_replacement(company.economy.money, cost, company.engine_renew_money) {
                crate::news::push_autoreplace_failed_news(
                    state,
                    vehicle_id,
                    CommandError::InsufficientFunds,
                );
                return Ok(false);
            }
            replace_chain(state, vehicle_id, new_engine, wagon_removal, current_tick)?;
            if let Some(c) = state.companies.get_mut(owner.index()) {
                c.economy.money -= cost;
            }
            if state.active_company == owner {
                state.economy.money -= cost;
            }
            return Ok(true);
        }
    }

    if !company.engine_renew
        || !vehicle.needs_autorenewing(current_tick, company.engine_renew_months)
    {
        return Ok(false);
    }
    let Some(new_engine) = engine_by_id(from_engine_id) else {
        return Err(CommandError::EngineNotFound);
    };
    if !engine_available_in_year(new_engine, calendar_year) {
        crate::news::push_autoreplace_failed_news(state, vehicle_id, CommandError::EngineNotFound);
        return Ok(false);
    }
    let wagon_removal = company.renew_keep_length;
    let vehicle = &state.vehicles[vehicle_idx];
    let cost = autoreplace_cost(vehicle, new_engine);
    if !can_afford_replacement(company.economy.money, cost, company.engine_renew_money) {
        crate::news::push_autoreplace_failed_news(
            state,
            vehicle_id,
            CommandError::InsufficientFunds,
        );
        return Ok(false);
    }
    replace_chain(state, vehicle_id, new_engine, wagon_removal, current_tick)?;
    if let Some(c) = state.companies.get_mut(owner.index()) {
        c.economy.money -= cost;
    }
    if state.active_company == owner {
        state.economy.money -= cost;
    }
    Ok(true)
}

/// `ReplaceChain` — reconstruye el consist al autoreemplazar (`autoreplace_cmd.cpp`).
///
/// - Cambia el motor de la cabeza (y unidades con la misma regla, si aplica).
/// - Dual-head: spawnea/elimina la unidad trasera según el nuevo motor.
/// - Wagon removal: si `wagon_removal` y el consist supera la longitud antigua
///   redondeada a teselas, vende vagones desde la cola.
fn replace_chain(
    state: &mut GameState,
    head_id: u32,
    new_engine: &EngineDef,
    wagon_removal: bool,
    current_tick: u64,
) -> Result<(), crate::CommandError> {
    let Some(head_idx) = state.vehicles.iter().position(|v| v.id == head_id) else {
        return Err(crate::CommandError::VehicleNotFound);
    };
    let is_train = state.vehicles[head_idx].kind == VehicleKind::Train;
    let old_total = state.vehicles[head_idx].cached_total_length;
    // Longitud antigua redondeada a teselas (OpenTTD: CeilDiv(..., TILE_SIZE)*TILE_SIZE).
    let old_total_rounded = old_total.div_ceil(16).saturating_mul(16);

    apply_engine_with_refit(&mut state.vehicles[head_idx], new_engine, current_tick);

    if !is_train {
        return Ok(());
    }

    sync_dual_head_after_replace(state, head_id, new_engine, current_tick);

    // Reemplazar otras unidades del consist con la misma regla from→to (vagones).
    let unit_ids = consist_unit_ids(&state.vehicles, head_id);
    for &uid in &unit_ids {
        if uid == head_id {
            continue;
        }
        let Some(v) = state.vehicles.iter().find(|v| v.id == uid) else {
            continue;
        };
        let from = v
            .engine_id
            .unwrap_or(crate::engine::default_engine_id(v.kind));
        let Some(rule) = resolve_rule(&state.autoreplace_rules, from, v.group_id).copied() else {
            continue;
        };
        if rule.from_engine_id == rule.to_engine_id {
            continue;
        }
        let Some(eng) = engine_by_id(rule.to_engine_id) else {
            continue;
        };
        if let Some(unit) = state.vehicles.iter_mut().find(|v| v.id == uid) {
            apply_engine_with_refit(unit, eng, current_tick);
        }
    }

    consist_changed(&mut state.vehicles, head_id);

    if wagon_removal {
        trim_consist_to_length(state, head_id, old_total_rounded);
    }
    Ok(())
}

fn sync_dual_head_after_replace(
    state: &mut GameState,
    head_id: u32,
    new_engine: &EngineDef,
    current_tick: u64,
) {
    let rear_id = state
        .vehicles
        .iter()
        .find(|v| v.id == head_id)
        .and_then(|v| v.other_multiheaded_part);

    if new_engine.is_dual_headed() {
        if rear_id.is_some() {
            // Actualizar motor de la trasera existente.
            if let Some(rid) = rear_id
                && let Some(rear) = state.vehicles.iter_mut().find(|v| v.id == rid)
            {
                apply_engine_with_refit(rear, new_engine, current_tick);
            }
            return;
        }
        // Spawnear trasera dual-head.
        let next_id = state
            .vehicles
            .iter()
            .map(|v| v.id)
            .max()
            .unwrap_or(0)
            .saturating_add(1);
        let Some(head) = state.vehicles.iter().find(|v| v.id == head_id) else {
            return;
        };
        let depot_pos = head.pos;
        let owner = head.owner;
        let direction = head.direction;
        // Insertar trasera justo después de la cabeza en la cadena.
        let old_next = state
            .vehicles
            .iter()
            .find(|v| v.id == head_id)
            .and_then(|v| v.next_unit);
        let mut rear = Vehicle::new(next_id, new_engine.kind, depot_pos, depot_pos);
        rear.running = false;
        rear.engine_id = Some(new_engine.id);
        rear.capacity = new_engine.capacity;
        rear.cargo_type = new_engine.cargo;
        rear.build_tick = current_tick;
        rear.owner = owner;
        rear.direction = direction;
        rear.prev_unit = Some(head_id);
        rear.next_unit = old_next;
        rear.other_multiheaded_part = Some(head_id);
        rear.depot_leave_cleared = false;
        if let Some(n) = old_next
            && let Some(nv) = state.vehicles.iter_mut().find(|v| v.id == n)
        {
            nv.prev_unit = Some(next_id);
        }
        if let Some(front) = state.vehicles.iter_mut().find(|v| v.id == head_id) {
            front.next_unit = Some(next_id);
            front.other_multiheaded_part = Some(next_id);
        }
        state.vehicles.push(rear);
    } else if let Some(rid) = rear_id {
        // Quitar trasera dual-head obsoleta.
        let after = state
            .vehicles
            .iter()
            .find(|v| v.id == rid)
            .and_then(|v| v.next_unit);
        if let Some(front) = state.vehicles.iter_mut().find(|v| v.id == head_id) {
            front.next_unit = after;
            front.other_multiheaded_part = None;
        }
        if let Some(n) = after
            && let Some(nv) = state.vehicles.iter_mut().find(|v| v.id == n)
        {
            nv.prev_unit = Some(head_id);
        }
        state.vehicles.retain(|v| v.id != rid);
    }
}

fn trim_consist_to_length(state: &mut GameState, head_id: u32, max_length: u16) {
    while state
        .vehicles
        .iter()
        .find(|v| v.id == head_id)
        .is_some_and(|h| h.cached_total_length > max_length)
    {
        let ids = consist_unit_ids(&state.vehicles, head_id);
        // Buscar el último vagón (no dual-head rear).
        let Some(&tail) = ids.iter().rev().find(|&&id| {
            state.vehicles.iter().any(|v| {
                v.id == id
                    && v.id != head_id
                    && v.other_multiheaded_part.is_none()
                    && v.engine_id
                        .and_then(engine_by_id)
                        .is_some_and(engine_is_wagon)
            })
        }) else {
            break;
        };
        let _ = detach_unit(&mut state.vehicles, tail);
        state.vehicles.retain(|v| v.id != tail);
        consist_changed(&mut state.vehicles, head_id);
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::map::TileCoord;
    use crate::map::TileKind;
    use crate::vehicle::{Vehicle, VehicleKind};

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

    #[test]
    fn autorenew_same_engine_when_old_enough() {
        let mut state = GameState::new(8, 8);
        let depot = TileCoord::new(2, 2);
        state.map.set_kind(depot, TileKind::RoadDepot).unwrap();
        state.companies[0].engine_renew_money = 0;
        state.economy.money = 500_000;
        state.companies[0].economy.money = 500_000;
        let mut v = Vehicle::new(1, VehicleKind::Truck, depot, depot);
        v.engine_id = Some(crate::engine::default_engine_id(VehicleKind::Truck));
        v.max_age_days = 30;
        v.build_tick = 0;
        state.vehicles.push(v);
        state.tick = crate::GameTick::new(
            u64::from(30_u32 + 6 * 30) * u64::from(crate::economy::TICKS_PER_DAY),
        );
        assert!(try_autoreplace_vehicle(&mut state, 1).unwrap());
        assert_eq!(state.vehicles[0].build_tick, state.tick.get());
    }

    #[test]
    fn autorenew_respects_engine_renew_money() {
        let mut state = GameState::new(8, 8);
        let depot = TileCoord::new(2, 2);
        state.map.set_kind(depot, TileKind::RoadDepot).unwrap();
        state.companies[0].engine_renew_money = 1_000_000;
        state.companies[0].economy.money = 50_000;
        state.economy.money = 50_000;
        let mut v = Vehicle::new(1, VehicleKind::Truck, depot, depot);
        v.engine_id = Some(crate::engine::default_engine_id(VehicleKind::Truck));
        v.max_age_days = 30;
        v.build_tick = 0;
        state.vehicles.push(v);
        state.tick = crate::GameTick::new(
            u64::from(30_u32 + 6 * 30) * u64::from(crate::economy::TICKS_PER_DAY),
        );
        assert!(!try_autoreplace_vehicle(&mut state, 1).unwrap());
    }

    #[test]
    fn replace_chain_spawns_dual_head_rear() {
        use crate::engine::{ENGINE_TRAIN_KIRBY, ENGINE_TRAIN_MANLEY_MOREL};
        use crate::train_consist::consist_unit_ids;

        let mut state = GameState::new(8, 8);
        let depot = TileCoord::new(2, 2);
        state.map.set_kind(depot, TileKind::RailDepot).unwrap();
        state.companies[0].engine_renew_money = 0;
        state.economy.money = 5_000_000;
        state.companies[0].economy.money = 5_000_000;
        // Año ≥ 1956 para que Manley-Morel esté disponible.
        state.tick = crate::GameTick::new(
            u64::from(60_u32) * 365 * u64::from(crate::economy::TICKS_PER_DAY),
        );
        let mut head = Vehicle::new(1, VehicleKind::Train, depot, depot);
        head.engine_id = Some(ENGINE_TRAIN_KIRBY);
        state.vehicles.push(head);
        state.autoreplace_rules.push(AutoReplaceRule::new(
            ENGINE_TRAIN_KIRBY,
            ENGINE_TRAIN_MANLEY_MOREL,
        ));
        assert!(
            try_autoreplace_vehicle(&mut state, 1).unwrap(),
            "depot={:?} year={}",
            state.map.get_kind(depot),
            crate::rail_signals::calendar_year_at_tick(state.tick)
        );
        let ids = consist_unit_ids(&state.vehicles, 1);
        assert!(
            ids.len() >= 2,
            "dual-head debe spawnear trasera; units={ids:?}"
        );
        let head = state.vehicles.iter().find(|v| v.id == 1).unwrap();
        assert_eq!(head.engine_id, Some(ENGINE_TRAIN_MANLEY_MOREL));
        assert!(head.other_multiheaded_part.is_some());
    }

    #[test]
    fn replace_chain_wagon_removal_trims_tail() {
        use crate::engine::ENGINE_TRAIN_KIRBY;
        use crate::train_consist::{attach_wagon, consist_unit_ids};

        let mut state = GameState::new(8, 8);
        let depot = TileCoord::new(2, 2);
        state.map.set_kind(depot, TileKind::RailDepot).unwrap();
        state.companies[0].engine_renew_money = 0;
        state.companies[0].renew_keep_length = true;
        state.economy.money = 5_000_000;
        state.companies[0].economy.money = 5_000_000;
        let mut head = Vehicle::new(1, VehicleKind::Train, depot, depot);
        head.engine_id = Some(ENGINE_TRAIN_KIRBY);
        state.vehicles.push(head);
        for id in 2..=4 {
            let mut w = Vehicle::new(id, VehicleKind::Train, depot, depot);
            w.engine_id = Some(crate::engine::ENGINE_WAGON_PASSENGER);
            w.prev_unit = None;
            state.vehicles.push(w);
            let _ = attach_wagon(&mut state.vehicles, 1, id);
        }
        let before = consist_unit_ids(&state.vehicles, 1).len();
        assert!(before >= 3);
        // Autorrenew same engine dispara ReplaceChain + trim si crece.
        // Simular crecimiento artificial: subir longitud cacheada y forzar trim.
        if let Some(h) = state.vehicles.iter_mut().find(|v| v.id == 1) {
            h.cached_total_length = 80; // > redondeo de longitud real
        }
        // Llamar trim directo (API interna vía try con misma engine no vende).
        trim_consist_to_length(&mut state, 1, 16);
        let after = consist_unit_ids(&state.vehicles, 1).len();
        assert!(
            after < before,
            "wagon removal debe acortar; {before}→{after}"
        );
    }
}
