//! Estado `Crashed` / `crashed_ctr` y `RoadVehCheckTrainCrash` en pasos a nivel.

use crate::GameState;
use crate::map::{Map, TileCoord, is_road_level_crossing};
use crate::news::{NewsReference, NewsType, add_news_item, default_display_for_type};
use crate::road_movement::traffic::is_road_vehicle_kind;
use crate::sim_events::SimEvent;
use crate::vehicle::{Vehicle, VehicleKind};
use std::collections::HashMap;

/// Tras este contador el vehículo estrellado se elimina (`roadveh_cmd.cpp`).
pub const CRASHED_CTR_REMOVE: u16 = 2_220;
/// Valor inicial al chocar (no inundación).
pub const CRASHED_CTR_START: u16 = 1;

/// Tren(es) presentes por tesela para el chequeo de pasos a nivel del tick.
#[derive(Debug, Clone, Default)]
pub struct TrainCrashIndex {
    by_tile: HashMap<TileCoord, Vec<usize>>,
}

impl TrainCrashIndex {
    pub fn rebuild(&mut self, vehicles: &[Vehicle]) {
        self.by_tile.clear();
        for (index, vehicle) in vehicles.iter().enumerate() {
            if vehicle.kind == VehicleKind::Train {
                self.by_tile.entry(vehicle.pos).or_default().push(index);
            }
        }
    }

    fn at(&self, pos: TileCoord) -> &[usize] {
        self.by_tile.get(&pos).map_or(&[], Vec::as_slice)
    }

    /// Actualiza una unidad de tren luego de que la propagación del consist
    /// modificó su tesela.
    pub fn update_vehicle(&mut self, vehicles: &[Vehicle], index: usize, previous: TileCoord) {
        let Some(vehicle) = vehicles.get(index) else {
            return;
        };
        if vehicle.kind != VehicleKind::Train || vehicle.pos == previous {
            return;
        }
        if let Some(indices) = self.by_tile.get_mut(&previous) {
            indices.retain(|&other| other != index);
            if indices.is_empty() {
                self.by_tile.remove(&previous);
            }
        }
        self.by_tile.entry(vehicle.pos).or_default().push(index);
    }
}

/// Marca la cadena (cabeza road o tren) como estrellada.
pub fn crash_vehicle(v: &mut Vehicle, flooded: bool) {
    v.crashed = true;
    v.running = false;
    v.cur_speed = 0;
    v.crashed_ctr = if flooded { 2_000 } else { CRASHED_CTR_START };
}

/// ¿Hay un tren cerca en Z de un roadveh en cruce a nivel?
#[must_use]
pub fn road_veh_check_train_crash(map: &Map, vehicles: &[Vehicle], v_idx: usize) -> bool {
    let Some(v) = vehicles.get(v_idx) else {
        return false;
    };
    if !is_road_vehicle_kind(v.kind) || v.crashed {
        return false;
    }
    let Some(tile) = map.get(v.pos) else {
        return false;
    };
    if !is_road_level_crossing(tile.mapt, tile.m5, tile.kind) {
        return false;
    }
    let vz = v.z_pos.unwrap_or(0);
    vehicles.iter().any(|t| {
        t.kind == VehicleKind::Train
            && !t.crashed
            && t.pos == v.pos
            && (t.z_pos.unwrap_or(0) - vz).abs() <= 6
    })
}

/// Aplica choque RV↔tren en cruce; emite evento y noticia.
pub fn maybe_road_train_crash(state: &mut GameState, v_idx: usize) -> bool {
    if !road_veh_check_train_crash(&state.map, &state.vehicles, v_idx) {
        return false;
    }
    let vehicle_id = state.vehicles[v_idx].id;
    let at = state.vehicles[v_idx].pos;
    crash_vehicle(&mut state.vehicles[v_idx], false);
    // Marcar trenes en la misma tesela.
    for t in &mut state.vehicles {
        if t.kind == VehicleKind::Train && t.pos == at && !t.crashed {
            crash_vehicle(t, false);
        }
    }
    state
        .runtime
        .pending_sim_events
        .push(SimEvent::LevelCrossing { at });
    state
        .runtime
        .pending_sim_events
        .push(SimEvent::RoadVehCrash { vehicle_id, at });
    let id = state.news.next_id;
    state.news.next_id = state.news.next_id.saturating_add(1);
    let item = crate::news::NewsItem::new(
        id,
        format!("Choque en paso a nivel (vehículo #{vehicle_id})"),
        Some(format!(
            "Un vehículo de carretera chocó con un tren en ({}, {}).",
            at.x, at.y
        )),
        NewsType::Accident,
        default_display_for_type(NewsType::Accident),
        state.tick,
        NewsReference::Tile(at),
    );
    add_news_item(state, item);
    true
}

/// Variante indexada de [`maybe_road_train_crash`] para el barrido de movimiento.
///
/// Evita revisar toda la flota por cada roadveh: sólo se consulta si está sobre
/// un paso a nivel y sólo compara las unidades de tren de esa tesela.
pub fn maybe_road_train_crash_indexed(
    state: &mut GameState,
    v_idx: usize,
    trains: &TrainCrashIndex,
) -> bool {
    let Some(v) = state.vehicles.get(v_idx) else {
        return false;
    };
    if !is_road_vehicle_kind(v.kind) || v.crashed {
        return false;
    }
    let at = v.pos;
    let vz = v.z_pos.unwrap_or(0);
    let Some(tile) = state.map.get(at) else {
        return false;
    };
    if !is_road_level_crossing(tile.mapt, tile.m5, tile.kind) {
        return false;
    }
    let collided: Vec<usize> = trains
        .at(at)
        .iter()
        .copied()
        .filter(|&index| {
            state.vehicles.get(index).is_some_and(|train| {
                train.kind == VehicleKind::Train
                    && !train.crashed
                    && (train.z_pos.unwrap_or(0) - vz).abs() <= 6
            })
        })
        .collect();
    if collided.is_empty() {
        return false;
    }
    let vehicle_id = state.vehicles[v_idx].id;
    crash_vehicle(&mut state.vehicles[v_idx], false);
    for index in collided {
        crash_vehicle(&mut state.vehicles[index], false);
    }
    state
        .runtime
        .pending_sim_events
        .push(SimEvent::LevelCrossing { at });
    state
        .runtime
        .pending_sim_events
        .push(SimEvent::RoadVehCrash { vehicle_id, at });
    let id = state.news.next_id;
    state.news.next_id = state.news.next_id.saturating_add(1);
    let item = crate::news::NewsItem::new(
        id,
        format!("Choque en paso a nivel (vehículo #{vehicle_id})"),
        Some(format!(
            "Un vehículo de carretera chocó con un tren en ({}, {}).",
            at.x, at.y
        )),
        NewsType::Accident,
        default_display_for_type(NewsType::Accident),
        state.tick,
        NewsReference::Tile(at),
    );
    add_news_item(state, item);
    true
}

/// Tick de animación/eliminación de vehículos estrellados (`RoadVehIsCrashed`).
pub fn tick_crashed_vehicles(state: &mut GameState) {
    let mut remove = Vec::new();
    for v in &mut state.vehicles {
        if !v.crashed {
            continue;
        }
        v.crashed_ctr = v.crashed_ctr.saturating_add(1);
        v.cur_speed = 0;
        v.running = false;
        if v.crashed_ctr >= CRASHED_CTR_REMOVE {
            remove.push(v.id);
        }
    }
    if remove.is_empty() {
        return;
    }
    // Quitar también unidades de consist enlazadas.
    let mut doomed = remove.clone();
    for id in &remove {
        for v in &state.vehicles {
            if v.next_unit == Some(*id)
                || v.prev_unit == Some(*id)
                || v.other_multiheaded_part == Some(*id)
            {
                doomed.push(v.id);
            }
        }
    }
    doomed.sort_unstable();
    doomed.dedup();
    state.vehicles.retain(|v| !doomed.contains(&v.id));
}

/// Coordenada de tesela (helper de tests / eventos).
#[must_use]
pub fn crash_tile_of(v: &Vehicle) -> TileCoord {
    v.pos
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::map::{OTTD_MP_ROAD, TileKind};
    use crate::vehicle::VehicleKind;

    #[test]
    fn crash_sets_ctr_and_stops() {
        let mut v = Vehicle::new(
            1,
            VehicleKind::Bus,
            TileCoord::new(1, 1),
            TileCoord::new(2, 2),
        );
        v.running = true;
        v.cur_speed = 40;
        crash_vehicle(&mut v, false);
        assert!(v.crashed);
        assert!(!v.running);
        assert_eq!(v.crashed_ctr, CRASHED_CTR_START);
        assert_eq!(v.cur_speed, 0);
    }

    #[test]
    fn detect_train_on_level_crossing() {
        let mut state = GameState::new(8, 8);
        let at = TileCoord::new(3, 3);
        if let Some(mut t) = state.map.get(at) {
            t.kind = TileKind::Road;
            t.mapt = OTTD_MP_ROAD << 4;
            t.m5 = 1 << 6;
            state.map.set_tile(at, t).unwrap();
        }
        assert!(
            state
                .map
                .get(at)
                .is_some_and(|t| is_road_level_crossing(t.mapt, t.m5, t.kind))
        );
        let mut bus = Vehicle::new(1, VehicleKind::Bus, at, at);
        bus.z_pos = Some(0);
        let mut train = Vehicle::new(2, VehicleKind::Train, at, at);
        train.z_pos = Some(0);
        state.vehicles = vec![bus, train];
        assert!(road_veh_check_train_crash(&state.map, &state.vehicles, 0));
    }
}
