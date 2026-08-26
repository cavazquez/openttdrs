//! Caché de sprites NewGRF para casas urbanas (Action1/2/3 Houses).

use std::collections::HashMap;

use bevy::prelude::*;
use openttdrs_core::HouseSpecDef;

use crate::render::newgrf_cache::{
    DecodedSpriteImagePolicy, decoded_sprite_image, runtime_fingerprint, vars,
};

/// `(house_id, view_idx, runtime_fp)` → textura RGBA.
#[derive(Resource, Default)]
pub(crate) struct NewGrfHouseSpriteCache {
    handles: HashMap<(u16, u8, u32), Handle<Image>>,
}

impl NewGrfHouseSpriteCache {
    pub(crate) fn clear(&mut self) {
        self.handles.clear();
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
        let idx = u8::try_from(view_idx % def.newgrf_views.len().max(1)).unwrap_or(0);
        let key = (def.id, idx, fp);
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
    }
}
