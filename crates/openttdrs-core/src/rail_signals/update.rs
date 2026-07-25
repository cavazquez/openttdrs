//! Invalidación globset y escritura de estados verde/rojo.

use std::collections::{BTreeSet, HashSet};

use crate::map::{Map, TileCoord, TileKind};
use crate::vehicle::{Vehicle, VehicleKind};

use super::encoding::signal_exit_dir;
use super::encoding::signal_track_for_bit;
use super::encoding::{
    SIGTYPE_BLOCK, SIGTYPE_COMBO, SIGTYPE_ENTRY, rail_signal_present_mask, signal_type_for_track,
};
use super::presignal::{
    compute_exit_signal_greens_scoped, explore_sig_segment, refresh_signal_tile_states,
    stabilize_combo_presignal_greens_scoped,
};
use super::rail_tile_is_signals;
use super::topology::rail_block_ahead_with_wormholes;

/// Umbral `OpenTTD`: forzar drenado al llegar a 64 entradas ([`SIG_GLOB_UPDATE`]).
pub const SIG_GLOB_UPDATE: usize = 64;

/// Entrada de `_globset`: tesela + dirección de entrada (`DiagDirection`).
///
/// `enter_dir == u8::MAX` significa «cualquier dirección» (invalidación amplia).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SignalGlobEntry {
    pub tile: TileCoord,
    pub enter_dir: u8,
}

impl SignalGlobEntry {
    pub const ANY_DIR: u8 = u8::MAX;

    #[must_use]
    pub const fn new(tile: TileCoord, enter_dir: u8) -> Self {
        Self { tile, enter_dir }
    }

    #[must_use]
    pub const fn any_dir(tile: TileCoord) -> Self {
        Self {
            tile,
            enter_dir: Self::ANY_DIR,
        }
    }
}

/// Cola de invalidación de señales (`_globset` de `signal.cpp`).
pub type SignalGlobSet = HashSet<SignalGlobEntry>;

/// Índice efímero y ordenado de teselas con señales.
///
/// La primera consulta recorre el mapa; los drenados siguientes sincronizan solo
/// las teselas presentes en `_globset`, incluidas altas y bajas por comandos.
#[derive(Debug, Clone, Default)]
pub struct SignalSpatialIndex {
    tiles: BTreeSet<TileCoord>,
    initialized: bool,
    full_map_scans: u64,
}

impl SignalSpatialIndex {
    /// Cantidad de teselas de señal actualmente indexadas.
    #[must_use]
    pub fn len(&self) -> usize {
        self.tiles.len()
    }

    /// `true` si todavía no hay señales indexadas.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.tiles.is_empty()
    }

    /// Barridos completos usados para inicializar/reconstruir el índice.
    #[must_use]
    pub const fn full_map_scans(&self) -> u64 {
        self.full_map_scans
    }

    fn ensure_initialized(&mut self, map: &Map) {
        if self.initialized {
            return;
        }
        self.tiles.clear();
        let (w, h) = map.dimensions();
        for y in 0..i32::try_from(h).unwrap_or(i32::MAX) {
            for x in 0..i32::try_from(w).unwrap_or(i32::MAX) {
                let c = TileCoord::new(x, y);
                if map.get(c).is_some_and(is_signal_tile) {
                    self.tiles.insert(c);
                }
            }
        }
        self.initialized = true;
        self.full_map_scans = self.full_map_scans.saturating_add(1);
    }

    fn sync_glob_tiles(&mut self, map: &Map, seeds: &SignalGlobSet) {
        self.ensure_initialized(map);
        for entry in seeds {
            if map.get(entry.tile).is_some_and(is_signal_tile) {
                self.tiles.insert(entry.tile);
            } else {
                self.tiles.remove(&entry.tile);
            }
        }
    }
}

fn is_signal_tile(tile: crate::map::Tile) -> bool {
    tile.kind == TileKind::Rail && rail_tile_is_signals(tile.m5)
}

/// `true` si el globset alcanzó el umbral de flush (64).
#[must_use]
pub fn signal_globset_needs_flush(set: &SignalGlobSet) -> bool {
    set.len() >= SIG_GLOB_UPDATE
}

/// Encola una tesela ferroviaria para refresco local de señales (cualquier dir).
pub fn enqueue_signal_glob(set: &mut SignalGlobSet, tile: TileCoord) {
    set.insert(SignalGlobEntry::any_dir(tile));
}

/// Encola `(tile, DiagDirection)` como en `AddSideToSignalBuffer`.
pub fn enqueue_signal_glob_side(set: &mut SignalGlobSet, tile: TileCoord, enter_dir: u8) {
    set.insert(SignalGlobEntry::new(tile, enter_dir));
}

/// Encola posiciones (y `movement_target`) de trenes para invalidar bloques ocupados.
pub fn enqueue_trains_for_signal_update(set: &mut SignalGlobSet, vehicles: &[Vehicle]) {
    for v in vehicles {
        if v.kind != VehicleKind::Train {
            continue;
        }
        enqueue_signal_glob(set, v.pos);
        if let Some(next) = v.movement_target() {
            enqueue_signal_glob(set, next);
        }
    }
}

/// Encola teselas de reserva PBS (path signals dependen de `reserved_steps`).
pub fn enqueue_pbs_reservations_for_signal_update(set: &mut SignalGlobSet, vehicles: &[Vehicle]) {
    for v in vehicles {
        if v.kind != VehicleKind::Train {
            continue;
        }
        for step in &v.reserved_steps {
            enqueue_signal_glob(set, step.tile);
        }
    }
}

/// Señales cuyo bloque contiene alguna tesela de `seeds`, más entries/combos aguas arriba.
#[must_use]
pub fn collect_signals_affected_by_tiles(map: &Map, seeds: &SignalGlobSet) -> HashSet<TileCoord> {
    collect_signals_affected_by_tiles_with_wormholes(map, seeds, None)
}

/// Como [`collect_signals_affected_by_tiles`], con wormholes JGR.
#[must_use]
pub fn collect_signals_affected_by_tiles_with_wormholes(
    map: &Map,
    seeds: &SignalGlobSet,
    wormholes: Option<&crate::pathfinder::TunnelWormholes>,
) -> HashSet<TileCoord> {
    if seeds.is_empty() {
        return HashSet::new();
    }
    let (w, h) = map.dimensions();
    let mut signal_tiles = BTreeSet::new();
    for y in 0..i32::try_from(h).unwrap_or(i32::MAX) {
        for x in 0..i32::try_from(w).unwrap_or(i32::MAX) {
            let c = TileCoord::new(x, y);
            if map.get(c).is_some_and(is_signal_tile) {
                signal_tiles.insert(c);
            }
        }
    }
    collect_signals_affected_from_candidates(map, seeds, &signal_tiles, wormholes)
}

/// Consulta incremental usando un índice persistente de señales.
#[must_use]
pub fn collect_signals_affected_by_tiles_indexed(
    map: &Map,
    seeds: &SignalGlobSet,
    index: &mut SignalSpatialIndex,
    wormholes: Option<&crate::pathfinder::TunnelWormholes>,
) -> HashSet<TileCoord> {
    if seeds.is_empty() {
        return HashSet::new();
    }
    index.sync_glob_tiles(map, seeds);
    collect_signals_affected_from_candidates(map, seeds, &index.tiles, wormholes)
}

fn collect_signals_affected_from_candidates(
    map: &Map,
    seeds: &SignalGlobSet,
    signal_tiles: &BTreeSet<TileCoord>,
    wormholes: Option<&crate::pathfinder::TunnelWormholes>,
) -> HashSet<TileCoord> {
    let seed_tiles: HashSet<_> = seeds.iter().map(|entry| entry.tile).collect();
    let mut affected = HashSet::new();
    for &c in signal_tiles {
        let Some(tile) = map.get(c) else {
            continue;
        };
        if seed_tiles.contains(&c) {
            affected.insert(c);
        }
        let present = rail_signal_present_mask(tile.m3);
        let rails = tile.m5 & 0x3F;
        for bit in 0..4u8 {
            if present & (1 << bit) == 0 {
                continue;
            }
            let exit_dir = signal_exit_dir(rails, bit);
            let block = rail_block_ahead_with_wormholes(map, c, exit_dir, wormholes);
            if block.iter().any(|tile| seed_tiles.contains(tile)) {
                affected.insert(c);
            }
        }
    }

    // Cierre de dependencias presignal en ambos sentidos. Una entry/combo
    // afectada necesita los estados de sus exits; una exit afectada invalida
    // entries/combos aguas arriba. Se itera sobre el índice, nunca sobre la grilla.
    loop {
        let count_before = affected.len();
        for &c in signal_tiles {
            let Some(tile) = map.get(c) else {
                continue;
            };
            let present = rail_signal_present_mask(tile.m3);
            let rails = tile.m5 & 0x3F;
            for bit in 0..4u8 {
                if present & (1 << bit) == 0 {
                    continue;
                }
                let sig_type = signal_track_for_bit(rails, bit)
                    .map_or(SIGTYPE_BLOCK, |track| signal_type_for_track(tile.m2, track));
                if sig_type != SIGTYPE_ENTRY && sig_type != SIGTYPE_COMBO {
                    continue;
                }
                let probe = explore_sig_segment(map, c, signal_exit_dir(rails, bit), wormholes);
                if probe.exits.iter().any(|(exit, _)| affected.contains(exit)) {
                    affected.insert(c);
                }
                if affected.contains(&c) {
                    affected.extend(probe.exits.into_iter().map(|(exit, _)| exit));
                }
            }
        }
        if affected.len() == count_before {
            break;
        }
    }
    affected
}

/// Recalcula verde/rojo en **todas** las teselas con señales (API explícita).
///
/// En simulación el tick usa [`drain_signal_globset_with_wormholes`] (incremental).
/// Esta función queda para setup de tests, parity y carga de mapa.
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
    update_rail_signal_states_with_wormholes(map, vehicles, dirty, clear_dirty, None);
}

/// Como [`update_rail_signal_states`], con wormholes JGR para segmentos/bloques.
pub fn update_rail_signal_states_with_wormholes(
    map: &mut Map,
    vehicles: &[Vehicle],
    dirty: &mut Vec<TileCoord>,
    clear_dirty: bool,
    wormholes: Option<&crate::pathfinder::TunnelWormholes>,
) {
    update_rail_signal_states_scoped(map, vehicles, dirty, clear_dirty, None, wormholes);
}

/// Como [`update_rail_signal_states`], limitado a teselas de `scope` (None = mapa entero).
#[allow(clippy::implicit_hasher)]
pub fn update_rail_signal_states_scoped(
    map: &mut Map,
    vehicles: &[Vehicle],
    dirty: &mut Vec<TileCoord>,
    clear_dirty: bool,
    scope: Option<&HashSet<TileCoord>>,
    wormholes: Option<&crate::pathfinder::TunnelWormholes>,
) {
    if clear_dirty {
        dirty.clear();
    }
    if scope.is_some_and(HashSet::is_empty) {
        return;
    }
    let exit_green = compute_exit_signal_greens_scoped(map, vehicles, wormholes, scope);
    let signal_green =
        stabilize_combo_presignal_greens_scoped(map, vehicles, &exit_green, wormholes, scope);
    let refresh_one = |map: &mut Map, c: TileCoord, dirty: &mut Vec<TileCoord>| {
        let Some(tile) = map.get(c) else {
            return;
        };
        if let Some(out) =
            refresh_signal_tile_states(map, vehicles, c, tile, &signal_green, wormholes)
            && out.m3hi != tile.m3hi
        {
            let _ = map.set_tile(c, out);
            dirty.push(c);
        }
    };
    if let Some(tiles) = scope {
        for &c in tiles {
            refresh_one(map, c, dirty);
        }
        return;
    }
    let (w, h) = map.dimensions();
    for y in 0..i32::try_from(h).unwrap_or(i32::MAX) {
        for x in 0..i32::try_from(w).unwrap_or(i32::MAX) {
            refresh_one(map, TileCoord::new(x, y), dirty);
        }
    }
}

/// Drena `_globset`: refresca señales afectadas y vacía la cola.
///
/// Si `globset` está vacío, no hace nada (ahorra el barrido post-movimiento).
pub fn drain_signal_globset(
    map: &mut Map,
    vehicles: &[Vehicle],
    dirty: &mut Vec<TileCoord>,
    globset: &mut SignalGlobSet,
) {
    drain_signal_globset_with_wormholes(map, vehicles, dirty, globset, None);
}

/// Como [`drain_signal_globset`], con wormholes JGR.
pub fn drain_signal_globset_with_wormholes(
    map: &mut Map,
    vehicles: &[Vehicle],
    dirty: &mut Vec<TileCoord>,
    globset: &mut SignalGlobSet,
    wormholes: Option<&crate::pathfinder::TunnelWormholes>,
) {
    if globset.is_empty() {
        return;
    }
    let affected = collect_signals_affected_by_tiles_with_wormholes(map, globset, wormholes);
    globset.clear();
    if affected.is_empty() {
        return;
    }
    update_rail_signal_states_scoped(map, vehicles, dirty, false, Some(&affected), wormholes);
}

/// Drena `_globset` usando un índice persistente, sin barrer la grilla tras inicializarlo.
pub fn drain_signal_globset_indexed_with_wormholes(
    map: &mut Map,
    vehicles: &[Vehicle],
    dirty: &mut Vec<TileCoord>,
    globset: &mut SignalGlobSet,
    index: &mut SignalSpatialIndex,
    wormholes: Option<&crate::pathfinder::TunnelWormholes>,
) {
    if globset.is_empty() {
        return;
    }
    let affected = collect_signals_affected_by_tiles_indexed(map, globset, index, wormholes);
    globset.clear();
    if affected.is_empty() {
        return;
    }
    update_rail_signal_states_scoped(map, vehicles, dirty, false, Some(&affected), wormholes);
}
