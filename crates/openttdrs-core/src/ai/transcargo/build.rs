//! Construcción de infraestructura `TransCargo`: vías, señales, estaciones y depósitos.

use crate::GameState;
use crate::command::{Command, LevelMode, apply_command};
use crate::company::CompanyId;
use crate::map::rail_bit_for_sides;
use crate::map::{RAIL_TB_Y, TileCoord, TileKind, opposite_diag_dir};
use crate::pathfinder::{PathNetwork, find_path, find_rail_build_path};
use crate::rail_signals::{SIGTYPE_BLOCK, rail_tile_is_signals};

use super::plan::{RoutePlan, pick_station_tile};
use crate::ai::build_queue::{AiBuildFinish, AiBuildQueue, record_build_commands};

/// Corredor tendido: pathfind (#184) o fallback Manhattan L.
enum BuiltCorridor {
    Path(Vec<TileCoord>),
    Manhattan,
}

/// Planifica en un clon (grabando comandos) y devuelve la cola sin mutar `state`.
pub(crate) fn plan_freight_line_queue(
    state: &GameState,
    ai_id: CompanyId,
    plan: RoutePlan,
) -> Option<AiBuildQueue> {
    let mut endpoints = None;
    let commands = record_build_commands(state, |tmp| {
        endpoints = build_freight_line(tmp, ai_id, plan);
    });
    let (load_st, unload_st, depot) = endpoints?;
    if commands.is_empty() {
        return None;
    }
    Some(AiBuildQueue {
        company: ai_id,
        commands,
        finish: AiBuildFinish::TransCargo {
            load_st,
            unload_st,
            depot,
            source: plan.source,
            dest: plan.dest,
            cargo: plan.cargo,
        },
    })
}

pub(crate) fn build_freight_line(
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
        // Un solo polyline: no recalcular A* tras estaciones (evita tramos huérfanos).
        let mut corridor = place_rail_corridor(state, load_st, unload_st);
        if !place_rail_station_owned(state, load_st, ai_id) {
            return;
        }
        if !place_rail_station_owned(state, unload_st, ai_id) {
            return;
        }
        reapply_built_corridor(state, load_st, unload_st, &corridor);
        if !rail_stations_connected(state, load_st, unload_st) {
            // Último recurso: L Manhattan (mismo from/to, sin segundo A*).
            flatten_build_band(state, load_st, unload_st);
            place_rail_manhattan_corridor(state, load_st, unload_st);
            reapply_l_corner(state, load_st, unload_st);
            corridor = BuiltCorridor::Manhattan;
        }
        if let Some(spur) = try_spur_to_existing_network(state, load_st, unload_st, &existing_ai) {
            corridor = spur;
        }
        if !rail_stations_connected(state, load_st, unload_st) {
            return;
        }
        place_corridor_signals(state, load_st, unload_st, &corridor);
        depot_out = try_place_depot_near(state, load_st)
            .or_else(|| find_rail_depot_linked_to(state, load_st));
        reapply_corridor_corners(state, load_st, unload_st, &corridor);
        // Depósito/autorail puede romper el codo: reafirmar y revalidar.
        if !rail_stations_connected(state, load_st, unload_st) {
            reapply_built_corridor(state, load_st, unload_st, &corridor);
            if !rail_stations_connected(state, load_st, unload_st) {
                depot_out = None;
            }
        }
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
    if has_load && has_unload && rail_stations_connected(state, load_st, unload_st) {
        Some((load_st, unload_st, depot))
    } else {
        None
    }
}

/// 2ª/3ª ruta: empalmar a un vecino Rail de la red existente (la estación hub
/// puede no aceptar el eje del spur — p. ej. andén E-W vs spur N-S).
fn try_spur_to_existing_network(
    state: &mut GameState,
    load_st: TileCoord,
    unload_st: TileCoord,
    existing_ai: &[TileCoord],
) -> Option<BuiltCorridor> {
    if rail_stations_connected(state, load_st, unload_st) {
        return None;
    }
    let mut join_targets: Vec<TileCoord> = Vec::new();
    for &hub in existing_ai {
        if hub == load_st || hub == unload_st {
            continue;
        }
        if !rail_stations_connected(state, hub, unload_st) {
            continue;
        }
        for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
            let n = TileCoord::new(hub.x + dx, hub.y + dy);
            if state.map.get_kind(n) == Some(TileKind::Rail)
                && find_path(&state.map, n, unload_st, PathNetwork::Rail).is_some()
            {
                join_targets.push(n);
            }
        }
        join_targets.push(hub);
    }
    for join in join_targets {
        let spur = place_rail_corridor(state, load_st, join);
        reapply_built_corridor(state, load_st, join, &spur);
        if !rail_stations_connected(state, load_st, unload_st)
            && find_path(&state.map, load_st, join, PathNetwork::Rail).is_none()
        {
            flatten_build_band(state, load_st, join);
            place_rail_manhattan_corridor(state, load_st, join);
            reapply_l_corner(state, load_st, join);
        }
        if rail_stations_connected(state, load_st, unload_st) {
            return Some(spur);
        }
    }
    None
}

/// Reutilizar un depósito ya enlazado a la red si el nuevo no cabe (2ª ruta).
fn find_rail_depot_linked_to(state: &GameState, load_st: TileCoord) -> Option<TileCoord> {
    let ai_id = state.active_company;
    let (mw, mh) = state.map.dimensions();
    for y in 0..mh.cast_signed() {
        for x in 0..mw.cast_signed() {
            let c = TileCoord::new(x, y);
            if state.map.get_kind(c) != Some(TileKind::RailDepot) {
                continue;
            }
            // Solo reutilizar depósitos propios.
            if let Some(tile) = state.map.get(c) {
                let owner = crate::company::CompanyId::from_tile_m1(tile.m1, state.companies.len());
                if owner != ai_id {
                    continue;
                }
            }
            if find_path(&state.map, c, load_st, PathNetwork::Rail).is_some() {
                return Some(c);
            }
        }
    }
    None
}

/// Reafirma el corredor entre estaciones (cura huecos del drenado progresivo).
pub(crate) fn repair_freight_corridor(
    state: &mut GameState,
    ai_id: CompanyId,
    load_st: TileCoord,
    unload_st: TileCoord,
) -> bool {
    with_ai_active(state, ai_id, |state| {
        let mut corridor = place_rail_corridor(state, load_st, unload_st);
        reapply_built_corridor(state, load_st, unload_st, &corridor);
        if !rail_stations_connected(state, load_st, unload_st) {
            flatten_build_band(state, load_st, unload_st);
            place_rail_manhattan_corridor(state, load_st, unload_st);
            reapply_l_corner(state, load_st, unload_st);
            corridor = BuiltCorridor::Manhattan;
            reapply_built_corridor(state, load_st, unload_st, &corridor);
        }
        reapply_corridor_corners(state, load_st, unload_st, &corridor);
    });
    rail_stations_connected(state, load_st, unload_st)
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

/// Pathfind de buildables (#184); si falla o el tendido queda incompleto → L Manhattan.
fn place_rail_corridor(state: &mut GameState, from: TileCoord, to: TileCoord) -> BuiltCorridor {
    if let Some(path) = find_rail_build_path(&state.map, from, to)
        && path.len() >= 2
    {
        flatten_path_band(state, &path);
        if place_rail_along_path(state, &path) && path_has_rail_or_station(state, &path) {
            return BuiltCorridor::Path(path);
        }
    }
    flatten_build_band(state, from, to);
    place_rail_manhattan_corridor(state, from, to);
    BuiltCorridor::Manhattan
}

/// Reaplica el corredor ya elegido (mismo path / mismo L). No vuelve a pathfind.
fn reapply_built_corridor(
    state: &mut GameState,
    from: TileCoord,
    to: TileCoord,
    corridor: &BuiltCorridor,
) {
    match corridor {
        BuiltCorridor::Path(path) => {
            let _ = place_rail_along_path(state, path);
            reapply_path_corners(state, path);
        }
        BuiltCorridor::Manhattan => {
            place_rail_manhattan_corridor(state, from, to);
            reapply_l_corner(state, from, to);
        }
    }
}

fn rail_stations_connected(state: &GameState, a: TileCoord, b: TileCoord) -> bool {
    find_path(&state.map, a, b, PathNetwork::Rail).is_some()
}

fn path_has_rail_or_station(state: &GameState, path: &[TileCoord]) -> bool {
    path.iter().all(|&c| {
        matches!(
            state.map.get_kind(c),
            Some(TileKind::Rail | TileKind::Station | TileKind::RailDepot)
        )
    })
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
///
/// Devuelve `false` si alguna tesela intermedia no pudo tenderse.
fn place_rail_along_path(state: &mut GameState, path: &[TileCoord]) -> bool {
    if path.len() < 2 {
        return false;
    }
    let mut ok = true;
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
        let placed = match (prev, next) {
            (Some(p), Some(n)) => {
                let d_in = (c.x - p.x, c.y - p.y);
                let d_out = (n.x - c.x, n.y - c.y);
                if d_in == d_out {
                    place_rail_axis(state, c, d_in.1 != 0, p)
                } else if (d_in.0 != 0 && d_out.1 != 0) || (d_in.1 != 0 && d_out.0 != 0) {
                    place_path_corner_curve(state, c, p, n)
                } else {
                    place_rail_axis(state, c, d_in.1 != 0, p)
                }
            }
            (None, Some(other)) | (Some(other), None) => {
                let axis_y = c.x == other.x;
                place_rail_axis(state, c, axis_y, other)
            }
            (None, None) => true,
        };
        ok &= placed;
    }
    ok
}

/// `DiagDir` de [`crate::map::diag_dir_offset`]: W=0, S=1, E=2, N=3.
fn pathfinder_diag_from_step(dx: i32, dy: i32) -> u8 {
    match (dx.signum(), dy.signum()) {
        (-1, 0) => 0,
        (0, 1) => 1,
        (1, 0) => 2,
        (0, -1) => 3,
        _ => 0,
    }
}

fn place_path_corner_curve(
    state: &mut GameState,
    corner: TileCoord,
    from_prev: TileCoord,
    to_next: TileCoord,
) -> bool {
    if matches!(
        state.map.get_kind(corner),
        Some(TileKind::Station | TileKind::RailDepot)
    ) {
        return true;
    }
    let step_in = (
        (corner.x - from_prev.x).signum(),
        (corner.y - from_prev.y).signum(),
    );
    let step_out = (
        (to_next.x - corner.x).signum(),
        (to_next.y - corner.y).signum(),
    );
    // Lado de entrada = opuesto al sentido de llegada; salida = sentido de marcha.
    // (Usar el sentido de marcha en ambos lados produce RIGHT también en giros Y→X,
    // que YAPF no atraviesa; Y→X necesita LEFT.)
    let travel_in = pathfinder_diag_from_step(step_in.0, step_in.1);
    let travel_out = pathfinder_diag_from_step(step_out.0, step_out.1);
    let curve = rail_bit_for_sides(opposite_diag_dir(travel_in), travel_out);
    if curve == 0 {
        return false;
    }
    // Nivelar al ancla del tramo si hace falta y crear la curva.
    if apply_command(state, &Command::PlaceRailBits(corner, curve)).is_err() {
        let _ = apply_command(
            state,
            &Command::LevelLand {
                from: from_prev,
                to: corner,
                mode: LevelMode::Level,
            },
        );
        if apply_command(state, &Command::PlaceRailBits(corner, curve)).is_err() {
            return false;
        }
    }
    // Solo la curva: OR con ejes X|Y crea cruces o piezas que no conectan el giro.
    let existing = state
        .map
        .get(corner)
        .filter(|t| t.kind == TileKind::Rail)
        .map_or(0, |t| t.m5 & 0x3F);
    if existing != curve {
        let _ = apply_command(state, &Command::SetRailBits(corner, curve));
    }
    state.map.get_kind(corner) == Some(TileKind::Rail)
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

fn place_rail_axis(
    state: &mut GameState,
    c: TileCoord,
    axis_y: bool,
    height_anchor: TileCoord,
) -> bool {
    if matches!(
        state.map.get_kind(c),
        Some(TileKind::Station | TileKind::RailDepot)
    ) {
        return true;
    }
    let bits = if axis_y { 0x02 } else { 0x01 };
    if apply_command(state, &Command::PlaceRailBits(c, bits)).is_ok() {
        return true;
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
    apply_command(state, &Command::PlaceRailBits(c, bits)).is_ok()
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
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::many_single_char_names
)]
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
        assert_eq!(
            tb, 0x20,
            "codo L debe ser curva RIGHT, no cruce: m5={tb:#04x}"
        );
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
        let start = TileCoord::new(2, 2);
        let end = TileCoord::new(10, 4);
        let planned = find_rail_build_path(&state.map, start, end).expect("A* build");
        let corridor = place_rail_corridor(&mut state, start, end);
        let BuiltCorridor::Path(path) = corridor else {
            panic!("debe usar pathfind para rodear agua");
        };
        assert_eq!(path, planned);
        for &tile in &path {
            assert_ne!(
                state.map.get_kind(tile),
                Some(TileKind::Water),
                "path no debe incluir agua {tile:?}"
            );
        }
        let rail_on_path = path
            .iter()
            .filter(|tile| state.map.get_kind(**tile) == Some(TileKind::Rail))
            .count();
        assert!(
            rail_on_path >= path.len().saturating_sub(1),
            "casi todo el path debe ser Rail; rail={rail_on_path} path={path:?}"
        );
        for i in 1..path.len() - 1 {
            let prev = path[i - 1];
            let corner = path[i];
            let next = path[i + 1];
            let turn =
                (corner.x - prev.x, corner.y - prev.y) != (next.x - corner.x, next.y - corner.y);
            if !turn || state.map.get_kind(corner) != Some(TileKind::Rail) {
                continue;
            }
            let tb = state.map.get(corner).unwrap().m5 & 0x3F;
            assert_ne!(
                tb, 0x03,
                "codo pathfind no debe ser CROSS en {corner:?}: {tb:#04x}"
            );
            assert_ne!(
                tb & 0x03,
                0x03,
                "codo no debe tener X|Y en {corner:?}: {tb:#04x}"
            );
        }
    }

    #[test]
    fn reconnect_after_stations_reuses_same_path_and_stays_connected() {
        // Regresión: un segundo A* distinto dejaba tramos huérfanos tras estaciones.
        let mut state = GameState::new(16, 14);
        state.economy.money = 500_000;
        // Bosque denso: el A* rodea; el polyline debe ser estable al reaplicar.
        for x in 4..10 {
            for y in 3..6 {
                state
                    .map
                    .set_kind(TileCoord::new(x, y), TileKind::Forest)
                    .unwrap();
            }
        }
        let load = TileCoord::new(2, 4);
        let unload = TileCoord::new(12, 8);
        let corridor = place_rail_corridor(&mut state, load, unload);
        let BuiltCorridor::Path(path) = &corridor else {
            panic!("esperado pathfind alrededor del bosque");
        };
        let path_len = path.len();
        let ai = CompanyId::PLAYER;
        assert!(place_rail_station_owned(&mut state, load, ai));
        assert!(place_rail_station_owned(&mut state, unload, ai));
        reapply_built_corridor(&mut state, load, unload, &corridor);
        assert!(
            rail_stations_connected(&state, load, unload),
            "estaciones deben quedar unidas por el mismo polyline; path={path:?}"
        );
        // No debe aparecer una segunda red Manhattan ajena al path (muchas rails extra).
        let rail_n = (0..16)
            .flat_map(|x| (0..14).map(move |y| TileCoord::new(x, y)))
            .filter(|&c| {
                matches!(
                    state.map.get_kind(c),
                    Some(TileKind::Rail | TileKind::Station)
                )
            })
            .count();
        assert!(
            rail_n <= path_len + 6,
            "demasiadas teselas de vía/estación (doble tendido?): rail_n={rail_n} path_len={path_len}"
        );
    }

    #[test]
    fn path_corner_y_then_x_is_traversable() {
        let mut state = GameState::new(8, 8);
        state.economy.money = 500_000;
        let path = [
            TileCoord::new(2, 2),
            TileCoord::new(2, 3),
            TileCoord::new(2, 4),
            TileCoord::new(3, 4),
            TileCoord::new(4, 4),
        ];
        assert!(place_rail_along_path(&mut state, &path));
        let corner = TileCoord::new(2, 4);
        let tb = state.map.get(corner).expect("corner").m5 & 0x3F;
        assert_eq!(tb, 0x10, "Y→X debe ser LEFT, no RIGHT: m5={tb:#04x}");
        assert!(
            find_path(
                &state.map,
                TileCoord::new(2, 2),
                TileCoord::new(4, 4),
                PathNetwork::Rail
            )
            .is_some(),
            "codo Y→X debe ser transitable"
        );
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
        assert_eq!(
            tb, 0x22,
            "codo compartido = RIGHT|Y, no cruce: m5={tb:#04x}"
        );
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

    #[test]
    fn find_rail_depot_linked_to_filters_own_depots_only() {
        use crate::command::{Command, apply_command};

        let mut state = GameState::new(16, 16);
        state.economy.money = 500_000;

        // Crear una compañía rival.
        state.ensure_rival_transcargo();
        let rival = crate::company::CompanyId(1);

        // Rival crea un depósito conectado a una vía.
        state.set_active_company(rival);
        for x in 2..=6_i32 {
            apply_command(&mut state, &Command::PlaceRail(TileCoord::new(x, 4))).unwrap();
        }
        let rival_depot = TileCoord::new(4, 5);
        apply_command(&mut state, &Command::PlaceRailDepotDir(rival_depot, 3)).unwrap();

        // Jugador crea vías en otra ubicación sin depósito propio.
        state.set_active_company(crate::company::CompanyId::PLAYER);
        let player_station = TileCoord::new(5, 8);
        for x in 3..=7_i32 {
            apply_command(&mut state, &Command::PlaceRail(TileCoord::new(x, 8))).unwrap();
        }

        // La búsqueda no debe devolver el depósito del rival.
        let result = find_rail_depot_linked_to(&state, player_station);
        assert!(
            result.is_none(),
            "no debe reutilizar depósito de otra compañía"
        );

        // Ahora el jugador crea su propio depósito conectado a sus vías.
        let player_depot = TileCoord::new(5, 9);
        apply_command(&mut state, &Command::PlaceRailDepotDir(player_depot, 3)).unwrap();

        // Ahora la búsqueda debe encontrar el depósito del jugador.
        let result = find_rail_depot_linked_to(&state, player_station);
        assert_eq!(
            result,
            Some(player_depot),
            "debe encontrar el depósito propio"
        );
    }
}
