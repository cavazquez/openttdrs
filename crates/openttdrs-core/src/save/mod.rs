//! Persistencia en disco del [`crate::game_state::GameState`] (JSON con versión de esquema).
//!
//! El formato con envoltorio (`version` + `state`) es el oficial a partir de I7.
//! `load` y [`load_from_str`] aceptan también JSON plano legado (solo `GameState`).

mod io;
mod migrate;

pub use io::{load, load_from_str, save};

/// Versión de esquema del JSON en disco (`GameStateFile.version`).
///
/// v2: los cruces de vía sintéticos `X|Y` pasan a ser empalmes con piezas
/// reales (recta + curvas); v1 se migra al cargar.
/// v3: las intersecciones de dos rectas vuelven a ser cruce X|Y (sin curvas),
/// como `OpenTTD`; los empalmes `0x3F` de v2 se migran al cargar.
/// v4: órdenes `Tile` en paradas/waypoints pasan a variantes tipadas; flags
/// `full_load` / `no_unload` explícitos en `VehicleOrder::Station`.
/// v5: campo opcional `Vehicle::name` (nombre personalizado).
/// v6: órdenes `VehicleOrder::Depot` con flag `stop`; migración desde `Tile` en depósito.
/// v7: horario MVP y autoreemplazo global.
/// v8: edad, grupos, lateness, autofill display, reglas extendidas.
/// v9: pools de órdenes compartidas.
/// v10: órdenes condicionales.
/// v11: teselas `MP_CLEAR` con `m5 == 0` (valor por defecto) → hierba completa.
/// v12: consist ferroviario (`next_unit` / `prev_unit` / longitudes); trenes
/// puntuales migran a consist de una unidad.
/// v13: cargo packets en estación/vehículo; balances `CargoStock` se hidratan.
/// v14: pool multi-compañía (`companies`, `owner` en vehículo/estación).
/// v15: railtypes en `m8` + `current_rail_type` (vías existentes → normal).
/// v16: monorail/maglev como `RailType` 2/3 (sin migración de datos; esquema).
/// v17: stack `NewGRF` (`newgrf_stack`) — config + cabecera; sin Action0–14.
/// v18: link graph observacional (`link_graph`).
/// v19: intervalo de servicio por defecto en vehículos antiguos.
/// v20: `cargo_dist` (modo Manual/Asymmetric/Symmetric; flows reconstruidos).
/// v21: `cargo_rng` persistido en `GameState` (antes efímero en runtime).
/// v22: rating persistente por carga (`Station::goods`) en vez de derivado del tiempo de espera.
/// v23: rating de autoridad local por compañía + `growth_rate` / `grow_counter` por pueblo.
/// v24: `random` / `interactive_random` / `cur_tileloop_tile` (`cargo_rng` → alias `random`).
/// v25: órdenes de estación usan `load_type` / `unload_type`; el lector acepta
/// los cinco booleanos legacy de v24 y JSON plano. Añade ajustes persistentes
/// de lado de circulación y de señales (con defaults compatibles).
/// v26: separa el límite efectivo de préstamo de un override individual
/// (`CompanyEconomy.max_loan_override`); migra el valor único de JSON antiguo.
/// v27: completa el último slot custom de cargo (`CargoType` 63) y acepta
/// arrays propios de 32 slots al deserializar estados anteriores.
pub const CURRENT_SAVE_VERSION: u32 = 27;

const SAVE_VERSION: u32 = CURRENT_SAVE_VERSION;

/// Error al guardar o cargar desde archivo / texto.
#[derive(Debug)]
pub enum SaveError {
    Io(std::io::Error),
    Json(serde_json::Error),
    UnsupportedVersion(u32),
    /// Archivo JSON excede el límite de seguridad.
    JsonSizeExceeded {
        actual: u64,
        limit: u64,
    },
}

impl std::fmt::Display for SaveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SaveError::Io(e) => write!(f, "{e}"),
            SaveError::Json(e) => write!(f, "{e}"),
            SaveError::UnsupportedVersion(v) => {
                write!(f, "versión de save no soportada: {v}")
            }
            SaveError::JsonSizeExceeded { actual, limit } => {
                write!(
                    f,
                    "archivo JSON excede el límite: {actual} bytes > {limit} bytes"
                )
            }
        }
    }
}

impl std::error::Error for SaveError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            SaveError::Io(e) => Some(e),
            SaveError::Json(e) => Some(e),
            SaveError::UnsupportedVersion(_) | SaveError::JsonSizeExceeded { .. } => None,
        }
    }
}

impl From<std::io::Error> for SaveError {
    fn from(e: std::io::Error) -> Self {
        SaveError::Io(e)
    }
}

impl From<serde_json::Error> for SaveError {
    fn from(e: serde_json::Error) -> Self {
        SaveError::Json(e)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use crate::{Command, Industry, IndustryKind, TileCoord, Vehicle, VehicleKind, VehicleOrder};

    use super::*;

    #[test]
    fn v5_migrates_depot_tile_orders_to_depot_variant() {
        let mut s = crate::GameState::new(8, 8);
        let depot = TileCoord::new(2, 2);
        crate::apply_command(&mut s, &Command::PlaceRoad(TileCoord::new(1, 2))).unwrap();
        crate::apply_command(&mut s, &Command::PlaceRoadDepotDir(depot, 0)).unwrap();
        let mut v = Vehicle::new(1, VehicleKind::Bus, depot, depot);
        v.orders = vec![VehicleOrder::tile(depot)];
        s.vehicles.push(v);

        let file = io::GameStateFile {
            version: 5,
            state: s,
        };
        let text = serde_json::to_string(&file).unwrap();
        let loaded = load_from_str(&text).unwrap();
        assert!(matches!(
            loaded.vehicles[0].orders[0],
            VehicleOrder::Depot { stop: true, .. }
        ));
    }

    #[test]
    fn save_load_roundtrip_file() {
        let mut s = crate::GameState::new(6, 6);
        s.industries
            .push(Industry::new(TileCoord::new(1, 1), IndustryKind::CoalMine));
        let dir = std::env::temp_dir();
        let path = dir.join(format!(
            "openttdrs_save_roundtrip_{}.json",
            std::process::id()
        ));
        save(&s, &path).unwrap();
        let loaded = load(&path).unwrap();
        assert_eq!(s.save_json().unwrap(), loaded.save_json().unwrap());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn legacy_flat_json_still_loads() {
        let mut s = crate::GameState::new(3, 3);
        s.tick.advance();
        let flat = s.save_json().unwrap();
        let loaded = load_from_str(&flat).unwrap();
        assert_eq!(loaded.tick.get(), s.tick.get());
    }

    #[test]
    fn v24_boolean_order_flags_migrate_to_complete_types() {
        use crate::{OrderLoadType, OrderNonStop, OrderUnloadType};

        let stop = TileCoord::new(1, 1);
        let mut state = crate::GameState::new(3, 3);
        let mut vehicle = Vehicle::new(1, VehicleKind::Truck, stop, stop);
        vehicle.orders = vec![VehicleOrder::station_with_types(
            stop,
            OrderLoadType::FullLoadAny,
            OrderUnloadType::Transfer,
            OrderNonStop::NonStopDestination,
        )];
        state.vehicles.push(vehicle);

        let mut json = serde_json::to_value(io::GameStateFile { version: 24, state }).unwrap();
        let order = json["state"]["vehicles"][0]["orders"][0]
            .as_object_mut()
            .unwrap();
        order.remove("load_type");
        order.remove("unload_type");
        order.insert("full_load_any".into(), true.into());
        order.insert("transfer".into(), true.into());

        let loaded = load_from_str(&serde_json::to_string(&json).unwrap()).unwrap();
        assert_eq!(
            loaded.vehicles[0].orders[0].load_type(),
            OrderLoadType::FullLoadAny
        );
        assert_eq!(
            loaded.vehicles[0].orders[0].unload_type(),
            OrderUnloadType::Transfer
        );
    }

    #[test]
    fn v25_migrates_custom_company_max_loan_override() {
        let mut state = crate::GameState::new(8, 8);
        // v25 tenía el valor efectivo, pero aún no el campo de override.
        state.companies[0].economy.max_loan = 455_000;
        state.companies[0].economy.max_loan_override = None;

        let file = io::GameStateFile { version: 25, state };
        let loaded = load_from_str(&serde_json::to_string(&file).unwrap()).unwrap();

        assert_eq!(loaded.companies[0].economy.max_loan_override, Some(455_000));
        assert_eq!(loaded.companies[0].economy.max_loan, 455_000);
        assert_eq!(loaded.economy.max_loan_override, Some(455_000));
        assert_eq!(loaded.economy.max_loan, 455_000);
    }

    #[test]
    fn save_load_after_n_steps() {
        let mut s = crate::GameState::new(5, 5);
        for _ in 0..7 {
            s.step();
        }
        let dir = std::env::temp_dir();
        let path = dir.join(format!("openttdrs_save_steps_{}.json", std::process::id()));
        save(&s, &path).unwrap();
        let mut loaded = load(&path).unwrap();
        assert_eq!(loaded.tick.get(), s.tick.get());
        loaded.step();
        s.step();
        assert_eq!(loaded.tick.get(), s.tick.get());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn roundtrip_str_versioned() {
        let s = crate::GameState::new(4, 4);
        let tmp =
            std::env::temp_dir().join(format!("openttdrs_save_str_{}.json", std::process::id()));
        save(&s, &tmp).unwrap();
        let text = std::fs::read_to_string(&tmp).unwrap();
        assert!(text.contains("\"version\""));
        let again = load_from_str(&text).unwrap();
        assert_eq!(again.map.dimensions(), s.map.dimensions());
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn v1_migrates_synthetic_crossing_to_curve_junction() {
        use crate::{Command, TileKind, command::apply_command};

        let mut s = crate::GameState::new(8, 8);
        for x in 2..=6_i32 {
            apply_command(&mut s, &Command::PlaceRail(TileCoord::new(x, 3))).unwrap();
        }
        for y in 4..=5_i32 {
            apply_command(&mut s, &Command::PlaceRail(TileCoord::new(4, y))).unwrap();
        }
        let mut t = s.map.get(TileCoord::new(4, 3)).unwrap();
        assert_eq!(t.kind, TileKind::Rail);
        t.m5 = (t.m5 & 0xC0) | 0x03;
        s.map.set_tile(TileCoord::new(4, 3), t).unwrap();

        let file = io::GameStateFile {
            version: 1,
            state: s,
        };
        let text = serde_json::to_string(&file).unwrap();
        let loaded = load_from_str(&text).unwrap();
        let bits = loaded.map.get(TileCoord::new(4, 3)).unwrap().m5 & 0x3F;
        assert_eq!(bits, 0x29, "cruce migrado a piezas reales: {bits:#04x}");
    }

    #[test]
    fn v2_migrates_full_junction_to_clean_crossing() {
        use crate::{Command, TileKind, command::apply_command};

        let mut s = crate::GameState::new(9, 9);
        for x in 2..=6_i32 {
            apply_command(&mut s, &Command::PlaceRail(TileCoord::new(x, 4))).unwrap();
        }
        for y in 2..=6_i32 {
            apply_command(&mut s, &Command::PlaceRail(TileCoord::new(4, y))).unwrap();
        }
        let mut t = s.map.get(TileCoord::new(4, 4)).unwrap();
        assert_eq!(t.kind, TileKind::Rail);
        t.m5 = (t.m5 & 0xC0) | 0x3F;
        s.map.set_tile(TileCoord::new(4, 4), t).unwrap();

        let file = io::GameStateFile {
            version: 2,
            state: s,
        };
        let text = serde_json::to_string(&file).unwrap();
        let loaded = load_from_str(&text).unwrap();
        let bits = loaded.map.get(TileCoord::new(4, 4)).unwrap().m5 & 0x3F;
        assert_eq!(bits, 0x03, "cruce limpio X|Y: {bits:#04x}");
    }

    #[test]
    fn v3_migrates_tile_orders_at_stations_to_station_variant() {
        use crate::map::TileKind;
        use crate::{Station, StopKind, Vehicle, VehicleKind, VehicleOrder};

        let mut s = crate::GameState::new(8, 8);
        let stop = TileCoord::new(3, 3);
        s.stations
            .push(Station::new_with_kind(stop, StopKind::TruckStop));
        let mut tile = s.map.get(stop).unwrap();
        tile.kind = TileKind::Station;
        s.map.set_tile(stop, tile).unwrap();
        let mut v = Vehicle::new(1, VehicleKind::Truck, stop, TileCoord::new(0, 0));
        v.orders = vec![VehicleOrder::tile(stop)];
        s.vehicles.push(v);

        let file = io::GameStateFile {
            version: 3,
            state: s,
        };
        let text = serde_json::to_string(&file).unwrap();
        let loaded = load_from_str(&text).unwrap();
        let order = loaded.vehicles[0].orders[0];
        assert!(matches!(order, VehicleOrder::Station { station, .. } if station == stop));
        assert!(!order.full_load());
        assert!(!order.no_unload());
    }

    #[test]
    fn v3_migrates_station_orders_without_flags_and_resaves_as_v4() {
        use crate::{Station, StopKind, Vehicle, VehicleKind, VehicleOrder};

        let mut s = crate::GameState::new(4, 4);
        s.stations.push(Station::new_with_kind(
            TileCoord::new(1, 1),
            StopKind::TruckStop,
        ));
        let mut v = Vehicle::new(
            1,
            VehicleKind::Truck,
            TileCoord::new(0, 0),
            TileCoord::new(2, 2),
        );
        v.orders = vec![VehicleOrder::station(TileCoord::new(1, 1))];
        s.vehicles.push(v);

        let file = io::GameStateFile {
            version: 3,
            state: s,
        };
        let text = serde_json::to_string(&file).unwrap();
        let loaded = load_from_str(&text).unwrap();
        let dir = std::env::temp_dir();
        let path = dir.join(format!("openttdrs_v3_migrate_{}.json", std::process::id()));
        save(&loaded, &path).unwrap();
        let saved_text = std::fs::read_to_string(&path).unwrap();
        assert!(saved_text.contains(&format!("\"version\": {CURRENT_SAVE_VERSION}")));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn unsupported_save_version_returns_error() {
        let s = crate::GameState::new(2, 2);
        let file = io::GameStateFile {
            version: CURRENT_SAVE_VERSION + 1,
            state: s,
        };
        let text = serde_json::to_string(&file).unwrap();
        let err = load_from_str(&text).unwrap_err();
        assert!(matches!(err, SaveError::UnsupportedVersion(v) if v == CURRENT_SAVE_VERSION + 1));
    }

    #[test]
    fn v16_migrates_empty_newgrf_stack_to_vanilla() {
        let mut s = crate::GameState::new(2, 2);
        s.newgrf_stack.clear();
        let file = io::GameStateFile {
            version: 16,
            state: s,
        };
        let text = serde_json::to_string(&file).unwrap();
        let loaded = load_from_str(&text).unwrap();
        assert_eq!(
            loaded.newgrf_stack,
            crate::newgrf_config::default_vanilla_stack()
        );
    }

    #[test]
    fn random_persists_across_save_load_roundtrip() {
        let mut s = crate::GameState::new(4, 4);
        for _ in 0..5 {
            let _ = s.random.next();
        }
        let expected_next = {
            let mut copy = s.random;
            copy.next()
        };
        let dir = std::env::temp_dir();
        let path = dir.join(format!(
            "openttdrs_random_persist_{}.json",
            std::process::id()
        ));
        save(&s, &path).unwrap();
        let mut loaded = load(&path).unwrap();
        assert_eq!(loaded.random.next(), expected_next);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn random_v20_save_loads_with_default_seed_via_cargo_rng_alias() {
        let mut s = crate::GameState::new(3, 3);
        for _ in 0..10 {
            let _ = s.random.next();
        }
        let file = io::GameStateFile {
            version: 20,
            state: s.clone(),
        };
        let text = serde_json::to_string(&file).unwrap();
        let mut v: serde_json::Value = serde_json::from_str(&text).unwrap();
        if let Some(state) = v.get_mut("state").and_then(|s| s.as_object_mut()) {
            state.remove("random");
            state.remove("cargo_rng");
        }
        let modified_text = serde_json::to_string(&v).unwrap();
        let loaded = load_from_str(&modified_text).unwrap();
        let default_rng = crate::linkgraph_parity::Randomizer::new(1);
        assert_eq!(loaded.random.state, default_rng.state);
    }

    #[test]
    fn cur_tileloop_tile_persists_across_save_load_roundtrip() {
        let mut s = crate::GameState::new(64, 64);
        s.cur_tileloop_tile = 42;
        let path = std::env::temp_dir().join(format!(
            "openttdrs_tileloop_persist_{}.json",
            std::process::id()
        ));
        save(&s, &path).unwrap();
        let loaded = load(&path).unwrap();
        assert_eq!(loaded.cur_tileloop_tile, 42);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn legacy_json_without_new_vehicle_station_fields_still_loads() {
        let mut s = crate::GameState::new(4, 4);
        s.stations.push(crate::Station::new(TileCoord::new(1, 1)));
        s.vehicles.push(crate::Vehicle::new(
            1,
            crate::VehicleKind::Truck,
            TileCoord::new(1, 1),
            TileCoord::new(2, 2),
        ));
        let mut v: serde_json::Value = serde_json::from_str(&s.save_json().unwrap()).unwrap();
        if let Some(stations) = v
            .get_mut("stations")
            .and_then(serde_json::Value::as_array_mut)
        {
            for station in stations {
                let _ = station.as_object_mut().map(|obj| {
                    obj.remove("stop_kind");
                    obj.remove("cargo_stock");
                });
            }
        }
        if let Some(vehicles) = v
            .get_mut("vehicles")
            .and_then(serde_json::Value::as_array_mut)
        {
            for vehicle in vehicles {
                let _ = vehicle.as_object_mut().map(|obj| {
                    obj.remove("cargo_type");
                    obj.remove("running");
                    obj.remove("no_network_route_to_order");
                });
            }
        }
        let legacy_text = serde_json::to_string(&v).unwrap();
        let loaded = load_from_str(&legacy_text).unwrap();
        assert_eq!(loaded.stations.len(), 1);
        assert_eq!(loaded.vehicles.len(), 1);
        assert!(loaded.vehicles[0].running);
        assert!(!loaded.vehicles[0].no_network_route_to_order);
        assert_eq!(loaded.stations[0].cargo_stock, crate::CargoStock::default());
    }

    #[test]
    fn load_json_with_invalid_current_order_sanitizes() {
        let mut s = crate::GameState::new(4, 4);
        let mut v = Vehicle::new(
            1,
            VehicleKind::Truck,
            TileCoord::new(0, 0),
            TileCoord::new(1, 0),
        );
        v.orders = vec![
            VehicleOrder::station(TileCoord::new(1, 1)),
            VehicleOrder::station(TileCoord::new(2, 2)),
        ];
        s.vehicles.push(v);
        let mut v_json: serde_json::Value = serde_json::from_str(&s.save_json().unwrap()).unwrap();
        if let Some(vehicles) = v_json
            .get_mut("vehicles")
            .and_then(serde_json::Value::as_array_mut)
            && let Some(vehicle) = vehicles.first_mut()
            && let Some(obj) = vehicle.as_object_mut()
        {
            obj.insert("current_order".to_string(), serde_json::json!(99));
        }
        let corrupted_text = serde_json::to_string(&v_json).unwrap();
        let loaded = load_from_str(&corrupted_text).unwrap();
        assert_eq!(loaded.vehicles[0].current_order, 0);
    }

    #[test]
    fn load_json_with_empty_orders_and_invalid_current_order_sanitizes() {
        let mut s = crate::GameState::new(4, 4);
        let v = Vehicle::new(
            1,
            VehicleKind::Truck,
            TileCoord::new(0, 0),
            TileCoord::new(1, 0),
        );
        s.vehicles.push(v);
        let mut v_json: serde_json::Value = serde_json::from_str(&s.save_json().unwrap()).unwrap();
        if let Some(vehicles) = v_json
            .get_mut("vehicles")
            .and_then(serde_json::Value::as_array_mut)
            && let Some(vehicle) = vehicles.first_mut()
            && let Some(obj) = vehicle.as_object_mut()
        {
            obj.insert("current_order".to_string(), serde_json::json!(5));
        }
        let corrupted_text = serde_json::to_string(&v_json).unwrap();
        let loaded = load_from_str(&corrupted_text).unwrap();
        assert_eq!(loaded.vehicles[0].current_order, 0);
        assert_eq!(loaded.vehicles[0].orders.len(), 0);
    }

    #[test]
    fn vehicle_advance_and_sync_no_panic_after_sanitization() {
        use crate::map::Map;

        let mut s = crate::GameState::new(8, 8);
        let map = Map::new_flat(8, 8, 0);
        let mut v = Vehicle::new(
            1,
            VehicleKind::Truck,
            TileCoord::new(0, 0),
            TileCoord::new(1, 0),
        );
        v.orders = vec![
            VehicleOrder::station(TileCoord::new(2, 2)),
            VehicleOrder::station(TileCoord::new(3, 3)),
        ];
        v.current_order = 99;
        s.vehicles.push(v);

        s.sanitize_all_vehicle_orders();
        assert_eq!(s.vehicles[0].current_order, 0);

        s.vehicles[0].sync_order_destination(&map);
        s.vehicles[0].advance_after_loading();
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn excessive_json_file_is_rejected() {
        // Crear archivo JSON > 100 MB
        let dir = std::env::temp_dir();
        let path = dir.join(format!("openttdrs_json_bomb_{}.json", std::process::id()));
        let huge_json = format!(
            r#"{{"version": 1, "state": {{"map": {{"tiles": [{}]}}}}}}"#,
            "0,".repeat(60_000_000) // ~240 MB de JSON
        );
        std::fs::write(&path, huge_json).unwrap();
        let err = load(&path).expect_err("debe rechazar JSON excesivo");
        assert!(matches!(err, SaveError::JsonSizeExceeded { .. }));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn excessive_json_string_is_rejected() {
        // String JSON > 100 MB
        let huge_json = format!(
            r#"{{"version": 1, "state": {{"map": {{"tiles": [{}]}}}}}}"#,
            "0,".repeat(60_000_000) // ~240 MB de JSON
        );
        let err = load_from_str(&huge_json).expect_err("debe rechazar JSON excesivo");
        assert!(matches!(err, SaveError::JsonSizeExceeded { .. }));
    }

    #[test]
    fn valid_fixtures_still_load_after_limits() {
        // Verificar que fixtures válidos siguen cargando
        let s = crate::GameState::new(16, 16);
        let dir = std::env::temp_dir();
        let path = dir.join(format!(
            "openttdrs_valid_fixture_{}.json",
            std::process::id()
        ));
        save(&s, &path).unwrap();
        let loaded = load(&path).unwrap();
        assert_eq!(loaded.map.dimensions(), s.map.dimensions());
        let _ = std::fs::remove_file(&path);
    }
}
