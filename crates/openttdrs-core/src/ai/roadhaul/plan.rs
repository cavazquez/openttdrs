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
pub(super) fn next_bus_plan(state: &GameState, ai_id: CompanyId) -> Option<BusPlan> {
    let served: Vec<TileCoord> = state
        .stations
        .iter()
        .filter(|s| s.owner == ai_id && s.stop_kind == StopKind::BusStop)
        .map(|s| s.pos)
        .collect();

    let towns: Vec<TileCoord> = state.towns.iter().map(|t| t.pos).collect();
    if towns.len() < 2 {
        return None;
    }

    let mut best: Option<(u32, TileCoord, TileCoord)> = None;
    for i in 0..towns.len() {
        for j in (i + 1)..towns.len() {
            let a = towns[i];
            let b = towns[j];
            let dist = a.x.abs_diff(b.x) + a.y.abs_diff(b.y);
            if dist < 6 {
                continue;
            }
            let a_served = served.iter().any(|s| (s.x - a.x).abs() <= 3 && (s.y - a.y).abs() <= 3);
            let b_served = served.iter().any(|s| (s.x - b.x).abs() <= 3 && (s.y - b.y).abs() <= 3);
            if a_served && b_served {
                continue;
            }
            if best.is_none_or(|(d, _, _)| dist > d) {
                best = Some((dist, a, b));
            }
        }
    }
    best.map(|(_, town_a, town_b)| BusPlan { town_a, town_b })
}
