//! Remapeo de color de estructuras vanilla (`PALETTE_TO_STRUCT_*` en OpenTTD).
//!
//! Varios tipos comparten los mismos sprite IDs; el tono (marrón, amarillo, rojo…)
//! se aplica en runtime con las tablas recolor 795–801 del baseset. Las
//! iglesias de ciudad comparten el mismo mecanismo con las tablas 1438/1439.

use std::collections::{BTreeSet, HashMap};
use std::path::Path;
use std::sync::OnceLock;

use bevy::prelude::*;
use openttdrs_core::{BridgePiece, BridgeType, RailType};

use super::bridge_sprites_generated::{
    BridgeDeckSpriteIds, bridge_deck_sprite_ids, bridge_ramp_sprite_id,
};
use super::company_palette::{rgba_to_bevy_image, tiles_assets_dir};

#[path = "bridge_structure_palette_data_generated.rs"]
mod generated;

use generated::{
    STRUCT_REMAP_BLUE, STRUCT_REMAP_BROWN, STRUCT_REMAP_CHURCH_CREAM, STRUCT_REMAP_CHURCH_RED,
    STRUCT_REMAP_CONCRETE, STRUCT_REMAP_GREEN, STRUCT_REMAP_RED, STRUCT_REMAP_WHITE,
    STRUCT_REMAP_YELLOW,
};

/// Paleta de estructura aplicable a sprites vanilla compartidos.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum BridgeStructurePalette {
    #[default]
    None = 0,
    Blue = 1,
    Brown = 2,
    White = 3,
    Red = 4,
    Green = 5,
    Concrete = 6,
    Yellow = 7,
    ChurchRed = 8,
    ChurchCream = 9,
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
            Self::Blue => 795,     // PALETTE_TO_STRUCT_BLUE
            Self::Brown => 796,    // PALETTE_TO_STRUCT_BROWN
            Self::White => 797,    // PALETTE_TO_STRUCT_WHITE
            Self::Red => 798,      // PALETTE_TO_STRUCT_RED
            Self::Green => 799,    // PALETTE_TO_STRUCT_GREEN
            Self::Concrete => 800, // PALETTE_TO_STRUCT_CONCRETE
            Self::Yellow => 801,   // PALETTE_TO_STRUCT_YELLOW
            Self::ChurchRed => 1438,
            Self::ChurchCream => 1439,
        }
    }

    /// Traduce las `PaletteID` de estructura que aparecen directamente en
    /// `town_land.h`. El cero (`PAL_NONE`) se mantiene fuera de este tipo para
    /// que quien llama no pueda confundir una paleta ausente con una textura
    /// recoloreada.
    #[must_use]
    pub const fn from_openttd_palette_id(id: u32) -> Option<Self> {
        match id {
            795 => Some(Self::Blue),
            796 => Some(Self::Brown),
            797 => Some(Self::White),
            798 => Some(Self::Red),
            799 => Some(Self::Green),
            800 => Some(Self::Concrete),
            801 => Some(Self::Yellow),
            1438 => Some(Self::ChurchRed),
            1439 => Some(Self::ChurchCream),
            _ => None,
        }
    }
}

/// Paleta de estructura según tipo de puente (`_orig_bridge` + `DrawBridgeMiddle`).
#[must_use]
pub const fn bridge_structure_palette(bt: BridgeType) -> BridgeStructurePalette {
    match bt {
        // `_bridge_sprite_table_concrete_*` usa PALETTE_TO_STRUCT_RED: el
        // nombre del puente no coincide con el nombre de la rampa de paleta.
        BridgeType::Concrete | BridgeType::CantileverRed => BridgeStructurePalette::Red,
        BridgeType::SuspensionConcrete | BridgeType::TubularSilicon => {
            BridgeStructurePalette::Concrete
        }
        BridgeType::SuspensionSteelYellow | BridgeType::TubularYellow => {
            BridgeStructurePalette::Yellow
        }
        BridgeType::CantileverBrown => BridgeStructurePalette::Brown,
        _ => BridgeStructurePalette::None,
    }
}

/// Las cabezas genéricas comparten sprites entre varios puentes. Sólo las
/// variantes de carretera, monorriel y maglev admiten recolor: las ocho de
/// rail/electric (`2437..=2444`) llevan su apariencia propia en el PNG.
///
/// `bridge_land.h` da a las cabezas del arco de viga (`GirderSteel`) la
/// paleta blanca, pero mantiene sus vanos en `PAL_NONE`; por eso no puede
/// resolverse únicamente a partir del tipo de puente.
const fn is_generic_recolourable_bridge_head(sprite_id: u32) -> bool {
    matches!(sprite_id, 2445..=2452 | 4326..=4333 | 4366..=4373)
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
    if matches!(bt, BridgeType::GirderSteel) {
        return if is_generic_recolourable_bridge_head(sprite_id) {
            BridgeStructurePalette::White
        } else {
            BridgeStructurePalette::None
        };
    }
    if (sprite_id >= 2437 && sprite_id <= 2444)
        // El puente de concreto remapea tablero y frente, pero sus pilares
        // `SPR_BTCON_{X,Y}_PILLAR` son PAL_NONE en `bridge_land.h`.
        || (matches!(bt, BridgeType::Concrete) && (sprite_id == 2505 || sprite_id == 2506))
    {
        BridgeStructurePalette::None
    } else {
        bridge_structure_palette(bt)
    }
}

fn pairs_for(palette: BridgeStructurePalette) -> &'static [([u8; 3], [u8; 3])] {
    match palette {
        BridgeStructurePalette::None => &[],
        BridgeStructurePalette::Blue => &STRUCT_REMAP_BLUE,
        BridgeStructurePalette::Brown => &STRUCT_REMAP_BROWN,
        BridgeStructurePalette::White => &STRUCT_REMAP_WHITE,
        BridgeStructurePalette::Red => &STRUCT_REMAP_RED,
        BridgeStructurePalette::Green => &STRUCT_REMAP_GREEN,
        BridgeStructurePalette::Concrete => &STRUCT_REMAP_CONCRETE,
        BridgeStructurePalette::Yellow => &STRUCT_REMAP_YELLOW,
        BridgeStructurePalette::ChurchRed => &STRUCT_REMAP_CHURCH_RED,
        BridgeStructurePalette::ChurchCream => &STRUCT_REMAP_CHURCH_CREAM,
    }
}

#[must_use]
pub fn build_structure_remap_table(palette: BridgeStructurePalette) -> HashMap<[u8; 3], [u8; 3]> {
    pairs_for(palette).iter().copied().collect()
}

fn remap_table_cached(palette: BridgeStructurePalette) -> &'static HashMap<[u8; 3], [u8; 3]> {
    static TABLES: [OnceLock<HashMap<[u8; 3], [u8; 3]>>; 10] = [const { OnceLock::new() }; 10];
    TABLES[palette as usize].get_or_init(|| build_structure_remap_table(palette))
}

/// Recolorea un buffer RGBA8 in-place con la paleta de estructura indicada.
///
/// Los tiles se extraen con la misma paleta DOS que el blitter. La tabla
/// generada conserva el índice NFO correcto; en especial no consume la Action0
/// de la pseudo-sprite como si fuera un color de estructura.
pub fn recolor_structure_rgba8(buf: &mut [u8], palette: BridgeStructurePalette) {
    if !palette.needs_recolor() {
        return;
    }
    let table = remap_table_cached(palette);
    let (pixels, _) = buf.as_chunks_mut::<4>();
    for px in pixels {
        if px[3] == 0 {
            continue;
        }
        let key = [px[0], px[1], px[2]];
        if let Some(&rgb) = table.get(&key) {
            if rgb == [0, 0, 0] {
                px.copy_from_slice(&[0, 0, 0, 0]);
            } else {
                px[0] = rgb[0];
                px[1] = rgb[1];
                px[2] = rgb[2];
            }
        }
    }
}

/// Alias de compatibilidad para los llamadores de puentes.
pub fn recolor_bridge_rgba8(buf: &mut [u8], palette: BridgeStructurePalette) {
    recolor_structure_rgba8(buf, palette);
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
                BridgeStructurePalette::White,
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
    let mut ids = BTreeSet::new();
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
                .chain(deck.rear_mono.iter())
                .chain(deck.rear_maglev.iter())
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
        // Las rampas de carretera, monorriel y maglev también llevan la
        // paleta estructural. Antes sólo se cacheaban los vanos: al llegar a
        // una rampa esos medios caían al PNG sin recolor y parecían vía rail
        // normal o una desconexión visual.
        for (rail, rail_type) in [
            (false, RailType::Rail),
            (true, RailType::Rail),
            (true, RailType::Monorail),
            (true, RailType::Maglev),
        ] {
            for tileh in [0, 1] {
                for dir in 0..4 {
                    let sid = bridge_ramp_sprite_id(bridge_type, rail, rail_type, tileh, dir);
                    if bridge_structure_palette_for_sprite(bridge_type, sid).needs_recolor() {
                        ids.insert(sid);
                    }
                }
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
        // Índice DOS 71 → 68 en `PALETTE_TO_STRUCT_YELLOW`. Es importante
        // que Action0 no desplace este índice a la entrada anterior.
        assert_eq!(table.get(&[64, 20, 8]), Some(&[96, 44, 4]));
        assert_eq!(table.get(&[196, 128, 108]), Some(&[252, 248, 128]));
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
        assert_eq!(&px[..3], &[96, 44, 4]);
    }

    #[test]
    fn red_palette_matches_the_openttd_dos_indices() {
        let table = build_structure_remap_table(BridgeStructurePalette::Red);
        // `ogfx1_base.nfo` pseudo-sprite 798: índices 71, 72 y 76.
        assert_eq!(table.get(&[64, 20, 8]), Some(&[60, 0, 0]));
        assert_eq!(table.get(&[84, 28, 16]), Some(&[92, 0, 0]));
        assert_eq!(table.get(&[168, 92, 76]), Some(&[212, 52, 52]));
    }

    #[test]
    fn transparent_pixels_stay_transparent_when_recolored() {
        // El índice 0 ya fue convertido a alpha cero al extraer OpenGFX. El
        // remapeo de estructura no debe volverlo opaco.
        let mut px = [0u8, 0, 0, 0];
        recolor_bridge_rgba8(&mut px, BridgeStructurePalette::Red);
        assert_eq!(px, [0, 0, 0, 0]);
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
        assert_eq!(
            bridge_structure_palette_for_sprite(BridgeType::GirderSteel, 2437),
            BridgeStructurePalette::None
        );
        assert_eq!(
            bridge_structure_palette_for_sprite(BridgeType::GirderSteel, 2445),
            BridgeStructurePalette::White
        );
        assert_eq!(
            bridge_structure_palette_for_sprite(BridgeType::GirderSteel, 4326),
            BridgeStructurePalette::White
        );
    }

    #[test]
    fn structure_palette_selection_matches_bridge_land_tables() {
        // `bridge_land.h`: el concreto base es STRUCT_RED, aunque el puente
        // de suspensión concreta use STRUCT_CONCRETE.
        assert_eq!(
            bridge_structure_palette_for_sprite(BridgeType::Concrete, 2493),
            BridgeStructurePalette::Red
        );
        assert_eq!(
            bridge_structure_palette_for_sprite(BridgeType::Concrete, 2497),
            BridgeStructurePalette::Red
        );
        assert_eq!(
            bridge_structure_palette_for_sprite(BridgeType::Concrete, 2505),
            BridgeStructurePalette::None
        );
        assert_eq!(
            bridge_structure_palette_for_sprite(BridgeType::SuspensionConcrete, 2481),
            BridgeStructurePalette::Concrete
        );
        assert_eq!(
            bridge_structure_palette_for_sprite(BridgeType::CantileverRed, 2523),
            BridgeStructurePalette::Red
        );
    }

    #[test]
    fn recolor_inventory_covers_typed_bridge_decks_and_ramps() {
        let ids = bridge_sprite_ids_for_structure_recolor();
        // C++ generic bridge heads: road, monorail and maglev have palette;
        // rail/electric does not. Cubrimos las cuatro direcciones de cada uno.
        for sid in [2445, 2452, 4326, 4333, 4366, 4373] {
            assert!(ids.contains(&sid), "falta rampa recoloreada {sid}");
        }
        for sid in [4347, 4350, 4387, 4390] {
            assert!(ids.contains(&sid), "falta tablero tipado {sid}");
        }
        for sid in [2437, 2444, 2505, 2506] {
            assert!(
                !ids.contains(&sid),
                "{sid} es PAL_NONE en bridge_land.h y no debe recolorearse"
            );
        }
    }
}
