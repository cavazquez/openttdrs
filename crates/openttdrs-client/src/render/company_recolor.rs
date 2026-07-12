//! Helpers de render para sprites recoloreados por compañía.

pub(crate) use crate::sprites::CompanyColoredSprites;

use bevy::prelude::*;

use crate::render::AtlasSprite;
use crate::sprites::{CompanyColour, tile_filename};

/// Usa textura recoloreada de la paleta activa o del owner; si no, el atlas.
#[must_use]
pub fn sprite_from_atlas_or_company_colour(
    company: Option<&CompanyColoredSprites>,
    colour: Option<CompanyColour>,
    atlas: &AtlasSprite,
    asset_path: &str,
    tint: Color,
) -> Sprite {
    if let Some(c) = company {
        let handle = match colour {
            Some(col) => c.tile_handle_path_for_colour(col, asset_path),
            None => c.tile_handle_path(asset_path),
        };
        if let Some(handle) = handle {
            return Sprite {
                image: handle.clone(),
                color: tint,
                ..default()
            };
        }
    }
    atlas.sprite_colored(tint)
}

/// Como [`sprite_from_atlas_or_company_colour`] con tinte blanco.
#[must_use]
pub fn sprite_from_atlas_or_company_white_colour(
    company: Option<&CompanyColoredSprites>,
    colour: Option<CompanyColour>,
    atlas: &AtlasSprite,
    asset_path: &str,
) -> Sprite {
    sprite_from_atlas_or_company_colour(company, colour, atlas, asset_path, Color::WHITE)
}

/// Atlas o PNG recoloreado con la paleta `random_colour` de la industria (P4).
#[must_use]
pub fn sprite_from_atlas_or_industry_palette(
    company: &mut CompanyColoredSprites,
    images: &mut Assets<Image>,
    atlas: &AtlasSprite,
    sprite_id: u32,
    industry_colour: crate::sprites::CompanyColour,
) -> Sprite {
    if let Some(handle) = company.industry_sprite_handle(sprite_id, industry_colour, images) {
        return Sprite {
            image: handle,
            color: Color::WHITE,
            ..default()
        };
    }
    atlas.sprite_colored(Color::WHITE)
}

/// Sprite desde caché de compañía o carga directa del asset server (previews).
#[must_use]
pub fn sprite_from_company_or_asset(
    company: Option<&CompanyColoredSprites>,
    asset_server: &AssetServer,
    asset_path: &str,
    tint: Color,
) -> Sprite {
    let filename = tile_filename(asset_path);
    if let Some(c) = company
        && let Some(handle) = c.tile_handle(filename)
    {
        return Sprite {
            image: handle.clone(),
            color: tint,
            ..default()
        };
    }
    Sprite {
        image: asset_server.load(asset_path.to_string()),
        color: tint,
        ..default()
    }
}
