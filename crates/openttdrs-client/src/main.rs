//! Cliente isométrico: sprites de `OpenGFX` + gizmos de overlay para el [`openttdrs_core::GameState`] del core.
//!
//! Para cargar un mapa real de `OpenTTD`, exportar con `scripts/parse_sav.py` y
//! luego ejecutar el cliente con la variable de entorno:
//!
//! ```text
//! OTTDMAP_FILE=/ruta/al/mapa.ottdmap cargo run -p openttdrs-client
//! Mapas ≥ 1024 teselas (32×32+): culling por viewport (panear regenera solo la ventana visible).
//! Umbral: `OPENTTDRS_MAP_VIEWPORT_THRESHOLD` (256–65536, default 1024).
//! Desactivar: OTTDMAP_FILE=… OPENTTDRS_MAP_VIEWPORT_OFF=1 cargo run -p openttdrs-client
//! ```
//!
//! Persistencia JSON (`openttdrs_core::save`, versión + `state` o legado plano):
//! `OTTDJSON_LOAD=/ruta/estado.json` al arranque (falla con mensaje si el path/JSON es inválido;
//! no cae a partida procedural), o **F5** / **Ctrl+S** para guardar y **F9** / **Ctrl+L** para
//! cargar (archivo por defecto `save/openttdrs_sim.json`, o `OPENTTDRS_JSON_SAVE`). Tras cargar se
//! redibuja todo el mapa y se reajusta la cámara (también si cambia el tamaño del mapa en el JSON).
//! Para importar un save nativo sin pasar por el selector: `OPENTTDRS_SAV_LOAD=/ruta/partida.sav`.
//! **P** pausa el tick de simulación; **F4** alterna la ruta de guardado entre `save/openttdrs_sim.json` y
//! `save/openttdrs_autosave.json` (visible en el HUD). **Clic en el mapa** selecciona tesela; **panel Construir**
//! (esquina inferior izquierda) aplica carretera / estación en esa tesela.
//! Bases de sprites de señal: `OPENTTDRS_SIGNAL_BASE` / `OPENTTDRS_SIGNAL_ALT_BASE` (512–8192).
//!
//! Red (I8 / #21): `--server [HOST:PORT]` listen-server, `--client HOST[:PORT]`,
//! o `cargo run -p openttdrs-net --bin openttdrs-dedicated`. Ver `docs/adr/0001-multiplayer-v1.md`.

#![allow(clippy::needless_pass_by_value)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::default_constructed_unit_structs)]
// Queries Bevy con filtros (With/Without) suelen disparar type_complexity sin aportar claridad.
#![allow(clippy::type_complexity)]

mod app_icon;
mod audio;
mod bevy_app;
mod camera;
mod config;
mod debug_gizmos;
mod i18n;
mod iso;
mod news_prefs;
mod persistence;
mod render;
mod render_trace;
mod settings;
mod simulation;
mod sprites;
mod startup;
mod state;
#[cfg(target_os = "linux")]
mod tray;
mod ui;
mod window_status;

#[cfg(test)]
mod client_coverage_test;

use audio::warn_missing_optional_assets;
use network::parse_net_cli;
use startup::check_required_assets;
use std::path::{Path, PathBuf};

mod network;

fn main() {
    let asset_root = resolve_asset_root();
    let asset_root_text = asset_root.to_string_lossy();
    if !check_required_assets(&asset_root_text) {
        return;
    }
    warn_missing_optional_assets(&asset_root);
    if std::env::args_os().any(|arg| arg == "--check-assets") {
        println!("Assets OK: {}", asset_root.display());
        return;
    }

    match network::parse_handshake_smoke(std::env::args()) {
        Ok(Some(addr)) => {
            if let Err(error) = network::run_handshake_smoke(&addr) {
                eprintln!("error: {error}");
                std::process::exit(1);
            }
            return;
        }
        Ok(None) => {}
        Err(error) => {
            eprintln!("error: {error}");
            std::process::exit(2);
        }
    }

    let net = parse_net_cli(std::env::args());
    bevy_app::run(&asset_root_text, net);
}

fn resolve_asset_root() -> PathBuf {
    let override_path = std::env::var_os("OPENTTDRS_ASSET_ROOT").map(PathBuf::from);
    let executable_dir = std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(Path::to_path_buf));
    let current_dir = std::env::current_dir().ok();
    select_asset_root(
        override_path,
        executable_dir,
        current_dir,
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."),
    )
}

fn select_asset_root(
    override_path: Option<PathBuf>,
    executable_dir: Option<PathBuf>,
    current_dir: Option<PathBuf>,
    development_root: PathBuf,
) -> PathBuf {
    let fallback = override_path
        .clone()
        .or_else(|| executable_dir.clone())
        .or_else(|| current_dir.clone())
        .unwrap_or_else(|| development_root.clone());
    [
        override_path,
        executable_dir,
        current_dir,
        Some(development_root),
    ]
    .into_iter()
    .flatten()
    .find(|candidate| asset_layout_present(candidate))
    .unwrap_or(fallback)
}

fn asset_layout_present(root: &Path) -> bool {
    root.join("assets").is_dir() && root.join("static/fonts/DejaVuSansMono.ttf").is_file()
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod asset_root_tests {
    use super::select_asset_root;
    use std::fs;

    fn make_asset_root(dir: &std::path::Path) {
        fs::create_dir_all(dir.join("assets")).expect("assets");
        fs::create_dir_all(dir.join("static/fonts")).expect("fonts");
        fs::write(dir.join("static/fonts/DejaVuSansMono.ttf"), b"font").expect("font");
    }

    #[test]
    fn packaged_assets_next_to_executable_beat_development_checkout() {
        let temp = tempfile::tempdir().expect("tempdir");
        let packaged = temp.path().join("package");
        let development = temp.path().join("checkout");
        make_asset_root(&packaged);
        make_asset_root(&development);

        let selected = select_asset_root(None, Some(packaged.clone()), None, development);
        assert_eq!(selected, packaged);
    }

    #[test]
    fn explicit_asset_root_has_highest_priority() {
        let temp = tempfile::tempdir().expect("tempdir");
        let explicit = temp.path().join("explicit");
        let packaged = temp.path().join("package");
        make_asset_root(&explicit);
        make_asset_root(&packaged);

        let selected = select_asset_root(
            Some(explicit.clone()),
            Some(packaged),
            None,
            temp.path().join("checkout"),
        );
        assert_eq!(selected, explicit);
    }
}
