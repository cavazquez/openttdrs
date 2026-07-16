//! Exploración de segmentos y estabilización de verdes (presignals), sin mutar el mapa.

use std::collections::{HashMap, HashSet};

use crate::map::{Map, Tile, TileCoord, TileKind, rail_signal_diag_dir_offset as diag_dir_offset};
use crate::vehicle::Vehicle;

use super::encoding::{
    SIGTYPE_BLOCK, SIGTYPE_COMBO, SIGTYPE_ENTRY, SIGTYPE_EXIT, is_pbs_signal_type,
    rail_signal_present_mask, signal_on_track_mask, signal_type_for_track,
};
use super::encoding::{signal_exit_dir, signal_track_for_bit};
use super::rail_tile_is_signals;
use super::rail_traversal_bits;
use super::topology::{
    block_is_occupied_by_trains, rail_block_ahead_with_wormholes, rail_neighbors,
};

/// Resultado simplificado de `ProbeSigSeg` / `ExploreSegment` (`signal.cpp`).
///
/// Flags `Exit` / `MultiExit` / `Green` / `MultiGreen`. Ocupación (`Train`) sigue
/// vía [`rail_block_ahead`] + [`block_is_occupied_by_trains`].
#[derive(Debug, Clone, Default)]
#[allow(clippy::struct_excessive_bools)] // espejo de `SigFlag` upstream
pub(super) struct SigSegmentProbe {
    /// `(tile, signal_bit)` de exit/combo al final del segmento.
    pub(super) exits: Vec<(TileCoord, u8)>,
    pub(super) has_exit: bool,
    pub(super) multi_exit: bool,
    pub(super) has_green: bool,
    pub(super) multi_green: bool,
}

impl SigSegmentProbe {
    /// Rellena flags Green a partir del mapa de verdes estabilizado / pasada 1.
    pub(super) fn with_green_flags(mut self, greens: &HashMap<(TileCoord, u8), bool>) -> Self {
        self.has_exit = !self.exits.is_empty();
        self.multi_exit = self.exits.len() >= 2;
        let green_n = self
            .exits
            .iter()
            .filter(|k| greens.get(k).copied().unwrap_or(false))
            .count();
        self.has_green = green_n >= 1;
        self.multi_green = green_n >= 2;
        self
    }
}

/// `true` si la tesela puede formar parte del segmento `ProbeSigSeg` (vía, estación, túnel, puente).
#[must_use]
pub(super) fn is_sig_segment_traversable(map: &Map, c: TileCoord) -> bool {
    rail_traversal_bits(map, c) != 0
}

/// Explora el segmento PBS/presignal desde `signal_tile` hacia `exit_dir`.
///
/// Paridad v0 de `ProbeSigSeg`: atraviesa estación/túnel/puente y wormholes JGR;
/// no atraviesa señales block/path/entry; al hallar exit/combo registra y corta esa rama.
pub(super) fn explore_sig_segment(
    map: &Map,
    signal_tile: TileCoord,
    exit_dir: u8,
    wormholes: Option<&crate::pathfinder::TunnelWormholes>,
) -> SigSegmentProbe {
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
        if !is_sig_segment_traversable(map, cur) {
            continue;
        }
        // Señales solo en `TileKind::Rail` (no sobre estación/túnel/puente).
        if tile.kind == TileKind::Rail && rail_tile_is_signals(tile.m5) {
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
        // Wormhole JGR: salto al otro extremo del túnel (desconectado en el mapa).
        if let Some(wh) = wormholes
            && let Some(other) = wh.other_end(cur)
            && is_sig_segment_traversable(map, other)
            && !visited.contains(&other)
        {
            queue.push(other);
        }
    }
    probe.has_exit = !probe.exits.is_empty();
    probe.multi_exit = probe.exits.len() >= 2;
    probe
}

/// Bit de señal en el sentido contrario del mismo carril (`ReverseTrackdir`).
fn reverse_signal_bit(rails: u8, bit: u8) -> Option<u8> {
    let track = signal_track_for_bit(rails, bit)?;
    let mask = signal_on_track_mask(track);
    (0..4u8).find(|&b| b != bit && mask & (1 << b) != 0)
}

/// `true` si hay señal en ambos sentidos del mismo carril (two-way).
fn is_bidir_signal_on_bit(tile: &Tile, bit: u8) -> bool {
    let present = rail_signal_present_mask(tile.m3);
    reverse_signal_bit(tile.m5 & 0x3F, bit).is_some_and(|rev| present & (1 << rev) != 0)
}

#[cfg(test)]
pub(super) fn presignal_exit_targets_ahead(
    map: &Map,
    signal_tile: TileCoord,
    exit_dir: u8,
    wormholes: Option<&crate::pathfinder::TunnelWormholes>,
) -> Vec<(TileCoord, u8)> {
    explore_sig_segment(map, signal_tile, exit_dir, wormholes).exits
}

/// Propaga verde de combos hasta punto fijo (entry → combo → … → exit).
///
/// Pasada 1 deja combos como “solo bloque propio”; aquí aplican la regla entry
/// leyendo exits/combos aguas abajo ya estabilizados (arregla cadenas combo).
/// Combo bidireccional: regla `MultiExit`/`MultiGreen` de `signal.cpp` (~441–448).
pub(super) fn stabilize_combo_presignal_greens(
    map: &Map,
    vehicles: &[Vehicle],
    exit_green: &HashMap<(TileCoord, u8), bool>,
    wormholes: Option<&crate::pathfinder::TunnelWormholes>,
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
                signal_bit_block_green(map, vehicles, c, &tile, bit, SIGTYPE_COMBO, wormholes)
            });
            let probe = explore_sig_segment(map, c, exit_dir, wormholes).with_green_flags(&greens);
            let entry_ok = if probe.has_exit {
                probe.has_green
            } else {
                true
            };
            let mut new_green = exit_ok && entry_ok;
            // Combo two-way: MultiExit + (ningún verde, o un solo verde con cara reversa verde).
            if new_green && is_bidir_signal_on_bit(&tile, bit) && probe.multi_exit {
                let rev_green = reverse_signal_bit(rails, bit)
                    .and_then(|rev| greens.get(&(c, rev)).copied())
                    .unwrap_or(false);
                if !probe.has_green || (!probe.multi_green && rev_green) {
                    new_green = false;
                }
            }
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
    tile: &Tile,
    bit: u8,
    sig_type: u8,
    wormholes: Option<&crate::pathfinder::TunnelWormholes>,
) -> bool {
    let exit_dir = signal_exit_dir(tile.m5 & 0x3F, bit);
    let block = rail_block_ahead_with_wormholes(map, c, exit_dir, wormholes);
    if is_pbs_signal_type(sig_type) {
        // Path verde solo con reserva válida hasta posición segura (TryReservePath OK).
        crate::rail_pbs::pbs_exit_has_complete_reservation(map, vehicles, c, exit_dir, &block)
    } else {
        !block_is_occupied_by_trains(vehicles, c, &block)
    }
}

pub(super) fn compute_exit_signal_greens(
    map: &Map,
    vehicles: &[Vehicle],
    wormholes: Option<&crate::pathfinder::TunnelWormholes>,
) -> HashMap<(TileCoord, u8), bool> {
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
                    signal_bit_block_green(map, vehicles, c, &tile, bit, sig_type, wormholes),
                );
            }
        }
    }
    exit_green
}

pub(super) fn refresh_signal_tile_states(
    map: &Map,
    vehicles: &[Vehicle],
    c: TileCoord,
    tile: crate::map::Tile,
    signal_green: &HashMap<(TileCoord, u8), bool>,
    wormholes: Option<&crate::pathfinder::TunnelWormholes>,
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
            let own_block_ok =
                signal_bit_block_green(map, vehicles, c, &tile, bit, SIGTYPE_BLOCK, wormholes);
            let probe =
                explore_sig_segment(map, c, exit_dir, wormholes).with_green_flags(signal_green);
            if probe.has_exit {
                // OpenTTD: entry verde solo si bloque propio libre Y Exit+Green en el segmento.
                own_block_ok && probe.has_green
            } else {
                own_block_ok
            }
        } else {
            signal_green.get(&(c, bit)).copied().unwrap_or_else(|| {
                signal_bit_block_green(map, vehicles, c, &tile, bit, sig_type, wormholes)
            })
        };
        if green {
            states |= 1 << bit;
        }
    }
    let mut out = tile;
    out.m3hi = (out.m3hi & 0x0F) | (states << 4);
    Some(out)
}
