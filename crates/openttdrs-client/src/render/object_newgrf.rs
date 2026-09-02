//! Caché de sprites NewGRF para objetos de mapa (Action1/3 Objects).

use std::collections::HashMap;

use bevy::prelude::*;
use openttdrs_core::{DecodedSprite, ObjectSpecDef};

use crate::render::newgrf_cache::{
    DecodedSpriteImagePolicy, decoded_sprite_image, runtime_fingerprint, vars,
};

/// `(spec_id, slot, runtime_fp)` → textura RGBA. El bit alto del slot separa
/// piezas TileSeq de vistas planas para no reutilizar una textura por error.
#[derive(Resource, Default)]
pub(crate) struct NewGrfObjectSpriteCache {
    handles: HashMap<(u16, u16, u32), Handle<Image>>,
}

impl NewGrfObjectSpriteCache {
    pub(crate) fn clear(&mut self) {
        self.handles.clear();
    }

    /// Textura Raw para una vista del spec (mirror industry/road NewGRF Raw).
    pub(crate) fn handle_for(
        &mut self,
        def: &ObjectSpecDef,
        view_idx: usize,
        view: &DecodedSprite,
        images: &mut Assets<Image>,
    ) -> Handle<Image> {
        let idx = u16::try_from(view_idx % def.views.len().max(1)).unwrap_or(0);
        let key = (def.id, idx, 0);
        self.handles
            .entry(key)
            .or_insert_with(|| {
                images.add(decoded_sprite_image(view, DecodedSpriteImagePolicy::Raw))
            })
            .clone()
    }

    /// Textura resolviendo Action2 con el contexto de la tesela.
    pub(crate) fn handle_for_runtime(
        &mut self,
        def: &ObjectSpecDef,
        view_idx: usize,
        ctx: &mut openttdrs_core::Action2EvalCtx,
        images: &mut Assets<Image>,
    ) -> Option<Handle<Image>> {
        let fp = if def.newgrf_runtime.is_some() {
            runtime_fingerprint(ctx, vars::OBJECT, false)
        } else {
            0
        };
        let view = if def.newgrf_runtime.is_some() {
            def.newgrf_view_runtime(view_idx, ctx)?
        } else {
            def.view(view_idx)?.clone()
        };
        let idx = u16::try_from(view_idx % def.views.len().max(1)).unwrap_or(0);
        let key = (def.id, idx, fp);
        Some(
            self.handles
                .entry(key)
                .or_insert_with(|| {
                    images.add(decoded_sprite_image(&view, DecodedSpriteImagePolicy::Raw))
                })
                .clone(),
        )
    }

    /// Materializa una pieza ya resuelta de un layout `TileSeq` de objeto.
    pub(crate) fn handle_for_layout(
        &mut self,
        def: &ObjectSpecDef,
        slot: u16,
        runtime_fp: u32,
        sprite: &DecodedSprite,
        images: &mut Assets<Image>,
    ) -> Handle<Image> {
        let key = (def.id, 0x8000 | (slot & 0x7FFF), runtime_fp);
        self.handles
            .entry(key)
            .or_insert_with(|| {
                images.add(decoded_sprite_image(sprite, DecodedSpriteImagePolicy::Raw))
            })
            .clone()
    }
}

/// Spec con vistas NewGRF para un `ObjectType` ya resuelto.
#[must_use]
pub(crate) fn newgrf_object_def_for_type(
    catalog: &[ObjectSpecDef],
    object_type: u16,
) -> Option<&ObjectSpecDef> {
    if !openttdrs_core::is_newgrf_object_type_id(object_type) {
        return None;
    }
    let def = openttdrs_core::object_spec_def(catalog, object_type)?;
    if def.has_views() { Some(def) } else { None }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use openttdrs_core::OBJECT_SIZE_1X1;

    #[test]
    fn object_sprite_cache_builds_handle_from_views() {
        let rgba = vec![
            255u8, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 0, 255,
        ];
        let view = DecodedSprite {
            width: 2,
            height: 2,
            x_offs: -1,
            y_offs: -2,
            rgba,
            mask: Vec::new(),
        };
        let def = ObjectSpecDef {
            id: 5,
            class_label: "TEST".into(),
            name: "t".into(),
            size: OBJECT_SIZE_1X1,
            from_newgrf: true,
            local_id: 0,
            grfid: 0,
            newgrf_grf_version: 0,
            climate_mask: openttdrs_core::DEFAULT_OBJECT_CLIMATE_MASK,
            build_cost_factor: openttdrs_core::DEFAULT_OBJECT_BUILD_COST_FACTOR,
            callback_mask: 0,
            views: vec![view.clone()],
            newgrf_runtime: None,
            associated_badges: Vec::new(),
        };
        let mut images = Assets::<Image>::default();
        let mut cache = NewGrfObjectSpriteCache::default();
        let handle = cache.handle_for(&def, 0, &view, &mut images);
        assert!(images.get(&handle).is_some());
    }

    #[test]
    fn object_sprite_cache_re_resolves_action2_for_tile_context() {
        use openttdrs_core::{
            Action2VarAdjust, Action2VarEntry, Action2VarTerm, DecodedSprite, TrainSpriteAssign,
            TrainSpriteGraphics,
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
        let def = ObjectSpecDef {
            id: 5,
            class_label: "TEST".into(),
            name: "runtime".into(),
            size: OBJECT_SIZE_1X1,
            from_newgrf: true,
            local_id: 3,
            grfid: 0,
            newgrf_grf_version: 0,
            climate_mask: openttdrs_core::DEFAULT_OBJECT_CLIMATE_MASK,
            build_cost_factor: openttdrs_core::DEFAULT_OBJECT_BUILD_COST_FACTOR,
            callback_mask: 0,
            views: vec![red, blue],
            newgrf_runtime: Some(Box::new(runtime)),
            associated_badges: Vec::new(),
        };
        let mut images = Assets::<Image>::default();
        let mut cache = NewGrfObjectSpriteCache::default();
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
