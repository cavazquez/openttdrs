//! Operaciones de E/S para persistencia en disco del [`GameState`].

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::GameState;

use super::migrate::migrate_loaded_state;
use super::{SAVE_VERSION, SaveError};

/// Límite de bytes para archivos JSON de partidas guardadas.
///
/// Partidas 4096×4096 con miles de vehículos/estaciones: ~10–50 MB en JSON.
/// Este límite (100 MB) cubre casos reales y previene agotamiento de memoria.
const MAX_JSON_SAVE_BYTES: u64 = 100 * 1024 * 1024;

/// Contenedor en disco: una sola versión de esquema por ahora; migraciones futuras leen `version`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct GameStateFile {
    pub version: u32,
    pub state: GameState,
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
    // Verificar tamaño del archivo antes de leerlo
    let metadata = std::fs::metadata(path)?;
    let file_size = metadata.len();
    if file_size > MAX_JSON_SAVE_BYTES {
        return Err(SaveError::JsonSizeExceeded {
            actual: file_size,
            limit: MAX_JSON_SAVE_BYTES,
        });
    }
    let text = std::fs::read_to_string(path)?;
    load_from_str(&text)
}

/// Igual que [`load`] pero desde memoria (p. ej. `OTTDJSON_LOAD`).
///
/// # Errors
///
/// Ver [`load`].
pub fn load_from_str(text: &str) -> Result<GameState, SaveError> {
    // Verificar tamaño del texto antes de parsear
    let text_size = text.len() as u64;
    if text_size > MAX_JSON_SAVE_BYTES {
        return Err(SaveError::JsonSizeExceeded {
            actual: text_size,
            limit: MAX_JSON_SAVE_BYTES,
        });
    }
    let v: serde_json::Value = serde_json::from_str(text)?;
    if v.get("version").is_some() && v.get("state").is_some() {
        let file: GameStateFile = serde_json::from_value(v)?;
        migrate_loaded_state(file.version, file.state)
    } else {
        let mut state = GameState::load_json(text)?;
        crate::command::normalize_synthetic_rail_crossings(&mut state.map);
        state.map.migrate_legacy_clear_grass_m5();
        state.rebuild_station_flows();
        state.sanitize_all_vehicle_orders();
        Ok(state)
    }
}
