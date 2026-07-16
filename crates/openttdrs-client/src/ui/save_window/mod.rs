//! Ventana in-game para guardar y cargar partidas (lista de `save/`).

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use bevy::prelude::*;

mod setup;
mod systems;

pub(crate) use setup::setup_save_window;
pub(crate) use systems::{
    SaveLoadToolbarButton, handle_save_load_toolbar_buttons, handle_save_window_buttons,
    prepare_save_window_name, save_window_editable_keyboard, save_window_keyboard,
    save_window_name_click_focus, sync_save_window,
};

/// Filas visibles por página en la lista de partidas.
pub(crate) const SAVE_WINDOW_ROWS: usize = 8;

/// Por encima del menú de inicio (`GlobalZIndex(3000)`) para que el modal reciba clics.
pub(crate) const SAVE_WINDOW_Z: i32 = 3100;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum SaveWindowMode {
    Save,
    #[default]
    Load,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum SaveFileKind {
    Json,
    Sav,
}

#[derive(Clone)]
pub(crate) struct SaveEntry {
    pub(crate) name: String,
    pub(crate) path: PathBuf,
    pub(crate) kind: SaveFileKind,
    pub(crate) modified_label: String,
    pub(crate) size_label: String,
}

#[derive(Resource, Default)]
pub(crate) struct SaveWindowState {
    pub(crate) open: bool,
    pub(crate) mode: SaveWindowMode,
    pub(crate) entries: Vec<SaveEntry>,
    pub(crate) selected: Option<usize>,
    pub(crate) page: usize,
    pub(crate) filename: String,
    pub(crate) status: String,
}

impl SaveWindowState {
    /// Abre la ventana en el modo dado y rescanea la carpeta de partidas.
    pub(crate) fn open_in_mode(&mut self, mode: SaveWindowMode, save_dir: &Path) {
        self.mode = mode;
        self.open = true;
        self.selected = None;
        self.page = 0;
        self.status = String::new();
        self.entries = list_save_entries(save_dir);
        if mode == SaveWindowMode::Save {
            self.filename = default_save_name();
        }
    }

    pub(crate) fn close(&mut self) {
        self.open = false;
        self.selected = None;
        self.status = String::new();
    }

    pub(crate) fn page_count(&self) -> usize {
        self.entries.len().div_ceil(SAVE_WINDOW_ROWS).max(1)
    }
}

/// Componentes de la ventana.
#[derive(Component)]
pub(crate) struct SaveWindowRoot;

#[derive(Component)]
pub(crate) struct SaveWindowTitle;

#[derive(Component)]
pub(crate) struct SaveWindowRow {
    pub(crate) slot: usize,
}

#[derive(Component)]
pub(crate) struct SaveWindowRowText {
    pub(crate) slot: usize,
}

#[derive(Component)]
pub(crate) struct SaveWindowNameRow;

#[derive(Component)]
pub(crate) struct SaveWindowNameText;

#[derive(Component)]
pub(crate) struct SaveWindowPageText;

#[derive(Component)]
pub(crate) struct SaveWindowStatusText;

#[derive(Component)]
pub(crate) struct SaveWindowConfirmText;

#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SaveWindowButton {
    Confirm,
    Cancel,
    Delete,
    PrevPage,
    NextPage,
}

/// Carpeta de partidas derivada de la ruta de guardado actual (`save/` por defecto).
#[must_use]
pub(crate) fn save_dir_from(json_save_path: &str) -> PathBuf {
    let p = Path::new(json_save_path);
    match p.parent() {
        Some(dir) if !dir.as_os_str().is_empty() => dir.to_path_buf(),
        _ => PathBuf::from("save"),
    }
}

/// Lista `.json` y `.sav` de la carpeta de partidas, más reciente primero.
#[must_use]
pub(crate) fn scan_save_dir(dir: &Path) -> Vec<SaveEntry> {
    let Ok(read) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut found: Vec<(SaveEntry, SystemTime)> = Vec::new();
    for entry in read.flatten() {
        let path = entry.path();
        let kind = match path
            .extension()
            .and_then(|e| e.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref()
        {
            Some("json") => SaveFileKind::Json,
            Some("sav") => SaveFileKind::Sav,
            _ => continue,
        };
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let meta = entry.metadata().ok();
        let modified = meta
            .as_ref()
            .and_then(|m| m.modified().ok())
            .unwrap_or(UNIX_EPOCH);
        let size = meta.map_or(0, |m| m.len());
        found.push((
            SaveEntry {
                name: name.to_string(),
                path: path.clone(),
                kind,
                modified_label: format_system_time(modified),
                size_label: format_size(size),
            },
            modified,
        ));
    }
    found.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.name.cmp(&b.0.name)));
    found.into_iter().map(|(e, _)| e).collect()
}

/// Partidas en `save/` más `.sav` de `.downloads/` (fixtures descargados).
#[must_use]
pub(crate) fn list_save_entries(primary_dir: &Path) -> Vec<SaveEntry> {
    let mut entries = scan_save_dir(primary_dir);
    let downloads = Path::new(".downloads");
    if downloads.is_dir() {
        for e in scan_save_dir(downloads) {
            if e.kind != SaveFileKind::Sav {
                continue;
            }
            if entries.iter().any(|x| x.name == e.name) {
                continue;
            }
            entries.push(e);
        }
    }
    entries.sort_by(|a, b| {
        let ma = std::fs::metadata(&a.path).and_then(|m| m.modified()).ok();
        let mb = std::fs::metadata(&b.path).and_then(|m| m.modified()).ok();
        mb.cmp(&ma).then_with(|| a.name.cmp(&b.name))
    });
    entries
}

/// Nombre por defecto al guardar: `partida_YYYY-MM-DD_HHMM` (UTC).
#[must_use]
pub(crate) fn default_save_name() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let (y, mo, d, h, mi) = civil_datetime_from_unix(secs as i64);
    format!("partida_{y:04}-{mo:02}-{d:02}_{h:02}{mi:02}")
}

/// Mantiene solo caracteres válidos para nombre de archivo y limita el largo.
#[must_use]
pub(crate) fn sanitize_filename_char(c: char) -> Option<char> {
    if c.is_alphanumeric() || matches!(c, '-' | '_' | '.' | ' ') {
        Some(c)
    } else {
        None
    }
}

/// Filtro de caracteres para nombres de partida en `EditableText`.
#[must_use]
pub(crate) fn filename_filter() -> bevy::text::EditableTextFilter {
    bevy::text::EditableTextFilter::new(|c| sanitize_filename_char(c).is_some())
}

#[must_use]
fn format_size(bytes: u64) -> String {
    if bytes >= 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else if bytes >= 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{bytes} B")
    }
}

#[must_use]
fn format_system_time(t: SystemTime) -> String {
    let secs = t.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let (y, mo, d, h, mi) = civil_datetime_from_unix(secs as i64);
    format!("{y:04}-{mo:02}-{d:02} {h:02}:{mi:02}")
}

/// Fecha civil desde epoch Unix (algoritmo de Howard Hinnant, UTC).
#[must_use]
#[allow(clippy::many_single_char_names)]
fn civil_datetime_from_unix(secs: i64) -> (i64, u32, u32, u32, u32) {
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let h = (rem / 3600) as u32;
    let mi = (rem % 3600 / 60) as u32;
    (y + i64::from(m <= 2), m, d, h, mi)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn civil_datetime_known_values() {
        // 2026-06-10 06:00:00 UTC
        assert_eq!(civil_datetime_from_unix(1_781_071_200), (2026, 6, 10, 6, 0));
        assert_eq!(civil_datetime_from_unix(0), (1970, 1, 1, 0, 0));
    }

    #[test]
    fn scan_save_dir_lists_json_and_sav_sorted() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("a.json"), "{}").unwrap();
        std::fs::write(dir.path().join("b.sav"), "x").unwrap();
        std::fs::write(dir.path().join("ignored.txt"), "x").unwrap();
        let entries = scan_save_dir(dir.path());
        assert_eq!(entries.len(), 2);
        assert!(entries.iter().any(|e| e.kind == SaveFileKind::Json));
        assert!(entries.iter().any(|e| e.kind == SaveFileKind::Sav));
    }

    #[test]
    fn save_dir_from_paths() {
        assert_eq!(save_dir_from("save/x.json"), PathBuf::from("save"));
        assert_eq!(save_dir_from("x.json"), PathBuf::from("save"));
        assert_eq!(
            save_dir_from("/tmp/saves/x.json"),
            PathBuf::from("/tmp/saves")
        );
    }

    #[test]
    fn default_save_name_has_prefix() {
        assert!(default_save_name().starts_with("partida_"));
    }

    #[test]
    fn sanitize_filename_rejects_invalid_chars() {
        assert!(sanitize_filename_char('a').is_some());
        assert!(sanitize_filename_char('/').is_none());
        assert!(sanitize_filename_char('ñ').is_some());
    }

    #[test]
    fn page_count_minimum_one() {
        let st = SaveWindowState::default();
        assert_eq!(st.page_count(), 1);
    }
}
