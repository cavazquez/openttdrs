//! Utilidades de proyección isométrica.
#![allow(clippy::unwrap_used)] // tests de `compute_tileh` usan mapas mínimos fijos

mod coords;
mod slope;
mod util;
mod water;

#[allow(unused_imports)]
pub use coords::{
    gizmo_diamond, iso, overlay_pos, tile_pos, tile_pos_half, world_pos_to_tile_coord,
    world_to_tile,
};
#[allow(unused_imports)]
pub use slope::{
    SLOPE_HALF_H, compute_tileh, slope_label, tile_min_corner_height, tile_min_z,
    tile_slope_and_min_z, tile_slope_bits_from_heights,
};
pub use util::wang_hash;
#[allow(unused_imports)]
pub use water::{
    infer_coast_tileh_when_flat, shore_png_index, shore_sprite_half_h, shore_tileh_for_draw_shore,
};

#[cfg(test)]
use openttdrs_core::{Map, TileCoord, TileKind};
#[cfg(test)]
use slope::water_void_effective_height_for_slope;

/// Desplazamiento horizontal por tesela en pantalla (la tesela mide 64 px de ancho).
pub const ISO_HW: f32 = 32.0;
/// Desplazamiento vertical por tesela en pantalla (ratio 2:1 isométrico).
pub const ISO_QH: f32 = 16.0;
/// La mitad de la altura de los sprites de tesela (64×31 → 15.5 px).
pub const TILE_HALF_H: f32 = 15.5;
/// Píxeles de elevación en Y por cada unidad de altura de `OpenTTD`.
pub const HEIGHT_PX: f32 = 8.0;

#[cfg(test)]
mod compute_tileh_tests {
    //! Regresión: `compute_tileh` debe coincidir con `GetTileSlopeZ` / `GetTileSlopeGivenHeight`
    //! (`tile_map.cpp` de OpenTTD): hnorth@(tx,ty), hwest@(tx+1,ty), heast@(tx,ty+1), hsouth@(tx+1,ty+1).

    use super::compute_tileh;
    use openttdrs_core::{Map, TileCoord};

    fn set_h(map: &mut Map, x: i32, y: i32, h: u8) {
        map.set_height(TileCoord::new(x, y), h).unwrap();
    }

    #[test]
    fn flat_2x2_all_zero() {
        let m = Map::new_flat(2, 2, 0);
        assert_eq!(compute_tileh(&m, 0, 0), 0);
        assert_eq!(compute_tileh(&m, 1, 0), 0);
        assert_eq!(compute_tileh(&m, 0, 1), 0);
        assert_eq!(compute_tileh(&m, 1, 1), 0);
    }

    #[test]
    fn only_hnorth_raised_tile_00() {
        let mut m = Map::new_flat(2, 2, 0);
        set_h(&mut m, 0, 0, 1);
        assert_eq!(compute_tileh(&m, 0, 0), 8); // SLOPE_N
    }

    #[test]
    fn only_hwest_raised_tile_00() {
        let mut m = Map::new_flat(2, 2, 0);
        set_h(&mut m, 1, 0, 1);
        assert_eq!(compute_tileh(&m, 0, 0), 1); // SLOPE_W
    }

    #[test]
    fn only_heast_raised_tile_00() {
        let mut m = Map::new_flat(2, 2, 0);
        set_h(&mut m, 0, 1, 1);
        assert_eq!(compute_tileh(&m, 0, 0), 4); // SLOPE_E
    }

    #[test]
    fn only_hsouth_raised_tile_00() {
        let mut m = Map::new_flat(2, 2, 0);
        set_h(&mut m, 1, 1, 1);
        assert_eq!(compute_tileh(&m, 0, 0), 2); // SLOPE_S
    }

    #[test]
    fn hwest_and_hsouth_sw_slope() {
        let mut m = Map::new_flat(2, 2, 0);
        set_h(&mut m, 1, 0, 1);
        set_h(&mut m, 1, 1, 1);
        assert_eq!(compute_tileh(&m, 0, 0), 3); // SLOPE_SW
    }

    #[test]
    fn map_edge_1x1_void_corners_read_as_zero() {
        let mut m = Map::new_flat(1, 1, 0);
        set_h(&mut m, 0, 0, 2);
        // Fuera del mapa → altura 0; solo hnorth=2 > min(0,0,0,0)
        assert_eq!(compute_tileh(&m, 0, 0), 8);
    }

    #[test]
    fn thin_map_2x1_row() {
        let mut m = Map::new_flat(2, 1, 0);
        set_h(&mut m, 1, 0, 1);
        // (0,0): hnorth=0, hwest=1, heast/hsouth fuera → 0; min=0 → solo W
        assert_eq!(compute_tileh(&m, 0, 0), 1);
        // (1,0): hnorth=1, hwest fuera 0, heast/hsouth 0 → min=0 → N
        assert_eq!(compute_tileh(&m, 1, 0), 8);
    }

    #[test]
    fn inner_tile_flat_when_plateau_uniform() {
        let m = Map::new_flat(3, 3, 5);
        assert_eq!(compute_tileh(&m, 1, 1), 0);
        assert_eq!(compute_tileh(&m, 0, 1), 0);
    }
}

#[cfg(test)]
mod water_coast_height_tests {
    //! Agua con `height` 0 en el export no debe hundir las esquinas de la costa.

    use super::{
        TILE_HALF_H, shore_sprite_half_h, shore_tileh_for_draw_shore, tile_slope_and_min_z,
        water_void_effective_height_for_slope,
    };
    use openttdrs_core::{Map, TileCoord, TileKind};

    #[test]
    fn peninsula_grass_flat_when_water_corners_stored_zero() {
        let mut m = Map::new_flat(2, 2, 0);
        m.set_kind(TileCoord::new(0, 0), TileKind::Grass).unwrap();
        m.set_height(TileCoord::new(0, 0), 5).unwrap();
        for (x, y) in [(1, 0), (0, 1), (1, 1)] {
            m.set_kind(TileCoord::new(x, y), TileKind::Water).unwrap();
            m.set_height(TileCoord::new(x, y), 0).unwrap();
        }
        let (tileh, min_z) = tile_slope_and_min_z(&m, 0, 0);
        assert_eq!(min_z, 5, "min_h no debe ser 0 por las celdas de agua");
        assert_eq!(tileh, 0);
    }

    #[test]
    fn water_pool_inherits_ring_grass_height() {
        // Anillo de hierba h=5, charco 2×2 de agua con height 0 en el centro de un 4×4.
        let mut m = Map::new_flat(4, 4, 0);
        for y in 0..4 {
            for x in 0..4 {
                let ring = x == 0 || y == 0 || x == 3 || y == 3;
                if ring {
                    m.set_kind(TileCoord::new(x, y), TileKind::Grass).unwrap();
                    m.set_height(TileCoord::new(x, y), 5).unwrap();
                } else {
                    m.set_kind(TileCoord::new(x, y), TileKind::Water).unwrap();
                    m.set_height(TileCoord::new(x, y), 0).unwrap();
                }
            }
        }
        let (tileh, min_z) = tile_slope_and_min_z(&m, 1, 1);
        assert_eq!(min_z, 5);
        assert_eq!(tileh, 0);
    }

    #[test]
    fn mp_water_never_exposes_terrain_slope_bits() {
        use super::compute_tileh;
        let mut m = Map::new_flat(4, 4, 0);
        for y in 0..4 {
            for x in 0..4 {
                let ring = x == 0 || y == 0 || x == 3 || y == 3;
                if ring {
                    m.set_kind(TileCoord::new(x, y), TileKind::Grass).unwrap();
                    m.set_height(TileCoord::new(x, y), 5).unwrap();
                } else {
                    m.set_kind(TileCoord::new(x, y), TileKind::Water).unwrap();
                    m.set_height(TileCoord::new(x, y), 0).unwrap();
                }
            }
        }
        assert_eq!(compute_tileh(&m, 1, 1), 0);
    }

    #[test]
    fn shore_tileh_uses_diagonal_slope_not_infer_priority_w() {
        // 2×2: agua (0,0); hierba con alturas que dan SLOPE_SW (3) en el cuarteto.
        // `infer_coast` miraría primero tierra en (1,0) y devolvería solo W (1).
        let mut m = Map::new_flat(2, 2, 1);
        m.set_kind(TileCoord::new(0, 0), TileKind::Water).unwrap();
        m.set_height(TileCoord::new(0, 0), 1).unwrap();
        m.set_kind(TileCoord::new(1, 0), TileKind::Grass).unwrap();
        m.set_height(TileCoord::new(1, 0), 3).unwrap();
        m.set_kind(TileCoord::new(0, 1), TileKind::Grass).unwrap();
        m.set_height(TileCoord::new(0, 1), 1).unwrap();
        m.set_kind(TileCoord::new(1, 1), TileKind::Grass).unwrap();
        m.set_height(TileCoord::new(1, 1), 3).unwrap();
        assert_eq!(shore_tileh_for_draw_shore(&m, 0, 0, 2, 2), 3);
    }

    #[test]
    fn water_height_nonzero_is_preserved_for_slope_sampling() {
        // Si MP_WATER ya trae altura válida, no debemos sustituirla por min(vecinos).
        let mut m = Map::new_flat(3, 3, 0);
        m.set_kind(TileCoord::new(1, 1), TileKind::Water).unwrap();
        m.set_height(TileCoord::new(1, 1), 7).unwrap();
        // Vecinos de tierra más bajos (si hubiese inferencia, bajaría).
        for (x, y, h) in [(0, 1, 3), (2, 1, 4), (1, 0, 2), (1, 2, 5)] {
            m.set_kind(TileCoord::new(x, y), TileKind::Grass).unwrap();
            m.set_height(TileCoord::new(x, y), h).unwrap();
        }
        let got = water_void_effective_height_for_slope(&m, 1, 1, 3, 3, 7);
        assert_eq!(got, 7);
    }

    #[test]
    fn unsupported_raw_shore_slopes_fallback_to_infer() {
        // raw=7 (WSE) no tiene sprite legacy directo; debe caer a inferencia.
        let mut m = Map::new_flat(2, 2, 0);
        m.set_kind(TileCoord::new(0, 0), TileKind::Water).unwrap();
        m.set_height(TileCoord::new(0, 0), 0).unwrap(); // hnorth
        m.set_kind(TileCoord::new(1, 0), TileKind::Grass).unwrap();
        m.set_height(TileCoord::new(1, 0), 1).unwrap(); // hwest
        m.set_kind(TileCoord::new(0, 1), TileKind::Grass).unwrap();
        m.set_height(TileCoord::new(0, 1), 1).unwrap(); // heast
        m.set_kind(TileCoord::new(1, 1), TileKind::Grass).unwrap();
        m.set_height(TileCoord::new(1, 1), 1).unwrap(); // hsouth => raw 7
        assert_eq!(shore_tileh_for_draw_shore(&m, 0, 0, 2, 2), 3);
    }

    #[test]
    fn shore_half_h_matches_effective_slope_anchor() {
        assert_eq!(shore_sprite_half_h(1), TILE_HALF_H);
        assert_eq!(shore_sprite_half_h(4), TILE_HALF_H);

        for tileh in [2, 3, 6, 8, 9, 12] {
            assert_eq!(shore_sprite_half_h(tileh), 11.5);
        }
    }
}

#[cfg(test)]
mod tile_min_corner_height_tests {
    use super::{tile_min_corner_height, tile_min_z};
    use openttdrs_core::{Map, TileCoord};

    fn set_h(map: &mut Map, x: i32, y: i32, h: u8) {
        map.set_height(TileCoord::new(x, y), h).unwrap();
    }

    #[test]
    fn plateau_all_same() {
        let m = Map::new_flat(2, 2, 9);
        assert_eq!(tile_min_corner_height(&m, 0, 0), 9);
    }

    #[test]
    fn min_follows_lowest_corner_sample() {
        let mut m = Map::new_flat(2, 2, 5);
        set_h(&mut m, 1, 1, 2);
        // Esquinas de (0,0): N=5, W=5, E=5, S=2 → min 2
        assert_eq!(tile_min_corner_height(&m, 0, 0), 2);
    }

    #[test]
    fn tile_min_z_out_of_bounds() {
        let m = Map::new_flat(1, 1, 1);
        assert_eq!(tile_min_z(&m, TileCoord::new(-1, 0)), 0);
        assert_eq!(tile_min_z(&m, TileCoord::new(0, 1)), 0);
    }
}

#[cfg(test)]
mod infer_coast_tileh_tests {
    use super::infer_coast_tileh_when_flat;
    use openttdrs_core::{Map, TileCoord, TileKind};

    #[test]
    fn land_in_quartet_east_sample_prefers_w_slope() {
        let mut m = Map::new_flat(2, 2, 3);
        for y in 0..2 {
            for x in 0..2 {
                m.set_kind(TileCoord::new(x, y), TileKind::Water).unwrap();
            }
        }
        m.set_kind(TileCoord::new(1, 0), TileKind::Grass).unwrap();
        assert_eq!(infer_coast_tileh_when_flat(&m, 0, 0, 2, 2), 1);
    }

    #[test]
    fn land_north_outside_quartet_prefers_n() {
        // Fila y=0 hierba, y=1 agua: la costa mira hacia N; el 2×2 de (0,1) es todo agua.
        let mut m = Map::new_flat(2, 3, 2);
        for x in 0..2 {
            m.set_kind(TileCoord::new(x, 0), TileKind::Grass).unwrap();
            m.set_kind(TileCoord::new(x, 1), TileKind::Water).unwrap();
            m.set_kind(TileCoord::new(x, 2), TileKind::Water).unwrap();
        }
        assert_eq!(infer_coast_tileh_when_flat(&m, 0, 1, 2, 3), 8);
    }

    #[test]
    fn land_west_and_south_corners_prefers_sw_diagonal() {
        let mut m = Map::new_flat(2, 2, 3);
        for y in 0..2 {
            for x in 0..2 {
                m.set_kind(TileCoord::new(x, y), TileKind::Water).unwrap();
            }
        }
        m.set_kind(TileCoord::new(1, 0), TileKind::Grass).unwrap(); // hwest
        m.set_kind(TileCoord::new(1, 1), TileKind::Grass).unwrap(); // hsouth
        assert_eq!(infer_coast_tileh_when_flat(&m, 0, 0, 2, 2), 3);
    }

    #[test]
    fn diagonal_land_outside_quartet_keeps_screen_side_orientation() {
        let mut m = Map::new_flat(3, 3, 3);
        for y in 0..3 {
            for x in 0..3 {
                m.set_kind(TileCoord::new(x, y), TileKind::Water).unwrap();
            }
        }

        m.set_kind(TileCoord::new(2, 0), TileKind::Grass).unwrap(); // screen-left
        assert_eq!(infer_coast_tileh_when_flat(&m, 1, 1, 3, 3), 1);

        m.set_kind(TileCoord::new(2, 0), TileKind::Water).unwrap();
        m.set_kind(TileCoord::new(0, 2), TileKind::Grass).unwrap(); // screen-right
        assert_eq!(infer_coast_tileh_when_flat(&m, 1, 1, 3, 3), 4);
    }
}

#[cfg(test)]
mod world_pos_to_tile_tests {
    use bevy::prelude::Vec2;

    use super::{
        HEIGHT_PX, ISO_HW, Map, TILE_HALF_H, TileCoord, TileKind, iso, world_pos_to_tile_coord,
        world_to_tile,
    };

    /// Mapa al mismo nivel: el centro del sprite (como en `tile_pos`) debe mapear a su tesela;
    /// [`world_to_tile`] daño con el desfase de elevación.
    #[test]
    fn corrects_height_offset_for_flat_tileh() {
        let m = Map::new_flat(256, 256, 5);
        let tx: i32 = 137;
        let ty: i32 = 118;
        let base_z = super::tile_min_corner_height(&m, tx as u32, ty as u32);
        let elev = f32::from(base_z) * HEIGHT_PX;
        let p = iso(tx, ty);
        let center = Vec2::new(p.x, p.y - TILE_HALF_H + elev);
        assert_eq!(world_pos_to_tile_coord(center, &m), Some((tx, ty)));
        assert_ne!(world_to_tile(center), (tx, ty));
    }

    #[test]
    fn keeps_same_tile_on_left_and_right_half_of_diamond() {
        let m = Map::new_flat(256, 256, 5);
        let tx: i32 = 149;
        let ty: i32 = 122;
        let base_z = super::tile_min_corner_height(&m, tx as u32, ty as u32);
        let elev = f32::from(base_z) * HEIGHT_PX;
        let top = iso(tx, ty) + Vec2::new(0.0, elev);
        // Dos puntos bien dentro del mismo rombo (mitad izquierda y derecha).
        let left_inside = top + Vec2::new(-8.0, -8.0);
        let right_inside = top + Vec2::new(8.0, -8.0);

        assert_eq!(world_pos_to_tile_coord(left_inside, &m), Some((tx, ty)));
        assert_eq!(world_pos_to_tile_coord(right_inside, &m), Some((tx, ty)));
    }

    #[test]
    fn keeps_same_tile_near_top_inside_of_diamond() {
        let m = Map::new_flat(256, 256, 5);
        let tx: i32 = 149;
        let ty: i32 = 122;
        let base_z = super::tile_min_corner_height(&m, tx as u32, ty as u32);
        let elev = f32::from(base_z) * HEIGHT_PX;
        let center = Vec2::new(iso(tx, ty).x, iso(tx, ty).y - TILE_HALF_H + elev);
        let near_top_inside = center + Vec2::new(0.0, TILE_HALF_H - 1.0);
        assert_eq!(world_pos_to_tile_coord(near_top_inside, &m), Some((tx, ty)));
    }

    #[test]
    fn diamond_top_vertex_still_resolves_tile() {
        let m = Map::new_flat(12, 8, 4);
        let tx = 1;
        let ty = 5;
        let base_z = super::tile_min_corner_height(&m, tx as u32, ty as u32);
        let elev = f32::from(base_z) * HEIGHT_PX;
        let center = Vec2::new(iso(tx, ty).x, iso(tx, ty).y - TILE_HALF_H + elev);
        let top_vertex = center + Vec2::new(0.0, TILE_HALF_H - 0.5);
        assert_eq!(
            world_pos_to_tile_coord(top_vertex, &m),
            Some((tx, ty)),
            "vértice superior del rombo debe seguir eligiendo la tesela"
        );
    }

    #[test]
    fn west_neighbor_of_column_zero_is_not_absorbed_by_zero() {
        let mut m = Map::new_flat(12, 8, 4);
        let house = TileCoord::new(0, 5);
        let west = TileCoord::new(1, 5);
        m.set_kind(house, TileKind::House).unwrap();
        m.set_kind(west, TileKind::Road).unwrap();
        m.set_mapt_m5(west, 0x20, 0x05).unwrap();

        let center_west = {
            let base_z = super::tile_min_corner_height(&m, 1, 5);
            let elev = f32::from(base_z) * HEIGHT_PX;
            let p = iso(1, 5);
            Vec2::new(p.x, p.y - 19.5 + elev)
        };
        assert_eq!(
            world_pos_to_tile_coord(center_west, &m),
            Some((1, 5)),
            "estacionamiento/vía en (1,5) no debe leerse como (0,5)"
        );
    }

    #[test]
    fn edge_column_zero_keeps_clicks_on_left_half_of_diamond() {
        let m = Map::new_flat(12, 8, 4);
        let tx = 0;
        let ty = 5;
        let base_z = super::tile_min_corner_height(&m, tx as u32, ty as u32);
        let elev = f32::from(base_z) * HEIGHT_PX;
        let center = Vec2::new(iso(tx, ty).x, iso(tx, ty).y - TILE_HALF_H + elev);
        let left_vertex = center + Vec2::new(-ISO_HW + 2.0, 0.0);
        assert_eq!(
            world_pos_to_tile_coord(left_vertex, &m),
            Some((tx, ty)),
            "clic en borde izquierdo de (0, ty) debe mapear a columna 0"
        );
    }

    #[test]
    fn road_flat_keeps_bottom_visible_area_in_same_tile() {
        let mut m = Map::new_flat(256, 256, 5);
        let tx: i32 = 149;
        let ty: i32 = 122;
        m.set_kind(TileCoord::new(tx, ty), TileKind::Road).unwrap();
        let base_z = super::tile_min_corner_height(&m, tx as u32, ty as u32);
        let elev = f32::from(base_z) * HEIGHT_PX;
        // Carretera plana puede tener half_h visual mayor (~19.5).
        let center = Vec2::new(iso(tx, ty).x, iso(tx, ty).y - 19.5 + elev);
        let near_bottom_inside = center + Vec2::new(0.0, 19.5 - 1.0);
        assert_eq!(
            world_pos_to_tile_coord(near_bottom_inside, &m),
            Some((tx, ty))
        );
    }
}
