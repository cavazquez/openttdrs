//! Autoreemplazo de motores en depósito (paridad reducida con `OpenTTD` autoreplace).
//!
//! Incluye `ReplaceChain` (P3.12): reconstrucción de consist con dual-head y
//! wagon removal (`renew_keep_length`).

use crate::engine::{EngineDef, engine_available_in_year, engine_by_id};
use crate::refit::{refittable_cargo_types_for_engine_with_catalog_and_climate, vehicle_in_depot};
use crate::train_consist::{consist_unit_ids, detach_unit, engine_is_wagon};
use crate::vehicle::{Vehicle, VehicleKind};
use crate::{CompanyId, GameState, economy};

/// Regla: al entrar en depósito, sustituir `from_engine_id` por `to_engine_id`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AutoReplaceRule {
    pub from_engine_id: u16,
    pub to_engine_id: u16,
    /// Compañía propietaria de la regla. Los JSON anteriores no lo tenían y
    /// por compatibilidad se interpretan como reglas de la compañía jugadora.
    #[serde(default)]
    pub owner: Option<CompanyId>,
    #[serde(default = "default_rule_enabled")]
    pub enabled: bool,
    #[serde(default)]
    pub only_when_old: bool,
    /// `None` = regla para todos los grupos; `Some(id)` = solo vehículos del
    /// grupo concreto.
    #[serde(default)]
    pub group_id: Option<u32>,
    /// Regla para el grupo implícito de vehículos sin grupo (`DEFAULT_GROUP`)
    /// en vez de para todos (`ALL_GROUP`).
    #[serde(default)]
    pub default_group_only: bool,
    /// Índice del pool denso `ERNW` de `OpenTTD`, si provino de un `.sav`.
    /// Mantenerlo evita reordenar referencias externas al reexportar.
    #[serde(default)]
    pub sav_pool_id: Option<u16>,
    /// Siguiente nodo de la lista `ERNW` de la compañía, en índices de pool
    /// (no en la codificación de disco `index + 1`).
    #[serde(default)]
    pub sav_next_pool_id: Option<u16>,
}

const fn default_rule_enabled() -> bool {
    true
}

impl AutoReplaceRule {
    #[must_use]
    pub const fn new(from_engine_id: u16, to_engine_id: u16) -> Self {
        Self::new_for_company(from_engine_id, to_engine_id, CompanyId::PLAYER)
    }

    #[must_use]
    pub const fn new_for_company(from_engine_id: u16, to_engine_id: u16, owner: CompanyId) -> Self {
        Self {
            from_engine_id,
            to_engine_id,
            owner: Some(owner),
            enabled: true,
            only_when_old: false,
            group_id: None,
            default_group_only: false,
            sav_pool_id: None,
            sav_next_pool_id: None,
        }
    }
}

#[must_use]
pub fn resolve_rule(
    rules: &[AutoReplaceRule],
    from_engine_id: u16,
    vehicle_group: Option<u32>,
) -> Option<&AutoReplaceRule> {
    resolve_rule_for_company(rules, CompanyId::PLAYER, from_engine_id, vehicle_group)
}

/// Busca la regla de autoreemplazo para una compañía, respetando las dos
/// pseudo-categorías de `OpenTTD`: grupo concreto, `DEFAULT_GROUP` (sin grupo)
/// y el fallback `ALL_GROUP`.
#[must_use]
pub fn resolve_rule_for_company(
    rules: &[AutoReplaceRule],
    owner: CompanyId,
    from_engine_id: u16,
    vehicle_group: Option<u32>,
) -> Option<&AutoReplaceRule> {
    let applies_to_owner =
        |rule: &&AutoReplaceRule| rule.owner.unwrap_or(CompanyId::PLAYER) == owner;
    if let Some(gid) = vehicle_group
        && let Some(r) = rules.iter().find(|r| {
            applies_to_owner(r)
                && r.enabled
                && r.from_engine_id == from_engine_id
                && r.group_id == Some(gid)
        })
    {
        return Some(r);
    }
    if vehicle_group.is_none()
        && let Some(r) = rules.iter().find(|r| {
            applies_to_owner(r)
                && r.enabled
                && r.from_engine_id == from_engine_id
                && r.group_id.is_none()
                && r.default_group_only
        })
    {
        return Some(r);
    }
    rules.iter().find(|r| {
        applies_to_owner(r)
            && r.enabled
            && r.from_engine_id == from_engine_id
            && r.group_id.is_none()
            && !r.default_group_only
    })
}

fn refit_target_cargo(
    engine: &EngineDef,
    vehicle: &Vehicle,
    cargo_catalog: &[crate::cargo_spec::CargoSpecDef],
    climate: crate::Climate,
) -> Option<crate::CargoType> {
    if let Some(cargo) = engine.cargo {
        return Some(cargo);
    }
    vehicle.cargo_type.and_then(|current| {
        let options = refittable_cargo_types_for_engine_with_catalog_and_climate(
            engine,
            cargo_catalog,
            climate,
        );
        options
            .contains(&current)
            .then_some(current)
            .or_else(|| options.first().copied())
    })
}

fn refit_cost_for_replacement(state: &GameState, vehicle: &Vehicle, new_engine: &EngineDef) -> i64 {
    let Some(target_cargo) = refit_target_cargo(
        new_engine,
        vehicle,
        &state.cargo_spec_catalog,
        state.climate,
    ) else {
        return 0;
    };
    if vehicle.cargo_type == Some(target_cargo) {
        return 0;
    }
    let mut refit_probe = vehicle.clone();
    refit_probe.engine_id = Some(new_engine.id);
    economy::vehicle_refit_cost_with_callbacks(
        &state.global_economy,
        new_engine,
        &mut refit_probe,
        target_cargo,
        vehicle.cargo_subtype,
        state.climate,
        &state.cargo_spec_catalog,
    )
    .0
}

fn autoreplace_cost(state: &GameState, vehicle: &Vehicle, new_engine: &EngineDef) -> i64 {
    let mut probe = vehicle.clone();
    probe.engine_id = Some(new_engine.id);
    let purchase = economy::vehicle_purchase_cost_with_callbacks(new_engine, &mut probe);
    let mut total =
        purchase - economy::vehicle_sell_refund_with_catalog(vehicle, &state.engine_catalog);
    total = total.saturating_add(refit_cost_for_replacement(state, vehicle, new_engine));

    // `ReplaceChain` reconstruye también las unidades con una regla propia
    // (vagones/unidades articuladas compradas). El coste de refit debe
    // estimarse para cada una, no sólo para la cabeza, porque CB15E puede
    // devolver un factor distinto por motor/cargo/subtipo.
    if matches!(
        vehicle.kind,
        VehicleKind::Train | VehicleKind::Bus | VehicleKind::Truck | VehicleKind::Tram
    ) {
        for unit_id in consist_unit_ids(&state.vehicles, vehicle.id) {
            if unit_id == vehicle.id {
                continue;
            }
            let Some(unit) = state
                .vehicles
                .iter()
                .find(|candidate| candidate.id == unit_id)
            else {
                continue;
            };
            if unit.newgrf_articulated {
                continue;
            }
            let is_dual_head =
                vehicle.other_multiheaded_part == Some(unit_id) && new_engine.is_dual_headed();
            let replacement = if is_dual_head {
                Some(new_engine.clone())
            } else {
                let from = unit
                    .engine_id
                    .unwrap_or(crate::engine::default_engine_id(unit.kind));
                resolve_rule_for_company(&state.autoreplace_rules, unit.owner, from, unit.group_id)
                    .filter(|rule| rule.from_engine_id != rule.to_engine_id)
                    .and_then(|rule| engine_for_state(state, rule.to_engine_id))
            };
            if let Some(replacement) = replacement {
                total = total.saturating_add(refit_cost_for_replacement(state, unit, &replacement));
            }
        }
    }
    total
}

/// Resuelve un motor usando primero el catálogo activo (incluidos ids
/// asignados por `NewGRF`) y luego la tabla vanilla para saves antiguos.
fn engine_for_state(state: &GameState, engine_id: u16) -> Option<EngineDef> {
    crate::engine::engine_in_catalog(&state.engine_catalog, engine_id)
        .cloned()
        .or_else(|| engine_by_id(engine_id).cloned())
}

fn apply_engine_with_refit(
    vehicle: &mut Vehicle,
    new_engine: &EngineDef,
    current_tick: u64,
    cargo_spec_catalog: &[crate::cargo_spec::CargoSpecDef],
    climate: crate::Climate,
) {
    vehicle.engine_id = Some(new_engine.id);
    vehicle.unit_length = crate::newgrf_callback::vehicle_unit_length(new_engine, vehicle);
    if let Some(c) = new_engine.cargo {
        vehicle.cargo_type = Some(c);
    } else if let Some(current) = vehicle.cargo_type {
        let refittable = refittable_cargo_types_for_engine_with_catalog_and_climate(
            new_engine,
            cargo_spec_catalog,
            climate,
        );
        if !refittable.contains(&current)
            && let Some(&first) = refittable.first()
        {
            vehicle.cargo_type = Some(first);
        }
    }
    // `DetermineCapacity` se vuelve a ejecutar al cambiar el motor, no en el
    // siguiente tick de carga. Esto es observable para CB36 dependiente del
    // cargo y evita que autoreplace conserve transitoriamente la capacidad
    // del motor anterior. Las locomotoras sin capacidad propia mantienen el
    // placeholder hasta que `ConsistChanged` suma los vagones.
    let callback_capacity =
        crate::newgrf_callback::resolve_vehicle_capacity_property_callback(new_engine, vehicle);
    let raw_capacity =
        callback_capacity.or((new_engine.capacity > 0).then_some(new_engine.capacity));
    if let Some(raw_capacity) = raw_capacity {
        let cargo = vehicle
            .cargo_type
            .or(new_engine.cargo)
            .unwrap_or(match vehicle.kind {
                VehicleKind::Bus | VehicleKind::Tram | VehicleKind::Aircraft => {
                    crate::CargoType::Passengers
                }
                VehicleKind::Truck | VehicleKind::Ship => crate::CargoType::Goods,
                VehicleKind::Train => crate::CargoType::Passengers,
            });
        vehicle.capacity = crate::cargo_spec::apply_cargo_capacity_multiplier(
            raw_capacity,
            cargo_spec_catalog,
            cargo,
        );
    } else if vehicle.kind != VehicleKind::Train {
        // Un motor sin propiedad de capacidad no puede conservar la carga de
        // la unidad sustituida (p. ej. una pieza articulada no transportable).
        vehicle.capacity = 0;
    }
    if vehicle.cargo_type.is_some() {
        vehicle.refit_capacity = u16::try_from(vehicle.capacity).unwrap_or(u16::MAX);
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

    if let Some(rule) = resolve_rule_for_company(
        &state.autoreplace_rules,
        vehicle.owner,
        from_engine_id,
        vehicle.group_id,
    ) && rule.from_engine_id != rule.to_engine_id
    {
        if rule.only_when_old
            && !vehicle.needs_autorenewing(current_tick, company.engine_renew_months)
        {
            return false;
        }
        if let Some(new_engine) = engine_for_state(state, rule.to_engine_id)
            && new_engine.kind == vehicle.kind
            && engine_available_in_year(&new_engine, calendar_year)
        {
            needed_money =
                needed_money.saturating_add(autoreplace_cost(state, vehicle, &new_engine));
            return needed_money <= company.economy.money;
        }
    }

    if !company.engine_renew
        || !vehicle.needs_autorenewing(current_tick, company.engine_renew_months)
    {
        return false;
    }
    let Some(engine) = engine_for_state(state, from_engine_id) else {
        return false;
    };
    if !engine_available_in_year(&engine, calendar_year) {
        return false;
    }
    needed_money = needed_money.saturating_add(autoreplace_cost(state, vehicle, &engine));
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

    if let Some(rule) = resolve_rule_for_company(
        &state.autoreplace_rules,
        owner,
        from_engine_id,
        vehicle.group_id,
    )
    .copied()
    {
        if rule.only_when_old
            && !vehicle.needs_autorenewing(current_tick, company.engine_renew_months)
        {
            return Ok(false);
        }
        if rule.from_engine_id != rule.to_engine_id {
            let Some(new_engine) = engine_for_state(state, rule.to_engine_id) else {
                return Err(CommandError::EngineNotFound);
            };
            if new_engine.kind != vehicle.kind {
                return Err(CommandError::AutoreplaceNotAllowed);
            }
            if !engine_available_in_year(&new_engine, calendar_year) {
                crate::news::push_autoreplace_failed_news(
                    state,
                    vehicle_id,
                    CommandError::EngineNotFound,
                );
                return Ok(false);
            }
            let wagon_removal = company.renew_keep_length;
            let cost = autoreplace_cost(state, &state.vehicles[vehicle_idx], &new_engine);
            if !can_afford_replacement(company.economy.money, cost, company.engine_renew_money) {
                crate::news::push_autoreplace_failed_news(
                    state,
                    vehicle_id,
                    CommandError::InsufficientFunds,
                );
                return Ok(false);
            }
            replace_chain(state, vehicle_id, &new_engine, wagon_removal, current_tick)?;
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
    let Some(new_engine) = engine_for_state(state, from_engine_id) else {
        return Err(CommandError::EngineNotFound);
    };
    if !engine_available_in_year(&new_engine, calendar_year) {
        crate::news::push_autoreplace_failed_news(state, vehicle_id, CommandError::EngineNotFound);
        return Ok(false);
    }
    let wagon_removal = company.renew_keep_length;
    let vehicle = &state.vehicles[vehicle_idx];
    let cost = autoreplace_cost(state, vehicle, &new_engine);
    if !can_afford_replacement(company.economy.money, cost, company.engine_renew_money) {
        crate::news::push_autoreplace_failed_news(
            state,
            vehicle_id,
            CommandError::InsufficientFunds,
        );
        return Ok(false);
    }
    replace_chain(state, vehicle_id, &new_engine, wagon_removal, current_tick)?;
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
    let vehicle_kind = state.vehicles[head_idx].kind;
    let is_train = vehicle_kind == VehicleKind::Train;
    let is_road = matches!(
        vehicle_kind,
        VehicleKind::Bus | VehicleKind::Truck | VehicleKind::Tram
    );
    let old_total = state.vehicles[head_idx].cached_total_length;
    // Longitud antigua redondeada a teselas (OpenTTD: CeilDiv(..., TILE_SIZE)*TILE_SIZE).
    let old_total_rounded = old_total.div_ceil(16).saturating_mul(16);

    apply_engine_with_refit(
        &mut state.vehicles[head_idx],
        new_engine,
        current_tick,
        &state.cargo_spec_catalog,
        state.climate,
    );

    if !is_train && !is_road {
        return Ok(());
    }

    // Las piezas creadas por el callback pertenecen a la definición anterior
    // y deben retirarse antes de reconstruir la nueva cadena. Los vagones que
    // el jugador compró permanecen enlazados.
    remove_newgrf_articulated_parts(state, head_id);
    if is_train {
        sync_dual_head_after_replace(state, head_id, new_engine, current_tick);
    }

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
        let Some(rule) =
            resolve_rule_for_company(&state.autoreplace_rules, v.owner, from, v.group_id).copied()
        else {
            continue;
        };
        if rule.from_engine_id == rule.to_engine_id {
            continue;
        }
        let Some(eng) = engine_for_state(state, rule.to_engine_id) else {
            continue;
        };
        if let Some(unit) = state.vehicles.iter_mut().find(|v| v.id == uid) {
            apply_engine_with_refit(
                unit,
                &eng,
                current_tick,
                &state.cargo_spec_catalog,
                state.climate,
            );
        }
    }

    if !new_engine.is_dual_headed() {
        crate::command::vehicles::spawn_newgrf_articulated_parts(state, head_id, new_engine);
    }
    if is_train {
        crate::train_consist::consist_changed_with_map_and_catalog_and_cargo_with_freight_multiplier_and_wagon_speed_limits(
            &mut state.vehicles,
            head_id,
            Some(&state.map),
            &state.engine_catalog,
            &state.cargo_spec_catalog,
            state.freight_trains,
            state.construction.wagon_speed_limits,
        );
    }

    if wagon_removal && is_train {
        trim_consist_to_length(state, head_id, old_total_rounded);
    }
    Ok(())
}

/// Retira sólo las unidades que fueron creadas por CB16 en una materialización
/// anterior y vuelve a unir los vagones ordinarios de la cadena.
fn remove_newgrf_articulated_parts(state: &mut GameState, head_id: u32) {
    let ids = consist_unit_ids(&state.vehicles, head_id);
    for id in ids {
        let Some(part) = state.vehicles.iter().find(|v| v.id == id) else {
            continue;
        };
        if !part.newgrf_articulated {
            continue;
        }
        let previous = part.prev_unit;
        let next = part.next_unit;
        if let Some(previous_id) = previous
            && let Some(previous) = state.vehicles.iter_mut().find(|v| v.id == previous_id)
        {
            previous.next_unit = next;
        }
        if let Some(next_id) = next
            && let Some(next) = state.vehicles.iter_mut().find(|v| v.id == next_id)
        {
            next.prev_unit = previous;
        }
        state.vehicles.retain(|v| v.id != id);
    }
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
                apply_engine_with_refit(
                    rear,
                    new_engine,
                    current_tick,
                    &state.cargo_spec_catalog,
                    state.climate,
                );
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
        rear.cargo_type = new_engine.cargo;
        apply_engine_with_refit(
            &mut rear,
            new_engine,
            current_tick,
            &state.cargo_spec_catalog,
            state.climate,
        );
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
                        .and_then(|id| engine_for_state(state, id))
                        .is_some_and(|engine| engine_is_wagon(&engine))
            })
        }) else {
            break;
        };
        let _ = detach_unit(&mut state.vehicles, tail);
        state.vehicles.retain(|v| v.id != tail);
        crate::train_consist::consist_changed_with_map_and_catalog_and_cargo_with_freight_multiplier_and_wagon_speed_limits(
            &mut state.vehicles,
            head_id,
            Some(&state.map),
            &state.engine_catalog,
            &state.cargo_spec_catalog,
            state.freight_trains,
            state.construction.wagon_speed_limits,
        );
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::command::apply_command;
    use crate::map::TileCoord;
    use crate::map::TileKind;
    use crate::newgrf_sprites::{
        Action2VarAdjust, Action2VarEntry, Action2VarOp, Action2VarTerm, TrainSpriteAssign,
        TrainSpriteGraphics,
    };
    use crate::vehicle::{Vehicle, VehicleKind};

    fn one_articulated_part_callback() -> TrainSpriteGraphics {
        let literal = |value: u8| Action2VarTerm {
            variable: 0x1A,
            param: None,
            adjust: Action2VarAdjust {
                and_mask: u32::from(value),
                ..Action2VarAdjust::default()
            },
        };
        let mut gfx = TrainSpriteGraphics::default();
        gfx.assigns.push(TrainSpriteAssign {
            local_id: 0,
            set_id: 2,
        });
        gfx.action2_var.insert(
            2,
            Action2VarEntry {
                first: Action2VarTerm {
                    variable: 0x10,
                    param: None,
                    adjust: Action2VarAdjust {
                        and_mask: u32::from(u8::MAX),
                        ..Action2VarAdjust::default()
                    },
                },
                ops: Vec::new(),
                ranges: vec![(3, 1, 1)],
                default: 4,
            },
        );
        gfx.action2_var.insert(
            3,
            Action2VarEntry {
                first: literal(1),
                ops: Vec::new(),
                ranges: Vec::new(),
                default: 0,
            },
        );
        gfx.action2_var.insert(
            4,
            Action2VarEntry {
                first: literal(0xFF),
                ops: vec![
                    Action2VarOp {
                        operator: 0x0A,
                        rhs: literal(0x80),
                    },
                    Action2VarOp {
                        operator: 0x00,
                        rhs: literal(0x7F),
                    },
                ],
                ranges: Vec::new(),
                default: 0,
            },
        );
        gfx
    }

    fn property_callback(value: u32) -> TrainSpriteGraphics {
        let mut gfx = TrainSpriteGraphics::default();
        gfx.assigns.push(TrainSpriteAssign {
            local_id: 0,
            set_id: 2,
        });
        gfx.action2_var.insert(
            2,
            Action2VarEntry {
                first: Action2VarTerm {
                    variable: 0x1A,
                    param: None,
                    adjust: Action2VarAdjust {
                        and_mask: value,
                        ..Action2VarAdjust::default()
                    },
                },
                ops: Vec::new(),
                ranges: Vec::new(),
                default: 0,
            },
        );
        gfx
    }

    #[test]
    fn group_rule_wins_over_global() {
        let rules = vec![
            AutoReplaceRule {
                from_engine_id: 10,
                to_engine_id: 11,
                owner: Some(CompanyId::PLAYER),
                enabled: true,
                only_when_old: false,
                group_id: None,
                default_group_only: false,
                sav_pool_id: None,
                sav_next_pool_id: None,
            },
            AutoReplaceRule {
                from_engine_id: 10,
                to_engine_id: 12,
                owner: Some(CompanyId::PLAYER),
                enabled: true,
                only_when_old: false,
                group_id: Some(1),
                default_group_only: false,
                sav_pool_id: None,
                sav_next_pool_id: None,
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
    fn company_and_default_group_scopes_are_not_global() {
        let mut all_groups = AutoReplaceRule::new_for_company(10, 11, CompanyId::PLAYER);
        all_groups.sav_pool_id = Some(0);
        let mut ungrouped = AutoReplaceRule::new_for_company(10, 12, CompanyId::PLAYER);
        ungrouped.default_group_only = true;
        ungrouped.sav_pool_id = Some(1);
        let other_company = AutoReplaceRule::new_for_company(10, 13, CompanyId(1));
        let rules = vec![all_groups, ungrouped, other_company];

        assert_eq!(
            resolve_rule_for_company(&rules, CompanyId::PLAYER, 10, None)
                .map(|rule| rule.to_engine_id),
            Some(12)
        );
        assert_eq!(
            resolve_rule_for_company(&rules, CompanyId::PLAYER, 10, Some(7))
                .map(|rule| rule.to_engine_id),
            Some(11)
        );
        assert_eq!(
            resolve_rule_for_company(&rules, CompanyId(1), 10, None).map(|rule| rule.to_engine_id),
            Some(13)
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

    #[test]
    fn autoreplace_applies_newgrf_capacity_before_leaving_depot() {
        use crate::engine::{ENGINE_BUS_MPS, engine_by_id};

        let mut state = GameState::new(8, 8);
        let depot = TileCoord::new(2, 2);
        state.map.set_kind(depot, TileKind::RoadDepot).unwrap();
        state.companies[0].engine_renew_money = 0;
        state.companies[0].economy.money = 5_000_000;
        state.economy.money = 5_000_000;

        let mut old_engine = engine_by_id(ENGINE_BUS_MPS).unwrap().clone();
        old_engine.id = 1_401;
        old_engine.from_newgrf = true;
        old_engine.newgrf_grfid = 0x4F4C_4443;
        old_engine.newgrf_local_id = 0;
        old_engine.capacity = 12;
        let mut new_engine = old_engine.clone();
        new_engine.id = 1_402;
        new_engine.newgrf_grfid = 0x4E45_5743;
        new_engine.newgrf_runtime = Some(Box::new(property_callback(77)));
        state.engine_catalog.extend([old_engine, new_engine]);

        let mut vehicle = Vehicle::new(1, VehicleKind::Bus, depot, depot);
        vehicle.engine_id = Some(1_401);
        vehicle.capacity = 12;
        vehicle.cargo_type = Some(crate::CargoType::Passengers);
        state.vehicles.push(vehicle);
        state
            .autoreplace_rules
            .push(AutoReplaceRule::new(1_401, 1_402));

        assert!(try_autoreplace_vehicle(&mut state, 1).unwrap());
        let replaced = state.vehicles.iter().find(|v| v.id == 1).unwrap();
        assert_eq!(replaced.engine_id, Some(1_402));
        assert_eq!(replaced.capacity, 77);
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn autoreplace_cost_includes_refit_cost_for_changed_default_cargo() {
        let depot = TileCoord::new(2, 2);
        let mut state = GameState::new(8, 8);
        let mut old_engine = crate::engine::engine_by_id(crate::engine::ENGINE_TRUCK_MPS)
            .unwrap()
            .clone();
        old_engine.id = 1_410;
        let mut new_engine = old_engine.clone();
        new_engine.id = 1_411;
        new_engine.cargo = Some(crate::CargoType::Coal);
        new_engine.refit_cost = 4;
        state.engine_catalog.push(old_engine);

        let mut vehicle = Vehicle::new(1, VehicleKind::Truck, depot, depot);
        vehicle.engine_id = Some(1_410);
        vehicle.cargo_type = Some(crate::CargoType::Mail);
        let with_refit = autoreplace_cost(&state, &vehicle, &new_engine);

        new_engine.refit_cost = 0;
        let without_refit = autoreplace_cost(&state, &vehicle, &new_engine);
        let expected_refit = crate::economy::pricebase::get_price(
            &state.global_economy,
            crate::economy::pricebase::PriceIndex::BuildVehicleRoad,
            4,
            -10,
        );
        assert_eq!(with_refit - without_refit, expected_refit);
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn autoreplace_cost_includes_refit_cost_for_each_replaced_unit() {
        let depot = TileCoord::new(2, 2);
        let mut state = GameState::new(8, 8);
        let mut old_engine = crate::engine::engine_by_id(crate::engine::ENGINE_TRUCK_MPS)
            .unwrap()
            .clone();
        old_engine.id = 1_420;
        let mut new_engine = old_engine.clone();
        new_engine.id = 1_421;
        new_engine.cargo = Some(crate::CargoType::Coal);
        new_engine.refit_cost = 4;
        state
            .engine_catalog
            .extend([old_engine, new_engine.clone()]);
        state
            .autoreplace_rules
            .push(AutoReplaceRule::new(1_420, 1_421));

        let mut head = Vehicle::new(1, VehicleKind::Truck, depot, depot);
        head.engine_id = Some(1_420);
        head.cargo_type = Some(crate::CargoType::Mail);
        head.next_unit = Some(2);
        let mut tail = Vehicle::new(2, VehicleKind::Truck, depot, depot);
        tail.engine_id = Some(1_420);
        tail.cargo_type = Some(crate::CargoType::Mail);
        tail.prev_unit = Some(1);
        state.vehicles.extend([head, tail]);

        let with_refit = autoreplace_cost(&state, &state.vehicles[0], &new_engine);
        if let Some(engine) = state
            .engine_catalog
            .iter_mut()
            .find(|engine| engine.id == 1_421)
        {
            engine.refit_cost = 0;
        }
        new_engine.refit_cost = 0;
        let without_refit = autoreplace_cost(&state, &state.vehicles[0], &new_engine);
        let expected_refit = crate::economy::pricebase::get_price(
            &state.global_economy,
            crate::economy::pricebase::PriceIndex::BuildVehicleRoad,
            4,
            -10,
        );
        assert_eq!(with_refit - without_refit, expected_refit * 2);
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn autoreplace_uses_active_newgrf_catalog_and_rebuilds_articulated_chain() {
        use crate::engine::{ENGINE_TRAIN_KIRBY, ENGINE_WAGON_PASSENGER, EngineDef};
        use crate::newgrf_config::NewGrfEntry;
        use crate::train_consist::consist_unit_ids;

        let mut state = GameState::new(8, 8);
        let depot = TileCoord::new(2, 2);
        state.map.set_kind(depot, TileKind::RailDepot).unwrap();
        state.companies[0].engine_renew_money = 0;
        state.companies[0].economy.money = 5_000_000;
        state.economy.money = 5_000_000;

        let make_front = |id: u16, grfid: u32, name: &str| {
            let mut engine: EngineDef = crate::engine::engine_by_id(ENGINE_TRAIN_KIRBY)
                .unwrap()
                .clone();
            engine.id = id;
            engine.name = name.into();
            engine.price = 1;
            engine.intro_year = 1900;
            engine.from_newgrf = true;
            engine.newgrf_local_id = 0;
            engine.newgrf_grfid = grfid;
            engine.vehicle_callback_mask = 1 << 4;
            engine.newgrf_runtime = Some(Box::new(one_articulated_part_callback()));
            engine
        };
        let make_part = |id: u16, grfid: u32, name: &str| {
            let mut engine: EngineDef = crate::engine::engine_by_id(ENGINE_WAGON_PASSENGER)
                .unwrap()
                .clone();
            engine.id = id;
            engine.name = name.into();
            engine.price = 0;
            engine.intro_year = 1900;
            engine.from_newgrf = true;
            engine.newgrf_local_id = 1;
            engine.newgrf_grfid = grfid;
            engine
        };
        let old_grfid = 0x4F4C_4441;
        let new_grfid = 0x4E45_5742;
        let old_front_id = 1_200;
        let old_part_id = 1_201;
        let new_front_id = 1_210;
        let new_part_id = 1_211;
        state.engine_catalog.extend([
            make_front(old_front_id, old_grfid, "Old articulated front"),
            make_part(old_part_id, old_grfid, "Old articulated module"),
            make_front(new_front_id, new_grfid, "New articulated front"),
            make_part(new_part_id, new_grfid, "New articulated module"),
        ]);
        for (grfid, filename) in [(old_grfid, "old.grf"), (new_grfid, "new.grf")] {
            state.newgrf_stack.push(NewGrfEntry {
                filename: filename.into(),
                grfid,
                name: filename.into(),
                description: String::new(),
                grf_version: 8,
                enabled: true,
                is_static: false,
                params: Vec::new(),
            });
        }

        apply_command(
            &mut state,
            &crate::command::Command::BuildVehicleAtDepot(depot, old_front_id),
        )
        .unwrap();
        assert_eq!(
            state.vehicles.len(),
            2,
            "el front materializa su parte vieja"
        );
        // El callback devuelve la pieza, por lo que la unidad creada usa el id
        // del catálogo NewGRF, no la tabla vanilla.
        assert!(state.engine_catalog.iter().any(|e| e.id == old_part_id));
        assert_eq!(state.vehicles[0].engine_id, Some(old_front_id));
        let generated_old = state
            .vehicles
            .iter()
            .find(|vehicle| vehicle.newgrf_articulated)
            .map(|vehicle| vehicle.id)
            .unwrap();

        // Compra un wagon vanilla y engánchalo detrás de la pieza automática.
        apply_command(
            &mut state,
            &crate::command::Command::BuildVehicleAtDepot(depot, ENGINE_WAGON_PASSENGER),
        )
        .unwrap();
        let wagon_id = state
            .vehicles
            .iter()
            .find(|vehicle| vehicle.id != 1 && vehicle.id != generated_old)
            .map(|vehicle| vehicle.id)
            .unwrap();
        crate::command::apply_command(
            &mut state,
            &crate::command::Command::AttachWagonToConsist {
                head_id: 1,
                wagon_id,
            },
        )
        .unwrap();
        state
            .autoreplace_rules
            .push(AutoReplaceRule::new(old_front_id, new_front_id));
        assert!(try_autoreplace_vehicle(&mut state, 1).unwrap());

        let ids = consist_unit_ids(&state.vehicles, 1);
        assert_eq!(ids.len(), 3, "front + parte nueva + wagon del usuario");
        assert_eq!(state.vehicles[0].engine_id, Some(new_front_id));
        let generated_new = state
            .vehicles
            .iter()
            .find(|vehicle| vehicle.newgrf_articulated)
            .unwrap();
        assert_eq!(generated_new.engine_id, Some(new_part_id));
        assert!(
            !state
                .vehicles
                .iter()
                .any(|vehicle| vehicle.id == generated_old)
        );
        assert!(state.vehicles.iter().any(|vehicle| {
            vehicle.id == wagon_id && !vehicle.newgrf_articulated && vehicle.prev_unit.is_some()
        }));
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn autoreplace_rebuilds_active_newgrf_road_articulated_chain() {
        use crate::engine::{ENGINE_BUS_FOSTER, ENGINE_BUS_MPS, EngineDef};
        use crate::newgrf_config::NewGrfEntry;
        use crate::train_consist::consist_unit_ids;

        let mut state = GameState::new(8, 8);
        let depot = TileCoord::new(2, 2);
        state.map.set_kind(depot, TileKind::RoadDepot).unwrap();
        state.companies[0].engine_renew_money = 0;
        state.companies[0].economy.money = 5_000_000;
        state.economy.money = 5_000_000;

        let make_front = |id: u16, grfid: u32, name: &str| {
            let mut engine: EngineDef =
                crate::engine::engine_by_id(ENGINE_BUS_MPS).unwrap().clone();
            engine.id = id;
            engine.name = name.into();
            engine.price = 1;
            engine.intro_year = 1900;
            engine.from_newgrf = true;
            engine.newgrf_local_id = 0;
            engine.newgrf_grfid = grfid;
            engine.vehicle_callback_mask = 1 << 4;
            engine.newgrf_runtime = Some(Box::new(one_articulated_part_callback()));
            engine
        };
        let make_part = |id: u16, grfid: u32, name: &str| {
            let mut engine: EngineDef = crate::engine::engine_by_id(ENGINE_BUS_FOSTER)
                .unwrap()
                .clone();
            engine.id = id;
            engine.name = name.into();
            engine.price = 0;
            engine.intro_year = 1900;
            engine.from_newgrf = true;
            engine.newgrf_local_id = 1;
            engine.newgrf_grfid = grfid;
            engine
        };
        let old_grfid = 0x4F4C_5241;
        let new_grfid = 0x4E45_5252;
        let old_front_id = 1_300;
        let old_part_id = 1_301;
        let new_front_id = 1_310;
        let new_part_id = 1_311;
        state.engine_catalog.extend([
            make_front(old_front_id, old_grfid, "Old articulated bus"),
            make_part(old_part_id, old_grfid, "Old bus module"),
            make_front(new_front_id, new_grfid, "New articulated bus"),
            make_part(new_part_id, new_grfid, "New bus module"),
        ]);
        for (grfid, filename) in [(old_grfid, "old-road.grf"), (new_grfid, "new-road.grf")] {
            state.newgrf_stack.push(NewGrfEntry {
                filename: filename.into(),
                grfid,
                name: filename.into(),
                description: String::new(),
                grf_version: 8,
                enabled: true,
                is_static: false,
                params: Vec::new(),
            });
        }

        apply_command(
            &mut state,
            &crate::command::Command::BuildVehicleAtDepot(depot, old_front_id),
        )
        .unwrap();
        assert_eq!(state.vehicles.len(), 2);
        let generated_old = state
            .vehicles
            .iter()
            .find(|vehicle| vehicle.newgrf_articulated)
            .map(|vehicle| vehicle.id)
            .unwrap();
        state
            .autoreplace_rules
            .push(AutoReplaceRule::new(old_front_id, new_front_id));

        assert!(try_autoreplace_vehicle(&mut state, 1).unwrap());

        assert_eq!(consist_unit_ids(&state.vehicles, 1).len(), 2);
        assert!(
            !state
                .vehicles
                .iter()
                .any(|vehicle| vehicle.engine_id == Some(old_part_id)),
            "old generated id={generated_old}; vehicles={:?}",
            state
                .vehicles
                .iter()
                .map(|vehicle| {
                    (
                        vehicle.id,
                        vehicle.engine_id,
                        vehicle.newgrf_articulated,
                        vehicle.prev_unit,
                        vehicle.next_unit,
                    )
                })
                .collect::<Vec<_>>()
        );
        let head = state
            .vehicles
            .iter()
            .find(|vehicle| vehicle.id == 1)
            .unwrap();
        assert_eq!(head.kind, VehicleKind::Bus);
        assert_eq!(head.engine_id, Some(new_front_id));
        let generated_new = state
            .vehicles
            .iter()
            .find(|vehicle| vehicle.newgrf_articulated)
            .unwrap();
        assert_eq!(generated_new.kind, VehicleKind::Bus);
        assert_eq!(generated_new.engine_id, Some(new_part_id));
        assert_eq!(generated_new.prev_unit, Some(head.id));
        assert_eq!(
            generated_new.road_depot_phase,
            crate::vehicle::RoadDepotPhase::InDepot
        );
        assert_eq!(
            generated_new.road_state,
            crate::road_movement::RVSB_IN_DEPOT
        );
    }
}
