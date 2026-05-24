use crate::industry::Industry;
use crate::map::Map;
use crate::station::Station;
use crate::tick::GameTick;
use crate::tnbp_decode::JgrTunnelRecord;
use crate::vehicle::Vehicle;

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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CompanyEconomy {
    pub money: i64,
    pub loan: i64,
}

impl Default for CompanyEconomy {
    fn default() -> Self {
        Self {
            money: 100_000,
            loan: 0,
        }
    }
}

pub const ROAD_BUILD_COST: i64 = 10;
pub const RAIL_BUILD_COST: i64 = 25;
pub const STATION_BUILD_COST: i64 = 200;
pub const DEPOT_BUILD_COST: i64 = 150;
pub const TUNNEL_BUILD_COST_PER_TILE: i64 = 90;
pub const BRIDGE_BUILD_COST_PER_TILE: i64 = 70;
pub const CLEAR_TILE_COST: i64 = 5;

/// Ingreso por unidad de cargo entregada en estación (`sim_step::unload_vehicles`).
///
/// Es la palanca principal de dificultad económica frente a costes de construcción;
/// conviene ajustarla en PRs de balance dedicados para poder revisar el diff aislado.
pub const CARGO_DELIVERY_PAYMENT: i64 = 12;

/// Estado global mínimo del mundo simulado.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GameState {
    pub map: Map,
    pub tick: GameTick,
    pub industries: Vec<Industry>,
    pub vehicles: Vec<Vehicle>,
    pub stations: Vec<Station>,
    pub stats: SimStats,
    #[serde(default)]
    pub economy: CompanyEconomy,
    /// Túneles JGR decodificados desde footer `TNBP` del `.ottdmap` (vacío si no hay o no aplica).
    #[serde(default)]
    pub jgr_tunnels_from_footer: Vec<JgrTunnelRecord>,
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
            stats: SimStats::default(),
            economy: CompanyEconomy::default(),
            jgr_tunnels_from_footer: Vec::new(),
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
            stats: SimStats::default(),
            economy: CompanyEconomy::default(),
            jgr_tunnels_from_footer: Vec::new(),
        }
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
}
