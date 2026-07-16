//! Caché de sprites NewGRF Action5 catenary in-world (`0x05`).

use std::collections::HashMap;

use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use openttdrs_core::{DecodedSprite, catenary_action5_local_slot};

use crate::render::{AtlasSprite, WorldAssets};

/// Slot local `0..35` → textura RGBA.
#[derive(Resource, Default)]
pub(crate) struct NewGrfCatenarySpriteCache {
    handles: HashMap<u8, Handle<Image>>,
}

impl NewGrfCatenarySpriteCache {
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

/// Sprite de catenaria: NewGRF Action5 si hay slot; si no, OpenGFX.
pub(crate) fn catenary_sprite_colored(
    assets: &WorldAssets,
    sprite_id: u32,
    tint: Color,
    catenary_newgrf: &[Option<DecodedSprite>],
    cache: Option<&mut NewGrfCatenarySpriteCache>,
    images: Option<&mut Assets<Image>>,
) -> Option<Sprite> {
    if let Some(slot) = catenary_action5_local_slot(sprite_id)
        && let (Some(cache), Some(images), Some(decoded)) = (
            cache,
            images,
            catenary_newgrf.get(slot).and_then(|s| s.as_ref()),
        )
    {
        let handle = cache.handle_for(u8::try_from(slot).unwrap_or(0), decoded, images);
        return Some(Sprite {
            image: handle,
            color: tint,
            ..default()
        });
    }
    assets
        .rail
        .get(&sprite_id)
        .map(|img: &AtlasSprite| img.sprite_colored(tint))
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use openttdrs_core::{
        GameState, apply_newgrf_action5_catenary, build_grf_v2_action5_with_sprite,
    };

    #[test]
    fn catenary_sprite_cache_builds_handle_from_action5() {
        let mut indices = vec![0u8; 8 * 8];
        for y in 2..6 {
            for x in 2..6 {
                indices[y * 8 + x] = 174;
            }
        }
        let bytes =
            build_grf_v2_action5_with_sprite(0x05, 1039, 8, 8, &indices, [b'E', b'L', 0, 3], "elw");
        let dir = tempfile::tempdir().expect("tmp");
        std::fs::write(dir.path().join("elw.grf"), &bytes).expect("write");
        let mut state = GameState::new(4, 4);
        state
            .newgrf_stack
            .push(openttdrs_core::NewGrfEntry::new("elw.grf", 2));
        apply_newgrf_action5_catenary(&mut state, &[dir.path()]);
        let spr = state.runtime.catenary_newgrf_sprites[0]
            .as_ref()
            .expect("slot 0");
        let mut images = Assets::<Image>::default();
        let mut cache = NewGrfCatenarySpriteCache::default();
        let handle = cache.handle_for(0, spr, &mut images);
        assert!(images.get(&handle).is_some());
    }
}
