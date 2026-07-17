//! Planificación de rutas `TransCargo`: selección de industrias y estaciones.

use crate::GameState;
use crate::cargo::CargoType;
use crate::company::CompanyId;
use crate::industry::IndustryKind;
use crate::map::TileCoord;

#[derive(Debug, Clone, Copy)]
pub(crate) struct RoutePlan {
    pub source: TileCoord,
    pub dest: TileCoord,
    pub cargo: CargoType,
}

pub(super) fn ai_route_count(state: &GameState, ai_id: CompanyId) -> usize {
    use crate::vehicle::VehicleKind;
    state
        .vehicles
        .iter()
        .filter(|v| v.owner == ai_id && v.kind == VehicleKind::Train && v.is_consist_head())
        .count()
}

pub(super) fn industry_served_by_ai(
    state: &GameState,
    ai_id: CompanyId,
    industry_pos: TileCoord,
) -> bool {
    // Solo estaciones «de carga» junto a la industria (±2), no toda la cobertura
    // de radio 4 (una estación de carbón no debe marcar el bosque como servido).
    state.stations.iter().any(|st| {
        st.owner == ai_id
            && (st.pos.x - industry_pos.x).abs() <= 2
            && (st.pos.y - industry_pos.y).abs() <= 2
    })
}

pub(crate) fn next_unserved_plan(state: &GameState, ai_id: CompanyId) -> Option<RoutePlan> {
    let factory = state
        .industries
        .iter()
        .find(|i| i.kind == IndustryKind::Factory)
        .map(|i| i.pos)?;

    // Prioridad: carbón → madera → petróleo (todos descargan en la fábrica).
    let candidates = [
        (IndustryKind::CoalMine, CargoType::Coal),
        (IndustryKind::Forest, CargoType::Wood),
        (IndustryKind::OilWell, CargoType::Oil),
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

pub(super) fn tile_buildable_for_station(map: &crate::map::Map, c: crate::map::TileCoord) -> bool {
    use crate::map::TileKind;
    matches!(
        map.get_kind(c),
        Some(TileKind::Grass | TileKind::Forest | TileKind::Rail)
    )
}

/// Offsets candidatos a ±2 (cardinales) respecto de la industria.
pub(super) fn station_candidates_near(
    industry: crate::map::TileCoord,
) -> [crate::map::TileCoord; 4] {
    use crate::map::TileCoord;
    [
        TileCoord::new(industry.x + 2, industry.y),
        TileCoord::new(industry.x - 2, industry.y),
        TileCoord::new(industry.x, industry.y + 2),
        TileCoord::new(industry.x, industry.y - 2),
    ]
}

pub(super) fn pick_station_tile(
    map: &crate::map::Map,
    industry: crate::map::TileCoord,
    toward: crate::map::TileCoord,
    avoid: &[crate::map::TileCoord],
) -> Option<crate::map::TileCoord> {
    use crate::map::TileCoord;
    let mut cands: Vec<TileCoord> = station_candidates_near(industry)
        .into_iter()
        .filter(|&c| map.get(c).is_some() && tile_buildable_for_station(map, c))
        .filter(|c| !avoid.contains(c))
        .collect();
    // Preferir la tesela más cercana al otro extremo (corredor corto).
    cands.sort_by_key(|c| (c.x - toward.x).abs() + (c.y - toward.y).abs());
    cands.into_iter().next()
}
