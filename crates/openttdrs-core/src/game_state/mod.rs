mod canonical_hash;
mod runtime;

pub use runtime::SimulationRuntime;

use crate::industry::Industry;
use crate::map::{Map, TileCoord};
use crate::station::Station;
use crate::tick::GameTick;
use crate::tnbp_decode::JgrTunnelRecord;
use crate::vehicle::Vehicle;
use crate::world_gen::Climate;

/// Evento efímero para animación «+$» en el cliente (no se serializa).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IncomePopup {
    pub amount: i64,
    pub at: TileCoord,
}

/// Contadores acumulativos de la simulación (carga/descarga, producción).
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SimStats {
    /// Eventos de carga (vehículo tomó cargo en una industria).
    pub cargo_pickups: u64,
    /// Eventos de descarga (vehículo entregó en una estación).
    pub cargo_deliveries: u64,
    /// Unidades de cargo cargadas (suma de `load`).
    pub cargo_units_loaded: u64,
    /// Unidades de cargo entregadas en estación.
    pub cargo_units_delivered: u64,
    /// Unidades añadidas al stock de industrias por `Industry::produce`.
    pub industry_cargo_units_produced: u64,
    /// Pasajeros generados en paradas bus por demanda urbana.
    #[serde(default)]
    pub town_passengers_generated: u64,
    /// Correo generado en paradas bus por demanda urbana.
    #[serde(default)]
    pub town_mail_generated: u64,
    /// Ingresos acumulados por entregas de carga (dinero de compañía).
    #[serde(default)]
    pub cargo_income_earned: u64,
    /// Costes de explotación de vehículos acumulados.
    #[serde(default)]
    pub vehicle_running_costs: u64,
    /// Series mensuales para gráficos (Income / Operating Profit).
    #[serde(default)]
    pub economy_history: EconomyHistory,
}

/// Número de meses retenidos en gráficos económicos.
pub const ECONOMY_HISTORY_MONTHS: usize = 36;

/// Muestra mensual de la economía (deltas del mes + valor de compañía al cierre).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MonthlyEconomySample {
    pub income: u64,
    pub running_costs: u64,
    pub deliveries: u64,
    /// Patrimonio neto al cierre del mes (`money - loan`).
    #[serde(default)]
    pub company_value: i64,
}

impl MonthlyEconomySample {
    #[must_use]
    pub const fn operating_profit(self) -> i64 {
        self.income.cast_signed() - self.running_costs.cast_signed()
    }
}

/// Ring buffer de muestras mensuales + baselines para calcular deltas.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct EconomyHistory {
    pub samples: Vec<MonthlyEconomySample>,
    #[serde(default)]
    pub last_income: u64,
    #[serde(default)]
    pub last_running_costs: u64,
    #[serde(default)]
    pub last_deliveries: u64,
}

impl EconomyHistory {
    /// Registra el mes a partir de los totales acumulados y el valor de compañía.
    pub fn push_month_from_totals(
        &mut self,
        income_total: u64,
        running_costs_total: u64,
        deliveries_total: u64,
        company_value: i64,
    ) {
        let sample = MonthlyEconomySample {
            income: income_total.saturating_sub(self.last_income),
            running_costs: running_costs_total.saturating_sub(self.last_running_costs),
            deliveries: deliveries_total.saturating_sub(self.last_deliveries),
            company_value,
        };
        self.last_income = income_total;
        self.last_running_costs = running_costs_total;
        self.last_deliveries = deliveries_total;
        self.samples.push(sample);
        if self.samples.len() > ECONOMY_HISTORY_MONTHS {
            let drop = self.samples.len() - ECONOMY_HISTORY_MONTHS;
            self.samples.drain(0..drop);
        }
    }
}

/// Patrimonio neto simplificado (espejo de Finances: efectivo − préstamo).
#[must_use]
pub fn company_net_value(money: i64, loan: i64) -> i64 {
    money.saturating_sub(loan)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod economy_history_tests {
    use super::{ECONOMY_HISTORY_MONTHS, EconomyHistory, company_net_value};

    #[test]
    fn push_month_from_totals_stores_deltas_and_caps_length() {
        let mut history = EconomyHistory::default();
        history.push_month_from_totals(100, 40, 2, 90_000);
        history.push_month_from_totals(250, 90, 5, 95_000);
        assert_eq!(history.samples.len(), 2);
        assert_eq!(history.samples[0].income, 100);
        assert_eq!(history.samples[0].running_costs, 40);
        assert_eq!(history.samples[0].deliveries, 2);
        assert_eq!(history.samples[0].operating_profit(), 60);
        assert_eq!(history.samples[0].company_value, 90_000);
        assert_eq!(history.samples[1].income, 150);
        assert_eq!(history.samples[1].running_costs, 50);
        assert_eq!(history.samples[1].deliveries, 3);
        assert_eq!(history.samples[1].company_value, 95_000);

        for i in 0..ECONOMY_HISTORY_MONTHS {
            let total = 250 + (i as u64 + 1) * 10;
            history.push_month_from_totals(total, 90, 5, 100_000);
        }
        assert_eq!(history.samples.len(), ECONOMY_HISTORY_MONTHS);
        assert_eq!(history.samples.last().map(|s| s.income), Some(10));
    }

    #[test]
    fn company_net_value_is_money_minus_loan() {
        assert_eq!(company_net_value(100_000, 20_000), 80_000);
        assert_eq!(company_net_value(10_000, 50_000), -40_000);
    }

    #[test]
    fn monthly_close_records_per_company_history() {
        use crate::timer::tick_at_end_of_day;
        use crate::{GameState, GameTick};

        let mut s = GameState::new(12, 12);
        // Ambos rivales antes del cierre: `tick_ai` también puede crear RoadHaul
        // en el mismo tick mensual (después del cierre) y dejarlo sin muestra.
        s.ensure_rival_ais();
        assert_eq!(s.companies.len(), 3);
        s.tick = GameTick::new(tick_at_end_of_day(30));
        s.sync_timers_from_tick();
        s.step();
        assert!(
            s.companies
                .iter()
                .all(|c| !c.economy_history.samples.is_empty()),
            "cada compañía debe tener cierre mensual"
        );
        assert_eq!(
            s.stats.economy_history.samples.len(),
            s.companies[s.active_company.index()]
                .economy_history
                .samples
                .len()
        );
    }

    #[test]
    fn set_active_company_swaps_economy_mirrors() {
        use crate::{CompanyId, GameState};

        let mut s = GameState::new(8, 8);
        s.ensure_rival_transcargo();
        s.economy.money = 50_000;
        s.sync_active_from_mirrors();
        let rival = s.companies.iter().find(|c| c.is_ai).expect("rival").id;
        assert!(s.set_active_company(rival));
        assert_eq!(s.active_company, rival);
        assert_eq!(s.economy.money, s.companies[rival.index()].economy.money);
        assert!(s.set_active_company(CompanyId::PLAYER));
        assert_eq!(s.active_company, CompanyId::PLAYER);
        assert_eq!(s.economy.money, 50_000);
    }

    #[test]
    fn monthly_close_records_town_and_industry_history() {
        use crate::industry::IndustryKind;
        use crate::map::TileCoord;
        use crate::timer::tick_at_end_of_day;
        use crate::{GameState, GameTick, Industry, Town};

        let mut s = GameState::new(12, 12);
        s.towns.push(Town {
            id: 1,
            pos: TileCoord::new(3, 3),
            name: "Hist".into(),
            population: 50,
            passengers_served: 4,
            mail_served: 1,
            ..Default::default()
        });
        let mut industry = Industry::new(TileCoord::new(5, 5), IndustryKind::CoalMine);
        industry.produced_total = 8;
        industry.transported_total = 2;
        industry.stock = 6;
        s.industries.push(industry);
        s.tick = GameTick::new(tick_at_end_of_day(30));
        s.sync_timers_from_tick();
        s.step();
        assert_eq!(s.towns[0].history.samples.len(), 1);
        assert_eq!(s.towns[0].history.samples[0].population, 50);
        assert_eq!(s.towns[0].history.samples[0].passengers_served, 4);
        assert_eq!(s.industries[0].history.samples.len(), 1);
        assert_eq!(s.industries[0].history.samples[0].produced, 8);
        assert_eq!(s.industries[0].history.samples[0].transported, 2);
        assert_eq!(s.industries[0].history.samples[0].stock, 6);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CompanyEconomy {
    pub money: i64,
    pub loan: i64,
    /// Tope de préstamo (`economy.cpp` `max_loan`; por defecto 300 000).
    #[serde(default = "default_max_loan")]
    pub max_loan: i64,
}

const fn default_max_loan() -> i64 {
    crate::economy::DEFAULT_MAX_LOAN
}

impl Default for CompanyEconomy {
    fn default() -> Self {
        Self {
            money: 100_000,
            loan: 0,
            max_loan: crate::economy::DEFAULT_MAX_LOAN,
        }
    }
}

pub const ROAD_BUILD_COST: i64 = 95;
pub const RAIL_BUILD_COST: i64 = 100;
pub const STATION_BUILD_COST: i64 = 200;
/// Coste de waypoint ferroviario (`Price::BuildWaypointRail` en `OpenTTD`).
pub const WAYPOINT_BUILD_COST: i64 = 600;
pub const DEPOT_BUILD_COST: i64 = 150;
pub const TUNNEL_BUILD_COST_PER_TILE: i64 = 90;
pub const BRIDGE_BUILD_COST_PER_TILE: i64 = 70;
pub const CLEAR_TILE_COST: i64 = 5;
/// Precio base por esquina (`Price::Terraform`, dificultad media sin inflación).
pub const TERRAFORM_BASE_PRICE: i64 = 250;
/// Precio base por tesela de terreno comprado (`Price::BuildObject`).
pub const BUY_LAND_BASE_PRICE: i64 = 40;
/// Precio base faro/transmisor (`Price::BuildObject`).
#[allow(dead_code)]
pub const BUILD_OBJECT_BASE_PRICE: i64 = 40;
/// Alias en tick 0 (sin inflación de precios); preferir [`crate::economy::terraform_cost_per_corner`].
pub const TERRAFORM_COST: i64 = TERRAFORM_BASE_PRICE;

/// Pago plano legado (sustituido por [`crate::economy::transported_goods_income`]).
#[deprecated(note = "usar economy::transported_goods_income")]
pub const CARGO_DELIVERY_PAYMENT: i64 = 12;

/// Estado global del mundo simulado.
///
/// ## Campos persistidos vs. efímeros
///
/// Los campos en el nivel superior de esta estructura (excepto `runtime`) se
/// **persisten** al guardar la partida como JSON. El campo `runtime` contiene
/// **datos efímeros** que no se guardan y deben reconstruirse tras cargar.
///
/// Ver [`SimulationRuntime`] para detalles de campos no persistidos.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[allow(clippy::struct_excessive_bools)]
pub struct GameState {
    // ───── Campos persistidos ─────
    pub map: Map,
    pub tick: GameTick,
    /// Reloj de calendario (edad de vehículos, noticias, año mostrado).
    #[serde(default)]
    pub calendar: crate::timer::CalendarTimer,
    /// Reloj de economía (intereses, inflación, subsidios, producción).
    #[serde(default)]
    pub economy_timer: crate::timer::EconomyTimer,
    pub industries: Vec<Industry>,
    pub vehicles: Vec<Vehicle>,
    pub stations: Vec<Station>,
    /// Ciudades (importadas de saves de `OpenTTD`; vacío en mapas procedurales).
    #[serde(default)]
    pub towns: Vec<crate::town::Town>,
    pub stats: SimStats,
    /// Espejo de la compañía activa (jugador). Fuente de verdad: [`Self::companies`].
    #[serde(default)]
    pub economy: CompanyEconomy,
    /// Color espejo de la compañía activa (`Colours` en `OpenTTD`; 0 = azul oscuro).
    #[serde(default)]
    pub company_colour: u8,
    /// Pool de compañías (jugador + rivales IA). Vacío en saves anteriores a v14 → migración.
    #[serde(default)]
    pub companies: Vec<crate::company::Company>,
    /// Compañía que emite comandos del jugador / UI.
    #[serde(default)]
    pub active_company: crate::company::CompanyId,
    /// Tipo de vía activo al construir (`_cur_railtype` en `OpenTTD`).
    #[serde(default)]
    pub current_rail_type: crate::rail_type::RailType,
    /// Tipo de carretera activo (`_last_built_roadtype` / `_cur_roadtype` clase road).
    #[serde(default)]
    pub current_road_type: crate::road_type::RoadType,
    /// Tipo de tranvía activo (`_last_built_tramtype`).
    #[serde(default = "default_current_tram_type")]
    pub current_tram_type: crate::road_type::RoadType,
    /// Catálogo road/tram (vanilla + Action0 `RoadTypes`).
    #[serde(default = "crate::road_type::vanilla_road_type_catalog")]
    pub road_type_catalog: Vec<crate::road_type::RoadTypeDef>,
    /// Clase de estación ferroviaria activa (picker `NewGRF` / vanilla).
    #[serde(default)]
    pub current_station_class: crate::station_class::StationClassId,
    /// Spec de estación ferroviaria activa dentro de la clase.
    #[serde(default)]
    pub current_station_spec: crate::station_class::StationSpecId,
    /// Catálogo de clases de estación (vanilla + Action0 Stations).
    #[serde(default = "crate::station_class::vanilla_station_class_catalog")]
    pub station_class_catalog: Vec<crate::station_class::StationClassDef>,
    /// Catálogo de specs de estación (vanilla + Action0 Stations).
    #[serde(default = "crate::station_class::vanilla_station_spec_catalog")]
    pub station_spec_catalog: Vec<crate::station_class::StationSpecDef>,
    /// Clase de road stop activa (`None` = sin selección `NewGRF`).
    #[serde(default)]
    pub current_road_stop_class: Option<u16>,
    /// Spec de road stop activo (`None` = vanilla / sin `NewGRF`).
    #[serde(default)]
    pub current_road_stop_spec: Option<u16>,
    /// Catálogo de clases de road stop (Action0 `RoadStops`).
    #[serde(default)]
    pub road_stop_class_catalog: Vec<crate::road_stop_spec::RoadStopClassDef>,
    /// Catálogo de specs de road stop (Action0 `RoadStops`).
    #[serde(default)]
    pub road_stop_spec_catalog: Vec<crate::road_stop_spec::RoadStopSpecDef>,
    /// Catálogo de motores (vanilla + Action0 Trains).
    #[serde(default = "crate::engine::vanilla_engine_catalog")]
    pub engine_catalog: Vec<crate::engine::EngineDef>,
    /// Specs `NewGRF` de teselas de industria (gfx ≥175).
    #[serde(default)]
    pub industry_tile_spec_catalog: Vec<crate::industry_tile::IndustryTileSpecDef>,
    /// Equivalente persistido de `AnimatedTileList` para animaciones `NewGRF` de industria.
    #[serde(default)]
    pub newgrf_animated_industry_tiles: std::collections::HashSet<TileCoord>,
    /// Overrides vanilla gfx → `NewGRF` (`GetTranslatedIndustryTileID`).
    #[serde(default = "crate::industry_tile::empty_industry_tile_overrides")]
    pub industry_tile_overrides: Vec<u16>,
    /// Specs `NewGRF` de industrias (id ≥37).
    #[serde(default)]
    pub industry_spec_catalog: Vec<crate::industry_spec::IndustrySpecDef>,
    /// Overrides vanilla industry → id `NewGRF` (`prop 0x09`).
    #[serde(default = "crate::industry_spec::empty_industry_overrides")]
    pub industry_overrides: Vec<u16>,
    /// Specs `NewGRF` de casas (id ≥110).
    #[serde(default)]
    pub house_spec_catalog: Vec<crate::house_spec::HouseSpecDef>,
    /// Overrides vanilla house → id `NewGRF` (`prop 0x15`).
    #[serde(default = "crate::house_spec::empty_house_overrides")]
    pub house_overrides: Vec<u16>,
    /// Specs `NewGRF` de teselas de aeropuerto (gfx ≥74).
    #[serde(default)]
    pub airport_tile_spec_catalog: Vec<crate::airport_tile_spec::AirportTileSpecDef>,
    /// Overrides vanilla airport tile → gfx `NewGRF`.
    #[serde(default = "crate::airport_tile_spec::empty_airport_tile_overrides")]
    pub airport_tile_overrides: Vec<u16>,
    /// Specs `NewGRF` de aeropuertos (id ≥10).
    #[serde(default)]
    pub airport_spec_catalog: Vec<crate::airport_class::NewgrfAirportSpecDef>,
    /// Aeropuertos vanilla deshabilitados por Action0 `08=FF` (índice `AT_*`).
    #[serde(default)]
    pub airport_vanilla_disabled: Vec<bool>,
    /// Catálogo Action0 `Badges` (`0x15`).
    #[serde(default)]
    pub badge_catalog: Vec<crate::badge::BadgeDef>,
    /// Catálogo Action11 + Action0 `Sounds` (`0x0C`).
    #[serde(default)]
    pub sound_effect_catalog: Vec<crate::sound_effect::SoundEffectDef>,
    /// Catálogo Action0 `Cargoes` (`0x0B`); no altera [`crate::cargo::CargoType`].
    #[serde(default)]
    pub cargo_spec_catalog: Vec<crate::cargo_spec::CargoSpecDef>,
    /// Catálogo Action0 `Objects` (`0x0F`).
    #[serde(default)]
    pub object_spec_catalog: Vec<crate::object_spec::ObjectSpecDef>,
    /// Catálogo de puentes (13 slots vanilla + overrides Action0 `0x06`).
    #[serde(default = "crate::bridge_spec::vanilla_bridge_spec_catalog")]
    pub bridge_spec_catalog: Vec<crate::bridge_spec::BridgeSpecDef>,
    /// Catálogo de features de canal (Action0 `0x05`; 9 slots).
    #[serde(default = "crate::canal_spec::vanilla_canal_feature_catalog")]
    pub canal_feature_catalog: Vec<crate::canal_spec::CanalFeatureDef>,
    /// Spec de objeto `NewGRF` activo (`0` = ninguno; id del catálogo).
    #[serde(default)]
    pub current_object_spec: u16,
    /// Clase de aeropuerto activa (picker).
    #[serde(default)]
    pub current_airport_class: crate::airport_class::AirportClassId,
    /// Spec de aeropuerto activo dentro de la clase.
    #[serde(default)]
    pub current_airport_spec: crate::airport_class::AirportSpecId,
    /// Spec `NewGRF` activo (`None` = vanilla [`Self::current_airport_spec`]).
    #[serde(default)]
    pub current_airport_newgrf_id: Option<u16>,
    /// Clima del paisaje (`LandscapeType` en `OpenTTD`).
    #[serde(default)]
    pub climate: Climate,
    /// Semilla de generación procedural (0 = sin terreno aleatorio explícito).
    #[serde(default)]
    pub world_seed: u64,
    /// Túneles JGR decodificados desde footer `TNBP` del `.ottdmap` (vacío si no hay o no aplica).
    #[serde(default)]
    pub jgr_tunnels_from_footer: Vec<JgrTunnelRecord>,
    /// Historial de noticias (más reciente al frente).
    #[serde(default)]
    pub news: crate::news::NewsQueue,
    /// Noticia «primer vehículo en marcha» ya emitida.
    #[serde(default)]
    pub news_first_vehicle_running_sent: bool,
    /// Reglas de autoreemplazo de motores en depósito.
    #[serde(default)]
    pub autoreplace_rules: Vec<crate::autoreplace::AutoReplaceRule>,
    /// Grupos de vehículos.
    #[serde(default)]
    pub vehicle_groups: Vec<crate::vehicle_group::VehicleGroup>,
    /// Pools de órdenes compartidas.
    #[serde(default)]
    pub shared_order_lists: Vec<crate::shared_orders::SharedOrderList>,
    /// Subsidios activos u ofrecidos.
    #[serde(default)]
    pub subsidies: Vec<crate::subsidy::Subsidy>,
    /// Contador para IDs de subsidio.
    #[serde(default)]
    pub next_subsidy_id: u32,
    /// Tolerancia del ayuntamiento a demolición municipal (`difficulty.town_council_tolerance`).
    #[serde(default)]
    pub town_council_tolerance: crate::town::TownCouncilTolerance,
    /// Desastres ambientales habilitados.
    #[serde(default = "default_true")]
    pub disasters_enabled: bool,
    /// Límite de ruido de aeropuerto (`economy.station_noise_level`).
    #[serde(default)]
    pub station_noise_level: bool,
    /// Lado de señales y circulación (`construction.train_signal_side`).
    #[serde(default)]
    pub construction: crate::construction_settings::ConstructionSettings,
    /// Ticks hasta la próxima comprobación de desastre.
    #[serde(default = "default_disaster_timer")]
    pub disaster_timer: u64,
    /// OVNIs u otros crafts de desastre en vuelo (#188; efímero, no se guarda).
    #[serde(skip, default)]
    pub disaster_crafts: Vec<crate::disaster::DisasterCraft>,
    /// Ajustes de pathfinding / PBS (`pf.wait_for_pbs_path`, etc.).
    #[serde(default)]
    pub pathfinding: crate::pathfinding_settings::PathfindingSettings,
    /// Modelo de aceleración de trenes (`vehicle.train_acceleration_model`).
    #[serde(default)]
    pub train_acceleration_model: crate::engine::TrainAccelerationModel,
    /// Ajustes de IA rival (`TransCargo`; UI-8 / #44).
    #[serde(default)]
    pub ai: crate::ai::AiSettings,
    /// Cheats / sandbox formales (UI-7; off por defecto).
    #[serde(default)]
    pub cheats: crate::cheats::CheatsState,
    /// Órdenes / selectgoods (`order.selectgoods` en `OpenTTD`).
    #[serde(default)]
    pub order: crate::cargo::OrderSettings,
    /// Stack `NewGRF` activo (Fase 7 MVP; sin ejecución Action0–14).
    #[serde(default = "crate::newgrf_config::default_vanilla_stack")]
    pub newgrf_stack: Vec<crate::newgrf_config::NewGrfEntry>,
    /// Carteles del mapa (`Sign` en `OpenTTD`).
    #[serde(default)]
    pub signs: Vec<crate::sign::Sign>,
    /// Contador para IDs de cartel.
    #[serde(default = "default_next_sign_id")]
    pub next_sign_id: u32,
    /// Meses consecutivos en quiebra de la compañía activa.
    #[serde(default)]
    pub bankruptcy_streak: u8,
    /// Partida cerrada (endscreen); no emitir más `GameOver`.
    #[serde(default)]
    pub game_finished: bool,
    /// Flujos estación→estación observados (link graph → `station_flows`).
    #[serde(default)]
    pub link_graph: crate::link_graph::LinkGraphStats,
    /// Modo `CargoDist` (`Manual` / `Asymmetric` / `Symmetric`).
    #[serde(default)]
    pub cargo_dist: crate::flow_stat::CargoDistSettings,
    /// GameScript-lite: story / goals / league (#43).
    #[serde(default)]
    pub gs: crate::gs::GsState,
    /// RNG global de simulación (`_random` en `OpenTTD`): rating, subsidios, averías, etc.
    #[serde(default = "default_random", alias = "cargo_rng")]
    pub random: crate::linkgraph_parity::Randomizer,
    /// RNG interactivo / UI (`_interactive_random` en `OpenTTD`); no afecta la sim determinista.
    #[serde(default = "default_interactive_random")]
    pub interactive_random: crate::linkgraph_parity::Randomizer,
    /// Índice LFSR del tile loop (`_cur_tileloop_tile` en `OpenTTD`).
    #[serde(default = "crate::map::tile_loop::default_cur_tileloop_tile")]
    pub cur_tileloop_tile: u32,
    /// Inflación compuesta, recesiones y escala global de `max_loan` (`_economy`).
    #[serde(default)]
    pub global_economy: crate::economy::GlobalEconomy,
    /// No mandar a servicio si no hay averías (`order.no_servicing_if_no_breakdowns`).
    #[serde(default = "default_true")]
    pub no_servicing_if_no_breakdowns: bool,
    /// Nivel de averías (`difficulty.vehicle_breakdowns`: 0=ninguna, 1=reducidas, 2=normales).
    #[serde(default = "default_vehicle_breakdowns")]
    pub vehicle_breakdowns: u8,
    /// Años de bonificación de subsidio adjudicado (`difficulty.subsidy_duration`).
    #[serde(default = "default_subsidy_duration")]
    pub subsidy_duration: u8,
    /// Índice de multiplicador de subsidio (`difficulty.subsidy_multiplier`: 0..=3).
    #[serde(default = "default_subsidy_multiplier")]
    pub subsidy_multiplier: u8,
    /// `economy.timekeeping_units == Wallclock` — meses económicos de 30 días.
    #[serde(default)]
    pub using_wallclock_units: bool,

    // ───── Campos efímeros (NO persistidos) ─────
    /// Datos de runtime que no se guardan en el save JSON.
    #[serde(skip)]
    pub runtime: SimulationRuntime,
    /// Colas de construcción IA (una obra activa por rival); no se persisten.
    #[serde(skip, default)]
    pub ai_build_queues: Vec<crate::ai::AiBuildQueue>,
}

const fn default_true() -> bool {
    true
}

const fn default_vehicle_breakdowns() -> u8 {
    2
}

const fn default_subsidy_duration() -> u8 {
    1
}

const fn default_subsidy_multiplier() -> u8 {
    1
}

const fn default_disaster_timer() -> u64 {
    crate::disaster::DISASTER_CHECK_INTERVAL
}

const fn default_current_tram_type() -> crate::road_type::RoadType {
    crate::road_type::RoadType::Tram
}

const fn default_next_sign_id() -> u32 {
    1
}

fn default_random() -> crate::linkgraph_parity::Randomizer {
    crate::linkgraph_parity::Randomizer::new(1)
}

fn default_interactive_random() -> crate::linkgraph_parity::Randomizer {
    crate::linkgraph_parity::Randomizer::new(1u32.wrapping_mul(0x0123_4567))
}

impl GameState {
    #[must_use]
    pub fn new(map_width: u32, map_height: u32) -> Self {
        let mut state = Self {
            map: Map::new_flat(map_width, map_height, 1),
            tick: GameTick::default(),
            calendar: crate::timer::CalendarTimer::from_tick(0),
            economy_timer: crate::timer::EconomyTimer::from_tick(0),
            industries: Vec::new(),
            vehicles: Vec::new(),
            stations: Vec::new(),
            towns: Vec::new(),
            stats: SimStats::default(),
            economy: CompanyEconomy::default(),
            company_colour: 0,
            companies: vec![crate::company::Company::player(
                CompanyEconomy::default(),
                0,
            )],
            active_company: crate::company::CompanyId::PLAYER,
            current_rail_type: crate::rail_type::RailType::Rail,
            current_road_type: crate::road_type::RoadType::Road,
            current_tram_type: crate::road_type::RoadType::Tram,
            road_type_catalog: crate::road_type::vanilla_road_type_catalog(),
            current_station_class: crate::station_class::StationClassId::Default,
            current_station_spec: crate::station_class::StationSpecId::DefaultRail,
            station_class_catalog: crate::station_class::vanilla_station_class_catalog(),
            station_spec_catalog: crate::station_class::vanilla_station_spec_catalog(),
            current_road_stop_class: None,
            current_road_stop_spec: None,
            road_stop_class_catalog: Vec::new(),
            road_stop_spec_catalog: Vec::new(),
            engine_catalog: crate::engine::vanilla_engine_catalog(),
            industry_tile_spec_catalog: Vec::new(),
            newgrf_animated_industry_tiles: std::collections::HashSet::new(),
            industry_tile_overrides: crate::industry_tile::empty_industry_tile_overrides(),
            industry_spec_catalog: Vec::new(),
            industry_overrides: crate::industry_spec::empty_industry_overrides(),
            house_spec_catalog: Vec::new(),
            house_overrides: crate::house_spec::empty_house_overrides(),
            airport_tile_spec_catalog: Vec::new(),
            airport_tile_overrides: crate::airport_tile_spec::empty_airport_tile_overrides(),
            airport_spec_catalog: Vec::new(),
            airport_vanilla_disabled: vec![
                false;
                crate::airport_class::NEW_AIRPORT_OFFSET as usize
            ],
            badge_catalog: Vec::new(),
            sound_effect_catalog: Vec::new(),
            cargo_spec_catalog: Vec::new(),
            object_spec_catalog: Vec::new(),
            bridge_spec_catalog: crate::bridge_spec::vanilla_bridge_spec_catalog(),
            canal_feature_catalog: crate::canal_spec::vanilla_canal_feature_catalog(),
            current_object_spec: 0,
            current_airport_class: crate::airport_class::AirportClassId::Small,
            current_airport_spec: crate::airport_class::AirportSpecId::Small,
            current_airport_newgrf_id: None,
            climate: Climate::default(),
            world_seed: 0,
            jgr_tunnels_from_footer: Vec::new(),
            news: crate::news::NewsQueue::default(),
            news_first_vehicle_running_sent: false,
            autoreplace_rules: Vec::new(),
            vehicle_groups: Vec::new(),
            shared_order_lists: Vec::new(),
            subsidies: Vec::new(),
            next_subsidy_id: 1,
            disasters_enabled: true,
            station_noise_level: false,
            construction: crate::construction_settings::ConstructionSettings::default(),
            town_council_tolerance: crate::town::TownCouncilTolerance::default(),
            disaster_timer: default_disaster_timer(),
            disaster_crafts: Vec::new(),
            pathfinding: crate::pathfinding_settings::PathfindingSettings::default(),
            train_acceleration_model: crate::engine::TrainAccelerationModel::Original,
            ai: crate::ai::AiSettings::default(),
            cheats: crate::cheats::CheatsState::default(),
            order: crate::cargo::OrderSettings::default(),
            newgrf_stack: crate::newgrf_config::default_vanilla_stack(),
            signs: Vec::new(),
            next_sign_id: 1,
            bankruptcy_streak: 0,
            game_finished: false,
            link_graph: crate::link_graph::LinkGraphStats::default(),
            cargo_dist: crate::flow_stat::CargoDistSettings::default(),
            gs: crate::gs::GsState::default(),
            random: crate::linkgraph_parity::Randomizer::new(1),
            interactive_random: default_interactive_random(),
            cur_tileloop_tile: crate::map::tile_loop::default_cur_tileloop_tile(),
            global_economy: crate::economy::GlobalEconomy::new(),
            no_servicing_if_no_breakdowns: true,
            vehicle_breakdowns: default_vehicle_breakdowns(),
            subsidy_duration: default_subsidy_duration(),
            subsidy_multiplier: default_subsidy_multiplier(),
            using_wallclock_units: false,
            runtime: SimulationRuntime::new(),
            ai_build_queues: Vec::new(),
        };
        state.finish_new_game_startup();
        state
    }

    /// Inicializa economía global (inflación previa a 1950, `max_loan` escalado).
    pub fn finish_new_game_startup(&mut self) {
        let start_year = crate::news::CALENDAR_BASE_YEAR;
        self.global_economy.startup(&mut self.random, start_year);
        self.sync_scaled_max_loan();
    }

    /// Propaga `global_economy.scaled_max_loan()` a todas las compañías y al espejo activo.
    pub fn sync_scaled_max_loan(&mut self) {
        let max_loan = self.global_economy.scaled_max_loan();
        for company in &mut self.companies {
            company.economy.max_loan = max_loan;
        }
        self.economy.max_loan = max_loan;
    }

    /// Crea un estado a partir de un mapa ya construido (sin industrias ni vehículos).
    #[must_use]
    pub fn from_map(map: Map) -> Self {
        let mut state = Self {
            map,
            tick: GameTick::default(),
            calendar: crate::timer::CalendarTimer::from_tick(0),
            economy_timer: crate::timer::EconomyTimer::from_tick(0),
            industries: Vec::new(),
            vehicles: Vec::new(),
            stations: Vec::new(),
            towns: Vec::new(),
            stats: SimStats::default(),
            economy: CompanyEconomy::default(),
            company_colour: 0,
            companies: vec![crate::company::Company::player(
                CompanyEconomy::default(),
                0,
            )],
            active_company: crate::company::CompanyId::PLAYER,
            current_rail_type: crate::rail_type::RailType::Rail,
            current_road_type: crate::road_type::RoadType::Road,
            current_tram_type: crate::road_type::RoadType::Tram,
            road_type_catalog: crate::road_type::vanilla_road_type_catalog(),
            current_station_class: crate::station_class::StationClassId::Default,
            current_station_spec: crate::station_class::StationSpecId::DefaultRail,
            station_class_catalog: crate::station_class::vanilla_station_class_catalog(),
            station_spec_catalog: crate::station_class::vanilla_station_spec_catalog(),
            current_road_stop_class: None,
            current_road_stop_spec: None,
            road_stop_class_catalog: Vec::new(),
            road_stop_spec_catalog: Vec::new(),
            engine_catalog: crate::engine::vanilla_engine_catalog(),
            industry_tile_spec_catalog: Vec::new(),
            newgrf_animated_industry_tiles: std::collections::HashSet::new(),
            industry_tile_overrides: crate::industry_tile::empty_industry_tile_overrides(),
            industry_spec_catalog: Vec::new(),
            industry_overrides: crate::industry_spec::empty_industry_overrides(),
            house_spec_catalog: Vec::new(),
            house_overrides: crate::house_spec::empty_house_overrides(),
            airport_tile_spec_catalog: Vec::new(),
            airport_tile_overrides: crate::airport_tile_spec::empty_airport_tile_overrides(),
            airport_spec_catalog: Vec::new(),
            airport_vanilla_disabled: vec![
                false;
                crate::airport_class::NEW_AIRPORT_OFFSET as usize
            ],
            badge_catalog: Vec::new(),
            sound_effect_catalog: Vec::new(),
            cargo_spec_catalog: Vec::new(),
            object_spec_catalog: Vec::new(),
            bridge_spec_catalog: crate::bridge_spec::vanilla_bridge_spec_catalog(),
            canal_feature_catalog: crate::canal_spec::vanilla_canal_feature_catalog(),
            current_object_spec: 0,
            current_airport_class: crate::airport_class::AirportClassId::Small,
            current_airport_spec: crate::airport_class::AirportSpecId::Small,
            current_airport_newgrf_id: None,
            climate: Climate::default(),
            world_seed: 0,
            jgr_tunnels_from_footer: Vec::new(),
            news: crate::news::NewsQueue::default(),
            news_first_vehicle_running_sent: false,
            autoreplace_rules: Vec::new(),
            vehicle_groups: Vec::new(),
            shared_order_lists: Vec::new(),
            subsidies: Vec::new(),
            next_subsidy_id: 1,
            disasters_enabled: true,
            station_noise_level: false,
            construction: crate::construction_settings::ConstructionSettings::default(),
            town_council_tolerance: crate::town::TownCouncilTolerance::default(),
            disaster_timer: default_disaster_timer(),
            disaster_crafts: Vec::new(),
            pathfinding: crate::pathfinding_settings::PathfindingSettings::default(),
            train_acceleration_model: crate::engine::TrainAccelerationModel::Original,
            ai: crate::ai::AiSettings::default(),
            cheats: crate::cheats::CheatsState::default(),
            order: crate::cargo::OrderSettings::default(),
            newgrf_stack: crate::newgrf_config::default_vanilla_stack(),
            signs: Vec::new(),
            next_sign_id: 1,
            bankruptcy_streak: 0,
            game_finished: false,
            link_graph: crate::link_graph::LinkGraphStats::default(),
            cargo_dist: crate::flow_stat::CargoDistSettings::default(),
            gs: crate::gs::GsState::default(),
            random: crate::linkgraph_parity::Randomizer::new(1),
            interactive_random: default_interactive_random(),
            cur_tileloop_tile: crate::map::tile_loop::default_cur_tileloop_tile(),
            global_economy: crate::economy::GlobalEconomy::new(),
            no_servicing_if_no_breakdowns: true,
            vehicle_breakdowns: default_vehicle_breakdowns(),
            subsidy_duration: default_subsidy_duration(),
            subsidy_multiplier: default_subsidy_multiplier(),
            using_wallclock_units: false,
            runtime: SimulationRuntime::new(),
            ai_build_queues: Vec::new(),
        };
        state.finish_new_game_startup();
        state
    }

    /// Reconstruye datos efímeros tras cargar un save (caches, RNG, etc.).
    ///
    /// Llama este método después de deserializar desde JSON para inicializar
    /// correctamente los campos de [`SimulationRuntime`].
    pub fn hydrate_runtime(&mut self) {
        self.ensure_timers_from_tick();
        if self.cur_tileloop_tile == 0 {
            self.cur_tileloop_tile = crate::map::tile_loop::default_cur_tileloop_tile();
        }
        self.runtime = SimulationRuntime::new();
        self.rebuild_station_flows();
        self.sanitize_all_vehicle_orders();
        self.sync_scaled_max_loan();
    }

    /// Deriva los relojes desde `tick` si el save no los tenía (migración serde).
    pub fn ensure_timers_from_tick(&mut self) {
        if self.calendar.year == 0 && self.calendar.month == 0 {
            self.sync_timers_from_tick();
        }
    }

    /// Fuerza la alineación de relojes con el tick actual (tests / cheats).
    pub fn sync_timers_from_tick(&mut self) {
        self.calendar = crate::timer::CalendarTimer::from_tick(self.tick.get());
        self.economy_timer = crate::timer::EconomyTimer::from_tick_with_wallclock(
            self.tick.get(),
            self.using_wallclock_units,
        );
    }

    /// Avanza los relojes de calendario y economía un tick de simulación.
    pub(crate) fn advance_game_timers(&mut self) {
        self.economy_timer.using_wallclock = self.using_wallclock_units;
        self.runtime.calendar_triggers = self.calendar.elapsed_tick();
        self.runtime.economy_triggers = self.economy_timer.elapsed_tick();
    }

    /// Sanitiza `current_order` en todos los vehículos para prevenir indexación fuera de límites.
    ///
    /// Debe llamarse después de cargar un save o deserializar desde JSON.
    pub fn sanitize_all_vehicle_orders(&mut self) {
        for vehicle in &mut self.vehicles {
            vehicle.sanitize_current_order();
        }
    }

    /// Reconstruye `StationFlows` con el pipeline `OpenTTD` (Demand + MCF1/2).
    pub fn rebuild_station_flows(&mut self) {
        use crate::flow_stat::{DistributionType, StationFlows};
        use crate::linkgraph_parity::{
            build_jobs_from_game, run_full_pipeline, to_station_flows_helper,
        };

        self.runtime.station_flow_rebuilds = self.runtime.station_flow_rebuilds.saturating_add(1);

        if matches!(self.cargo_dist.distribution, DistributionType::Manual) {
            self.runtime.station_flows = StationFlows::default();
            return;
        }

        let (map_w, map_h) = self.map.dimensions();
        let jobs = build_jobs_from_game(
            &self.stations,
            &self.link_graph,
            self.cargo_dist.distribution,
            map_w,
            map_h,
        );
        let mut merged = StationFlows::default();
        for (cargo, mut job) in jobs {
            run_full_pipeline(&mut job);
            let part = to_station_flows_helper(&job, cargo);
            for (station, table) in part.by_station {
                let dest = merged.by_station.entry(station).or_default();
                for (c, map) in table.by_cargo {
                    let dest_map = dest.by_cargo.entry(c).or_default();
                    for (origin, fs) in map.by_origin {
                        for (via, amount) in fs.shares {
                            dest_map.add_flow(origin, via, amount);
                        }
                    }
                }
            }
        }
        self.runtime.station_flows = merged;
        // P3.18: `RerouteCargo` cuando cambian los flows (hop obsoleto).
        self.reroute_stale_cargo_hops();
    }

    /// Reasigna `next_hop` de packets cuya vía ya no existe en los flows.
    fn reroute_stale_cargo_hops(&mut self) {
        use crate::cargo::ALL_CARGO_TYPES;
        use crate::cargo_packet::StationHopKey;
        use crate::flow_stat::DistributionType;

        if matches!(self.cargo_dist.distribution, DistributionType::Manual) {
            return;
        }

        let station_count = self.stations.len();
        for st_idx in 0..station_count {
            let st_pos = self.stations[st_idx].pos;
            let hops: Vec<_> = self.stations[st_idx]
                .cargo_packets
                .by_next_hop
                .keys()
                .filter_map(|StationHopKey(h)| *h)
                .collect();
            for avoid in hops {
                for cargo in ALL_CARGO_TYPES {
                    // ¿Algún share sigue apuntando a `avoid`?
                    let still_valid = self
                        .runtime
                        .station_flows
                        .by_station
                        .get(&st_pos)
                        .and_then(|t| t.by_cargo.get(&cargo))
                        .is_some_and(|m| {
                            m.by_origin.values().any(|fs| {
                                fs.shares.iter().any(|(via, amt)| *via == avoid && *amt > 0)
                            })
                        });
                    if still_valid {
                        continue;
                    }
                    let flows = &self.runtime.station_flows;
                    let rng = &mut self.random;
                    let _ = self.stations[st_idx].cargo_packets.reroute(
                        u32::MAX,
                        avoid,
                        Some(st_pos),
                        |origin| {
                            let origin = origin.unwrap_or(st_pos);
                            flows.get_via_excluding(st_pos, cargo, origin, avoid, Some(st_pos), rng)
                        },
                    );
                }
            }
        }

        // Vehicles unloading / staged transfer at stations.
        for v_idx in 0..self.vehicles.len() {
            let Some(st_pos) = self.vehicles[v_idx].last_station_visited else {
                continue;
            };
            let Some(cargo) = self.vehicles[v_idx].cargo_type else {
                continue;
            };
            let hops: Vec<_> = self.vehicles[v_idx]
                .cargo_packets
                .packets
                .iter()
                .filter_map(|p| p.next_hop)
                .collect();
            for avoid in hops {
                let still_valid = self
                    .runtime
                    .station_flows
                    .by_station
                    .get(&st_pos)
                    .and_then(|t| t.by_cargo.get(&cargo))
                    .is_some_and(|m| {
                        m.by_origin
                            .values()
                            .any(|fs| fs.shares.iter().any(|(via, amt)| *via == avoid && *amt > 0))
                    });
                if still_valid {
                    continue;
                }
                let flows = &self.runtime.station_flows;
                let rng = &mut self.random;
                let _ = self.vehicles[v_idx].cargo_packets.reroute(
                    u32::MAX,
                    avoid,
                    Some(st_pos),
                    |origin| {
                        let origin = origin.unwrap_or(st_pos);
                        flows.get_via_excluding(st_pos, cargo, origin, avoid, Some(st_pos), rng)
                    },
                );
            }
        }
    }

    /// Activa la traza de paridad: cada `step()` añade un registro por tick.
    ///
    /// La línea base para derivar eventos es el estado actual. Coste cero
    /// mientras esté desactivada (`self.runtime.parity == None`).
    pub fn enable_parity_trace(&mut self) {
        self.runtime.parity = Some(crate::parity::ParityTracer::with_baseline(self));
    }

    /// Extrae y vacía los registros de paridad acumulados (vacío si la traza
    /// está desactivada).
    pub fn take_parity_records(&mut self) -> Vec<crate::parity::TickRecord> {
        self.runtime
            .parity
            .as_mut()
            .map(crate::parity::ParityTracer::drain_records)
            .unwrap_or_default()
    }

    /// Avanza un tick de simulación (equivalente conceptual a un frame lógico del juego).
    ///
    /// Orden dentro del tick:
    /// 1. Producción de industrias.
    /// 2. Carga/descarga según posición actual del vehículo.
    /// 3. Movimiento del vehículo (vehicle.step).
    pub fn step(&mut self) {
        crate::sim_step::step(self);
    }

    /// Igual que [`Self::step`] con tiempos por fase (profiling headless).
    #[must_use]
    pub fn step_profiled(&mut self) -> crate::sim_step::TickPhaseTimings {
        crate::sim_step::step_profiled(self)
    }

    /// Aplica una secuencia de comandos en orden (núcleo I8 / #21).
    ///
    /// No avanza ticks: el caller debe llamar [`Self::step`] según el protocolo
    /// de red o el harness de replay (`docs/adr/0001-multiplayer-v1.md`).
    ///
    /// # Errors
    ///
    /// Propaga el primer [`crate::command::CommandError`] y deja el estado
    /// parcialmente aplicado.
    pub fn apply_command_log(
        &mut self,
        cmds: &[crate::command::Command],
    ) -> Result<(), crate::command::CommandError> {
        for cmd in cmds {
            crate::command::apply_command(self, cmd)?;
        }
        Ok(())
    }

    /// Serializa el estado a JSON (UTF-8) para guardado o depuración.
    ///
    /// # Errors
    ///
    /// Falla si algún campo no es serializable (no debería ocurrir en tipos propios).
    pub fn save_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Restaura un estado desde JSON producido por [`Self::save_json`].
    ///
    /// # Errors
    ///
    /// Devuelve error si el texto no es JSON válido o no coincide el esquema.
    pub fn load_json(s: &str) -> Result<Self, serde_json::Error> {
        let mut state: Self = serde_json::from_str(s)?;
        state.hydrate_runtime();
        Ok(state)
    }

    /// Enlaces wormhole JGR (`tile_n` ↔ `tile_s`) para pathfinding.
    #[must_use]
    pub fn jgr_tunnel_wormholes(&self) -> crate::pathfinder::TunnelWormholes {
        crate::pathfinder::TunnelWormholes::from_jgr_records(
            &self.map,
            &self.jgr_tunnels_from_footer,
        )
    }

    /// Asegura al menos la compañía jugador (sin pisar economías del pool).
    pub fn ensure_companies(&mut self) {
        if self.companies.is_empty() {
            self.companies.push(crate::company::Company::player(
                self.economy,
                self.company_colour,
            ));
            self.active_company = crate::company::CompanyId::PLAYER;
        }
        // Migración: historial global → compañía activa si aún no tiene series.
        let active_idx = self.active_company.index();
        if let Some(c) = self.companies.get_mut(active_idx)
            && c.economy_history.samples.is_empty()
            && !self.stats.economy_history.samples.is_empty()
        {
            c.economy_history = self.stats.economy_history.clone();
            if c.cargo_income_earned == 0 {
                c.cargo_income_earned = self.stats.cargo_income_earned;
            }
            if c.vehicle_running_costs == 0 {
                c.vehicle_running_costs = self.stats.vehicle_running_costs;
            }
            if c.cargo_deliveries == 0 {
                c.cargo_deliveries = self.stats.cargo_deliveries;
            }
        }
    }

    /// Antes de un comando del jugador: reabsorbe espejos mutados (tests/UI).
    pub fn prepare_player_command(&mut self) {
        self.ensure_companies();
        self.sync_active_from_mirrors();
    }

    /// Copia economía/color de la compañía activa a los campos espejo.
    pub fn sync_mirrors_from_active(&mut self) {
        let idx = self.active_company.index();
        if let Some(c) = self.companies.get(idx) {
            self.economy = c.economy;
            self.company_colour = c.colour;
        }
    }

    /// Escribe los espejos en la compañía activa (tras comandos del jugador).
    pub fn sync_active_from_mirrors(&mut self) {
        let idx = self.active_company.index();
        if let Some(c) = self.companies.get_mut(idx) {
            c.economy = self.economy;
            c.colour = self.company_colour;
        }
    }

    /// Cambia la compañía activa (comandos / HUD) y sincroniza espejos.
    ///
    /// Devuelve `false` si `id` no está en el pool.
    pub fn set_active_company(&mut self, id: crate::company::CompanyId) -> bool {
        self.ensure_companies();
        if !self.companies.iter().any(|c| c.id == id) {
            return false;
        }
        if self.active_company == id {
            return true;
        }
        self.sync_active_from_mirrors();
        self.active_company = id;
        self.sync_mirrors_from_active();
        true
    }

    /// Acredita dinero a una compañía (y espejo si es la activa).
    pub fn credit_company(&mut self, id: crate::company::CompanyId, amount: i64) {
        if amount == 0 {
            return;
        }
        if let Some(c) = self.companies.get_mut(id.index()) {
            c.economy.money = c.economy.money.saturating_add(amount);
        }
        if id == self.active_company {
            self.economy.money = self.economy.money.saturating_add(amount);
        }
    }

    /// Debita dinero de una compañía (y espejo si es la activa).
    pub fn debit_company(&mut self, id: crate::company::CompanyId, amount: i64) {
        if amount == 0 {
            return;
        }
        if let Some(c) = self.companies.get_mut(id.index()) {
            c.economy.money = c.economy.money.saturating_sub(amount);
        }
        if id == self.active_company {
            self.economy.money = self.economy.money.saturating_sub(amount);
        }
    }

    /// Añade rival `TransCargo` si aún no existe (por nombre).
    pub fn ensure_rival_transcargo(&mut self) {
        self.ensure_companies();
        if crate::company::company_id_by_name(
            &self.companies,
            crate::company::RIVAL_NAME_TRANSCARGO,
        )
        .is_some()
        {
            return;
        }
        let id = u8::try_from(self.companies.len()).unwrap_or(1);
        let colour = crate::company::first_free_company_colour(&self.companies);
        let mut rival = crate::company::Company::rival_transcargo(
            CompanyEconomy {
                money: 200_000,
                loan: 0,
                max_loan: crate::economy::DEFAULT_MAX_LOAN,
            },
            colour,
        );
        rival.id = crate::company::CompanyId(id);
        self.companies.push(rival);
    }

    /// Añade rival `RoadHaul` (buses) si aún no existe.
    pub fn ensure_rival_roadhaul(&mut self) {
        self.ensure_companies();
        if crate::company::company_id_by_name(&self.companies, crate::company::RIVAL_NAME_ROADHAUL)
            .is_some()
        {
            return;
        }
        let id = u8::try_from(self.companies.len()).unwrap_or(2);
        let colour = crate::company::first_free_company_colour(&self.companies);
        let mut rival = crate::company::Company::rival_roadhaul(
            CompanyEconomy {
                money: 150_000,
                loan: 0,
                max_loan: crate::economy::DEFAULT_MAX_LOAN,
            },
            colour,
        );
        rival.id = crate::company::CompanyId(id);
        self.companies.push(rival);
    }

    /// Rivales Rust de nueva partida: `TransCargo` + `RoadHaul`.
    pub fn ensure_rival_ais(&mut self) {
        self.ensure_rival_transcargo();
        self.ensure_rival_roadhaul();
    }

    /// Economía de una compañía (fallback al espejo del jugador).
    #[must_use]
    pub fn company_economy(&self, id: crate::company::CompanyId) -> CompanyEconomy {
        self.companies
            .get(id.index())
            .map_or(self.economy, |c| c.economy)
    }
}
