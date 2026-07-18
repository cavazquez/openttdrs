//! IA rival «RoadHaul»: líneas de buses entre pueblos.
//!
//! Carretera Manhattan + `PlaceBusStop`; sin Squirrel. Complementa `TransCargo`.
//! Varias rutas (#191); par por score población/distancia (#192).

mod build;
mod fleet;
mod plan;

use crate::GameState;
use crate::company::{RIVAL_NAME_ROADHAUL, company_id_by_name};
use crate::map::{TileCoord, TileKind};
use crate::vehicle::VehicleKind;

use build::plan_bus_line_queue;
use fleet::buy_and_order_bus;
use plan::{next_bus_plan, roadhaul_route_count};

use super::build_queue::{company_has_build_queue, enqueue_build_queue, road_endpoints_connected};

/// Máximo de líneas bus (#191).
pub const ROADHAUL_MAX_ROUTES: usize = 3;

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

/// Encola una línea bus (no aplica en este tick).
pub fn tick_roadhaul(state: &mut GameState) {
    maintain_roadhaul_vehicles(state);
    let Some(ai_id) = company_id_by_name(&state.companies, RIVAL_NAME_ROADHAUL) else {
        return;
    };
    if company_has_build_queue(state, ai_id) {
        return;
    }

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
    let Some(queue) = plan_bus_line_queue(state, ai_id, plan) else {
        return;
    };
    enqueue_build_queue(state, queue);
}

/// Cierre de obra bus: comprar solo si paradas + path + depósito cerraron.
pub(crate) fn complete_bus_build(
    state: &mut GameState,
    ai_id: crate::company::CompanyId,
    stop_a: TileCoord,
    stop_b: TileCoord,
    depot: TileCoord,
) {
    let has_a = state
        .stations
        .iter()
        .any(|s| s.owner == ai_id && s.pos == stop_a);
    let has_b = state
        .stations
        .iter()
        .any(|s| s.owner == ai_id && s.pos == stop_b);
    if !has_a || !has_b {
        return;
    }
    if state.map.get_kind(depot) != Some(TileKind::RoadDepot) {
        return;
    }
    if !road_endpoints_connected(state, stop_a, stop_b) {
        return;
    }
    let _ = buy_and_order_bus(state, ai_id, stop_a, stop_b, depot);
}
