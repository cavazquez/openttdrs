//! Caché de sprites NewGRF para teselas de industria (Action1/3 IndustryTiles).

use std::collections::HashMap;

use bevy::prelude::*;
use openttdrs_core::IndustryTileSpecDef;

use crate::render::newgrf_cache::{
    DecodedSpriteImagePolicy, decoded_sprite_image, runtime_fingerprint, vars,
};
use crate::sprites::CompanyColour;

/// `(gfx, view_idx, company_colour, runtime_fp)` → textura RGBA.
#[derive(Resource, Default)]
pub(crate) struct NewGrfIndustrySpriteCache {
    handles: HashMap<(u16, u8, u8, u32), Handle<Image>>,
}

impl NewGrfIndustrySpriteCache {
    pub(crate) fn handle_for_runtime(
        &mut self,
        def: &IndustryTileSpecDef,
        view_idx: usize,
        colour: Option<CompanyColour>,
        ctx: &mut openttdrs_core::Action2EvalCtx,
        images: &mut Assets<Image>,
    ) -> Option<Handle<Image>> {
        let colour_key = colour.map(CompanyColour::as_u8).unwrap_or(0xFF);
        let fp = if def.newgrf_runtime.is_some() {
            runtime_fingerprint(ctx, vars::INDUSTRY, false)
        } else {
            0
        };
        let view = if def.newgrf_runtime.is_some() {
            def.newgrf_view_runtime(view_idx, ctx)?
        } else {
            def.newgrf_view(view_idx)?.clone()
        };
        let idx = u8::try_from(view_idx % def.newgrf_views.len().max(1)).unwrap_or(0);
        let key = (def.gfx.as_u16(), idx, colour_key, fp);
        Some(
            self.handles
                .entry(key)
                .or_insert_with(|| {
                    images.add(decoded_sprite_image(
                        &view,
                        DecodedSpriteImagePolicy::MaskedAndRecolored { colour },
                    ))
                })
                .clone(),
        )
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
    use openttdrs_core::{
        GameState, apply_newgrf_industry_tiles, build_action0_industry_tile_payload,
        build_grf_v2_industry_tile_with_preview_sprite,
    };

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
    }
}
