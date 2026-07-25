//! Migraciones de esquema para persistencia.

use crate::GameState;

use super::{CURRENT_SAVE_VERSION, SaveError};

/// Aplica migraciones encadenadas hasta [`CURRENT_SAVE_VERSION`].
pub(super) fn migrate_loaded_state(
    version: u32,
    mut state: GameState,
) -> Result<GameState, SaveError> {
    if version > CURRENT_SAVE_VERSION {
        return Err(SaveError::UnsupportedVersion(version));
    }
    let mut v = version;
    while v < CURRENT_SAVE_VERSION {
        match v {
            1 | 2 => {
                crate::command::normalize_synthetic_rail_crossings(&mut state.map);
            }
            3 => migrate_state_v3_to_v4(&mut state),
            4 | 6 | 7 | 8 | 9 | 15 => {}
            5 => migrate_state_v5_to_v6(&mut state),
            10 => migrate_state_v10_to_v11(&mut state),
            11 => migrate_state_v11_to_v12(&mut state),
            12 => migrate_state_v12_to_v13(&mut state),
            13 => migrate_state_v13_to_v14(&mut state),
            14 => migrate_state_v14_to_v15(&mut state),
            16 => migrate_state_v16_to_v17(&mut state),
            17 => migrate_state_v17_to_v18(&mut state),
            18 => migrate_state_v18_to_v19(&mut state),
            19 => migrate_state_v19_to_v20(&mut state),
            20 => migrate_state_v20_to_v21(&mut state),
            21 => migrate_state_v21_to_v22(&mut state),
            22 => migrate_state_v22_to_v23(&mut state),
            _ => return Err(SaveError::UnsupportedVersion(version)),
        }
        v += 1;
    }
    after_migrate_refresh_newgrf(&mut state);
    state.rebuild_station_flows();
    state.sanitize_all_vehicle_orders();
    Ok(state)
}

/// v20: modo `CargoDist` por defecto (`Manual`); `station_flows` se reconstruyen.
fn migrate_state_v19_to_v20(state: &mut GameState) {
    state.cargo_dist = crate::flow_stat::CargoDistSettings::default();
}

/// v23: rating de autoridad por compañía; campos de crecimiento urbano.
fn migrate_state_v22_to_v23(state: &mut GameState) {
    for town in &mut state.towns {
        town.migrate_legacy_authority_rating();
        if town.authority_ratings.is_empty() {
            town.authority_ratings =
                vec![crate::town::TOWN_RATING_INITIAL; crate::town::MAX_TOWN_AUTHORITY_COMPANIES];
        }
        if town.growth_rate == 0 && town.grow_counter == 0 {
            town.init_grow_counter();
        }
    }
}

/// v22: rating persistente por carga (`Station::goods`).
///
/// El save antiguo solo guardaba el rating agregado. Se reparte a las cargas que la estación
/// estaba moviendo para que una línea en marcha no aparezca como recién construida.
fn migrate_state_v21_to_v22(state: &mut GameState) {
    for station in &mut state.stations {
        for cargo in crate::cargo::ALL_CARGO_TYPES {
            if station.cargo_stock.get(cargo) == 0 {
                continue;
            }
            let entry = station.goods.get_mut(cargo);
            entry.has_rating = true;
            entry.rating = station.rating;
        }
    }
}

/// v21: `cargo_rng` persistido en `GameState` (campo faltante usa default seed 1).
fn migrate_state_v20_to_v21(_state: &mut GameState) {
    // Serde ya aplica el default `Randomizer::new(1)` si el campo está ausente.
}

/// v19: intervalo de servicio por defecto en vehículos antiguos.
fn migrate_state_v18_to_v19(state: &mut GameState) {
    for v in &mut state.vehicles {
        if v.service_interval_days == 0 {
            v.service_interval_days = crate::vehicle::DEFAULT_SERVICE_INTERVAL_DAYS;
        }
        if v.last_service_day == 0 {
            v.last_service_day =
                crate::news::calendar_day_index(crate::tick::GameTick::new(v.build_tick));
        }
    }
}

/// v18: link graph vacío (observacional; sin datos previos).
fn migrate_state_v17_to_v18(state: &mut GameState) {
    state.link_graph = crate::link_graph::LinkGraphStats::default();
}

/// Tras migrar: reaplicar `RoadTypes` del stack si hay archivos.
fn after_migrate_refresh_newgrf(state: &mut GameState) {
    crate::newgrf_actions::apply_newgrf_stack_catalogs_default_dirs(state);
}

/// v17: stack `NewGRF` vacío → `OpenGFX` documentado.
fn migrate_state_v16_to_v17(state: &mut GameState) {
    if state.newgrf_stack.is_empty() {
        state.newgrf_stack = crate::newgrf_config::default_vanilla_stack();
    }
}

/// v15: railtype por defecto en vías existentes (`m8` bits 0–5 = Rail).
fn migrate_state_v14_to_v15(state: &mut GameState) {
    use crate::map::TileKind;
    use crate::rail_type::{RailType, set_rail_type_on_tile};
    state.current_rail_type = RailType::Rail;
    let (w, h) = state.map.dimensions();
    for y in 0..h.cast_signed() {
        for x in 0..w.cast_signed() {
            let c = crate::map::TileCoord::new(x, y);
            let Some(tile) = state.map.get(c) else {
                continue;
            };
            if tile.kind != TileKind::Rail {
                continue;
            }
            let _ = state.map.set_tile(
                c,
                set_rail_type_on_tile(
                    tile,
                    RailType::from_u8(u8::try_from(tile.m8 & 0x3F).unwrap_or(0)),
                ),
            );
        }
    }
}

/// v14: pool de compañías + owners por defecto (jugador).
fn migrate_state_v13_to_v14(state: &mut GameState) {
    state.ensure_companies();
    for v in &mut state.vehicles {
        let _ = v.owner;
    }
    for s in &mut state.stations {
        let _ = s.owner;
    }
}

/// v13: hidratar packets desde balances agregados.
fn migrate_state_v12_to_v13(state: &mut GameState) {
    for station in &mut state.stations {
        station.ensure_packets_from_stock();
    }
    for vehicle in &mut state.vehicles {
        vehicle.ensure_packets_from_legacy();
    }
}

/// v12: inicializar campos de consist en trenes existentes.
fn migrate_state_v11_to_v12(state: &mut GameState) {
    use crate::vehicle::VehicleKind;
    let train_ids: Vec<u32> = state
        .vehicles
        .iter()
        .filter(|v| v.kind == VehicleKind::Train)
        .map(|v| v.id)
        .collect();
    for id in train_ids {
        if let Some(v) = state.vehicles.iter_mut().find(|x| x.id == id) {
            v.next_unit = None;
            v.prev_unit = None;
            if v.unit_length == 0 {
                v.unit_length = crate::train_consist::VEHICLE_LENGTH;
            }
            if v.cached_total_length == 0 {
                v.cached_total_length = u16::from(v.unit_length);
            }
        }
        crate::train_consist::consist_changed(&mut state.vehicles, id);
    }
}

/// v4: tipar órdenes legacy y normalizar flags de parada.
fn migrate_state_v3_to_v4(state: &mut GameState) {
    use std::collections::HashMap;

    use crate::TileCoord;
    use crate::map::TileKind;
    use crate::station::{StopKind, is_rail_waypoint_tile};
    use crate::vehicle::VehicleOrder;

    let station_kinds: HashMap<TileCoord, StopKind> = state
        .stations
        .iter()
        .map(|s| (s.pos, s.stop_kind))
        .collect();

    for vehicle in &mut state.vehicles {
        vehicle.orders = vehicle
            .orders
            .iter()
            .map(|&order| match order {
                VehicleOrder::Tile(pos) => {
                    if station_kinds.get(&pos) == Some(&StopKind::RailWaypoint)
                        || station_kinds.get(&pos) == Some(&StopKind::RoadWaypoint)
                        || state
                            .map
                            .get(pos)
                            .is_some_and(|t| is_rail_waypoint_tile(&t))
                    {
                        VehicleOrder::waypoint(pos)
                    } else if station_kinds.contains_key(&pos)
                        || state
                            .map
                            .get(pos)
                            .is_some_and(|t| t.kind == TileKind::Station)
                    {
                        VehicleOrder::station(pos)
                    } else {
                        VehicleOrder::tile(pos)
                    }
                }
                VehicleOrder::Station {
                    station,
                    full_load,
                    no_unload,
                    ..
                } => VehicleOrder::station_with_flags(station, full_load, no_unload),
                VehicleOrder::Waypoint { waypoint, .. } => VehicleOrder::waypoint(waypoint),
                VehicleOrder::Depot { depot, stop, .. } => VehicleOrder::Depot {
                    depot,
                    stop,
                    wait_ticks: 0,
                    travel_ticks: 0,
                    refit_cargo: None,
                },
                VehicleOrder::Conditional { .. } => order,
            })
            .collect();
    }
}

/// v6: órdenes en teselas de depósito pasan a `VehicleOrder::Depot`.
fn migrate_state_v5_to_v6(state: &mut GameState) {
    use crate::map::TileKind;
    use crate::vehicle::VehicleOrder;

    for vehicle in &mut state.vehicles {
        vehicle.orders = vehicle
            .orders
            .iter()
            .map(|&order| {
                if let VehicleOrder::Tile(pos) = order
                    && matches!(
                        state.map.get_kind(pos),
                        Some(TileKind::RoadDepot | TileKind::RailDepot)
                    )
                {
                    VehicleOrder::depot(pos)
                } else {
                    order
                }
            })
            .collect();
    }
}

/// v11: `m5 == 0` en hierba `MP_CLEAR` era el default de `new_flat`, no suelo desnudo.
fn migrate_state_v10_to_v11(state: &mut GameState) {
    state.map.migrate_legacy_clear_grass_m5();
}
