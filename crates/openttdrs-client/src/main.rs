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
mod state_bootstrap;
mod state_stations;
mod ui;
mod vehicle_render;
mod window_status;
mod world_render;

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
