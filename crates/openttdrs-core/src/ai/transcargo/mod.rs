//! IA rival «TransCargo»: hasta N rutas de freight (carbón / madera / petróleo).
//!
//! Vía Manhattan en L, nivelado del corredor y señales de bloque bidireccionales.
//! Ver `docs/archive/epics/ai_rivals.md` (épica cerrada).

pub(crate) mod build;
mod fleet;
pub(crate) mod plan;

use crate::GameState;
use crate::command::Command;
use crate::company::{RIVAL_NAME_TRANSCARGO, company_id_by_name};
use crate::map::{TileCoord, TileKind};
use crate::vehicle::VehicleKind;

use build::{build_freight_line, plan_freight_line_queue, repair_freight_corridor};
use fleet::{buy_and_order_train, refresh_ai_train_cargo_types};
use plan::{RoutePlan, ai_route_count, next_unserved_plan};

use super::build_queue::{company_has_build_queue, enqueue_build_queue, rail_endpoints_connected};

/// Alias histórico (= [`super::DEFAULT_AI_MAX_ROUTES`]).
#[allow(clippy::cast_lossless)]
pub const MAX_AI_ROUTES: usize = super::DEFAULT_AI_MAX_ROUTES as usize;
/// Alias histórico (= [`super::DEFAULT_AI_BUILD_MONEY_THRESHOLD`]).
pub const AI_BUILD_MONEY_THRESHOLD: i64 = super::DEFAULT_AI_BUILD_MONEY_THRESHOLD;

/// Rival estático documentado en `docs/archive/epics/ai_rivals.md`.
#[derive(Debug, Default, Clone, Copy)]
pub struct TransCargoAi;

impl super::CompanyAi for TransCargoAi {
    fn decide(&self, state: &GameState) -> Vec<Command> {
        let _ = state;
        Vec::new()
    }
}

/// Mantiene trenes IA en marcha y `cargo_type` anclado (cada tick).
pub fn maintain_transcargo_vehicles(state: &mut GameState) {
    state.ensure_rival_transcargo();
    let Some(ai_id) = company_id_by_name(&state.companies, RIVAL_NAME_TRANSCARGO) else {
        return;
    };
    refresh_ai_train_cargo_types(state, ai_id);
    for v in &mut state.vehicles {
        if v.owner == ai_id && v.kind == VehicleKind::Train && v.is_consist_head() {
            v.running = true;
        }
    }
}

/// Encola una línea freight (no aplica en este tick).
pub fn tick_transcargo(state: &mut GameState) {
    maintain_transcargo_vehicles(state);
    let Some(ai_id) = company_id_by_name(&state.companies, RIVAL_NAME_TRANSCARGO) else {
        return;
    };
    if company_has_build_queue(state, ai_id) {
        return;
    }

    let ai = state.ai.clamped();
    let routes = ai_route_count(state, ai_id);
    if routes >= ai.max_routes_usize() {
        return;
    }
    let money = state.company_economy(ai_id).money;
    if money < ai.build_money_threshold {
        return;
    }
    let Some(plan) = next_unserved_plan(state, ai_id) else {
        return;
    };
    if let Some(queue) = plan_freight_line_queue(state, ai_id, plan) {
        enqueue_build_queue(state, queue);
        return;
    }
    // Fallback: mapa ya congestionado (p. ej. 2ª/3ª ruta) — construir one-shot.
    if let Some((load_st, unload_st, depot)) = build_freight_line(state, ai_id, plan) {
        let _ = buy_and_order_train(state, ai_id, load_st, unload_st, depot, plan);
    }
}

/// Cierre de obra: comprar tren solo si estaciones + path + depósito cerraron.
pub(crate) fn complete_freight_build(
    state: &mut GameState,
    ai_id: crate::company::CompanyId,
    load_st: TileCoord,
    unload_st: TileCoord,
    depot: TileCoord,
    plan: RoutePlan,
) {
    let has_load = state
        .stations
        .iter()
        .any(|s| s.owner == ai_id && s.pos == load_st);
    let has_unload = state
        .stations
        .iter()
        .any(|s| s.owner == ai_id && s.pos == unload_st);
    if !has_load || !has_unload {
        return;
    }
    if state.map.get_kind(depot) != Some(TileKind::RailDepot) {
        return;
    }
    // Cura huecos del replay progresivo antes de comprar.
    if !rail_endpoints_connected(state, load_st, unload_st)
        && !repair_freight_corridor(state, ai_id, load_st, unload_st)
    {
        return;
    }
    if !rail_endpoints_connected(state, load_st, unload_st) {
        return;
    }
    let _ = buy_and_order_train(state, ai_id, load_st, unload_st, depot, plan);
}
