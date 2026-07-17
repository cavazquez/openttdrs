//! Tests del catálogo de escenarios.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use super::*;
use crate::cargo::CargoType;
use crate::industry::IndustryKind;
use crate::map::TileCoord;
use crate::rail_signals::{SIGTYPE_BLOCK, SIGTYPE_ENTRY, SIGTYPE_EXIT};
use crate::station::road_stop_approach_tile;
use crate::vehicle::{Vehicle, VehicleKind, VehicleOrder};
use crate::{PathNetwork, find_path};

#[test]
fn export_junction_json_roundtrips_rail_signals_mixed() {
    let path = std::env::temp_dir().join(format!("openttdrs_junction_{}.json", std::process::id()));
    export_junction_json("rail_signals_mixed", &path).expect("export");
    let loaded = crate::save::load(&path).expect("load");
    let _ = std::fs::remove_file(&path);
    let original = build_rail_signals_mixed();
    assert_eq!(loaded.map.dimensions(), original.map.dimensions());
    assert_eq!(loaded.vehicles.len(), original.vehicles.len());
    assert_eq!(loaded.stations.len(), original.stations.len());
    assert!(!loaded.vehicles.is_empty());
}

#[test]
fn truck_bay_layout_is_consistent() {
    let state = build_truck_bay();
    assert_eq!(
        road_stop_approach_tile(&state.map, TRUCK_BAY_LOAD_STOP),
        Some(TRUCK_BAY_LOAD_ROAD)
    );
    assert_eq!(
        road_stop_approach_tile(&state.map, TRUCK_BAY_DELIVER_STOP),
        Some(TRUCK_BAY_DELIVER_ROAD)
    );
    assert_eq!(state.vehicles.len(), 1);
    assert_eq!(state.stations.len(), 2);
    // La ruta calculada existe y contiene las dos esquinas de la L.
    let path = find_path(
        &state.map,
        TRUCK_BAY_LOAD_ROAD,
        TRUCK_BAY_DELIVER_ROAD,
        PathNetwork::Road,
    )
    .expect("ruta por carretera");
    assert!(path.contains(&TileCoord::new(10, 6)));
    assert!(path.contains(&TileCoord::new(10, 12)));
}

#[test]
fn unknown_scenario_returns_none() {
    assert!(build_scenario("nope").is_none());
    assert!(build_scenario("truck_bay").is_some());
    assert!(build_scenario("train_line").is_some());
    assert!(build_scenario("train_supply").is_some());
    assert!(build_scenario("train_supply_signal").is_some());
    assert!(build_scenario("train_signal").is_some());
    assert!(build_scenario("train_pbs").is_some());
    assert!(build_scenario("ai_rival_line").is_some());
    assert!(build_scenario("train_supply_dual").is_some());
    assert!(build_scenario("rail_signals_mixed").is_some());
    assert_eq!(
        scenario_names(),
        &[
            "truck_bay",
            "train_line",
            "train_supply",
            "train_supply_dual",
            "train_supply_signal",
            "train_signal",
            "train_pbs",
            "ai_rival_line",
            "rail_signals_mixed",
            "loan_interest",
            "town_growth",
            "breakdown",
        ]
    );
}

#[test]
fn ai_rival_builds_line_after_monthly_tick() {
    use crate::economy::TICKS_PER_MONTH;
    let mut state = build_ai_rival_line();
    assert!(state.companies.iter().any(|c| c.is_ai));
    // Avanzar hasta el primer tick mensual (IA corre en múltiplos de TICKS_PER_MONTH).
    for _ in 0..=TICKS_PER_MONTH {
        state.step();
    }
    let ai_id = state.companies.iter().find(|c| c.is_ai).unwrap().id;
    assert!(
        state.stations.iter().any(|s| s.owner == ai_id),
        "TransCargo debe tener estaciones"
    );
    assert!(
        state.vehicles.iter().any(|v| v.owner == ai_id),
        "TransCargo debe tener tren"
    );
}

#[test]
fn ai_rival_builds_second_wood_route_on_l() {
    use crate::economy::TICKS_PER_MONTH;
    use crate::vehicle::VehicleKind;

    let mut state = build_ai_rival_line();
    // Dos cierres mensuales: 1ª carbón, 2ª madera (L).
    for _ in 0..=(TICKS_PER_MONTH * 2) {
        state.step();
    }
    let ai_id = state.companies.iter().find(|c| c.is_ai).unwrap().id;
    let trains = state
        .vehicles
        .iter()
        .filter(|v| v.owner == ai_id && v.kind == VehicleKind::Train && v.is_consist_head())
        .count();
    assert!(
        trains >= 2,
        "TransCargo debe tener 2 trenes (carbón + madera), got {trains}"
    );
    // La 2ª ruta ancla carga al bosque (estación a ±2 de Forest).
    let wood_load = state.stations.iter().filter(|s| s.owner == ai_id).any(|s| {
        state.industries.iter().any(|i| {
            i.kind == IndustryKind::Forest
                && (i.pos.x - s.pos.x).abs() <= 2
                && (i.pos.y - s.pos.y).abs() <= 2
        })
    });
    assert!(wood_load, "debe existir estación IA junto al bosque");
    // Tras unos ticks el tren de madera debe cargar Wood (industria más cercana).
    let mut saw_wood = false;
    for _ in 0..4_000 {
        state.step();
        if state
            .vehicles
            .iter()
            .any(|v| v.owner == ai_id && v.cargo > 0 && v.cargo_type == Some(CargoType::Wood))
        {
            saw_wood = true;
            break;
        }
    }
    assert!(saw_wood, "el tren de la ruta bosque debe cargar madera");
    assert!(
        state.stations.iter().filter(|s| s.owner == ai_id).count() >= 3,
        "al menos 3 estaciones IA (2 carga + descarga)"
    );
}

#[test]
fn ai_rival_builds_third_oil_route() {
    use crate::economy::TICKS_PER_MONTH;
    use crate::vehicle::VehicleKind;

    let mut state = build_ai_rival_line();
    assert_eq!(state.ai.max_routes, 3);
    // Tres cierres mensuales: carbón, madera, petróleo.
    for _ in 0..=(TICKS_PER_MONTH * 3) {
        state.step();
    }
    let ai_id = state.companies.iter().find(|c| c.is_ai).unwrap().id;
    let trains = state
        .vehicles
        .iter()
        .filter(|v| v.owner == ai_id && v.kind == VehicleKind::Train && v.is_consist_head())
        .count();
    assert!(
        trains >= 3,
        "TransCargo debe tener 3 trenes (carbón + madera + petróleo), got {trains}"
    );
    let oil_load = state.stations.iter().filter(|s| s.owner == ai_id).any(|s| {
        state.industries.iter().any(|i| {
            i.kind == IndustryKind::OilWell
                && (i.pos.x - s.pos.x).abs() <= 2
                && (i.pos.y - s.pos.y).abs() <= 2
        })
    });
    assert!(oil_load, "debe existir estación IA junto al pozo");
    let mut saw_oil = false;
    for _ in 0..12_000 {
        state.step();
        if state
            .vehicles
            .iter()
            .any(|v| v.owner == ai_id && v.cargo > 0 && v.cargo_type == Some(CargoType::Oil))
        {
            saw_oil = true;
            break;
        }
    }
    assert!(saw_oil, "el tren de la ruta petróleo debe cargar oil");
}

#[test]
fn ai_rival_flattens_terrain_and_places_block_signals() {
    use crate::TileKind;
    use crate::command::{Command, apply_command};
    use crate::economy::TICKS_PER_MONTH;
    use crate::rail_signals::rail_tile_is_signals;
    use crate::vehicle::VehicleKind;

    let mut state = build_ai_rival_line();
    // Baches en el corredor carbón (y=5) para forzar LevelLand de la IA.
    state.economy.money = 500_000;
    for x in [8, 9, 10, 11] {
        for _ in 0..2 {
            apply_command(&mut state, &Command::RaiseLand(TileCoord::new(x, 5))).ok();
        }
    }
    for _ in 0..=TICKS_PER_MONTH {
        state.step();
    }
    let ai_id = state.companies.iter().find(|c| c.is_ai).unwrap().id;
    assert!(
        state
            .vehicles
            .iter()
            .any(|v| v.owner == ai_id && v.kind == VehicleKind::Train),
        "TransCargo debe construir pese al terreno irregular"
    );
    let (mw, mh) = state.map.dimensions();
    let mut signals = 0usize;
    for y in 0..mh.cast_signed() {
        for x in 0..mw.cast_signed() {
            let c = TileCoord::new(x, y);
            if let Some(t) = state.map.get(c)
                && t.kind == TileKind::Rail
                && rail_tile_is_signals(t.m5)
                && t.m1 == ai_id.0
            {
                signals += 1;
            }
        }
    }
    assert!(
        signals >= 1,
        "TransCargo debe colocar al menos una señal de bloque, got {signals}"
    );
}

#[test]
fn ai_rival_delivers_coal_and_awards_subsidy() {
    use crate::cargo::CargoType;
    use crate::economy::TICKS_PER_MONTH;

    let mut state = build_ai_rival_line();
    for _ in 0..=TICKS_PER_MONTH {
        state.step();
    }
    let ai_id = state.companies.iter().find(|c| c.is_ai).unwrap().id;
    assert!(state.vehicles.iter().any(|v| v.owner == ai_id), "tren IA");
    assert!(
        state
            .subsidies
            .iter()
            .any(|s| s.cargo == CargoType::Coal && !s.awarded),
        "oferta de subsidio carbón"
    );

    let income_before = state.companies[ai_id.index()].cargo_income_earned;
    let mut earned = false;
    let mut awarded = false;
    for _ in 0..12_000 {
        state.step();
        if state
            .subsidies
            .iter()
            .any(|s| s.awarded && s.awarded_company == Some(ai_id))
        {
            awarded = true;
        }
        if state.companies[ai_id.index()].cargo_income_earned > income_before {
            earned = true;
        }
        if earned && awarded {
            break;
        }
    }
    assert!(
        earned,
        "TransCargo debe registrar ingreso por entrega de carbón"
    );
    assert!(awarded, "TransCargo debe adjudicar el subsidio al entregar");
}

#[test]
fn feeder_share_credits_first_station_owner() {
    use crate::company::{CompanyId, feeder_share_of};
    use crate::station::StopKind;

    let mut state = GameState::new(12, 8);
    state.ensure_companies();
    state.ensure_rival_transcargo();
    let ai = state.companies.iter().find(|c| c.is_ai).unwrap().id;
    let hub = TileCoord::new(3, 3);
    let mut hub_st = crate::Station::new_with_kind(hub, StopKind::RailStation);
    hub_st.owner = ai;
    state.stations = vec![hub_st];

    let payment = 100_i64;
    let share = feeder_share_of(payment);
    assert_eq!(share, 75);
    let ai_before = state.company_economy(ai).money;
    let player_before = state.economy.money;
    state.credit_company(ai, share);
    state.credit_company(CompanyId::PLAYER, payment - share);
    assert_eq!(state.company_economy(ai).money, ai_before + 75);
    assert_eq!(state.economy.money, player_before + 25);
    assert_eq!(
        state.companies[CompanyId::PLAYER.index()].economy.money,
        player_before + 25
    );
}

/// Hub IA → destino jugador: al descargar se paga 75 % al feeder, se marca
/// `feeder_paid` y se reinserta el packet con `first_station` intacto.
#[test]
fn feeder_share_paid_on_unload_preserves_packet_flags() {
    use crate::cargo::CargoType;
    use crate::cargo_packet::CargoPacket;
    use crate::company::CompanyId;
    use crate::station::StopKind;

    let mut state = GameState::new(16, 10);
    state.ensure_companies();
    state.ensure_rival_transcargo();
    let ai = state.companies.iter().find(|c| c.is_ai).unwrap().id;
    let hub = TileCoord::new(2, 2);
    let dest = TileCoord::new(10, 5);
    let mut hub_st = crate::Station::new_with_kind(hub, StopKind::TruckStop);
    hub_st.owner = ai;
    let mut dest_st = crate::Station::new_with_kind(dest, StopKind::TruckStop);
    dest_st.owner = CompanyId::PLAYER;
    state.stations = vec![hub_st, dest_st];

    let mut truck = Vehicle::new(90, VehicleKind::Truck, dest, dest);
    truck.set_vehicle_orders(vec![VehicleOrder::station(dest)]);
    truck.sync_order_destination(&state.map);
    let mut packet = CargoPacket::new(CargoType::Coal, 8, TileCoord::new(1, 1));
    packet.first_station = Some(hub);
    packet.feeder_paid = false;
    truck.cargo_packets.push(packet);
    truck.sync_cargo_from_packets();
    truck.cargo_source = Some(hub);
    truck.last_pickup_station = Some(hub);
    state.vehicles.push(truck);

    let ai_before = state.company_economy(ai).money;
    let player_before = state.company_economy(CompanyId::PLAYER).money;
    for _ in 0..8 {
        state.step();
        if state.vehicles[0].cargo == 0 {
            break;
        }
    }

    assert_eq!(state.vehicles[0].cargo, 0, "debe descargar en destino");
    let waiting = state.stations[1].cargo_stock.get(CargoType::Coal);
    assert_eq!(waiting, 8, "freight queda en cola del hub de destino");
    let reinserted = state.stations[1]
        .cargo_packets
        .packets
        .iter()
        .find(|p| p.cargo == CargoType::Coal)
        .expect("packet reinsertado");
    assert_eq!(reinserted.first_station, Some(hub));
    assert!(reinserted.next_hop.is_none(), "trasbordo limpia next_hop");
    assert!(reinserted.feeder_paid, "feeder liquidado una sola vez");
    assert!(
        state.company_economy(ai).money > ai_before,
        "IA feeder debe cobrar su 75 %"
    );
    assert!(reinserted.feeder_share > 0, "packet acumula feeder_share");
    assert!(
        state.company_economy(CompanyId::PLAYER).money > player_before,
        "jugador cobra el resto del ingreso"
    );
}

#[test]
fn cargo_next_hop_keeps_packet_until_destination_station() {
    use crate::cargo::CargoType;
    use crate::cargo_packet::CargoPacket;
    use crate::station::StopKind;

    let mut state = GameState::new(16, 10);
    let via = TileCoord::new(4, 4);
    let dest = TileCoord::new(10, 5);
    state.stations = vec![
        crate::Station::new_with_kind(via, StopKind::TruckStop),
        crate::Station::new_with_kind(dest, StopKind::TruckStop),
    ];

    let mut truck = Vehicle::new(91, VehicleKind::Truck, via, dest);
    truck.set_vehicle_orders(vec![
        VehicleOrder::station(via),
        VehicleOrder::station(dest),
    ]);
    truck.current_order = 0;
    truck.sync_order_destination(&state.map);
    let mut packet = CargoPacket::new(CargoType::Goods, 6, TileCoord::new(0, 0));
    packet.next_hop = Some(dest);
    truck.cargo_packets.push(packet);
    truck.sync_cargo_from_packets();
    truck.last_pickup_station = Some(TileCoord::new(0, 0));
    state.vehicles.push(truck);

    for _ in 0..16 {
        state.step();
    }
    assert_eq!(
        state.vehicles[0].cargo, 6,
        "con next_hop=dest no debe descargar en via"
    );
    assert!(
        state.stations[0].cargo_stock.get(CargoType::Goods) == 0,
        "via no recibe trasbordo prematuro"
    );

    state.vehicles[0].pos = dest;
    state.vehicles[0].current_order = 1;
    state.vehicles[0].sync_order_destination(&state.map);
    for _ in 0..16 {
        state.step();
        if state.vehicles[0].cargo == 0 {
            break;
        }
    }
    assert_eq!(state.vehicles[0].cargo, 0, "en dest sí descarga");
    assert_eq!(
        state.stations[1].cargo_stock.get(CargoType::Goods),
        6,
        "goods en cola del destino"
    );
}

#[test]
fn cargo_asymmetric_next_hop_from_flow_stat() {
    use crate::cargo::CargoType;
    use crate::cargo_packet::CargoPacket;
    use crate::flow_stat::DistributionType;
    use crate::station::StopKind;

    let mut state = GameState::new(16, 10);
    let via = TileCoord::new(4, 4);
    let order_dest = TileCoord::new(10, 5);
    let flow_dest = TileCoord::new(12, 6);
    state.stations = vec![
        crate::Station::new_with_kind(via, StopKind::TruckStop),
        crate::Station::new_with_kind(order_dest, StopKind::TruckStop),
        crate::Station::new_with_kind(flow_dest, StopKind::TruckStop),
    ];
    state
        .link_graph
        .record_flow(via, flow_dest, CargoType::Goods, 50);
    state.cargo_dist.distribution = DistributionType::Asymmetric;
    state.rebuild_station_flows();

    let mut truck = Vehicle::new(92, VehicleKind::Truck, via, order_dest);
    truck.set_vehicle_orders(vec![
        VehicleOrder::station(via),
        VehicleOrder::station(order_dest),
    ]);
    truck.current_order = 0;
    truck.sync_order_destination(&state.map);
    state.vehicles.push(truck);

    let mut waiting = CargoPacket::new(CargoType::Goods, 4, via);
    waiting.first_station = Some(via);
    state.stations[0].cargo_packets.push(waiting);
    state.stations[0].sync_stock_from_packets();

    for _ in 0..8 {
        state.step();
        if state.vehicles[0].cargo > 0 {
            break;
        }
    }
    assert!(state.vehicles[0].cargo > 0, "debe cargar goods");
    let hop = state.vehicles[0]
        .cargo_packets
        .packets
        .first()
        .and_then(|p| p.next_hop);
    assert_eq!(
        hop,
        Some(flow_dest),
        "Asymmetric usa FlowStat (no order_dest)"
    );
}

#[test]
fn load_from_station_sets_first_station_when_missing() {
    use crate::cargo::CargoType;
    use crate::cargo_packet::CargoPacket;
    use crate::station::StopKind;

    let mut state = GameState::new(12, 8);
    let stop = TileCoord::new(4, 4);
    let mut st = crate::Station::new_with_kind(stop, StopKind::TruckStop);
    st.cargo_packets
        .push(CargoPacket::new(CargoType::Goods, 6, stop));
    st.sync_stock_from_packets();
    assert!(
        st.cargo_packets
            .packets
            .iter()
            .all(|p| p.first_station.is_none())
    );
    state.stations = vec![st];

    let mut truck = Vehicle::new(91, VehicleKind::Truck, stop, stop);
    truck.set_vehicle_orders(vec![VehicleOrder::station_with_flags(stop, true, false)]);
    truck.sync_order_destination(&state.map);
    state.vehicles.push(truck);

    for _ in 0..8 {
        state.step();
        if state.vehicles[0].cargo > 0 {
            break;
        }
    }
    assert!(state.vehicles[0].cargo > 0, "debe cargar desde la cola");
    assert!(
        state.vehicles[0]
            .cargo_packets
            .packets
            .iter()
            .all(|p| p.first_station == Some(stop)),
        "first_station anclado al embarque"
    );
}

#[test]
fn train_pbs_both_corridors_reserve_without_overlap() {
    let mut state = build_train_pbs();
    state.step();
    let n = state
        .vehicles
        .iter()
        .find(|v| v.id == TRAIN_PBS_NORTH_ID)
        .expect("norte");
    let s = state
        .vehicles
        .iter()
        .find(|v| v.id == TRAIN_PBS_SOUTH_ID)
        .expect("sur");
    assert!(
        n.reserved_steps.len() >= 3,
        "norte reserva: {:?}",
        n.reserved_steps
    );
    assert!(
        s.reserved_steps.len() >= 3,
        "sur reserva: {:?}",
        s.reserved_steps
    );
    assert!(
        n.reserved_steps
            .iter()
            .all(|r| r.tile.y == TRAIN_PBS_NORTH_Y)
            && s.reserved_steps
                .iter()
                .all(|r| r.tile.y == TRAIN_PBS_SOUTH_Y),
        "reservas disjuntas por corredor"
    );
    assert!(
        crate::rail_pbs::reservation_ends_at_safe_wait(&state.map, n),
        "norte hasta safe wait"
    );
}

#[test]
fn train_signal_layout_is_consistent() {
    let state = build_train_signal();
    let signal_tile = state.map.get(TRAIN_SIGNAL_TILE).unwrap();
    assert!(crate::rail_signals::rail_tile_is_signals(signal_tile.m5));
    assert_eq!(state.vehicles.len(), 2);
    assert_eq!(state.vehicles[0].id, TRAIN_SIGNAL_LEAD_ID);
    assert_eq!(state.vehicles[0].pos, TRAIN_SIGNAL_TILE);
    assert_eq!(state.vehicles[1].id, TRAIN_SIGNAL_BLOCKER_ID);
    assert_eq!(state.vehicles[1].pos, TRAIN_SIGNAL_BLOCK_TILE);
    assert!(!state.vehicles[1].running);
}

#[test]
fn train_line_layout_is_consistent() {
    use crate::station::{rail_station_approach_tile, rail_station_stop_tile};

    let state = build_train_line();
    assert_eq!(
        rail_station_approach_tile(&state.map, TRAIN_LINE_STATION_A),
        Some(TileCoord::new(2, 6)),
        "acceso a la estación A"
    );
    assert_eq!(
        rail_station_approach_tile(&state.map, TRAIN_LINE_STATION_B),
        Some(TileCoord::new(12, 9)),
        "acceso a la estación B (boca al norte)"
    );
    assert_eq!(state.vehicles.len(), 1);
    assert_eq!(state.stations.len(), 2);
    assert_eq!(state.vehicles[0].kind, VehicleKind::Train);
    assert!(
        !state.vehicles[0].path.is_empty(),
        "el tren arranca con ruta desde el depósito"
    );
    assert_eq!(
        rail_station_stop_tile(&state.map, TRAIN_LINE_STATION_A),
        Some(TRAIN_LINE_STATION_A),
        "destino de parada A = plataforma"
    );
    assert_eq!(
        rail_station_stop_tile(&state.map, TRAIN_LINE_STATION_B),
        Some(TRAIN_LINE_STATION_B),
        "destino de parada B = plataforma"
    );
    // La línea conecta depósito → plataforma A → esquina → plataforma B.
    let a_to_b = find_path(
        &state.map,
        TRAIN_LINE_STATION_A,
        TRAIN_LINE_STATION_B,
        PathNetwork::Rail,
    )
    .expect("ruta ferroviaria A → B");
    assert!(a_to_b.contains(&TRAIN_LINE_SIGNAL), "pasa por la señal");
    assert!(a_to_b.contains(&TRAIN_LINE_CORNER), "pasa por la esquina");
    assert!(
        find_path(
            &state.map,
            TRAIN_LINE_DEPOT,
            TRAIN_LINE_STATION_A,
            PathNetwork::Rail
        )
        .is_some(),
        "el depósito conecta con la estación A"
    );
    // La señal quedó colocada sobre la recta.
    let signal_tile = state.map.get(TRAIN_LINE_SIGNAL).unwrap();
    assert!(crate::rail_signals::rail_tile_is_signals(signal_tile.m5));
}

#[test]
fn train_supply_has_mine_factory_and_signals() {
    use crate::station;

    let state = build_train_supply();
    assert_eq!(state.vehicles.len(), 1);
    assert_eq!(state.industries.len(), 2);
    assert_eq!(state.industries[0].kind, IndustryKind::CoalMine);
    assert_eq!(state.industries[1].kind, IndustryKind::Factory);
    assert!(
        station::industry_in_station_coverage(
            &state.industries[0],
            TRAIN_LINE_STATION_A,
            station::STATION_COVERAGE_RADIUS,
        ),
        "mina en cobertura de estación A"
    );
    assert!(
        station::industry_in_station_coverage(
            &state.industries[1],
            TRAIN_LINE_STATION_B,
            station::STATION_COVERAGE_RADIUS,
        ),
        "fábrica en cobertura de estación B"
    );
    for &signal in &[
        TRAIN_SUPPLY_SIGNAL_WEST,
        TRAIN_LINE_SIGNAL,
        TRAIN_SUPPLY_SIGNAL_EAST,
        TRAIN_SUPPLY_SIGNAL_SOUTH,
    ] {
        let tile = state.map.get(signal).unwrap();
        assert!(
            crate::rail_signals::rail_tile_is_signals(tile.m5),
            "señal en {signal:?}"
        );
    }
}

#[test]
fn train_supply_signal_snapshot_has_blocker_on_signal() {
    let state = build_train_supply_signal_snapshot();
    assert_eq!(state.vehicles.len(), 2);
    assert_eq!(state.vehicles[0].pos, TRAIN_SUPPLY_WAIT_SIGNAL);
    assert_eq!(state.vehicles[0].cargo, 20);
    assert_eq!(state.vehicles[1].id, TRAIN_SUPPLY_BLOCKER_ID);
    assert_eq!(state.vehicles[1].pos, TRAIN_SUPPLY_BLOCK_TILE);
}

#[test]
#[allow(clippy::too_many_lines)]
fn train_supply_dual_has_two_tracks_signals_and_paths() {
    use crate::map::TileKind;
    use crate::station::{self, rail_station_stop_tile};

    let state = build_train_supply_dual();
    assert_eq!(state.vehicles.len(), 2);
    assert_eq!(state.stations.len(), 2);
    assert_eq!(
        state.map.get_kind(TRAIN_DUAL_DEPOT),
        Some(TileKind::RailDepot)
    );
    assert!(
        state
            .map
            .get_kind(TRAIN_DUAL_COAL_MINE)
            .is_some_and(|k| k == TileKind::Industry),
        "mina visible en el mapa"
    );
    assert!(
        state
            .map
            .get_kind(TRAIN_DUAL_FACTORY)
            .is_some_and(|k| k == TileKind::Industry),
        "fábrica visible en el mapa"
    );
    assert!(
        state
            .vehicles
            .iter()
            .find(|v| v.id == TRAIN_DUAL_VEHICLE_ID)
            .is_some_and(|v| v.pos == TRAIN_DUAL_DEPOT),
        "tren 1 arranca en el depósito"
    );
    assert!(
        state
            .vehicles
            .iter()
            .find(|v| v.id == TRAIN_DUAL_VEHICLE_2_ID)
            .is_some_and(|v| v.pos == TRAIN_DUAL_DEPOT && !v.running),
        "tren 2 espera en el depósito hasta que el 1 libere la salida"
    );
    let mine = state
        .industries
        .iter()
        .find(|i| i.pos == TRAIN_DUAL_COAL_MINE)
        .expect("mina");
    let factory = state
        .industries
        .iter()
        .find(|i| i.pos == TRAIN_DUAL_FACTORY)
        .expect("fábrica");
    assert!(
        station::industry_in_station_coverage(
            mine,
            TRAIN_DUAL_STATION_A,
            station::STATION_COVERAGE_RADIUS,
        ),
        "mina en cobertura de estación A"
    );
    assert!(
        station::industry_in_station_coverage(
            factory,
            TRAIN_DUAL_STATION_B,
            station::STATION_COVERAGE_RADIUS,
        ),
        "fábrica en cobertura de estación B"
    );
    for &y in &[TRAIN_DUAL_TRACK_OUT_Y, TRAIN_DUAL_TRACK_RET_Y] {
        for &x in &[5, 7, 9] {
            let tile = state.map.get(TileCoord::new(x, y)).unwrap();
            assert!(
                crate::rail_signals::rail_tile_is_signals(tile.m5),
                "señal en ({x},{y})"
            );
            assert_eq!(
                crate::rail_signals::rail_signal_present_mask(tile.m3).count_ones(),
                1,
                "una señal unidireccional en ({x},{y})"
            );
        }
    }
    let out_mask = crate::rail_signals::rail_signal_present_mask(
        state
            .map
            .get(TileCoord::new(7, TRAIN_DUAL_TRACK_OUT_Y))
            .unwrap()
            .m3,
    );
    let ret_mask = crate::rail_signals::rail_signal_present_mask(
        state
            .map
            .get(TileCoord::new(7, TRAIN_DUAL_TRACK_RET_Y))
            .unwrap()
            .m3,
    );
    assert_eq!(out_mask, 0b0100, "señales ida miran hacia +x");
    assert_eq!(ret_mask, 0b1000, "señales vuelta miran hacia -x");
    assert!(
        find_path(
            &state.map,
            TRAIN_DUAL_DEPOT,
            TRAIN_DUAL_STATION_A,
            PathNetwork::Rail,
        )
        .is_some(),
        "ruta depósito → A"
    );
    assert!(
        find_path(
            &state.map,
            TRAIN_DUAL_STATION_A,
            TRAIN_DUAL_STATION_B,
            PathNetwork::Rail,
        )
        .is_some(),
        "ruta ida A → B"
    );
    assert!(
        find_path(
            &state.map,
            TRAIN_DUAL_STATION_B,
            TRAIN_DUAL_STATION_A,
            PathNetwork::Rail,
        )
        .is_some(),
        "ruta vuelta B → A"
    );
    assert_eq!(
        rail_station_stop_tile(&state.map, TRAIN_DUAL_STATION_A),
        Some(TRAIN_DUAL_STATION_A)
    );
    assert_eq!(
        rail_station_stop_tile(&state.map, TRAIN_DUAL_STATION_B),
        Some(TRAIN_DUAL_STATION_B)
    );
}

#[test]
fn train_supply_dual_signals_green_on_clear_blocks() {
    let mut state = build_train_supply_dual();
    let mut dirty = Vec::new();
    crate::rail_signals::update_rail_signal_states(
        &mut state.map,
        &state.vehicles,
        &mut dirty,
        true,
    );
    for &y in &[TRAIN_DUAL_TRACK_OUT_Y, TRAIN_DUAL_TRACK_RET_Y] {
        let tile = state.map.get(TileCoord::new(7, y)).unwrap();
        assert!(
            crate::rail_signals::signal_is_green(tile.m3hi, 2)
                || crate::rail_signals::signal_is_green(tile.m3hi, 3),
            "señal en (7,{y}) debe estar verde con bloque libre: m3hi={:#x}",
            tile.m3hi
        );
    }
}

#[test]
fn train_supply_dual_second_train_never_passes_closed_signal() {
    use crate::rail_signals::train_blocked_by_signal;

    let mut state = build_train_supply_dual();
    for _ in 0..8_000 {
        let blocked = state
            .vehicles
            .iter()
            .find(|v| v.id == TRAIN_DUAL_VEHICLE_2_ID)
            .filter(|v| v.running && v.movement_target().is_some())
            .is_some_and(|v| train_blocked_by_signal(&state.map, &state.vehicles, v));
        let pos_before = state
            .vehicles
            .iter()
            .find(|v| v.id == TRAIN_DUAL_VEHICLE_2_ID)
            .map(|v| v.pos);
        state.step();
        if blocked {
            let pos_after = state
                .vehicles
                .iter()
                .find(|v| v.id == TRAIN_DUAL_VEHICLE_2_ID)
                .map(|v| v.pos);
            assert_eq!(
                pos_before, pos_after,
                "tren 2 no debe avanzar con señal/bloque cerrado"
            );
        }
    }
}

#[test]
fn train_supply_dual_follower_waits_before_last_signal() {
    use crate::rail_signals::train_blocked_by_signal;
    use std::collections::VecDeque;

    let mut state = build_train_supply_dual();
    let signal_pos = TileCoord::new(9, TRAIN_DUAL_TRACK_OUT_Y);
    let leader_pos = TileCoord::new(11, TRAIN_DUAL_TRACK_OUT_Y);
    let follower_pos = TileCoord::new(8, TRAIN_DUAL_TRACK_OUT_Y);

    {
        let leader = state
            .vehicles
            .iter_mut()
            .find(|v| v.id == TRAIN_DUAL_VEHICLE_ID)
            .expect("tren 1");
        leader.pos = leader_pos;
        leader.dest = TRAIN_DUAL_STATION_B;
        leader.current_order = 1;
        leader.path.clear();
        leader.running = false;
    }
    {
        let follower = state
            .vehicles
            .iter_mut()
            .find(|v| v.id == TRAIN_DUAL_VEHICLE_2_ID)
            .expect("tren 2");
        follower.pos = follower_pos;
        follower.dest = TRAIN_DUAL_STATION_B;
        follower.current_order = 1;
        follower.path = VecDeque::from([
            TileCoord::new(9, TRAIN_DUAL_TRACK_OUT_Y),
            TileCoord::new(10, TRAIN_DUAL_TRACK_OUT_Y),
            TileCoord::new(11, TRAIN_DUAL_TRACK_OUT_Y),
            TRAIN_DUAL_STATION_B,
        ]);
        follower.running = true;
        follower.set_cruise_speed();
        follower.progress = 200;
    }

    let mut dirty = Vec::new();
    crate::rail_signals::update_rail_signal_states(
        &mut state.map,
        &state.vehicles,
        &mut dirty,
        true,
    );
    let follower = state
        .vehicles
        .iter()
        .find(|v| v.id == TRAIN_DUAL_VEHICLE_2_ID)
        .expect("tren 2");
    assert!(
        train_blocked_by_signal(&state.map, &state.vehicles, follower),
        "seguidor debe frenar al completar la tesela previa a la última señal"
    );

    for _ in 0..500 {
        state.step();
        let f = state
            .vehicles
            .iter()
            .find(|v| v.id == TRAIN_DUAL_VEHICLE_2_ID)
            .expect("tren 2");
        assert!(
            f.pos.x < signal_pos.x,
            "no debe entrar al bloque tras la señal {signal_pos:?}: pos={:?}",
            f.pos
        );
    }
}

#[test]
fn train_supply_dual_follower_waits_at_signal_behind_leader() {
    use crate::rail_signals::train_blocked_by_signal;
    use std::collections::VecDeque;

    let mut state = build_train_supply_dual();
    let leader_pos = TileCoord::new(8, TRAIN_DUAL_TRACK_OUT_Y);
    let signal_pos = TileCoord::new(7, TRAIN_DUAL_TRACK_OUT_Y);
    let follower_pos = TileCoord::new(6, TRAIN_DUAL_TRACK_OUT_Y);

    {
        let leader = state
            .vehicles
            .iter_mut()
            .find(|v| v.id == TRAIN_DUAL_VEHICLE_ID)
            .expect("tren 1");
        leader.pos = leader_pos;
        leader.dest = TRAIN_DUAL_STATION_B;
        leader.current_order = 1;
        leader.cargo = 20;
        leader.cargo_type = Some(crate::CargoType::Coal);
        leader.path.clear();
        leader.running = false;
    }
    {
        let follower = state
            .vehicles
            .iter_mut()
            .find(|v| v.id == TRAIN_DUAL_VEHICLE_2_ID)
            .expect("tren 2");
        follower.pos = follower_pos;
        follower.dest = TRAIN_DUAL_STATION_B;
        follower.current_order = 1;
        follower.path = VecDeque::from([
            TileCoord::new(7, TRAIN_DUAL_TRACK_OUT_Y),
            TileCoord::new(8, TRAIN_DUAL_TRACK_OUT_Y),
            TileCoord::new(9, TRAIN_DUAL_TRACK_OUT_Y),
            TRAIN_DUAL_STATION_B,
        ]);
        follower.running = true;
        follower.set_cruise_speed();
        follower.progress = 200;
    }

    let mut dirty = Vec::new();
    crate::rail_signals::update_rail_signal_states(
        &mut state.map,
        &state.vehicles,
        &mut dirty,
        true,
    );
    let follower = state
        .vehicles
        .iter()
        .find(|v| v.id == TRAIN_DUAL_VEHICLE_2_ID)
        .expect("tren 2");
    assert!(
        train_blocked_by_signal(&state.map, &state.vehicles, follower),
        "seguidor debe frenar al completar la tesela previa a la señal"
    );

    for _ in 0..500 {
        state.step();
        let f = state
            .vehicles
            .iter()
            .find(|v| v.id == TRAIN_DUAL_VEHICLE_2_ID)
            .expect("tren 2");
        assert!(
            f.pos.x <= signal_pos.x,
            "el seguidor no debe entrar al bloque protegido por {signal_pos:?}: pos={:?}",
            f.pos
        );
    }
    let f = state
        .vehicles
        .iter()
        .find(|v| v.id == TRAIN_DUAL_VEHICLE_2_ID)
        .expect("tren 2");
    assert!(
        f.pos.x <= signal_pos.x,
        "debe esperar en o antes de la señal: pos={:?}",
        f.pos
    );
}

#[test]
fn train_supply_dual_round_trip_returns_to_a() {
    let mut state = build_train_supply_dual();
    let mut used_return_track = false;
    for _ in 0..12_000 {
        state.step();
        let train = state
            .vehicles
            .iter()
            .find(|v| v.id == TRAIN_DUAL_VEHICLE_ID)
            .expect("tren dual");
        if train.pos.y == TRAIN_DUAL_TRACK_RET_Y {
            used_return_track = true;
        }
    }
    let train = state
        .vehicles
        .iter()
        .find(|v| v.id == TRAIN_DUAL_VEHICLE_ID)
        .expect("tren dual");
    assert!(
        state.stats.cargo_deliveries > 0,
        "debe haber descargado carbón en B (pos={:?} order={} cargo={})",
        train.pos,
        train.current_order,
        train.cargo,
    );
    assert!(
        used_return_track,
        "debe circular por la vía de vuelta y={TRAIN_DUAL_TRACK_RET_Y}"
    );
    assert_eq!(
        train.pos, TRAIN_DUAL_STATION_A,
        "tras el ciclo debe volver a estación A: {:?}",
        train.pos
    );
}

#[test]
fn train_supply_dual_tick_698_no_signal_violation() {
    use crate::rail_signals::train_blocked_by_signal;

    let mut state = build_train_supply_dual();
    for tick in 1..=698 {
        state.step();
        let vehicles = state.vehicles.clone();
        for v in &vehicles {
            if v.id != TRAIN_DUAL_VEHICLE_2_ID || !v.running {
                continue;
            }
            if !train_blocked_by_signal(&state.map, &vehicles, v) {
                continue;
            }
            let after = state
                .vehicles
                .iter()
                .find(|t| t.id == TRAIN_DUAL_VEHICLE_2_ID)
                .expect("tren 2");
            assert_eq!(
                v.pos, after.pos,
                "tick {tick}: tren 2 avanzó con señal/bloque cerrado {:?}→{:?}",
                v.pos, after.pos
            );
        }
    }
}

fn signal_type_on_any_track(m2: u8) -> u8 {
    use crate::rail_signals::{SignalTrack, signal_type_for_track};
    [
        SignalTrack::X,
        SignalTrack::Y,
        SignalTrack::Upper,
        SignalTrack::Lower,
        SignalTrack::Left,
        SignalTrack::Right,
    ]
    .into_iter()
    .map(|t| signal_type_for_track(m2, t))
    .find(|&t| t != SIGTYPE_BLOCK)
    .unwrap_or(SIGTYPE_BLOCK)
}

#[test]
fn rail_signals_mixed_demo_has_presignal_station_and_two_way() {
    use crate::rail_signals::{
        rail_signal_present_mask, rail_signal_state_mask, update_rail_signal_states,
    };
    use crate::station;
    let mut state = build_rail_signals_mixed();
    update_rail_signal_states(&mut state.map, &state.vehicles, &mut Vec::new(), true);
    assert_eq!(state.map.dimensions(), (36, 22));
    assert_eq!(state.stations.len(), 2);
    assert_eq!(
        state.vehicles.len(),
        2,
        "líder activo + bloqueador en plataforma 2"
    );
    assert_eq!(state.industries.len(), 2);
    assert!(
        station::industry_in_station_coverage(
            state
                .industries
                .iter()
                .find(|i| i.pos == RAIL_SIGNALS_DEMO_MINE)
                .expect("mina"),
            RAIL_SIGNALS_DEMO_LOAD_STATION,
            station::STATION_COVERAGE_RADIUS,
        ),
        "mina en cobertura de estación de carga"
    );
    assert!(
        station::industry_in_station_coverage(
            state
                .industries
                .iter()
                .find(|i| i.pos == RAIL_SIGNALS_DEMO_FACTORY)
                .expect("fábrica"),
            RAIL_SIGNALS_DEMO_UNLOAD_STATION,
            station::STATION_COVERAGE_RADIUS,
        ),
        "fábrica en cobertura de estación de descarga"
    );
    let lead = state
        .vehicles
        .iter()
        .find(|v| v.id == RAIL_SIGNALS_DEMO_LEAD_ID)
        .expect("tren líder");
    assert!(lead.running);
    assert_eq!(lead.pos, RAIL_SIGNALS_DEMO_DEPOT);
    assert_eq!(lead.orders.len(), 2);
    let load_approach =
        crate::station::rail_station_approach_tile(&state.map, RAIL_SIGNALS_DEMO_LOAD_STATION)
            .or_else(|| {
                crate::station::rail_station_stop_tile(&state.map, RAIL_SIGNALS_DEMO_LOAD_STATION)
            })
            .expect("acceso estación carga");
    assert!(
        find_path(
            &state.map,
            TileCoord::new(2, RAIL_SIGNALS_DEMO_MAIN_Y),
            load_approach,
            PathNetwork::Rail,
        )
        .is_some(),
        "red ferroviaria depósito → carga"
    );

    let entry = state.map.get(RAIL_SIGNALS_DEMO_ENTRY).unwrap();
    assert_eq!(
        signal_type_on_any_track(entry.m2),
        SIGTYPE_ENTRY,
        "entry en ramificación presignal, no en línea principal"
    );
    for exit in [RAIL_SIGNALS_DEMO_EXIT1, RAIL_SIGNALS_DEMO_EXIT2] {
        let tile = state.map.get(exit).unwrap();
        assert_eq!(
            signal_type_on_any_track(tile.m2),
            SIGTYPE_EXIT,
            "exit en {exit:?}"
        );
    }
    for two_way in [
        RAIL_SIGNALS_DEMO_TWO_WAY_WEST,
        RAIL_SIGNALS_DEMO_TWO_WAY_EAST,
    ] {
        let tile = state.map.get(two_way).unwrap();
        assert_eq!(
            rail_signal_present_mask(tile.m3) & 0x0C,
            0x0C,
            "two-way en {two_way:?}"
        );
    }
    let entry_state = rail_signal_state_mask(entry.m3hi);
    let entry_present = rail_signal_present_mask(entry.m3);
    assert_eq!(
        entry_state & entry_present,
        entry_present,
        "entry verde: plataforma 1 libre aunque la 2 esté bloqueada"
    );
}

#[test]
fn rail_signals_mixed_train_cycles_orders_after_delivery() {
    let mut state = build_rail_signals_mixed();
    let mut saw_delivery = false;
    for _ in 0..1200 {
        let before = state.stats.cargo_deliveries;
        state.step();
        let v = state
            .vehicles
            .iter()
            .find(|veh| veh.id == RAIL_SIGNALS_DEMO_LEAD_ID)
            .expect("tren líder");
        if state.stats.cargo_deliveries > before {
            saw_delivery = true;
        }
        assert!(
            !(saw_delivery && v.no_network_route_to_order),
            "sin ruta tras primera entrega: orden {} pos {:?} dest {:?}",
            v.current_order + 1,
            v.pos,
            v.dest
        );
    }
    let v = state
        .vehicles
        .iter()
        .find(|veh| veh.id == RAIL_SIGNALS_DEMO_LEAD_ID)
        .expect("tren líder");
    assert!(
        state.stats.cargo_deliveries >= 2,
        "debe completar al menos dos entregas (ciclos): got {} entregas, orden {}, cargo {}",
        state.stats.cargo_deliveries,
        v.current_order + 1,
        v.cargo
    );
}

#[test]
fn rail_signals_mixed_path_from_plat2_to_load_station() {
    use crate::parity::scenario::{RAIL_SIGNALS_DEMO_LOAD_STATION, build_rail_signals_mixed};
    let state = build_rail_signals_mixed();
    let from = TileCoord::new(25, 14);
    let dest = crate::station::resolve_order_destination(
        &state.map,
        VehicleKind::Train,
        VehicleOrder::station(RAIL_SIGNALS_DEMO_LOAD_STATION),
    );
    let path = find_path(&state.map, from, dest, PathNetwork::Rail);
    assert!(
        path.is_some(),
        "debe haber ruta plataforma 2 → carga (conector de vuelta): from {from:?} dest {dest:?}"
    );
}

#[test]
fn rail_signals_mixed_has_all_signal_types() {
    use crate::rail_signals::{
        SignalTrack, default_signal_variant, rail_tile_is_signals, signal_placement_for_track,
        signal_type_for_track,
    };
    let state = build_rail_signals_mixed();
    let variant = default_signal_variant(crate::news::CALENDAR_BASE_YEAR);
    for &(x, expected_type) in RAIL_SIGNALS_MIXED_TYPES {
        let c = rail_signals_mixed_coord(x);
        let tile = state.map.get(c).expect("tesela señal");
        assert!(
            rail_tile_is_signals(tile.m5),
            "tesela ({x},{RAIL_SIGNALS_MIXED_Y}) debe ser RAIL_TILE_SIGNALS"
        );
        assert_eq!(
            signal_type_for_track(tile.m2, SignalTrack::X),
            expected_type,
            "tipo en x={x}"
        );
        let placement = signal_placement_for_track(SignalTrack::X, 0, variant, expected_type)
            .expect("encoding");
        assert_eq!(tile.m2 & 0x0F, placement.m2 & 0x0F, "m2 tipo en x={x}");
        assert_eq!(tile.m3 & 0xF0, placement.m3 & 0xF0, "m3 presente en x={x}");
    }
}

#[test]
fn rail_signals_mixed_json_roundtrip_preserves_encoding() {
    use crate::rail_signals::{SignalTrack, signal_type_for_track};
    let state = build_rail_signals_mixed();
    let json = state.save_json().expect("guardar");
    let restored = GameState::load_json(&json).expect("cargar");
    for &(x, expected_type) in RAIL_SIGNALS_MIXED_TYPES {
        let before = state.map.get(rail_signals_mixed_coord(x)).unwrap();
        let after = restored.map.get(rail_signals_mixed_coord(x)).unwrap();
        assert_eq!(before.m2, after.m2, "m2 x={x}");
        assert_eq!(before.m3, after.m3, "m3 x={x}");
        assert_eq!(before.m3hi, after.m3hi, "m3hi x={x}");
        assert_eq!(before.m5, after.m5, "m5 x={x}");
        assert_eq!(
            signal_type_for_track(after.m2, SignalTrack::X),
            expected_type
        );
    }
}
