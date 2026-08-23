//! Caché unificada de sprites Action5 (foundations / oneway / roadstops / GUI / …).

use std::collections::HashMap;

use bevy::prelude::*;
use openttdrs_core::DecodedSprite;

use crate::render::newgrf_cache::{DecodedSpriteImagePolicy, decoded_sprite_image};

/// Clave `(type_id, slot, runtime_fp)` → textura RGBA.
///
/// Action5 usa siempre `runtime_fp=0`; los RoadStops Action3 reutilizan la
/// caché con el fingerprint de su contexto Action2 para no congelar la primera
/// variante random que se haya renderizado.
#[derive(Resource, Default)]
pub(crate) struct NewGrfAction5SpriteCache {
    handles: HashMap<(u8, u16, u32), Handle<Image>>,
}

impl NewGrfAction5SpriteCache {
    pub(crate) fn clear(&mut self) {
        self.handles.clear();
    }

    pub(crate) fn handle_for(
        &mut self,
        type_id: u8,
        slot: u16,
        sprite: &DecodedSprite,
        images: &mut Assets<Image>,
    ) -> Handle<Image> {
        self.handle_for_variant(type_id, slot, 0, sprite, images)
    }

    /// Textura de una variante Action2 cuya identidad adicional es `runtime_fp`.
    pub(crate) fn handle_for_variant(
        &mut self,
        type_id: u8,
        slot: u16,
        runtime_fp: u32,
        sprite: &DecodedSprite,
        images: &mut Assets<Image>,
    ) -> Handle<Image> {
        self.handles
            .entry((type_id, slot, runtime_fp))
            .or_insert_with(|| {
                images.add(decoded_sprite_image(sprite, DecodedSpriteImagePolicy::Raw))
            })
            .clone()
    }

    pub(crate) fn sprite_colored(
        &mut self,
        type_id: u8,
        slot: usize,
        table: &[Option<DecodedSprite>],
        tint: Color,
        images: &mut Assets<Image>,
    ) -> Option<Sprite> {
        let decoded = table.get(slot).and_then(|s| s.as_ref())?;
        let slot_u16 = u16::try_from(slot).ok()?;
        let handle = self.handle_for(type_id, slot_u16, decoded, images);
        Some(Sprite {
            image: handle,
            color: tint,
            ..default()
        })
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use openttdrs_core::apply_newgrf_action5_foundations;
    use openttdrs_core::newgrf_sprites::build_grf_v2_action5_with_sprite;
    use openttdrs_core::prelude::GameState;

    #[test]
    fn foundation_action5_cache_builds_handle() {
        let mut indices = vec![0u8; 8 * 8];
        for y in 2..6 {
            for x in 2..6 {
                indices[y * 8 + x] = 174;
            }
        }
        let bytes =
            build_grf_v2_action5_with_sprite(0x06, 0, 8, 8, &indices, [b'F', b'N', 0, 3], "fn3");
        let dir = tempfile::tempdir().expect("tmp");
        std::fs::write(dir.path().join("fn3.grf"), &bytes).expect("write");
        let mut state = GameState::new(4, 4);
        state
            .newgrf_stack
            .push(openttdrs_core::NewGrfEntry::new("fn3.grf", 2));
        apply_newgrf_action5_foundations(&mut state, &[dir.path()]);
        let spr = state.runtime.foundation_newgrf_sprites[0]
            .as_ref()
            .expect("slot 0");
        let mut images = Assets::<Image>::default();
        let mut cache = NewGrfAction5SpriteCache::default();
        let handle = cache.handle_for(0x06, 0, spr, &mut images);
        assert!(images.get(&handle).is_some());
    }

    #[test]
    fn runtime_variants_do_not_reuse_the_first_road_stop_sprite() {
        let red = DecodedSprite {
            width: 1,
            height: 1,
            x_offs: 0,
            y_offs: 0,
            rgba: vec![255, 0, 0, 255],
            mask: Vec::new(),
        };
        let blue = DecodedSprite {
            width: 1,
            height: 1,
            x_offs: 0,
            y_offs: 0,
            rgba: vec![0, 0, 255, 255],
            mask: Vec::new(),
        };
        let mut images = Assets::<Image>::default();
        let mut cache = NewGrfAction5SpriteCache::default();
        let first = cache.handle_for_variant(0x14, 6, 10, &red, &mut images);
        let repeated = cache.handle_for_variant(0x14, 6, 10, &red, &mut images);
        let changed = cache.handle_for_variant(0x14, 6, 11, &blue, &mut images);
        assert_eq!(first, repeated);
        assert_ne!(first, changed);
        assert_eq!(images.len(), 2);
    }
}
