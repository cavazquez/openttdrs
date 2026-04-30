use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CargoType {
    Passengers,
    Mail,
    Goods,
    Coal,
    Wood,
    Oil,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CargoStock {
    pub passengers: u32,
    pub mail: u32,
    pub goods: u32,
    pub coal: u32,
    pub wood: u32,
    pub oil: u32,
}

impl CargoStock {
    #[must_use]
    pub const fn get(self, cargo: CargoType) -> u32 {
        match cargo {
            CargoType::Passengers => self.passengers,
            CargoType::Mail => self.mail,
            CargoType::Goods => self.goods,
            CargoType::Coal => self.coal,
            CargoType::Wood => self.wood,
            CargoType::Oil => self.oil,
        }
    }

    pub fn add(&mut self, cargo: CargoType, amount: u32) {
        let slot = match cargo {
            CargoType::Passengers => &mut self.passengers,
            CargoType::Mail => &mut self.mail,
            CargoType::Goods => &mut self.goods,
            CargoType::Coal => &mut self.coal,
            CargoType::Wood => &mut self.wood,
            CargoType::Oil => &mut self.oil,
        };
        *slot = slot.saturating_add(amount);
    }

    #[must_use]
    pub fn take(&mut self, cargo: CargoType, amount: u32) -> u32 {
        let slot = match cargo {
            CargoType::Passengers => &mut self.passengers,
            CargoType::Mail => &mut self.mail,
            CargoType::Goods => &mut self.goods,
            CargoType::Coal => &mut self.coal,
            CargoType::Wood => &mut self.wood,
            CargoType::Oil => &mut self.oil,
        };
        let taken = (*slot).min(amount);
        *slot -= taken;
        taken
    }
}
