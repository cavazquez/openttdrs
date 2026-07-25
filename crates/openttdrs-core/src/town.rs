//! Demanda urbana mínima: casas en cobertura de parada generan pasajeros y correo.

use crate::cargo::CargoType;
use crate::entity_history::TownHistory;
use crate::industry::Industry;
use crate::map::{Map, TileCoord};
use crate::station::{self, STATION_COVERAGE_RADIUS, Station, StopKind};
use crate::world_gen::Climate;

/// Efectos de carga que alimentan metas de crecimiento (`TownEffect` simplificado).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
pub enum TownGrowthEffect {
    Passengers = 0,
    Mail = 1,
    Goods = 2,
    /// Comida (ártico) — proxy vía `Goods` hasta existir cargo Food.
    Food = 3,
    /// Agua (trópico) — proxy vía `Oil` hasta existir cargo Water.
    Water = 4,
}

pub const TOWN_GROWTH_EFFECT_COUNT: usize = 5;

/// Meta especial: comida solo en invierno (`TOWN_GROWTH_WINTER`).
pub const TOWN_GROWTH_WINTER: u32 = u32::MAX - 1;
/// Meta especial: comida/agua en desierto (`TOWN_GROWTH_DESERT`).
pub const TOWN_GROWTH_DESERT: u32 = u32::MAX;
/// Umbral de población para exigir comida en ártico.
pub const TOWN_GROWTH_WINTER_POP_THRESHOLD: u32 = 90;
/// Umbral de población para exigir comida/agua en trópico.
pub const TOWN_GROWTH_DESERT_POP_THRESHOLD: u32 = 60;
/// Meses de crecimiento forzado al financiar edificios (`fund_buildings`).
pub const FUND_BUILDINGS_MONTHS: u8 = 3;
/// Valoración de partida de la autoridad local (`RATING_INITIAL`, `town_type.h:45`).
pub const TOWN_RATING_INITIAL: i16 = 500;

/// Ciudad (importada de saves de `OpenTTD` o creada por el juego).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Town {
    pub id: u32,
    pub pos: crate::map::TileCoord,
    pub name: String,
    pub population: u32,
    /// Valoración de la autoridad local (-1000..=1000; arranca en `TOWN_RATING_INITIAL`).
    #[serde(default = "default_town_rating")]
    pub local_authority_rating: i16,
    /// Pasajeros entregados cerca de la ciudad (contador de crecimiento).
    #[serde(default)]
    pub passengers_served: u32,
    /// Correo entregado cerca de la ciudad.
    #[serde(default)]
    pub mail_served: u32,
    /// Veces que la compañía financió edificios (`TownFundBuildings`).
    #[serde(default)]
    pub growth_funded: u32,
    /// Metas mensuales por efecto (`town->goal[]`).
    #[serde(default)]
    pub goals: [u32; TOWN_GROWTH_EFFECT_COUNT],
    /// Entregas del mes en curso (`received_new`).
    #[serde(default)]
    pub received_new: [u32; TOWN_GROWTH_EFFECT_COUNT],
    /// Entregas del mes anterior (`received_old`), usadas para el gate de crecimiento.
    #[serde(default)]
    pub received_old: [u32; TOWN_GROWTH_EFFECT_COUNT],
    /// Meses restantes de crecimiento forzado por financiación.
    #[serde(default)]
    pub fund_buildings_months: u8,
    /// Resultado de `UpdateTownGrowth` (solo crece si es `true`).
    #[serde(default)]
    pub is_growing: bool,
    /// Series mensuales (población / servicio).
    #[serde(default)]
    pub history: TownHistory,
    /// Ruido acumulado de aeropuertos (`Town::noise_reached`).
    #[serde(default)]
    pub noise_reached: u16,
}

const fn default_town_rating() -> i16 {
    TOWN_RATING_INITIAL
}

impl Default for Town {
    fn default() -> Self {
        Self {
            id: 0,
            pos: TileCoord::new(0, 0),
            name: String::new(),
            population: 0,
            local_authority_rating: TOWN_RATING_INITIAL,
            passengers_served: 0,
            mail_served: 0,
            growth_funded: 0,
            goals: [0; TOWN_GROWTH_EFFECT_COUNT],
            received_new: [0; TOWN_GROWTH_EFFECT_COUNT],
            received_old: [0; TOWN_GROWTH_EFFECT_COUNT],
            fund_buildings_months: 0,
            is_growing: false,
            history: TownHistory::default(),
            noise_reached: 0,
        }
    }
}

impl Town {
    /// Inicializa metas según clima (`InitTownAndName` / clima ártico-trópico).
    pub fn init_growth_goals(&mut self, climate: Climate) {
        self.goals = [0; TOWN_GROWTH_EFFECT_COUNT];
        match climate {
            Climate::SubArctic => {
                self.goals[TownGrowthEffect::Food as usize] = TOWN_GROWTH_WINTER;
            }
            Climate::SubTropical => {
                self.goals[TownGrowthEffect::Food as usize] = TOWN_GROWTH_DESERT;
                self.goals[TownGrowthEffect::Water as usize] = TOWN_GROWTH_DESERT;
            }
            Climate::Temperate | Climate::Toyland => {}
        }
    }

    /// Ajusta la valoración y devuelve el delta aplicado (clamp -1000..=1000).
    pub fn adjust_rating(&mut self, delta: i8) -> i8 {
        let before = self.local_authority_rating;
        let next = i32::from(before) + i32::from(delta);
        self.local_authority_rating = i16::try_from(next.clamp(-1000, 1000)).unwrap_or(0);
        i8::try_from(self.local_authority_rating - before).unwrap_or(delta)
    }

    /// Registra entrega de carga urbana que impulsa el crecimiento.
    pub fn record_town_cargo_delivery(&mut self, cargo: CargoType, amount: u32) {
        match cargo {
            CargoType::Passengers => {
                self.passengers_served = self.passengers_served.saturating_add(amount);
                self.add_received(TownGrowthEffect::Passengers, amount);
            }
            CargoType::Mail => {
                self.mail_served = self.mail_served.saturating_add(amount);
                self.add_received(TownGrowthEffect::Mail, amount);
            }
            CargoType::Goods => {
                self.add_received(TownGrowthEffect::Goods, amount);
                // Proxy Food hasta existir cargo dedicado.
                self.add_received(TownGrowthEffect::Food, amount);
            }
            CargoType::Oil => {
                // Proxy Water (trópico) hasta existir cargo dedicado.
                self.add_received(TownGrowthEffect::Water, amount);
            }
            _ => {}
        }
    }

    fn add_received(&mut self, effect: TownGrowthEffect, amount: u32) {
        let i = effect as usize;
        self.received_new[i] = self.received_new[i].saturating_add(amount);
    }
}

/// ¿La meta del efecto está satisfecha con las entregas del mes anterior?
#[must_use]
pub fn town_goal_satisfied(goal: u32, received: u32, population: u32) -> bool {
    if goal == 0 {
        return true;
    }
    if goal == TOWN_GROWTH_WINTER {
        if population <= TOWN_GROWTH_WINTER_POP_THRESHOLD {
            return true;
        }
        return received > 0;
    }
    if goal == TOWN_GROWTH_DESERT {
        if population <= TOWN_GROWTH_DESERT_POP_THRESHOLD {
            return true;
        }
        return received > 0;
    }
    received >= goal
}

/// Actualiza `is_growing` (`UpdateTownGrowth`).
///
/// Financiar edificios fuerza crecimiento aunque no haya estación cerca.
pub fn update_town_growth_state(town: &mut Town, stations: &[Station]) {
    if town.fund_buildings_months > 0 {
        town.is_growing = true;
        return;
    }
    let has_station = stations.iter().any(|st| {
        !matches!(
            st.stop_kind,
            StopKind::RailWaypoint | StopKind::RoadWaypoint | StopKind::Buoy
        ) && crate::economy::manhattan_distance(st.pos, town.pos) <= TOWN_AUTHORITY_RADIUS
    });
    if !has_station {
        town.is_growing = false;
        return;
    }
    for (i, &goal) in town.goals.iter().enumerate() {
        if !town_goal_satisfied(goal, town.received_old[i], town.population) {
            town.is_growing = false;
            return;
        }
    }
    let activity = town.received_old.iter().copied().sum::<u32>() > 0
        || town.passengers_served.saturating_add(town.mail_served) > 0
        || town.growth_funded > 0;
    town.is_growing = activity;
}

/// Rollover mensual de entregas + decaimiento de financiación + gate de crecimiento.
pub fn process_town_monthly_growth(towns: &mut [Town], stations: &[Station]) {
    for town in towns {
        town.received_old = town.received_new;
        town.received_new = [0; TOWN_GROWTH_EFFECT_COUNT];
        if town.fund_buildings_months > 0 {
            town.fund_buildings_months = town.fund_buildings_months.saturating_sub(1);
        }
        update_town_growth_state(town, stations);
    }
}

/// Periodo de generación (mismo orden de magnitud que [`crate::INDUSTRY_PRODUCE_TICKS`]).
pub const TOWN_PRODUCE_TICKS: u64 = 256;
/// Revisión de crecimiento urbano.
pub const TOWN_GROWTH_TICKS: u64 = 512;
/// Población añadida por ciclo de crecimiento cuando hay servicio.
pub const TOWN_GROWTH_POPULATION_STEP: u32 = 10;

pub const PASSENGERS_PER_HOUSE: u32 = 2;
pub const MAIL_PER_HOUSE: u32 = 1;

/// Tope de espera en parada bus (análogo al stock de industria).
pub const STATION_TOWN_CARGO_CAPACITY: u32 = 500;

/// Radio de influencia de la autoridad local sobre nuevas estaciones.
pub const TOWN_AUTHORITY_RADIUS: u32 = 20;
/// Valoración mínima para construir estación cerca de una ciudad.
pub const AUTHORITY_MIN_STATION: i16 = -200;

pub const TOWN_ADVERTISE_COST: i64 = 1_000;
pub const TOWN_ADVERTISE_RATING_BOOST: i8 = 25;
pub const FUND_BUILDINGS_COST: i64 = 5_000;
pub const FUND_BUILDINGS_RATING_BOOST: i8 = 50;

/// Penalización al construir estación cerca de una ciudad.
pub const STATION_BUILD_RATING_PENALTY: i8 = -15;

/// Añade pasajeros/correo en paradas bus según casas dentro del radio de cobertura.
///
/// La cantidad generada pasa por [`station::move_goods_to_station`]: el rating decide
/// cuánto llega al andén. El reparto entre paradas que se pisan las mismas casas
/// exige producción por casa (P1.7 del roadmap de paridad).
pub fn produce_town_cargo(
    map: &Map,
    industries: &[Industry],
    stations: &mut [Station],
    tick: u64,
    selectgoods: bool,
) -> (u64, u64) {
    if tick == 0 || !tick.is_multiple_of(TOWN_PRODUCE_TICKS) {
        return (0, 0);
    }

    let mut passengers = 0_u64;
    let mut mail = 0_u64;

    for idx in 0..stations.len() {
        if !matches!(
            stations[idx].stop_kind,
            StopKind::BusStop | StopKind::Airport
        ) {
            continue;
        }
        let coverage = station::station_coverage_for(map, industries, &stations[idx]);
        if coverage.house_tiles == 0 {
            continue;
        }

        let pax_room =
            STATION_TOWN_CARGO_CAPACITY.saturating_sub(stations[idx].cargo_stock.passengers);
        let mail_room = STATION_TOWN_CARGO_CAPACITY.saturating_sub(stations[idx].cargo_stock.mail);
        let pax_amount = (coverage.house_tiles * PASSENGERS_PER_HOUSE).min(pax_room);
        let mail_amount = (coverage.house_tiles * MAIL_PER_HOUSE).min(mail_room);
        let source = stations[idx].pos;

        passengers += u64::from(station::move_goods_to_station(
            stations,
            &[idx],
            CargoType::Passengers,
            pax_amount,
            source,
            selectgoods,
            None,
        ));
        mail += u64::from(station::move_goods_to_station(
            stations,
            &[idx],
            CargoType::Mail,
            mail_amount,
            source,
            selectgoods,
            None,
        ));
    }

    (passengers, mail)
}

/// Crece la población si `is_growing` y hay cobertura de casas (o financiación).
///
/// Además intenta expansión física (calles/casas) y devuelve teselas dirty.
pub fn grow_town_if_served(
    map: &mut Map,
    industries: &[Industry],
    stations: &[Station],
    towns: &mut [Town],
    tick: u64,
) -> Vec<TileCoord> {
    if tick == 0 || !tick.is_multiple_of(TOWN_GROWTH_TICKS) {
        return Vec::new();
    }
    let mut dirty = Vec::new();
    for town in towns {
        update_town_growth_state(town, stations);
        if !town.is_growing {
            continue;
        }
        let funded = town.fund_buildings_months > 0 || town.growth_funded > 0;
        let has_station = stations.iter().any(|st| {
            !matches!(
                st.stop_kind,
                StopKind::RailWaypoint | StopKind::RoadWaypoint | StopKind::Buoy
            ) && crate::economy::manhattan_distance(st.pos, town.pos) <= TOWN_AUTHORITY_RADIUS
        });
        // Financiación permite crecer sin estación; el resto exige una cerca.
        if !funded && !has_station {
            continue;
        }
        let coverage =
            station::station_coverage_at(map, industries, town.pos, STATION_COVERAGE_RADIUS);
        // Casas existentes / financiación: step abstracto. Con estación también
        // expandimos físicamente aunque aún no haya casas en cobertura.
        if coverage.house_tiles > 0 || funded || has_station {
            town.population = town.population.saturating_add(
                TOWN_GROWTH_POPULATION_STEP + u32::from(town.fund_buildings_months.min(3)),
            );
            dirty.extend(crate::town_expand::expand_town_physically(map, town, tick));
        }
    }
    dirty
}

/// Impulso inmediato al financiar edificios (feedback en UI + arranque del ciclo).
pub fn apply_fund_buildings_boost(town: &mut Town) {
    town.growth_funded = town.growth_funded.saturating_add(1);
    town.fund_buildings_months = FUND_BUILDINGS_MONTHS;
    town.is_growing = true;
    town.population = town
        .population
        .saturating_add(TOWN_GROWTH_POPULATION_STEP + u32::from(FUND_BUILDINGS_MONTHS));
}

#[must_use]
pub fn nearest_town_index(towns: &[Town], pos: TileCoord) -> Option<(usize, u32)> {
    towns
        .iter()
        .enumerate()
        .map(|(i, t)| (i, crate::economy::manhattan_distance(t.pos, pos)))
        .min_by_key(|(_, d)| *d)
}

/// Comprueba si la autoridad local permite una nueva estación en `pos`.
#[must_use]
pub fn authority_allows_new_station(towns: &[Town], pos: TileCoord) -> bool {
    let Some((idx, dist)) = nearest_town_index(towns, pos) else {
        return true;
    };
    if dist > TOWN_AUTHORITY_RADIUS {
        return true;
    }
    towns[idx].local_authority_rating >= AUTHORITY_MIN_STATION
}

/// Aplica penalización de autoridad al construir estación cerca de una ciudad.
pub fn apply_station_build_rating_penalty(towns: &mut [Town], pos: TileCoord) -> Option<(u32, i8)> {
    let (idx, dist) = nearest_town_index(towns, pos)?;
    if dist > TOWN_AUTHORITY_RADIUS {
        return None;
    }
    let town_id = towns[idx].id;
    let delta = towns[idx].adjust_rating(STATION_BUILD_RATING_PENALTY);
    Some((town_id, delta))
}

/// Registra entrega de carga urbana en la ciudad más cercana dentro del radio.
pub fn record_delivery_near_town(
    towns: &mut [Town],
    station_pos: TileCoord,
    cargo: CargoType,
    amount: u32,
) {
    let Some((idx, dist)) = nearest_town_index(towns, station_pos) else {
        return;
    };
    if dist > TOWN_AUTHORITY_RADIUS {
        return;
    }
    towns[idx].record_town_cargo_delivery(cargo, amount);
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::map::{TileCoord, TileKind};

    #[test]
    fn produce_adds_cargo_when_houses_in_coverage() {
        let mut map = Map::new_flat(16, 16, 0);
        let stop_pos = TileCoord::new(8, 8);
        map.set_kind(TileCoord::new(7, 8), TileKind::House).unwrap();
        map.set_kind(TileCoord::new(8, 7), TileKind::House).unwrap();

        let mut stations = vec![Station::new_with_kind(stop_pos, StopKind::BusStop)];
        // La parada ya tiene servicio: si no, selectgoods no deja llegar pasajeros.
        stations[0].goods.get_mut(CargoType::Passengers).last_speed = 1;
        stations[0].goods.get_mut(CargoType::Mail).last_speed = 1;

        let (pax, mail) = produce_town_cargo(&map, &[], &mut stations, TOWN_PRODUCE_TICKS, true);
        // 4 pax × (175+1) >> 8 = 2; 2 mail × 176 >> 8 = 1.
        assert_eq!(pax, 2);
        assert_eq!(mail, 1);
        assert_eq!(stations[0].cargo_stock.passengers, 2);
        assert_eq!(stations[0].cargo_stock.mail, 1);
    }

    #[test]
    fn produce_skips_non_bus_stops() {
        let mut map = Map::new_flat(8, 8, 0);
        let pos = TileCoord::new(2, 2);
        map.set_kind(TileCoord::new(2, 1), TileKind::House).unwrap();
        let mut stations = vec![Station::new_with_kind(pos, StopKind::TruckStop)];

        let (pax, mail) = produce_town_cargo(&map, &[], &mut stations, TOWN_PRODUCE_TICKS, true);
        assert_eq!(pax, 0);
        assert_eq!(mail, 0);
    }

    #[test]
    fn authority_blocks_station_when_rating_too_low() {
        let towns = vec![Town {
            id: 1,
            pos: TileCoord::new(5, 5),
            name: "Test".into(),
            population: 100,
            local_authority_rating: -500,
            passengers_served: 0,
            mail_served: 0,
            growth_funded: 0,
            ..Default::default()
        }];
        assert!(!authority_allows_new_station(&towns, TileCoord::new(6, 5)));
        assert!(authority_allows_new_station(&towns, TileCoord::new(30, 30)));
    }

    #[test]
    fn town_grows_when_served() {
        let mut map = Map::new_flat(16, 16, 0);
        let town_pos = TileCoord::new(8, 8);
        map.set_kind(TileCoord::new(7, 8), TileKind::House).unwrap();
        let mut towns = vec![Town {
            id: 0,
            pos: town_pos,
            name: "Grow".into(),
            population: 100,
            local_authority_rating: 0,
            passengers_served: 10,
            mail_served: 0,
            growth_funded: 0,
            is_growing: true,
            ..Default::default()
        }];
        let stations = vec![Station::new_with_kind(
            TileCoord::new(8, 9),
            StopKind::BusStop,
        )];
        grow_town_if_served(&mut map, &[], &stations, &mut towns, TOWN_GROWTH_TICKS);
        assert!(towns[0].population > 100);
    }

    #[test]
    fn town_does_not_grow_when_goals_unmet() {
        let mut map = Map::new_flat(16, 16, 0);
        let town_pos = TileCoord::new(8, 8);
        map.set_kind(TileCoord::new(7, 8), TileKind::House).unwrap();
        let mut towns = vec![Town {
            id: 0,
            pos: town_pos,
            name: "Stuck".into(),
            population: 120,
            passengers_served: 10,
            ..Default::default()
        }];
        towns[0].init_growth_goals(Climate::SubArctic);
        let stations = vec![Station::new_with_kind(
            TileCoord::new(8, 9),
            StopKind::BusStop,
        )];
        grow_town_if_served(&mut map, &[], &stations, &mut towns, TOWN_GROWTH_TICKS);
        assert_eq!(towns[0].population, 120);
        assert!(!towns[0].is_growing);
    }

    #[test]
    fn arctic_food_goal_blocks_large_town_without_goods() {
        let mut town = Town {
            id: 0,
            pos: TileCoord::new(5, 5),
            name: "Arctic".into(),
            population: 120,
            passengers_served: 50,
            ..Default::default()
        };
        town.init_growth_goals(Climate::SubArctic);
        let stations = vec![Station::new_with_kind(
            TileCoord::new(5, 6),
            StopKind::BusStop,
        )];
        update_town_growth_state(&mut town, &stations);
        assert!(!town.is_growing);
        town.received_old[TownGrowthEffect::Food as usize] = 1;
        update_town_growth_state(&mut town, &stations);
        assert!(town.is_growing);
    }

    #[test]
    fn fund_buildings_forces_growth_gate_without_station() {
        let mut town = Town {
            id: 0,
            pos: TileCoord::new(5, 5),
            name: "Fund".into(),
            population: 200,
            fund_buildings_months: 3,
            ..Default::default()
        };
        town.init_growth_goals(Climate::SubArctic);
        update_town_growth_state(&mut town, &[]);
        assert!(town.is_growing);
    }

    #[test]
    fn fund_buildings_grows_without_station() {
        let mut map = Map::new_flat(16, 16, 0);
        let mut towns = vec![Town {
            id: 0,
            pos: TileCoord::new(8, 8),
            name: "Funded".into(),
            population: 50,
            fund_buildings_months: 3,
            growth_funded: 1,
            is_growing: true,
            ..Default::default()
        }];
        grow_town_if_served(&mut map, &[], &[], &mut towns, TOWN_GROWTH_TICKS);
        assert!(towns[0].population > 50);
        assert!(towns[0].is_growing);
    }

    #[test]
    fn apply_fund_boost_raises_population_immediately() {
        let mut town = Town {
            id: 0,
            pos: TileCoord::new(0, 0),
            name: "X".into(),
            population: 40,
            ..Default::default()
        };
        apply_fund_buildings_boost(&mut town);
        assert_eq!(town.fund_buildings_months, FUND_BUILDINGS_MONTHS);
        assert!(town.is_growing);
        assert!(town.population > 40);
        assert_eq!(town.growth_funded, 1);
    }

    #[test]
    fn adjust_rating_clamps() {
        let mut town = Town {
            id: 0,
            pos: TileCoord::new(0, 0),
            name: "X".into(),
            population: 0,
            local_authority_rating: 990,
            passengers_served: 0,
            mail_served: 0,
            growth_funded: 0,
            ..Default::default()
        };
        town.adjust_rating(50);
        assert_eq!(town.local_authority_rating, 1000);
    }

    /// `OpenTTD` arranca los pueblos en `RATING_INITIAL = 500` (`town_type.h:45`),
    /// no en neutral, así que la autoridad empieza siendo moderadamente favorable.
    #[test]
    fn new_town_starts_at_initial_rating() {
        let town = Town {
            pos: TileCoord::new(5, 5),
            ..Default::default()
        };
        assert_eq!(town.local_authority_rating, TOWN_RATING_INITIAL);
        assert_eq!(TOWN_RATING_INITIAL, 500);
        assert!(authority_allows_new_station(
            std::slice::from_ref(&town),
            TileCoord::new(6, 6)
        ));
    }
}
