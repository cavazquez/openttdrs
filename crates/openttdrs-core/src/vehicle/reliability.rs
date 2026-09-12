//! Fiabilidad, servicio y averías del vehículo.

use crate::cargodist::parity::Randomizer;
use crate::vehicle::{Vehicle, VehicleKind};

/// Umbral de fiabilidad bajo el cual conviene servicio en depósito.
pub const SERVICING_RELIABILITY_THRESHOLD: u16 = 5_000;
/// Intervalo de revisión por defecto de trenes y vehículos de carretera.
pub const DEFAULT_SERVICE_INTERVAL_DAYS: u16 = 150;
/// Intervalo de revisión por defecto de trenes (`DEF_SERVINT_DAYS_TRAINS`).
pub const DEFAULT_SERVICE_INTERVAL_DAYS_TRAINS: u16 = DEFAULT_SERVICE_INTERVAL_DAYS;
/// Intervalo de revisión por defecto de vehículos de carretera
/// (`DEF_SERVINT_DAYS_ROADVEH`).
pub const DEFAULT_SERVICE_INTERVAL_DAYS_ROAD_VEHICLES: u16 = 150;
/// Intervalo de revisión por defecto de aeronaves (`DEF_SERVINT_DAYS_AIRCRAFT`).
pub const DEFAULT_SERVICE_INTERVAL_DAYS_AIRCRAFT: u16 = 100;
/// Intervalo de revisión por defecto de barcos (`DEF_SERVINT_DAYS_SHIPS`).
pub const DEFAULT_SERVICE_INTERVAL_DAYS_SHIPS: u16 = 360;
/// Duración máxima de avería en ticks (`breakdown_delay` hasta 255).
pub const BREAKDOWN_DURATION_TICKS: u32 = 255;
/// Velocidad mínima para acumular riesgo de avería (`vehicle.cpp:1340`).
pub const MIN_SPEED_FOR_BREAKDOWN: u16 = 5;
/// Días de calendario por año (paridad `CalendarTime::DAYS_IN_LEAP_YEAR`).
pub const DAYS_PER_VEHICLE_YEAR: u32 = 366;
/// Edad económica máxima representable por `EconomyTime::MAX_DATE`.
const MAX_ECONOMY_AGE_DAYS: u32 = 1_826_212_865;

/// Devuelve el intervalo inicial de servicio para el tipo nativo de vehículo.
#[must_use]
pub const fn default_service_interval_days_for_kind(kind: VehicleKind) -> u16 {
    match kind {
        VehicleKind::Aircraft => DEFAULT_SERVICE_INTERVAL_DAYS_AIRCRAFT,
        VehicleKind::Ship => DEFAULT_SERVICE_INTERVAL_DAYS_SHIPS,
        VehicleKind::Bus | VehicleKind::Truck | VehicleKind::Tram => {
            DEFAULT_SERVICE_INTERVAL_DAYS_ROAD_VEHICLES
        }
        VehicleKind::Train => DEFAULT_SERVICE_INTERVAL_DAYS_TRAINS,
    }
}

/// Tabla `_breakdown_chance[rel >> 10]` (`vehicle.cpp:1303-1312`).
const BREAKDOWN_CHANCE_TABLE: [u8; 64] = [
    3, 3, 3, 3, 3, 3, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 13, 13, 13, 13,
    14, 15, 16, 17, 19, 21, 25, 28, 31, 34, 37, 40, 44, 48, 52, 56, 60, 64, 68, 72, 80, 90, 100,
    110, 120, 130, 140, 150, 170, 190, 210, 230, 250, 250, 250,
];

/// Bonus de fiabilidad efectiva para barcos (`vehicle.cpp:1355`).
const SHIP_RELIABILITY_BONUS: u32 = 0x6666;
/// Bonus de fiabilidad efectiva con averías reducidas (`vehicle.cpp:1358`).
const REDUCED_BREAKDOWN_RELIABILITY_BONUS: u32 = 0x6666;

pub(crate) fn initial_reliability_for_engine(
    engine_id: u16,
    kind: super::model::VehicleKind,
) -> u16 {
    u16::from(crate::engine::engine_for_vehicle(kind, engine_id).reliability_pct) * 100
}

pub(crate) fn init_vehicle_reliability_from_engine(
    vehicle: &mut super::model::Vehicle,
    engine: &crate::engine::EngineDef,
) {
    vehicle.reliability = initial_reliability_for_engine(engine.id, engine.kind);
    vehicle.reliability_spd_dec = engine.reliability_spd_dec;
    vehicle.max_age_days = u32::from(engine.lifelength_years) * DAYS_PER_VEHICLE_YEAR;
}

pub(crate) fn init_vehicle_reliability_from_engine_with_catalog(
    vehicle: &mut super::model::Vehicle,
    engine: &crate::engine::EngineDef,
    engine_catalog: &[crate::engine::EngineDef],
) {
    let source = crate::engine::engine_reliability_source(engine, engine_catalog);
    vehicle.reliability = u16::from(source.reliability_pct) * 100;
    vehicle.reliability_spd_dec = source.reliability_spd_dec;
    vehicle.max_age_days = u32::from(source.lifelength_years) * DAYS_PER_VEHICLE_YEAR;
}

fn scale_reliability_to_openttd(reliability: u16) -> u32 {
    u32::from(reliability) * 65535 / 10000
}

fn decay_reliability_port(reliability: u16, spd_dec: u16) -> u16 {
    let dec = u32::from(spd_dec) * 10000 / 65535;
    reliability.saturating_sub(u16::try_from(dec).unwrap_or(u16::MAX))
}

fn effective_reliability_for_breakdown(
    reliability: u16,
    kind: VehicleKind,
    reduced_breakdowns: bool,
) -> u32 {
    let mut rel = scale_reliability_to_openttd(reliability);
    if kind == VehicleKind::Ship {
        rel = rel.saturating_add(SHIP_RELIABILITY_BONUS);
    }
    if reduced_breakdowns {
        rel = rel.saturating_add(REDUCED_BREAKDOWN_RELIABILITY_BONUS);
    }
    rel.min(65535)
}

fn breakdown_table_index(reliability: u16, kind: VehicleKind, reduced_breakdowns: bool) -> usize {
    let rel = effective_reliability_for_breakdown(reliability, kind, reduced_breakdowns);
    usize::from((rel >> 10).min(63) as u8)
}

/// `Chance16I(a, b, r)` con los 16 bits bajos de `r`.
fn chance16i(a: u32, b: u32, r: u32) -> bool {
    if b == 0 {
        return false;
    }
    ((u32::from(u16::try_from(r).unwrap_or(u16::MAX)) * b + b / 2) >> 16) < a
}

fn extract_bits(value: u32, offset: u32, count: u32) -> u8 {
    let mask = if count >= 32 {
        u32::MAX
    } else {
        (1_u32 << count) - 1
    };
    u8::try_from((value >> offset) & mask).unwrap_or(u8::MAX)
}

/// Ejecuta `VehicleServiceInDepot` sobre la cabeza y toda su cadena `Next()`.
///
/// `Vehicle` conserva la lógica de una unidad porque varios controladores
/// históricos no reciben la flota completa. Los puntos autoritativos que sí
/// tienen `FleetIndex` llaman a esta variante para mantener sincronizados
/// fiabilidad, fechas, averías y callbacks de cada unidad con motor.
pub(crate) fn service_vehicle_chain_with_catalog(
    vehicles: &mut [Vehicle],
    fleet: &crate::fleet_index::FleetIndex,
    head_id: u32,
    engine_catalog: &[crate::engine::EngineDef],
) {
    service_vehicle_units_with_catalog(vehicles, fleet, head_id, engine_catalog, false);
}

/// Completa el servicio de las unidades posteriores cuando la cabeza ya fue
/// servida dentro de un controlador que sólo tenía un `&mut Vehicle`.
pub(crate) fn service_vehicle_followers_with_catalog(
    vehicles: &mut [Vehicle],
    fleet: &crate::fleet_index::FleetIndex,
    head_id: u32,
    engine_catalog: &[crate::engine::EngineDef],
) {
    service_vehicle_units_with_catalog(vehicles, fleet, head_id, engine_catalog, true);
}

fn service_vehicle_units_with_catalog(
    vehicles: &mut [Vehicle],
    fleet: &crate::fleet_index::FleetIndex,
    head_id: u32,
    engine_catalog: &[crate::engine::EngineDef],
    skip_head: bool,
) {
    let slots: Vec<usize> = fleet
        .consist(head_id)
        .iter()
        .filter_map(|&id| fleet.slot(id))
        .collect();
    let Some(&head_slot) = slots.first() else {
        return;
    };
    let Some(head_kind) = vehicles.get(head_slot).map(|vehicle| vehicle.kind) else {
        return;
    };
    for (chain_index, slot) in slots.into_iter().enumerate() {
        if skip_head && chain_index == 0 {
            continue;
        }
        let Some(vehicle) = vehicles.get(slot) else {
            break;
        };
        if vehicle.kind != head_kind {
            break;
        }
        vehicles[slot].service_at_depot_with_catalog(engine_catalog);
    }
}

impl super::model::Vehicle {
    /// Restaura fiabilidad tras servicio en depósito.
    pub fn service_at_depot(&mut self) {
        let engine_id = self
            .engine_id
            .unwrap_or_else(|| crate::engine::default_engine_id(self.kind));
        self.reliability = initial_reliability_for_engine(engine_id, self.kind);
        self.needs_servicing = false;
        self.breakdown_ctr = 0;
        self.breakdown_delay = 0;
        self.breakdown_chance /= 4;
        if self.service_breakdown_level == 1 {
            self.breakdown_chance = 0;
        }
        self.breakdowns_since_last_service = 0;
        let service_day =
            crate::news::calendar_day_index(crate::tick::GameTick::new(self.sim_tick));
        self.last_service_day = service_day;
        self.last_service_newgrf_day = i32::try_from(service_day).unwrap_or(i32::MAX);
        self.service_generation = self.service_generation.wrapping_add(1);
    }

    /// Igual que [`Self::service_at_depot`], resolviendo `SyncReliability`
    /// desde el catálogo runtime cuando el motor proviene de `NewGRF`.
    pub(crate) fn service_at_depot_with_catalog(
        &mut self,
        engine_catalog: &[crate::engine::EngineDef],
    ) {
        let engine_id = self
            .engine_id
            .unwrap_or_else(|| crate::engine::default_engine_id(self.kind));
        if let Some(engine) = crate::engine::engine_in_catalog(engine_catalog, engine_id)
            .or_else(|| crate::engine::engine_by_id(engine_id))
        {
            let source = crate::engine::engine_reliability_source(engine, engine_catalog);
            self.reliability = u16::from(source.reliability_pct) * 100;
            self.reliability_spd_dec = source.reliability_spd_dec;
            self.max_age_days = u32::from(source.lifelength_years) * DAYS_PER_VEHICLE_YEAR;
        } else {
            self.reliability = initial_reliability_for_engine(engine_id, self.kind);
        }
        self.needs_servicing = false;
        self.breakdown_ctr = 0;
        self.breakdown_delay = 0;
        self.breakdown_chance /= 4;
        if self.service_breakdown_level == 1 {
            self.breakdown_chance = 0;
        }
        self.breakdowns_since_last_service = 0;
        let service_day =
            crate::news::calendar_day_index(crate::tick::GameTick::new(self.sim_tick));
        self.last_service_day = service_day;
        self.last_service_newgrf_day = i32::try_from(service_day).unwrap_or(i32::MAX);
        self.service_generation = self.service_generation.wrapping_add(1);
    }

    /// ¿Toca revisión? (`NeedsServicing`: intervalo en días o % de fiabilidad).
    ///
    /// Órdenes `Depot { stop: false }` = «servicio si hace falta» (se saltan si esto es false).
    #[must_use]
    pub fn requires_service(&self) -> bool {
        self.interval_requires_service(false)
    }

    /// Igual que [`Self::requires_service`] pero con intervalo en % si la compañía lo usa.
    #[must_use]
    pub fn requires_service_for_company(&self, servint_ispercent: bool) -> bool {
        self.interval_requires_service(servint_ispercent)
    }

    /// Evaluación completa con ajustes de partida y autoreemplazo (`NeedsServicing`).
    #[must_use]
    pub fn requires_service_with(&self, state: &crate::GameState) -> bool {
        if !self.running {
            return false;
        }
        let servint_ispercent = state
            .companies
            .get(self.owner.index())
            .is_some_and(|c| c.servint_ispercent);
        if !self.interval_requires_service(servint_ispercent) {
            return false;
        }
        if !state.no_servicing_if_no_breakdowns || state.vehicle_breakdowns != 0 {
            return true;
        }
        crate::autoreplace::pending_autoreplace_for_service(state, self)
    }

    fn interval_requires_service(&self, servint_ispercent: bool) -> bool {
        if servint_ispercent {
            let engine_id = self
                .engine_id
                .unwrap_or_else(|| crate::engine::default_engine_id(self.kind));
            let engine_rel = initial_reliability_for_engine(engine_id, self.kind);
            let pct = u32::from(self.service_interval_days.min(100));
            let threshold = u32::from(engine_rel) * (100 - pct) / 100;
            if u32::from(self.reliability) >= threshold {
                return false;
            }
        } else {
            let day = crate::news::calendar_day_index(crate::tick::GameTick::new(self.sim_tick));
            let interval = u64::from(self.service_interval_days.max(1));
            if day.saturating_sub(self.last_service_day) < interval {
                return false;
            }
        }
        true
    }

    /// ¿El vehículo está parado por avería activa? (`breakdown_ctr == 1`).
    ///
    /// Con `ctr > 2` aún se mueve mientras cuenta atrás hacia la avería; con `ctr == 2`
    /// el tick actual la dispara (`HandleBreakdown`).
    #[must_use]
    pub fn is_broken_down(&self) -> bool {
        self.breakdown_ctr == 1 && self.kind != VehicleKind::Aircraft
    }

    /// Edad del vehículo en días de calendario desde la compra.
    #[must_use]
    pub fn vehicle_age_days(&self, current_tick: u64) -> u64 {
        let age_ticks = current_tick.saturating_sub(self.build_tick);
        age_ticks / u64::from(crate::economy::TICKS_PER_DAY)
    }

    /// `AgeVehicle`: duplica `reliability_spd_dec` en ciertos años tras `max_age`.
    pub fn age_vehicle_calendar_day(&mut self, calendar_day: u64) {
        if !self.is_primary_for_aging() {
            return;
        }
        let build_day =
            crate::news::calendar_day_index(crate::tick::GameTick::new(self.build_tick));
        let age_days = calendar_day.saturating_sub(build_day);
        let past_max = age_days.saturating_sub(u64::from(self.max_age_days));
        for i in 0_u32..=4_u32 {
            let boundary = u64::from(i) * u64::from(DAYS_PER_VEHICLE_YEAR);
            if past_max == boundary {
                self.reliability_spd_dec = self.reliability_spd_dec.saturating_mul(2);
                break;
            }
        }
    }

    /// Incrementa la edad económica una vez por cada barrido diario.
    ///
    /// `OpenTTD` actualiza todas las unidades ferroviarias y navales, pero sólo
    /// la cabeza de los vehículos de carretera y los aviones normales. Las
    /// sombras/rotores no existen como vehículos runtime en este port.
    pub fn age_vehicle_economy_day(&mut self) {
        if !self.participates_in_economy_day() {
            return;
        }

        self.economy_age_days = self.economy_age_days.min(MAX_ECONOMY_AGE_DAYS);
        if self.economy_age_days < MAX_ECONOMY_AGE_DAYS {
            self.economy_age_days += 1;
        }
    }

    /// Avanza el `day_counter` sólo para las unidades cuyo handler económico
    /// nativo lo incrementa. Los vagones sí lo hacen; las partes articuladas
    /// viales retornan antes de llegar a ese incremento.
    pub fn advance_newgrf_day_counter(&mut self) -> bool {
        if !self.participates_in_economy_day() {
            return false;
        }
        self.newgrf_day_counter = self.newgrf_day_counter.wrapping_add(1);
        self.newgrf_day_counter.is_multiple_of(8)
    }

    /// Aplica `DecreaseVehicleValue` con la misma aritmética de dinero entero
    /// que el motor nativo.
    pub fn decrease_vehicle_value(&mut self) {
        self.value -= self.value >> 8;
    }

    fn participates_in_economy_day(&self) -> bool {
        match self.kind {
            VehicleKind::Train | VehicleKind::Ship | VehicleKind::Aircraft => true,
            VehicleKind::Bus | VehicleKind::Truck | VehicleKind::Tram => self.prev_unit.is_none(),
        }
    }

    /// Barrido diario de economía: decaimiento de fiabilidad y acumulación de avería.
    pub fn check_vehicle_breakdown(&mut self, rng: &mut Randomizer) {
        self.check_vehicle_breakdown_with_setting(rng, 2, false);
    }

    /// Variante que respeta `difficulty.vehicle_breakdowns` de `OpenTTD`:
    /// 0=ninguna, 1=reducidas, 2=normales.
    pub(crate) fn check_vehicle_breakdown_with_setting(
        &mut self,
        rng: &mut Randomizer,
        breakdown_level: u8,
        no_servicing_if_no_breakdowns: bool,
    ) {
        if breakdown_level == 0 && no_servicing_if_no_breakdowns {
            return;
        }
        self.reliability = decay_reliability_port(self.reliability, self.reliability_spd_dec);
        self.needs_servicing = self.requires_service();

        if breakdown_level == 0
            || !self.running
            || self.awaiting_load_window
            || self.cargo_transfer_active()
        {
            return;
        }
        if self.breakdown_ctr != 0 {
            return;
        }
        if self.cur_speed < MIN_SPEED_FOR_BREAKDOWN {
            return;
        }

        let r = rng.next();
        let mut chance = u16::from(self.breakdown_chance) + 1;
        if chance16i(1, 25, r) {
            chance += 25;
        }
        self.breakdown_chance = chance.min(255) as u8;

        let threshold = BREAKDOWN_CHANCE_TABLE
            [breakdown_table_index(self.reliability, self.kind, breakdown_level == 1)];
        if u16::from(threshold) > chance {
            return;
        }

        self.breakdown_ctr = extract_bits(r, 16, 6) + 0x3F;
        self.breakdown_delay = extract_bits(r, 24, 7) + 0x80;
        self.breakdown_chance = 0;
    }

    /// Fases de `HandleBreakdown` durante el movimiento.
    ///
    /// Devuelve `true` si el vehículo debe permanecer detenido durante este
    /// tick. El evento de entrada se detecta comparando el estado anterior en
    /// el ciclo de movimiento; `OpenTTD` mantiene este retorno activo durante
    /// toda la avería, no sólo en el tick que la inicia.
    pub fn handle_breakdown(&mut self, _tick: u64) -> bool {
        match self.breakdown_ctr {
            0 => false,
            2 => {
                self.breakdown_ctr = 1;
                self.breakdowns_since_last_service =
                    self.breakdowns_since_last_service.saturating_add(1);
                if self.kind == VehicleKind::Aircraft {
                    return false;
                }
                self.cur_speed = 0;
                self.advance_active_breakdown();
                true
            }
            1 => {
                if self.kind == VehicleKind::Aircraft {
                    return false;
                }
                self.advance_active_breakdown();
                true
            }
            _ => {
                if !self.cargo_loading && !self.cargo_unloading {
                    self.breakdown_ctr -= 1;
                }
                false
            }
        }
    }

    /// Aplica la cadencia nativa a una avería ya activa.
    fn advance_active_breakdown(&mut self) {
        let cadence = if self.kind == VehicleKind::Train {
            4
        } else {
            2
        };
        if self.newgrf_tick_counter.is_multiple_of(cadence) && self.breakdown_delay > 0 {
            self.breakdown_delay -= 1;
            if self.breakdown_delay == 0 {
                self.breakdown_ctr = 0;
            }
        }
    }

    fn is_primary_for_aging(&self) -> bool {
        if self.prev_unit.is_some() {
            return false;
        }
        if self.kind == VehicleKind::Train {
            return self.engine_id.is_none_or(|id| {
                crate::engine::engine_for_vehicle(self.kind, id).is_train_engine()
            });
        }
        true
    }

    /// Edad del vehículo en años de calendario aproximados.
    #[must_use]
    pub fn vehicle_age_years(&self, current_tick: u64) -> u32 {
        let age_ticks = current_tick.saturating_sub(self.build_tick);
        u32::try_from(age_ticks / crate::economy::TICKS_PER_YEAR).unwrap_or(u32::MAX)
    }
}

/// Procesa el barrido diario de calendario: envejecimiento de fiabilidad.
///
/// Cada tick solo procesa vehículos con `index % DAY_TICKS == calendar.date_fract`
/// (`RunVehicleCalendarDayProc`, `vehicle.cpp:937-947`).
pub(crate) fn process_vehicle_calendar_day(state: &mut crate::GameState) {
    let calendar_day = state.calendar.day_index();
    let tick = state.tick.get();
    let fract = usize::from(state.calendar.date_fract);
    let day_ticks = usize::from(crate::timer::DAY_TICKS);
    let mut i = fract;
    while i < state.vehicles.len() {
        state.vehicles[i].sim_tick = tick;
        if state.vehicles[i].prev_unit.is_none() {
            state.vehicles[i].age_vehicle_calendar_day(calendar_day);
        }
        i = i.saturating_add(day_ticks);
    }
}

/// Procesa el barrido diario de economía: riesgo de avería.
///
/// Cada tick solo procesa `index % DAY_TICKS == economy_timer.date_fract`
/// (`RunEconomyVehicleDayProc`, `vehicle.cpp:954-960`).
pub(crate) fn process_vehicle_economy_day(state: &mut crate::GameState) {
    let tick = state.tick.get();
    let world_seed = state.world_seed;
    let fract = usize::from(state.economy_timer.date_fract);
    let day_ticks = usize::from(crate::timer::DAY_TICKS);
    let breakdown_level = state.vehicle_breakdowns.min(2);
    let no_servicing = state.no_servicing_if_no_breakdowns;
    let mut i = fract;
    while i < state.vehicles.len() {
        state.vehicles[i].sim_tick = tick;

        // OpenTTD evaluates CB32 before `OnNewEconomyDay`, when the vehicle's
        // day counter is still at its previous value.  The staggered sweep
        // visits each vehicle once per economy day, so keeping the counter in
        // the persisted model reproduces both the initial callback and the
        // 32-day cadence across save/load.
        let callback_32day = state.vehicles[i].newgrf_day_counter.is_multiple_of(32);
        if callback_32day {
            let engine = state.vehicles[i].engine_id.and_then(|engine_id| {
                state
                    .engine_catalog
                    .iter()
                    .find(|candidate| candidate.id == engine_id)
                    .cloned()
            });
            if let Some(engine) = engine
                && let Some(effect) = crate::newgrf_callback::resolve_vehicle_32day_callback(
                    &engine,
                    &mut state.vehicles[i],
                )
            {
                if effect.invalidate_palette {
                    state.vehicles[i].newgrf_palette_generation =
                        state.vehicles[i].newgrf_palette_generation.wrapping_add(1);
                }
                if effect.trigger_randomisation {
                    crate::newgrf_callback::trigger_vehicle_randomisation(
                        &engine,
                        &mut state.vehicles[i],
                        crate::vehicle::VehicleRandomTrigger::Callback32,
                        world_seed,
                        tick,
                    );
                }
            }
        }
        state.vehicles[i].age_vehicle_economy_day();
        if state.vehicles[i].advance_newgrf_day_counter() {
            state.vehicles[i].decrease_vehicle_value();
        }
        if state.vehicles[i].is_timetable_controller_unit(&state.engine_catalog) {
            // `RoadVehicle::OnNewEconomyDay` omite `CheckVehicleBreakdown`
            // mientras el vehículo sigue bloqueado. El resto del handler
            // (edad, servicio y órdenes) continúa ejecutándose en ese slot.
            let blocked_road_vehicle = matches!(
                state.vehicles[i].kind,
                VehicleKind::Bus | VehicleKind::Truck | VehicleKind::Tram
            ) && state.vehicles[i].blocked_ctr != 0;
            if !blocked_road_vehicle {
                state.vehicles[i].check_vehicle_breakdown_with_setting(
                    &mut state.random,
                    breakdown_level,
                    no_servicing,
                );
            }
            // `RunEconomyVehicleDayProc` llama `OnNewEconomyDay` para este
            // slot; en road vehicles eso incluye `CheckIfRoadVehNeedsService`.
            // Hacerlo aquí (y no para toda la flota al cambiar el día) mantiene
            // el barrido `index % DAY_TICKS`, como OpenTTD.
            check_road_vehicle_needs_service(state, i);
            // El equivalente naval se ejecuta en el mismo callback económico,
            // después de actualizar averías y antes del movimiento del tick.
            check_ship_needs_service(state, i);
        }
        if state.vehicles[i].is_timetable_controller_unit(&state.engine_catalog) {
            // El port cobra costos con acumulación fraccional por tick, pero
            // conserva también el contador nativo para UI y saves. Como en
            // `OnNewEconomyDay`, el período anterior termina después del
            // callback de averías/servicio y antes del movimiento siguiente.
            state.vehicles[i].running_ticks = 0;
        }
        i = i.saturating_add(day_ticks);
    }
}

/// Actualiza `needs_servicing` con la lógica completa de `NeedsServicing`.
pub(crate) fn update_vehicle_servicing_flags(state: &mut crate::GameState) {
    let tick = state.tick.get();
    let len = state.vehicles.len();
    for i in 0..len {
        if state.vehicles[i].prev_unit.is_some() {
            continue;
        }
        state.vehicles[i].sim_tick = tick;
    }
    for i in 0..len {
        if state.vehicles[i].prev_unit.is_some() {
            continue;
        }
        let needs = {
            let state_ref: &crate::GameState = state;
            state_ref.vehicles[i].requires_service_with(state_ref)
        };
        state.vehicles[i].needs_servicing = needs;
    }
}

/// Penalización máxima de desvío para depósito automático (simplificado de `roadveh_cmd.cpp`).
const ROAD_SERVICE_MAX_PENALTY: u32 = 20;

/// Inserta orden de depósito para un barco que necesita servicio.
///
/// `CheckIfShipNeedsService` sólo considera depósitos de la compañía,
/// navegables desde la cuenca actual y dentro de `DistanceSquare <= 80²`.
/// El estado local conserva la configuración `servint_ships` por compañía,
/// por lo que un cero mantiene el servicio automático desactivado.
fn check_ship_needs_service(state: &mut crate::GameState, idx: usize) {
    use crate::depot::{MAX_SHIP_DEPOT_SEARCH_DISTANCE, nearest_reachable_ship_depot_tile_indexed};
    use crate::refit::vehicle_is_in_depot;
    use crate::vehicle::VehicleKind;
    use crate::vehicle::order::VehicleOrder;

    let Some(vehicle) = state.vehicles.get(idx) else {
        return;
    };
    let has_persistent_depot_order = vehicle
        .orders
        .iter()
        .any(|order| matches!(order, VehicleOrder::Depot { stop: true, .. }));
    if vehicle.kind != VehicleKind::Ship
        || !vehicle.running
        || vehicle.prev_unit.is_some()
        || has_persistent_depot_order
        || vehicle.awaiting_load_window
        || vehicle.cargo_transfer_active()
        || state
            .companies
            .get(vehicle.owner.index())
            .is_none_or(|company| company.servint_ships == 0)
    {
        return;
    }

    let needs = {
        let state_ref: &crate::GameState = state;
        state_ref.vehicles[idx].requires_service_with(state_ref)
    };
    if !needs {
        return;
    }
    // `CheckIfShipNeedsService` llama primero a `VehicleServiceInDepot` si la
    // cadena ya está dentro de un depósito. No esperar a la salida importa
    // cuando la orden actual es un depósito manual o de servicio: el barco
    // puede permanecer allí varios días y OpenTTD lo deja listo en este
    // callback económico.
    if vehicle_is_in_depot(&state.map, vehicle) {
        let engine_catalog = state.engine_catalog.clone();
        // Los barcos no tienen unidades `Next()` en el modelo nativo. Este
        // callback también se invoca directamente desde fixtures/cargas antes
        // de reconstruir los índices efímeros, por lo que la operación de una
        // sola nave debe conservar una ruta independiente del `FleetIndex`.
        state.vehicles[idx].service_at_depot_with_catalog(&engine_catalog);
        return;
    }
    let (pos, owner) = {
        let vehicle = &state.vehicles[idx];
        (vehicle.pos, vehicle.owner)
    };
    let Some(depot) = nearest_reachable_ship_depot_tile_indexed(
        &state.map,
        pos,
        owner,
        MAX_SHIP_DEPOT_SEARCH_DISTANCE,
        &mut state.runtime.depot_spatial_index,
    ) else {
        cancel_pending_ship_service_order(state, idx);
        return;
    };
    // Fuera de un depósito, conservar una orden existente evita insertar una
    // segunda parada de servicio. La búsqueda debe ocurrir antes de este
    // filtro: si el depósito temporal dejó de ser válido, OpenTTD convierte
    // la orden activa en `Dummy` en vez de dejarla atascada.
    if vehicle
        .orders
        .iter()
        .any(|order| matches!(order, VehicleOrder::Depot { .. }))
    {
        return;
    }
    let vehicle = &mut state.vehicles[idx];
    vehicle.needs_servicing = true;
    vehicle.orders.insert(
        vehicle.current_order,
        VehicleOrder::depot_pass_through(depot),
    );
    vehicle.path.clear();
    vehicle.sync_order_destination_with_stations(&state.map, &state.stations);
}

/// Retira la orden de servicio automática si `OpenTTD` ya no encuentra un
/// depósito propio alcanzable.
///
/// El runtime nativo convierte sólo su `current_order` temporal en `Dummy`;
/// la lista persistente de órdenes no pierde el circuito del jugador. El
/// modelo local representa esa orden temporal dentro de `orders`, por lo que
/// hay que eliminar la entrada `stop:false` que este módulo insertó y volver a
/// sincronizar el destino de la orden real que queda debajo.
fn cancel_pending_ship_service_order(state: &mut crate::GameState, idx: usize) {
    let Some(vehicle) = state.vehicles.get(idx) else {
        return;
    };
    if !matches!(
        vehicle.current_order_ref(),
        Some(crate::vehicle::order::VehicleOrder::Depot { stop: false, .. })
    ) {
        return;
    }
    let order_idx = vehicle.current_order;
    {
        let vehicle = &mut state.vehicles[idx];
        if order_idx >= vehicle.orders.len() {
            return;
        }
        vehicle.orders.remove(order_idx);
        vehicle.sanitize_current_order();
        vehicle.path.clear();
        vehicle.no_network_route_to_order = false;
        if vehicle.orders.is_empty() {
            vehicle.dest = vehicle.pos;
        }
    }
    if !state.vehicles[idx].orders.is_empty() {
        state.vehicles[idx].sync_order_destination_with_stations(&state.map, &state.stations);
    }
}

/// Inserta orden de depósito para el vehículo road de un slot de economía
/// (`CheckIfRoadVehNeedsService`).
///
/// Se invoca desde [`process_vehicle_economy_day`], que reparte los vehículos
/// entre los 74 ticks diarios. No debe convertirse en un barrido de la flota al
/// iniciar el día: en una partida grande eso concentra miles de A* en un tick.
fn check_road_vehicle_needs_service(state: &mut crate::GameState, idx: usize) {
    use crate::depot::nearest_reachable_depot_tile_indexed;
    use crate::vehicle::VehicleKind;
    use crate::vehicle::order::VehicleOrder;

    let Some(vehicle) = state.vehicles.get(idx) else {
        return;
    };
    if !matches!(
        vehicle.kind,
        VehicleKind::Bus | VehicleKind::Truck | VehicleKind::Tram
    ) || !vehicle.running
        || vehicle.prev_unit.is_some()
        || vehicle
            .orders
            .iter()
            .any(|o| matches!(o, VehicleOrder::Depot { .. }))
    {
        return;
    }

    let needs = {
        let state_ref: &crate::GameState = state;
        state_ref.vehicles[idx].requires_service_with(state_ref)
    };
    if !needs {
        return;
    }
    let (pos, kind) = {
        let v = &state.vehicles[idx];
        (v.pos, v.kind)
    };
    let Some(depot) = nearest_reachable_depot_tile_indexed(
        &state.map,
        pos,
        kind,
        &mut state.runtime.depot_spatial_index,
    ) else {
        return;
    };
    let dist = crate::economy::manhattan_distance(pos, depot);
    if dist > ROAD_SERVICE_MAX_PENALTY {
        return;
    }
    let vehicle = &mut state.vehicles[idx];
    vehicle.needs_servicing = true;
    vehicle.orders.insert(
        vehicle.current_order,
        VehicleOrder::depot_pass_through(depot),
    );
    vehicle.path.clear();
    vehicle.sync_order_destination_with_stations(&state.map, &state.stations);
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::map::TileCoord;
    use crate::vehicle::Vehicle;

    #[test]
    fn breakdown_table_uses_scaled_reliability() {
        assert_eq!(breakdown_table_index(10_000, VehicleKind::Bus, false), 63);
        assert_eq!(breakdown_table_index(1_000, VehicleKind::Bus, false), 6);
        assert_eq!(
            breakdown_table_index(1_000, VehicleKind::Ship, false),
            breakdown_table_index(5_000, VehicleKind::Bus, false)
        );
        assert!(
            breakdown_table_index(1_000, VehicleKind::Bus, true)
                > breakdown_table_index(1_000, VehicleKind::Bus, false)
        );
    }

    #[test]
    fn reliability_decays_by_engine_spd_dec() {
        let mut v = Vehicle::new(
            1,
            VehicleKind::Bus,
            TileCoord::new(0, 0),
            TileCoord::new(1, 0),
        );
        v.reliability = 5_000;
        v.reliability_spd_dec = 80;
        v.running = true;
        let before = v.reliability;
        v.check_vehicle_breakdown(&mut Randomizer::new(1));
        assert!(v.reliability < before);
    }

    #[test]
    fn sync_reliability_initializes_vehicle_from_variant_parent() {
        let mut parent =
            crate::engine::engine_for_vehicle(VehicleKind::Ship, crate::engine::ENGINE_SHIP_MPS)
                .clone();
        parent.id = 20_011;
        parent.reliability_pct = 61;
        parent.reliability_spd_dec = 44;
        parent.lifelength_years = 17;
        let mut child =
            crate::engine::engine_for_vehicle(VehicleKind::Ship, crate::engine::ENGINE_SHIP_OIL)
                .clone();
        child.id = 20_012;
        child.variant_parent_id = Some(parent.id);
        child.extra_flags = crate::engine::EXTRA_ENGINE_FLAG_SYNC_RELIABILITY;
        child.reliability_pct = 91;
        child.reliability_spd_dec = 99;
        child.lifelength_years = 30;

        let catalog = [parent, child.clone()];
        let mut vehicle = Vehicle::new(
            1,
            VehicleKind::Ship,
            TileCoord::new(0, 0),
            TileCoord::new(1, 0),
        );
        init_vehicle_reliability_from_engine_with_catalog(&mut vehicle, &child, &catalog);

        assert_eq!(vehicle.reliability, 6_100);
        assert_eq!(vehicle.reliability_spd_dec, 44);
        assert_eq!(vehicle.max_age_days, 17 * DAYS_PER_VEHICLE_YEAR);
    }

    #[test]
    fn sync_reliability_survives_service_at_depot() {
        let mut parent =
            crate::engine::engine_for_vehicle(VehicleKind::Ship, crate::engine::ENGINE_SHIP_MPS)
                .clone();
        parent.id = 20_021;
        parent.reliability_pct = 61;
        parent.reliability_spd_dec = 44;
        parent.lifelength_years = 17;
        let mut child =
            crate::engine::engine_for_vehicle(VehicleKind::Ship, crate::engine::ENGINE_SHIP_OIL)
                .clone();
        child.id = 20_022;
        child.variant_parent_id = Some(parent.id);
        child.extra_flags = crate::engine::EXTRA_ENGINE_FLAG_SYNC_RELIABILITY;

        let catalog = [parent, child.clone()];
        let mut vehicle = Vehicle::new(
            1,
            VehicleKind::Ship,
            TileCoord::new(0, 0),
            TileCoord::new(1, 0),
        );
        vehicle.engine_id = Some(child.id);
        vehicle.reliability = 1_000;
        vehicle.needs_servicing = true;
        vehicle.breakdown_chance = 200;
        vehicle.breakdowns_since_last_service = 9;
        vehicle.last_service_newgrf_day = -7;
        vehicle.sim_tick = u64::from(crate::economy::TICKS_PER_DAY) * 11;
        vehicle.service_at_depot_with_catalog(&catalog);

        assert_eq!(vehicle.reliability, 6_100);
        assert_eq!(vehicle.reliability_spd_dec, 44);
        assert_eq!(vehicle.max_age_days, 17 * DAYS_PER_VEHICLE_YEAR);
        assert!(!vehicle.needs_servicing);
        assert_eq!(vehicle.breakdown_chance, 50);
        assert_eq!(vehicle.breakdowns_since_last_service, 0);
        assert_eq!(vehicle.last_service_day, 11);
        assert_eq!(vehicle.last_service_newgrf_day, 11);
    }

    #[test]
    fn reduced_breakdowns_service_clears_post_service_chance() {
        let mut vehicle = Vehicle::new(
            1,
            VehicleKind::Bus,
            TileCoord::new(0, 0),
            TileCoord::new(1, 0),
        );
        vehicle.service_breakdown_level = 1;
        vehicle.breakdown_chance = 200;

        vehicle.service_at_depot();

        assert_eq!(vehicle.breakdown_chance, 0);
    }

    #[test]
    fn service_at_depot_updates_linked_engine_units() {
        let depot = TileCoord::new(0, 0);
        let mut head = Vehicle::new(1, VehicleKind::Train, depot, depot);
        let mut tail = Vehicle::new(2, VehicleKind::Train, depot, depot);
        head.next_unit = Some(tail.id);
        tail.prev_unit = Some(head.id);
        head.breakdown_chance = 200;
        head.breakdowns_since_last_service = 3;
        head.reliability = 1_000;
        head.sim_tick = u64::from(crate::economy::TICKS_PER_DAY) * 4;
        tail.breakdown_chance = 200;
        tail.breakdowns_since_last_service = 3;
        tail.reliability = 1_000;
        tail.sim_tick = u64::from(crate::economy::TICKS_PER_DAY) * 4;

        let mut vehicles = vec![head, tail];
        let mut fleet = crate::fleet_index::FleetIndex::default();
        fleet.rebuild(&vehicles);

        service_vehicle_chain_with_catalog(&mut vehicles, &fleet, 1, &[]);

        for vehicle in &vehicles {
            assert_eq!(vehicle.breakdown_chance, 50);
            assert_eq!(vehicle.breakdowns_since_last_service, 0);
            assert_eq!(vehicle.last_service_day, 4);
        }
    }

    #[test]
    fn disabled_breakdowns_never_accumulate_or_trigger() {
        let mut v = Vehicle::new(
            1,
            VehicleKind::Train,
            TileCoord::new(0, 0),
            TileCoord::new(1, 0),
        );
        v.running = true;
        v.cur_speed = 100;
        v.reliability = 100;
        v.breakdown_chance = 250;
        v.check_vehicle_breakdown_with_setting(&mut Randomizer::new(7), 0, false);
        assert_eq!(v.breakdown_ctr, 0);
        assert_eq!(v.breakdown_chance, 250);
    }

    #[test]
    fn reliability_spd_dec_doubles_after_max_age_year_boundary() {
        let mut v = Vehicle::new(
            1,
            VehicleKind::Bus,
            TileCoord::new(0, 0),
            TileCoord::new(1, 0),
        );
        v.reliability_spd_dec = 80;
        v.max_age_days = DAYS_PER_VEHICLE_YEAR;
        v.build_tick = 0;
        let calendar_day = u64::from(DAYS_PER_VEHICLE_YEAR);
        v.age_vehicle_calendar_day(calendar_day);
        assert_eq!(v.reliability_spd_dec, 160);
    }

    #[test]
    fn breakdown_requires_min_speed_for_chance() {
        let mut v = Vehicle::new(
            1,
            VehicleKind::Truck,
            TileCoord::new(0, 0),
            TileCoord::new(1, 0),
        );
        v.reliability = 100;
        v.running = true;
        v.cur_speed = 0;
        v.check_vehicle_breakdown(&mut Randomizer::new(99));
        assert_eq!(v.breakdown_chance, 0);
        assert_eq!(v.breakdown_ctr, 0);
    }

    #[test]
    fn handle_breakdown_stops_vehicle_at_phase_two() {
        let mut v = Vehicle::new(
            1,
            VehicleKind::Bus,
            TileCoord::new(0, 0),
            TileCoord::new(1, 0),
        );
        v.breakdown_ctr = 2;
        v.breakdown_delay = 120;
        v.cur_speed = 40;
        assert!(v.handle_breakdown(0));
        assert_eq!(v.breakdown_ctr, 1);
        assert_eq!(v.cur_speed, 0);
        assert_eq!(v.breakdowns_since_last_service, 1);
    }

    #[test]
    fn breakdown_delay_uses_native_vehicle_cadence() {
        let mut v = Vehicle::new(
            1,
            VehicleKind::Ship,
            TileCoord::new(0, 0),
            TileCoord::new(1, 0),
        );
        v.breakdown_ctr = 1;
        v.breakdown_delay = 2;

        v.newgrf_tick_counter = 1;
        assert!(v.handle_breakdown(0));
        assert_eq!(v.breakdown_delay, 2);

        v.newgrf_tick_counter = 2;
        assert!(v.handle_breakdown(0));
        assert_eq!(v.breakdown_delay, 1);
    }

    #[test]
    fn breakdown_start_applies_active_cadence_before_movement() {
        let mut v = Vehicle::new(
            1,
            VehicleKind::Ship,
            TileCoord::new(0, 0),
            TileCoord::new(1, 0),
        );
        v.breakdown_ctr = 2;
        v.breakdown_delay = 1;
        v.newgrf_tick_counter = 2;
        v.cur_speed = 40;

        assert!(v.handle_breakdown(0));
        assert_eq!(v.breakdown_ctr, 0);
        assert_eq!(v.breakdown_delay, 0);
        assert_eq!(v.breakdowns_since_last_service, 1);
        assert_eq!(v.cur_speed, 0);
    }

    #[test]
    fn breakdown_counter_saturates_at_native_limit() {
        let mut v = Vehicle::new(
            1,
            VehicleKind::Aircraft,
            TileCoord::new(0, 0),
            TileCoord::new(1, 0),
        );
        v.breakdown_ctr = 2;
        v.breakdowns_since_last_service = u8::MAX;

        assert!(!v.handle_breakdown(0));
        assert_eq!(v.breakdowns_since_last_service, u8::MAX);
    }

    #[test]
    fn staggered_day_sweep_processes_one_slot_per_tick() {
        let mut state = crate::GameState::new(8, 8);
        for i in 0..crate::timer::DAY_TICKS {
            let mut v = Vehicle::new(
                u32::from(i) + 1,
                VehicleKind::Bus,
                TileCoord::new(1, 1),
                TileCoord::new(2, 1),
            );
            v.reliability = 5_000;
            v.reliability_spd_dec = 80;
            v.running = true;
            v.cur_speed = 40;
            state.vehicles.push(v);
        }
        let before: Vec<u16> = state.vehicles.iter().map(|v| v.reliability).collect();
        state.calendar.date_fract = 3;
        state.economy_timer.date_fract = 3;
        process_vehicle_calendar_day(&mut state);
        process_vehicle_economy_day(&mut state);
        let changed: Vec<usize> = state
            .vehicles
            .iter()
            .enumerate()
            .filter(|(i, v)| v.reliability != before[*i])
            .map(|(i, _)| i)
            .collect();
        assert_eq!(changed, vec![3]);
    }

    #[test]
    fn economy_day_resets_running_ticks_for_controller_units() {
        let mut state = crate::GameState::new(8, 8);
        let mut vehicle = Vehicle::new(
            1,
            VehicleKind::Bus,
            TileCoord::new(1, 1),
            TileCoord::new(2, 1),
        );
        vehicle.running_ticks = 37;
        state.vehicles.push(vehicle);
        state.economy_timer.date_fract = 0;

        process_vehicle_economy_day(&mut state);

        assert_eq!(state.vehicles[0].running_ticks, 0);
    }

    #[test]
    fn economy_age_follows_native_vehicle_unit_rules() {
        let pos = TileCoord::new(1, 1);
        let mut train = Vehicle::new(1, VehicleKind::Train, pos, pos);
        let mut wagon = Vehicle::new(2, VehicleKind::Train, pos, pos);
        wagon.prev_unit = Some(train.id);
        let mut bus = Vehicle::new(3, VehicleKind::Bus, pos, pos);
        let mut articulated_bus = Vehicle::new(4, VehicleKind::Bus, pos, pos);
        articulated_bus.prev_unit = Some(bus.id);
        articulated_bus.newgrf_articulated = true;
        let mut ship = Vehicle::new(5, VehicleKind::Ship, pos, pos);
        let mut aircraft = Vehicle::new(6, VehicleKind::Aircraft, pos, pos);

        for vehicle in [
            &mut train,
            &mut wagon,
            &mut bus,
            &mut articulated_bus,
            &mut ship,
            &mut aircraft,
        ] {
            vehicle.economy_age_days = 17;
            vehicle.age_vehicle_economy_day();
        }

        assert_eq!(train.economy_age_days, 18);
        assert_eq!(wagon.economy_age_days, 18);
        assert_eq!(bus.economy_age_days, 18);
        assert_eq!(articulated_bus.economy_age_days, 17);
        assert_eq!(ship.economy_age_days, 18);
        assert_eq!(aircraft.economy_age_days, 18);
    }

    #[test]
    fn economy_age_stays_at_native_maximum() {
        let pos = TileCoord::new(1, 1);
        let mut vehicle = Vehicle::new(1, VehicleKind::Ship, pos, pos);
        vehicle.economy_age_days = MAX_ECONOMY_AGE_DAYS;

        vehicle.age_vehicle_economy_day();

        assert_eq!(vehicle.economy_age_days, MAX_ECONOMY_AGE_DAYS);
    }

    #[test]
    fn economy_day_sweep_advances_only_the_selected_age_slot() {
        let mut state = crate::GameState::new(8, 8);
        for id in 1..=2 {
            let mut vehicle = Vehicle::new(
                id,
                VehicleKind::Bus,
                TileCoord::new(1, 1),
                TileCoord::new(2, 1),
            );
            vehicle.economy_age_days = 9;
            state.vehicles.push(vehicle);
        }
        state.economy_timer.date_fract = 1;

        process_vehicle_economy_day(&mut state);

        assert_eq!(state.vehicles[0].economy_age_days, 9);
        assert_eq!(state.vehicles[1].economy_age_days, 10);
    }

    #[test]
    fn newgrf_day_counter_matches_native_economy_units() {
        let pos = TileCoord::new(1, 1);
        let mut train = Vehicle::new(1, VehicleKind::Train, pos, pos);
        let mut wagon = Vehicle::new(2, VehicleKind::Train, pos, pos);
        wagon.prev_unit = Some(train.id);
        let mut road_head = Vehicle::new(3, VehicleKind::Bus, pos, pos);
        let mut articulated_road = Vehicle::new(4, VehicleKind::Bus, pos, pos);
        articulated_road.prev_unit = Some(road_head.id);
        articulated_road.newgrf_articulated = true;
        let mut ship = Vehicle::new(5, VehicleKind::Ship, pos, pos);
        let mut aircraft = Vehicle::new(6, VehicleKind::Aircraft, pos, pos);

        for vehicle in [
            &mut train,
            &mut wagon,
            &mut road_head,
            &mut articulated_road,
            &mut ship,
            &mut aircraft,
        ] {
            vehicle.newgrf_day_counter = 41;
            vehicle.advance_newgrf_day_counter();
        }

        assert_eq!(train.newgrf_day_counter, 42);
        assert_eq!(wagon.newgrf_day_counter, 42);
        assert_eq!(road_head.newgrf_day_counter, 42);
        assert_eq!(articulated_road.newgrf_day_counter, 41);
        assert_eq!(ship.newgrf_day_counter, 42);
        assert_eq!(aircraft.newgrf_day_counter, 42);
    }

    #[test]
    fn economy_day_depreciates_value_on_every_eighth_unit_day() {
        let mut state = crate::GameState::new(8, 8);
        let mut vehicle = Vehicle::new(
            1,
            VehicleKind::Bus,
            TileCoord::new(1, 1),
            TileCoord::new(2, 1),
        );
        vehicle.newgrf_day_counter = 7;
        vehicle.value = 25_600;
        state.vehicles.push(vehicle);
        state.economy_timer.date_fract = 0;

        process_vehicle_economy_day(&mut state);

        assert_eq!(state.vehicles[0].newgrf_day_counter, 8);
        assert_eq!(state.vehicles[0].value, 25_500);
    }

    #[test]
    fn free_train_wagon_does_not_run_front_engine_economy_handler() {
        let pos = TileCoord::new(1, 1);
        let mut wagon = Vehicle::new(1, VehicleKind::Train, pos, pos);
        wagon.engine_id = Some(crate::engine::ENGINE_WAGON_COAL);
        wagon.running = true;
        wagon.cur_speed = 100;
        wagon.reliability = 1_000;
        wagon.reliability_spd_dec = 80;
        wagon.breakdown_chance = u8::MAX;
        wagon.needs_servicing = false;
        let reliability_before = wagon.reliability;
        let mut state = crate::GameState::new(8, 8);
        state.vehicles.push(wagon);
        state.economy_timer.date_fract = 0;

        process_vehicle_economy_day(&mut state);

        assert_eq!(state.vehicles[0].reliability, reliability_before);
        assert_eq!(state.vehicles[0].breakdown_chance, u8::MAX);
        assert_eq!(state.vehicles[0].breakdown_ctr, 0);
        assert!(!state.vehicles[0].needs_servicing);
    }

    #[test]
    fn stopped_vehicle_still_decays_reliability_in_economy_sweep() {
        let pos = TileCoord::new(1, 1);
        let mut vehicle = Vehicle::new(1, VehicleKind::Bus, pos, pos);
        vehicle.running = false;
        vehicle.reliability = 5_000;
        vehicle.reliability_spd_dec = 80;
        vehicle.breakdown_chance = 17;
        let reliability_before = vehicle.reliability;
        let mut state = crate::GameState::new(8, 8);
        state.vehicles.push(vehicle);
        state.economy_timer.date_fract = 0;

        process_vehicle_economy_day(&mut state);

        assert!(state.vehicles[0].reliability < reliability_before);
        assert_eq!(state.vehicles[0].breakdown_chance, 17);
        assert_eq!(state.vehicles[0].breakdown_ctr, 0);
    }

    #[test]
    fn reduced_breakdowns_decay_reliability_during_loading_window() {
        let pos = TileCoord::new(1, 1);
        let mut vehicle = Vehicle::new(1, VehicleKind::Bus, pos, pos);
        vehicle.running = true;
        vehicle.awaiting_load_window = true;
        vehicle.cur_speed = 100;
        vehicle.reliability = 5_000;
        vehicle.reliability_spd_dec = 80;
        vehicle.breakdown_chance = 17;
        let reliability_before = vehicle.reliability;

        vehicle.check_vehicle_breakdown_with_setting(&mut Randomizer::new(7), 1, false);

        assert!(vehicle.reliability < reliability_before);
        assert_eq!(vehicle.breakdown_chance, 17);
        assert_eq!(vehicle.breakdown_ctr, 0);
    }

    #[test]
    fn blocked_road_vehicle_skips_economy_breakdown_check() {
        let pos = TileCoord::new(1, 1);
        let mut vehicle = Vehicle::new(1, VehicleKind::Bus, pos, pos);
        vehicle.running = true;
        vehicle.cur_speed = 100;
        vehicle.blocked_ctr = 1;
        vehicle.reliability = 5_000;
        vehicle.reliability_spd_dec = 80;
        vehicle.breakdown_chance = 17;
        let reliability_before = vehicle.reliability;
        let mut state = crate::GameState::new(8, 8);
        state.vehicles.push(vehicle);
        state.economy_timer.date_fract = 0;

        process_vehicle_economy_day(&mut state);

        assert_eq!(state.vehicles[0].reliability, reliability_before);
        assert_eq!(state.vehicles[0].breakdown_chance, 17);
        assert_eq!(state.vehicles[0].breakdown_ctr, 0);
    }

    #[test]
    fn road_vehicle_service_check_runs_in_its_economy_slot() {
        use crate::vehicle::order::VehicleOrder;
        use crate::{Command, GameState, apply_command};

        let mut state = GameState::new(12, 12);
        let depot = TileCoord::new(6, 4);
        let road = TileCoord::new(3, 4);
        for x in 2..=5 {
            apply_command(
                &mut state,
                &Command::PlaceRoadBits(TileCoord::new(x, 4), 0x0F),
            )
            .unwrap();
        }
        apply_command(&mut state, &Command::PlaceRoadDepotDir(depot, 0)).unwrap();
        for id in [1, 2] {
            let mut v = Vehicle::new(id, VehicleKind::Bus, road, TileCoord::new(6, 4));
            v.running = true;
            v.service_interval_days = 1;
            v.last_service_day = 0;
            v.orders = vec![VehicleOrder::station(TileCoord::new(8, 4))];
            state.vehicles.push(v);
        }
        state.tick = crate::GameTick::new(u64::from(crate::economy::TICKS_PER_DAY));
        state.sync_timers_from_tick();
        state.economy_timer.date_fract = 1;
        process_vehicle_economy_day(&mut state);
        assert!(matches!(
            state.vehicles[1].orders[0],
            VehicleOrder::Depot { stop: false, .. }
        ));
        assert!(matches!(
            state.vehicles[0].orders[0],
            VehicleOrder::Station { .. }
        ));

        state.economy_timer.date_fract = 0;
        process_vehicle_economy_day(&mut state);
        assert!(matches!(
            state.vehicles[0].orders[0],
            VehicleOrder::Depot { stop: false, .. }
        ));
    }

    #[test]
    fn ship_vehicle_service_check_uses_own_reachable_depot() {
        use crate::vehicle::order::VehicleOrder;
        use crate::{
            Command, GameState, TileKind, VehicleKind, WaterClass, apply_command,
            ship_depot_footprint,
        };

        let mut state = GameState::new(24, 8);
        for y in [2_i32, 3_i32] {
            for x in 0..24_i32 {
                crate::map::make_water_tile(&mut state.map, TileCoord::new(x, y), WaterClass::Sea)
                    .unwrap();
            }
        }
        let rival_depot = TileCoord::new(5, 2);
        let own_depot = TileCoord::new(17, 2);
        apply_command(&mut state, &Command::PlaceShipDepotDir(rival_depot, 3)).unwrap();
        apply_command(&mut state, &Command::PlaceShipDepotDir(own_depot, 3)).unwrap();
        for tile in ship_depot_footprint(rival_depot, 3) {
            let mut raw = state.map.get(tile).unwrap();
            raw.m1 = (raw.m1 & !0x1F) | 1;
            state.map.set_tile(tile, raw).unwrap();
        }
        state.companies[0].servint_ships = 360;

        let from = TileCoord::new(7, 2);
        let mut ship = Vehicle::new(1, VehicleKind::Ship, from, from);
        ship.running = true;
        ship.service_interval_days = 1;
        ship.last_service_day = 0;
        ship.orders = vec![VehicleOrder::station(TileCoord::new(20, 2))];
        state.vehicles.push(ship);
        state.tick = crate::GameTick::new(u64::from(crate::economy::TICKS_PER_DAY));
        state.sync_timers_from_tick();
        state.economy_timer.date_fract = 0;

        process_vehicle_economy_day(&mut state);

        assert!(matches!(
            state.vehicles[0].orders[0],
            VehicleOrder::Depot {
                depot,
                stop: false,
                ..
            } if depot == own_depot
        ));
        assert_eq!(state.vehicles[0].dest, own_depot);
        assert_eq!(state.map.get_kind(rival_depot), Some(TileKind::ShipDepot));
    }

    #[test]
    fn service_interval_remains_due_during_breakdown_countdown() {
        let mut vehicle = Vehicle::new(
            1,
            VehicleKind::Bus,
            TileCoord::new(0, 0),
            TileCoord::new(1, 0),
        );
        vehicle.running = true;
        vehicle.service_interval_days = 1;
        vehicle.last_service_day = 0;
        vehicle.sim_tick = u64::from(crate::economy::TICKS_PER_DAY);
        vehicle.breakdown_ctr = 64;

        assert!(vehicle.requires_service_for_company(false));
    }

    #[test]
    fn ship_service_check_services_a_ship_already_inside_depot() {
        use crate::vehicle::order::VehicleOrder;
        use crate::{Command, GameState, TileKind, VehicleKind, WaterClass, apply_command};

        let mut state = GameState::new(12, 8);
        let depot = TileCoord::new(5, 3);
        for y in [2_i32, 3_i32] {
            for x in 0..12_i32 {
                crate::map::make_water_tile(&mut state.map, TileCoord::new(x, y), WaterClass::Sea)
                    .unwrap();
            }
        }
        apply_command(&mut state, &Command::PlaceShipDepotDir(depot, 2)).unwrap();
        let north = crate::ship_depot_north_tile(&state.map, depot).unwrap();
        let target = TileCoord::new(8, 2);
        let mut ship = Vehicle::new(1, VehicleKind::Ship, north, north);
        ship.running = true;
        ship.ship_state = crate::ship_movement::SHIP_STATE_DEPOT;
        ship.service_interval_days = 1;
        ship.last_service_day = 0;
        ship.reliability = 1_000;
        ship.needs_servicing = true;
        ship.orders = vec![VehicleOrder::station(target)];
        state.vehicles.push(ship);
        state.companies[0].servint_ships = 360;
        state.tick = crate::GameTick::new(u64::from(crate::economy::TICKS_PER_DAY));
        state.sync_timers_from_tick();
        state.economy_timer.date_fract = 0;

        process_vehicle_economy_day(&mut state);

        assert_eq!(
            state.vehicles[0].reliability,
            initial_reliability_for_engine(
                crate::engine::default_engine_id(VehicleKind::Ship),
                VehicleKind::Ship,
            )
        );
        assert!(!state.vehicles[0].needs_servicing);
        assert_eq!(state.vehicles[0].orders.len(), 1);
        assert!(matches!(
            state.vehicles[0].current_order_ref(),
            Some(VehicleOrder::Station { station, .. }) if *station == target
        ));
        assert_eq!(state.map.get_kind(north), Some(TileKind::ShipDepot));
    }

    #[test]
    fn ship_service_check_does_not_interrupt_existing_depot_order() {
        use crate::vehicle::order::VehicleOrder;
        use crate::{Command, GameState, TileKind, VehicleKind, WaterClass, apply_command};

        let mut state = GameState::new(12, 8);
        for y in [2_i32, 3_i32] {
            for x in 0..12_i32 {
                crate::map::make_water_tile(&mut state.map, TileCoord::new(x, y), WaterClass::Sea)
                    .unwrap();
            }
        }
        let depot = TileCoord::new(5, 3);
        apply_command(&mut state, &Command::PlaceShipDepotDir(depot, 2)).unwrap();
        let north = crate::ship_depot_north_tile(&state.map, depot).unwrap();
        let mut ship = Vehicle::new(1, VehicleKind::Ship, north, north);
        ship.running = true;
        ship.ship_state = crate::ship_movement::SHIP_STATE_DEPOT;
        ship.service_interval_days = 1;
        ship.last_service_day = 0;
        ship.reliability = 1_000;
        ship.reliability_spd_dec = 0;
        ship.needs_servicing = true;
        ship.orders = vec![VehicleOrder::depot(north)];
        state.vehicles.push(ship);
        state.companies[0].servint_ships = 360;
        state.tick = crate::GameTick::new(u64::from(crate::economy::TICKS_PER_DAY));
        state.sync_timers_from_tick();
        state.economy_timer.date_fract = 0;

        process_vehicle_economy_day(&mut state);

        assert_eq!(state.vehicles[0].reliability, 1_000);
        assert!(state.vehicles[0].needs_servicing);
        assert_eq!(state.vehicles[0].orders, vec![VehicleOrder::depot(north)]);
        assert_eq!(state.map.get_kind(north), Some(TileKind::ShipDepot));
    }

    #[test]
    fn ship_service_order_is_removed_when_no_owned_depot_is_reachable() {
        use crate::vehicle::order::VehicleOrder;
        use crate::{Command, GameState, TileKind, VehicleKind, WaterClass, apply_command};

        let mut state = GameState::new(24, 8);
        for y in [2_i32, 3_i32] {
            for x in 0..24_i32 {
                crate::map::make_water_tile(&mut state.map, TileCoord::new(x, y), WaterClass::Sea)
                    .unwrap();
            }
        }
        let depot = TileCoord::new(17, 2);
        apply_command(&mut state, &Command::PlaceShipDepotDir(depot, 3)).unwrap();
        state.companies[0].servint_ships = 360;

        let from = TileCoord::new(7, 2);
        let target = TileCoord::new(20, 2);
        let mut ship = Vehicle::new(1, VehicleKind::Ship, from, from);
        ship.running = true;
        ship.service_interval_days = 1;
        ship.last_service_day = 0;
        ship.orders = vec![VehicleOrder::station(target)];
        state.vehicles.push(ship);
        state.tick = crate::GameTick::new(u64::from(crate::economy::TICKS_PER_DAY));
        state.sync_timers_from_tick();
        state.economy_timer.date_fract = 0;

        process_vehicle_economy_day(&mut state);
        assert!(matches!(
            state.vehicles[0].current_order_ref(),
            Some(VehicleOrder::Depot { stop: false, .. })
        ));

        for tile in crate::ship_depot_footprint(depot, 3) {
            let mut raw = state.map.get(tile).unwrap();
            raw.m1 = (raw.m1 & !0x1F) | 1;
            state.map.set_tile(tile, raw).unwrap();
        }
        process_vehicle_economy_day(&mut state);

        assert_eq!(state.vehicles[0].orders.len(), 1);
        assert!(matches!(
            state.vehicles[0].current_order_ref(),
            Some(VehicleOrder::Station { station, .. }) if *station == target
        ));
        assert_eq!(state.vehicles[0].dest, target);
        assert_eq!(state.map.get_kind(depot), Some(TileKind::ShipDepot));
    }
}
