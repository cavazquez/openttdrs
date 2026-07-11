//! Tipos de carretera / tranvía (`RoadType` en `OpenTTD`).
//!
//! En teselas `MP_ROAD`:
//! - tipo de **carretera** en `Tile.m8` bits 0–5 (`GetRoadTypeRoad`);
//! - tipo de **tranvía** en `Tile.m8` bits 6–11 (`GetRoadTypeTram`);
//! - trazado de carretera en `m5` bits 0–3;
//! - trazado de tranvía en `m3` bits 0–3.
//!
//! Catálogo filtrable (UI-6f): vanilla hoy; entradas `NewGRF` cuando exista Action0–14.

use serde::{Deserialize, Serialize};

use crate::map::Tile;

/// Clase carretera vs tranvía (`RoadTramType` / `RTT_ROAD` / `RTT_TRAM` en `OpenTTD`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum RoadTramType {
    Road = 0,
    Tram = 1,
}

impl RoadTramType {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Road => "Carretera",
            Self::Tram => "Tranvía",
        }
    }
}

/// Identificador de tipo de carretera/tranvía (valores alineados con `OpenTTD` vanilla).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[repr(u8)]
pub enum RoadType {
    #[default]
    Road = 0,
    /// Tranvía vanilla (`ROADTYPE_TRAM`).
    Tram = 1,
}

impl RoadType {
    #[must_use]
    pub const fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::Tram,
            _ => Self::Road,
        }
    }

    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Road => "Carretera",
            Self::Tram => "Tranvía",
        }
    }

    #[must_use]
    pub const fn short_label(self) -> &'static str {
        match self {
            Self::Road => "Norm",
            Self::Tram => "Tram",
        }
    }

    /// Clase a la que pertenece este tipo vanilla.
    #[must_use]
    pub const fn road_tram_type(self) -> RoadTramType {
        match self {
            Self::Road => RoadTramType::Road,
            Self::Tram => RoadTramType::Tram,
        }
    }

    /// Tipo por defecto de una clase (`GetDefaultRoadType`).
    #[must_use]
    pub const fn default_for_class(class: RoadTramType) -> Self {
        match class {
            RoadTramType::Road => Self::Road,
            RoadTramType::Tram => Self::Tram,
        }
    }
}

/// Metadatos de un tipo disponible para construir (catálogo UI / futuro `NewGRF`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RoadTypeDef {
    pub id: RoadType,
    pub class: RoadTramType,
    pub label: &'static str,
    pub short_label: &'static str,
    /// Año de introducción (filtro de disponibilidad por calendario).
    pub intro_year: u16,
    /// `true` si proviene de `NewGRF` (hoy siempre `false` en el catálogo vanilla).
    pub from_newgrf: bool,
}

const VANILLA_ROAD_TYPES: &[RoadTypeDef] = &[
    RoadTypeDef {
        id: RoadType::Road,
        class: RoadTramType::Road,
        label: "Carretera normal",
        short_label: "Norm",
        intro_year: 0,
        from_newgrf: false,
    },
    RoadTypeDef {
        id: RoadType::Tram,
        class: RoadTramType::Tram,
        label: "Tranvía eléctrico",
        short_label: "Tram",
        intro_year: 0,
        from_newgrf: false,
    },
];

/// Catálogo completo (vanilla; `NewGRF` se añadirá al runtime Action0–14).
#[must_use]
pub fn all_road_type_defs() -> &'static [RoadTypeDef] {
    VANILLA_ROAD_TYPES
}

/// Definición de un `RoadType` concreto, si está en el catálogo.
#[must_use]
pub fn road_type_def(id: RoadType) -> Option<&'static RoadTypeDef> {
    VANILLA_ROAD_TYPES.iter().find(|d| d.id == id)
}

/// Lista filtrable por clase y texto (`GetRoadTypeDropDownList` + filtro de toolbar).
///
/// El filtro es case-insensitive sobre `label` / `short_label`. Tipos `NewGRF`
/// aparecerán aquí cuando el runtime los registre.
#[must_use]
pub fn list_road_types(
    class: RoadTramType,
    filter: &str,
    calendar_year: u32,
) -> Vec<&'static RoadTypeDef> {
    let needle = filter.trim().to_ascii_lowercase();
    VANILLA_ROAD_TYPES
        .iter()
        .filter(|d| d.class == class)
        .filter(|d| calendar_year >= u32::from(d.intro_year))
        .filter(|d| {
            if needle.is_empty() {
                return true;
            }
            d.label.to_ascii_lowercase().contains(&needle)
                || d.short_label.to_ascii_lowercase().contains(&needle)
        })
        .collect()
}

/// Máscara de bits 0–5 de `m8` (tipo de carretera).
const ROADTYPE_ROAD_M8_MASK: u16 = 0x003F;
/// Máscara de bits 6–11 de `m8` (tipo de tranvía).
const ROADTYPE_TRAM_M8_MASK: u16 = 0x0FC0;

/// Bits de trazado de tranvía (`m3` nibble bajo).
#[must_use]
pub const fn tram_track_bits(tile: &Tile) -> u8 {
    tile.m3 & 0x0F
}

/// Tipo de carretera en `m8` bits 0–5.
#[must_use]
pub fn road_type_from_tile(tile: &Tile) -> RoadType {
    RoadType::from_u8(u8::try_from(tile.m8 & ROADTYPE_ROAD_M8_MASK).unwrap_or(0))
}

/// Tipo de tranvía en `m8` bits 6–11; `None` si no hay.
#[must_use]
pub fn tram_road_type_from_tile(tile: &Tile) -> Option<RoadType> {
    let t = ((tile.m8 >> 6) & 0x3F) as u8;
    if t == 0 || t == 0x3F {
        None
    } else {
        Some(RoadType::from_u8(t))
    }
}

/// ¿La tesela tiene overlay de tranvía?
#[must_use]
pub fn tile_has_tram_track(tile: &Tile) -> bool {
    tram_track_bits(tile) != 0 || tram_road_type_from_tile(tile).is_some()
}

/// Escribe el tipo de carretera en `m8` bits 0–5; conserva el resto.
#[must_use]
pub fn set_road_type_on_tile(mut tile: Tile, road: RoadType) -> Tile {
    tile.m8 = (tile.m8 & !ROADTYPE_ROAD_M8_MASK) | u16::from(road.as_u8());
    tile
}

/// Escribe el tipo de tranvía en `m8` bits 6–11; conserva el resto.
#[must_use]
pub fn set_tram_road_type_on_tile(mut tile: Tile, tram: Option<RoadType>) -> Tile {
    let clear = tile.m8 & !ROADTYPE_TRAM_M8_MASK;
    tile.m8 = match tram {
        Some(rt) => clear | (u16::from(rt.as_u8()) << 6),
        None => clear,
    };
    tile
}

/// Escribe bits de trazado de tranvía en `m3` (nibble bajo); conserva owner nibble.
#[must_use]
pub fn set_tram_track_bits_on_tile(mut tile: Tile, bits: u8) -> Tile {
    tile.m3 = (tile.m3 & 0xF0) | (bits & 0x0F);
    tile
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::GameState;
    use crate::map::{TileCoord, TileKind};

    #[test]
    fn set_tram_type_roundtrip() {
        let mut state = GameState::new(4, 4);
        let c = TileCoord::new(1, 1);
        let mut tile = state.map.get(c).unwrap();
        tile.kind = TileKind::Road;
        tile = set_tram_track_bits_on_tile(tile, 0x05);
        tile = set_tram_road_type_on_tile(tile, Some(RoadType::Tram));
        state.map.set_tile(c, tile).unwrap();
        let t = state.map.get(c).unwrap();
        assert_eq!(tram_track_bits(&t), 0x05);
        assert_eq!(tram_road_type_from_tile(&t), Some(RoadType::Tram));
    }

    #[test]
    fn set_road_type_preserves_tram_bits() {
        let state = GameState::new(2, 2);
        let c = TileCoord::new(0, 0);
        let mut tile = state.map.get(c).unwrap();
        tile.kind = TileKind::Road;
        tile = set_tram_road_type_on_tile(tile, Some(RoadType::Tram));
        tile = set_road_type_on_tile(tile, RoadType::Road);
        assert_eq!(road_type_from_tile(&tile), RoadType::Road);
        assert_eq!(tram_road_type_from_tile(&tile), Some(RoadType::Tram));
    }

    #[test]
    fn list_road_types_filters_by_class_and_text() {
        let roads = list_road_types(RoadTramType::Road, "", 1950);
        assert_eq!(roads.len(), 1);
        assert_eq!(roads[0].id, RoadType::Road);

        let trams = list_road_types(RoadTramType::Tram, "tram", 1950);
        assert_eq!(trams.len(), 1);
        assert_eq!(trams[0].id, RoadType::Tram);

        assert!(list_road_types(RoadTramType::Road, "zzz", 1950).is_empty());
        assert!(list_road_types(RoadTramType::Tram, "carretera", 1950).is_empty());
    }
}
