//! Estado persistente por tipo de carga en una estación (`GoodsEntry`, `station_base.h:167`).
//!
//! El rating no se recalcula desde cero en cada barrido: vive aquí y converge poco a poco
//! hacia el objetivo que da `UpdateStationRating`, de modo que un mal servicio puntual no
//! hunde la estación ni un buen viaje la arregla de golpe.

use crate::cargo::{ALL_CARGO_TYPES, CargoType};

/// Rating con el que nace cada entrada de carga (`INITIAL_STATION_RATING`, `station_base.h:23`).
pub const INITIAL_STATION_RATING: u8 = 175;

/// Paso máximo de convergencia por barrido (`Clamp(rating - or_, -2, 2)`).
pub const STATION_RATING_MAX_STEP: i16 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct GoodsEntry {
    /// Rating 0–255 de esta carga en esta estación.
    #[serde(default = "default_rating")]
    pub rating: u8,
    /// `State::Rating`: la estación llegó a tener esta carga en espera alguna vez.
    ///
    /// Se limpia tras 255 barridos sin recogida. Mientras esté en `false` el rating solo
    /// sube de uno en uno hasta `INITIAL_STATION_RATING`.
    #[serde(default)]
    pub has_rating: bool,
    /// Velocidad del último vehículo que intentó cargar (`last_speed`); 0 = ninguno todavía.
    #[serde(default)]
    pub last_speed: u8,
    /// Edad en años del último vehículo que intentó cargar (`last_age`).
    #[serde(default = "default_last_age")]
    pub last_age: u8,
    /// Media de carga en espera por destino en el barrido anterior (`max_waiting_cargo`).
    #[serde(default)]
    pub max_waiting_cargo: u32,
    /// Parte fraccionaria al recibir producción (`amount_fract`, 8 bits bajos).
    ///
    /// `MoveGoodsToStation` reparte en unidades × (`rating + 1`); aquí se acumula el resto
    /// hasta completar una unidad entera (`UpdateStationWaiting`).
    #[serde(default)]
    pub amount_fract: u8,
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
        }
    }
}

impl GoodsEntry {
    /// ¿Algún vehículo intentó cargar aquí? (`HasVehicleEverTriedLoading`).
    #[must_use]
    pub const fn has_vehicle_ever_tried_loading(&self) -> bool {
        self.last_speed != 0
    }

    /// Acerca el rating al objetivo en pasos de como mucho ±2 y devuelve el nuevo valor.
    pub fn converge_rating_towards(&mut self, target: i16) -> u8 {
        let current = i16::from(self.rating);
        let step = (target - current).clamp(-STATION_RATING_MAX_STEP, STATION_RATING_MAX_STEP);
        self.rating = u8::try_from((current + step).clamp(0, 255)).unwrap_or(self.rating);
        self.rating
    }
}

/// Las once entradas de carga de una estación (`Station::goods`).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct StationGoods {
    entries: [GoodsEntry; ALL_CARGO_TYPES.len()],
}

impl Default for StationGoods {
    fn default() -> Self {
        Self {
            entries: [GoodsEntry::default(); ALL_CARGO_TYPES.len()],
        }
    }
}

impl StationGoods {
    #[must_use]
    pub fn get(&self, cargo: CargoType) -> &GoodsEntry {
        &self.entries[cargo.temperate_id() as usize]
    }

    pub fn get_mut(&mut self, cargo: CargoType) -> &mut GoodsEntry {
        &mut self.entries[cargo.temperate_id() as usize]
    }

    /// Rating actual de una carga.
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
mod tests {
    use super::*;

    #[test]
    fn new_entry_starts_at_initial_rating() {
        let goods = StationGoods::default();
        assert_eq!(goods.rating(CargoType::Coal), INITIAL_STATION_RATING);
        assert_eq!(INITIAL_STATION_RATING, 175);
        assert!(!goods.get(CargoType::Coal).has_rating);
    }

    #[test]
    fn entries_are_independent_per_cargo() {
        let mut goods = StationGoods::default();
        goods.get_mut(CargoType::Coal).rating = 10;
        assert_eq!(goods.rating(CargoType::Coal), 10);
        assert_eq!(goods.rating(CargoType::Wood), INITIAL_STATION_RATING);
    }

    /// El rating se mueve como mucho ±2 por barrido, así que tarda decenas de ciclos en
    /// recorrer el rango: es lo que hace que servir bien una estación sea un compromiso.
    #[test]
    fn rating_converges_two_points_at_a_time() {
        let mut entry = GoodsEntry::default();
        assert_eq!(entry.converge_rating_towards(255), 177);
        assert_eq!(entry.converge_rating_towards(255), 179);
        assert_eq!(entry.converge_rating_towards(0), 177);
        entry.rating = 1;
        assert_eq!(entry.converge_rating_towards(0), 0);
        assert_eq!(entry.converge_rating_towards(0), 0);
    }
}
