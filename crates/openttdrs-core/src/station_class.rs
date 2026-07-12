//! Clases y specs de estación ferroviaria (`StationClass` / `StationSpec` de `OpenTTD`).
//!
//! Catálogo: vanilla (id 0) + `NewGRF` Action0 Stations (ids ≥1).

use serde::{Deserialize, Serialize};

/// Identificador de clase de estación (`StationClassID`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub struct StationClassId(pub u16);

impl StationClassId {
    pub const DEFAULT: Self = Self(0);
    /// Compatibilidad con el enum anterior.
    #[allow(non_upper_case_globals)]
    pub const Default: Self = Self::DEFAULT;

    #[must_use]
    pub const fn from_u16(v: u16) -> Self {
        Self(v)
    }

    #[must_use]
    pub const fn as_u16(self) -> u16 {
        self.0
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self.0 {
            0 => "Por defecto",
            _ => "NewGRF",
        }
    }

    #[must_use]
    pub const fn short_label(self) -> &'static str {
        match self.0 {
            0 => "Dflt",
            _ => "NGRF",
        }
    }
}

/// Identificador de spec dentro del catálogo global.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub struct StationSpecId(pub u16);

impl StationSpecId {
    pub const DEFAULT_RAIL: Self = Self(0);
    /// Compatibilidad con el enum anterior.
    #[allow(non_upper_case_globals)]
    pub const DefaultRail: Self = Self::DEFAULT_RAIL;

    #[must_use]
    pub const fn from_u16(v: u16) -> Self {
        Self(v)
    }

    #[must_use]
    pub const fn as_u16(self) -> u16 {
        self.0
    }
}

/// Metadatos de una clase (`StationClass`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StationClassDef {
    pub id: StationClassId,
    pub label: String,
    pub short_label: String,
    pub from_newgrf: bool,
}

/// Spec de estación (`StationSpec` simplificado).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StationSpecDef {
    pub id: StationSpecId,
    pub class: StationClassId,
    pub label: String,
    pub short_label: String,
    /// Bits 0..=6 = tamaños 1..=7 deshabilitados; bit 7 = >7.
    pub disallowed_platforms: u8,
    /// Bits 0..=6 = longitudes 1..=7 deshabilitadas; bit 7 = >7.
    pub disallowed_lengths: u8,
    pub from_newgrf: bool,
}

impl StationSpecDef {
    #[must_use]
    pub fn allows_platforms(&self, platforms: u8) -> bool {
        let n = platforms.clamp(1, 7);
        (self.disallowed_platforms & (1 << (n - 1))) == 0
    }

    #[must_use]
    pub fn allows_length(&self, length: u8) -> bool {
        let n = length.clamp(1, 7);
        (self.disallowed_lengths & (1 << (n - 1))) == 0
    }
}

/// Catálogo vanilla de clases.
#[must_use]
pub fn vanilla_station_class_catalog() -> Vec<StationClassDef> {
    vec![StationClassDef {
        id: StationClassId::DEFAULT,
        label: "Por defecto".into(),
        short_label: "Dflt".into(),
        from_newgrf: false,
    }]
}

/// Catálogo vanilla de specs.
#[must_use]
pub fn vanilla_station_spec_catalog() -> Vec<StationSpecDef> {
    vec![StationSpecDef {
        id: StationSpecId::DEFAULT_RAIL,
        class: StationClassId::DEFAULT,
        label: "Estación ferroviaria".into(),
        short_label: "Rail".into(),
        disallowed_platforms: 0,
        disallowed_lengths: 0,
        from_newgrf: false,
    }]
}

#[must_use]
pub fn all_station_class_defs() -> Vec<StationClassDef> {
    vanilla_station_class_catalog()
}

#[must_use]
pub fn all_station_spec_defs() -> Vec<StationSpecDef> {
    vanilla_station_spec_catalog()
}

#[must_use]
pub fn station_class_def(
    catalog: &[StationClassDef],
    id: StationClassId,
) -> Option<&StationClassDef> {
    catalog.iter().find(|c| c.id == id)
}

#[must_use]
pub fn station_spec_def(catalog: &[StationSpecDef], id: StationSpecId) -> Option<&StationSpecDef> {
    catalog.iter().find(|s| s.id == id)
}

#[must_use]
pub fn list_station_classes<'a>(
    catalog: &'a [StationClassDef],
    filter: &str,
) -> Vec<&'a StationClassDef> {
    let needle = filter.trim().to_ascii_lowercase();
    catalog
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

#[must_use]
pub fn list_station_specs<'a>(
    catalog: &'a [StationSpecDef],
    class: StationClassId,
    filter: &str,
) -> Vec<&'a StationSpecDef> {
    let needle = filter.trim().to_ascii_lowercase();
    catalog
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

/// Layout gfx; `NewGRF` sin prop 0E → vanilla.
#[must_use]
pub fn station_spec_layout(spec: StationSpecId, platforms: usize, length: usize) -> Vec<u8> {
    let _ = spec;
    crate::rail_station_layout(platforms, length)
}

#[must_use]
pub fn next_free_station_class_id(catalog: &[StationClassDef]) -> Option<StationClassId> {
    for id in 1u16..=1023 {
        let c = StationClassId::from_u16(id);
        if !catalog.iter().any(|d| d.id == c) {
            return Some(c);
        }
    }
    None
}

#[must_use]
pub fn next_free_station_spec_id(catalog: &[StationSpecDef]) -> Option<StationSpecId> {
    for id in 1u16..=1023 {
        let s = StationSpecId::from_u16(id);
        if !catalog.iter().any(|d| d.id == s) {
            return Some(s);
        }
    }
    None
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn list_filters_class_and_spec() {
        let classes = vanilla_station_class_catalog();
        let specs = vanilla_station_spec_catalog();
        assert_eq!(list_station_classes(&classes, "").len(), 1);
        assert_eq!(list_station_classes(&classes, "def").len(), 1);
        assert!(list_station_classes(&classes, "zzz").is_empty());

        let filtered = list_station_specs(&specs, StationClassId::Default, "ferro");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].id, StationSpecId::DefaultRail);
        assert!(list_station_specs(&specs, StationClassId::Default, "zzz").is_empty());
    }

    #[test]
    fn default_spec_allows_all_sizes() {
        let specs = vanilla_station_spec_catalog();
        let spec = station_spec_def(&specs, StationSpecId::DefaultRail).unwrap();
        for n in 1..=7u8 {
            assert!(spec.allows_platforms(n));
            assert!(spec.allows_length(n));
        }
    }

    #[test]
    fn disallowed_bitmask_blocks_size() {
        let mut spec = vanilla_station_spec_catalog().remove(0);
        spec.disallowed_platforms = 1 << 2;
        assert!(!spec.allows_platforms(3));
        assert!(spec.allows_platforms(2));
    }
}
