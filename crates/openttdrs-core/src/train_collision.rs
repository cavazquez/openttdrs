//! Choques de trenes (`CheckTrainCollision` / `Train::Crash` simplificado).
//!
//! `OpenTTD` solo choca trenes de la **misma** compañía; depósitos excluidos.
//! El bloqueo preventivo evita la mayoría; `force_proceed` puede forzar solape.

use crate::GameState;
use crate::company::CompanyId;
use crate::fleet_index::FleetIndex;
use crate::map::{Map, TileCoord, TileKind};
use crate::news::{NewsReference, NewsType, add_news_item, default_display_for_type};
use crate::sim_events::SimEvent;
use crate::vehicle::{Vehicle, VehicleKind};
use std::collections::{HashMap, HashSet};

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
    let mut fleet = FleetIndex::default();
    fleet.rebuild(vehicles);
    let mut occupants: HashMap<TileCoord, Vec<(u32, CompanyId)>> = HashMap::new();
    let mut reported_pairs = HashSet::new();

    for head in vehicles
        .iter()
        .filter(|v| v.kind == VehicleKind::Train && v.is_consist_head())
    {
        if map.get_kind(head.pos) == Some(TileKind::RailDepot) {
            continue;
        }
        for at in consist_occupied_tiles_indexed(vehicles, &fleet, head) {
            // Depósito compartido no es choque.
            if map.get_kind(at) == Some(TileKind::RailDepot) {
                continue;
            }
            let entries = occupants.entry(at).or_default();
            for &(other_id, other_owner) in entries.iter() {
                if other_owner != head.owner {
                    continue;
                }
                let pair = (other_id.min(head.id), other_id.max(head.id));
                if reported_pairs.insert(pair) {
                    out.push(TrainCollision {
                        a: other_id,
                        b: head.id,
                        at,
                    });
                }
            }
            entries.push((head.id, head.owner));
        }
    }
    out
}

/// Huella del consist usando el índice ya construido para este tick.
///
/// El helper público reconstruye el índice en cada llamada para APIs aisladas;
/// la detección de choques la invoca para toda la flota y debe compartirlo.
fn consist_occupied_tiles_indexed(
    vehicles: &[Vehicle],
    fleet: &FleetIndex,
    head: &Vehicle,
) -> Vec<TileCoord> {
    let ids = fleet.consist(head.id);
    let mut tiles = Vec::with_capacity(ids.len());
    for &id in ids {
        let Some(slot) = fleet.slot(id) else {
            continue;
        };
        let pos = vehicles[slot].pos;
        if !tiles.contains(&pos) {
            tiles.push(pos);
        }
    }
    // Mantener la salvaguarda para escenarios antiguos sin cadena física.
    if ids.len() <= 1 {
        let span = u32::from(head.cached_total_length)
            .div_ceil(u32::from(crate::train_consist::TILE_FRACTIONS))
            .max(1) as usize;
        for &tile in head.rail_tile_history.iter().take(span.saturating_sub(1)) {
            if !tiles.contains(&tile) {
                tiles.push(tile);
            }
        }
    }
    tiles
}

/// Destruye los consists involucrados, emite evento y noticia.
pub fn resolve_train_collisions(state: &mut GameState) {
    let collisions = detect_train_collisions(&state.map, &state.vehicles);
    if collisions.is_empty() {
        return;
    }
    let mut fleet = FleetIndex::default();
    fleet.rebuild(&state.vehicles);
    let mut doomed = HashSet::new();
    for c in &collisions {
        let victims: HashSet<u32> = fleet
            .consist(c.a)
            .iter()
            .chain(fleet.consist(c.b))
            .copied()
            .collect();
        doomed.extend(victims.iter().copied());
        state
            .runtime
            .pending_sim_events
            .push(SimEvent::TrainCollision {
                at: c.at,
                vehicle_a: c.a,
                vehicle_b: c.b,
            });
        let victims = u32::try_from(victims.len()).unwrap_or(0);
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
