//! Caché de sprites NewGRF para carretera/tram in-world (Action1/3 RoadTypes).

use std::collections::HashMap;

use bevy::prelude::*;
use openttdrs_core::RoadTypeDef;

use crate::render::newgrf_cache::{
    DecodedSpriteImagePolicy, decoded_sprite_image, runtime_fingerprint, vars,
};

/// `(road_type_id, view_idx, runtime_fp)` → textura RGBA.
#[derive(Resource, Default)]
pub(crate) struct NewGrfRoadSpriteCache {
    handles: HashMap<(u8, u8, u32), Handle<Image>>,
    specific_handles: HashMap<(u8, u8, u8, u32), Handle<Image>>,
}

impl NewGrfRoadSpriteCache {
    pub(crate) fn clear(&mut self) {
        self.handles.clear();
        self.specific_handles.clear();
    }

    /// Textura re-resolviendo Action2 con vars de tesela.
    pub(crate) fn handle_for_runtime(
        &mut self,
        def: &RoadTypeDef,
        view_idx: usize,
        ctx: &mut openttdrs_core::Action2EvalCtx,
        images: &mut Assets<Image>,
    ) -> Option<Handle<Image>> {
        let fp = if def.newgrf_runtime.is_some() {
            runtime_fingerprint(ctx, vars::ROAD, false)
        } else {
            0
        };
        let view = if def.newgrf_runtime.is_some() {
            def.newgrf_view_runtime(view_idx, ctx)?
        } else {
            def.newgrf_view(view_idx)?.clone()
        };
        let idx = u8::try_from(view_idx % def.newgrf_views.len().max(1)).unwrap_or(0);
        let key = (def.id.as_u8(), idx, fp);
        Some(
            self.handles
                .entry(key)
                .or_insert_with(|| {
                    images.add(decoded_sprite_image(&view, DecodedSpriteImagePolicy::Raw))
                })
                .clone(),
        )
    }

    /// Textura de un grupo Action3 específico (`ROTSG_*`) con vars de tesela.
    /// El selector forma parte de la clave: dos grupos del mismo roadtype
    /// pueden resolver sets distintos para bridge/overlay/catenaria.
    pub(crate) fn handle_for_specific_runtime(
        &mut self,
        def: &RoadTypeDef,
        selector: u8,
        view_idx: usize,
        ctx: &mut openttdrs_core::Action2EvalCtx,
        images: &mut Assets<Image>,
    ) -> Option<Handle<Image>> {
        let fp = if def.newgrf_runtime.is_some() {
            runtime_fingerprint(ctx, vars::ROAD, false)
        } else {
            0
        };
        let view = def.newgrf_specific_view_runtime(selector, view_idx, ctx)?;
        let idx = u8::try_from(view_idx).unwrap_or(u8::MAX);
        let key = (def.id.as_u8(), selector, idx, fp);
        Some(
            self.specific_handles
                .entry(key)
                .or_insert_with(|| {
                    images.add(decoded_sprite_image(&view, DecodedSpriteImagePolicy::Raw))
                })
                .clone(),
        )
    }
}

/// Si el tipo de carretera de la tesela trae vistas NewGRF, devuelve el def.
#[must_use]
pub(crate) fn newgrf_road_def_for_tile(
    catalog: &[RoadTypeDef],
    tile: openttdrs_core::map::Tile,
) -> Option<&RoadTypeDef> {
    let rt = openttdrs_core::road_type_from_tile(&tile);
    if rt.is_vanilla() {
        return None;
    }
    let def = openttdrs_core::road_type_def(catalog, rt)?;
    if def.newgrf_view(0).is_some() || def.newgrf_runtime.is_some() {
        Some(def)
    } else {
        None
    }
}

/// Si el tipo de tranvía de la tesela trae vistas NewGRF, devuelve el def.
#[must_use]
pub(crate) fn newgrf_tram_def_for_tile(
    catalog: &[RoadTypeDef],
    tile: openttdrs_core::map::Tile,
) -> Option<&RoadTypeDef> {
    let rt = openttdrs_core::tram_road_type_from_tile(&tile)?;
    if rt.is_vanilla() {
        return None;
    }
    let def = openttdrs_core::road_type_def(catalog, rt)?;
    if def.newgrf_view(0).is_some() || def.newgrf_runtime.is_some() {
        Some(def)
    } else {
        None
    }
}

/// Índice de vista NewGRF: mismo que OpenGFX (`road_flat_sprite_index`).
#[must_use]
pub(crate) fn road_newgrf_view_index(tileh: u8, road_bits: u8) -> usize {
    crate::sprites::road_flat_sprite_index(tileh, road_bits)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use openttdrs_core::apply_newgrf_road_types;
    use openttdrs_core::map::TileKind;
    use openttdrs_core::newgrf_actions::build_action0_roadtype_payload;
    use openttdrs_core::newgrf_sprites::build_grf_v2_roadtype_with_preview_sprite;
    use openttdrs_core::prelude::GameState;
    use openttdrs_core::set_tram_road_type_on_tile;

    #[test]
    fn road_sprite_cache_builds_handle_from_catalog_views() {
        let a0 = build_action0_roadtype_payload(b"COBB", false, 1970, "Cobble Road");
        let mut indices = vec![0u8; 8 * 8];
        for y in 2..6 {
            for x in 2..6 {
                indices[y * 8 + x] = 174;
            }
        }
        let bytes = build_grf_v2_roadtype_with_preview_sprite(
            &a0,
            0,
            8,
            8,
            &indices,
            [b'R', b'W', 0, 1],
            "rworld",
        );
        let dir = tempfile::tempdir().expect("tmp");
        std::fs::write(dir.path().join("rworld.grf"), &bytes).expect("write");
        let mut state = GameState::new(4, 4);
        state
            .newgrf_stack
            .push(openttdrs_core::NewGrfEntry::new("rworld.grf", 2));
        apply_newgrf_road_types(&mut state, &[dir.path()]);
        let def = state
            .road_type_catalog
            .iter()
            .find(|d| d.from_newgrf)
            .expect("newgrf road");
        let mut images = Assets::<Image>::default();
        let mut cache = NewGrfRoadSpriteCache::default();
        let mut ctx = openttdrs_core::Action2EvalCtx::default();
        let handle = cache
            .handle_for_runtime(def, 0, &mut ctx, &mut images)
            .expect("handle");
        assert!(images.get(&handle).is_some());
        let again = cache
            .handle_for_runtime(def, 0, &mut ctx, &mut images)
            .expect("cached");
        assert_eq!(handle, again);
        // Curva / cruce: índices distintos al 0 (módulo len si hay una sola vista).
        let cross = road_newgrf_view_index(0, 0x0F);
        assert_ne!(cross, 0);
        let h_cross = cache
            .handle_for_runtime(def, cross, &mut ctx, &mut images)
            .expect("cross");
        // Con 1 vista, el módulo reutiliza el mismo handle.
        assert_eq!(handle, h_cross);
        // Pendiente diagonal: índice OpenGFX 11 (módulo → misma textura).
        let slope = road_newgrf_view_index(12, 0x05);
        assert_eq!(slope, 11);
        let h_slope = cache
            .handle_for_runtime(def, slope, &mut ctx, &mut images)
            .expect("slope");
        assert_eq!(handle, h_slope);
    }

    #[test]
    fn road_newgrf_view_index_matches_opengfx_table() {
        assert_eq!(
            road_newgrf_view_index(0, 0x05),
            crate::sprites::road_flat_sprite_index(0, 0x05)
        );
        assert_eq!(
            road_newgrf_view_index(0, 0x0A),
            crate::sprites::road_flat_sprite_index(0, 0x0A)
        );
        assert_eq!(
            road_newgrf_view_index(0, 0x0F),
            crate::sprites::road_flat_sprite_index(0, 0x0F)
        );
        // Pendientes diagonales: índices fijos 11–14.
        assert_eq!(road_newgrf_view_index(12, 0x05), 11); // NE
        assert_eq!(road_newgrf_view_index(6, 0x05), 12); // SE
        assert_eq!(road_newgrf_view_index(3, 0x05), 13); // SW
        assert_eq!(road_newgrf_view_index(9, 0x05), 14); // NW
    }

    #[test]
    fn newgrf_tram_def_requires_non_vanilla_tram_type() {
        let a0 = build_action0_roadtype_payload(b"TRMX", true, 1980, "Fancy Tram");
        let mut indices = vec![0u8; 8 * 8];
        for y in 2..6 {
            for x in 2..6 {
                indices[y * 8 + x] = 174;
            }
        }
        let bytes = build_grf_v2_roadtype_with_preview_sprite(
            &a0,
            0,
            8,
            8,
            &indices,
            [b'T', b'W', 0, 1],
            "tworld",
        );
        let dir = tempfile::tempdir().expect("tmp");
        std::fs::write(dir.path().join("tworld.grf"), &bytes).expect("write");
        let mut state = GameState::new(4, 4);
        state
            .newgrf_stack
            .push(openttdrs_core::NewGrfEntry::new("tworld.grf", 2));
        apply_newgrf_road_types(&mut state, &[dir.path()]);
        let tram_id = state
            .road_type_catalog
            .iter()
            .find(|d| d.from_newgrf)
            .expect("newgrf tram")
            .id;

        let mut tile = state.map.get(openttdrs_core::TileCoord::new(0, 0)).unwrap();
        tile.kind = TileKind::Road;
        tile = set_tram_road_type_on_tile(tile, Some(tram_id));
        assert!(newgrf_tram_def_for_tile(&state.road_type_catalog, tile).is_some());

        tile = set_tram_road_type_on_tile(tile, Some(openttdrs_core::RoadType::TRAM));
        assert!(newgrf_tram_def_for_tile(&state.road_type_catalog, tile).is_none());
    }

    #[test]
    fn specific_bridge_group_survives_roadtype_catalog_and_uses_selector_cache() {
        use openttdrs_core::newgrf_sprites::{
            DecodedSprite, TrainSpriteAssign, TrainSpriteGraphics,
        };
        use openttdrs_core::{RoadTramType, RoadType};

        let sprite = DecodedSprite {
            width: 1,
            height: 1,
            x_offs: 0,
            y_offs: 0,
            rgba: vec![255, 0, 0, 255],
            mask: Vec::new(),
        };
        let mut graphics = TrainSpriteGraphics {
            sets: vec![vec![sprite.clone()], vec![sprite.clone()]],
            assigns: vec![TrainSpriteAssign {
                local_id: 0,
                set_id: 0,
            }],
            ..TrainSpriteGraphics::default()
        };
        graphics.specific_assigns.insert((0, 6), 1); // ROTSG_BRIDGE
        let mut def = RoadTypeDef {
            id: RoadType::from_u8(2),
            class: RoadTramType::Road,
            label: "Bridge only".into(),
            short_label: "BRDG".into(),
            intro_year: 0,
            max_speed: 0,
            cost_multiplier: 0,
            maintenance_multiplier: 0,
            flags: 0,
            powered_mask: 1 << 2,
            from_tramtypes_feature: false,
            from_newgrf: true,
            newgrf_preview: None,
            newgrf_views: Vec::new(),
            newgrf_local_id: 0,
            newgrf_runtime: Some(Box::new(graphics)),
            newgrf_grfid: 0,
            newgrf_type_tables: None,
        };
        assert!(def.has_newgrf_specific_group(6));
        let mut ctx = openttdrs_core::Action2EvalCtx::default();
        assert_eq!(
            def.newgrf_specific_view_runtime(6, 0, &mut ctx)
                .expect("bridge view")
                .rgba,
            sprite.rgba
        );

        let mut images = Assets::<Image>::default();
        let mut cache = NewGrfRoadSpriteCache::default();
        let first = cache
            .handle_for_specific_runtime(&def, 6, 0, &mut ctx, &mut images)
            .expect("bridge handle");
        let second = cache
            .handle_for_specific_runtime(&def, 6, 0, &mut ctx, &mut images)
            .expect("cached bridge handle");
        assert_eq!(first, second);
        // El selector es parte de la clave; un grupo distinto no debe
        // reutilizar accidentalmente la textura del puente.
        def.newgrf_runtime
            .as_mut()
            .expect("runtime")
            .specific_assigns
            .insert((0, 1), 0);
        let overlay = cache
            .handle_for_specific_runtime(&def, 1, 0, &mut ctx, &mut images)
            .expect("overlay handle");
        assert_ne!(first, overlay);
    }
}
