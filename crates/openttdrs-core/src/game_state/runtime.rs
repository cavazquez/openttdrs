use crate::command::Command;
use crate::map::{Tile, TileCoord};
use std::collections::{HashSet, VecDeque};

/// Campos efímeros de la simulación (no persistidos; reconstruidos tras carga).
///
/// Todos los campos aquí tienen `#[serde(skip)]` implícito por no estar en
/// `GameState` serializado. Estos datos no aparecen en el save JSON y deben
/// reconstruirse/limpiarse tras cargar un save.
#[derive(Debug, Clone)]
pub struct SimulationRuntime {
    /// `VehicleID -> slot` y topología de consists, reconstruidos una vez por tick.
    pub fleet_index: crate::fleet_index::FleetIndex,

    /// Teselas propias de estaciones/terminales -> slots de estación.
    pub terminal_spatial_index: crate::fleet_index::TerminalSpatialIndex,

    /// Depósitos por tipo; evita barridos repetidos de mapas grandes.
    pub depot_spatial_index: crate::depot::DepotSpatialIndex,

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

    /// Teselas con ascensor Large Office en movimiento (`AnimatedTileList`).
    pub active_house_lifts: HashSet<TileCoord>,

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

    /// Slots Action5 signal graphics `0x04` (240; `None` = `OpenGFX`).
    pub signal_action5_newgrf_sprites: Vec<Option<crate::newgrf_sprites::DecodedSprite>>,

    /// Slots Action5 foundations `0x06` (90; `None` = `OpenGFX`).
    pub foundation_newgrf_sprites: Vec<Option<crate::newgrf_sprites::DecodedSprite>>,

    /// Slots Action5 one-way roads `0x09` (18; `None` = `OpenGFX`).
    pub oneway_newgrf_sprites: Vec<Option<crate::newgrf_sprites::DecodedSprite>>,

    /// Slots Action5 road stops `0x11` (8; `None` = `OpenGFX`). No es el catálogo Action0 `RoadStops`.
    pub roadstop_action5_newgrf_sprites: Vec<Option<crate::newgrf_sprites::DecodedSprite>>,

    /// Slots Action5 `OpenTTD` GUI `0x15` (192; `None` = sprite base).
    pub openttd_gui_newgrf_sprites: Vec<Option<crate::newgrf_sprites::DecodedSprite>>,

    /// Slots Action5 airport preview `0x16` (9; `None` = preview vanilla).
    pub airport_preview_newgrf_sprites: Vec<Option<crate::newgrf_sprites::DecodedSprite>>,

    /// Slots Action5 bridge decks `0x1B` (24; `None` = `OpenGFX`).
    pub bridge_decks_newgrf_sprites: Vec<Option<crate::newgrf_sprites::DecodedSprite>>,

    /// Slots Action5 canals `0x08` (65; `None` = `OpenGFX`).
    pub canal_action5_newgrf_sprites: Vec<Option<crate::newgrf_sprites::DecodedSprite>>,

    /// Slots Action5 2CC colour maps `0x0A` (256; `None` = `OpenGFX`).
    pub twocc_action5_newgrf_sprites: Vec<Option<crate::newgrf_sprites::DecodedSprite>>,

    /// Slots Action5 tramway `0x0B` (119; `None` = `OpenGFX`).
    pub tramway_action5_newgrf_sprites: Vec<Option<crate::newgrf_sprites::DecodedSprite>>,

    /// Overrides Action3 `RailType` `Signals`, indexados por `RailType` vanilla.
    pub rail_signal_newgrf: Vec<Option<crate::rail_type::RailSignalSpriteSpec>>,

    /// Action3 `TrackOverlay` por `RailType` (selector 1).
    pub rail_type_overlay_newgrf: Vec<Option<crate::rail_type::RailSignalSpriteSpec>>,

    /// Action3 `Underlay` por `RailType` (selector 0).
    pub rail_type_underlay_newgrf: Vec<Option<crate::rail_type::RailSignalSpriteSpec>>,

    /// Props Action0 runtime por `RailType` vanilla.
    pub rail_type_props: [crate::rail_type::RailTypeRuntimeProps; 4],

    /// Badges asociados a cada `RailType` vanilla/custom representable
    /// (`Action0` prop `0x1E`), indexados por el id de vía.
    pub rail_type_badges: [Vec<u16>; 4],

    /// Techos Action0 `0x14` por `RailType` vanilla (`0` = sin límite).
    /// Espejo de `rail_type_props[].max_speed` para callers existentes.
    pub rail_type_max_speed: [u16; 4],

    /// `FlowStat` reconstruidos desde `link_graph` (no persistidos).
    pub station_flows: crate::flow_stat::StationFlows,

    /// Reconstrucciones completas de `station_flows` desde que se creó este runtime.
    ///
    /// Contador diagnóstico para detectar regresiones en el hot path de `CargoDist`;
    /// no forma parte del estado autoritativo ni se persiste.
    pub station_flow_rebuilds: u64,

    /// Grabador opcional: cada `apply_command` exitoso se encola (plan IA progresiva).
    pub command_recorder: Option<VecDeque<Command>>,

    /// Diagnósticos `NewGRF` del último apply (listas truncadas, badges inválidos, …).
    /// No se persiste; se reconstruye al reaplicar el stack.
    pub newgrf_diagnostics: Vec<String>,

    /// Overrides baseset [`crate::SoundId`] → `(grfid, local_id)` `NewGRF` (Action0 prop `0x0A`).
    /// Índice = `SoundId` 0..72; reconstruido al aplicar Sounds.
    pub sound_overrides: [Option<(u32, u8)>; crate::sound_id::SOUND_COUNT],

    /// Cola de reproducción `NewGRF` (drenable por cliente / tests; no se persiste).
    /// El cliente puede drenar a Bevy Audio más adelante; la AC se valida en core.
    pub pending_newgrf_sounds: Vec<crate::sound_effect::PendingNewgrfSound>,
}

impl Default for SimulationRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl SimulationRuntime {
    /// Crea un runtime con valores por defecto apropiados para una nueva partida.
    #[must_use]
    pub fn new() -> Self {
        Self {
            fleet_index: crate::fleet_index::FleetIndex::default(),
            terminal_spatial_index: crate::fleet_index::TerminalSpatialIndex::default(),
            depot_spatial_index: crate::depot::DepotSpatialIndex::default(),
            path_cache: crate::pathfinder::PathCache::default(),
            pending_income_popups: Vec::new(),
            pending_sim_events: crate::sim_events::SimEventQueue::new(),
            industry_tile_dirty: Vec::new(),
            landscape_tile_dirty: Vec::new(),
            tile_loop_visited: Vec::new(),
            active_house_lifts: HashSet::new(),
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
            signal_action5_newgrf_sprites: Vec::new(),
            foundation_newgrf_sprites: Vec::new(),
            oneway_newgrf_sprites: Vec::new(),
            roadstop_action5_newgrf_sprites: Vec::new(),
            openttd_gui_newgrf_sprites: Vec::new(),
            airport_preview_newgrf_sprites: Vec::new(),
            bridge_decks_newgrf_sprites: Vec::new(),
            canal_action5_newgrf_sprites: Vec::new(),
            twocc_action5_newgrf_sprites: Vec::new(),
            tramway_action5_newgrf_sprites: Vec::new(),
            rail_signal_newgrf: Vec::new(),
            rail_type_overlay_newgrf: Vec::new(),
            rail_type_underlay_newgrf: Vec::new(),
            rail_type_props: crate::rail_type::RailTypeRuntimeProps::defaults(),
            rail_type_badges: std::array::from_fn(|_| Vec::new()),
            rail_type_max_speed: [0; 4],
            station_flows: crate::flow_stat::StationFlows::default(),
            station_flow_rebuilds: 0,
            command_recorder: None,
            newgrf_diagnostics: Vec::new(),
            sound_overrides: [None; crate::sound_id::SOUND_COUNT],
            pending_newgrf_sounds: Vec::new(),
        }
    }

    /// Inicia el delta visual de un tick de simulación.
    ///
    /// Las listas de señales y reservas son consumidas por el cliente después
    /// de `GameState::step`; por eso se limpian al comienzo del tick siguiente,
    /// no al terminar el actual. `tile_loop_visited` y `signal_globset` no se
    /// tocan aquí: el primero se consume en `AnimateAnimatedTiles` del próximo
    /// tick y el segundo puede contener trabajo pendiente de señales.
    pub fn begin_tick_visual_delta(&mut self) {
        self.signal_tile_dirty.clear();
        self.reservation_tile_dirty.clear();
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
        self.pending_newgrf_sounds.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::SimulationRuntime;
    use crate::map::{Map, TileCoord};
    use crate::rail_signals::SignalGlobEntry;

    #[test]
    fn tick_visual_delta_preserves_cross_tick_work_and_clears_render_deltas() {
        let mut runtime = SimulationRuntime::new();
        let coord = TileCoord::new(3, 4);
        let map = Map::new_flat(8, 8, 0);
        let Some(tile) = map.get(coord) else {
            panic!("la tesela de prueba debe pertenecer al mapa");
        };
        runtime.tile_loop_visited.push((coord, tile));
        runtime
            .signal_globset
            .insert(SignalGlobEntry::any_dir(coord));
        runtime.signal_tile_dirty.push(coord);
        runtime.reservation_tile_dirty.push(coord);

        runtime.begin_tick_visual_delta();

        assert_eq!(runtime.tile_loop_visited.len(), 1);
        assert!(
            runtime
                .signal_globset
                .contains(&SignalGlobEntry::any_dir(coord))
        );
        assert!(runtime.signal_tile_dirty.is_empty());
        assert!(runtime.reservation_tile_dirty.is_empty());
    }
}
