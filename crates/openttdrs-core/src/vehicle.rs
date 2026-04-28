use crate::map::TileCoord;

/// Capacidad de carga por defecto (unidades de cargo).
pub const VEHICLE_CAPACITY: u32 = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VehicleKind {
    Truck,
}

/// Vehículo que se desplaza tesela a tesela en dirección cardinal hacia su destino.
///
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
}

impl Vehicle {
    #[must_use]
    pub fn new(id: u32, kind: VehicleKind, pos: TileCoord, dest: TileCoord) -> Self {
        Self { id, kind, pos, origin: pos, dest, cargo: 0, capacity: VEHICLE_CAPACITY }
    }

    /// Avanza una tesela en la dirección cardinal que reduce la distancia Manhattan al destino.
    /// Prioriza el eje X; si está en el mismo X, mueve en Y.
    /// Al llegar intercambia `dest` y `origin` para invertir el trayecto.
    pub fn step(&mut self) {
        if self.pos == self.dest {
            std::mem::swap(&mut self.dest, &mut self.origin);
            return;
        }
        let dx = self.dest.x - self.pos.x;
        let dy = self.dest.y - self.pos.y;
        if dx != 0 {
            self.pos.x += dx.signum();
        } else {
            self.pos.y += dy.signum();
        }
    }

    #[must_use]
    pub fn manhattan_to_dest(&self) -> u32 {
        self.pos.x.abs_diff(self.dest.x) + self.pos.y.abs_diff(self.dest.y)
    }
}
