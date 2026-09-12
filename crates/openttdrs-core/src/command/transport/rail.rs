use crate::GameState;
use crate::economy::{
    rail_build_cost_factored, rail_convert_cost, signal_build_cost, signal_clear_cost,
    train_depot_build_cost,
};
use crate::map::{
    Map, TileCoord, TileKind, opposite_diag_dir, rail_bit_for_sides, rail_bits_touching_side,
    rail_trackbits_valid_on_slope, resolve_existing_tunnel_end, tile_slope_and_z,
};
use crate::pathfinder::{station_entrance_faces_rail, station_site_tile_allows_build};
use crate::rail_signals::{
    RAIL_REMOVE_REFUND, RAIL_TILE_NORMAL, RAIL_TILE_SIGNALS, rail_signal_present_mask,
    rail_signal_state_mask, rail_tile_is_signals,
};

use super::super::terraform::{apply_autoslope_if_needed, check_autoslope_flat};
use super::super::{CommandError, require_tile_owned_by_active};
use super::shared::check_object_can_be_auto_cleared;

#[allow(unused_imports)]
use crate::command::transport::internal::{
    axis_line, check_in_bounds, place_single_transport_tile,
    place_single_transport_tile_with_depot_id, propagate_rail_diag_to_neighbors,
    refresh_track_junction_from_neighbor, register_depot, trackbits_to_signal_present,
};

pub(crate) fn check_place_rail(map: &Map, c: TileCoord) -> Result<(), CommandError> {
    check_in_bounds(map, c)?;
    match map.get_kind(c).unwrap_or(TileKind::Grass) {
        TileKind::Water => Err(CommandError::CannotPlaceRailOnWater),
        TileKind::Void => Err(CommandError::CannotPlaceRailOnVoid),
        // No pisar estaciones: dejaba label `Tren` + tile Rail CROSS (#193).
        TileKind::Station => Err(CommandError::CannotPlaceStationOnOccupiedTile),
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
    inflation_prices: u64,
) -> Result<(), CommandError> {
    if check_rail_trackbits_on_tile(map, c, final_bits).is_ok() {
        return Ok(());
    }
    let (tileh, _) = tile_slope_and_z(map, c).ok_or(CommandError::OutOfBounds)?;
    if tileh == 0 {
        return Err(CommandError::InvalidRailOnSlope);
    }
    check_autoslope_flat(map, c, inflation_prices)?;
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

pub(in crate::command::transport) use crate::map::{
    RAIL_TB_CROSS, RAIL_TB_HORZ, RAIL_TB_VERT, RAIL_TB_X, RAIL_TB_Y,
};

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
                        bits |= crate::map::rail_bit_for_sides(a, b);
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

pub(in crate::command::transport) fn write_normal_rail_tile(
    state: &mut GameState,
    c: TileCoord,
    trackbits: u8,
) -> Result<(), CommandError> {
    require_tile_owned_by_active(state, c)?;
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
    let was_rail = matches!(
        tile.kind,
        TileKind::Rail | TileKind::RailDepot | TileKind::RailTunnel | TileKind::RailBridge
    );
    let keep_m8 = tile.m8;

    tile.kind = TileKind::Rail;
    tile.mapt = MP_RAILWAY_MAPT;
    tile.m1 = state.active_company.0;
    tile.m2_hi = 0;
    tile.m6 = 0;
    tile.m7 = 0;
    // Conservar railtype existente; vía nueva → tipo activo del jugador.
    tile.m8 = if was_rail {
        keep_m8
    } else {
        crate::rail_type::set_rail_type_on_tile(tile, state.current_rail_type).m8
    };

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
    // Una sola curva (codo L, con o sin un eje ajeno): no re-inferir.
    // Con 3–4 vecinos `from_neighbors` lo convertiría en CROSS y rompería el giro.
    if current != 0 && (current & RAIL_PARALLEL_MASK).is_power_of_two() {
        return Ok(());
    }
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
    check_object_can_be_auto_cleared(state, c)?;
    let rail_cost_multiplier =
        state.runtime.rail_type_props[usize::from(state.current_rail_type.as_u8())].cost_multiplier;
    let connection = rail_depot_connection(&state.map, c, dir);
    if let Some((exit, before, after)) = connection
        && before != after
    {
        require_tile_owned_by_active(state, exit)?;
        check_rail_trackbits_on_tile(&state.map, exit, after)?;
    }
    let depot_id =
        crate::depot::next_free_depot_id(&state.map).ok_or(CommandError::DepotPoolFull)?;
    place_single_transport_tile_with_depot_id(
        state,
        c,
        TileKind::RailDepot,
        0x10,
        (2 << 6) | dir,
        train_depot_build_cost(&state.global_economy, rail_cost_multiplier),
        depot_id,
    )?;
    let depot_tile = state.map.get(c).ok_or(CommandError::OutOfBounds)?;
    let depot_tile = crate::rail_type::set_rail_type_on_tile(depot_tile, state.current_rail_type);
    state
        .map
        .set_tile(c, depot_tile)
        .map_err(|_| CommandError::OutOfBounds)?;
    if let Some((exit, before, after)) = connection
        && before != after
    {
        // El empalme automático afecta sólo a la salida. No debe ejecutar la
        // propagación de autorraíl, que contaminaría líneas paralelas vecinas.
        write_normal_rail_tile(state, exit, after)?;
    }
    register_depot(state, depot_id, c);
    Ok(())
}

fn charge_rail_build(state: &mut GameState) {
    let mult =
        state.runtime.rail_type_props[usize::from(state.current_rail_type.as_u8())].cost_multiplier;
    state.economy.money -= rail_build_cost_factored(&state.global_economy, mult);
}

fn rail_depot_connection(map: &Map, depot: TileCoord, dir: u8) -> Option<(TileCoord, u8, u8)> {
    let (dx, dy) = crate::pathfinder::diag_dir_offset(dir);
    let exit = TileCoord::new(depot.x + dx, depot.y + dy);
    let existing = existing_rail_trackbits(map, exit);
    if existing == 0 {
        return None;
    }

    let depot_side = opposite_diag_dir(dir);
    if existing & rail_bits_touching_side(depot_side) != 0 {
        return Some((exit, existing, existing));
    }

    let mut add = 0;
    for network_side in 0..4 {
        if network_side != depot_side && existing & rail_bits_touching_side(network_side) != 0 {
            add |= rail_bit_for_sides(depot_side, network_side);
        }
    }
    Some((exit, existing, merge_rail_trackbits(existing, add)))
}

pub(in crate::command::transport) fn rail_axis_y_from_trackbits(tb: u8) -> bool {
    let tb = tb & 0x3F;
    if tb & RAIL_TB_X != 0 && tb & RAIL_TB_Y == 0 {
        return false;
    }
    if tb & RAIL_TB_Y != 0 && tb & RAIL_TB_X == 0 {
        return true;
    }
    // CROSS / ambiguo: sin preferencia (el caller debe usar `dir`).
    true
}

/// Eje inequívoco desde trackbits (`Some` solo si hay un solo eje X/Y).
#[must_use]
pub(in crate::command::transport) fn rail_axis_y_unambiguous(tb: u8) -> Option<bool> {
    let tb = tb & 0x3F;
    let has_x = tb & RAIL_TB_X != 0;
    let has_y = tb & RAIL_TB_Y != 0;
    match (has_x, has_y) {
        (true, false) => Some(false),
        (false, true) => Some(true),
        _ => None,
    }
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
    check_object_can_be_auto_cleared(state, c)?;
    let add = bits & 0x3F;
    apply_autoslope_if_needed(state, c)?;
    let tb = merged_rail_trackbits_on_tile(&state.map, c, add);
    check_rail_trackbits_on_tile(&state.map, c, tb)?;
    write_normal_rail_tile(state, c, tb)?;
    if (add & RAIL_DIAG_MASK) != 0 {
        propagate_rail_diag_to_neighbors(state, c, add)?;
    } else if (add & RAIL_PARALLEL_MASK) != 0 {
        refresh_rail_neighbors_after_place(state, c)?;
    }
    charge_rail_build(state);
    Ok(())
}

pub(in crate::command) fn set_rail_bits(
    state: &mut GameState,
    c: TileCoord,
    bits: u8,
) -> Result<(), CommandError> {
    check_place_rail(&state.map, c)?;
    check_object_can_be_auto_cleared(state, c)?;
    apply_autoslope_if_needed(state, c)?;
    let tb = (bits & 0x3F).max(RAIL_TB_X);
    check_rail_trackbits_on_tile(&state.map, c, tb)?;
    write_normal_rail_tile(state, c, tb)?;
    refresh_rail_neighbors(state, c)?;
    charge_rail_build(state);
    Ok(())
}

pub(in crate::command) fn place_rail(
    state: &mut GameState,
    c: TileCoord,
) -> Result<(), CommandError> {
    check_place_rail(&state.map, c)?;
    check_object_can_be_auto_cleared(state, c)?;
    apply_autoslope_if_needed(state, c)?;
    let tb = rail_trackbits_from_neighbors(&state.map, c);
    check_rail_trackbits_on_tile(&state.map, c, tb)?;
    write_normal_rail_tile(state, c, tb)?;
    refresh_rail_neighbors(state, c)?;
    charge_rail_build(state);
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
        crate::rail_signals::SIGTYPE_BLOCK,
    ) else {
        return Err(CommandError::CannotPlaceSignalOnTrack);
    };
    if rail_tile_is_signals(tile.m5) {
        let present = crate::rail_signals::rail_signal_present_mask(tile.m3);
        if present & (1 << placement.sig_bit) != 0
            || present & crate::rail_signals::signal_on_track_mask(track) != 0
        {
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
    require_tile_owned_by_active(state, c)?;
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

fn rail_tunnel_other_end(map: &Map, c: TileCoord) -> Option<TileCoord> {
    let tile = map.get(c)?;
    if tile.kind != TileKind::RailTunnel
        || !tile.is_tunnel_bridge_tile()
        || tile.m5 & 0x80 != 0
        || tile.m5 & 0x0C != 0
    {
        return None;
    }
    let (tileh, _) = tile_slope_and_z(map, c)?;
    crate::map::inclined_slope_direction(tileh)?;

    let other = crate::pathfinder::tunnel_other_end(map, c, TileKind::RailTunnel)
        .or_else(|| resolve_existing_tunnel_end(map, c))?;
    let other_tile = map.get(other)?;
    (other_tile.kind == TileKind::RailTunnel
        && other_tile.is_tunnel_bridge_tile()
        && other_tile.m5 & 0x80 == 0
        && other_tile.m5 & 0x0C == 0)
        .then_some(other)
}

fn rail_portal_other_end(map: &Map, c: TileCoord) -> Option<TileCoord> {
    match map.get_kind(c) {
        Some(TileKind::RailTunnel) => rail_tunnel_other_end(map, c),
        Some(TileKind::RailBridge) => {
            let tile = map.get(c)?;
            if !tile.is_tunnel_bridge_tile() || tile.m5 & 0x80 == 0 || tile.m5 & 0x0C != 0 {
                return None;
            }
            let other = crate::rail_bridge_other_end(map, c)?;
            let other_tile = map.get(other)?;
            (other_tile.kind == TileKind::RailBridge
                && other_tile.is_tunnel_bridge_tile()
                && other_tile.m5 & 0x80 != 0
                && other_tile.m5 & 0x0C == 0)
                .then_some(other)
        }
        _ => None,
    }
}

fn rail_conversion_endpoints(map: &Map, c: TileCoord) -> Result<[TileCoord; 2], CommandError> {
    let tile = map.get(c).ok_or(CommandError::OutOfBounds)?;
    match tile.kind {
        TileKind::Rail | TileKind::RailDepot => Ok([c, c]),
        TileKind::Station
            if crate::station::is_rail_station_type(crate::station::station_type_from_m6(
                tile.m6,
            )) =>
        {
            Ok([c, c])
        }
        TileKind::Road if crate::map::is_road_level_crossing(tile.mapt, tile.m5, tile.kind) => {
            Ok([c, c])
        }
        TileKind::RailTunnel | TileKind::RailBridge => rail_portal_other_end(map, c)
            .map(|other| [c, other])
            .ok_or(CommandError::NoRailToConvert),
        _ => Err(CommandError::NoRailToConvert),
    }
}

fn rail_conversion_cost(
    state: &GameState,
    c: TileCoord,
    endpoints: [TileCoord; 2],
    from: crate::rail_type::RailType,
    to: crate::rail_type::RailType,
) -> i64 {
    let Some(tile) = state.map.get(c) else {
        return 0;
    };
    let units = if endpoints[1] != endpoints[0] {
        i64::try_from(axis_line(c, endpoints[1]).len()).unwrap_or(i64::MAX)
    } else if tile.kind == TileKind::Rail {
        i64::from((tile.m5 & 0x3F).count_ones())
    } else {
        1
    };
    rail_convert_cost(
        &state.global_economy,
        from,
        to,
        &state.runtime.rail_type_props,
    )
    .saturating_mul(units)
}

pub(in crate::command) fn check_convert_rail(
    state: &GameState,
    c: TileCoord,
    to_type: crate::rail_type::RailType,
) -> Result<(), CommandError> {
    check_in_bounds(&state.map, c)?;
    let endpoints = rail_conversion_endpoints(&state.map, c)?;
    require_tile_owned_by_active(state, endpoints[0])?;
    if endpoints[1] != endpoints[0] {
        require_tile_owned_by_active(state, endpoints[1])?;
    }

    let tile = state.map.get(c).ok_or(CommandError::OutOfBounds)?;
    let current = crate::rail_type::rail_type_from_tile(tile);
    if current == to_type
        || (state.construction.disable_elrails
            && current == crate::rail_type::RailType::Electric
            && to_type == crate::rail_type::RailType::Rail)
    {
        return Ok(());
    }

    let is_portal = endpoints[1] != endpoints[0];
    let compatible = crate::rail_type::rail_types_compatible_with_props(
        current,
        to_type,
        &state.runtime.rail_type_props,
    );
    if is_portal {
        // `TunnelBridgeIsFree` is deliberately checked only for conversions
        // between incompatible rail networks. Rail <-> electric may happen
        // while a train is on the portal, just like in OpenTTD.
        if !compatible
            && state.vehicles.iter().any(|vehicle| {
                endpoints.contains(&vehicle.pos)
                    && matches!(
                        vehicle.kind,
                        crate::vehicle::VehicleKind::Train
                            | crate::vehicle::VehicleKind::Truck
                            | crate::vehicle::VehicleKind::Bus
                            | crate::vehicle::VehicleKind::Tram
                            | crate::vehicle::VehicleKind::Ship
                    )
            })
        {
            return Err(CommandError::VehicleInTheWay);
        }
    } else {
        if tile.kind == TileKind::Rail
            && state.vehicles.iter().any(|vehicle| {
                vehicle.pos == c && train_incompatible_with_rail_type(state, vehicle, to_type)
            })
        {
            // No convertir si un tren en la tesela quedaría incompatible con
            // el nuevo tipo; se conserva el error específico de la orden plana.
            return Err(CommandError::TrainIncompatibleWithRailType);
        }
        if !compatible
            && tile.kind != TileKind::Rail
            && state.vehicles.iter().any(|vehicle| {
                vehicle.pos == c && !matches!(vehicle.kind, crate::vehicle::VehicleKind::Aircraft)
            })
        {
            return Err(CommandError::VehicleInTheWay);
        }
    }

    let conversion_cost = rail_conversion_cost(state, c, endpoints, current, to_type);
    if state.economy.money < conversion_cost {
        return Err(CommandError::InsufficientFunds);
    }
    Ok(())
}

fn train_incompatible_with_rail_type(
    state: &GameState,
    vehicle: &crate::vehicle::Vehicle,
    to_type: crate::rail_type::RailType,
) -> bool {
    vehicle.kind == crate::vehicle::VehicleKind::Train
        && vehicle.engine_id.is_some_and(|engine_id| {
            let required = crate::rail_type::required_rail_type_for_engine(engine_id);
            !(required == to_type
                || crate::rail_type::rail_types_compatible_with_props(
                    required,
                    to_type,
                    &state.runtime.rail_type_props,
                ))
        })
}

/// Convierte el tipo de vía de una tesela (`CmdConvertRail`).
pub(in crate::command) fn convert_rail(
    state: &mut GameState,
    c: TileCoord,
    to_type: crate::rail_type::RailType,
) -> Result<(), CommandError> {
    check_convert_rail(state, c, to_type)?;
    let endpoints = rail_conversion_endpoints(&state.map, c)?;
    let tile = state.map.get(c).ok_or(CommandError::OutOfBounds)?;
    let current = crate::rail_type::rail_type_from_tile(tile);
    if current == to_type {
        return Ok(());
    }
    // OpenTTD treats an existing electric tile as already compatible with
    // normal rail while `vehicle.disable_elrails` is active; converting it to
    // Rail is therefore a no-op rather than a paid map mutation.
    if state.construction.disable_elrails
        && current == crate::rail_type::RailType::Electric
        && to_type == crate::rail_type::RailType::Rail
    {
        return Ok(());
    }

    let is_portal = endpoints[1] != endpoints[0];
    let conversion_cost = rail_conversion_cost(state, c, endpoints, current, to_type);

    if is_portal {
        // The reservation is attached to the first portal in the native map
        // format. It is enough to inspect the corresponding track and release
        // the owning consist before changing its power contract.
        if let Some(track) = crate::bridge_spec::tunnel_bridge_rail_track(tile) {
            let reservation_owner = state.vehicles.iter().position(|vehicle| {
                vehicle.kind == crate::vehicle::VehicleKind::Train
                    && vehicle.is_consist_head()
                    && train_incompatible_with_rail_type(state, vehicle, to_type)
                    && vehicle
                        .reserved_steps
                        .iter()
                        .any(|step| endpoints.contains(&step.tile) && step.track & track != 0)
            });
            if let Some(index) = reservation_owner {
                crate::rail_pbs::free_train_track_reservation(
                    &mut state.map,
                    &mut state.vehicles[index],
                    &mut state.runtime.reservation_tile_dirty,
                );
            }
        }
    } else if let Some(index) = state.vehicles.iter().position(|vehicle| {
        vehicle.kind == crate::vehicle::VehicleKind::Train
            && vehicle.is_consist_head()
            && train_incompatible_with_rail_type(state, vehicle, to_type)
            && vehicle.reserved_steps.iter().any(|step| step.tile == c)
    }) {
        crate::rail_pbs::free_train_track_reservation(
            &mut state.map,
            &mut state.vehicles[index],
            &mut state.runtime.reservation_tile_dirty,
        );
    }

    let endpoint_count = if is_portal { 2 } else { 1 };
    for endpoint in endpoints.iter().take(endpoint_count).copied() {
        let endpoint_tile = state.map.get(endpoint).ok_or(CommandError::OutOfBounds)?;
        let out = crate::rail_type::set_rail_type_on_tile(endpoint_tile, to_type);
        state
            .map
            .set_tile(endpoint, out)
            .map_err(|_| CommandError::OutOfBounds)?;
    }
    state.economy.money -= conversion_cost;
    Ok(())
}

pub(in crate::command) fn place_rail_signal(
    state: &mut GameState,
    c: TileCoord,
    orientation: u8,
    fract_x: u8,
    fract_y: u8,
    sig_type: u8,
    variant: u8,
) -> Result<(), CommandError> {
    require_tile_owned_by_active(state, c)?;
    let tile = state.map.get(c).ok_or(CommandError::OutOfBounds)?;
    let tb = tile.m5 & 0x3F;
    let track = crate::rail_signals::resolve_signal_track(tb, fract_x, fract_y)
        .ok_or(CommandError::CannotPlaceSignalOnTrack)?;
    let face = crate::rail_signals::signal_facing_for_orientation(track, orientation);
    check_place_rail_signal(&state.map, c, track, face)?;
    let year = crate::rail_signals::calendar_year_at_tick(state.tick);
    let variant = if variant <= 1 {
        variant
    } else {
        crate::rail_signals::default_signal_variant(year)
    };
    let placement_sig_type = match sig_type {
        crate::rail_signals::SIGTYPE_BLOCK
        | crate::rail_signals::SIGTYPE_ENTRY
        | crate::rail_signals::SIGTYPE_EXIT
        | crate::rail_signals::SIGTYPE_COMBO
        | crate::rail_signals::SIGTYPE_PATH
        | crate::rail_signals::SIGTYPE_PATH_ONEWAY => sig_type,
        _ => crate::rail_signals::SIGTYPE_BLOCK,
    };
    let placement =
        crate::rail_signals::signal_placement_for_track(track, face, variant, placement_sig_type)
            .ok_or(CommandError::CannotPlaceSignalOnTrack)?;
    if rail_tile_is_signals(tile.m5) {
        let present = crate::rail_signals::rail_signal_present_mask(tile.m3);
        if present & crate::rail_signals::signal_on_track_mask(track) != 0 {
            return cycle_rail_signal_side(state, c, track);
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
    crate::rail_signals::enqueue_signal_glob(&mut state.runtime.signal_globset, c);
    state.economy.money -= signal_build_cost(&state.global_economy);
    Ok(())
}

pub(in crate::command) fn cycle_rail_signal_type(
    state: &mut GameState,
    c: TileCoord,
    fract_x: u8,
    fract_y: u8,
) -> Result<(), CommandError> {
    require_tile_owned_by_active(state, c)?;
    let tile = state.map.get(c).ok_or(CommandError::OutOfBounds)?;
    if !rail_tile_is_signals(tile.m5) {
        return Err(CommandError::CannotPlaceSignalOnTrack);
    }
    let tb = tile.m5 & 0x3F;
    let track = crate::rail_signals::resolve_signal_track(tb, fract_x, fract_y)
        .ok_or(CommandError::CannotPlaceSignalOnTrack)?;
    let present = crate::rail_signals::rail_signal_present_mask(tile.m3);
    if present & crate::rail_signals::signal_on_track_mask(track) == 0 {
        return Err(CommandError::CannotPlaceSignalOnTrack);
    }
    let mut out = tile;
    out.m2 = crate::rail_signals::cycle_signal_type_m2(out.m2, track);
    state
        .map
        .set_tile(c, out)
        .map_err(|_| CommandError::OutOfBounds)?;
    crate::rail_signals::enqueue_signal_glob(&mut state.runtime.signal_globset, c);
    Ok(())
}

/// Alterna la variante visual de una señal existente sin cambiar su lógica.
pub(in crate::command) fn cycle_rail_signal_variant(
    state: &mut GameState,
    c: TileCoord,
    fract_x: u8,
    fract_y: u8,
) -> Result<(), CommandError> {
    require_tile_owned_by_active(state, c)?;
    let tile = state.map.get(c).ok_or(CommandError::OutOfBounds)?;
    if !rail_tile_is_signals(tile.m5) {
        return Err(CommandError::CannotPlaceSignalOnTrack);
    }
    let tb = tile.m5 & 0x3F;
    let track = crate::rail_signals::resolve_signal_track(tb, fract_x, fract_y)
        .ok_or(CommandError::CannotPlaceSignalOnTrack)?;
    let present = crate::rail_signals::rail_signal_present_mask(tile.m3);
    if present & crate::rail_signals::signal_on_track_mask(track) == 0 {
        return Err(CommandError::CannotPlaceSignalOnTrack);
    }
    let mut out = tile;
    out.m2 = crate::rail_signals::cycle_signal_variant_m2(out.m2, track);
    state
        .map
        .set_tile(c, out)
        .map_err(|_| CommandError::OutOfBounds)?;
    Ok(())
}

pub(crate) fn check_cycle_rail_signal_type(
    map: &Map,
    c: TileCoord,
    fract_x: u8,
    fract_y: u8,
) -> Result<(), CommandError> {
    check_in_bounds(map, c)?;
    let Some(tile) = map.get(c) else {
        return Err(CommandError::OutOfBounds);
    };
    if !rail_tile_is_signals(tile.m5) {
        return Err(CommandError::CannotPlaceSignalOnTrack);
    }
    let tb = tile.m5 & 0x3F;
    let track = crate::rail_signals::resolve_signal_track(tb, fract_x, fract_y)
        .ok_or(CommandError::CannotPlaceSignalOnTrack)?;
    let present = crate::rail_signals::rail_signal_present_mask(tile.m3);
    if present & crate::rail_signals::signal_on_track_mask(track) == 0 {
        return Err(CommandError::CannotPlaceSignalOnTrack);
    }
    Ok(())
}

pub(in crate::command) fn cycle_rail_signal_side(
    state: &mut GameState,
    c: TileCoord,
    track: crate::rail_signals::SignalTrack,
) -> Result<(), CommandError> {
    require_tile_owned_by_active(state, c)?;
    let tile = state.map.get(c).ok_or(CommandError::OutOfBounds)?;
    if !rail_tile_is_signals(tile.m5) {
        return Err(CommandError::CannotPlaceSignalOnTrack);
    }
    let present = crate::rail_signals::rail_signal_present_mask(tile.m3);
    if present & crate::rail_signals::signal_on_track_mask(track) == 0 {
        return Err(CommandError::CannotPlaceSignalOnTrack);
    }
    let sig_type = crate::rail_signals::signal_type_for_track(tile.m2, track);
    let mut out = tile;
    let old_present = present;
    out.m3 = crate::rail_signals::cycle_signal_side_m3(out.m3, track, sig_type);
    let new_present = crate::rail_signals::rail_signal_present_mask(out.m3);
    let added = new_present & !old_present;
    let states = crate::rail_signals::rail_signal_state_mask(out.m3hi) | added;
    out.m3hi = (out.m3hi & 0x0F) | (states << 4);
    state
        .map
        .set_tile(c, out)
        .map_err(|_| CommandError::OutOfBounds)?;
    crate::rail_signals::enqueue_signal_glob(&mut state.runtime.signal_globset, c);
    Ok(())
}

pub(crate) fn check_remove_rail_signal(
    map: &Map,
    c: TileCoord,
    fract_x: u8,
    fract_y: u8,
) -> Result<(), CommandError> {
    check_in_bounds(map, c)?;
    let Some(tile) = map.get(c) else {
        return Err(CommandError::OutOfBounds);
    };
    if !rail_tile_is_signals(tile.m5) {
        return Err(CommandError::CannotPlaceSignalOnTrack);
    }
    let tb = tile.m5 & 0x3F;
    let track = crate::rail_signals::resolve_signal_track(tb, fract_x, fract_y)
        .ok_or(CommandError::CannotPlaceSignalOnTrack)?;
    let present = crate::rail_signals::rail_signal_present_mask(tile.m3);
    if present & crate::rail_signals::signal_on_track_mask(track) == 0 {
        return Err(CommandError::CannotPlaceSignalOnTrack);
    }
    Ok(())
}

pub(in crate::command) fn remove_rail_signal(
    state: &mut GameState,
    c: TileCoord,
    fract_x: u8,
    fract_y: u8,
) -> Result<(), CommandError> {
    check_remove_rail_signal(&state.map, c, fract_x, fract_y)?;
    require_tile_owned_by_active(state, c)?;
    let tile = state.map.get(c).ok_or(CommandError::OutOfBounds)?;
    let tb = tile.m5 & 0x3F;
    let track = crate::rail_signals::resolve_signal_track(tb, fract_x, fract_y)
        .ok_or(CommandError::CannotPlaceSignalOnTrack)?;
    let present = crate::rail_signals::rail_signal_present_mask(tile.m3);
    let on_track = crate::rail_signals::signal_on_track_mask(track);
    // Quita todos los bits de señal del carril (one-way o two-way).
    let mut out = tile;
    let new_present = present & !on_track;
    if new_present == 0 {
        out.m5 = tb | (RAIL_TILE_NORMAL << 6);
        out.m2 = 0;
        out.m3 &= 0x0F;
        out.m3hi &= 0x0F;
    } else {
        out.m3 = (out.m3 & 0x0F) | (new_present << 4);
        let states = crate::rail_signals::rail_signal_state_mask(out.m3hi) & new_present;
        out.m3hi = (out.m3hi & 0x0F) | (states << 4);
        // Limpia bits de tipo/variante del carril quitado en m2.
        out.m2 = crate::rail_signals::clear_signal_type_bits_m2(out.m2, track);
    }
    state
        .map
        .set_tile(c, out)
        .map_err(|_| CommandError::OutOfBounds)?;
    crate::rail_signals::enqueue_signal_glob(&mut state.runtime.signal_globset, c);
    state.economy.money -= signal_clear_cost(&state.global_economy);
    Ok(())
}
