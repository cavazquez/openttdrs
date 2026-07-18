//! Desastres ambientales (simplificación de `disaster_vehicle.cpp`).
//!
//! OVNIs (`SmallUfo` / `BigUfo`) spawnean un craft animado que vuela al objetivo
//! y solo entonces destruye (#188). El resto de desastres siguen siendo inmediatos.

use crate::GameState;
use crate::economy::TICKS_PER_YEAR;
use crate::map::{Map, TileCoord, TileKind};
use crate::sim_events::{DisasterKind, SimEvent};

/// Intervalo base entre comprobaciones de desastre.
pub const DISASTER_CHECK_INTERVAL: u64 = TICKS_PER_YEAR / 4;

/// Ticks de vuelo del OVNI antes del impacto (~1 día de tránsito).
pub const UFO_FLIGHT_TICKS: u16 = 74;
/// Cada cuántos ticks el OVNI avanza una tesela hacia el objetivo.
pub const UFO_MOVE_EVERY_TICKS: u16 = 4;
/// Altitud visual del OVNI (misma escala que `Vehicle::altitude`).
pub const UFO_ALTITUDE: u8 = 24;

fn default_disaster_timer() -> u64 {
    DISASTER_CHECK_INTERVAL
}

/// Craft de desastre en vuelo (OVNI). No es un [`crate::Vehicle`] de compañía.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisasterCraft {
    pub id: u32,
    pub kind: DisasterKind,
    pub pos: TileCoord,
    pub target: TileCoord,
    pub altitude: u8,
    /// Edad en ticks desde el spawn.
    pub age: u16,
    /// Ticks restantes hasta el impacto (0 = impactar este tick).
    pub ticks_to_impact: u16,
}

impl DisasterCraft {
    #[must_use]
    pub fn is_ufo(&self) -> bool {
        matches!(self.kind, DisasterKind::SmallUfo | DisasterKind::BigUfo)
    }
}

/// Avanza el temporizador y puede disparar un desastre aleatorio.
pub fn tick_disasters(state: &mut GameState) {
    tick_disaster_crafts(state);
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

/// Mueve OVNIs en vuelo e impacta al llegar / agotar el temporizador.
pub fn tick_disaster_crafts(state: &mut GameState) {
    if state.disaster_crafts.is_empty() {
        return;
    }
    let mut impacts = Vec::new();
    for craft in &mut state.disaster_crafts {
        craft.age = craft.age.saturating_add(1);
        if craft.ticks_to_impact > 0 {
            craft.ticks_to_impact -= 1;
        }
        if craft.age.is_multiple_of(UFO_MOVE_EVERY_TICKS) {
            craft.pos = step_toward(craft.pos, craft.target);
        }
        if craft.pos == craft.target || craft.ticks_to_impact == 0 {
            impacts.push((craft.id, craft.kind, craft.target));
        }
    }
    for (id, kind, at) in impacts {
        state.disaster_crafts.retain(|c| c.id != id);
        destroy_tile_contents(state, at);
        state
            .runtime
            .pending_sim_events
            .push(SimEvent::Disaster { kind, at });
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

/// Lanza un desastre. OVNIs spawnean craft en vuelo; el resto impacta al instante.
pub fn trigger_disaster_at(state: &mut GameState, kind: DisasterKind, at: TileCoord) {
    if matches!(kind, DisasterKind::SmallUfo | DisasterKind::BigUfo) {
        spawn_ufo_craft(state, kind, at);
        crate::news::push_disaster_news(state, kind, at);
        return;
    }
    destroy_tile_contents(state, at);
    state
        .runtime
        .pending_sim_events
        .push(SimEvent::Disaster { kind, at });
    crate::news::push_disaster_news(state, kind, at);
}

fn spawn_ufo_craft(state: &mut GameState, kind: DisasterKind, target: TileCoord) {
    let id = state
        .disaster_crafts
        .iter()
        .map(|c| c.id)
        .max()
        .unwrap_or(0)
        .saturating_add(1);
    let pos = ufo_spawn_pos(&state.map, target);
    state.disaster_crafts.push(DisasterCraft {
        id,
        kind,
        pos,
        target,
        altitude: UFO_ALTITUDE,
        age: 0,
        ticks_to_impact: UFO_FLIGHT_TICKS,
    });
}

fn ufo_spawn_pos(map: &Map, target: TileCoord) -> TileCoord {
    let (w, h) = map.dimensions();
    let max_x = w.cast_signed().saturating_sub(1).max(0);
    let max_y = h.cast_signed().saturating_sub(1).max(0);
    let x = (target.x - 8).clamp(0, max_x);
    let y = (target.y - 8).clamp(0, max_y);
    TileCoord::new(x, y)
}

fn step_toward(from: TileCoord, to: TileCoord) -> TileCoord {
    let dx = (to.x - from.x).signum();
    let dy = (to.y - from.y).signum();
    TileCoord::new(from.x + dx, from.y + dy)
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

        force_disaster(&mut state, DisasterKind::Airplane, pos);
        assert!(state.vehicles.is_empty());
        assert_eq!(state.map.get_kind(pos), Some(TileKind::Grass));
        let events = state.runtime.pending_sim_events.drain();
        assert!(events.iter().any(|e| matches!(
            e,
            SimEvent::Disaster {
                kind: DisasterKind::Airplane,
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
    fn ufo_spawns_craft_before_impact() {
        let mut state = GameState::new(16, 16);
        let pos = TileCoord::new(10, 10);
        apply_command(&mut state, &Command::PlaceRail(pos)).unwrap();
        let mut train = Vehicle::new(1, VehicleKind::Train, pos, pos);
        train.running = true;
        state.vehicles.push(train);

        force_disaster(&mut state, DisasterKind::BigUfo, pos);
        assert_eq!(state.disaster_crafts.len(), 1);
        assert!(state.disaster_crafts[0].is_ufo());
        assert_ne!(state.disaster_crafts[0].pos, pos);
        // Aún no impactó: vía y tren siguen.
        assert_eq!(state.vehicles.len(), 1);
        assert_eq!(state.map.get_kind(pos), Some(TileKind::Rail));
        assert!(state.news.items.iter().any(|n| n.headline.contains("OVNI")));
    }

    #[test]
    fn ufo_impacts_after_flight_ticks() {
        let mut state = GameState::new(16, 16);
        let pos = TileCoord::new(10, 10);
        apply_command(&mut state, &Command::PlaceRail(pos)).unwrap();
        force_disaster(&mut state, DisasterKind::SmallUfo, pos);
        assert_eq!(state.disaster_crafts.len(), 1);

        for _ in 0..UFO_FLIGHT_TICKS {
            tick_disaster_crafts(&mut state);
        }
        assert!(
            state.disaster_crafts.is_empty(),
            "el OVNI debe impactar y desaparecer"
        );
        assert_eq!(state.map.get_kind(pos), Some(TileKind::Grass));
        let events = state.runtime.pending_sim_events.drain();
        assert!(events.iter().any(|e| matches!(
            e,
            SimEvent::Disaster {
                kind: DisasterKind::SmallUfo,
                ..
            }
        )));
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
