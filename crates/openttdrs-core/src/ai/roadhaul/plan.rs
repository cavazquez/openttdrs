//! Elección de par de pueblos para la línea de buses.

use crate::GameState;
use crate::company::CompanyId;
use crate::map::TileCoord;
use crate::station::StopKind;
use crate::vehicle::{VehicleKind, VehicleOrder};

#[derive(Debug, Clone, Copy)]
pub(super) struct BusPlan {
    pub town_a: TileCoord,
    pub town_b: TileCoord,
}

/// Número de buses `RoadHaul` con al menos dos órdenes de estación.
pub(super) fn roadhaul_route_count(state: &GameState, ai_id: CompanyId) -> usize {
    state
        .vehicles
        .iter()
        .filter(|v| v.owner == ai_id && v.kind == VehicleKind::Bus)
        .filter(|v| {
            v.orders
                .iter()
                .filter(|o| matches!(o, VehicleOrder::Station { .. }))
                .count()
                >= 2
        })
        .count()
}

/// Par de pueblos aún no servidos por paradas bus de esta IA (Manhattan ≥ 6).
///
/// Prefiere el par **más cercano** (ruta corta y corredor más fiable). Antes
/// maximizaba la distancia y `RoadHaul` tendía a unir los pueblos más lejanos.
pub(super) fn next_bus_plan(state: &GameState, ai_id: CompanyId) -> Option<BusPlan> {
    let served: Vec<TileCoord> = state
        .stations
        .iter()
        .filter(|s| s.owner == ai_id && s.stop_kind == StopKind::BusStop)
        .map(|s| s.pos)
        .collect();

    let towns: Vec<(TileCoord, u32)> = state.towns.iter().map(|t| (t.pos, t.population)).collect();
    if towns.len() < 2 {
        return None;
    }

    // (dist, -población conjunta, a, b): menor dist; empate → más población.
    let mut best: Option<(u32, i64, TileCoord, TileCoord)> = None;
    for i in 0..towns.len() {
        for j in (i + 1)..towns.len() {
            let (a, pop_a) = towns[i];
            let (b, pop_b) = towns[j];
            let dist = a.x.abs_diff(b.x) + a.y.abs_diff(b.y);
            if dist < 6 {
                continue;
            }
            let a_served = served
                .iter()
                .any(|s| (s.x - a.x).abs() <= 3 && (s.y - a.y).abs() <= 3);
            let b_served = served
                .iter()
                .any(|s| (s.x - b.x).abs() <= 3 && (s.y - b.y).abs() <= 3);
            if a_served && b_served {
                continue;
            }
            let pop_key = -i64::from(pop_a.saturating_add(pop_b));
            let cand = (dist, pop_key, a, b);
            if best.is_none_or(|cur| cand < cur) {
                best = Some(cand);
            }
        }
    }
    best.map(|(_, _, town_a, town_b)| BusPlan { town_a, town_b })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::company::{RIVAL_NAME_ROADHAUL, company_id_by_name};
    use crate::town::Town;

    #[test]
    fn next_bus_plan_prefers_closest_town_pair() {
        let mut state = GameState::new(48, 32);
        state.ensure_rival_ais();
        let near_a = TileCoord::new(8, 8);
        let near_b = TileCoord::new(14, 10); // dist 8
        let far = TileCoord::new(40, 28); // lejos de ambos
        state.towns.push(Town {
            id: 1,
            pos: near_a,
            name: "CercaA".into(),
            population: 400,
            ..Town::default()
        });
        state.towns.push(Town {
            id: 2,
            pos: near_b,
            name: "CercaB".into(),
            population: 400,
            ..Town::default()
        });
        state.towns.push(Town {
            id: 3,
            pos: far,
            name: "Lejos".into(),
            population: 900,
            ..Town::default()
        });
        let ai_id = company_id_by_name(&state.companies, RIVAL_NAME_ROADHAUL).unwrap();
        let plan = next_bus_plan(&state, ai_id).expect("plan");
        let ends = [plan.town_a, plan.town_b];
        assert!(ends.contains(&near_a) && ends.contains(&near_b));
        assert!(!ends.contains(&far));
    }
}
