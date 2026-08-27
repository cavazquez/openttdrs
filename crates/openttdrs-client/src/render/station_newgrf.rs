//! Caché de sprites NewGRF para estaciones rail in-world (Action1/3 Stations).

use std::collections::HashMap;

use bevy::prelude::*;
use openttdrs_core::prelude::*;
use openttdrs_core::{StationSpecDef, StationSpecId};

use crate::render::newgrf_cache::{
    DecodedSpriteImagePolicy, decoded_sprite_image, runtime_fingerprint, vars,
};
use crate::sprites::CompanyColour;

/// `(station_spec_id, slot, company_colour, runtime_fp)` → textura RGBA.
///
/// Los slots de layouts usan el bit alto, separado de los índices de vista,
/// para que un TileSeq y una vista plana nunca compartan accidentalmente una
/// textura aunque ambos pertenezcan al mismo spec.
#[derive(Resource, Default)]
pub(crate) struct NewGrfStationSpriteCache {
    handles: HashMap<(u16, u16, u8, u32), Handle<Image>>,
}

impl NewGrfStationSpriteCache {
    pub(crate) fn clear(&mut self) {
        self.handles.clear();
    }

    /// Textura re-resolviendo Action2 con vars de tesela.
    pub(crate) fn handle_for_runtime(
        &mut self,
        def: &StationSpecDef,
        view_idx: usize,
        colour: Option<CompanyColour>,
        ctx: &mut openttdrs_core::Action2EvalCtx,
        images: &mut Assets<Image>,
    ) -> Option<Handle<Image>> {
        let colour_key = colour.map(CompanyColour::as_u8).unwrap_or(0xFF);
        let fp = if def.newgrf_runtime.is_some() {
            runtime_fingerprint(ctx, vars::STATION, false)
        } else {
            0
        };
        let view = if def.newgrf_runtime.is_some() {
            def.newgrf_view_runtime(view_idx, ctx)?
        } else {
            def.newgrf_view(view_idx)?.clone()
        };
        let idx = u16::try_from(view_idx % def.newgrf_views.len().max(1)).unwrap_or(0);
        let key = (def.id.as_u16(), idx, colour_key, fp);
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

    /// Materializa una pieza ya resuelta de un layout `TileSeq`.
    pub(crate) fn handle_for_layout(
        &mut self,
        def: &StationSpecDef,
        slot: u16,
        colour: Option<CompanyColour>,
        runtime_fp: u32,
        sprite: &openttdrs_core::DecodedSprite,
        images: &mut Assets<Image>,
    ) -> Handle<Image> {
        let colour_key = colour.map(CompanyColour::as_u8).unwrap_or(0xFF);
        let key = (
            def.id.as_u16(),
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
