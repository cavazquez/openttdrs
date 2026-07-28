//! Specs de objetos `NewGRF` (`Objects`, feature Action0 `0x0F`).
//!
//! Catálogo runtime parcial: clase, tamaño y nombre; sprites opcionales vía Action1/3.

use serde::{Deserialize, Serialize};

/// Tamaño por defecto 1×1 (`OpenTTD` `OBJECT_SIZE_1X1` = `0x11`).
pub const OBJECT_SIZE_1X1: u8 = 0x11;

/// Primer id de objeto definido por `NewGRF` (`OpenTTD` `NEW_OBJECT_OFFSET`).
///
/// Ids 0–4 quedan para vanilla (transmisor, faro, terreno comprado, …).
pub const NEW_OBJECT_OFFSET: u16 = 5;

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

impl ObjectSpecDef {
    /// Ancho en teselas (`size` nibble bajo).
    #[must_use]
    pub const fn size_width(&self) -> u8 {
        self.size & 0x0F
    }

    /// Alto en teselas (`size` nibble alto).
    #[must_use]
    pub const fn size_height(&self) -> u8 {
        self.size >> 4
    }

    /// `true` si el spec es 1×1.
    #[must_use]
    pub const fn is_1x1(&self) -> bool {
        object_size_is_1x1(self.size)
    }

    /// Vista Action1/3 por índice.
    #[must_use]
    pub fn view(&self, idx: usize) -> Option<&crate::newgrf_sprites::DecodedSprite> {
        self.views.get(idx)
    }

    #[must_use]
    pub fn has_views(&self) -> bool {
        !self.views.is_empty()
    }
}

/// `true` si el byte de tamaño codifica 1×1.
#[must_use]
pub const fn object_size_is_1x1(size: u8) -> bool {
    size == OBJECT_SIZE_1X1
}

/// Catálogo vacío (objetos solo desde `NewGRF`).
#[must_use]
pub fn empty_object_spec_catalog() -> Vec<ObjectSpecDef> {
    Vec::new()
}

/// Siguiente id libre en el catálogo (`≥` [`NEW_OBJECT_OFFSET`]).
#[must_use]
pub fn next_free_object_spec_id(catalog: &[ObjectSpecDef]) -> Option<u16> {
    (NEW_OBJECT_OFFSET..u16::MAX).find(|&id| !catalog.iter().any(|d| d.id == id))
}

#[must_use]
pub fn object_spec_def(catalog: &[ObjectSpecDef], id: u16) -> Option<&ObjectSpecDef> {
    catalog.iter().find(|d| d.id == id)
}
