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
