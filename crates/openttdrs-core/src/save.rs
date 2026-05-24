//! Persistencia en disco del [`GameState`] (JSON con versión de esquema).
//!
//! El formato con envoltorio (`version` + `state`) es el oficial a partir de I7.
//! `load` y [`load_from_str`] aceptan también JSON plano legado (solo `GameState`).

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::GameState;

/// Versión de esquema del JSON en disco (`GameStateFile.version`).
pub const CURRENT_SAVE_VERSION: u32 = 1;

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
        Ok(GameState::load_json(text)?)
    }
}

/// Aplica migraciones encadenadas hasta [`CURRENT_SAVE_VERSION`].
fn migrate_loaded_state(version: u32, state: GameState) -> Result<GameState, SaveError> {
    match version {
        CURRENT_SAVE_VERSION => Ok(state),
        n => Err(SaveError::UnsupportedVersion(n)),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use crate::{Industry, IndustryKind, TileCoord};

    use super::*;

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
