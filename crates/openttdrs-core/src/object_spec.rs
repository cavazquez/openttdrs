//! Specs de objetos `NewGRF` (`Objects`, feature Action0 `0x0F`).
//!
//! Catálogo runtime: clase, tamaño, clima, coste y nombre; sprites opcionales vía Action1/3.

use serde::{Deserialize, Serialize};

/// Tamaño por defecto 1×1 (`OpenTTD` `OBJECT_SIZE_1X1` = `0x11`).
pub const OBJECT_SIZE_1X1: u8 = 0x11;

/// Primer id de objeto definido por `NewGRF` (`OpenTTD` `NEW_OBJECT_OFFSET`).
///
/// Ids 0–4 quedan para vanilla (transmisor, faro, terreno comprado, …).
pub const NEW_OBJECT_OFFSET: u16 = 5;

/// Factor de coste de construcción por defecto (1× precio base).
pub const DEFAULT_OBJECT_BUILD_COST_FACTOR: u8 = 1;

/// Máscara de climas por defecto (todos).
pub const DEFAULT_OBJECT_CLIMATE_MASK: u8 = 0x0F;

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
    #[serde(default)]
    pub local_id: u8,
    /// GRFID del set.
    #[serde(default)]
    pub grfid: u32,
    /// Máscara de climas Action0 `0x0B` (`LandscapeTypes`).
    #[serde(default = "default_object_climate_mask")]
    pub climate_mask: u8,
    /// Multiplicador de coste Action0 `0x0D` (`build_cost_multiplier`).
    #[serde(default = "default_object_build_cost_factor")]
    pub build_cost_factor: u8,
    /// Vistas Action1/3 (opcional; catálogo-only si vacío; no se serializa).
    #[serde(default, skip)]
    pub views: Vec<crate::newgrf_sprites::DecodedSprite>,
    /// Ids de badges asociados (catálogo `badge`).
    #[serde(default)]
    pub associated_badges: Vec<u16>,
}

const fn default_object_climate_mask() -> u8 {
    DEFAULT_OBJECT_CLIMATE_MASK
}

const fn default_object_build_cost_factor() -> u8 {
    DEFAULT_OBJECT_BUILD_COST_FACTOR
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

    /// Número de teselas del footprint.
    #[must_use]
    pub fn tile_count(&self) -> u32 {
        u32::from(self.size_width()).saturating_mul(u32::from(self.size_height()))
    }

    /// `true` si el spec es 1×1.
    #[must_use]
    pub const fn is_1x1(&self) -> bool {
        object_size_is_1x1(self.size)
    }

    /// `true` si el clima activo está permitido.
    #[must_use]
    pub const fn available_in_climate(&self, climate_bit: u8) -> bool {
        self.climate_mask & climate_bit != 0
    }

    /// Vista Action1/3 por índice (módulo `len` si hay varias).
    #[must_use]
    pub fn view(&self, idx: usize) -> Option<&crate::newgrf_sprites::DecodedSprite> {
        if self.views.is_empty() {
            return None;
        }
        self.views.get(idx % self.views.len())
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

/// Codifica offset (dx, dy) dentro del footprint en `m2`.
#[must_use]
pub const fn encode_object_tile_offset(dx: u8, dy: u8) -> u8 {
    (dx & 0x0F) | ((dy & 0x0F) << 4)
}

/// Decodifica offset (dx, dy) desde `m2`.
#[must_use]
pub const fn decode_object_tile_offset(m2: u8) -> (u8, u8) {
    (m2 & 0x0F, m2 >> 4)
}

/// Índice de tesela en el footprint (fila mayor: `dy * width + dx`).
#[must_use]
pub fn object_footprint_tile_index(dx: u8, dy: u8, width: u8) -> usize {
    usize::from(dy).saturating_mul(usize::from(width)) + usize::from(dx)
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

/// Specs del catálogo seleccionables en el picker (cualquier tamaño W×H válido).
#[must_use]
pub fn list_buildable_object_specs(catalog: &[ObjectSpecDef]) -> Vec<&ObjectSpecDef> {
    catalog
        .iter()
        .filter(|d| d.size_width() > 0 && d.size_height() > 0)
        .collect()
}

/// Alias histórico: ahora lista todos los specs construibles (incl. >1×1).
#[must_use]
pub fn list_1x1_object_specs(catalog: &[ObjectSpecDef]) -> Vec<&ObjectSpecDef> {
    list_buildable_object_specs(catalog)
}

/// `true` si `id` es vanilla construible (0/1) o un spec del catálogo con tamaño válido.
#[must_use]
pub fn is_selectable_object_spec(catalog: &[ObjectSpecDef], id: u16) -> bool {
    matches!(id, 0 | 1)
        || object_spec_def(catalog, id).is_some_and(|d| d.size_width() > 0 && d.size_height() > 0)
}
