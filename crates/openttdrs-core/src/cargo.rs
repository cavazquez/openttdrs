//! Tipos de carga vanilla por clima (`OpenTTD` `cargo_const.h` / `cargo_type.h`).
//!
//! Cada clima expone hasta 12 slots (`NUM_ORIGINAL_CARGO`); los labels no se
//! aliasan entre climas (#224). Los 11 temperate históricos conservan
//! [`CargoType::cargo_id`] 0..10 para migrar saves propios.

use serde::{Deserialize, Serialize};

use crate::Climate;

/// Número de slots vanilla por landscape (`NUM_ORIGINAL_CARGO`).
pub const NUM_ORIGINAL_CARGO: usize = 12;

/// Cantidad de cargos vanilla distintos (todos los climas).
pub const VANILLA_CARGO_COUNT: usize = 31;

/// Los 11 cargos activos del clima templado (slot 11 es void).
pub const TEMPERATE_CARGO_TYPES: [CargoType; 11] = [
    CargoType::Passengers,
    CargoType::Coal,
    CargoType::Mail,
    CargoType::Oil,
    CargoType::Livestock,
    CargoType::Goods,
    CargoType::Grain,
    CargoType::Wood,
    CargoType::IronOre,
    CargoType::Steel,
    CargoType::Valuables,
];

/// Catálogo arctic (12 slots; el 8 es void en upstream).
pub const ARCTIC_CARGO_TYPES: [CargoType; 11] = [
    CargoType::Passengers,
    CargoType::Coal,
    CargoType::Mail,
    CargoType::Oil,
    CargoType::Livestock,
    CargoType::Goods,
    CargoType::Wheat,
    CargoType::Wood,
    CargoType::Paper,
    CargoType::Gold,
    CargoType::Food,
];

/// Catálogo tropic (12 activos).
pub const TROPIC_CARGO_TYPES: [CargoType; 12] = [
    CargoType::Passengers,
    CargoType::Rubber,
    CargoType::Mail,
    CargoType::Oil,
    CargoType::Fruit,
    CargoType::Goods,
    CargoType::Maize,
    CargoType::Wood,
    CargoType::CopperOre,
    CargoType::Water,
    CargoType::Diamonds,
    CargoType::Food,
];

/// Catálogo Toyland (12 activos).
pub const TOYLAND_CARGO_TYPES: [CargoType; 12] = [
    CargoType::Passengers,
    CargoType::Sugar,
    CargoType::Mail,
    CargoType::Toys,
    CargoType::Batteries,
    CargoType::Candy,
    CargoType::Toffee,
    CargoType::Cola,
    CargoType::CottonCandy,
    CargoType::Bubbles,
    CargoType::Plastic,
    CargoType::FizzyDrinks,
];

/// Todos los cargos vanilla (índice = [`CargoType::cargo_id`]).
pub const ALL_CARGO_TYPES: [CargoType; VANILLA_CARGO_COUNT] = [
    CargoType::Passengers,
    CargoType::Coal,
    CargoType::Mail,
    CargoType::Oil,
    CargoType::Livestock,
    CargoType::Goods,
    CargoType::Grain,
    CargoType::Wood,
    CargoType::IronOre,
    CargoType::Steel,
    CargoType::Valuables,
    CargoType::Wheat,
    CargoType::Paper,
    CargoType::Gold,
    CargoType::Food,
    CargoType::Rubber,
    CargoType::Fruit,
    CargoType::Maize,
    CargoType::CopperOre,
    CargoType::Water,
    CargoType::Diamonds,
    CargoType::Sugar,
    CargoType::Toys,
    CargoType::Batteries,
    CargoType::Candy,
    CargoType::Toffee,
    CargoType::Cola,
    CargoType::CottonCandy,
    CargoType::Bubbles,
    CargoType::Plastic,
    CargoType::FizzyDrinks,
];

/// Ajustes de órdenes relacionados con carga (`_settings_game.order`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrderSettings {
    /// Truncar carga en estación a los 255 días sin recogida (`selectgoods`).
    #[serde(default = "default_selectgoods")]
    pub selectgoods: bool,
}

const fn default_selectgoods() -> bool {
    true
}

impl Default for OrderSettings {
    fn default() -> Self {
        Self { selectgoods: true }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CargoType {
    Passengers,
    Coal,
    Mail,
    Oil,
    Livestock,
    Goods,
    Grain,
    Wood,
    IronOre,
    Steel,
    Valuables,
    Wheat,
    Paper,
    Gold,
    Food,
    Rubber,
    Fruit,
    Maize,
    CopperOre,
    Water,
    Diamonds,
    Sugar,
    Toys,
    Batteries,
    Candy,
    Toffee,
    Cola,
    CottonCandy,
    Bubbles,
    Plastic,
    FizzyDrinks,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CargoStock {
    pub passengers: u32,
    pub coal: u32,
    pub mail: u32,
    pub oil: u32,
    pub livestock: u32,
    pub goods: u32,
    pub grain: u32,
    pub wood: u32,
    pub iron_ore: u32,
    pub steel: u32,
    pub valuables: u32,
    #[serde(default)]
    pub wheat: u32,
    #[serde(default)]
    pub paper: u32,
    #[serde(default)]
    pub gold: u32,
    #[serde(default)]
    pub food: u32,
    #[serde(default)]
    pub rubber: u32,
    #[serde(default)]
    pub fruit: u32,
    #[serde(default)]
    pub maize: u32,
    #[serde(default)]
    pub copper_ore: u32,
    #[serde(default)]
    pub water: u32,
    #[serde(default)]
    pub diamonds: u32,
    #[serde(default)]
    pub sugar: u32,
    #[serde(default)]
    pub toys: u32,
    #[serde(default)]
    pub batteries: u32,
    #[serde(default)]
    pub candy: u32,
    #[serde(default)]
    pub toffee: u32,
    #[serde(default)]
    pub cola: u32,
    #[serde(default)]
    pub cotton_candy: u32,
    #[serde(default)]
    pub bubbles: u32,
    #[serde(default)]
    pub plastic: u32,
    #[serde(default)]
    pub fizzy_drinks: u32,
}

impl CargoStock {
    #[must_use]
    pub const fn get(self, cargo: CargoType) -> u32 {
        match cargo {
            CargoType::Passengers => self.passengers,
            CargoType::Coal => self.coal,
            CargoType::Mail => self.mail,
            CargoType::Oil => self.oil,
            CargoType::Livestock => self.livestock,
            CargoType::Goods => self.goods,
            CargoType::Grain => self.grain,
            CargoType::Wood => self.wood,
            CargoType::IronOre => self.iron_ore,
            CargoType::Steel => self.steel,
            CargoType::Valuables => self.valuables,
            CargoType::Wheat => self.wheat,
            CargoType::Paper => self.paper,
            CargoType::Gold => self.gold,
            CargoType::Food => self.food,
            CargoType::Rubber => self.rubber,
            CargoType::Fruit => self.fruit,
            CargoType::Maize => self.maize,
            CargoType::CopperOre => self.copper_ore,
            CargoType::Water => self.water,
            CargoType::Diamonds => self.diamonds,
            CargoType::Sugar => self.sugar,
            CargoType::Toys => self.toys,
            CargoType::Batteries => self.batteries,
            CargoType::Candy => self.candy,
            CargoType::Toffee => self.toffee,
            CargoType::Cola => self.cola,
            CargoType::CottonCandy => self.cotton_candy,
            CargoType::Bubbles => self.bubbles,
            CargoType::Plastic => self.plastic,
            CargoType::FizzyDrinks => self.fizzy_drinks,
        }
    }

    pub fn add(&mut self, cargo: CargoType, amount: u32) {
        let slot = self.slot_mut(cargo);
        *slot = slot.saturating_add(amount);
    }

    #[must_use]
    pub fn take(&mut self, cargo: CargoType, amount: u32) -> u32 {
        let slot = self.slot_mut(cargo);
        let taken = (*slot).min(amount);
        *slot -= taken;
        taken
    }

    fn slot_mut(&mut self, cargo: CargoType) -> &mut u32 {
        match cargo {
            CargoType::Passengers => &mut self.passengers,
            CargoType::Coal => &mut self.coal,
            CargoType::Mail => &mut self.mail,
            CargoType::Oil => &mut self.oil,
            CargoType::Livestock => &mut self.livestock,
            CargoType::Goods => &mut self.goods,
            CargoType::Grain => &mut self.grain,
            CargoType::Wood => &mut self.wood,
            CargoType::IronOre => &mut self.iron_ore,
            CargoType::Steel => &mut self.steel,
            CargoType::Valuables => &mut self.valuables,
            CargoType::Wheat => &mut self.wheat,
            CargoType::Paper => &mut self.paper,
            CargoType::Gold => &mut self.gold,
            CargoType::Food => &mut self.food,
            CargoType::Rubber => &mut self.rubber,
            CargoType::Fruit => &mut self.fruit,
            CargoType::Maize => &mut self.maize,
            CargoType::CopperOre => &mut self.copper_ore,
            CargoType::Water => &mut self.water,
            CargoType::Diamonds => &mut self.diamonds,
            CargoType::Sugar => &mut self.sugar,
            CargoType::Toys => &mut self.toys,
            CargoType::Batteries => &mut self.batteries,
            CargoType::Candy => &mut self.candy,
            CargoType::Toffee => &mut self.toffee,
            CargoType::Cola => &mut self.cola,
            CargoType::CottonCandy => &mut self.cotton_candy,
            CargoType::Bubbles => &mut self.bubbles,
            CargoType::Plastic => &mut self.plastic,
            CargoType::FizzyDrinks => &mut self.fizzy_drinks,
        }
    }

    #[must_use]
    pub fn pick_freight_to_load(self, preferred: Option<CargoType>) -> Option<CargoType> {
        if let Some(cargo) = preferred {
            if cargo.is_freight() && self.get(cargo) > 0 {
                return Some(cargo);
            }
            return None;
        }
        ALL_CARGO_TYPES
            .iter()
            .copied()
            .filter(|cargo| cargo.is_freight() && self.get(*cargo) > 0)
            .max_by_key(|cargo| self.get(*cargo))
    }
}

impl CargoType {
    #[must_use]
    pub const fn cargo_id(self) -> u8 {
        match self {
            Self::Passengers => 0,
            Self::Coal => 1,
            Self::Mail => 2,
            Self::Oil => 3,
            Self::Livestock => 4,
            Self::Goods => 5,
            Self::Grain => 6,
            Self::Wood => 7,
            Self::IronOre => 8,
            Self::Steel => 9,
            Self::Valuables => 10,
            Self::Wheat => 11,
            Self::Paper => 12,
            Self::Gold => 13,
            Self::Food => 14,
            Self::Rubber => 15,
            Self::Fruit => 16,
            Self::Maize => 17,
            Self::CopperOre => 18,
            Self::Water => 19,
            Self::Diamonds => 20,
            Self::Sugar => 21,
            Self::Toys => 22,
            Self::Batteries => 23,
            Self::Candy => 24,
            Self::Toffee => 25,
            Self::Cola => 26,
            Self::CottonCandy => 27,
            Self::Bubbles => 28,
            Self::Plastic => 29,
            Self::FizzyDrinks => 30,
        }
    }

    #[must_use]
    pub const fn from_cargo_id(id: u8) -> Option<Self> {
        match id {
            0 => Some(Self::Passengers),
            1 => Some(Self::Coal),
            2 => Some(Self::Mail),
            3 => Some(Self::Oil),
            4 => Some(Self::Livestock),
            5 => Some(Self::Goods),
            6 => Some(Self::Grain),
            7 => Some(Self::Wood),
            8 => Some(Self::IronOre),
            9 => Some(Self::Steel),
            10 => Some(Self::Valuables),
            11 => Some(Self::Wheat),
            12 => Some(Self::Paper),
            13 => Some(Self::Gold),
            14 => Some(Self::Food),
            15 => Some(Self::Rubber),
            16 => Some(Self::Fruit),
            17 => Some(Self::Maize),
            18 => Some(Self::CopperOre),
            19 => Some(Self::Water),
            20 => Some(Self::Diamonds),
            21 => Some(Self::Sugar),
            22 => Some(Self::Toys),
            23 => Some(Self::Batteries),
            24 => Some(Self::Candy),
            25 => Some(Self::Toffee),
            26 => Some(Self::Cola),
            27 => Some(Self::CottonCandy),
            28 => Some(Self::Bubbles),
            29 => Some(Self::Plastic),
            30 => Some(Self::FizzyDrinks),
            _ => None,
        }
    }

    #[must_use]
    pub const fn temperate_id(self) -> u8 {
        self.cargo_id()
    }

    #[must_use]
    pub const fn from_temperate_id(id: u8) -> Option<Self> {
        if id <= 10 {
            Self::from_cargo_id(id)
        } else {
            None
        }
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Passengers => "PASS",
            Self::Coal => "COAL",
            Self::Mail => "MAIL",
            Self::Oil => "OIL_",
            Self::Livestock => "LVST",
            Self::Goods => "GOOD",
            Self::Grain => "GRAI",
            Self::Wood => "WOOD",
            Self::IronOre => "IORE",
            Self::Steel => "STEL",
            Self::Valuables => "VALU",
            Self::Wheat => "WHEA",
            Self::Paper => "PAPR",
            Self::Gold => "GOLD",
            Self::Food => "FOOD",
            Self::Rubber => "RUBR",
            Self::Fruit => "FRUT",
            Self::Maize => "MAIZ",
            Self::CopperOre => "CORE",
            Self::Water => "WATR",
            Self::Diamonds => "DIAM",
            Self::Sugar => "SUGR",
            Self::Toys => "TOYS",
            Self::Batteries => "BATT",
            Self::Candy => "SWET",
            Self::Toffee => "TOFF",
            Self::Cola => "COLA",
            Self::CottonCandy => "CTCD",
            Self::Bubbles => "BUBL",
            Self::Plastic => "PLST",
            Self::FizzyDrinks => "FZDR",
        }
    }

    #[must_use]
    pub const fn label_u32(self) -> u32 {
        let b = self.label().as_bytes();
        u32::from_be_bytes([b[0], b[1], b[2], b[3]])
    }

    #[must_use]
    pub fn from_label(label: &str) -> Option<Self> {
        let t = label.trim();
        ALL_CARGO_TYPES
            .iter()
            .copied()
            .find(|c| c.label().eq_ignore_ascii_case(t))
    }

    #[must_use]
    pub fn for_climate(climate: Climate) -> &'static [CargoType] {
        match climate {
            Climate::Temperate => &TEMPERATE_CARGO_TYPES,
            Climate::SubArctic => &ARCTIC_CARGO_TYPES,
            Climate::SubTropical => &TROPIC_CARGO_TYPES,
            Climate::Toyland => &TOYLAND_CARGO_TYPES,
        }
    }

    #[must_use]
    pub const fn from_climate_slot(climate: Climate, slot: u8) -> Option<Self> {
        match climate {
            Climate::Temperate => match slot {
                0 => Some(Self::Passengers),
                1 => Some(Self::Coal),
                2 => Some(Self::Mail),
                3 => Some(Self::Oil),
                4 => Some(Self::Livestock),
                5 => Some(Self::Goods),
                6 => Some(Self::Grain),
                7 => Some(Self::Wood),
                8 => Some(Self::IronOre),
                9 => Some(Self::Steel),
                10 => Some(Self::Valuables),
                _ => None,
            },
            Climate::SubArctic => match slot {
                0 => Some(Self::Passengers),
                1 => Some(Self::Coal),
                2 => Some(Self::Mail),
                3 => Some(Self::Oil),
                4 => Some(Self::Livestock),
                5 => Some(Self::Goods),
                6 => Some(Self::Wheat),
                7 => Some(Self::Wood),
                // Slot 8 vacío en ártico (OpenTTD).
                9 => Some(Self::Paper),
                10 => Some(Self::Gold),
                11 => Some(Self::Food),
                _ => None,
            },
            Climate::SubTropical => match slot {
                0 => Some(Self::Passengers),
                1 => Some(Self::Rubber),
                2 => Some(Self::Mail),
                3 => Some(Self::Oil),
                4 => Some(Self::Fruit),
                5 => Some(Self::Goods),
                6 => Some(Self::Maize),
                7 => Some(Self::Wood),
                8 => Some(Self::CopperOre),
                9 => Some(Self::Water),
                10 => Some(Self::Diamonds),
                11 => Some(Self::Food),
                _ => None,
            },
            Climate::Toyland => match slot {
                0 => Some(Self::Passengers),
                1 => Some(Self::Sugar),
                2 => Some(Self::Mail),
                3 => Some(Self::Toys),
                4 => Some(Self::Batteries),
                5 => Some(Self::Candy),
                6 => Some(Self::Toffee),
                7 => Some(Self::Cola),
                8 => Some(Self::CottonCandy),
                9 => Some(Self::Bubbles),
                10 => Some(Self::Plastic),
                11 => Some(Self::FizzyDrinks),
                _ => None,
            },
        }
    }

    #[must_use]
    pub const fn climate_slot(self, climate: Climate) -> Option<u8> {
        match climate {
            Climate::Temperate => match self {
                Self::Passengers => Some(0),
                Self::Coal => Some(1),
                Self::Mail => Some(2),
                Self::Oil => Some(3),
                Self::Livestock => Some(4),
                Self::Goods => Some(5),
                Self::Grain => Some(6),
                Self::Wood => Some(7),
                Self::IronOre => Some(8),
                Self::Steel => Some(9),
                Self::Valuables => Some(10),
                _ => None,
            },
            Climate::SubArctic => match self {
                Self::Passengers => Some(0),
                Self::Coal => Some(1),
                Self::Mail => Some(2),
                Self::Oil => Some(3),
                Self::Livestock => Some(4),
                Self::Goods => Some(5),
                Self::Wheat => Some(6),
                Self::Wood => Some(7),
                Self::Paper => Some(9),
                Self::Gold => Some(10),
                Self::Food => Some(11),
                _ => None,
            },
            Climate::SubTropical => match self {
                Self::Passengers => Some(0),
                Self::Rubber => Some(1),
                Self::Mail => Some(2),
                Self::Oil => Some(3),
                Self::Fruit => Some(4),
                Self::Goods => Some(5),
                Self::Maize => Some(6),
                Self::Wood => Some(7),
                Self::CopperOre => Some(8),
                Self::Water => Some(9),
                Self::Diamonds => Some(10),
                Self::Food => Some(11),
                _ => None,
            },
            Climate::Toyland => match self {
                Self::Passengers => Some(0),
                Self::Sugar => Some(1),
                Self::Mail => Some(2),
                Self::Toys => Some(3),
                Self::Batteries => Some(4),
                Self::Candy => Some(5),
                Self::Toffee => Some(6),
                Self::Cola => Some(7),
                Self::CottonCandy => Some(8),
                Self::Bubbles => Some(9),
                Self::Plastic => Some(10),
                Self::FizzyDrinks => Some(11),
                _ => None,
            },
        }
    }

    #[must_use]
    pub const fn is_freight(self) -> bool {
        !matches!(self, Self::Passengers | Self::Mail)
    }

    #[must_use]
    pub const fn is_town_cargo(self) -> bool {
        matches!(self, Self::Passengers | Self::Mail)
    }

    #[must_use]
    pub const fn display_name(self) -> &'static str {
        match self {
            Self::Passengers => "pasajeros",
            Self::Coal => "carbón",
            Self::Mail => "correo",
            Self::Oil => "petróleo",
            Self::Livestock => "ganado",
            Self::Goods => "mercancías",
            Self::Grain => "grano",
            Self::Wood => "madera",
            Self::IronOre => "mineral de hierro",
            Self::Steel => "acero",
            Self::Valuables => "objetos de valor",
            Self::Wheat => "trigo",
            Self::Paper => "papel",
            Self::Gold => "oro",
            Self::Food => "comida",
            Self::Rubber => "caucho",
            Self::Fruit => "fruta",
            Self::Maize => "maíz",
            Self::CopperOre => "mineral de cobre",
            Self::Water => "agua",
            Self::Diamonds => "diamantes",
            Self::Sugar => "azúcar",
            Self::Toys => "juguetes",
            Self::Batteries => "baterías",
            Self::Candy => "caramelos",
            Self::Toffee => "toffee",
            Self::Cola => "cola",
            Self::CottonCandy => "algodón de azúcar",
            Self::Bubbles => "burbujas",
            Self::Plastic => "plástico",
            Self::FizzyDrinks => "refrescos",
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn temperate_table_has_eleven_cargos() {
        assert_eq!(TEMPERATE_CARGO_TYPES.len(), 11);
        for (i, cargo) in TEMPERATE_CARGO_TYPES.iter().enumerate() {
            assert_eq!(cargo.temperate_id() as usize, i);
            let slot = u8::try_from(i).expect("temperate slot fits u8");
            assert_eq!(cargo.climate_slot(Climate::Temperate), Some(slot));
        }
    }

    #[test]
    fn each_climate_matches_openttd_15_3_catalog() {
        assert_eq!(CargoType::for_climate(Climate::Temperate).len(), 11);
        assert_eq!(CargoType::for_climate(Climate::SubArctic).len(), 11);
        assert_eq!(CargoType::for_climate(Climate::SubTropical).len(), 12);
        assert_eq!(CargoType::for_climate(Climate::Toyland).len(), 12);
        assert_eq!(
            CargoType::from_climate_slot(Climate::Toyland, 8),
            Some(CargoType::CottonCandy)
        );
        assert_eq!(
            CargoType::from_climate_slot(Climate::SubArctic, 10),
            Some(CargoType::Gold)
        );
        assert_eq!(
            CargoType::from_climate_slot(Climate::SubTropical, 1),
            Some(CargoType::Rubber)
        );
        assert_eq!(CargoType::from_climate_slot(Climate::SubArctic, 8), None);
        assert_ne!(
            CargoType::from_climate_slot(Climate::Toyland, 1),
            Some(CargoType::Coal)
        );
    }

    #[test]
    fn labels_are_unique_fourcc() {
        let mut seen = std::collections::HashSet::new();
        for cargo in ALL_CARGO_TYPES {
            assert_eq!(cargo.label().len(), 4);
            assert!(seen.insert(cargo.label_u32()), "dup {}", cargo.label());
            assert_eq!(CargoType::from_label(cargo.label()), Some(cargo));
        }
    }

    #[test]
    fn pick_freight_honors_preferred_type() {
        let stock = CargoStock {
            coal: 5,
            wood: 20,
            ..Default::default()
        };
        assert_eq!(
            stock.pick_freight_to_load(Some(CargoType::Coal)),
            Some(CargoType::Coal)
        );
        assert_eq!(stock.pick_freight_to_load(Some(CargoType::Oil)), None);
    }

    #[test]
    fn pick_freight_without_preference_takes_largest_waiting() {
        let stock = CargoStock {
            coal: 5,
            wood: 20,
            goods: 12,
            iron_ore: 40,
            ..Default::default()
        };
        assert_eq!(stock.pick_freight_to_load(None), Some(CargoType::IronOre));
    }

    #[test]
    fn stock_keeps_toyland_identity() {
        let mut stock = CargoStock::default();
        stock.add(CargoType::CottonCandy, 9);
        stock.add(CargoType::Batteries, 3);
        assert_eq!(stock.get(CargoType::CottonCandy), 9);
        assert_eq!(stock.get(CargoType::Wood), 0);
        assert_eq!(stock.get(CargoType::Coal), 0);
        assert_eq!(stock.get(CargoType::Batteries), 3);
    }
}
