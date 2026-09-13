//! Conversión `DecodedSprite` → `Image` con política RGBA explícita.

use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use openttdrs_core::{
    DecodedSprite, bake_sprite_company_mask, bake_sprite_company_palette, bake_sprite_crash,
    bake_sprite_two_company_palette, bake_sprite_two_company_palette_with_map,
};

use crate::sprites::CompanyColour;
use crate::sprites::bridge_structure_palette::{BridgeStructurePalette, recolor_structure_rgba8};

const TILE_LAYOUT_PALETTE_MODIFIERS: u8 =
    openttdrs_core::newgrf_sprites::TILE_LAYOUT_SPRITE_MODIFIER_TRANSPARENT
        | openttdrs_core::newgrf_sprites::TILE_LAYOUT_SPRITE_MODIFIER_RECOLOUR;
const PALETTE_TO_TRANSPARENT: u16 = 802;

/// Política de bake/recolor al subir un sprite NewGRF a textura.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DecodedSpriteImagePolicy {
    /// RGBA cruda del decode (road / shore / catenary).
    Raw,
    /// Máscara de compañía sin recolor post-bake (vehículos / buy window).
    Masked { colour: CompanyColour },
    /// Máscara opcional con el color de compañía (industria / estación).
    MaskedAndRecolored { colour: Option<CompanyColour> },
    /// PaletteID de compañía explícita (`775..=790`) escrita por Action2.
    CompanyPalette { colour: CompanyColour },
    /// Paleta de dos colores de compañía (`SPR_2CCMAP_BASE + offset`).
    TwoCompany {
        primary: CompanyColour,
        secondary: CompanyColour,
    },
    /// Remapeo gris oscuro de un vehículo en estado de choque (`804`).
    Crash,
    /// Máscara que oscurece el destino para `PALETTE_TO_TRANSPARENT` (`802`).
    Transparent,
    /// Paleta `PALETTE_TO_STRUCT_*` aplicada sobre un sprite compartido.
    Structure { palette: BridgeStructurePalette },
}

pub(crate) fn decoded_sprite_image(
    sprite: &DecodedSprite,
    policy: DecodedSpriteImagePolicy,
) -> Image {
    decoded_sprite_image_with_twocc_map(sprite, policy, None)
}

/// Selecciona la paleta por defecto de `DrawCommonTileSeq` sólo cuando el
/// wire activa `transparent` o `recolour`. `opaque` controla la visibilidad y
/// no debe convertir por sí solo una textura en una rampa de compañía. También
/// conserva una paleta directa que el core no puede hornear, como
/// `PALETTE_TO_STRUCT_*`.
///
/// `direct_palette=0` conserva la ruta histórica para layouts sin paleta
/// explícita.
pub(crate) fn decoded_tile_layout_image_with_palette(
    sprite: &DecodedSprite,
    sprite_modifiers: u8,
    direct_palette: u16,
    default_policy: DecodedSpriteImagePolicy,
) -> Image {
    decoded_tile_layout_image_with_palette_and_twocc_map(
        sprite,
        sprite_modifiers,
        direct_palette,
        default_policy,
        None,
    )
}

/// Variante de [`decoded_tile_layout_image_with_palette`] que conserva una
/// paleta directa y un mapa Action5 `2CC`.
pub(crate) fn decoded_tile_layout_image_with_palette_and_twocc_map(
    sprite: &DecodedSprite,
    sprite_modifiers: u8,
    direct_palette: u16,
    default_policy: DecodedSpriteImagePolicy,
    twocc_map: Option<&DecodedSprite>,
) -> Image {
    let policy = if direct_palette == PALETTE_TO_TRANSPARENT
        && sprite_modifiers
            & openttdrs_core::newgrf_sprites::TILE_LAYOUT_SPRITE_MODIFIER_TRANSPARENT
            != 0
    {
        DecodedSpriteImagePolicy::Transparent
    } else if let Some(palette) =
        BridgeStructurePalette::from_openttd_palette_id(u32::from(direct_palette))
    {
        DecodedSpriteImagePolicy::Structure { palette }
    } else if sprite_modifiers & TILE_LAYOUT_PALETTE_MODIFIERS != 0 {
        default_policy
    } else {
        DecodedSpriteImagePolicy::Raw
    };
    decoded_sprite_image_with_twocc_map(sprite, policy, twocc_map)
}

/// Convierte el modo nativo `Transparent` en una textura que Bevy puede
/// componer sobre el framebuffer actual.
///
/// OpenTTD no pinta el RGB del sprite en este modo: para cada píxel con alpha
/// oscurece el destino a `3/4` (o en proporción a un alpha parcial). Una
/// textura negra con alpha equivalente conserva esa semántica sin inventar un
/// color de origen. El valor máximo `64/255` es la representación de 1/4 en
/// el canal alpha de ocho bits.
fn destination_transparent_rgba8(sprite: &DecodedSprite) -> Vec<u8> {
    let mut rgba = sprite.rgba.clone();
    let (pixels, _) = rgba.as_chunks_mut::<4>();
    for pixel in pixels {
        let source_alpha = u16::from(pixel[3]);
        pixel[0] = 0;
        pixel[1] = 0;
        pixel[2] = 0;
        pixel[3] = ((source_alpha * 64 + 127) / 255) as u8;
    }
    rgba
}

/// Color de una entrada `TileLayout` después de aplicar la preferencia de
/// transparencia de su categoría.
///
/// `SPRITE_MODIFIER_OPAQUE` no cambia la paleta ni los píxeles: sólo impide
/// que `IsTransparencySet`/`IsInvisibilitySet` afecte a esa entrada. En Bevy
/// la preferencia se representa en el alpha del `Sprite`, por eso la entrada
/// opaca recupera alpha 1 sin perder un RGB que el caller ya haya elegido.
pub(crate) fn tile_layout_sprite_color(color: Color, sprite_modifiers: u8) -> Color {
    if sprite_modifiers & openttdrs_core::newgrf_sprites::TILE_LAYOUT_SPRITE_MODIFIER_OPAQUE == 0 {
        return color;
    }
    let rgba = color.to_srgba();
    Color::srgba(rgba.red, rgba.green, rgba.blue, 1.0)
}

/// Ajusta el color de una entrada `TileLayout` teniendo en cuenta una paleta
/// que ya representa transparencia de destino.
///
/// La textura de `PALETTE_TO_TRANSPARENT` ya contiene la cobertura de la
/// máscara negra; aplicar además el alpha de la categoría volvería a
/// oscurecerla por segunda vez, algo que no hace el blitter nativo.
pub(crate) fn tile_layout_sprite_color_with_palette(
    color: Color,
    sprite_modifiers: u8,
    direct_palette: u16,
) -> Color {
    if direct_palette == PALETTE_TO_TRANSPARENT {
        let rgba = color.to_srgba();
        return Color::srgba(rgba.red, rgba.green, rgba.blue, 1.0);
    }
    tile_layout_sprite_color(color, sprite_modifiers)
}

/// Decide si `IsInvisibilitySet` suprime una entrada de la secuencia.
///
/// El filtro se aplica al stream `BUILD`, no al ground que cada feature dibuja
/// por separado antes de `DrawCommonTileSeq`. Un parent suprimido debe además
/// limpiar el parent activo en el caller para que sus children no reaparezcan.
#[must_use]
pub(crate) const fn tile_layout_entry_is_hidden(
    sprite_modifiers: u8,
    category_hidden: bool,
) -> bool {
    category_hidden
        && sprite_modifiers & openttdrs_core::newgrf_sprites::TILE_LAYOUT_SPRITE_MODIFIER_OPAQUE
            == 0
}

pub(crate) fn decoded_sprite_image_with_twocc_map(
    sprite: &DecodedSprite,
    policy: DecodedSpriteImagePolicy,
    twocc_map: Option<&DecodedSprite>,
) -> Image {
    let rgba = match policy {
        DecodedSpriteImagePolicy::Raw => sprite.rgba.clone(),
        DecodedSpriteImagePolicy::Masked { colour } => {
            if sprite.mask.is_empty() {
                sprite.rgba.clone()
            } else {
                bake_sprite_company_mask(sprite, colour.as_u8())
            }
        }
        DecodedSpriteImagePolicy::MaskedAndRecolored { colour } => {
            if sprite.mask.is_empty() {
                sprite.rgba.clone()
            } else {
                let c = colour.map(CompanyColour::as_u8).unwrap_or(0);
                bake_sprite_company_mask(sprite, c)
            }
        }
        DecodedSpriteImagePolicy::CompanyPalette { colour } => {
            bake_sprite_company_palette(sprite, colour.as_u8())
        }
        DecodedSpriteImagePolicy::TwoCompany { primary, secondary } => {
            if let Some(map) = twocc_map {
                bake_sprite_two_company_palette_with_map(
                    sprite,
                    primary.as_u8(),
                    secondary.as_u8(),
                    Some(map),
                )
            } else {
                bake_sprite_two_company_palette(sprite, primary.as_u8(), secondary.as_u8())
            }
        }
        DecodedSpriteImagePolicy::Crash => bake_sprite_crash(sprite),
        DecodedSpriteImagePolicy::Transparent => destination_transparent_rgba8(sprite),
        DecodedSpriteImagePolicy::Structure { palette } => {
            let mut rgba = sprite.rgba.clone();
            recolor_structure_rgba8(&mut rgba, palette);
            rgba
        }
    };
    Image::new(
        Extent3d {
            width: u32::from(sprite.width),
            height: u32::from(sprite.height),
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        rgba,
        TextureFormat::Rgba8UnormSrgb,
        default(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sprite_with_rgba(rgba: Vec<u8>) -> DecodedSprite {
        DecodedSprite {
            width: 1,
            height: 1,
            x_offs: 0,
            y_offs: 0,
            rgba,
            mask: Vec::new(),
        }
    }

    #[test]
    fn raw_policy_keeps_rgba_bytes() {
        let sprite = sprite_with_rgba(vec![10, 20, 30, 255]);
        let img = decoded_sprite_image(&sprite, DecodedSpriteImagePolicy::Raw);
        assert_eq!(img.data.as_deref(), Some(&[10, 20, 30, 255][..]));
    }

    #[test]
    fn masked_and_recolored_none_skips_recolor_on_raw() {
        let sprite = sprite_with_rgba(vec![1, 2, 3, 255]);
        let img = decoded_sprite_image(
            &sprite,
            DecodedSpriteImagePolicy::MaskedAndRecolored { colour: None },
        );
        assert_eq!(img.data.as_deref(), Some(&[1, 2, 3, 255][..]));
    }

    #[test]
    fn masked_and_recolored_without_mask_does_not_guess_company_colour_from_rgb() {
        // Dark blue is a meaningful ordinary RGB value in 32bpp sprites.
        // Sólo la máscara NewGRF autoriza su recolor.
        let sprite = sprite_with_rgba(vec![8, 24, 88, 255]);
        let img = decoded_sprite_image(
            &sprite,
            DecodedSpriteImagePolicy::MaskedAndRecolored {
                colour: Some(CompanyColour::Green),
            },
        );
        assert_eq!(img.data.as_deref(), Some(&[8, 24, 88, 255][..]));
    }

    #[test]
    fn explicit_company_palette_recolours_palette_only_sprite() {
        let sprite = sprite_with_rgba(vec![8, 24, 88, 255]); // author ramp, shade 0
        let img = decoded_sprite_image(
            &sprite,
            DecodedSpriteImagePolicy::CompanyPalette {
                colour: CompanyColour::Green,
            },
        );
        assert_ne!(img.data.as_deref(), Some(&[8, 24, 88, 255][..]));
        assert_eq!(img.data.as_deref().map(|rgba| rgba[3]), Some(255));
    }

    #[test]
    fn tile_layout_applies_direct_structure_palette_after_decode() {
        let sprite = sprite_with_rgba(vec![64, 20, 8, 255]);
        let img =
            decoded_tile_layout_image_with_palette(&sprite, 0, 801, DecodedSpriteImagePolicy::Raw);
        assert_eq!(img.data.as_deref(), Some(&[96, 44, 4, 255][..]));
    }

    #[test]
    fn tile_layout_uses_default_palette_only_for_palette_modifiers() {
        let sprite = sprite_with_rgba(vec![8, 24, 88, 255]);
        let policy = DecodedSpriteImagePolicy::CompanyPalette {
            colour: CompanyColour::Green,
        };
        let raw = decoded_tile_layout_image_with_palette(&sprite, 0, 0, policy);
        assert_eq!(raw.data.as_deref(), Some(&[8, 24, 88, 255][..]));

        let opaque = decoded_tile_layout_image_with_palette(
            &sprite,
            openttdrs_core::newgrf_sprites::TILE_LAYOUT_SPRITE_MODIFIER_OPAQUE,
            0,
            policy,
        );
        assert_eq!(opaque.data.as_deref(), Some(&[8, 24, 88, 255][..]));

        let recoloured = decoded_tile_layout_image_with_palette(
            &sprite,
            openttdrs_core::newgrf_sprites::TILE_LAYOUT_SPRITE_MODIFIER_RECOLOUR,
            0,
            policy,
        );
        assert_ne!(recoloured.data.as_deref(), Some(&[8, 24, 88, 255][..]));
    }

    #[test]
    fn tile_layout_transparent_palette_builds_destination_mask() {
        let sprite = DecodedSprite {
            width: 2,
            height: 1,
            x_offs: 0,
            y_offs: 0,
            rgba: vec![40, 80, 120, 255, 200, 160, 80, 128],
            mask: Vec::new(),
        };
        let img = decoded_tile_layout_image_with_palette(
            &sprite,
            openttdrs_core::newgrf_sprites::TILE_LAYOUT_SPRITE_MODIFIER_TRANSPARENT,
            PALETTE_TO_TRANSPARENT,
            DecodedSpriteImagePolicy::Raw,
        );
        assert_eq!(img.data.as_deref(), Some(&[0, 0, 0, 64, 0, 0, 0, 32][..]));
    }

    #[test]
    fn tile_layout_transparent_palette_does_not_double_category_alpha() {
        let category_tint = Color::srgba(1.0, 1.0, 1.0, 0.45);
        let color = tile_layout_sprite_color_with_palette(category_tint, 0, PALETTE_TO_TRANSPARENT);
        let rgba = color.to_srgba();
        assert!((rgba.red - 1.0).abs() < f32::EPSILON);
        assert!((rgba.green - 1.0).abs() < f32::EPSILON);
        assert!((rgba.blue - 1.0).abs() < f32::EPSILON);
        assert_eq!(rgba.alpha, 1.0);
    }

    #[test]
    fn tile_layout_opaque_modifier_bypasses_category_alpha() {
        let transparent = Color::srgba(0.2, 0.3, 0.4, 0.45);
        let regular = tile_layout_sprite_color(transparent, 0);
        let opaque = tile_layout_sprite_color(
            transparent,
            openttdrs_core::newgrf_sprites::TILE_LAYOUT_SPRITE_MODIFIER_OPAQUE,
        );

        assert_eq!(regular, transparent);
        let rgba = opaque.to_srgba();
        assert_eq!(
            (rgba.red, rgba.green, rgba.blue, rgba.alpha),
            (0.2, 0.3, 0.4, 1.0)
        );
    }

    #[test]
    fn tile_layout_hidden_entry_respects_opaque_modifier() {
        assert!(tile_layout_entry_is_hidden(0, true));
        assert!(!tile_layout_entry_is_hidden(
            openttdrs_core::newgrf_sprites::TILE_LAYOUT_SPRITE_MODIFIER_OPAQUE,
            true,
        ));
        assert!(!tile_layout_entry_is_hidden(0, false));
    }
}
