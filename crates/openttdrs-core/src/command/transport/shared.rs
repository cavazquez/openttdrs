use crate::map::{
    Map, OBJECT_TYPE_STATUE_COMPANY, TileCoord, TileKind, WaterClass, make_water_tile,
    object_type_from_tile, water_class_from_m1,
};
use crate::{CLEAR_TILE_COST, GameState};

use super::super::{CommandError, in_bounds, require_tile_owned_by_active, tile_owner};

#[allow(unused_imports)]
use crate::command::transport::internal::{
    RAIL_DIAG_MASK, RAIL_PARALLEL_MASK, RAIL_TB_HORZ, RAIL_TB_VERT, RAIL_TB_X, RAIL_TB_Y,
    check_rail_trackbits_on_tile, existing_rail_trackbits, is_rail_gap_fill_kind,
    is_rail_path_endpoint, offset_along_horz_rail, offset_along_vert_rail, write_normal_rail_tile,
    write_rail_gap_tile,
};

pub(crate) fn check_in_bounds(map: &Map, c: TileCoord) -> Result<(), CommandError> {
    in_bounds(map, c)
}

pub(in crate::command::transport) fn bridge_line(
    map: &mut Map,
    horizontal: bool,
    fixed: i32,
    len: i32,
) {
    let coord = |i: i32| {
        if horizontal {
            TileCoord::new(i, fixed)
        } else {
            TileCoord::new(fixed, i)
        }
    };
    let mut i = 0;
    while i < len {
        while i < len && !is_rail_path_endpoint(map, coord(i)) {
            i += 1;
        }
        if i >= len {
            break;
        }
        let start = i;
        i += 1;
        let gap_start = i;
        while i < len && map.get_kind(coord(i)).is_some_and(is_rail_gap_fill_kind) {
            i += 1;
        }
        if i < len && is_rail_path_endpoint(map, coord(i)) && gap_start < i {
            for g in gap_start..i {
                write_rail_gap_tile(map, coord(g), horizontal);
            }
        }
        if i == start {
            i += 1;
        }
    }
}

pub(in crate::command::transport) fn trackbits_to_signal_present(tb: u8) -> u8 {
    if tb == RAIL_TB_X || tb == RAIL_TB_Y {
        0b1100
    } else {
        0x0F
    }
}

pub(in crate::command::transport) fn propagate_rail_diag_to_neighbors(
    state: &mut GameState,
    c: TileCoord,
    add: u8,
) -> Result<(), CommandError> {
    let add = add & RAIL_DIAG_MASK;
    if add == 0 {
        return Ok(());
    }
    for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
        let n = TileCoord::new(c.x + dx, c.y + dy);
        if state.map.get_kind(n) != Some(TileKind::Rail) {
            continue;
        }
        if tile_owner(state, n).is_some_and(|o| o != state.active_company) {
            continue;
        }
        let existing = existing_rail_trackbits(&state.map, n);
        let existing_diag = existing & RAIL_DIAG_MASK;
        if existing_diag == 0 || existing_diag == add {
            continue;
        }
        let merged = existing | add;
        if merged == existing {
            continue;
        }
        check_rail_trackbits_on_tile(&state.map, n, merged)?;
        write_normal_rail_tile(state, n, merged)?;
    }
    Ok(())
}

pub(in crate::command::transport) fn junction_merge_for_neighbor(
    holder_tb: u8,
    neighbor_tb: u8,
    dx: i32,
    dy: i32,
) -> Option<u8> {
    if neighbor_tb & RAIL_PARALLEL_MASK == 0 || neighbor_tb.count_ones() != 1 {
        return None;
    }
    let neighbor_horz = neighbor_tb & RAIL_TB_HORZ != 0;
    let neighbor_vert = neighbor_tb & RAIL_TB_VERT != 0;

    let holder_horz = holder_tb & RAIL_TB_HORZ != 0 && holder_tb & RAIL_TB_VERT == 0;
    let holder_vert = holder_tb & RAIL_TB_VERT != 0 && holder_tb & RAIL_TB_HORZ == 0;

    if holder_horz && neighbor_vert && offset_along_vert_rail(dx, dy) {
        return Some(holder_tb | neighbor_tb);
    }
    if holder_vert && neighbor_horz && offset_along_horz_rail(dx, dy) {
        return Some(holder_tb | neighbor_tb);
    }
    None
}

pub(in crate::command::transport) fn refresh_track_junction_from_neighbor(
    state: &mut GameState,
    holder: TileCoord,
    neighbor: TileCoord,
    neighbor_tb: u8,
) -> Result<(), CommandError> {
    let holder_tb = existing_rail_trackbits(&state.map, holder);
    if holder_tb == 0 {
        return Ok(());
    }
    let dx = neighbor.x - holder.x;
    let dy = neighbor.y - holder.y;
    let Some(merged) = junction_merge_for_neighbor(holder_tb, neighbor_tb, dx, dy) else {
        return Ok(());
    };
    if merged == holder_tb {
        return Ok(());
    }
    check_rail_trackbits_on_tile(&state.map, holder, merged)?;
    write_normal_rail_tile(state, holder, merged)
}

pub(crate) fn check_single_transport_tile(map: &Map, c: TileCoord) -> Result<(), CommandError> {
    check_in_bounds(map, c)?;
    let kind = map.get_kind(c).unwrap_or(TileKind::Grass);
    if transport_tile_is_buildable(kind) {
        Ok(())
    } else {
        Err(build_error_for_kind(kind))
    }
}

pub(crate) fn check_clear_tile(map: &Map, c: TileCoord) -> Result<(), CommandError> {
    check_in_bounds(map, c)?;
    if map.get_kind(c) == Some(TileKind::Void) {
        Err(CommandError::CannotPlaceRoadOnVoid)
    } else {
        Ok(())
    }
}

pub(in crate::command) fn transport_tile_is_buildable(kind: TileKind) -> bool {
    !matches!(kind, TileKind::Water | TileKind::Void)
}

pub(in crate::command) fn build_error_for_kind(kind: TileKind) -> CommandError {
    match kind {
        TileKind::Water => CommandError::CannotPlaceRoadOnWater,
        TileKind::Void => CommandError::CannotPlaceRoadOnVoid,
        _ => CommandError::OutOfBounds,
    }
}

pub(in crate::command) fn place_single_transport_tile(
    state: &mut GameState,
    c: TileCoord,
    kind_to_place: TileKind,
    mapt: u8,
    m5: u8,
    cost: i64,
) -> Result<(), CommandError> {
    check_single_transport_tile(&state.map, c)?;
    require_tile_owned_by_active(state, c)?;
    state
        .map
        .set_kind(c, kind_to_place)
        .map_err(|_| CommandError::OutOfBounds)?;
    state
        .map
        .set_mapt_m5(c, mapt, m5)
        .map_err(|_| CommandError::OutOfBounds)?;
    let _ = state.map.set_m1(c, state.active_company.0);
    state.economy.money -= cost;
    Ok(())
}

pub(crate) fn axis_line(a: TileCoord, b: TileCoord) -> Vec<TileCoord> {
    if (b.x - a.x).abs() >= (b.y - a.y).abs() {
        let step = if b.x >= a.x { 1 } else { -1 };
        let mut out = Vec::new();
        let mut x = a.x;
        loop {
            out.push(TileCoord::new(x, a.y));
            if x == b.x {
                break;
            }
            x += step;
        }
        out
    } else {
        let step = if b.y >= a.y { 1 } else { -1 };
        let mut out = Vec::new();
        let mut y = a.y;
        loop {
            out.push(TileCoord::new(a.x, y));
            if y == b.y {
                break;
            }
            y += step;
        }
        out
    }
}

fn check_town_demolition_rating(
    state: &GameState,
    c: TileCoord,
    kind: TileKind,
) -> Result<(), CommandError> {
    if state.cheats.magic_bulldozer_active() {
        return Ok(());
    }
    let tile = state.map.get(c).ok_or(CommandError::OutOfBounds)?;
    if !crate::company::CompanyId::is_town_owner_m1(tile.m1) {
        return Ok(());
    }
    let check_type = match kind {
        TileKind::RoadBridge
        | TileKind::RailBridge
        | TileKind::RoadTunnel
        | TileKind::RailTunnel => crate::town::TownRatingCheckType::TunnelBridgeRemove,
        TileKind::Road => crate::town::TownRatingCheckType::RoadRemove,
        _ => return Ok(()),
    };
    let Some((idx, dist)) = crate::town::nearest_town_index(&state.towns, c) else {
        return Ok(());
    };
    if dist > crate::town::TOWN_AUTHORITY_RADIUS {
        return Ok(());
    }
    if crate::town::check_town_rating(
        &state.towns[idx],
        state.active_company,
        check_type,
        state.town_council_tolerance,
    ) {
        Ok(())
    } else {
        Err(CommandError::AuthorityRatingTooLow)
    }
}

pub(in crate::command) fn clear_tile(
    state: &mut GameState,
    c: TileCoord,
) -> Result<(), CommandError> {
    check_clear_tile(&state.map, c)?;
    if let Some(kind) = state.map.get_kind(c) {
        check_town_demolition_rating(state, c, kind)?;
    }
    if !state.cheats.magic_bulldozer_active() {
        require_tile_owned_by_active(state, c)?;
    }
    if let Some(industry_idx) = state.industries.iter().position(|i| i.contains_tile(c)) {
        let industry_tiles = state.industries[industry_idx].tiles.clone();
        for tile in industry_tiles {
            state
                .map
                .set_kind(tile, TileKind::Grass)
                .map_err(|_| CommandError::OutOfBounds)?;
            state
                .map
                .set_mapt_m5(tile, 0x00, 0x00)
                .map_err(|_| CommandError::OutOfBounds)?;
        }
        state.industries.remove(industry_idx);
        state.economy.money -= CLEAR_TILE_COST;
        return Ok(());
    }
    if let Some(object_tiles) =
        crate::map::object_footprint_at(&state.map, c, &state.object_spec_catalog)
    {
        let statue_owner = state
            .map
            .get(c)
            .filter(|tile| object_type_from_tile(tile) == Some(OBJECT_TYPE_STATUE_COMPANY))
            .map(|tile| crate::company::CompanyId(tile.m1));
        for tile in &object_tiles {
            if !state.cheats.magic_bulldozer_active() {
                require_tile_owned_by_active(state, *tile)?;
            }
        }
        for &tile in &object_tiles {
            state
                .map
                .set_kind(tile, TileKind::Grass)
                .map_err(|_| CommandError::OutOfBounds)?;
            state
                .map
                .set_mapt_m5(tile, 0x00, 0x00)
                .map_err(|_| CommandError::OutOfBounds)?;
            let _ = state.map.set_m2(tile, 0);
            crate::command::sign::remove_signs_at(state, tile);
        }
        state.stations.retain(|s| !object_tiles.contains(&s.pos));
        // `Object` upstream conserva el pueblo de la estatua. El port no
        // mantiene ese pool, por lo que la estatua se vincula al pueblo más
        // cercano; fue colocada dentro de su búsqueda 9×9.
        if let Some(owner) = statue_owner
            && let Some((town_idx, _)) = crate::town::nearest_town_index(&state.towns, c)
        {
            state.towns[town_idx].set_statue(owner, false);
        }
        state.economy.money -= CLEAR_TILE_COST;
        return Ok(());
    }

    // Una boya es una estación superpuesta sobre agua. Al retirarla, la
    // tesela subyacente debe volver a ser agua (con su clase original), no
    // hierba: de lo contrario se destruye un canal, mar o río navegable.
    if state
        .stations
        .iter()
        .any(|station| station.pos == c && station.stop_kind == crate::station::StopKind::Buoy)
    {
        let water_class = state
            .map
            .get(c)
            .map_or(WaterClass::Sea, |tile| water_class_from_m1(tile.m1));
        make_water_tile(&mut state.map, c, water_class).map_err(|_| CommandError::OutOfBounds)?;
        let mut tile = state.map.get(c).ok_or(CommandError::OutOfBounds)?;
        tile.m6 = 0;
        state
            .map
            .set_tile(c, tile)
            .map_err(|_| CommandError::OutOfBounds)?;
        state.stations.retain(|station| station.pos != c);
        state.economy.money -= CLEAR_TILE_COST;
        return Ok(());
    }

    state
        .map
        .set_kind(c, TileKind::Grass)
        .map_err(|_| CommandError::OutOfBounds)?;
    state
        .map
        .set_mapt_m5(c, 0x00, 0x00)
        .map_err(|_| CommandError::OutOfBounds)?;
    state.stations.retain(|s| s.pos != c);
    state
        .industries
        .retain(|industry| !industry.contains_tile(c));
    crate::command::sign::remove_signs_at(state, c);
    state.economy.money -= CLEAR_TILE_COST;
    Ok(())
}
