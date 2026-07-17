//! Cola de construcción IA: planificar en tick mensual, aplicar 1 comando cada N ticks.

use std::collections::VecDeque;

use crate::GameState;
use crate::cargo::CargoType;
use crate::command::{Command, apply_command};
use crate::company::CompanyId;
use crate::map::TileCoord;
use crate::pathfinder::{PathNetwork, find_path};

/// Intervalo entre comandos de obra (~0.65 s a ~37 Hz).
pub const AI_BUILD_COMMANDS_INTERVAL_TICKS: u64 = 24;

/// Cierre de una obra encolada: comprar vehículo solo si la infra cerró bien.
#[derive(Debug, Clone)]
pub enum AiBuildFinish {
    TransCargo {
        load_st: TileCoord,
        unload_st: TileCoord,
        depot: TileCoord,
        source: TileCoord,
        dest: TileCoord,
        cargo: CargoType,
    },
    RoadHaul {
        stop_a: TileCoord,
        stop_b: TileCoord,
        depot: TileCoord,
    },
}

/// Una obra activa por compañía rival.
#[derive(Debug, Clone)]
pub struct AiBuildQueue {
    pub company: CompanyId,
    pub commands: VecDeque<Command>,
    pub finish: AiBuildFinish,
}

#[must_use]
pub fn company_has_build_queue(state: &GameState, company: CompanyId) -> bool {
    state.ai_build_queues.iter().any(|q| q.company == company)
}

pub fn enqueue_build_queue(state: &mut GameState, queue: AiBuildQueue) {
    if company_has_build_queue(state, queue.company) {
        return;
    }
    state.ai_build_queues.push(queue);
}

/// Aplica como máximo un comando por cola activa cuando `tick` cae en el intervalo.
pub fn drain_ai_build_queues(state: &mut GameState, tick: u64) {
    if state.ai_build_queues.is_empty() {
        return;
    }
    if !tick.is_multiple_of(AI_BUILD_COMMANDS_INTERVAL_TICKS) {
        return;
    }

    let mut completed: Vec<AiBuildQueue> = Vec::new();
    let mut i = 0;
    while i < state.ai_build_queues.len() {
        let company = state.ai_build_queues[i].company;
        let cmd = state.ai_build_queues[i].commands.pop_front();
        if let Some(cmd) = cmd {
            // Fallo no fatal: saltar el comando (ya extraído) y seguir drenando.
            with_ai_active(state, company, |state| {
                let _ = apply_command(state, &cmd);
            });
        }
        if state.ai_build_queues[i].commands.is_empty() {
            completed.push(state.ai_build_queues.remove(i));
        } else {
            i += 1;
        }
    }

    for queue in completed {
        complete_build_queue(state, &queue);
    }
}

fn complete_build_queue(state: &mut GameState, queue: &AiBuildQueue) {
    match &queue.finish {
        AiBuildFinish::TransCargo {
            load_st,
            unload_st,
            depot,
            source,
            dest,
            cargo,
        } => {
            super::transcargo::complete_freight_build(
                state,
                queue.company,
                *load_st,
                *unload_st,
                *depot,
                super::transcargo::plan::RoutePlan {
                    source: *source,
                    dest: *dest,
                    cargo: *cargo,
                },
            );
        }
        AiBuildFinish::RoadHaul {
            stop_a,
            stop_b,
            depot,
        } => {
            super::roadhaul::complete_bus_build(state, queue.company, *stop_a, *stop_b, *depot);
        }
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

/// Path rail entre estaciones (revalidación al cerrar obra).
#[must_use]
pub(crate) fn rail_endpoints_connected(state: &GameState, a: TileCoord, b: TileCoord) -> bool {
    find_path(&state.map, a, b, PathNetwork::Rail).is_some()
}

/// Path road entre paradas.
#[must_use]
pub(crate) fn road_endpoints_connected(state: &GameState, a: TileCoord, b: TileCoord) -> bool {
    find_path(&state.map, a, b, PathNetwork::Road).is_some()
}

/// Graba comandos de un build one-shot sobre un clon del estado.
pub(crate) fn record_build_commands(
    state: &GameState,
    build: impl FnOnce(&mut GameState),
) -> VecDeque<Command> {
    let mut tmp = state.clone();
    tmp.ai_build_queues.clear();
    tmp.runtime.command_recorder = Some(VecDeque::new());
    build(&mut tmp);
    tmp.runtime.command_recorder.take().unwrap_or_default()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::ai::tick_ai_companies;
    use crate::company::RIVAL_NAME_TRANSCARGO;
    use crate::map::TileKind;
    use crate::vehicle::VehicleKind;

    #[test]
    fn freight_queue_builds_progressively_then_buys_train() {
        let mut state = crate::parity::build_ai_rival_line();
        let tc_id = state
            .companies
            .iter()
            .find(|c| c.name == RIVAL_NAME_TRANSCARGO)
            .expect("TransCargo")
            .id;

        let plan = crate::ai::transcargo::plan::next_unserved_plan(&state, tc_id)
            .expect("plan carbón→fábrica");
        let queue = crate::ai::transcargo::build::plan_freight_line_queue(&state, tc_id, plan)
            .expect("plan_freight_line_queue");
        enqueue_build_queue(&mut state, queue);

        assert!(company_has_build_queue(&state, tc_id));
        let total_cmds = state.ai_build_queues[0].commands.len();
        assert!(total_cmds > 8, "cola con varios comandos: {total_cmds}");

        // Tras unos pocos drenados: vía parcial, aún sin tren.
        let partial_steps = (total_cmds / 4).max(3);
        for _ in 0..partial_steps {
            let t = state.tick.get();
            let aligned = t - (t % AI_BUILD_COMMANDS_INTERVAL_TICKS);
            tick_ai_companies(&mut state, aligned);
            state.tick = crate::GameTick::new(aligned + AI_BUILD_COMMANDS_INTERVAL_TICKS);
        }
        let (mw, mh) = state.map.dimensions();
        let rails = (0..mw.cast_signed())
            .flat_map(|x| (0..mh.cast_signed()).map(move |y| TileCoord::new(x, y)))
            .filter(|&c| state.map.get_kind(c) == Some(TileKind::Rail))
            .count();
        assert!(rails > 0, "debe haber vía parcial tras K ticks");
        assert!(
            !state
                .vehicles
                .iter()
                .any(|v| v.owner == tc_id && v.kind == VehicleKind::Train),
            "sin tren hasta cerrar la cola"
        );

        for _ in 0..20_000 {
            if !company_has_build_queue(&state, tc_id)
                && state
                    .vehicles
                    .iter()
                    .any(|v| v.owner == tc_id && v.kind == VehicleKind::Train)
            {
                break;
            }
            let t = state.tick.get();
            let aligned = if t.is_multiple_of(AI_BUILD_COMMANDS_INTERVAL_TICKS) {
                t
            } else {
                t + (AI_BUILD_COMMANDS_INTERVAL_TICKS - t % AI_BUILD_COMMANDS_INTERVAL_TICKS)
            };
            tick_ai_companies(&mut state, aligned);
            state.tick = crate::GameTick::new(aligned + 1);
        }

        assert!(!company_has_build_queue(&state, tc_id));
        let trains = state
            .vehicles
            .iter()
            .filter(|v| v.owner == tc_id && v.kind == VehicleKind::Train && v.is_consist_head())
            .count();
        assert_eq!(trains, 1, "tren comprado al cerrar infra");

        let stations: Vec<_> = state
            .stations
            .iter()
            .filter(|s| s.owner == tc_id)
            .map(|s| s.pos)
            .collect();
        assert!(stations.len() >= 2, "dos estaciones IA");
        assert!(
            rail_endpoints_connected(&state, stations[0], stations[1]),
            "estaciones unidas por path"
        );
    }
}
