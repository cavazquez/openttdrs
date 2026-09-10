//! Helpers compartidos para caches NewGRF in-world (#135).

mod fingerprint;
mod image_factory;

use openttdrs_core::map::SPR_FLAT_WATER_TILE;
use openttdrs_core::newgrf_sprites::{ResolvedTileLayout, ResolvedTileLayoutSprite};

use crate::render::{AtlasSprite, WorldAssets};

pub(crate) use fingerprint::runtime_fingerprint;
pub(crate) use image_factory::{
    DecodedSpriteImagePolicy, decoded_sprite_image, decoded_sprite_image_with_twocc_map,
};

/// Baseset sprites that are safe to use as a `TileLayout` ground without
/// guessing a palette, an animation, or NFO geometry. They all share the
/// flat 64×31 tile geometry and `xrel=-31, yrel=0`.
const DIRECT_FLAT_GROUND_SPRITES: [u16; 4] = [3924, 3981, 4000, SPR_FLAT_WATER_TILE as u16];

/// Base-sprite data that a TileLayout renderer needs in addition to the atlas
/// rect. `AtlasSprite` deliberately has no NFO offset, so keeping this small
/// whitelist prevents a direct reference from silently using a wrong anchor.
#[derive(Clone)]
pub(crate) struct DirectTileLayoutGround {
    pub(crate) atlas: AtlasSprite,
    pub(crate) width: f32,
    pub(crate) height: f32,
    pub(crate) x_offs: f32,
    pub(crate) y_offs: f32,
}

/// Whether every TileLayout entry can be emitted by the current compact
/// renderer. Action1 entries are fully decoded; a direct base reference is
/// accepted only for one of the audited flat ground sprites, never for BUILD
/// parents or children.
#[must_use]
pub(crate) fn tile_layout_is_renderable(layout: &ResolvedTileLayout) -> bool {
    if !layout.complete
        || layout
            .sequence
            .iter()
            .any(|entry| entry.action1_sprite().is_none())
    {
        return false;
    }
    match layout.ground.as_ref() {
        None => true,
        Some(ground) => {
            ground.action1_sprite().is_some()
                || ground
                    .base_sprite_id()
                    .is_some_and(|id| DIRECT_FLAT_GROUND_SPRITES.contains(&id))
        }
    }
}

/// Resolves the texture and audited NFO geometry of a supported direct base
/// ground. Action1 sprites stay in the per-feature NewGRF cache instead.
#[must_use]
pub(crate) fn direct_tile_layout_ground(
    ground: &ResolvedTileLayoutSprite,
    assets: &WorldAssets,
) -> Option<DirectTileLayoutGround> {
    if ground.action1_sprite().is_some() {
        return None;
    }
    let atlas = match ground.base_sprite_id()? {
        3924 => assets.industries.get(&3924)?.clone(), // SPR_FLAT_BARE_LAND
        3981 => assets.grass.clone(),                  // SPR_FLAT_GRASS_TILE
        4000 => assets.rough_flat[0].clone(),          // SPR_FLAT_ROUGH_LAND
        id if id == SPR_FLAT_WATER_TILE as u16 => assets.water.clone(),
        _ => return None,
    };
    Some(DirectTileLayoutGround {
        atlas,
        width: 64.0,
        height: 31.0,
        x_offs: -31.0,
        y_offs: 0.0,
    })
}

/// Listas de vars Action2 que entran en el fingerprint por dominio.
pub(crate) mod vars {
    pub const ROAD: &[u8] = &[0x40, 0x42, 0x45, 0x5F];
    pub const RAIL_SIGNAL: &[u8] = &[0x10, 0x18, 0x40, 0x41, 0x42, 0x43, 0x44, 0x45, 0x5F];
    /// Variables de `IndustryTileScopeResolver` y entradas básicas del
    /// `IndustriesScopeResolver` que pueden cambiar la vista runtime.
    pub const INDUSTRY: &[u8] = &[0x40, 0x41, 0x42, 0x43, 0x44, 0x5F, 0x7A];
    /// Variables disponibles en `ObjectScopeResolver` para una tesela que ya
    /// está en el mapa: offset, terreno, pueblo/distancias, animación,
    /// propietario y random. Las variables de teselas vecinas siguen fuera
    /// del fingerprint hasta completar su contexto global.
    pub const OBJECT: &[u8] = &[0x40, 0x41, 0x43, 0x44, 0x45, 0x46, 0x5F];
    /// Variables de `HouseScopeResolver` presentes en `Tile`, pueblo,
    /// conteos precalculados y vecinos de la tesela.
    pub const HOUSE: &[u8] = &[
        0x40, 0x41, 0x42, 0x43, 0x44, 0x45, 0x46, 0x47, 0x60, 0x61, 0x62, 0x63, 0x5F,
    ];
    pub const ROAD_STOP: &[u8] = &[0x40, 0x41, 0x42, 0x43, 0x44, 0x49, 0x50, 0x5F];
    pub const STATION: &[u8] = &[0x10, 0x40, 0x42, 0x43, 0x4A, 0x5F, 0x67];
    /// Variables `AirportTileScopeResolver` que pueden cambiar la vista por
    /// posición, frame o estado de una tesela vecina. Las tablas
    /// `parameterized_vars` se incorporan además por `runtime_fingerprint`.
    pub const AIRPORT_TILE: &[u8] = &[0x41, 0x42, 0x43, 0x44, 0x5F, 0x60, 0x61, 0x62, 0x7A];
    /// Variables `CanalScopeResolver` usadas por pendientes, bordes y
    /// callbacks de `Canals`: altura, terreno, conectividad, random y
    /// parámetros del callback actual.
    pub const CANAL: &[u8] = &[0x0C, 0x10, 0x18, 0x80, 0x81, 0x82, 0x83];
    /// Dominio de vehículo conservado para las regresiones unitarias del
    /// fingerprint histórico; el renderer ya no usa un fingerprint de contexto
    /// como identidad del asset.
    #[cfg(test)]
    pub const TRAIN: &[u8] = &[0x10, 0x40, 0x47, 0x43, 0x5F, 0xB2, 0xB4, 0xC8];
}

#[cfg(test)]
mod tests {
    use super::*;
    use openttdrs_core::DecodedSprite;

    fn action1_sprite() -> ResolvedTileLayoutSprite {
        ResolvedTileLayoutSprite {
            sprite: Some(DecodedSprite {
                width: 1,
                height: 1,
                x_offs: 0,
                y_offs: 0,
                rgba: vec![0, 0, 0, 0],
                mask: Vec::new(),
            }),
            base_sprite: None,
            origin: [0, 0, 0],
            extent: [1, 1, 1],
        }
    }

    #[test]
    fn direct_base_ground_is_only_accepted_for_audited_flat_ground_ids() {
        let direct_ground = ResolvedTileLayoutSprite {
            sprite: None,
            base_sprite: Some(3981),
            origin: [0, 0, 0],
            extent: [0, 0, 0],
        };
        let layout = ResolvedTileLayout {
            ground: Some(direct_ground.clone()),
            sequence: vec![action1_sprite()],
            complete: true,
        };
        assert!(tile_layout_is_renderable(&layout));

        let mut unsupported_ground = layout.clone();
        unsupported_ground
            .ground
            .as_mut()
            .expect("ground")
            .base_sprite = Some(4061);
        assert!(tile_layout_is_renderable(&unsupported_ground));

        let mut direct_build = layout;
        direct_build.sequence[0].sprite = None;
        direct_build.sequence[0].base_sprite = Some(3981);
        assert!(
            !tile_layout_is_renderable(&direct_build),
            "base BUILD sprites need their own NFO anchors and bounds"
        );
    }
}
