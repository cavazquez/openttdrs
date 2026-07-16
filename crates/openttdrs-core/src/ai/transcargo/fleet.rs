//! Gestión de flota `TransCargo`: compra de trenes, órdenes y subsidios.

use crate::GameState;
use crate::cargo::CargoType;
use crate::command::{Command, apply_command};
use crate::company::CompanyId;
use crate::economy::TICKS_PER_MONTH;
use crate::engine::ENGINE_TRAIN_KIRBY;
use crate::map::{TileCoord, TileKind};
use crate::pathfinder::{PathNetwork, find_path};
use crate::subsidy::{SUBSIDY_OFFER_MONTHS, Subsidy};
use crate::vehicle::{VehicleKind, VehicleOrder};

use super::plan::RoutePlan;

/// `sync_cargo_from_packets` pone `cargo_type = None` al vaciar; reanclar al
/// cargo de la industria junto a la primera orden de estación.
pub(super) fn refresh_ai_train_cargo_types(state: &mut GameState, ai_id: CompanyId) {
    let hints: Vec<(u32, TileCoord)> = state
        .vehicles
        .iter()
        .filter(|v| v.owner == ai_id && v.kind == VehicleKind::Train && v.is_consist_head())
        .filter(|v| v.cargo == 0)
        .filter_map(|v| {
            let load = v.orders.iter().find_map(|o| match o {
                VehicleOrder::Station { station, .. } => Some(*station),
                _ => None,
            })?;
            Some((v.id, load))
        })
        .collect();
    for (vid, load_st) in hints {
        let cargo = state.industries.iter().find_map(|ind| {
            if (ind.pos.x - load_st.x).abs() <= 2 && (ind.pos.y - load_st.y).abs() <= 2 {
                Some(ind.output_cargo())
            } else {
                None
            }
        });
        if let (Some(cargo), Some(v)) = (cargo, state.vehicles.iter_mut().find(|v| v.id == vid)) {
            v.cargo_type = Some(cargo);
        }
    }
}

pub(super) fn buy_and_order_train(
    state: &mut GameState,
    ai_id: CompanyId,
    load_st: TileCoord,
    unload_st: TileCoord,
    depot: TileCoord,
    plan: RoutePlan,
) -> bool {
    if state.map.get_kind(depot) != Some(TileKind::RailDepot) {
        return false;
    }

    let mut ok = false;
    with_ai_active(state, ai_id, |state| {
        if apply_command(
            state,
            &Command::BuildVehicleAtDepot(depot, ENGINE_TRAIN_KIRBY),
        )
        .is_err()
        {
            return;
        }

        let Some(vid) = state
            .vehicles
            .iter()
            .filter(|v| v.owner == ai_id)
            .map(|v| v.id)
            .max()
        else {
            return;
        };

        let _ = apply_command(
            state,
            &Command::SetVehicleOrderList(
                vid,
                vec![
                    VehicleOrder::station_with_flags(load_st, true, false),
                    VehicleOrder::station(unload_st),
                ],
            ),
        );
        let _ = apply_command(state, &Command::ToggleVehicleRunning(vid));

        if let Some(v) = state.vehicles.iter_mut().find(|v| v.id == vid) {
            v.owner = ai_id;
            v.cargo_type = Some(plan.cargo);
            // Destino de movimiento: plataforma o vía de acceso (no path a andén lateral).
            let dest = crate::station::rail_station_stop_tile(&state.map, load_st)
                .or_else(|| crate::station::rail_station_approach_tile(&state.map, load_st))
                .unwrap_or(load_st);
            if let Some(path) = find_path(&state.map, v.pos, dest, PathNetwork::Rail) {
                v.path = path.into_iter().collect();
                v.dest = dest;
            }
            v.running = true;
        }
        ok = true;
    });
    if ok {
        seed_route_subsidy(state, plan.cargo, plan.source, unload_st);
    }
    ok
}

fn with_ai_active(state: &mut GameState, ai_id: CompanyId, f: impl FnOnce(&mut GameState)) {
    let prev_active = state.active_company;
    state.active_company = ai_id;
    if let Some(c) = state.companies.get(ai_id.index()) {
        state.economy = c.economy;
        state.company_colour = c.colour;
    }
    f(state);
    state.active_company = prev_active;
    state.sync_mirrors_from_active();
}

fn seed_route_subsidy(
    state: &mut GameState,
    cargo: CargoType,
    source: TileCoord,
    unload_st: TileCoord,
) {
    if state.subsidies.iter().any(|s| {
        s.cargo == cargo && s.source_industry_pos == source && s.dest_station_pos == unload_st
    }) {
        return;
    }
    let tick = state.tick.get();
    let id = state.next_subsidy_id;
    state.next_subsidy_id = state.next_subsidy_id.saturating_add(1);
    state.subsidies.push(Subsidy {
        id,
        cargo,
        source_industry_pos: source,
        dest_station_pos: unload_st,
        offer_expires_tick: tick.saturating_add(u64::from(SUBSIDY_OFFER_MONTHS) * TICKS_PER_MONTH),
        awarded: false,
        award_expires_tick: 0,
        awarded_company: None,
    });
}
