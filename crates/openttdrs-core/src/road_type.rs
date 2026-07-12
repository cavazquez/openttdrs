//! Tipos de carretera / tranvía (`RoadType` en `OpenTTD`).
//!
//! En teselas `MP_ROAD`:
//! - tipo de **carretera** en `Tile.m8` bits 0–5 (`GetRoadTypeRoad`);
//! - tipo de **tranvía** en `Tile.m8` bits 6–11 (`GetRoadTypeTram`);
//! - trazado de carretera en `m5` bits 0–3;
//! - trazado de tranvía en `m3` bits 0–3.
//!
//! Catálogo: vanilla (0/1) + tipos `NewGRF` Action0 feature 0x12 (IDs ≥2).

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

/// Identificador de tipo (0 = Road vanilla, 1 = Tram vanilla, ≥2 = `NewGRF`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub struct RoadType(pub u8);

impl RoadType {
    pub const ROAD: Self = Self(0);
    pub const TRAM: Self = Self(1);

    /// Compatibilidad con código que usaba el enum.
    #[allow(non_upper_case_globals)]
    pub const Road: Self = Self::ROAD;
    /// Compatibilidad con código que usaba el enum.
    #[allow(non_upper_case_globals)]
    pub const Tram: Self = Self::TRAM;

    #[must_use]
    pub const fn from_u8(v: u8) -> Self {
        Self(v & 0x3F)
    }

    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self.0
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self.0 {
            0 => "Carretera",
            1 => "Tranvía",
            _ => "NewGRF",
        }
    }

    #[must_use]
    pub const fn short_label(self) -> &'static str {
        match self.0 {
            0 => "Norm",
            1 => "Tram",
            _ => "NGRF",
        }
    }

    /// Clase vanilla por ID; tipos `NewGRF` deben consultarse en el catálogo.
    #[must_use]
    pub const fn road_tram_type(self) -> RoadTramType {
        match self.0 {
            1 => RoadTramType::Tram,
            _ => RoadTramType::Road,
        }
    }

    #[must_use]
    pub const fn default_for_class(class: RoadTramType) -> Self {
        match class {
            RoadTramType::Road => Self::ROAD,
            RoadTramType::Tram => Self::TRAM,
        }
    }

    #[must_use]
    pub const fn is_vanilla(self) -> bool {
        self.0 <= 1
    }
}

/// Metadatos de un tipo disponible para construir.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoadTypeDef {
    pub id: RoadType,
    pub class: RoadTramType,
    pub label: String,
    pub short_label: String,
    pub intro_year: u16,
    pub from_newgrf: bool,
    /// Preview Action1/3 (primera vista); no se serializa en saves.
    #[serde(default, skip)]
    pub newgrf_preview: Option<crate::newgrf_sprites::DecodedSprite>,
    /// Vistas Action1/3 para in-world (índice = `road_flat_sprite_index` en plano).
    #[serde(default, skip)]
    pub newgrf_views: Vec<crate::newgrf_sprites::DecodedSprite>,
}

impl RoadTypeDef {
    /// Preview `NewGRF` si el tipo trae sprite Action1/3.
    #[must_use]
    pub fn newgrf_preview_sprite(&self) -> Option<&crate::newgrf_sprites::DecodedSprite> {
        self.newgrf_preview
            .as_ref()
            .or_else(|| self.newgrf_views.first())
    }

    /// Vista in-world (`idx` módulo longitud; plano usa `road_flat_sprite_index`).
    #[must_use]
    pub fn newgrf_view(&self, idx: usize) -> Option<&crate::newgrf_sprites::DecodedSprite> {
        if self.newgrf_views.is_empty() {
            return self.newgrf_preview.as_ref();
        }
        self.newgrf_views.get(idx % self.newgrf_views.len())
    }
}

/// Catálogo vanilla (Road + Tram).
#[must_use]
pub fn vanilla_road_type_catalog() -> Vec<RoadTypeDef> {
    vec![
        RoadTypeDef {
            id: RoadType::ROAD,
            class: RoadTramType::Road,
            label: "Carretera normal".into(),
            short_label: "Norm".into(),
            intro_year: 0,
            from_newgrf: false,
            newgrf_preview: None,
            newgrf_views: Vec::new(),
        },
        RoadTypeDef {
            id: RoadType::TRAM,
            class: RoadTramType::Tram,
            label: "Tranvía eléctrico".into(),
            short_label: "Tram".into(),
            intro_year: 0,
            from_newgrf: false,
            newgrf_preview: None,
            newgrf_views: Vec::new(),
        },
    ]
}

/// Catálogo vanilla estático (tests / fallback sin `GameState`).
#[must_use]
pub fn all_road_type_defs() -> Vec<RoadTypeDef> {
    vanilla_road_type_catalog()
}

/// Definición de un `RoadType` en el catálogo dado.
#[must_use]
pub fn road_type_def(catalog: &[RoadTypeDef], id: RoadType) -> Option<&RoadTypeDef> {
    catalog.iter().find(|d| d.id == id)
}

/// Lista filtrable por clase y texto.
#[must_use]
pub fn list_road_types<'a>(
    catalog: &'a [RoadTypeDef],
    class: RoadTramType,
    filter: &str,
    calendar_year: u32,
) -> Vec<&'a RoadTypeDef> {
    let needle = filter.trim().to_ascii_lowercase();
    catalog
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

/// Siguiente ID libre ≥2 (máx. 63 por bits `m8`).
#[must_use]
pub fn next_free_road_type_id(catalog: &[RoadTypeDef]) -> Option<RoadType> {
    for id in 2u8..=63 {
        let rt = RoadType::from_u8(id);
        if !catalog.iter().any(|d| d.id == rt) {
            return Some(rt);
        }
    }
    None
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
        let cat = vanilla_road_type_catalog();
        let roads = list_road_types(&cat, RoadTramType::Road, "", 1950);
        assert_eq!(roads.len(), 1);
        assert_eq!(roads[0].id, RoadType::Road);

        let trams = list_road_types(&cat, RoadTramType::Tram, "tram", 1950);
        assert_eq!(trams.len(), 1);
        assert_eq!(trams[0].id, RoadType::Tram);

        assert!(list_road_types(&cat, RoadTramType::Road, "zzz", 1950).is_empty());
        assert!(list_road_types(&cat, RoadTramType::Tram, "carretera", 1950).is_empty());
    }
}
