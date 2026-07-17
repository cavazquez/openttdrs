//! Construcción de infraestructura `TransCargo`: vías, señales, estaciones y depósitos.

use crate::GameState;
use crate::command::{Command, LevelMode, apply_command};
use crate::company::CompanyId;
use crate::map::rail_bit_for_sides;
use crate::map::{RAIL_TB_Y, TileCoord, TileKind};
use crate::pathfinder::{PathNetwork, find_path};
use crate::rail_signals::{SIGTYPE_BLOCK, rail_tile_is_signals};

use super::plan::{RoutePlan, pick_station_tile};

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
        flatten_build_band(state, load_st, unload_st);
        place_rail_manhattan_corridor(state, load_st, unload_st);
        if !place_rail_station_owned(state, load_st, ai_id) {
            return;
        }
        if !place_rail_station_owned(state, unload_st, ai_id) {
            return;
        }
        // Reconectar vía bajo/alrededor de las estaciones.
        place_rail_manhattan_corridor(state, load_st, unload_st);
        place_corridor_block_signals(state, load_st, unload_st);
        depot_out = try_place_depot_near(state, load_st);
        // El depósito (autorraíl) puede refrescar vecinos: reafirmar el codo L.
        reapply_l_corner(state, load_st, unload_st);
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
