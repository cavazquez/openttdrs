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

#[inline]
fn slope_bits_from_corner_vals(hnorth: u8, hwest: u8, heast: u8, hsouth: u8) -> (u8, u8) {
    let min_h = hnorth.min(hwest).min(heast).min(hsouth);
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
    (tileh.min(14), min_h)
}

#[inline]
fn corner_height(map: &Map, cx: i32, cy: i32) -> u8 {
    map.get(TileCoord::new(cx, cy)).map_or(0, |t| t.height)
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

/// `m5` de boca de túnel: dirección en bits 0–1 + tipo de transporte en bits 2–3.
#[must_use]
pub fn tunnel_entrance_m5(tileh: u8, rail: bool) -> Option<u8> {
    let dir = inclined_slope_direction(tileh)?;
    Some(dir | if rail { 0x04 } else { 0 })
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

/// Offset de tesela en dirección diagonal (`TileOffsByDiagDir`).
#[must_use]
pub const fn diag_dir_offset(dir: u8) -> (i32, i32) {
    const OFFSETS: [(i32, i32); 4] = [(-1, 0), (0, 1), (1, 0), (0, -1)];
    OFFSETS[dir as usize & 3]
}

/// Otra entrada del túnel si se construyera desde `start` (`GetOtherTunnelEnd`).
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
}
