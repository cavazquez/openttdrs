//! Caché de sprites NewGRF para teselas de industria (Action1/3 IndustryTiles).

use std::collections::HashMap;

use bevy::prelude::*;
use openttdrs_core::IndustryTileSpecDef;

use crate::render::newgrf_cache::{
    DecodedSpriteImagePolicy, decoded_sprite_image, runtime_fingerprint, vars,
};
use crate::sprites::CompanyColour;

/// `(gfx, slot, company_colour, runtime_fp)` → textura RGBA. El bit alto de
/// `slot` separa piezas `TileSeq` de las vistas planas.
#[derive(Resource, Default)]
pub(crate) struct NewGrfIndustrySpriteCache {
    handles: HashMap<(u16, u16, u8, u32), Handle<Image>>,
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
        let idx = u16::try_from(view_idx % def.newgrf_views.len().max(1)).unwrap_or(0);
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

    /// Materializa una pieza ya resuelta de un layout `TileSeq` de industria.
    pub(crate) fn handle_for_layout(
        &mut self,
        def: &IndustryTileSpecDef,
        slot: u16,
        colour: Option<CompanyColour>,
        runtime_fp: u32,
        sprite: &openttdrs_core::DecodedSprite,
        images: &mut Assets<Image>,
    ) -> Handle<Image> {
        let colour_key = colour.map(CompanyColour::as_u8).unwrap_or(0xFF);
        let key = (
            def.gfx.as_u16(),
            0x8000 | (slot & 0x7FFF),
            colour_key,
            runtime_fp,
        );
        self.handles
            .entry(key)
            .or_insert_with(|| {
                images.add(decoded_sprite_image(
                    sprite,
                    DecodedSpriteImagePolicy::MaskedAndRecolored { colour },
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
}
