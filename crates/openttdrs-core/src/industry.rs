use crate::Climate;
use crate::cargodist::parity::Randomizer;
use crate::entity_history::IndustryHistory;
use crate::industry_spec::IndustrySpecDef;
use crate::map::TileCoord;
use crate::station::{self, Station, StopKind};
use crate::{ALL_CARGO_TYPES, CargoStock, CargoType};

/// Ticks entre cada ciclo de producción (equivale a `INDUSTRY_PRODUCE_TICKS` del upstream).
pub const INDUSTRY_PRODUCE_TICKS: u64 = 256;

/// Unidades de salida de una fábrica de goods por ciclo a `prod_level` por defecto (legacy).
///
/// Las procesadoras usan [`IndustrySpec::processing_inputs`] y la fórmula
/// `out += in * multiplier / 256`.
pub const INDUSTRY_PRODUCE_AMOUNT: u32 = 8;

/// Insumos por ciclo de fábrica temperate (`IndustrySpec::Factory`, `build_industry.h`).
pub const FACTORY_LIVESTOCK_INPUT: u32 = 8;
pub const FACTORY_GRAIN_INPUT: u32 = 8;
pub const FACTORY_STEEL_INPUT: u32 = 8;

/// Entrada de procesamiento con multiplicador hacia la salida (`/256`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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

/// Días desde el origen de `OpenTTD` (1920-01-01) hasta el año base que usa
/// este proyecto para iniciar una partida (1950-01-01).
///
/// `TimerGameCalendar::date` del modelo reducido empieza en cero en 1950,
/// mientras que `Industry::construction_date` y las variables `NewGRF` 0x46
/// usan la fecha absoluta del calendario nativo. La diferencia incluye los
/// siete años bisiestos de 1920..1948.
pub const OPENTTD_CALENDAR_DAYS_TILL_BASE_YEAR: u32 = 10_957;

/// Industria creada sin compañía fundadora (`INVALID_OWNER`).
pub const INDUSTRY_FOUNDER_INVALID: u8 = 0xFF;

/// `IndustryConstructionType::ICT_UNKNOWN`.
pub const INDUSTRY_CONSTRUCTION_UNKNOWN: u8 = 0;
/// Construcción normal durante una partida.
pub const INDUSTRY_CONSTRUCTION_NORMAL_GAMEPLAY: u8 = 1;
/// Industria creada por la generación de mapa.
pub const INDUSTRY_CONSTRUCTION_MAP_GENERATION: u8 = 2;
/// Industria creada desde el editor de escenarios.
pub const INDUSTRY_CONSTRUCTION_SCENARIO_EDITOR: u8 = 3;

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
    /// Central térmica: consume carbón y no produce carga transportable.
    PowerStation,
    IronOreMine,
    CopperOreMine,
    GoldMine,
    DiamondMine,
    Forest,
    Farm,
    /// Granja tropic (`IT_FARM_2`): maíz.
    FarmTropic,
    OilWells,
    OilRefinery,
    Factory,
    /// Fábrica tropic (`IT_FACTORY_2`): caucho + cobre + madera → goods.
    FactoryTropic,
    Sawmill,
    PaperMill,
    PrintingWorks,
    FoodProcessingPlant,
    FruitPlantation,
    RubberPlantation,
    WaterSupply,
    WaterTower,
    LumberMill,
    /// Acería temperate (mineral de hierro → acero).
    SteelMill,
    /// Banco temperate (objetos de valor).
    Bank,
    /// Banco ártico/trópico: acepta oro/diamantes.
    BankArcticTropic,
    CottonCandy,
    CandyFactory,
    BatteryFarm,
    ColaWells,
    ToyShop,
    ToyFactory,
    PlasticFountain,
    FizzyDrinkFactory,
    BubbleGenerator,
    ToffeeQuarry,
    SugarMine,
}

impl IndustrySpec {
    /// Industrias colocables en este clima (`LandscapeType` en `OpenTTD`).
    #[must_use]
    pub fn specs_for_climate(climate: Climate) -> &'static [IndustrySpec] {
        match climate {
            Climate::Temperate => &[
                Self::CoalMine,
                Self::PowerStation,
                Self::Sawmill,
                Self::Forest,
                Self::OilRefinery,
                Self::Factory,
                Self::SteelMill,
                Self::Farm,
                Self::OilWells,
                Self::Bank,
                Self::IronOreMine,
            ],
            Climate::SubArctic => &[
                Self::CoalMine,
                Self::PowerStation,
                Self::Forest,
                Self::OilRefinery,
                Self::PrintingWorks,
                Self::Farm,
                Self::OilWells,
                Self::FoodProcessingPlant,
                Self::PaperMill,
                Self::GoldMine,
                Self::BankArcticTropic,
            ],
            Climate::SubTropical => &[
                Self::OilRefinery,
                Self::CopperOreMine,
                Self::OilWells,
                Self::FoodProcessingPlant,
                Self::BankArcticTropic,
                Self::DiamondMine,
                Self::FruitPlantation,
                Self::RubberPlantation,
                Self::WaterSupply,
                Self::WaterTower,
                Self::FactoryTropic,
                Self::FarmTropic,
                Self::LumberMill,
            ],
            Climate::Toyland => &[
                Self::CottonCandy,
                Self::CandyFactory,
                Self::BatteryFarm,
                Self::ColaWells,
                Self::ToyShop,
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

    /// ID base de `IndustryType` en `OpenTTD`, previo a cualquier sustitución
    /// `NewGRF`. La generación de mapa itera estos IDs en orden ascendente al
    /// decidir las industrias forzadas, no en el orden del catálogo Rust.
    #[must_use]
    pub const fn native_type(self) -> u8 {
        match self {
            Self::CoalMine => 0,
            Self::PowerStation => 1,
            Self::Sawmill => 2,
            Self::Forest => 3,
            Self::OilRefinery => 4,
            Self::Factory => 6,
            Self::PrintingWorks => 7,
            Self::SteelMill => 8,
            Self::Farm => 9,
            Self::CopperOreMine => 10,
            Self::OilWells => 11,
            Self::Bank => 12,
            Self::FoodProcessingPlant => 13,
            Self::PaperMill => 14,
            Self::GoldMine => 15,
            Self::BankArcticTropic => 16,
            Self::DiamondMine => 17,
            Self::IronOreMine => 18,
            Self::FruitPlantation => 19,
            Self::RubberPlantation => 20,
            Self::WaterSupply => 21,
            Self::WaterTower => 22,
            Self::FactoryTropic => 23,
            Self::FarmTropic => 24,
            Self::LumberMill => 25,
            Self::CottonCandy => 26,
            Self::CandyFactory => 27,
            Self::BatteryFarm => 28,
            Self::ColaWells => 29,
            Self::ToyShop => 30,
            Self::ToyFactory => 31,
            Self::PlasticFountain => 32,
            Self::FizzyDrinkFactory => 33,
            Self::BubbleGenerator => 34,
            Self::ToffeeQuarry => 35,
            Self::SugarMine => 36,
        }
    }

    /// Tipos vanilla que no pueden quedar a 14 teselas o menos de `self`.
    ///
    /// Es el campo `IndustrySpec::conflicting` de
    /// `table/build_industry.h`. Sólo enumera tipos representables por este
    /// catálogo Rust: `IT_OIL_RIG` sigue fuera del modelo de industria
    /// procedural y, por tanto, no se puede expresar todavía como conflicto
    /// del refinery.
    #[must_use]
    pub const fn conflicting_specs(self) -> &'static [IndustrySpec] {
        match self {
            Self::CoalMine => &[Self::PowerStation],
            Self::PowerStation => &[Self::CoalMine],
            Self::Sawmill => &[Self::Forest],
            Self::Forest => &[Self::Sawmill, Self::PaperMill],
            Self::OilRefinery => &[],
            Self::Factory => &[Self::Farm, Self::SteelMill],
            Self::PrintingWorks => &[Self::PaperMill],
            Self::SteelMill => &[Self::IronOreMine, Self::Factory],
            Self::Farm => &[Self::Factory, Self::FoodProcessingPlant],
            Self::CopperOreMine | Self::RubberPlantation | Self::LumberMill => {
                &[Self::FactoryTropic]
            }
            Self::OilWells => &[Self::OilRefinery],
            Self::Bank => &[Self::Bank],
            Self::FoodProcessingPlant => &[Self::FruitPlantation, Self::Farm, Self::FarmTropic],
            Self::PaperMill => &[Self::Forest, Self::PrintingWorks],
            Self::GoldMine | Self::DiamondMine => &[Self::BankArcticTropic],
            Self::BankArcticTropic => &[Self::GoldMine, Self::DiamondMine],
            Self::IronOreMine => &[Self::SteelMill],
            Self::FruitPlantation | Self::FarmTropic => &[Self::FoodProcessingPlant],
            Self::WaterSupply => &[Self::WaterTower],
            Self::WaterTower => &[Self::WaterSupply],
            Self::FactoryTropic => &[
                Self::RubberPlantation,
                Self::CopperOreMine,
                Self::LumberMill,
            ],
            Self::CottonCandy | Self::ToffeeQuarry | Self::SugarMine => &[Self::CandyFactory],
            Self::CandyFactory => &[Self::CottonCandy, Self::ToffeeQuarry, Self::SugarMine],
            Self::BatteryFarm | Self::ToyShop | Self::PlasticFountain => &[Self::ToyFactory],
            Self::ColaWells | Self::BubbleGenerator => &[Self::FizzyDrinkFactory],
            Self::ToyFactory => &[Self::PlasticFountain, Self::BatteryFarm, Self::ToyShop],
            Self::FizzyDrinkFactory => &[Self::ColaWells, Self::BubbleGenerator],
        }
    }

    /// Especies Temperate que `GenerateIndustries` fuerza una vez durante
    /// creación de mapa. Corresponde a `appear_creation > 0` para el clima y
    /// al bucle ascendente de `IndustryType`; los chequeos/intententos de
    /// ubicación se portan por separado.
    #[must_use]
    pub const fn temperate_map_creation_force_one() -> &'static [IndustrySpec] {
        &[
            Self::CoalMine,
            Self::PowerStation,
            Self::Sawmill,
            Self::Forest,
            Self::OilRefinery,
            Self::Factory,
            Self::SteelMill,
            Self::Farm,
            Self::OilWells,
            Self::IronOreMine,
        ]
    }

    /// Probabilidad base `appear_creation` de `build_industry.h` para la
    /// generación de mapas (`GetScaledIndustryGenerationProbability`).
    ///
    /// El valor es el byte de la tabla vanilla antes de multiplicar por 16 y
    /// escalar por el tamaño del mapa. Un cero significa que la especie no se
    /// sortea ni se fuerza en ese clima. Mantener esta tabla junto al catálogo
    /// evita que el generador procedural vuelva a elegir especies de forma
    /// uniforme, lo que desplaza el stream RNG en mapas medianos/grandes.
    #[must_use]
    pub const fn map_creation_probability(self, climate: Climate) -> u8 {
        match climate {
            Climate::Temperate => match self {
                Self::CoalMine => 8,
                Self::PowerStation
                | Self::Sawmill
                | Self::Forest
                | Self::Factory
                | Self::SteelMill
                | Self::IronOreMine => 5,
                Self::OilRefinery | Self::OilWells => 4,
                Self::Farm => 9,
                _ => 0,
            },
            Climate::SubArctic => match self {
                Self::CoalMine => 8,
                Self::Forest
                | Self::PowerStation
                | Self::OilWells
                | Self::PrintingWorks
                | Self::PaperMill => 5,
                Self::Farm => 9,
                Self::OilRefinery | Self::GoldMine => 4,
                Self::FoodProcessingPlant => 3,
                Self::BankArcticTropic => 6,
                _ => 0,
            },
            Climate::SubTropical => match self {
                Self::OilRefinery
                | Self::CopperOreMine
                | Self::FoodProcessingPlant
                | Self::DiamondMine
                | Self::FruitPlantation
                | Self::RubberPlantation
                | Self::WaterSupply
                | Self::FactoryTropic => 4,
                Self::BankArcticTropic | Self::OilWells => 5,
                Self::WaterTower => 8,
                Self::FarmTropic => 2,
                _ => 0,
            },
            Climate::Toyland => match self {
                Self::CottonCandy
                | Self::CandyFactory
                | Self::ColaWells
                | Self::ToyFactory
                | Self::PlasticFountain
                | Self::BubbleGenerator
                | Self::ToffeeQuarry => 5,
                Self::BatteryFarm | Self::ToyShop | Self::FizzyDrinkFactory | Self::SugarMine => 4,
                _ => 0,
            },
        }
    }

    #[must_use]
    pub const fn kind(self) -> IndustryKind {
        match self {
            Self::CoalMine
            | Self::IronOreMine
            | Self::CopperOreMine
            | Self::GoldMine
            | Self::DiamondMine
            | Self::WaterSupply
            | Self::BatteryFarm
            | Self::PlasticFountain
            | Self::SugarMine
            | Self::ToffeeQuarry => IndustryKind::CoalMine,
            Self::Forest
            | Self::Farm
            | Self::FarmTropic
            | Self::FruitPlantation
            | Self::RubberPlantation
            | Self::CottonCandy
            | Self::BubbleGenerator => IndustryKind::Forest,
            Self::OilWells | Self::OilRefinery | Self::ColaWells => IndustryKind::OilWell,
            Self::PowerStation
            | Self::Factory
            | Self::FactoryTropic
            | Self::Sawmill
            | Self::PaperMill
            | Self::PrintingWorks
            | Self::FoodProcessingPlant
            | Self::SteelMill
            | Self::Bank
            | Self::BankArcticTropic
            | Self::WaterTower
            | Self::LumberMill
            | Self::CandyFactory
            | Self::ToyShop
            | Self::ToyFactory
            | Self::FizzyDrinkFactory => IndustryKind::Factory,
        }
    }

    /// Cargos producidos (primario primero). Sin aliases temperate.
    #[must_use]
    pub const fn produced_cargos(self) -> &'static [CargoType] {
        match self {
            Self::CoalMine => &[CargoType::Coal],
            Self::Forest | Self::LumberMill => &[CargoType::Wood],
            Self::Farm => &[CargoType::Grain, CargoType::Livestock],
            Self::FarmTropic => &[CargoType::Maize],
            Self::OilWells => &[CargoType::Oil],
            Self::IronOreMine => &[CargoType::IronOre],
            Self::CopperOreMine => &[CargoType::CopperOre],
            Self::GoldMine => &[CargoType::Gold],
            Self::DiamondMine => &[CargoType::Diamonds],
            Self::Bank => &[CargoType::Valuables],
            Self::FruitPlantation => &[CargoType::Fruit],
            Self::RubberPlantation => &[CargoType::Rubber],
            Self::WaterSupply => &[CargoType::Water],
            Self::CottonCandy => &[CargoType::CottonCandy],
            Self::BatteryFarm => &[CargoType::Batteries],
            Self::ColaWells => &[CargoType::Cola],
            Self::PlasticFountain => &[CargoType::Plastic],
            Self::BubbleGenerator => &[CargoType::Bubbles],
            Self::ToffeeQuarry => &[CargoType::Toffee],
            Self::SugarMine => &[CargoType::Sugar],
            Self::Sawmill
            | Self::Factory
            | Self::FactoryTropic
            | Self::OilRefinery
            | Self::PrintingWorks => &[CargoType::Goods],
            Self::SteelMill => &[CargoType::Steel],
            Self::PaperMill => &[CargoType::Paper],
            Self::FoodProcessingPlant => &[CargoType::Food],
            Self::CandyFactory => &[CargoType::Candy],
            Self::ToyFactory => &[CargoType::Toys],
            Self::FizzyDrinkFactory => &[CargoType::FizzyDrinks],
            Self::PowerStation | Self::BankArcticTropic | Self::WaterTower | Self::ToyShop => &[],
        }
    }

    /// Salida primaria (compat con stock único).
    #[must_use]
    pub const fn output_cargo(self) -> CargoType {
        match self {
            Self::CoalMine => CargoType::Coal,
            Self::Forest | Self::LumberMill => CargoType::Wood,
            Self::Farm => CargoType::Grain,
            Self::FarmTropic => CargoType::Maize,
            Self::OilWells => CargoType::Oil,
            Self::IronOreMine => CargoType::IronOre,
            Self::CopperOreMine => CargoType::CopperOre,
            Self::GoldMine => CargoType::Gold,
            Self::DiamondMine => CargoType::Diamonds,
            Self::Bank => CargoType::Valuables,
            Self::FruitPlantation => CargoType::Fruit,
            Self::RubberPlantation => CargoType::Rubber,
            Self::WaterSupply => CargoType::Water,
            Self::CottonCandy => CargoType::CottonCandy,
            Self::BatteryFarm => CargoType::Batteries,
            Self::ColaWells => CargoType::Cola,
            Self::PlasticFountain => CargoType::Plastic,
            Self::BubbleGenerator => CargoType::Bubbles,
            Self::ToffeeQuarry => CargoType::Toffee,
            Self::SugarMine => CargoType::Sugar,
            Self::SteelMill => CargoType::Steel,
            Self::PaperMill => CargoType::Paper,
            Self::FoodProcessingPlant => CargoType::Food,
            Self::CandyFactory => CargoType::Candy,
            Self::ToyFactory => CargoType::Toys,
            Self::FizzyDrinkFactory => CargoType::FizzyDrinks,
            Self::Sawmill
            | Self::Factory
            | Self::FactoryTropic
            | Self::OilRefinery
            | Self::PrintingWorks
            | Self::PowerStation
            | Self::BankArcticTropic
            | Self::WaterTower
            | Self::ToyShop => CargoType::Goods,
        }
    }

    /// Cargos aceptados (insumos / sumideros).
    #[must_use]
    pub const fn accepted_cargos(self) -> &'static [CargoType] {
        match self {
            Self::PowerStation => &[CargoType::Coal],
            Self::Sawmill | Self::PaperMill => &[CargoType::Wood],
            Self::OilRefinery => &[CargoType::Oil],
            Self::SteelMill => &[CargoType::IronOre],
            Self::Factory => &[CargoType::Livestock, CargoType::Grain, CargoType::Steel],
            Self::FactoryTropic => &[CargoType::Rubber, CargoType::CopperOre, CargoType::Wood],
            Self::PrintingWorks => &[CargoType::Paper],
            Self::FoodProcessingPlant => &[
                CargoType::Livestock,
                CargoType::Grain,
                CargoType::Fruit,
                CargoType::Wheat,
                CargoType::Maize,
            ],
            Self::Bank => &[CargoType::Valuables],
            Self::BankArcticTropic => &[CargoType::Gold, CargoType::Diamonds],
            Self::WaterTower => &[CargoType::Water],
            Self::CandyFactory => &[CargoType::Sugar, CargoType::Toffee, CargoType::CottonCandy],
            Self::ToyFactory => &[CargoType::Plastic, CargoType::Batteries],
            Self::ToyShop => &[CargoType::Toys],
            Self::FizzyDrinkFactory => &[CargoType::Cola, CargoType::Bubbles],
            _ => &[],
        }
    }

    /// Insumos y multiplicadores de procesadoras (`build_industry.h` / P1.5).
    #[must_use]
    #[allow(clippy::too_many_lines)]
    pub const fn processing_inputs(self) -> &'static [IndustryProcessingInput] {
        match self {
            Self::PowerStation => &[IndustryProcessingInput {
                cargo: CargoType::Coal,
                batch: 8,
                multiplier: 0,
            }],
            Self::WaterTower => &[IndustryProcessingInput {
                cargo: CargoType::Water,
                batch: 8,
                multiplier: 0,
            }],
            Self::ToyShop => &[IndustryProcessingInput {
                cargo: CargoType::Toys,
                batch: 8,
                multiplier: 0,
            }],
            Self::BankArcticTropic => &[IndustryProcessingInput {
                cargo: CargoType::Gold,
                batch: 8,
                multiplier: 0,
            }],
            Self::Sawmill | Self::PaperMill => &[IndustryProcessingInput {
                cargo: CargoType::Wood,
                batch: 8,
                multiplier: 256,
            }],
            Self::OilRefinery => &[IndustryProcessingInput {
                cargo: CargoType::Oil,
                batch: 8,
                multiplier: 256,
            }],
            Self::SteelMill => &[IndustryProcessingInput {
                cargo: CargoType::IronOre,
                batch: 8,
                multiplier: 256,
            }],
            Self::PrintingWorks => &[IndustryProcessingInput {
                cargo: CargoType::Paper,
                batch: 8,
                multiplier: 256,
            }],
            Self::Factory => &[
                IndustryProcessingInput {
                    cargo: CargoType::Livestock,
                    batch: FACTORY_LIVESTOCK_INPUT,
                    multiplier: 256,
                },
                IndustryProcessingInput {
                    cargo: CargoType::Grain,
                    batch: FACTORY_GRAIN_INPUT,
                    multiplier: 256,
                },
                IndustryProcessingInput {
                    cargo: CargoType::Steel,
                    batch: FACTORY_STEEL_INPUT,
                    multiplier: 256,
                },
            ],
            Self::FactoryTropic => &[
                IndustryProcessingInput {
                    cargo: CargoType::Rubber,
                    batch: 8,
                    multiplier: 256,
                },
                IndustryProcessingInput {
                    cargo: CargoType::CopperOre,
                    batch: 8,
                    multiplier: 256,
                },
                IndustryProcessingInput {
                    cargo: CargoType::Wood,
                    batch: 8,
                    multiplier: 256,
                },
            ],
            Self::FoodProcessingPlant => &[
                IndustryProcessingInput {
                    cargo: CargoType::Livestock,
                    batch: 8,
                    multiplier: 256,
                },
                IndustryProcessingInput {
                    cargo: CargoType::Wheat,
                    batch: 8,
                    multiplier: 256,
                },
            ],
            Self::CandyFactory => &[
                IndustryProcessingInput {
                    cargo: CargoType::Sugar,
                    batch: 8,
                    multiplier: 256,
                },
                IndustryProcessingInput {
                    cargo: CargoType::Toffee,
                    batch: 8,
                    multiplier: 256,
                },
                IndustryProcessingInput {
                    cargo: CargoType::CottonCandy,
                    batch: 8,
                    multiplier: 256,
                },
            ],
            Self::ToyFactory => &[
                IndustryProcessingInput {
                    cargo: CargoType::Plastic,
                    batch: 8,
                    multiplier: 256,
                },
                IndustryProcessingInput {
                    cargo: CargoType::Batteries,
                    batch: 8,
                    multiplier: 256,
                },
            ],
            Self::FizzyDrinkFactory => &[
                IndustryProcessingInput {
                    cargo: CargoType::Cola,
                    batch: 8,
                    multiplier: 256,
                },
                IndustryProcessingInput {
                    cargo: CargoType::Bubbles,
                    batch: 8,
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
    #[must_use]
    pub const fn production_rate(self) -> u8 {
        match self {
            Self::CoalMine => 15,
            Self::Forest | Self::CottonCandy | Self::BubbleGenerator => 13,
            Self::OilWells | Self::ColaWells | Self::WaterSupply => 12,
            Self::Farm
            | Self::IronOreMine
            | Self::CopperOreMine
            | Self::ToffeeQuarry
            | Self::FruitPlantation
            | Self::RubberPlantation => 10,
            Self::FarmTropic | Self::BatteryFarm | Self::SugarMine => 11,
            Self::GoldMine | Self::DiamondMine => 7,
            Self::Bank => 6,
            Self::PlasticFountain => 14,
            Self::PowerStation
            | Self::Factory
            | Self::FactoryTropic
            | Self::Sawmill
            | Self::PaperMill
            | Self::PrintingWorks
            | Self::FoodProcessingPlant
            | Self::SteelMill
            | Self::OilRefinery
            | Self::BankArcticTropic
            | Self::WaterTower
            | Self::LumberMill
            | Self::CandyFactory
            | Self::ToyShop
            | Self::ToyFactory
            | Self::FizzyDrinkFactory => 0,
        }
    }

    /// `production_rate[1]` (Farm temperate/arctic: Livestock).
    #[must_use]
    pub const fn production_rate_secondary(self) -> Option<u8> {
        match self {
            Self::Farm => Some(10),
            _ => None,
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
            | Self::DiamondMine
            | Self::OilWells
            | Self::ColaWells
            | Self::WaterSupply
            | Self::PlasticFountain
            | Self::SugarMine
            | Self::ToffeeQuarry
            | Self::BubbleGenerator => IndustryLifeType::Extractive,
            Self::Forest
            | Self::Farm
            | Self::FarmTropic
            | Self::FruitPlantation
            | Self::RubberPlantation
            | Self::CottonCandy
            | Self::BatteryFarm => IndustryLifeType::Organic,
            Self::PowerStation
            | Self::Bank
            | Self::BankArcticTropic
            | Self::WaterTower
            | Self::ToyShop => IndustryLifeType::BlackHole,
            Self::Factory
            | Self::FactoryTropic
            | Self::Sawmill
            | Self::PaperMill
            | Self::PrintingWorks
            | Self::FoodProcessingPlant
            | Self::SteelMill
            | Self::OilRefinery
            | Self::LumberMill
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
    /// Stock del segundo cargo producido (Farm: Livestock; 0 si no aplica).
    #[serde(default)]
    pub secondary_stock: u32,
    /// Cargo aceptado pendiente de procesar por callbacks `NewGRF`.
    ///
    /// El modelo vanilla conserva sus entradas en estaciones; este buffer
    /// reproduce los `accepted[i].waiting` de `OpenTTD` para CB1/CB2 y evita
    /// perder entregas cuando el GRF decide consumirlas más tarde.
    #[serde(default)]
    pub newgrf_accepted_cargo_waiting: CargoStock,
    /// Último día económico en que cada cargo fue aceptado, en la escala
    /// absoluta de `OpenTTD` (1920-01-01 = 0). Cero equivale a nunca/legacy.
    #[serde(default)]
    pub newgrf_last_accepted: CargoStock,
    /// Salidas `NewGRF` que no caben en los dos stocks legacy (`stock` y
    /// `secondary_stock`). Las dos salidas principales se reflejan también en
    /// esos campos para mantener compatibilidad con el cargador existente.
    #[serde(default)]
    pub newgrf_extra_produced_cargo: CargoStock,
    pub capacity: u32,
    /// Color aleatorio de industria (`Colours` 0–15) para edificios con paleta.
    #[serde(default)]
    pub random_colour: u8,
    /// Layout elegido al crear la industria (`Industry::selected_layout`).
    ///
    /// `OpenTTD` conserva el ordinal uno-based (cero identifica legacy) incluso
    /// cuando dos layouts producen la misma huella geométrica; no se debe
    /// inferir únicamente desde `tiles`.
    #[serde(default)]
    pub selected_layout: u8,
    /// Bits aleatorios persistentes de la instancia para scopes `NewGRF`.
    ///
    /// La tabla `INDY` los guarda como `random` desde `SLV_82`; los saves
    /// antiguos dejan cero, que es también el valor vanilla por defecto.
    #[serde(default)]
    pub newgrf_random: u16,
    /// Registros `7C` persistentes por industria para callbacks `NewGRF`.
    ///
    /// La clave es el índice PSA (0..255) y el valor conserva los 32 bits
    /// escritos por `\2psto`. El GRFID se resuelve desde el catálogo activo.
    #[serde(default)]
    pub newgrf_persistent_regs: std::collections::HashMap<u8, u32>,
    /// Índice del pool `PersistentStorage` referenciado por `INDY.psa`.
    ///
    /// `None` identifica una industria sin storage nativo. El índice no se
    /// confunde con `instance_id`: el pool PSA puede tener huecos y su
    /// identidad debe sobrevivir a un round-trip `.sav`.
    #[serde(default)]
    pub newgrf_persistent_storage_id: Option<u32>,
    /// Compañía que fundó la industria (`INVALID_OWNER` se representa como
    /// `None`). Las industrias generadas en el mapa no tienen fundador.
    #[serde(default)]
    pub founder: Option<crate::company::CompanyId>,
    /// Fecha absoluta de construcción, en días desde 1920-01-01 como en
    /// `TimerGameCalendar::Date` de `OpenTTD`.
    #[serde(default)]
    pub construction_date: u32,
    /// Forma en que se creó la industria (`IndustryConstructionType`).
    #[serde(default)]
    pub construction_type: u8,
    /// Flags de control de `GameScript` (`IndustryControlFlags`), opacos para
    /// el modelo reducido pero visibles desde `NewGRF`.
    #[serde(default)]
    pub control_flags: u8,
    /// Marca que la industria fue elegida como destino de una entrega.
    #[serde(default)]
    pub was_cargo_delivered: bool,
    /// Último año económico en que produjo carga.
    #[serde(default)]
    pub last_prod_year: u32,
    /// `IndustryID` de mapa (`MAP2`, bytes bajo/alto). El ID 0 es válido en un
    /// `.sav`; cuando no existe una entidad correspondiente también se usa
    /// como fallback legacy.
    #[serde(default)]
    pub instance_id: u16,
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
    /// Id global en `industry_spec_catalog` si proviene de `NewGRF` (`None` = vanilla).
    #[serde(default)]
    pub newgrf_type_id: Option<u16>,
    /// Pueblo asociado por `ClosestTownFromTile` al crear la industria.
    ///
    /// Las industrias fundadas sobre una casa (bancos árticos/tropicales)
    /// resuelven el pueblo desde la tesela (`Town::GetByTile`), que puede no
    /// ser el pueblo geométricamente más cercano.  Conservarlo evita perder
    /// esa relación cuando la casa es reemplazada por la huella industrial;
    /// saves antiguos dejan `None` y usan el fallback geométrico.
    #[serde(default)]
    pub town_id: Option<u32>,
    /// Rate de producción `NewGRF` (copia del def al colocar; `None` = usar vanilla).
    #[serde(default)]
    pub newgrf_production_rate: Option<u8>,
    /// Rate de la segunda salida `NewGRF` (copia de `production_rates[1]`).
    #[serde(default)]
    pub newgrf_secondary_production_rate: Option<u8>,
    /// Cargo de salida `NewGRF` resuelto (`None` = usar vanilla/`kind`).
    #[serde(default)]
    pub newgrf_output_cargo: Option<CargoType>,
    /// Segundo cargo de salida `NewGRF`, si el label pudo resolverse.
    #[serde(default)]
    pub newgrf_secondary_output_cargo: Option<CargoType>,
    /// Cargos de salida adicionales habilitados por `CargoTypesUnlimited`.
    /// Las dos primeras salidas conservan los stocks legacy; las siguientes
    /// utilizan `newgrf_extra_produced_cargo`.
    #[serde(default)]
    pub newgrf_extra_output_cargos: Vec<CargoType>,
    /// Tasas de producción de las salidas desde el tercer slot, alineadas con
    /// `newgrf_extra_output_cargos`.
    #[serde(default)]
    pub newgrf_extra_production_rates: Vec<u8>,
    /// `true` cuando los callbacks `0x14B`/`0x14C` reemplazaron las listas
    /// estáticas al fundar la industria. Permite representar una lista
    /// callback vacía sin volver al fallback vanilla.
    #[serde(default)]
    pub newgrf_dynamic_cargo_types: bool,
    /// Cargos efectivos por slot de entrada (`None` = slot vacío legacy).
    ///
    /// `OpenTTD` conserva los huecos que los GRF antiguos crean devolviendo un
    /// cargo inválido desde `CBID_INDUSTRY_INPUT_CARGO_TYPES`; no se deben
    /// compactar porque los índices de los callbacks de sufijo/multiplicador
    /// siguen siendo los originales.
    #[serde(default)]
    pub newgrf_input_cargo_slots: Vec<Option<CargoType>>,
    /// Cargos efectivos por slot de salida (`None` = slot vacío legacy).
    #[serde(default)]
    pub newgrf_output_cargo_slots: Vec<Option<CargoType>>,
    /// Insumos y multiplicadores de una procesadora `NewGRF`.
    #[serde(default)]
    pub newgrf_processing_inputs: Vec<IndustryProcessingInput>,
    /// Multiplicadores del segundo output para esos mismos insumos.
    #[serde(default)]
    pub newgrf_processing_secondary_multipliers: Vec<u16>,
    /// Multiplicadores de las salidas desde el tercer slot, aplanados como
    /// `[input][extra_output]`.
    #[serde(default)]
    pub newgrf_processing_extra_multipliers: Vec<u16>,
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
            secondary_stock: 0,
            newgrf_accepted_cargo_waiting: CargoStock::default(),
            newgrf_last_accepted: CargoStock::default(),
            newgrf_extra_produced_cargo: CargoStock::default(),
            capacity: INDUSTRY_STOCK_CAPACITY,
            random_colour: 0,
            selected_layout: 0,
            newgrf_random: 0,
            newgrf_persistent_regs: std::collections::HashMap::new(),
            newgrf_persistent_storage_id: None,
            founder: None,
            construction_date: 0,
            construction_type: INDUSTRY_CONSTRUCTION_UNKNOWN,
            control_flags: 0,
            was_cargo_delivered: false,
            last_prod_year: 0,
            instance_id: 0,
            produced_total: 0,
            transported_total: 0,
            history: IndustryHistory::default(),
            counter: 0,
            prod_level: PRODLEVEL_DEFAULT,
            newgrf_type_id: None,
            town_id: None,
            newgrf_production_rate: None,
            newgrf_secondary_production_rate: None,
            newgrf_output_cargo: None,
            newgrf_secondary_output_cargo: None,
            newgrf_extra_output_cargos: Vec::new(),
            newgrf_extra_production_rates: Vec::new(),
            newgrf_dynamic_cargo_types: false,
            newgrf_input_cargo_slots: Vec::new(),
            newgrf_output_cargo_slots: Vec::new(),
            newgrf_processing_inputs: Vec::new(),
            newgrf_processing_secondary_multipliers: Vec::new(),
            newgrf_processing_extra_multipliers: Vec::new(),
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
            secondary_stock: 0,
            newgrf_accepted_cargo_waiting: CargoStock::default(),
            newgrf_last_accepted: CargoStock::default(),
            newgrf_extra_produced_cargo: CargoStock::default(),
            capacity: INDUSTRY_STOCK_CAPACITY,
            random_colour: 0,
            selected_layout: 0,
            newgrf_random: 0,
            newgrf_persistent_regs: std::collections::HashMap::new(),
            newgrf_persistent_storage_id: None,
            founder: None,
            construction_date: 0,
            construction_type: INDUSTRY_CONSTRUCTION_UNKNOWN,
            control_flags: 0,
            was_cargo_delivered: false,
            last_prod_year: 0,
            instance_id: 0,
            produced_total: 0,
            transported_total: 0,
            history: IndustryHistory::default(),
            counter: 0,
            prod_level: PRODLEVEL_DEFAULT,
            newgrf_type_id: None,
            town_id: None,
            newgrf_production_rate: None,
            newgrf_secondary_production_rate: None,
            newgrf_output_cargo: None,
            newgrf_secondary_output_cargo: None,
            newgrf_extra_output_cargos: Vec::new(),
            newgrf_extra_production_rates: Vec::new(),
            newgrf_dynamic_cargo_types: false,
            newgrf_input_cargo_slots: Vec::new(),
            newgrf_output_cargo_slots: Vec::new(),
            newgrf_processing_inputs: Vec::new(),
            newgrf_processing_secondary_multipliers: Vec::new(),
            newgrf_processing_extra_multipliers: Vec::new(),
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
            secondary_stock: 0,
            newgrf_accepted_cargo_waiting: CargoStock::default(),
            newgrf_last_accepted: CargoStock::default(),
            newgrf_extra_produced_cargo: CargoStock::default(),
            capacity: INDUSTRY_STOCK_CAPACITY,
            random_colour,
            selected_layout: 0,
            newgrf_random: 0,
            newgrf_persistent_regs: std::collections::HashMap::new(),
            newgrf_persistent_storage_id: None,
            founder: None,
            construction_date: 0,
            construction_type: INDUSTRY_CONSTRUCTION_UNKNOWN,
            control_flags: 0,
            was_cargo_delivered: false,
            last_prod_year: 0,
            instance_id: 0,
            produced_total: 0,
            transported_total: 0,
            history: IndustryHistory::default(),
            counter: 0,
            prod_level: PRODLEVEL_DEFAULT,
            newgrf_type_id: None,
            town_id: None,
            newgrf_production_rate: None,
            newgrf_secondary_production_rate: None,
            newgrf_output_cargo: None,
            newgrf_secondary_output_cargo: None,
            newgrf_extra_output_cargos: Vec::new(),
            newgrf_extra_production_rates: Vec::new(),
            newgrf_dynamic_cargo_types: false,
            newgrf_input_cargo_slots: Vec::new(),
            newgrf_output_cargo_slots: Vec::new(),
            newgrf_processing_inputs: Vec::new(),
            newgrf_processing_secondary_multipliers: Vec::new(),
            newgrf_processing_extra_multipliers: Vec::new(),
        }
    }

    /// Asigna el `IndustryID` de mapa (`m2`).
    #[must_use]
    pub fn with_instance_id(mut self, instance_id: u16) -> Self {
        self.instance_id = instance_id;
        self
    }

    /// Asigna el pueblo asociado por `ClosestTownFromTile` al crearla.
    #[must_use]
    pub const fn with_town_id(mut self, town_id: Option<u32>) -> Self {
        self.town_id = town_id;
        self
    }

    /// Asigna `Industry.random_colour` (`Colours` 0–15).
    #[must_use]
    pub fn with_random_colour(mut self, random_colour: u8) -> Self {
        self.random_colour = random_colour % 16;
        self
    }

    /// Conserva el ordinal uno-based del layout usado para materializar la huella.
    #[must_use]
    pub const fn with_selected_layout(mut self, selected_layout: u8) -> Self {
        self.selected_layout = selected_layout;
        self
    }

    /// Conserva los bits aleatorios de la instancia `NewGRF`.
    #[must_use]
    pub const fn with_newgrf_random(mut self, random: u16) -> Self {
        self.newgrf_random = random;
        self
    }

    /// Conserva la compañía fundadora (`None` = `INVALID_OWNER`).
    #[must_use]
    pub const fn with_founder(mut self, founder: Option<crate::company::CompanyId>) -> Self {
        self.founder = founder;
        self
    }

    /// Conserva la fecha absoluta de construcción de `OpenTTD`.
    #[must_use]
    pub const fn with_construction_date(mut self, construction_date: u32) -> Self {
        self.construction_date = construction_date;
        self
    }

    /// Conserva el tipo de construcción (`ICT_*`).
    #[must_use]
    pub const fn with_construction_type(mut self, construction_type: u8) -> Self {
        self.construction_type = construction_type;
        self
    }

    /// Conserva flags de control de `GameScript` sin interpretarlos.
    #[must_use]
    pub const fn with_control_flags(mut self, control_flags: u8) -> Self {
        self.control_flags = control_flags;
        self
    }

    /// Marca la industria como destino de una entrega.
    #[must_use]
    pub const fn with_was_cargo_delivered(mut self, delivered: bool) -> Self {
        self.was_cargo_delivered = delivered;
        self
    }

    /// Conserva el último año económico con producción.
    #[must_use]
    pub const fn with_last_prod_year(mut self, year: u32) -> Self {
        self.last_prod_year = year;
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
        if let Some(rate) = self.newgrf_production_rate {
            return rate;
        }
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
        Self::scaled_production_amount(self.production_rate(), self.prod_level)
    }

    /// Unidades del segundo output (Farm Livestock), si el spec lo define.
    #[must_use]
    pub fn produce_secondary_amount(&self) -> u32 {
        let Some(rate) = self
            .newgrf_secondary_production_rate
            .or_else(|| self.spec.and_then(IndustrySpec::production_rate_secondary))
        else {
            return 0;
        };
        Self::scaled_production_amount(rate, self.prod_level)
    }

    fn scaled_production_amount(rate: u8, prod_level: u8) -> u32 {
        if prod_level == PRODLEVEL_CLOSURE || rate == 0 {
            return 0;
        }
        (u32::from(rate) * u32::from(prod_level))
            .div_ceil(u32::from(PRODLEVEL_DEFAULT))
            .min(255)
    }

    fn production_rate_for_output(&self, index: usize) -> u8 {
        match index {
            0 => self.production_rate(),
            1 => self
                .newgrf_secondary_production_rate
                .or_else(|| self.spec.and_then(IndustrySpec::production_rate_secondary))
                .unwrap_or(0),
            _ => self
                .newgrf_extra_production_rates
                .get(index - 2)
                .copied()
                .unwrap_or(0),
        }
    }

    fn production_amounts_for_outputs(&self, outputs: &[CargoType]) -> Vec<u32> {
        outputs
            .iter()
            .enumerate()
            .map(|(index, _)| {
                Self::scaled_production_amount(
                    self.production_rate_for_output(index),
                    self.prod_level,
                )
            })
            .collect()
    }

    /// Cargos producidos del spec (primario + secundario).
    #[must_use]
    pub fn produced_cargos(&self) -> Vec<CargoType> {
        if self.newgrf_type_id.is_some() {
            let mut cargos = Vec::with_capacity(2 + self.newgrf_extra_output_cargos.len());
            if let Some(cargo) = self.newgrf_output_cargo {
                cargos.push(cargo);
            }
            if let Some(cargo) = self.newgrf_secondary_output_cargo {
                cargos.push(cargo);
            }
            cargos.extend(self.newgrf_extra_output_cargos.iter().copied());
            if self.newgrf_dynamic_cargo_types || !cargos.is_empty() {
                return cargos;
            }
        }
        if let Some(spec) = self.spec {
            return spec.produced_cargos().to_vec();
        }
        match self.kind {
            IndustryKind::CoalMine => vec![CargoType::Coal],
            IndustryKind::Forest => vec![CargoType::Wood],
            IndustryKind::OilWell => vec![CargoType::Oil],
            IndustryKind::Factory => vec![CargoType::Goods],
        }
    }

    /// Segundo cargo de salida (Farm → Livestock).
    #[must_use]
    pub fn secondary_output_cargo(&self) -> Option<CargoType> {
        self.newgrf_secondary_output_cargo
            .or_else(|| self.produced_cargos().get(1).copied())
    }

    /// Cantidad de un cargo aceptado que espera el callback `NewGRF`.
    #[must_use]
    pub const fn accepted_cargo_waiting(&self, cargo: CargoType) -> u32 {
        self.newgrf_accepted_cargo_waiting.get(cargo)
    }

    /// Última fecha absoluta en que se aceptó `cargo` (`Industry::AcceptedCargo`).
    #[must_use]
    pub const fn last_accepted_date(&self, cargo: CargoType) -> u32 {
        self.newgrf_last_accepted.get(cargo)
    }

    /// Guarda la fecha absoluta de la última aceptación de `cargo`.
    pub fn set_last_accepted_date(&mut self, cargo: CargoType, date: u32) {
        self.newgrf_last_accepted.set(cargo, date);
    }

    /// Registra una entrega aceptada y su fecha económica absoluta.
    ///
    /// El llamador decide si el callback de producción consume la cola; este
    /// método sólo refleja el estado nativo de aceptación y evita marcarlo
    /// para ajustes internos de una cola ya importada.
    pub fn record_accepted_cargo(&mut self, cargo: CargoType, amount: u32, date: u32) {
        if amount == 0 {
            return;
        }
        self.add_accepted_cargo_waiting(cargo, amount);
        self.set_last_accepted_date(cargo, date);
    }

    /// Añade cargo a la cola de entradas de la industria.
    pub fn add_accepted_cargo_waiting(&mut self, cargo: CargoType, amount: u32) {
        let current = self.accepted_cargo_waiting(cargo);
        let room = u32::from(u16::MAX).saturating_sub(current);
        self.newgrf_accepted_cargo_waiting
            .add(cargo, amount.min(room));
    }

    /// Retira cargo de la cola de entradas de la industria.
    #[must_use]
    pub fn take_accepted_cargo_waiting(&mut self, cargo: CargoType, amount: u32) -> u32 {
        self.newgrf_accepted_cargo_waiting.take(cargo, amount)
    }

    /// ¿La industria tiene un slot de entrada para `cargo`?
    ///
    /// `DeliverGoodsToIndustry` consulta la lista efectiva de `accepted`, no
    /// la lista de insumos que casualmente se usa en un ciclo de producción.
    /// Esto importa para sumideros y para GRF que dejan huecos `INVALID_CARGO`
    /// o declaran entradas que no tienen multiplicador de salida.
    #[must_use]
    pub fn accepts_cargo(&self, cargo: CargoType) -> bool {
        if !self.newgrf_input_cargo_slots.is_empty() {
            return self
                .newgrf_input_cargo_slots
                .iter()
                .flatten()
                .any(|&candidate| candidate == cargo);
        }
        if self.newgrf_type_id.is_some() {
            return self
                .newgrf_processing_inputs
                .iter()
                .any(|input| input.cargo == cargo);
        }
        if let Some(spec) = self.spec {
            return spec.accepted_cargos().contains(&cargo);
        }
        match self.kind {
            IndustryKind::Factory => IndustrySpec::Factory.accepted_cargos().contains(&cargo),
            _ => false,
        }
    }

    /// Aplica la ruta vanilla de `TriggerIndustryProduction` a las entradas
    /// que ya fueron aceptadas por la industria.
    ///
    /// La función se usa después de descargar vehículos, cuando no hay CB1
    /// de llegada. Consume cada cola `accepted[i].waiting` y agrega la salida
    /// de cada columna de la matriz de multiplicadores. Devuelve las unidades
    /// realmente añadidas a los stocks (respetando la capacidad), para que el
    /// llamador actualice estadísticas y deltas mensuales.
    pub fn process_accepted_cargo_without_callback(&mut self) -> u32 {
        let inputs = self.processing_inputs().to_vec();
        let outputs = self.produced_cargos();
        if inputs.is_empty() {
            // Un sumidero sin matriz todavía debe retirar la entrega de su
            // cola; de lo contrario la misma carga se volvería a procesar en
            // cada callback posterior.
            for cargo in ALL_CARGO_TYPES {
                let _ = self.take_accepted_cargo_waiting(cargo, u32::MAX);
            }
            return 0;
        }

        let mut waiting = Vec::with_capacity(inputs.len());
        for input in &inputs {
            let amount = self.take_accepted_cargo_waiting(input.cargo, u32::MAX);
            waiting.push(amount);
        }
        // La tabla vanilla puede aceptar cargos con multiplicador cero (por
        // ejemplo Fruit/Maize en la planta de alimentos). `TriggerIndustryProduction`
        // también retira esos slots aunque no generen salida; no dejarlos en
        // la cola evita que una entrega vieja se procese indefinidamente.
        for cargo in ALL_CARGO_TYPES {
            let _ = self.take_accepted_cargo_waiting(cargo, u32::MAX);
        }
        if outputs.is_empty() {
            return 0;
        }

        let stock_before: Vec<u32> = outputs
            .iter()
            .map(|&cargo| self.output_stock(cargo))
            .collect();
        for (input_idx, amount) in waiting.into_iter().enumerate() {
            if amount == 0 {
                continue;
            }
            for (output_idx, &cargo) in outputs.iter().enumerate() {
                let multiplier = self.processing_multiplier_for_output(
                    &inputs,
                    input_idx,
                    output_idx,
                    outputs.len(),
                );
                let produced = amount
                    .saturating_mul(u32::from(multiplier))
                    .saturating_div(256);
                if produced == 0 {
                    continue;
                }
                if self.newgrf_type_id.is_some() {
                    self.add_newgrf_produced_cargo(cargo, produced);
                } else if output_idx == 0 {
                    self.stock = self.stock.saturating_add(produced).min(self.capacity);
                } else if output_idx == 1 {
                    self.secondary_stock = self
                        .secondary_stock
                        .saturating_add(produced)
                        .min(self.capacity);
                }
            }
        }
        outputs
            .iter()
            .enumerate()
            .map(|(index, &cargo)| self.output_stock(cargo).saturating_sub(stock_before[index]))
            .sum()
    }

    fn output_stock(&self, cargo: CargoType) -> u32 {
        if Some(cargo) == self.newgrf_output_cargo || cargo == self.output_cargo() {
            self.stock
        } else if Some(cargo) == self.newgrf_secondary_output_cargo
            || Some(cargo) == self.secondary_output_cargo()
        {
            self.secondary_stock
        } else {
            self.extra_produced_cargo(cargo)
        }
    }

    fn processing_multiplier_for_output(
        &self,
        inputs: &[IndustryProcessingInput],
        input_idx: usize,
        output_idx: usize,
        output_count: usize,
    ) -> u16 {
        if output_idx == 0 {
            return inputs.get(input_idx).map_or(0, |input| input.multiplier);
        }
        if self.newgrf_type_id.is_none() {
            return 0;
        }
        if output_idx == 1 {
            return self
                .newgrf_processing_secondary_multipliers
                .get(input_idx)
                .copied()
                .unwrap_or(0);
        }
        self.newgrf_processing_extra_multipliers
            .get(input_idx.saturating_mul(output_count.saturating_sub(2)) + output_idx - 2)
            .copied()
            .unwrap_or(0)
    }

    /// Registra una salida de callback. Las dos salidas legacy se reflejan en
    /// `stock`/`secondary_stock`; el resto queda disponible para el transporte
    /// `NewGRF` sin alterar el formato histórico del estado.
    pub fn add_newgrf_produced_cargo(&mut self, cargo: CargoType, amount: u32) {
        if amount == 0 {
            return;
        }
        if Some(cargo) == self.newgrf_output_cargo || cargo == self.output_cargo() {
            self.stock = self.stock.saturating_add(amount).min(self.capacity);
        } else if Some(cargo) == self.newgrf_secondary_output_cargo
            || Some(cargo) == self.secondary_output_cargo()
        {
            self.secondary_stock = self
                .secondary_stock
                .saturating_add(amount)
                .min(self.capacity);
        } else {
            self.newgrf_extra_produced_cargo.add(cargo, amount);
        }
    }

    /// Cantidad de una salida `NewGRF` que no está en los stocks legacy.
    #[must_use]
    pub const fn extra_produced_cargo(&self, cargo: CargoType) -> u32 {
        self.newgrf_extra_produced_cargo.get(cargo)
    }

    /// Retira una salida `NewGRF` adicional ya entregada a la industria.
    #[must_use]
    pub fn take_extra_produced_cargo(&mut self, cargo: CargoType, amount: u32) -> u32 {
        self.newgrf_extra_produced_cargo.take(cargo, amount)
    }

    /// Salida de procesadora escalada por `prod_level` (legacy; preferir [`Self::processing_output_amount`]).
    #[must_use]
    pub fn factory_output_amount(&self) -> u32 {
        self.processing_output_amount()
    }

    /// Unidades de salida tras consumir los lotes de `Self::processing_inputs`.
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

    /// Salidas de una procesadora para todos los slots `NewGRF` activos.
    fn processing_output_amounts_all(&self) -> Vec<u32> {
        if self.prod_level == PRODLEVEL_CLOSURE {
            return vec![0u32; self.produced_cargos().len()];
        }
        let inputs = self.processing_inputs();
        if inputs.is_empty() {
            return vec![0u32; self.produced_cargos().len()];
        }
        let outputs = self.produced_cargos();
        if self.newgrf_dynamic_cargo_types && outputs.is_empty() {
            return Vec::new();
        }
        if self.newgrf_type_id.is_none() {
            let mut amounts = vec![0u32; outputs.len()];
            if let Some(primary) = amounts.first_mut() {
                *primary = self.processing_output_amount();
            }
            return amounts;
        }
        let mut amounts = vec![0u32; outputs.len()];
        for (input_idx, input) in inputs.iter().enumerate() {
            let consumed = scaled_processing_batch(input.batch, self.prod_level);
            if let Some(primary) = amounts.first_mut() {
                *primary = (*primary)
                    .saturating_add(consumed.saturating_mul(u32::from(input.multiplier)) / 256);
            }
            if let Some(secondary) = amounts.get_mut(1) {
                let multiplier = self
                    .newgrf_processing_secondary_multipliers
                    .get(input_idx)
                    .copied()
                    .unwrap_or(0);
                *secondary = (*secondary)
                    .saturating_add(consumed.saturating_mul(u32::from(multiplier)) / 256);
            }
            for (output_idx, amount) in amounts.iter_mut().enumerate().skip(2) {
                let multiplier = self
                    .newgrf_processing_extra_multipliers
                    .get(input_idx.saturating_mul(outputs.len() - 2) + output_idx - 2)
                    .copied()
                    .unwrap_or(0);
                *amount =
                    (*amount).saturating_add(consumed.saturating_mul(u32::from(multiplier)) / 256);
            }
        }
        amounts
    }

    fn all_output_stocks_full(&self) -> bool {
        let outputs = self.produced_cargos();
        !outputs.is_empty()
            && outputs.iter().enumerate().all(|(index, cargo)| {
                let stock = match index {
                    0 => self.stock,
                    1 => self.secondary_stock,
                    _ => self.extra_produced_cargo(*cargo),
                };
                stock >= self.capacity
            })
    }

    fn processing_inputs(&self) -> &[IndustryProcessingInput] {
        if self.newgrf_type_id.is_some() {
            return &self.newgrf_processing_inputs;
        }
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

    /// Produce cargo primario (y secundario si aplica) si el tick cae en el periodo.
    pub fn produce(&mut self, tick: u64) {
        if self.requires_station_inputs() || self.is_closing() {
            return;
        }
        let outputs = self.produced_cargos();
        let amounts = self.production_amounts_for_outputs(&outputs);
        if amounts.iter().all(|&amount| amount == 0) {
            return;
        }
        if self.produces_on_tick(tick) {
            for (index, (&cargo, &amount)) in outputs.iter().zip(&amounts).enumerate() {
                if amount == 0 {
                    continue;
                }
                if self.newgrf_type_id.is_some() {
                    self.add_newgrf_produced_cargo(cargo, amount);
                } else if index == 0 {
                    self.stock = self.stock.saturating_add(amount).min(self.capacity);
                } else if index == 1 {
                    self.secondary_stock = self
                        .secondary_stock
                        .saturating_add(amount)
                        .min(self.capacity);
                }
            }
        }
    }

    /// Procesadoras: consumen insumos en estaciones de carga dentro de cobertura.
    ///
    /// Devuelve `true` si hubo un ciclo de procesamiento en este tick.
    pub fn produce_from_nearby_stations(&mut self, stations: &mut [Station], tick: u64) -> bool {
        self.produce_from_nearby_stations_with_callback_and_newgrf(stations, tick, false, None)
    }

    /// Variante que deja las entradas en la cola de la industria para que el
    /// callback `ProductionCargoArrival` decida cuánto consumir y producir.
    pub fn produce_from_nearby_stations_with_callback(
        &mut self,
        stations: &mut [Station],
        tick: u64,
        callback_on_arrival: bool,
    ) -> bool {
        self.produce_from_nearby_stations_with_callback_and_newgrf(
            stations,
            tick,
            callback_on_arrival,
            None,
        )
    }

    /// Variante de procesamiento que consulta `CBID_INDUSTRY_REFUSE_CARGO`
    /// después de verificar que todas las entradas están disponibles y antes
    /// de retirarlas de las estaciones. Esto conserva la cola intacta cuando
    /// una industria `NewGRF` rechaza temporalmente uno de sus cargos.
    pub fn produce_from_nearby_stations_with_callback_and_newgrf(
        &mut self,
        stations: &mut [Station],
        tick: u64,
        callback_on_arrival: bool,
        newgrf_def: Option<&IndustrySpecDef>,
    ) -> bool {
        let inputs = self.processing_inputs();
        if inputs.is_empty() || self.is_closing() {
            return false;
        }
        if !self.produces_on_tick(tick) || self.all_output_stocks_full() {
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

        if let Some(def) = newgrf_def
            && requirements.iter().any(|&(cargo, _)| {
                crate::newgrf_callback::resolve_industry_refuse_cargo_callback(def, self, cargo)
                    == Some(true)
            })
        {
            return false;
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
                    if callback_on_arrival {
                        self.add_accepted_cargo_waiting(cargo, take);
                    }
                    remaining -= take;
                }
            }
            debug_assert_eq!(remaining, 0);
        }

        if callback_on_arrival {
            return true;
        }

        let outputs = self.produced_cargos();
        let amounts = self.processing_output_amounts_all();
        if amounts.iter().all(|&amount| amount == 0) {
            return self.life_type() == IndustryLifeType::BlackHole;
        }
        for (index, (&cargo, &amount)) in outputs.iter().zip(&amounts).enumerate() {
            if amount == 0 {
                continue;
            }
            if self.newgrf_type_id.is_some() {
                self.add_newgrf_produced_cargo(cargo, amount);
            } else if index == 0 {
                self.stock = self.stock.saturating_add(amount).min(self.capacity);
            } else if index == 1 {
                self.secondary_stock = self
                    .secondary_stock
                    .saturating_add(amount)
                    .min(self.capacity);
            }
        }
        true
    }

    /// Industrias que transforman cargo entregado en estaciones cercanas.
    #[must_use]
    pub fn requires_station_inputs(&self) -> bool {
        !self.processing_inputs().is_empty()
    }

    /// Insumos de estación (cargo + lote a `prod_level` por defecto) para UI y tests.
    #[must_use]
    pub fn station_input_requirements(&self) -> Vec<(CargoType, u32)> {
        if !self.newgrf_processing_inputs.is_empty() {
            return self
                .newgrf_processing_inputs
                .iter()
                .map(|input| (input.cargo, input.batch))
                .collect();
        }
        let requirements: &[(CargoType, u32)] = match self.spec.unwrap_or(match self.kind {
            IndustryKind::Factory => IndustrySpec::Factory,
            IndustryKind::CoalMine => IndustrySpec::CoalMine,
            IndustryKind::Forest => IndustrySpec::Forest,
            IndustryKind::OilWell => IndustrySpec::OilWells,
        }) {
            IndustrySpec::Factory => &[
                (CargoType::Livestock, FACTORY_LIVESTOCK_INPUT),
                (CargoType::Grain, FACTORY_GRAIN_INPUT),
                (CargoType::Steel, FACTORY_STEEL_INPUT),
            ],
            IndustrySpec::PowerStation => &[(CargoType::Coal, 8)],
            IndustrySpec::Sawmill | IndustrySpec::PaperMill => &[(CargoType::Wood, 8)],
            IndustrySpec::OilRefinery => &[(CargoType::Oil, 8)],
            IndustrySpec::SteelMill => &[(CargoType::IronOre, 8)],
            IndustrySpec::PrintingWorks => &[(CargoType::Paper, 8)],
            IndustrySpec::CandyFactory => &[
                (CargoType::Sugar, 8),
                (CargoType::Toffee, 8),
                (CargoType::CottonCandy, 8),
            ],
            IndustrySpec::ToyFactory => &[(CargoType::Plastic, 8), (CargoType::Batteries, 8)],
            IndustrySpec::FizzyDrinkFactory => &[(CargoType::Cola, 8), (CargoType::Bubbles, 8)],
            IndustrySpec::FactoryTropic => &[
                (CargoType::Rubber, 8),
                (CargoType::CopperOre, 8),
                (CargoType::Wood, 8),
            ],
            IndustrySpec::FoodProcessingPlant => {
                &[(CargoType::Livestock, 8), (CargoType::Wheat, 8)]
            }
            _ => &[],
        };
        requirements.to_vec()
    }

    #[must_use]
    pub fn output_cargo(&self) -> CargoType {
        if let Some(cargo) = self.newgrf_output_cargo {
            return cargo;
        }
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

    /// Asocia datos `NewGRF` resueltos al colocar desde [`crate::industry_spec::IndustrySpecDef`].
    #[must_use]
    pub fn with_newgrf(
        mut self,
        type_id: u16,
        production_rate: u8,
        output_cargo: Option<CargoType>,
    ) -> Self {
        self.newgrf_type_id = Some(type_id);
        self.newgrf_production_rate = Some(production_rate);
        self.newgrf_secondary_production_rate = None;
        self.newgrf_output_cargo = output_cargo;
        self.newgrf_secondary_output_cargo = None;
        self.newgrf_extra_output_cargos.clear();
        self.newgrf_extra_production_rates.clear();
        self.newgrf_dynamic_cargo_types = false;
        self.newgrf_input_cargo_slots.clear();
        self.newgrf_output_cargo_slots = output_cargo.map(Some).into_iter().collect();
        self.newgrf_processing_inputs.clear();
        self.newgrf_processing_secondary_multipliers.clear();
        self.newgrf_processing_extra_multipliers.clear();
        self
    }

    /// Asocia todos los productores/insumos resueltos de un `IndustrySpecDef`.
    #[must_use]
    pub fn with_newgrf_spec(mut self, type_id: u16, def: &IndustrySpecDef) -> Self {
        let outputs = def.produced_cargo_types();
        self.newgrf_type_id = Some(type_id);
        self.newgrf_production_rate = Some(def.primary_production_rate());
        self.newgrf_secondary_production_rate = def.secondary_production_rate();
        self.newgrf_output_cargo = outputs.first().copied();
        self.newgrf_secondary_output_cargo = outputs.get(1).copied();
        self.newgrf_extra_output_cargos = outputs.iter().copied().skip(2).collect();
        self.newgrf_extra_production_rates = def.production_rates.iter().copied().skip(2).collect();
        self.newgrf_dynamic_cargo_types = false;
        self.newgrf_input_cargo_slots = def
            .accepted_cargo_labels
            .iter()
            .map(|label| crate::industry_spec::cargo_type_from_label(Some(label.as_str())))
            .collect();
        self.newgrf_output_cargo_slots = def
            .produced_cargo_labels
            .iter()
            .map(|label| crate::industry_spec::cargo_type_from_label(Some(label.as_str())))
            .collect();

        let accepted = def.accepted_cargo_types();
        let output_count = outputs.len();
        self.newgrf_processing_secondary_multipliers = accepted
            .iter()
            .enumerate()
            .map(|(input_idx, _)| {
                if output_count < 2 {
                    0
                } else {
                    def.input_multipliers
                        .get(input_idx.saturating_mul(output_count) + 1)
                        .copied()
                        .unwrap_or(0)
                }
            })
            .collect();
        self.newgrf_processing_extra_multipliers = if output_count > 2 {
            accepted
                .iter()
                .enumerate()
                .flat_map(|(input_idx, _)| {
                    (2..output_count).map(move |output_idx| {
                        def.input_multipliers
                            .get(input_idx.saturating_mul(output_count) + output_idx)
                            .copied()
                            .unwrap_or(0)
                    })
                })
                .collect()
        } else {
            Vec::new()
        };
        self.newgrf_processing_inputs = accepted
            .into_iter()
            .enumerate()
            .map(|(input_idx, cargo)| IndustryProcessingInput {
                cargo,
                batch: 8,
                multiplier: newgrf_input_multiplier(
                    &def.input_multipliers,
                    input_idx,
                    output_count,
                ),
            })
            .collect();
        self
    }
}

fn newgrf_input_multiplier(multipliers: &[u16], input_idx: usize, output_count: usize) -> u16 {
    if output_count == 0 {
        return 0;
    }
    let matrix_idx = input_idx.saturating_mul(output_count);
    multipliers
        .get(matrix_idx)
        .copied()
        .or_else(|| multipliers.get(input_idx).copied())
        .unwrap_or(256)
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
    let mut total = 0u32;
    total = total.saturating_add(transport_industry_cargo_stock(
        industry,
        stations,
        selectgoods,
        industry.output_cargo(),
        true,
    ));
    if let Some(secondary) = industry.secondary_output_cargo() {
        total = total.saturating_add(transport_industry_cargo_stock(
            industry,
            stations,
            selectgoods,
            secondary,
            false,
        ));
    }
    // CB1/CB2 v2 puede producir más de las dos salidas históricas. Esas
    // cantidades viven en `newgrf_extra_produced_cargo` y siguen el mismo
    // reparto por rating/cobertura que los stocks legacy.
    let primary = industry.output_cargo();
    let secondary = industry.secondary_output_cargo();
    for cargo in ALL_CARGO_TYPES {
        if cargo == primary || Some(cargo) == secondary {
            continue;
        }
        total = total.saturating_add(transport_industry_extra_cargo_stock(
            industry,
            stations,
            selectgoods,
            cargo,
        ));
    }
    total
}

fn transport_industry_cargo_stock(
    industry: &mut Industry,
    stations: &mut [Station],
    selectgoods: bool,
    cargo: CargoType,
    primary: bool,
) -> u32 {
    let stock = if primary {
        industry.stock
    } else {
        industry.secondary_stock
    };
    if stock == 0 {
        return 0;
    }
    let nearby = covering_output_station_indices(industry, stations);
    let eligible: Vec<usize> = nearby
        .into_iter()
        .filter(|&idx| station::can_move_goods_to_station(&stations[idx], cargo, selectgoods))
        .collect();
    if eligible.is_empty() {
        return 0;
    }
    let amount = stock.min(255);
    // Se detrae todo lo intentado, no solo lo entregado: el rating decide cuánto se pierde.
    if primary {
        industry.stock = industry.stock.saturating_sub(amount);
    } else {
        industry.secondary_stock = industry.secondary_stock.saturating_sub(amount);
    }
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

fn transport_industry_extra_cargo_stock(
    industry: &mut Industry,
    stations: &mut [Station],
    selectgoods: bool,
    cargo: CargoType,
) -> u32 {
    let stock = industry.extra_produced_cargo(cargo);
    if stock == 0 {
        return 0;
    }
    let nearby = covering_output_station_indices(industry, stations);
    let eligible: Vec<usize> = nearby
        .into_iter()
        .filter(|&idx| station::can_move_goods_to_station(&stations[idx], cargo, selectgoods))
        .collect();
    if eligible.is_empty() {
        return 0;
    }
    let amount = stock.min(255);
    let _ = industry.take_extra_produced_cargo(cargo, amount);
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

/// Acción decodificada de los callbacks `CBID_INDUSTRY_PRODUCTION_CHANGE` /
/// `CBID_INDUSTRY_MONTHLYPROD_CHANGE` (bits 0..3 del resultado `NewGRF`).
///
/// `OpenTTD` aplica las acciones sobre `prod_level`; el callback `Production256Ticks`
/// tiene otro formato y no se convierte a este enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndustryProductionAction {
    NoChange,
    Halve,
    Double,
    Close,
    Standard,
    Divide(u8),
    Multiply(u8),
    Decrease,
    Increase,
    Set(u8),
}

/// Aplica una acción de cambio de producción de industria.
///
/// Devuelve el cambio observable para noticias/telemetría. `Standard` se deja
/// para el algoritmo vanilla porque necesita el `Randomizer` y la información
/// mensual de transporte del estado.
#[must_use]
pub fn apply_industry_production_action(
    industry: &mut Industry,
    action: IndustryProductionAction,
) -> IndustryProductionChange {
    if industry.is_closing() {
        return IndustryProductionChange::None;
    }
    match action {
        IndustryProductionAction::NoChange | IndustryProductionAction::Standard => {
            IndustryProductionChange::None
        }
        IndustryProductionAction::Close => {
            industry.prod_level = PRODLEVEL_CLOSURE;
            IndustryProductionChange::Closing
        }
        IndustryProductionAction::Halve | IndustryProductionAction::Divide(1) => {
            if industry.prod_level <= PRODLEVEL_MINIMUM {
                industry.prod_level = PRODLEVEL_CLOSURE;
                IndustryProductionChange::Closing
            } else {
                let old = industry.prod_level;
                industry.prod_level = (old / 2).max(PRODLEVEL_MINIMUM);
                IndustryProductionChange::Decreased
            }
        }
        IndustryProductionAction::Double | IndustryProductionAction::Multiply(1) => {
            if industry.prod_level >= PRODLEVEL_MAXIMUM {
                IndustryProductionChange::None
            } else {
                industry.prod_level = industry.prod_level.saturating_mul(2).min(PRODLEVEL_MAXIMUM);
                IndustryProductionChange::Increased
            }
        }
        IndustryProductionAction::Divide(times) => {
            let mut changed = false;
            for _ in 0..times {
                if industry.prod_level <= PRODLEVEL_MINIMUM {
                    industry.prod_level = PRODLEVEL_CLOSURE;
                    return IndustryProductionChange::Closing;
                }
                industry.prod_level = (industry.prod_level / 2).max(PRODLEVEL_MINIMUM);
                changed = true;
            }
            if changed {
                IndustryProductionChange::Decreased
            } else {
                IndustryProductionChange::None
            }
        }
        IndustryProductionAction::Multiply(times) => {
            let old = industry.prod_level;
            for _ in 0..times {
                industry.prod_level = industry.prod_level.saturating_mul(2).min(PRODLEVEL_MAXIMUM);
            }
            if industry.prod_level > old {
                IndustryProductionChange::Increased
            } else {
                IndustryProductionChange::None
            }
        }
        IndustryProductionAction::Decrease => {
            if industry.prod_level <= PRODLEVEL_MINIMUM {
                industry.prod_level = PRODLEVEL_CLOSURE;
                IndustryProductionChange::Closing
            } else {
                industry.prod_level = industry.prod_level.saturating_sub(1);
                IndustryProductionChange::Decreased
            }
        }
        IndustryProductionAction::Increase => {
            if industry.prod_level >= PRODLEVEL_MAXIMUM {
                IndustryProductionChange::None
            } else {
                industry.prod_level = industry.prod_level.saturating_add(1).min(PRODLEVEL_MAXIMUM);
                IndustryProductionChange::Increased
            }
        }
        IndustryProductionAction::Set(level) => {
            let level = level.clamp(PRODLEVEL_MINIMUM, PRODLEVEL_MAXIMUM);
            let old = industry.prod_level;
            industry.prod_level = level;
            match level.cmp(&old) {
                std::cmp::Ordering::Greater => IndustryProductionChange::Increased,
                std::cmp::Ordering::Less => IndustryProductionChange::Decreased,
                std::cmp::Ordering::Equal => IndustryProductionChange::None,
            }
        }
    }
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

    #[test]
    fn temperate_map_creation_force_one_matches_native_industry_order() {
        let specs = IndustrySpec::temperate_map_creation_force_one();
        assert_eq!(
            specs
                .iter()
                .map(|spec| spec.native_type())
                .collect::<Vec<_>>(),
            vec![0, 1, 2, 3, 4, 6, 8, 9, 11, 18]
        );
        assert!(
            specs
                .windows(2)
                .all(|pair| pair[0].native_type() < pair[1].native_type())
        );
        assert_eq!(IndustrySpec::Bank.native_type(), 12);
    }

    #[test]
    fn map_creation_probabilities_match_vanilla_tables() {
        use crate::Climate;

        let cases: &[(Climate, &[(IndustrySpec, u8)])] = &[
            (
                Climate::Temperate,
                &[
                    (IndustrySpec::CoalMine, 8),
                    (IndustrySpec::PowerStation, 5),
                    (IndustrySpec::Sawmill, 5),
                    (IndustrySpec::Forest, 5),
                    (IndustrySpec::OilRefinery, 4),
                    (IndustrySpec::Factory, 5),
                    (IndustrySpec::SteelMill, 5),
                    (IndustrySpec::Farm, 9),
                    (IndustrySpec::OilWells, 4),
                    (IndustrySpec::Bank, 0),
                    (IndustrySpec::IronOreMine, 5),
                ],
            ),
            (
                Climate::SubArctic,
                &[
                    (IndustrySpec::CoalMine, 8),
                    (IndustrySpec::PowerStation, 5),
                    (IndustrySpec::Forest, 5),
                    (IndustrySpec::OilRefinery, 4),
                    (IndustrySpec::PrintingWorks, 5),
                    (IndustrySpec::Farm, 9),
                    (IndustrySpec::OilWells, 5),
                    (IndustrySpec::FoodProcessingPlant, 3),
                    (IndustrySpec::PaperMill, 5),
                    (IndustrySpec::GoldMine, 4),
                    (IndustrySpec::BankArcticTropic, 6),
                ],
            ),
            (
                Climate::SubTropical,
                &[
                    (IndustrySpec::OilRefinery, 4),
                    (IndustrySpec::CopperOreMine, 4),
                    (IndustrySpec::OilWells, 5),
                    (IndustrySpec::FoodProcessingPlant, 4),
                    (IndustrySpec::BankArcticTropic, 5),
                    (IndustrySpec::DiamondMine, 4),
                    (IndustrySpec::FruitPlantation, 4),
                    (IndustrySpec::RubberPlantation, 4),
                    (IndustrySpec::WaterSupply, 4),
                    (IndustrySpec::WaterTower, 8),
                    (IndustrySpec::FactoryTropic, 4),
                    (IndustrySpec::FarmTropic, 2),
                    (IndustrySpec::LumberMill, 0),
                ],
            ),
            (
                Climate::Toyland,
                &[
                    (IndustrySpec::CottonCandy, 5),
                    (IndustrySpec::CandyFactory, 5),
                    (IndustrySpec::BatteryFarm, 4),
                    (IndustrySpec::ColaWells, 5),
                    (IndustrySpec::ToyShop, 4),
                    (IndustrySpec::ToyFactory, 5),
                    (IndustrySpec::PlasticFountain, 5),
                    (IndustrySpec::FizzyDrinkFactory, 4),
                    (IndustrySpec::BubbleGenerator, 5),
                    (IndustrySpec::ToffeeQuarry, 5),
                    (IndustrySpec::SugarMine, 4),
                ],
            ),
        ];

        for (climate, expected) in cases {
            let actual = IndustrySpec::specs_for_climate(*climate)
                .iter()
                .map(|spec| (*spec, spec.map_creation_probability(*climate)))
                .collect::<Vec<_>>();
            assert_eq!(&actual, expected);
        }
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

    #[test]
    fn newgrf_production_actions_match_callback_levels() {
        let mut mine = Industry::new(TileCoord::new(2, 2), IndustryKind::CoalMine);
        assert_eq!(
            apply_industry_production_action(&mut mine, IndustryProductionAction::Halve),
            IndustryProductionChange::Decreased
        );
        assert_eq!(mine.prod_level, PRODLEVEL_DEFAULT / 2);
        assert_eq!(
            apply_industry_production_action(&mut mine, IndustryProductionAction::Halve),
            IndustryProductionChange::Decreased
        );
        assert_eq!(mine.prod_level, PRODLEVEL_MINIMUM);
        assert_eq!(
            apply_industry_production_action(&mut mine, IndustryProductionAction::Halve),
            IndustryProductionChange::Closing
        );
        assert!(mine.is_closing());

        let mut mine = Industry::new(TileCoord::new(2, 2), IndustryKind::CoalMine);
        assert_eq!(
            apply_industry_production_action(&mut mine, IndustryProductionAction::Multiply(2)),
            IndustryProductionChange::Increased
        );
        assert_eq!(mine.prod_level, PRODLEVEL_DEFAULT * 4);
        assert_eq!(
            apply_industry_production_action(&mut mine, IndustryProductionAction::Set(200)),
            IndustryProductionChange::Increased
        );
        assert_eq!(mine.prod_level, PRODLEVEL_MAXIMUM);
    }

    #[test]
    fn newgrf_industry_keeps_both_output_rates_and_cargos() {
        let def = IndustrySpecDef {
            id: 37,
            local_id: 0,
            subst_id: 0,
            override_id: None,
            layouts: Vec::new(),
            produced_cargo_indices: vec![1, 4],
            produced_cargo_labels: vec!["COAL".into(), "LVST".into()],
            accepted_cargo_indices: Vec::new(),
            accepted_cargo_labels: Vec::new(),
            production_rates: vec![15, 7],
            input_multipliers: Vec::new(),
            callback_mask: 0,
            behaviour: 0,
            cost_multiplier: 0,
            associated_badges: Vec::new(),
            newgrf_badge_translation: Vec::new(),
            name: "Dual producer".into(),
            from_newgrf: true,
            grfid: 1,
            newgrf_local_id: 0,
            newgrf_runtime: None,
        };
        let mut mine = Industry::new(TileCoord::new(0, 0), IndustryKind::CoalMine)
            .with_newgrf_spec(def.id, &def);

        assert_eq!(
            mine.produced_cargos(),
            vec![CargoType::Coal, CargoType::Livestock]
        );
        assert_eq!(mine.produce_amount(), 15);
        assert_eq!(mine.produce_secondary_amount(), 7);
        assert_eq!(mine.secondary_output_cargo(), Some(CargoType::Livestock));

        mine.produce(INDUSTRY_PRODUCE_TICKS);
        assert_eq!(mine.stock, 15);
        assert_eq!(mine.secondary_stock, 7);
    }

    #[test]
    fn newgrf_processor_applies_input_matrix_to_both_outputs() {
        let def = IndustrySpecDef {
            id: 38,
            local_id: 1,
            subst_id: 0,
            override_id: None,
            layouts: Vec::new(),
            produced_cargo_indices: vec![1, 4],
            produced_cargo_labels: vec!["COAL".into(), "LVST".into()],
            accepted_cargo_indices: vec![7],
            accepted_cargo_labels: vec!["WOOD".into()],
            production_rates: vec![0, 0],
            // One input × two outputs: 128/256 and 64/256.
            input_multipliers: vec![128, 64],
            callback_mask: 0,
            behaviour: 0,
            cost_multiplier: 0,
            associated_badges: Vec::new(),
            newgrf_badge_translation: Vec::new(),
            name: "Dual processor".into(),
            from_newgrf: true,
            grfid: 1,
            newgrf_local_id: 1,
            newgrf_runtime: None,
        };
        let pos = TileCoord::new(4, 4);
        let mut processor =
            Industry::new(pos, IndustryKind::Factory).with_newgrf_spec(def.id, &def);
        let mut stations = vec![Station::new_with_kind(
            TileCoord::new(5, 4),
            StopKind::TruckStop,
        )];
        stations[0].cargo_stock.wood = 8;

        assert_eq!(
            processor.station_input_requirements(),
            vec![(CargoType::Wood, 8)]
        );
        assert!(processor.produce_from_nearby_stations(&mut stations, INDUSTRY_PRODUCE_TICKS * 2));
        assert_eq!(processor.stock, 4);
        assert_eq!(processor.secondary_stock, 2);
        assert_eq!(stations[0].cargo_stock.wood, 0);
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
    fn factory_consumes_livestock_grain_steel_from_nearby_truck_stop() {
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
        stations[0].cargo_stock.livestock = 10;
        stations[0].cargo_stock.grain = 10;
        stations[0].cargo_stock.steel = 10;

        assert!(fact.produce_from_nearby_stations(&mut stations, 512));
        assert_eq!(fact.stock, fact.processing_output_amount());
        assert_eq!(
            stations[0].cargo_stock.livestock,
            10 - FACTORY_LIVESTOCK_INPUT
        );
        assert_eq!(stations[0].cargo_stock.grain, 10 - FACTORY_GRAIN_INPUT);
        assert_eq!(stations[0].cargo_stock.steel, 10 - FACTORY_STEEL_INPUT);
    }

    #[test]
    fn newgrf_extra_produced_cargo_reaches_station() {
        let industry_pos = TileCoord::new(4, 4);
        let mut industry = Industry::new(industry_pos, IndustryKind::CoalMine);
        industry
            .newgrf_extra_produced_cargo
            .add(CargoType::Paper, 7);
        let mut stations = vec![Station::new_with_kind(
            TileCoord::new(5, 4),
            StopKind::RailStation,
        )];

        let moved = transport_industry_goods(&mut industry, &mut stations, false);
        assert_eq!(
            moved,
            7 * (u32::from(crate::station::INITIAL_STATION_RATING) + 1) / 256
        );
        assert_eq!(industry.extra_produced_cargo(CargoType::Paper), 0);
        assert!(stations[0].cargo_stock.paper > 0);
    }

    #[test]
    fn power_station_consumes_only_coal_and_produces_no_cargo() {
        let pos = TileCoord::new(4, 4);
        let mut power_station = Industry::with_tiles_spec(
            pos,
            IndustryKind::Factory,
            IndustrySpec::PowerStation,
            vec![pos],
            0,
        );
        let mut stations = vec![Station::new_with_kind(
            TileCoord::new(5, 4),
            StopKind::RailStation,
        )];
        stations[0].cargo_stock.coal = 16;

        assert!(power_station.produce_from_nearby_stations(&mut stations, 512));
        assert_eq!(power_station.life_type(), IndustryLifeType::BlackHole);
        assert_eq!(power_station.stock, 0);
        assert_eq!(stations[0].cargo_stock.coal, 8);
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
    fn steel_mill_consumes_iron_ore_for_steel() {
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

        assert!(mill.produce_from_nearby_stations(&mut stations, 512));
        assert_eq!(mill.stock, 8);
        assert_eq!(mill.output_cargo(), CargoType::Steel);
        assert_eq!(stations[0].cargo_stock.iron_ore, 8);
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
        stations[0].cargo_stock.livestock = FACTORY_LIVESTOCK_INPUT;
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

    #[test]
    fn toyland_cotton_candy_is_not_wood_alias() {
        assert_eq!(
            IndustrySpec::CottonCandy.output_cargo(),
            CargoType::CottonCandy
        );
        assert_eq!(
            IndustrySpec::BatteryFarm.output_cargo(),
            CargoType::Batteries
        );
        assert_eq!(IndustrySpec::SugarMine.output_cargo(), CargoType::Sugar);
        assert_eq!(
            IndustrySpec::CandyFactory.accepted_cargos(),
            &[CargoType::Sugar, CargoType::Toffee, CargoType::CottonCandy]
        );
    }

    #[test]
    fn arctic_paper_chain_io() {
        assert_eq!(
            IndustrySpec::PaperMill.accepted_cargos(),
            &[CargoType::Wood]
        );
        assert_eq!(IndustrySpec::PaperMill.output_cargo(), CargoType::Paper);
        assert_eq!(
            IndustrySpec::PrintingWorks.accepted_cargos(),
            &[CargoType::Paper]
        );
        assert_eq!(IndustrySpec::GoldMine.output_cargo(), CargoType::Gold);
    }

    #[test]
    fn tropic_factory_accepts_rubber_copper_wood() {
        assert_eq!(
            IndustrySpec::CopperOreMine.output_cargo(),
            CargoType::CopperOre
        );
        assert_eq!(
            IndustrySpec::FactoryTropic.accepted_cargos(),
            &[CargoType::Rubber, CargoType::CopperOre, CargoType::Wood]
        );
    }
}
