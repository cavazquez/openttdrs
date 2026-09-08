//! Comprobación y preparación de los assets OpenGFX antes de arrancar Bevy.
//!
//! El atlas versionado contiene todos los sprites usados por el cliente. Los
//! PNG individuales son un derivado local: se materializan automáticamente la
//! primera vez que se ejecuta el cliente desde un clone, sin descargar ni
//! invocar scripts auxiliares.

use image::{DynamicImage, ImageFormat};
use std::fs;
use std::path::{Path, PathBuf};

use crate::sprites::{TILE_ATLAS_NAMES, TILE_ATLAS_PAGE_COUNT, TILE_ATLAS_RECTS};

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

const TILE_DIRECTORY: &str = "assets/opengfx/tiles";
const ATLAS_DIRECTORY: &str = "assets/opengfx/atlas";
const ATLAS_DERIVATION_MARKER: &str = ".openttdrs-atlas-fnv64";

fn required_asset_paths(root: &Path) -> Vec<PathBuf> {
    let tiles_dir = root.join(TILE_DIRECTORY);
    let mut required = vec![
        tiles_dir.join("grass.png"),
        tiles_dir.join("water.png"),
        // El mapa se dibuja desde el atlas (gen_tile_atlas.py).
        root.join(ATLAS_DIRECTORY).join("tiles_atlas_0.png"),
        root.join("static/fonts/DejaVuSansMono.ttf"),
    ];
    required.extend(VEHICLE_PNGS.iter().map(|name| tiles_dir.join(name)));
    required
}

fn missing_required_assets(root: &Path) -> Vec<PathBuf> {
    required_asset_paths(root)
        .into_iter()
        .filter(|path| !path.is_file())
        .collect()
}

/// Hash estable, deliberadamente simple, del atlas empaquetado.
///
/// No es un mecanismo de seguridad: sólo evita conservar PNGs del caché que
/// materializa el cliente si el usuario actualiza el checkout.
fn atlas_fingerprint(root: &Path) -> Result<String, String> {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for page in 0..TILE_ATLAS_PAGE_COUNT {
        let path = root
            .join(ATLAS_DIRECTORY)
            .join(format!("tiles_atlas_{page}.png"));
        let bytes = fs::read(&path)
            .map_err(|error| format!("no se pudo leer {}: {error}", path.display()))?;
        for byte in bytes {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    Ok(format!("{hash:016x}-{}", TILE_ATLAS_NAMES.len()))
}

fn output_path(tiles_dir: &Path, name: &str) -> Result<PathBuf, String> {
    let path = Path::new(name);
    if path.components().count() != 1 || path.file_name().is_none() {
        return Err(format!("nombre de sprite inseguro en el atlas: {name}"));
    }
    Ok(tiles_dir.join(path))
}

fn decode_atlas_pages(root: &Path) -> Result<Vec<DynamicImage>, String> {
    (0..TILE_ATLAS_PAGE_COUNT)
        .map(|page| {
            let path = root
                .join(ATLAS_DIRECTORY)
                .join(format!("tiles_atlas_{page}.png"));
            image::open(&path)
                .map_err(|error| format!("no se pudo decodificar {}: {error}", path.display()))
        })
        .collect()
}

fn write_tiles_from_atlas(
    tiles_dir: &Path,
    atlas_pages: &[DynamicImage],
    names: &[(&str, u32)],
    rects: &[(u16, u16, u16, u16, u16)],
) -> Result<usize, String> {
    fs::create_dir_all(tiles_dir).map_err(|error| {
        format!(
            "no se pudo crear el directorio de sprites {}: {error}",
            tiles_dir.display()
        )
    })?;

    for &(name, rect_index) in names {
        let &(page, x, y, width, height) = rects
            .get(rect_index as usize)
            .ok_or_else(|| format!("rect inválido {rect_index} para {name}"))?;
        let atlas = atlas_pages
            .get(usize::from(page))
            .ok_or_else(|| format!("página de atlas inválida {page} para {name}"))?;
        let right = u32::from(x) + u32::from(width);
        let bottom = u32::from(y) + u32::from(height);
        if right > atlas.width() || bottom > atlas.height() {
            return Err(format!(
                "rect fuera de la página {page} para {name}: {x},{y} {width}×{height} en {}×{}",
                atlas.width(),
                atlas.height()
            ));
        }

        let sprite = atlas.crop_imm(
            u32::from(x),
            u32::from(y),
            u32::from(width),
            u32::from(height),
        );
        let output = output_path(tiles_dir, name)?;
        sprite
            .save_with_format(&output, ImageFormat::Png)
            .map_err(|error| format!("no se pudo escribir {}: {error}", output.display()))?;
    }
    Ok(names.len())
}

fn materialize_bundled_tiles(root: &Path) -> Result<usize, String> {
    let fingerprint = atlas_fingerprint(root)?;
    let tiles_dir = root.join(TILE_DIRECTORY);
    let marker = tiles_dir.join(ATLAS_DERIVATION_MARKER);
    let up_to_date = fs::read_to_string(&marker)
        .ok()
        .is_some_and(|stored| stored.trim() == fingerprint)
        && TILE_ATLAS_NAMES
            .iter()
            .all(|(name, _)| output_path(&tiles_dir, name).is_ok_and(|path| path.is_file()));
    if up_to_date {
        return Ok(0);
    }

    let pages = decode_atlas_pages(root)?;
    let generated = write_tiles_from_atlas(&tiles_dir, &pages, TILE_ATLAS_NAMES, TILE_ATLAS_RECTS)?;
    fs::write(&marker, format!("{fingerprint}\n"))
        .map_err(|error| format!("no se pudo escribir {}: {error}", marker.display()))?;
    Ok(generated)
}

/// Devuelve `false` si faltan PNG/fuentes requeridos (el binario debe salir sin iniciar Bevy).
#[must_use]
pub fn check_required_assets(repo_root: &str) -> bool {
    let root = Path::new(repo_root);
    let tiles_dir = root.join(TILE_DIRECTORY);
    // Un árbol que ya contiene tiles sin nuestro marker puede ser un baseset
    // elegido por el mantenedor (por ejemplo 32bpp): no lo sobrescribimos.
    // Un clone nuevo, o un caché creado por este cliente, sí se completa y se
    // invalida correctamente cuando cambia el atlas versionado.
    let needs_materialization = !missing_required_assets(root).is_empty()
        || tiles_dir.join(ATLAS_DERIVATION_MARKER).is_file();
    if needs_materialization {
        match materialize_bundled_tiles(root) {
            Ok(0) => {}
            Ok(generated) => {
                eprintln!(
                    "Gráficos OpenGFX preparados desde el atlas incluido ({generated} sprites locales)."
                );
            }
            Err(error) => {
                eprintln!("No se pudieron preparar los gráficos OpenGFX incluidos: {error}");
            }
        }
    }

    let missing: Vec<String> = missing_required_assets(root)
        .into_iter()
        .map(|path| path.display().to_string())
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
    eprintln!(
        "El atlas incluido no pudo materializar sus sprites. Revisa que el checkout sea escribible y vuelve a ejecutar `cargo run`."
    );
    false
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::{
        ATLAS_DERIVATION_MARKER, TILE_DIRECTORY, check_required_assets, write_tiles_from_atlas,
    };
    use crate::sprites::TILE_ATLAS_NAMES;
    use image::{DynamicImage, Rgba, RgbaImage};
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
        let a = dir.path().join("assets/opengfx/atlas");
        let f = dir.path().join("static/fonts");
        fs::create_dir_all(&t).expect("mkdir");
        fs::create_dir_all(&a).expect("mkdir");
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
        fs::write(a.join("tiles_atlas_0.png"), png).expect("write");
        fs::write(f.join("DejaVuSansMono.ttf"), []).expect("write");
        assert!(check_required_assets(dir.path().to_str().unwrap()));
    }

    #[test]
    fn materializes_named_sprites_from_the_bundled_atlas() {
        let dir = tempfile::tempdir().expect("tempdir");
        let tiles = dir.path().join("tiles");
        let mut page = RgbaImage::new(3, 1);
        page.put_pixel(0, 0, Rgba([0x12, 0x34, 0x56, 0xff]));
        page.put_pixel(1, 0, Rgba([0xab, 0xcd, 0xef, 0xff]));
        let pages = [DynamicImage::ImageRgba8(page)];
        let names = [("grass.png", 0), ("water.png", 1)];
        let rects = [(0, 0, 0, 1, 1), (0, 1, 0, 1, 1)];

        assert_eq!(
            write_tiles_from_atlas(&tiles, &pages, &names, &rects).expect("materialize"),
            2
        );
        assert_eq!(
            image::open(tiles.join("grass.png"))
                .expect("grass")
                .to_rgba8()
                .get_pixel(0, 0),
            &Rgba([0x12, 0x34, 0x56, 0xff])
        );
        assert_eq!(
            image::open(tiles.join("water.png"))
                .expect("water")
                .to_rgba8()
                .get_pixel(0, 0),
            &Rgba([0xab, 0xcd, 0xef, 0xff])
        );
    }

    #[test]
    fn clean_checkout_bootstraps_all_runtime_tiles_from_the_versioned_atlas() {
        let dir = tempfile::tempdir().expect("tempdir");
        let atlas_dir = dir.path().join("assets/opengfx/atlas");
        let font_dir = dir.path().join("static/fonts");
        fs::create_dir_all(&atlas_dir).expect("atlas dir");
        fs::create_dir_all(&font_dir).expect("font dir");
        fs::copy(
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../assets/opengfx/atlas/tiles_atlas_0.png"
            ),
            atlas_dir.join("tiles_atlas_0.png"),
        )
        .expect("copy bundled atlas");
        fs::write(font_dir.join("DejaVuSansMono.ttf"), []).expect("font");

        assert!(check_required_assets(
            dir.path().to_str().expect("utf8 root")
        ));
        let tiles = dir.path().join(TILE_DIRECTORY);
        assert!(tiles.join(ATLAS_DERIVATION_MARKER).is_file());
        for name in [
            "grass.png",
            "water.png",
            "vehicle_bus_n.png",
            "vehicle_train_e.png",
            "toolbar_pause.png",
        ] {
            assert!(tiles.join(name).is_file(), "missing generated {name}");
        }
        assert!(
            TILE_ATLAS_NAMES
                .iter()
                .all(|(name, _)| tiles.join(name).is_file())
        );
    }
}
