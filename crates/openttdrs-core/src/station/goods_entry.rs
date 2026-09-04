//! Estado persistente por tipo de carga en una estación (`GoodsEntry`, `station_base.h:167`).

use serde::{Deserialize, Serialize};

use crate::cargo::{ALL_CARGO_TYPES, CargoType, MAX_CARGO_ID};

/// Rating con el que nace cada entrada de carga (`INITIAL_STATION_RATING`).
pub const INITIAL_STATION_RATING: u8 = 175;

/// Paso máximo de convergencia por barrido (`Clamp(rating - or_, -2, 2)`).
pub const STATION_RATING_MAX_STEP: i16 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct GoodsEntry {
    #[serde(default = "default_rating")]
    pub rating: u8,
    #[serde(default)]
    pub has_rating: bool,
    #[serde(default)]
    pub last_speed: u8,
    #[serde(default = "default_last_age")]
    pub last_age: u8,
    #[serde(default)]
    pub max_waiting_cargo: u32,
    #[serde(default)]
    pub amount_fract: u8,
    /// Bits `GoodsEntry::State` de `OpenTTD` usados por `var 69`:
    /// `EverAccepted`, `LastMonth`, `CurrentMonth` y `AcceptedBigtick`.
    #[serde(default)]
    pub newgrf_state: u8,
}

const fn default_rating() -> u8 {
    INITIAL_STATION_RATING
}

const fn default_last_age() -> u8 {
    255
}

impl Default for GoodsEntry {
    fn default() -> Self {
        Self {
            rating: INITIAL_STATION_RATING,
            has_rating: false,
            last_speed: 0,
            last_age: 255,
            max_waiting_cargo: 0,
            amount_fract: 0,
            newgrf_state: 0,
        }
    }
}

impl GoodsEntry {
    #[must_use]
    pub const fn has_vehicle_ever_tried_loading(&self) -> bool {
        self.last_speed != 0
    }

    /// Representación compacta de `GoodsEntry::ConvertState` (`var 69`).
    #[must_use]
    pub const fn convert_state(self) -> u8 {
        self.newgrf_state & 0x0F
    }

    /// Marca una entrega final para los tres estados que `OpenTTD` actualiza
    /// durante el mes/intervalo actual.
    pub fn mark_final_delivery(&mut self) {
        self.newgrf_state |= 0b1101;
    }

    /// Rota los flags mensuales al comenzar un nuevo mes.
    pub fn roll_newgrf_month(&mut self) {
        let current = self.newgrf_state & (1 << 2);
        self.newgrf_state = (self.newgrf_state & 1) | (current >> 1);
    }

    /// Limpia el flag del intervalo de aceptación sin tocar el historial.
    pub fn clear_newgrf_bigtick(&mut self) {
        self.newgrf_state &= !(1 << 3);
    }

    pub fn converge_rating_towards(&mut self, target: i16) -> u8 {
        let current = i16::from(self.rating);
        let step = (target - current).clamp(-STATION_RATING_MAX_STEP, STATION_RATING_MAX_STEP);
        self.rating = u8::try_from((current + step).clamp(0, 255)).unwrap_or(self.rating);
        self.rating
    }
}

/// Entradas de carga de una estación. Acepta arrays legacy de 11 al deserializar
/// y hasta el límite de IDs custom que mantiene el runtime.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StationGoods {
    entries: [GoodsEntry; (MAX_CARGO_ID as usize) + 1],
}

impl Default for StationGoods {
    fn default() -> Self {
        Self {
            entries: [GoodsEntry::default(); (MAX_CARGO_ID as usize) + 1],
        }
    }
}

impl Serialize for StationGoods {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.entries.as_slice().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for StationGoods {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let mut v = Vec::<GoodsEntry>::deserialize(deserializer)?;
        if v.len() > (MAX_CARGO_ID as usize) + 1 {
            return Err(serde::de::Error::custom(format!(
                "StationGoods: {} entradas > {}",
                v.len(),
                (MAX_CARGO_ID as usize) + 1,
            )));
        }
        v.resize((MAX_CARGO_ID as usize) + 1, GoodsEntry::default());
        let entries: [GoodsEntry; (MAX_CARGO_ID as usize) + 1] = v
            .try_into()
            .map_err(|_| serde::de::Error::custom("StationGoods pad failed"))?;
        Ok(Self { entries })
    }
}

impl StationGoods {
    #[must_use]
    pub fn get(&self, cargo: CargoType) -> &GoodsEntry {
        &self.entries[usize::from(cargo.cargo_id()).min(self.entries.len() - 1)]
    }

    pub fn get_mut(&mut self, cargo: CargoType) -> &mut GoodsEntry {
        let index = usize::from(cargo.cargo_id()).min(self.entries.len() - 1);
        &mut self.entries[index]
    }

    #[must_use]
    pub fn rating(&self, cargo: CargoType) -> u8 {
        self.get(cargo).rating
    }

    pub fn iter(&self) -> impl Iterator<Item = (CargoType, &GoodsEntry)> {
        ALL_CARGO_TYPES
            .into_iter()
            .map(|cargo| (cargo, self.get(cargo)))
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn new_entry_starts_at_initial_rating() {
        let goods = StationGoods::default();
        assert_eq!(goods.rating(CargoType::Coal), INITIAL_STATION_RATING);
        assert!(!goods.get(CargoType::Coal).has_rating);
    }

    #[test]
    fn entries_are_independent_per_cargo() {
        let mut goods = StationGoods::default();
        goods.get_mut(CargoType::Coal).rating = 10;
        assert_eq!(goods.rating(CargoType::Coal), 10);
        assert_eq!(goods.rating(CargoType::Wood), INITIAL_STATION_RATING);
        goods.get_mut(CargoType::CottonCandy).rating = 40;
        assert_eq!(goods.rating(CargoType::CottonCandy), 40);
    }

    #[test]
    fn legacy_eleven_entry_array_deserializes() {
        let legacy = vec![GoodsEntry::default(); 11];
        let json = serde_json::to_string(&legacy).expect("ser");
        let goods: StationGoods = serde_json::from_str(&json).expect("de");
        assert_eq!(goods.rating(CargoType::Valuables), INITIAL_STATION_RATING);
        assert_eq!(goods.rating(CargoType::Food), INITIAL_STATION_RATING);
    }

    #[test]
    fn rating_converges_two_points_at_a_time() {
        let mut entry = GoodsEntry::default();
        assert_eq!(entry.converge_rating_towards(255), 177);
        assert_eq!(entry.converge_rating_towards(255), 179);
        assert_eq!(entry.converge_rating_towards(0), 177);
        entry.rating = 1;
        assert_eq!(entry.converge_rating_towards(0), 0);
    }

    #[test]
    fn convert_state_matches_newgrf_var_69_bits() {
        let entry = GoodsEntry {
            newgrf_state: 0x0F,
            ..GoodsEntry::default()
        };
        assert_eq!(entry.convert_state(), 0x0F);
    }

    #[test]
    fn newgrf_state_rolls_month_and_acceptance_interval() {
        let mut entry = GoodsEntry {
            newgrf_state: 0x0F,
            ..GoodsEntry::default()
        };
        entry.roll_newgrf_month();
        assert_eq!(entry.convert_state(), 0x03, "ever + last month");
        entry.mark_final_delivery();
        entry.clear_newgrf_bigtick();
        assert_eq!(entry.convert_state(), 0x07, "ever + last + current month");
    }
}
