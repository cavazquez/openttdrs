//! IA rival «TransCargo»: hasta N rutas de freight (carbón / madera / petróleo).
//!
//! Vía Manhattan en L, nivelado del corredor y señales de bloque bidireccionales.
//! Ver `docs/epics/ai_rivals.md`.

mod build;
mod fleet;
mod plan;

use crate::GameState;
use crate::command::Command;
use crate::vehicle::VehicleKind;

use build::build_freight_line;
use fleet::{buy_and_order_train, refresh_ai_train_cargo_types};
use plan::{ai_route_count, next_unserved_plan};

/// Alias histórico (= [`super::DEFAULT_AI_MAX_ROUTES`]).
#[allow(clippy::cast_lossless)]
pub const MAX_AI_ROUTES: usize = super::DEFAULT_AI_MAX_ROUTES as usize;
/// Alias histórico (= [`super::DEFAULT_AI_BUILD_MONEY_THRESHOLD`]).
pub const AI_BUILD_MONEY_THRESHOLD: i64 = super::DEFAULT_AI_BUILD_MONEY_THRESHOLD;

/// Rival estático documentado en `docs/epics/ai_rivals.md`.
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
    let Some(ai_id) = state.companies.iter().find(|c| c.is_ai).map(|c| c.id) else {
        return;
    };
    refresh_ai_train_cargo_types(state, ai_id);
    for v in &mut state.vehicles {
        if v.owner == ai_id && v.kind == VehicleKind::Train && v.is_consist_head() {
            v.running = true;
        }
    }
}

/// Ejecuta un paso de construcción/compra para `TransCargo` si hace falta.
pub fn tick_transcargo(state: &mut GameState) {
    maintain_transcargo_vehicles(state);
    let Some(ai_id) = state.companies.iter().find(|c| c.is_ai).map(|c| c.id) else {
        return;
    };

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
    let Some((load_st, unload_st, depot)) = build_freight_line(state, ai_id, plan) else {
        return;
    };
    let _ = buy_and_order_train(state, ai_id, load_st, unload_st, depot, plan);
}
