//! Specs de cargo `NewGRF` (`Cargoes`, feature Action0 `0x0B`).
//!
//! Catálogo runtime parcial (etiqueta / bitnum / nombre). No altera
//! [`crate::cargo::CargoType`] ni la economía.

use serde::{Deserialize, Serialize};

/// Spec de cargo definido por Action0 (catálogo, no economía).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CargoSpecDef {
    pub id: u8,
    pub bitnum: u8,
    pub label: String,
    pub name: String,
    pub from_newgrf: bool,
    /// GRFID del set (`0` = sin set).
    #[serde(default, skip)]
    pub grfid: u32,
}

/// Catálogo vacío (specs solo desde `NewGRF`).
#[must_use]
pub fn empty_cargo_spec_catalog() -> Vec<CargoSpecDef> {
    Vec::new()
}

#[must_use]
pub fn cargo_spec_def(catalog: &[CargoSpecDef], id: u8) -> Option<&CargoSpecDef> {
    catalog.iter().find(|d| d.id == id)
}
