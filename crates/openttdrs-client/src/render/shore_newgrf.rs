//! Caché de sprites NewGRF Action5 shore in-world (`0x0D`).

use std::collections::HashMap;

use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use openttdrs_core::DecodedSprite;

/// Bit en `ShoreTile` para marcar costa NewGRF (no animar con frames OpenGFX).
pub(crate) const NEWGRF_SHORE_TILE_FLAG: u8 = 0x80;

/// Slot `0..17` → textura RGBA.
#[derive(Resource, Default)]
pub(crate) struct NewGrfShoreSpriteCache {
    handles: HashMap<u8, Handle<Image>>,
}

impl NewGrfShoreSpriteCache {
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

    /// Textura del slot shore NewGRF (lazy).
    pub(crate) fn handle_for(
        &mut self,
        slot: u8,
        sprite: &DecodedSprite,
        images: &mut Assets<Image>,
    ) -> Handle<Image> {
        self.handles
            .entry(slot)
            .or_insert_with(|| images.add(Self::decoded_to_image(sprite)))
            .clone()
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use openttdrs_core::{GameState, apply_newgrf_action5_shore, build_grf_v2_action5_with_sprite};

    #[test]
    fn shore_sprite_cache_builds_handle_from_action5() {
        let mut indices = vec![0u8; 8 * 8];
        for y in 2..6 {
            for x in 2..6 {
                indices[y * 8 + x] = 174;
            }
        }
        let bytes =
            build_grf_v2_action5_with_sprite(0x0D, 2, 8, 8, &indices, [b'S', b'W', 0, 3], "sworld");
        let dir = tempfile::tempdir().expect("tmp");
        std::fs::write(dir.path().join("sworld.grf"), &bytes).expect("write");
        let mut state = GameState::new(4, 4);
        state
            .newgrf_stack
            .push(openttdrs_core::NewGrfEntry::new("sworld.grf", 2));
        apply_newgrf_action5_shore(&mut state, &[dir.path()]);
        let spr = state.runtime.shore_newgrf_sprites[2]
            .as_ref()
            .expect("slot 2");
        let mut images = Assets::<Image>::default();
        let mut cache = NewGrfShoreSpriteCache::default();
        let handle = cache.handle_for(2, spr, &mut images);
        assert!(images.get(&handle).is_some());
        let again = cache.handle_for(2, spr, &mut images);
        assert_eq!(handle, again);
    }
}
