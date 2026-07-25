//! Specs de casas vanilla (`HouseSpec` / `_original_house_specs`).
//!
//! Datos generados en el módulo privado `crate::sav::house_population_generated`; aquí viven
//! las consultas runtime para zonas, años, pesos y aceptación (P3.5–P3.7).

use crate::cargo::CargoType;
use crate::map::TileCoord;
use crate::sav::house_population_generated::{
    HOUSE_ACCEPTS_CARGO, HOUSE_AVAILABILITY, HOUSE_BUILDING_FLAGS, HOUSE_CARGO_ACCEPTANCE,
    HOUSE_MAIL_GENERATION, HOUSE_MAX_YEAR, HOUSE_MAX_YEAR_OF, HOUSE_MIN_YEAR, HOUSE_MINIMUM_LIFE,
    HOUSE_POPULATION, HOUSE_PROBABILITY, HOUSE_SIZE_1X1, HOUSE_SPEC_COUNT,
};
use crate::town::{HouseZone, NUM_HOUSE_ZONES, Town, TownLayout};
use crate::world_gen::{Climate, DEF_SNOW_LINE_HEIGHT};

/// Número de `HouseID` vanilla.
pub const NUM_HOUSES_VANILLA: usize = HOUSE_SPEC_COUNT;
pub const HOUSE_YEAR_MAX: u32 = HOUSE_MAX_YEAR;

/// Flags de edificio (`BuildingFlag`).
pub const BUILDING_FLAG_SIZE_1X1: u8 = 1 << 0;
pub const BUILDING_FLAG_NOT_SLOPED: u8 = 1 << 1;
pub const BUILDING_FLAG_SIZE_2X1: u8 = 1 << 2;
pub const BUILDING_FLAG_SIZE_1X2: u8 = 1 << 3;
pub const BUILDING_FLAG_SIZE_2X2: u8 = 1 << 4;
pub const BUILDING_FLAG_IS_ANIMATED: u8 = 1 << 5;
pub const BUILDING_FLAG_IS_CHURCH: u8 = 1 << 6;
pub const BUILDING_FLAG_IS_STADIUM: u8 = 1 << 7;

/// Umbral de aceptación de estación en octavos (`amt >= 8`).
pub const STATION_ACCEPTANCE_THRESHOLD: u32 = 8;

/// Vista de un `HouseSpec` vanilla.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HouseSpec {
    pub id: u16,
    pub min_year: u32,
    pub max_year: u32,
    pub population: u16,
    pub mail_generation: u16,
    pub probability: u8,
    pub minimum_life: u8,
    pub building_flags: u8,
    pub availability: u16,
    pub cargo_acceptance: [u8; 3],
    pub accepts_cargo: [u8; 3],
}

impl HouseSpec {
    #[must_use]
    pub fn get(house_id: u16) -> Option<Self> {
        let i = usize::from(house_id);
        if i >= HOUSE_SPEC_COUNT {
            return None;
        }
        Some(Self {
            id: house_id,
            min_year: HOUSE_MIN_YEAR[i],
            max_year: HOUSE_MAX_YEAR_OF[i],
            population: HOUSE_POPULATION[i],
            mail_generation: HOUSE_MAIL_GENERATION[i],
            probability: HOUSE_PROBABILITY[i],
            minimum_life: HOUSE_MINIMUM_LIFE[i],
            building_flags: HOUSE_BUILDING_FLAGS[i],
            availability: HOUSE_AVAILABILITY[i],
            cargo_acceptance: HOUSE_CARGO_ACCEPTANCE[i],
            accepts_cargo: HOUSE_ACCEPTS_CARGO[i],
        })
    }

    #[must_use]
    pub const fn is_size_1x1(self) -> bool {
        self.building_flags & BUILDING_FLAG_SIZE_1X1 != 0
            && self.building_flags
                & (BUILDING_FLAG_SIZE_2X1 | BUILDING_FLAG_SIZE_1X2 | BUILDING_FLAG_SIZE_2X2)
                == 0
    }

    #[must_use]
    pub const fn is_church(self) -> bool {
        self.building_flags & BUILDING_FLAG_IS_CHURCH != 0
    }

    #[must_use]
    pub const fn is_stadium(self) -> bool {
        self.building_flags & BUILDING_FLAG_IS_STADIUM != 0
    }

    #[must_use]
    pub const fn requires_flat(self) -> bool {
        self.building_flags & BUILDING_FLAG_NOT_SLOPED != 0
    }

    /// ¿El spec admite la zona/clima pedidos? (`building_availability.All(zones)`).
    #[must_use]
    pub const fn matches_zones(self, required: u16) -> bool {
        self.availability & required == required
    }
}

/// Máscara de clima para el landscape actual (`GetClimateMaskForLandscape`).
#[must_use]
pub fn climate_zone_mask(climate: Climate, tile_height: u8) -> u16 {
    match climate {
        Climate::Temperate => 1 << (HouseZone::ClimateTemperate as u8),
        Climate::SubArctic => {
            if i32::from(tile_height) > i32::from(DEF_SNOW_LINE_HEIGHT) {
                1 << (HouseZone::ClimateSubarcticAboveSnow as u8)
            } else {
                1 << (HouseZone::ClimateSubarcticBelowSnow as u8)
            }
        }
        Climate::SubTropical => 1 << (HouseZone::ClimateSubtropic as u8),
        Climate::Toyland => 1 << (HouseZone::ClimateToyland as u8),
    }
}

/// Convierte el índice de aceptación generado a [`CargoType`] del port.
#[must_use]
pub fn house_accept_to_cargo(idx: u8) -> Option<CargoType> {
    match idx {
        0 => Some(CargoType::Passengers),
        1 => Some(CargoType::Mail),
        // Goods y Food (3) → Goods (proxy hasta existir cargo dedicado).
        2 | 3 => Some(CargoType::Goods),
        // Water → Oil (proxy trópico).
        4 => Some(CargoType::Oil),
        _ => None,
    }
}

/// Aporta aceptación de una casa a contadores por cargo (`AddAcceptedCargo_Town`).
pub fn add_accepted_cargo_of_house(house_id: u16, amounts: &mut [u32; 5]) {
    let Some(hs) = HouseSpec::get(house_id) else {
        return;
    };
    for i in 0..3 {
        let cargo_idx = hs.accepts_cargo[i];
        let amt = u32::from(hs.cargo_acceptance[i]);
        if amt == 0 {
            continue;
        }
        let slot = match cargo_idx {
            0 => 0,     // passengers
            1 => 1,     // mail
            2 | 3 => 2, // goods / food
            4 => 3,     // water → oil slot en coverage
            _ => continue,
        };
        amounts[slot] = amounts[slot].saturating_add(amt);
    }
}

/// Elige un `HouseID` ponderado por zona/clima/año (`TryBuildTownHouse` simplificado a 1×1).
#[must_use]
pub fn pick_town_house_id(
    town: &Town,
    zone: HouseZone,
    climate: Climate,
    tile_height: u8,
    calendar_year: u32,
    rng_value: u32,
) -> Option<u16> {
    let climate_mask = climate_zone_mask(climate, tile_height);
    let zone_mask = (1u16 << (zone as u8)) | climate_mask;

    let mut probs: Vec<(u16, u32)> = Vec::new();
    let mut probability_max = 0_u32;
    for (id, &is_1x1) in HOUSE_SIZE_1X1.iter().enumerate() {
        if !is_1x1 {
            continue;
        }
        let hs = HouseSpec::get(u16::try_from(id).unwrap_or(0))?;
        if !hs.matches_zones(zone_mask) {
            continue;
        }
        if calendar_year < hs.min_year || calendar_year > hs.max_year {
            continue;
        }
        if hs.is_church() && town.has_church {
            continue;
        }
        if hs.is_stadium() && town.has_stadium {
            continue;
        }
        // Evitar specs sin población (estatuas/parques) en expansión normal.
        if hs.population == 0 && !hs.is_church() {
            continue;
        }
        let p = u32::from(hs.probability.max(1));
        probability_max = probability_max.saturating_add(p);
        probs.push((hs.id, p));
    }
    if probability_max == 0 || probs.is_empty() {
        return None;
    }

    let mut r = rng_value % probability_max;
    for (id, p) in probs {
        if p > r {
            return Some(id);
        }
        r -= p;
    }
    None
}

/// Distancia al cuadrado entre teselas (`DistanceSquare`).
#[must_use]
pub fn distance_square(a: TileCoord, b: TileCoord) -> u32 {
    let dx = a.x.abs_diff(b.x);
    let dy = a.y.abs_diff(b.y);
    dx.saturating_mul(dx).saturating_add(dy.saturating_mul(dy))
}

/// Zona urbana del tile respecto al pueblo (`GetTownRadiusGroup`).
#[must_use]
pub fn get_town_radius_group(town: &Town, tile: TileCoord) -> HouseZone {
    let dist = distance_square(tile, town.pos);
    if town.fund_buildings_months != 0 && dist <= 25 {
        return HouseZone::TownCentre;
    }
    let mut smallest = HouseZone::TownEdge;
    for i in 0..NUM_HOUSE_ZONES {
        let radius = town.squared_town_zone_radius[i];
        if radius > 0 && dist < radius {
            // HouseZone valores 0..4 coinciden con el índice de radio.
            if let Some(zone) = HouseZone::from_zone_index(i) {
                smallest = zone;
            }
        }
    }
    smallest
}

/// Iteraciones de `GrowTownAtRoad` según layout y casas.
#[must_use]
pub fn grow_town_at_road_iterations(layout: TownLayout, num_houses: u16) -> i32 {
    let n = i32::from(num_houses);
    match layout {
        TownLayout::BetterRoads => 10 + n * 2 / 9,
        TownLayout::Grid2x2 | TownLayout::Grid3x3 => 10 + n / 9,
        TownLayout::Original | TownLayout::Random => 10 + n * 4 / 9,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::map::TileCoord;
    use crate::town::{Town, update_town_radius};

    #[test]
    fn tall_office_is_temperate_centre_only() {
        let hs = HouseSpec::get(0).unwrap();
        assert_eq!(hs.population, 187);
        assert!(hs.is_size_1x1());
        let centre_temp =
            (1 << HouseZone::TownCentre as u8) | (1 << HouseZone::ClimateTemperate as u8);
        assert!(hs.matches_zones(centre_temp));
        let edge_temp = (1 << HouseZone::TownEdge as u8) | (1 << HouseZone::ClimateTemperate as u8);
        assert!(!hs.matches_zones(edge_temp));
    }

    #[test]
    fn church_flag_and_unique() {
        let hs = HouseSpec::get(3).unwrap();
        assert!(hs.is_church());
        assert_eq!(hs.population, 5);
    }

    #[test]
    fn pick_respects_year_and_zone() {
        let mut town = Town {
            pos: TileCoord::new(10, 10),
            num_houses: 48,
            fund_buildings_months: 0,
            ..Default::default()
        };
        update_town_radius(&mut town);
        // Con 48 casas el radio centre es 9; dist 0 → TownCentre.
        let zone = get_town_radius_group(&town, TileCoord::new(10, 10));
        assert_eq!(zone, HouseZone::TownCentre);
        let id = pick_town_house_id(&town, zone, Climate::Temperate, 1, 1980, 42).unwrap();
        let hs = HouseSpec::get(id).unwrap();
        assert!(hs.is_size_1x1());
        assert!(hs.min_year <= 1980 && hs.max_year >= 1980);
    }

    #[test]
    fn acceptance_sums_goods_from_office() {
        let mut amounts = [0u32; 5];
        add_accepted_cargo_of_house(0, &mut amounts);
        assert_eq!(amounts[0], 8); // passengers
        assert_eq!(amounts[1], 3); // mail
        assert_eq!(amounts[2], 4); // goods
    }
}
