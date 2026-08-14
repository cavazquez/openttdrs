//! Paletas de las capas de casas vanilla (`_town_draw_tile_data`).
//!
//! Las entradas de `town_land.h` contienen un sprite y una `PaletteID` por
//! capa. El atlas es RGBA, por lo que ya no retiene el índice DOS necesario
//! para aplicar el recolor durante el draw. Esta caché prepara sólo las
//! combinaciones realmente presentes en las 1760 entradas vanilla y conserva
//! el sprite lógico/paleta en la traza de paridad.

use std::collections::{BTreeSet, HashMap};

use bevy::prelude::*;

use super::bridge_structure_palette::{BridgeStructurePalette, recolor_structure_rgba8};
use super::company_palette::{CompanyColour, recolor_rgba8, rgba_to_bevy_image, tiles_assets_dir};
use super::{HOUSE_DRAW_DATA, house_sprite_asset_filename};

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
        self.sprites.clear();
        for (sprite_id, palette) in house_paletted_sprite_pairs() {
            // No ocultamos una paleta nueva/no implementada tras el PNG de
            // base: al no insertarla el spawner la marcará como fallback en
            // `world-draw` y la regresión de tabla dará el motivo exacto.
            if !supports_house_palette(palette) {
                continue;
            }
            if let Some(handle) = load_recolored_house_png(sprite_id, palette, images) {
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
) -> Option<Handle<Image>> {
    let mut img = image::open(tiles_assets_dir().join(house_sprite_asset_filename(sprite_id)))
        .ok()?
        .into_rgba8();

    if let Some(structure) = BridgeStructurePalette::from_openttd_palette_id(palette) {
        recolor_structure_rgba8(img.as_mut(), structure);
    } else if (775..=790).contains(&palette) {
        recolor_rgba8(img.as_mut(), CompanyColour::from_u8((palette - 775) as u8));
    } else {
        return None;
    }

    Some(images.add(rgba_to_bevy_image(img)))
}

#[cfg(test)]
mod tests {
    use super::*;

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
