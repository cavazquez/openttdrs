//! Caché de sprites NewGRF para teselas de industria (Action1/3 IndustryTiles).

use std::collections::HashMap;

use bevy::prelude::*;
use openttdrs_core::IndustryTileSpecDef;

use crate::render::newgrf_cache::{
    DecodedSpriteImagePolicy, decoded_sprite_image,
    decoded_tile_layout_image_with_palette_and_twocc_map, runtime_fingerprint,
    twocc_map_for_palette, vars,
};
use crate::sprites::CompanyColour;

/// `(gfx, slot, company_colour, runtime_fp, sprite_modifiers, direct_palette)`
/// → textura RGBA.
/// El bit alto de `slot` separa piezas `TileSeq` de las vistas planas.
#[derive(Resource, Default)]
pub(crate) struct NewGrfIndustrySpriteCache {
    handles: HashMap<(u16, u16, u8, u32, u8, u16), Handle<Image>>,
    twocc_maps: Vec<Option<openttdrs_core::DecodedSprite>>,
}

impl NewGrfIndustrySpriteCache {
    pub(crate) fn clear(&mut self) {
        self.handles.clear();
    }

    /// Instala la tabla Action5 `0x0A` vigente para los TileLayout de
    /// industrias. Las texturas ya horneadas dependen de ella.
    pub(crate) fn set_twocc_maps(&mut self, maps: &[Option<openttdrs_core::DecodedSprite>]) {
        if self.twocc_maps != maps {
            self.handles.clear();
            self.twocc_maps = maps.to_vec();
        }
    }

    #[cfg(test)]
    pub(crate) fn handle_for_runtime(
        &mut self,
        def: &IndustryTileSpecDef,
        view_idx: usize,
        colour: Option<CompanyColour>,
        ctx: &mut openttdrs_core::Action2EvalCtx,
        images: &mut Assets<Image>,
    ) -> Option<Handle<Image>> {
        let view = if def.newgrf_runtime.is_some() {
            def.newgrf_view_runtime(view_idx, ctx)?
        } else {
            def.newgrf_view(view_idx)?.clone()
        };
        Some(self.handle_for_resolved_view(def, view_idx, colour, ctx, &view, images))
    }

    /// Materializa una vista ya resuelta sin volver a ejecutar Action2.
    pub(crate) fn handle_for_resolved_view(
        &mut self,
        def: &IndustryTileSpecDef,
        view_idx: usize,
        colour: Option<CompanyColour>,
        ctx: &openttdrs_core::Action2EvalCtx,
        view: &openttdrs_core::DecodedSprite,
        images: &mut Assets<Image>,
    ) -> Handle<Image> {
        let colour_key = colour.map(CompanyColour::as_u8).unwrap_or(0xFF);
        let fp = if def.newgrf_runtime.is_some() {
            runtime_fingerprint(ctx, vars::INDUSTRY, false)
        } else {
            0
        };
        // La vista runtime puede existir sin preview/vistas estáticas. No
        // aliasar sus orientaciones distintas en el slot cero del caché.
        let idx = if def.newgrf_runtime.is_some() {
            u16::try_from(view_idx).unwrap_or(u16::MAX)
        } else {
            u16::try_from(view_idx % def.newgrf_views.len().max(1)).unwrap_or(0)
        };
        let key = (def.gfx.as_u16(), idx, colour_key, fp, 0, 0);
        self.handles
            .entry(key)
            .or_insert_with(|| {
                images.add(decoded_sprite_image(
                    view,
                    DecodedSpriteImagePolicy::MaskedAndRecolored { colour },
                ))
            })
            .clone()
    }

    /// Materializa una pieza ya resuelta de un layout `TileSeq` de industria.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn handle_for_layout(
        &mut self,
        def: &IndustryTileSpecDef,
        slot: u16,
        colour: Option<CompanyColour>,
        runtime_fp: u32,
        sprite_modifiers: u8,
        direct_palette: u16,
        sprite: &openttdrs_core::DecodedSprite,
        images: &mut Assets<Image>,
    ) -> Handle<Image> {
        let colour_key = colour.map(CompanyColour::as_u8).unwrap_or(0xFF);
        let key = (
            def.gfx.as_u16(),
            0x8000 | (slot & 0x7FFF),
            colour_key,
            runtime_fp,
            sprite_modifiers,
            direct_palette,
        );
        let twocc_map = twocc_map_for_palette(&self.twocc_maps, direct_palette);
        self.handles
            .entry(key)
            .or_insert_with(|| {
                images.add(decoded_tile_layout_image_with_palette_and_twocc_map(
                    sprite,
                    sprite_modifiers,
                    direct_palette,
                    DecodedSpriteImagePolicy::MaskedAndRecolored { colour },
                    twocc_map.as_ref(),
                ))
            })
            .clone()
    }
}

#[must_use]
pub(crate) fn newgrf_industry_tile_def(
    catalog: &[IndustryTileSpecDef],
    gfx: u16,
) -> Option<&IndustryTileSpecDef> {
    let def = openttdrs_core::industry_tile_spec_def(catalog, gfx)?;
    if def.has_newgrf_sprites() {
        Some(def)
    } else {
        None
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use openttdrs_core::apply_newgrf_industry_tiles;
    use openttdrs_core::newgrf_actions::build_action0_industry_tile_payload;
    use openttdrs_core::newgrf_sprites::build_grf_v2_industry_tile_with_preview_sprite;
    use openttdrs_core::prelude::GameState;
    use openttdrs_core::{
        Action2VarAdjust, Action2VarEntry, Action2VarTerm, DecodedSprite, IndustryTileGfxId,
        TrainSpriteAssign, TrainSpriteGraphics,
    };

    fn solid(r: u8, g: u8, b: u8) -> DecodedSprite {
        DecodedSprite {
            width: 1,
            height: 1,
            x_offs: 0,
            y_offs: 0,
            rgba: vec![r, g, b, 255],
            mask: Vec::new(),
        }
    }

    fn industry_tile_with_parent_7c_selector() -> IndustryTileSpecDef {
        let red = solid(255, 0, 0);
        let blue = solid(0, 0, 255);
        let mut runtime = TrainSpriteGraphics {
            sets: vec![vec![red.clone()], vec![blue.clone()]],
            assigns: vec![TrainSpriteAssign {
                local_id: 3,
                set_id: 4,
            }],
            ..Default::default()
        };
        runtime.action2_var.insert(
            4,
            Action2VarEntry {
                // 0x80 es el marcador de scope parent que el parser añade a
                // un Action2 tipo 0x82; el selector lee parent 7C[5].
                first: Action2VarTerm {
                    variable: 0x7C,
                    param: Some(5),
                    adjust: Action2VarAdjust {
                        shift: 0x80,
                        and_mask: 0xFF,
                        ..Default::default()
                    },
                },
                ops: Vec::new(),
                ranges: vec![(7, 0, 0), (8, 1, 1)],
                default: 7,
            },
        );
        runtime.action2_to_action1.insert(7, 0);
        runtime.action2_to_action1.insert(8, 1);
        IndustryTileSpecDef {
            gfx: IndustryTileGfxId(175),
            subst_id: 0,
            from_newgrf: true,
            slopes_refused: 0,
            accepts_cargo_indices: Vec::new(),
            accepts_cargo_labels: Vec::new(),
            acceptance: Vec::new(),
            callback_mask: 0,
            animation_frames: 0,
            animation_status: 0,
            animation_speed: 0,
            animation_triggers: 0,
            animation_special_flags: 0,
            associated_badges: Vec::new(),
            newgrf_badge_translation: Vec::new(),
            newgrf_local_id: 3,
            newgrf_grfid: 0,
            newgrf_preview: Some(red.clone()),
            newgrf_views: vec![red, blue],
            newgrf_runtime: Some(Box::new(runtime)),
        }
    }

    #[test]
    fn industry_sprite_cache_builds_handle_from_catalog_views() {
        let a0 = build_action0_industry_tile_payload(0, None);
        let mut indices = vec![0u8; 8 * 8];
        for y in 2..6 {
            for x in 2..6 {
                indices[y * 8 + x] = 174;
            }
        }
        let bytes = build_grf_v2_industry_tile_with_preview_sprite(
            &a0,
            0,
            8,
            8,
            &indices,
            [b'I', b'W', 0, 1],
            "iworld",
        );
        let dir = tempfile::tempdir().expect("tmp");
        std::fs::write(dir.path().join("iworld.grf"), &bytes).expect("write");
        let mut state = GameState::new(4, 4);
        state
            .newgrf_stack
            .push(openttdrs_core::NewGrfEntry::new("iworld.grf", 2));
        apply_newgrf_industry_tiles(&mut state, &[dir.path()]);
        let def = state
            .industry_tile_spec_catalog
            .iter()
            .find(|d| d.from_newgrf)
            .expect("newgrf industry tile");
        let mut images = Assets::<Image>::default();
        let mut cache = NewGrfIndustrySpriteCache::default();
        let mut ctx = openttdrs_core::Action2EvalCtx::default();
        let handle = cache
            .handle_for_runtime(def, 0, None, &mut ctx, &mut images)
            .expect("handle");
        assert!(images.get(&handle).is_some());

        let sprite = DecodedSprite {
            width: 2,
            height: 1,
            x_offs: 0,
            y_offs: 0,
            rgba: vec![40, 40, 40, 255, 120, 120, 120, 255],
            mask: vec![0xC6, 0x50],
        };
        let mut indices: Vec<u8> = (0..=u8::MAX).collect();
        indices[0xC6] = 174;
        indices[0x50] = 175;
        let map = DecodedSprite {
            width: 256,
            height: 1,
            x_offs: 0,
            y_offs: 0,
            rgba: openttdrs_core::newgrf_sprites::indices_to_rgba(&indices, 256, 1).unwrap(),
            mask: Vec::new(),
        };
        let direct_palette = openttdrs_core::TWOCC_PALETTE_BASE + 2 + 3 * 16;
        let mut maps = vec![None; openttdrs_core::TWOCC_ACTION5_SLOT_COUNT];
        maps[usize::from(direct_palette - openttdrs_core::TWOCC_PALETTE_BASE)] = Some(map.clone());
        cache.set_twocc_maps(&maps);
        let layout_handle = cache.handle_for_layout(
            def,
            0,
            Some(CompanyColour::Red),
            0,
            openttdrs_core::newgrf_sprites::TILE_LAYOUT_SPRITE_MODIFIER_RECOLOUR,
            direct_palette,
            &sprite,
            &mut images,
        );
        assert_eq!(
            images
                .get(&layout_handle)
                .expect("industry 2CC TileLayout")
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
    }

    #[test]
    fn industry_sprite_cache_rekeys_parent_7c_without_losing_reuse() {
        let def = industry_tile_with_parent_7c_selector();
        let mut images = Assets::<Image>::default();
        let mut cache = NewGrfIndustrySpriteCache::default();

        let mut first = openttdrs_core::Action2EvalCtx::default();
        first.parent_persistent_registers.insert(5, 0);
        let red_handle = cache
            .handle_for_runtime(&def, 0, None, &mut first, &mut images)
            .expect("vista roja de parent 7C=0");
        assert_eq!(
            images
                .get(&red_handle)
                .and_then(|image| image.data.as_deref()),
            Some(&[255, 0, 0, 255][..])
        );

        let mut second = openttdrs_core::Action2EvalCtx::default();
        second.parent_persistent_registers.insert(5, 1);
        let blue_handle = cache
            .handle_for_runtime(&def, 0, None, &mut second, &mut images)
            .expect("vista azul de parent 7C=1");
        assert_ne!(red_handle, blue_handle);
        assert_eq!(
            images
                .get(&blue_handle)
                .and_then(|image| image.data.as_deref()),
            Some(&[0, 0, 255, 255][..])
        );
        assert_eq!(cache.handles.len(), 2);

        let mut repeated = openttdrs_core::Action2EvalCtx::default();
        repeated.parent_persistent_registers.insert(5, 1);
        let reused = cache
            .handle_for_runtime(&def, 0, None, &mut repeated, &mut images)
            .expect("vista azul repetida");
        assert_eq!(reused, blue_handle);
        assert_eq!(cache.handles.len(), 2);
        assert_eq!(images.len(), 2);
    }

    #[test]
    fn industry_runtime_only_cache_keeps_view_index() {
        let mut def = industry_tile_with_parent_7c_selector();
        let red = def.newgrf_views[0].clone();
        let blue = def.newgrf_views[1].clone();
        let local_id = def.newgrf_local_id;
        def.newgrf_views.clear();
        def.newgrf_runtime = Some(Box::new(TrainSpriteGraphics {
            sets: vec![vec![red.clone(), blue.clone()]],
            assigns: vec![TrainSpriteAssign {
                local_id,
                set_id: 0,
            }],
            ..Default::default()
        }));

        let mut images = Assets::<Image>::default();
        let mut cache = NewGrfIndustrySpriteCache::default();
        let mut first_ctx = openttdrs_core::Action2EvalCtx::default();
        let first = cache
            .handle_for_runtime(&def, 0, None, &mut first_ctx, &mut images)
            .expect("industry view 0");
        let mut second_ctx = openttdrs_core::Action2EvalCtx::default();
        let second = cache
            .handle_for_runtime(&def, 1, None, &mut second_ctx, &mut images)
            .expect("industry view 1");
        assert_ne!(first, second);
        assert_eq!(
            images.get(&first).and_then(|image| image.data.as_deref()),
            Some(&red.rgba[..])
        );
        assert_eq!(
            images.get(&second).and_then(|image| image.data.as_deref()),
            Some(&blue.rgba[..])
        );
    }
}
