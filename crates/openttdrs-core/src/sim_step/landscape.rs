//! `CallLandscapeTick` — orden `OpenTTD`: town → trees → station → industry → companies → linkgraph.

use crate::flow_stat::StationFlows;
use crate::linkgraph_parity::{
    Job, build_jobs_from_cargo_dist, run_full_pipeline, to_station_flows_helper,
};
use crate::{CargoType, GameState, station};

/// Tick de economía en el que se spawnean/unen jobs del linkgraph (`SPAWN_JOIN_TICK`).
pub const LINKGRAPH_SPAWN_JOIN_TICK: u16 = 21;

/// `CallLandscapeTick` (`landscape.cpp:1727-1740`).
pub(super) fn call_landscape_tick(state: &mut GameState, t: u64) {
    on_tick_town(state, t);
    on_tick_trees(state);
    on_tick_water(state);
    on_tick_station(state, t);
    on_tick_industry(state, t);
    on_tick_companies(state, t);
    on_tick_link_graph(state);
}

/// `OnTick_Town`: demanda urbana y crecimiento.
fn on_tick_town(state: &mut GameState, t: u64) {
    super::economy::produce_town_demand(state, t);
    super::economy::grow_towns(state, t);
    // Renovación en visitas del tile loop (P3.6).
    let visit_coords: Vec<_> = state
        .runtime
        .tile_loop_visited
        .iter()
        .map(|(c, _)| *c)
        .collect();
    let dirty = crate::town::tile_loop_town_house_renovation(
        &mut state.map,
        &mut state.towns,
        &visit_coords,
        state.climate,
        state.calendar.year,
        &state.house_spec_catalog,
        &state.house_overrides,
        &mut state.random,
    );
    state.runtime.landscape_tile_dirty.extend(dirty);
}

/// `OnTick_Trees`: ciclo de vegetación sobre las visitas del tile loop.
fn on_tick_trees(state: &mut GameState) {
    crate::map::tree_tile_loop::tick_tree_tile_loop(state);
}

/// Inundación desde agua (`TileLoop_Water` / P3.2) sobre las visitas del tile loop.
fn on_tick_water(state: &mut GameState) {
    crate::map::water_flood::tick_water_flood(state);
}

/// `OnTick_Station`: rating y trigger de aceptación de animación `NewGRF`.
fn on_tick_station(state: &mut GameState, t: u64) {
    if t > 0 && t.is_multiple_of(u64::from(crate::economy::STATION_RATING_TICKS)) {
        station::update_station_ratings_with_cargo_callbacks(
            &mut state.stations,
            &state.cargo_spec_catalog,
            state.order.selectgoods,
            &mut state.random,
        );
    }

    trigger_station_acceptance_animations(state, t);
}

/// Emite CB140 `AcceptanceTick` cada 250 ticks, escalonado por estación.
///
/// `Station::index` en `OpenTTD` es el ID del pool. Los saves importados ya
/// conservan ese identificador en `ottd_station_id`; las estaciones nativas
/// usan su posición estable en el vector como equivalente. El área es
/// `TA_WHOLE`, por eso un solo disparo recorre todas las teselas de la misma
/// estación lógica.
fn trigger_station_acceptance_animations(state: &mut GameState, t: u64) {
    let period = u64::from(crate::economy::STATION_ACCEPTANCE_TICKS);
    let station_anchors: Vec<_> = state
        .stations
        .iter()
        .enumerate()
        .filter_map(|(index, station)| {
            let station_index = station
                .ottd_station_id
                .map_or_else(|| u64::try_from(index).unwrap_or(u64::MAX), u64::from);
            t.wrapping_add(station_index)
                .is_multiple_of(period)
                .then_some(station.pos)
        })
        .collect();

    for station_anchor in station_anchors {
        if let Some(station) = state
            .stations
            .iter_mut()
            .find(|station| station.pos == station_anchor)
        {
            for cargo in crate::ALL_CARGO_TYPES {
                station.goods.get_mut(cargo).clear_newgrf_bigtick();
            }
        }
        let dirty =
            crate::map::trigger_newgrf_station_animation_for_station_with_towns_and_world_and_cargo_catalog(
                &mut state.map,
                t,
                &mut state.stations,
                &state.companies,
                &state.towns,
                &state.industries,
                &state.cargo_spec_catalog,
                state.climate,
                &state.station_spec_catalog,
                &mut state.newgrf_animated_station_tiles,
                station_anchor,
                crate::StationAnimationTrigger::AcceptanceTick,
                None,
            );
        state.runtime.industry_tile_dirty.extend(dirty);
        super::trigger_airport_animation_at(
            state,
            station_anchor,
            crate::AirportAnimationTrigger::AcceptanceTick,
            None,
        );
        super::trigger_road_stop_animation_at(
            state,
            station_anchor,
            crate::StationAnimationTrigger::AcceptanceTick,
            None,
        );
    }
}

/// `OnTick_Industry`: producción y cambio diario de nivel.
fn on_tick_industry(state: &mut GameState, t: u64) {
    super::economy::produce_industries(state, t);
    if state.runtime.calendar_triggers.new_day {
        for industry in &mut state.industries {
            industry.accumulate_accepted_waiting();
        }
        super::economy::maybe_change_industry_production(state);
    }
}

/// `OnTick_Companies`: rivales / `GameScript` (hooks del port).
fn on_tick_companies(state: &mut GameState, t: u64) {
    crate::ai::tick_ai_companies(state, t);
    crate::gs::tick_gs(state);
    crate::subsidy::tick_subsidies(state);
    crate::disaster::tick_disasters(state);
}

/// Integra una lista de jobs ya vencidos en la representación de flows.
///
/// El trabajo se construye sobre una copia del grafo en el tick de spawn y se
/// ejecuta aquí sólo al llegar a su `join_date`, igual que el `JoinNext` de
/// `OpenTTD`. Mantener esta operación aislada también evita que el scheduler
/// síncrono y los comandos que fuerzan una reconstrucción diverjan.
fn station_flows_from_jobs(jobs: Vec<(CargoType, Job)>) -> StationFlows {
    let mut merged = StationFlows::default();
    for (cargo, mut job) in jobs {
        run_full_pipeline(&mut job);
        let part = to_station_flows_helper(&job, cargo);
        for (station_tile, table) in part.by_station {
            let dest = merged.by_station.entry(station_tile).or_default();
            for (c, map) in table.by_cargo {
                let dest_map = dest.by_cargo.entry(c).or_default();
                for (origin, fs) in map.by_origin {
                    for (via, amount) in fs.shares {
                        dest_map.add_flow(origin, via, amount);
                    }
                }
            }
        }
    }
    merged
}

/// `OnTick_LinkGraph` (P2.21) — jobs síncronos sobre copia del grafo cuando
/// `economy_timer.date_fract == 21`, con cadencia nativa de PATS
/// `linkgraph.recalc_interval` (segundos convertidos a días económicos) y
/// latencia nativa de `linkgraph.recalc_time`.
fn on_tick_link_graph(state: &mut GameState) {
    if state.economy_timer.date_fract != LINKGRAPH_SPAWN_JOIN_TICK {
        return;
    }
    let native = state.cargo_dist.openttd_settings();
    // Operar siempre sobre el valor nativo clamped (también para JSON legacy):
    // OpenTTD nunca permite un intervalo menor a cuatro segundos, es decir,
    // dos días económicos en este reloj.
    let interval =
        u32::from(native.recalc_interval_seconds / crate::flow_stat::ECONOMY_SECONDS_PER_DAY)
            .max(1);
    let offset = state.economy_timer.date % interval;
    let date = state.economy_timer.date;

    // OpenTTD: offset==0 → SpawnNext; offset==interval/2 → JoinNext. El
    // segundo sólo integra el primer job cuya fecha de join ya venció.
    if offset == 0 {
        let cargo_dist = state.cargo_dist;
        if !cargo_dist.has_automatic_distribution() {
            state.runtime.pending_linkgraph_jobs.clear();
            state.runtime.station_flows = StationFlows::default();
            return;
        }

        // Copia observacional: el pipeline no muta estaciones ni el grafo en
        // vivo y conserva los ajustes que existían al crear el job.
        let stations = state.stations.clone();
        let link_graph = state.link_graph.clone();
        let cargo_catalog = state.cargo_spec_catalog.clone();
        let (map_w, map_h) = state.map.dimensions();
        let jobs = build_jobs_from_cargo_dist(
            &stations,
            &link_graph,
            cargo_dist,
            &cargo_catalog,
            map_w,
            map_h,
        );
        if !jobs.is_empty() {
            let join_date = date.saturating_add(u32::from(
                native.recalc_time_seconds / crate::flow_stat::ECONOMY_SECONDS_PER_DAY,
            ));
            // El runtime opaco deja de representar el estado una vez que se
            // crea un job nuevo sobre el grafo vivo.
            state.link_graph.runtime_chunks.clear();
            state
                .runtime
                .pending_linkgraph_jobs
                .push(crate::game_state::PendingLinkGraphJob { join_date, jobs });
        }
    }

    if offset == interval / 2 {
        let due = state
            .runtime
            .pending_linkgraph_jobs
            .first()
            .is_some_and(|job| job.join_date <= date);
        if due {
            // Sólo se integra una cabeza por marca, como `JoinNext`; si hay
            // jobs superpuestos, los siguientes esperan la marca posterior.
            let pending = state.runtime.pending_linkgraph_jobs.remove(0);
            state.runtime.station_flows = station_flows_from_jobs(pending.jobs);
            // Tras `JoinNext`, LGRJ/LGRS ya no describen la cola que sigue en
            // Rust; el writer emitirá tablas vacías hasta el próximo spawn.
            state.link_graph.runtime_chunks.clear();
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::cargodist::legacy::flow_stat::DistributionType as GameDistribution;
    use crate::cargodist::parity::{BaseEdge, BaseNode, DistributionType, Job, LinkGraphSettings};
    use crate::map::TileKind;
    use crate::newgrf_sprites::{
        Action2VarAdjust, Action2VarEntry, Action2VarTerm, TrainSpriteAssign, TrainSpriteGraphics,
    };
    use crate::station::{Station, StopKind};
    use crate::{CargoType, STATION_ANIMATION_TRIGGER_ACCEPTANCE_TICK, TileCoord};

    /// CB140 sintético: escribe en el frame el byte bajo de `var 18`.
    fn acceptance_trigger_callbacks() -> TrainSpriteGraphics {
        let mut gfx = TrainSpriteGraphics::default();
        gfx.assigns.push(TrainSpriteAssign {
            local_id: 0,
            set_id: 2,
        });
        gfx.action2_var.insert(
            2,
            Action2VarEntry {
                first: Action2VarTerm {
                    variable: 0x18,
                    param: None,
                    adjust: Action2VarAdjust {
                        shift: 0,
                        and_mask: 0xFF,
                        ..Action2VarAdjust::default()
                    },
                },
                ops: Vec::new(),
                ranges: Vec::new(),
                default: 0,
            },
        );
        gfx
    }

    fn animated_station_state() -> (GameState, TileCoord, TileCoord) {
        let first = TileCoord::new(1, 1);
        let second = TileCoord::new(4, 1);
        let mut state = GameState::new(6, 4);
        for coord in [first, second] {
            let mut tile = state.map.get(coord).unwrap();
            tile.kind = TileKind::Station;
            tile.mapt = 0x50;
            tile.m5 = 0;
            tile.m6 = 0;
            state.map.set_tile(coord, tile).unwrap();
        }
        state.stations = vec![
            Station::new_with_kind(first, StopKind::RailStation),
            Station::new_with_kind(second, StopKind::RailStation),
        ];
        let spec = &mut state.station_spec_catalog[0];
        spec.from_newgrf = true;
        spec.animation_triggers = STATION_ANIMATION_TRIGGER_ACCEPTANCE_TICK;
        spec.newgrf_runtime = Some(Box::new(acceptance_trigger_callbacks()));
        (state, first, second)
    }

    fn animated_road_stop_state() -> (GameState, TileCoord) {
        let pos = TileCoord::new(1, 1);
        let mut state = GameState::new(4, 4);
        let mut tile = state.map.get(pos).unwrap();
        tile.kind = TileKind::Station;
        tile.mapt = 0x50;
        tile.m5 = crate::RSV_DRIVE_THROUGH_X;
        tile.m6 = 2;
        state.map.set_tile(pos, tile).unwrap();
        let mut station = Station::new_with_kind(pos, StopKind::BusStop);
        station.road_stop_spec = Some(7);
        state.stations.push(station);
        state.road_stop_spec_catalog.push(crate::RoadStopSpecDef {
            id: 7,
            class: 0,
            label: "RoadStop animado".into(),
            short_label: "RSAN".into(),
            stop_type: crate::ROADSTOP_TYPE_BUS,
            from_newgrf: true,
            grfid: 0x5253_414E,
            newgrf_local_id: 0,
            newgrf_grf_version: 0,
            draw_mode: crate::ROADSTOP_DRAW_MODE_DEFAULT,
            random_cargo_triggers: 0,
            flags: 0,
            build_cost_multiplier: 16,
            clear_cost_multiplier: 16,
            bridgeable_info: [crate::road_stop_spec::RoadStopBridgeableInfo::default();
                crate::road_stop_spec::ROADSTOP_LAYOUT_COUNT],
            callback_mask: 0,
            animation_status: 1,
            animation_frames: u8::MAX,
            animation_speed: 0,
            animation_triggers: crate::ROADSTOP_ANIMATION_TRIGGER_ACCEPTANCE_TICK,
            newgrf_views: Vec::new(),
            newgrf_runtime: Some(Box::new(acceptance_trigger_callbacks())),
            newgrf_type_tables: None,
            associated_badges: Vec::new(),
            newgrf_badge_translation: Vec::new(),
        });
        (state, pos)
    }

    #[test]
    fn acceptance_animation_uses_250_ticks_and_staggers_native_stations() {
        let (mut state, first, second) = animated_station_state();

        on_tick_station(&mut state, 248);
        assert_eq!(state.map.get(first).unwrap().m7, 0);
        assert_eq!(state.map.get(second).unwrap().m7, 0);

        // El índice 1 se dispara un tick antes que el índice 0.
        on_tick_station(&mut state, 249);
        assert_eq!(state.map.get(first).unwrap().m7, 0);
        assert_eq!(state.map.get(second).unwrap().m7, 6);

        on_tick_station(&mut state, 250);
        assert_eq!(state.map.get(first).unwrap().m7, 6);
        assert_eq!(state.map.get(second).unwrap().m7, 6);
    }

    #[test]
    fn acceptance_animation_uses_imported_station_id_for_phase() {
        let (mut state, first, second) = animated_station_state();
        state.stations[0].ottd_station_id = Some(7);
        state.stations[1].ottd_station_id = Some(8);

        on_tick_station(&mut state, 242);
        assert_eq!(state.map.get(first).unwrap().m7, 0);
        assert_eq!(state.map.get(second).unwrap().m7, 6);

        on_tick_station(&mut state, 243);
        assert_eq!(state.map.get(first).unwrap().m7, 6);
    }

    #[test]
    fn acceptance_animation_reaches_newgrf_road_stops() {
        let (mut state, _pos) = animated_road_stop_state();

        on_tick_station(&mut state, 249);
        assert_eq!(state.stations[0].road_stop_animation_frame, 0);

        on_tick_station(&mut state, 250);
        assert_eq!(state.stations[0].road_stop_animation_frame, 6);
    }

    fn linkgraph_test_state() -> GameState {
        let mut state = GameState::new(8, 8);
        let source = TileCoord::new(1, 1);
        let destination = TileCoord::new(5, 5);
        state
            .stations
            .push(Station::new_with_kind(source, StopKind::RailStation));
        state
            .stations
            .push(Station::new_with_kind(destination, StopKind::RailStation));
        state
            .link_graph
            .record_trip(source, destination, CargoType::Coal, 10, 20, 4);
        state.cargo_dist.distribution = GameDistribution::Asymmetric;
        state.economy_timer.date_fract = LINKGRAPH_SPAWN_JOIN_TICK;
        state
    }

    #[test]
    fn linkgraph_spawn_keeps_snapshot_until_recalc_time_join_date() {
        let mut state = linkgraph_test_state();
        state.cargo_dist.per_cargo = Some(crate::flow_stat::CargoDistPerCargoSettings {
            recalc_interval_seconds: 8,
            recalc_time_seconds: 9,
            distribution_default: GameDistribution::Asymmetric,
            ..Default::default()
        });

        // Día 0, offset 0: se crea el job, pero OpenTTD aún no integra sus
        // flows. La fecha nativa es 9 / 2 = 4 días económicos.
        on_tick_link_graph(&mut state);
        assert_eq!(state.runtime.pending_linkgraph_jobs.len(), 1);
        assert_eq!(state.runtime.pending_linkgraph_jobs[0].join_date, 4);
        assert!(state.runtime.station_flows.by_station.is_empty());

        // Día 2: es la primera marca de JoinNext, pero el job todavía no venció.
        state.economy_timer.date = 2;
        on_tick_link_graph(&mut state);
        assert_eq!(state.runtime.pending_linkgraph_jobs.len(), 1);
        assert!(state.runtime.station_flows.by_station.is_empty());

        // Día 6: primera marca de join posterior a join_date; recién aquí se
        // publican los shares calculados sobre la copia del grafo.
        state.economy_timer.date = 6;
        on_tick_link_graph(&mut state);
        assert!(state.runtime.pending_linkgraph_jobs.is_empty());
        assert!(!state.runtime.station_flows.by_station.is_empty());
    }

    #[test]
    fn linkgraph_pending_job_preserves_graph_snapshot() {
        let mut state = linkgraph_test_state();
        state.cargo_dist.per_cargo = Some(crate::flow_stat::CargoDistPerCargoSettings {
            recalc_interval_seconds: 8,
            recalc_time_seconds: 9,
            distribution_default: GameDistribution::Asymmetric,
            ..Default::default()
        });
        on_tick_link_graph(&mut state);
        assert_eq!(state.runtime.pending_linkgraph_jobs.len(), 1);

        // Cambiar el grafo después del spawn no altera el snapshot ya iniciado.
        let extra = TileCoord::new(7, 7);
        state
            .link_graph
            .record_trip(TileCoord::new(1, 1), extra, CargoType::Coal, 99, 99, 1);
        state.economy_timer.date = 2;
        on_tick_link_graph(&mut state);

        let pending = state.runtime.pending_linkgraph_jobs.first();
        assert_eq!(pending.map(|job| job.jobs.len()), Some(1));
        let Some(pending) = pending else {
            panic!("job pending");
        };
        let (cargo, job) = &pending.jobs[0];
        assert_eq!(*cargo, CargoType::Coal);
        assert_eq!(job.nodes.len(), 2);
        assert!(
            job.edges
                .iter()
                .flat_map(|edges| edges.iter())
                .all(|edge| edge.dest < 2)
        );
    }

    #[test]
    fn linkgraph_test_job_has_stable_join_pipeline() {
        let settings = LinkGraphSettings {
            distribution: DistributionType::Asymmetric,
            ..Default::default()
        };
        let job = Job::new(
            vec![
                BaseNode {
                    station: 0,
                    x: 1,
                    y: 1,
                    supply: 10,
                    demand: 0,
                },
                BaseNode {
                    station: 1,
                    x: 5,
                    y: 5,
                    supply: 0,
                    demand: 10,
                },
            ],
            vec![
                vec![BaseEdge {
                    dest: 1,
                    capacity: 20,
                    usage: 0,
                    travel_time: 4,
                }],
                Vec::new(),
            ],
            settings,
        );
        let flows = station_flows_from_jobs(vec![(CargoType::Coal, job)]);
        assert!(!flows.by_station.is_empty());
    }
}
