use std::collections::VecDeque;

use crate::cargo::CargoType;
use crate::engine::{
    ROAD_ACCEL_ORIGINAL, accelerate_train_speed, decelerate_road_speed, decelerate_train_speed,
    default_engine_id, engine_for_vehicle, progress_step_for_speed, train_acceleration,
    update_road_speed,
};
use crate::map::TileCoord;
use crate::train_movement::{ACCEL_SLOWDOWN, is_45_degree_turn};

/// Umbral de fiabilidad bajo el cual conviene servicio en depósito.
pub const SERVICING_RELIABILITY_THRESHOLD: u16 = 5_000;
/// Duración de una avería (~3 días de calendario).
pub const BREAKDOWN_DURATION_TICKS: u32 = crate::economy::TICKS_PER_TRANSIT_DAY * 3;

const fn default_vehicle_reliability() -> u16 {
    8_500
}

fn initial_reliability_for_engine(engine_id: u16, kind: VehicleKind) -> u16 {
    u16::from(crate::engine::engine_for_vehicle(kind, engine_id).reliability_pct) * 100
}

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
    /// Navega por teselas de agua (`TileKind::Water`).
    Ship,
    /// Vuela en línea recta entre origen y destino (ignora terreno).
    Aircraft,
}

/// Fase de vuelo MVP (aviones).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum AircraftPhase {
    #[default]
    InHangar,
    Taxi,
    Takeoff,
    Flying,
    Landing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum VehicleOrder {
    Station {
        station: TileCoord,
        /// Esperar carga completa antes de salir (`OrderLoadType::FullLoad` / `FullLoadAny`).
        #[serde(default)]
        full_load: bool,
        /// No descargar en esta parada (`OrderUnloadType::NoUnload`).
        #[serde(default)]
        no_unload: bool,
        /// Espera mínima en parada con horario activo (ticks de sim).
        #[serde(default)]
        wait_ticks: u32,
        /// Tiempo mínimo de viaje hasta esta orden (ticks desde la anterior).
        #[serde(default)]
        travel_ticks: u32,
    },
    Waypoint {
        waypoint: TileCoord,
        #[serde(default)]
        travel_ticks: u32,
    },
    /// Parada en depósito (`stop`: detener al llegar; `false` = pasar sin parar).
    Depot {
        depot: TileCoord,
        #[serde(default = "default_depot_stop")]
        stop: bool,
        #[serde(default)]
        wait_ticks: u32,
        #[serde(default)]
        travel_ticks: u32,
    },
    Tile(TileCoord),
    /// Salta a `jump_to` si la condición se cumple al llegar a esta «orden».
    Conditional {
        condition: OrderConditionKind,
        value: u8,
        jump_to: usize,
    },
}

/// Condición de orden condicional (MVP).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderConditionKind {
    CargoLoadAbove,
    CargoLoadBelow,
}

impl VehicleOrder {
    #[must_use]
    pub const fn destination(self) -> TileCoord {
        match self {
            Self::Station { station, .. } => station,
            Self::Waypoint { waypoint, .. } => waypoint,
            Self::Depot { depot, .. } => depot,
            Self::Tile(pos) => pos,
            Self::Conditional { .. } => TileCoord::new(0, 0),
        }
    }

    #[must_use]
    pub const fn conditional(condition: OrderConditionKind, value: u8, jump_to: usize) -> Self {
        Self::Conditional {
            condition,
            value,
            jump_to,
        }
    }

    #[must_use]
    pub const fn is_conditional(self) -> bool {
        matches!(self, Self::Conditional { .. })
    }

    #[must_use]
    pub fn evaluate_conditional(self, vehicle: &Vehicle) -> usize {
        let Self::Conditional {
            condition,
            value,
            jump_to,
        } = self
        else {
            return vehicle.current_order;
        };
        let pct = if vehicle.capacity == 0 {
            0
        } else {
            u8::try_from((u64::from(vehicle.cargo) * 100 / u64::from(vehicle.capacity)).min(100))
                .unwrap_or(100)
        };
        let ok = match condition {
            OrderConditionKind::CargoLoadAbove => pct > value,
            OrderConditionKind::CargoLoadBelow => pct < value,
        };
        if ok {
            jump_to
        } else if vehicle.orders.is_empty() {
            0
        } else {
            (vehicle.current_order + 1) % vehicle.orders.len()
        }
    }

    #[must_use]
    pub const fn station(station: TileCoord) -> Self {
        Self::Station {
            station,
            full_load: false,
            no_unload: false,
            wait_ticks: 0,
            travel_ticks: 0,
        }
    }

    #[must_use]
    pub const fn station_with_flags(station: TileCoord, full_load: bool, no_unload: bool) -> Self {
        Self::Station {
            station,
            full_load,
            no_unload,
            wait_ticks: 0,
            travel_ticks: 0,
        }
    }

    #[must_use]
    pub const fn waypoint(waypoint: TileCoord) -> Self {
        Self::Waypoint {
            waypoint,
            travel_ticks: 0,
        }
    }

    #[must_use]
    pub const fn depot(depot: TileCoord) -> Self {
        Self::Depot {
            depot,
            stop: true,
            wait_ticks: 0,
            travel_ticks: 0,
        }
    }

    #[must_use]
    pub const fn depot_pass_through(depot: TileCoord) -> Self {
        Self::Depot {
            depot,
            stop: false,
            wait_ticks: 0,
            travel_ticks: 0,
        }
    }

    #[must_use]
    pub const fn tile(tile: TileCoord) -> Self {
        Self::Tile(tile)
    }

    #[must_use]
    pub const fn is_depot(self) -> bool {
        matches!(self, Self::Depot { .. })
    }

    #[must_use]
    pub const fn depot_stops(self) -> bool {
        matches!(self, Self::Depot { stop: true, .. })
    }

    #[must_use]
    pub const fn is_pass_through(self) -> bool {
        matches!(
            self,
            Self::Waypoint { .. } | Self::Depot { stop: false, .. }
        )
    }

    #[must_use]
    pub const fn full_load(self) -> bool {
        matches!(
            self,
            Self::Station {
                full_load: true,
                ..
            }
        )
    }

    #[must_use]
    pub const fn no_unload(self) -> bool {
        matches!(
            self,
            Self::Station {
                no_unload: true,
                ..
            }
        )
    }

    /// Alterna «carga completa» en una parada de estación.
    #[must_use]
    pub fn with_toggled_full_load(self) -> Option<Self> {
        match self {
            Self::Station {
                station,
                full_load,
                no_unload,
                wait_ticks,
                travel_ticks,
            } => Some(Self::Station {
                station,
                full_load: !full_load,
                no_unload,
                wait_ticks,
                travel_ticks,
            }),
            _ => None,
        }
    }

    /// Alterna «no descargar» en una parada de estación.
    #[must_use]
    pub fn with_toggled_no_unload(self) -> Option<Self> {
        match self {
            Self::Station {
                station,
                full_load,
                no_unload,
                wait_ticks,
                travel_ticks,
            } => Some(Self::Station {
                station,
                full_load,
                no_unload: !no_unload,
                wait_ticks,
                travel_ticks,
            }),
            _ => None,
        }
    }

    /// Alterna «parar en depósito» en una orden de depósito.
    #[must_use]
    pub fn with_toggled_depot_stop(self) -> Option<Self> {
        match self {
            Self::Depot {
                depot,
                stop,
                wait_ticks,
                travel_ticks,
            } => Some(Self::Depot {
                depot,
                stop: !stop,
                wait_ticks,
                travel_ticks,
            }),
            _ => None,
        }
    }

    #[must_use]
    pub const fn wait_ticks(self) -> u32 {
        match self {
            Self::Station { wait_ticks, .. } | Self::Depot { wait_ticks, .. } => wait_ticks,
            _ => 0,
        }
    }

    #[must_use]
    pub const fn travel_ticks(self) -> u32 {
        match self {
            Self::Station { travel_ticks, .. }
            | Self::Waypoint { travel_ticks, .. }
            | Self::Depot { travel_ticks, .. } => travel_ticks,
            Self::Tile(_) | Self::Conditional { .. } => 0,
        }
    }

    #[must_use]
    pub fn with_cycled_wait(self) -> Option<Self> {
        use crate::timetable::cycle_wait_ticks;
        match self {
            Self::Station {
                station,
                full_load,
                no_unload,
                wait_ticks,
                travel_ticks,
            } => Some(Self::Station {
                station,
                full_load,
                no_unload,
                wait_ticks: cycle_wait_ticks(wait_ticks),
                travel_ticks,
            }),
            Self::Depot {
                depot,
                stop,
                wait_ticks,
                travel_ticks,
            } => Some(Self::Depot {
                depot,
                stop,
                wait_ticks: cycle_wait_ticks(wait_ticks),
                travel_ticks,
            }),
            _ => None,
        }
    }

    #[must_use]
    pub fn with_cycled_travel(self) -> Self {
        use crate::timetable::cycle_travel_ticks;
        match self {
            Self::Station {
                station,
                full_load,
                no_unload,
                wait_ticks,
                travel_ticks,
            } => Self::Station {
                station,
                full_load,
                no_unload,
                wait_ticks,
                travel_ticks: cycle_travel_ticks(travel_ticks),
            },
            Self::Waypoint {
                waypoint,
                travel_ticks,
            } => Self::Waypoint {
                waypoint,
                travel_ticks: cycle_travel_ticks(travel_ticks),
            },
            Self::Depot {
                depot,
                stop,
                wait_ticks,
                travel_ticks,
            } => Self::Depot {
                depot,
                stop,
                wait_ticks,
                travel_ticks: cycle_travel_ticks(travel_ticks),
            },
            Self::Tile(t) => Self::Tile(t),
            Self::Conditional {
                condition,
                value,
                jump_to,
            } => Self::Conditional {
                condition,
                value,
                jump_to,
            },
        }
    }

    #[must_use]
    pub fn with_wait_ticks(self, wait_ticks: u32) -> Option<Self> {
        match self {
            Self::Station {
                station,
                full_load,
                no_unload,
                travel_ticks,
                ..
            } => Some(Self::Station {
                station,
                full_load,
                no_unload,
                wait_ticks,
                travel_ticks,
            }),
            Self::Depot {
                depot,
                stop,
                travel_ticks,
                ..
            } => Some(Self::Depot {
                depot,
                stop,
                wait_ticks,
                travel_ticks,
            }),
            _ => None,
        }
    }

    #[must_use]
    pub fn with_travel_ticks(self, travel_ticks: u32) -> Self {
        match self {
            Self::Station {
                station,
                full_load,
                no_unload,
                wait_ticks,
                ..
            } => Self::Station {
                station,
                full_load,
                no_unload,
                wait_ticks,
                travel_ticks,
            },
            Self::Waypoint { waypoint, .. } => Self::Waypoint {
                waypoint,
                travel_ticks,
            },
            Self::Depot {
                depot,
                stop,
                wait_ticks,
                ..
            } => Self::Depot {
                depot,
                stop,
                wait_ticks,
                travel_ticks,
            },
            other => other,
        }
    }
}

/// Nombre personalizado del jugador (`OpenTTD` `MAX_LENGTH_VEHICLE_NAME_CHARS` = 32).
pub const MAX_VEHICLE_NAME_CHARS: usize = 32;

/// Motivo de una espera de horario en curso.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum TimetableWaitKind {
    #[default]
    None,
    AfterArrival,
    AfterUnload,
    AfterLoad,
    TravelEarly,
}

/// Vehículo que avanza sub-tile (`progress` 0–255) siguiendo un camino BFS.
///
/// Si no hay camino calculado (`path` vacío y `pos != dest`) usa movimiento Manhattan
/// como fallback solo cuando **no hay órdenes** (vehículo libre / tests unitarios sin `GameState`).
/// Con órdenes activas, si no hay ruta por red (`no_network_route_to_order`) el vehículo no avanza.
/// Los trenes nunca usan el fallback Manhattan: sin ruta por vía no se mueven.
/// Al llegar invierte el trayecto (va y vuelve entre `origin` y `dest`).
#[allow(clippy::struct_excessive_bools)]
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
    /// Nombre personalizado; si falta, la UI usa modelo + id.
    #[serde(default)]
    pub name: Option<String>,
    /// Velocidad actual (unidades `OpenTTD`; 0 = parado).
    #[serde(default)]
    pub cur_speed: u16,
    /// Fracción sub-unidad de velocidad (`Vehicle::subspeed`).
    #[serde(default)]
    pub subspeed: u8,
    /// Pasos de vía reservados por PBS (`tesela` + `TrackBit`).
    #[serde(default)]
    pub reserved_steps: Vec<crate::rail_pbs::ReservedRailStep>,
    /// Historial reciente de teselas (cabeza → atrás) para huella PBS del consist.
    #[serde(default)]
    pub rail_tile_history: VecDeque<TileCoord>,
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
    /// Packets a bordo (fuente de verdad Fase 2); `cargo`/`cargo_source` se sincronizan.
    #[serde(default)]
    pub cargo_packets: crate::cargo_packet::VehicleCargoList,
    /// Carga gradual en curso (no avanzar orden hasta terminar o `full_load`).
    #[serde(default)]
    pub cargo_loading: bool,
    /// Descarga gradual en curso.
    #[serde(default)]
    pub cargo_unloading: bool,
    /// Giro de salida en la tesela actual (0 = inactivo; 1..=255 anima el cambio de sentido).
    #[serde(default)]
    pub depart_turn: u8,
    /// Llegó a una orden de estación y espera la ventana de carga/descarga del
    /// siguiente tick (análogo a `OpenTTD` `Vehicle::BeginLoading`: la orden
    /// no avanza hasta que la fase de carga tuvo oportunidad de actuar).
    #[serde(default)]
    pub awaiting_load_window: bool,
    /// Ignorar señal roja en el próximo paso de simulación (trenes).
    #[serde(default)]
    pub force_proceed: bool,
    /// Contador de ticks esperando path PBS / señal (`wait_counter` en `OpenTTD`).
    #[serde(default)]
    pub wait_counter: u32,
    /// Tren marcado stuck ante path sin reserva (`VehicleRailFlag::Stuck`).
    #[serde(default)]
    pub pbs_stuck: bool,
    /// Horario activo para este vehículo.
    #[serde(default)]
    pub timetable_active: bool,
    /// Ticks restantes de espera en la parada actual.
    #[serde(default)]
    pub timetable_wait_remaining: u32,
    #[serde(default)]
    pub timetable_wait_kind: TimetableWaitKind,
    /// Tick de simulación al salir de la orden anterior (viaje mínimo).
    #[serde(default)]
    pub timetable_leg_start_tick: u64,
    /// Autoreemplazo ya intentado en esta parada en depósito.
    #[serde(default)]
    pub autoreplace_attempted_this_stop: bool,
    /// Tick de simulación en que se compró el vehículo.
    #[serde(default)]
    pub build_tick: u64,
    /// Grupo de flota opcional.
    #[serde(default)]
    pub group_id: Option<u32>,
    /// Pool de órdenes compartidas enlazado.
    #[serde(default)]
    pub shared_order_id: Option<u32>,
    /// Retraso acumulado del horario (ticks; positivo = tarde).
    #[serde(default)]
    pub timetable_lateness: i32,
    /// Autofill: medir ciclos y rellenar tiempos.
    #[serde(default)]
    pub timetable_autofill: bool,
    /// Muestras de autofill por índice de orden `(wait, travel)`.
    #[serde(default)]
    pub timetable_autofill_samples: Vec<(u32, u32)>,
    /// Orden visual en ventana de depósito.
    #[serde(default)]
    pub depot_display_slot: Option<u8>,
    /// Modo de visualización del horario (persistido por partida).
    #[serde(default)]
    pub timetable_display_seconds: bool,
    /// Tick actual (efímero; no se guarda).
    #[serde(skip, default)]
    pub(crate) sim_tick: u64,
    /// Fiabilidad actual (0..=10 000 ≈ porcentaje × 100; desde motor al comprar).
    #[serde(default = "default_vehicle_reliability")]
    pub reliability: u16,
    /// Fiabilidad por debajo del umbral de servicio recomendado.
    #[serde(default)]
    pub needs_servicing: bool,
    /// Ticks restantes de avería (vehículo parado).
    #[serde(default)]
    pub breakdown_ticks_remaining: u32,
    /// Fase de vuelo (solo aviones; resto ignora).
    #[serde(default)]
    pub aircraft_phase: AircraftPhase,
    /// Altitud de vuelo (0 = suelo; ~8 = crucero). Offset visual Z.
    #[serde(default)]
    pub altitude: u8,
    /// Ticks restantes en fase Takeoff/Landing.
    #[serde(default)]
    pub aircraft_phase_ticks: u16,
    /// Siguiente unidad del consist (`OpenTTD` `Next()`); solo trenes.
    #[serde(default)]
    pub next_unit: Option<u32>,
    /// Unidad anterior del consist; `None` = cabeza (front engine).
    #[serde(default)]
    pub prev_unit: Option<u32>,
    /// Longitud de esta unidad en fracciones (`VEHICLE_LENGTH` = 8).
    #[serde(default = "default_unit_length")]
    pub unit_length: u8,
    /// Longitud total del consist en fracciones (solo válida en la cabeza).
    #[serde(default = "default_cached_total_length")]
    pub cached_total_length: u16,
    /// Potencia agregada del consist (HP); cabeza.
    #[serde(default)]
    pub cached_power_hp: u32,
    /// Peso agregado del consist (t); cabeza.
    #[serde(default)]
    pub cached_weight_t: u16,
}

fn default_unit_length() -> u8 {
    crate::train_consist::VEHICLE_LENGTH
}

fn default_cached_total_length() -> u16 {
    u16::from(crate::train_consist::VEHICLE_LENGTH)
}

impl Vehicle {
    #[must_use]
    pub fn new(id: u32, kind: VehicleKind, pos: TileCoord, dest: TileCoord) -> Self {
        let cargo_type = match kind {
            VehicleKind::Bus | VehicleKind::Aircraft => Some(CargoType::Passengers),
            VehicleKind::Truck | VehicleKind::Train | VehicleKind::Ship => None,
        };
        let engine_id = default_engine_id(kind);
        let reliability = initial_reliability_for_engine(engine_id, kind);
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
            name: None,
            cur_speed: 0,
            subspeed: 0,
            path: VecDeque::new(),
            reserved_steps: Vec::new(),
            rail_tile_history: VecDeque::new(),
            orders: Vec::new(),
            current_order: 0,
            no_network_route_to_order: false,
            cargo_source: None,
            cargo_transit_ticks: 0,
            cargo_packets: crate::cargo_packet::VehicleCargoList::default(),
            cargo_loading: false,
            cargo_unloading: false,
            depart_turn: 0,
            awaiting_load_window: false,
            force_proceed: false,
            wait_counter: 0,
            pbs_stuck: false,
            timetable_active: false,
            timetable_wait_remaining: 0,
            timetable_wait_kind: TimetableWaitKind::None,
            timetable_leg_start_tick: 0,
            autoreplace_attempted_this_stop: false,
            build_tick: 0,
            group_id: None,
            shared_order_id: None,
            timetable_lateness: 0,
            timetable_autofill: false,
            timetable_autofill_samples: Vec::new(),
            depot_display_slot: None,
            timetable_display_seconds: false,
            sim_tick: 0,
            reliability,
            needs_servicing: false,
            breakdown_ticks_remaining: 0,
            aircraft_phase: AircraftPhase::InHangar,
            altitude: 0,
            aircraft_phase_ticks: 0,
            next_unit: None,
            prev_unit: None,
            unit_length: crate::train_consist::VEHICLE_LENGTH,
            cached_total_length: u16::from(crate::train_consist::VEHICLE_LENGTH),
            cached_power_hp: 0,
            cached_weight_t: 0,
        }
    }

    /// ¿Es la cabeza del consist (o no es tren)?
    #[must_use]
    pub fn is_consist_head(&self) -> bool {
        self.kind != VehicleKind::Train || self.prev_unit.is_none()
    }

    /// ¿Es un vagón enganchado (no cabeza)?
    #[must_use]
    pub fn is_wagon_unit(&self) -> bool {
        self.kind == VehicleKind::Train && self.prev_unit.is_some()
    }

    /// Restaura fiabilidad tras servicio en depósito.
    pub fn service_at_depot(&mut self) {
        let engine_id = self
            .engine_id
            .unwrap_or_else(|| default_engine_id(self.kind));
        self.reliability = initial_reliability_for_engine(engine_id, self.kind);
        self.needs_servicing = false;
        self.breakdown_ticks_remaining = 0;
    }

    /// Comprueba avería durante el movimiento; devuelve `true` si acaba de averiarse.
    pub fn check_breakdown(&mut self, tick: u64) -> bool {
        if self.breakdown_ticks_remaining > 0 {
            self.breakdown_ticks_remaining = self.breakdown_ticks_remaining.saturating_sub(1);
            self.cur_speed = 0;
            return false;
        }
        if !self.running || self.cur_speed == 0 {
            return false;
        }
        if tick.is_multiple_of(256) {
            self.reliability = self.reliability.saturating_sub(10);
            self.needs_servicing = self.reliability < SERVICING_RELIABILITY_THRESHOLD;
        }
        if self.reliability >= 4_000 {
            return false;
        }
        let chance = (tick.wrapping_mul(u64::from(self.id.wrapping_add(1))) % 256) as u32;
        if chance != 0 {
            return false;
        }
        self.breakdown_ticks_remaining = BREAKDOWN_DURATION_TICKS;
        self.cur_speed = 0;
        true
    }

    /// Edad del vehículo en años de calendario aproximados.
    #[must_use]
    pub fn vehicle_age_years(&self, current_tick: u64) -> u32 {
        let age_ticks = current_tick.saturating_sub(self.build_tick);
        u32::try_from(age_ticks / crate::economy::TICKS_PER_YEAR).unwrap_or(u32::MAX)
    }

    /// Umbral simplificado: últimos ~20 % de vida útil (25 años).
    #[must_use]
    pub fn needs_autorenewing(&self, current_tick: u64) -> bool {
        self.vehicle_age_years(current_tick) >= 20
    }

    pub(crate) fn resolve_conditional_orders(&mut self) {
        const MAX_STEPS: usize = 64;
        for _ in 0..MAX_STEPS {
            let Some(order) = self.orders.get(self.current_order).copied() else {
                break;
            };
            if !order.is_conditional() {
                break;
            }
            self.current_order = order.evaluate_conditional(self);
            self.path.clear();
            self.progress = 0;
        }
    }

    fn record_timetable_autofill_sample(&mut self, wait: u32, travel: u32) {
        if !self.timetable_autofill {
            return;
        }
        let idx = self.current_order;
        if self.timetable_autofill_samples.len() <= idx {
            self.timetable_autofill_samples.resize(idx + 1, (0, 0));
        }
        let (w, t) = &mut self.timetable_autofill_samples[idx];
        *w = u32::midpoint(*w, wait);
        *t = u32::midpoint(*t, travel);
        if let Some(order) = self.orders.get_mut(idx) {
            *order = order.with_travel_ticks(*t);
            if let Some(updated) = order.with_wait_ticks(*w) {
                *order = updated;
            }
        }
    }

    fn update_timetable_lateness_on_wait_end(&mut self, planned_wait: u32) {
        if !self.timetable_active {
            return;
        }
        let delta = i32::try_from(planned_wait).unwrap_or(i32::MAX);
        self.timetable_lateness = self.timetable_lateness.saturating_sub(delta);
    }

    pub(crate) fn mark_cargo_loaded(&mut self, at: TileCoord) {
        if self.cargo_source.is_none() {
            self.cargo_source = Some(at);
        }
        // No resetear transit si ya hay packets a bordo (carga gradual / top-up).
        if self.cargo_packets.is_empty() {
            self.cargo_transit_ticks = 0;
        }
    }

    /// Sincroniza campos agregados desde `cargo_packets`.
    pub fn sync_cargo_from_packets(&mut self) {
        self.cargo = self.cargo_packets.total();
        if self.cargo == 0 {
            self.cargo_type = match self.kind {
                VehicleKind::Bus | VehicleKind::Aircraft => Some(CargoType::Passengers),
                VehicleKind::Truck | VehicleKind::Train | VehicleKind::Ship => None,
            };
            self.cargo_source = None;
            self.cargo_transit_ticks = 0;
            return;
        }
        if let Some(ct) = self.cargo_packets.primary_type() {
            self.cargo_type = Some(ct);
        }
        self.cargo_source = self.cargo_packets.primary_source();
        let days = self.cargo_packets.max_periods_in_transit();
        self.cargo_transit_ticks =
            u32::from(days).saturating_mul(crate::economy::TICKS_PER_TRANSIT_DAY);
    }

    /// Hidrata packets desde campos legacy si la lista está vacía.
    pub fn ensure_packets_from_legacy(&mut self) {
        if self.cargo_packets.is_empty() && self.cargo > 0 {
            let days = crate::economy::ticks_to_transit_days(self.cargo_transit_ticks);
            let cargo_type = self.cargo_type.or(match self.kind {
                VehicleKind::Bus | VehicleKind::Aircraft => Some(CargoType::Passengers),
                VehicleKind::Truck | VehicleKind::Train | VehicleKind::Ship => {
                    Some(CargoType::Goods)
                }
            });
            self.cargo_packets = crate::cargo_packet::VehicleCargoList::from_legacy(
                self.cargo,
                cargo_type,
                self.cargo_source,
                days,
                self.pos,
            );
        }
        self.sync_cargo_from_packets();
    }

    /// ¿Hay transferencia gradual (carga o descarga) en curso?
    #[must_use]
    pub fn cargo_transfer_active(&self) -> bool {
        self.cargo_loading || self.cargo_unloading
    }

    pub(crate) fn clear_cargo(&mut self) {
        self.cargo_packets.clear();
        self.cargo_loading = false;
        self.cargo_unloading = false;
        self.cargo = 0;
        self.cargo_type = match self.kind {
            VehicleKind::Bus | VehicleKind::Aircraft => Some(CargoType::Passengers),
            VehicleKind::Truck | VehicleKind::Train | VehicleKind::Ship => None,
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

    /// Etiqueta para UI: nombre personalizado o «modelo #id».
    #[must_use]
    pub fn display_name(&self) -> String {
        if let Some(name) = &self.name {
            let trimmed = name.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
        format!("{} #{}", self.effective_engine().name, self.id)
    }

    /// Añade una orden al final sin reiniciar `current_order` (salvo lista vacía).
    pub fn append_order(&mut self, order: VehicleOrder, map: &crate::map::Map) {
        let was_empty = self.orders.is_empty();
        self.orders.push(order);
        if was_empty {
            self.current_order = 0;
            self.path.clear();
            self.no_network_route_to_order = false;
            self.sync_order_destination(map);
        }
    }

    #[must_use]
    pub fn effective_speed(&self) -> u16 {
        self.cur_speed
    }

    fn update_movement_speed(&mut self) {
        let engine = self.effective_engine();
        let max_speed = engine.max_speed;
        let (power, weight) = if self.kind == VehicleKind::Train
            && (self.cached_power_hp > 0 || self.cached_weight_t > 0)
        {
            (
                self.cached_power_hp.max(engine.power_hp),
                self.cached_weight_t.max(engine.weight_t),
            )
        } else {
            (engine.power_hp, engine.weight_t)
        };
        if self.running && self.movement_target().is_some() {
            let (cur, sub) = if self.kind == VehicleKind::Train {
                accelerate_train_speed(self.cur_speed, self.subspeed, power, weight, max_speed)
            } else {
                update_road_speed(
                    self.cur_speed,
                    self.subspeed,
                    ROAD_ACCEL_ORIGINAL,
                    0,
                    max_speed,
                )
            };
            self.cur_speed = cur;
            self.subspeed = sub;
        } else {
            let (cur, sub) = if self.kind == VehicleKind::Train {
                let accel = train_acceleration(power, weight);
                decelerate_train_speed(self.cur_speed, self.subspeed, accel)
            } else {
                decelerate_road_speed(self.cur_speed, self.subspeed)
            };
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
        matches!(
            self.kind,
            VehicleKind::Bus | VehicleKind::Truck | VehicleKind::Ship | VehicleKind::Aircraft
        )
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
        // Un tren o barco nunca avanza fuera de la red: sin camino no se mueve.
        if matches!(
            self.kind,
            VehicleKind::Train | VehicleKind::Ship | VehicleKind::Aircraft
        ) {
            return None;
        }
        if !self.orders.is_empty() {
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

        self.resolve_conditional_orders();

        if self.holding_for_timetable() {
            self.update_movement_speed();
            return;
        }

        // Carga/descarga gradual: no mover hasta cerrar la transferencia.
        if self.cargo_transfer_active() {
            self.cur_speed = 0;
            self.progress = 255;
            return;
        }

        // Cierra la ventana de carga abierta en la llegada del tick anterior.
        // En `sim_step` las fases de carga/descarga corren antes que el
        // movimiento, así que a esta altura ya tuvieron su oportunidad: si
        // actuaron, la orden avanzó y la bandera se limpió; si no, la salida
        // de la parada se decide ahora.
        self.complete_station_load_window();

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
                    self.set_direction_with_curve_penalty(direction_from_tile_step(self.pos, next));
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

    /// Máximo de teselas recordadas para huella PBS / consist.
    const RAIL_HISTORY_CAP: usize = 32;

    fn push_rail_tile_history(&mut self, left: TileCoord) {
        if self.kind != VehicleKind::Train {
            return;
        }
        if self.rail_tile_history.front() != Some(&left) {
            self.rail_tile_history.push_front(left);
        }
        while self.rail_tile_history.len() > Self::RAIL_HISTORY_CAP {
            self.rail_tile_history.pop_back();
        }
    }

    fn advance_one_tile(&mut self) {
        if let Some(next) = self.path.pop_front() {
            self.update_direction_step(self.pos, next);
            if self.orders.is_empty() {
                self.origin = self.pos;
            }
            let left = self.pos;
            self.pos = next;
            self.push_rail_tile_history(left);
            if self.pos == self.dest {
                self.advance_destination_after_arrival();
            }
        } else if self.pos == self.dest {
            self.advance_destination_after_arrival();
        } else {
            if matches!(
                self.kind,
                VehicleKind::Train | VehicleKind::Ship | VehicleKind::Aircraft
            ) || !self.orders.is_empty()
            {
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
        self.set_direction_with_curve_penalty(direction_from_tile_step(from, to));
    }

    /// Cambia `direction` aplicando penalización de curva del modelo original:
    /// carretera `v->cur_speed -= v->cur_speed >> 2` (`roadveh_cmd.cpp:1481`);
    /// tren `_accel_slowdown` (`train_cmd.cpp:3564-3568`, locomotora).
    fn set_direction_with_curve_penalty(&mut self, new_dir: VehicleDirection) {
        if new_dir != self.direction {
            match self.kind {
                VehicleKind::Train => {
                    let params = &ACCEL_SLOWDOWN[0];
                    let turn = if is_45_degree_turn(self.direction, new_dir) {
                        params.small_turn
                    } else {
                        params.large_turn
                    };
                    let penalty = (u32::from(turn) * u32::from(self.cur_speed)) >> 8;
                    self.cur_speed = self
                        .cur_speed
                        .saturating_sub(u16::try_from(penalty).unwrap_or(0));
                }
                VehicleKind::Bus
                | VehicleKind::Truck
                | VehicleKind::Ship
                | VehicleKind::Aircraft => {
                    self.cur_speed -= self.cur_speed >> 2;
                }
            }
        }
        self.direction = new_dir;
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
        self.awaiting_load_window = false;
        self.no_network_route_to_order = false;
        if self.orders.is_empty() {
            self.dest = self.pos;
            return;
        }
        if let Some(&first) = self.orders.first() {
            self.origin = self.pos;
            if !matches!(
                self.kind,
                VehicleKind::Train | VehicleKind::Ship | VehicleKind::Aircraft
            ) {
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
        let early = self.travel_early_wait_ticks();
        if early > 0 {
            self.timetable_wait_remaining = early.max(1);
            self.timetable_wait_kind = TimetableWaitKind::TravelEarly;
            self.progress = 255;
            return;
        }
        self.finish_arrival_processing();
    }

    fn finish_arrival_processing(&mut self) {
        // Llegada a una orden de estación: abre una «ventana de carga» de un
        // tick (análogo a `Vehicle::BeginLoading` de OpenTTD) para que la fase
        // de carga/descarga de `sim_step` actúe antes de avanzar la orden.
        // `sim_step::finish_station_load_windows` la cierra tras esa fase.
        if !self.awaiting_load_window
            && matches!(
                self.orders.get(self.current_order),
                Some(VehicleOrder::Station { .. })
            )
        {
            self.awaiting_load_window = true;
            self.progress = 255;
            return;
        }
        self.finish_arrival_after_load_window();
    }

    /// Cierra la ventana de carga abierta en la llegada (inicio del `step`
    /// siguiente). Si las fases de carga/descarga actuaron, ya avanzaron la
    /// orden (`advance_after_loading`/`_unloading`) y aquí no queda nada.
    fn complete_station_load_window(&mut self) {
        if !self.awaiting_load_window {
            return;
        }
        // Carga/descarga gradual: mantener la ventana abierta mientras haya transferencia.
        if self.cargo_transfer_active() {
            self.progress = 255;
            return;
        }
        self.awaiting_load_window = false;
        if !self.orders.is_empty() && self.pos == self.dest && self.progress == 255 {
            self.finish_arrival_after_load_window();
        }
    }

    fn finish_arrival_after_load_window(&mut self) {
        if self.cargo_transfer_active() {
            self.progress = 255;
            return;
        }
        if self.cargo > 0
            && !self
                .orders
                .get(self.current_order)
                .is_some_and(|o| o.no_unload())
        {
            self.progress = 255;
            return;
        }
        let pass_through = self.orders[self.current_order].is_pass_through();
        if self.orders[self.current_order].depot_stops() {
            self.service_at_depot();
            self.running = false;
            self.progress = 255;
            return;
        }
        if self.orders[self.current_order].full_load() && self.cargo < self.capacity {
            self.progress = 255;
            return;
        }
        if self.schedule_timetable_wait(TimetableWaitKind::AfterArrival) {
            self.progress = 255;
            return;
        }
        self.do_advance_after_arrival(pass_through);
    }

    fn do_advance_after_arrival(&mut self, pass_through: bool) {
        if pass_through {
            self.progress = 0;
        } else {
            self.progress = 255;
        }
        self.advance_to_next_order();
    }

    /// Tras descargar en la parada actual, pasar a la siguiente orden antes de la fase de carga.
    pub(crate) fn advance_after_unloading(&mut self) {
        if self.orders.is_empty() {
            return;
        }
        if self.schedule_timetable_wait(TimetableWaitKind::AfterUnload) {
            self.progress = 255;
            return;
        }
        self.do_advance_after_unloading();
    }

    fn do_advance_after_unloading(&mut self) {
        self.path.clear();
        self.depart_turn = 0;
        self.progress = 255;
        self.advance_to_next_order();
    }

    /// Tras cargar en la parada actual, pasar a la siguiente orden aunque haya carga a bordo.
    pub(crate) fn advance_after_loading(&mut self) {
        if self.orders.is_empty() {
            return;
        }
        if self.orders[self.current_order].full_load() && self.cargo < self.capacity {
            return;
        }
        if self.schedule_timetable_wait(TimetableWaitKind::AfterLoad) {
            self.progress = 255;
            return;
        }
        self.do_advance_after_loading();
    }

    fn do_advance_after_loading(&mut self) {
        self.path.clear();
        self.depart_turn = 0;
        self.progress = 255;
        self.advance_to_next_order();
    }

    fn advance_to_next_order(&mut self) {
        self.awaiting_load_window = false;
        if self.orders.is_empty() {
            return;
        }
        self.current_order = (self.current_order + 1) % self.orders.len();
        self.origin = self.pos;
        self.timetable_leg_start_tick = self.sim_tick;
        if self.kind != VehicleKind::Train {
            self.dest = self.orders[self.current_order].destination();
        }
    }

    fn holding_for_timetable(&self) -> bool {
        self.timetable_active && self.timetable_wait_remaining > 0
    }

    fn schedule_timetable_wait(&mut self, kind: TimetableWaitKind) -> bool {
        if !self.timetable_active {
            return false;
        }
        let wait = self
            .orders
            .get(self.current_order)
            .map_or(0, |o| o.wait_ticks());
        if wait == 0 {
            return false;
        }
        self.timetable_wait_remaining = wait;
        self.timetable_wait_kind = kind;
        true
    }

    fn travel_early_wait_ticks(&self) -> u32 {
        if !self.timetable_active {
            return 0;
        }
        let travel = self
            .orders
            .get(self.current_order)
            .map_or(0, |o| o.travel_ticks());
        if travel == 0 || self.timetable_leg_start_tick == 0 {
            return 0;
        }
        let elapsed = self.sim_tick.saturating_sub(self.timetable_leg_start_tick);
        if elapsed >= u64::from(travel) {
            return 0;
        }
        u32::try_from(u64::from(travel).saturating_sub(elapsed)).unwrap_or(1)
    }

    pub(crate) fn complete_timetable_wait(&mut self) {
        let kind = self.timetable_wait_kind;
        let planned = self
            .orders
            .get(self.current_order)
            .map_or(0, |o| o.wait_ticks());
        self.timetable_wait_kind = TimetableWaitKind::None;
        if kind != TimetableWaitKind::None && planned > 0 {
            self.update_timetable_lateness_on_wait_end(planned);
            let travel = self
                .orders
                .get(self.current_order)
                .map_or(0, |o| o.travel_ticks());
            self.record_timetable_autofill_sample(planned, travel);
        }
        match kind {
            TimetableWaitKind::None => {}
            TimetableWaitKind::TravelEarly => {
                if self.timetable_active {
                    self.timetable_lateness = self.timetable_lateness.saturating_add(1);
                }
                self.finish_arrival_processing();
            }
            TimetableWaitKind::AfterArrival => {
                let pass_through = self.orders[self.current_order].is_pass_through();
                self.do_advance_after_arrival(pass_through);
            }
            TimetableWaitKind::AfterUnload => self.do_advance_after_unloading(),
            TimetableWaitKind::AfterLoad => self.do_advance_after_loading(),
        }
        self.resolve_conditional_orders();
    }

    pub(crate) fn tick_timetable_wait(&mut self) {
        if self.timetable_wait_remaining == 0 {
            return;
        }
        self.timetable_wait_remaining = self.timetable_wait_remaining.saturating_sub(1);
        if self.timetable_wait_remaining == 0 {
            self.complete_timetable_wait();
        }
    }

    /// Actualiza `dest` según la orden actual (vía adyacente para estaciones de tren).
    pub fn sync_order_destination(&mut self, map: &crate::map::Map) {
        self.resolve_conditional_orders();
        if self.orders.is_empty() {
            return;
        }
        let order = self.orders[self.current_order];
        if order.is_conditional() {
            return;
        }
        self.dest = if self.kind == VehicleKind::Aircraft {
            match order {
                VehicleOrder::Station { station, .. } => {
                    // Prefer apron/loading if the hangar ancla está en un footprint.
                    crate::airport::airport_loading_tile_at(map, station)
                }
                _ => crate::station::resolve_order_destination(map, self.kind, order),
            }
        } else {
            crate::station::resolve_order_destination(map, self.kind, order)
        };
    }

    /// Salida con sentido opuesto al de llegada (giro animado en parada bus/camión).
    #[must_use]
    pub(crate) fn needs_depart_turnaround(&self) -> bool {
        if matches!(
            self.kind,
            VehicleKind::Train | VehicleKind::Ship | VehicleKind::Aircraft
        ) {
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
        self.set_direction_with_curve_penalty(outbound);
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

const fn default_depot_stop() -> bool {
    true
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
        use crate::command::apply_command;
        use crate::{Command, GameState};

        let mut state = GameState::new(24, 18);
        let stop = TileCoord::new(15, 3);
        let road = TileCoord::new(15, 4);
        apply_command(&mut state, &Command::SetRoadBits(road, 0x0A)).unwrap();
        apply_command(&mut state, &Command::PlaceBusStop(stop, 1)).unwrap();

        let mut v = Vehicle::new(0, VehicleKind::Bus, TileCoord::new(14, 4), stop);
        v.set_station_orders(vec![stop, TileCoord::new(21, 3)]);
        v.sync_order_destination(&state.map);
        assert_eq!(v.dest, stop, "bus entra a la tesela de la bahía (Fase 2)");
        v.path = VecDeque::from([road, stop]);
        v.direction = DIR_NW;
        v.set_cruise_speed();
        v.progress = 250;
        v.step();
        assert_eq!(v.pos, road, "pasa por la carretera de acceso sin anclarse");
        while v.pos != stop {
            v.step();
        }
        assert_eq!(v.progress, 255, "anclado dentro de la bahía al llegar");
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
    fn train_without_path_never_walks_off_rail() {
        // Tren sin órdenes con destino lejano y sin camino por red: no debe
        // avanzar en Manhattan (caminar por el pasto hasta el depósito).
        let mut v = Vehicle::new(
            0,
            VehicleKind::Train,
            TileCoord::new(4, 14),
            TileCoord::new(10, 14),
        );
        v.set_cruise_speed();
        for _ in 0..200 {
            v.step();
        }
        assert_eq!(
            v.pos,
            TileCoord::new(4, 14),
            "el tren no debe salir de la vía"
        );

        // Un camión libre (sin órdenes) conserva el fallback Manhattan.
        let mut truck = Vehicle::new(
            1,
            VehicleKind::Truck,
            TileCoord::new(4, 14),
            TileCoord::new(10, 14),
        );
        truck.set_cruise_speed();
        for _ in 0..200 {
            truck.step();
        }
        assert_ne!(truck.pos, TileCoord::new(4, 14));
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
    fn road_vehicle_loses_quarter_speed_on_turn() {
        // OpenTTD AM_ORIGINAL: `v->cur_speed -= v->cur_speed >> 2` al cambiar
        // de dirección (roadveh_cmd.cpp:1481).
        let mut v = Vehicle::new(
            0,
            VehicleKind::Truck,
            TileCoord::new(1, 1),
            TileCoord::new(2, 2),
        );
        v.direction = DIR_SE;
        v.path = VecDeque::from([TileCoord::new(1, 2), TileCoord::new(2, 2)]);
        v.set_cruise_speed();
        let cruise = v.cur_speed;
        while v.pos != TileCoord::new(1, 2) {
            v.step();
        }
        assert_eq!(v.cur_speed, cruise, "tramo recto: sin penalización");
        while v.pos != TileCoord::new(2, 2) {
            v.step();
        }
        assert_eq!(
            v.cur_speed,
            cruise - (cruise >> 2),
            "giro SE→SW: −25 % de velocidad"
        );
    }

    #[test]
    fn train_loses_speed_on_direction_change() {
        // OpenTTD AM_ORIGINAL: `_accel_slowdown` al cambiar dirección en la
        // locomotora (train_cmd.cpp:3564-3568). Giro SE→SW = 90° → large_turn.
        let mut v = Vehicle::new(
            0,
            VehicleKind::Train,
            TileCoord::new(1, 1),
            TileCoord::new(2, 2),
        );
        v.direction = DIR_SE;
        v.path = VecDeque::from([TileCoord::new(1, 2), TileCoord::new(2, 2)]);
        v.set_cruise_speed();
        let cruise = v.cur_speed;
        while v.pos != TileCoord::new(1, 2) {
            v.step();
        }
        assert_eq!(v.cur_speed, cruise, "tramo recto: sin penalización");
        while v.pos != TileCoord::new(2, 2) {
            v.step();
        }
        assert_eq!(
            v.cur_speed,
            cruise - ((cruise * 128) >> 8),
            "giro SE→SW: −50 % de velocidad"
        );
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

    #[test]
    fn timetable_wait_delays_order_advance() {
        let pos = TileCoord::new(1, 1);
        let mut v = Vehicle::new(1, VehicleKind::Bus, pos, pos);
        v.timetable_active = true;
        let wait_order = VehicleOrder::station(pos).with_cycled_wait().unwrap();
        v.orders = vec![wait_order, VehicleOrder::station(TileCoord::new(3, 3))];
        assert_eq!(v.orders[0].wait_ticks(), 30);
        v.running = true;
        v.progress = 255;
        v.sim_tick = 100;
        v.step();
        // Primer step: abre la ventana de carga; el segundo agenda la espera.
        v.step();
        assert_eq!(v.timetable_wait_remaining, 30);
        assert_eq!(v.current_order, 0);
        for _ in 0..30 {
            v.tick_timetable_wait();
        }
        assert_eq!(v.current_order, 1);
    }

    #[test]
    fn service_at_depot_restores_reliability() {
        let mut v = Vehicle::new(
            1,
            VehicleKind::Bus,
            TileCoord::new(0, 0),
            TileCoord::new(1, 0),
        );
        v.reliability = 2_000;
        v.needs_servicing = true;
        v.breakdown_ticks_remaining = 50;
        v.service_at_depot();
        assert!(v.reliability >= 8_000);
        assert!(!v.needs_servicing);
        assert_eq!(v.breakdown_ticks_remaining, 0);
    }

    #[test]
    fn check_breakdown_triggers_when_unreliable() {
        let mut v = Vehicle::new(
            7,
            VehicleKind::Truck,
            TileCoord::new(0, 0),
            TileCoord::new(1, 0),
        );
        v.reliability = 3_000;
        v.running = true;
        v.cur_speed = 50;
        let tick = 7_u64 * 256;
        assert!(v.check_breakdown(tick));
        assert!(v.breakdown_ticks_remaining > 0);
        assert_eq!(v.cur_speed, 0);
    }
}
