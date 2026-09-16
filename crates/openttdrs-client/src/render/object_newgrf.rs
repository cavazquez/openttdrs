//! Caché de sprites NewGRF para objetos de mapa (Action1/3 Objects).

use std::collections::HashMap;

use bevy::prelude::*;
use openttdrs_core::{
    DecodedSprite, OBJECT_FLAG_USES_2CC, ObjectSpecDef, TWOCC_ACTION5_SLOT_COUNT,
    TWOCC_PALETTE_BASE,
};

use crate::render::newgrf_cache::{
    DecodedSpriteImagePolicy, decoded_sprite_image, decoded_sprite_image_with_twocc_map,
    decoded_tile_layout_image_with_palette_and_twocc_map, runtime_fingerprint,
    twocc_map_for_palette, vars,
};
use crate::sprites::CompanyColour;

/// `(spec_id, slot, object_colour, runtime_fp, sprite_modifiers, direct_palette)`
/// → textura RGBA. El bit alto del slot separa piezas TileSeq de vistas planas
/// para no reutilizar una textura por error; `object_colour` conserva el
/// offset 2CC de la instancia.
#[derive(Resource, Default)]
pub(crate) struct NewGrfObjectSpriteCache {
    handles: HashMap<(u16, u16, u8, u32, u8, u16), Handle<Image>>,
    twocc_maps: Vec<Option<DecodedSprite>>,
}

/// Convierte `Object::colour` al palette id que recibiría
/// `DrawNewGRFTileSeq`. El byte de un objeto 2CC es
/// `colour1 + colour2 * 16`; los objetos de una sola rampa sólo conservan el
/// nibble bajo.
fn object_image_policy(def: &ObjectSpecDef, object_colour: u8) -> DecodedSpriteImagePolicy {
    if def.flags & OBJECT_FLAG_USES_2CC != 0 {
        DecodedSpriteImagePolicy::TwoCompany {
            primary: CompanyColour::from_u8(object_colour & 0x0F),
            secondary: CompanyColour::from_u8(object_colour >> 4),
        }
    } else {
        DecodedSpriteImagePolicy::CompanyPalette {
            colour: CompanyColour::from_u8(object_colour & 0x0F),
        }
    }
}

impl NewGrfObjectSpriteCache {
    pub(crate) fn clear(&mut self) {
        self.handles.clear();
    }

    /// Instala la tabla Action5 `0x0A` vigente para los objetos.
    ///
    /// Las texturas horneadas dependen del mapa además de la librea; si el
    /// runtime reemplaza los slots, descartar los handles evita conservar una
    /// imagen de una versión anterior del GRF.
    pub(crate) fn set_twocc_maps(&mut self, maps: &[Option<DecodedSprite>]) {
        if self.twocc_maps != maps {
            self.handles.clear();
            self.twocc_maps = maps.to_vec();
        }
    }

    fn twocc_map_for_palette(&self, palette_id: u16) -> Option<DecodedSprite> {
        twocc_map_for_palette(&self.twocc_maps, palette_id)
    }

    fn twocc_map_for(&self, def: &ObjectSpecDef, object_colour: u8) -> Option<DecodedSprite> {
        if def.flags & OBJECT_FLAG_USES_2CC == 0 {
            return None;
        }
        self.twocc_map_for_palette(TWOCC_PALETTE_BASE + u16::from(object_colour))
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
        // Keep the picker namespace apart from a runtime view with
        // `object_colour=0` and `runtime_fp=0`; both otherwise look like the
        // same cache entry even though the picker intentionally stays raw.
        let key = (def.id, 0x4000 | idx, 0, 0, 0, 0);
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
        let view = if def.newgrf_runtime.is_some() {
            def.newgrf_view_runtime(view_idx, ctx)?
        } else {
            def.view(view_idx)?.clone()
        };
        Some(self.handle_for_resolved_view(def, view_idx, ctx, &view, images))
    }

    /// Materializa una vista ya resuelta sin volver a ejecutar Action2.
    pub(crate) fn handle_for_resolved_view(
        &mut self,
        def: &ObjectSpecDef,
        view_idx: usize,
        ctx: &openttdrs_core::Action2EvalCtx,
        view: &openttdrs_core::DecodedSprite,
        images: &mut Assets<Image>,
    ) -> Handle<Image> {
        let fp = if def.newgrf_runtime.is_some() {
            runtime_fingerprint(ctx, vars::OBJECT, false)
        } else {
            0
        };
        // El runtime puede publicar sólo el resultado Action2. Mantener el
        // índice solicitado evita reutilizar la textura de otra orientación
        // cuando `def.views` está vacío.
        let idx = if def.newgrf_runtime.is_some() {
            u16::try_from(view_idx).unwrap_or(u16::MAX)
        } else {
            u16::try_from(view_idx % def.views.len().max(1)).unwrap_or(0)
        };
        let object_colour = ctx
            .vars
            .get(&0x47)
            .and_then(|value| u8::try_from(*value).ok())
            .unwrap_or_default();
        let key = (def.id, idx, object_colour, fp, 0, 0);
        let policy = object_image_policy(def, object_colour);
        let twocc_map = self.twocc_map_for(def, object_colour);
        self.handles
            .entry(key)
            .or_insert_with(|| {
                images.add(decoded_sprite_image_with_twocc_map(
                    view,
                    policy,
                    twocc_map.as_ref(),
                ))
            })
            .clone()
    }

    /// Materializa una pieza ya resuelta de un layout `TileSeq` de objeto.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn handle_for_layout(
        &mut self,
        def: &ObjectSpecDef,
        slot: u16,
        object_colour: u8,
        runtime_fp: u32,
        sprite_modifiers: u8,
        direct_palette: u16,
        sprite: &DecodedSprite,
        images: &mut Assets<Image>,
    ) -> Handle<Image> {
        let key = (
            def.id,
            0x8000 | (slot & 0x7FFF),
            object_colour,
            runtime_fp,
            sprite_modifiers,
            direct_palette,
        );
        let policy = object_image_policy(def, object_colour);
        let twocc_map = if (TWOCC_PALETTE_BASE
            ..TWOCC_PALETTE_BASE + TWOCC_ACTION5_SLOT_COUNT as u16)
            .contains(&direct_palette)
        {
            self.twocc_map_for_palette(direct_palette)
        } else {
            self.twocc_map_for(def, object_colour)
        };
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
            clear_cost_factor: openttdrs_core::DEFAULT_OBJECT_CLEAR_COST_FACTOR,
            flags: 0,
            animation_frames: 0,
            animation_status: 0xFF,
            animation_speed: 2,
            animation_triggers: 0,
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
    fn object_sprite_cache_bakes_instance_2cc_and_separates_liveries() {
        let sprite = DecodedSprite {
            width: 2,
            height: 1,
            x_offs: 0,
            y_offs: 0,
            rgba: vec![40, 40, 40, 255, 120, 120, 120, 255],
            // Author CC (C6) and secondary CC (50).
            mask: vec![0xC6, 0x50],
        };
        let def = ObjectSpecDef {
            id: 5,
            class_label: "2CC ".into(),
            name: "2CC object".into(),
            size: OBJECT_SIZE_1X1,
            from_newgrf: true,
            local_id: 0,
            grfid: 0,
            newgrf_grf_version: 0,
            climate_mask: openttdrs_core::DEFAULT_OBJECT_CLIMATE_MASK,
            build_cost_factor: openttdrs_core::DEFAULT_OBJECT_BUILD_COST_FACTOR,
            clear_cost_factor: openttdrs_core::DEFAULT_OBJECT_CLEAR_COST_FACTOR,
            flags: OBJECT_FLAG_USES_2CC,
            animation_frames: 0,
            animation_status: 0xFF,
            animation_speed: 2,
            animation_triggers: 0,
            callback_mask: 0,
            views: vec![sprite.clone()],
            newgrf_runtime: None,
            associated_badges: Vec::new(),
        };
        let mut images = Assets::<Image>::default();
        let mut cache = NewGrfObjectSpriteCache::default();

        let mut first = openttdrs_core::Action2EvalCtx::default();
        first.vars.insert(0x47, 4 + 9 * 16);
        let first_handle = cache
            .handle_for_runtime(&def, 0, &mut first, &mut images)
            .expect("2CC object view");
        let first_image = images.get(&first_handle).expect("2CC image");
        assert_eq!(
            first_image.data.as_deref(),
            Some(&openttdrs_core::bake_sprite_two_company_palette(&sprite, 4, 9)[..])
        );

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
        let object_colour: u8 = 4 + 9 * 16;
        let mut maps = vec![None; TWOCC_ACTION5_SLOT_COUNT];
        maps[usize::from(object_colour)] = Some(map.clone());
        let direct_palette = TWOCC_PALETTE_BASE + 2 + 3 * 16;
        maps[usize::from(direct_palette - TWOCC_PALETTE_BASE)] = Some(map.clone());
        cache.set_twocc_maps(&maps);
        let mapped_handle = cache
            .handle_for_runtime(&def, 0, &mut first, &mut images)
            .expect("mapped 2CC object view");
        let mapped_data = images
            .get(&mapped_handle)
            .expect("mapped 2CC image")
            .data
            .clone();
        assert_ne!(first_handle, mapped_handle);
        assert_eq!(
            mapped_data.as_deref(),
            Some(
                &openttdrs_core::bake_sprite_two_company_palette_with_map(
                    &sprite,
                    4,
                    9,
                    Some(&map),
                )[..]
            )
        );
        let layout_handle = cache.handle_for_layout(
            &def,
            0,
            object_colour,
            0,
            openttdrs_core::newgrf_sprites::TILE_LAYOUT_SPRITE_MODIFIER_RECOLOUR,
            0,
            &sprite,
            &mut images,
        );
        let layout_image = images.get(&layout_handle).expect("mapped TileSeq image");
        assert_eq!(layout_image.data, mapped_data);
        let raw_layout_handle =
            cache.handle_for_layout(&def, 0, object_colour, 0, 0, 0, &sprite, &mut images);
        assert_ne!(layout_handle, raw_layout_handle);
        assert_eq!(
            images
                .get(&raw_layout_handle)
                .expect("raw TileSeq image")
                .data
                .as_deref(),
            Some(&sprite.rgba[..])
        );
        let direct_layout_handle = cache.handle_for_layout(
            &def,
            0,
            object_colour,
            0,
            openttdrs_core::newgrf_sprites::TILE_LAYOUT_SPRITE_MODIFIER_RECOLOUR,
            direct_palette,
            &sprite,
            &mut images,
        );
        assert_eq!(
            images
                .get(&direct_layout_handle)
                .expect("direct 2CC TileSeq image")
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

        let mut second = openttdrs_core::Action2EvalCtx::default();
        second.vars.insert(0x47, 4 + 2 * 16);
        let second_handle = cache
            .handle_for_runtime(&def, 0, &mut second, &mut images)
            .expect("second 2CC object view");
        assert_ne!(first_handle, second_handle);
        assert_eq!(cache.handles.len(), 5);
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
            clear_cost_factor: openttdrs_core::DEFAULT_OBJECT_CLEAR_COST_FACTOR,
            flags: 0,
            animation_frames: 0,
            animation_status: 0xFF,
            animation_speed: 2,
            animation_triggers: 0,
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

    #[test]
    fn object_runtime_only_cache_keeps_view_index() {
        use openttdrs_core::{DecodedSprite, TrainSpriteAssign, TrainSpriteGraphics};

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
        let def = ObjectSpecDef {
            id: 6,
            class_label: "RTO ".into(),
            name: "runtime-only object".into(),
            size: OBJECT_SIZE_1X1,
            from_newgrf: true,
            local_id: 0,
            grfid: 0,
            newgrf_grf_version: 0,
            climate_mask: openttdrs_core::DEFAULT_OBJECT_CLIMATE_MASK,
            build_cost_factor: openttdrs_core::DEFAULT_OBJECT_BUILD_COST_FACTOR,
            clear_cost_factor: openttdrs_core::DEFAULT_OBJECT_CLEAR_COST_FACTOR,
            flags: 0,
            animation_frames: 0,
            animation_status: 0xFF,
            animation_speed: 2,
            animation_triggers: 0,
            callback_mask: 0,
            views: Vec::new(),
            newgrf_runtime: Some(Box::new(TrainSpriteGraphics {
                sets: vec![vec![red.clone(), blue.clone()]],
                assigns: vec![TrainSpriteAssign {
                    local_id: 0,
                    set_id: 0,
                }],
                ..Default::default()
            })),
            associated_badges: Vec::new(),
        };
        let mut images = Assets::<Image>::default();
        let mut cache = NewGrfObjectSpriteCache::default();
        let mut first_ctx = openttdrs_core::Action2EvalCtx::default();
        let first = cache
            .handle_for_runtime(&def, 0, &mut first_ctx, &mut images)
            .expect("object view 0");
        let mut second_ctx = openttdrs_core::Action2EvalCtx::default();
        let second = cache
            .handle_for_runtime(&def, 1, &mut second_ctx, &mut images)
            .expect("object view 1");
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

    #[test]
    fn object_runtime_cache_separates_previous_action2_result() {
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
                local_id: 0,
                set_id: 7,
            }],
            ..Default::default()
        };
        runtime.action2_var.insert(
            7,
            Action2VarEntry {
                first: Action2VarTerm {
                    variable: 0x1C,
                    param: None,
                    adjust: Action2VarAdjust {
                        and_mask: u32::MAX,
                        ..Default::default()
                    },
                },
                ops: Vec::new(),
                ranges: vec![(0, 0, 0), (1, 1, 1)],
                default: 0,
            },
        );
        let def = ObjectSpecDef {
            id: 7,
            class_label: "RSLT".into(),
            name: "previous Action2 result".into(),
            size: OBJECT_SIZE_1X1,
            from_newgrf: true,
            local_id: 0,
            grfid: 0,
            newgrf_grf_version: 0,
            climate_mask: openttdrs_core::DEFAULT_OBJECT_CLIMATE_MASK,
            build_cost_factor: openttdrs_core::DEFAULT_OBJECT_BUILD_COST_FACTOR,
            clear_cost_factor: openttdrs_core::DEFAULT_OBJECT_CLEAR_COST_FACTOR,
            flags: 0,
            animation_frames: 0,
            animation_status: 0xFF,
            animation_speed: 2,
            animation_triggers: 0,
            callback_mask: 0,
            views: Vec::new(),
            newgrf_runtime: Some(Box::new(runtime)),
            associated_badges: Vec::new(),
        };
        let mut images = Assets::<Image>::default();
        let mut cache = NewGrfObjectSpriteCache::default();
        let mut first_ctx = openttdrs_core::Action2EvalCtx::default();
        let first = cache
            .handle_for_runtime(&def, 0, &mut first_ctx, &mut images)
            .expect("result 0");
        let mut second_ctx = openttdrs_core::Action2EvalCtx {
            last_result: 1,
            ..Default::default()
        };
        let second = cache
            .handle_for_runtime(&def, 0, &mut second_ctx, &mut images)
            .expect("result 1");
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
