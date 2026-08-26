//! Conversión `DecodedSprite` → `Image` con política RGBA explícita.

use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use openttdrs_core::{DecodedSprite, bake_sprite_company_mask, bake_sprite_company_palette};

use crate::sprites::CompanyColour;

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
}

pub(crate) fn decoded_sprite_image(
    sprite: &DecodedSprite,
    policy: DecodedSpriteImagePolicy,
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
}
