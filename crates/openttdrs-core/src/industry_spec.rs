//! Specs de industria `NewGRF` (`Industries`, feature Action0 `0x0A`).
//!
//! Catálogo runtime ids ≥ [`NEW_INDUSTRY_OFFSET`]. Layouts resuelven `0xFE` → gfx
//! global de `IndustryTiles` del mismo GRF. Cargos como índices/labels vía
//! [`crate::cargo_spec`] (`GetCargoTranslation`); `callback_mask` se almacena
//! sin ejecutar (#228).

use serde::{Deserialize, Serialize};

use crate::cargo::CargoType;
use crate::cargo_spec::{CargoSpecDef, cargo_spec_def, cargo_type_label};
use crate::map::TileCoord;

/// Primer tipo de industria definido por `NewGRF` (`OpenTTD` `NEW_INDUSTRYOFFSET`).
pub const NEW_INDUSTRY_OFFSET: u16 = 37;
/// Total de slots de industria (`OpenTTD` `NUM_INDUSTRYTYPES`).
pub const NUM_INDUSTRY_TYPES: u16 = 240;
/// Id inválido / sin override (`OpenTTD` `IT_INVALID` simplificado).
pub const INVALID_INDUSTRY: u16 = NUM_INDUSTRY_TYPES;
/// Salidas originales (`INDUSTRY_ORIGINAL_NUM_OUTPUTS`).
pub const INDUSTRY_ORIGINAL_NUM_OUTPUTS: usize = 2;
/// Entradas originales (`INDUSTRY_ORIGINAL_NUM_INPUTS`).
pub const INDUSTRY_ORIGINAL_NUM_INPUTS: usize = 3;
/// Máximo moderno de cargos aceptados por una industria (`INDUSTRY_NUM_INPUTS`).
pub const INDUSTRY_NUM_INPUTS: usize = 16;
/// Máximo moderno de cargos producidos por una industria (`INDUSTRY_NUM_OUTPUTS`).
pub const INDUSTRY_NUM_OUTPUTS: usize = 16;
/// Bit `IndustryCallbackMask::ProductionCargoArrival`: callback al llegar carga.
pub const INDUSTRY_CALLBACK_PRODUCTION_CARGO_ARRIVAL_MASK: u16 = 1 << 1;
/// Bit `IndustryCallbackMask::Production256Ticks`: callback de producción periódico.
pub const INDUSTRY_CALLBACK_PRODUCTION_256_TICKS_MASK: u16 = 1 << 2;
/// Bit `IndustryCallbackMask::Location`: consulta CB `0x28` antes de construir.
pub const INDUSTRY_CALLBACK_LOCATION_MASK: u16 = 1 << 3;
/// Bit `IndustryCallbackMask::ProductionChange`: cambio diario de producción.
pub const INDUSTRY_CALLBACK_PRODUCTION_CHANGE_MASK: u16 = 1 << 4;
/// Bit `IndustryCallbackMask::MonthlyProdChange`: cambio mensual de producción.
pub const INDUSTRY_CALLBACK_MONTHLY_PROD_CHANGE_MASK: u16 = 1 << 5;
/// Bit `IndustryCallbackMask::ProdChangeBuild`: nivel inicial al fundar.
pub const INDUSTRY_CALLBACK_PROD_CHANGE_BUILD_MASK: u16 = 1 << 14;
/// Tesela de un layout (`IndustryTileLayoutTile`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndustryLayoutTile {
    pub x: i8,
    pub y: i8,
    /// Gfx global (vanilla &lt;175 o `NewGRF` ≥175).
    pub gfx: u16,
}

/// Layout multitile (lista de offsets desde la tesela norte).
pub type IndustryTileLayout = Vec<IndustryLayoutTile>;

/// Spec `NewGRF` de industria (feature Action0 `0x0A`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndustrySpecDef {
    /// Id global (`≥` [`NEW_INDUSTRY_OFFSET`]).
    pub id: u16,
    pub local_id: u8,
    /// Substitute vanilla (`prop 0x08`).
    pub subst_id: u8,
    /// Override de industria vanilla (`prop 0x09`).
    pub override_id: Option<u8>,
    /// Layouts (`prop 0x0A`); gfx ya resueltos a ids globales.
    pub layouts: Vec<IndustryTileLayout>,
    /// Índices GRF-local de cargos producidos (`0x10` / `0x25`).
    pub produced_cargo_indices: Vec<u8>,
    /// Labels resueltos (`GetCargoTranslation` / `cargo_spec`).
    pub produced_cargo_labels: Vec<String>,
    /// Índices GRF-local de cargos aceptados (`0x11` / `0x26`).
    pub accepted_cargo_indices: Vec<u8>,
    /// Labels resueltos de aceptación.
    pub accepted_cargo_labels: Vec<String>,
    /// Rates de producción (`0x12`/`0x13` / `0x27`).
    pub production_rates: Vec<u8>,
    /// Multiplicadores de input (`0x1C`–`0x1E` / `0x28`); aplanados `[in][out]`.
    pub input_multipliers: Vec<u16>,
    /// Callback mask (`0x21` lo + `0x22` hi); call sites parciales #266.
    pub callback_mask: u16,
    /// Cost multiplier (`0x0F`).
    pub cost_multiplier: u8,
    pub name: String,
    pub from_newgrf: bool,
    pub grfid: u32,
    /// Runtime Action2 para CBs de industria (#266); no se serializa.
    #[serde(default, skip)]
    pub newgrf_local_id: u8,
    #[serde(default, skip)]
    pub newgrf_runtime: Option<Box<crate::newgrf_sprites::TrainSpriteGraphics>>,
}

impl IndustrySpecDef {
    /// ¿El GRF declaró callback al recibir cargo en la industria?
    #[must_use]
    pub const fn has_production_cargo_arrival_callback(&self) -> bool {
        self.callback_mask & INDUSTRY_CALLBACK_PRODUCTION_CARGO_ARRIVAL_MASK != 0
    }

    /// ¿El GRF declaró callback de producción cada 256 ticks?
    #[must_use]
    pub const fn has_production_256_ticks_callback(&self) -> bool {
        self.callback_mask & INDUSTRY_CALLBACK_PRODUCTION_256_TICKS_MASK != 0
    }

    /// ¿El GRF declaró un callback de cambio de producción diario?
    #[must_use]
    pub const fn has_production_change_callback(&self) -> bool {
        self.callback_mask & INDUSTRY_CALLBACK_PRODUCTION_CHANGE_MASK != 0
    }

    /// ¿El GRF declaró un callback de cambio de producción mensual?
    #[must_use]
    pub const fn has_monthly_production_change_callback(&self) -> bool {
        self.callback_mask & INDUSTRY_CALLBACK_MONTHLY_PROD_CHANGE_MASK != 0
    }

    /// ¿El GRF declaró un callback que fija la producción inicial?
    #[must_use]
    pub const fn has_production_change_build_callback(&self) -> bool {
        self.callback_mask & INDUSTRY_CALLBACK_PROD_CHANGE_BUILD_MASK != 0
    }

    /// ¿El GRF declaró CB `0x28` para autorizar la ubicación de la industria?
    #[must_use]
    pub const fn has_location_callback(&self) -> bool {
        self.callback_mask & INDUSTRY_CALLBACK_LOCATION_MASK != 0
    }

    /// Primer layout (o vacío).
    #[must_use]
    pub fn primary_layout(&self) -> &[IndustryLayoutTile] {
        self.layouts.first().map_or(&[], Vec::as_slice)
    }

    /// Footprint absoluto desde origen norte.
    #[must_use]
    pub fn footprint_at(&self, origin: TileCoord, layout_idx: usize) -> Vec<(TileCoord, u16)> {
        let Some(layout) = self.layouts.get(layout_idx) else {
            return Vec::new();
        };
        layout
            .iter()
            .map(|t| {
                (
                    TileCoord::new(
                        origin.x.wrapping_add(i32::from(t.x)),
                        origin.y.wrapping_add(i32::from(t.y)),
                    ),
                    t.gfx,
                )
            })
            .collect()
    }

    /// Rate primario (salida 0).
    #[must_use]
    pub fn primary_production_rate(&self) -> u8 {
        self.production_rates.first().copied().unwrap_or(0)
    }

    /// Rate de producción de la segunda salida (`prop 0x13`).
    #[must_use]
    pub fn secondary_production_rate(&self) -> Option<u8> {
        self.production_rates.get(1).copied()
    }

    /// Primer cargo de salida mapeable a [`CargoType`] conocido.
    #[must_use]
    pub fn primary_output_cargo(&self) -> Option<CargoType> {
        cargo_type_from_label(self.produced_cargo_labels.first().map(String::as_str))
    }

    /// Segundo cargo de salida mapeable a [`CargoType`] conocido.
    #[must_use]
    pub fn secondary_output_cargo(&self) -> Option<CargoType> {
        cargo_type_from_label(self.produced_cargo_labels.get(1).map(String::as_str))
    }

    /// Cargos de salida mapeables, conservando el orden declarado por el GRF.
    #[must_use]
    pub fn produced_cargo_types(&self) -> Vec<CargoType> {
        self.produced_cargo_labels
            .iter()
            .filter_map(|label| cargo_type_from_label(Some(label.as_str())))
            .collect()
    }

    /// Cargos de entrada mapeables a [`CargoType`].
    #[must_use]
    pub fn accepted_cargo_types(&self) -> Vec<CargoType> {
        self.accepted_cargo_labels
            .iter()
            .filter_map(|l| cargo_type_from_label(Some(l.as_str())))
            .collect()
    }

    /// ¿Procesadora? (tiene inputs aceptados).
    #[must_use]
    pub fn is_processor(&self) -> bool {
        !self.accepted_cargo_labels.is_empty()
    }
}

/// Catálogo vacío (solo `NewGRF`).
#[must_use]
pub fn empty_industry_spec_catalog() -> Vec<IndustrySpecDef> {
    Vec::new()
}

/// Tabla de overrides vanilla → id `NewGRF` (`prop 0x09`).
#[must_use]
pub fn empty_industry_overrides() -> Vec<u16> {
    vec![INVALID_INDUSTRY; NEW_INDUSTRY_OFFSET as usize]
}

#[must_use]
pub fn industry_spec_def(catalog: &[IndustrySpecDef], id: u16) -> Option<&IndustrySpecDef> {
    catalog.iter().find(|d| d.id == id)
}

/// Siguiente id libre en `[NEW_INDUSTRY_OFFSET, NUM_INDUSTRY_TYPES)`.
#[must_use]
pub fn next_free_industry_id(catalog: &[IndustrySpecDef]) -> Option<u16> {
    (NEW_INDUSTRY_OFFSET..NUM_INDUSTRY_TYPES).find(|&id| !catalog.iter().any(|d| d.id == id))
}

/// Traduce id limpio aplicando override `NewGRF` (`GetTranslatedIndustryID`).
#[must_use]
pub fn get_translated_industry_id(clean: u16, overrides: &[u16]) -> u16 {
    if clean == 0xFF {
        return clean;
    }
    if let Some(&ovr) = overrides.get(usize::from(clean))
        && ovr != INVALID_INDUSTRY
    {
        return ovr;
    }
    clean
}

/// `GetCargoTranslation`: índice GRF-local → label vía `cargo_spec` / tabla bitnum.
///
/// No inventa aliases de clima: si no hay `cargo_spec` ni bitnum conocido en
/// [`TEMPERATE_CARGO_TYPES`] (misma tabla climate-independent del default),
/// devuelve `None`.
#[must_use]
pub fn get_cargo_translation(cargo: u8, catalog: &[CargoSpecDef]) -> Option<String> {
    get_cargo_translation_for_climate(cargo, catalog, crate::Climate::Temperate)
}

/// Traduce un cargo usando los slots vanilla activos del clima.
///
/// Los GRF anteriores a la tabla climate-independent usan el slot local
/// (`GetClimateDependentCargoTranslationTable`): el slot 6 es `WHEA` en
/// Arctic, `MAIZ` en Tropic y `TOFF` en Toyland. Para los índices fuera de
/// esos slots también se intenta el `bitnum` de los cargos activos, cubriendo
/// la tabla independiente de clima usada por GRF modernos. Un `CargoSpecDef`
/// explícito siempre tiene prioridad porque puede redefinir el label.
#[must_use]
pub fn get_cargo_translation_for_climate(
    cargo: u8,
    catalog: &[CargoSpecDef],
    climate: crate::Climate,
) -> Option<String> {
    if cargo == 0xFF {
        return None;
    }
    if let Some(def) = catalog
        .iter()
        .find(|d| d.bitnum == cargo && d.bitnum != 0xFF)
    {
        return Some(def.label.clone());
    }
    if let Some(def) = cargo_spec_def(catalog, cargo) {
        return Some(def.label.clone());
    }
    if let Some(default) = CargoType::from_climate_slot(climate, cargo) {
        return Some(cargo_type_label(default).to_string());
    }
    CargoType::for_climate(climate)
        .iter()
        .find(|&&default| default.bitnum() == cargo)
        .map(|&default| cargo_type_label(default).to_string())
}

/// Resuelve label a [`CargoType`] conocido (case-insensitive).
#[must_use]
pub fn cargo_type_from_label(label: Option<&str>) -> Option<CargoType> {
    let label = label?.trim();
    if label.is_empty() {
        return None;
    }
    CargoType::from_label(label)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn translate_applies_override() {
        let mut ovr = empty_industry_overrides();
        ovr[3] = 40;
        assert_eq!(get_translated_industry_id(3, &ovr), 40);
        assert_eq!(get_translated_industry_id(2, &ovr), 2);
        assert_eq!(ovr.len(), NEW_INDUSTRY_OFFSET as usize);
    }

    #[test]
    fn next_free_starts_at_37() {
        assert_eq!(next_free_industry_id(&[]), Some(37));
        let catalog = vec![IndustrySpecDef {
            id: 37,
            local_id: 0,
            subst_id: 0,
            override_id: None,
            layouts: Vec::new(),
            produced_cargo_indices: Vec::new(),
            produced_cargo_labels: Vec::new(),
            accepted_cargo_indices: Vec::new(),
            accepted_cargo_labels: Vec::new(),
            production_rates: Vec::new(),
            input_multipliers: Vec::new(),
            callback_mask: 0,
            cost_multiplier: 0,
            name: String::new(),
            from_newgrf: true,
            grfid: 1,
            newgrf_local_id: 0,
            newgrf_runtime: None,
        }];
        assert_eq!(next_free_industry_id(&catalog), Some(38));
    }

    #[test]
    fn cargo_translation_uses_bitnum_table() {
        assert_eq!(get_cargo_translation(1, &[]).as_deref(), Some("COAL"));
        assert_eq!(get_cargo_translation(7, &[]).as_deref(), Some("WOOD"));
        assert_eq!(get_cargo_translation(0xFF, &[]), None);
    }

    #[test]
    fn cargo_translation_uses_active_climate_slots() {
        assert_eq!(
            get_cargo_translation_for_climate(6, &[], crate::Climate::SubArctic).as_deref(),
            Some("WHEA")
        );
        assert_eq!(
            get_cargo_translation_for_climate(1, &[], crate::Climate::SubTropical).as_deref(),
            Some("RUBR")
        );
        assert_eq!(
            get_cargo_translation_for_climate(3, &[], crate::Climate::Toyland).as_deref(),
            Some("TOYS")
        );
        assert_eq!(
            get_cargo_translation_for_climate(11, &[], crate::Climate::Toyland).as_deref(),
            Some("FZDR")
        );
        assert_eq!(
            get_cargo_translation_for_climate(8, &[], crate::Climate::SubArctic),
            None
        );
    }

    #[test]
    fn cargo_type_from_label_accepts_all_vanilla_climates() {
        assert_eq!(cargo_type_from_label(Some("WHEA")), Some(CargoType::Wheat));
        assert_eq!(cargo_type_from_label(Some("rubr")), Some(CargoType::Rubber));
        assert_eq!(cargo_type_from_label(Some("TOFF")), Some(CargoType::Toffee));
        assert_eq!(cargo_type_from_label(Some("unknown")), None);
    }
}
