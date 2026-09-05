//! Paletas de las capas de casas vanilla (`_town_draw_tile_data`).
//!
//! Las entradas de `town_land.h` contienen un sprite y una `PaletteID` por
//! capa. El atlas es RGBA, por lo que ya no retiene el índice DOS necesario
//! para aplicar el recolor durante el draw. Esta caché prepara sólo las
//! combinaciones realmente presentes en las 1760 entradas vanilla y conserva
//! el sprite lógico/paleta en la traza de paridad.

use std::collections::{BTreeSet, HashMap};
use std::path::Path;

use bevy::prelude::*;

use super::bridge_structure_palette::{BridgeStructurePalette, recolor_structure_rgba8};
use super::company_palette::{CompanyColour, recolor_rgba8, rgba_to_bevy_image, tiles_assets_dir};
use super::{HOUSE_DRAW_DATA, TILE_ATLAS_NAMES, TILE_ATLAS_RECTS, house_sprite_asset_filename};

/// Copias RGBA de sprites de casa con una `PaletteID` vanilla aplicada.
#[derive(Resource, Clone, Default)]
pub(crate) struct HousePaletteSprites {
    sprites: HashMap<(u32, u32), Handle<Image>>,
}

impl HousePaletteSprites {
    /// Construye las combinaciones `(sprite, paleta)` presentes en la tabla
    /// vanilla. No toca las entradas `PAL_NONE`, que continúan usando el atlas
    /// compartido y por tanto conservan el batching habitual.
    pub(crate) fn build_all(&mut self, images: &mut Assets<Image>) {
        self.build_from_tiles_dir(images, &tiles_assets_dir());
    }

    fn build_from_tiles_dir(&mut self, images: &mut Assets<Image>, tiles: &Path) {
        self.sprites.clear();
        let mut pages = HashMap::new();
        for (sprite_id, palette) in house_paletted_sprite_pairs() {
            // No ocultamos una paleta nueva/no implementada tras el PNG de
            // base: al no insertarla el spawner la marcará como fallback en
            // `world-draw` y la regresión de tabla dará el motivo exacto.
            if !supports_house_palette(palette) {
                continue;
            }
            if let Some(handle) =
                load_recolored_house_png(sprite_id, palette, images, tiles, &mut pages)
            {
                self.sprites.insert((sprite_id, palette), handle);
            }
        }
    }

    #[must_use]
    pub(crate) fn handle(&self, sprite_id: u32, palette: u32) -> Option<&Handle<Image>> {
        self.sprites.get(&(sprite_id, palette))
    }

    /// Permite que los tests de integración fallen si algún asset activo (8 o
    /// 32 bpp) no pudo producir su copia recoloreada.
    #[must_use]
    #[cfg(test)]
    pub(crate) fn covers_all_generated_pairs(&self) -> bool {
        house_paletted_sprite_pairs()
            .into_iter()
            .all(|pair| self.sprites.contains_key(&pair))
    }
}

/// Todas las combinaciones no nulas que OpenTTD declara para las casas
/// vanilla. Es una función y no una constante para mantener la gran tabla
/// generada compacta y poder validarla directamente en tests.
#[must_use]
pub(crate) fn house_paletted_sprite_pairs() -> BTreeSet<(u32, u32)> {
    let mut pairs = BTreeSet::new();
    for spec in &HOUSE_DRAW_DATA {
        for (sprite_id, palette) in [(spec.s1, spec.s1_palette), (spec.s2, spec.s2_palette)] {
            if sprite_id != 0 && palette != 0 {
                pairs.insert((sprite_id, palette));
            }
        }
    }
    pairs
}

/// `PaletteID` que el renderer puede aplicar a las capas de casa vanilla.
#[must_use]
pub(crate) const fn supports_house_palette(palette: u32) -> bool {
    (palette >= 775 && palette <= 790)
        || BridgeStructurePalette::from_openttd_palette_id(palette).is_some()
}

fn load_recolored_house_png(
    sprite_id: u32,
    palette: u32,
    images: &mut Assets<Image>,
    tiles: &Path,
    pages: &mut HashMap<u16, Option<image::RgbaImage>>,
) -> Option<Handle<Image>> {
    let mut img = load_house_rgba(sprite_id, tiles, pages)?;

    if let Some(structure) = BridgeStructurePalette::from_openttd_palette_id(palette) {
        recolor_structure_rgba8(img.as_mut(), structure);
    } else if (775..=790).contains(&palette) {
        recolor_rgba8(img.as_mut(), CompanyColour::from_u8((palette - 775) as u8));
    } else {
        return None;
    }

    Some(images.add(rgba_to_bevy_image(img)))
}

/// Los PNGs sueltos son opcionales; un checkout limpio distribuye el atlas.
/// Conserva el override 8/32 bpp y decodifica cada página una sola vez por build.
fn load_house_rgba(
    sprite_id: u32,
    tiles: &Path,
    pages: &mut HashMap<u16, Option<image::RgbaImage>>,
) -> Option<image::RgbaImage> {
    let name = house_sprite_asset_filename(sprite_id);
    if let Ok(img) = image::open(tiles.join(&name)) {
        return Some(img.into_rgba8());
    }
    let entry = TILE_ATLAS_NAMES
        .binary_search_by(|(n, _)| n.cmp(&name.as_str()))
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
    fn all_house_palettes_load_from_distributed_atlas_without_loose_pngs() {
        let dir = tempfile::tempdir().expect("tempdir");
        let atlas_dir = dir.path().join("atlas");
        std::fs::create_dir(&atlas_dir).expect("mkdir");
        for page in 0..super::super::TILE_ATLAS_PAGE_COUNT {
            let name = format!("tiles_atlas_{page}.png");
            std::fs::copy(
                tiles_assets_dir().join("../atlas").join(&name),
                atlas_dir.join(name),
            )
            .expect("distributed atlas");
        }
        let tiles = dir.path().join("tiles");
        assert!(!tiles.exists());
        let mut images = Assets::<Image>::default();
        let mut palettes = HousePaletteSprites::default();
        palettes.build_from_tiles_dir(&mut images, &tiles);
        assert!(palettes.covers_all_generated_pairs());

        // Verifica píxeles del recorte y su paleta, no sólo handles presentes.
        let mut pages = HashMap::new();
        for (sprite_id, palette) in house_paletted_sprite_pairs() {
            let mut expected = load_house_rgba(sprite_id, &tiles, &mut pages).expect("atlas crop");
            if let Some(structure) = BridgeStructurePalette::from_openttd_palette_id(palette) {
                recolor_structure_rgba8(expected.as_mut(), structure);
            } else {
                recolor_rgba8(
                    expected.as_mut(),
                    CompanyColour::from_u8((palette - 775) as u8),
                );
            }
            let actual = images
                .get(palettes.handle(sprite_id, palette).expect("palette handle"))
                .expect("image");
            assert_eq!(actual.data.as_deref(), Some(expected.as_raw().as_slice()));
            assert_eq!(actual.width(), expected.width());
            assert_eq!(actual.height(), expected.height());
        }
    }

    #[test]
    fn loose_house_png_overrides_atlas_and_missing_assets_stay_missing() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut pages = HashMap::new();
        assert!(load_house_rgba(1421, dir.path(), &mut pages).is_none());
        let expected = image::RgbaImage::from_pixel(2, 3, image::Rgba([13, 24, 35, 46]));
        expected
            .save(dir.path().join(house_sprite_asset_filename(1421)))
            .expect("png");
        assert_eq!(
            load_house_rgba(1421, dir.path(), &mut pages),
            Some(expected)
        );
    }

    #[test]
    fn atlas_crop_preserves_bounds_pixels_and_rejects_truncated_page() {
        let dir = tempfile::tempdir().expect("tempdir");
        let atlas_dir = dir.path().join("atlas");
        std::fs::create_dir(&atlas_dir).expect("mkdir");
        let name = house_sprite_asset_filename(1421);
        let entry = TILE_ATLAS_NAMES
            .iter()
            .find(|(n, _)| *n == name)
            .expect("entry");
        let (page, x, y, width, height) = TILE_ATLAS_RECTS[entry.1 as usize];
        let (x, y, width, height) = (
            u32::from(x),
            u32::from(y),
            u32::from(width),
            u32::from(height),
        );
        let path = atlas_dir.join(format!("tiles_atlas_{page}.png"));
        let mut atlas = image::RgbaImage::new(x + width, y + height);
        let first = image::Rgba([12, 23, 34, 45]);
        let last = image::Rgba([56, 67, 78, 89]);
        atlas.put_pixel(x, y, first);
        atlas.put_pixel(x + width - 1, y + height - 1, last);
        atlas.save(&path).expect("atlas");
        let tiles = dir.path().join("tiles");
        let actual = load_house_rgba(1421, &tiles, &mut HashMap::new()).expect("crop");
        assert_eq!(actual.dimensions(), (width, height));
        assert_eq!(*actual.get_pixel(0, 0), first);
        assert_eq!(*actual.get_pixel(width - 1, height - 1), last);
        image::RgbaImage::new(1, 1)
            .save(&path)
            .expect("truncated atlas");
        assert!(load_house_rgba(1421, &tiles, &mut HashMap::new()).is_none());
    }

    #[test]
    fn generated_house_pairs_cover_structure_church_and_company_palettes() {
        let pairs = house_paletted_sprite_pairs();
        assert!(pairs.contains(&(1421, 797)), "estructura blanca");
        assert!(pairs.contains(&(1434, 1438)), "iglesia roja");
        assert!(pairs.contains(&(1470, 779)), "casa color compañía rojo");
    }

    #[test]
    fn every_palette_used_by_vanilla_house_data_is_supported() {
        for (_, palette) in house_paletted_sprite_pairs() {
            assert!(
                supports_house_palette(palette),
                "PaletteID {palette} de town_land.h no está implementada"
            );
        }
    }

    #[test]
    fn palette_support_keeps_company_and_church_ranges_distinct() {
        assert!(supports_house_palette(775));
        assert!(supports_house_palette(790));
        assert!(supports_house_palette(795));
        assert!(supports_house_palette(1438));
        assert!(supports_house_palette(1439));
        assert!(!supports_house_palette(0));
        assert!(!supports_house_palette(804));
    }
}
