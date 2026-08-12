//! Pendientes y altura de tesela al estilo OpenTTD (`GetTileSlopeZ`, pendientes inclinadas).

use super::{Map, TileCoord};

/// Pendiente inclinada NE (`tileh` 12).
pub const SLOPE_NE: u8 = 12;
/// Pendiente inclinada SE (`tileh` 6).
pub const SLOPE_SE: u8 = 6;
/// Pendiente inclinada SW (`tileh` 3).
pub const SLOPE_SW: u8 = 3;
/// Pendiente inclinada NW (`tileh` 9).
pub const SLOPE_NW: u8 = 9;

/// Bit de pendiente empinada (`SLOPE_STEEP` en `slope_type.h`).
pub const SLOPE_STEEP: u8 = 0x10;

#[inline]
fn slope_bits_from_corner_vals(hnorth: u8, hwest: u8, heast: u8, hsouth: u8) -> (u8, u8) {
    let min_h = hnorth.min(hwest).min(heast).min(hsouth);
    let max_h = hnorth.max(hwest).max(heast).max(hsouth);
    let mut tileh: u8 = 0;
    if hwest > min_h {
        tileh |= 1;
    }
    if hsouth > min_h {
        tileh |= 2;
    }
    if heast > min_h {
        tileh |= 4;
    }
    if hnorth > min_h {
        tileh |= 8;
    }
    // `GetTileSlopeGivenHeight`: como máximo una esquina puede estar 2 unidades por encima del mínimo.
    if max_h.saturating_sub(min_h) == 2 {
        tileh |= SLOPE_STEEP;
    }
    (tileh, min_h)
}

#[inline]
fn corner_height(map: &Map, cx: i32, cy: i32) -> u8 {
    // `GetTileSlopeZ` no lee una tesela virtual a altura 0 en los bordes:
    // fija x+1/y+1 a `Map::MaxX/Y`. Usar `0` aquí convertía toda la última
    // fila/columna en pendientes artificiales (y alteraba bocas de túnel).
    let (width, height) = map.dimensions();
    if width == 0 || height == 0 {
        return 0;
    }
    let max_x = i32::try_from(width - 1).unwrap_or(i32::MAX);
    let max_y = i32::try_from(height - 1).unwrap_or(i32::MAX);
    let x = cx.clamp(0, max_x);
    let y = cy.clamp(0, max_y);
    map.get(TileCoord::new(x, y)).map_or(0, |tile| tile.height)
}

/// `(tileh, z)` con `z` = mínimo de las cuatro esquinas (= `GetTileZ` en terreno).
#[must_use]
pub fn tile_slope_and_z(map: &Map, c: TileCoord) -> Option<(u8, u8)> {
    let tx = c.x;
    let ty = c.y;
    let hnorth = corner_height(map, tx, ty);
    let hwest = corner_height(map, tx + 1, ty);
    let heast = corner_height(map, tx, ty + 1);
    let hsouth = corner_height(map, tx + 1, ty + 1);
    Some(slope_bits_from_corner_vals(hnorth, hwest, heast, hsouth))
}

/// `true` si la tesela es una pendiente inclinada válida como boca de túnel.
#[must_use]
pub const fn is_tunnel_entrance_slope(tileh: u8) -> bool {
    inclined_slope_direction(tileh).is_some()
}

/// `m5` de boca de túnel: dirección en bits 0–1 + tipo de transporte en bits
/// 2–3 (`TransportType` de OpenTTD: 0 = rail, 1 = road).
#[must_use]
pub fn tunnel_entrance_m5(tileh: u8, rail: bool) -> Option<u8> {
    let dir = inclined_slope_direction(tileh)?;
    Some(dir | if rail { 0 } else { 0x04 })
}

/// Dirección diagonal hacia arriba en una pendiente inclinada (`GetInclinedSlopeDirection`).
#[must_use]
pub const fn inclined_slope_direction(tileh: u8) -> Option<u8> {
    match tileh {
        SLOPE_NE => Some(0),
        SLOPE_SE => Some(1),
        SLOPE_SW => Some(2),
        SLOPE_NW => Some(3),
        _ => None,
    }
}

/// Pendiente complementaria (`ComplementSlope` en OpenTTD).
#[must_use]
pub const fn complement_slope(tileh: u8) -> u8 {
    if tileh == 0 {
        0
    } else {
        let c = tileh ^ 0x0F;
        if c > 14 { 14 } else { c }
    }
}

const TILE_SIZE_SUB: i32 = 16;
const TILE_HEIGHT_SUB: i32 = 8;

/// Altura de un nivel de terreno en píxeles (`TILE_HEIGHT` de OpenTTD).
pub const TILE_PIXEL_HEIGHT: i16 = 8;

/// Altura sub-tesela en unidades `TILE_HEIGHT` (`GetPartialPixelZ` / `landscape.cpp`).
///
/// Además de las pendientes continuas, OpenTTD usa los bits altos de `Slope`
/// para una fundación de media tesela. En esa mitad nivelada la altura salta
/// directamente al máximo de la pendiente; luego se evalúa la pendiente base
/// en la otra mitad. Esta función conserva ambas capas de la codificación.
#[must_use]
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::collapsible_match,
    clippy::match_same_arms
)]
pub fn partial_pixel_z(sub_x: f32, sub_y: f32, tileh: u8) -> u8 {
    let x = sub_x.clamp(0.0, 15.0).round() as i32;
    let y = sub_y.clamp(0.0, 15.0).round() as i32;
    // `IsHalftileSlope` + `GetHalftileSlopeCorner`. El máximo es de dos
    // niveles únicamente en las pendientes empinadas.
    if tileh & 0x20 != 0 {
        let max_z = if tileh & SLOPE_STEEP != 0 {
            TILE_HEIGHT_SUB * 2
        } else {
            TILE_HEIGHT_SUB
        };
        let leveled = match (tileh >> 6) & 3 {
            // Corner::W, ::S, ::E, ::N.
            0 => x > y,
            1 => x + y >= TILE_SIZE_SUB,
            2 => x <= y,
            _ => x + y < TILE_SIZE_SUB,
        };
        if leveled {
            return max_z as u8;
        }
    }

    // `RemoveHalftileSlope`: conserva la pendiente de cinco bits, incluidos
    // los cuatro casos `SLOPE_STEEP_*`.
    match tileh & 0x1F {
        1 if x >= y => ((x - y) >> 1) as u8,
        2 if x + y >= TILE_SIZE_SUB => ((1 + x + y - TILE_SIZE_SUB) >> 1) as u8,
        3 => ((x + 1) >> 1) as u8,
        4 if y >= x => ((1 + y - x) >> 1) as u8,
        5 if x >= y => ((x - y) >> 1) as u8,
        5 => ((1 + y - x) >> 1) as u8,
        6 => ((y + 1) >> 1) as u8,
        7 if x + y <= TILE_SIZE_SUB => (TILE_HEIGHT_SUB - ((TILE_SIZE_SUB - x - y) >> 1)) as u8,
        7 => TILE_HEIGHT_SUB as u8,
        8 if x + y <= TILE_SIZE_SUB => ((TILE_SIZE_SUB - x - y) >> 1) as u8,
        9 => ((TILE_SIZE_SUB - y) >> 1) as u8,
        10 if x + y < TILE_SIZE_SUB => ((TILE_SIZE_SUB - x - y) >> 1) as u8,
        10 => ((1 + x + y - TILE_SIZE_SUB) >> 1) as u8,
        11 if x < y => (TILE_HEIGHT_SUB - ((1 + y - x) >> 1)) as u8,
        11 => TILE_HEIGHT_SUB as u8,
        12 => ((TILE_SIZE_SUB - x) >> 1) as u8,
        13 if y < x => (TILE_HEIGHT_SUB - ((x - y) >> 1)) as u8,
        13 => TILE_HEIGHT_SUB as u8,
        14 if x + y >= TILE_SIZE_SUB => {
            (TILE_HEIGHT_SUB - ((1 + x + y - TILE_SIZE_SUB) >> 1)) as u8
        }
        14 => TILE_HEIGHT_SUB as u8,
        // `SLOPE_STEEP_S`, `W`, `N`, `E`, respectivamente.
        0x17 => ((1 + x + y) >> 1) as u8,
        0x1B => ((TILE_SIZE_SUB + x - y) >> 1) as u8,
        0x1D => ((TILE_SIZE_SUB - x + TILE_SIZE_SUB - y) >> 1) as u8,
        0x1E => ((TILE_SIZE_SUB + 1 + y - x) >> 1) as u8,
        _ => 0,
    }
}

/// Offset de altura (`dz`) para sub-tesela `(sub_x, sub_y)` sobre `tileh`.
#[must_use]
pub fn slope_dz_at_subtile(sub_x: f32, sub_y: f32, tileh: u8) -> f32 {
    f32::from(partial_pixel_z(sub_x, sub_y, tileh))
}

/// `dz` en una tesela del mapa (pendiente inclinada diagonal, etc.).
#[must_use]
pub fn slope_dz_on_tile(map: &Map, c: TileCoord, sub_x: f32, sub_y: f32) -> f32 {
    tile_slope_and_z(map, c).map_or(0.0, |(tileh, _)| slope_dz_at_subtile(sub_x, sub_y, tileh))
}

/// Z en píxeles al estilo `GetSlopePixelZ` (base `GetTileZ` + `GetPartialPixelZ`).
#[must_use]
pub fn slope_pixel_z(map: &Map, c: TileCoord, sub_x: f32, sub_y: f32) -> i16 {
    let Some((tileh, z)) = tile_slope_and_z(map, c) else {
        return 0;
    };
    i16::from(z) * TILE_PIXEL_HEIGHT + i16::from(partial_pixel_z(sub_x, sub_y, tileh))
}

/// Offset de tesela en dirección diagonal (`TileOffsByDiagDir`).
#[must_use]
pub const fn diag_dir_offset(dir: u8) -> (i32, i32) {
    const OFFSETS: [(i32, i32); 4] = [(-1, 0), (0, 1), (1, 0), (0, -1)];
    OFFSETS[dir as usize & 3]
}

/// Salida prospectiva de un túnel que se construiría desde `start`.
///
/// Esta función parte de la pendiente del terreno; no debe usarse para un
/// túnel ya cargado, porque la primera tesela al mismo nivel no necesariamente
/// es su portal opuesto.
#[must_use]
pub fn resolve_tunnel_end(map: &Map, start: TileCoord) -> Option<TileCoord> {
    let (start_tileh, start_z) = tile_slope_and_z(map, start)?;
    let dir = inclined_slope_direction(start_tileh)?;
    let (dx, dy) = diag_dir_offset(dir);
    let mut c = start;
    loop {
        c = TileCoord::new(c.x + dx, c.y + dy);
        map.get(c)?;
        let (_, z) = tile_slope_and_z(map, c)?;
        if z == start_z {
            return Some(c);
        }
    }
}

/// Portal opuesto de un túnel ya persistido en el mapa.
///
/// Es la contraparte segura de `OpenTTD::GetOtherTunnelEnd`: sigue la dirección
/// almacenada en `m5`, exige que el portal final sea una tesela túnel con la
/// dirección inversa y conserve el mismo `GetTileZ`. No infiere la dirección
/// desde la pendiente, porque una fundación o un borde del mapa pueden hacerla
/// distinta de la codificación real del save.
#[must_use]
pub fn resolve_existing_tunnel_end(map: &Map, start: TileCoord) -> Option<TileCoord> {
    let start_tile = map.get(start)?;
    if !start_tile.is_tunnel_bridge_tile() || start_tile.m5 & 0x80 != 0 {
        return None;
    }
    let (_, start_z) = tile_slope_and_z(map, start)?;
    let direction = start_tile.m5 & 0x03;
    let reverse_direction = direction.wrapping_add(2) & 0x03;
    let (step_x, step_y) = diag_dir_offset(direction);
    let (width, height) = map.dimensions();
    let mut pos = start;
    for _ in 0..width.max(height) {
        pos = TileCoord::new(pos.x + step_x, pos.y + step_y);
        let tile = map.get(pos)?;
        if tile.is_tunnel_bridge_tile()
            && tile.m5 & 0x80 == 0
            && tile.m5 & 0x03 == reverse_direction
            && tile_slope_and_z(map, pos).is_some_and(|(_, z)| z == start_z)
        {
            return Some(pos);
        }
    }
    None
}

/// Teselas del túnel desde la entrada `start` hasta `end` (ambas inclusive).
#[must_use]
pub fn tunnel_path_tiles(map: &Map, start: TileCoord, end: TileCoord) -> Vec<TileCoord> {
    let (start_tileh, _) = tile_slope_and_z(map, start).unwrap_or((0, 0));
    let Some(dir) = inclined_slope_direction(start_tileh) else {
        return vec![start];
    };
    let (dx, dy) = diag_dir_offset(dir);
    let mut out = vec![start];
    let mut c = start;
    while c != end {
        c = TileCoord::new(c.x + dx, c.y + dy);
        out.push(c);
        if out.len() > 10_000 {
            break;
        }
    }
    out
}

/// Ruta completa para preview / validación; `None` si la entrada no admite túnel.
#[must_use]
pub fn tunnel_preview_path(map: &Map, start: TileCoord) -> Option<Vec<TileCoord>> {
    let (start_tileh, _) = tile_slope_and_z(map, start)?;
    inclined_slope_direction(start_tileh)?;
    let end = resolve_tunnel_end(map, start)?;
    let (end_tileh, _) = tile_slope_and_z(map, end)?;
    if complement_slope(start_tileh) != end_tileh {
        return None;
    }
    Some(tunnel_path_tiles(map, start, end))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn set_ne_slope(map: &mut Map, tx: i32, ty: i32, base: u8) {
        map.set_height(TileCoord::new(tx, ty), base + 1).unwrap();
        map.set_height(TileCoord::new(tx, ty + 1), base + 1)
            .unwrap();
        map.set_height(TileCoord::new(tx + 1, ty), base).unwrap();
        map.set_height(TileCoord::new(tx + 1, ty + 1), base)
            .unwrap();
    }

    fn set_sw_slope(map: &mut Map, tx: i32, ty: i32, base: u8) {
        map.set_height(TileCoord::new(tx, ty), base).unwrap();
        map.set_height(TileCoord::new(tx, ty + 1), base).unwrap();
        map.set_height(TileCoord::new(tx + 1, ty), base + 1)
            .unwrap();
        map.set_height(TileCoord::new(tx + 1, ty + 1), base + 1)
            .unwrap();
    }

    #[test]
    fn ne_slope_resolves_sw_end_at_same_z() {
        let mut map = Map::new_flat(12, 12, 1);
        set_ne_slope(&mut map, 5, 5, 1);
        set_sw_slope(&mut map, 3, 5, 1);
        let start = TileCoord::new(5, 5);
        let (tileh, z) = tile_slope_and_z(&map, start).unwrap();
        assert_eq!(tileh, SLOPE_NE);
        assert_eq!(z, 1);
        let end = resolve_tunnel_end(&map, start).unwrap();
        assert_eq!(end, TileCoord::new(3, 5));
        let (end_h, end_z) = tile_slope_and_z(&map, end).unwrap();
        assert_eq!(end_h, SLOPE_SW);
        assert_eq!(end_z, 1);
        assert!(tunnel_preview_path(&map, start).is_some());
    }

    #[test]
    fn flat_tile_has_no_tunnel_direction() {
        let map = Map::new_flat(4, 4, 2);
        let c = TileCoord::new(1, 1);
        assert!(inclined_slope_direction(tile_slope_and_z(&map, c).unwrap().0).is_none());
        assert!(resolve_tunnel_end(&map, c).is_none());
    }

    #[test]
    fn complement_ne_is_sw() {
        assert_eq!(complement_slope(SLOPE_NE), SLOPE_SW);
    }

    #[test]
    fn inclined_ne_slope_partial_z_matches_openrtd() {
        assert_eq!(partial_pixel_z(0.0, 9.0, SLOPE_NE), 8);
        assert_eq!(partial_pixel_z(15.0, 9.0, SLOPE_NE), 0);
        assert_eq!(partial_pixel_z(8.0, 9.0, SLOPE_NE), 4);
    }

    #[test]
    fn slope_pixel_z_combines_tile_z_and_partial() {
        let mut map = Map::new_flat(4, 4, 4);
        // N+E elevados → SLOPE_NE en (1,1).
        map.set_height(TileCoord::new(1, 1), 5).unwrap();
        map.set_height(TileCoord::new(1, 2), 5).unwrap();
        let c = TileCoord::new(1, 1);
        let (tileh, z) = tile_slope_and_z(&map, c).unwrap();
        assert_eq!(tileh, SLOPE_NE);
        assert_eq!(z, 4);
        assert_eq!(
            slope_pixel_z(&map, c, 0.0, 9.0),
            i16::from(z) * TILE_PIXEL_HEIGHT + 8
        );
        assert_eq!(
            slope_pixel_z(&map, c, 15.0, 9.0),
            i16::from(z) * TILE_PIXEL_HEIGHT
        );
    }

    #[test]
    fn inclined_sw_slope_partial_z_matches_openrtd() {
        assert_eq!(partial_pixel_z(0.0, 9.0, SLOPE_SW), 0);
        assert_eq!(partial_pixel_z(15.0, 9.0, SLOPE_SW), 8);
    }

    #[test]
    fn three_corner_partial_z_matches_upstream_table() {
        // SLOPE_WSE = W | S | E. Antes se trataba erróneamente como una
        // pendiente que cae hacia SE; esto desplazaba puntos PCP de catenaria.
        const SLOPE_WSE: u8 = 0x07;
        // Las otras tres variantes son rotaciones, no la misma fórmula
        // aplicada a los ejes intercambiados.
        const SLOPE_NWS: u8 = 0x0B;
        const SLOPE_SEN: u8 = 0x0D;
        const SLOPE_ENW: u8 = 0x0E;

        assert_eq!(partial_pixel_z(0.0, 0.0, SLOPE_WSE), 0);
        assert_eq!(partial_pixel_z(8.0, 15.0, SLOPE_WSE), 8);

        assert_eq!(partial_pixel_z(8.0, 0.0, SLOPE_NWS), 8);
        assert_eq!(partial_pixel_z(15.0, 8.0, SLOPE_SEN), 5);
        assert_eq!(partial_pixel_z(15.0, 8.0, SLOPE_ENW), 4);
    }

    #[test]
    fn halftile_and_steep_partial_z_preserve_open_ttd_encoding() {
        // SLOPE_W con la mitad W nivelada: la mitad x > y queda al máximo.
        const SLOPE_HALFTILE_W: u8 = 0x20 | 0x01;
        assert_eq!(partial_pixel_z(15.0, 0.0, SLOPE_HALFTILE_W), 8);
        assert_eq!(partial_pixel_z(0.0, 15.0, SLOPE_HALFTILE_W), 0);

        // SLOPE_STEEP_S = SLOPE_STEEP | SLOPE_WSE.
        assert_eq!(partial_pixel_z(0.0, 0.0, SLOPE_STEEP | 0x07), 0);
        assert_eq!(partial_pixel_z(15.0, 15.0, SLOPE_STEEP | 0x07), 15);
    }

    #[test]
    fn flat_slope_has_zero_partial_z() {
        assert_eq!(partial_pixel_z(7.5, 9.0, 0), 0);
    }

    #[test]
    fn map_edges_clamp_corner_heights_like_openttd() {
        let mut map = Map::new_flat(2, 2, 0);
        map.set_height(TileCoord::new(1, 1), 5).unwrap();
        let (tileh, z) = tile_slope_and_z(&map, TileCoord::new(1, 1)).unwrap();
        assert_eq!(tileh, 0);
        assert_eq!(z, 5);
    }

    #[test]
    fn tunnel_path_follows_diagonal() {
        let mut map = Map::new_flat(8, 8, 1);
        set_ne_slope(&mut map, 4, 4, 1);
        set_sw_slope(&mut map, 2, 4, 1);
        let start = TileCoord::new(4, 4);
        let end = resolve_tunnel_end(&map, start).unwrap();
        let path = tunnel_path_tiles(&map, start, end);
        assert_eq!(path.len(), 3);
        assert_eq!(path[0], start);
        assert_eq!(path[2], end);
    }

    #[test]
    fn existing_tunnel_uses_saved_direction_and_opposite_portal() {
        let mut map = Map::new_flat(8, 4, 2);
        let west = TileCoord::new(1, 1);
        let east = TileCoord::new(5, 1);
        let mut west_tile = map.get(west).unwrap();
        west_tile.kind = crate::map::TileKind::RailTunnel;
        west_tile.mapt = 0x90;
        // DiagDirection::SW: +X, transport rail, tunnel flag clear.
        west_tile.m5 = 0x02;
        map.set_tile(west, west_tile).unwrap();
        let mut east_tile = map.get(east).unwrap();
        east_tile.kind = crate::map::TileKind::RailTunnel;
        east_tile.mapt = 0x90;
        // Dirección opuesta NE: -X.
        east_tile.m5 = 0;
        map.set_tile(east, east_tile).unwrap();

        assert_eq!(resolve_existing_tunnel_end(&map, west), Some(east));
        assert_eq!(resolve_existing_tunnel_end(&map, east), Some(west));
    }
}
