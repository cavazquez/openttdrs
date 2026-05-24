//! Comprobación de assets OpenGFX mínimos antes de arrancar Bevy.

use std::path::Path;

const VEHICLE_PNGS: &[&str] = &[
    "vehicle_bus_n.png",
    "vehicle_bus_ne.png",
    "vehicle_bus_e.png",
    "vehicle_bus_se.png",
    "vehicle_bus_s.png",
    "vehicle_bus_sw.png",
    "vehicle_bus_w.png",
    "vehicle_bus_nw.png",
    "vehicle_truck_n.png",
    "vehicle_truck_ne.png",
    "vehicle_truck_e.png",
    "vehicle_truck_se.png",
    "vehicle_truck_s.png",
    "vehicle_truck_sw.png",
    "vehicle_truck_w.png",
    "vehicle_truck_nw.png",
    "vehicle_truck_n_loaded.png",
    "vehicle_truck_ne_loaded.png",
    "vehicle_truck_e_loaded.png",
    "vehicle_truck_se_loaded.png",
    "vehicle_truck_s_loaded.png",
    "vehicle_truck_sw_loaded.png",
    "vehicle_truck_w_loaded.png",
    "vehicle_truck_nw_loaded.png",
    "vehicle_train_n.png",
    "vehicle_train_ne.png",
    "vehicle_train_e.png",
    "vehicle_train_se.png",
    "vehicle_train_s.png",
    "vehicle_train_sw.png",
    "vehicle_train_w.png",
    "vehicle_train_nw.png",
];

/// Devuelve `false` si faltan PNG/fuentes requeridos (el binario debe salir sin iniciar Bevy).
#[must_use]
pub fn check_required_assets(repo_root: &str) -> bool {
    let root = Path::new(repo_root);
    let tiles_dir = root.join("assets/opengfx/tiles");
    let mut required: Vec<_> = vec![
        tiles_dir.join("grass.png"),
        tiles_dir.join("water.png"),
        root.join("static/fonts/DejaVuSansMono.ttf"),
    ];
    required.extend(VEHICLE_PNGS.iter().map(|name| tiles_dir.join(name)));

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
mod tests {
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
        let t = dir.path().join("assets/opengfx/tiles");
        let f = dir.path().join("static/fonts");
        fs::create_dir_all(&t).expect("mkdir");
        fs::create_dir_all(&f).expect("mkdir");
        let png = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/one_pixel.png"
        ));
        for name in [
            "grass.png",
            "water.png",
            "vehicle_bus_n.png",
            "vehicle_bus_ne.png",
            "vehicle_bus_e.png",
            "vehicle_bus_se.png",
            "vehicle_bus_s.png",
            "vehicle_bus_sw.png",
            "vehicle_bus_w.png",
            "vehicle_bus_nw.png",
            "vehicle_truck_n.png",
            "vehicle_truck_ne.png",
            "vehicle_truck_e.png",
            "vehicle_truck_se.png",
            "vehicle_truck_s.png",
            "vehicle_truck_sw.png",
            "vehicle_truck_w.png",
            "vehicle_truck_nw.png",
            "vehicle_truck_n_loaded.png",
            "vehicle_truck_ne_loaded.png",
            "vehicle_truck_e_loaded.png",
            "vehicle_truck_se_loaded.png",
            "vehicle_truck_s_loaded.png",
            "vehicle_truck_sw_loaded.png",
            "vehicle_truck_w_loaded.png",
            "vehicle_truck_nw_loaded.png",
            "vehicle_train_n.png",
            "vehicle_train_ne.png",
            "vehicle_train_e.png",
            "vehicle_train_se.png",
            "vehicle_train_s.png",
            "vehicle_train_sw.png",
            "vehicle_train_w.png",
            "vehicle_train_nw.png",
        ] {
            fs::write(t.join(name), png).expect("write");
        }
        fs::write(f.join("DejaVuSansMono.ttf"), []).expect("write");
        assert!(check_required_assets(dir.path().to_str().unwrap()));
    }
}
