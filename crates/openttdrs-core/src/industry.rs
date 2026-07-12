use crate::CargoType;
use crate::Climate;
use crate::entity_history::IndustryHistory;
use crate::map::TileCoord;
use crate::station::{self, STATION_COVERAGE_RADIUS, Station, StopKind};

/// Ticks entre cada ciclo de producción (equivale a `INDUSTRY_PRODUCE_TICKS` del upstream).
pub const INDUSTRY_PRODUCE_TICKS: u64 = 256;

/// Unidades producidas por ciclo.
pub const INDUSTRY_PRODUCE_AMOUNT: u32 = 8;

/// Insumos por ciclo de fábrica (`CargoType::Goods`).
pub const FACTORY_WOOD_INPUT: u32 = 4;
pub const FACTORY_COAL_INPUT: u32 = 2;

/// Capacidad máxima de stock por defecto.
pub const INDUSTRY_STOCK_CAPACITY: u32 = 500;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum IndustryKind {
    CoalMine,
    Forest,
    /// Extracción liviana (pozos de petróleo, etc.): mismo ritmo de stock que mina.
    OilWell,
    /// Procesamiento: produce la mitad de frecuencia que mina/bosque.
    Factory,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum IndustrySpec {
    CoalMine,
    IronOreMine,
    CopperOreMine,
    GoldMine,
    Forest,
    Farm,
    OilWells,
    OilRefinery,
    Factory,
    Sawmill,
    /// Plantación de algodón de azúcar (Toyland).
    CottonCandy,
    /// Fábrica de caramelos (Toyland).
    CandyFactory,
    /// Granja de baterías (Toyland).
    BatteryFarm,
    /// Pozo de cola (Toyland).
    ColaWells,
    /// Fábrica de juguetes (Toyland).
    ToyFactory,
    /// Fuente de plástico (Toyland).
    PlasticFountain,
    /// Fábrica de bebidas gaseosas (Toyland).
    FizzyDrinkFactory,
    /// Generador de burbujas (Toyland).
    BubbleGenerator,
    /// Cantera de toffee (Toyland).
    ToffeeQuarry,
    /// Mina de azúcar (Toyland).
    SugarMine,
}

impl IndustrySpec {
    /// Industrias colocables en este clima (`LandscapeType` en `OpenTTD`).
    #[must_use]
    pub fn specs_for_climate(climate: Climate) -> &'static [IndustrySpec] {
        match climate {
            Climate::Temperate => &[
                Self::CoalMine,
                Self::Forest,
                Self::Sawmill,
                Self::Factory,
                Self::Farm,
                Self::IronOreMine,
            ],
            Climate::SubArctic => &[
                Self::CoalMine,
                Self::Forest,
                Self::Sawmill,
                Self::Factory,
                Self::GoldMine,
                Self::IronOreMine,
            ],
            Climate::SubTropical => &[
                Self::OilWells,
                Self::OilRefinery,
                Self::Farm,
                Self::Factory,
                Self::CopperOreMine,
            ],
            Climate::Toyland => &[
                Self::CottonCandy,
                Self::CandyFactory,
                Self::BatteryFarm,
                Self::ColaWells,
                Self::ToyFactory,
                Self::PlasticFountain,
                Self::FizzyDrinkFactory,
                Self::BubbleGenerator,
                Self::ToffeeQuarry,
                Self::SugarMine,
            ],
        }
    }

    #[must_use]
    pub fn available_in(self, climate: Climate) -> bool {
        Self::specs_for_climate(climate).contains(&self)
    }

    #[must_use]
    pub const fn kind(self) -> IndustryKind {
        match self {
            Self::CoalMine
            | Self::IronOreMine
            | Self::CopperOreMine
            | Self::GoldMine
            | Self::BatteryFarm
            | Self::PlasticFountain
            | Self::SugarMine
            | Self::ToffeeQuarry => IndustryKind::CoalMine,
            Self::Forest | Self::Farm | Self::CottonCandy | Self::BubbleGenerator => {
                IndustryKind::Forest
            }
            Self::OilWells | Self::OilRefinery | Self::ColaWells => IndustryKind::OilWell,
            Self::Factory
            | Self::Sawmill
            | Self::CandyFactory
            | Self::ToyFactory
            | Self::FizzyDrinkFactory => IndustryKind::Factory,
        }
    }

    #[must_use]
    pub const fn output_cargo(self) -> CargoType {
        match self {
            Self::Forest
            | Self::Farm
            | Self::Sawmill
            | Self::CottonCandy
            | Self::BubbleGenerator => CargoType::Wood,
            Self::OilWells | Self::OilRefinery | Self::ColaWells => CargoType::Oil,
            Self::Factory | Self::CandyFactory | Self::ToyFactory | Self::FizzyDrinkFactory => {
                CargoType::Goods
            }
            Self::CoalMine
            | Self::IronOreMine
            | Self::CopperOreMine
            | Self::GoldMine
            | Self::BatteryFarm
            | Self::PlasticFountain
            | Self::SugarMine
            | Self::ToffeeQuarry => CargoType::Coal,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Industry {
    pub pos: TileCoord,
    #[serde(default = "default_industry_tiles")]
    pub tiles: Vec<TileCoord>,
    #[serde(default)]
    pub spec: Option<IndustrySpec>,
    pub kind: IndustryKind,
    pub stock: u32,
    pub capacity: u32,
    /// Color aleatorio de industria (`Colours` 0–15) para edificios con paleta.
    #[serde(default)]
    pub random_colour: u8,
    /// `IndustryID` de mapa (`m2`); 0 = desconocido / legacy.
    #[serde(default)]
    pub instance_id: u8,
    /// Unidades producidas acumuladas (para deltas mensuales).
    #[serde(default)]
    pub produced_total: u64,
    /// Unidades cargadas desde esta industria (para deltas mensuales).
    #[serde(default)]
    pub transported_total: u64,
    /// Series mensuales (stock / producido / transportado).
    #[serde(default)]
    pub history: IndustryHistory,
}

fn default_industry_tiles() -> Vec<TileCoord> {
    Vec::new()
}

impl Default for Industry {
    fn default() -> Self {
        Self::new(TileCoord::new(0, 0), IndustryKind::CoalMine)
    }
}

#[inline]
#[must_use]
pub const fn industry_produce_period_ticks(kind: IndustryKind) -> u64 {
    match kind {
        IndustryKind::Factory => INDUSTRY_PRODUCE_TICKS * 2,
        IndustryKind::CoalMine | IndustryKind::Forest | IndustryKind::OilWell => {
            INDUSTRY_PRODUCE_TICKS
        }
    }
}

impl Industry {
    #[must_use]
    pub fn new(pos: TileCoord, kind: IndustryKind) -> Self {
        Self {
            pos,
            tiles: vec![pos],
            spec: None,
            kind,
            stock: 0,
            capacity: INDUSTRY_STOCK_CAPACITY,
            random_colour: 0,
            instance_id: 0,
            produced_total: 0,
            transported_total: 0,
            history: IndustryHistory::default(),
        }
    }

    #[must_use]
    pub fn with_tiles(pos: TileCoord, kind: IndustryKind, tiles: Vec<TileCoord>) -> Self {
        Self {
            pos,
            tiles,
            spec: None,
            kind,
            stock: 0,
            capacity: INDUSTRY_STOCK_CAPACITY,
            random_colour: 0,
            instance_id: 0,
            produced_total: 0,
            transported_total: 0,
            history: IndustryHistory::default(),
        }
    }

    #[must_use]
    pub fn with_tiles_spec(
        pos: TileCoord,
        kind: IndustryKind,
        spec: IndustrySpec,
        tiles: Vec<TileCoord>,
        random_colour: u8,
    ) -> Self {
        Self {
            pos,
            tiles,
            spec: Some(spec),
            kind,
            stock: 0,
            capacity: INDUSTRY_STOCK_CAPACITY,
            random_colour,
            instance_id: 0,
            produced_total: 0,
            transported_total: 0,
            history: IndustryHistory::default(),
        }
    }

    /// Asigna el `IndustryID` de mapa (`m2`).
    #[must_use]
    pub fn with_instance_id(mut self, instance_id: u8) -> Self {
        self.instance_id = instance_id;
        self
    }

    #[must_use]
    pub fn contains_tile(&self, c: TileCoord) -> bool {
        self.pos == c || self.tiles.contains(&c)
    }

    /// Produce cargo primario si el tick cae en el periodo (minas, bosques, pozos, aserradero…).
    pub fn produce(&mut self, tick: u64) {
        if self.requires_station_inputs() {
            return;
        }
        let period = industry_produce_period_ticks(self.kind);
        if tick > 0 && tick.is_multiple_of(period) {
            self.stock = self
                .stock
                .saturating_add(INDUSTRY_PRODUCE_AMOUNT)
                .min(self.capacity);
        }
    }

    /// Fábricas: consumen madera/carbón en estaciones de carga dentro de cobertura y producen goods.
    ///
    /// Devuelve `true` si hubo un ciclo de procesamiento en este tick.
    pub fn produce_from_nearby_stations(&mut self, stations: &mut [Station], tick: u64) -> bool {
        if !self.requires_station_inputs() {
            return false;
        }
        let period = industry_produce_period_ticks(self.kind);
        if tick == 0 || !tick.is_multiple_of(period) || self.stock >= self.capacity {
            return false;
        }

        let requirements = self.station_input_requirements();
        let station_indices = covering_freight_station_indices(self, stations);
        if station_indices.is_empty() {
            return false;
        }

        for &idx in &station_indices {
            stations[idx].ensure_packets_from_stock();
        }
        for &(cargo, amount) in requirements {
            let available: u32 = station_indices
                .iter()
                .map(|&idx| stations[idx].cargo_stock.get(cargo))
                .sum();
            if available < amount {
                return false;
            }
        }

        for &(cargo, amount) in requirements {
            let mut remaining = amount;
            for &idx in &station_indices {
                if remaining == 0 {
                    break;
                }
                let take = stations[idx].cargo_stock.get(cargo).min(remaining);
                if take > 0 {
                    let _ = stations[idx].take_waiting_cargo(cargo, take);
                    remaining -= take;
                }
            }
            debug_assert_eq!(remaining, 0);
        }

        self.stock = self
            .stock
            .saturating_add(INDUSTRY_PRODUCE_AMOUNT)
            .min(self.capacity);
        true
    }

    /// Solo industrias que transforman cargo entregado en estaciones (p. ej. fábrica → goods).
    #[must_use]
    pub fn requires_station_inputs(&self) -> bool {
        self.output_cargo() == CargoType::Goods
    }

    #[must_use]
    pub fn station_input_requirements(&self) -> &'static [(CargoType, u32)] {
        if self.output_cargo() == CargoType::Goods {
            &[
                (CargoType::Wood, FACTORY_WOOD_INPUT),
                (CargoType::Coal, FACTORY_COAL_INPUT),
            ]
        } else {
            &[]
        }
    }

    #[must_use]
    pub fn output_cargo(&self) -> CargoType {
        if let Some(spec) = self.spec {
            return spec.output_cargo();
        }
        match self.kind {
            IndustryKind::CoalMine => CargoType::Coal,
            IndustryKind::Forest => CargoType::Wood,
            IndustryKind::OilWell => CargoType::Oil,
            IndustryKind::Factory => CargoType::Goods,
        }
    }
}

fn covering_freight_station_indices(industry: &Industry, stations: &[Station]) -> Vec<usize> {
    stations
        .iter()
        .enumerate()
        .filter(|(_, station)| {
            matches!(
                station.stop_kind,
                StopKind::TruckStop | StopKind::RailStation
            ) && station::industry_in_station_coverage(
                industry,
                station.pos,
                STATION_COVERAGE_RADIUS,
            )
        })
        .map(|(idx, _)| idx)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::station::StopKind;

    #[test]
    fn toyland_specs_exclude_temperate_mines() {
        use crate::Climate;

        assert!(!IndustrySpec::CoalMine.available_in(Climate::Toyland));
        assert!(IndustrySpec::FizzyDrinkFactory.available_in(Climate::Toyland));
        assert!(!IndustrySpec::FizzyDrinkFactory.available_in(Climate::Temperate));
        assert!(IndustrySpec::CoalMine.available_in(Climate::Temperate));
    }

    #[test]
    fn factory_without_station_inputs_does_not_auto_produce() {
        let mut fact = Industry::new(TileCoord::new(0, 0), IndustryKind::Factory);
        fact.produce(512);
        assert_eq!(fact.stock, 0);
    }

    #[test]
    fn sawmill_still_auto_produces_wood() {
        let mut saw = Industry::with_tiles_spec(
            TileCoord::new(0, 0),
            IndustryKind::Factory,
            IndustrySpec::Sawmill,
            vec![TileCoord::new(0, 0)],
            0,
        );
        saw.produce(512);
        assert_eq!(saw.stock, INDUSTRY_PRODUCE_AMOUNT);
    }

    #[test]
    fn factory_consumes_wood_and_coal_from_nearby_truck_stop() {
        let fact_pos = TileCoord::new(4, 4);
        let stop_pos = TileCoord::new(5, 4);
        let mut fact = Industry::with_tiles_spec(
            fact_pos,
            IndustryKind::Factory,
            IndustrySpec::Factory,
            vec![fact_pos],
            0,
        );
        let mut stations = vec![Station::new_with_kind(stop_pos, StopKind::TruckStop)];
        stations[0].cargo_stock.wood = 10;
        stations[0].cargo_stock.coal = 10;

        assert!(fact.produce_from_nearby_stations(&mut stations, 512));
        assert_eq!(fact.stock, INDUSTRY_PRODUCE_AMOUNT);
        assert_eq!(stations[0].cargo_stock.wood, 10 - FACTORY_WOOD_INPUT);
        assert_eq!(stations[0].cargo_stock.coal, 10 - FACTORY_COAL_INPUT);
    }

    #[test]
    fn factory_skips_when_inputs_missing() {
        let fact_pos = TileCoord::new(0, 0);
        let mut fact = Industry::new(fact_pos, IndustryKind::Factory);
        let mut stations = vec![Station::new_with_kind(
            TileCoord::new(1, 0),
            StopKind::TruckStop,
        )];
        stations[0].cargo_stock.wood = FACTORY_WOOD_INPUT;
        assert!(!fact.produce_from_nearby_stations(&mut stations, 512));
        assert_eq!(fact.stock, 0);
    }
}
