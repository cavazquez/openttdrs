//! Specs de objetos `NewGRF` (`Objects`, feature Action0 `0x0F`).
//!
//! Catálogo runtime parcial: clase, tamaño y nombre; sprites opcionales vía Action1/3.

use serde::{Deserialize, Serialize};

/// Tamaño por defecto 1×1 (`OpenTTD` `OBJECT_SIZE_1X1` = `0x11`).
pub const OBJECT_SIZE_1X1: u8 = 0x11;

/// Spec de objeto definido por Action0.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObjectSpecDef {
    pub id: u16,
    pub class_label: String,
    pub name: String,
    /// Byte tamaño (`low nibble` = ancho, `high` = alto).
    pub size: u8,
    pub from_newgrf: bool,
    /// Id local Action0/Action3 en el GRF.
    #[serde(default, skip)]
    pub local_id: u8,
    /// GRFID del set.
    #[serde(default, skip)]
    pub grfid: u32,
    /// Vistas Action1/3 (opcional; catálogo-only si vacío).
    #[serde(default, skip)]
    pub views: Vec<crate::newgrf_sprites::DecodedSprite>,
}

/// Catálogo vacío (objetos solo desde `NewGRF`).
#[must_use]
pub fn empty_object_spec_catalog() -> Vec<ObjectSpecDef> {
    Vec::new()
}

/// Siguiente id libre en el catálogo.
#[must_use]
pub fn next_free_object_spec_id(catalog: &[ObjectSpecDef]) -> Option<u16> {
    (0u16..u16::MAX).find(|&id| !catalog.iter().any(|d| d.id == id))
}

#[must_use]
pub fn object_spec_def(catalog: &[ObjectSpecDef], id: u16) -> Option<&ObjectSpecDef> {
    catalog.iter().find(|d| d.id == id)
}
