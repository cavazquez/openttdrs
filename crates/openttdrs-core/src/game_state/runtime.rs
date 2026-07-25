use crate::command::Command;
use crate::map::{Tile, TileCoord};
use std::collections::{HashSet, VecDeque};

/// Campos efímeros de la simulación (no persistidos; reconstruidos tras carga).
///
/// Todos los campos aquí tienen `#[serde(skip)]` implícito por no estar en
/// `GameState` serializado. Estos datos no aparecen en el save JSON y deben
/// reconstruirse/limpiarse tras cargar un save.
#[derive(Debug, Clone, Default)]
pub struct SimulationRuntime {
    /// Caché efímera de rutas A* (no persistida).
    pub path_cache: crate::pathfinder::PathCache,

    /// Ingresos recién cobrados (drenados por el cliente para texto flotante).
    pub pending_income_popups: Vec<super::IncomePopup>,

    /// Eventos del tick para audio/FX/UI en el cliente.
    pub pending_sim_events: crate::sim_events::SimEventQueue,

    /// Teselas industriales con `m1` mutado este tick (obra P6 → remap cliente).
    pub industry_tile_dirty: Vec<TileCoord>,

    /// Teselas de paisaje (nieve estacional, etc.) mutadas este tick → remap cliente.
    pub landscape_tile_dirty: Vec<TileCoord>,

    /// Teselas visitadas por `RunTileLoop` este tick (una pasada LFSR; no persistido).
    pub tile_loop_visited: Vec<(TileCoord, Tile)>,

    /// Teselas con señales cuyo estado verde/rojo cambió este tick (remap cliente).
    pub signal_tile_dirty: Vec<TileCoord>,

    /// Cola `_globset`: teselas que invalidan señales (movimiento / construcción).
    pub signal_globset: crate::rail_signals::SignalGlobSet,

    /// Índice efímero de señales; evita barridos completos por cada drenado.
    pub signal_spatial_index: crate::rail_signals::SignalSpatialIndex,

    /// Teselas con reserva PBS activa cuyo `m2_hi` cambió (remap cliente).
    pub reservation_tile_dirty: Vec<TileCoord>,

    /// Conjunto de teselas con reserva PBS del tick anterior (sincronización mapa).
    pub reservation_tiles_active: HashSet<TileCoord>,

    /// Eventos de noticia recién creados (consumidos por el cliente).
    pub pending_news_events: Vec<crate::news::PendingNewsEvent>,

    /// Claves `(vehículo, tipo de aviso)` ya notificadas mientras persiste la condición.
    pub news_advice_sent: HashSet<u64>,

    /// Último día de calendario en que se ejecutó purga de noticias antiguas.
    pub news_last_purge_day: u64,

    /// Bordes del reloj de calendario en el tick actual.
    pub calendar_triggers: crate::timer::TimerTriggers,

    /// Bordes del reloj de economía en el tick actual.
    pub economy_triggers: crate::timer::TimerTriggers,

    /// Tracer de paridad opcional (coste cero si es `None`; no se persiste).
    pub parity: Option<crate::parity::ParityTracer>,

    /// Slots `SPR_SHORE_BASE + 0..17` desde Action5 `0x0D` (`None` = `OpenGFX`).
    /// Se reconstruye al aplicar el stack; no se persiste en el save.
    pub shore_newgrf_sprites: Vec<Option<crate::newgrf_sprites::DecodedSprite>>,

    /// Slots Action5 catenary `0x05` (wires/entrances/pylons; `None` = `OpenGFX`).
    pub catenary_newgrf_sprites: Vec<Option<crate::newgrf_sprites::DecodedSprite>>,

    /// `FlowStat` reconstruidos desde `link_graph` (no persistidos).
    pub station_flows: crate::flow_stat::StationFlows,

    /// Reconstrucciones completas de `station_flows` desde que se creó este runtime.
    ///
    /// Contador diagnóstico para detectar regresiones en el hot path de `CargoDist`;
    /// no forma parte del estado autoritativo ni se persiste.
    pub station_flow_rebuilds: u64,

    /// Grabador opcional: cada `apply_command` exitoso se encola (plan IA progresiva).
    pub command_recorder: Option<VecDeque<Command>>,
}

impl SimulationRuntime {
    /// Crea un runtime con valores por defecto apropiados para una nueva partida.
    #[must_use]
    pub fn new() -> Self {
        Self {
            path_cache: crate::pathfinder::PathCache::default(),
            pending_income_popups: Vec::new(),
            pending_sim_events: crate::sim_events::SimEventQueue::new(),
            industry_tile_dirty: Vec::new(),
            landscape_tile_dirty: Vec::new(),
            tile_loop_visited: Vec::new(),
            signal_tile_dirty: Vec::new(),
            signal_globset: HashSet::new(),
            signal_spatial_index: crate::rail_signals::SignalSpatialIndex::default(),
            reservation_tile_dirty: Vec::new(),
            reservation_tiles_active: HashSet::new(),
            pending_news_events: Vec::new(),
            news_advice_sent: HashSet::new(),
            news_last_purge_day: 0,
            calendar_triggers: crate::timer::TimerTriggers::default(),
            economy_triggers: crate::timer::TimerTriggers::default(),
            parity: None,
            shore_newgrf_sprites: Vec::new(),
            catenary_newgrf_sprites: Vec::new(),
            station_flows: crate::flow_stat::StationFlows::default(),
            station_flow_rebuilds: 0,
            command_recorder: None,
        }
    }

    /// Limpia las estructuras efímeras manteniendo capacidades asignadas cuando sea apropiado.
    pub fn clear_transient(&mut self) {
        self.pending_income_popups.clear();
        self.pending_sim_events.discard_all();
        self.industry_tile_dirty.clear();
        self.landscape_tile_dirty.clear();
        self.tile_loop_visited.clear();
        self.signal_tile_dirty.clear();
        self.signal_globset.clear();
        self.reservation_tile_dirty.clear();
        self.pending_news_events.clear();
    }
}
