//! Clases y specs de paradas de carretera `NewGRF` (`RoadStops`, feature Action0 `0x14`).
//!
//! Catálogo runtime parcial: clase, tipo de parada y nombre; sprites opcionales vía Action1/3.

use serde::{Deserialize, Serialize};

/// Metadatos de una clase de road stop (`RoadStopClass`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoadStopClassDef {
    pub id: u16,
    pub label: String,
    pub short_label: String,
    pub from_newgrf: bool,
}

/// Spec de road stop (`RoadStopSpec` simplificado).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoadStopSpecDef {
    pub id: u16,
    pub class: u16,
    pub label: String,
    pub short_label: String,
    /// `0` = bus, `1` = truck (común en OTTD `RoadStopAvailabilityType`).
    pub stop_type: u8,
    pub from_newgrf: bool,
    /// GRFID del `NewGRF` que definió este spec (`0` = vanilla / sin set).
    #[serde(default, skip)]
    pub grfid: u32,
    /// Vistas Action1/3 (opcional; catálogo-only si vacío).
    #[serde(default, skip)]
    pub newgrf_views: Vec<crate::newgrf_sprites::DecodedSprite>,
    /// Ids de badges asociados (catálogo `badge`).
    #[serde(default)]
    pub associated_badges: Vec<u16>,
}

impl RoadStopSpecDef {
    /// Vista Action1/3 para índice de dirección (módulo `len` si hay varias).
    #[must_use]
    pub fn newgrf_view(&self, idx: usize) -> Option<&crate::newgrf_sprites::DecodedSprite> {
        if self.newgrf_views.is_empty() {
            return None;
        }
        self.newgrf_views.get(idx % self.newgrf_views.len())
    }

    /// `true` si el `stop_type` coincide con la clase de parada a construir.
    #[must_use]
    pub fn matches_stop_kind(&self, kind: crate::station::StopKind) -> bool {
        match kind {
            crate::station::StopKind::BusStop => self.stop_type == 0,
            crate::station::StopKind::TruckStop => self.stop_type == 1,
            _ => false,
        }
    }
}

/// Catálogo vacío de clases (solo desde `NewGRF`).
#[must_use]
pub fn empty_road_stop_class_catalog() -> Vec<RoadStopClassDef> {
    Vec::new()
}

/// Catálogo vacío de specs (solo desde `NewGRF`).
#[must_use]
pub fn empty_road_stop_spec_catalog() -> Vec<RoadStopSpecDef> {
    Vec::new()
}

/// Siguiente id libre de clase.
#[must_use]
pub fn next_free_road_stop_class_id(catalog: &[RoadStopClassDef]) -> Option<u16> {
    (0u16..u16::MAX).find(|&id| !catalog.iter().any(|d| d.id == id))
}

/// Siguiente id libre de spec.
#[must_use]
pub fn next_free_road_stop_spec_id(catalog: &[RoadStopSpecDef]) -> Option<u16> {
    (0u16..u16::MAX).find(|&id| !catalog.iter().any(|d| d.id == id))
}

#[must_use]
pub fn road_stop_class_def(catalog: &[RoadStopClassDef], id: u16) -> Option<&RoadStopClassDef> {
    catalog.iter().find(|d| d.id == id)
}

#[must_use]
pub fn road_stop_spec_def(catalog: &[RoadStopSpecDef], id: u16) -> Option<&RoadStopSpecDef> {
    catalog.iter().find(|d| d.id == id)
}

/// Primer spec `from_newgrf` compatible con `kind` (auto-select S-slice).
#[must_use]
pub fn first_matching_road_stop_spec(
    catalog: &[RoadStopSpecDef],
    kind: crate::station::StopKind,
) -> Option<&RoadStopSpecDef> {
    catalog
        .iter()
        .find(|d| d.from_newgrf && d.matches_stop_kind(kind))
}

/// Clases con al menos un spec compatible con `kind`.
#[must_use]
pub fn list_road_stop_classes<'a>(
    classes: &'a [RoadStopClassDef],
    specs: &[RoadStopSpecDef],
    kind: crate::station::StopKind,
) -> Vec<&'a RoadStopClassDef> {
    classes
        .iter()
        .filter(|c| {
            specs
                .iter()
                .any(|s| s.class == c.id && s.matches_stop_kind(kind))
        })
        .collect()
}

/// Specs de una clase (o todas si `class` es `None`) filtrados por `kind`.
#[must_use]
pub fn list_road_stop_specs(
    specs: &[RoadStopSpecDef],
    class: Option<u16>,
    kind: crate::station::StopKind,
) -> Vec<&RoadStopSpecDef> {
    specs
        .iter()
        .filter(|s| s.matches_stop_kind(kind))
        .filter(|s| class.is_none_or(|c| s.class == c))
        .collect()
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::station::StopKind;

    fn sample_catalog() -> (Vec<RoadStopClassDef>, Vec<RoadStopSpecDef>) {
        let classes = vec![
            RoadStopClassDef {
                id: 0,
                label: "Bus".into(),
                short_label: "BUS".into(),
                from_newgrf: true,
            },
            RoadStopClassDef {
                id: 1,
                label: "Truck".into(),
                short_label: "TRK".into(),
                from_newgrf: true,
            },
        ];
        let specs = vec![
            RoadStopSpecDef {
                id: 10,
                class: 0,
                label: "Bus A".into(),
                short_label: "BA".into(),
                stop_type: 0,
                from_newgrf: true,
                grfid: 0,
                newgrf_views: Vec::new(),
                associated_badges: Vec::new(),
            },
            RoadStopSpecDef {
                id: 11,
                class: 1,
                label: "Truck A".into(),
                short_label: "TA".into(),
                stop_type: 1,
                from_newgrf: true,
                grfid: 0,
                newgrf_views: Vec::new(),
                associated_badges: Vec::new(),
            },
        ];
        (classes, specs)
    }

    #[test]
    fn list_helpers_filter_by_stop_kind() {
        let (classes, specs) = sample_catalog();
        let bus_classes = list_road_stop_classes(&classes, &specs, StopKind::BusStop);
        assert_eq!(bus_classes.len(), 1);
        assert_eq!(bus_classes[0].id, 0);
        let truck_specs = list_road_stop_specs(&specs, Some(1), StopKind::TruckStop);
        assert_eq!(truck_specs.len(), 1);
        assert_eq!(truck_specs[0].id, 11);
        assert!(list_road_stop_specs(&specs, Some(0), StopKind::TruckStop).is_empty());
    }
}
