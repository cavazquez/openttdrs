//! Desastres ambientales (simplificación de `disaster_vehicle.cpp`).

use crate::GameState;
use crate::economy::TICKS_PER_YEAR;
use crate::map::{Map, TileCoord, TileKind};
use crate::sim_events::{DisasterKind, SimEvent};

/// Intervalo base entre comprobaciones de desastre.
pub const DISASTER_CHECK_INTERVAL: u64 = TICKS_PER_YEAR / 4;

fn default_disaster_timer() -> u64 {
    DISASTER_CHECK_INTERVAL
}

/// Avanza el temporizador y puede disparar un desastre aleatorio.
pub fn tick_disasters(state: &mut GameState) {
    if !state.disasters_enabled {
        return;
    }
    if state.disaster_timer == 0 {
        state.disaster_timer = default_disaster_timer();
        if (state.tick.get() / 997).is_multiple_of(3) {
            let kind = random_disaster_kind(state.tick.get());
            if let Some(at) = pick_disaster_target(&state.map, &state.vehicles, state.tick.get()) {
                trigger_disaster_at(state, kind, at);
            }
        }
    } else {
        state.disaster_timer = state.disaster_timer.saturating_sub(1);
    }
}

#[must_use]
const fn random_disaster_kind(tick: u64) -> DisasterKind {
    match tick % 6 {
        0 => DisasterKind::SmallUfo,
        1 => DisasterKind::Airplane,
        2 => DisasterKind::Helicopter,
        3 => DisasterKind::BigUfo,
        4 => DisasterKind::Submarine,
        _ => DisasterKind::CoalMineSubsidence,
    }
}

fn pick_disaster_target(map: &Map, vehicles: &[crate::Vehicle], tick: u64) -> Option<TileCoord> {
    if !vehicles.is_empty() {
        let idx = usize::try_from(tick / 13).unwrap_or(0) % vehicles.len();
        return Some(vehicles[idx].pos);
    }
    let (w, h) = map.dimensions();
    if w == 0 || h == 0 {
        return None;
    }
    let x = i32::try_from(tick % u64::from(w)).unwrap_or(0);
    let y = i32::try_from((tick / 17) % u64::from(h)).unwrap_or(0);
    Some(TileCoord::new(x, y))
}

/// Destruye infraestructura/vehículos en `at` y emite [`SimEvent::Disaster`].
pub fn trigger_disaster_at(state: &mut GameState, kind: DisasterKind, at: TileCoord) {
    destroy_tile_contents(state, at);
    state
        .pending_sim_events
        .push(SimEvent::Disaster { kind, at });
    crate::news::push_disaster_news(state, kind, at);
}

fn destroy_tile_contents(state: &mut GameState, at: TileCoord) {
    state.vehicles.retain(|v| v.pos != at);
    if let Some(kind) = state.map.get_kind(at) {
        match kind {
            TileKind::Grass
            | TileKind::Forest
            | TileKind::CoalField
            | TileKind::Water
            | TileKind::Void
            | TileKind::House => {}
            TileKind::Industry => {
                state.industries.retain(|ind| !ind.contains_tile(at));
            }
            TileKind::Station => {
                state.stations.retain(|s| s.pos != at);
            }
            _ => {
                let _ = state.map.set_kind(at, TileKind::Grass);
                let _ = state.map.set_mapt_m5(at, 0x00, 0x00);
            }
        }
    }
}

/// Fuerza un desastre en tests o depuración.
pub fn force_disaster(state: &mut GameState, kind: DisasterKind, at: TileCoord) {
    trigger_disaster_at(state, kind, at);
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::{Command, GameState, Vehicle, VehicleKind, apply_command};

    #[test]
    fn forced_disaster_removes_vehicle_and_emits_event() {
        let mut state = GameState::new(8, 8);
        let pos = TileCoord::new(3, 3);
        apply_command(&mut state, &Command::PlaceRail(pos)).unwrap();
        let mut train = Vehicle::new(1, VehicleKind::Train, pos, pos);
        train.running = true;
        state.vehicles.push(train);

        force_disaster(&mut state, DisasterKind::SmallUfo, pos);
        assert!(state.vehicles.is_empty());
        assert_eq!(state.map.get_kind(pos), Some(TileKind::Grass));
        let events = state.pending_sim_events.drain();
        assert!(events.iter().any(|e| matches!(
            e,
            SimEvent::Disaster {
                kind: DisasterKind::SmallUfo,
                at,
            } if *at == pos
        )));
        assert!(
            state
                .news
                .items
                .iter()
                .any(|n| n.news_type == crate::NewsType::Accident)
        );
    }

    #[test]
    fn disasters_disabled_skips_timer() {
        let mut state = GameState::new(4, 4);
        state.disasters_enabled = false;
        state.disaster_timer = 1;
        tick_disasters(&mut state);
        assert_eq!(state.disaster_timer, 1);
    }
}
