//! Paletas `PalSpriteID` de los árboles toyland.
//!
//! Las filas toyland de `tree_land.h` comparten el sprite base y cambian su
//! apariencia mediante `PALETTE_TO_*`. Como el atlas distribuido es RGBA, la
//! paleta debe hornearse en una copia de la textura antes de que Bevy la use.

use std::collections::{BTreeSet, HashMap};
use std::path::Path;

use bevy::prelude::*;
use image::RgbaImage;

use super::company_palette::{CompanyColour, recolor_rgba8, rgba_to_bevy_image, tiles_assets_dir};
use super::{
    TILE_ATLAS_NAMES, TILE_ATLAS_RECTS, TREE_LAYOUT_PALETTE, TREE_LAYOUT_SPRITE, TREE_SPRITE_COUNT,
};

const SPR_TREES_BASE: u32 = 1576;

/// Copias RGBA de los árboles que declaran `PALETTE_TO_*` en el layout vanilla.
#[derive(Resource, Clone, Default)]
pub(crate) struct TreePaletteSprites {
    sprites: HashMap<(u32, u32), Handle<Image>>,
}

impl TreePaletteSprites {
    pub(crate) fn build_all(&mut self, images: &mut Assets<Image>) {
        self.build_from_tiles_dir(images, &tiles_assets_dir());
    }

    fn build_from_tiles_dir(&mut self, images: &mut Assets<Image>, tiles: &Path) {
        self.sprites.clear();
        let mut pages = HashMap::new();
        for (sprite_id, palette) in tree_paletted_sprite_pairs() {
            let Some(mut img) = load_tree_rgba(sprite_id, tiles, &mut pages) else {
                continue;
            };
            recolor_rgba8(img.as_mut(), CompanyColour::from_u8((palette - 775) as u8));
            self.sprites
                .insert((sprite_id, palette), images.add(rgba_to_bevy_image(img)));
        }
    }

    #[must_use]
    pub(crate) fn handle(&self, sprite_id: u32, palette: u32) -> Option<&Handle<Image>> {
        self.sprites.get(&(sprite_id, palette))
    }

    #[must_use]
    #[cfg(test)]
    pub(crate) fn covers_all_generated_pairs(&self) -> bool {
        tree_paletted_sprite_pairs()
            .into_iter()
            .all(|pair| self.sprites.contains_key(&pair))
    }
}

/// Todas las combinaciones `(sprite, paleta)` que puede emitir el layout
/// vanilla. Cada entrada se expande a las siete etapas de crecimiento porque
/// `DrawTile_Trees` suma la etapa al sprite base seleccionado por la tabla.
#[must_use]
pub(crate) fn tree_paletted_sprite_pairs() -> BTreeSet<(u32, u32)> {
    let mut pairs = BTreeSet::new();
    for (sprite_row, palette_row) in TREE_LAYOUT_SPRITE.iter().zip(TREE_LAYOUT_PALETTE) {
        for (&sprite, palette) in sprite_row.iter().zip(palette_row) {
            if palette == 0 {
                continue;
            }
            for stage in 0..=6u32 {
                pairs.insert((
                    SPR_TREES_BASE + u32::from(sprite) + stage,
                    u32::from(palette),
                ));
            }
        }
    }
    pairs
}

fn tree_sprite_asset_filename(sprite_id: u32) -> Option<String> {
    let relative = sprite_id.checked_sub(SPR_TREES_BASE)?;
    (relative < TREE_SPRITE_COUNT as u32).then(|| format!("tree_{relative:02}.png"))
}

fn load_tree_rgba(
    sprite_id: u32,
    tiles: &Path,
    pages: &mut HashMap<u16, Option<RgbaImage>>,
) -> Option<RgbaImage> {
    let name = tree_sprite_asset_filename(sprite_id)?;
    if let Ok(img) = image::open(tiles.join(&name)) {
        return Some(img.into_rgba8());
    }
    let entry = TILE_ATLAS_NAMES
        .binary_search_by(|(n, _)| (*n).cmp(name.as_str()))
        .ok()?;
    let &(page, x, y, width, height) = TILE_ATLAS_RECTS.get(TILE_ATLAS_NAMES[entry].1 as usize)?;
    let atlas_dir = tiles.parent()?.join("atlas");
    let img = pages
        .entry(page)
        .or_insert_with(|| {
            image::open(atlas_dir.join(format!("tiles_atlas_{page}.png")))
                .ok()
                .map(image::DynamicImage::into_rgba8)
        })
        .as_ref()?;
    let (x, y, width, height) = (
        u32::from(x),
        u32::from(y),
        u32::from(width),
        u32::from(height),
    );
    if x + width > img.width() || y + height > img.height() {
        return None;
    }
    Some(image::imageops::crop_imm(img, x, y, width, height).to_image())
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn toyland_layout_expands_every_palette_to_all_growth_stages() {
        let pairs = tree_paletted_sprite_pairs();
        assert!(!pairs.is_empty());
        assert!(pairs.contains(&(1947, 779))); // `0x79b`, PALETTE_TO_RED.
        assert!(pairs.contains(&(1953, 790))); // `0x7a1`, one of its stages.
        assert!(pairs.contains(&(2003, 784))); // `0x7d3`, PALETTE_TO_CREAM.
        for stage in 0..=6u32 {
            assert!(pairs.contains(&(1947 + stage, 779)));
        }
    }

    #[test]
    fn all_tree_palettes_load_from_the_distributed_atlas() {
        let dir = tempfile::tempdir().expect("tempdir");
        let atlas_dir = dir.path().join("atlas");
        std::fs::create_dir(&atlas_dir).expect("mkdir");
        for page in 0..super::super::TILE_ATLAS_PAGE_COUNT {
            let name = format!("tiles_atlas_{page}.png");
            std::fs::copy(
                tiles_assets_dir()
                    .parent()
                    .expect("tiles parent")
                    .join("atlas")
                    .join(&name),
                atlas_dir.join(name),
            )
            .expect("distributed atlas");
        }
        let tiles = dir.path().join("tiles");
        let mut images = Assets::<Image>::default();
        let mut palettes = TreePaletteSprites::default();
        palettes.build_from_tiles_dir(&mut images, &tiles);
        assert!(palettes.covers_all_generated_pairs());

        let mut pages = HashMap::new();
        for (sprite_id, palette) in tree_paletted_sprite_pairs() {
            let mut expected = load_tree_rgba(sprite_id, &tiles, &mut pages).expect("atlas crop");
            recolor_rgba8(
                expected.as_mut(),
                CompanyColour::from_u8((palette - 775) as u8),
            );
            let actual = images
                .get(palettes.handle(sprite_id, palette).expect("palette handle"))
                .expect("image");
            assert_eq!(actual.data.as_deref(), Some(expected.as_raw().as_slice()));
            assert_eq!(actual.width(), expected.width());
            assert_eq!(actual.height(), expected.height());
        }
    }
}
