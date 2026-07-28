//! Caché de sprites NewGRF para objetos de mapa (Action1/3 Objects).

use std::collections::HashMap;

use bevy::prelude::*;
use openttdrs_core::{DecodedSprite, ObjectSpecDef};

use crate::render::newgrf_cache::{DecodedSpriteImagePolicy, decoded_sprite_image};

/// `(spec_id, view_idx)` → textura RGBA.
#[derive(Resource, Default)]
pub(crate) struct NewGrfObjectSpriteCache {
    handles: HashMap<(u16, u8), Handle<Image>>,
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
        let idx = u8::try_from(view_idx % def.views.len().max(1)).unwrap_or(0);
        let key = (def.id, idx);
        self.handles
            .entry(key)
            .or_insert_with(|| {
                images.add(decoded_sprite_image(view, DecodedSpriteImagePolicy::Raw))
            })
            .clone()
    }
}

/// Spec con vistas NewGRF para un `m5` de objeto.
#[must_use]
pub(crate) fn newgrf_object_def_for_m5(
    catalog: &[ObjectSpecDef],
    m5: u8,
) -> Option<&ObjectSpecDef> {
    if !openttdrs_core::is_newgrf_object_type(m5) {
        return None;
    }
    let def = openttdrs_core::object_spec_def(catalog, u16::from(m5))?;
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
            climate_mask: openttdrs_core::DEFAULT_OBJECT_CLIMATE_MASK,
            build_cost_factor: openttdrs_core::DEFAULT_OBJECT_BUILD_COST_FACTOR,
            views: vec![view.clone()],
            associated_badges: Vec::new(),
        };
        let mut images = Assets::<Image>::default();
        let mut cache = NewGrfObjectSpriteCache::default();
        let handle = cache.handle_for(&def, 0, &view, &mut images);
        assert!(images.get(&handle).is_some());
    }
}
