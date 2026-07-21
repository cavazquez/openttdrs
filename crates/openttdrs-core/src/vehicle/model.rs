//! Modelo de datos del vehículo: struct, enums, constructor.

use std::collections::VecDeque;

use crate::cargo::CargoType;
use crate::map::TileCoord;

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
    /// Tranvía: misma lógica de movimiento que bus, pathfinding sobre bits m3.
    Tram,
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

const fn default_vehicle_reliability() -> u16 {
    8_500
}

const fn default_service_interval_days() -> u16 {
    crate::vehicle::reliability::DEFAULT_SERVICE_INTERVAL_DAYS
}

const fn default_running_true() -> bool {
    true
}

const fn default_vehicle_direction() -> VehicleDirection {
    DIR_NE
}

const fn default_max_curve_speed() -> u16 {
    u16::MAX
}

fn default_unit_length() -> u8 {
    crate::train_consist::VEHICLE_LENGTH
}

fn default_cached_total_length() -> u16 {
    u16::from(crate::train_consist::VEHICLE_LENGTH)
}

/// Fase interna de un vehículo road dentro de un depósito.
///
/// Equivale al estado `RVSB_IN_DEPOT` y a los frames de entrada/salida de
/// `OpenTTD`, sin exponer detalles de la tabla de conducción al resto del core.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
pub enum RoadDepotPhase {
    #[default]
    None,
    InDepot,
    Entering {
        direction: VehicleDirection,
        progress: u8,
    },
    Exiting {
        direction: VehicleDirection,
        progress: u8,
    },
}

fn seed_newgrf_random_bits(id: u32) -> u8 {
    (id.wrapping_mul(0x9E37_79B9) >> 24) as u8
}

const fn default_depot_leave_cleared() -> bool {
    true
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
    /// Compañía propietaria (Fase 4; default jugador).
    #[serde(default)]
    pub owner: crate::company::CompanyId,
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
    /// Tren: remanente físico de `DoUpdateSpeed` (`Vehicle::progress` de `OpenTTD`).
    /// Carretera/tranvía: fracción 0..=255 hacia la siguiente tesela.
    #[serde(default)]
    pub progress: u8,
    /// Tren: píxeles consumidos en la tesela actual (0..15) hacia el cruce.
    #[serde(default)]
    pub rail_pixel: u8,
    /// Caché de esfuerzo tractor máximo (N) del consist; cabeza.
    #[serde(default)]
    pub cached_max_te_n: u32,
    /// Caché de coeficiente de arrastre del consist; cabeza.
    #[serde(default)]
    pub cached_air_drag: u32,
    /// Consist: todos los motores tienen `RailTilts` (`tcache.cached_tilt`).
    #[serde(default)]
    pub cached_tilt: bool,
    /// Consist: mínimo `curve_speed_mod` (`tcache.cached_curve_speed_mod`).
    #[serde(default)]
    pub cached_curve_speed_mod: i16,
    /// Techo de curva Realistic (`tcache.cached_max_curve_speed`); `u16::MAX` = sin límite.
    #[serde(default = "default_max_curve_speed")]
    pub cached_max_curve_speed: u16,
    /// Dirección previa de la cabeza (lag de vagones para `GetCurveSpeedLimit`).
    #[serde(default = "default_vehicle_direction")]
    pub curve_prev_direction: VehicleDirection,
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
    /// Altura en píxeles (`Vehicle::z_pos` / `GetSlopePixelZ`); `None` = sin sincronizar.
    #[serde(default)]
    pub z_pos: Option<i16>,
    /// Contador de movimiento para SFX de motor (`vehicle.cpp` `motion_counter`).
    #[serde(default)]
    pub motion_counter: u16,
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
    pub orders: Vec<crate::vehicle::order::VehicleOrder>,
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
    /// Última estación donde se cargó (link graph observacional; no persistido).
    #[serde(skip)]
    pub last_pickup_station: Option<TileCoord>,
    /// Tick de la última carga (para `travel_time` del link graph; no persistido).
    #[serde(skip)]
    pub last_depart_tick: Option<u64>,
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
    /// Proxy de `Track::Depot`: `false` = en depósito (oculto); `true` = fuera / leave OK.
    /// Por defecto `true` para no bloquear poses de vagones en vía tras import/spawn.
    #[serde(default = "default_depot_leave_cleared")]
    pub depot_leave_cleared: bool,
    /// Estado de depósito para bus/camión/tranvía.
    #[serde(default)]
    pub road_depot_phase: RoadDepotPhase,
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
    /// Fiabilidad actual (0..=10 000 ≈ porcentaje × 100; desde motor al comprar).
    #[serde(default = "default_vehicle_reliability")]
    pub reliability: u16,
    /// Fiabilidad por debajo del umbral de servicio recomendado.
    #[serde(default)]
    pub needs_servicing: bool,
    /// Intervalo de revisión en días de calendario.
    #[serde(default = "default_service_interval_days")]
    pub service_interval_days: u16,
    /// Día de calendario del último servicio en depósito.
    #[serde(default)]
    pub last_service_day: u64,
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
    /// Índice de waypoint FTA en el aeropuerto actual (`v->pos` en `OpenTTD`).
    #[serde(default)]
    pub airport_pos: u8,
    /// Waypoint FTA previo (liberación de bloques).
    #[serde(default)]
    pub airport_prev_pos: u8,
    /// Heading FTA (`v->state` / `AirportMovementStates`).
    #[serde(default)]
    pub airport_heading: crate::airport_fta::AirportHeading,
    /// `true` mientras el avión está bajo control FTA de un aeropuerto Country.
    #[serde(default)]
    pub airport_fta_active: bool,
    /// Bloques FTA que este avión tiene reservados en la estación (`airport_blocks`).
    #[serde(default)]
    pub airport_blocks_held: u64,
    /// Siguiente unidad del consist (`OpenTTD` `Next()`); solo trenes.
    #[serde(default)]
    pub next_unit: Option<u32>,
    /// Unidad anterior del consist; `None` = cabeza (front engine).
    #[serde(default)]
    pub prev_unit: Option<u32>,
    /// Par dual-headed (`other_multiheaded_part`); `None` si no aplica.
    #[serde(default)]
    pub other_multiheaded_part: Option<u32>,
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
    /// Bits aleatorios `NewGRF` del vehículo (`random_bits`; random Action2 / consist).
    #[serde(default)]
    pub newgrf_random_bits: u8,
    /// Registros persistentes `NewGRF` (`7C` / `\2psto`); copiados al ctx al dibujar.
    #[serde(default)]
    pub newgrf_persistent_regs: std::collections::HashMap<u8, u32>,
    /// Beneficio neto del año en curso (ingresos − costes; solo cabeza de consist).
    #[serde(default)]
    pub profit_this_year: i64,
    /// Beneficio neto del año anterior (solo cabeza de consist).
    #[serde(default)]
    pub profit_last_year: i64,
    /// Refit pendiente al llegar a depósito con orden (`VehicleOrder::Depot.refit_cargo`).
    #[serde(skip, default)]
    pub(crate) pending_depot_order_refit: Option<CargoType>,
}

impl Vehicle {
    #[must_use]
    pub fn new(id: u32, kind: VehicleKind, pos: TileCoord, dest: TileCoord) -> Self {
        let cargo_type = match kind {
            VehicleKind::Bus | VehicleKind::Tram | VehicleKind::Aircraft => {
                Some(CargoType::Passengers)
            }
            VehicleKind::Truck | VehicleKind::Train | VehicleKind::Ship => None,
        };
        let engine_id = crate::engine::default_engine_id(kind);
        let reliability =
            crate::vehicle::reliability::initial_reliability_for_engine(engine_id, kind);
        Self {
            id,
            kind,
            owner: crate::company::CompanyId::PLAYER,
            pos,
            origin: pos,
            dest,
            cargo: 0,
            cargo_type,
            capacity: super::VEHICLE_CAPACITY,
            running: true,
            progress: 0,
            rail_pixel: 0,
            cached_max_te_n: 0,
            cached_air_drag: 0,
            cached_tilt: false,
            cached_curve_speed_mod: 0,
            cached_max_curve_speed: u16::MAX,
            curve_prev_direction: DIR_NE,
            direction: DIR_NE,
            engine_id: Some(engine_id),
            name: None,
            cur_speed: 0,
            z_pos: None,
            motion_counter: 0,
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
            last_pickup_station: None,
            last_depart_tick: None,
            cargo_loading: false,
            cargo_unloading: false,
            depart_turn: 0,
            awaiting_load_window: false,
            force_proceed: false,
            wait_counter: 0,
            depot_leave_cleared: true,
            road_depot_phase: RoadDepotPhase::None,
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
            service_interval_days: crate::vehicle::reliability::DEFAULT_SERVICE_INTERVAL_DAYS,
            last_service_day: 0,
            breakdown_ticks_remaining: 0,
            aircraft_phase: AircraftPhase::InHangar,
            altitude: 0,
            aircraft_phase_ticks: 0,
            airport_pos: 0,
            airport_prev_pos: 0,
            airport_heading: crate::airport_fta::AirportHeading::Hangar,
            airport_fta_active: false,
            airport_blocks_held: 0,
            next_unit: None,
            prev_unit: None,
            other_multiheaded_part: None,
            unit_length: crate::train_consist::VEHICLE_LENGTH,
            cached_total_length: u16::from(crate::train_consist::VEHICLE_LENGTH),
            cached_power_hp: 0,
            cached_weight_t: 0,
            newgrf_random_bits: seed_newgrf_random_bits(id),
            newgrf_persistent_regs: std::collections::HashMap::new(),
            profit_this_year: 0,
            profit_last_year: 0,
            pending_depot_order_refit: None,
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

    #[must_use]
    pub fn effective_engine(&self) -> &'static crate::engine::EngineDef {
        crate::engine::engine_for_vehicle(
            self.kind,
            self.engine_id
                .unwrap_or_else(|| crate::engine::default_engine_id(self.kind)),
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

    #[must_use]
    pub fn effective_speed(&self) -> u16 {
        self.cur_speed
    }

    #[must_use]
    pub fn manhattan_to_dest(&self) -> u32 {
        self.pos.x.abs_diff(self.dest.x) + self.pos.y.abs_diff(self.dest.y)
    }

    /// Velocidad de crucero inmediata (tests / saves legacy).
    pub fn set_cruise_speed(&mut self) {
        self.cur_speed = self.effective_engine().max_speed;
        self.subspeed = 0;
    }

    pub fn set_orders(&mut self, orders: Vec<TileCoord>) {
        self.set_vehicle_orders(
            orders
                .into_iter()
                .map(crate::vehicle::order::VehicleOrder::tile)
                .collect(),
        );
    }

    pub fn set_station_orders(&mut self, stations: Vec<TileCoord>) {
        self.set_vehicle_orders(
            stations
                .into_iter()
                .map(crate::vehicle::order::VehicleOrder::station)
                .collect(),
        );
    }

    pub fn set_vehicle_orders(&mut self, orders: Vec<crate::vehicle::order::VehicleOrder>) {
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

    /// Invierte el sentido de marcha (depósito / tests).
    pub fn reverse_heading(&mut self) {
        self.direction = super::reverse_direction(self.direction);
        self.progress = 0;
        self.depart_turn = 0;
    }

    /// Umbral simplificado: últimos ~20 % de vida útil (25 años).
    #[must_use]
    pub fn needs_autorenewing(&self, current_tick: u64) -> bool {
        self.vehicle_age_years(current_tick) >= 20
    }

    /// Sanitiza `current_order` para prevenir indexación fuera de límites.
    ///
    /// Política: si `orders` está vacío → `current_order=0`; si `current_order >= len` → clamp a 0.
    pub fn sanitize_current_order(&mut self) {
        if self.orders.is_empty() || self.current_order >= self.orders.len() {
            self.current_order = 0;
        }
    }

    /// Referencia segura a la orden actual (None si índice inválido o sin órdenes).
    #[must_use]
    pub fn current_order_ref(&self) -> Option<&crate::vehicle::order::VehicleOrder> {
        self.orders.get(self.current_order)
    }

    /// Referencia mutable segura a la orden actual (None si índice inválido o sin órdenes).
    #[must_use]
    pub fn current_order_mut(&mut self) -> Option<&mut crate::vehicle::order::VehicleOrder> {
        self.orders.get_mut(self.current_order)
    }
}
