use std::collections::VecDeque;

use crate::map::TileCoord;

/// Capacidad de carga por defecto (unidades de cargo).
pub const VEHICLE_CAPACITY: u32 = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VehicleKind {
    Truck,
}

/// Vehículo que se desplaza tesela a tesela siguiendo un camino BFS.
///
/// Si no hay camino calculado (`path` vacío y `pos != dest`) usa movimiento Manhattan
/// como fallback para preservar compatibilidad con tests sin vías.
/// Al llegar invierte el trayecto (va y vuelve entre `origin` y `dest`).
#[derive(Debug, Clone)]
pub struct Vehicle {
    pub id:       u32,
    pub kind:     VehicleKind,
    pub pos:      TileCoord,
    /// Punto de partida del trayecto actual; se intercambia con `dest` en cada llegada.
    pub origin:   TileCoord,
    pub dest:     TileCoord,
    pub cargo:    u32,
    pub capacity: u32,
    /// Camino calculado por el pathfinder (siguiente tile en el frente).
    pub path:     VecDeque<TileCoord>,
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
        }
    }

    /// Avanza un paso: sigue el path BFS si está disponible; si no, Manhattan.
    /// Al llegar al destino invierte trayecto y vacía el path (se recomputa en GameState).
    pub fn step(&mut self) {
        if let Some(next) = self.path.pop_front() {
            self.pos = next;
            if self.pos == self.dest {
                std::mem::swap(&mut self.dest, &mut self.origin);
                // path ya está vacío; GameState recomputa el próximo tick
            }
        } else if self.pos == self.dest {
            // Llegada sin path (modo Manhattan): invertir
            std::mem::swap(&mut self.dest, &mut self.origin);
        } else {
            // Manhattan fallback: no hay vías en el mapa
            let dx = self.dest.x - self.pos.x;
            let dy = self.dest.y - self.pos.y;
            if dx != 0 {
                self.pos.x += dx.signum();
            } else if dy != 0 {
                self.pos.y += dy.signum();
            }
        }
    }

    #[must_use]
    pub fn manhattan_to_dest(&self) -> u32 {
        self.pos.x.abs_diff(self.dest.x) + self.pos.y.abs_diff(self.dest.y)
    }
}
