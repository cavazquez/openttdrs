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

/// Campos crudos de `Vehicle::current_order` que no forman parte de la orden
/// persistida del jugador. `OpenTTD` usa estos valores para representar el
/// estado de carga (`OT_LOADING`), flags temporales y el destino nativo aun
/// cuando la lista `ORDL` no cambió.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct VehicleOrderRuntime {
    pub order_type: u8,
    pub flags: u8,
    pub dest: u16,
    pub refit_cargo: u8,
    pub wait_time: u16,
    pub travel_time: u16,
    pub max_speed: u16,
}

/// Elemento del caché de ruta de `RoadVehicle` en `VEHS`.
///
/// Se mantiene separado de `Vehicle::path` porque `OpenTTD` guarda también el
/// `Trackdir` elegido y el índice lineal de la tesela, datos que no se pueden
/// reconstruir de forma fiable a partir de una cola de coordenadas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RoadPathEntry {
    pub trackdir: u8,
    pub tile: u32,
}

/// Triggers de randomización de vehículos usados por `CBID_RANDOM_TRIGGER`.
///
/// Los valores siguen el orden de `VehicleRandomTrigger` de `OpenTTD`. Se
/// mantienen como una máscara compacta porque el runtime Action2 conserva
/// triggers pendientes entre evaluaciones.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum VehicleRandomTrigger {
    NewCargo = 0,
    Depot = 1,
    Empty = 2,
    AnyNewCargo = 3,
    Callback32 = 4,
}

impl VehicleRandomTrigger {
    /// Bit de espera que se expone en la variable `5F`.
    #[must_use]
    pub const fn mask(self) -> u8 {
        1 << (self as u8)
    }
}

const fn default_vehicle_reliability() -> u16 {
    8_500
}

const fn default_vehicle_reliability_spd_dec() -> u16 {
    crate::engine::DEFAULT_RELIABILITY_SPD_DEC
}

const fn default_vehicle_max_age_days() -> u32 {
    30 * crate::vehicle::reliability::DAYS_PER_VEHICLE_YEAR
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

fn seed_newgrf_random_bits(id: u32) -> u16 {
    (id.wrapping_mul(0x9E37_79B9) >> 16) as u16
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
    /// Número visible de unidad (`Vehicle::unitnumber`).
    #[serde(default)]
    pub unit_number: u16,
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
    /// Subtipo de carga (`Vehicle::cargo_subtype`) usado por `NewGRF`.
    #[serde(default)]
    pub cargo_subtype: u8,
    pub capacity: u32,
    #[serde(default = "default_running_true")]
    pub running: bool,
    /// Remanente físico de `DoUpdateSpeed` (`Vehicle::progress` de `OpenTTD`).
    #[serde(default)]
    pub progress: u8,
    /// Estado de conducción road (`RVSB_*` / trackdir).
    #[serde(default)]
    pub road_state: u8,
    /// Flags generales del vehículo de carretera (`RoadVehicle::gv_flags`).
    #[serde(default)]
    pub road_gv_flags: u16,
    /// Caché de ruta nativo de carretera (`RoadVehicle::path`).
    #[serde(default)]
    pub road_path: Vec<RoadPathEntry>,
    /// Frame dentro de la tabla `_road_drive_data` actual.
    #[serde(default)]
    pub frame: u8,
    /// Contador de bloqueo por tráfico (`RoadVehFindCloseTo`).
    #[serde(default)]
    pub blocked_ctr: u16,
    /// Ventana para ejecutar una reversa vial forzada (`RoadVehicle::reverse_ctr`).
    #[serde(default)]
    pub reverse_ctr: u8,
    /// Carril opuesto durante adelantamiento (`RVSB_DRIVE_SIDE` o 0).
    #[serde(default)]
    pub overtaking: u8,
    /// Contador de ticks de adelantamiento (`RV_OVERTAKE_TIMEOUT = 35`).
    #[serde(default)]
    pub overtaking_ctr: u8,
    /// Vehículo estrellado (`VehState::Crashed`).
    #[serde(default)]
    pub crashed: bool,
    /// Animación / eliminación tras choque (`crashed_ctr`, máx. 2220).
    #[serde(default)]
    pub crashed_ctr: u16,
    /// Tren: píxeles consumidos en la tesela actual (0..15) hacia el cruce.
    #[serde(default)]
    pub rail_pixel: u8,
    /// Estado persistido de `Train::track` (índice de `Track`, no `TrackBits`).
    #[serde(default)]
    pub train_track: u8,
    /// Animación de choque nativa (`Train::crash_anim_pos`).
    #[serde(default)]
    pub train_crash_anim_pos: u16,
    /// Flags nativos específicos del tren (`Train::flags`).
    #[serde(default)]
    pub train_flags: u16,
    /// Flags de vehículo general (`Train::gv_flags`).
    #[serde(default)]
    pub train_gv_flags: u16,
    /// Techo de velocidad por tipo de vía (`gcache.cached_max_track_speed`).
    #[serde(default)]
    pub cached_max_track_speed: u16,
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
    /// Velocidad máxima del consist (`vcache.cached_max_speed`); mínimo por unidad.
    #[serde(default = "default_max_curve_speed")]
    pub cached_max_speed: u16,
    /// Railtypes compatibles del consist (`compatible_railtypes`, bitmask `RailType`).
    #[serde(default)]
    pub compatible_railtypes: u8,
    /// Vagón motorizado (`VehicleRailFlag::PoweredWagon`).
    #[serde(default)]
    pub powered_wagon: bool,
    /// Dirección previa de la cabeza (lag de vagones para `GetCurveSpeedLimit`).
    #[serde(default = "default_vehicle_direction")]
    pub curve_prev_direction: VehicleDirection,
    /// Orientación gráfica (`OpenTTD` `Direction` 0..7).
    #[serde(default = "default_vehicle_direction")]
    pub direction: VehicleDirection,
    /// Motor `OpenGFX` (`None` en saves antiguos → default por [`VehicleKind`]).
    #[serde(default)]
    pub engine_id: Option<u16>,
    /// `EngineID` nativo de `OpenTTD` leído desde `VEHS.common.engine_type`.
    ///
    /// El catálogo Rust no contiene necesariamente todos los motores de un
    /// `NewGRF`. Mantener el identificador wire separado permite reexportar un
    /// `.sav` sin sustituir silenciosamente un motor desconocido por el
    /// primero del catálogo vanilla.
    #[serde(default)]
    pub native_engine_type: Option<u16>,
    /// Sprite base nativo (`Vehicle::spritenum`) conservado para round-trip SAV.
    #[serde(default)]
    pub native_sprite_num: u8,
    /// Nombre personalizado; si falta, la UI usa modelo + id.
    #[serde(default)]
    pub name: Option<String>,
    /// Velocidad actual (unidades `OpenTTD`; 0 = parado).
    #[serde(default)]
    pub cur_speed: u16,
    /// Aceleración persistida por `Vehicle::acceleration`.
    #[serde(default)]
    pub acceleration: u8,
    /// Capacidad máxima de refit (`Vehicle::refit_cap`).
    #[serde(default)]
    pub refit_capacity: u16,
    /// Altura en píxeles (`Vehicle::z_pos` / `GetSlopePixelZ`); `None` = sin sincronizar.
    #[serde(default)]
    pub z_pos: Option<i16>,
    /// Posición X continua de barco en dieciseisavos de tesela (`Vehicle::x_pos`).
    #[serde(default)]
    pub ship_x: i32,
    /// Posición Y continua de barco en dieciseisavos de tesela (`Vehicle::y_pos`).
    #[serde(default)]
    pub ship_y: i32,
    /// `ship_x`/`ship_y` inicializados (saves antiguos → false).
    #[serde(default)]
    pub ship_pos_valid: bool,
    /// Track actual del barco (`Track` / `v->state` bits → índice 0..5).
    #[serde(default)]
    pub ship_track: u8,
    /// Bits de estado nativos del barco (`Ship::state`).
    ///
    /// El estado puede representar además de una vía (`TrackBits`) un
    /// depósito o un wormhole. `ship_track` es sólo la proyección utilizable
    /// por el controlador Rust; conservar los bits crudos evita que un
    /// round-trip de SAV convierta esos estados especiales en vía plana.
    #[serde(default)]
    pub ship_state: u8,
    /// Rotación gráfica persistida por `SlVehicleShip`.
    #[serde(default)]
    pub ship_rotation: u8,
    /// Caché de ruta nativo de barco (`Ship::path`, sólo `Trackdir`).
    #[serde(default)]
    pub ship_path: Vec<u8>,
    /// Contador de tick del barco (`Ship::tick_counter`; esclusa cada 8).
    #[serde(default)]
    pub ship_tick_counter: u8,
    /// Contador de movimiento para SFX de motor (`vehicle.cpp` `motion_counter`).
    #[serde(default)]
    pub motion_counter: u32,
    /// Fracción sub-unidad de velocidad (`Vehicle::subspeed`).
    #[serde(default)]
    pub subspeed: u8,
    /// Pasos de vía reservados por PBS (`tesela` + `TrackBit`).
    #[serde(default)]
    pub reserved_steps: Vec<crate::rail_pbs::ReservedRailStep>,
    /// Historial reciente de teselas (cabeza → atrás) para huella PBS del consist.
    #[serde(default)]
    pub rail_tile_history: VecDeque<TileCoord>,
    /// Historial reciente de teselas (cabeza → atrás) para la pose de cadenas
    /// articuladas road. Se persiste para que una carga/reanudación no vuelva
    /// a colocar todos los eslabones sobre la cabeza durante el primer tick.
    #[serde(default)]
    pub road_tile_history: VecDeque<TileCoord>,
    /// Camino calculado por el pathfinder (siguiente tile en el frente).
    pub path: VecDeque<TileCoord>,
    /// Lista circular de destinos asignados por el jugador.
    #[serde(default)]
    pub orders: Vec<crate::vehicle::order::VehicleOrder>,
    /// Índice de orden real (`cur_real_order_index` / `Vehicle::current_order` del port).
    #[serde(default, alias = "cur_real_order_index")]
    pub current_order: usize,
    /// Índice de orden implícita (`cur_implicit_order_index`).
    #[serde(default)]
    pub cur_implicit_order_index: usize,
    /// Snapshot crudo de `Vehicle::current_order` para round-trip SAV. Se
    /// regenera desde `orders[current_order]` cuando una partida JSON nueva no
    /// trae este campo.
    #[serde(default)]
    pub current_order_state: Option<VehicleOrderRuntime>,
    /// Última estación visitada (`Vehicle::last_station_visited`).
    #[serde(default)]
    pub last_station_visited: Option<TileCoord>,
    /// Último intento de `find_path` falló estando `orders` no vacío; no usar Manhattan (queda bloqueado).
    #[serde(default)]
    pub no_network_route_to_order: bool,
    /// Tesela donde se cargó el lote actual (origen para pago por distancia).
    #[serde(default)]
    pub cargo_source: Option<TileCoord>,
    /// Ticks con carga a bordo (envejecimiento / penalización de pago).
    #[serde(default)]
    pub cargo_transit_ticks: u32,
    /// Cuenta atrás nativa hasta el siguiente envejecimiento de carga.
    #[serde(default)]
    pub cargo_age_counter: u16,
    /// Packets a bordo (fuente de verdad Fase 2); `cargo`/`cargo_source` se sincronizan.
    #[serde(default)]
    pub cargo_packets: crate::cargo_packet::VehicleCargoList,
    /// Última estación donde el vehículo pudo salir con carga
    /// (`Vehicle::last_loading_station`). Se usa también como origen de la
    /// métrica de viaje del link graph.
    #[serde(default)]
    pub last_pickup_station: Option<TileCoord>,
    /// Tick en que dejó la última estación con carga
    /// (`Vehicle::last_loading_tick`).
    #[serde(default)]
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
    /// Evento transitorio para CB140: terminó una parada y debe emitir
    /// `VehicleDeparts` antes de iniciar el movimiento siguiente.
    #[serde(skip)]
    pub(crate) station_departure_pending: bool,
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
    /// Próxima salida permitida tras unbunch (`depot_unbunching_next_departure`).
    #[serde(default)]
    pub depot_unbunching_next_departure: u64,
    /// Última salida de un depósito unbunch (`depot_unbunching_last_departure`).
    #[serde(default)]
    pub depot_unbunching_last_departure: u64,
    /// Duración del último viaje redondo para separar salidas unbunch (ticks).
    #[serde(default)]
    pub round_trip_time: u32,
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
    /// Ticks transcurridos en la orden actual (`Vehicle::current_order_time`).
    #[serde(skip, default)]
    pub(crate) current_order_time: u32,
    /// Inicio escalonado del ciclo de horario (`Vehicle::timetable_start`).
    #[serde(default)]
    pub timetable_start: u32,
    /// El vehículo ya completó la primera llegada del horario.
    #[serde(default)]
    pub timetable_started: bool,
    /// Bits nativos de `VehicleFlags` que todavía no tienen un campo de
    /// runtime dedicado. Los bits de horario se mantienen sincronizados con
    /// `timetable_started`/`timetable_autofill` al exportar.
    #[serde(default)]
    pub vehicle_flags: u16,
    /// Autoreemplazo ya intentado en esta parada en depósito.
    #[serde(default)]
    pub autoreplace_attempted_this_stop: bool,
    /// Tick de simulación en que se compró el vehículo.
    #[serde(default)]
    pub build_tick: u64,
    /// Edad contable en días de economía (`Vehicle::economy_age`).
    #[serde(default)]
    pub economy_age_days: u32,
    /// Año calendario nativo de compra (`Vehicle::build_year`). Cuando es
    /// cero, el escritor lo deriva de `build_tick` para vehículos creados por
    /// openttdrs.
    #[serde(default)]
    pub build_year: u32,
    /// Grupo de flota opcional.
    #[serde(default)]
    pub group_id: Option<u32>,
    /// Pool de órdenes compartidas enlazado.
    #[serde(default)]
    pub shared_order_id: Option<u32>,
    /// Siguiente vehículo de la cadena nativa de órdenes compartidas.
    #[serde(default)]
    pub next_shared_vehicle_id: Option<u32>,
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
    /// Fecha protegida para callbacks `NewGRF` (`date_of_last_service_newgrf`).
    #[serde(default)]
    pub last_service_newgrf_day: i32,
    /// Decaimiento diario de fiabilidad (copia del motor al comprar).
    #[serde(default = "default_vehicle_reliability_spd_dec")]
    pub reliability_spd_dec: u16,
    /// Vida útil máxima en días de calendario (`max_age`).
    #[serde(default = "default_vehicle_max_age_days")]
    pub max_age_days: u32,
    /// Acumulador de riesgo de avería (`breakdown_chance`).
    #[serde(default)]
    pub breakdown_chance: u8,
    /// Fase de avería (`breakdown_ctr`; ver `HandleBreakdown`).
    #[serde(default)]
    pub breakdown_ctr: u8,
    /// Ticks restantes de avería activa (`breakdown_delay`).
    #[serde(default)]
    pub breakdown_delay: u8,
    /// Averías sufridas desde la última revisión (`breakdowns_since_last_service`).
    #[serde(default)]
    pub breakdowns_since_last_service: u8,
    /// Contador económico diario (`Vehicle::day_counter`), usado por CB32.
    #[serde(default)]
    pub newgrf_day_counter: u8,
    /// Contador de tick nativo (`Vehicle::tick_counter`) usado por callbacks
    /// de vehículos y por la cadencia de tráfico/averías.
    #[serde(default)]
    pub newgrf_tick_counter: u8,
    /// Ticks activos acumulados en el día actual (`Vehicle::running_ticks`).
    #[serde(default)]
    pub running_ticks: u8,
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
    /// Posición X continua FTA en dieciseisavos de tesela (`x_pos` de `OpenTTD`).
    #[serde(default)]
    pub airport_sub_x: i32,
    /// Posición Y continua FTA en dieciseisavos de tesela (`y_pos` de `OpenTTD`).
    #[serde(default)]
    pub airport_sub_y: i32,
    /// Los campos continuos FTA fueron inicializados (compatibilidad con saves anteriores).
    #[serde(default)]
    pub airport_subpos_valid: bool,
    /// El avión alcanzó físicamente el waypoint FTA actual.
    #[serde(default)]
    pub airport_waypoint_reached: bool,
    /// El waypoint alcanzado es un stand válido de carga/descarga.
    #[serde(default)]
    pub airport_loading_stand_reached: bool,
    /// Heading FTA (`v->state` / `AirportMovementStates`).
    #[serde(default)]
    pub airport_heading: crate::airport_fta::AirportHeading,
    /// Contador de giros consecutivos del avión (`Aircraft::number_consecutive_turns`).
    #[serde(default)]
    pub aircraft_number_consecutive_turns: u8,
    /// Contador de giro FTA (`Aircraft::turn_counter`).
    #[serde(default)]
    pub aircraft_turn_counter: u8,
    /// Flags nativos de aeronave (`Aircraft::flags`).
    #[serde(default)]
    pub aircraft_flags: u8,
    /// `true` mientras el avión está bajo control FTA de un aeropuerto Country.
    #[serde(default)]
    pub airport_fta_active: bool,
    /// Ancla de la estación que controla el circuito FTA actual.
    #[serde(default)]
    pub airport_fta_station: Option<TileCoord>,
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
    /// Unidad creada automáticamente por `CBID_VEHICLE_ARTIC_ENGINE`.
    ///
    /// No se deriva del enlace `prev_unit`: un wagon comprado por el jugador
    /// puede pertenecer al mismo consist y debe sobrevivir a un reemplazo que
    /// reconstruya las piezas articuladas.
    #[serde(default)]
    pub newgrf_articulated: bool,
    /// `CUSTOM_VEHICLE_SPRITENUM_REVERSED`: el callback de articulación pide
    /// resolver la vista del eslabón en la dirección opuesta a la cabeza.
    #[serde(default)]
    pub newgrf_mirrored: bool,
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
    /// Bits aleatorios `NewGRF` del vehículo (`random_bits`; 16 bits de
    /// `OpenTTD`, usados por Action2 random y scopes de consist).
    #[serde(default)]
    pub newgrf_random_bits: u16,
    /// Triggers de randomización pendientes (`VehicleRandomTriggers`).
    ///
    /// `OpenTTD` los conserva hasta que el grupo Action2 activo los consume;
    /// persistirlos evita que un save/load cambie la variante seleccionada.
    #[serde(default)]
    pub newgrf_waiting_random_triggers: u8,
    /// Registros persistentes `NewGRF` (`7C` / `\2psto`); writeback tras eval CB/Action2.
    #[serde(default)]
    pub newgrf_persistent_regs: std::collections::HashMap<u8, u32>,
    /// Generación visual incrementada por CB32 cuando solicita invalidar la
    /// paleta. No altera la simulación; sólo impide reutilizar una textura
    /// horneada antes de la invalidación.
    #[serde(default)]
    pub newgrf_palette_generation: u32,
    /// Beneficio neto del año en curso (ingresos − costes; solo cabeza de consist).
    #[serde(default)]
    pub profit_this_year: i64,
    /// Beneficio neto del año anterior (solo cabeza de consist).
    #[serde(default)]
    pub profit_last_year: i64,
    /// Cuenta atrás de carga/descarga (`Vehicle::load_unload_ticks`).
    #[serde(default)]
    pub load_unload_ticks: u16,
    /// Campo legacy de migración (`cargo_paid_for`) conservado por saves
    /// modernos para no perder el contador al cruzar `OpenTTD`.
    #[serde(default)]
    pub cargo_paid_for: u16,
    /// Valor contable persistido (`Vehicle::value`, Money con 8 bits de
    /// fracción en el wire format).
    #[serde(default)]
    pub value: i64,
    /// Refit pendiente al llegar a depósito con orden (`VehicleOrder::Depot.refit_cargo`).
    #[serde(skip, default)]
    pub(crate) pending_depot_order_refit: Option<CargoType>,
    /// Acumulador fraccional de coste de explotación (prorrateo anual por tick).
    #[serde(default)]
    pub running_cost_accum: u64,
}

impl Vehicle {
    #[must_use]
    #[allow(clippy::too_many_lines)]
    pub fn new(id: u32, kind: VehicleKind, pos: TileCoord, dest: TileCoord) -> Self {
        let cargo_type = match kind {
            VehicleKind::Bus | VehicleKind::Tram | VehicleKind::Aircraft => {
                Some(CargoType::Passengers)
            }
            VehicleKind::Truck | VehicleKind::Train | VehicleKind::Ship => None,
        };
        let engine_id = crate::engine::default_engine_id(kind);
        let engine = crate::engine::engine_for_vehicle(kind, engine_id);
        let reliability =
            crate::vehicle::reliability::initial_reliability_for_engine(engine_id, kind);
        let reliability_spd_dec = engine.reliability_spd_dec;
        let max_age_days =
            u32::from(engine.lifelength_years) * crate::vehicle::reliability::DAYS_PER_VEHICLE_YEAR;
        Self {
            id,
            kind,
            unit_number: u16::try_from(id.saturating_add(1)).unwrap_or(u16::MAX),
            owner: crate::company::CompanyId::PLAYER,
            pos,
            origin: pos,
            dest,
            cargo: 0,
            cargo_type,
            cargo_subtype: 0,
            capacity: super::VEHICLE_CAPACITY,
            running: true,
            progress: 0,
            road_state: 0,
            road_gv_flags: 0,
            road_path: Vec::new(),
            frame: 0,
            blocked_ctr: 0,
            reverse_ctr: 0,
            overtaking: 0,
            overtaking_ctr: 0,
            crashed: false,
            crashed_ctr: 0,
            rail_pixel: 0,
            train_track: 0,
            train_crash_anim_pos: 0,
            train_flags: 0,
            train_gv_flags: 0,
            cached_max_track_speed: 0,
            cached_max_te_n: 0,
            cached_air_drag: 0,
            cached_tilt: false,
            cached_curve_speed_mod: 0,
            cached_max_curve_speed: u16::MAX,
            cached_max_speed: u16::MAX,
            compatible_railtypes: 0,
            powered_wagon: false,
            curve_prev_direction: DIR_NE,
            direction: DIR_NE,
            engine_id: Some(engine_id),
            native_engine_type: None,
            native_sprite_num: 0,
            name: None,
            cur_speed: 0,
            acceleration: 0,
            refit_capacity: 0,
            z_pos: None,
            ship_x: 0,
            ship_y: 0,
            ship_pos_valid: false,
            ship_track: 0,
            ship_state: 0,
            ship_rotation: 0,
            ship_path: Vec::new(),
            ship_tick_counter: 0,
            motion_counter: 0,
            subspeed: 0,
            path: VecDeque::new(),
            reserved_steps: Vec::new(),
            rail_tile_history: VecDeque::new(),
            road_tile_history: VecDeque::new(),
            orders: Vec::new(),
            current_order: 0,
            cur_implicit_order_index: 0,
            current_order_state: None,
            last_station_visited: None,
            no_network_route_to_order: false,
            cargo_source: None,
            cargo_transit_ticks: 0,
            cargo_age_counter: 0,
            cargo_packets: crate::cargo_packet::VehicleCargoList::default(),
            last_pickup_station: None,
            last_depart_tick: None,
            cargo_loading: false,
            cargo_unloading: false,
            depart_turn: 0,
            awaiting_load_window: false,
            station_departure_pending: false,
            force_proceed: false,
            wait_counter: 0,
            depot_leave_cleared: true,
            depot_unbunching_next_departure: 0,
            depot_unbunching_last_departure: 0,
            round_trip_time: 0,
            road_depot_phase: RoadDepotPhase::None,
            pbs_stuck: false,
            timetable_active: false,
            timetable_wait_remaining: 0,
            timetable_wait_kind: TimetableWaitKind::None,
            timetable_leg_start_tick: 0,
            current_order_time: 0,
            timetable_start: 0,
            timetable_started: false,
            vehicle_flags: 0,
            autoreplace_attempted_this_stop: false,
            build_tick: 0,
            economy_age_days: 0,
            build_year: 0,
            group_id: None,
            shared_order_id: None,
            next_shared_vehicle_id: None,
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
            last_service_newgrf_day: 0,
            reliability_spd_dec,
            max_age_days,
            breakdown_chance: 0,
            breakdown_ctr: 0,
            breakdown_delay: 0,
            breakdowns_since_last_service: 0,
            newgrf_day_counter: 0,
            newgrf_tick_counter: 0,
            running_ticks: 0,
            aircraft_phase: AircraftPhase::InHangar,
            altitude: 0,
            aircraft_phase_ticks: 0,
            airport_pos: 0,
            airport_prev_pos: 0,
            airport_sub_x: 0,
            airport_sub_y: 0,
            airport_subpos_valid: false,
            airport_waypoint_reached: false,
            airport_loading_stand_reached: false,
            airport_heading: crate::airport_fta::AirportHeading::Hangar,
            aircraft_number_consecutive_turns: 0,
            aircraft_turn_counter: 0,
            aircraft_flags: 0,
            airport_fta_active: false,
            airport_fta_station: None,
            airport_blocks_held: 0,
            next_unit: None,
            prev_unit: None,
            other_multiheaded_part: None,
            newgrf_articulated: false,
            newgrf_mirrored: false,
            unit_length: crate::train_consist::VEHICLE_LENGTH,
            cached_total_length: u16::from(crate::train_consist::VEHICLE_LENGTH),
            cached_power_hp: 0,
            cached_weight_t: 0,
            newgrf_random_bits: seed_newgrf_random_bits(id),
            newgrf_waiting_random_triggers: 0,
            newgrf_persistent_regs: std::collections::HashMap::new(),
            newgrf_palette_generation: 0,
            profit_this_year: 0,
            profit_last_year: 0,
            load_unload_ticks: 0,
            cargo_paid_for: 0,
            value: 0,
            running_cost_accum: 0,
            pending_depot_order_refit: None,
        }
    }

    /// ¿Es la cabeza del consist (o un vehículo que no tiene cadena)?
    ///
    /// Los articulados de carretera usan `Next()` igual que los trenes, pero
    /// sus remolques se marcan con `newgrf_articulated`.  Tratar esos remolques
    /// como cabezas haría que el simulador los moviese y que la UI los dibujase
    /// dos veces.
    #[must_use]
    pub fn is_consist_head(&self) -> bool {
        match self.kind {
            VehicleKind::Train => self.prev_unit.is_none(),
            VehicleKind::Bus | VehicleKind::Truck | VehicleKind::Tram => {
                !self.newgrf_articulated || self.prev_unit.is_none()
            }
            VehicleKind::Ship | VehicleKind::Aircraft => true,
        }
    }

    /// ¿Es un vagón enganchado (no cabeza)?
    #[must_use]
    pub fn is_wagon_unit(&self) -> bool {
        self.kind == VehicleKind::Train && self.prev_unit.is_some()
    }

    /// ¿Es una unidad vial creada automáticamente por `CBID_VEHICLE_ARTIC_ENGINE`?
    #[must_use]
    pub fn is_articulated_unit(&self) -> bool {
        self.newgrf_articulated
            && self.prev_unit.is_some()
            && matches!(
                self.kind,
                VehicleKind::Bus | VehicleKind::Truck | VehicleKind::Tram
            )
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
        self.cur_speed = crate::newgrf_callback::effective_vehicle_max_speed(self);
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
        self.cur_implicit_order_index = 0;
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

    /// Umbral simplificado: `age - max_age >= engine_renew_months * 30` días.
    #[must_use]
    pub fn needs_autorenewing(&self, current_tick: u64, engine_renew_months: i16) -> bool {
        if self.prev_unit.is_some() {
            return false;
        }
        if self.kind == VehicleKind::Train
            && self.engine_id.is_some_and(|id| {
                !crate::engine::engine_for_vehicle(self.kind, id).is_train_engine()
            })
        {
            return false;
        }
        let age_days = self.vehicle_age_days(current_tick);
        let threshold = i64::from(engine_renew_months) * 30;
        i64::try_from(age_days.saturating_sub(u64::from(self.max_age_days))).unwrap_or(i64::MAX)
            >= threshold
    }

    /// Índice real de orden (`cur_real_order_index`).
    #[must_use]
    pub const fn cur_real_order_index(&self) -> usize {
        self.current_order
    }

    /// Sanitiza índices de orden para prevenir indexación fuera de límites.
    ///
    /// Política: si `orders` está vacío → índices a 0; si algún índice `>= len` → clamp a 0.
    pub fn sanitize_current_order(&mut self) {
        if self.orders.is_empty() || self.current_order >= self.orders.len() {
            self.current_order = 0;
        }
        if self.orders.is_empty() || self.cur_implicit_order_index >= self.orders.len() {
            self.cur_implicit_order_index = 0;
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
