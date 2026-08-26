//! Specs de objetos `NewGRF` (`Objects`, feature Action0 `0x0F`).
//!
//! Catálogo runtime: clase, tamaño, clima, coste y nombre; sprites/callbacks opcionales vía
//! Action1/2/3.

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

/// Bit `SlopeCheck` de la máscara de callbacks Action0 `0x15`.
///
/// Corresponde a [`CBID_OBJECT_LAND_SLOPE_CHECK`](crate::CBID_OBJECT_LAND_SLOPE_CHECK).
pub const OBJECT_CALLBACK_SLOPE_CHECK_MASK: u16 = 1 << 0;

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
    /// Máscara de callbacks Action0 `0x15` (WORD).
    #[serde(default)]
    pub callback_mask: u16,
    /// Vistas Action1/3 (opcional; catálogo-only si vacío; no se serializa).
    #[serde(default, skip)]
    pub views: Vec<crate::newgrf_sprites::DecodedSprite>,
    /// Grafo Action2/3 para callbacks de construcción (no se serializa).
    #[serde(default, skip)]
    pub newgrf_runtime: Option<Box<crate::newgrf_sprites::TrainSpriteGraphics>>,
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

    /// `true` si el objeto solicita CB `0x157` para cada tesela de su footprint.
    #[must_use]
    pub const fn has_slope_check_callback(&self) -> bool {
        self.callback_mask & OBJECT_CALLBACK_SLOPE_CHECK_MASK != 0
    }

    /// Vista Action1/3 por índice (módulo `len` si hay varias).
    #[must_use]
    pub fn view(&self, idx: usize) -> Option<&crate::newgrf_sprites::DecodedSprite> {
        if self.views.is_empty() {
            return None;
        }
        self.views.get(idx % self.views.len())
    }

    /// Resuelve una vista Action1/3 usando el grafo Action2 del objeto.
    ///
    /// `OpenTTD` vuelve a evaluar el grupo de sprites para cada tesela del
    /// objeto.  La vista estática de [`Self::view`] sólo representa el preview
    /// y no puede observar variables como pendiente, aleatorio o animación.
    /// El cliente conserva el resultado como `DecodedSprite` propio para que
    /// el caché de texturas pueda asociarlo al contexto runtime.
    pub fn newgrf_view_runtime(
        &self,
        idx: usize,
        ctx: &mut crate::newgrf_sprites::Action2EvalCtx,
    ) -> Option<crate::newgrf_sprites::DecodedSprite> {
        let runtime = self.newgrf_runtime.as_ref()?;
        let views = runtime.views_for_local_id_ctx(self.local_id, ctx)?;
        if views.is_empty() {
            return None;
        }
        Some(views[idx % views.len()].clone())
    }

    #[must_use]
    pub fn has_views(&self) -> bool {
        !self.views.is_empty() || self.newgrf_runtime.is_some()
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

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::newgrf_sprites::{
        Action2VarAdjust, Action2VarEntry, Action2VarTerm, DecodedSprite, TrainSpriteAssign,
        TrainSpriteGraphics,
    };

    fn solid(r: u8, g: u8, b: u8) -> DecodedSprite {
        DecodedSprite {
            width: 1,
            height: 1,
            x_offs: 0,
            y_offs: 0,
            rgba: vec![r, g, b, 255],
            mask: Vec::new(),
        }
    }

    #[test]
    fn runtime_view_uses_object_scope_variables() {
        let red = solid(255, 0, 0);
        let blue = solid(0, 0, 255);
        let mut runtime = TrainSpriteGraphics {
            sets: vec![vec![red.clone()], vec![blue.clone()]],
            assigns: vec![TrainSpriteAssign {
                local_id: 4,
                set_id: 6,
            }],
            ..Default::default()
        };
        runtime.action2_var.insert(
            6,
            Action2VarEntry {
                first: Action2VarTerm {
                    variable: 0x41,
                    param: None,
                    adjust: Action2VarAdjust {
                        and_mask: 0xFF,
                        ..Default::default()
                    },
                },
                ops: Vec::new(),
                ranges: vec![(8, 3, 3)],
                default: 9,
            },
        );
        runtime.action2_to_action1.insert(8, 0);
        runtime.action2_to_action1.insert(9, 1);
        let def = ObjectSpecDef {
            id: NEW_OBJECT_OFFSET,
            class_label: "TEST".into(),
            name: "runtime".into(),
            size: OBJECT_SIZE_1X1,
            from_newgrf: true,
            local_id: 4,
            grfid: 0,
            climate_mask: DEFAULT_OBJECT_CLIMATE_MASK,
            build_cost_factor: DEFAULT_OBJECT_BUILD_COST_FACTOR,
            callback_mask: 0,
            views: vec![red, blue],
            newgrf_runtime: Some(Box::new(runtime)),
            associated_badges: Vec::new(),
        };
        let mut first = crate::newgrf_sprites::Action2EvalCtx::default();
        first.vars.insert(0x41, 3);
        assert_eq!(def.newgrf_view_runtime(0, &mut first).unwrap().rgba[0], 255);
        let mut second = crate::newgrf_sprites::Action2EvalCtx::default();
        second.vars.insert(0x41, 4);
        assert_eq!(
            def.newgrf_view_runtime(0, &mut second).unwrap().rgba[2],
            255
        );
    }
}
