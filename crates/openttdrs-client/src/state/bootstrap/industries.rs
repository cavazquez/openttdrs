//! Industrias del mapa procedural y adaptador mínimo para `.ottdmap`.
//!
//! La importación de `.sav` vive en `openttdrs-core::sav`: el cliente no debe
//! volver a interpretar `INDY` ni `CAPA`, porque herramientas y servidor tienen
//! que observar exactamente el mismo estado económico.

use openttdrs_core::prelude::*;
use openttdrs_core::sav::{
    industry_kind_from_gfx, industry_random_colour_from_instance, industry_spec_from_gfx,
};
use openttdrs_core::{Industry, IndustryKind, OttdmapExtras, get_clean_industry_gfx};

pub(crate) use openttdrs_core::sav::industry_group_from_gfx;

/// Población de industrias para mapas creados localmente o `.ottdmap` sin
/// `INDY`. El fallback de componentes se ejecuta en core para no divergir de
/// la conversión de saves.
pub(crate) fn place_industries(
    state: &mut GameState,
    from_ottd_file: bool,
    ottd_extras: Option<&OttdmapExtras>,
) {
    if from_ottd_file {
        openttdrs_core::sav::hydrate_industries_from_map_tiles(state, ottd_extras);
        return;
    }

    let (map_w, map_h) = state.map.dimensions();
    let mut coal_count = 0_u32;
    let mut forest_count = 0_u32;
    let mut industry_count = 0_u32;
    const PROCEDURAL_STRIDE: u32 = 4;

    for y in 0..map_h {
        for x in 0..map_w {
            let coord = TileCoord::new(x as i32, y as i32);
            let Some(tile) = state.map.get(coord) else {
                continue;
            };
            match tile.kind {
                TileKind::CoalField if coal_count.is_multiple_of(PROCEDURAL_STRIDE) => {
                    state
                        .industries
                        .push(Industry::new(coord, IndustryKind::CoalMine));
                    coal_count += 1;
                }
                TileKind::CoalField => coal_count += 1,
                TileKind::Forest if forest_count.is_multiple_of(PROCEDURAL_STRIDE) => {
                    state
                        .industries
                        .push(Industry::new(coord, IndustryKind::Forest));
                    forest_count += 1;
                }
                TileKind::Forest => forest_count += 1,
                TileKind::Industry if industry_count.is_multiple_of(16) => {
                    let gfx = get_clean_industry_gfx(tile.m5, tile.m6);
                    let kind = industry_kind_from_gfx(gfx);
                    let instance_id = openttdrs_core::industry_instance_id(&tile);
                    let industry = if let Some(spec) = industry_spec_from_gfx(gfx) {
                        Industry::with_tiles_spec(
                            coord,
                            kind,
                            spec,
                            vec![coord],
                            industry_random_colour_from_instance(instance_id),
                        )
                    } else {
                        Industry::new(coord, kind)
                            .with_random_colour(industry_random_colour_from_instance(instance_id))
                    }
                    .with_instance_id(instance_id);
                    state.industries.push(industry);
                    industry_count += 1;
                }
                TileKind::Industry => industry_count += 1,
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn group_labels_come_from_core_catalog() {
        assert_eq!(industry_group_from_gfx(40), "Factory");
        assert_eq!(industry_group_from_gfx(142), "Toy Factory");
        assert_eq!(industry_group_from_gfx(255), "Unknown gfx");
    }

    #[test]
    #[allow(clippy::expect_used)] // fixture fijo: una tesela ausente es un bug del test
    fn otdmap_fallback_groups_adjacent_same_industry() {
        let mut state = GameState::new(2, 1);
        for x in 0..2 {
            let coord = TileCoord::new(x, 0);
            let mut tile = state.map.get(coord).expect("fixture tile");
            tile.kind = TileKind::Industry;
            tile.m1 = 0x80;
            tile.m2 = 10;
            tile.m5 = x as u8;
            state.map.set_tile(coord, tile).expect("set fixture tile");
        }

        place_industries(&mut state, true, None);

        assert_eq!(state.industries.len(), 1);
        assert_eq!(state.industries[0].tiles.len(), 2);
    }

    #[test]
    #[allow(clippy::expect_used)] // fixture fijo: una tesela ausente es un bug del test
    fn procedural_population_keeps_sparse_fields() {
        let mut state = GameState::new(8, 1);
        for x in 0..8 {
            state
                .map
                .set_kind(TileCoord::new(x, 0), TileKind::CoalField)
                .expect("set field");
        }

        place_industries(&mut state, false, None);

        assert_eq!(state.industries.len(), 2);
        assert!(
            state
                .industries
                .iter()
                .all(|industry| industry.kind == IndustryKind::CoalMine)
        );
    }
}
