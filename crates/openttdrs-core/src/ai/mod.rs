//! Hooks para decisión automática de compañías (rivales CPU).
//!
//! Ver `docs/archive/epics/ai_rivals.md` y pathfind #184.
//! Rivales Rust: `TransCargo` (rail) + `RoadHaul` (buses).

mod build_queue;
mod roadhaul;
mod settings;
pub(crate) mod transcargo;

pub use build_queue::{
    AI_BUILD_COMMANDS_INTERVAL_TICKS, AiBuildFinish, AiBuildQueue, company_has_build_queue,
    drain_ai_build_queues, enqueue_build_queue,
};
pub use settings::{AiSettings, DEFAULT_AI_BUILD_MONEY_THRESHOLD, DEFAULT_AI_MAX_ROUTES};
pub use transcargo::{AI_BUILD_MONEY_THRESHOLD, MAX_AI_ROUTES, TransCargoAi};

use crate::GameState;
use crate::command::Command;
use crate::company::{RIVAL_NAME_ROADHAUL, RIVAL_NAME_TRANSCARGO};
use crate::economy::TICKS_PER_MONTH;
use crate::format_money;
use crate::vehicle::{VehicleKind, VehicleOrder};

/// Política que propone comandos para una compañía (rival o bot de pruebas).
pub trait CompanyAi {
    fn decide(&self, state: &GameState) -> Vec<Command>;
}

/// Ciclo de IA: mantenimiento cada tick; drenado de obra; encolado mensual.
pub fn tick_ai_companies(state: &mut GameState, tick: u64) {
    if !state.ai.enabled {
        return;
    }
    if !state.companies.iter().any(|c| c.is_ai) {
        return;
    }
    transcargo::maintain_transcargo_vehicles(state);
    roadhaul::maintain_roadhaul_vehicles(state);
    drain_ai_build_queues(state, tick);
    if tick == 0 || !tick.is_multiple_of(TICKS_PER_MONTH) {
        return;
    }
    transcargo::tick_transcargo(state);
    roadhaul::tick_roadhaul(state);
}

/// Texto de debug para la ventana AI settings (#44).
#[must_use]
pub fn format_ai_debug_status(state: &GameState) -> String {
    let ai = state.ai.clamped();
    let mut lines = vec![format!(
        "IA: {} · umbral {} · máx. rutas rail {}",
        if ai.enabled { "ON" } else { "OFF" },
        format_money(ai.build_money_threshold),
        ai.max_routes
    )];
    let rivals: Vec<_> = state.companies.iter().filter(|c| c.is_ai).collect();
    if rivals.is_empty() {
        lines.push("Sin compañía IA en la partida.".into());
        return lines.join("\n");
    }
    for company in rivals {
        let econ = state.company_economy(company.id);
        lines.push(format!(
            "{} · color {} · {}",
            company.name,
            company.colour,
            format_money(econ.money)
        ));
        if company.name == RIVAL_NAME_TRANSCARGO {
            append_train_routes(&mut lines, state, company.id, ai.max_routes);
        } else if company.name == RIVAL_NAME_ROADHAUL {
            append_bus_routes(&mut lines, state, company.id);
        }
    }
    lines.join("\n")
}

fn append_train_routes(
    lines: &mut Vec<String>,
    state: &GameState,
    ai_id: crate::company::CompanyId,
    max_routes: u8,
) {
    let trains: Vec<_> = state
        .vehicles
        .iter()
        .filter(|v| v.owner == ai_id && v.kind == VehicleKind::Train && v.is_consist_head())
        .collect();
    lines.push(format!(
        "  Rutas / trenes: {} / {}",
        trains.len(),
        max_routes
    ));
    if trains.is_empty() {
        lines.push("  (sin trenes)".into());
    }
    for v in trains {
        lines.push(format_vehicle_route_line(v));
    }
}

fn append_bus_routes(lines: &mut Vec<String>, state: &GameState, ai_id: crate::company::CompanyId) {
    let buses: Vec<_> = state
        .vehicles
        .iter()
        .filter(|v| v.owner == ai_id && v.kind == VehicleKind::Bus)
        .collect();
    lines.push(format!(
        "  Rutas / buses: {} / {}",
        buses.len(),
        roadhaul::ROADHAUL_MAX_ROUTES
    ));
    if buses.is_empty() {
        lines.push("  (sin buses)".into());
    }
    for v in buses {
        lines.push(format_vehicle_route_line(v));
    }
}

fn format_vehicle_route_line(v: &crate::vehicle::Vehicle) -> String {
    let stations: Vec<String> = v
        .orders
        .iter()
        .filter_map(|o| match o {
            VehicleOrder::Station { station, .. } => Some(format!("({},{})", station.x, station.y)),
            _ => None,
        })
        .collect();
    let cargo = v
        .cargo_type
        .map_or_else(|| "?".into(), |c| format!("{c:?}"));
    let route = if stations.is_empty() {
        "sin órdenes".into()
    } else {
        stations.join(" → ")
    };
    format!(
        "  #{} · {} · {} · {}",
        v.id,
        cargo,
        if v.running { "marcha" } else { "parado" },
        route
    )
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::company::{RIVAL_NAME_ROADHAUL, RIVAL_NAME_TRANSCARGO};
    use crate::map::TileCoord;
    use crate::town::Town;

    #[test]
    fn disabled_ai_skips_construction() {
        let mut state = GameState::new(32, 32);
        state.ensure_rival_ais();
        state.ai.enabled = false;
        let before = state.vehicles.len();
        for _ in 0..(TICKS_PER_MONTH + 2) {
            let tick = state.tick.get();
            tick_ai_companies(&mut state, tick);
            state.tick.advance();
        }
        assert_eq!(state.vehicles.len(), before);
    }

    #[test]
    fn debug_status_lists_both_rivals() {
        let mut state = GameState::new(16, 16);
        state.ensure_rival_ais();
        let text = format_ai_debug_status(&state);
        assert!(text.contains(RIVAL_NAME_TRANSCARGO));
        assert!(text.contains(RIVAL_NAME_ROADHAUL));
        assert!(text.contains("ON"));
    }

    #[test]
    fn ensure_rivals_use_distinct_free_colours() {
        let mut state = GameState::new(8, 8);
        apply_set_colour(&mut state, 1);
        state.ensure_rival_ais();
        let tc = state
            .companies
            .iter()
            .find(|c| c.name == RIVAL_NAME_TRANSCARGO)
            .unwrap();
        let rh = state
            .companies
            .iter()
            .find(|c| c.name == RIVAL_NAME_ROADHAUL)
            .unwrap();
        assert_ne!(tc.colour, rh.colour);
        assert_ne!(tc.colour, state.company_colour);
        assert_ne!(rh.colour, state.company_colour);
    }

    fn apply_set_colour(state: &mut GameState, colour: u8) {
        crate::command::apply_command(state, &Command::SetCompanyColour(colour)).unwrap();
    }

    #[test]
    fn roadhaul_builds_one_bus_route_on_month() {
        let mut state = GameState::new(32, 24);
        state.ensure_rival_ais();
        state.towns.push(Town {
            id: 1,
            pos: TileCoord::new(4, 4),
            name: "Norte".into(),
            population: 800,
            ..Town::default()
        });
        state.towns.push(Town {
            id: 2,
            pos: TileCoord::new(20, 16),
            name: "Sur".into(),
            population: 600,
            ..Town::default()
        });
        // Dar liquidez al rival de buses.
        if let Some(c) = state
            .companies
            .iter_mut()
            .find(|c| c.name == RIVAL_NAME_ROADHAUL)
        {
            c.economy.money = 200_000;
        }
        let tick = TICKS_PER_MONTH;
        state.tick = crate::GameTick::new(tick);
        tick_ai_companies(&mut state, tick);
        assert!(
            !state.ai_build_queues.is_empty(),
            "el tick mensual debe encolar la obra, no one-shot"
        );

        // Drenar la cola progresiva hasta el bus.
        let rh_id = state
            .companies
            .iter()
            .find(|c| c.name == RIVAL_NAME_ROADHAUL)
            .unwrap()
            .id;
        for _ in 0..4_000 {
            if state
                .vehicles
                .iter()
                .any(|v| v.owner == rh_id && v.kind == VehicleKind::Bus)
            {
                break;
            }
            let t = state.tick.get();
            tick_ai_companies(&mut state, t);
            state.tick.advance();
        }

        let buses = state
            .vehicles
            .iter()
            .filter(|v| v.owner == rh_id && v.kind == VehicleKind::Bus)
            .count();
        assert_eq!(buses, 1, "RoadHaul debe comprar un bus al cerrar la cola");
        let bus_stops = state
            .stations
            .iter()
            .filter(|s| s.owner == rh_id && s.stop_kind == crate::station::StopKind::BusStop)
            .count();
        assert_eq!(bus_stops, 2, "dos paradas bus");
    }

    #[test]
    fn roadhaul_can_build_second_route_after_first_completes() {
        let mut state = GameState::new(48, 32);
        state.ensure_rival_ais();
        state.towns.push(Town {
            id: 1,
            pos: TileCoord::new(4, 4),
            name: "A".into(),
            population: 900,
            ..Town::default()
        });
        state.towns.push(Town {
            id: 2,
            pos: TileCoord::new(14, 4),
            name: "B".into(),
            population: 800,
            ..Town::default()
        });
        state.towns.push(Town {
            id: 3,
            pos: TileCoord::new(4, 18),
            name: "C".into(),
            population: 700,
            ..Town::default()
        });
        if let Some(c) = state
            .companies
            .iter_mut()
            .find(|c| c.name == RIVAL_NAME_ROADHAUL)
        {
            c.economy.money = 500_000;
        }
        let rh_id = state
            .companies
            .iter()
            .find(|c| c.name == RIVAL_NAME_ROADHAUL)
            .unwrap()
            .id;

        // Dos ciclos mes + drenado de cola → hasta 2 buses (#191).
        for cycle in 0_u64..2 {
            let target_buses = usize::try_from(cycle + 1).unwrap_or(1);
            let tick = TICKS_PER_MONTH.saturating_mul(cycle + 1);
            state.tick = crate::GameTick::new(tick);
            tick_ai_companies(&mut state, tick);
            for _ in 0..4_000 {
                let buses = state
                    .vehicles
                    .iter()
                    .filter(|v| v.owner == rh_id && v.kind == VehicleKind::Bus)
                    .count();
                if buses >= target_buses {
                    break;
                }
                let t = state.tick.get();
                tick_ai_companies(&mut state, t);
                state.tick.advance();
            }
        }

        let buses = state
            .vehicles
            .iter()
            .filter(|v| v.owner == rh_id && v.kind == VehicleKind::Bus)
            .count();
        assert!(
            buses >= 2,
            "RoadHaul debe poder abrir una segunda línea; buses={buses}"
        );
    }

    #[test]
    fn monthly_tick_does_not_enqueue_second_route_while_queue_active() {
        let mut state = GameState::new(32, 24);
        state.ensure_rival_ais();
        state.towns.push(Town {
            id: 1,
            pos: TileCoord::new(4, 4),
            name: "Norte".into(),
            population: 800,
            ..Town::default()
        });
        state.towns.push(Town {
            id: 2,
            pos: TileCoord::new(20, 16),
            name: "Sur".into(),
            population: 600,
            ..Town::default()
        });
        if let Some(c) = state
            .companies
            .iter_mut()
            .find(|c| c.name == RIVAL_NAME_ROADHAUL)
        {
            c.economy.money = 200_000;
        }
        let tick = TICKS_PER_MONTH;
        state.tick = crate::GameTick::new(tick);
        tick_ai_companies(&mut state, tick);
        let queued = state.ai_build_queues.len();
        assert_eq!(queued, 1);
        // Otro mes con cola activa: no segunda obra.
        tick_ai_companies(&mut state, tick.saturating_mul(2));
        assert_eq!(state.ai_build_queues.len(), 1, "una obra por rival");
    }

    #[test]
    fn two_rivals_coexist_after_ensure() {
        let mut state = GameState::new(8, 8);
        state.ensure_rival_ais();
        assert_eq!(state.companies.iter().filter(|c| c.is_ai).count(), 2);
        assert!(
            state
                .companies
                .iter()
                .any(|c| c.name == RIVAL_NAME_TRANSCARGO)
        );
        assert!(
            state
                .companies
                .iter()
                .any(|c| c.name == RIVAL_NAME_ROADHAUL)
        );
    }
}
