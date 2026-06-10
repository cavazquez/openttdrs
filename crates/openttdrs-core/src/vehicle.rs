use std::collections::VecDeque;

use crate::cargo::CargoType;
use crate::engine::{
    ROAD_ACCEL_ORIGINAL, decelerate_road_speed, default_engine_id, engine_for_vehicle,
    progress_step_for_speed, update_road_speed,
};
use crate::map::TileCoord;

/// Capacidad de carga por defecto (unidades de cargo).
pub const VEHICLE_CAPACITY: u32 = 20;

/// Paso sub-tile de referencia (bus MPS en diagonal). Ver [`crate::REFERENCE_PROGRESS_STEP`].
pub const VEHICLE_PROGRESS_STEP: u8 = crate::engine::REFERENCE_PROGRESS_STEP;

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
    /// Motor `OpenGFX` (`None` en saves antiguos → default por [`VehicleKind`]).
    #[serde(default)]
    pub engine_id: Option<u16>,
    /// Velocidad actual (unidades `OpenTTD`; 0 = parado).
    #[serde(default)]
    pub cur_speed: u16,
    /// Fracción sub-unidad de velocidad (`Vehicle::subspeed`).
    #[serde(default)]
    pub subspeed: u8,
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
    /// Tesela donde se cargó el lote actual (origen para pago por distancia).
    #[serde(default)]
    pub cargo_source: Option<TileCoord>,
    /// Ticks con carga a bordo (envejecimiento / penalización de pago).
    #[serde(default)]
    pub cargo_transit_ticks: u32,
    /// Giro de salida en la tesela actual (0 = inactivo; 1..=255 anima el cambio de sentido).
    #[serde(default)]
    pub depart_turn: u8,
}

impl Vehicle {
    #[must_use]
    pub fn new(id: u32, kind: VehicleKind, pos: TileCoord, dest: TileCoord) -> Self {
        let cargo_type = match kind {
            VehicleKind::Bus => Some(CargoType::Passengers),
            VehicleKind::Truck | VehicleKind::Train => None,
        };
        let engine_id = default_engine_id(kind);
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
            engine_id: Some(engine_id),
            cur_speed: 0,
            subspeed: 0,
            path: VecDeque::new(),
            orders: Vec::new(),
            current_order: 0,
            no_network_route_to_order: false,
            cargo_source: None,
            cargo_transit_ticks: 0,
            depart_turn: 0,
        }
    }

    pub(crate) fn mark_cargo_loaded(&mut self, at: TileCoord) {
        self.cargo_source = Some(at);
        self.cargo_transit_ticks = 0;
    }

    pub(crate) fn clear_cargo(&mut self) {
        self.cargo = 0;
        self.cargo_type = match self.kind {
            VehicleKind::Bus => Some(CargoType::Passengers),
            VehicleKind::Truck | VehicleKind::Train => None,
        };
        self.cargo_source = None;
        self.cargo_transit_ticks = 0;
    }

    #[must_use]
    pub fn effective_engine(&self) -> &'static crate::engine::EngineDef {
        engine_for_vehicle(
            self.kind,
            self.engine_id
                .unwrap_or_else(|| default_engine_id(self.kind)),
        )
    }

    #[must_use]
    pub fn effective_speed(&self) -> u16 {
        self.cur_speed
    }

    fn update_movement_speed(&mut self) {
        let max_speed = self.effective_engine().max_speed;
        if self.running && self.movement_target().is_some() {
            let (cur, sub) = update_road_speed(
                self.cur_speed,
                self.subspeed,
                ROAD_ACCEL_ORIGINAL,
                0,
                max_speed,
            );
            self.cur_speed = cur;
            self.subspeed = sub;
        } else {
            let (cur, sub) = decelerate_road_speed(self.cur_speed, self.subspeed);
            self.cur_speed = cur;
            self.subspeed = sub;
        }
    }

    /// Dirección del paso en curso (eje del carril / vía).
    #[must_use]
    pub fn movement_direction(&self) -> VehicleDirection {
        let Some(next) = self.movement_target() else {
            return self.direction;
        };
        direction_from_tile_step(self.pos, next)
    }

    /// Avance sub-tile por tick según motor y dirección.
    #[must_use]
    pub fn progress_step(&self) -> u8 {
        progress_step_for_speed(self.effective_speed(), self.movement_direction())
    }

    /// Ticks de sim estimados para cruzar una tesela en la dirección actual.
    #[must_use]
    pub fn ticks_per_tile(&self) -> u32 {
        let step = self.progress_step().max(1);
        255_u32.div_ceil(u32::from(step))
    }

    /// Como `OpenTTD` `GetImage`: semi-lleno/lleno cambia sprite en bus/camión.
    #[must_use]
    pub fn uses_loaded_road_sprite(&self) -> bool {
        if self.cargo < self.capacity / 2 {
            return false;
        }
        matches!(self.kind, VehicleKind::Bus | VehicleKind::Truck)
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
            self.update_movement_speed();
            self.progress = 0;
            return;
        }

        self.update_movement_speed();

        if self.kind == VehicleKind::Train {
            self.apply_immediate_train_turnaround();
        }

        if self.movement_target().is_none() {
            if self.cur_speed == 0 && self.pos == self.dest {
                self.advance_destination_after_arrival();
            }
            return;
        }

        if self.cur_speed == 0 {
            return;
        }

        if self.depart_turn > 0 {
            let step = u16::from(self.progress_step().max(1));
            let next = u16::from(self.depart_turn) + step;
            if next < 255 {
                if let Ok(t) = u8::try_from(next) {
                    self.depart_turn = t;
                }
            } else {
                self.depart_turn = 0;
                self.progress = 0;
                if let Some(next) = self.movement_target() {
                    self.direction = direction_from_tile_step(self.pos, next);
                }
            }
            return;
        }

        if self.progress == 255 && self.needs_depart_turnaround() {
            self.depart_turn = 1;
            return;
        }

        let step = u16::from(self.progress_step());
        let next = u16::from(self.progress) + step;
        if next < 255 {
            if let Ok(progress) = u8::try_from(next) {
                self.progress = progress;
            }
            return;
        }
        let mut remaining = next;
        loop {
            remaining = remaining.saturating_sub(255);
            self.progress = 0;
            self.advance_one_tile();
            if remaining < 255 {
                // Si `advance_destination_after_arrival` ancló en 255, no pisar con el resto.
                if self.progress != 255
                    && let Ok(progress) = u8::try_from(remaining)
                {
                    self.progress = progress;
                }
                return;
            }
            if self.movement_target().is_none() {
                return;
            }
        }
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
        self.depart_turn = 0;
        self.no_network_route_to_order = false;
        if let Some(&first) = self.orders.first() {
            self.origin = self.pos;
            if self.kind != VehicleKind::Train {
                self.dest = first.destination();
            }
        }
    }

    /// Velocidad de crucero inmediata (tests / saves legacy).
    pub fn set_cruise_speed(&mut self) {
        self.cur_speed = self.effective_engine().max_speed;
        self.subspeed = 0;
    }

    fn advance_destination_after_arrival(&mut self) {
        self.path.clear();
        self.depart_turn = 0;
        if self.orders.is_empty() {
            self.progress = 0;
            return;
        }
        // Anclado al final del carril de entrada (evita salto visual al llegar a parada/estación).
        self.progress = 255;
        self.current_order = (self.current_order + 1) % self.orders.len();
        self.origin = self.pos;
        if self.kind != VehicleKind::Train {
            self.dest = self.orders[self.current_order].destination();
        }
    }

    /// Actualiza `dest` según la orden actual (vía adyacente para estaciones de tren).
    pub fn sync_order_destination(&mut self, map: &crate::map::Map) {
        if self.orders.is_empty() {
            return;
        }
        let order = self.orders[self.current_order];
        self.dest = crate::station::resolve_order_destination(map, self.kind, order);
    }

    /// Salida con sentido opuesto al de llegada (giro animado en parada bus/camión).
    #[must_use]
    pub(crate) fn needs_depart_turnaround(&self) -> bool {
        if self.kind == VehicleKind::Train {
            return false;
        }
        let Some(next) = self.movement_target() else {
            return false;
        };
        let outbound = direction_from_tile_step(self.pos, next);
        outbound == reverse_direction(self.direction)
    }

    /// Tren: invierte el rumbo en el acto si la siguiente tesela exige sentido opuesto.
    fn apply_immediate_train_turnaround(&mut self) {
        let Some(next) = self.movement_target() else {
            return;
        };
        let outbound = direction_from_tile_step(self.pos, next);
        if outbound != reverse_direction(self.direction) {
            return;
        }
        self.direction = outbound;
        self.depart_turn = 0;
        if self.progress == 255 {
            self.progress = 0;
        }
    }

    /// Invierte el sentido de marcha (depósito / tests).
    pub fn reverse_heading(&mut self) {
        self.direction = reverse_direction(self.direction);
        self.progress = 0;
        self.depart_turn = 0;
    }
}

/// Sentido opuesto en la rosa de 8 direcciones `OpenTTD`.
#[must_use]
pub const fn reverse_direction(d: VehicleDirection) -> VehicleDirection {
    (d + 4) % 8
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
        v.set_cruise_speed();
        let ticks = v.ticks_per_tile();
        for tick in 1..ticks {
            v.step();
            assert_eq!(v.pos, TileCoord::new(0, 0), "tick {tick}");
            assert!(v.progress > 0);
        }
        v.step();
        assert_eq!(v.pos, TileCoord::new(1, 0));
        assert!(v.progress < v.progress_step());
    }

    #[test]
    fn train_reverses_immediately_when_next_tile_opposite() {
        let mut v = Vehicle::new(
            0,
            VehicleKind::Train,
            TileCoord::new(21, 15),
            TileCoord::new(21, 15),
        );
        v.path = VecDeque::from([TileCoord::new(20, 15)]);
        v.direction = DIR_SW;
        v.progress = 255;
        v.cur_speed = 0;
        v.step();
        assert_eq!(v.direction, DIR_NE, "giro inmediato al volver por la vía");
        assert_eq!(v.progress, 0);
    }

    #[test]
    fn arrival_at_order_keeps_progress_at_lane_end() {
        let mut v = Vehicle::new(
            0,
            VehicleKind::Bus,
            TileCoord::new(15, 4),
            TileCoord::new(15, 3),
        );
        v.set_station_orders(vec![TileCoord::new(15, 3), TileCoord::new(21, 3)]);
        v.path = VecDeque::from([TileCoord::new(15, 3)]);
        v.direction = DIR_NW;
        v.set_cruise_speed();
        v.progress = 250;
        v.step();
        assert_eq!(v.pos, TileCoord::new(15, 3));
        assert_eq!(v.progress, 255, "anclado al final del carril al llegar");
    }

    #[test]
    fn vehicle_accelerates_from_standstill_before_moving() {
        let mut v = Vehicle::new(
            0,
            VehicleKind::Bus,
            TileCoord::new(0, 0),
            TileCoord::new(1, 0),
        );
        v.path = VecDeque::from([TileCoord::new(1, 0)]);
        assert_eq!(v.cur_speed, 0);
        v.step();
        assert_eq!(v.pos, TileCoord::new(0, 0));
        assert!(v.cur_speed > 0);
        assert_eq!(v.progress, 0);
    }

    #[test]
    fn vehicle_decelerates_when_idle() {
        let mut v = Vehicle::new(
            0,
            VehicleKind::Truck,
            TileCoord::new(2, 2),
            TileCoord::new(2, 2),
        );
        v.cur_speed = 96;
        v.subspeed = 0;
        for _ in 0..160 {
            v.step();
            if v.cur_speed == 0 {
                break;
            }
        }
        assert_eq!(v.cur_speed, 0);
        assert_eq!(v.subspeed, 0);
    }

    #[test]
    fn loaded_sprite_for_bus_and_truck() {
        let mut bus = Vehicle::new(
            0,
            VehicleKind::Bus,
            TileCoord::new(0, 0),
            TileCoord::new(1, 0),
        );
        assert!(!bus.uses_loaded_road_sprite());
        bus.cargo = VEHICLE_CAPACITY / 2;
        assert!(bus.uses_loaded_road_sprite());
        let mut truck = Vehicle::new(
            1,
            VehicleKind::Truck,
            TileCoord::new(0, 0),
            TileCoord::new(1, 0),
        );
        truck.cargo = VEHICLE_CAPACITY / 2;
        assert!(truck.uses_loaded_road_sprite());
    }

    #[test]
    fn train_moves_slower_than_bus_on_same_path() {
        let mut bus = Vehicle::new(
            0,
            VehicleKind::Bus,
            TileCoord::new(0, 0),
            TileCoord::new(3, 0),
        );
        bus.path = VecDeque::from([
            TileCoord::new(1, 0),
            TileCoord::new(2, 0),
            TileCoord::new(3, 0),
        ]);
        let mut train = Vehicle::new(
            1,
            VehicleKind::Train,
            TileCoord::new(0, 0),
            TileCoord::new(3, 0),
        );
        train.path = bus.path.clone();
        bus.set_cruise_speed();
        train.set_cruise_speed();

        let bus_ticks = bus.ticks_per_tile();
        let train_ticks = train.ticks_per_tile();
        assert!(train_ticks > bus_ticks);

        let mut bus_steps = 0;
        while bus.pos.x < 1 {
            bus.step();
            bus_steps += 1;
        }
        let mut train_steps = 0;
        while train.pos.x < 1 {
            train.step();
            train_steps += 1;
        }
        assert!(train_steps > bus_steps);
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
        v.set_cruise_speed();
        for _ in 0..v.ticks_per_tile() {
            v.step();
        }
        assert_eq!(v.direction, DIR_SW);
    }
}
