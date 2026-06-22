//! Helpers de render para sprites recoloreados por compañía.

pub(crate) use crate::sprites::CompanyColoredSprites;

use bevy::prelude::*;

use crate::render::AtlasSprite;
use crate::sprites::tile_filename;

/// Usa textura recoloreada si existe; si no, el sprite del atlas.
#[must_use]
pub fn sprite_from_atlas_or_company(
    company: Option<&CompanyColoredSprites>,
    atlas: &AtlasSprite,
    asset_path: &str,
    tint: Color,
) -> Sprite {
    if let Some(c) = company
        && let Some(handle) = c.tile_handle_path(asset_path)
    {
        return Sprite {
            image: handle.clone(),
            color: tint,
            ..default()
        };
    }
    atlas.sprite_colored(tint)
}

/// Como [`sprite_from_atlas_or_company`] con tinte blanco.
#[must_use]
pub fn sprite_from_atlas_or_company_white(
    company: Option<&CompanyColoredSprites>,
    atlas: &AtlasSprite,
    asset_path: &str,
) -> Sprite {
    sprite_from_atlas_or_company(company, atlas, asset_path, Color::WHITE)
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
