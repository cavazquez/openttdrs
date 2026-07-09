//! Señales ferroviarias de bloque (v1): colocación, bloques y simulación simple.

use std::collections::{HashMap, HashSet};

use crate::map::{Map, TileCoord, TileKind};
use crate::news::{CALENDAR_BASE_YEAR, calendar_day_index, calendar_year_day};
use crate::station::is_rail_waypoint_tile;
use crate::tick::GameTick;
use crate::vehicle::{Vehicle, VehicleKind};

/// Subtipo de tesela ferroviaria en bits 6–7 de `m5` (`RailTileType`).
pub const RAIL_TILE_NORMAL: u8 = 0;
pub const RAIL_TILE_SIGNALS: u8 = 1;

const RAIL_TB_X: u8 = 0x01;
const RAIL_TB_Y: u8 = 0x02;
const RAIL_TB_UPPER: u8 = 0x04;
const RAIL_TB_LOWER: u8 = 0x08;
const RAIL_TB_LEFT: u8 = 0x10;
const RAIL_TB_RIGHT: u8 = 0x20;
const RAIL_TB_HORZ: u8 = RAIL_TB_UPPER | RAIL_TB_LOWER;
const RAIL_TB_VERT: u8 = RAIL_TB_LEFT | RAIL_TB_RIGHT;

/// Pieza de vía sobre la que se coloca una señal (`Track` en `track_type.h`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SignalTrack {
    X = 0,
    Y = 1,
    Upper = 2,
    Lower = 3,
    Left = 4,
    Right = 5,
}

impl SignalTrack {
    #[must_use]
    pub const fn track_bit(self) -> u8 {
        match self {
            Self::X => RAIL_TB_X,
            Self::Y => RAIL_TB_Y,
            Self::Upper => RAIL_TB_UPPER,
            Self::Lower => RAIL_TB_LOWER,
            Self::Left => RAIL_TB_LEFT,
            Self::Right => RAIL_TB_RIGHT,
        }
    }

    #[must_use]
    const fn ottd_track(self) -> u8 {
        self as u8
    }

    #[must_use]
    const fn from_track_bit(bit: u8) -> Option<Self> {
        match bit {
            RAIL_TB_X => Some(Self::X),
            RAIL_TB_Y => Some(Self::Y),
            RAIL_TB_UPPER => Some(Self::Upper),
            RAIL_TB_LOWER => Some(Self::Lower),
            RAIL_TB_LEFT => Some(Self::Left),
            RAIL_TB_RIGHT => Some(Self::Right),
            _ => None,
        }
    }

    /// `(DiagDir, sig_bit)` — alineado con `DrawSignals` / `_signal_along_trackdir`.
    #[must_use]
    const fn facings(self) -> &'static [(u8, u8)] {
        match self {
            Self::X => &[(0, 2), (2, 3)],
            Self::Y | Self::Left => &[(3, 2), (1, 3)],
            Self::Upper => &[(3, 3), (0, 2)],
            Self::Lower => &[(2, 0), (1, 1)],
            Self::Right => &[(3, 0), (1, 1)],
        }
    }
}

/// Más de un carril incompatible en la tesela (no señales en cruces).
#[must_use]
pub fn tracks_overlap(bits: u8) -> bool {
    if bits.count_ones() <= 1 {
        return false;
    }
    bits != RAIL_TB_HORZ && bits != RAIL_TB_VERT
}

/// Elige la pieza de vía bajo el cursor (`GenericPlaceSignals` en `rail_gui.cpp`).
#[must_use]
pub fn resolve_signal_track(trackbits: u8, fract_x: u8, fract_y: u8) -> Option<SignalTrack> {
    if tracks_overlap(trackbits) {
        return None;
    }
    let mut selected = trackbits;
    if selected & RAIL_TB_VERT != 0 {
        let pick = if fract_x <= fract_y {
            RAIL_TB_RIGHT
        } else {
            RAIL_TB_LEFT
        };
        if selected & pick == 0 {
            return None;
        }
        selected = pick;
    } else if selected & RAIL_TB_HORZ != 0 {
        let pick = if u16::from(fract_x) + u16::from(fract_y) <= 256 {
            RAIL_TB_UPPER
        } else {
            RAIL_TB_LOWER
        };
        if selected & pick == 0 {
            return None;
        }
        selected = pick;
    }
    SignalTrack::from_track_bit(selected)
}

/// Coste de colocación de una señal de bloque (`Price::BuildSignal` aprox.).
pub const SIGNAL_BUILD_COST: i64 = 40;
/// Reembolso parcial al quitar vía (`Price::ClearRail` aprox.).
pub const RAIL_REMOVE_REFUND: i64 = 10;
/// Reembolso al quitar una señal (mitad del coste de colocación).
pub const SIGNAL_REMOVE_REFUND: i64 = 20;

pub const SIGTYPE_BLOCK: u8 = 0;
pub const SIGTYPE_ENTRY: u8 = 1;
pub const SIGTYPE_EXIT: u8 = 2;
pub const SIGTYPE_COMBO: u8 = 3;
pub const SIGTYPE_PATH: u8 = 4;
pub const SIGTYPE_PATH_ONEWAY: u8 = 5;
pub const SIGTYPE_LAST_NOPBS: u8 = 3;

/// `true` si el tipo usa lógica PBS (`IsPbsSignal` en `OpenTTD`).
#[must_use]
pub const fn is_pbs_signal_type(sig_type: u8) -> bool {
    sig_type >= SIGTYPE_PATH
}

/// Siguiente tipo al ciclar con Ctrl (`CycleSignalType` en `rail_cmd.cpp`).
///
/// Orden `OpenTTD`: block → entry → exit → combo → path → path oneway → block.
#[must_use]
pub const fn next_placeable_signal_type(current: u8) -> u8 {
    match current {
        SIGTYPE_BLOCK => SIGTYPE_ENTRY,
        SIGTYPE_ENTRY => SIGTYPE_EXIT,
        SIGTYPE_EXIT => SIGTYPE_COMBO,
        SIGTYPE_COMBO => SIGTYPE_PATH,
        SIGTYPE_PATH => SIGTYPE_PATH_ONEWAY,
        _ => SIGTYPE_BLOCK,
    }
}

/// Nombre corto del tipo para UI / logs.
#[must_use]
pub const fn signal_type_label(sig_type: u8) -> &'static str {
    match sig_type {
        SIGTYPE_ENTRY => "entry",
        SIGTYPE_EXIT => "exit",
        SIGTYPE_COMBO => "combo",
        SIGTYPE_PATH => "path",
        SIGTYPE_PATH_ONEWAY => "path 1vía",
        _ => "block",
    }
}

/// Año calendario a partir del cual se colocan señales eléctricas por defecto
/// (`gui.semaphore_build_before` en `OpenTTD`).
pub const SEMAPHORE_BUILD_BEFORE_YEAR: u32 = CALENDAR_BASE_YEAR;

#[must_use]
pub fn calendar_year_at_tick(tick: GameTick) -> u32 {
    calendar_year_day(calendar_day_index(tick)).0
}

/// `0` = semáforo, `1` = eléctrica (`SignalVariant` en `OpenTTD`).
#[must_use]
pub fn default_signal_variant(year: u32) -> u8 {
    u8::from(year >= SEMAPHORE_BUILD_BEFORE_YEAR)
}

#[must_use]
pub fn rail_tile_is_signals(m5: u8) -> bool {
    (m5 >> 6) & 0x3 == RAIL_TILE_SIGNALS
}

#[must_use]
pub fn rail_signal_present_mask(m3: u8) -> u8 {
    (m3 >> 4) & 0x0F
}

#[must_use]
pub fn rail_signal_state_mask(m3hi: u8) -> u8 {
    (m3hi >> 4) & 0x0F
}

#[must_use]
pub fn signal_is_green(m3hi: u8, sig_bit: u8) -> bool {
    (rail_signal_state_mask(m3hi) >> sig_bit) & 1 != 0
}

/// Trackbits transitables (misma lógica que el pathfinder).
#[must_use]
pub(crate) fn rail_traversal_bits(map: &Map, c: TileCoord) -> u8 {
    let Some(t) = map.get(c) else {
        return 0;
    };
    match t.kind {
        TileKind::Rail => {
            let tb = t.m5 & 0x3F;
            if tb == 0 { RAIL_TB_X } else { tb }
        }
        TileKind::RailTunnel | TileKind::RailBridge => RAIL_TB_X | RAIL_TB_Y,
        TileKind::Station if is_rail_station_tile_kind(&t) || is_rail_waypoint_tile(&t) => {
            if t.m5 & 1 != 0 { RAIL_TB_Y } else { RAIL_TB_X }
        }
        _ => 0,
    }
}

#[must_use]
const fn opposite_dir(d: u8) -> u8 {
    (d + 2) & 3
}

#[must_use]
const fn diag_dir_offset(d: u8) -> (i32, i32) {
    match d & 3 {
        0 => (1, 0),
        1 => (0, 1),
        2 => (-1, 0),
        _ => (0, -1),
    }
}

#[must_use]
const fn rail_bits_touching_side(side: u8) -> u8 {
    match side & 3 {
        0 => 0x25,
        1 => 0x2A,
        2 => 0x19,
        _ => 0x16,
    }
}

#[must_use]
pub(crate) fn rail_neighbors(map: &Map, cur: TileCoord, prev: Option<TileCoord>) -> Vec<TileCoord> {
    let tb = rail_traversal_bits(map, cur);
    if tb == 0 {
        return Vec::new();
    }
    let mut out = Vec::new();
    for dir in 0..4u8 {
        let (dx, dy) = diag_dir_offset(dir);
        let next = TileCoord::new(cur.x + dx, cur.y + dy);
        if prev == Some(next) {
            continue;
        }
        if tb & rail_bits_touching_side(dir) == 0 {
            continue;
        }
        let entry = opposite_dir(dir);
        if rail_traversal_bits(map, next) & rail_bits_touching_side(entry) != 0 {
            out.push(next);
        }
    }
    out
}

#[must_use]
pub(crate) fn dir_from_to(from: TileCoord, to: TileCoord) -> Option<u8> {
    let dx = to.x - from.x;
    let dy = to.y - from.y;
    match (dx, dy) {
        (1, 0) => Some(0),
        (0, 1) => Some(1),
        (-1, 0) => Some(2),
        (0, -1) => Some(3),
        _ => None,
    }
}

#[must_use]
fn rail_continuation_along(
    map: &Map,
    cur: TileCoord,
    prev: TileCoord,
    preferred_dir: u8,
) -> Option<TileCoord> {
    let neighbors: Vec<_> = rail_neighbors(map, cur, Some(prev))
        .into_iter()
        .filter(|n| *n != prev)
        .collect();
    match neighbors.len() {
        0 => None,
        1 => Some(neighbors[0]),
        _ => neighbors
            .into_iter()
            .find(|n| dir_from_to(cur, *n) == Some(preferred_dir)),
    }
}

/// Teselas de conector en un cruce: ramas perpendiculares a la vía del bloque.
///
/// Sin esto, un tren que gira a la vía perpendicular (p. ej. `(10,5)` en el escenario
/// dual) deja de contar como ocupación y la señal pasa a verde con el bloque aún en uso.
fn junction_spur_tiles(map: &Map, block: &[TileCoord], exit_dir: u8) -> Vec<TileCoord> {
    if block.is_empty() {
        return Vec::new();
    }
    let block_set: HashSet<TileCoord> = block.iter().copied().collect();
    let mut spurs = Vec::new();
    for (i, &tile) in block.iter().enumerate() {
        let forward = if i == 0 {
            exit_dir
        } else {
            dir_from_to(block[i - 1], tile).unwrap_or(exit_dir)
        };
        let back = opposite_dir(forward);
        for n in rail_neighbors(map, tile, None) {
            if block_set.contains(&n) || spurs.contains(&n) {
                continue;
            }
            let Some(d) = dir_from_to(tile, n) else {
                continue;
            };
            if d != forward && d != back {
                spurs.push(n);
            }
        }
    }
    spurs
}

/// Teselas del bloque protegido al salir de `signal_tile` hacia `exit_dir`.
#[must_use]
pub fn rail_block_ahead(map: &Map, signal_tile: TileCoord, exit_dir: u8) -> Vec<TileCoord> {
    let (dx, dy) = diag_dir_offset(exit_dir);
    let start = TileCoord::new(signal_tile.x + dx, signal_tile.y + dy);
    if map.get(start).is_none() {
        return Vec::new();
    }
    let mut block = vec![start];
    let mut cur = start;
    let mut prev = signal_tile;
    let mut forward = exit_dir;
    while let Some(next) = rail_continuation_along(map, cur, prev, forward) {
        if map
            .get(next)
            .is_some_and(|t| t.kind == TileKind::Rail && rail_tile_is_signals(t.m5))
        {
            break;
        }
        block.push(next);
        if let Some(dir) = dir_from_to(cur, next) {
            forward = dir;
        }
        prev = cur;
        cur = next;
    }
    block.extend(junction_spur_tiles(map, &block, exit_dir));
    block
}

#[must_use]
fn is_rail_station_tile_kind(tile: &crate::map::Tile) -> bool {
    tile.kind == TileKind::Station && (tile.m6 >> 3).trailing_zeros() >= 4
}

/// `true` si algún tren ocupa el bloque protegido por la señal en `signal_tile`.
///
/// Un tren que está sobre `signal_tile` aún no entró al bloque, así que su
/// `movement_target` no lo reserva: de lo contrario un tren detenido sobre su
/// propia señal la pondría en rojo y no podría salir (deadlock).
#[must_use]
fn block_is_occupied_by_trains(
    vehicles: &[Vehicle],
    signal_tile: TileCoord,
    block: &[TileCoord],
) -> bool {
    let block_set: HashSet<TileCoord> = block.iter().copied().collect();
    for v in vehicles {
        if v.kind != VehicleKind::Train {
            continue;
        }
        if block_set.contains(&v.pos) {
            return true;
        }
        if !v.running || v.pos == signal_tile {
            continue;
        }
        if let Some(next) = v.movement_target()
            && block_set.contains(&next)
        {
            return true;
        }
    }
    false
}

/// Resultado de evaluar señales al planificar ruta (YAPF `SignalCost` simplificado).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum YapfSignalRouting {
    /// Sin señal relevante en la dirección de marcha.
    Clear,
    /// Penalización por señal roja (el camino sigue siendo válido).
    Penalty(u32),
    /// Sentido único en contra: rama inválida (`EndSegmentReason::DeadEnd`).
    DeadEnd,
}

/// Penalización YAPF por señal de bloque roja (aprox. `rail_firstred_penalty`).
pub const YAPF_RED_SIGNAL_PENALTY: u32 = 100;
/// Penalización YAPF por cruzar una señal path por detrás (`yapf_costrail.hpp`).
pub const YAPF_PBS_BEHIND_PENALTY: u32 = 100;

/// Evalúa señales al planificar salida de `tile` en `exit_dir` (convención `OpenTTD` / `rail_signals`).
///
/// Replica la regla central de `CYapfCostRailT::SignalCost`: señal unidireccional solo en
/// sentido contrario → callejón sin salida; roja a favor → penalización.
#[must_use]
pub fn yapf_routing_signal(map: &Map, tile: TileCoord, exit_dir: u8) -> YapfSignalRouting {
    let Some(t) = map.get(tile) else {
        return YapfSignalRouting::Clear;
    };
    if t.kind != TileKind::Rail || !rail_tile_is_signals(t.m5) {
        return YapfSignalRouting::Clear;
    }
    let rails = t.m5 & 0x3F;
    let present = rail_signal_present_mask(t.m3);
    if present == 0 {
        return YapfSignalRouting::Clear;
    }

    let mut along = false;
    let mut against = false;
    let mut red_penalty = 0u32;
    for bit in 0..4u8 {
        if present & (1 << bit) == 0 {
            continue;
        }
        let sig_type = signal_track_for_bit(rails, bit)
            .map_or(SIGTYPE_BLOCK, |track| signal_type_for_track(t.m2, track));
        let sig_exit = signal_exit_dir(rails, bit);
        if sig_exit == exit_dir {
            along = true;
            if signal_track_for_bit(rails, bit)
                .is_some_and(|track| signal_type_for_track(t.m2, track) == SIGTYPE_ENTRY)
            {
                continue;
            }
            if is_pbs_signal_type(sig_type) {
                if !signal_is_green(t.m3hi, bit) {
                    red_penalty = red_penalty.saturating_add(YAPF_RED_SIGNAL_PENALTY);
                }
                continue;
            }
            if !signal_is_green(t.m3hi, bit) {
                red_penalty = red_penalty.saturating_add(YAPF_RED_SIGNAL_PENALTY);
            }
        } else if sig_exit == opposite_dir(exit_dir) {
            against = true;
            if sig_type == SIGTYPE_PATH {
                red_penalty = red_penalty.saturating_add(YAPF_PBS_BEHIND_PENALTY);
            }
        }
    }

    let is_oneway = present.is_power_of_two()
        || (0..4).any(|bit| {
            present & (1 << bit) != 0
                && signal_track_for_bit(rails, bit)
                    .map(|track| signal_type_for_track(t.m2, track))
                    .is_some_and(|ty| ty == SIGTYPE_PATH_ONEWAY)
        });
    if against && !along && is_oneway {
        return YapfSignalRouting::DeadEnd;
    }
    if red_penalty > 0 {
        YapfSignalRouting::Penalty(red_penalty)
    } else {
        YapfSignalRouting::Clear
    }
}

/// Bits de señal presentes que controlan la salida `from` → `to`.
#[must_use]
fn signal_bits_for_exit(map: &Map, from: TileCoord, to: TileCoord) -> Vec<u8> {
    let Some(tile) = map.get(from) else {
        return Vec::new();
    };
    if tile.kind != TileKind::Rail || !rail_tile_is_signals(tile.m5) {
        return Vec::new();
    }
    let exit_dir = dir_from_to(from, to).unwrap_or(0);
    let rails = tile.m5 & 0x3F;
    let present = rail_signal_present_mask(tile.m3);
    (0..4u8)
        .filter(|&bit| present & (1 << bit) != 0 && signal_exit_dir(rails, bit) == exit_dir)
        .collect()
}

/// Siguiente tesela tras `via` según el path del tren (o `dest` si es el último salto).
#[must_use]
fn path_continuation_after(vehicle: &Vehicle, via: TileCoord) -> Option<TileCoord> {
    if vehicle.path.front() != Some(&via) {
        return None;
    }
    if let Some(&next) = vehicle.path.get(1) {
        return Some(next);
    }
    if vehicle.dest != via {
        return Some(vehicle.dest);
    }
    None
}

/// `true` si la salida `signal_tile` → `beyond` está prohibida (rojo u ocupado).
///
/// Path / `PathOneWay`: el verde se deriva de la reserva PBS; no se exige verde previa
/// ni se usa ocupación de bloque (evita deadlock reserva↔rojo). `PathOneWay` en sentido
/// contrario ya es `DeadEnd` vía `yapf_routing_signal`.
#[must_use]
fn signal_exit_denied(
    map: &Map,
    vehicles: &[Vehicle],
    signal_tile: TileCoord,
    beyond: TileCoord,
    tile: &crate::map::Tile,
) -> bool {
    let exit_dir = dir_from_to(signal_tile, beyond).unwrap_or(0);
    if matches!(
        yapf_routing_signal(map, signal_tile, exit_dir),
        YapfSignalRouting::DeadEnd
    ) {
        return true;
    }
    let mut checked = false;
    for bit in signal_bits_for_exit(map, signal_tile, beyond) {
        checked = true;
        let rails = tile.m5 & 0x3F;
        let sig_type = signal_track_for_bit(rails, bit)
            .map_or(SIGTYPE_BLOCK, |track| signal_type_for_track(tile.m2, track));
        // Path: el verde se deriva de la reserva; no bloquear por rojo/ocupación de bloque.
        if is_pbs_signal_type(sig_type) {
            continue;
        }
        if sig_type == SIGTYPE_ENTRY {
            if !signal_is_green(tile.m3hi, bit) {
                return true;
            }
            continue;
        }
        if !signal_is_green(tile.m3hi, bit) {
            return true;
        }
        let block = rail_block_ahead(map, signal_tile, exit_dir);
        if block_is_occupied_by_trains(vehicles, signal_tile, &block) {
            return true;
        }
    }
    // Ningún bit controla esta salida: ocupación de bloque solo para señales no-PBS
    // (path permite pasar por detrás; PathOneWay ya es DeadEnd arriba).
    if !checked && tile.kind == TileKind::Rail && rail_tile_is_signals(tile.m5) {
        let rails = tile.m5 & 0x3F;
        let present = rail_signal_present_mask(tile.m3);
        let has_non_pbs = (0..4u8).any(|bit| {
            present & (1 << bit) != 0
                && !is_pbs_signal_type(
                    signal_track_for_bit(rails, bit)
                        .map_or(SIGTYPE_BLOCK, |track| signal_type_for_track(tile.m2, track)),
                )
        });
        if has_non_pbs {
            let block = rail_block_ahead(map, signal_tile, exit_dir);
            if block_is_occupied_by_trains(vehicles, signal_tile, &block) {
                return true;
            }
        }
    }
    false
}

/// `true` si un paso `from` → `to` puede incluirse en una reserva PBS.
#[must_use]
pub(crate) fn rail_step_signal_allows(
    map: &Map,
    vehicles: &[Vehicle],
    from: TileCoord,
    to: TileCoord,
    beyond: Option<TileCoord>,
) -> bool {
    if let Some(sig_tile) = map.get(to)
        && sig_tile.kind == TileKind::Rail
        && rail_tile_is_signals(sig_tile.m5)
        && let Some(beyond) = beyond
        && signal_exit_denied(map, vehicles, to, beyond, &sig_tile)
    {
        return false;
    }
    let Some(from_tile) = map.get(from) else {
        return true;
    };
    if from_tile.kind != TileKind::Rail || !rail_tile_is_signals(from_tile.m5) {
        return true;
    }
    !signal_exit_denied(map, vehicles, from, to, &from_tile)
}

/// `true` si el avance sub-tesela de este tick completaría la tesela actual.
#[must_use]
fn train_would_complete_current_tile(vehicle: &Vehicle) -> bool {
    if vehicle.depart_turn > 0 {
        let step = u16::from(vehicle.progress_step().max(1));
        return u16::from(vehicle.depart_turn).saturating_add(step) >= 255;
    }
    if vehicle.progress == 255 && vehicle.needs_depart_turnaround() {
        return true;
    }
    let step = u16::from(vehicle.progress_step());
    if step == 0 {
        return false;
    }
    u16::from(vehicle.progress).saturating_add(step) >= 255
}

/// `true` si la salida path en `signal_tile` → `beyond` carece de reserva completa.
#[must_use]
fn path_exit_lacks_reservation(
    map: &Map,
    vehicle: &Vehicle,
    signal_tile: TileCoord,
    beyond: TileCoord,
    tile: &crate::map::Tile,
) -> bool {
    let exit_dir = dir_from_to(signal_tile, beyond).unwrap_or(0);
    if matches!(
        yapf_routing_signal(map, signal_tile, exit_dir),
        YapfSignalRouting::DeadEnd
    ) {
        return true;
    }
    let bits = signal_bits_for_exit(map, signal_tile, beyond);
    if bits.is_empty() {
        return false;
    }
    let rails = tile.m5 & 0x3F;
    let any_pbs = bits.iter().any(|&bit| {
        signal_track_for_bit(rails, bit)
            .is_some_and(|track| is_pbs_signal_type(signal_type_for_track(tile.m2, track)))
    });
    if !any_pbs {
        return false;
    }
    // Debe reservar `beyond` y llegar a una posición segura (TryReservePath).
    let has_beyond = vehicle.reserved_steps.iter().any(|s| s.tile == beyond);
    !(has_beyond && crate::rail_pbs::reservation_ends_at_safe_wait(map, vehicle))
}

/// `true` si el tren debe esperar ante la tesela de señal `to` (sin entrar al bloque).
#[must_use]
fn train_held_before_signal_tile(
    map: &Map,
    vehicles: &[Vehicle],
    vehicle: &Vehicle,
    to: TileCoord,
    signal_tile: &crate::map::Tile,
) -> bool {
    let Some(beyond) = path_continuation_after(vehicle, to) else {
        return false;
    };
    let denied = signal_exit_denied(map, vehicles, to, beyond, signal_tile)
        || path_exit_lacks_reservation(map, vehicle, to, beyond, signal_tile);
    if !denied {
        return false;
    }
    if vehicle.pos == to {
        return true;
    }
    // Detenido en la tesela de aproximación: mantener espera aunque `progress_step` sea 0.
    if vehicle.cur_speed == 0 && vehicle.progress > 0 {
        return true;
    }
    train_would_complete_current_tile(vehicle)
}

/// `true` si el tren no puede avanzar por falta de reserva PBS completa en path.
#[must_use]
pub fn train_blocked_by_pbs_path(map: &Map, vehicle: &Vehicle) -> bool {
    if vehicle.kind != VehicleKind::Train || !vehicle.running {
        return false;
    }
    let from = vehicle.pos;
    let Some(to) = vehicle.movement_target() else {
        return false;
    };

    if let Some(signal_tile) = map.get(to)
        && signal_tile.kind == TileKind::Rail
        && rail_tile_is_signals(signal_tile.m5)
        && let Some(beyond) = path_continuation_after(vehicle, to)
        && path_exit_lacks_reservation(map, vehicle, to, beyond, &signal_tile)
    {
        return true;
    }

    let Some(tile) = map.get(from) else {
        return false;
    };
    if tile.kind != TileKind::Rail || !rail_tile_is_signals(tile.m5) {
        return false;
    }
    path_exit_lacks_reservation(map, vehicle, from, to, &tile)
}

/// `true` si el tren no puede avanzar al siguiente paso por señal en rojo.
#[must_use]
pub fn train_blocked_by_signal(map: &Map, vehicles: &[Vehicle], vehicle: &Vehicle) -> bool {
    if vehicle.kind != VehicleKind::Train || !vehicle.running {
        return false;
    }
    let from = vehicle.pos;
    let Some(to) = vehicle.movement_target() else {
        return false;
    };

    if let Some(signal_tile) = map.get(to)
        && signal_tile.kind == TileKind::Rail
        && rail_tile_is_signals(signal_tile.m5)
        && train_held_before_signal_tile(map, vehicles, vehicle, to, &signal_tile)
    {
        return true;
    }

    let Some(tile) = map.get(from) else {
        return false;
    };
    if tile.kind != TileKind::Rail || !rail_tile_is_signals(tile.m5) {
        return false;
    }
    signal_exit_denied(map, vehicles, from, to, &tile)
        || path_exit_lacks_reservation(map, vehicle, from, to, &tile)
}

/// `true` si otro tren ocupa la vía delante (misma dirección o frente a frente).
#[must_use]
pub fn train_blocked_by_traffic(map: &Map, vehicles: &[Vehicle], vehicle: &Vehicle) -> bool {
    if vehicle.kind != VehicleKind::Train || !vehicle.running {
        return false;
    }
    let Some(next) = vehicle.movement_target() else {
        return false;
    };
    let self_id = vehicle.id;

    if vehicles
        .iter()
        .any(|v| v.id != self_id && v.kind == VehicleKind::Train && v.pos == next)
    {
        return true;
    }

    // Varios trenes pueden compartir la misma tesela de depósito (OpenTTD).
    if map.get_kind(vehicle.pos) != Some(crate::map::TileKind::RailDepot)
        && vehicles
            .iter()
            .any(|v| v.id != self_id && v.kind == VehicleKind::Train && v.pos == vehicle.pos)
    {
        return true;
    }

    let mut prev = vehicle.pos;
    let mut cur = next;
    for _ in 0..64 {
        if let Some(other) = vehicles
            .iter()
            .find(|v| v.id != self_id && v.kind == VehicleKind::Train && v.pos == cur)
        {
            if !other.running {
                return true;
            }
            if let Some(other_next) = other.movement_target() {
                if other_next == prev || other_next == vehicle.pos {
                    return true;
                }
            } else {
                return true;
            }
            return true;
        }

        let neighbors = rail_neighbors(map, cur, Some(prev));
        let continuations: Vec<_> = neighbors.into_iter().filter(|n| *n != prev).collect();
        if continuations.len() != 1 {
            break;
        }
        prev = cur;
        cur = continuations[0];
    }
    false
}

/// Resultado simplificado de `ProbeSigSeg` / `ExploreSegment` (`signal.cpp`).
///
/// Solo flags `Exit` (lista de exit/combo que cierran el segmento). Ocupación
/// (`Train`) sigue vía [`rail_block_ahead`] + [`block_is_occupied_by_trains`].
#[derive(Debug, Clone, Default)]
struct SigSegmentProbe {
    /// `(tile, signal_bit)` de exit/combo al final del segmento.
    exits: Vec<(TileCoord, u8)>,
}

/// Explora el segmento PBS/presignal desde `signal_tile` hacia `exit_dir`.
///
/// Paridad v0 de `ProbeSigSeg`: no atraviesa señales block/path/entry; al hallar
/// exit/combo registra y corta esa rama (no busca exits más allá de un block).
fn explore_sig_segment(map: &Map, signal_tile: TileCoord, exit_dir: u8) -> SigSegmentProbe {
    let (dx, dy) = diag_dir_offset(exit_dir);
    let start = TileCoord::new(signal_tile.x + dx, signal_tile.y + dy);
    let mut probe = SigSegmentProbe::default();
    let mut queue = vec![start];
    let mut visited = HashSet::from([signal_tile]);

    while let Some(cur) = queue.pop() {
        if !visited.insert(cur) {
            continue;
        }
        let Some(tile) = map.get(cur) else {
            continue;
        };
        if tile.kind != TileKind::Rail {
            continue;
        }
        if rail_tile_is_signals(tile.m5) {
            let present = rail_signal_present_mask(tile.m3);
            let rails = tile.m5 & 0x3F;
            let mut closes_segment = false;
            for bit in 0..4u8 {
                if present & (1 << bit) == 0 {
                    continue;
                }
                let sig_type = signal_track_for_bit(rails, bit)
                    .map_or(SIGTYPE_BLOCK, |track| signal_type_for_track(tile.m2, track));
                if sig_type == SIGTYPE_EXIT || sig_type == SIGTYPE_COMBO {
                    probe.exits.push((cur, bit));
                    closes_segment = true;
                } else {
                    // Block / path / entry: frontera del segmento.
                    closes_segment = true;
                }
            }
            if closes_segment {
                continue;
            }
        }
        for n in rail_neighbors(map, cur, None) {
            if !visited.contains(&n) {
                queue.push(n);
            }
        }
    }
    probe
}

fn presignal_exit_targets_ahead(
    map: &Map,
    signal_tile: TileCoord,
    exit_dir: u8,
) -> Vec<(TileCoord, u8)> {
    explore_sig_segment(map, signal_tile, exit_dir).exits
}

/// Propaga verde de combos hasta punto fijo (entry → combo → … → exit).
///
/// Pasada 1 deja combos como “solo bloque propio”; aquí aplican la regla entry
/// leyendo exits/combos aguas abajo ya estabilizados (arregla cadenas combo).
fn stabilize_combo_presignal_greens(
    map: &Map,
    vehicles: &[Vehicle],
    exit_green: &HashMap<(TileCoord, u8), bool>,
) -> HashMap<(TileCoord, u8), bool> {
    let mut greens = exit_green.clone();
    let (w, h) = map.dimensions();
    let mut combos = Vec::new();
    for y in 0..i32::try_from(h).unwrap_or(i32::MAX) {
        for x in 0..i32::try_from(w).unwrap_or(i32::MAX) {
            let c = TileCoord::new(x, y);
            let Some(tile) = map.get(c) else {
                continue;
            };
            if tile.kind != TileKind::Rail || !rail_tile_is_signals(tile.m5) {
                continue;
            }
            let present = rail_signal_present_mask(tile.m3);
            let rails = tile.m5 & 0x3F;
            for bit in 0..4u8 {
                if present & (1 << bit) == 0 {
                    continue;
                }
                let sig_type = signal_track_for_bit(rails, bit)
                    .map_or(SIGTYPE_BLOCK, |track| signal_type_for_track(tile.m2, track));
                if sig_type == SIGTYPE_COMBO {
                    combos.push((c, bit, tile));
                }
            }
        }
    }

    // Profundidad típica de árboles combo << 8; tope evita bucles patológicos.
    for _ in 0..8 {
        let mut changed = false;
        for &(c, bit, tile) in &combos {
            let rails = tile.m5 & 0x3F;
            let exit_dir = signal_exit_dir(rails, bit);
            let exit_ok = exit_green.get(&(c, bit)).copied().unwrap_or_else(|| {
                signal_bit_block_green(map, vehicles, c, &tile, bit, SIGTYPE_COMBO)
            });
            let targets = presignal_exit_targets_ahead(map, c, exit_dir);
            let entry_ok = if targets.is_empty() {
                true
            } else {
                targets
                    .iter()
                    .any(|key| greens.get(key).copied().unwrap_or(false))
            };
            let new_green = exit_ok && entry_ok;
            let prev = greens.insert((c, bit), new_green);
            if prev != Some(new_green) {
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    greens
}

fn signal_bit_block_green(
    map: &Map,
    vehicles: &[Vehicle],
    c: TileCoord,
    tile: &crate::map::Tile,
    bit: u8,
    sig_type: u8,
) -> bool {
    let exit_dir = signal_exit_dir(tile.m5 & 0x3F, bit);
    let block = rail_block_ahead(map, c, exit_dir);
    if is_pbs_signal_type(sig_type) {
        // Path verde solo con reserva válida hasta posición segura (TryReservePath OK).
        crate::rail_pbs::pbs_exit_has_complete_reservation(map, vehicles, c, exit_dir, &block)
    } else {
        !block_is_occupied_by_trains(vehicles, c, &block)
    }
}

fn compute_exit_signal_greens(map: &Map, vehicles: &[Vehicle]) -> HashMap<(TileCoord, u8), bool> {
    let (w, h) = map.dimensions();
    let mut exit_green = HashMap::new();
    for y in 0..i32::try_from(h).unwrap_or(i32::MAX) {
        for x in 0..i32::try_from(w).unwrap_or(i32::MAX) {
            let c = TileCoord::new(x, y);
            let Some(tile) = map.get(c) else {
                continue;
            };
            if tile.kind != TileKind::Rail || !rail_tile_is_signals(tile.m5) {
                continue;
            }
            let present = rail_signal_present_mask(tile.m3);
            let rails = tile.m5 & 0x3F;
            for bit in 0..4u8 {
                if present & (1 << bit) == 0 {
                    continue;
                }
                let sig_type = signal_track_for_bit(rails, bit)
                    .map_or(SIGTYPE_BLOCK, |track| signal_type_for_track(tile.m2, track));
                if sig_type == SIGTYPE_ENTRY {
                    continue;
                }
                exit_green.insert(
                    (c, bit),
                    signal_bit_block_green(map, vehicles, c, &tile, bit, sig_type),
                );
            }
        }
    }
    exit_green
}

fn refresh_signal_tile_states(
    map: &Map,
    vehicles: &[Vehicle],
    c: TileCoord,
    tile: crate::map::Tile,
    signal_green: &HashMap<(TileCoord, u8), bool>,
) -> Option<crate::map::Tile> {
    if tile.kind != TileKind::Rail || !rail_tile_is_signals(tile.m5) {
        return None;
    }
    let present = rail_signal_present_mask(tile.m3);
    if present == 0 {
        return None;
    }
    let rails = tile.m5 & 0x3F;
    let mut states = 0u8;
    for bit in 0..4u8 {
        if present & (1 << bit) == 0 {
            continue;
        }
        let sig_type = signal_track_for_bit(rails, bit)
            .map_or(SIGTYPE_BLOCK, |track| signal_type_for_track(tile.m2, track));
        let green = if sig_type == SIGTYPE_ENTRY {
            let exit_dir = signal_exit_dir(rails, bit);
            let own_block_ok = signal_bit_block_green(map, vehicles, c, &tile, bit, SIGTYPE_BLOCK);
            let targets = presignal_exit_targets_ahead(map, c, exit_dir);
            if targets.is_empty() {
                own_block_ok
            } else {
                // OpenTTD: entry verde solo si el bloque propio está libre Y algún exit/combo
                // aguas abajo está verde (valores ya estabilizados para combos).
                own_block_ok
                    && targets
                        .iter()
                        .any(|key| signal_green.get(key).copied().unwrap_or(false))
            }
        } else {
            signal_green
                .get(&(c, bit))
                .copied()
                .unwrap_or_else(|| signal_bit_block_green(map, vehicles, c, &tile, bit, sig_type))
        };
        if green {
            states |= 1 << bit;
        }
    }
    let mut out = tile;
    out.m3hi = (out.m3hi & 0x0F) | (states << 4);
    Some(out)
}

/// Recalcula verde/rojo en todas las teselas con señales.
///
/// Las teselas cuyo `m3hi` cambia se añaden a `dirty` (para remap visual en el cliente).
/// Si `clear_dirty` es `true`, vacía `dirty` al inicio; si no, solo añade entradas nuevas.
///
/// Orden (paridad simplificada de `UpdateSignalsOnSegment`):
/// 1. Pasada block/exit/path/combo-bloque (`compute_exit_signal_greens`).
/// 2. Estabilizar combos (`ProbeSigSeg` v0 + punto fijo entry→combo→exit).
/// 3. Escribir estados; entries leen greens estabilizados.
pub fn update_rail_signal_states(
    map: &mut Map,
    vehicles: &[Vehicle],
    dirty: &mut Vec<TileCoord>,
    clear_dirty: bool,
) {
    if clear_dirty {
        dirty.clear();
    }
    let exit_green = compute_exit_signal_greens(map, vehicles);
    let signal_green = stabilize_combo_presignal_greens(map, vehicles, &exit_green);
    let (w, h) = map.dimensions();
    for y in 0..i32::try_from(h).unwrap_or(i32::MAX) {
        for x in 0..i32::try_from(w).unwrap_or(i32::MAX) {
            let c = TileCoord::new(x, y);
            let Some(tile) = map.get(c) else {
                continue;
            };
            if let Some(out) = refresh_signal_tile_states(map, vehicles, c, tile, &signal_green)
                && out.m3hi != tile.m3hi
            {
                let _ = map.set_tile(c, out);
                dirty.push(c);
            }
        }
    }
}

#[must_use]
pub fn signal_on_track_mask(track: SignalTrack) -> u8 {
    const MASKS: [u8; 6] = [0xC, 0xC, 0xC, 0x3, 0xC, 0x3];
    MASKS[track as usize]
}

#[must_use]
pub fn signal_type_for_track(m2: u8, track: SignalTrack) -> u8 {
    let base = if matches!(track, SignalTrack::Lower | SignalTrack::Right) {
        4
    } else {
        0
    };
    (m2 >> base) & 7
}

/// Reemplaza el tipo PBS/block de una señal en `m2` (`SetSignalType` en `rail_map.h`).
#[must_use]
pub fn cycle_signal_type_m2(m2: u8, track: SignalTrack) -> u8 {
    let base = if matches!(track, SignalTrack::Lower | SignalTrack::Right) {
        4
    } else {
        0
    };
    let current = signal_type_for_track(m2, track);
    let new_type = next_placeable_signal_type(current);
    (m2 & !(7 << base)) | ((new_type & 7) << base)
}

/// Limpia tipo y variante de señal del carril en `m2` al quitar la señal.
#[must_use]
pub fn clear_signal_type_bits_m2(m2: u8, track: SignalTrack) -> u8 {
    let (base, var_bit) = if matches!(track, SignalTrack::Lower | SignalTrack::Right) {
        (4, 7)
    } else {
        (0, 3)
    };
    m2 & !(7 << base) & !(1 << var_bit)
}

/// Alterna one-way / two-way en el carril (`CycleSignalSide` en `rail_map.h`).
#[must_use]
pub fn cycle_signal_side_m3(m3: u8, track: SignalTrack, sig_type: u8) -> u8 {
    let pos = if matches!(track, SignalTrack::Lower | SignalTrack::Right) {
        4
    } else {
        6
    };
    let mut side = (m3 >> pos) & 3;
    side = side.saturating_sub(1);
    if side == 0 {
        side = if sig_type > SIGTYPE_LAST_NOPBS { 2 } else { 3 };
    }
    (m3 & !(3 << pos)) | (side << pos)
}

#[must_use]
pub(crate) fn signal_track_for_bit(rails: u8, sig_bit: u8) -> Option<SignalTrack> {
    if tracks_overlap(rails) {
        return None;
    }
    if rails == RAIL_TB_HORZ {
        return Some(if sig_bit <= 1 {
            SignalTrack::Lower
        } else {
            SignalTrack::Upper
        });
    }
    if rails == RAIL_TB_VERT {
        return Some(if sig_bit <= 1 {
            SignalTrack::Right
        } else {
            SignalTrack::Left
        });
    }
    SignalTrack::from_track_bit(rails)
}

#[must_use]
pub(crate) fn signal_exit_dir(rails: u8, sig_bit: u8) -> u8 {
    let track = signal_track_for_bit(rails, sig_bit).or({
        if rails & RAIL_TB_Y != 0 {
            Some(SignalTrack::Y)
        } else if rails & RAIL_TB_X != 0 {
            Some(SignalTrack::X)
        } else {
            None
        }
    });
    let Some(track) = track else {
        return 0;
    };
    track
        .facings()
        .iter()
        .find(|(_, bit)| *bit == sig_bit)
        .map_or(0, |(face, _)| *face)
}

/// Datos de codificación de una señal de bloque en una tesela.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SignalPlacement {
    pub sig_bit: u8,
    pub m2: u8,
    pub m3: u8,
    pub m3hi: u8,
}

/// Direcciones válidas para colocar señal en una pieza de vía (`DiagDir`).
#[must_use]
pub fn valid_signal_facings_track(track: SignalTrack) -> &'static [u8] {
    match track {
        SignalTrack::X => &[0, 2],
        SignalTrack::Y | SignalTrack::Left | SignalTrack::Right => &[3, 1],
        SignalTrack::Upper => &[3, 0],
        SignalTrack::Lower => &[2, 1],
    }
}

/// Elige la orientación de colocación más cercana a `orientation` (0..3).
#[must_use]
pub fn signal_facing_for_orientation(track: SignalTrack, orientation: u8) -> u8 {
    let facings = valid_signal_facings_track(track);
    let ori = orientation % 4;
    if let Some(f) = facings.iter().copied().find(|f| *f == ori) {
        return f;
    }
    facings.first().copied().unwrap_or(ori)
}

/// Siguiente orientación al rotar con RMB sobre vía.
#[must_use]
pub fn cycle_signal_facing(track: SignalTrack, current: u8) -> u8 {
    let facings = valid_signal_facings_track(track);
    if facings.is_empty() {
        return current % 4;
    }
    let cur = signal_facing_for_orientation(track, current);
    let idx = facings.iter().position(|&f| f == cur).unwrap_or(0);
    facings[(idx + 1) % facings.len()]
}

#[must_use]
pub fn signal_bit_for_facing(track: SignalTrack, face: u8) -> Option<u8> {
    track
        .facings()
        .iter()
        .find(|(f, _)| *f == face % 4)
        .map(|(_, bit)| *bit)
}

const OTTD_TRACK_LOWER: u8 = 3;
const OTTD_TRACK_RIGHT: u8 = 5;

fn m2_for_signal(sig_type: u8, variant: u8, track: u8) -> u8 {
    let base = if track == OTTD_TRACK_LOWER || track == OTTD_TRACK_RIGHT {
        4
    } else {
        0
    };
    let var_bit = if track == OTTD_TRACK_LOWER || track == OTTD_TRACK_RIGHT {
        7
    } else {
        3
    };
    ((sig_type & 7) << base) | ((variant & 1) << var_bit)
}

/// Codifica una señal unidireccional (`sig_type`: block, path, path oneway).
#[must_use]
pub fn signal_placement_for_track(
    track: SignalTrack,
    face: u8,
    variant: u8,
    sig_type: u8,
) -> Option<SignalPlacement> {
    let sig_bit = signal_bit_for_facing(track, face)?;
    let present = 1 << sig_bit;
    Some(SignalPlacement {
        sig_bit,
        m2: m2_for_signal(sig_type, variant, track.ottd_track()),
        m3: present << 4,
        m3hi: present << 4,
    })
}

/// Compatibilidad: resuelve la pieza con fracción centrada.
#[must_use]
pub fn signal_placement_for_facing(
    trackbits: u8,
    face: u8,
    variant: u8,
) -> Option<SignalPlacement> {
    let track = resolve_signal_track(trackbits, 128, 128)?;
    signal_placement_for_track(track, face, variant, SIGTYPE_BLOCK)
}

/// Compatibilidad con tests: señal por defecto en la primera dirección válida.
#[must_use]
pub fn encode_block_signal_on_track(trackbits: u8) -> (u8, u8, u8) {
    encode_block_signal_on_track_with_variant(trackbits, default_signal_variant(CALENDAR_BASE_YEAR))
}

#[must_use]
pub fn encode_block_signal_on_track_with_variant(trackbits: u8, variant: u8) -> (u8, u8, u8) {
    let track = resolve_signal_track(trackbits, 128, 128);
    let face = track
        .map(valid_signal_facings_track)
        .and_then(|f| f.first().copied())
        .unwrap_or(0);
    if let Some(t) = track
        && let Some(p) = signal_placement_for_track(t, face, variant, SIGTYPE_BLOCK)
    {
        (p.m2, p.m3, p.m3hi)
    } else {
        (0, 0, 0)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use crate::GameState;
    use crate::map::TileKind;

    fn write_rail(map: &mut Map, c: TileCoord, tb: u8) {
        map.set_kind(c, TileKind::Rail).expect("kind");
        let mut t = map.get(c).expect("tile");
        t.m5 = tb | (RAIL_TILE_NORMAL << 6);
        map.set_tile(c, t).expect("tile");
    }

    fn write_signal(map: &mut Map, c: TileCoord, tb: u8) {
        write_signal_facing(map, c, tb, None);
    }

    fn write_signal_facing(map: &mut Map, c: TileCoord, tb: u8, face: Option<u8>) {
        map.set_kind(c, TileKind::Rail).expect("kind");
        let track = resolve_signal_track(tb, 128, 128).expect("track");
        let face = face.unwrap_or_else(|| {
            valid_signal_facings_track(track)
                .first()
                .copied()
                .unwrap_or(0)
        });
        let placement =
            signal_placement_for_track(track, face, 1, SIGTYPE_BLOCK).expect("placement");
        let mut t = map.get(c).expect("tile");
        t.m5 = tb | (RAIL_TILE_SIGNALS << 6);
        t.m2 = placement.m2;
        t.m3 = placement.m3;
        t.m3hi = placement.m3hi;
        map.set_tile(c, t).expect("tile");
    }

    fn write_signal_on_track(map: &mut Map, c: TileCoord, tb: u8, track: SignalTrack, face: u8) {
        map.set_kind(c, TileKind::Rail).expect("kind");
        let placement =
            signal_placement_for_track(track, face, 1, SIGTYPE_BLOCK).expect("placement");
        let mut t = map.get(c).expect("tile");
        t.m5 = tb | (RAIL_TILE_SIGNALS << 6);
        t.m2 = placement.m2;
        t.m3 = placement.m3;
        t.m3hi = placement.m3hi;
        map.set_tile(c, t).expect("tile");
    }

    #[test]
    fn signal_placement_is_single_bit() {
        let p = signal_placement_for_track(SignalTrack::X, 0, 1, SIGTYPE_BLOCK).expect("NE on X");
        assert_eq!(p.m3 >> 4, 0b0100);
        let p2 = signal_placement_for_track(SignalTrack::X, 2, 1, SIGTYPE_BLOCK).expect("SW on X");
        assert_eq!(p2.m3 >> 4, 0b1000);
    }

    #[test]
    fn signal_exit_dir_horz_upper_and_lower() {
        assert_eq!(signal_exit_dir(RAIL_TB_HORZ, 2), 0);
        assert_eq!(signal_exit_dir(RAIL_TB_HORZ, 3), 3);
        assert_eq!(signal_exit_dir(RAIL_TB_HORZ, 0), 2);
        assert_eq!(signal_exit_dir(RAIL_TB_HORZ, 1), 1);
    }

    #[test]
    fn signal_bits_for_exit_horz_upper_lane() {
        let mut map = Map::new_flat(8, 8, 0);
        write_signal_facing(&mut map, TileCoord::new(1, 0), RAIL_TB_HORZ, Some(0));
        let bits = signal_bits_for_exit(&map, TileCoord::new(1, 0), TileCoord::new(2, 0));
        assert_eq!(bits, vec![2], "señal upper NE controla salida hacia NE");
    }

    #[test]
    fn train_blocked_on_horz_signal_when_block_occupied() {
        use crate::Vehicle;
        use crate::vehicle::VehicleKind;

        let mut state = GameState::new(8, 8);
        for x in 0..=3 {
            write_rail(&mut state.map, TileCoord::new(x, 0), RAIL_TB_HORZ);
        }
        write_signal_facing(&mut state.map, TileCoord::new(1, 0), RAIL_TB_HORZ, Some(0));
        let blocker = Vehicle::new(
            2,
            VehicleKind::Train,
            TileCoord::new(2, 0),
            TileCoord::new(2, 0),
        );
        let mut on_signal = Vehicle::new(
            1,
            VehicleKind::Train,
            TileCoord::new(1, 0),
            TileCoord::new(5, 0),
        );
        on_signal.running = true;
        on_signal.path = std::collections::VecDeque::from([TileCoord::new(2, 0)]);
        state.vehicles.push(on_signal);
        state.vehicles.push(blocker);
        let mut dirty = Vec::new();
        update_rail_signal_states(&mut state.map, &state.vehicles, &mut dirty, true);
        assert!(train_blocked_by_signal(
            &state.map,
            &state.vehicles,
            &state.vehicles[0]
        ));
        assert!(
            !dirty.is_empty(),
            "el estado visual de la señal debe marcarse como sucio"
        );
        let tile = state.map.get(TileCoord::new(1, 0)).expect("signal tile");
        assert_eq!(
            rail_signal_state_mask(tile.m3hi) & 0b0100,
            0,
            "señal en rojo cuando el bloque está ocupado"
        );
    }

    /// En HORZ, una señal solo en Upper no controla las salidas del carril Lower.
    #[test]
    fn horz_upper_signal_does_not_control_lower_exits() {
        let mut map = Map::new_flat(8, 4, 0);
        write_signal_on_track(
            &mut map,
            TileCoord::new(1, 1),
            RAIL_TB_HORZ,
            SignalTrack::Upper,
            0,
        );
        // Upper face 0 → bit 2, exit dir 0 (+X).
        assert_eq!(
            signal_bits_for_exit(&map, TileCoord::new(1, 1), TileCoord::new(2, 1)),
            vec![2]
        );
        // Lower exits: dir 2 (−X, bit 0) y dir 1 (+Y, bit 1) — sin señal Lower.
        assert!(
            signal_bits_for_exit(&map, TileCoord::new(1, 1), TileCoord::new(0, 1)).is_empty(),
            "Upper no controla salida Lower hacia −X"
        );
        assert!(
            signal_bits_for_exit(&map, TileCoord::new(1, 1), TileCoord::new(1, 2)).is_empty(),
            "Upper no controla salida Lower hacia +Y"
        );
        let block = rail_block_ahead(&map, TileCoord::new(1, 1), 0);
        assert!(
            block.contains(&TileCoord::new(2, 1)),
            "bloque Upper sigue el corredor HORZ hacia +X"
        );
    }

    #[test]
    fn signal_bits_for_exit_vert_left_lane() {
        let mut map = Map::new_flat(8, 8, 0);
        write_signal_on_track(
            &mut map,
            TileCoord::new(0, 1),
            RAIL_TB_VERT,
            SignalTrack::Left,
            3,
        );
        // Left facings: (3, 2) NW y (1, 3) SE — face 3 → bit 2, salida dir 3 (−Y).
        let bits = signal_bits_for_exit(&map, TileCoord::new(0, 1), TileCoord::new(0, 0));
        assert_eq!(bits, vec![2], "señal Left NW controla salida hacia NW");
    }

    #[test]
    fn train_blocked_on_vert_signal_when_block_occupied() {
        use crate::Vehicle;
        use crate::vehicle::VehicleKind;

        let mut state = GameState::new(4, 8);
        for y in 0..=3 {
            write_rail(&mut state.map, TileCoord::new(1, y), RAIL_TB_VERT);
        }
        // Left NW (bit 2): salida hacia −Y (dir 3).
        write_signal_on_track(
            &mut state.map,
            TileCoord::new(1, 2),
            RAIL_TB_VERT,
            SignalTrack::Left,
            3,
        );
        let blocker = Vehicle::new(
            2,
            VehicleKind::Train,
            TileCoord::new(1, 1),
            TileCoord::new(1, 1),
        );
        let mut on_signal = Vehicle::new(
            1,
            VehicleKind::Train,
            TileCoord::new(1, 2),
            TileCoord::new(1, 0),
        );
        on_signal.running = true;
        on_signal.path = std::collections::VecDeque::from([TileCoord::new(1, 1)]);
        state.vehicles.push(on_signal);
        state.vehicles.push(blocker);
        update_rail_signal_states(&mut state.map, &state.vehicles, &mut Vec::new(), true);
        assert!(train_blocked_by_signal(
            &state.map,
            &state.vehicles,
            &state.vehicles[0]
        ));
        let tile = state.map.get(TileCoord::new(1, 2)).expect("signal");
        assert_eq!(
            rail_signal_state_mask(tile.m3hi) & 0b0100,
            0,
            "señal Vert Left en rojo con bloque ocupado"
        );
    }

    #[test]
    fn cycle_signal_side_m3_full_cycle_on_x() {
        let mut m3 = 0x40; // one-way bit 2
        m3 = cycle_signal_side_m3(m3, SignalTrack::X, SIGTYPE_BLOCK);
        assert_eq!(m3 >> 4, 0x0C, "→ two-way");
        m3 = cycle_signal_side_m3(m3, SignalTrack::X, SIGTYPE_BLOCK);
        assert_eq!(m3 >> 4, 0x08, "→ one-way bit 3");
        m3 = cycle_signal_side_m3(m3, SignalTrack::X, SIGTYPE_BLOCK);
        assert_eq!(m3 >> 4, 0x04, "→ one-way bit 2");
    }

    #[test]
    fn cycle_signal_side_m3_on_horz_upper_and_lower() {
        let upper = cycle_signal_side_m3(0x40, SignalTrack::Upper, SIGTYPE_BLOCK);
        assert_eq!(upper >> 4, 0x0C, "Upper: bits 2+3");
        let lower = cycle_signal_side_m3(0x10, SignalTrack::Lower, SIGTYPE_BLOCK);
        assert_eq!(lower >> 4, 0x03, "Lower: bits 0+1");
    }

    #[test]
    fn two_way_terminal_allows_both_exit_dirs() {
        use crate::Vehicle;
        use crate::vehicle::VehicleKind;

        let mut map = Map::new_flat(10, 4, 0);
        for x in 0..=4 {
            write_rail(&mut map, TileCoord::new(x, 1), RAIL_TB_X);
        }
        write_signal_facing(&mut map, TileCoord::new(2, 1), RAIL_TB_X, Some(0));
        let mut tile = map.get(TileCoord::new(2, 1)).expect("sig");
        tile.m3 = cycle_signal_side_m3(tile.m3, SignalTrack::X, SIGTYPE_BLOCK);
        tile.m3hi = (tile.m3hi & 0x0F) | (rail_signal_present_mask(tile.m3) << 4);
        map.set_tile(TileCoord::new(2, 1), tile).expect("two-way");

        let present = rail_signal_present_mask(map.get(TileCoord::new(2, 1)).expect("sig").m3);
        assert_eq!(present, 0x0C, "two-way bits 2+3");

        let east = signal_bits_for_exit(&map, TileCoord::new(2, 1), TileCoord::new(3, 1));
        let west = signal_bits_for_exit(&map, TileCoord::new(2, 1), TileCoord::new(1, 1));
        assert_eq!(east, vec![2]);
        assert_eq!(west, vec![3]);

        let mut eastbound = Vehicle::new(
            1,
            VehicleKind::Train,
            TileCoord::new(2, 1),
            TileCoord::new(4, 1),
        );
        eastbound.running = true;
        eastbound.path = std::collections::VecDeque::from([TileCoord::new(3, 1)]);
        let mut westbound = Vehicle::new(
            2,
            VehicleKind::Train,
            TileCoord::new(2, 1),
            TileCoord::new(0, 1),
        );
        westbound.running = true;
        westbound.path = std::collections::VecDeque::from([TileCoord::new(1, 1)]);

        update_rail_signal_states(&mut map, &[eastbound.clone()], &mut Vec::new(), true);
        assert!(!train_blocked_by_signal(
            &map,
            &[eastbound.clone()],
            &eastbound
        ));
        update_rail_signal_states(&mut map, &[westbound.clone()], &mut Vec::new(), true);
        assert!(!train_blocked_by_signal(
            &map,
            &[westbound.clone()],
            &westbound
        ));
    }

    #[test]
    fn next_placeable_signal_type_cycles_all_six() {
        let mut t = SIGTYPE_BLOCK;
        let order = [
            SIGTYPE_ENTRY,
            SIGTYPE_EXIT,
            SIGTYPE_COMBO,
            SIGTYPE_PATH,
            SIGTYPE_PATH_ONEWAY,
            SIGTYPE_BLOCK,
        ];
        for want in order {
            t = next_placeable_signal_type(t);
            assert_eq!(t, want);
        }
    }

    #[test]
    fn default_signal_variant_before_and_after_semaphore_year() {
        assert_eq!(default_signal_variant(1949), 0);
        assert_eq!(default_signal_variant(1950), 1);
    }

    #[test]
    fn m2_variant_bit_set_for_electric_on_x() {
        let p = signal_placement_for_track(SignalTrack::X, 0, 1, SIGTYPE_BLOCK).expect("electric");
        assert_eq!(p.m2 & 0x08, 0x08);
        let s = signal_placement_for_track(SignalTrack::X, 0, 0, SIGTYPE_BLOCK).expect("semaphore");
        assert_eq!(s.m2 & 0x08, 0);
    }

    #[test]
    fn resolve_signal_track_on_upper_lane() {
        assert_eq!(
            resolve_signal_track(RAIL_TB_UPPER, 64, 64),
            Some(SignalTrack::Upper)
        );
        assert_eq!(
            resolve_signal_track(RAIL_TB_LOWER, 200, 100),
            Some(SignalTrack::Lower)
        );
        assert!(resolve_signal_track(RAIL_TB_X | RAIL_TB_Y, 128, 128).is_none());
    }

    #[test]
    fn cycle_signal_side_m3_adds_second_direction_on_x() {
        let m3 = 0x40; // solo bit 2
        let out = cycle_signal_side_m3(m3, SignalTrack::X, SIGTYPE_BLOCK);
        assert_eq!(out >> 4, 0x0C, "both bits 2 and 3");
    }

    #[test]
    fn entry_presignal_blocks_when_no_exit_is_green() {
        use crate::Vehicle;
        use crate::vehicle::VehicleKind;

        let mut map = Map::new_flat(12, 8, 0);
        for x in 0..=5 {
            write_rail(&mut map, TileCoord::new(x, 2), RAIL_TB_X);
        }
        write_signal_facing(&mut map, TileCoord::new(1, 2), RAIL_TB_X, Some(0));
        let mut entry = map.get(TileCoord::new(1, 2)).expect("entry");
        entry.m2 = (SIGTYPE_ENTRY & 7) | (1 << 3);
        map.set_tile(TileCoord::new(1, 2), entry).expect("entry");
        write_signal_facing(&mut map, TileCoord::new(4, 2), RAIL_TB_X, Some(0));
        let mut exit = map.get(TileCoord::new(4, 2)).expect("exit");
        exit.m2 = (SIGTYPE_EXIT & 7) | (1 << 3);
        map.set_tile(TileCoord::new(4, 2), exit).expect("exit");
        let blocker = Vehicle::new(
            2,
            VehicleKind::Train,
            TileCoord::new(5, 2),
            TileCoord::new(5, 2),
        );
        let mut train = Vehicle::new(
            1,
            VehicleKind::Train,
            TileCoord::new(1, 2),
            TileCoord::new(5, 2),
        );
        train.running = true;
        train.cur_speed = 0;
        train.progress = 200;
        train.path = std::collections::VecDeque::from([TileCoord::new(2, 2)]);
        let vehicles = vec![train.clone(), blocker];
        update_rail_signal_states(&mut map, &vehicles, &mut Vec::new(), true);
        let entry_tile = map.get(TileCoord::new(1, 2)).expect("entry");
        assert_eq!(
            rail_signal_state_mask(entry_tile.m3hi) & 0b0100,
            0,
            "entry roja si ninguna exit verde"
        );
        assert!(
            train_blocked_by_signal(&map, &vehicles, &train),
            "entry roja debe detener el tren"
        );
    }

    #[test]
    fn entry_presignal_red_when_own_block_occupied_even_if_exit_green() {
        use crate::Vehicle;
        use crate::vehicle::VehicleKind;

        let mut map = Map::new_flat(12, 8, 0);
        for x in 0..=6 {
            write_rail(&mut map, TileCoord::new(x, 2), RAIL_TB_X);
        }
        write_signal_facing(&mut map, TileCoord::new(1, 2), RAIL_TB_X, Some(0));
        let mut entry = map.get(TileCoord::new(1, 2)).expect("entry");
        entry.m2 = (SIGTYPE_ENTRY & 7) | (1 << 3);
        map.set_tile(TileCoord::new(1, 2), entry).expect("entry");
        write_signal_facing(&mut map, TileCoord::new(4, 2), RAIL_TB_X, Some(0));
        let mut exit = map.get(TileCoord::new(4, 2)).expect("exit");
        exit.m2 = (SIGTYPE_EXIT & 7) | (1 << 3);
        map.set_tile(TileCoord::new(4, 2), exit).expect("exit");
        // Ocupa el bloque entre entry y exit; el bloque tras la exit queda libre.
        let mid_blocker = Vehicle::new(
            2,
            VehicleKind::Train,
            TileCoord::new(3, 2),
            TileCoord::new(3, 2),
        );
        let mut train = Vehicle::new(
            1,
            VehicleKind::Train,
            TileCoord::new(1, 2),
            TileCoord::new(6, 2),
        );
        train.running = true;
        train.cur_speed = 0;
        train.progress = 200;
        train.path = std::collections::VecDeque::from([TileCoord::new(2, 2)]);
        let vehicles = vec![train.clone(), mid_blocker];
        update_rail_signal_states(&mut map, &vehicles, &mut Vec::new(), true);
        let entry_tile = map.get(TileCoord::new(1, 2)).expect("entry");
        assert_eq!(
            rail_signal_state_mask(entry_tile.m3hi) & 0b0100,
            0,
            "entry roja si el bloque propio está ocupado"
        );
        assert!(train_blocked_by_signal(&map, &vehicles, &train));
    }

    /// Entry → Combo → Exit: si el bloque tras la exit está ocupado, combo y entry rojas.
    #[test]
    fn entry_stays_red_when_combo_downstream_exit_occupied() {
        use crate::Vehicle;
        use crate::vehicle::VehicleKind;

        let mut map = Map::new_flat(14, 6, 0);
        for x in 0..=9 {
            write_rail(&mut map, TileCoord::new(x, 2), RAIL_TB_X);
        }
        // Entry @1, Combo @4, Exit @7; blocker tras exit @8.
        write_signal_facing(&mut map, TileCoord::new(1, 2), RAIL_TB_X, Some(0));
        let mut entry = map.get(TileCoord::new(1, 2)).expect("entry");
        entry.m2 = (SIGTYPE_ENTRY & 7) | (1 << 3);
        map.set_tile(TileCoord::new(1, 2), entry).expect("entry");

        write_signal_facing(&mut map, TileCoord::new(4, 2), RAIL_TB_X, Some(0));
        let mut combo = map.get(TileCoord::new(4, 2)).expect("combo");
        combo.m2 = (SIGTYPE_COMBO & 7) | (1 << 3);
        map.set_tile(TileCoord::new(4, 2), combo).expect("combo");

        write_signal_facing(&mut map, TileCoord::new(7, 2), RAIL_TB_X, Some(0));
        let mut exit = map.get(TileCoord::new(7, 2)).expect("exit");
        exit.m2 = (SIGTYPE_EXIT & 7) | (1 << 3);
        map.set_tile(TileCoord::new(7, 2), exit).expect("exit");

        let blocker = Vehicle::new(
            2,
            VehicleKind::Train,
            TileCoord::new(8, 2),
            TileCoord::new(8, 2),
        );
        update_rail_signal_states(&mut map, &[blocker], &mut Vec::new(), true);

        let combo_tile = map.get(TileCoord::new(4, 2)).expect("combo");
        assert_eq!(
            rail_signal_state_mask(combo_tile.m3hi) & 0b0100,
            0,
            "combo roja: exit aguas abajo ocupada"
        );
        let entry_tile = map.get(TileCoord::new(1, 2)).expect("entry");
        assert_eq!(
            rail_signal_state_mask(entry_tile.m3hi) & 0b0100,
            0,
            "entry debe leer combo estabilizada (no pasada 1)"
        );
    }

    #[test]
    fn combo_green_only_when_own_block_and_downstream_exit_green() {
        use crate::Vehicle;
        use crate::vehicle::VehicleKind;

        let mut map = Map::new_flat(14, 6, 0);
        for x in 0..=9 {
            write_rail(&mut map, TileCoord::new(x, 2), RAIL_TB_X);
        }
        write_signal_facing(&mut map, TileCoord::new(4, 2), RAIL_TB_X, Some(0));
        let mut combo = map.get(TileCoord::new(4, 2)).expect("combo");
        combo.m2 = (SIGTYPE_COMBO & 7) | (1 << 3);
        map.set_tile(TileCoord::new(4, 2), combo).expect("combo");
        write_signal_facing(&mut map, TileCoord::new(7, 2), RAIL_TB_X, Some(0));
        let mut exit = map.get(TileCoord::new(7, 2)).expect("exit");
        exit.m2 = (SIGTYPE_EXIT & 7) | (1 << 3);
        map.set_tile(TileCoord::new(7, 2), exit).expect("exit");

        // Sin ocupación: combo verde.
        update_rail_signal_states(&mut map, &[], &mut Vec::new(), true);
        let combo_tile = map.get(TileCoord::new(4, 2)).expect("combo");
        assert_ne!(
            rail_signal_state_mask(combo_tile.m3hi) & 0b0100,
            0,
            "combo verde con exit libre"
        );

        // Bloque propio de combo ocupado → roja aunque exit libre.
        let mid = Vehicle::new(
            1,
            VehicleKind::Train,
            TileCoord::new(5, 2),
            TileCoord::new(5, 2),
        );
        update_rail_signal_states(&mut map, &[mid], &mut Vec::new(), true);
        let combo_tile = map.get(TileCoord::new(4, 2)).expect("combo");
        assert_eq!(
            rail_signal_state_mask(combo_tile.m3hi) & 0b0100,
            0,
            "combo roja con bloque propio ocupado"
        );
    }

    #[test]
    fn explore_sig_segment_stops_at_block_signal() {
        let mut map = Map::new_flat(14, 6, 0);
        for x in 0..=9 {
            write_rail(&mut map, TileCoord::new(x, 2), RAIL_TB_X);
        }
        // Entry @1 → block @4 → exit @7 (exit no debe contar para la entry).
        write_signal_facing(&mut map, TileCoord::new(1, 2), RAIL_TB_X, Some(0));
        let mut entry = map.get(TileCoord::new(1, 2)).expect("entry");
        entry.m2 = (SIGTYPE_ENTRY & 7) | (1 << 3);
        map.set_tile(TileCoord::new(1, 2), entry).expect("entry");
        write_signal_facing(&mut map, TileCoord::new(4, 2), RAIL_TB_X, Some(0)); // block
        write_signal_facing(&mut map, TileCoord::new(7, 2), RAIL_TB_X, Some(0));
        let mut exit = map.get(TileCoord::new(7, 2)).expect("exit");
        exit.m2 = (SIGTYPE_EXIT & 7) | (1 << 3);
        map.set_tile(TileCoord::new(7, 2), exit).expect("exit");

        let targets = presignal_exit_targets_ahead(&map, TileCoord::new(1, 2), 0);
        assert!(
            targets.is_empty(),
            "block intermedio cierra el segmento: {targets:?}"
        );
    }

    #[test]
    fn block_ahead_stops_at_next_signal() {
        let mut map = Map::new_flat(8, 8, 0);
        write_rail(&mut map, TileCoord::new(0, 0), RAIL_TB_X);
        write_signal(&mut map, TileCoord::new(1, 0), RAIL_TB_X);
        write_rail(&mut map, TileCoord::new(2, 0), RAIL_TB_X);
        write_rail(&mut map, TileCoord::new(3, 0), RAIL_TB_X);
        let block = rail_block_ahead(&map, TileCoord::new(1, 0), 0);
        assert_eq!(
            block,
            vec![TileCoord::new(2, 0), TileCoord::new(3, 0)],
            "bloque hasta la siguiente señal o fin de vía"
        );
    }

    #[test]
    fn train_blocked_when_block_occupied() {
        use crate::Vehicle;
        use crate::vehicle::VehicleKind;

        let mut state = GameState::new(8, 8);
        write_rail(&mut state.map, TileCoord::new(0, 0), RAIL_TB_X);
        write_signal(&mut state.map, TileCoord::new(1, 0), RAIL_TB_X);
        write_rail(&mut state.map, TileCoord::new(2, 0), RAIL_TB_X);
        let blocker = Vehicle::new(
            2,
            VehicleKind::Train,
            TileCoord::new(2, 0),
            TileCoord::new(2, 0),
        );
        let mut on_signal = Vehicle::new(
            1,
            VehicleKind::Train,
            TileCoord::new(1, 0),
            TileCoord::new(5, 0),
        );
        on_signal.running = true;
        on_signal.path = std::collections::VecDeque::from([TileCoord::new(2, 0)]);
        state.vehicles.push(on_signal);
        state.vehicles.push(blocker);
        let mut dirty = Vec::new();
        update_rail_signal_states(&mut state.map, &state.vehicles, &mut dirty, true);
        assert!(train_blocked_by_signal(
            &state.map,
            &state.vehicles,
            &state.vehicles[0]
        ));
        assert!(!dirty.is_empty());
    }

    #[test]
    fn dual_scenario_signal_9_controls_eastbound_exit() {
        use crate::parity::{
            TRAIN_DUAL_TRACK_OUT_Y, TRAIN_DUAL_VEHICLE_2_ID, build_train_supply_dual,
        };
        use std::collections::VecDeque;

        let mut state = build_train_supply_dual();
        let sig = TileCoord::new(9, TRAIN_DUAL_TRACK_OUT_Y);
        let tile = state.map.get(sig).expect("señal 9");
        assert!(rail_tile_is_signals(tile.m5), "m5={:#x}", tile.m5);
        let bits =
            signal_bits_for_exit(&state.map, sig, TileCoord::new(10, TRAIN_DUAL_TRACK_OUT_Y));
        assert_eq!(
            bits,
            vec![2],
            "m5={:#x} m3={:#x} m2={:#x}",
            tile.m5,
            tile.m3,
            tile.m2
        );

        let leader_pos = TileCoord::new(11, TRAIN_DUAL_TRACK_OUT_Y);
        let follower_pos = TileCoord::new(8, TRAIN_DUAL_TRACK_OUT_Y);
        {
            let leader = state
                .vehicles
                .iter_mut()
                .find(|v| v.id == 1)
                .expect("tren 1");
            leader.pos = leader_pos;
            leader.running = true;
        }
        {
            let follower = state
                .vehicles
                .iter_mut()
                .find(|v| v.id == TRAIN_DUAL_VEHICLE_2_ID)
                .expect("tren 2");
            follower.pos = follower_pos;
            follower.path = VecDeque::from([
                TileCoord::new(9, TRAIN_DUAL_TRACK_OUT_Y),
                TileCoord::new(10, TRAIN_DUAL_TRACK_OUT_Y),
                leader_pos,
            ]);
            follower.running = true;
            follower.set_cruise_speed();
            follower.progress = 200;
        }
        let mut dirty = Vec::new();
        update_rail_signal_states(&mut state.map, &state.vehicles, &mut dirty, true);
        let follower = state.vehicles.iter().find(|v| v.id == 2).expect("tren 2");
        assert_eq!(follower.movement_target(), Some(sig));
        let sig_tile = state.map.get(sig).expect("señal 9");
        assert!(
            train_blocked_by_signal(&state.map, &state.vehicles, follower),
            "m3hi={:#x} block={:?}",
            sig_tile.m3hi,
            rail_block_ahead(&state.map, sig, 0)
        );
        let block = rail_block_ahead(&state.map, sig, 0);
        assert!(
            block.contains(&leader_pos),
            "el bloque tras la señal 9 debe incluir al líder: {block:?}"
        );
    }

    #[test]
    fn dual_scenario_signal_stays_red_when_leader_on_perpendicular_connector() {
        use crate::parity::{
            TRAIN_DUAL_TRACK_OUT_Y, TRAIN_DUAL_VEHICLE_2_ID, build_train_supply_dual,
        };
        use std::collections::VecDeque;

        let mut state = build_train_supply_dual();
        let sig = TileCoord::new(9, TRAIN_DUAL_TRACK_OUT_Y);
        let connector = TileCoord::new(10, 5);
        let follower_pos = TileCoord::new(8, TRAIN_DUAL_TRACK_OUT_Y);

        {
            let leader = state
                .vehicles
                .iter_mut()
                .find(|v| v.id == 1)
                .expect("tren 1");
            leader.pos = connector;
            leader.path = VecDeque::from([TileCoord::new(10, TRAIN_DUAL_TRACK_OUT_Y - 2)]);
            leader.running = true;
        }
        {
            let follower = state
                .vehicles
                .iter_mut()
                .find(|v| v.id == TRAIN_DUAL_VEHICLE_2_ID)
                .expect("tren 2");
            follower.pos = follower_pos;
            follower.path = VecDeque::from([
                TileCoord::new(9, TRAIN_DUAL_TRACK_OUT_Y),
                TileCoord::new(10, TRAIN_DUAL_TRACK_OUT_Y),
            ]);
            follower.running = true;
            follower.set_cruise_speed();
            follower.progress = 200;
        }

        let block = rail_block_ahead(&state.map, sig, 0);
        assert!(
            block.contains(&connector),
            "el conector perpendicular debe formar parte del bloque: {block:?}"
        );

        let mut dirty = Vec::new();
        update_rail_signal_states(&mut state.map, &state.vehicles, &mut dirty, true);
        let sig_tile = state.map.get(sig).expect("señal 9");
        assert_eq!(
            rail_signal_state_mask(sig_tile.m3hi) & 0b0100,
            0,
            "señal debe seguir en rojo con tren en conector perpendicular"
        );
        assert!(
            train_blocked_by_signal(
                &state.map,
                &state.vehicles,
                state.vehicles.iter().find(|v| v.id == 2).expect("tren 2")
            ),
            "seguidor no debe avanzar"
        );
    }

    #[test]
    fn train_blocked_before_entering_signal_tile() {
        use crate::Vehicle;
        use crate::vehicle::VehicleKind;

        let mut state = GameState::new(8, 8);
        for x in 0..=4 {
            write_rail(&mut state.map, TileCoord::new(x, 0), RAIL_TB_X);
        }
        write_signal(&mut state.map, TileCoord::new(2, 0), RAIL_TB_X);
        let blocker = Vehicle::new(
            2,
            VehicleKind::Train,
            TileCoord::new(3, 0),
            TileCoord::new(3, 0),
        );
        let mut approaching = Vehicle::new(
            1,
            VehicleKind::Train,
            TileCoord::new(1, 0),
            TileCoord::new(4, 0),
        );
        approaching.running = true;
        approaching.path = (2..=4)
            .map(|x| TileCoord::new(x, 0))
            .collect::<std::collections::VecDeque<_>>();
        state.vehicles.push(approaching);
        state.vehicles.push(blocker);
        let mut dirty = Vec::new();
        update_rail_signal_states(&mut state.map, &state.vehicles, &mut dirty, true);
        assert!(
            !train_blocked_by_signal(&state.map, &state.vehicles, &state.vehicles[0]),
            "puede avanzar sub-tesela dentro de la tesela de aproximación"
        );
        state.vehicles[0].progress = 200;
        state.vehicles[0].set_cruise_speed();
        assert!(
            train_blocked_by_signal(&state.map, &state.vehicles, &state.vehicles[0]),
            "debe frenar al completar la tesela previa a la señal"
        );
    }

    #[test]
    fn sim_train_waits_until_block_ahead_clears() {
        use crate::Vehicle;
        use crate::VehicleKind;

        let mut state = GameState::new(12, 4);
        for x in 0..=6 {
            write_rail(&mut state.map, TileCoord::new(x, 0), RAIL_TB_X);
        }
        write_signal(&mut state.map, TileCoord::new(2, 0), RAIL_TB_X);

        let mut lead = Vehicle::new(
            1,
            VehicleKind::Train,
            TileCoord::new(2, 0),
            TileCoord::new(5, 0),
        );
        lead.running = true;
        lead.path = (3..=5)
            .map(|x| TileCoord::new(x, 0))
            .collect::<std::collections::VecDeque<_>>();
        lead.set_cruise_speed();

        let blocker = Vehicle::new(
            2,
            VehicleKind::Train,
            TileCoord::new(3, 0),
            TileCoord::new(3, 0),
        );
        state.vehicles.push(lead);
        state.vehicles.push(blocker);

        let start = state.vehicles[0].pos;
        for _ in 0..300 {
            state.step();
        }
        assert_eq!(
            state.vehicles[0].pos, start,
            "el tren debe esperar en la señal con el bloque ocupado"
        );

        state.vehicles.pop();
        for _ in 0..800 {
            state.step();
        }
        assert_ne!(
            state.vehicles[0].pos, start,
            "al liberarse el bloque el tren debe avanzar"
        );
    }

    #[test]
    fn multiple_trains_in_rail_depot_do_not_block_each_other() {
        use std::collections::VecDeque;

        use crate::vehicle::{Vehicle, VehicleKind};

        let mut map = Map::new_flat(6, 6, 0);
        let depot = TileCoord::new(2, 2);
        map.set_kind(depot, crate::map::TileKind::RailDepot)
            .expect("depot tile");
        write_rail(&mut map, TileCoord::new(2, 1), RAIL_TB_Y);

        let mut lead = Vehicle::new(1, VehicleKind::Train, depot, TileCoord::new(2, 1));
        lead.path = VecDeque::from([TileCoord::new(2, 1)]);
        lead.running = true;
        let follower = Vehicle::new(2, VehicleKind::Train, depot, TileCoord::new(2, 1));
        let vehicles = vec![lead.clone(), follower];
        assert!(
            !train_blocked_by_traffic(&map, &vehicles, &lead),
            "varios trenes en el mismo depósito no deben bloquearse entre sí"
        );
    }

    #[test]
    fn trains_block_head_on_without_signal() {
        use std::collections::VecDeque;

        use crate::vehicle::{Vehicle, VehicleKind};

        let mut map = Map::new_flat(10, 10, 0);
        for x in 0..5 {
            write_rail(&mut map, TileCoord::new(x, 0), RAIL_TB_X);
        }
        let mut east = Vehicle::new(
            1,
            VehicleKind::Train,
            TileCoord::new(0, 0),
            TileCoord::new(4, 0),
        );
        east.path = VecDeque::from([
            TileCoord::new(1, 0),
            TileCoord::new(2, 0),
            TileCoord::new(3, 0),
            TileCoord::new(4, 0),
        ]);
        east.running = true;
        let mut west = Vehicle::new(
            2,
            VehicleKind::Train,
            TileCoord::new(4, 0),
            TileCoord::new(0, 0),
        );
        west.path = VecDeque::from([
            TileCoord::new(3, 0),
            TileCoord::new(2, 0),
            TileCoord::new(1, 0),
            TileCoord::new(0, 0),
        ]);
        west.running = true;
        let vehicles = vec![east.clone(), west];
        assert!(
            train_blocked_by_traffic(&map, &vehicles, &east),
            "trenes frente a frente deben detenerse sin señales"
        );
    }

    #[test]
    fn path_oneway_blocks_reverse_through_signal_tile() {
        use crate::Vehicle;
        use crate::command::{Command, apply_command};
        use crate::vehicle::VehicleKind;

        let mut state = GameState::new(8, 4);
        for x in 0..=4 {
            write_rail(&mut state.map, TileCoord::new(x, 0), RAIL_TB_X);
        }
        apply_command(
            &mut state,
            &Command::PlaceRailSignal(TileCoord::new(2, 0), 0, 128, 128, SIGTYPE_PATH_ONEWAY),
        )
        .expect("path oneway");
        let mut train = Vehicle::new(
            1,
            VehicleKind::Train,
            TileCoord::new(2, 0),
            TileCoord::new(0, 0),
        );
        train.running = true;
        train.path = std::collections::VecDeque::from([TileCoord::new(1, 0)]);
        let mut dirty = Vec::new();
        update_rail_signal_states(&mut state.map, &[train.clone()], &mut dirty, true);
        assert!(
            train_blocked_by_signal(&state.map, &[train.clone()], &train),
            "PathOneWay debe bloquear el sentido contrario a la señal"
        );
    }
}
