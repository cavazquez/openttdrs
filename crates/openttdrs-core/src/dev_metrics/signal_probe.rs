//! Sonda headless: ¿el tren esperó en una señal y reanudó al liberarse el bloque?

use crate::GameState;
use crate::map::TileCoord;
use crate::parity::{ParityEvent, TickRecord};
use crate::vehicle::{Vehicle, VehicleKind};

/// Opciones para observar espera en señal.
#[derive(Debug, Clone, Copy)]
pub struct SignalWaitProbeOptions {
    pub vehicle_id: u32,
    /// Tesela de la señal donde debe detenerse el tren.
    pub signal_tile: TileCoord,
    /// Tren estacionado que ocupa el bloque (se inyecta al llegar el líder a `signal_tile`).
    pub blocker_id: Option<u32>,
    pub blocker_spawn_tile: Option<TileCoord>,
    /// Ticks máximos hasta detectar `SignalWaitStarted`.
    pub max_ticks_until_wait: u64,
    /// Ticks máximos tras retirar el bloqueador hasta `SignalWaitFinished`.
    pub max_ticks_after_release: u64,
}

/// Resultado de la sonda de espera en señal.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SignalWaitReport {
    pub vehicle_id: u32,
    pub signal_tile: TileCoord,
    pub waited: bool,
    pub resumed: bool,
    pub blocker_spawned: bool,
    pub tick_wait_started: Option<u64>,
    pub tick_wait_finished: Option<u64>,
    pub ticks_run: u64,
}

fn spawn_blocker_train(state: &mut GameState, blocker_id: u32, tile: TileCoord) {
    if state.vehicles.iter().any(|v| v.id == blocker_id) {
        return;
    }
    let mut blocker = Vehicle::new(blocker_id, VehicleKind::Train, tile, tile);
    blocker.running = false;
    state.vehicles.push(blocker);
}

fn find_signal_wait_started(
    records: &[TickRecord],
    vehicle_id: u32,
    signal_tile: TileCoord,
) -> Option<u64> {
    records.iter().find_map(|r| {
        r.events.iter().find_map(|e| match e {
            ParityEvent::SignalWaitStarted { vehicle, tile }
                if *vehicle == vehicle_id && *tile == signal_tile =>
            {
                Some(r.tick)
            }
            _ => None,
        })
    })
}

fn find_signal_wait_finished(
    records: &[TickRecord],
    vehicle_id: u32,
    signal_tile: TileCoord,
) -> Option<u64> {
    records.iter().find_map(|r| {
        r.events.iter().find_map(|e| match e {
            ParityEvent::SignalWaitFinished { vehicle, tile }
                if *vehicle == vehicle_id && *tile == signal_tile =>
            {
                Some(r.tick)
            }
            _ => None,
        })
    })
}

fn maybe_spawn_blocker(state: &mut GameState, opts: &SignalWaitProbeOptions) -> bool {
    let (Some(blocker_id), Some(blocker_tile)) = (opts.blocker_id, opts.blocker_spawn_tile) else {
        return false;
    };
    let Some(vehicle) = state.vehicles.iter().find(|v| v.id == opts.vehicle_id) else {
        return false;
    };
    if vehicle.pos != opts.signal_tile || vehicle.cargo == 0 {
        return false;
    }
    spawn_blocker_train(state, blocker_id, blocker_tile);
    true
}

/// Avanza la simulación con traza de paridad y comprueba espera → liberación en señal.
///
/// Si `blocker_spawn_tile` está definido, el bloqueador **no** forma parte del escenario
/// inicial: se coloca cuando el tren cargado llega a `signal_tile`, para no bloquear el
/// lookahead ferroviario desde la estación de carga.
#[must_use]
pub fn probe_signal_wait(state: &mut GameState, opts: &SignalWaitProbeOptions) -> SignalWaitReport {
    if !state.vehicles.iter().any(|v| v.id == opts.vehicle_id) {
        return SignalWaitReport {
            vehicle_id: opts.vehicle_id,
            signal_tile: opts.signal_tile,
            waited: false,
            resumed: false,
            blocker_spawned: false,
            tick_wait_started: None,
            tick_wait_finished: None,
            ticks_run: 0,
        };
    }

    state.enable_parity_trace();
    let mut records = Vec::new();
    let mut ticks_run = 0u64;
    let mut blocker_spawned = opts.blocker_spawn_tile.is_none() && opts.blocker_id.is_some();

    for _ in 0..opts.max_ticks_until_wait {
        if !blocker_spawned {
            blocker_spawned = maybe_spawn_blocker(state, opts);
        }
        state.step();
        ticks_run += 1;
        records.extend(state.take_parity_records());
    }

    let tick_wait_started = find_signal_wait_started(&records, opts.vehicle_id, opts.signal_tile);
    let waited = tick_wait_started.is_some();

    if waited && let Some(blocker_id) = opts.blocker_id {
        state.vehicles.retain(|v| v.id != blocker_id);
    }

    if waited {
        for _ in 0..opts.max_ticks_after_release {
            state.step();
            ticks_run += 1;
            records.extend(state.take_parity_records());
        }
    }

    let tick_wait_finished = tick_wait_started
        .and_then(|_| find_signal_wait_finished(&records, opts.vehicle_id, opts.signal_tile));
    let resumed = tick_wait_finished.is_some();

    SignalWaitReport {
        vehicle_id: opts.vehicle_id,
        signal_tile: opts.signal_tile,
        waited,
        resumed,
        blocker_spawned,
        tick_wait_started,
        tick_wait_finished,
        ticks_run,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parity::{
        TRAIN_LINE_SIGNAL, TRAIN_SUPPLY_BLOCK_TILE, TRAIN_SUPPLY_BLOCKER_ID,
        TRAIN_SUPPLY_VEHICLE_ID, TRAIN_SUPPLY_WAIT_SIGNAL, build_train_supply,
    };


    #[test]
    fn train_supply_waits_and_resumes_at_mid_signal() {
        let mut state = build_train_supply();
        let report = probe_signal_wait(
            &mut state,
            &SignalWaitProbeOptions {
                vehicle_id: TRAIN_SUPPLY_VEHICLE_ID,
                signal_tile: TRAIN_SUPPLY_WAIT_SIGNAL,
                blocker_id: Some(TRAIN_SUPPLY_BLOCKER_ID),
                blocker_spawn_tile: Some(TRAIN_SUPPLY_BLOCK_TILE),
                max_ticks_until_wait: 900,
                max_ticks_after_release: 300,
            },
        );
        assert!(
            report.blocker_spawned,
            "debe inyectar bloqueador: {report:?}"
        );
        assert!(report.waited, "debe esperar en la señal: {report:?}");
        assert!(
            report.resumed,
            "debe reanudar tras retirar bloqueador: {report:?}"
        );
        assert_eq!(report.signal_tile, TRAIN_LINE_SIGNAL);
    }
}
