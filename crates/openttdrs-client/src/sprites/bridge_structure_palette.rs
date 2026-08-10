//! Remapeo de color de estructuras de puente (`PALETTE_TO_STRUCT_*` en OpenTTD).
//!
//! Varios tipos comparten los mismos sprite IDs; el tono (marrón, amarillo, rojo…)
//! se aplica en runtime con las tablas recolor 795–801 del baseset.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::OnceLock;

use bevy::prelude::*;
use openttdrs_core::{BridgePiece, BridgeType};

use super::bridge_sprites_generated::{BridgeDeckSpriteIds, bridge_deck_sprite_ids};
use super::company_palette::{rgba_to_bevy_image, tiles_assets_dir};

#[path = "bridge_structure_palette_data_generated.rs"]
mod generated;

use generated::{STRUCT_REMAP_BROWN, STRUCT_REMAP_CONCRETE, STRUCT_REMAP_RED, STRUCT_REMAP_YELLOW};

/// Paleta de estructura aplicable a sprites de puente compartidos.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum BridgeStructurePalette {
    #[default]
    None = 0,
    Brown = 1,
    Red = 2,
    Concrete = 3,
    Yellow = 4,
}

impl BridgeStructurePalette {
    #[must_use]
    pub const fn needs_recolor(self) -> bool {
        !matches!(self, Self::None)
    }

    /// ID de paleta lógico que expone `DrawTile_TunnelBridge` en la traza de
    /// OpenTTD. El cliente aplica el mismo recolor sobre una textura RGBA,
    /// pero conservar este valor permite detectar si un puente parece tener
    /// rail normal sólo porque se perdió su paleta estructural.
    #[must_use]
    pub const fn openttd_palette_id(self) -> u32 {
        match self {
            Self::None => 0,
            Self::Brown => 796,    // PALETTE_TO_STRUCT_BROWN
            Self::Red => 798,      // PALETTE_TO_STRUCT_RED
            Self::Concrete => 800, // PALETTE_TO_STRUCT_CONCRETE
            Self::Yellow => 801,   // PALETTE_TO_STRUCT_YELLOW
        }
    }
}

/// Paleta de estructura según tipo de puente (`_orig_bridge` + `DrawBridgeMiddle`).
#[must_use]
pub const fn bridge_structure_palette(bt: BridgeType) -> BridgeStructurePalette {
    match bt {
        BridgeType::SuspensionConcrete | BridgeType::TubularSilicon => {
            BridgeStructurePalette::Concrete
        }
        BridgeType::SuspensionSteelYellow | BridgeType::TubularYellow => {
            BridgeStructurePalette::Yellow
        }
        BridgeType::CantileverBrown => BridgeStructurePalette::Brown,
        BridgeType::CantileverRed => BridgeStructurePalette::Red,
        _ => BridgeStructurePalette::None,
    }
}

/// Paleta de una pieza concreta de puente, tal como la tabla vanilla de
/// OpenTTD la entrega a `DrawTile_TunnelBridge`.
///
/// Las cabezas genéricas `2437..=2444` ya contienen la vía rail/electric
/// normal en el propio PNG y sus cuatro entradas usan `PAL_NONE`. Aplicarles
/// el recolor de la estructura convertía una vía normal en la apariencia del
/// puente (roja/amarilla/concreta) y hacía difícil distinguir su conexión con
/// mono/maglev. Las cabezas de carretera, mono y maglev usan bloques distintos
/// y sí reciben la paleta estructural.
#[must_use]
pub const fn bridge_structure_palette_for_sprite(
    bt: BridgeType,
    sprite_id: u32,
) -> BridgeStructurePalette {
    if sprite_id >= 2437 && sprite_id <= 2444 {
        BridgeStructurePalette::None
    } else {
        bridge_structure_palette(bt)
    }
}

fn pairs_for(palette: BridgeStructurePalette) -> &'static [([u8; 3], [u8; 3])] {
    match palette {
        BridgeStructurePalette::None => &[],
        BridgeStructurePalette::Brown => &STRUCT_REMAP_BROWN,
        BridgeStructurePalette::Red => &STRUCT_REMAP_RED,
        BridgeStructurePalette::Concrete => &STRUCT_REMAP_CONCRETE,
        BridgeStructurePalette::Yellow => &STRUCT_REMAP_YELLOW,
    }
}

#[must_use]
pub fn build_structure_remap_table(palette: BridgeStructurePalette) -> HashMap<[u8; 3], [u8; 3]> {
    pairs_for(palette).iter().copied().collect()
}

fn remap_table_cached(palette: BridgeStructurePalette) -> &'static HashMap<[u8; 3], [u8; 3]> {
    static TABLES: [OnceLock<HashMap<[u8; 3], [u8; 3]>>; 5] = [const { OnceLock::new() }; 5];
    TABLES[palette as usize].get_or_init(|| build_structure_remap_table(palette))
}

/// Recolorea un buffer RGBA8 in-place con la paleta de estructura indicada.
pub fn recolor_bridge_rgba8(buf: &mut [u8], palette: BridgeStructurePalette) {
    if !palette.needs_recolor() {
        return;
    }
    let table = remap_table_cached(palette);
    for px in buf.chunks_exact_mut(4) {
        if px[3] == 0 {
            continue;
        }
        let key = [px[0], px[1], px[2]];
        if let Some(&rgb) = table.get(&key) {
            px[0] = rgb[0];
            px[1] = rgb[1];
            px[2] = rgb[2];
        }
    }
}

/// Sprites de puente recoloreados (PNG sueltos, fuera del atlas).
#[derive(Resource, Clone, Default)]
pub struct BridgePaletteSprites {
    sprites: HashMap<(u32, BridgeStructurePalette), Handle<Image>>,
}

impl BridgePaletteSprites {
    pub fn build_all(&mut self, images: &mut Assets<Image>) {
        self.sprites.clear();
        let tiles = tiles_assets_dir();
        for sid in bridge_sprite_ids_for_structure_recolor() {
            for palette in [
                BridgeStructurePalette::Brown,
                BridgeStructurePalette::Red,
                BridgeStructurePalette::Concrete,
                BridgeStructurePalette::Yellow,
            ] {
                let key = (sid, palette);
                if self.sprites.contains_key(&key) {
                    continue;
                }
                let path = tiles.join(BridgeDeckSpriteIds::atlas_name(sid));
                if let Some(handle) = load_recolored_bridge_png(&path, palette, images) {
                    self.sprites.insert(key, handle);
                }
            }
        }
    }

    #[must_use]
    pub fn handle(
        &self,
        sprite_id: u32,
        palette: BridgeStructurePalette,
    ) -> Option<&Handle<Image>> {
        if !palette.needs_recolor() {
            return None;
        }
        self.sprites.get(&(sprite_id, palette))
    }
}

fn load_recolored_bridge_png(
    path: &Path,
    palette: BridgeStructurePalette,
    images: &mut Assets<Image>,
) -> Option<Handle<Image>> {
    let mut img = image::open(path).ok()?.into_rgba8();
    recolor_bridge_rgba8(img.as_mut(), palette);
    Some(images.add(rgba_to_bevy_image(img)))
}

/// IDs de sprite usados por tipos con recolor de estructura.
#[must_use]
pub fn bridge_sprite_ids_for_structure_recolor() -> Vec<u32> {
    let mut ids = HashSet::new();
    for bt in 0..13u8 {
        let Some(bridge_type) = BridgeType::from_u8(bt) else {
            continue;
        };
        for piece in [
            BridgePiece::North,
            BridgePiece::South,
            BridgePiece::InnerNorth,
            BridgePiece::InnerSouth,
            BridgePiece::MiddleOdd,
            BridgePiece::MiddleEven,
        ] {
            let deck = bridge_deck_sprite_ids(bridge_type, piece);
            for sid in deck
                .rear_rail
                .iter()
                .chain(deck.rear_road.iter())
                .chain(deck.front.iter())
                .chain(deck.pillar.iter())
                .copied()
                .filter(|id| {
                    *id != 0
                        && bridge_structure_palette_for_sprite(bridge_type, *id).needs_recolor()
                })
            {
                ids.insert(sid);
            }
        }
    }
    ids.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn yellow_palette_maps_brown_structure_to_gold() {
        let table = build_structure_remap_table(BridgeStructurePalette::Yellow);
        assert_eq!(table.get(&[64, 20, 8]), Some(&[32, 4, 0]));
        assert_eq!(table.get(&[196, 128, 108]), Some(&[252, 212, 0]));
    }

    #[test]
    fn suspension_steel_yellow_uses_yellow_palette() {
        assert_eq!(
            bridge_structure_palette(BridgeType::SuspensionSteelYellow),
            BridgeStructurePalette::Yellow
        );
    }

    #[test]
    fn recolor_changes_brown_pixel() {
        let mut px = [64u8, 20, 8, 255];
        recolor_bridge_rgba8(&mut px, BridgeStructurePalette::Yellow);
        assert_eq!(&px[..3], &[32, 4, 0]);
    }

    #[test]
    fn trace_palette_ids_match_openttd_constants() {
        assert_eq!(BridgeStructurePalette::None.openttd_palette_id(), 0);
        assert_eq!(BridgeStructurePalette::Concrete.openttd_palette_id(), 800);
        assert_eq!(BridgeStructurePalette::Yellow.openttd_palette_id(), 801);
    }

    #[test]
    fn generic_rail_heads_keep_their_own_palette() {
        assert_eq!(
            bridge_structure_palette_for_sprite(BridgeType::CantileverRed, 2440),
            BridgeStructurePalette::None
        );
        assert_eq!(
            bridge_structure_palette_for_sprite(BridgeType::TubularYellow, 2437),
            BridgeStructurePalette::None
        );
        assert_eq!(
            bridge_structure_palette_for_sprite(BridgeType::TubularYellow, 4367),
            BridgeStructurePalette::Yellow
        );
    }
}
