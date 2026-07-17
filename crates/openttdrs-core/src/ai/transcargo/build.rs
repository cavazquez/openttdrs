//! Construcción de infraestructura `TransCargo`: vías, señales, estaciones y depósitos.

use crate::GameState;
use crate::command::{Command, LevelMode, apply_command};
use crate::company::CompanyId;
use crate::map::rail_bit_for_sides;
use crate::map::{RAIL_TB_Y, TileCoord, TileKind};
use crate::pathfinder::{PathNetwork, find_path, find_rail_build_path};
use crate::rail_signals::{SIGTYPE_BLOCK, rail_tile_is_signals};

use super::plan::{RoutePlan, pick_station_tile};

/// Corredor tendido: pathfind (#184) o fallback Manhattan L.
enum BuiltCorridor {
    Path(Vec<TileCoord>),
    Manhattan,
}

pub(super) fn build_freight_line(
    state: &mut GameState,
    ai_id: CompanyId,
    plan: RoutePlan,
) -> Option<(TileCoord, TileCoord, TileCoord)> {
    let existing_ai: Vec<TileCoord> = state
        .stations
        .iter()
        .filter(|s| s.owner == ai_id)
        .map(|s| s.pos)
        .collect();
    let load_st = pick_station_tile(&state.map, plan.source, plan.dest, &existing_ai)?;
    // Preferir reutilizar una estación de descarga ya existente junto al destino.
    let unload_reuse = existing_ai.iter().copied().find(|&st| {
        (st.x - plan.dest.x).abs() <= 2 && (st.y - plan.dest.y).abs() <= 2 && st != load_st
    });
    let unload_st = unload_reuse.or_else(|| {
        let mut avoid = existing_ai.clone();
        avoid.push(load_st);
        pick_station_tile(&state.map, plan.dest, plan.source, &avoid)
    })?;
    if load_st == unload_st {
        return None;
    }

    let mut depot_out = None;
    with_ai_active(state, ai_id, |state| {
        let corridor = place_rail_corridor(state, load_st, unload_st);
        if !place_rail_station_owned(state, load_st, ai_id) {
            return;
        }
        if !place_rail_station_owned(state, unload_st, ai_id) {
            return;
        }
        // Reconectar vía bajo/alrededor de las estaciones.
        let _ = place_rail_corridor(state, load_st, unload_st);
        place_corridor_signals(state, load_st, unload_st, &corridor);
        depot_out = try_place_depot_near(state, load_st);
        // El depósito (autorraíl) puede refrescar vecinos: reafirmar curvas.
        reapply_corridor_corners(state, load_st, unload_st, &corridor);
    });

    let depot = depot_out?;
    let has_load = state
        .stations
        .iter()
        .any(|s| s.owner == ai_id && s.pos == load_st);
    let has_unload = state
        .stations
        .iter()
        .any(|s| s.owner == ai_id && s.pos == unload_st);
    if has_load && has_unload {
        Some((load_st, unload_st, depot))
    } else {
        None
    }
}

fn with_ai_active(state: &mut GameState, ai_id: CompanyId, f: impl FnOnce(&mut GameState)) {
    let prev_active = state.active_company;
    state.active_company = ai_id;
    if let Some(c) = state.companies.get(ai_id.index()) {
        state.economy = c.economy;
        state.company_colour = c.colour;
    }
    f(state);
    state.active_company = prev_active;
    state.sync_mirrors_from_active();
}

/// Pathfind de buildables (#184); si falla, L Manhattan (comportamiento previo).
fn place_rail_corridor(state: &mut GameState, from: TileCoord, to: TileCoord) -> BuiltCorridor {
    if let Some(path) = find_rail_build_path(&state.map, from, to)
        && path.len() >= 2
    {
        flatten_path_band(state, &path);
        place_rail_along_path(state, &path);
        return BuiltCorridor::Path(path);
    }
    flatten_build_band(state, from, to);
    place_rail_manhattan_corridor(state, from, to);
    BuiltCorridor::Manhattan
}

fn place_corridor_signals(
    state: &mut GameState,
    from: TileCoord,
    to: TileCoord,
    corridor: &BuiltCorridor,
) {
    match corridor {
        BuiltCorridor::Path(path) => place_path_block_signals(state, path),
        BuiltCorridor::Manhattan => place_corridor_block_signals(state, from, to),
    }
}

fn reapply_corridor_corners(
    state: &mut GameState,
    from: TileCoord,
    to: TileCoord,
    corridor: &BuiltCorridor,
) {
    match corridor {
        BuiltCorridor::Path(path) => reapply_path_corners(state, path),
        BuiltCorridor::Manhattan => reapply_l_corner(state, from, to),
    }
}

/// Coloca vía siguiendo un polyline cardenal (curvas en giros, sin X|Y).
fn place_rail_along_path(state: &mut GameState, path: &[TileCoord]) {
    if path.len() < 2 {
        return;
    }
    for i in 0..path.len() {
        let c = path[i];
        if matches!(
            state.map.get_kind(c),
            Some(TileKind::Station | TileKind::RailDepot)
        ) {
            continue;
        }
        let prev = i.checked_sub(1).map(|j| path[j]);
        let next = path.get(i + 1).copied();
        match (prev, next) {
            (Some(p), Some(n)) => {
                let d_in = (c.x - p.x, c.y - p.y);
                let d_out = (n.x - c.x, n.y - c.y);
                if d_in == d_out {
                    place_rail_axis(state, c, d_in.1 != 0, p);
                } else if (d_in.0 != 0 && d_out.1 != 0) || (d_in.1 != 0 && d_out.0 != 0) {
                    place_path_corner_curve(state, c, p, n);
                } else {
                    place_rail_axis(state, c, d_in.1 != 0, p);
                }
            }
            (None, Some(other)) | (Some(other), None) => {
                let axis_y = c.x == other.x;
                place_rail_axis(state, c, axis_y, other);
            }
            (None, None) => {}
        }
    }
}

fn diag_dir_from_step(dx: i32, dy: i32) -> u8 {
    match (dx.signum(), dy.signum()) {
        (1, 0) => 0,  // NE
        (0, 1) => 1,  // SE
        (-1, 0) => 2, // SW
        (0, -1) => 3, // NW
        _ => 0,
    }
}

fn place_path_corner_curve(
    state: &mut GameState,
    corner: TileCoord,
    from_prev: TileCoord,
    to_next: TileCoord,
) {
    if matches!(
        state.map.get_kind(corner),
        Some(TileKind::Station | TileKind::RailDepot)
    ) {
        return;
    }
    let dx_in = (corner.x - from_prev.x).signum();
    let dy_in = (corner.y - from_prev.y).signum();
    let dx_out = (to_next.x - corner.x).signum();
    let dy_out = (to_next.y - corner.y).signum();
    let entry = diag_dir_from_step(dx_in, dy_in);
    let exit = diag_dir_from_step(dx_out, dy_out);
    let curve = rail_bit_for_sides(entry, exit);
    if curve == 0 {
        return;
    }
    // Crear vía con la curva; SetRailBits fusiona Y ajeno si ya había rail.
    let _ = apply_command(state, &Command::PlaceRailBits(corner, curve));
    let existing = state
        .map
        .get(corner)
        .filter(|t| t.kind == TileKind::Rail)
        .map_or(0, |t| t.m5 & 0x3F);
    let bits = curve | (existing & RAIL_TB_Y);
    if bits != existing {
        let _ = apply_command(state, &Command::SetRailBits(corner, bits));
    }
}

fn reapply_path_corners(state: &mut GameState, path: &[TileCoord]) {
    if path.len() < 3 {
        return;
    }
    for i in 1..path.len() - 1 {
        let p = path[i - 1];
        let c = path[i];
        let n = path[i + 1];
        let d_in = (c.x - p.x, c.y - p.y);
        let d_out = (n.x - c.x, n.y - c.y);
        if d_in != d_out && ((d_in.0 != 0 && d_out.1 != 0) || (d_in.1 != 0 && d_out.0 != 0)) {
            place_path_corner_curve(state, c, p, n);
        }
    }
}

/// Nivela la **banda** del polyline (±`margin`), sin bbox Manhattan.
///
/// Cada `LevelLand` solo cubre un tramo corto (par adyacente o vecino ±1),
/// propagando la altura del inicio del path. Evita aplanar medio mapa en L/desvíos.
fn flatten_path_band(state: &mut GameState, path: &[TileCoord]) {
    const MARGIN: i32 = 1;
    if path.len() < 2 {
        return;
    }
    for w in path.windows(2) {
        level_path_strip(state, w[0], w[1], MARGIN);
    }
    // Vecinos laterales de cada tesela del path (misma altura local).
    for &c in path {
        if tile_skip_path_terraform(state, c) {
            continue;
        }
        for (dx, dy) in [(-1_i32, 0), (1, 0), (0, -1), (0, 1)] {
            let n = TileCoord::new(c.x + dx, c.y + dy);
            if state.map.get(n).is_none() || tile_skip_path_terraform(state, n) {
                continue;
            }
            let _ = apply_command(
                state,
                &Command::LevelLand {
                    from: c,
                    to: n,
                    mode: LevelMode::Level,
                },
            );
        }
    }
}

fn tile_skip_path_terraform(state: &GameState, c: TileCoord) -> bool {
    matches!(
        state.map.get_kind(c),
        Some(
            TileKind::Water
                | TileKind::Void
                | TileKind::House
                | TileKind::Industry
                | TileKind::Station
                | TileKind::RailDepot
        )
    )
}

/// Nivela el segmento cardenal `a`→`b` y un margen perpendicular ±`margin`.
fn level_path_strip(state: &mut GameState, a: TileCoord, b: TileCoord, margin: i32) {
    if tile_skip_path_terraform(state, a) {
        return;
    }
    let dx = (b.x - a.x).signum();
    let dy = (b.y - a.y).signum();
    let (px, py) = (-dy, dx); // perpendicular
    let _ = apply_command(
        state,
        &Command::LevelLand {
            from: a,
            to: b,
            mode: LevelMode::Level,
        },
    );
    if margin <= 0 || (px == 0 && py == 0) {
        return;
    }
    for m in [-margin, margin] {
        let side_a = TileCoord::new(a.x + px * m, a.y + py * m);
        let side_b = TileCoord::new(b.x + px * m, b.y + py * m);
        if state.map.get(side_a).is_some() && !tile_skip_path_terraform(state, side_a) {
            let _ = apply_command(
                state,
                &Command::LevelLand {
                    from: a,
                    to: side_a,
                    mode: LevelMode::Level,
                },
            );
        }
        if state.map.get(side_b).is_some() && !tile_skip_path_terraform(state, side_b) {
            let _ = apply_command(
                state,
                &Command::LevelLand {
                    from: b,
                    to: side_b,
                    mode: LevelMode::Level,
                },
            );
        }
    }
}

fn place_path_block_signals(state: &mut GameState, path: &[TileCoord]) {
    if path.len() < 3 {
        return;
    }
    let interior: Vec<(TileCoord, bool)> = path[1..path.len() - 1]
        .iter()
        .enumerate()
        .filter_map(|(idx, &c)| {
            let i = idx + 1;
            let p = path[i - 1];
            let n = path[i + 1];
            let d_in = (c.x - p.x, c.y - p.y);
            let d_out = (n.x - c.x, n.y - c.y);
            if d_in != d_out {
                return None;
            }
            Some((c, d_in.1 != 0))
        })
        .collect();
    if interior.is_empty() {
        return;
    }
    let idxs: Vec<usize> = if interior.len() >= 8 {
        vec![
            interior.len() / 4,
            interior.len() / 2,
            (3 * interior.len()) / 4,
        ]
    } else {
        vec![interior.len() / 2]
    };
    for i in idxs {
        let (c, axis_y) = interior[i];
        try_place_two_way_block_signal(state, c, axis_y);
    }
}

/// Coloca vía en corredor Manhattan: primero eje X, luego eje Y.
///
/// Usa `PlaceRailBits` por eje para **fusionar** en tramos con corredores previos.
/// El codo L no recibe ejes: solo la curva (más ejes ajenos si otra ruta atraviesa).
fn place_rail_manhattan_corridor(state: &mut GameState, from: TileCoord, to: TileCoord) {
    let step_x = (to.x - from.x).signum();
    let step_y = (to.y - from.y).signum();
    let mut c = from;
    while c.x != to.x {
        c = TileCoord::new(c.x + step_x, c.y);
        // No poner eje X en el futuro codo L (evitar X|Y al propagar el tramo Y).
        let is_l_corner = step_y != 0 && c.x == to.x;
        if !is_l_corner {
            place_rail_axis(state, c, false, from);
        }
    }
    let corner = c;
    while c.y != to.y {
        c = TileCoord::new(c.x, c.y + step_y);
        place_rail_axis(state, c, true, from);
    }
    if step_x != 0 && step_y != 0 {
        place_l_corner_curve(state, corner, step_x, step_y);
    }
}

fn reapply_l_corner(state: &mut GameState, from: TileCoord, to: TileCoord) {
    let step_x = (to.x - from.x).signum();
    let step_y = (to.y - from.y).signum();
    if step_x == 0 || step_y == 0 {
        return;
    }
    let corner = TileCoord::new(to.x, from.y);
    place_l_corner_curve(state, corner, step_x, step_y);
}

/// Pieza de giro en la esquina X→Y (`DiagDir` NE=0 … NW=3).
fn place_l_corner_curve(state: &mut GameState, corner: TileCoord, step_x: i32, step_y: i32) {
    if matches!(
        state.map.get_kind(corner),
        Some(TileKind::Station | TileKind::RailDepot)
    ) {
        return;
    }
    // Entrada por el lado opuesto a `step_x`; salida hacia `step_y`.
    let entry = if step_x > 0 { 0u8 } else { 2u8 };
    let exit = if step_y > 0 { 1u8 } else { 3u8 };
    let curve = rail_bit_for_sides(entry, exit);
    let existing = state
        .map
        .get(corner)
        .filter(|t| t.kind == TileKind::Rail)
        .map_or(0, |t| t.m5 & 0x3F);
    // El L sigue en Y: conservar ese eje si otra ruta ya atraviesa el codo.
    // El X que llega por propagación del tramo horizontal es ruido (crea CROSS).
    let bits = curve | (existing & RAIL_TB_Y);
    let _ = apply_command(state, &Command::SetRailBits(corner, bits));
}

fn place_rail_axis(state: &mut GameState, c: TileCoord, axis_y: bool, height_anchor: TileCoord) {
    if matches!(
        state.map.get_kind(c),
        Some(TileKind::Station | TileKind::RailDepot)
    ) {
        return;
    }
    let bits = if axis_y { 0x02 } else { 0x01 };
    if apply_command(state, &Command::PlaceRailBits(c, bits)).is_ok() {
        return;
    }
    // Pendiente / desnivel: igualar a la altura del ancla del corredor y reintentar.
    let _ = apply_command(
        state,
        &Command::LevelLand {
            from: height_anchor,
            to: c,
            mode: LevelMode::Level,
        },
    );
    let _ = apply_command(state, &Command::PlaceRailBits(c, bits));
}

/// Nivela la banda Manhattan (con margen) a la altura de `anchor`.
fn flatten_build_band(state: &mut GameState, anchor: TileCoord, other: TileCoord) {
    let (mw, mh) = state.map.dimensions();
    let max_x = i32::try_from(mw.saturating_sub(1)).unwrap_or(0);
    let max_y = i32::try_from(mh.saturating_sub(1)).unwrap_or(0);
    let min_x = (anchor.x.min(other.x) - 1).max(0);
    let min_y = (anchor.y.min(other.y) - 1).max(0);
    let end_x = (anchor.x.max(other.x) + 1).min(max_x);
    let end_y = (anchor.y.max(other.y) + 1).min(max_y);
    let to = TileCoord::new(
        if anchor.x <= i32::midpoint(min_x, end_x) {
            end_x
        } else {
            min_x
        },
        if anchor.y <= i32::midpoint(min_y, end_y) {
            end_y
        } else {
            min_y
        },
    );
    let _ = apply_command(
        state,
        &Command::LevelLand {
            from: anchor,
            to,
            mode: LevelMode::Level,
        },
    );
}

/// Teselas interiores del corredor X→Y (sin extremos) y si el tramo es eje Y.
fn manhattan_interior_tiles(from: TileCoord, to: TileCoord) -> Vec<(TileCoord, bool)> {
    let step_x = (to.x - from.x).signum();
    let step_y = (to.y - from.y).signum();
    let mut out = Vec::new();
    let mut c = from;
    while c.x != to.x {
        c = TileCoord::new(c.x + step_x, c.y);
        if c != to {
            out.push((c, false));
        }
    }
    while c.y != to.y {
        c = TileCoord::new(c.x, c.y + step_y);
        if c != to {
            out.push((c, true));
        }
    }
    out
}

fn try_place_two_way_block_signal(state: &mut GameState, c: TileCoord, axis_y: bool) {
    let Some(tile) = state.map.get(c) else {
        return;
    };
    if tile.kind != TileKind::Rail {
        return;
    }
    if rail_tile_is_signals(tile.m5) {
        return;
    }
    let tb = tile.m5 & 0x3F;
    // Solo tramos rectos puros (evitar cruces / curvas del codo L).
    if axis_y {
        if tb != 0x02 {
            return;
        }
    } else if tb != 0x01 {
        return;
    }
    let orient = u8::from(axis_y);
    if apply_command(
        state,
        &Command::PlaceRailSignal(c, orient, 128, 128, SIGTYPE_BLOCK),
    )
    .is_err()
    {
        return;
    }
    // 2.º clic → bidireccional (mismo encoding que la UI).
    let _ = apply_command(
        state,
        &Command::PlaceRailSignal(c, orient, 128, 128, SIGTYPE_BLOCK),
    );
}

/// Señales de bloque en el corredor (punto medio y cuartiles si es largo).
fn place_corridor_block_signals(state: &mut GameState, from: TileCoord, to: TileCoord) {
    let tiles = manhattan_interior_tiles(from, to);
    if tiles.is_empty() {
        return;
    }
    let idxs: Vec<usize> = if tiles.len() >= 8 {
        vec![tiles.len() / 4, tiles.len() / 2, (3 * tiles.len()) / 4]
    } else {
        vec![tiles.len() / 2]
    };
    for i in idxs {
        let (c, axis_y) = tiles[i];
        try_place_two_way_block_signal(state, c, axis_y);
    }
}

fn place_rail_if_needed(state: &mut GameState, c: TileCoord) {
    // Boca de depósito / enlaces ortogonales: autorraíl por vecinos.
    if matches!(
        state.map.get_kind(c),
        Some(TileKind::Station | TileKind::RailDepot)
    ) {
        return;
    }
    let _ = apply_command(state, &Command::PlaceRail(c));
}

fn place_rail_station_owned(state: &mut GameState, st_pos: TileCoord, ai_id: CompanyId) -> bool {
    if state.stations.iter().any(|s| s.pos == st_pos) {
        if let Some(st) = state.stations.iter_mut().find(|s| s.pos == st_pos) {
            st.owner = ai_id;
        }
        return true;
    }
    if state.map.get_kind(st_pos) == Some(TileKind::Rail) {
        let _ = apply_command(state, &Command::ClearTile(st_pos));
    }
    for dir in 0..4u8 {
        if apply_command(state, &Command::PlaceRailStation(st_pos, dir)).is_ok() {
            if let Some(st) = state.stations.iter_mut().find(|s| s.pos == st_pos) {
                st.owner = ai_id;
            }
            return true;
        }
    }
    false
}

fn try_place_depot_near(state: &mut GameState, load_st: TileCoord) -> Option<TileCoord> {
    // Candidatos: adyacentes (boca a andén) y laterales (boca a vía paralela).
    // Se acepta el primero con path real al destino de la orden (plataforma).
    let preferred = [
        (
            TileCoord::new(load_st.x + 1, load_st.y),
            load_st,
            0u8, // boca al oeste
        ),
        (
            TileCoord::new(load_st.x - 1, load_st.y),
            load_st,
            2u8, // boca al este
        ),
        (TileCoord::new(load_st.x, load_st.y + 1), load_st, 3u8),
        (TileCoord::new(load_st.x, load_st.y - 1), load_st, 1u8),
        (
            TileCoord::new(load_st.x + 1, load_st.y + 1),
            TileCoord::new(load_st.x + 1, load_st.y),
            3u8,
        ),
        (
            TileCoord::new(load_st.x - 1, load_st.y + 1),
            TileCoord::new(load_st.x - 1, load_st.y),
            3u8,
        ),
        (
            TileCoord::new(load_st.x + 1, load_st.y - 1),
            TileCoord::new(load_st.x + 1, load_st.y),
            1u8,
        ),
        (
            TileCoord::new(load_st.x - 1, load_st.y - 1),
            TileCoord::new(load_st.x - 1, load_st.y),
            1u8,
        ),
    ];
    let stop = crate::station::rail_station_stop_tile(&state.map, load_st).unwrap_or(load_st);
    for (depot, mouth, dir) in preferred {
        if state.map.get(depot).is_none() {
            continue;
        }
        if state.map.get_kind(depot) == Some(TileKind::RailDepot) {
            if find_path(&state.map, depot, stop, PathNetwork::Rail).is_some() {
                return Some(depot);
            }
            continue;
        }
        if mouth != load_st && mouth != depot {
            place_rail_if_needed(state, mouth);
        }
        if apply_command(state, &Command::PlaceRailDepotDir(depot, dir)).is_err() {
            continue;
        }
        if find_path(&state.map, depot, stop, PathNetwork::Rail).is_some() {
            return Some(depot);
        }
        // Layout inválido para pathfinding: retirar y probar otro.
        let _ = apply_command(state, &Command::ClearTile(depot));
    }
    None
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::map::TileCoord;

    #[test]
    fn manhattan_corridor_places_l_shape() {
        let mut state = GameState::new(16, 16);
        state.economy.money = 500_000;
        let a = TileCoord::new(2, 2);
        let b = TileCoord::new(8, 6);
        place_rail_manhattan_corridor(&mut state, a, b);
        // Esquina del L en (8,2).
        assert_eq!(
            state.map.get_kind(TileCoord::new(8, 2)),
            Some(TileKind::Rail)
        );
        assert_eq!(
            state.map.get_kind(TileCoord::new(8, 6)),
            Some(TileKind::Rail)
        );
        assert_eq!(
            state.map.get_kind(TileCoord::new(5, 2)),
            Some(TileKind::Rail)
        );
    }

    #[test]
    fn manhattan_corridor_corner_is_curve_not_crossing() {
        let mut state = GameState::new(16, 16);
        state.economy.money = 500_000;
        let a = TileCoord::new(2, 2);
        let b = TileCoord::new(8, 6);
        place_rail_manhattan_corridor(&mut state, a, b);
        let corner = TileCoord::new(8, 2);
        let tb = state.map.get(corner).expect("corner rail").m5 & 0x3F;
        // step_x>0, step_y>0 → RIGHT (NE↔SE), sin ejes X|Y.
        assert_eq!(tb, 0x20, "codo L debe ser curva RIGHT, no cruce: m5={tb:#04x}");
        assert!(
            find_path(
                &state.map,
                TileCoord::new(7, 2),
                TileCoord::new(8, 3),
                PathNetwork::Rail
            )
            .is_some(),
            "la curva del codo debe permitir girar X→Y"
        );
    }

    #[test]
    fn path_terraform_does_not_flatten_outside_band() {
        let mut state = GameState::new(16, 16);
        state.economy.money = 500_000;
        let from = TileCoord::new(2, 2);
        let to = TileCoord::new(10, 8);
        let planned = find_rail_build_path(&state.map, from, to).expect("A*");
        let mut band = std::collections::HashSet::new();
        for &c in &planned {
            for (dx, dy) in [(0, 0), (-1, 0), (1, 0), (0, -1), (0, 1)] {
                band.insert(TileCoord::new(c.x + dx, c.y + dy));
            }
        }
        let far = (2..=10)
            .flat_map(|x| (2..=8).map(move |y| TileCoord::new(x, y)))
            .find(|c| !band.contains(c))
            .expect("tesela en bbox Manhattan fuera de la banda");
        state.map.set_height(far, 4).unwrap();

        let corridor = place_rail_corridor(&mut state, from, to);
        assert!(matches!(corridor, BuiltCorridor::Path(_)));
        assert_eq!(
            state.map.get(far).unwrap().height,
            4,
            "terraform de path no debe aplanar {far:?} fuera de la banda ±1"
        );
    }

    #[test]
    fn path_corridor_prefers_grass_around_dense_forest() {
        let mut state = GameState::new(14, 8);
        state.economy.money = 500_000;
        for x in 3..10 {
            state
                .map
                .set_kind(TileCoord::new(x, 2), TileKind::Forest)
                .unwrap();
        }
        let a = TileCoord::new(2, 2);
        let b = TileCoord::new(11, 2);
        let planned = find_rail_build_path(&state.map, a, b).expect("A*");
        let forest_hits = planned
            .iter()
            .filter(|c| state.map.get_kind(**c) == Some(TileKind::Forest))
            .count();
        assert!(
            forest_hits < 7,
            "debe cruzar menos bosque que la línea directa (7): hits={forest_hits} path={planned:?}"
        );
        let corridor = place_rail_corridor(&mut state, a, b);
        assert!(matches!(corridor, BuiltCorridor::Path(_)));
        let rail_n = (0..14)
            .flat_map(|x| (0..8).map(move |y| TileCoord::new(x, y)))
            .filter(|&c| state.map.get_kind(c) == Some(TileKind::Rail))
            .count();
        assert!(rail_n >= 8, "corredor tendido; rail_tiles={rail_n}");
    }

    #[test]
    fn path_corridor_avoids_water_and_keeps_curve_topology() {
        let mut state = GameState::new(16, 12);
        state.economy.money = 500_000;
        // Agua bloquea el L corto en y=2 entre x=4..8.
        for x in 4..9 {
            state
                .map
                .set_kind(TileCoord::new(x, 2), TileKind::Water)
                .unwrap();
        }
        let a = TileCoord::new(2, 2);
        let b = TileCoord::new(10, 4);
        let planned = find_rail_build_path(&state.map, a, b).expect("A* build");
        let corridor = place_rail_corridor(&mut state, a, b);
        let BuiltCorridor::Path(path) = corridor else {
            panic!("debe usar pathfind para rodear agua");
        };
        assert_eq!(path, planned);
        for &c in &path {
            assert_ne!(
                state.map.get_kind(c),
                Some(TileKind::Water),
                "path no debe incluir agua {c:?}"
            );
        }
        let rail_on_path = path
            .iter()
            .filter(|c| state.map.get_kind(**c) == Some(TileKind::Rail))
            .count();
        assert!(
            rail_on_path >= path.len().saturating_sub(1),
            "casi todo el path debe ser Rail; rail={rail_on_path} path={path:?}"
        );
        for i in 1..path.len() - 1 {
            let p = path[i - 1];
            let c = path[i];
            let n = path[i + 1];
            let turn = (c.x - p.x, c.y - p.y) != (n.x - c.x, n.y - c.y);
            if !turn || state.map.get_kind(c) != Some(TileKind::Rail) {
                continue;
            }
            let tb = state.map.get(c).unwrap().m5 & 0x3F;
            assert_ne!(tb, 0x03, "codo pathfind no debe ser CROSS en {c:?}: {tb:#04x}");
            assert_ne!(tb & 0x03, 0x03, "codo no debe tener X|Y en {c:?}: {tb:#04x}");
        }
    }

    #[test]
    fn second_corridor_keeps_through_axis_plus_curve_at_shared_corner() {
        let mut state = GameState::new(16, 16);
        state.economy.money = 500_000;
        // Ruta 1: (2,2)→(8,6) deja Y en (8,4).
        place_rail_manhattan_corridor(&mut state, TileCoord::new(2, 2), TileCoord::new(8, 6));
        // Ruta 2: (4,4)→(8,6); codo en (8,4) donde ya hay Y de la ruta 1.
        place_rail_manhattan_corridor(&mut state, TileCoord::new(4, 4), TileCoord::new(8, 6));
        let corner = TileCoord::new(8, 4);
        let tb = state.map.get(corner).expect("shared corner").m5 & 0x3F;
        assert_eq!(tb, 0x22, "codo compartido = RIGHT|Y, no cruce: m5={tb:#04x}");
        assert!(
            find_path(
                &state.map,
                TileCoord::new(8, 3),
                TileCoord::new(8, 5),
                PathNetwork::Rail
            )
            .is_some(),
            "la ruta 1 debe seguir pudiendo atravesar en Y"
        );
        assert!(
            find_path(
                &state.map,
                TileCoord::new(7, 4),
                TileCoord::new(8, 5),
                PathNetwork::Rail
            )
            .is_some(),
            "la ruta 2 debe poder girar en el codo"
        );
    }
}
