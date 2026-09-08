//! Operaciones de E/S para persistencia en disco del [`GameState`].

use std::io::{self, Write};
use std::path::Path;

use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;

use crate::GameState;

use super::migrate::migrate_loaded_state;
use super::{SAVE_VERSION, SaveError};

/// Límite de bytes para archivos JSON de partidas guardadas.
///
/// El writer y los dos loaders aplican esta misma cuota. Antes de reemplazar
/// un destino, el writer cuenta los bytes que produciría exactamente el
/// serializador pretty; por eso un JSON guardado con éxito no será rechazado
/// luego sólo por este límite.
///
/// La cuota protege la memoria del parser; no pretende expresar la capacidad
/// de todos los tamaños de mapa ni se aumenta sin medir ese coste.
pub(super) const MAX_JSON_SAVE_BYTES: u64 = 100 * 1024 * 1024;

/// Contenedor en disco: una sola versión de esquema por ahora; migraciones futuras leen `version`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct GameStateFile {
    pub version: u32,
    pub state: GameState,
}

/// Vista prestada del contenedor versionado al serializar una partida nueva.
///
/// Mantiene el wire format de [`GameStateFile`] sin clonar todo el estado sólo
/// para medirlo o escribirlo.
#[derive(Serialize)]
struct GameStateFileRef<'a> {
    version: u32,
    state: &'a GameState,
}

/// Writer que descarta los bytes pero conserva su cantidad exacta.
#[derive(Default)]
struct ByteCounter {
    bytes: u64,
}

impl Write for ByteCounter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let added = u64::try_from(buffer.len())
            .map_err(|_| io::Error::other("el fragmento JSON no cabe en u64"))?;
        self.bytes = self
            .bytes
            .checked_add(added)
            .ok_or_else(|| io::Error::other("el JSON supera el contador de tamaño"))?;
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn versioned_file(state: &GameState) -> GameStateFileRef<'_> {
    GameStateFileRef {
        version: SAVE_VERSION,
        state,
    }
}

fn serialized_json_size(file: &impl Serialize) -> Result<u64, SaveError> {
    let mut counter = ByteCounter::default();
    serde_json::to_writer_pretty(&mut counter, file)?;
    Ok(counter.bytes)
}

fn ensure_json_size(actual: u64, limit: u64) -> Result<(), SaveError> {
    if actual > limit {
        return Err(SaveError::JsonSizeExceeded { actual, limit });
    }
    Ok(())
}

/// Escribe bytes en un temporal hermano y reemplaza `path` sólo al terminar.
///
/// El destino anterior nunca se abre para truncarlo. Si la serialización, la
/// escritura, el `sync_all` o el reemplazo fallan, el temporal se elimina al
/// salir y el archivo previo sigue siendo el destino visible.
///
/// Se comparte con el exportador SAV para que los dos formatos tengan la misma
/// garantía frente a disco lleno, cuotas y errores parciales de E/S.
///
/// # Errors
///
/// Propaga el fallo de crear/escribir/sincronizar el temporal o de reemplazar
/// el destino.
pub(crate) fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    write_atomic_with(path, |file| file.write_all(bytes))
}

fn write_atomic_with<F>(path: &Path, write: F) -> io::Result<()>
where
    F: FnOnce(&mut std::fs::File) -> io::Result<()>,
{
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let existing_permissions = match std::fs::metadata(path) {
        Ok(metadata) => Some(metadata.permissions()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => return Err(error),
    };

    let mut temporary = NamedTempFile::new_in(parent)?;
    write(temporary.as_file_mut())?;
    if let Some(permissions) = existing_permissions {
        temporary.as_file().set_permissions(permissions)?;
    }
    temporary.as_file().sync_all()?;
    temporary
        .persist(path)
        .map(|_| ())
        .map_err(|error| error.error)
}

/// Escribe `state` en `path` como JSON formateado (versión + estado).
///
/// # Errors
///
/// Fallos de E/S o serialización.
pub fn save(state: &GameState, path: &Path) -> Result<(), SaveError> {
    save_with_limit(state, path, MAX_JSON_SAVE_BYTES)
}

/// Variante interna de [`save`] con una cuota explícita para probar fronteras.
///
/// Cuenta primero el mismo stream que escribirá después. Así, un rechazo por
/// tamaño no crea directorios ni abre/reemplaza el archivo de destino.
fn save_with_limit(state: &GameState, path: &Path, limit: u64) -> Result<(), SaveError> {
    let file = versioned_file(state);
    let actual = serialized_json_size(&file)?;
    ensure_json_size(actual, limit)?;

    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)?;
    }
    write_atomic_with(path, |temporary| {
        serde_json::to_writer_pretty(temporary, &file).map_err(io::Error::other)
    })?;
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
    ensure_json_size(metadata.len(), MAX_JSON_SAVE_BYTES)?;
    let text = std::fs::read_to_string(path)?;
    load_from_str(&text)
}

/// Igual que [`load`] pero desde memoria (p. ej. `OTTDJSON_LOAD`).
///
/// # Errors
///
/// Ver [`load`].
pub fn load_from_str(text: &str) -> Result<GameState, SaveError> {
    load_from_str_with_limit(text, MAX_JSON_SAVE_BYTES)
}

pub(super) fn load_from_str_with_limit(text: &str, limit: u64) -> Result<GameState, SaveError> {
    // Verificar tamaño del texto antes de parsear
    let text_size = text.len() as u64;
    ensure_json_size(text_size, limit)?;
    let v: serde_json::Value = serde_json::from_str(text)?;
    if v.get("version").is_some() && v.get("state").is_some() {
        let file: GameStateFile = serde_json::from_value(v)?;
        migrate_loaded_state(file.version, file.state)
    } else {
        let mut state = GameState::load_json(text)?;
        crate::command::normalize_synthetic_rail_crossings(&mut state.map);
        state.map.migrate_legacy_clear_grass_m5();
        state.ensure_timers_from_tick();
        state.rebuild_station_flows();
        state.sanitize_all_vehicle_orders();
        // JSON plano no lleva versión: tratarlo como anterior a v26 para no
        // borrar un `max_loan` individual al reconstruir el runtime.
        state.infer_legacy_company_max_loan_overrides();
        state.sync_scaled_max_loan();
        Ok(state)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::io::{self, Write};

    use crate::GameState;

    use super::{
        MAX_JSON_SAVE_BYTES, SaveError, load, load_from_str_with_limit, save, save_with_limit,
        serialized_json_size, versioned_file, write_atomic, write_atomic_with,
    };

    fn versioned_json_size(state: &GameState) -> u64 {
        serialized_json_size(&versioned_file(state)).unwrap()
    }

    #[test]
    fn failed_atomic_write_keeps_existing_contents_and_cleans_temporary() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("partida.json");
        std::fs::write(&path, b"previous save").unwrap();

        let error = write_atomic_with(&path, |file| {
            file.write_all(b"partially written replacement")?;
            Err(io::Error::other("injected write failure"))
        });

        assert!(error.is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"previous save");
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    #[test]
    fn successful_atomic_write_replaces_existing_contents() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("partida.json");
        std::fs::write(&path, b"previous save").unwrap();

        write_atomic(&path, b"complete replacement").unwrap();

        assert_eq!(std::fs::read(&path).unwrap(), b"complete replacement");
    }

    #[test]
    fn json_save_limit_accepts_exact_and_larger_quotas() {
        let state = GameState::new(4, 4);
        let size = versioned_json_size(&state);
        let directory = tempfile::tempdir().unwrap();
        let exact = directory.path().join("exact.json");
        let larger = directory.path().join("larger.json");

        save_with_limit(&state, &exact, size).unwrap();
        save_with_limit(&state, &larger, size + 1).unwrap();

        let exact_text = std::fs::read_to_string(&exact).unwrap();
        assert_eq!(std::fs::metadata(&exact).unwrap().len(), size);
        assert_eq!(std::fs::metadata(&larger).unwrap().len(), size);
        assert_eq!(
            load_from_str_with_limit(&exact_text, size)
                .unwrap()
                .map
                .dimensions(),
            state.map.dimensions()
        );
        assert_eq!(
            load_from_str_with_limit(&exact_text, size + 1)
                .unwrap()
                .map
                .dimensions(),
            state.map.dimensions()
        );
        assert_eq!(
            load(&exact).unwrap().map.dimensions(),
            state.map.dimensions()
        );
    }

    #[test]
    fn json_save_limit_rejects_before_replacing_destination() {
        let state = GameState::new(4, 4);
        let actual = versioned_json_size(&state);
        let limit = actual - 1;
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("partida.json");
        std::fs::write(&path, b"previous save").unwrap();

        let error = save_with_limit(&state, &path, limit).unwrap_err();

        assert!(matches!(
            error,
            SaveError::JsonSizeExceeded {
                actual: reported_actual,
                limit: reported_limit,
            } if reported_actual == actual && reported_limit == limit
        ));
        assert_eq!(std::fs::read(&path).unwrap(), b"previous save");
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);

        let new_path = directory.path().join("not-created").join("partida.json");
        assert!(save_with_limit(&state, &new_path, limit).is_err());
        assert!(!new_path.parent().unwrap().exists());
    }

    #[test]
    fn json_load_limit_uses_the_same_pre_exact_and_post_boundary() {
        let state = GameState::new(4, 4);
        let size = versioned_json_size(&state);
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("partida.json");
        save_with_limit(&state, &path, size).unwrap();
        let text = std::fs::read_to_string(path).unwrap();

        let error = load_from_str_with_limit(&text, size - 1).unwrap_err();
        assert!(matches!(
            error,
            SaveError::JsonSizeExceeded { actual, limit }
                if actual == size && limit == size - 1
        ));
        assert!(load_from_str_with_limit(&text, size).is_ok());
        assert!(load_from_str_with_limit(&text, size + 1).is_ok());
    }

    #[test]
    fn json_size_matrix_for_empty_maps_is_checked_without_output_allocation() {
        for side in [16, 64, 128, 256, 512] {
            let state = GameState::new(side, side);
            let actual = versioned_json_size(&state);
            eprintln!("JSON pretty versionado vacío {side}²: {actual} bytes");
            assert!(
                actual <= MAX_JSON_SAVE_BYTES,
                "{side}² debe caber en la cuota JSON actual"
            );
        }
    }

    #[test]
    #[ignore = "smoke de 1024²: cuenta el stream sin crear un JSON gigante"]
    fn json_1024_size_policy_rejects_before_replacing_destination() {
        let state = GameState::new(1024, 1024);
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("partida.json");
        std::fs::write(&path, b"previous save").unwrap();

        let error = save(&state, &path).unwrap_err();

        let SaveError::JsonSizeExceeded { actual, limit } = error else {
            panic!("un mapa 1024² vacío debe exceder la cuota JSON");
        };
        eprintln!("JSON pretty versionado vacío 1024²: {actual} bytes");
        assert_eq!(limit, MAX_JSON_SAVE_BYTES);
        assert!(actual > limit);
        assert_eq!(std::fs::read(&path).unwrap(), b"previous save");
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
    }
}
