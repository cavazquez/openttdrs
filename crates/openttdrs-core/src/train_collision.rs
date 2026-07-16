//! Choques de trenes (`CheckTrainCollision` / `Train::Crash` simplificado).
//!
//! `OpenTTD` solo choca trenes de la **misma** compañía; depósitos excluidos.
//! El bloqueo preventivo evita la mayoría; `force_proceed` puede forzar solape.

use crate::GameState;
use crate::map::{Map, TileCoord, TileKind};
use crate::news::{NewsReference, NewsType, add_news_item, default_display_for_type};
use crate::sim_events::SimEvent;
use crate::train_consist::{consist_occupied_tiles, same_consist};
use crate::vehicle::{Vehicle, VehicleKind};

/// Par de cabezas de consist que colisionan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrainCollision {
    pub a: u32,
    pub b: u32,
    pub at: TileCoord,
}

/// Detecta solapes de huella entre cabezas de tren de la misma compañía.
#[must_use]
pub fn detect_train_collisions(map: &Map, vehicles: &[Vehicle]) -> Vec<TrainCollision> {
    let mut out = Vec::new();
    let heads: Vec<&Vehicle> = vehicles
        .iter()
        .filter(|v| v.kind == VehicleKind::Train && v.is_consist_head())
        .collect();
    for i in 0..heads.len() {
        for j in (i + 1)..heads.len() {
            let a = heads[i];
            let b = heads[j];
            if a.owner != b.owner {
                continue;
            }
            if same_consist(vehicles, a.id, b.id) {
                continue;
            }
            if map.get_kind(a.pos) == Some(TileKind::RailDepot)
                || map.get_kind(b.pos) == Some(TileKind::RailDepot)
            {
                continue;
            }
            let tiles_a = consist_occupied_tiles(vehicles, a.id);
            let tiles_b = consist_occupied_tiles(vehicles, b.id);
            let Some(&at) = tiles_a.iter().find(|t| tiles_b.contains(t)) else {
                continue;
            };
            // Depósito compartido no es choque.
            if map.get_kind(at) == Some(TileKind::RailDepot) {
                continue;
            }
            out.push(TrainCollision {
                a: a.id,
                b: b.id,
                at,
            });
        }
    }
    out
}

/// Destruye los consists involucrados, emite evento y noticia.
pub fn resolve_train_collisions(state: &mut GameState) {
    let collisions = detect_train_collisions(&state.map, &state.vehicles);
    if collisions.is_empty() {
        return;
    }
    let mut doomed = std::collections::HashSet::new();
    for c in &collisions {
        doomed.insert(c.a);
        doomed.insert(c.b);
        // Incluir vagones del consist.
        for v in &state.vehicles {
            if same_consist(&state.vehicles, c.a, v.id) || same_consist(&state.vehicles, c.b, v.id)
            {
                doomed.insert(v.id);
            }
        }
        state.runtime.pending_sim_events.push(SimEvent::TrainCollision {
            at: c.at,
            vehicle_a: c.a,
            vehicle_b: c.b,
        });
        let victims = 2u32.saturating_add(
            u32::try_from(
                state
                    .vehicles
                    .iter()
                    .filter(|v| {
                        same_consist(&state.vehicles, c.a, v.id)
                            || same_consist(&state.vehicles, c.b, v.id)
                    })
                    .count(),
            )
            .unwrap_or(0),
        );
        let id = state.news.next_id;
        state.news.next_id = state.news.next_id.saturating_add(1);
        let item = crate::news::NewsItem::new(
            id,
            format!("Choque de trenes ({victims} víctimas)"),
            Some(format!(
                "Los trenes #{} y #{} colisionaron en ({}, {}).",
                c.a, c.b, c.at.x, c.at.y
            )),
            NewsType::Accident,
            default_display_for_type(NewsType::Accident),
            state.tick,
            NewsReference::Tile(c.at),
        );
        add_news_item(state, item);
    }
    state.vehicles.retain(|v| !doomed.contains(&v.id));
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::{Command, GameState, Vehicle, apply_command};

    #[test]
    fn same_tile_trains_crash_and_emit_news() {
        let mut state = GameState::new(16, 16);
        let pos = TileCoord::new(4, 4);
        apply_command(&mut state, &Command::PlaceRail(pos)).unwrap();
        let mut a = Vehicle::new(1, VehicleKind::Train, pos, pos);
        a.running = true;
        let mut b = Vehicle::new(2, VehicleKind::Train, pos, pos);
        b.running = true;
        state.vehicles.push(a);
        state.vehicles.push(b);

        resolve_train_collisions(&mut state);
        assert!(state.vehicles.is_empty());
        assert!(
            state
                .runtime
                .pending_sim_events
                .drain()
                .iter()
                .any(|e| matches!(e, SimEvent::TrainCollision { .. }))
        );
        assert!(
            state
                .news
                .items
                .iter()
                .any(|n| n.news_type == NewsType::Accident)
        );
    }

    #[test]
    fn different_owners_do_not_crash() {
        let mut state = GameState::new(16, 16);
        state.ensure_rival_transcargo();
        let pos = TileCoord::new(4, 4);
        apply_command(&mut state, &Command::PlaceRail(pos)).unwrap();
        let mut a = Vehicle::new(1, VehicleKind::Train, pos, pos);
        a.running = true;
        let mut b = Vehicle::new(2, VehicleKind::Train, pos, pos);
        b.running = true;
        b.owner = crate::company::CompanyId(1);
        state.vehicles.push(a);
        state.vehicles.push(b);

        resolve_train_collisions(&mut state);
        assert_eq!(state.vehicles.len(), 2);
    }
}
