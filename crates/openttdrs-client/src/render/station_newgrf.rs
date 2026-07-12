//! Caché de sprites NewGRF para estaciones rail in-world (Action1/3 Stations).

use std::collections::HashMap;

use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use openttdrs_core::{DecodedSprite, Map, Station, StationSpecDef, StationSpecId, TileCoord};

use crate::sprites::CompanyColour;
use crate::sprites::company_palette::recolor_rgba8;

/// `(station_spec_id, view_idx, company_colour)` → textura RGBA.
#[derive(Resource, Default)]
pub(crate) struct NewGrfStationSpriteCache {
    handles: HashMap<(u16, u8, u8), Handle<Image>>,
}

impl NewGrfStationSpriteCache {
    pub(crate) fn clear(&mut self) {
        self.handles.clear();
    }

    fn decoded_to_image(sprite: &DecodedSprite, colour: Option<CompanyColour>) -> Image {
        let mut rgba = if sprite.mask.is_empty() {
            sprite.rgba.clone()
        } else {
            let c = colour.map(CompanyColour::as_u8).unwrap_or(0);
            openttdrs_core::bake_sprite_company_mask(sprite, c)
        };
        if let Some(c) = colour {
            recolor_rgba8(&mut rgba, c);
        }
        Image::new(
            Extent3d {
                width: u32::from(sprite.width),
                height: u32::from(sprite.height),
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            rgba,
            TextureFormat::Rgba8UnormSrgb,
            default(),
        )
    }

    /// Textura de la vista `view_idx` (MVP: 0), opcionalmente recoloreada.
    pub(crate) fn handle_for(
        &mut self,
        def: &StationSpecDef,
        view_idx: usize,
        colour: Option<CompanyColour>,
        images: &mut Assets<Image>,
    ) -> Option<Handle<Image>> {
        let colour_key = colour.map(CompanyColour::as_u8).unwrap_or(0xFF);
        if def.newgrf_runtime.is_some() {
            let mut ctx = openttdrs_core::Action2EvalCtx::default();
            let view = def.newgrf_view_runtime(view_idx, &mut ctx)?;
            let idx = u8::try_from(view_idx).unwrap_or(0);
            let key = (def.id.as_u16(), idx, colour_key);
            return Some(
                self.handles
                    .entry(key)
                    .or_insert_with(|| images.add(Self::decoded_to_image(&view, colour)))
                    .clone(),
            );
        }
        let view = def.newgrf_view(view_idx)?;
        let idx = u8::try_from(view_idx % def.newgrf_views.len().max(1)).unwrap_or(0);
        let key = (def.id.as_u16(), idx, colour_key);
        Some(
            self.handles
                .entry(key)
                .or_insert_with(|| images.add(Self::decoded_to_image(view, colour)))
                .clone(),
        )
    }
}

/// Spec NewGRF con vista 0 para la estación/waypoint que cubre `coord`.
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
    if def.newgrf_view(0).is_some() {
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
        GameState, apply_newgrf_stations, build_action0_station_payload,
        build_grf_v2_station_with_preview_sprite,
    };

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
        let handle = cache.handle_for(def, 0, None, &mut images).expect("handle");
        assert!(images.get(&handle).is_some());
        let again = cache.handle_for(def, 0, None, &mut images).expect("cached");
        assert_eq!(handle, again);
        let recolored = cache
            .handle_for(def, 0, Some(CompanyColour::Red), &mut images)
            .expect("recolor");
        assert_ne!(handle, recolored);
    }
}
