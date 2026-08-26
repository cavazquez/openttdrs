//! Fiabilidad, servicio y averías del vehículo.

use crate::cargodist::parity::Randomizer;
use crate::vehicle::VehicleKind;

/// Umbral de fiabilidad bajo el cual conviene servicio en depósito.
pub const SERVICING_RELIABILITY_THRESHOLD: u16 = 5_000;
/// Intervalo de revisión por defecto (`OpenTTD` `service_interval` ≈ 150 días).
pub const DEFAULT_SERVICE_INTERVAL_DAYS: u16 = 150;
/// Duración máxima de avería en ticks (`breakdown_delay` hasta 255).
pub const BREAKDOWN_DURATION_TICKS: u32 = 255;
/// Velocidad mínima para acumular riesgo de avería (`vehicle.cpp:1340`).
pub const MIN_SPEED_FOR_BREAKDOWN: u16 = 5;
/// Días de calendario por año (paridad `CalendarTime::DAYS_IN_LEAP_YEAR`).
pub const DAYS_PER_VEHICLE_YEAR: u32 = 366;

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
        self.breakdown_chance = 0;
        self.last_service_day =
            crate::news::calendar_day_index(crate::tick::GameTick::new(self.sim_tick));
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
        if self.breakdown_ctr != 0 {
            return false;
        }
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
        if !self.running {
            return;
        }
        if breakdown_level == 0 && no_servicing_if_no_breakdowns {
            return;
        }
        if breakdown_level == 1 && (self.awaiting_load_window || self.cargo_transfer_active()) {
            return;
        }
        self.reliability = decay_reliability_port(self.reliability, self.reliability_spd_dec);
        self.needs_servicing = self.requires_service();

        if breakdown_level == 0 {
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
    /// Devuelve `true` si el vehículo acaba de entrar en avería (humo/sonido).
    pub fn handle_breakdown(&mut self, tick: u64) -> bool {
        match self.breakdown_ctr {
            0 => false,
            2 => {
                self.breakdown_ctr = 1;
                if self.kind != VehicleKind::Aircraft {
                    self.cur_speed = 0;
                }
                true
            }
            1 => {
                if self.kind == VehicleKind::Aircraft {
                    return false;
                }
                let half_rate = self.kind == VehicleKind::Train && (tick & 3) != 0;
                if !half_rate && self.breakdown_delay > 0 {
                    self.breakdown_delay -= 1;
                    if self.breakdown_delay == 0 {
                        self.breakdown_ctr = 0;
                    }
                }
                false
            }
            _ => {
                if !self.cargo_loading && !self.cargo_unloading {
                    self.breakdown_ctr -= 1;
                }
                false
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
                && effect.trigger_randomisation
            {
                crate::newgrf_callback::trigger_vehicle_randomisation(
                    &engine,
                    &mut state.vehicles[i],
                    crate::vehicle::VehicleRandomTrigger::Callback32,
                    world_seed,
                    tick,
                );
                // `invalidate_palette` is represented by the changed Action2
                // fingerprint/random bits.  The explicit result is retained
                // in the resolver API so the client cache can consume it when
                // colour callbacks are wired into the renderer.
            }
        }
        state.vehicles[i].newgrf_day_counter = state.vehicles[i].newgrf_day_counter.wrapping_add(1);
        if state.vehicles[i].prev_unit.is_none() {
            state.vehicles[i].check_vehicle_breakdown_with_setting(
                &mut state.random,
                breakdown_level,
                no_servicing,
            );
            // `RunEconomyVehicleDayProc` llama `OnNewEconomyDay` para este
            // slot; en road vehicles eso incluye `CheckIfRoadVehNeedsService`.
            // Hacerlo aquí (y no para toda la flota al cambiar el día) mantiene
            // el barrido `index % DAY_TICKS`, como OpenTTD.
            check_road_vehicle_needs_service(state, i);
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
    vehicle.sync_order_destination(&state.map);
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
}
