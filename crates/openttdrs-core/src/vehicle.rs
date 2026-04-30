use std::collections::VecDeque;

use crate::map::TileCoord;

/// Capacidad de carga por defecto (unidades de cargo).
pub const VEHICLE_CAPACITY: u32 = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum VehicleKind {
    Truck,
    /// Misma lógica de movimiento que camión; pensado para rutas sobre `TileKind::Rail`.
    Train,
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
    pub capacity: u32,
    /// Camino calculado por el pathfinder (siguiente tile en el frente).
    pub path: VecDeque<TileCoord>,
    /// Lista circular de destinos asignados por el jugador.
    #[serde(default)]
    pub orders: Vec<TileCoord>,
    #[serde(default)]
    pub current_order: usize,
}

impl Vehicle {
    #[must_use]
    pub fn new(id: u32, kind: VehicleKind, pos: TileCoord, dest: TileCoord) -> Self {
        Self {
            id,
            kind,
            pos,
            origin: pos,
            dest,
            cargo: 0,
            capacity: VEHICLE_CAPACITY,
            path: VecDeque::new(),
            orders: Vec::new(),
            current_order: 0,
        }
    }

    /// Avanza un paso: sigue el path BFS si está disponible; si no, Manhattan.
    /// Al llegar al destino invierte trayecto y vacía el path (se recomputa en `GameState`).
    pub fn step(&mut self) {
        if let Some(next) = self.path.pop_front() {
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
            if dx != 0 {
                self.pos.x += dx.signum();
            } else if dy != 0 {
                self.pos.y += dy.signum();
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
        self.orders = orders;
        self.current_order = 0;
        self.path.clear();
        if let Some(&first) = self.orders.first() {
            self.origin = self.pos;
            self.dest = first;
        }
    }

    fn advance_destination_after_arrival(&mut self) {
        self.path.clear();
        if self.orders.is_empty() {
            std::mem::swap(&mut self.dest, &mut self.origin);
            return;
        }
        self.current_order = (self.current_order + 1) % self.orders.len();
        self.origin = self.pos;
        self.dest = self.orders[self.current_order];
    }
}
