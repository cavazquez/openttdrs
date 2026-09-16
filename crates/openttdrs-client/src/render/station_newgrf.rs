//! Caché de sprites NewGRF para estaciones rail in-world (Action1/3 Stations).

use std::collections::HashMap;

use bevy::prelude::*;
use openttdrs_core::prelude::*;
use openttdrs_core::{StationSpecDef, StationSpecId};

use crate::render::newgrf_cache::{
    DecodedSpriteImagePolicy, decoded_sprite_image,
    decoded_tile_layout_image_with_palette_and_twocc_map, runtime_fingerprint,
    twocc_map_for_palette, vars,
};
use crate::sprites::CompanyColour;

/// `(station_spec_id, slot, company_colour, runtime_fp, sprite_modifiers,
/// direct_palette)` → textura RGBA.
///
/// Los slots de layouts usan el bit alto, separado de los índices de vista,
/// para que un TileSeq y una vista plana nunca compartan accidentalmente una
/// textura aunque ambos pertenezcan al mismo spec.
#[derive(Resource, Default)]
pub(crate) struct NewGrfStationSpriteCache {
    handles: HashMap<(u16, u16, u8, u32, u8, u16), Handle<Image>>,
    twocc_maps: Vec<Option<openttdrs_core::DecodedSprite>>,
}

impl NewGrfStationSpriteCache {
    pub(crate) fn clear(&mut self) {
        self.handles.clear();
    }

    /// Instala la tabla Action5 `0x0A` vigente para los TileLayout de
    /// estaciones. Las texturas ya horneadas dependen de ella.
    pub(crate) fn set_twocc_maps(&mut self, maps: &[Option<openttdrs_core::DecodedSprite>]) {
        if self.twocc_maps != maps {
            self.handles.clear();
            self.twocc_maps = maps.to_vec();
        }
    }

    /// Textura re-resolviendo Action2 con vars de tesela.
    #[cfg(test)]
    pub(crate) fn handle_for_runtime(
        &mut self,
        def: &StationSpecDef,
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
    ///
    /// El fingerprint se toma después de la resolución: `var 1C`, `STO` y
    /// procedimientos pueden dejar estado que la siguiente evaluación
    /// consulta y que también distingue la textura resultante.
    pub(crate) fn handle_for_resolved_view(
        &mut self,
        def: &StationSpecDef,
        view_idx: usize,
        colour: Option<CompanyColour>,
        ctx: &openttdrs_core::Action2EvalCtx,
        view: &openttdrs_core::DecodedSprite,
        images: &mut Assets<Image>,
    ) -> Handle<Image> {
        let colour_key = colour.map(CompanyColour::as_u8).unwrap_or(0xFF);
        let fp = if def.newgrf_runtime.is_some() {
            runtime_fingerprint(ctx, vars::STATION, false)
        } else {
            0
        };
        // Un runtime NewGRF puede no publicar vistas estáticas. En ese caso
        // `newgrf_views.len() == 0` no debe convertir todas las orientaciones
        // en la misma entrada de caché: el índice solicitado sigue siendo
        // parte de la identidad de la vista materializada.
        let idx = if def.newgrf_runtime.is_some() {
            u16::try_from(view_idx).unwrap_or(u16::MAX)
        } else {
            u16::try_from(view_idx % def.newgrf_views.len().max(1)).unwrap_or(0)
        };
        let key = (def.id.as_u16(), idx, colour_key, fp, 0, 0);
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

    /// Materializa una pieza ya resuelta de un layout `TileSeq`.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn handle_for_layout(
        &mut self,
        def: &StationSpecDef,
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
            def.id.as_u16(),
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

/// Índice de vista de estación tras aplicar CB14 si el spec lo declaró.
///
/// El contexto se recibe separado del resolver de sprites: CB14 instala sus
/// parámetros genéricos (`0x0C`/`0x10`/`0x18`) y no debe borrar las vars reales
/// de tesela que Action2 usa inmediatamente después para elegir el gráfico.
#[must_use]
pub(crate) fn station_newgrf_view_index_for_tile(
    def: &StationSpecDef,
    m5: u8,
    ctx: &mut openttdrs_core::Action2EvalCtx,
) -> usize {
    let layout = openttdrs_core::apply_station_draw_tile_layout_callback(def, m5, m5 & 1 != 0, ctx);
    openttdrs_core::station_newgrf_view_index(layout)
}

/// Resuelve la vista plana que `DrawNewStationTile` usaría para una
/// orientación. Los specs runtime-only no tienen una copia en
/// `newgrf_views`, por lo que deben pasar por el grafo Action2 igual que los
/// layouts y conservar la vista elegida por el contexto actual.
#[must_use]
pub(crate) fn station_newgrf_view_for_tile(
    def: &StationSpecDef,
    view_idx: usize,
    ctx: &mut openttdrs_core::Action2EvalCtx,
) -> Option<openttdrs_core::DecodedSprite> {
    if def.newgrf_runtime.is_some() {
        def.newgrf_view_runtime(view_idx, ctx)
    } else {
        def.newgrf_view(view_idx).cloned()
    }
}

/// Spec NewGRF con vistas Action1/3 para la estación/waypoint que cubre `coord`.
#[must_use]
pub(crate) fn newgrf_station_def_for_tile<'a>(
    catalog: &'a [StationSpecDef],
    map: &Map,
    stations: &[Station],
    coord: TileCoord,
) -> Option<&'a StationSpecDef> {
    let st = openttdrs_core::station_at_tile(map, stations, coord)?;
    if st.station_spec == StationSpecId::DEFAULT_RAIL {
        return None;
    }
    let def = openttdrs_core::station_spec_def(catalog, st.station_spec)?;
    if !def.newgrf_views.is_empty() || def.newgrf_preview.is_some() || def.newgrf_runtime.is_some()
    {
        Some(def)
    } else {
        None
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use openttdrs_core::apply_newgrf_stations;
    use openttdrs_core::newgrf_actions::build_action0_station_payload;
    use openttdrs_core::newgrf_sprites::build_grf_v2_station_with_preview_sprite;
    use openttdrs_core::prelude::GameState;

    fn two_cc_layout_fixture() -> (
        openttdrs_core::DecodedSprite,
        openttdrs_core::DecodedSprite,
        u16,
    ) {
        let sprite = openttdrs_core::DecodedSprite {
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
        let map = openttdrs_core::DecodedSprite {
            width: 256,
            height: 1,
            x_offs: 0,
            y_offs: 0,
            rgba: openttdrs_core::newgrf_sprites::indices_to_rgba(&indices, 256, 1).unwrap(),
            mask: Vec::new(),
        };
        let palette = openttdrs_core::TWOCC_PALETTE_BASE + 2 + 3 * 16;
        (sprite, map, palette)
    }

    #[test]
    fn station_sprite_cache_builds_handle_from_catalog_views() {
        let a0 = build_action0_station_payload(b"MODN", b"Plat", 0, 0, "Andén moderno");
        let mut indices = vec![0u8; 8 * 8];
        for y in 2..6 {
            for x in 2..6 {
                indices[y * 8 + x] = 174;
            }
        }
        let bytes = build_grf_v2_station_with_preview_sprite(
            &a0,
            0,
            8,
            8,
            &indices,
            [b'S', b'W', 0, 1],
            "sworld",
        );
        let dir = tempfile::tempdir().expect("tmp");
        std::fs::write(dir.path().join("sworld.grf"), &bytes).expect("write");
        let mut state = GameState::new(4, 4);
        state
            .newgrf_stack
            .push(openttdrs_core::NewGrfEntry::new("sworld.grf", 2));
        apply_newgrf_stations(&mut state, &[dir.path()]);
        let def = state
            .station_spec_catalog
            .iter()
            .find(|d| d.from_newgrf)
            .expect("newgrf station");
        let mut images = Assets::<Image>::default();
        let mut cache = NewGrfStationSpriteCache::default();
        let mut ctx = openttdrs_core::Action2EvalCtx::default();
        let handle = cache
            .handle_for_runtime(def, 0, None, &mut ctx, &mut images)
            .expect("handle");
        assert!(images.get(&handle).is_some());
        let again = cache
            .handle_for_runtime(def, 0, None, &mut ctx, &mut images)
            .expect("cached");
        assert_eq!(handle, again);
        let recolored = cache
            .handle_for_runtime(def, 0, Some(CompanyColour::Red), &mut ctx, &mut images)
            .expect("recolor");
        assert_ne!(handle, recolored);

        let (sprite, map, direct_palette) = two_cc_layout_fixture();
        let mut maps = vec![None; openttdrs_core::TWOCC_ACTION5_SLOT_COUNT];
        maps[usize::from(direct_palette - openttdrs_core::TWOCC_PALETTE_BASE)] = Some(map.clone());
        cache.set_twocc_maps(&maps);
        let layout_handle = cache.handle_for_layout(
            def,
            0,
            None,
            0,
            openttdrs_core::newgrf_sprites::TILE_LAYOUT_SPRITE_MODIFIER_RECOLOUR,
            direct_palette,
            &sprite,
            &mut images,
        );
        assert_eq!(
            images
                .get(&layout_handle)
                .expect("station 2CC TileLayout")
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
    fn cache_keys_differ_by_view_index_from_m5_tiletype() {
        use openttdrs_core::{DecodedSprite, station_newgrf_view_index};

        fn solid(r: u8, g: u8, b: u8) -> DecodedSprite {
            DecodedSprite {
                width: 2,
                height: 2,
                x_offs: 0,
                y_offs: 0,
                rgba: vec![r, g, b, 255, r, g, b, 255, r, g, b, 255, r, g, b, 255],
                mask: Vec::new(),
            }
        }

        let def = StationSpecDef {
            id: StationSpecId::from_u16(9),
            class: openttdrs_core::StationClassId::from_u16(1),
            label: "Multi".into(),
            short_label: "MULT".into(),
            disallowed_platforms: 0,
            disallowed_lengths: 0,
            callback_mask: 0,
            flags: 0,
            animation_status: 0xFF,
            animation_frames: 0,
            animation_speed: 2,
            animation_triggers: 0,
            from_newgrf: true,
            newgrf_preview: None,
            newgrf_views: vec![solid(255, 0, 0), solid(0, 255, 0), solid(0, 0, 255)],
            newgrf_local_id: 0,
            newgrf_runtime: None,
            newgrf_grfid: 0,
            newgrf_grf_version: 0,
            newgrf_type_tables: None,
            associated_badges: Vec::new(),
            newgrf_badge_translation: Vec::new(),
            custom_layouts: Default::default(),
        };
        let mut images = Assets::<Image>::default();
        let mut cache = NewGrfStationSpriteCache::default();
        let mut ctx = openttdrs_core::Action2EvalCtx::default();
        let idx0 = station_newgrf_view_index(0x00);
        let idx2 = station_newgrf_view_index(0x02);
        let h0 = cache
            .handle_for_runtime(&def, idx0, None, &mut ctx, &mut images)
            .expect("v0");
        let h2 = cache
            .handle_for_runtime(&def, idx2, None, &mut ctx, &mut images)
            .expect("v2");
        assert_ne!(h0, h2);
    }

    #[test]
    fn runtime_only_station_view_and_cache_keep_directional_sprite() {
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
        let def = StationSpecDef {
            id: StationSpecId::from_u16(11),
            class: openttdrs_core::StationClassId::from_u16(1),
            label: "Runtime only".into(),
            short_label: "RTO".into(),
            disallowed_platforms: 0,
            disallowed_lengths: 0,
            callback_mask: 0,
            flags: 0,
            animation_status: 0xFF,
            animation_frames: 0,
            animation_speed: 2,
            animation_triggers: 0,
            from_newgrf: true,
            newgrf_preview: None,
            newgrf_views: Vec::new(),
            newgrf_local_id: 0,
            newgrf_runtime: Some(Box::new(TrainSpriteGraphics {
                sets: vec![vec![red.clone(), blue.clone()]],
                assigns: vec![TrainSpriteAssign {
                    local_id: 0,
                    set_id: 0,
                }],
                ..Default::default()
            })),
            newgrf_grfid: 0,
            newgrf_grf_version: 0,
            newgrf_type_tables: None,
            associated_badges: Vec::new(),
            newgrf_badge_translation: Vec::new(),
            custom_layouts: Default::default(),
        };
        let mut images = Assets::<Image>::default();
        let mut cache = NewGrfStationSpriteCache::default();
        let mut view_ctx = openttdrs_core::Action2EvalCtx::default();
        assert_eq!(
            station_newgrf_view_for_tile(&def, 1, &mut view_ctx)
                .expect("runtime-only station view")
                .rgba,
            blue.rgba
        );

        let mut first_ctx = openttdrs_core::Action2EvalCtx::default();
        let first = cache
            .handle_for_runtime(&def, 0, None, &mut first_ctx, &mut images)
            .expect("direction 0");
        let mut second_ctx = openttdrs_core::Action2EvalCtx::default();
        let second = cache
            .handle_for_runtime(&def, 1, None, &mut second_ctx, &mut images)
            .expect("direction 1");
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
    fn draw_layout_callback_changes_the_rendered_view_index() {
        use openttdrs_core::{
            Action2VarAdjust, Action2VarEntry, Action2VarTerm, TrainSpriteAssign,
            TrainSpriteGraphics,
        };

        let mut gfx = TrainSpriteGraphics::default();
        gfx.assigns.push(TrainSpriteAssign {
            local_id: 0,
            set_id: 3,
        });
        gfx.action2_var.insert(
            3,
            Action2VarEntry {
                first: Action2VarTerm {
                    variable: 0x1A,
                    param: None,
                    adjust: Action2VarAdjust {
                        shift: 0,
                        and_mask: 6,
                        ..Action2VarAdjust::default()
                    },
                },
                ops: Vec::new(),
                ranges: Vec::new(),
                default: 0,
            },
        );
        let def = StationSpecDef {
            id: StationSpecId::from_u16(9),
            class: openttdrs_core::StationClassId::from_u16(1),
            label: "CB14".into(),
            short_label: "CB14".into(),
            disallowed_platforms: 0,
            disallowed_lengths: 0,
            callback_mask: 1 << 1,
            flags: 0,
            animation_status: 0xFF,
            animation_frames: 0,
            animation_speed: 2,
            animation_triggers: 0,
            from_newgrf: true,
            newgrf_preview: None,
            newgrf_views: Vec::new(),
            newgrf_local_id: 0,
            newgrf_runtime: Some(Box::new(gfx)),
            newgrf_grfid: 0,
            newgrf_grf_version: 0,
            newgrf_type_tables: None,
            associated_badges: Vec::new(),
            newgrf_badge_translation: Vec::new(),
            custom_layouts: Default::default(),
        };
        let mut ctx = openttdrs_core::Action2EvalCtx::default();
        assert_eq!(
            station_newgrf_view_index_for_tile(&def, 3, &mut ctx),
            7,
            "CB14=6 conserva eje Y y el renderer usa la vista 7"
        );
    }

    #[test]
    fn station_sprite_cache_rekeys_when_animation_frame_changes() {
        use openttdrs_core::{
            Action2VarAdjust, Action2VarEntry, Action2VarTerm, DecodedSprite, TrainSpriteAssign,
            TrainSpriteGraphics,
        };

        fn solid(r: u8, g: u8, b: u8) -> DecodedSprite {
            DecodedSprite {
                width: 2,
                height: 2,
                x_offs: 0,
                y_offs: 0,
                rgba: vec![r, g, b, 255, r, g, b, 255, r, g, b, 255, r, g, b, 255],
                mask: Vec::new(),
            }
        }

        let first = solid(255, 0, 0);
        let second = solid(0, 0, 255);
        let mut gfx = TrainSpriteGraphics {
            sets: vec![vec![first.clone()], vec![second.clone()]],
            assigns: vec![TrainSpriteAssign {
                local_id: 0,
                set_id: 2,
            }],
            ..TrainSpriteGraphics::default()
        };
        gfx.action2_var.insert(
            2,
            Action2VarEntry {
                first: Action2VarTerm {
                    variable: 0x4A,
                    param: None,
                    adjust: Action2VarAdjust {
                        shift: 0,
                        and_mask: 0xFF,
                        ..Action2VarAdjust::default()
                    },
                },
                ops: Vec::new(),
                ranges: vec![(1, 1, 1)],
                default: 0,
            },
        );
        let def = StationSpecDef {
            id: StationSpecId::from_u16(10),
            class: openttdrs_core::StationClassId::from_u16(1),
            label: "Animada".into(),
            short_label: "ANIM".into(),
            disallowed_platforms: 0,
            disallowed_lengths: 0,
            callback_mask: 0,
            flags: 0,
            animation_status: 1,
            animation_frames: 1,
            animation_speed: 0,
            animation_triggers: 0,
            from_newgrf: true,
            newgrf_preview: Some(first.clone()),
            newgrf_views: vec![first, second],
            newgrf_local_id: 0,
            newgrf_runtime: Some(Box::new(gfx)),
            newgrf_grfid: 0,
            newgrf_grf_version: 0,
            newgrf_type_tables: None,
            associated_badges: Vec::new(),
            newgrf_badge_translation: Vec::new(),
            custom_layouts: Default::default(),
        };
        let mut images = Assets::<Image>::default();
        let mut cache = NewGrfStationSpriteCache::default();
        let mut ctx = openttdrs_core::Action2EvalCtx::default();
        ctx.vars.insert(0x4A, 0);
        let frame_zero = cache
            .handle_for_runtime(&def, 0, None, &mut ctx, &mut images)
            .expect("frame 0");
        ctx.vars.insert(0x4A, 1);
        let frame_one = cache
            .handle_for_runtime(&def, 0, None, &mut ctx, &mut images)
            .expect("frame 1");

        assert_ne!(
            frame_zero, frame_one,
            "MAP7 debe invalidar la textura cacheada"
        );
    }
}
