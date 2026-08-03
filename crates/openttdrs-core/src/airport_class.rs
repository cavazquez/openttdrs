//! Clases y specs de aeropuerto (`AirportClass` / `AirportSpec` de `OpenTTD`).
//!
//! Catálogo filtrable (UI-6): vanilla + `NewGRF` Action0 Airports (`≥10`).

use serde::{Deserialize, Serialize};

/// Primera spec definida por `NewGRF` (`OpenTTD` `NEW_AIRPORT_OFFSET`).
pub const NEW_AIRPORT_OFFSET: u16 = 10;
/// Máximo de aeropuertos (`OpenTTD` `NUM_AIRPORTS`).
pub const NUM_AIRPORTS: u16 = 128;

/// Identificador de clase de aeropuerto (`AirportClassID`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[repr(u16)]
pub enum AirportClassId {
    /// Helipuertos / hangares pequeños.
    Heliport = 0,
    /// Aeropuertos pequeños (country / commuter).
    #[default]
    Small = 1,
    /// Aeropuertos grandes (city / metropolitan).
    Large = 2,
    /// Hubs (international / intercontinental).
    Hub = 3,
}

impl AirportClassId {
    #[must_use]
    pub const fn from_u16(v: u16) -> Self {
        match v {
            0 => Self::Heliport,
            2 => Self::Large,
            3 => Self::Hub,
            _ => Self::Small,
        }
    }

    #[must_use]
    pub const fn as_u16(self) -> u16 {
        self as u16
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Heliport => "Helipuerto",
            Self::Small => "Pequeño",
            Self::Large => "Grande",
            Self::Hub => "Hub",
        }
    }
}

/// Identificador de spec dentro del catálogo global.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[repr(u16)]
pub enum AirportSpecId {
    /// Helipuerto 1×1.
    Heliport = 0,
    /// Helidepósito 2×2.
    Helidepot = 1,
    /// Aeropuerto country / small 4×3.
    #[default]
    Small = 2,
    /// Commuter 5×4.
    Commuter = 3,
    /// City 6×6.
    City = 4,
    /// Metropolitan 6×6 (doble pista).
    Metropolitan = 5,
    /// International 7×7.
    International = 6,
    /// Intercontinental 9×11.
    Intercontinental = 7,
    /// Oilrig 1×1 (helipad sobre plataforma; FTA = Heliport).
    Oilrig = 8,
    /// Helistation 4×2 (hangar + 3 helipads).
    Helistation = 9,
}

impl AirportSpecId {
    #[must_use]
    pub const fn from_u16(v: u16) -> Self {
        match v {
            0 => Self::Heliport,
            1 => Self::Helidepot,
            3 => Self::Commuter,
            4 => Self::City,
            5 => Self::Metropolitan,
            6 => Self::International,
            7 => Self::Intercontinental,
            8 => Self::Oilrig,
            9 => Self::Helistation,
            _ => Self::Small,
        }
    }

    #[must_use]
    pub const fn as_u16(self) -> u16 {
        self as u16
    }

    #[must_use]
    pub const fn is_heliport_only(self) -> bool {
        matches!(
            self,
            Self::Heliport | Self::Helidepot | Self::Oilrig | Self::Helistation
        )
    }

    /// Mapea `AirportTypes` (`AT_*`) de `OpenTTD` a nuestro `AirportSpecId`.
    ///
    /// `AT_SMALL=0, AT_LARGE=1, AT_HELIPORT=2, AT_METROPOLITAN=3,
    /// AT_INTERNATIONAL=4, AT_COMMUTER=5, AT_HELIDEPOT=6, AT_INTERCON=7,
    /// AT_HELISTATION=8, AT_OILRIG=9` (`airport.h`).
    /// `NewGRF` (`≥10`) se conserva como id global en
    /// [`crate::station::Station::airport_newgrf_spec_id`].
    #[must_use]
    pub const fn from_ottd_airport_type(at: u8) -> Self {
        if (at as u16) >= NEW_AIRPORT_OFFSET {
            // Placeholder vanilla; el id NewGRF viaja en `Station.airport_newgrf_spec_id`.
            return Self::Small;
        }
        match at {
            1 => Self::City, // AT_LARGE
            2 => Self::Heliport,
            3 => Self::Metropolitan,
            4 => Self::International,
            5 => Self::Commuter,
            6 => Self::Helidepot,
            7 => Self::Intercontinental,
            8 => Self::Helistation,
            9 => Self::Oilrig,
            _ => Self::Small, // AT_SMALL=0
        }
    }

    /// Id raw `OpenTTD` (`AT_*` o `NewGRF` ≥10).
    #[must_use]
    pub const fn as_ottd_airport_type(self) -> u8 {
        match self {
            Self::Small => 0,
            Self::City => 1,
            Self::Heliport => 2,
            Self::Metropolitan => 3,
            Self::International => 4,
            Self::Commuter => 5,
            Self::Helidepot => 6,
            Self::Intercontinental => 7,
            Self::Helistation => 8,
            Self::Oilrig => 9,
        }
    }
}

/// Tesela de layout `NewGRF` ya resuelta a gfx global o vanilla.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AirportLayoutTile {
    pub x: i8,
    pub y: i8,
    /// Gfx de airport tile (vanilla &lt;74 o `NewGRF` ≥74).
    pub gfx: u16,
}

/// Una rotación de layout `NewGRF`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AirportTileLayout {
    pub rotation: u8,
    pub tiles: Vec<AirportLayoutTile>,
}

/// Selector Action3 purchase (`PurchaseDefaultMapSpriteGroupHandler`, cid `0xFF`).
pub const AIRPORT_ACTION3_PURCHASE: u8 = 0xFF;

/// Spec `NewGRF` de aeropuerto (ids globales ≥ [`NEW_AIRPORT_OFFSET`]).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewgrfAirportSpecDef {
    pub id: u16,
    pub class: AirportClassId,
    pub label: String,
    pub short_label: String,
    pub size_x: i32,
    pub size_y: i32,
    pub catchment: i32,
    pub noise_level: u8,
    /// Spec vanilla de la que hereda clase/FTA flags (sin ejecutar FTA `NewGRF`).
    pub subst_id: AirportSpecId,
    pub layouts: Vec<AirportTileLayout>,
    pub enabled: bool,
    pub min_year: u16,
    pub max_year: u16,
    pub maintenance_cost: u16,
    #[serde(default, skip)]
    pub newgrf_local_id: u8,
    #[serde(default, skip)]
    pub newgrf_grfid: u32,
    /// Grupo Action3 default (preview / render).
    #[serde(default, skip)]
    pub newgrf_views: Vec<crate::newgrf_sprites::DecodedSprite>,
    /// Grupo Action3 purchase (`0xFF`) para picker.
    #[serde(default, skip)]
    pub newgrf_purchase_views: Vec<crate::newgrf_sprites::DecodedSprite>,
}

impl NewgrfAirportSpecDef {
    #[must_use]
    pub fn fta_flags(&self) -> AirportFtaFlags {
        airport_spec_def(self.subst_id).map_or(AirportFtaFlags::empty(), |d| d.fta_flags)
    }

    #[must_use]
    pub fn has_newgrf_sprites(&self) -> bool {
        !self.newgrf_views.is_empty() || !self.newgrf_purchase_views.is_empty()
    }

    /// Vista para picker: purchase si hay; si no, default.
    #[must_use]
    pub fn newgrf_preview_sprite(&self) -> Option<&crate::newgrf_sprites::DecodedSprite> {
        self.newgrf_purchase_views
            .first()
            .or_else(|| self.newgrf_views.first())
    }
}

/// Siguiente id libre ≥ [`NEW_AIRPORT_OFFSET`].
#[must_use]
pub fn next_free_airport_id(catalog: &[NewgrfAirportSpecDef]) -> Option<u16> {
    let mut used: Vec<u16> = catalog.iter().map(|d| d.id).collect();
    used.sort_unstable();
    used.dedup();
    let mut candidate = NEW_AIRPORT_OFFSET;
    for &u in &used {
        if u == candidate {
            candidate = candidate.saturating_add(1);
        } else if u > candidate {
            break;
        }
    }
    (candidate < NUM_AIRPORTS).then_some(candidate)
}

/// Specs `NewGRF` de una clase (filtradas).
#[must_use]
pub fn list_newgrf_airport_specs<'a>(
    class: AirportClassId,
    filter: &str,
    newgrf: &'a [NewgrfAirportSpecDef],
) -> Vec<&'a NewgrfAirportSpecDef> {
    let needle = filter.trim().to_ascii_lowercase();
    newgrf
        .iter()
        .filter(|s| s.enabled && s.class == class)
        .filter(|s| {
            needle.is_empty()
                || s.label.to_ascii_lowercase().contains(&needle)
                || s.short_label.to_ascii_lowercase().contains(&needle)
        })
        .collect()
}

/// Lookup `NewGRF` por id global.
#[must_use]
pub fn newgrf_airport_spec_def(
    catalog: &[NewgrfAirportSpecDef],
    id: u16,
) -> Option<&NewgrfAirportSpecDef> {
    catalog.iter().find(|s| s.id == id && s.enabled)
}

/// Metadatos de una clase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AirportClassDef {
    pub id: AirportClassId,
    pub label: &'static str,
    pub from_newgrf: bool,
}

/// Flags FTA de aeropuerto (`AirportFTAClass::Flag` en `airport.h` / `airport.cpp`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AirportFtaFlags(u8);

impl AirportFtaFlags {
    pub const AIRPLANES: Self = Self(1);
    pub const HELICOPTERS: Self = Self(2);
    pub const SHORT_STRIP: Self = Self(4);

    #[must_use]
    pub const fn empty() -> Self {
        Self(0)
    }

    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    #[must_use]
    pub const fn contains(self, flag: Self) -> bool {
        self.0 & flag.0 == flag.0
    }

    #[must_use]
    pub const fn allows_airplanes(self) -> bool {
        self.contains(Self::AIRPLANES)
    }

    #[must_use]
    pub const fn allows_helicopters(self) -> bool {
        self.contains(Self::HELICOPTERS)
    }

    #[must_use]
    pub const fn short_strip(self) -> bool {
        self.contains(Self::SHORT_STRIP)
    }
}

/// Spec de aeropuerto (tamaño base; layout en [`crate::airport`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AirportSpecDef {
    pub id: AirportSpecId,
    pub class: AirportClassId,
    pub label: &'static str,
    pub short_label: &'static str,
    pub size_x: i32,
    pub size_y: i32,
    /// Radio de cobertura (teselas).
    pub catchment: i32,
    /// Ruido base (`AirportSpec.noise_level` en `airport_defaults.h`).
    pub noise_level: u8,
    /// Flags FTA (`Airplanes` / `Helicopters` / `ShortStrip`).
    pub fta_flags: AirportFtaFlags,
    pub from_newgrf: bool,
}

impl AirportSpecDef {
    /// Compatibilidad motor↔aeropuerto (`CanVehicleUseStation` para aviones).
    #[must_use]
    pub const fn allows_aircraft_subtype(self, is_helicopter: bool) -> bool {
        if is_helicopter {
            self.fta_flags.allows_helicopters()
        } else {
            self.fta_flags.allows_airplanes()
        }
    }
}

const VANILLA_CLASSES: &[AirportClassDef] = &[
    AirportClassDef {
        id: AirportClassId::Heliport,
        label: "Helipuerto",
        from_newgrf: false,
    },
    AirportClassDef {
        id: AirportClassId::Small,
        label: "Pequeño",
        from_newgrf: false,
    },
    AirportClassDef {
        id: AirportClassId::Large,
        label: "Grande",
        from_newgrf: false,
    },
    AirportClassDef {
        id: AirportClassId::Hub,
        label: "Hub",
        from_newgrf: false,
    },
];

/// Helipuertos: solo hélices (`HELIPORT` macro en `airport.cpp`).
const HELI_ONLY: AirportFtaFlags = AirportFtaFlags::HELICOPTERS;
/// Country / Commuter: ala fija + hélices + pista corta.
const SHORT_STRIP: AirportFtaFlags = AirportFtaFlags::AIRPLANES
    .union(AirportFtaFlags::HELICOPTERS)
    .union(AirportFtaFlags::SHORT_STRIP);
/// Resto de aeropuertos con pista: ala fija + hélices.
const FULL_STRIP: AirportFtaFlags = AirportFtaFlags::AIRPLANES.union(AirportFtaFlags::HELICOPTERS);

const VANILLA_SPECS: &[AirportSpecDef] = &[
    AirportSpecDef {
        id: AirportSpecId::Heliport,
        class: AirportClassId::Heliport,
        label: "Helipuerto",
        short_label: "Heli",
        size_x: 1,
        size_y: 1,
        catchment: 4,
        noise_level: 1,
        fta_flags: HELI_ONLY,
        from_newgrf: false,
    },
    AirportSpecDef {
        id: AirportSpecId::Helidepot,
        class: AirportClassId::Heliport,
        label: "Helidepósito",
        short_label: "HDep",
        size_x: 2,
        size_y: 2,
        catchment: 4,
        noise_level: 2,
        fta_flags: HELI_ONLY,
        from_newgrf: false,
    },
    AirportSpecDef {
        id: AirportSpecId::Small,
        class: AirportClassId::Small,
        label: "Aeropuerto pequeño",
        short_label: "Small",
        size_x: 4,
        size_y: 3,
        catchment: 4,
        noise_level: 3,
        fta_flags: SHORT_STRIP,
        from_newgrf: false,
    },
    AirportSpecDef {
        id: AirportSpecId::Commuter,
        class: AirportClassId::Small,
        label: "Commuter",
        short_label: "Comm",
        size_x: 5,
        size_y: 4,
        catchment: 4,
        noise_level: 4,
        fta_flags: SHORT_STRIP,
        from_newgrf: false,
    },
    AirportSpecDef {
        id: AirportSpecId::City,
        class: AirportClassId::Large,
        label: "City",
        short_label: "City",
        size_x: 6,
        size_y: 6,
        catchment: 5,
        noise_level: 5,
        fta_flags: FULL_STRIP,
        from_newgrf: false,
    },
    AirportSpecDef {
        id: AirportSpecId::Metropolitan,
        class: AirportClassId::Large,
        label: "Metropolitan",
        short_label: "Metro",
        size_x: 6,
        size_y: 6,
        catchment: 6,
        noise_level: 8,
        fta_flags: FULL_STRIP,
        from_newgrf: false,
    },
    AirportSpecDef {
        id: AirportSpecId::International,
        class: AirportClassId::Hub,
        label: "International",
        short_label: "Intl",
        size_x: 7,
        size_y: 7,
        catchment: 8,
        noise_level: 17,
        fta_flags: FULL_STRIP,
        from_newgrf: false,
    },
    AirportSpecDef {
        id: AirportSpecId::Intercontinental,
        class: AirportClassId::Hub,
        label: "Intercontinental",
        short_label: "ICont",
        size_x: 9,
        size_y: 11,
        catchment: 10,
        noise_level: 25,
        fta_flags: FULL_STRIP,
        from_newgrf: false,
    },
    AirportSpecDef {
        id: AirportSpecId::Oilrig,
        class: AirportClassId::Heliport,
        label: "Oilrig",
        short_label: "Oil",
        size_x: 1,
        size_y: 1,
        catchment: 4,
        noise_level: 0,
        fta_flags: HELI_ONLY,
        from_newgrf: false,
    },
    AirportSpecDef {
        id: AirportSpecId::Helistation,
        class: AirportClassId::Heliport,
        label: "Helistation",
        short_label: "HStn",
        size_x: 4,
        size_y: 2,
        catchment: 4,
        noise_level: 3,
        fta_flags: HELI_ONLY,
        from_newgrf: false,
    },
];

#[must_use]
pub fn all_airport_class_defs() -> &'static [AirportClassDef] {
    VANILLA_CLASSES
}

#[must_use]
pub fn all_airport_spec_defs() -> &'static [AirportSpecDef] {
    VANILLA_SPECS
}

#[must_use]
pub fn airport_class_def(id: AirportClassId) -> Option<&'static AirportClassDef> {
    VANILLA_CLASSES.iter().find(|c| c.id == id)
}

#[must_use]
pub fn airport_spec_def(id: AirportSpecId) -> Option<&'static AirportSpecDef> {
    VANILLA_SPECS.iter().find(|s| s.id == id)
}

#[must_use]
pub fn list_airport_classes(filter: &str) -> Vec<&'static AirportClassDef> {
    let needle = filter.trim().to_ascii_lowercase();
    VANILLA_CLASSES
        .iter()
        .filter(|c| needle.is_empty() || c.label.to_ascii_lowercase().contains(&needle))
        .collect()
}

#[must_use]
pub fn list_airport_specs(class: AirportClassId, filter: &str) -> Vec<&'static AirportSpecDef> {
    let needle = filter.trim().to_ascii_lowercase();
    VANILLA_SPECS
        .iter()
        .filter(|s| s.class == class)
        .filter(|s| {
            if needle.is_empty() {
                return true;
            }
            s.label.to_ascii_lowercase().contains(&needle)
                || s.short_label.to_ascii_lowercase().contains(&needle)
        })
        .collect()
}

/// ¿Puede este aeropuerto atender el subtipo de aeronave?
#[must_use]
pub fn airport_allows_aircraft(spec: AirportSpecId, is_helicopter: bool) -> bool {
    airport_spec_def(spec).is_none_or(|d| d.allows_aircraft_subtype(is_helicopter))
}

/// Población por unidad de ruido (índice 0 / consejo permisivo; `economy_settings.ini`).
pub const TOWN_NOISE_POPULATION_DEFAULT: u32 = 800;

/// Ruido efectivo tras atenuar por distancia al pueblo (`GetAirportNoiseLevelForDistance`).
#[must_use]
pub fn airport_noise_for_distance(
    noise_level: u8,
    distance: u32,
    town_tolerance_distance: u32,
) -> u8 {
    if noise_level < 2 {
        return noise_level;
    }
    let step = town_tolerance_distance.max(1);
    let noise_reduction = distance / step;
    let nl = u32::from(noise_level);
    if noise_reduction >= nl {
        1
    } else {
        u8::try_from(nl - noise_reduction).unwrap_or(1)
    }
}

/// Máximo de ruido que tolera un pueblo (`Town::MaxTownNoise`).
#[must_use]
pub fn max_town_noise(population: u32, town_noise_population: u32) -> u16 {
    if population == 0 {
        return 0;
    }
    let denom = town_noise_population.max(1);
    let v = population / denom + 3;
    u16::try_from(v.min(u32::from(u16::MAX))).unwrap_or(u16::MAX)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn list_filters_by_class() {
        let heli = list_airport_specs(AirportClassId::Heliport, "");
        assert!(heli.iter().any(|s| s.id == AirportSpecId::Heliport));
        assert!(heli.iter().any(|s| s.id == AirportSpecId::Helidepot));
        assert!(heli.iter().any(|s| s.id == AirportSpecId::Oilrig));
        assert!(heli.iter().any(|s| s.id == AirportSpecId::Helistation));
        assert!(!heli.iter().any(|s| s.id == AirportSpecId::Small));

        let large = list_airport_specs(AirportClassId::Large, "");
        assert!(large.iter().any(|s| s.id == AirportSpecId::City));
        assert!(large.iter().any(|s| s.id == AirportSpecId::Metropolitan));

        let hub = list_airport_specs(AirportClassId::Hub, "");
        assert!(hub.iter().any(|s| s.id == AirportSpecId::International));
        assert!(hub.iter().any(|s| s.id == AirportSpecId::Intercontinental));
    }

    #[test]
    fn large_and_hub_sizes_match_openttd() {
        assert_eq!(airport_spec_def(AirportSpecId::City).unwrap().size_x, 6);
        assert_eq!(airport_spec_def(AirportSpecId::City).unwrap().size_y, 6);
        assert_eq!(
            airport_spec_def(AirportSpecId::International)
                .unwrap()
                .size_x,
            7
        );
        assert_eq!(
            airport_spec_def(AirportSpecId::Intercontinental)
                .unwrap()
                .size_y,
            11
        );
    }

    #[test]
    fn fta_flags_match_openttd_airport_macros() {
        let heli = airport_spec_def(AirportSpecId::Heliport).unwrap();
        assert!(!heli.fta_flags.allows_airplanes());
        assert!(heli.fta_flags.allows_helicopters());
        assert!(!heli.fta_flags.short_strip());

        let small = airport_spec_def(AirportSpecId::Small).unwrap();
        assert!(
            small.fta_flags.allows_airplanes()
                && small.fta_flags.allows_helicopters()
                && small.fta_flags.short_strip()
        );

        let city = airport_spec_def(AirportSpecId::City).unwrap();
        assert!(
            city.fta_flags.allows_airplanes()
                && city.fta_flags.allows_helicopters()
                && !city.fta_flags.short_strip()
        );

        let inter = airport_spec_def(AirportSpecId::Intercontinental).unwrap();
        assert!(
            inter.fta_flags.allows_airplanes()
                && inter.fta_flags.allows_helicopters()
                && !inter.fta_flags.short_strip()
        );

        assert!(!airport_allows_aircraft(AirportSpecId::Heliport, false));
        assert!(airport_allows_aircraft(AirportSpecId::Heliport, true));
        assert!(airport_allows_aircraft(AirportSpecId::Small, false));
        assert!(airport_allows_aircraft(AirportSpecId::Small, true));
    }
}
