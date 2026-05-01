use std::collections::VecDeque;

use crate::cargo::CargoType;
use crate::map::TileCoord;

/// Capacidad de carga por defecto (unidades de cargo).
pub const VEHICLE_CAPACITY: u32 = 20;

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

/// Vehículo que se desplaza tesela a tesela siguiendo un camino BFS.
///
/// Si no hay camino calculado (`path` vacío y `pos != dest`) usa movimiento Manhattan
/// como fallback para preservar compatibilidad con tests sin vías.
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
    /// Camino calculado por el pathfinder (siguiente tile en el frente).
    pub path: VecDeque<TileCoord>,
    /// Lista circular de destinos asignados por el jugador.
    #[serde(default)]
    pub orders: Vec<VehicleOrder>,
    #[serde(default)]
    pub current_order: usize,
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
            path: VecDeque::new(),
            orders: Vec::new(),
            current_order: 0,
        }
    }

    /// Avanza un paso: sigue el path BFS si está disponible; si no, Manhattan.
    /// Al llegar al destino invierte trayecto y vacía el path (se recomputa en `GameState`).
    pub fn step(&mut self) {
        if !self.running {
            return;
        }
        if let Some(next) = self.path.pop_front() {
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
            // Manhattan fallback: no hay vías en el mapa
            let dx = self.dest.x - self.pos.x;
            let dy = self.dest.y - self.pos.y;
            let previous = self.pos;
            if dx != 0 {
                self.pos.x += dx.signum();
            } else if dy != 0 {
                self.pos.y += dy.signum();
            }
            if self.orders.is_empty() && self.pos != previous {
                self.origin = previous;
            }
            if self.pos == self.dest && !self.orders.is_empty() {
                self.advance_destination_after_arrival();
            }
        }
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
        if let Some(&first) = self.orders.first() {
            self.origin = self.pos;
            self.dest = first.destination();
        }
    }

    fn advance_destination_after_arrival(&mut self) {
        self.path.clear();
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
