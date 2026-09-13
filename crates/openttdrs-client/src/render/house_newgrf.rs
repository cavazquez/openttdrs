//! Caché de sprites NewGRF para casas urbanas (Action1/2/3 Houses).

use std::collections::HashMap;

use bevy::prelude::*;
use openttdrs_core::HouseSpecDef;

use crate::render::newgrf_cache::{
    DecodedSpriteImagePolicy, decoded_sprite_image,
    decoded_tile_layout_image_with_palette_and_twocc_map, runtime_fingerprint,
    twocc_map_for_palette, vars,
};

/// `(house_id, slot, runtime_fp, sprite_modifiers, direct_palette)` → textura
/// RGBA. El bit alto de `slot` separa piezas `TileSeq` de vistas planas.
#[derive(Resource, Default)]
pub(crate) struct NewGrfHouseSpriteCache {
    handles: HashMap<(u16, u16, u32, u8, u16), Handle<Image>>,
    twocc_maps: Vec<Option<openttdrs_core::DecodedSprite>>,
}

impl NewGrfHouseSpriteCache {
    pub(crate) fn clear(&mut self) {
        self.handles.clear();
    }

    /// Instala la tabla Action5 `0x0A` vigente para los TileLayout de casas.
    /// Las texturas ya horneadas dependen de ella.
    pub(crate) fn set_twocc_maps(&mut self, maps: &[Option<openttdrs_core::DecodedSprite>]) {
        if self.twocc_maps != maps {
            self.handles.clear();
            self.twocc_maps = maps.to_vec();
        }
    }

    /// Textura resolviendo Action2 con las variables de la tesela.
    pub(crate) fn handle_for_runtime(
        &mut self,
        def: &HouseSpecDef,
        view_idx: usize,
        ctx: &mut openttdrs_core::Action2EvalCtx,
        images: &mut Assets<Image>,
    ) -> Option<Handle<Image>> {
        let fp = if def.newgrf_runtime.is_some() {
            runtime_fingerprint(ctx, vars::HOUSE, false)
        } else {
            0
        };
        let view = if def.newgrf_runtime.is_some() {
            def.newgrf_view_runtime(view_idx, ctx)?
        } else {
            def.newgrf_view(view_idx)?.clone()
        };
        let idx = u16::try_from(view_idx % def.newgrf_views.len().max(1)).unwrap_or(0);
        let key = (def.id, idx, fp, 0, 0);
        Some(
            self.handles
                .entry(key)
                .or_insert_with(|| {
                    // HouseSpec todavía no conserva `random_colour`; por eso
                    // no se aplica una paleta de compañía aquí. La textura
                    // cruda mantiene los píxeles decodificados y deja esa
                    // diferencia explícita en la matriz de paridad.
                    images.add(decoded_sprite_image(&view, DecodedSpriteImagePolicy::Raw))
                })
                .clone(),
        )
    }

    /// Materializa una pieza ya resuelta de un layout `TileSeq` de casa.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn handle_for_layout(
        &mut self,
        def: &HouseSpecDef,
        slot: u16,
        runtime_fp: u32,
        sprite_modifiers: u8,
        direct_palette: u16,
        sprite: &openttdrs_core::DecodedSprite,
        images: &mut Assets<Image>,
    ) -> Handle<Image> {
        let key = (
            def.id,
            0x8000 | (slot & 0x7FFF),
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
                    DecodedSpriteImagePolicy::Raw,
                    twocc_map.as_ref(),
                ))
            })
            .clone()
    }
}

/// Spec NewGRF con vistas Action1/3 para un `HouseID` ya resuelto.
#[must_use]
pub(crate) fn newgrf_house_def_for_id(
    catalog: &[HouseSpecDef],
    house_id: u16,
) -> Option<&HouseSpecDef> {
    let def = openttdrs_core::house_spec_def(catalog, house_id)?;
    def.has_newgrf_sprites().then_some(def)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use openttdrs_core::{
        Action2VarAdjust, Action2VarEntry, Action2VarTerm, DEFAULT_HOUSE_AVAILABILITY,
        DEFAULT_HOUSE_PROBABILITY, DecodedSprite, TrainSpriteAssign, TrainSpriteGraphics,
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

    #[test]
    fn cache_re_resolves_house_action2_for_tile_context() {
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
                first: Action2VarTerm {
                    variable: 0x41,
                    param: None,
                    adjust: Action2VarAdjust {
                        and_mask: 0xFF,
                        ..Default::default()
                    },
                },
                ops: Vec::new(),
                ranges: vec![(7, 1, 1)],
                default: 8,
            },
        );
        runtime.action2_to_action1.insert(7, 0);
        runtime.action2_to_action1.insert(8, 1);
        let def = HouseSpecDef {
            id: 110,
            local_id: 3,
            subst_id: 0,
            building_flags: openttdrs_core::house_spec::BUILDING_FLAG_SIZE_1X1,
            min_year: 0,
            max_year: 5000,
            population: 1,
            mail_generation: 1,
            availability: DEFAULT_HOUSE_AVAILABILITY,
            probability: DEFAULT_HOUSE_PROBABILITY,
            processing_time: 0,
            extra_flags: 0,
            animation_frames: 0,
            animation_status: 0xFF,
            animation_speed: 2,
            override_id: None,
            callback_mask: 0,
            name: "runtime house".into(),
            from_newgrf: true,
            grfid: 0,
            newgrf_views: vec![red, blue],
            newgrf_local_id: 3,
            newgrf_runtime: Some(Box::new(runtime)),
        };
        let mut images = Assets::<Image>::default();
        let mut cache = NewGrfHouseSpriteCache::default();
        let mut first = openttdrs_core::Action2EvalCtx::default();
        first.vars.insert(0x41, 1);
        let red_handle = cache
            .handle_for_runtime(&def, 0, &mut first, &mut images)
            .expect("red runtime view");
        let mut second = openttdrs_core::Action2EvalCtx::default();
        second.vars.insert(0x41, 2);
        let blue_handle = cache
            .handle_for_runtime(&def, 0, &mut second, &mut images)
            .expect("blue runtime view");
        assert_ne!(red_handle, blue_handle);
        assert_eq!(cache.handles.len(), 2);

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
            &def,
            0,
            0,
            openttdrs_core::newgrf_sprites::TILE_LAYOUT_SPRITE_MODIFIER_RECOLOUR,
            direct_palette,
            &sprite,
            &mut images,
        );
        assert_eq!(
            images
                .get(&layout_handle)
                .expect("house 2CC TileLayout")
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
}
