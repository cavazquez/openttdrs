//! Elección de par de pueblos para la línea de buses.

use crate::GameState;
use crate::company::CompanyId;
use crate::map::TileCoord;
use crate::station::StopKind;
use crate::vehicle::{VehicleKind, VehicleOrder};

/// Manhattan mínima entre pueblos para una línea bus.
pub(super) const MIN_TOWN_DIST: u32 = 6;
/// Techo: evita corredores enormes aunque haya ciudades enormes (#192).
pub(super) const MAX_TOWN_DIST: u32 = 40;

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

/// Potencial de pax del par: `(pop_a * pop_b) / dist` (#192).
#[must_use]
pub(super) fn bus_pair_score(pop_a: u32, pop_b: u32, dist: u32) -> u64 {
    let pop = u64::from(pop_a.max(1)).saturating_mul(u64::from(pop_b.max(1)));
    pop / u64::from(dist.max(1))
}

fn town_has_bus_stop(served: &[TileCoord], town: TileCoord) -> bool {
    served
        .iter()
        .any(|s| (s.x - town.x).abs() <= 3 && (s.y - town.y).abs() <= 3)
}

/// Par de pueblos aún no ambos servidos (`MIN_TOWN_DIST`..=`MAX_TOWN_DIST`).
///
/// Maximiza `bus_pair_score`; empate → menor distancia, luego más población.
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

    // Ordenación lexicográfica invertida en score: mayor score gana.
    // Empate: menor dist, luego mayor pop conjunta (pop_key más negativo).
    let mut best: Option<(u64, u32, i64, TileCoord, TileCoord)> = None;
    for i in 0..towns.len() {
        for j in (i + 1)..towns.len() {
            let (a, pop_a) = towns[i];
            let (b, pop_b) = towns[j];
            let dist = a.x.abs_diff(b.x) + a.y.abs_diff(b.y);
            if !(MIN_TOWN_DIST..=MAX_TOWN_DIST).contains(&dist) {
                continue;
            }
            if town_has_bus_stop(&served, a) && town_has_bus_stop(&served, b) {
                continue;
            }
            let score = bus_pair_score(pop_a, pop_b, dist);
            let pop_key = -i64::from(pop_a.saturating_add(pop_b));
            let better = match best {
                None => true,
                Some((best_score, best_dist, best_pop, _, _)) => {
                    score > best_score
                        || (score == best_score && dist < best_dist)
                        || (score == best_score && dist == best_dist && pop_key < best_pop)
                }
            };
            if better {
                best = Some((score, dist, pop_key, a, b));
            }
        }
    }
    best.map(|(_, _, _, town_a, town_b)| BusPlan { town_a, town_b })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::company::{RIVAL_NAME_ROADHAUL, company_id_by_name};
    use crate::town::Town;

    #[test]
    fn bus_pair_score_prefers_larger_population_at_same_distance() {
        let small = bus_pair_score(200, 200, 10);
        let large = bus_pair_score(800, 600, 10);
        assert!(large > small);
    }

    #[test]
    fn bus_pair_score_penalizes_longer_distance() {
        let near = bus_pair_score(500, 500, 8);
        let far = bus_pair_score(500, 500, 32);
        assert!(near > far);
    }

    #[test]
    fn next_bus_plan_prefers_high_score_near_pair() {
        let mut state = GameState::new(48, 32);
        state.ensure_rival_ais();
        let near_a = TileCoord::new(8, 8);
        let near_b = TileCoord::new(14, 10); // dist 8
        let far = TileCoord::new(40, 28); // dist grande a ambos
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

    #[test]
    fn next_bus_plan_prefers_larger_towns_at_same_distance() {
        let mut state = GameState::new(32, 24);
        state.ensure_rival_ais();
        // Dos pares a dist 10; el grande debe ganar.
        let a = TileCoord::new(4, 4);
        let b = TileCoord::new(14, 4);
        let c = TileCoord::new(4, 16);
        let d = TileCoord::new(14, 16);
        state.towns.push(Town {
            id: 1,
            pos: a,
            name: "PequeA".into(),
            population: 100,
            ..Town::default()
        });
        state.towns.push(Town {
            id: 2,
            pos: b,
            name: "PequeB".into(),
            population: 100,
            ..Town::default()
        });
        state.towns.push(Town {
            id: 3,
            pos: c,
            name: "GrandeA".into(),
            population: 900,
            ..Town::default()
        });
        state.towns.push(Town {
            id: 4,
            pos: d,
            name: "GrandeB".into(),
            population: 800,
            ..Town::default()
        });
        let ai_id = company_id_by_name(&state.companies, RIVAL_NAME_ROADHAUL).unwrap();
        let plan = next_bus_plan(&state, ai_id).expect("plan");
        let ends = [plan.town_a, plan.town_b];
        assert!(ends.contains(&c) && ends.contains(&d));
    }

    #[test]
    fn next_bus_plan_skips_pair_already_served() {
        use crate::station::{Station, StopKind};
        let mut state = GameState::new(32, 24);
        state.ensure_rival_ais();
        let a = TileCoord::new(4, 4);
        let b = TileCoord::new(14, 4);
        let c = TileCoord::new(4, 16);
        state.towns.push(Town {
            id: 1,
            pos: a,
            name: "A".into(),
            population: 500,
            ..Town::default()
        });
        state.towns.push(Town {
            id: 2,
            pos: b,
            name: "B".into(),
            population: 500,
            ..Town::default()
        });
        state.towns.push(Town {
            id: 3,
            pos: c,
            name: "C".into(),
            population: 500,
            ..Town::default()
        });
        let ai_id = company_id_by_name(&state.companies, RIVAL_NAME_ROADHAUL).unwrap();
        let mut stop_a = Station::new(TileCoord::new(5, 4));
        stop_a.owner = ai_id;
        stop_a.stop_kind = StopKind::BusStop;
        let mut stop_b = Station::new(TileCoord::new(13, 4));
        stop_b.owner = ai_id;
        stop_b.stop_kind = StopKind::BusStop;
        state.stations.push(stop_a);
        state.stations.push(stop_b);

        let plan = next_bus_plan(&state, ai_id).expect("segunda ruta");
        let ends = [plan.town_a, plan.town_b];
        assert!(ends.contains(&c), "debe expandir hacia el pueblo libre");
        assert!(ends.contains(&a) || ends.contains(&b));
    }
}
