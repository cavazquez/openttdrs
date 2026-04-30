//! Cliente isométrico: sprites de `OpenGFX` + gizmos de overlay para el [`GameState`] del core.
//!
//! Para cargar un mapa real de `OpenTTD`, exportar con `scripts/parse_sav.py` y
//! luego ejecutar el cliente con la variable de entorno:
//!
//! ```
//! OTTDMAP_FILE=/ruta/al/mapa.ottdmap cargo run -p openttdrs-client
//! ```
//!
//! Persistencia JSON (`openttdrs_core::save`, versión + `state` o legado plano):
//! `OTTDJSON_LOAD=/ruta/estado.json` al arranque, o **F5** / **Ctrl+S** para guardar y **F9** / **Ctrl+L** para
//! cargar (archivo por defecto `openttdrs_sim.json`, o `OPENTTDRS_JSON_SAVE`). Tras cargar se
//! redibuja todo el mapa y se reajusta la cámara (también si cambia el tamaño del mapa en el JSON).
//! **P** pausa el tick de simulación; **F4** alterna la ruta de guardado entre `openttdrs_sim.json` y
//! `openttdrs_autosave.json` (visible en el HUD). **Clic en el mapa** selecciona tesela; **panel Construir**
//! (esquina inferior izquierda) aplica carretera / estación en esa tesela.
//! Bases de sprites de señal: `OPENTTDRS_SIGNAL_BASE` / `OPENTTDRS_SIGNAL_ALT_BASE` (512–4096).

#![allow(clippy::needless_pass_by_value)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::default_constructed_unit_structs)]
// Queries Bevy con filtros (With/Without) suelen disparar type_complexity sin aportar claridad.
#![allow(clippy::type_complexity)]

mod bevy_app;
mod camera;
mod config;
mod debug_gizmos;
mod iso;
mod persistence;
mod render;
mod simulation;
mod sprites;
mod state;
mod ui;
mod window_status;

#[cfg(test)]
mod client_coverage_test;

use std::path::Path;

fn main() {
    let asset_root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets");
    if !check_required_assets(asset_root) {
        return;
    }

    bevy_app::run(asset_root);
}

fn check_required_assets(asset_root: &str) -> bool {
    let tiles_dir = Path::new(asset_root).join("opengfx/tiles");
    let required = [
        tiles_dir.join("grass.png"),
        tiles_dir.join("water.png"),
        tiles_dir.join("vehicle_bus_sw.png"),
    ];

    let missing: Vec<String> = required
        .iter()
        .filter(|p| !p.is_file())
        .map(|p| p.display().to_string())
        .collect();

    if missing.is_empty() {
        return true;
    }

    eprintln!(
        "No se encontraron assets OpenGFX requeridos. Faltan {} archivos.",
        missing.len()
    );
    for path in &missing {
        eprintln!("Archivo faltante: {path}");
    }
    eprintln!("Genera los assets con: ./scripts/descargar_graficos.sh");
    false
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod main_asset_checks {
    use super::check_required_assets;
    use std::fs;

    #[test]
    fn check_required_assets_fails_when_missing() {
        let dir = tempfile::tempdir().expect("tempdir");
        assert!(!check_required_assets(dir.path().to_str().unwrap()));
    }

    #[test]
    fn check_required_assets_ok_with_min_pngs() {
        let dir = tempfile::tempdir().expect("tempdir");
        let t = dir.path().join("opengfx/tiles");
        fs::create_dir_all(&t).expect("mkdir");
        let png = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/one_pixel.png"
        ));
        for name in ["grass.png", "water.png", "vehicle_bus_sw.png"] {
            fs::write(t.join(name), png).expect("write");
        }
        assert!(check_required_assets(dir.path().to_str().unwrap()));
    }
}
