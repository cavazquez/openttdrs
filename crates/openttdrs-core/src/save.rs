//! Persistencia en disco del [`GameState`] (JSON con versión de esquema).
//!
//! El formato con envoltorio (`version` + `state`) es el oficial a partir de I7.
//! `load` y [`load_from_str`] aceptan también JSON plano legado (solo `GameState`).

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::GameState;

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
pub const CURRENT_SAVE_VERSION: u32 = 11;

const SAVE_VERSION: u32 = CURRENT_SAVE_VERSION;

/// Contenedor en disco: una sola versión de esquema por ahora; migraciones futuras leen `version`.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct GameStateFile {
    version: u32,
    state: GameState,
}

/// Error al guardar o cargar desde archivo / texto.
#[derive(Debug)]
pub enum SaveError {
    Io(std::io::Error),
    Json(serde_json::Error),
    UnsupportedVersion(u32),
}

impl std::fmt::Display for SaveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SaveError::Io(e) => write!(f, "{e}"),
            SaveError::Json(e) => write!(f, "{e}"),
            SaveError::UnsupportedVersion(v) => {
                write!(f, "versión de save no soportada: {v}")
            }
        }
    }
}

impl std::error::Error for SaveError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            SaveError::Io(e) => Some(e),
            SaveError::Json(e) => Some(e),
            SaveError::UnsupportedVersion(_) => None,
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

/// Escribe `state` en `path` como JSON formateado (versión + estado).
///
/// # Errors
///
/// Fallos de E/S o serialización.
pub fn save(state: &GameState, path: &Path) -> Result<(), SaveError> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)?;
    }
    let file = GameStateFile {
        version: SAVE_VERSION,
        state: state.clone(),
    };
    let json = serde_json::to_string_pretty(&file)?;
    std::fs::write(path, json)?;
    Ok(())
}

/// Carga desde archivo (formato versionado o JSON legado sin envoltorio).
///
/// # Errors
///
/// E/S, JSON inválido, o `UnsupportedVersion` si el número de versión no está soportado.
pub fn load(path: &Path) -> Result<GameState, SaveError> {
    let text = std::fs::read_to_string(path)?;
    load_from_str(&text)
}

/// Igual que [`load`] pero desde memoria (p. ej. `OTTDJSON_LOAD`).
///
/// # Errors
///
/// Ver [`load`].
pub fn load_from_str(text: &str) -> Result<GameState, SaveError> {
    let v: serde_json::Value = serde_json::from_str(text)?;
    if v.get("version").is_some() && v.get("state").is_some() {
        let file: GameStateFile = serde_json::from_value(v)?;
        migrate_loaded_state(file.version, file.state)
    } else {
        let mut state = GameState::load_json(text)?;
        crate::command::normalize_synthetic_rail_crossings(&mut state.map);
        state.map.migrate_legacy_clear_grass_m5();
        Ok(state)
    }
}

/// Aplica migraciones encadenadas hasta [`CURRENT_SAVE_VERSION`].
fn migrate_loaded_state(version: u32, mut state: GameState) -> Result<GameState, SaveError> {
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
            4 | 6 | 7 | 8 | 9 => {}
            5 => migrate_state_v5_to_v6(&mut state),
            10 => migrate_state_v10_to_v11(&mut state),
            _ => return Err(SaveError::UnsupportedVersion(version)),
        }
        v += 1;
    }
    Ok(state)
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

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use crate::{Command, Industry, IndustryKind, TileCoord, Vehicle, VehicleKind, VehicleOrder};

    use super::*;

    #[test]
    fn v5_migrates_depot_tile_orders_to_depot_variant() {
        let mut s = GameState::new(8, 8);
        let depot = TileCoord::new(2, 2);
        crate::apply_command(&mut s, &Command::PlaceRoad(TileCoord::new(1, 2))).unwrap();
        crate::apply_command(&mut s, &Command::PlaceRoadDepotDir(depot, 0)).unwrap();
        let mut v = Vehicle::new(1, VehicleKind::Bus, depot, depot);
        v.orders = vec![VehicleOrder::tile(depot)];
        s.vehicles.push(v);

        let file = GameStateFile {
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
        let mut s = GameState::new(6, 6);
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
        let mut s = GameState::new(3, 3);
        s.tick.advance();
        let flat = s.save_json().unwrap();
        let loaded = load_from_str(&flat).unwrap();
        assert_eq!(loaded.tick.get(), s.tick.get());
    }

    #[test]
    fn save_load_after_n_steps() {
        let mut s = GameState::new(5, 5);
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
        let s = GameState::new(4, 4);
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

        let mut s = GameState::new(8, 8);
        // Línea X (y=3) con ramal Y hacia el sur en x=4 (codo en (4,3)).
        for x in 2..=6_i32 {
            apply_command(&mut s, &Command::PlaceRail(TileCoord::new(x, 3))).unwrap();
        }
        for y in 4..=5_i32 {
            apply_command(&mut s, &Command::PlaceRail(TileCoord::new(4, y))).unwrap();
        }
        // Simula el save viejo: el empalme guardado como cruce sintético X|Y.
        let mut t = s.map.get(TileCoord::new(4, 3)).unwrap();
        assert_eq!(t.kind, TileKind::Rail);
        t.m5 = (t.m5 & 0xC0) | 0x03;
        s.map.set_tile(TileCoord::new(4, 3), t).unwrap();

        let file = GameStateFile {
            version: 1,
            state: s,
        };
        let text = serde_json::to_string(&file).unwrap();
        let loaded = load_from_str(&text).unwrap();
        let bits = loaded.map.get(TileCoord::new(4, 3)).unwrap().m5 & 0x3F;
        // X (NE↔SW) + LOWER (SE↔SW) + RIGHT (NE↔SE): empalme en T hacia el sur.
        assert_eq!(bits, 0x29, "cruce migrado a piezas reales: {bits:#04x}");
    }

    #[test]
    fn v2_migrates_full_junction_to_clean_crossing() {
        use crate::{Command, TileKind, command::apply_command};

        let mut s = GameState::new(9, 9);
        // Dos rectas que se cruzan en (4,4).
        for x in 2..=6_i32 {
            apply_command(&mut s, &Command::PlaceRail(TileCoord::new(x, 4))).unwrap();
        }
        for y in 2..=6_i32 {
            apply_command(&mut s, &Command::PlaceRail(TileCoord::new(4, y))).unwrap();
        }
        // Simula el save v2: la intersección guardada con las seis piezas.
        let mut t = s.map.get(TileCoord::new(4, 4)).unwrap();
        assert_eq!(t.kind, TileKind::Rail);
        t.m5 = (t.m5 & 0xC0) | 0x3F;
        s.map.set_tile(TileCoord::new(4, 4), t).unwrap();

        let file = GameStateFile {
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

        let mut s = GameState::new(8, 8);
        let stop = TileCoord::new(3, 3);
        s.stations
            .push(Station::new_with_kind(stop, StopKind::TruckStop));
        let mut tile = s.map.get(stop).unwrap();
        tile.kind = TileKind::Station;
        s.map.set_tile(stop, tile).unwrap();
        let mut v = Vehicle::new(1, VehicleKind::Truck, stop, TileCoord::new(0, 0));
        v.orders = vec![VehicleOrder::tile(stop)];
        s.vehicles.push(v);

        let file = GameStateFile {
            version: 3,
            state: s,
        };
        let text = serde_json::to_string(&file).unwrap();
        let loaded = load_from_str(&text).unwrap();
        assert!(matches!(
            loaded.vehicles[0].orders[0],
            VehicleOrder::Station {
                station,
                full_load: false,
                no_unload: false,
                ..
            } if station == stop
        ));
    }

    #[test]
    fn v3_migrates_station_orders_without_flags_and_resaves_as_v4() {
        use crate::{Station, StopKind, Vehicle, VehicleKind, VehicleOrder};

        let mut s = GameState::new(4, 4);
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

        let file = GameStateFile {
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
        let s = GameState::new(2, 2);
        let file = GameStateFile {
            version: CURRENT_SAVE_VERSION + 1,
            state: s,
        };
        let text = serde_json::to_string(&file).unwrap();
        let err = load_from_str(&text).unwrap_err();
        assert!(matches!(err, SaveError::UnsupportedVersion(v) if v == CURRENT_SAVE_VERSION + 1));
    }

    #[test]
    fn legacy_json_without_new_vehicle_station_fields_still_loads() {
        let mut s = GameState::new(4, 4);
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
}
