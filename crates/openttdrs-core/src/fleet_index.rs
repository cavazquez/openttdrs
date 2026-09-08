//! Índices efímeros de flota y terminales.
//!
//! No forman parte del estado serializado: se reconstruyen después de cargar y
//! al comenzar cada tick. El recorrido de un consist usa `next_unit` con lookup
//! O(1), evitando el `iter().find` por eslabón.

use ahash::{AHashMap, AHashSet};

use crate::map::{Map, TileCoord, TileKind};
use crate::station::Station;
use crate::vehicle::Vehicle;

#[derive(Debug, Clone, Default)]
pub struct FleetIndex {
    // Índice transitorio reconstruido al inicio del tick. `AHashMap` conserva
    // una semilla aleatoria (no degrada ante IDs de un SAV hostil) y evita el
    // coste de SipHash en los miles de consultas internas por tick.
    slots: AHashMap<u32, usize>,
    heads: AHashMap<u32, u32>,
    consists: AHashMap<u32, Vec<u32>>,
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

        let mut visited = AHashSet::with_capacity(vehicles.len());
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

    fn index_chain(&mut self, vehicles: &[Vehicle], head: u32, visited: &mut AHashSet<u32>) {
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

/// Campos persistidos de una estación que determinan sus entradas en el índice.
///
/// Se guarda una copia pequeña sólo al reconstruir. Compararla en cada tick
/// evita depender de que todos los autores de mutaciones de `Vec<Station>`
/// recuerden invalidar un cache runtime.
#[derive(Debug, Clone)]
struct StationTopology {
    pos: TileCoord,
    ottd_station_id: Option<u32>,
    airport_tiles: Vec<TileCoord>,
    joined_tiles: Vec<TileCoord>,
}

impl StationTopology {
    fn capture(station: &Station) -> Self {
        Self {
            pos: station.pos,
            ottd_station_id: station.ottd_station_id,
            airport_tiles: station.airport_tiles.clone(),
            joined_tiles: station.joined_tiles.clone(),
        }
    }

    fn matches(&self, station: &Station) -> bool {
        self.pos == station.pos
            && self.ottd_station_id == station.ottd_station_id
            && self.airport_tiles == station.airport_tiles
            && self.joined_tiles == station.joined_tiles
    }
}

/// Huella exacta de los inputs que puede cambiar el índice de terminales.
#[derive(Debug, Clone)]
struct TerminalIndexTopology {
    map_version: (u64, u64),
    stations: Vec<StationTopology>,
}

impl TerminalIndexTopology {
    fn capture(map: &Map, stations: &[Station]) -> Self {
        Self {
            map_version: map.terminal_topology_version(),
            stations: stations.iter().map(StationTopology::capture).collect(),
        }
    }

    fn matches(&self, map: &Map, stations: &[Station]) -> bool {
        self.map_version == map.terminal_topology_version()
            && self.stations.len() == stations.len()
            && self
                .stations
                .iter()
                .zip(stations)
                .all(|(topology, station)| topology.matches(station))
    }
}

/// Tile de estación/terminal → slots de estación que lo poseen.
#[derive(Debug, Clone, Default)]
pub struct TerminalSpatialIndex {
    by_tile: AHashMap<TileCoord, Vec<usize>>,
    rebuilds: u64,
    full_map_scans: u64,
    topology: Option<TerminalIndexTopology>,
}

impl TerminalSpatialIndex {
    /// Reconstruye el índice sin reutilizar su cache.
    ///
    /// [`Self::ensure_current`] es la ruta normal de tick/comando. Este método
    /// queda disponible como fallback explícito para diagnósticos y fixtures.
    pub fn rebuild(&mut self, map: &Map, stations: &[Station]) {
        self.by_tile.clear();
        let mut imported_station_slots = AHashMap::new();
        for (slot, station) in stations.iter().enumerate() {
            self.insert(station.pos, slot);
            for &tile in station.airport_tiles.iter().chain(&station.joined_tiles) {
                self.insert(tile, slot);
            }
            if let Some(station_id) = station.ottd_station_id {
                imported_station_slots.insert(station_id, slot);
            }
        }

        // En OpenTTD, MAP2 guarda el `StationID` de toda tesela
        // `MP_STATION`. Importar sólo el ancla obligaba a buscar linealmente
        // todas las estaciones cada vez que un vehículo paraba en un andén
        // grande. Al poblar el índice desde MAP2 la consulta queda O(1).
        if !imported_station_slots.is_empty() {
            self.full_map_scans = self.full_map_scans.saturating_add(1);
            let (width, _) = map.dimensions();
            if let Ok(width) = usize::try_from(width)
                && width != 0
            {
                for (dense_index, tile) in map.tiles().iter().enumerate() {
                    if !matches!(tile.kind, TileKind::Station | TileKind::Airport) {
                        continue;
                    }
                    let station_id = u32::from(tile.m2) | (u32::from(tile.m2_hi) << 8);
                    let Some(&slot) = imported_station_slots.get(&station_id) else {
                        continue;
                    };
                    let Ok(x) = i32::try_from(dense_index % width) else {
                        continue;
                    };
                    let Ok(y) = i32::try_from(dense_index / width) else {
                        continue;
                    };
                    self.insert(TileCoord::new(x, y), slot);
                }
            }
        }
        self.rebuilds = self.rebuilds.saturating_add(1);
        self.topology = Some(TerminalIndexTopology::capture(map, stations));
    }

    /// Reutiliza el índice si ni las teselas terminales ni los slots/huellas
    /// de estación cambiaron. Una carga de mapa recibe una época nueva y por
    /// tanto siempre entra por el rebuild verificable.
    pub fn ensure_current(&mut self, map: &Map, stations: &[Station]) {
        if self
            .topology
            .as_ref()
            .is_none_or(|topology| !topology.matches(map, stations))
        {
            self.rebuild(map, stations);
        }
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

    /// Barridos densos del mapa necesarios para resolver `StationID` importados.
    #[must_use]
    pub const fn full_map_scans(&self) -> u64 {
        self.full_map_scans
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)] // fixtures fijas: un fallo de setup invalida la regresión

    use super::*;
    use crate::{Command, GameState, Map, Station, TileCoord, TileKind, Vehicle, VehicleKind};

    fn imported_station(map: &mut Map, pos: TileCoord, station_id: u16) -> Station {
        let mut tile = map.get(pos).expect("tesela de estación dentro del mapa");
        tile.kind = TileKind::Station;
        let [low, high] = station_id.to_le_bytes();
        tile.m2 = low;
        tile.m2_hi = high;
        map.set_tile(pos, tile)
            .expect("escritura de tesela de estación");

        let mut station = Station::new(pos);
        station.ottd_station_id = Some(u32::from(station_id));
        station
    }

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
        index.rebuild(&Map::new_flat(8, 8, 0), &[station]);
        assert_eq!(index.at(anchor), &[0]);
        assert_eq!(index.at(joined), &[0]);
        assert_eq!(index.at(airport), &[0]);
    }

    #[test]
    fn terminal_index_covers_imported_station_tiles_by_ottd_id() {
        let anchor = TileCoord::new(1, 1);
        let platform = TileCoord::new(6, 4);
        let mut station = Station::new(anchor);
        station.ottd_station_id = Some(42);
        let mut map = Map::new_flat(8, 8, 0);
        let Some(mut tile) = map.get(platform) else {
            panic!("platform in map");
        };
        tile.kind = TileKind::Station;
        tile.m2 = 42;
        assert!(map.set_tile(platform, tile).is_ok());

        let mut index = TerminalSpatialIndex::default();
        index.rebuild(&map, &[station]);
        assert_eq!(index.at(platform), &[0]);
    }

    #[test]
    fn terminal_index_reuses_imported_scan_until_topology_changes() {
        let anchor = TileCoord::new(2, 2);
        let joined = TileCoord::new(3, 2);
        let mut map = Map::new_flat(8, 8, 0);
        let mut stations = vec![imported_station(&mut map, anchor, 42)];
        let mut index = TerminalSpatialIndex::default();

        index.ensure_current(&map, &stations);
        assert_eq!(index.at(anchor), &[0]);
        assert_eq!(index.rebuilds(), 1);
        assert_eq!(index.full_map_scans(), 1);

        // Un frame de animación de la tesela y cambios de carga no alteran la
        // asociación MAP2 ni la huella. Deben evitar tanto rebuild como scan.
        let mut tile = map.get(anchor).expect("ancla dentro del mapa");
        tile.m7 = 17;
        map.set_tile(anchor, tile).expect("actualización animada");
        stations[0].stock = 123;
        index.ensure_current(&map, &stations);
        assert_eq!(index.rebuilds(), 1);
        assert_eq!(index.full_map_scans(), 1);

        stations[0].joined_tiles.push(joined);
        index.ensure_current(&map, &stations);
        assert_eq!(index.at(joined), &[0]);
        assert_eq!(index.rebuilds(), 2);
        assert_eq!(index.full_map_scans(), 2);
    }

    #[test]
    fn terminal_index_tracks_station_lifecycle_and_loaded_map() {
        let sentinel = TileCoord::new(0, 0);
        let created = TileCoord::new(2, 2);
        let merge = TileCoord::new(3, 2);
        let mut map = Map::new_flat(8, 8, 0);
        let mut stations = vec![imported_station(&mut map, sentinel, 90)];
        let mut index = TerminalSpatialIndex::default();

        index.ensure_current(&map, &stations);
        assert_eq!(index.at(sentinel), &[0]);

        // Crear una estación importada añade un slot y una entrada MAP2.
        stations.push(imported_station(&mut map, created, 7));
        index.ensure_current(&map, &stations);
        assert_eq!(index.at(created), &[1]);

        // Al unir, el slot que desaparece pasa a la huella de la estación
        // conservada y MAP2 apunta a su StationID.
        stations.push(imported_station(&mut map, merge, 8));
        index.ensure_current(&map, &stations);
        assert_eq!(index.at(merge), &[2]);
        stations.remove(2);
        stations[1].joined_tiles.push(merge);
        map.set_m2_u16(merge, 7).expect("reasignar StationID unido");
        index.ensure_current(&map, &stations);
        assert_eq!(index.at(merge), &[1]);

        // Demoler una tesela de la huella y borrar la estación no deja slots
        // obsoletos, aun cuando el vector se reordena.
        stations[1].joined_tiles.clear();
        map.set_kind(merge, TileKind::Grass)
            .expect("demoler tesela unida");
        index.ensure_current(&map, &stations);
        assert!(index.at(merge).is_empty());
        map.set_kind(created, TileKind::Grass)
            .expect("demoler estación creada");
        stations.remove(1);
        index.ensure_current(&map, &stations);
        assert!(index.at(created).is_empty());
        assert_eq!(index.at(sentinel), &[0]);

        // Un mapa deserializado conserva las mismas teselas, pero recibe una
        // época runtime nueva y fuerza un rebuild verificable al cargar.
        let encoded = serde_json::to_string(&map).expect("serializar mapa");
        let loaded: Map = serde_json::from_str(&encoded).expect("cargar mapa");
        index.ensure_current(&loaded, &stations);
        assert_eq!(index.at(sentinel), &[0]);
        assert_eq!(index.rebuilds(), 7);
        assert_eq!(index.full_map_scans(), 7);
    }

    #[test]
    fn terminal_index_reuses_scan_for_idle_ticks_and_station_rename() {
        let anchor = TileCoord::new(2, 2);
        let mut state = GameState::new(8, 8);
        state
            .stations
            .push(imported_station(&mut state.map, anchor, 42));
        state
            .runtime
            .terminal_spatial_index
            .ensure_current(&state.map, &state.stations);

        state.step();
        state.step();
        crate::apply_command(
            &mut state,
            &Command::RenameStation {
                station_pos: anchor,
                name: Some("Terminal estable".to_owned()),
            },
        )
        .expect("renombrar no cambia topología");

        assert_eq!(state.runtime.terminal_spatial_index.at(anchor), &[0]);
        assert_eq!(state.runtime.terminal_spatial_index.rebuilds(), 1);
        assert_eq!(state.runtime.terminal_spatial_index.full_map_scans(), 1);
    }
}
