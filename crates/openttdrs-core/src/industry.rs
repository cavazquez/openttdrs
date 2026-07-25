use crate::CargoType;
use crate::Climate;
use crate::cargodist::parity::Randomizer;
use crate::entity_history::IndustryHistory;
use crate::map::TileCoord;
use crate::station::{self, Station, StopKind};

/// Ticks entre cada ciclo de producción (equivale a `INDUSTRY_PRODUCE_TICKS` del upstream).
pub const INDUSTRY_PRODUCE_TICKS: u64 = 256;

/// Unidades de salida de una fábrica de goods por ciclo a `prod_level` por defecto (legacy).
///
/// Las procesadoras usan [`IndustrySpec::processing_inputs`] y la fórmula
/// `out += in * multiplier / 256`.
pub const INDUSTRY_PRODUCE_AMOUNT: u32 = 8;

/// Insumos por ciclo de fábrica temperate (`IndustrySpec::Factory`).
pub const FACTORY_WOOD_INPUT: u32 = 4;
pub const FACTORY_COAL_INPUT: u32 = 2;

/// Entrada de procesamiento con multiplicador hacia la salida (`/256`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IndustryProcessingInput {
    pub cargo: CargoType,
    /// Unidades consumidas por ciclo a [`PRODLEVEL_DEFAULT`].
    pub batch: u32,
    /// Multiplicador hacia la salida (`economy.cpp:1156`).
    pub multiplier: u16,
}

/// Capacidad máxima de stock por defecto.
pub const INDUSTRY_STOCK_CAPACITY: u32 = 500;

/// Industria marcada para cierre (`PRODLEVEL_CLOSURE`, `industry.h:35`).
pub const PRODLEVEL_CLOSURE: u8 = 0x00;
/// Mínimo de producción antes de cerrar al bajar otra vez (`PRODLEVEL_MINIMUM`).
pub const PRODLEVEL_MINIMUM: u8 = 0x04;
/// Nivel de partida (`PRODLEVEL_DEFAULT`).
pub const PRODLEVEL_DEFAULT: u8 = 0x10;
/// Tope de producción (`PRODLEVEL_MAXIMUM`).
pub const PRODLEVEL_MAXIMUM: u8 = 0x80;

/// ≥ 60 % transportado el mes pasado (`PERCENT_TRANSPORTED_60`).
pub const PERCENT_TRANSPORTED_60: u8 = 153;
/// ≥ 80 % transportado (`PERCENT_TRANSPORTED_80`); solo modo smooth, reservado.
#[allow(dead_code)]
pub const PERCENT_TRANSPORTED_80: u8 = 204;

/// Tipo de vida económica (`IndustryLifeType`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndustryLifeType {
    /// Mina / pozo: sube y baja con el transporte.
    Extractive,
    /// Bosque / granja: igual que extractive en el modo original.
    Organic,
    /// Procesadora: no cambia `prod_level` por transporte; cierra por abandono (P1.5+).
    Processing,
    /// Sumidero (central térmica…): sin cambios.
    BlackHole,
}

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
    /// Acería temperate (hierro + carbón → acero).
    SteelMill,
    /// Banco temperate (objetos de valor).
    Bank,
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
                Self::SteelMill,
                Self::Bank,
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
            | Self::SteelMill
            | Self::Bank
            | Self::CandyFactory
            | Self::ToyFactory
            | Self::FizzyDrinkFactory => IndustryKind::Factory,
        }
    }

    #[must_use]
    pub const fn output_cargo(self) -> CargoType {
        match self {
            Self::Forest | Self::CottonCandy | Self::BubbleGenerator => CargoType::Wood,
            Self::Sawmill
            | Self::Factory
            | Self::OilRefinery
            | Self::CandyFactory
            | Self::ToyFactory
            | Self::FizzyDrinkFactory => CargoType::Goods,
            Self::Farm => CargoType::Grain,
            Self::OilWells | Self::ColaWells => CargoType::Oil,
            Self::SteelMill => CargoType::Steel,
            Self::Bank => CargoType::Valuables,
            Self::IronOreMine | Self::CopperOreMine => CargoType::IronOre,
            Self::CoalMine
            | Self::GoldMine
            | Self::BatteryFarm
            | Self::PlasticFountain
            | Self::SugarMine
            | Self::ToffeeQuarry => CargoType::Coal,
        }
    }

    /// Insumos y multiplicadores de procesadoras temperate (MVP P1.5).
    #[must_use]
    pub const fn processing_inputs(self) -> &'static [IndustryProcessingInput] {
        match self {
            Self::Sawmill => &[IndustryProcessingInput {
                cargo: CargoType::Wood,
                batch: 8,
                multiplier: 256,
            }],
            Self::OilRefinery => &[IndustryProcessingInput {
                cargo: CargoType::Oil,
                batch: 8,
                multiplier: 256,
            }],
            Self::SteelMill => &[
                IndustryProcessingInput {
                    cargo: CargoType::IronOre,
                    batch: 8,
                    multiplier: 256,
                },
                IndustryProcessingInput {
                    cargo: CargoType::Coal,
                    batch: 4,
                    multiplier: 256,
                },
            ],
            Self::Factory => &[
                IndustryProcessingInput {
                    cargo: CargoType::Wood,
                    batch: FACTORY_WOOD_INPUT,
                    multiplier: 256,
                },
                IndustryProcessingInput {
                    cargo: CargoType::Coal,
                    batch: FACTORY_COAL_INPUT,
                    multiplier: 256,
                },
            ],
            _ => &[],
        }
    }

    #[must_use]
    pub const fn is_processor(self) -> bool {
        !self.processing_inputs().is_empty()
    }

    /// `production_rate[0]` del spec vanilla (`build_industry.h`).
    ///
    /// Las procesadoras tienen 0: no auto-producen; transforman insumos (P1.5) o,
    /// en el caso de goods del port, el ciclo de fábrica cercano.
    #[must_use]
    pub const fn production_rate(self) -> u8 {
        match self {
            Self::CoalMine => 15,
            Self::Forest | Self::CottonCandy | Self::BubbleGenerator => 13,
            Self::OilWells | Self::ColaWells => 12,
            Self::Farm | Self::IronOreMine | Self::CopperOreMine | Self::ToffeeQuarry => 10,
            Self::GoldMine => 7,
            Self::Bank => 6,
            Self::BatteryFarm | Self::SugarMine => 11,
            Self::PlasticFountain => 14,
            Self::Factory
            | Self::Sawmill
            | Self::SteelMill
            | Self::OilRefinery
            | Self::CandyFactory
            | Self::ToyFactory
            | Self::FizzyDrinkFactory => 0,
        }
    }

    /// Vida económica del spec (`IndustryLifeType` en `build_industry.h`).
    #[must_use]
    pub const fn life_type(self) -> IndustryLifeType {
        match self {
            Self::CoalMine
            | Self::IronOreMine
            | Self::CopperOreMine
            | Self::GoldMine
            | Self::OilWells
            | Self::ColaWells
            | Self::PlasticFountain
            | Self::SugarMine
            | Self::ToffeeQuarry
            | Self::BubbleGenerator => IndustryLifeType::Extractive,
            Self::Forest | Self::Farm | Self::CottonCandy | Self::BatteryFarm => {
                IndustryLifeType::Organic
            }
            Self::Factory
            | Self::Sawmill
            | Self::SteelMill
            | Self::OilRefinery
            | Self::Bank
            | Self::CandyFactory
            | Self::ToyFactory
            | Self::FizzyDrinkFactory => IndustryLifeType::Processing,
        }
    }

    /// Pozos de petróleo temperate solo bajan (`IndustryBehaviour::DontIncrProd`).
    #[must_use]
    pub const fn only_decreases_production(self) -> bool {
        matches!(self, Self::OilWells)
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
    /// Fase de producción propia (`Industry::counter`, `industry_cmd.cpp:1807`).
    ///
    /// `OpenTTD` la siembra con 12 bits aleatorios al fundar y la decrementa cada tick,
    /// de modo que dos industrias vecinas no producen en el mismo tick.
    #[serde(default)]
    pub counter: u16,
    /// Nivel de producción (`Industry::prod_level`). Escala el `production_rate` del spec.
    ///
    /// `0` = marcada para cierre (se borra el mes siguiente). Arranca en
    /// [`PRODLEVEL_DEFAULT`].
    #[serde(default = "default_prod_level")]
    pub prod_level: u8,
}

const fn default_prod_level() -> u8 {
    PRODLEVEL_DEFAULT
}

/// Máscara de la fase de producción (`GB(r, 4, 12)`).
pub const INDUSTRY_COUNTER_MASK: u16 = 0x0FFF;

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
            counter: 0,
            prod_level: PRODLEVEL_DEFAULT,
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
            counter: 0,
            prod_level: PRODLEVEL_DEFAULT,
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
            counter: 0,
            prod_level: PRODLEVEL_DEFAULT,
        }
    }

    /// Asigna el `IndustryID` de mapa (`m2`).
    #[must_use]
    pub fn with_instance_id(mut self, instance_id: u8) -> Self {
        self.instance_id = instance_id;
        self
    }

    /// Asigna `Industry.random_colour` (`Colours` 0–15).
    #[must_use]
    pub fn with_random_colour(mut self, random_colour: u8) -> Self {
        self.random_colour = random_colour % 16;
        self
    }

    /// Siembra la fase de producción con 12 bits (`i->counter = GB(r, 4, 12)`).
    #[must_use]
    pub const fn with_counter(mut self, counter: u16) -> Self {
        self.counter = counter & INDUSTRY_COUNTER_MASK;
        self
    }

    /// ¿Este tick cae en el ciclo de producción de esta industria?
    ///
    /// `OpenTTD` decrementa `counter` cada tick y produce cuando es múltiplo de
    /// `INDUSTRY_PRODUCE_TICKS`; el desfase equivalente sobre el tick global es
    /// sumar la fase, que reparte las industrias entre ticks distintos.
    #[must_use]
    pub const fn produces_on_tick(&self, tick: u64) -> bool {
        if tick == 0 {
            return false;
        }
        let period = industry_produce_period_ticks(self.kind);
        (tick + self.counter as u64).is_multiple_of(period)
    }

    #[must_use]
    pub fn contains_tile(&self, c: TileCoord) -> bool {
        self.pos == c || self.tiles.contains(&c)
    }

    /// Rate base del spec o, sin spec, el del kind.
    #[must_use]
    pub fn production_rate(&self) -> u8 {
        if let Some(spec) = self.spec {
            return spec.production_rate();
        }
        match self.kind {
            IndustryKind::CoalMine => 15,
            IndustryKind::Forest => 13,
            IndustryKind::OilWell => 12,
            IndustryKind::Factory => 0,
        }
    }

    #[must_use]
    pub fn life_type(&self) -> IndustryLifeType {
        if let Some(spec) = self.spec {
            return spec.life_type();
        }
        match self.kind {
            IndustryKind::CoalMine | IndustryKind::OilWell => IndustryLifeType::Extractive,
            IndustryKind::Forest => IndustryLifeType::Organic,
            IndustryKind::Factory => IndustryLifeType::Processing,
        }
    }

    /// Unidades por ciclo de producción (`CeilDiv(rate * prod_level, PRODLEVEL_DEFAULT)`).
    #[must_use]
    pub fn produce_amount(&self) -> u32 {
        if self.prod_level == PRODLEVEL_CLOSURE {
            return 0;
        }
        let rate = u32::from(self.production_rate());
        if rate == 0 {
            return 0;
        }
        (rate * u32::from(self.prod_level))
            .div_ceil(u32::from(PRODLEVEL_DEFAULT))
            .min(255)
    }

    /// Salida de procesadora escalada por `prod_level` (legacy; preferir [`Self::processing_output_amount`]).
    #[must_use]
    pub fn factory_output_amount(&self) -> u32 {
        self.processing_output_amount()
    }

    /// Unidades de salida tras consumir los lotes de [`Self::processing_inputs`].
    #[must_use]
    pub fn processing_output_amount(&self) -> u32 {
        if self.prod_level == PRODLEVEL_CLOSURE {
            return 0;
        }
        let inputs = self.processing_inputs();
        if inputs.is_empty() {
            return 0;
        }
        inputs
            .iter()
            .map(|input| {
                let consumed = scaled_processing_batch(input.batch, self.prod_level);
                consumed.saturating_mul(u32::from(input.multiplier)) / 256
            })
            .sum()
    }

    fn processing_inputs(&self) -> &'static [IndustryProcessingInput] {
        if let Some(spec) = self.spec {
            return spec.processing_inputs();
        }
        if self.kind == IndustryKind::Factory {
            return IndustrySpec::Factory.processing_inputs();
        }
        &[]
    }

    #[must_use]
    pub const fn is_closing(&self) -> bool {
        self.prod_level == PRODLEVEL_CLOSURE
    }

    /// Porcentaje transportado el mes pasado en unidades 0–255 (`PctTransported`).
    #[must_use]
    pub fn last_month_pct_transported(&self) -> u8 {
        let Some(sample) = self.history.samples.last() else {
            return 0;
        };
        if sample.produced == 0 {
            return 0;
        }
        let pct = (u64::from(sample.transported) * 256) / u64::from(sample.produced);
        u8::try_from(pct.min(255)).unwrap_or(255)
    }

    /// Produce cargo primario si el tick cae en el periodo (minas, bosques, pozos…).
    pub fn produce(&mut self, tick: u64) {
        if self.requires_station_inputs() || self.is_closing() {
            return;
        }
        let amount = self.produce_amount();
        if amount == 0 {
            return;
        }
        if self.produces_on_tick(tick) {
            self.stock = self.stock.saturating_add(amount).min(self.capacity);
        }
    }

    /// Procesadoras: consumen insumos en estaciones de carga dentro de cobertura.
    ///
    /// Devuelve `true` si hubo un ciclo de procesamiento en este tick.
    pub fn produce_from_nearby_stations(&mut self, stations: &mut [Station], tick: u64) -> bool {
        let inputs = self.processing_inputs();
        if inputs.is_empty() || self.is_closing() {
            return false;
        }
        if !self.produces_on_tick(tick) || self.stock >= self.capacity {
            return false;
        }

        let requirements: Vec<(CargoType, u32)> = inputs
            .iter()
            .map(|input| {
                (
                    input.cargo,
                    scaled_processing_batch(input.batch, self.prod_level),
                )
            })
            .collect();
        let station_indices = covering_freight_station_indices(self, stations);
        if station_indices.is_empty() {
            return false;
        }

        for &idx in &station_indices {
            stations[idx].ensure_packets_from_stock();
        }
        for &(cargo, amount) in &requirements {
            let available: u32 = station_indices
                .iter()
                .map(|&idx| stations[idx].cargo_stock.get(cargo))
                .sum();
            if available < amount {
                return false;
            }
        }

        for &(cargo, amount) in &requirements {
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

        let output = self.processing_output_amount();
        if output == 0 {
            return false;
        }
        self.stock = self.stock.saturating_add(output).min(self.capacity);
        true
    }

    /// Industrias que transforman cargo entregado en estaciones cercanas.
    #[must_use]
    pub fn requires_station_inputs(&self) -> bool {
        !self.processing_inputs().is_empty()
    }

    /// Insumos de estación (cargo + lote a `prod_level` por defecto) para UI y tests.
    #[must_use]
    pub fn station_input_requirements(&self) -> &'static [(CargoType, u32)] {
        match self.spec.unwrap_or(match self.kind {
            IndustryKind::Factory => IndustrySpec::Factory,
            IndustryKind::CoalMine => IndustrySpec::CoalMine,
            IndustryKind::Forest => IndustrySpec::Forest,
            IndustryKind::OilWell => IndustrySpec::OilWells,
        }) {
            IndustrySpec::Factory => &[
                (CargoType::Wood, FACTORY_WOOD_INPUT),
                (CargoType::Coal, FACTORY_COAL_INPUT),
            ],
            IndustrySpec::Sawmill => &[(CargoType::Wood, 8)],
            IndustrySpec::OilRefinery => &[(CargoType::Oil, 8)],
            IndustrySpec::SteelMill => &[(CargoType::IronOre, 8), (CargoType::Coal, 4)],
            _ => &[],
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

fn scaled_processing_batch(batch: u32, prod_level: u8) -> u32 {
    (batch * u32::from(prod_level))
        .div_ceil(u32::from(PRODLEVEL_DEFAULT))
        .max(1)
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
                station::station_catchment_radius(station),
            )
        })
        .map(|(idx, _)| idx)
        .collect()
}

/// Estaciones en cobertura que pueden recibir la producción de esta industria.
fn covering_output_station_indices(industry: &Industry, stations: &[Station]) -> Vec<usize> {
    let cargo = industry.output_cargo();
    stations
        .iter()
        .enumerate()
        .filter(|(_, station)| {
            station.accepts_cargo(cargo)
                && station::industry_in_station_coverage(
                    industry,
                    station.pos,
                    station::station_catchment_radius(station),
                )
        })
        .map(|(idx, _)| idx)
        .collect()
}

/// Mueve stock de la industria a estaciones cercanas (`TransportIndustryGoods`).
///
/// Toma hasta 255 unidades del stock (como el `ClampTo<uint8_t>` del original) y las
/// reparte con [`station::move_goods_to_station`]. Lo que el rating no deja pasar se
/// pierde: ya no vuelve al stock de la industria.
///
/// Si ninguna estación puede recibir todavía (p. ej. `selectgoods` y nadie ha intentado
/// cargar), el stock se deja en la industria para la carga directa: destruirlo dejaría
/// las minas vacías hasta la primera visita.
///
/// Devuelve las unidades que acabaron en andenes.
pub fn transport_industry_goods(
    industry: &mut Industry,
    stations: &mut [Station],
    selectgoods: bool,
) -> u32 {
    if industry.stock == 0 {
        return 0;
    }
    let cargo = industry.output_cargo();
    let nearby = covering_output_station_indices(industry, stations);
    let eligible: Vec<usize> = nearby
        .into_iter()
        .filter(|&idx| station::can_move_goods_to_station(&stations[idx], cargo, selectgoods))
        .collect();
    if eligible.is_empty() {
        return 0;
    }
    let amount = industry.stock.min(255);
    // Se detrae todo lo intentado, no solo lo entregado: el rating decide cuánto se pierde.
    industry.stock = industry.stock.saturating_sub(amount);
    let moved = station::move_goods_to_station(
        stations,
        &eligible,
        cargo,
        amount,
        industry.pos,
        selectgoods,
        None,
    );
    if moved > 0 {
        industry.transported_total = industry.transported_total.saturating_add(u64::from(moved));
    }
    moved
}

/// Resultado de un cambio de producción (`ChangeIndustryProduction`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndustryProductionChange {
    None,
    Increased,
    Decreased,
    Closing,
}

/// Cambia el `prod_level` o marca cierre (`ChangeIndustryProduction`, modo original).
///
/// Con economía original los cambios ocurren en la llamada diaria (`monthly = false`);
/// la mensual solo sirve para borrar las ya marcadas. Las procesadoras no cambian aquí.
pub fn change_industry_production(
    industry: &mut Industry,
    monthly: bool,
    climate: Climate,
    rng: &mut Randomizer,
) -> IndustryProductionChange {
    // Modo original: la evaluación es diaria. La llamada mensual no hace nada.
    let original_economy = true;
    if monthly == original_economy {
        return IndustryProductionChange::None;
    }
    if industry.is_closing() {
        return IndustryProductionChange::None;
    }
    // Sin un mes cerrado no hay `LAST_MONTH` útil: no improvisar cambios el primer mes.
    if industry.history.samples.is_empty() {
        return IndustryProductionChange::None;
    }

    let life = industry.life_type();
    if !matches!(
        life,
        IndustryLifeType::Extractive | IndustryLifeType::Organic
    ) {
        return IndustryProductionChange::None;
    }

    let only_decrease = industry
        .spec
        .is_some_and(IndustrySpec::only_decreases_production)
        && climate == Climate::Temperate;

    // 1/3 de probabilidad de intentar un cambio (o siempre si solo baja).
    if !only_decrease && !chance16(rng, 1, 3) {
        return IndustryProductionChange::None;
    }

    let well_served = industry.last_month_pct_transported() > PERCENT_TRANSPORTED_60;
    // Si transportó bien XOR Chance16(1,3) → subir; si no → bajar.
    let increase = !only_decrease && well_served != chance16(rng, 1, 3);

    if increase {
        if industry.prod_level >= PRODLEVEL_MAXIMUM {
            return IndustryProductionChange::None;
        }
        industry.prod_level = (industry.prod_level.saturating_mul(2)).min(PRODLEVEL_MAXIMUM);
        IndustryProductionChange::Increased
    } else if industry.prod_level == PRODLEVEL_MINIMUM {
        industry.prod_level = PRODLEVEL_CLOSURE;
        IndustryProductionChange::Closing
    } else {
        industry.prod_level = (industry.prod_level / 2).max(PRODLEVEL_MINIMUM);
        IndustryProductionChange::Decreased
    }
}

/// `Chance16(a, b)`: probabilidad `a/b`.
fn chance16(rng: &mut Randomizer, a: u32, b: u32) -> bool {
    if b == 0 {
        return false;
    }
    rng.random_range(b) < a
}

/// Borra del mapa las industrias marcadas para cierre el mes pasado.
///
/// Devuelve las posiciones de las industrias eliminadas (para noticias).
pub fn remove_closed_industries(
    industries: &mut Vec<Industry>,
    map: &mut crate::map::Map,
) -> Vec<TileCoord> {
    let mut closed_at = Vec::new();
    industries.retain(|ind| {
        if !ind.is_closing() {
            return true;
        }
        closed_at.push(ind.pos);
        for &tile in &ind.tiles {
            let _ = map.set_kind(tile, crate::map::TileKind::Grass);
            let _ = map.set_m1(tile, 0);
            let _ = map.set_m2(tile, 0);
            let _ = map.set_mapt_m5(tile, 0, 0);
        }
        if ind.tiles.is_empty() {
            let _ = map.set_kind(ind.pos, crate::map::TileKind::Grass);
        }
        false
    });
    closed_at
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

    /// Cada industria lleva su propia fase (`i->counter`), así que dos minas
    /// fundadas a la vez no vuelcan su producción en el mismo tick.
    #[test]
    fn industries_produce_on_their_own_phase() {
        let mut plain = Industry::new(TileCoord::new(0, 0), IndustryKind::CoalMine);
        let mut shifted =
            Industry::new(TileCoord::new(1, 0), IndustryKind::CoalMine).with_counter(100);
        let expected = plain.produce_amount();

        plain.produce(INDUSTRY_PRODUCE_TICKS);
        shifted.produce(INDUSTRY_PRODUCE_TICKS);
        assert_eq!(plain.stock, expected);
        assert_eq!(shifted.stock, 0);

        shifted.produce(INDUSTRY_PRODUCE_TICKS - 100);
        assert_eq!(shifted.stock, expected);
    }

    #[test]
    fn counter_keeps_only_twelve_bits() {
        let ind = Industry::new(TileCoord::new(0, 0), IndustryKind::CoalMine).with_counter(0xFFFF);
        assert_eq!(ind.counter, INDUSTRY_COUNTER_MASK);
    }

    #[test]
    fn factory_without_station_inputs_does_not_auto_produce() {
        let mut fact = Industry::new(TileCoord::new(0, 0), IndustryKind::Factory);
        fact.produce(512);
        assert_eq!(fact.stock, 0);
    }

    /// El aserradero vanilla es procesadora (`production_rate = 0`): no auto-produce madera.
    #[test]
    fn sawmill_does_not_auto_produce() {
        let mut saw = Industry::with_tiles_spec(
            TileCoord::new(0, 0),
            IndustryKind::Factory,
            IndustrySpec::Sawmill,
            vec![TileCoord::new(0, 0)],
            0,
        );
        saw.produce(512);
        assert_eq!(saw.stock, 0);
        assert_eq!(saw.production_rate(), 0);
        assert_eq!(saw.life_type(), IndustryLifeType::Processing);
    }

    #[test]
    fn coal_mine_produces_fifteen_at_default_level() {
        let mine = Industry::new(TileCoord::new(0, 0), IndustryKind::CoalMine);
        assert_eq!(mine.prod_level, PRODLEVEL_DEFAULT);
        assert_eq!(mine.produce_amount(), 15);
    }

    #[test]
    fn doubling_prod_level_doubles_output() {
        let mut mine = Industry::new(TileCoord::new(0, 0), IndustryKind::CoalMine);
        mine.prod_level = PRODLEVEL_DEFAULT * 2;
        assert_eq!(mine.produce_amount(), 30);
    }

    /// Sin transporte, bajar una y otra vez desde el mínimo cierra la mina.
    #[test]
    fn poor_service_closes_mine_from_minimum() {
        let mut mine = Industry::new(TileCoord::new(0, 0), IndustryKind::CoalMine);
        mine.prod_level = PRODLEVEL_MINIMUM;
        mine.history.push_month(0, 100, 0); // 0 % transportado
        let mut rng = Randomizer::new(1);
        // Forzar el cambio: repetir hasta que baje (Chance16 puede saltar).
        let mut closed = false;
        for _ in 0..32 {
            let change = change_industry_production(&mut mine, false, Climate::Temperate, &mut rng);
            if change == IndustryProductionChange::Closing {
                closed = true;
                break;
            }
        }
        assert!(
            closed,
            "una mina al mínimo sin transporte tiene que acabar cerrando"
        );
        assert_eq!(mine.prod_level, PRODLEVEL_CLOSURE);
    }

    #[test]
    fn closed_industries_are_removed_next_month() {
        let mut map = crate::map::Map::new_flat(8, 8, 0);
        let pos = TileCoord::new(2, 2);
        let _ = map.set_kind(pos, crate::map::TileKind::Industry);
        let mut mine = Industry::new(pos, IndustryKind::CoalMine);
        mine.prod_level = PRODLEVEL_CLOSURE;
        let mut industries = vec![mine];
        let closed = remove_closed_industries(&mut industries, &mut map);
        assert_eq!(closed, vec![pos]);
        assert!(industries.is_empty());
        assert_eq!(map.get_kind(pos), Some(crate::map::TileKind::Grass));
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
        assert_eq!(fact.stock, fact.processing_output_amount());
        assert_eq!(stations[0].cargo_stock.wood, 10 - FACTORY_WOOD_INPUT);
        assert_eq!(stations[0].cargo_stock.coal, 10 - FACTORY_COAL_INPUT);
    }

    #[test]
    fn sawmill_consumes_wood_for_goods() {
        let pos = TileCoord::new(0, 0);
        let mut saw = Industry::with_tiles_spec(
            pos,
            IndustryKind::Factory,
            IndustrySpec::Sawmill,
            vec![pos],
            0,
        );
        let mut stations = vec![Station::new_with_kind(
            TileCoord::new(1, 0),
            StopKind::TruckStop,
        )];
        stations[0].cargo_stock.wood = 16;

        assert!(saw.produce_from_nearby_stations(&mut stations, 512));
        assert_eq!(saw.stock, 8);
        assert_eq!(saw.output_cargo(), CargoType::Goods);
        assert_eq!(stations[0].cargo_stock.wood, 8);
    }

    #[test]
    fn steel_mill_consumes_iron_and_coal_for_steel() {
        let pos = TileCoord::new(0, 0);
        let mut mill = Industry::with_tiles_spec(
            pos,
            IndustryKind::Factory,
            IndustrySpec::SteelMill,
            vec![pos],
            0,
        );
        let mut stations = vec![Station::new_with_kind(
            TileCoord::new(1, 0),
            StopKind::TruckStop,
        )];
        stations[0].cargo_stock.iron_ore = 16;
        stations[0].cargo_stock.coal = 16;

        assert!(mill.produce_from_nearby_stations(&mut stations, 512));
        assert_eq!(mill.stock, 12);
        assert_eq!(mill.output_cargo(), CargoType::Steel);
        assert_eq!(stations[0].cargo_stock.iron_ore, 8);
        assert_eq!(stations[0].cargo_stock.coal, 12);
    }

    #[test]
    fn oil_refinery_consumes_oil_for_goods() {
        let pos = TileCoord::new(0, 0);
        let mut refinery = Industry::with_tiles_spec(
            pos,
            IndustryKind::OilWell,
            IndustrySpec::OilRefinery,
            vec![pos],
            0,
        );
        let mut stations = vec![Station::new_with_kind(
            TileCoord::new(1, 0),
            StopKind::TruckStop,
        )];
        stations[0].cargo_stock.oil = 16;

        assert!(refinery.produce_from_nearby_stations(&mut stations, 512));
        assert_eq!(refinery.stock, 8);
        assert_eq!(refinery.output_cargo(), CargoType::Goods);
        assert_eq!(stations[0].cargo_stock.oil, 8);
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

    /// Dos estaciones sobre la misma mina se reparten el carbón según el rating:
    /// servir bien la parada ya no es cosmética, decide quién se lleva la producción.
    #[test]
    fn two_stations_compete_for_mine_output_by_rating() {
        let mine_pos = TileCoord::new(4, 4);
        let mut mine = Industry::new(mine_pos, IndustryKind::CoalMine);
        mine.stock = 255;

        let mut good = Station::new_with_kind(TileCoord::new(3, 4), StopKind::TruckStop);
        let mut bad = Station::new_with_kind(TileCoord::new(5, 4), StopKind::TruckStop);
        good.goods.get_mut(CargoType::Coal).last_speed = 1;
        bad.goods.get_mut(CargoType::Coal).last_speed = 1;
        good.goods.get_mut(CargoType::Coal).rating = 200;
        bad.goods.get_mut(CargoType::Coal).rating = 50;
        let mut stations = vec![good, bad];

        let moved = transport_industry_goods(&mut mine, &mut stations, true);
        assert!(moved > 0);
        assert_eq!(
            mine.stock, 0,
            "el intento detrae el stock aunque el rating recorte"
        );
        assert!(
            stations[0].cargo_stock.coal > stations[1].cargo_stock.coal,
            "buena {} vs mala {}",
            stations[0].cargo_stock.coal,
            stations[1].cargo_stock.coal
        );
    }

    /// Con selectgoods, una estación nunca visitada no se lleva nada: el stock se queda
    /// en la mina para la carga directa hasta que alguien intente cargar.
    #[test]
    fn unvisited_station_leaves_stock_on_industry() {
        let mut mine = Industry::new(TileCoord::new(0, 0), IndustryKind::CoalMine);
        mine.stock = 40;
        let mut stations = vec![Station::new_with_kind(
            TileCoord::new(1, 0),
            StopKind::TruckStop,
        )];
        assert_eq!(transport_industry_goods(&mut mine, &mut stations, true), 0);
        assert_eq!(mine.stock, 40);
        assert_eq!(stations[0].cargo_stock.coal, 0);
    }
}
