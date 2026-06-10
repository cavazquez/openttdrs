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

    /// Mayor cantidad en espera entre tipos de carga (camión/tren).
    #[must_use]
    pub fn pick_freight_to_load(self, preferred: Option<CargoType>) -> Option<CargoType> {
        if let Some(cargo) = preferred {
            if cargo.is_freight() && self.get(cargo) > 0 {
                return Some(cargo);
            }
            return None;
        }

        const FREIGHT: [CargoType; 4] = [
            CargoType::Coal,
            CargoType::Wood,
            CargoType::Oil,
            CargoType::Goods,
        ];
        FREIGHT
            .iter()
            .copied()
            .filter(|cargo| self.get(*cargo) > 0)
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
}

#[cfg(test)]
mod tests {
    use super::*;

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
            ..Default::default()
        };
        assert_eq!(stock.pick_freight_to_load(None), Some(CargoType::Wood));
    }
}
