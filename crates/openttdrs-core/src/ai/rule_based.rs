//! IA rival «TransCargo»: construye una línea mina→fábrica y un tren (Fase 4c).
//!
//! Heurística mínima: una sola ruta de carbón, sin terraform ni señales.

use crate::GameState;
use crate::cargo::CargoType;
use crate::command::{Command, apply_command};
use crate::company::CompanyId;
use crate::engine::ENGINE_TRAIN_KIRBY;
use crate::industry::IndustryKind;
use crate::map::{TileCoord, TileKind};
use crate::pathfinder::{PathNetwork, find_path};
use crate::vehicle::VehicleKind;

use super::CompanyAi;

/// Rival estático documentado en `docs/epics/ai_rivals.md`.
#[derive(Debug, Default, Clone, Copy)]
pub struct TransCargoAi;

impl CompanyAi for TransCargoAi {
    fn decide(&self, state: &GameState) -> Vec<Command> {
        let _ = state;
        Vec::new()
    }
}

/// Ejecuta un paso de construcción/compra para `TransCargo` si hace falta.
pub fn tick_transcargo(state: &mut GameState) {
    state.ensure_rival_transcargo();
    let Some(ai_id) = state.companies.iter().find(|c| c.is_ai).map(|c| c.id) else {
        return;
    };
    if state.vehicles.iter().any(|v| v.owner == ai_id) {
        for v in &mut state.vehicles {
            if v.owner == ai_id && v.kind == VehicleKind::Train {
                v.running = true;
            }
        }
        return;
    }
    if !build_coal_line(state, ai_id) {
        return;
    }
    let _ = buy_and_order_train(state, ai_id);
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

fn build_coal_line(state: &mut GameState, ai_id: CompanyId) -> bool {
    let Some(mine) = state
        .industries
        .iter()
        .find(|i| i.kind == IndustryKind::CoalMine)
        .map(|i| i.pos)
    else {
        return false;
    };
    let Some(factory) = state
        .industries
        .iter()
        .find(|i| i.kind == IndustryKind::Factory)
        .map(|i| i.pos)
    else {
        return false;
    };

    if mine.y != factory.y {
        return false;
    }
    let y = mine.y;
    // Estaciones en hierba a ±2 de las industrias; vía continua entre ellas.
    let load_st = TileCoord::new(mine.x + 2, y);
    let unload_st = TileCoord::new(factory.x - 2, y);
    if load_st.x >= unload_st.x {
        return false;
    }
    if state.map.get(load_st).is_none() || state.map.get(unload_st).is_none() {
        return false;
    }

    with_ai_active(state, ai_id, |state| {
        // 1) Vía en todo el corredor (incluye teselas de estación: se pisan al colocar).
        for x in load_st.x..=unload_st.x {
            let c = TileCoord::new(x, y);
            if matches!(
                state.map.get_kind(c),
                Some(TileKind::Rail | TileKind::Station | TileKind::RailDepot)
            ) {
                continue;
            }
            let _ = apply_command(state, &Command::PlaceRail(c));
        }

        // 2) Estaciones: probar dirs 0..3 (entrada debe mirar a la vía).
        for st_pos in [load_st, unload_st] {
            if state.stations.iter().any(|s| s.pos == st_pos) {
                continue;
            }
            if state.map.get_kind(st_pos) == Some(TileKind::Rail) {
                let _ = apply_command(state, &Command::ClearTile(st_pos));
            }
            for dir in 0..4u8 {
                if apply_command(state, &Command::PlaceRailStation(st_pos, dir)).is_ok() {
                    break;
                }
            }
        }
        for st in &mut state.stations {
            if st.pos == load_st || st.pos == unload_st {
                st.owner = ai_id;
            }
        }

        // 3) Depósito al sur de la primera tesela de vía tras la estación de carga.
        let mouth = TileCoord::new(load_st.x + 1, y);
        let depot = TileCoord::new(load_st.x + 1, y + 1);
        if state.map.get(depot).is_some() {
            if state.map.get_kind(mouth) != Some(TileKind::Rail) {
                let _ = apply_command(state, &Command::PlaceRail(mouth));
            }
            if state.map.get_kind(depot) != Some(TileKind::RailDepot) {
                let _ = apply_command(state, &Command::PlaceRailDepotDir(depot, 3));
            }
        }
    });

    let has_load = state
        .stations
        .iter()
        .any(|s| s.owner == ai_id && s.pos == load_st);
    let has_unload = state
        .stations
        .iter()
        .any(|s| s.owner == ai_id && s.pos == unload_st);
    has_load && has_unload
}

fn buy_and_order_train(state: &mut GameState, ai_id: CompanyId) -> bool {
    let ai_stations: Vec<TileCoord> = state
        .stations
        .iter()
        .filter(|s| s.owner == ai_id)
        .map(|s| s.pos)
        .collect();
    let Some(&load_st) = ai_stations.iter().min_by_key(|p| p.x) else {
        return false;
    };
    let Some(&unload_st) = ai_stations.iter().max_by_key(|p| p.x) else {
        return false;
    };
    if load_st == unload_st {
        return false;
    }
    let depot = TileCoord::new(load_st.x + 1, load_st.y + 1);
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
            &Command::SetVehicleOrders(vid, vec![load_st, unload_st]),
        );
        let _ = apply_command(state, &Command::ToggleVehicleRunning(vid));

        if let Some(v) = state.vehicles.iter_mut().find(|v| v.id == vid) {
            v.owner = ai_id;
            v.cargo_type = Some(CargoType::Coal);
            if let Some(path) = find_path(&state.map, v.pos, load_st, PathNetwork::Rail) {
                v.path = path.into_iter().collect();
                v.dest = load_st;
            }
            v.running = true;
        }
        ok = true;
    });
    ok
}
