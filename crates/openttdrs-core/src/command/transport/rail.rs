use crate::map::{Map, TileCoord, TileKind, rail_trackbits_valid_on_slope, tile_slope_and_z};
use crate::pathfinder::{station_entrance_faces_rail, station_site_tile_allows_build};
use crate::rail_signals::{
    RAIL_REMOVE_REFUND, RAIL_TILE_NORMAL, RAIL_TILE_SIGNALS, SIGNAL_BUILD_COST,
    SIGNAL_REMOVE_REFUND, rail_signal_present_mask, rail_signal_state_mask, rail_tile_is_signals,
};
use crate::{DEPOT_BUILD_COST, GameState, RAIL_BUILD_COST};

use super::super::CommandError;
use super::super::terraform::{apply_autoslope_if_needed, check_autoslope_flat};

#[allow(unused_imports)]
use crate::command::transport::internal::{
    bridge_line, check_in_bounds, place_single_transport_tile,
    refresh_track_junction_from_neighbor, trackbits_to_signal_present,
};

pub(crate) fn check_place_rail(map: &Map, c: TileCoord) -> Result<(), CommandError> {
    check_in_bounds(map, c)?;
    match map.get_kind(c).unwrap_or(TileKind::Grass) {
        TileKind::Water => Err(CommandError::CannotPlaceRailOnWater),
        TileKind::Void => Err(CommandError::CannotPlaceRailOnVoid),
        _ => Ok(()),
    }
}

pub(in crate::command::transport) fn existing_rail_trackbits(map: &Map, c: TileCoord) -> u8 {
    map.get(c)
        .filter(|t| t.kind == TileKind::Rail)
        .map_or(0, |t| t.m5 & 0x3F)
}

pub(crate) fn check_rail_trackbits_with_autoslope(
    map: &Map,
    c: TileCoord,
    final_bits: u8,
    tick: u64,
) -> Result<(), CommandError> {
    if check_rail_trackbits_on_tile(map, c, final_bits).is_ok() {
        return Ok(());
    }
    let (tileh, _) = tile_slope_and_z(map, c).ok_or(CommandError::OutOfBounds)?;
    if tileh == 0 {
        return Err(CommandError::InvalidRailOnSlope);
    }
    check_autoslope_flat(map, c, tick)?;
    if !rail_trackbits_valid_on_slope(0, final_bits) {
        return Err(CommandError::InvalidRailOnSlope);
    }
    Ok(())
}

pub(crate) fn check_rail_trackbits_on_tile(
    map: &Map,
    c: TileCoord,
    final_bits: u8,
) -> Result<(), CommandError> {
    let (tileh, _) = tile_slope_and_z(map, c).ok_or(CommandError::OutOfBounds)?;
    if !rail_trackbits_valid_on_slope(tileh, final_bits) {
        return Err(CommandError::InvalidRailOnSlope);
    }
    Ok(())
}

pub(crate) fn merged_rail_trackbits_on_tile(map: &Map, c: TileCoord, add_bits: u8) -> u8 {
    merge_rail_trackbits(existing_rail_trackbits(map, c), add_bits)
}

pub(in crate::command::transport) const MP_RAILWAY_MAPT: u8 = 0x10;

pub(in crate::command::transport) const RAIL_TB_X: u8 = 1;

pub(in crate::command::transport) const RAIL_TB_Y: u8 = 2;

pub(in crate::command::transport) const RAIL_TB_UPPER: u8 = 0x04;

pub(in crate::command::transport) const RAIL_TB_LOWER: u8 = 0x08;

pub(in crate::command::transport) const RAIL_TB_LEFT: u8 = 0x10;

pub(in crate::command::transport) const RAIL_TB_RIGHT: u8 = 0x20;

pub(in crate::command::transport) const RAIL_TB_CROSS: u8 = RAIL_TB_X | RAIL_TB_Y;

pub(in crate::command::transport) const RAIL_TB_HORZ: u8 = RAIL_TB_UPPER | RAIL_TB_LOWER;

pub(in crate::command::transport) const RAIL_TB_VERT: u8 = RAIL_TB_LEFT | RAIL_TB_RIGHT;

pub(in crate::command::transport) const RAIL_PARALLEL_MASK: u8 = 0x3C;

pub(in crate::command::transport) const RAIL_DIAG_MASK: u8 = RAIL_TB_X | RAIL_TB_Y;

pub(in crate::command::transport) fn offset_along_horz_rail(dx: i32, dy: i32) -> bool {
    dy == 0 && dx != 0
}

pub(in crate::command::transport) fn offset_along_vert_rail(dx: i32, dy: i32) -> bool {
    dx == 0 && dy != 0
}

pub(in crate::command::transport) fn connects_rail_network(kind: TileKind) -> bool {
    matches!(
        kind,
        TileKind::Rail
            | TileKind::Station
            | TileKind::RailDepot
            | TileKind::RailTunnel
            | TileKind::RailBridge
    )
}

pub(in crate::command::transport) fn rail_neighbor_connects(
    map: &Map,
    c: TileCoord,
    side: u8,
) -> bool {
    let (dx, dy) = crate::pathfinder::diag_dir_offset(side);
    let n = TileCoord::new(c.x + dx, c.y + dy);
    let Some(t) = map.get(n) else {
        return false;
    };
    if t.kind == TileKind::RailDepot {
        let (mx, my) = crate::pathfinder::diag_dir_offset(t.m5 & 0x03);
        return TileCoord::new(n.x + mx, n.y + my) == c;
    }
    connects_rail_network(t.kind)
}

#[must_use]
pub fn rail_trackbits_from_neighbors(map: &Map, c: TileCoord) -> u8 {
    let mut sides = [false; 4];
    for d in 0..4u8 {
        sides[usize::from(d)] = rail_neighbor_connects(map, c, d);
    }
    match sides.iter().filter(|s| **s).count() {
        // Sin vecinos: eje Y, como el comportamiento histórico del autorraíl.
        0 => RAIL_TB_Y,
        // Un solo vecino: recta a lo largo de ese eje (NE/SW → X, SE/NW → Y).
        1 => {
            if sides[0] || sides[2] {
                RAIL_TB_X
            } else {
                RAIL_TB_Y
            }
        }
        // Cuatro lados: cruce de dos rectas (X|Y) como cuando en `OpenTTD` una
        // línea atraviesa otra; la conexión sigue por la diagonal, sin curvas.
        4 => RAIL_TB_CROSS,
        _ => {
            let mut bits = 0;
            for a in 0..4u8 {
                for b in (a + 1)..4 {
                    if sides[a as usize] && sides[b as usize] {
                        bits |= crate::pathfinder::rail_bit_for_sides(a, b);
                    }
                }
            }
            bits
        }
    }
}

pub(crate) fn normalize_synthetic_rail_crossings(map: &mut Map) {
    const RAIL_TB_ALL: u8 = 0x3F;
    let (mw, mh) = map.dimensions();
    for y in 0..mh.cast_signed() {
        for x in 0..mw.cast_signed() {
            let c = TileCoord::new(x, y);
            let Some(mut t) = map.get(c) else {
                continue;
            };
            let bits5 = t.m5 & 0x3F;
            if t.kind != TileKind::Rail || (bits5 != RAIL_TB_CROSS && bits5 != RAIL_TB_ALL) {
                continue;
            }
            let bits = rail_trackbits_from_neighbors(map, c);
            if bits != bits5 {
                t.m5 = (t.m5 & 0xC0) | bits;
                let _ = map.set_tile(c, t);
            }
        }
    }
}

pub(crate) fn normalize_rail_trackbits_from_neighbors(map: &mut Map) {
    let (mw, mh) = map.dimensions();
    for y in 0..mh.cast_signed() {
        for x in 0..mw.cast_signed() {
            let c = TileCoord::new(x, y);
            let Some(mut t) = map.get(c) else {
                continue;
            };
            if t.kind != TileKind::Rail || t.m5 & 0x3F != 0 {
                continue;
            }
            let bits = rail_trackbits_from_neighbors(map, c);
            if bits == 0 {
                continue;
            }
            t.m5 = (t.m5 & 0xC0) | bits;
            let _ = map.set_tile(c, t);
        }
    }
}

pub(in crate::command::transport) fn is_rail_path_endpoint(map: &Map, c: TileCoord) -> bool {
    let Some(t) = map.get(c) else {
        return false;
    };
    match t.kind {
        TileKind::Rail | TileKind::RailDepot | TileKind::RailTunnel | TileKind::RailBridge => true,
        TileKind::Station => {
            let st = crate::station::station_type_from_m6(t.m6);
            st == 0 || st == crate::station::STATION_TYPE_RAIL_WAYPOINT
        }
        _ => false,
    }
}

pub(in crate::command::transport) fn is_rail_gap_fill_kind(kind: TileKind) -> bool {
    matches!(kind, TileKind::Grass | TileKind::Forest)
}

pub(in crate::command::transport) fn write_rail_gap_tile(
    map: &mut Map,
    c: TileCoord,
    axis_x: bool,
) {
    let Some(mut t) = map.get(c) else {
        return;
    };
    t.kind = TileKind::Rail;
    t.mapt = MP_RAILWAY_MAPT;
    let bits = if axis_x { RAIL_TB_X } else { RAIL_TB_Y };
    t.m5 = (t.m5 & 0xC0) | bits;
    let _ = map.set_tile(c, t);
}

pub(crate) fn bridge_collinear_rail_gaps(map: &mut Map) {
    let (mw, mh) = map.dimensions();
    for y in 0..mh.cast_signed() {
        bridge_line(map, true, y, mw.cast_signed());
    }
    for x in 0..mw.cast_signed() {
        bridge_line(map, false, x, mh.cast_signed());
    }
}

pub(in crate::command::transport) fn write_normal_rail_tile(
    state: &mut GameState,
    c: TileCoord,
    trackbits: u8,
) -> Result<(), CommandError> {
    let mut tile = state.map.get(c).ok_or(CommandError::OutOfBounds)?;
    let tb = trackbits & 0x3F;
    let had_signals = rail_tile_is_signals(tile.m5);
    let old_present = if had_signals {
        rail_signal_present_mask(tile.m3)
    } else {
        0
    };
    let old_states = if had_signals {
        rail_signal_state_mask(tile.m3hi)
    } else {
        0
    };
    let old_m2 = tile.m2;

    tile.kind = TileKind::Rail;
    tile.mapt = MP_RAILWAY_MAPT;
    tile.m1 = 0;
    tile.m2_hi = 0;
    tile.m6 = 0;
    tile.m7 = 0;
    tile.m8 = 0;

    if had_signals {
        let kept = old_present & trackbits_to_signal_present(tb);
        if kept != 0 {
            tile.m5 = tb | (RAIL_TILE_SIGNALS << 6);
            tile.m2 = old_m2;
            tile.m3 = (tile.m3 & 0x0F) | (kept << 4);
            tile.m3hi = (tile.m3hi & 0x0F) | ((old_states & kept) << 4);
            return state
                .map
                .set_tile(c, tile)
                .map_err(|_| CommandError::OutOfBounds);
        }
    }

    tile.m5 = tb | (RAIL_TILE_NORMAL << 6);
    tile.m2 = 0;
    tile.m3 = 0;
    tile.m3hi = 0;
    state
        .map
        .set_tile(c, tile)
        .map_err(|_| CommandError::OutOfBounds)
}

pub(in crate::command::transport) fn refresh_rail_trackbits(
    state: &mut GameState,
    c: TileCoord,
) -> Result<(), CommandError> {
    if !state.map.get_kind(c).is_some_and(|k| k == TileKind::Rail) {
        return Ok(());
    }
    let tile = state.map.get(c).ok_or(CommandError::OutOfBounds)?;
    let current = tile.m5 & 0x3F;
    // Carriles paralelos sin diagonal: no re-inferir con `from_neighbors` (destruye líneas).
    if current != 0 && (current & RAIL_PARALLEL_MASK) != 0 && (current & RAIL_DIAG_MASK) == 0 {
        return Ok(());
    }
    let tb = rail_trackbits_from_neighbors(&state.map, c);
    let mut out = tile;
    out.mapt = MP_RAILWAY_MAPT;
    out.m5 = (tb & 0x3F) | (out.m5 & 0xC0);
    state
        .map
        .set_tile(c, out)
        .map_err(|_| CommandError::OutOfBounds)
}

pub(in crate::command::transport) fn refresh_rail_neighbors_after_place(
    state: &mut GameState,
    changed: TileCoord,
) -> Result<(), CommandError> {
    let changed_tb = existing_rail_trackbits(&state.map, changed);
    if changed_tb == 0 {
        return Ok(());
    }
    for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
        let n = TileCoord::new(changed.x + dx, changed.y + dy);
        refresh_track_junction_from_neighbor(state, n, changed, changed_tb)?;
    }
    Ok(())
}

pub(in crate::command::transport) fn refresh_rail_neighbors(
    state: &mut GameState,
    changed: TileCoord,
) -> Result<(), CommandError> {
    refresh_rail_neighbors_after_place(state, changed)?;
    for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
        refresh_rail_trackbits(state, TileCoord::new(changed.x + dx, changed.y + dy))?;
    }
    Ok(())
}

pub(crate) fn check_rail_depot_placement(
    map: &Map,
    c: TileCoord,
    dir: u8,
) -> Result<(), CommandError> {
    check_in_bounds(map, c)?;
    match map.get_kind(c).unwrap_or(TileKind::Grass) {
        TileKind::Water => Err(CommandError::CannotPlaceRailOnWater),
        TileKind::Void => Err(CommandError::CannotPlaceRailOnVoid),
        k if !station_site_tile_allows_build(k) => {
            Err(CommandError::CannotPlaceStationOnOccupiedTile)
        }
        _ => {
            if station_entrance_faces_rail(map, c, dir & 0x03) {
                Ok(())
            } else {
                Err(CommandError::StationNotAdjacentToTransport)
            }
        }
    }
}

pub(in crate::command) fn place_rail_depot_dir(
    state: &mut GameState,
    c: TileCoord,
    dir: u8,
) -> Result<(), CommandError> {
    let dir = dir & 0x03;
    check_rail_depot_placement(&state.map, c, dir)?;
    place_single_transport_tile(
        state,
        c,
        TileKind::RailDepot,
        0x10,
        (2 << 6) | dir,
        DEPOT_BUILD_COST,
    )?;
    // La tesela de salida gana las piezas de empalme (curvas/recta) que
    // conectan la boca del depósito con la vía existente, como en OpenTTD.
    if let Some((exit, _)) = rail_depot_exit_for_dir(&state.map, c, dir)
        && state.map.get_kind(exit) == Some(TileKind::Rail)
    {
        let junction_bits = rail_trackbits_from_neighbors(&state.map, exit);
        let _ = place_rail_bits(state, exit, junction_bits);
    }
    Ok(())
}

pub(in crate::command) fn rail_depot_exit_for_dir(
    map: &Map,
    depot_pos: TileCoord,
    dir: u8,
) -> Option<(TileCoord, u8)> {
    let ((dx, dy), track_bit) = match dir & 0x03 {
        0 => ((-1_i32, 0_i32), RAIL_TB_X),
        1 => ((0_i32, 1_i32), RAIL_TB_Y),
        2 => ((1_i32, 0_i32), RAIL_TB_X),
        _ => ((0_i32, -1_i32), RAIL_TB_Y),
    };
    let c = TileCoord::new(depot_pos.x + dx, depot_pos.y + dy);
    let (mw, mh) = map.dimensions();
    if c.x < 0 || c.y < 0 || c.x >= mw.cast_signed() || c.y >= mh.cast_signed() {
        return None;
    }
    Some((c, track_bit))
}

pub(in crate::command::transport) fn rail_axis_y_from_trackbits(tb: u8) -> bool {
    let tb = tb & 0x3F;
    if tb & RAIL_TB_X != 0 && tb & RAIL_TB_Y == 0 {
        return false;
    }
    if tb & RAIL_TB_Y != 0 && tb & RAIL_TB_X == 0 {
        return true;
    }
    true
}

pub(in crate::command::transport) fn merge_rail_trackbits(existing: u8, add: u8) -> u8 {
    let merged = (existing | (add & 0x3F)) & 0x3F;
    if merged == 0 { add & 0x3F } else { merged }
}

#[must_use]
pub fn rail_bits_placement_target(map: &Map, c: TileCoord, add_bits: u8) -> (TileCoord, u8) {
    (c, merged_rail_trackbits_on_tile(map, c, add_bits))
}

pub(in crate::command) fn place_rail_bits(
    state: &mut GameState,
    c: TileCoord,
    bits: u8,
) -> Result<(), CommandError> {
    check_place_rail(&state.map, c)?;
    let add = bits & 0x3F;
    apply_autoslope_if_needed(state, c)?;
    let tb = merged_rail_trackbits_on_tile(&state.map, c, add);
    check_rail_trackbits_on_tile(&state.map, c, tb)?;
    write_normal_rail_tile(state, c, tb)?;
    state.economy.money -= RAIL_BUILD_COST;
    Ok(())
}

pub(in crate::command) fn set_rail_bits(
    state: &mut GameState,
    c: TileCoord,
    bits: u8,
) -> Result<(), CommandError> {
    check_place_rail(&state.map, c)?;
    apply_autoslope_if_needed(state, c)?;
    let tb = (bits & 0x3F).max(RAIL_TB_X);
    check_rail_trackbits_on_tile(&state.map, c, tb)?;
    write_normal_rail_tile(state, c, tb)?;
    refresh_rail_neighbors(state, c)?;
    state.economy.money -= RAIL_BUILD_COST;
    Ok(())
}

pub(in crate::command) fn place_rail(
    state: &mut GameState,
    c: TileCoord,
) -> Result<(), CommandError> {
    check_place_rail(&state.map, c)?;
    apply_autoslope_if_needed(state, c)?;
    let tb = rail_trackbits_from_neighbors(&state.map, c);
    check_rail_trackbits_on_tile(&state.map, c, tb)?;
    write_normal_rail_tile(state, c, tb)?;
    refresh_rail_neighbors(state, c)?;
    state.economy.money -= RAIL_BUILD_COST;
    Ok(())
}

pub(crate) fn check_remove_rail(map: &Map, c: TileCoord) -> Result<(), CommandError> {
    check_in_bounds(map, c)?;
    let Some(tile) = map.get(c) else {
        return Err(CommandError::OutOfBounds);
    };
    if tile.kind != TileKind::Rail {
        return Err(CommandError::NoRailToRemove);
    }
    let subtype = (tile.m5 >> 6) & 0x3;
    if subtype != RAIL_TILE_NORMAL && subtype != RAIL_TILE_SIGNALS {
        return Err(CommandError::NoRailToRemove);
    }
    Ok(())
}

pub(crate) fn check_place_rail_signal_oriented(
    map: &Map,
    c: TileCoord,
    orientation: u8,
    fract_x: u8,
    fract_y: u8,
) -> Result<(), CommandError> {
    let tb = map
        .get(c)
        .filter(|t| t.kind == TileKind::Rail)
        .map_or(0, |t| t.m5 & 0x3F);
    let Some(track) = crate::rail_signals::resolve_signal_track(tb, fract_x, fract_y) else {
        return Err(CommandError::CannotPlaceSignalOnTrack);
    };
    let face = crate::rail_signals::signal_facing_for_orientation(track, orientation);
    check_place_rail_signal(map, c, track, face)
}

pub(crate) fn check_place_rail_signal(
    map: &Map,
    c: TileCoord,
    track: crate::rail_signals::SignalTrack,
    face: u8,
) -> Result<(), CommandError> {
    check_in_bounds(map, c)?;
    let Some(tile) = map.get(c) else {
        return Err(CommandError::OutOfBounds);
    };
    if tile.kind != TileKind::Rail {
        return Err(CommandError::CannotPlaceSignalOnTrack);
    }
    let tb = tile.m5 & 0x3F;
    if tb & track.track_bit() == 0 || crate::rail_signals::tracks_overlap(tb) {
        return Err(CommandError::CannotPlaceSignalOnTrack);
    }
    let Some(placement) = crate::rail_signals::signal_placement_for_track(
        track,
        face,
        crate::rail_signals::default_signal_variant(crate::news::CALENDAR_BASE_YEAR),
    ) else {
        return Err(CommandError::CannotPlaceSignalOnTrack);
    };
    if rail_tile_is_signals(tile.m5) {
        let present = crate::rail_signals::rail_signal_present_mask(tile.m3);
        if present & (1 << placement.sig_bit) != 0 {
            // Clic repetido en la misma dirección: quitar señal (OpenTTD).
            return Ok(());
        }
    }
    Ok(())
}

pub(in crate::command::transport) fn clear_rail_tile_to_grass(
    state: &mut GameState,
    c: TileCoord,
) -> Result<(), CommandError> {
    state
        .map
        .set_kind(c, TileKind::Grass)
        .map_err(|_| CommandError::OutOfBounds)?;
    state
        .map
        .set_mapt_m5(c, 0x00, 0x00)
        .map_err(|_| CommandError::OutOfBounds)?;
    Ok(())
}

pub(in crate::command) fn remove_rail_bits(
    state: &mut GameState,
    c: TileCoord,
    bits: u8,
) -> Result<(), CommandError> {
    check_remove_rail(&state.map, c)?;
    let tile = state.map.get(c).ok_or(CommandError::OutOfBounds)?;
    let existing = tile.m5 & 0x3F;
    let remove = bits & 0x3F;
    let new_tb = if remove == 0x3F {
        0
    } else {
        existing & !remove
    };
    if new_tb == 0 {
        clear_rail_tile_to_grass(state, c)?;
    } else {
        let subtype = (tile.m5 >> 6) & 0x3;
        let mut out = tile;
        out.m5 = new_tb | ((subtype & 0x3) << 6);
        if rail_tile_is_signals(out.m5) {
            let present = crate::rail_signals::rail_signal_present_mask(out.m3);
            let kept = present & trackbits_to_signal_present(new_tb);
            out.m3 = (out.m3 & 0x0F) | (kept << 4);
            out.m3hi = (out.m3hi & 0x0F) | (kept << 4);
        }
        state
            .map
            .set_tile(c, out)
            .map_err(|_| CommandError::OutOfBounds)?;
    }
    refresh_rail_neighbors(state, c)?;
    state.economy.money += RAIL_REMOVE_REFUND;
    Ok(())
}

pub(in crate::command) fn remove_rail(
    state: &mut GameState,
    c: TileCoord,
) -> Result<(), CommandError> {
    remove_rail_bits(state, c, 0x3F)
}

pub(in crate::command) fn place_rail_signal(
    state: &mut GameState,
    c: TileCoord,
    orientation: u8,
    fract_x: u8,
    fract_y: u8,
) -> Result<(), CommandError> {
    let tile = state.map.get(c).ok_or(CommandError::OutOfBounds)?;
    let tb = tile.m5 & 0x3F;
    let track = crate::rail_signals::resolve_signal_track(tb, fract_x, fract_y)
        .ok_or(CommandError::CannotPlaceSignalOnTrack)?;
    let face = crate::rail_signals::signal_facing_for_orientation(track, orientation);
    check_place_rail_signal(&state.map, c, track, face)?;
    let year = crate::rail_signals::calendar_year_at_tick(state.tick);
    let variant = crate::rail_signals::default_signal_variant(year);
    let placement = crate::rail_signals::signal_placement_for_track(track, face, variant)
        .ok_or(CommandError::CannotPlaceSignalOnTrack)?;
    if rail_tile_is_signals(tile.m5) {
        let present = crate::rail_signals::rail_signal_present_mask(tile.m3);
        if present & (1 << placement.sig_bit) != 0 {
            return remove_rail_signal_bit(state, c, placement.sig_bit);
        }
    }
    let mut out = tile;
    if rail_tile_is_signals(out.m5) {
        let present = crate::rail_signals::rail_signal_present_mask(out.m3);
        let merged = present | (1 << placement.sig_bit);
        out.m3 = (out.m3 & 0x0F) | (merged << 4);
        out.m3hi = (out.m3hi & 0x0F) | (merged << 4);
        out.m2 |= placement.m2;
    } else {
        out.m5 = tb | (RAIL_TILE_SIGNALS << 6);
        out.m2 = placement.m2;
        out.m3 = placement.m3;
        out.m3hi = placement.m3hi;
    }
    state
        .map
        .set_tile(c, out)
        .map_err(|_| CommandError::OutOfBounds)?;
    state.economy.money -= SIGNAL_BUILD_COST;
    Ok(())
}

pub(in crate::command::transport) fn remove_rail_signal_bit(
    state: &mut GameState,
    c: TileCoord,
    sig_bit: u8,
) -> Result<(), CommandError> {
    let tile = state.map.get(c).ok_or(CommandError::OutOfBounds)?;
    if !rail_tile_is_signals(tile.m5) {
        return Err(CommandError::CannotPlaceSignalOnTrack);
    }
    let present = crate::rail_signals::rail_signal_present_mask(tile.m3);
    if present & (1 << sig_bit) == 0 {
        return Err(CommandError::CannotPlaceSignalOnTrack);
    }
    let mut out = tile;
    let new_present = present & !(1 << sig_bit);
    if new_present == 0 {
        let tb = out.m5 & 0x3F;
        out.m5 = tb | (RAIL_TILE_NORMAL << 6);
        out.m2 = 0;
        out.m3 &= 0x0F;
        out.m3hi &= 0x0F;
    } else {
        out.m3 = (out.m3 & 0x0F) | (new_present << 4);
        out.m3hi = (out.m3hi & 0x0F) | (new_present << 4);
    }
    state
        .map
        .set_tile(c, out)
        .map_err(|_| CommandError::OutOfBounds)?;
    state.economy.money += SIGNAL_REMOVE_REFUND;
    Ok(())
}
