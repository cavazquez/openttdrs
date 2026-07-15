//! Conversiones canónicas `TileCoord` ↔ índice lineal / `TileIndex` OpenTTD.
//!
//! Hay **dos** semánticas distintas; no fusionarlas:
//! - **Rectangular / linear** (`y * map_w + x`): saves `.sav` (`xy`), `.ottdmap`, vector `Map::tiles`.
//! - **OpenTTD Po2** (`x | (y << log2(map_w))`): `TileIndex` cuando ancho y alto son potencia de 2.

use super::types::TileCoord;

/// Índice lineal rectangular `y * map_w + x` (campo `xy` de saves, sin tope de altura).
///
/// Rechaza solo coordenadas negativas. Compatible con encode histórico de SAV.
#[must_use]
pub fn coord_to_linear_index(c: TileCoord, map_w: u32) -> Option<u32> {
    if c.x < 0 || c.y < 0 {
        return None;
    }
    Some(
        c.y.cast_unsigned()
            .saturating_mul(map_w)
            .saturating_add(c.x.cast_unsigned()),
    )
}

/// Inversa de [`coord_to_linear_index`]: `x = tile % map_w`, `y = tile / map_w`.
///
/// Sin chequeo de altura (decode SAV histórico). `map_w == 0` → `None`.
#[must_use]
pub fn coord_from_linear_index(tile: u64, map_w: u32) -> Option<TileCoord> {
    if map_w == 0 {
        return None;
    }
    let x = i32::try_from(tile % u64::from(map_w)).ok()?;
    let y = i32::try_from(tile / u64::from(map_w)).ok()?;
    Some(TileCoord::new(x, y))
}

/// Índice denso en el vector `Map::tiles` con bounds `(width, height)`.
#[must_use]
pub fn coord_to_dense_index(c: TileCoord, map_w: u32, map_h: u32) -> Option<usize> {
    if c.x < 0 || c.y < 0 {
        return None;
    }
    let ux = u32::try_from(c.x).ok()?;
    let uy = u32::try_from(c.y).ok()?;
    if ux >= map_w || uy >= map_h {
        return None;
    }
    let linear = uy.checked_mul(map_w)?.checked_add(ux)?;
    usize::try_from(linear).ok()
}

/// Convierte un `TileIndex` de OpenTTD a coordenadas cuando el mapa es potencia de 2 en X e Y
/// (misma convención que `TileXY`: `tile = x | (y << log2(map_w))`).
#[must_use]
pub fn openttd_tile_index_to_coord(tile: u32, map_w: u32, map_h: u32) -> Option<TileCoord> {
    if !map_w.is_power_of_two() || !map_h.is_power_of_two() {
        return None;
    }
    let log_w = map_w.trailing_zeros();
    let x = tile & (map_w - 1);
    let y = tile >> log_w;
    if y >= map_h {
        return None;
    }
    let xi = i32::try_from(x).ok()?;
    let yi = i32::try_from(y).ok()?;
    Some(TileCoord::new(xi, yi))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_roundtrip_64() {
        let c = TileCoord::new(5, 2);
        let i = coord_to_linear_index(c, 64).expect("index");
        assert_eq!(i, 5 + 2 * 64);
        assert_eq!(coord_from_linear_index(u64::from(i), 64), Some(c));
    }

    #[test]
    fn linear_roundtrip_non_square() {
        let c = TileCoord::new(3, 7);
        let i = coord_to_linear_index(c, 32).expect("index");
        assert_eq!(i, 3 + 7 * 32);
        assert_eq!(coord_from_linear_index(u64::from(i), 32), Some(c));
    }

    #[test]
    fn linear_rejects_negatives_and_zero_width() {
        assert_eq!(coord_to_linear_index(TileCoord::new(-1, 0), 64), None);
        assert_eq!(coord_to_linear_index(TileCoord::new(0, -1), 64), None);
        assert_eq!(coord_from_linear_index(0, 0), None);
    }

    #[test]
    fn dense_bounds() {
        assert_eq!(coord_to_dense_index(TileCoord::new(1, 1), 2, 2), Some(3));
        assert_eq!(coord_to_dense_index(TileCoord::new(2, 0), 2, 2), None);
        assert_eq!(coord_to_dense_index(TileCoord::new(0, 2), 2, 2), None);
        assert_eq!(coord_to_dense_index(TileCoord::new(-1, 0), 2, 2), None);
    }

    #[test]
    fn openttd_tile_index_roundtrip_2x2() {
        assert_eq!(
            openttd_tile_index_to_coord(0, 2, 2),
            Some(TileCoord::new(0, 0))
        );
        assert_eq!(
            openttd_tile_index_to_coord(1, 2, 2),
            Some(TileCoord::new(1, 0))
        );
        assert_eq!(
            openttd_tile_index_to_coord(2, 2, 2),
            Some(TileCoord::new(0, 1))
        );
        assert_eq!(
            openttd_tile_index_to_coord(3, 2, 2),
            Some(TileCoord::new(1, 1))
        );
        assert_eq!(openttd_tile_index_to_coord(4, 2, 2), None);
        assert_eq!(openttd_tile_index_to_coord(0, 3, 3), None);
    }

    #[test]
    fn po2_square_matches_linear() {
        for tile in 0u32..4 {
            let c = openttd_tile_index_to_coord(tile, 2, 2).expect("po2");
            let linear = coord_to_linear_index(c, 2).expect("linear");
            assert_eq!(linear, tile);
            assert_eq!(coord_from_linear_index(u64::from(tile), 2), Some(c));
        }
    }
}
