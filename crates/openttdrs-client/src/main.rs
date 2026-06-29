//! Cliente isométrico: sprites de `OpenGFX` + gizmos de overlay para el [`GameState`] del core.
//!
//! Para cargar un mapa real de `OpenTTD`, exportar con `scripts/parse_sav.py` y
//! luego ejecutar el cliente con la variable de entorno:
//!
//! ```
//! OTTDMAP_FILE=/ruta/al/mapa.ottdmap cargo run -p openttdrs-client
//! Mapas ≥ 1024 teselas (32×32+): culling por viewport (panear regenera solo la ventana visible).
//! Umbral: `OPENTTDRS_MAP_VIEWPORT_THRESHOLD` (256–65536, default 1024).
//! Desactivar: OTTDMAP_FILE=… OPENTTDRS_MAP_VIEWPORT_OFF=1 cargo run -p openttdrs-client
//! ```
//!
//! Persistencia JSON (`openttdrs_core::save`, versión + `state` o legado plano):
//! `OTTDJSON_LOAD=/ruta/estado.json` al arranque, o **F5** / **Ctrl+S** para guardar y **F9** / **Ctrl+L** para
//! cargar (archivo por defecto `save/openttdrs_sim.json`, o `OPENTTDRS_JSON_SAVE`). Tras cargar se
//! redibuja todo el mapa y se reajusta la cámara (también si cambia el tamaño del mapa en el JSON).
//! **P** pausa el tick de simulación; **F4** alterna la ruta de guardado entre `save/openttdrs_sim.json` y
//! `save/openttdrs_autosave.json` (visible en el HUD). **Clic en el mapa** selecciona tesela; **panel Construir**
//! (esquina inferior izquierda) aplica carretera / estación en esa tesela.
//! Bases de sprites de señal: `OPENTTDRS_SIGNAL_BASE` / `OPENTTDRS_SIGNAL_ALT_BASE` (512–4096).

#![allow(clippy::needless_pass_by_value)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::default_constructed_unit_structs)]
// Queries Bevy con filtros (With/Without) suelen disparar type_complexity sin aportar claridad.
#![allow(clippy::type_complexity)]

mod app_icon;
mod bevy_app;
mod camera;
mod config;
mod debug_gizmos;
mod iso;
mod news_prefs;
mod persistence;
mod render;
mod settings;
mod simulation;
mod sprites;
mod startup;
mod state;
mod ui;
mod window_status;

#[cfg(test)]
mod client_coverage_test;

use startup::check_required_assets;

fn main() {
    let repo_root = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");
    if !check_required_assets(repo_root) {
        return;
    }

    bevy_app::run(repo_root);
}
