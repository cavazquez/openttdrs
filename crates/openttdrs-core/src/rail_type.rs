//! Tipos de vía (`RailType` / `RailTypeInfo` de `OpenTTD`).
//!
//! Persistidos en `Tile.m8` bits 0–5 (`GetRailType` / `SetRailType` en `rail_map.h`).
//! MVP Fase 5: normal + eléctrico (mono/maglev → Fase 6).

use serde::{Deserialize, Serialize};

use crate::engine::{
    ENGINE_TRAIN_ASIASTAR, ENGINE_TRAIN_SH_30, ENGINE_TRAIN_SH_40, ENGINE_TRAIN_TIM, EngineDef,
};
use crate::map::Tile;

/// Identificador de tipo de vía (valores alineados con `RailType` upstream).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[repr(u8)]
pub enum RailType {
    #[default]
    Rail = 0,
    Electric = 1,
    // Reservados Fase 6:
    // Monorail = 2,
    // Maglev = 3,
}

impl RailType {
    #[must_use]
    pub const fn from_u8(v: u8) -> Self {
        match v & 0x3F {
            1 => Self::Electric,
            _ => Self::Rail,
        }
    }

    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Rail => "Normal",
            Self::Electric => "Eléctrica",
        }
    }

    /// ¿La vía lleva catenaria (render / compat)?
    #[must_use]
    pub const fn has_catenary(self) -> bool {
        matches!(self, Self::Electric)
    }
}

/// Máscara de bits 0–5 de `m8` (tipo de vía).
const RAILTYPE_M8_MASK: u16 = 0x003F;

/// Lee el tipo de vía de una tesela (`GetRailType`).
#[must_use]
pub fn rail_type_from_tile(tile: Tile) -> RailType {
    RailType::from_u8(u8::try_from(tile.m8 & RAILTYPE_M8_MASK).unwrap_or(0))
}

/// Escribe el tipo de vía en `m8` preservando el resto de bits.
#[must_use]
pub fn set_rail_type_on_tile(mut tile: Tile, rail_type: RailType) -> Tile {
    tile.m8 = (tile.m8 & !RAILTYPE_M8_MASK) | u16::from(rail_type.as_u8());
    tile
}

/// Coste de convertir una tesela de vía (`CmdConvertRail` simplificado).
pub const RAIL_CONVERT_COST: i64 = 15;

/// ¿El motor puede circular / comprarse sobre este tipo de vía?
///
/// Eléctricos (SH 30/40, TIM, `AsiaStar`) requieren vía electrificada.
/// Vapor/diésel/vagones aceptan ambos (como en `OpenTTD` temperate vanilla).
#[must_use]
pub fn engine_compatible_with_rail(engine: &EngineDef, rail_type: RailType) -> bool {
    if !engine.is_train_engine() && !engine.is_wagon() {
        return true;
    }
    if engine_requires_electric(engine.id) {
        return rail_type == RailType::Electric;
    }
    true
}

/// Motores eléctricos del catálogo temperate (ids 110–113).
#[must_use]
pub const fn engine_requires_electric(engine_id: u16) -> bool {
    matches!(
        engine_id,
        ENGINE_TRAIN_SH_30 | ENGINE_TRAIN_SH_40 | ENGINE_TRAIN_TIM | ENGINE_TRAIN_ASIASTAR
    )
}

/// Tipo de vía requerido por el motor (para UI / mensajes).
#[must_use]
pub fn required_rail_type_for_engine(engine_id: u16) -> RailType {
    if engine_requires_electric(engine_id) {
        RailType::Electric
    } else {
        RailType::Rail
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::engine::{ENGINE_TRAIN_KIRBY, engine_by_id};
    use crate::map::{Tile, TileKind};

    fn rail_tile(m8: u16) -> Tile {
        Tile {
            height: 1,
            kind: TileKind::Rail,
            mapt: 0x10,
            m5: 0x01,
            m1: 0,
            m6: 0,
            m8,
            m3: 0,
            m2: 0,
            m2_hi: 0,
            m7: 0,
            m3hi: 0,
        }
    }

    #[test]
    fn m8_roundtrip_rail_types() {
        let t = set_rail_type_on_tile(rail_tile(0), RailType::Electric);
        assert_eq!(rail_type_from_tile(t), RailType::Electric);
        let t2 = set_rail_type_on_tile(t, RailType::Rail);
        assert_eq!(rail_type_from_tile(t2), RailType::Rail);
    }

    #[test]
    fn electric_engine_needs_electric_track() {
        let asia = engine_by_id(ENGINE_TRAIN_ASIASTAR).unwrap();
        assert!(!engine_compatible_with_rail(asia, RailType::Rail));
        assert!(engine_compatible_with_rail(asia, RailType::Electric));
        let kirby = engine_by_id(ENGINE_TRAIN_KIRBY).unwrap();
        assert!(engine_compatible_with_rail(kirby, RailType::Rail));
        assert!(engine_compatible_with_rail(kirby, RailType::Electric));
    }
}
