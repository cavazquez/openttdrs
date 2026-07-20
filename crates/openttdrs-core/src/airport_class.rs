//! Clases y specs de aeropuerto (`AirportClass` / `AirportSpec` de `OpenTTD`).
//!
//! Catálogo filtrable (UI-6): vanilla hoy; `NewGRF` Action0 Airports ampliará
//! el registro cuando exista el runtime.

use serde::{Deserialize, Serialize};

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
            _ => Self::Small,
        }
    }

    #[must_use]
    pub const fn as_u16(self) -> u16 {
        self as u16
    }

    #[must_use]
    pub const fn is_heliport_only(self) -> bool {
        matches!(self, Self::Heliport | Self::Helidepot | Self::Oilrig)
    }
}

/// Metadatos de una clase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AirportClassDef {
    pub id: AirportClassId,
    pub label: &'static str,
    pub from_newgrf: bool,
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
    pub from_newgrf: bool,
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

const VANILLA_SPECS: &[AirportSpecDef] = &[
    AirportSpecDef {
        id: AirportSpecId::Heliport,
        class: AirportClassId::Heliport,
        label: "Helipuerto",
        short_label: "Heli",
        size_x: 1,
        size_y: 1,
        catchment: 4,
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
}
