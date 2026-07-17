//! IA rival «RoadHaul»: una línea de buses entre dos pueblos (MVP).
//!
//! Carretera Manhattan + `PlaceBusStop`; sin Squirrel. Complementa `TransCargo`.

mod build;
mod fleet;
mod plan;

use crate::GameState;
use crate::company::{RIVAL_NAME_ROADHAUL, company_id_by_name};
use crate::vehicle::VehicleKind;

use build::build_bus_line;
use fleet::buy_and_order_bus;
use plan::{next_bus_plan, roadhaul_route_count};

/// Máximo de rutas bus en el MVP (una línea).
pub const ROADHAUL_MAX_ROUTES: usize = 1;

/// Mantiene buses `RoadHaul` en marcha (cada tick).
pub fn maintain_roadhaul_vehicles(state: &mut GameState) {
    state.ensure_rival_roadhaul();
    let Some(ai_id) = company_id_by_name(&state.companies, RIVAL_NAME_ROADHAUL) else {
        return;
    };
    for v in &mut state.vehicles {
        if v.owner == ai_id && v.kind == VehicleKind::Bus {
            v.running = true;
        }
    }
}

/// Construcción/compra mensual de `RoadHaul` (como máximo una ruta).
pub fn tick_roadhaul(state: &mut GameState) {
    maintain_roadhaul_vehicles(state);
    let Some(ai_id) = company_id_by_name(&state.companies, RIVAL_NAME_ROADHAUL) else {
        return;
    };

    if roadhaul_route_count(state, ai_id) >= ROADHAUL_MAX_ROUTES {
        return;
    }
    let ai = state.ai.clamped();
    // Buses son más baratos: umbral al 50 % del de TransCargo.
    let threshold = ai.build_money_threshold / 2;
    if state.company_economy(ai_id).money < threshold {
        return;
    }
    let Some(plan) = next_bus_plan(state, ai_id) else {
        return;
    };
    let Some((stop_a, stop_b, depot)) = build_bus_line(state, ai_id, plan) else {
        return;
    };
    let _ = buy_and_order_bus(state, ai_id, stop_a, stop_b, depot);
}
