//! Badges `NewGRF` (`Badges`, feature Action0 `0x15`).
//!
//! Catálogo runtime parcial: etiqueta + flags; sin consumidor de UI aún.

use serde::{Deserialize, Serialize};

/// Spec de badge definido por Action0.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BadgeDef {
    pub id: u16,
    pub label: String,
    pub flags: u32,
    pub from_newgrf: bool,
    /// GRFID del set (`0` = vanilla / sin set).
    #[serde(default, skip)]
    pub grfid: u32,
}

/// Catálogo vacío (no hay badges vanilla).
#[must_use]
pub fn empty_badge_catalog() -> Vec<BadgeDef> {
    Vec::new()
}

/// Siguiente id libre en el catálogo.
#[must_use]
pub fn next_free_badge_id(catalog: &[BadgeDef]) -> Option<u16> {
    (0u16..u16::MAX).find(|&id| !catalog.iter().any(|d| d.id == id))
}

#[must_use]
pub fn badge_def(catalog: &[BadgeDef], id: u16) -> Option<&BadgeDef> {
    catalog.iter().find(|d| d.id == id)
}
