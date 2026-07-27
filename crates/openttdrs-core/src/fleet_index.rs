//! Índices efímeros de flota y terminales.
//!
//! No forman parte del estado serializado: se reconstruyen después de cargar y
//! al comenzar cada tick. El recorrido de un consist usa `next_unit` con lookup
//! O(1), evitando el `iter().find` por eslabón.

use std::collections::{HashMap, HashSet};

use crate::map::TileCoord;
use crate::station::Station;
use crate::vehicle::Vehicle;

#[derive(Debug, Clone, Default)]
pub struct FleetIndex {
    slots: HashMap<u32, usize>,
    heads: HashMap<u32, u32>,
    consists: HashMap<u32, Vec<u32>>,
    rebuilds: u64,
}

impl FleetIndex {
    pub fn rebuild(&mut self, vehicles: &[Vehicle]) {
        self.slots.clear();
        self.heads.clear();
        self.consists.clear();
        self.slots.reserve(vehicles.len());
        self.heads.reserve(vehicles.len());
        for (slot, vehicle) in vehicles.iter().enumerate() {
            self.slots.insert(vehicle.id, slot);
        }

        let mut visited = HashSet::with_capacity(vehicles.len());
        for vehicle in vehicles.iter().filter(|v| v.prev_unit.is_none()) {
            self.index_chain(vehicles, vehicle.id, &mut visited);
        }
        // Saves dañados/cadenas cíclicas no deben dejar ids sin lookup.
        for vehicle in vehicles {
            if !visited.contains(&vehicle.id) {
                self.index_chain(vehicles, vehicle.id, &mut visited);
            }
        }
        self.rebuilds = self.rebuilds.saturating_add(1);
    }

    fn index_chain(&mut self, vehicles: &[Vehicle], head: u32, visited: &mut HashSet<u32>) {
        let mut ids = Vec::new();
        let mut current = Some(head);
        while let Some(id) = current {
            if ids.len() > 256 || !visited.insert(id) {
                break;
            }
            ids.push(id);
            self.heads.insert(id, head);
            current = self
                .slot(id)
                .and_then(|slot| vehicles.get(slot))
                .and_then(|vehicle| vehicle.next_unit);
        }
        if !ids.is_empty() {
            self.consists.insert(head, ids);
        }
    }

    #[must_use]
    pub fn slot(&self, vehicle_id: u32) -> Option<usize> {
        self.slots.get(&vehicle_id).copied()
    }

    #[must_use]
    pub fn head_id(&self, vehicle_id: u32) -> Option<u32> {
        self.heads.get(&vehicle_id).copied()
    }

    #[must_use]
    pub fn consist(&self, head_id: u32) -> &[u32] {
        self.consists.get(&head_id).map_or(&[], Vec::as_slice)
    }

    #[must_use]
    pub const fn rebuilds(&self) -> u64 {
        self.rebuilds
    }
}

/// Tile de estación/terminal → slots de estación que lo poseen.
#[derive(Debug, Clone, Default)]
pub struct TerminalSpatialIndex {
    by_tile: HashMap<TileCoord, Vec<usize>>,
    rebuilds: u64,
}

impl TerminalSpatialIndex {
    pub fn rebuild(&mut self, stations: &[Station]) {
        self.by_tile.clear();
        for (slot, station) in stations.iter().enumerate() {
            self.insert(station.pos, slot);
            for &tile in station.airport_tiles.iter().chain(&station.joined_tiles) {
                self.insert(tile, slot);
            }
        }
        self.rebuilds = self.rebuilds.saturating_add(1);
    }

    fn insert(&mut self, tile: TileCoord, slot: usize) {
        let slots = self.by_tile.entry(tile).or_default();
        if !slots.contains(&slot) {
            slots.push(slot);
        }
    }

    #[must_use]
    pub fn at(&self, tile: TileCoord) -> &[usize] {
        self.by_tile.get(&tile).map_or(&[], Vec::as_slice)
    }

    #[must_use]
    pub const fn rebuilds(&self) -> u64 {
        self.rebuilds
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Station, TileCoord, Vehicle, VehicleKind};

    #[test]
    fn indexes_slots_and_consist_topology_in_one_rebuild() {
        let pos = TileCoord::new(1, 1);
        let mut head = Vehicle::new(90, VehicleKind::Train, pos, pos);
        let mut wagon = Vehicle::new(7, VehicleKind::Train, pos, pos);
        let mut tail = Vehicle::new(41, VehicleKind::Train, pos, pos);
        head.next_unit = Some(7);
        wagon.prev_unit = Some(90);
        wagon.next_unit = Some(41);
        tail.prev_unit = Some(7);
        let vehicles = vec![wagon, tail, head];

        let mut index = FleetIndex::default();
        index.rebuild(&vehicles);
        assert_eq!(index.slot(90), Some(2));
        assert_eq!(index.head_id(41), Some(90));
        assert_eq!(index.consist(90), &[90, 7, 41]);
        assert_eq!(index.rebuilds(), 1);
    }

    #[test]
    fn terminal_index_covers_joined_and_airport_tiles() {
        let anchor = TileCoord::new(3, 4);
        let joined = TileCoord::new(4, 4);
        let airport = TileCoord::new(5, 4);
        let mut station = Station::new(anchor);
        station.joined_tiles.push(joined);
        station.airport_tiles.push(airport);
        let mut index = TerminalSpatialIndex::default();
        index.rebuild(&[station]);
        assert_eq!(index.at(anchor), &[0]);
        assert_eq!(index.at(joined), &[0]);
        assert_eq!(index.at(airport), &[0]);
    }
}
