use std::collections::VecDeque;

use crate::cargo::CargoType;
use crate::map::TileCoord;

/// Capacidad de carga por defecto (unidades de cargo).
pub const VEHICLE_CAPACITY: u32 = 20;

/// Avance sub-tile por tick de sim (`OpenTTD` usa `progress` 0–255 por tesela).
pub const VEHICLE_PROGRESS_STEP: u8 = 51;

/// `OpenTTD` `Direction`: N=0, NE=1, E=2, SE=3, S=4, SW=5, W=6, NW=7.
pub type VehicleDirection = u8;

pub const DIR_N: VehicleDirection = 0;
pub const DIR_NE: VehicleDirection = 1;
pub const DIR_E: VehicleDirection = 2;
pub const DIR_SE: VehicleDirection = 3;
pub const DIR_S: VehicleDirection = 4;
pub const DIR_SW: VehicleDirection = 5;
pub const DIR_W: VehicleDirection = 6;
pub const DIR_NW: VehicleDirection = 7;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum VehicleKind {
    Truck,
    Bus,
    /// Misma lógica de movimiento que camión; pensado para rutas sobre `TileKind::Rail`.
    Train,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum VehicleOrder {
    Station { station: TileCoord },
    Tile(TileCoord),
}

impl VehicleOrder {
    #[must_use]
    pub const fn destination(self) -> TileCoord {
        match self {
            Self::Station { station } | Self::Tile(station) => station,
        }
    }

    #[must_use]
    pub const fn station(station: TileCoord) -> Self {
        Self::Station { station }
    }

    #[must_use]
    pub const fn tile(tile: TileCoord) -> Self {
        Self::Tile(tile)
    }
}

/// Vehículo que avanza sub-tile (`progress` 0–255) siguiendo un camino BFS.
///
/// Si no hay camino calculado (`path` vacío y `pos != dest`) usa movimiento Manhattan
/// como fallback solo cuando **no hay órdenes** (vehículo libre / tests unitarios sin `GameState`).
/// Con órdenes activas, si no hay ruta por red (`no_network_route_to_order`) el vehículo no avanza.
/// Al llegar invierte el trayecto (va y vuelve entre `origin` y `dest`).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Vehicle {
    pub id: u32,
    pub kind: VehicleKind,
    pub pos: TileCoord,
    /// Punto de partida del trayecto actual; se intercambia con `dest` en cada llegada.
    pub origin: TileCoord,
    pub dest: TileCoord,
    pub cargo: u32,
    #[serde(default)]
    pub cargo_type: Option<CargoType>,
    pub capacity: u32,
    #[serde(default = "default_running_true")]
    pub running: bool,
    /// Progreso hacia la siguiente tesela del camino (0 = anclado en `pos`, 255 = llegada).
    #[serde(default)]
    pub progress: u8,
    /// Orientación gráfica (`OpenTTD` `Direction` 0..7).
    #[serde(default = "default_vehicle_direction")]
    pub direction: VehicleDirection,
    /// Camino calculado por el pathfinder (siguiente tile en el frente).
    pub path: VecDeque<TileCoord>,
    /// Lista circular de destinos asignados por el jugador.
    #[serde(default)]
    pub orders: Vec<VehicleOrder>,
    #[serde(default)]
    pub current_order: usize,
    /// Último intento de `find_path` falló estando `orders` no vacío; no usar Manhattan (queda bloqueado).
    #[serde(default)]
    pub no_network_route_to_order: bool,
}

impl Vehicle {
    #[must_use]
    pub fn new(id: u32, kind: VehicleKind, pos: TileCoord, dest: TileCoord) -> Self {
        let cargo_type = match kind {
            VehicleKind::Bus => Some(CargoType::Passengers),
            VehicleKind::Truck | VehicleKind::Train => None,
        };
        Self {
            id,
            kind,
            pos,
            origin: pos,
            dest,
            cargo: 0,
            cargo_type,
            capacity: VEHICLE_CAPACITY,
            running: true,
            progress: 0,
            direction: DIR_NE,
            path: VecDeque::new(),
            orders: Vec::new(),
            current_order: 0,
            no_network_route_to_order: false,
        }
    }

    /// Ticks de sim necesarios para cruzar una tesela con [`VEHICLE_PROGRESS_STEP`].
    #[must_use]
    pub const fn ticks_per_tile() -> u32 {
        255 / VEHICLE_PROGRESS_STEP as u32
    }

    /// Como `OpenTTD` `GetImage`: camión semi-lleno/lleno cambia sprite.
    #[must_use]
    pub fn uses_loaded_road_sprite(&self) -> bool {
        self.kind == VehicleKind::Truck && self.cargo >= self.capacity / 2
    }

    /// Dirección de sprite para render (8 vías; cardinales en la mitad de giros).
    #[must_use]
    pub fn render_direction(&self) -> VehicleDirection {
        let Some(next) = self.movement_target() else {
            return self.direction;
        };
        let entry = direction_from_tile_step(self.pos, next);
        if self.progress < 128 {
            return entry;
        }
        if let Some(&after) = self.path.get(1) {
            let exit = direction_from_tile_step(next, after);
            if exit != entry {
                return turn_cardinal_direction(entry, exit);
            }
        }
        entry
    }

    /// Siguiente tesela hacia la que avanza (path BFS o paso Manhattan).
    #[must_use]
    pub fn movement_target(&self) -> Option<TileCoord> {
        if !self.running {
            return None;
        }
        if let Some(&next) = self.path.front() {
            return Some(next);
        }
        if self.pos == self.dest {
            return None;
        }
        if !self.orders.is_empty() && self.no_network_route_to_order {
            return None;
        }
        let dx = self.dest.x - self.pos.x;
        let dy = self.dest.y - self.pos.y;
        if dx == 0 && dy == 0 {
            return None;
        }
        Some(if dx != 0 {
            TileCoord::new(self.pos.x + dx.signum(), self.pos.y)
        } else {
            TileCoord::new(self.pos.x, self.pos.y + dy.signum())
        })
    }

    /// Avanza un tick de sim: sub-tile y, al completar 255, la tesela siguiente.
    pub fn step(&mut self) {
        if !self.running {
            return;
        }

        if self.movement_target().is_none() {
            self.progress = 0;
            if self.pos == self.dest {
                self.advance_destination_after_arrival();
            }
            return;
        }

        self.progress = self.progress.saturating_add(VEHICLE_PROGRESS_STEP);
        if self.progress < 255 {
            return;
        }
        self.progress = 0;
        self.advance_one_tile();
    }

    fn advance_one_tile(&mut self) {
        if let Some(next) = self.path.pop_front() {
            self.update_direction_step(self.pos, next);
            if self.orders.is_empty() {
                self.origin = self.pos;
            }
            self.pos = next;
            if self.pos == self.dest {
                self.advance_destination_after_arrival();
            }
        } else if self.pos == self.dest {
            self.advance_destination_after_arrival();
        } else {
            if !self.orders.is_empty() && self.no_network_route_to_order {
                return;
            }
            let dx = self.dest.x - self.pos.x;
            let dy = self.dest.y - self.pos.y;
            let previous = self.pos;
            if dx != 0 {
                self.pos.x += dx.signum();
            } else if dy != 0 {
                self.pos.y += dy.signum();
            }
            if self.pos != previous {
                self.update_direction_step(previous, self.pos);
            }
            if self.orders.is_empty() && self.pos != previous {
                self.origin = previous;
            }
            if self.pos == self.dest && !self.orders.is_empty() {
                self.advance_destination_after_arrival();
            }
        }
    }

    fn update_direction_step(&mut self, from: TileCoord, to: TileCoord) {
        self.direction = direction_from_tile_step(from, to);
    }

    #[must_use]
    pub fn manhattan_to_dest(&self) -> u32 {
        self.pos.x.abs_diff(self.dest.x) + self.pos.y.abs_diff(self.dest.y)
    }

    pub fn set_orders(&mut self, orders: Vec<TileCoord>) {
        self.set_vehicle_orders(orders.into_iter().map(VehicleOrder::tile).collect());
    }

    pub fn set_station_orders(&mut self, stations: Vec<TileCoord>) {
        self.set_vehicle_orders(stations.into_iter().map(VehicleOrder::station).collect());
    }

    pub fn set_vehicle_orders(&mut self, orders: Vec<VehicleOrder>) {
        self.orders = orders;
        self.current_order = 0;
        self.path.clear();
        self.progress = 0;
        self.no_network_route_to_order = false;
        if let Some(&first) = self.orders.first() {
            self.origin = self.pos;
            self.dest = first.destination();
        }
    }

    fn advance_destination_after_arrival(&mut self) {
        self.path.clear();
        self.progress = 0;
        if self.orders.is_empty() {
            return;
        }
        self.current_order = (self.current_order + 1) % self.orders.len();
        self.origin = self.pos;
        self.dest = self.orders[self.current_order].destination();
    }
}

const fn default_running_true() -> bool {
    true
}

const fn default_vehicle_direction() -> VehicleDirection {
    DIR_NE
}

/// Dirección diagonal/cardinal desde un paso entre teselas adyacentes.
#[must_use]
pub fn direction_from_tile_step(from: TileCoord, to: TileCoord) -> VehicleDirection {
    match (to.x - from.x, to.y - from.y) {
        (-1, 0) => DIR_NE,
        (0, 1) => DIR_SE,
        (1, 0) => DIR_SW,
        (0, -1) => DIR_NW,
        _ => DIR_NE,
    }
}

/// Sprite cardinal intermedio al girar 90° entre dos diagonales.
#[must_use]
const fn turn_cardinal_direction(
    entry: VehicleDirection,
    exit: VehicleDirection,
) -> VehicleDirection {
    match (entry, exit) {
        (DIR_NE, DIR_SE) | (DIR_SE, DIR_NE) => DIR_E,
        (DIR_SE, DIR_SW) | (DIR_SW, DIR_SE) => DIR_S,
        (DIR_SW, DIR_NW | DIR_NE) | (DIR_NW | DIR_NE, DIR_SW) => DIR_W,
        (DIR_NW, DIR_NE | DIR_SE) | (DIR_NE | DIR_SE, DIR_NW) => DIR_N,
        _ => entry,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn progress_requires_multiple_ticks_per_tile() {
        let mut v = Vehicle::new(
            0,
            VehicleKind::Truck,
            TileCoord::new(0, 0),
            TileCoord::new(1, 0),
        );
        v.path = VecDeque::from([TileCoord::new(1, 0)]);
        for tick in 1..Vehicle::ticks_per_tile() {
            v.step();
            assert_eq!(v.pos, TileCoord::new(0, 0), "tick {tick}");
            assert!(v.progress > 0);
        }
        v.step();
        assert_eq!(v.pos, TileCoord::new(1, 0));
        assert_eq!(v.progress, 0);
    }

    #[test]
    fn render_direction_uses_cardinal_in_turn_second_half() {
        let mut v = Vehicle::new(
            0,
            VehicleKind::Bus,
            TileCoord::new(0, 0),
            TileCoord::new(1, 1),
        );
        v.path = VecDeque::from([TileCoord::new(0, 1), TileCoord::new(1, 1)]);
        v.progress = 200;
        assert_eq!(v.render_direction(), DIR_S);
    }

    #[test]
    fn direction_updates_when_tile_advances() {
        let mut v = Vehicle::new(
            0,
            VehicleKind::Truck,
            TileCoord::new(0, 0),
            TileCoord::new(1, 0),
        );
        v.path = VecDeque::from([TileCoord::new(1, 0)]);
        for _ in 0..Vehicle::ticks_per_tile() {
            v.step();
        }
        assert_eq!(v.direction, DIR_SW);
    }
}
