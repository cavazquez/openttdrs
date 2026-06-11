//! Utilidades de proyección isométrica.
#![allow(clippy::unwrap_used)] // tests de `compute_tileh` usan mapas mínimos fijos

mod coords;
mod slope;
mod util;
mod water;

#[allow(unused_imports)]
pub use coords::{
    RoadStopSeqGfx, gizmo_diamond, iso, overlay_pos, remap_tile_offset,
    road_stop_build_sprite_center, road_stop_overlay_rel, road_stop_sprite_pos,
    road_vehicle_tile_anchor, tile_pos, tile_pos_half, world_pos_to_tile_coord, world_to_tile,
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
    fn three_corner_shore_slope_uses_its_own_sprite() {
        // raw=7 (WSE) tiene sprite propio en el set completo (slot 7); no debe
        // caer a inferencia ni alterarse por la altura efectiva (tierra a +1).
        let mut m = Map::new_flat(2, 2, 0);
        m.set_kind(TileCoord::new(0, 0), TileKind::Water).unwrap();
        m.set_height(TileCoord::new(0, 0), 0).unwrap(); // hnorth
        m.set_kind(TileCoord::new(1, 0), TileKind::Grass).unwrap();
        m.set_height(TileCoord::new(1, 0), 1).unwrap(); // hwest
        m.set_kind(TileCoord::new(0, 1), TileKind::Grass).unwrap();
        m.set_height(TileCoord::new(0, 1), 1).unwrap(); // heast
        m.set_kind(TileCoord::new(1, 1), TileKind::Grass).unwrap();
        m.set_height(TileCoord::new(1, 1), 1).unwrap(); // hsouth => raw 7
        assert_eq!(shore_tileh_for_draw_shore(&m, 0, 0, 2, 2), 7);
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
    fn remap_tile_offset_matches_bus_stop_ne_build_a() {
        let off = super::remap_tile_offset(2.0, 0.0, 0.0);
        assert_eq!(off.x, -8.0);
        assert_eq!(off.y, -4.0);
    }

    #[test]
    fn road_vehicle_anchor_matches_iso_at_tile_origin() {
        assert_eq!(
            super::road_vehicle_tile_anchor(3, 6, 0.0, 0.0, 0.0),
            iso(3, 6)
        );
        assert_eq!(
            super::road_vehicle_tile_anchor(0, 0, 0.0, 0.0, 0.0),
            iso(0, 0)
        );
    }

    #[test]
    fn road_vehicle_sw_subtile_uses_openrtd_lane_y9() {
        use openttdrs_core::{DIR_SW, straight_subtile};

        let (x0, y0) = straight_subtile(DIR_SW, 0);
        assert_eq!((x0, y0), (0.0, 9.0));
        let (x1, y1) = straight_subtile(DIR_SW, 255);
        assert_eq!((x1, y1), (15.0, 9.0));

        let exit = super::road_vehicle_tile_anchor(5, 6, x1, y1, 0.0);
        let entry = super::road_vehicle_tile_anchor(6, 6, x0, y0, 0.0);
        let delta = exit - entry;
        assert!(
            delta.length() < 4.0,
            "salida (5,6) y entrada (6,6) deben ser continuas: delta={delta:?}"
        );
    }

    #[test]
    fn road_vehicle_sw_lane_near_tile_center_mid_tile() {
        use openttdrs_core::{DIR_SW, straight_subtile};

        let (sub_x, sub_y) = straight_subtile(DIR_SW, 128);
        assert!((sub_x - 7.5).abs() < 0.1);
        assert_eq!(sub_y, 9.0);
        let lane = super::road_vehicle_tile_anchor(5, 6, sub_x, sub_y, 0.0);
        let ground = Vec2::new(iso(5, 6).x, iso(5, 6).y - super::TILE_HALF_H);
        let delta = lane - ground;
        assert!(
            delta.length() < 12.0,
            "mid-carril SW cerca del centro del rombo: delta={delta:?}"
        );
    }

    #[test]
    fn road_vehicle_ne_subtile_moves_along_y5() {
        use openttdrs_core::{DIR_NE, straight_subtile};

        let start = straight_subtile(DIR_NE, 0);
        let end = straight_subtile(DIR_NE, 255);
        assert_eq!(start, (15.0, 5.0));
        assert_eq!(end, (0.0, 5.0));
    }

    #[test]
    fn road_vehicle_ne_slope_dz_shifts_anchor_downhill() {
        use openttdrs_core::{SLOPE_NE, partial_pixel_z, slope_dz_at_subtile};

        let flat = super::road_vehicle_tile_anchor(5, 6, 0.0, 9.0, 0.0);
        let dz = slope_dz_at_subtile(0.0, 9.0, SLOPE_NE);
        assert_eq!(dz, f32::from(partial_pixel_z(0.0, 9.0, SLOPE_NE)));
        let raised = super::road_vehicle_tile_anchor(5, 6, 0.0, 9.0, dz);
        assert!(
            raised.y > flat.y,
            "carril alto en pendiente NE sube en pantalla"
        );
    }

    #[test]
    fn remap_tile_offset_ne_build_c_is_standard_remap() {
        let off = super::remap_tile_offset(0.0, 13.0, 0.0);
        assert_eq!(off.x, 52.0);
        assert_eq!(off.y, -26.0);
    }

    #[test]
    fn road_stop_ne_build_c_center_stays_on_station_tile() {
        let origin = iso(15, 2);
        let ground = Vec2::new(origin.x, origin.y - super::TILE_HALF_H);
        let seq = super::RoadStopSeqGfx {
            dx: 0.0,
            dy: 13.0,
            dz: 0.0,
            x_offs: -24.0,
            y_offs: -8.0,
            remap_x_adj: -13.0,
        };
        let c = super::road_stop_build_sprite_center(origin, 15, 2, 0, 0.07, seq, 24.0, 23.0);
        let dist_station = (c.x - ground.x).hypot(c.y - ground.y);
        let neighbor = iso(16, 2);
        let ground16 = Vec2::new(neighbor.x, neighbor.y - super::TILE_HALF_H);
        let dist_neighbor = (c.x - ground16.x).hypot(c.y - ground16.y);
        assert!(
            dist_station < dist_neighbor,
            "build_c debe quedar más cerca del centro de (15,2) que de (16,2)"
        );
    }

    #[test]
    fn road_stop_build_center_matches_north_anchor_top_left() {
        let seq = super::RoadStopSeqGfx {
            dx: 2.0,
            dy: 0.0,
            dz: 0.0,
            x_offs: -29.0,
            y_offs: -2.0,
            remap_x_adj: 0.0,
        };
        let w = 21.0;
        let h = 13.0;
        let origin = iso(2, 3);
        let c = super::road_stop_build_sprite_center(origin, 2, 3, 0, 0.05, seq, w, h);
        let north_tl = super::road_stop_sprite_pos(2, 3, 0, 0.05, seq);
        assert_eq!(c.x - w * 0.5, north_tl.x);
        assert_eq!(c.y + h * 0.5, north_tl.y);
    }

    #[test]
    fn road_stop_overlay_rel_matches_rail_station_for_ne_build_a() {
        let seq = super::RoadStopSeqGfx {
            dx: 2.0,
            dy: 0.0,
            dz: 0.0,
            x_offs: 0.0,
            y_offs: 0.0,
            remap_x_adj: 0.0,
        };
        let (xrel, yrel) = super::road_stop_overlay_rel(seq);
        let rail = crate::sprites::rail_station_overlay_rel(2.0, 0.0);
        assert_eq!(xrel, rail.0);
        assert_eq!(yrel, rail.1);
    }

    #[test]
    fn road_stop_overlay_rel_ne_build_c_adj() {
        let seq = super::RoadStopSeqGfx {
            dx: 0.0,
            dy: 13.0,
            dz: 0.0,
            x_offs: 0.0,
            y_offs: 0.0,
            remap_x_adj: -13.0,
        };
        let (xrel, yrel) = super::road_stop_overlay_rel(seq);
        let rail = crate::sprites::rail_station_overlay_rel(0.0, 13.0);
        assert_eq!(yrel, rail.1);
        assert_eq!(xrel, 0.0);
        assert_eq!(rail.0, 52.0);
    }

    #[test]
    fn road_stop_se_truck_build_a_center_stays_on_station_tile() {
        let tx = 14;
        let ty = 7;
        let origin = iso(tx, ty);
        let ground = Vec2::new(origin.x, origin.y - super::TILE_HALF_H);
        let seq = super::RoadStopSeqGfx {
            dx: 15.0,
            dy: 3.0,
            dz: 0.0,
            x_offs: -3.0,
            y_offs: -23.0,
            remap_x_adj: 8.0,
        };
        let w = 28.0;
        let h = 20.0;
        let c = super::road_stop_build_sprite_center(origin, tx, ty, 0, 0.05, seq, w, h);
        let top_left = super::road_stop_sprite_pos(tx, ty, 0, 0.05, seq);
        let bottom_right = Vec2::new(top_left.x + w, top_left.y - h);
        assert!(
            top_left.x <= ground.x
                && ground.x <= bottom_right.x
                && bottom_right.y <= ground.y
                && ground.y <= top_left.y,
            "truck SE build_a debe cubrir el centro de la tesela de parada"
        );
        let south = iso(tx, ty + 1);
        let ground_south = Vec2::new(south.x, south.y - super::TILE_HALF_H);
        let dist_station = (c.x - ground.x).hypot(c.y - ground.y);
        let dist_south = (c.x - ground_south.x).hypot(c.y - ground_south.y);
        assert!(
            dist_station < dist_south,
            "truck SE build_a no debe caer en tesela ({tx},{ty_plus})",
            ty_plus = ty + 1
        );
    }

    #[test]
    fn road_stop_generated_adj_exceptions() {
        let ne = crate::sprites::road_stop_build_layers(crate::sprites::StationTileClass::Bus, 0);
        assert_eq!(ne[0].remap_x_adj, 0.0);
        assert_eq!(ne[1].remap_x_adj, 7.0);
        assert_eq!(ne[1].y_offs, -12.0);
        assert_eq!(ne[2].remap_x_adj, -13.0);
        let se_bus =
            crate::sprites::road_stop_build_layers(crate::sprites::StationTileClass::Bus, 1);
        assert_eq!(se_bus[0].remap_x_adj, -3.0);
        assert_eq!(se_bus[2].remap_x_adj, 5.0);
        assert_eq!(se_bus[2].y_offs, -18.0);
        let sw_bus =
            crate::sprites::road_stop_build_layers(crate::sprites::StationTileClass::Bus, 2);
        assert_eq!(sw_bus[0].remap_x_adj, -8.0);
        assert_eq!(sw_bus[0].y_offs, -13.0);
        let nw_bus =
            crate::sprites::road_stop_build_layers(crate::sprites::StationTileClass::Bus, 3);
        assert_eq!(nw_bus[0].remap_x_adj, 8.0);
        assert_eq!(nw_bus[0].y_offs, -19.0);
        assert_eq!(nw_bus[1].remap_x_adj, -7.0);
        assert_eq!(nw_bus[1].y_offs, -10.0);
        let se_truck =
            crate::sprites::road_stop_build_layers(crate::sprites::StationTileClass::Truck, 1);
        assert_eq!(se_truck[0].remap_x_adj, 8.0);
        assert_eq!(se_truck[0].y_offs, -23.0);
        assert_eq!(se_truck[1].remap_x_adj, 0.0);
        assert_eq!(se_truck[2].remap_x_adj, -3.0);
    }

    #[test]
    fn road_stop_checklist_bus_layers_prefer_station_over_road_neighbor() {
        use crate::sprites::{StationTileClass, road_stop_build_layers, road_stop_seq_gfx};

        const CASES: [(i32, i32, usize, i32, i32); 4] = [
            (1, 9, 0, -1, 0),
            (3, 9, 1, 0, 1),
            (5, 9, 2, 1, 0),
            (7, 9, 3, 0, -1),
        ];

        for (tx, ty, dir, rdx, rdy) in CASES {
            let origin = iso(tx, ty);
            let ground = Vec2::new(origin.x, origin.y - super::TILE_HALF_H);
            let road = iso(tx + rdx, ty + rdy);
            let ground_road = Vec2::new(road.x, road.y - super::TILE_HALF_H);
            for spec in road_stop_build_layers(StationTileClass::Bus, dir) {
                let center = super::road_stop_build_sprite_center(
                    origin,
                    tx,
                    ty,
                    0,
                    spec.z,
                    road_stop_seq_gfx(spec),
                    spec.w,
                    spec.h,
                );
                let dist_station = (center.x - ground.x).hypot(center.y - ground.y);
                let dist_road = (center.x - ground_road.x).hypot(center.y - ground_road.y);
                assert!(
                    dist_station < dist_road,
                    "bus dir {dir} capa z={} en ({tx},{ty}) no debe caer hacia ({}, {})",
                    spec.z,
                    tx + rdx,
                    ty + rdy
                );
            }
        }
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
