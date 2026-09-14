//! Cálculo y actualización de reservas de trenes.

use std::collections::HashSet;

use crate::engine::TrainAccelerationModel;
use crate::map::{Map, TileCoord, TileKind, rail_traversal_bits};
use crate::vehicle::{Vehicle, VehicleKind};

use super::conflicts::{
    TrainOccupancyIndex, append_platform_reservation, append_platform_reservation_indexed,
    reserved_steps_overlap, tile_occupied_by_other_train,
};
use super::model::{
    MAX_TRAIN_RESERVATION_LEN, ReservedRailStep, track_for_rail_step, track_for_rail_transition,
    track_on_departure_tile,
};
use super::search::{
    find_path_to_safe_wait_with_wormholes, find_path_to_safe_wait_with_wormholes_indexed,
    is_safe_waiting_position, tile_has_any_pbs_signal,
};

/// Máximo de reservas que el tick incremental reconstruye por pasada.
///
/// Un save real puede traer cientos de trenes sin reserva efímera. Recalcularlos
/// todos en un frame bloquea la simulación; los restantes conservan su reserva
/// previa (o esperan una pasada posterior) sin ocupar rutas ajenas.
pub const MAX_INCREMENTAL_PBS_REFRESHES: usize = 8;

/// `true` si delante del tren hay (o habrá) un segmento PBS que exige reserva.
///
/// El segmento se activa cuando la salida del path está controlada por PBS o
/// termina en una señal de sentido único que el tren no puede atravesar. En
/// este último caso `TryPathReserve` debe dejar la reserva antes de la señal;
/// si se omite, el controlador no tiene el safe wait que usa `OpenTTD` al
/// invertir en el extremo de la línea.
#[must_use]
pub fn vehicle_segment_requires_path_reserve(map: &Map, vehicle: &Vehicle) -> bool {
    let route: Vec<_> = std::iter::once(vehicle.pos)
        .chain(vehicle.path.iter().copied().take(16))
        .collect();
    // Una estación es una posición segura: un path que la atraviesa no debe
    // activar PBS para señales que están detrás del andén. `TrainController`
    // corta allí la ruta efectiva y puede dejar el tren cargando antes de
    // volver a pedir una reserva.
    route
        .windows(2)
        .enumerate()
        .take_while(|(index, window)| {
            *index == 0
                || !map
                    .get(window[0])
                    .is_some_and(|tile| tile.kind == TileKind::Station)
        })
        .any(|(_, window)| {
            let [from, to] = window else {
                return false;
            };
            let Some(tile) = map.get(*from) else {
                return false;
            };
            if tile.kind != TileKind::Rail || !crate::rail_signals::rail_tile_is_signals(tile.m5) {
                return false;
            }
            let Some(step_track) = track_on_departure_tile(map, *from, *to)
                .or_else(|| track_for_rail_step(map, *from, *to))
            else {
                return false;
            };
            let opposing_oneway =
                crate::rail_signals::dir_from_to(*from, *to).is_some_and(|exit_dir| {
                    matches!(
                        crate::rail_signals::yapf_routing_signal(map, *from, exit_dir),
                        crate::rail_signals::YapfSignalRouting::DeadEnd
                    )
                });
            if opposing_oneway {
                return true;
            }
            let rails = tile.m5 & 0x3F;
            crate::rail_signals::signal_bits_for_exit(map, *from, *to)
                .into_iter()
                .any(|bit| {
                    tile.m3 & (0x10 << bit) != 0
                        && crate::rail_signals::signal_track_for_bit(rails, bit).is_some_and(
                            |track| {
                                track.track_bit() & step_track != 0
                                    && crate::rail_signals::is_pbs_signal_type(
                                        crate::rail_signals::signal_type_for_track(tile.m2, track),
                                    )
                            },
                        )
                })
        })
}

/// Calcula la reserva de un tren sin mutar el mapa global de reservas.
#[must_use]
pub fn compute_train_reservation(
    map: &Map,
    vehicles: &[Vehicle],
    vehicle_idx: usize,
    already_reserved: &HashSet<ReservedRailStep>,
) -> Vec<ReservedRailStep> {
    // Helper de tests / callers legacy: fuerza reserva aunque el default vanilla
    // de `pf.reserve_paths` sea `false`.
    let settings = crate::pathfinding_settings::PathfindingSettings {
        reserve_paths: true,
        ..crate::pathfinding_settings::PathfindingSettings::default()
    };
    compute_train_reservation_with_settings(map, vehicles, vehicle_idx, already_reserved, settings)
}

/// Como [`compute_train_reservation`], con settings PBS (look-ahead / `TryReserve`).
#[must_use]
pub fn compute_train_reservation_with_settings(
    map: &Map,
    vehicles: &[Vehicle],
    vehicle_idx: usize,
    already_reserved: &HashSet<ReservedRailStep>,
    settings: crate::pathfinding_settings::PathfindingSettings,
) -> Vec<ReservedRailStep> {
    compute_train_reservation_with_wormholes(
        map,
        vehicles,
        vehicle_idx,
        already_reserved,
        settings,
        None,
    )
}

/// Como [`compute_train_reservation_with_settings`], con wormholes de túnel en `TryReserve`.
#[must_use]
pub fn compute_train_reservation_with_wormholes(
    map: &Map,
    vehicles: &[Vehicle],
    vehicle_idx: usize,
    already_reserved: &HashSet<ReservedRailStep>,
    settings: crate::pathfinding_settings::PathfindingSettings,
    wormholes: Option<&crate::pathfinder::TunnelWormholes>,
) -> Vec<ReservedRailStep> {
    compute_train_reservation_with_wormholes_impl(
        map,
        vehicles,
        vehicle_idx,
        already_reserved,
        settings,
        wormholes,
        None,
    )
}

/// Variante interna que reutiliza la ocupación física indexada del pase PBS.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub(super) fn compute_train_reservation_with_wormholes_indexed(
    map: &Map,
    vehicles: &[Vehicle],
    vehicle_idx: usize,
    already_reserved: &HashSet<ReservedRailStep>,
    settings: crate::pathfinding_settings::PathfindingSettings,
    wormholes: Option<&crate::pathfinder::TunnelWormholes>,
    occupancy: &TrainOccupancyIndex,
) -> Vec<ReservedRailStep> {
    compute_train_reservation_with_wormholes_impl(
        map,
        vehicles,
        vehicle_idx,
        already_reserved,
        settings,
        wormholes,
        Some(occupancy),
    )
}

#[allow(clippy::too_many_lines, clippy::too_many_arguments)]
fn compute_train_reservation_with_wormholes_impl(
    map: &Map,
    vehicles: &[Vehicle],
    vehicle_idx: usize,
    already_reserved: &HashSet<ReservedRailStep>,
    settings: crate::pathfinding_settings::PathfindingSettings,
    wormholes: Option<&crate::pathfinder::TunnelWormholes>,
    occupancy: Option<&TrainOccupancyIndex>,
) -> Vec<ReservedRailStep> {
    let vehicle = &vehicles[vehicle_idx];
    if vehicle.kind != VehicleKind::Train || !vehicle.running {
        return Vec::new();
    }
    // OpenTTD: `SIGSEG_PBS || pf.reserve_paths`. Sin path signal delante y con
    // `reserve_paths=false` (default vanilla) las redes de block/entry/exit no
    // crean reservas PBS; gobiernan las señales clásicas.
    if !settings.reserve_paths && !vehicle_segment_requires_path_reserve(map, vehicle) {
        return Vec::new();
    }

    let path: Vec<TileCoord> = vehicle.path.iter().copied().collect();
    let mut along_path =
        reserve_along_path(map, vehicles, vehicle, &path, already_reserved, occupancy);
    if let Some(occupancy) = occupancy {
        append_platform_reservation_indexed(
            map,
            occupancy,
            vehicle,
            already_reserved,
            &mut along_path,
        );
    } else {
        append_platform_reservation(map, vehicles, vehicle, already_reserved, &mut along_path);
    }
    if reservation_ends_at_safe_wait_steps(map, vehicle.pos, &path, &along_path) {
        return along_path;
    }
    // TryReservePath: si el path de órdenes no llega a safe wait, buscar alternativa.
    // `path_backoff_interval == 255` desactiva look-ahead; si no, solo reintenta cuando
    // `should_retry_reservation(wait_counter)` (trenes no stuck tienen wait_counter=0 → siempre).
    if !settings.should_retry_reservation(vehicle.wait_counter) {
        return along_path;
    }
    let alt = if let Some(occupancy) = occupancy {
        find_path_to_safe_wait_with_wormholes_indexed(
            map,
            vehicles,
            vehicle.id,
            vehicle.pos,
            &path,
            already_reserved,
            wormholes,
            occupancy,
        )
    } else {
        find_path_to_safe_wait_with_wormholes(
            map,
            vehicles,
            vehicle.id,
            vehicle.pos,
            &path,
            already_reserved,
            wormholes,
        )
    };
    let Some(alt) = alt else {
        return along_path;
    };
    let mut alt_res = reserve_along_path(map, vehicles, vehicle, &alt, already_reserved, occupancy);
    if let Some(occupancy) = occupancy {
        append_platform_reservation_indexed(
            map,
            occupancy,
            vehicle,
            already_reserved,
            &mut alt_res,
        );
    } else {
        append_platform_reservation(map, vehicles, vehicle, already_reserved, &mut alt_res);
    }
    if alt_res.len() > along_path.len()
        || (alt_res.len() >= along_path.len()
            && reservation_ends_at_safe_wait_steps(map, vehicle.pos, &alt, &alt_res))
    {
        alt_res
    } else {
        along_path
    }
}

pub(super) fn reservation_ends_at_safe_wait_steps(
    map: &Map,
    pos: TileCoord,
    path: &[TileCoord],
    reserved: &[ReservedRailStep],
) -> bool {
    let Some(last) = reserved.last() else {
        return false;
    };
    let full: Vec<TileCoord> = std::iter::once(pos).chain(path.iter().copied()).collect();
    let next = full
        .iter()
        .position(|&c| c == last.tile)
        .and_then(|i| full.get(i + 1).copied());
    let passed_path = reserved
        .iter()
        .any(|s| tile_has_any_pbs_signal(map, s.tile));
    is_safe_waiting_position(map, last.tile, next, passed_path)
}

fn reserve_along_path(
    map: &Map,
    vehicles: &[Vehicle],
    vehicle: &Vehicle,
    path: &[TileCoord],
    already_reserved: &HashSet<ReservedRailStep>,
    occupancy: Option<&TrainOccupancyIndex>,
) -> Vec<ReservedRailStep> {
    // Desde depósito el follower PBS empieza en la boca (`path[0]`).
    if map.get_kind(vehicle.pos) == Some(TileKind::RailDepot) {
        return reserve_along_path_from_depot(
            map,
            vehicles,
            vehicle,
            path,
            already_reserved,
            occupancy,
        );
    }

    let mut out = Vec::new();
    let mut cur = vehicle.pos;
    let Some(pos_track) = path
        .first()
        .and_then(|&next| track_on_departure_tile(map, cur, next))
    else {
        let tb = rail_traversal_bits(map, cur);
        let track = (0..6u8)
            .find_map(|i| {
                let bit = 1_u8 << i;
                if tb & bit != 0 { Some(bit) } else { None }
            })
            .unwrap_or(tb & 0x3F);
        if track != 0 {
            out.push(ReservedRailStep::new(cur, track));
        }
        return out;
    };
    let start_step = ReservedRailStep::new(cur, pos_track);
    if reserved_steps_overlap(already_reserved, cur, pos_track)
        || reservation_tile_occupied(map, vehicles, vehicle.id, cur, pos_track, occupancy)
    {
        return out;
    }
    out.push(start_step);
    let extension_blocked = extend_reservation_along_path(
        map,
        vehicles,
        vehicle.id,
        path,
        already_reserved,
        &mut out,
        &mut cur,
        0,
        occupancy,
    );
    if extension_blocked {
        // `ExtendTrainReservation` is transaccional: si no puede alcanzar una
        // posición segura, OpenTTD deshace todos los pasos añadidos y
        // `ChooseTrainTrack` conserva únicamente la tesela actual.
        out.truncate(1);
    }
    out
}

fn reserve_along_path_from_depot(
    map: &Map,
    vehicles: &[Vehicle],
    vehicle: &Vehicle,
    path: &[TileCoord],
    already_reserved: &HashSet<ReservedRailStep>,
    occupancy: Option<&TrainOccupancyIndex>,
) -> Vec<ReservedRailStep> {
    let mut out = Vec::new();
    let Some(&entrance) = path.first() else {
        return out;
    };
    let beyond = path.get(1).copied();
    let Some(track) = beyond
        .and_then(|next| track_on_departure_tile(map, entrance, next))
        .or_else(|| {
            let tb = rail_traversal_bits(map, entrance);
            (0..6u8).find_map(|i| {
                let bit = 1_u8 << i;
                if tb & bit != 0 { Some(bit) } else { None }
            })
        })
    else {
        return out;
    };
    let step = ReservedRailStep::new(entrance, track);
    if reserved_steps_overlap(already_reserved, entrance, track)
        || reservation_tile_occupied(map, vehicles, vehicle.id, entrance, track, occupancy)
    {
        return out;
    }
    out.push(step);
    let mut cur = entrance;
    if is_safe_waiting_position(map, cur, beyond, tile_has_any_pbs_signal(map, cur)) {
        return out;
    }
    let extension_blocked = extend_reservation_along_path(
        map,
        vehicles,
        vehicle.id,
        path,
        already_reserved,
        &mut out,
        &mut cur,
        1,
        occupancy,
    );
    if extension_blocked {
        out.truncate(1);
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn extend_reservation_along_path(
    map: &Map,
    vehicles: &[Vehicle],
    vehicle_id: u32,
    path: &[TileCoord],
    already_reserved: &HashSet<ReservedRailStep>,
    out: &mut Vec<ReservedRailStep>,
    cur: &mut TileCoord,
    path_skip: usize,
    occupancy: Option<&TrainOccupancyIndex>,
) -> bool {
    let mut passed_path = tile_has_any_pbs_signal(map, *cur);
    for (i, &next) in path.iter().enumerate().skip(path_skip) {
        if out.len() >= MAX_TRAIN_RESERVATION_LEN {
            break;
        }
        let beyond = path.get(i + 1).copied();
        if !crate::rail_signals::rail_step_signal_allows(map, vehicles, *cur, next, beyond) {
            // Una señal PathOneWay que mira en sentido contrario es un
            // extremo de vía utilizable: se reserva hasta `cur` y se espera
            // allí, igual que `IsSafeWaitingPosition` en el oráculo nativo.
            // Las señales rojas normales no son una posición segura y deben
            // conservar el rollback transaccional de `ExtendTrainReservation`.
            if is_safe_waiting_position(map, *cur, Some(next), passed_path) {
                break;
            }
            return true;
        }
        let Some(track) = beyond
            .and_then(|after| track_for_rail_transition(map, *cur, next, after))
            .or_else(|| track_on_departure_tile(map, *cur, next))
            .or_else(|| track_for_rail_step(map, *cur, next))
        else {
            return true;
        };
        let step = ReservedRailStep::new(next, track);
        if reserved_steps_overlap(already_reserved, next, track) {
            return map.get_kind(next) == Some(TileKind::Station);
        }
        if reservation_tile_occupied(map, vehicles, vehicle_id, next, track, occupancy) {
            return map.get_kind(next) == Some(TileKind::Station);
        }
        out.push(step);
        *cur = next;
        if tile_has_any_pbs_signal(map, *cur) {
            passed_path = true;
        }
        if is_safe_waiting_position(map, *cur, beyond, passed_path) {
            break;
        }
    }
    false
}

fn reservation_tile_occupied(
    map: &Map,
    vehicles: &[Vehicle],
    self_id: u32,
    tile: TileCoord,
    track: u8,
    occupancy: Option<&TrainOccupancyIndex>,
) -> bool {
    occupancy.map_or_else(
        || tile_occupied_by_other_train(map, vehicles, self_id, tile, track),
        |index| index.tile_occupied_by_other_train(map, self_id, tile, track),
    )
}

/// Conserva pasos de una reserva previa aún válidos si `TryReserve` falla
/// (`FollowTrainReservation` simplificado de `pbs.cpp`).
///
/// Si hay reserva nueva, se usa. Si queda vacía pero el tren sigue sobre la
/// reserva anterior, se mantienen los pasos bajo el tren y por delante en `path`.
#[must_use]
pub fn follow_train_reservation(
    previous: &[ReservedRailStep],
    newly_reserved: Vec<ReservedRailStep>,
    vehicle: &Vehicle,
) -> Vec<ReservedRailStep> {
    if !newly_reserved.is_empty() {
        return newly_reserved;
    }
    if previous.is_empty() {
        return newly_reserved;
    }
    if !previous.iter().any(|s| s.tile == vehicle.pos) {
        return newly_reserved;
    }
    let path_tiles: HashSet<TileCoord> = vehicle.path.iter().copied().collect();
    previous
        .iter()
        .copied()
        .filter(|s| s.tile == vehicle.pos || path_tiles.contains(&s.tile))
        .collect()
}

/// Añade a la reserva las teselas aún ocupadas por el consist (vagones detrás
/// de la cabeza). `OpenTTD` mantiene esas pistas reservadas hasta que la cola las
/// abandona.
fn merge_consist_footprint(
    map: &Map,
    vehicles: &[Vehicle],
    fleet: &crate::fleet_index::FleetIndex,
    head_id: u32,
    mut reserved: Vec<ReservedRailStep>,
) -> Vec<ReservedRailStep> {
    // Una lista vacía significa que el segmento actual no requiere PBS
    // (por ejemplo, un tren importado sin ruta o una red de block signals con
    // `reserve_paths=false`). El footprint físico no es por sí solo una
    // reserva nativa: añadir la tesela actual aquí fabrica `m2_hi` aunque
    // OpenTTD todavía no haya elegido una ruta.
    if reserved.is_empty() {
        return reserved;
    }
    let occupied = crate::train_consist::consist_occupied_tiles_indexed(vehicles, fleet, head_id);
    let existing: HashSet<TileCoord> = reserved.iter().map(|s| s.tile).collect();
    for tile in occupied {
        if existing.contains(&tile) {
            continue;
        }
        // El depósito puede albergar varios consists; no es pista PBS reservable.
        if map.get_kind(tile) == Some(TileKind::RailDepot) {
            continue;
        }
        let tb = rail_traversal_bits(map, tile);
        let track = (0..6u8)
            .find_map(|i| {
                let bit = 1_u8 << i;
                if tb & bit != 0 { Some(bit) } else { None }
            })
            .unwrap_or(tb & 0x3F);
        if track != 0 {
            reserved.push(ReservedRailStep::new(tile, track));
        }
    }
    reserved
}

/// Recalcula `reserved_steps` de todos los trenes (orden por índice = prioridad).
///
/// Helper legacy/tests: `reserve_paths=true`. La simulación real usa
/// [`update_train_reservations_with_settings`] con `GameState.pathfinding`.
pub fn update_train_reservations(map: &Map, vehicles: &mut [Vehicle]) {
    let settings = crate::pathfinding_settings::PathfindingSettings {
        reserve_paths: true,
        ..crate::pathfinding_settings::PathfindingSettings::default()
    };
    update_train_reservations_with_settings(map, vehicles, settings);
}

/// Como [`update_train_reservations`], con settings PBS.
pub fn update_train_reservations_with_settings(
    map: &Map,
    vehicles: &mut [Vehicle],
    settings: crate::pathfinding_settings::PathfindingSettings,
) {
    update_train_reservations_with_wormholes(map, vehicles, settings, None);
}

/// Como [`update_train_reservations_with_settings`], con wormholes en `TryReserve`.
pub fn update_train_reservations_with_wormholes(
    map: &Map,
    vehicles: &mut [Vehicle],
    settings: crate::pathfinding_settings::PathfindingSettings,
    wormholes: Option<&crate::pathfinder::TunnelWormholes>,
) {
    let mut fleet = crate::fleet_index::FleetIndex::default();
    fleet.rebuild(vehicles);
    let mut global = HashSet::new();
    for i in 0..vehicles.len() {
        // Solo cabezas de consist reservan; vagones siguen la huella de la cabeza.
        if vehicles[i].kind != VehicleKind::Train || !vehicles[i].is_consist_head() {
            vehicles[i].reserved_steps.clear();
            continue;
        }
        // Un tren todavía cerrado dentro del depósito no participa del PBS
        // global. `tick_train_stay_in_depot` reserva atómicamente al autorizar
        // su salida; reservar antes permitía que varios consists apilados se
        // bloquearan entre sí y ocuparan el bloque de la vía principal.
        if map.get_kind(vehicles[i].pos) == Some(TileKind::RailDepot)
            && !vehicles[i].depot_leave_cleared
        {
            vehicles[i].reserved_steps.clear();
            continue;
        }
        let head_id = vehicles[i].id;
        let previous = vehicles[i].reserved_steps.clone();
        let reserved = compute_train_reservation_with_wormholes(
            map, vehicles, i, &global, settings, wormholes,
        );
        let reserved = follow_train_reservation(&previous, reserved, &vehicles[i]);
        let reserved = merge_consist_footprint(map, vehicles, &fleet, head_id, reserved);
        for step in &reserved {
            global.insert(*step);
        }
        vehicles[i].reserved_steps = reserved;
    }
}

/// Actualiza solo las reservas PBS vencidas o ausentes.
///
/// A diferencia de [`update_train_reservations_with_wormholes`], este camino
/// conserva las reservas que aún cubren el siguiente paso del tren y limita el
/// trabajo de recuperación tras cargar partidas grandes. Las reservas previas
/// se cargan primero en el conjunto global para que una actualización parcial
/// no permita solapamientos.
pub fn update_train_reservations_incremental_with_wormholes(
    map: &Map,
    vehicles: &mut [Vehicle],
    settings: crate::pathfinding_settings::PathfindingSettings,
    wormholes: Option<&crate::pathfinder::TunnelWormholes>,
) -> usize {
    update_train_reservations_incremental_with_wormholes_and_acceleration(
        map,
        vehicles,
        settings,
        wormholes,
        TrainAccelerationModel::Original,
    )
}

/// Variante incremental que usa el modelo físico real de la partida para
/// decidir cuándo `TrainController` puede abandonar la tesela actual.
///
/// `OpenTTD` no reintenta `TryPathReserve` sólo porque exista un `path`: la
/// reserva se actualiza desde el controlador cuando los dos handlers de ese
/// tick pueden alcanzar el borde. Usar el modelo del save evita fabricar PBS
/// anticipado al cerrar una estación, especialmente con aceleración realista.
pub fn update_train_reservations_incremental_with_wormholes_and_acceleration(
    map: &Map,
    vehicles: &mut [Vehicle],
    settings: crate::pathfinding_settings::PathfindingSettings,
    wormholes: Option<&crate::pathfinder::TunnelWormholes>,
    train_accel: TrainAccelerationModel,
) -> usize {
    update_train_reservations_incremental_with_wormholes_and_acceleration_phase(
        map,
        vehicles,
        settings,
        wormholes,
        train_accel,
        true,
    )
}

/// Variante interna que permite distinguir el barrido PBS previo al
/// movimiento del barrido de limpieza posterior. El `TryPathReserve` nativo
/// de un tren atascado ocurre en su handler de movimiento; el pase posterior
/// no debe volver a intentarlo después de que otro tren ya haya avanzado en
/// ese mismo tick.
#[allow(clippy::too_many_arguments)]
pub(crate) fn update_train_reservations_incremental_with_wormholes_and_acceleration_phase(
    map: &Map,
    vehicles: &mut [Vehicle],
    settings: crate::pathfinding_settings::PathfindingSettings,
    wormholes: Option<&crate::pathfinder::TunnelWormholes>,
    train_accel: TrainAccelerationModel,
    allow_stuck_wait_retry: bool,
) -> usize {
    let mut fleet = crate::fleet_index::FleetIndex::default();
    fleet.rebuild(vehicles);
    let occupancy = TrainOccupancyIndex::from_vehicles(map, vehicles, &fleet);
    let mut global = HashSet::new();
    for i in 0..vehicles.len() {
        if vehicles[i].kind != VehicleKind::Train || !vehicles[i].is_consist_head() {
            vehicles[i].reserved_steps.clear();
            continue;
        }
        if map.get_kind(vehicles[i].pos) == Some(TileKind::RailDepot)
            && !vehicles[i].depot_leave_cleared
        {
            vehicles[i].reserved_steps.clear();
            continue;
        }

        /*
         * Actualizar una reserva no puede significar conservar para siempre
         * su extremo viejo: al cruzar una señal el `path` se acorta aunque el
         * siguiente paso siga estando reservado. El barrido completo lo
         * descartaba incidentalmente; el camino incremental debe podarlo de
         * forma barata antes de usarlo como obstáculo global.
         *
         * Las teselas que la cola del consist aún ocupa se vuelven a añadir,
         * porque no necesariamente aparecen en el path de la cabeza.
         */
        let head_id = vehicles[i].id;
        let previous = std::mem::take(&mut vehicles[i].reserved_steps);
        let retained = prune_train_reservation_to_active_path(
            map,
            vehicles,
            &fleet,
            head_id,
            &vehicles[i],
            previous,
        );
        global.extend(retained.iter().copied());
        vehicles[i].reserved_steps = retained;
    }

    let mut refreshed = 0;
    for i in 0..vehicles.len() {
        if vehicles[i].kind != VehicleKind::Train || !vehicles[i].is_consist_head() {
            vehicles[i].reserved_steps.clear();
            continue;
        }
        if map.get_kind(vehicles[i].pos) == Some(TileKind::RailDepot)
            && !vehicles[i].depot_leave_cleared
        {
            for step in &vehicles[i].reserved_steps {
                global.remove(step);
            }
            vehicles[i].reserved_steps.clear();
            continue;
        }
        if !train_reservation_needs_refresh(
            &vehicles[i],
            train_accel,
            settings,
            allow_stuck_wait_retry,
        ) || refreshed >= MAX_INCREMENTAL_PBS_REFRESHES
        {
            continue;
        }

        let head_id = vehicles[i].id;
        let previous = vehicles[i].reserved_steps.clone();
        for step in &previous {
            global.remove(step);
        }
        let reserved = compute_train_reservation_with_wormholes_indexed(
            map, vehicles, i, &global, settings, wormholes, &occupancy,
        );
        let retry_due = allow_stuck_wait_retry
            && vehicles[i].pbs_stuck
            && settings.should_retry_reservation(vehicles[i].wait_counter.saturating_add(1));
        let reserved = if retry_due
            && !reservation_ends_at_safe_wait_steps(
                map,
                vehicles[i].pos,
                &vehicles[i].path.iter().copied().collect::<Vec<_>>(),
                &reserved,
            ) {
            // `TrainLocoHandler` hace rollback si el intento periódico no
            // consigue una posición segura; no deja publicada la extensión
            // parcial que haya recorrido `ChooseTrainTrack`.
            previous.clone()
        } else {
            follow_train_reservation(&previous, reserved, &vehicles[i])
        };
        let reserved = merge_consist_footprint(map, vehicles, &fleet, head_id, reserved);
        global.extend(reserved.iter().copied());
        vehicles[i].reserved_steps = reserved;
        refreshed += 1;
    }
    refreshed
}

/// Elimina de una reserva incremental los pasos que ya no están en la ruta
/// activa. Conserva la posición actual y la huella física del consist, porque
/// la cola puede seguir usando una tesela que la cabeza ya quitó de `path`.
fn prune_train_reservation_to_active_path(
    map: &Map,
    vehicles: &[Vehicle],
    fleet: &crate::fleet_index::FleetIndex,
    head_id: u32,
    vehicle: &Vehicle,
    previous: Vec<ReservedRailStep>,
) -> Vec<ReservedRailStep> {
    let path_tiles: HashSet<TileCoord> = vehicle.path.iter().copied().collect();
    let occupied: HashSet<TileCoord> =
        crate::train_consist::consist_occupied_tiles_indexed(vehicles, fleet, head_id)
            .into_iter()
            .collect();
    let retained = previous
        .into_iter()
        .filter(|step| {
            step.tile == vehicle.pos
                || path_tiles.contains(&step.tile)
                || occupied.contains(&step.tile)
        })
        .collect();
    merge_consist_footprint(map, vehicles, fleet, head_id, retained)
}

fn train_reservation_needs_refresh(
    vehicle: &Vehicle,
    train_accel: TrainAccelerationModel,
    settings: crate::pathfinding_settings::PathfindingSettings,
    allow_stuck_wait_retry: bool,
) -> bool {
    if !vehicle.running || vehicle.path.is_empty() {
        return false;
    }
    if vehicle.reserved_steps.is_empty() {
        // Una cabeza que todavía no tiene reserva debe ejecutar el primer
        // `TryPathReserve` tan pronto como pueda moverse. El backoff sólo
        // gobierna reintentos de una reserva ya existente; aplicarlo aquí
        // deja sin reservar escenarios PBS lentos cuyo contador comienza en
        // cero.
        return vehicle.cur_speed > 0;
    }
    // `CheckNextTrainTile` se ejecuta al entrar una tesela o en el backoff
    // periódico de `TrainController`; una predicción basada sólo en velocidad
    // también dispara la reserva antes de tiempo al frenar ante una señal roja.
    let backoff_due = settings.path_backoff_interval
        != crate::pathfinding_settings::PBS_WAIT_FOREVER
        && u32::from(vehicle.newgrf_tick_counter)
            .is_multiple_of(u32::from(settings.path_backoff_interval.max(1)));
    // La fase PBS corre antes de `Vehicle::Tick`, pero OpenTTD incrementa
    // `wait_counter` dentro del handler que acaba de quedar bloqueado. Usar
    // el valor siguiente permite que el reintento del múltiplo (20, 40, ...)
    // ocurra en el mismo tick en que vence el backoff nativo.
    let pbs_wait_backoff_due = allow_stuck_wait_retry
        && vehicle.pbs_stuck
        && settings.should_retry_reservation(vehicle.wait_counter.saturating_add(1));
    if !vehicle.train_tile_entered_this_tick
        && !pbs_wait_backoff_due
        && (!backoff_due || !vehicle.train_would_leave_tile_this_tick(train_accel))
    {
        return false;
    }
    vehicle
        .movement_target()
        .is_some_and(|next| !vehicle.reserved_steps.iter().any(|step| step.tile == next))
}

/// `true` si el tren no puede avanzar al `movement_target` (sin reserva en esa pista).
///
/// Si la reserva incluye la tesela actual pero aún no el siguiente paso (p. ej. señal
/// block roja cortó la extensión), no bloquea aquí: `train_blocked_by_signal` gobierna.
/// Path exige reserva más allá vía `path_exit_lacks_reservation`.
#[must_use]
pub fn train_blocked_by_reservation(map: &Map, vehicle: &Vehicle) -> bool {
    if vehicle.kind != VehicleKind::Train || !vehicle.running {
        return false;
    }
    if map.get_kind(vehicle.pos) == Some(TileKind::RailDepot) {
        return false;
    }
    if vehicle.reserved_steps.is_empty() {
        return false;
    }
    let Some(next) = vehicle.movement_target() else {
        return false;
    };
    if vehicle.path.front() != Some(&next) {
        return false;
    }
    let Some(track) = track_on_departure_tile(map, vehicle.pos, next)
        .or_else(|| track_for_rail_step(map, vehicle.pos, next))
    else {
        return false;
    };
    if vehicle.reserved_steps.iter().any(|s| {
        s.tile == next && (s.track == track || super::conflicts::tracks_overlap(s.track, track))
    }) {
        return false;
    }
    if vehicle.reserved_steps.iter().any(|s| s.tile == vehicle.pos) {
        return false;
    }
    true
}
