//! Caché unificada de sprites Action5 (foundations / oneway / roadstops / GUI / …).

use std::collections::HashMap;

use bevy::prelude::*;
use openttdrs_core::DecodedSprite;

use crate::render::newgrf_cache::{
    DecodedSpriteImagePolicy, decoded_sprite_image,
    decoded_tile_layout_image_with_palette_and_twocc_map, twocc_map_for_palette,
};
use crate::sprites::CompanyColour;

type Action5CacheKey = (u8, u16, u32, Option<CompanyColour>, u8, u16);

/// Clave `(type_id, slot, runtime_fp, company_colour, sprite_modifiers,
/// direct_palette)` → textura RGBA.
///
/// Action5 usa siempre `runtime_fp=0`; los RoadStops Action3 reutilizan la
/// caché con el fingerprint de su contexto Action2 para no congelar la primera
/// variante random que se haya renderizado.
#[derive(Resource, Default)]
pub(crate) struct NewGrfAction5SpriteCache {
    handles: HashMap<Action5CacheKey, Handle<Image>>,
    twocc_maps: Vec<Option<DecodedSprite>>,
}

impl NewGrfAction5SpriteCache {
    pub(crate) fn clear(&mut self) {
        self.handles.clear();
    }

    /// Instala la tabla Action5 `0x0A` vigente para los layouts sintéticos
    /// (roadstop y airport). Las texturas ya horneadas dependen de ella.
    pub(crate) fn set_twocc_maps(&mut self, maps: &[Option<DecodedSprite>]) {
        if self.twocc_maps != maps {
            self.handles.clear();
            self.twocc_maps = maps.to_vec();
        }
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
        self.handle_for_policy(
            (type_id, slot, runtime_fp, None, 0, 0),
            sprite,
            DecodedSpriteImagePolicy::Raw,
            images,
        )
    }

    /// Textura de una variante Action2 que conserva la máscara de color de
    /// compañía. `AirportTile` usa este caché sintético para sus layouts, y
    /// necesita la misma transformación que estaciones/industrias aunque no
    /// pertenezca a una tabla Action5 real.
    pub(crate) fn handle_for_variant_with_company_colour(
        &mut self,
        type_id: u8,
        slot: u16,
        runtime_fp: u32,
        colour: Option<CompanyColour>,
        sprite: &DecodedSprite,
        images: &mut Assets<Image>,
    ) -> Handle<Image> {
        self.handle_for_policy(
            (type_id, slot, runtime_fp, colour, 0, 0),
            sprite,
            DecodedSpriteImagePolicy::MaskedAndRecolored { colour },
            images,
        )
    }

    /// Variante de layout con la paleta de compañía como `default_palette`.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn handle_for_variant_with_company_colour_and_modifiers(
        &mut self,
        type_id: u8,
        slot: u16,
        runtime_fp: u32,
        colour: Option<CompanyColour>,
        sprite_modifiers: u8,
        direct_palette: u16,
        sprite: &DecodedSprite,
        images: &mut Assets<Image>,
    ) -> Handle<Image> {
        self.handle_for_layout_policy(
            (
                type_id,
                slot,
                runtime_fp,
                colour,
                sprite_modifiers,
                direct_palette,
            ),
            sprite,
            sprite_modifiers,
            direct_palette,
            DecodedSpriteImagePolicy::MaskedAndRecolored { colour },
            images,
        )
    }

    fn handle_for_policy(
        &mut self,
        key: Action5CacheKey,
        sprite: &DecodedSprite,
        policy: DecodedSpriteImagePolicy,
        images: &mut Assets<Image>,
    ) -> Handle<Image> {
        self.handles
            .entry(key)
            .or_insert_with(|| images.add(decoded_sprite_image(sprite, policy)))
            .clone()
    }

    fn handle_for_layout_policy(
        &mut self,
        key: Action5CacheKey,
        sprite: &DecodedSprite,
        sprite_modifiers: u8,
        direct_palette: u16,
        policy: DecodedSpriteImagePolicy,
        images: &mut Assets<Image>,
    ) -> Handle<Image> {
        let twocc_map = twocc_map_for_palette(&self.twocc_maps, direct_palette);
        self.handles
            .entry(key)
            .or_insert_with(|| {
                images.add(decoded_tile_layout_image_with_palette_and_twocc_map(
                    sprite,
                    sprite_modifiers,
                    direct_palette,
                    policy,
                    twocc_map.as_ref(),
                ))
            })
            .clone()
    }

    /// Materializa un Action5 que `DrawRailTileSeq` pinta con una paleta de
    /// compañía. La clave conserva el color para que dos depósitos de dueños
    /// distintos no reutilicen la primera textura horneada.
    pub(crate) fn sprite_company_palette(
        &mut self,
        type_id: u8,
        slot: usize,
        sprite: &DecodedSprite,
        colour: CompanyColour,
        images: &mut Assets<Image>,
    ) -> Option<Sprite> {
        let slot = u16::try_from(slot).ok()?;
        let handle = self.handle_for_policy(
            (type_id, slot, 0, Some(colour), 0, 0),
            sprite,
            DecodedSpriteImagePolicy::CompanyPalette { colour },
            images,
        );
        Some(Sprite {
            image: handle,
            color: Color::WHITE,
            ..default()
        })
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

    #[test]
    fn company_palette_variants_do_not_reuse_another_depot_owner() {
        let depot = DecodedSprite {
            width: 1,
            height: 1,
            x_offs: 0,
            y_offs: 0,
            rgba: vec![198, 198, 198, 255],
            mask: vec![198],
        };
        let mut images = Assets::<Image>::default();
        let mut cache = NewGrfAction5SpriteCache::default();
        let red = cache
            .sprite_company_palette(
                openttdrs_core::ACTION5_TYPE_TRAMWAY,
                113,
                &depot,
                CompanyColour::Red,
                &mut images,
            )
            .expect("red depot");
        let green = cache
            .sprite_company_palette(
                openttdrs_core::ACTION5_TYPE_TRAMWAY,
                113,
                &depot,
                CompanyColour::Green,
                &mut images,
            )
            .expect("green depot");
        assert_ne!(red.image, green.image);
        assert_eq!(images.len(), 2);
    }

    #[test]
    fn layout_2cc_uses_action5_map_and_invalidates_changed_map() {
        let sprite = DecodedSprite {
            width: 2,
            height: 1,
            x_offs: 0,
            y_offs: 0,
            rgba: vec![40, 40, 40, 255, 120, 120, 120, 255],
            mask: vec![0xC6, 0x50],
        };
        let mut map_indices: Vec<u8> = (0..=u8::MAX).collect();
        map_indices[0xC6] = 174;
        map_indices[0x50] = 175;
        let map = DecodedSprite {
            width: 256,
            height: 1,
            x_offs: 0,
            y_offs: 0,
            rgba: openttdrs_core::newgrf_sprites::indices_to_rgba(&map_indices, 256, 1).unwrap(),
            mask: Vec::new(),
        };
        let direct_palette = openttdrs_core::TWOCC_PALETTE_BASE + 2 + 3 * 16;
        let mut maps = vec![None; openttdrs_core::TWOCC_ACTION5_SLOT_COUNT];
        maps[usize::from(direct_palette - openttdrs_core::TWOCC_PALETTE_BASE)] = Some(map.clone());

        let mut images = Assets::<Image>::default();
        let mut cache = NewGrfAction5SpriteCache::default();
        cache.set_twocc_maps(&maps);
        let first = cache.handle_for_variant_with_company_colour_and_modifiers(
            0x14,
            6,
            0,
            Some(CompanyColour::Red),
            openttdrs_core::newgrf_sprites::TILE_LAYOUT_SPRITE_MODIFIER_RECOLOUR,
            direct_palette,
            &sprite,
            &mut images,
        );
        assert_eq!(
            images
                .get(&first)
                .expect("mapped roadstop layout")
                .data
                .as_deref(),
            Some(
                &openttdrs_core::bake_sprite_two_company_palette_with_map(
                    &sprite,
                    2,
                    3,
                    Some(&map),
                )[..]
            )
        );

        let mut changed_map = map.clone();
        let mut changed_indices: Vec<u8> = (0..=u8::MAX).collect();
        changed_indices[0xC6] = 180;
        changed_indices[0x50] = 181;
        changed_map.rgba =
            openttdrs_core::newgrf_sprites::indices_to_rgba(&changed_indices, 256, 1).unwrap();
        maps[usize::from(direct_palette - openttdrs_core::TWOCC_PALETTE_BASE)] =
            Some(changed_map.clone());
        cache.set_twocc_maps(&maps);
        let second = cache.handle_for_variant_with_company_colour_and_modifiers(
            0x14,
            6,
            0,
            Some(CompanyColour::Red),
            openttdrs_core::newgrf_sprites::TILE_LAYOUT_SPRITE_MODIFIER_RECOLOUR,
            direct_palette,
            &sprite,
            &mut images,
        );
        assert_ne!(first, second);
        assert_eq!(
            images
                .get(&second)
                .expect("changed roadstop layout")
                .data
                .as_deref(),
            Some(
                &openttdrs_core::bake_sprite_two_company_palette_with_map(
                    &sprite,
                    2,
                    3,
                    Some(&changed_map),
                )[..]
            )
        );
    }
}
