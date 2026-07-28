//! Specs de cargo `NewGRF` (`Cargoes`, feature Action0 `0x0B`).
//!
//! Catálogo runtime por label: pagos, pesos, clases y multiplicador de capacidad
//! alimentan economía / UI cuando el label coincide con un [`crate::cargo::CargoType`]
//! temperate (o se consulta por label). No inventa aliases de clima (#224).

use serde::{Deserialize, Serialize};

use crate::cargo::CargoType;
use crate::economy::CargoPaymentSpec;

/// Multiplicador de capacidad “×1” en formato OpenTTD (`0x100` = 1.0).
pub const DEFAULT_CARGO_CAPACITY_MULTIPLIER: u16 = 0x100;

/// Spec de cargo definido por Action0.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CargoSpecDef {
    pub id: u8,
    pub bitnum: u8,
    pub label: String,
    pub name: String,
    pub from_newgrf: bool,
    /// GRFID del set (`0` = sin set).
    #[serde(default)]
    pub grfid: u32,
    /// Peso de una unidad (`prop 0x0F`); `0` = default temperate.
    #[serde(default)]
    pub weight: u8,
    /// Precio base (`prop 0x12`); `0` = usar tabla temperate del `CargoType`.
    #[serde(default)]
    pub initial_payment: u32,
    /// `transit_periods[0]` (`prop 0x10`).
    #[serde(default)]
    pub transit_fast: u8,
    /// `transit_periods[1]` (`prop 0x11`).
    #[serde(default)]
    pub transit_slow: u8,
    /// Freight (`prop 0x15`).
    #[serde(default)]
    pub is_freight: bool,
    /// Cargo classes (`prop 0x16`).
    #[serde(default)]
    pub classes: u16,
    /// Multiplicador de capacidad vehículo (`prop 0x1D`); `0` → default `0x100`.
    #[serde(default = "default_capacity_multiplier")]
    pub capacity_multiplier: u16,
    /// Color barra rating (`prop 0x13`).
    #[serde(default)]
    pub rating_colour: u8,
    /// Color leyenda gráfico (`prop 0x14`).
    #[serde(default)]
    pub legend_colour: u8,
}

const fn default_capacity_multiplier() -> u16 {
    DEFAULT_CARGO_CAPACITY_MULTIPLIER
}

impl Default for CargoSpecDef {
    fn default() -> Self {
        Self {
            id: 0,
            bitnum: 0xFF,
            label: String::new(),
            name: String::new(),
            from_newgrf: false,
            grfid: 0,
            weight: 0,
            initial_payment: 0,
            transit_fast: 0,
            transit_slow: 0,
            is_freight: false,
            classes: 0,
            capacity_multiplier: DEFAULT_CARGO_CAPACITY_MULTIPLIER,
            rating_colour: 0,
            legend_colour: 0,
        }
    }
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

/// Busca por label 4 chars (case-insensitive).
#[must_use]
pub fn cargo_spec_by_label<'a>(catalog: &'a [CargoSpecDef], label: &str) -> Option<&'a CargoSpecDef> {
    catalog
        .iter()
        .find(|d| d.label.eq_ignore_ascii_case(label.trim()))
}

/// Label OpenTTD temperate del `CargoType` (`PASS`, `COAL`, …).
#[must_use]
pub const fn cargo_type_label(cargo: CargoType) -> &'static str {
    match cargo {
        CargoType::Passengers => "PASS",
        CargoType::Coal => "COAL",
        CargoType::Mail => "MAIL",
        CargoType::Oil => "OIL_",
        CargoType::Livestock => "LVST",
        CargoType::Goods => "GOOD",
        CargoType::Grain => "GRAI",
        CargoType::Wood => "WOOD",
        CargoType::IronOre => "IORE",
        CargoType::Steel => "STEL",
        CargoType::Valuables => "VALU",
    }
}

/// Spec de pago: override NewGRF si hay `initial_payment`/`transit_*`; si no, temperate.
#[must_use]
pub fn payment_spec_for_cargo(cargo: CargoType, catalog: &[CargoSpecDef]) -> CargoPaymentSpec {
    let vanilla = cargo.payment_spec();
    let Some(def) = cargo_spec_by_label(catalog, cargo_type_label(cargo)) else {
        return vanilla;
    };
    CargoPaymentSpec {
        base_rate: if def.initial_payment > 0 {
            i32::try_from(def.initial_payment).unwrap_or(vanilla.base_rate)
        } else {
            vanilla.base_rate
        },
        transit_fast_days: if def.initial_payment > 0 || def.transit_fast > 0 || def.transit_slow > 0
        {
            u16::from(def.transit_fast)
        } else {
            vanilla.transit_fast_days
        },
        transit_slow_days: if def.initial_payment > 0 || def.transit_fast > 0 || def.transit_slow > 0
        {
            u16::from(def.transit_slow)
        } else {
            vanilla.transit_slow_days
        },
    }
}

/// Capacidad efectiva: `base * multiplier / 0x100` (mínimo 1 si base > 0).
#[must_use]
pub fn apply_cargo_capacity_multiplier(base_capacity: u32, catalog: &[CargoSpecDef], cargo: CargoType) -> u32 {
    if base_capacity == 0 {
        return 0;
    }
    let mult = cargo_spec_by_label(catalog, cargo_type_label(cargo))
        .map(|d| {
            if d.capacity_multiplier == 0 {
                DEFAULT_CARGO_CAPACITY_MULTIPLIER
            } else {
                d.capacity_multiplier
            }
        })
        .unwrap_or(DEFAULT_CARGO_CAPACITY_MULTIPLIER);
    let scaled = (u64::from(base_capacity) * u64::from(mult)) / u64::from(DEFAULT_CARGO_CAPACITY_MULTIPLIER);
    u32::try_from(scaled).unwrap_or(u32::MAX).max(1)
}

/// Nombre UI: prioriza el spec NewGRF si existe.
#[must_use]
pub fn cargo_spec_display_name(cargo: CargoType, catalog: &[CargoSpecDef]) -> String {
    cargo_spec_by_label(catalog, cargo_type_label(cargo))
        .map(|d| d.name.clone())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| cargo.display_name().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payment_override_uses_initial_payment() {
        let catalog = vec![CargoSpecDef {
            id: 1,
            bitnum: 1,
            label: "COAL".into(),
            name: "Carbón XL".into(),
            from_newgrf: true,
            initial_payment: 9000,
            transit_fast: 3,
            transit_slow: 100,
            ..CargoSpecDef::default()
        }];
        let spec = payment_spec_for_cargo(CargoType::Coal, &catalog);
        assert_eq!(spec.base_rate, 9000);
        assert_eq!(spec.transit_fast_days, 3);
        assert_eq!(spec.transit_slow_days, 100);
    }

    #[test]
    fn capacity_multiplier_doubles() {
        let catalog = vec![CargoSpecDef {
            label: "PASS".into(),
            capacity_multiplier: 0x200,
            ..CargoSpecDef::default()
        }];
        assert_eq!(
            apply_cargo_capacity_multiplier(40, &catalog, CargoType::Passengers),
            80
        );
    }

    #[test]
    fn label_lookup_is_case_insensitive() {
        let catalog = vec![CargoSpecDef {
            label: "coal".into(),
            name: "x".into(),
            ..CargoSpecDef::default()
        }];
        assert!(cargo_spec_by_label(&catalog, "COAL").is_some());
    }
}
