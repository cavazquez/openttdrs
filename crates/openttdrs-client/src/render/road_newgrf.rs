//! Caché de sprites NewGRF para carretera in-world (Action1/3 RoadTypes).

use std::collections::HashMap;

use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use openttdrs_core::{DecodedSprite, RoadTypeDef};

/// `(road_type_id, view_idx)` → textura RGBA.
#[derive(Resource, Default)]
pub(crate) struct NewGrfRoadSpriteCache {
    handles: HashMap<(u8, u8), Handle<Image>>,
}

impl NewGrfRoadSpriteCache {
    pub(crate) fn clear(&mut self) {
        self.handles.clear();
    }

    fn decoded_to_image(sprite: &DecodedSprite) -> Image {
        Image::new(
            Extent3d {
                width: u32::from(sprite.width),
                height: u32::from(sprite.height),
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            sprite.rgba.clone(),
            TextureFormat::Rgba8UnormSrgb,
            default(),
        )
    }

    /// Textura de la vista `view_idx` (índice OpenGFX plano / `road_flat_sprite_index`).
    pub(crate) fn handle_for(
        &mut self,
        def: &RoadTypeDef,
        view_idx: usize,
        images: &mut Assets<Image>,
    ) -> Option<Handle<Image>> {
        let view = def.newgrf_view(view_idx)?;
        let idx = u8::try_from(view_idx % def.newgrf_views.len().max(1)).unwrap_or(0);
        let key = (def.id.as_u8(), idx);
        Some(
            self.handles
                .entry(key)
                .or_insert_with(|| images.add(Self::decoded_to_image(view)))
                .clone(),
        )
    }
}

/// Si el tipo de carretera de la tesela trae vistas NewGRF, devuelve el def.
#[must_use]
pub(crate) fn newgrf_road_def_for_tile(
    catalog: &[RoadTypeDef],
    tile: openttdrs_core::map::Tile,
) -> Option<&RoadTypeDef> {
    let rt = openttdrs_core::road_type_from_tile(&tile);
    if rt.is_vanilla() {
        return None;
    }
    let def = openttdrs_core::road_type_def(catalog, rt)?;
    if def.newgrf_view(0).is_some() {
        Some(def)
    } else {
        None
    }
}

/// Índice de vista NewGRF en plano: igual que `road_flat_sprite_index(0, road_bits)`.
#[must_use]
pub(crate) fn road_newgrf_view_index(road_bits: u8) -> usize {
    crate::sprites::road_flat_sprite_index(0, road_bits)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use openttdrs_core::{
        GameState, apply_newgrf_road_types, build_action0_roadtype_payload,
        build_grf_v2_roadtype_with_preview_sprite,
    };

    #[test]
    fn road_sprite_cache_builds_handle_from_catalog_views() {
        let a0 = build_action0_roadtype_payload(b"COBB", false, 1970, "Cobble Road");
        let mut indices = vec![0u8; 8 * 8];
        for y in 2..6 {
            for x in 2..6 {
                indices[y * 8 + x] = 174;
            }
        }
        let bytes = build_grf_v2_roadtype_with_preview_sprite(
            &a0,
            0,
            8,
            8,
            &indices,
            [b'R', b'W', 0, 1],
            "rworld",
        );
        let dir = tempfile::tempdir().expect("tmp");
        std::fs::write(dir.path().join("rworld.grf"), &bytes).expect("write");
        let mut state = GameState::new(4, 4);
        state
            .newgrf_stack
            .push(openttdrs_core::NewGrfEntry::new("rworld.grf", 2));
        apply_newgrf_road_types(&mut state, &[dir.path()]);
        let def = state
            .road_type_catalog
            .iter()
            .find(|d| d.from_newgrf)
            .expect("newgrf road");
        let mut images = Assets::<Image>::default();
        let mut cache = NewGrfRoadSpriteCache::default();
        let handle = cache.handle_for(def, 0, &mut images).expect("handle");
        assert!(images.get(&handle).is_some());
        let again = cache.handle_for(def, 0, &mut images).expect("cached");
        assert_eq!(handle, again);
        // Curva / cruce: índices distintos al 0 (módulo len si hay una sola vista).
        let cross = road_newgrf_view_index(0x0F);
        assert_ne!(cross, 0);
        let h_cross = cache.handle_for(def, cross, &mut images).expect("cross");
        // Con 1 vista, el módulo reutiliza el mismo handle.
        assert_eq!(handle, h_cross);
    }

    #[test]
    fn road_newgrf_view_index_matches_flat_opengfx_table() {
        assert_eq!(
            road_newgrf_view_index(0x05),
            crate::sprites::road_flat_sprite_index(0, 0x05)
        );
        assert_eq!(
            road_newgrf_view_index(0x0A),
            crate::sprites::road_flat_sprite_index(0, 0x0A)
        );
        assert_eq!(
            road_newgrf_view_index(0x0F),
            crate::sprites::road_flat_sprite_index(0, 0x0F)
        );
    }
}
