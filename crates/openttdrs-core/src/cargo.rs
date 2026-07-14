//! Tipos de carga temperate (`OpenTTD` `cargo_const.h` / clima templado).
//!
//! Temperate expone **11 cargos activos** (el slot 11 del array original es void).

use serde::{Deserialize, Serialize};

/// Los 11 cargos del clima templado (orden = bit `OpenTTD` 0..10).
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

/// Alias estable para iterar todos los cargos del port (hoy = temperate).
pub const ALL_CARGO_TYPES: [CargoType; 11] = TEMPERATE_CARGO_TYPES;

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
        }
    }

    /// Mayor cantidad en espera entre tipos de carga (camión/tren).
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
    pub const fn is_freight(self) -> bool {
        !matches!(self, Self::Passengers | Self::Mail)
    }

    #[must_use]
    pub const fn is_town_cargo(self) -> bool {
        matches!(self, Self::Passengers | Self::Mail)
    }

    /// Índice temperate `OpenTTD` (`CargoSpec` bit 0..10).
    #[must_use]
    pub const fn temperate_id(self) -> u8 {
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
        }
    }

    /// Inverso de [`Self::temperate_id`].
    #[must_use]
    pub const fn from_temperate_id(id: u8) -> Option<Self> {
        Some(match id {
            0 => Self::Passengers,
            1 => Self::Coal,
            2 => Self::Mail,
            3 => Self::Oil,
            4 => Self::Livestock,
            5 => Self::Goods,
            6 => Self::Grain,
            7 => Self::Wood,
            8 => Self::IronOre,
            9 => Self::Steel,
            10 => Self::Valuables,
            _ => return None,
        })
    }

    /// Nombre corto para UI / noticias.
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
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn temperate_table_has_eleven_cargos() {
        assert_eq!(TEMPERATE_CARGO_TYPES.len(), 11);
        for (i, cargo) in TEMPERATE_CARGO_TYPES.iter().enumerate() {
            assert_eq!(cargo.temperate_id() as usize, i);
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
}
