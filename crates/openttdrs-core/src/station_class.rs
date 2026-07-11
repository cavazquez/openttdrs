//! Clases y specs de estación ferroviaria (`StationClass` / `StationSpec` de `OpenTTD`).
//!
//! Catálogo filtrable (UI-6g): vanilla hoy; `NewGRF` Action0 Stations ampliará
//! el registro cuando exista el runtime Action0–14.

use serde::{Deserialize, Serialize};

/// Identificador de clase de estación (`StationClassID`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[repr(u16)]
pub enum StationClassId {
    /// Clase por defecto (`STAT_CLASS_DFLT`).
    #[default]
    Default = 0,
}

impl StationClassId {
    #[must_use]
    pub const fn from_u16(v: u16) -> Self {
        let _ = v;
        Self::Default
    }

    #[must_use]
    pub const fn as_u16(self) -> u16 {
        self as u16
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Default => "Por defecto",
        }
    }

    #[must_use]
    pub const fn short_label(self) -> &'static str {
        match self {
            Self::Default => "Dflt",
        }
    }
}

/// Identificador de spec dentro del catálogo global.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[repr(u16)]
pub enum StationSpecId {
    /// Estación ferroviaria vanilla.
    #[default]
    DefaultRail = 0,
}

impl StationSpecId {
    #[must_use]
    pub const fn from_u16(v: u16) -> Self {
        let _ = v;
        Self::DefaultRail
    }

    #[must_use]
    pub const fn as_u16(self) -> u16 {
        self as u16
    }
}

/// Metadatos de una clase (`StationClass`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StationClassDef {
    pub id: StationClassId,
    pub label: &'static str,
    pub short_label: &'static str,
    /// `true` si proviene de `NewGRF` (hoy siempre `false` en vanilla).
    pub from_newgrf: bool,
}

/// Spec de estación (`StationSpec` simplificado).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StationSpecDef {
    pub id: StationSpecId,
    pub class: StationClassId,
    pub label: &'static str,
    pub short_label: &'static str,
    /// Bits 0..=6 = tamaños 1..=7 deshabilitados; bit 7 = >7.
    pub disallowed_platforms: u8,
    /// Bits 0..=6 = longitudes 1..=7 deshabilitadas; bit 7 = >7.
    pub disallowed_lengths: u8,
    pub from_newgrf: bool,
}

impl StationSpecDef {
    /// ¿El número de andenes (1..=7) está permitido?
    #[must_use]
    pub fn allows_platforms(self, platforms: u8) -> bool {
        let n = platforms.clamp(1, 7);
        (self.disallowed_platforms & (1 << (n - 1))) == 0
    }

    /// ¿La longitud de andén (1..=7) está permitida?
    #[must_use]
    pub fn allows_length(self, length: u8) -> bool {
        let n = length.clamp(1, 7);
        (self.disallowed_lengths & (1 << (n - 1))) == 0
    }
}

const VANILLA_CLASSES: &[StationClassDef] = &[StationClassDef {
    id: StationClassId::Default,
    label: "Por defecto",
    short_label: "Dflt",
    from_newgrf: false,
}];

const VANILLA_SPECS: &[StationSpecDef] = &[StationSpecDef {
    id: StationSpecId::DefaultRail,
    class: StationClassId::Default,
    label: "Estación ferroviaria",
    short_label: "Rail",
    disallowed_platforms: 0,
    disallowed_lengths: 0,
    from_newgrf: false,
}];

/// Catálogo de clases (vanilla; `NewGRF` se añadirá al runtime).
#[must_use]
pub fn all_station_class_defs() -> &'static [StationClassDef] {
    VANILLA_CLASSES
}

/// Catálogo de specs.
#[must_use]
pub fn all_station_spec_defs() -> &'static [StationSpecDef] {
    VANILLA_SPECS
}

#[must_use]
pub fn station_class_def(id: StationClassId) -> Option<&'static StationClassDef> {
    VANILLA_CLASSES.iter().find(|c| c.id == id)
}

#[must_use]
pub fn station_spec_def(id: StationSpecId) -> Option<&'static StationSpecDef> {
    VANILLA_SPECS.iter().find(|s| s.id == id)
}

/// Lista filtrable de clases (`GetStationClassDropDownList` + filtro).
#[must_use]
pub fn list_station_classes(filter: &str) -> Vec<&'static StationClassDef> {
    let needle = filter.trim().to_ascii_lowercase();
    VANILLA_CLASSES
        .iter()
        .filter(|c| {
            if needle.is_empty() {
                return true;
            }
            c.label.to_ascii_lowercase().contains(&needle)
                || c.short_label.to_ascii_lowercase().contains(&needle)
        })
        .collect()
}

/// Lista filtrable de specs de una clase.
#[must_use]
pub fn list_station_specs(class: StationClassId, filter: &str) -> Vec<&'static StationSpecDef> {
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

/// Layout gfx para un spec; hoy solo vanilla → [`crate::rail_station_layout`].
///
/// `NewGRF` usará layouts custom (prop 0E) cuando el runtime exista.
#[must_use]
pub fn station_spec_layout(spec: StationSpecId, platforms: usize, length: usize) -> Vec<u8> {
    let _ = spec;
    crate::rail_station_layout(platforms, length)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn list_filters_class_and_spec() {
        assert_eq!(list_station_classes("").len(), 1);
        assert_eq!(list_station_classes("def").len(), 1);
        assert!(list_station_classes("zzz").is_empty());

        let specs = list_station_specs(StationClassId::Default, "ferro");
        assert_eq!(specs.len(), 1);
        assert_eq!(specs[0].id, StationSpecId::DefaultRail);
        assert!(list_station_specs(StationClassId::Default, "zzz").is_empty());
    }

    #[test]
    fn default_spec_allows_all_sizes() {
        let spec = station_spec_def(StationSpecId::DefaultRail).unwrap();
        for n in 1..=7u8 {
            assert!(spec.allows_platforms(n));
            assert!(spec.allows_length(n));
        }
    }

    #[test]
    fn disallowed_bitmask_blocks_size() {
        let mut spec = VANILLA_SPECS[0];
        spec.disallowed_platforms = 1 << 2; // bloquea 3 andenes
        assert!(!spec.allows_platforms(3));
        assert!(spec.allows_platforms(2));
    }
}
