//! Varias actualizaciones de la app Bevy en modo headless para subir cobertura de líneas en CI
//! (`cargo llvm-cov`) sin abrir ventana.

#![allow(clippy::expect_used)]

use std::fs;

fn seed_min_assets(root: &std::path::Path) {
    let tiles = root.join("assets/opengfx/tiles");
    fs::create_dir_all(&tiles).expect("mkdir tiles");
    let png = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/one_pixel.png"
    ));
    for name in [
        "grass.png",
        "water.png",
        "vehicle_bus_ne.png",
        "vehicle_bus_se.png",
        "vehicle_bus_sw.png",
        "vehicle_bus_nw.png",
        "vehicle_truck_ne.png",
        "vehicle_truck_se.png",
        "vehicle_truck_sw.png",
        "vehicle_truck_nw.png",
        "vehicle_truck_ne_loaded.png",
        "vehicle_truck_se_loaded.png",
        "vehicle_truck_sw_loaded.png",
        "vehicle_truck_nw_loaded.png",
    ] {
        fs::write(tiles.join(name), png).expect("write png");
    }
}

#[test]
fn headless_client_build_registers_plugins_for_coverage() {
    let dir = tempfile::tempdir().expect("tempdir");
    seed_min_assets(dir.path());
    let root = dir.path().to_str().expect("utf8 temp");
    // Sin `app.update()`: el subapp de render exige recursos de ventana/extractor que no existen
    // sin `WinitPlugin`; el registro de plugins y `Plugin::build` ya corre en `add_plugins`.
    let _app = crate::bevy_app::build_client_app(root, true);
}
