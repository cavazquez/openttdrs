//! IA rival «TransCargo»: hasta 2 rutas de freight (carbón / madera), vía Manhattan.
//!
//! Sin terraform ni señales. Ver `docs/epics/ai_rivals.md`.

use crate::GameState;
use crate::cargo::CargoType;
use crate::command::{Command, apply_command};
use crate::company::CompanyId;
use crate::economy::TICKS_PER_MONTH;
use crate::engine::ENGINE_TRAIN_KIRBY;
use crate::industry::IndustryKind;
use crate::map::{TileCoord, TileKind};
use crate::pathfinder::{PathNetwork, find_path};
use crate::subsidy::{SUBSIDY_OFFER_MONTHS, Subsidy};
use crate::vehicle::{VehicleKind, VehicleOrder};

use super::CompanyAi;

/// Alias histórico (= [`super::DEFAULT_AI_MAX_ROUTES`]).
#[allow(clippy::cast_lossless)]
pub const MAX_AI_ROUTES: usize = super::DEFAULT_AI_MAX_ROUTES as usize;
/// Alias histórico (= [`super::DEFAULT_AI_BUILD_MONEY_THRESHOLD`]).
pub const AI_BUILD_MONEY_THRESHOLD: i64 = super::DEFAULT_AI_BUILD_MONEY_THRESHOLD;

/// Rival estático documentado en `docs/epics/ai_rivals.md`.
#[derive(Debug, Default, Clone, Copy)]
pub struct TransCargoAi;

impl CompanyAi for TransCargoAi {
    fn decide(&self, state: &GameState) -> Vec<Command> {
        let _ = state;
        Vec::new()
    }
}

#[derive(Debug, Clone, Copy)]
struct RoutePlan {
    source: TileCoord,
    dest: TileCoord,
    cargo: CargoType,
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

/// `sync_cargo_from_packets` pone `cargo_type = None` al vaciar; reanclar al
/// cargo de la industria junto a la primera orden de estación.
fn refresh_ai_train_cargo_types(state: &mut GameState, ai_id: CompanyId) {
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

fn ai_route_count(state: &GameState, ai_id: CompanyId) -> usize {
    state
        .vehicles
        .iter()
        .filter(|v| v.owner == ai_id && v.kind == VehicleKind::Train && v.is_consist_head())
        .count()
}

fn industry_served_by_ai(state: &GameState, ai_id: CompanyId, industry_pos: TileCoord) -> bool {
    // Solo estaciones «de carga» junto a la industria (±2), no toda la cobertura
    // de radio 4 (una estación de carbón no debe marcar el bosque como servido).
    state.stations.iter().any(|st| {
        st.owner == ai_id
            && (st.pos.x - industry_pos.x).abs() <= 2
            && (st.pos.y - industry_pos.y).abs() <= 2
    })
}

fn next_unserved_plan(state: &GameState, ai_id: CompanyId) -> Option<RoutePlan> {
    let factory = state
        .industries
        .iter()
        .find(|i| i.kind == IndustryKind::Factory)
        .map(|i| i.pos)?;

    // Prioridad: carbón, luego madera.
    let candidates = [
        (IndustryKind::CoalMine, CargoType::Coal),
        (IndustryKind::Forest, CargoType::Wood),
    ];
    for (kind, cargo) in candidates {
        let Some(source) = state
            .industries
            .iter()
            .find(|i| i.kind == kind)
            .map(|i| i.pos)
        else {
            continue;
        };
        if industry_served_by_ai(state, ai_id, source) {
            continue;
        }
        return Some(RoutePlan {
            source,
            dest: factory,
            cargo,
        });
    }
    None
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

fn tile_buildable_for_station(map: &crate::map::Map, c: TileCoord) -> bool {
    matches!(
        map.get_kind(c),
        Some(TileKind::Grass | TileKind::Forest | TileKind::Rail)
    )
}

/// Offsets candidatos a ±2 (cardinales) respecto de la industria.
fn station_candidates_near(industry: TileCoord) -> [TileCoord; 4] {
    [
        TileCoord::new(industry.x + 2, industry.y),
        TileCoord::new(industry.x - 2, industry.y),
        TileCoord::new(industry.x, industry.y + 2),
        TileCoord::new(industry.x, industry.y - 2),
    ]
}

fn pick_station_tile(
    map: &crate::map::Map,
    industry: TileCoord,
    toward: TileCoord,
) -> Option<TileCoord> {
    let mut cands: Vec<TileCoord> = station_candidates_near(industry)
        .into_iter()
        .filter(|&c| map.get(c).is_some() && tile_buildable_for_station(map, c))
        .collect();
    // Preferir la tesela más cercana al otro extremo (corredor corto).
    cands.sort_by_key(|c| (c.x - toward.x).abs() + (c.y - toward.y).abs());
    cands.into_iter().next()
}

/// Coloca vía en corredor Manhattan: primero eje X, luego eje Y.
fn place_rail_manhattan_corridor(state: &mut GameState, from: TileCoord, to: TileCoord) {
    let mut c = from;
    let step_x = (to.x - from.x).signum();
    while c.x != to.x {
        c = TileCoord::new(c.x + step_x, c.y);
        place_rail_if_needed(state, c);
    }
    let step_y = (to.y - from.y).signum();
    while c.y != to.y {
        c = TileCoord::new(c.x, c.y + step_y);
        place_rail_if_needed(state, c);
    }
}

fn place_rail_if_needed(state: &mut GameState, c: TileCoord) {
    if matches!(
        state.map.get_kind(c),
        Some(TileKind::Rail | TileKind::Station | TileKind::RailDepot)
    ) {
        return;
    }
    let _ = apply_command(state, &Command::PlaceRail(c));
}

fn place_rail_station_owned(state: &mut GameState, st_pos: TileCoord, ai_id: CompanyId) -> bool {
    if state.stations.iter().any(|s| s.pos == st_pos) {
        if let Some(st) = state.stations.iter_mut().find(|s| s.pos == st_pos) {
            st.owner = ai_id;
        }
        return true;
    }
    if state.map.get_kind(st_pos) == Some(TileKind::Rail) {
        let _ = apply_command(state, &Command::ClearTile(st_pos));
    }
    for dir in 0..4u8 {
        if apply_command(state, &Command::PlaceRailStation(st_pos, dir)).is_ok() {
            if let Some(st) = state.stations.iter_mut().find(|s| s.pos == st_pos) {
                st.owner = ai_id;
            }
            return true;
        }
    }
    false
}

fn try_place_depot_near(state: &mut GameState, load_st: TileCoord) -> Option<TileCoord> {
    // Preferido: boca al norte hacia la línea, depósito al sur de la tesela
    // al este de la estación (layout que ya funciona en `ai_rival_line`).
    let preferred = [
        (
            TileCoord::new(load_st.x + 1, load_st.y + 1),
            TileCoord::new(load_st.x + 1, load_st.y),
            3u8,
        ),
        (
            TileCoord::new(load_st.x - 1, load_st.y + 1),
            TileCoord::new(load_st.x - 1, load_st.y),
            3u8,
        ),
        (
            TileCoord::new(load_st.x + 1, load_st.y - 1),
            TileCoord::new(load_st.x + 1, load_st.y),
            1u8,
        ),
        (
            TileCoord::new(load_st.x - 1, load_st.y - 1),
            TileCoord::new(load_st.x - 1, load_st.y),
            1u8,
        ),
        (TileCoord::new(load_st.x, load_st.y + 1), load_st, 3u8),
        (TileCoord::new(load_st.x, load_st.y - 1), load_st, 1u8),
    ];
    for (depot, mouth, dir) in preferred {
        if state.map.get(depot).is_none() {
            continue;
        }
        if state.map.get_kind(depot) == Some(TileKind::RailDepot) {
            return Some(depot);
        }
        if mouth != load_st && mouth != depot {
            place_rail_if_needed(state, mouth);
        }
        if apply_command(state, &Command::PlaceRailDepotDir(depot, dir)).is_ok() {
            return Some(depot);
        }
    }
    None
}

fn build_freight_line(
    state: &mut GameState,
    ai_id: CompanyId,
    plan: RoutePlan,
) -> Option<(TileCoord, TileCoord, TileCoord)> {
    let load_st = pick_station_tile(&state.map, plan.source, plan.dest)?;
    let unload_st = pick_station_tile(&state.map, plan.dest, plan.source)?;
    if load_st == unload_st {
        return None;
    }

    let mut depot_out = None;
    with_ai_active(state, ai_id, |state| {
        place_rail_manhattan_corridor(state, load_st, unload_st);
        if !place_rail_station_owned(state, load_st, ai_id) {
            return;
        }
        if !place_rail_station_owned(state, unload_st, ai_id) {
            return;
        }
        // Reconectar vía bajo/alrededor de las estaciones.
        place_rail_manhattan_corridor(state, load_st, unload_st);
        depot_out = try_place_depot_near(state, load_st);
    });

    let depot = depot_out?;
    let has_load = state
        .stations
        .iter()
        .any(|s| s.owner == ai_id && s.pos == load_st);
    let has_unload = state
        .stations
        .iter()
        .any(|s| s.owner == ai_id && s.pos == unload_st);
    if has_load && has_unload {
        Some((load_st, unload_st, depot))
    } else {
        None
    }
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

fn buy_and_order_train(
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
            if let Some(path) = find_path(&state.map, v.pos, load_st, PathNetwork::Rail) {
                v.path = path.into_iter().collect();
                v.dest = load_st;
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

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::map::TileCoord;

    #[test]
    fn manhattan_corridor_places_l_shape() {
        let mut state = GameState::new(16, 16);
        state.economy.money = 500_000;
        let a = TileCoord::new(2, 2);
        let b = TileCoord::new(8, 6);
        place_rail_manhattan_corridor(&mut state, a, b);
        // Esquina del L en (8,2).
        assert_eq!(
            state.map.get_kind(TileCoord::new(8, 2)),
            Some(TileKind::Rail)
        );
        assert_eq!(
            state.map.get_kind(TileCoord::new(8, 6)),
            Some(TileKind::Rail)
        );
        assert_eq!(
            state.map.get_kind(TileCoord::new(5, 2)),
            Some(TileKind::Rail)
        );
    }
}
