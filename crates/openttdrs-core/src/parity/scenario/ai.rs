//! Escenario AI rival (TransCargo).

use crate::industry::{Industry, IndustryKind};
use crate::map::TileCoord;
use crate::GameState;

pub fn build_ai_rival_line() -> GameState {
    let mut state = GameState::new(24, 14);
    state.world_seed = 0;
    state.disasters_enabled = false;
    state.economy.money = 100_000;
    state.ensure_companies();
    state.ensure_rival_transcargo();
    // Margen para 3 líneas (estaciones + vía + trenes) bajo umbral 80k.
    if let Some(ai) = state.companies.iter_mut().find(|c| c.is_ai) {
        ai.economy.money = 350_000;
    }

    let mine = TileCoord::new(2, 5);
    let factory = TileCoord::new(18, 5);
    let forest = TileCoord::new(2, 9);
    // Separado en X/Y de la fábrica para no compartir tesela de estación (±2).
    let oil = TileCoord::new(14, 11);
    state
        .industries
        .push(Industry::new(mine, IndustryKind::CoalMine));
    state
        .industries
        .push(Industry::new(factory, IndustryKind::Factory));
    state
        .industries
        .push(Industry::new(forest, IndustryKind::Forest));
    state
        .industries
        .push(Industry::new(oil, IndustryKind::OilWell));
    // Stock inicial para que los trenes puedan cargar tras construir.
    for pos in [mine, forest, oil] {
        if let Some(ind) = state.industries.iter_mut().find(|i| i.pos == pos) {
            ind.stock = 200;
        }
    }
    state
}
