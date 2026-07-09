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
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CompanyEconomy {
    pub money: i64,
    pub loan: i64,
    /// Tope de préstamo (`economy.cpp` `max_loan`; por defecto 300 000).
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

pub const ROAD_BUILD_COST: i64 = 10;
pub const RAIL_BUILD_COST: i64 = 25;
pub const STATION_BUILD_COST: i64 = 200;
/// Coste de waypoint ferroviario (`Price::BuildWaypointRail` en `OpenTTD`).
pub const WAYPOINT_BUILD_COST: i64 = 100;
pub const DEPOT_BUILD_COST: i64 = 150;
pub const TUNNEL_BUILD_COST_PER_TILE: i64 = 90;
pub const BRIDGE_BUILD_COST_PER_TILE: i64 = 70;
pub const CLEAR_TILE_COST: i64 = 5;
/// Precio base por esquina (`PriceBaseSpec` 250 → normalizado dificultad media ≈ 500).
pub const TERRAFORM_BASE_PRICE: i64 = 500;
/// Precio base por tesela de terreno comprado (`Price::BuildObject` / owned land).
pub const BUY_LAND_BASE_PRICE: i64 = 50;
/// Alias en tick 0 (sin inflación de precios); preferir [`crate::economy::terraform_cost_per_corner`].
pub const TERRAFORM_COST: i64 = TERRAFORM_BASE_PRICE;

/// Pago plano legado (sustituido por [`crate::economy::transported_goods_income`]).
#[deprecated(note = "usar economy::transported_goods_income")]
pub const CARGO_DELIVERY_PAYMENT: i64 = 12;

/// Estado global mínimo del mundo simulado.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GameState {
    pub map: Map,
    pub tick: GameTick,
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
    /// Clima del paisaje (`LandscapeType` en `OpenTTD`).
    #[serde(default)]
    pub climate: Climate,
    /// Semilla de generación procedural (0 = sin terreno aleatorio explícito).
    #[serde(default)]
    pub world_seed: u64,
    /// Túneles JGR decodificados desde footer `TNBP` del `.ottdmap` (vacío si no hay o no aplica).
    #[serde(default)]
    pub jgr_tunnels_from_footer: Vec<JgrTunnelRecord>,
    /// Caché efímera de rutas A* (no persistida).
    #[serde(skip)]
    pub path_cache: crate::pathfinder::PathCache,
    /// Ingresos recién cobrados (drenados por el cliente para texto flotante).
    #[serde(skip)]
    pub pending_income_popups: Vec<IncomePopup>,
    /// Eventos del tick para audio/FX/UI en el cliente.
    #[serde(skip)]
    pub pending_sim_events: crate::sim_events::SimEventQueue,
    /// Teselas industriales con `m1` mutado este tick (obra P6 → remap cliente).
    #[serde(skip)]
    pub industry_tile_dirty: Vec<TileCoord>,
    /// Teselas con señales cuyo estado verde/rojo cambió este tick (remap cliente).
    #[serde(skip)]
    pub signal_tile_dirty: Vec<TileCoord>,
    /// Cola `_globset`: teselas que invalidan señales (movimiento / construcción).
    #[serde(skip, default)]
    pub signal_globset: crate::rail_signals::SignalGlobSet,
    /// Teselas con reserva PBS activa cuyo `m2_hi` cambió (remap cliente).
    #[serde(skip)]
    pub reservation_tile_dirty: Vec<TileCoord>,
    /// Conjunto de teselas con reserva PBS del tick anterior (sincronización mapa).
    #[serde(skip, default)]
    pub reservation_tiles_active: std::collections::HashSet<TileCoord>,
    /// Historial de noticias (más reciente al frente).
    #[serde(default)]
    pub news: crate::news::NewsQueue,
    /// Eventos de noticia recién creados (consumidos por el cliente).
    #[serde(skip)]
    pub pending_news_events: Vec<crate::news::PendingNewsEvent>,
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
    /// Claves `(vehículo, tipo de aviso)` ya notificadas mientras persiste la condición.
    #[serde(skip, default)]
    pub news_advice_sent: std::collections::HashSet<u64>,
    /// Último día de calendario en que se ejecutó purga de noticias antiguas.
    #[serde(skip, default)]
    pub news_last_purge_day: u64,
    /// Tracer de paridad opcional (coste cero si es `None`; no se persiste).
    #[serde(skip, default)]
    pub parity: Option<crate::parity::ParityTracer>,
    /// Subsidios activos u ofrecidos.
    #[serde(default)]
    pub subsidies: Vec<crate::subsidy::Subsidy>,
    /// Contador para IDs de subsidio.
    #[serde(default)]
    pub next_subsidy_id: u32,
    /// Desastres ambientales habilitados.
    #[serde(default = "default_true")]
    pub disasters_enabled: bool,
    /// Ticks hasta la próxima comprobación de desastre.
    #[serde(default = "default_disaster_timer")]
    pub disaster_timer: u64,
    /// Ajustes de pathfinding / PBS (`pf.wait_for_pbs_path`, etc.).
    #[serde(default)]
    pub pathfinding: crate::pathfinding_settings::PathfindingSettings,
}

const fn default_true() -> bool {
    true
}

const fn default_disaster_timer() -> u64 {
    crate::disaster::DISASTER_CHECK_INTERVAL
}

impl GameState {
    #[must_use]
    pub fn new(map_width: u32, map_height: u32) -> Self {
        Self {
            map: Map::new_flat(map_width, map_height, 1),
            tick: GameTick::default(),
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
            climate: Climate::default(),
            world_seed: 0,
            jgr_tunnels_from_footer: Vec::new(),
            path_cache: crate::pathfinder::PathCache::default(),
            pending_income_popups: Vec::new(),
            pending_sim_events: crate::sim_events::SimEventQueue::new(),
            industry_tile_dirty: Vec::new(),
            signal_tile_dirty: Vec::new(),
            signal_globset: std::collections::HashSet::new(),
            reservation_tile_dirty: Vec::new(),
            reservation_tiles_active: std::collections::HashSet::new(),
            news: crate::news::NewsQueue::default(),
            pending_news_events: Vec::new(),
            news_first_vehicle_running_sent: false,
            autoreplace_rules: Vec::new(),
            vehicle_groups: Vec::new(),
            shared_order_lists: Vec::new(),
            news_advice_sent: std::collections::HashSet::new(),
            news_last_purge_day: 0,
            parity: None,
            subsidies: Vec::new(),
            next_subsidy_id: 1,
            disasters_enabled: true,
            disaster_timer: default_disaster_timer(),
            pathfinding: crate::pathfinding_settings::PathfindingSettings::default(),
        }
    }

    /// Crea un estado a partir de un mapa ya construido (sin industrias ni vehículos).
    #[must_use]
    pub fn from_map(map: Map) -> Self {
        Self {
            map,
            tick: GameTick::default(),
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
            climate: Climate::default(),
            world_seed: 0,
            jgr_tunnels_from_footer: Vec::new(),
            path_cache: crate::pathfinder::PathCache::default(),
            pending_income_popups: Vec::new(),
            pending_sim_events: crate::sim_events::SimEventQueue::new(),
            industry_tile_dirty: Vec::new(),
            signal_tile_dirty: Vec::new(),
            signal_globset: std::collections::HashSet::new(),
            reservation_tile_dirty: Vec::new(),
            reservation_tiles_active: std::collections::HashSet::new(),
            news: crate::news::NewsQueue::default(),
            pending_news_events: Vec::new(),
            news_first_vehicle_running_sent: false,
            autoreplace_rules: Vec::new(),
            vehicle_groups: Vec::new(),
            shared_order_lists: Vec::new(),
            news_advice_sent: std::collections::HashSet::new(),
            news_last_purge_day: 0,
            parity: None,
            subsidies: Vec::new(),
            next_subsidy_id: 1,
            disasters_enabled: true,
            disaster_timer: default_disaster_timer(),
            pathfinding: crate::pathfinding_settings::PathfindingSettings::default(),
        }
    }

    /// Activa la traza de paridad: cada `step()` añade un registro por tick.
    ///
    /// La línea base para derivar eventos es el estado actual. Coste cero
    /// mientras esté desactivada (`self.parity == None`).
    pub fn enable_parity_trace(&mut self) {
        self.parity = Some(crate::parity::ParityTracer::with_baseline(self));
    }

    /// Extrae y vacía los registros de paridad acumulados (vacío si la traza
    /// está desactivada).
    pub fn take_parity_records(&mut self) -> Vec<crate::parity::TickRecord> {
        self.parity
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
        serde_json::from_str(s)
    }

    /// Enlaces wormhole JGR (`tile_n` ↔ `tile_s`) para pathfinding.
    #[must_use]
    pub fn jgr_tunnel_wormholes(&self) -> crate::pathfinder::TunnelWormholes {
        crate::pathfinder::TunnelWormholes::from_jgr_records(
            &self.map,
            &self.jgr_tunnels_from_footer,
        )
    }

    /// Asegura al menos la compañía jugador y alinea espejos desde el pool.
    pub fn ensure_companies(&mut self) {
        if self.companies.is_empty() {
            self.companies.push(crate::company::Company::player(
                self.economy,
                self.company_colour,
            ));
            self.active_company = crate::company::CompanyId::PLAYER;
        }
        self.sync_mirrors_from_active();
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

    /// Añade rival `TransCargo` si aún no existe (escenarios / nueva partida con IA).
    pub fn ensure_rival_transcargo(&mut self) {
        self.ensure_companies();
        if self.companies.iter().any(|c| c.is_ai) {
            return;
        }
        let id = u8::try_from(self.companies.len()).unwrap_or(1);
        let mut rival = crate::company::Company::rival_transcargo(
            CompanyEconomy {
                money: 200_000,
                loan: 0,
                max_loan: crate::economy::DEFAULT_MAX_LOAN,
            },
            1, // rojo
        );
        rival.id = crate::company::CompanyId(id);
        self.companies.push(rival);
    }

    /// Economía de una compañía (fallback al espejo del jugador).
    #[must_use]
    pub fn company_economy(&self, id: crate::company::CompanyId) -> CompanyEconomy {
        self.companies
            .get(id.index())
            .map_or(self.economy, |c| c.economy)
    }
}
