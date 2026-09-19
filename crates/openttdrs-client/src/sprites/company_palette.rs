//! Remapeo de paleta de compañía OpenTTD (`PALETTE_MODIFIER_COLOUR`).
//!
//! Los PNG 8bpp se hornean con la rampa autora `COLOUR_DARK_BLUE` (compañía
//! 0). Al perder los índices de paleta durante la conversión a RGBA sólo se
//! puede reconocer esa rampa exacta; inferir que cualquier RGB perteneciente
//! a *alguna* rampa es company-colour recoloreaba píxeles ordinarios de
//! OpenGFX (techos, plataformas y edificios).

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use image::RgbaImage;

#[path = "company_palette_data_generated.rs"]
mod generated;
#[path = "company_palette_paths_generated.rs"]
mod paths_generated;

use super::{TILE_ATLAS_NAMES, TILE_ATLAS_RECTS};
use generated::{COMPANY_COLOUR_COUNT, COMPANY_RAMP_RGB, COMPANY_RAMP_SHADES};
use paths_generated::COMPANY_PALETTE_STATIC_PATHS;

/// Color de compañía del jugador (`Colours` en OpenTTD).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum CompanyColour {
    #[default]
    DarkBlue = 0,
    PaleGreen = 1,
    Pink = 2,
    Yellow = 3,
    Red = 4,
    LightBlue = 5,
    Green = 6,
    DarkGreen = 7,
    Blue = 8,
    Cream = 9,
    Mauve = 10,
    Purple = 11,
    Orange = 12,
    Brown = 13,
    Grey = 14,
    White = 15,
}

impl CompanyColour {
    #[must_use]
    pub fn from_u8(v: u8) -> Self {
        match v % COMPANY_COLOUR_COUNT as u8 {
            0 => Self::DarkBlue,
            1 => Self::PaleGreen,
            2 => Self::Pink,
            3 => Self::Yellow,
            4 => Self::Red,
            5 => Self::LightBlue,
            6 => Self::Green,
            7 => Self::DarkGreen,
            8 => Self::Blue,
            9 => Self::Cream,
            10 => Self::Mauve,
            11 => Self::Purple,
            12 => Self::Orange,
            13 => Self::Brown,
            14 => Self::Grey,
            15 => Self::White,
            _ => unreachable!(),
        }
    }

    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }

    /// `PALETTE_CC_DARK_BLUE` es identidad con los PNG horneados actuales.
    #[must_use]
    pub const fn needs_recolor(self) -> bool {
        !matches!(self, Self::DarkBlue)
    }
}

#[inline]
fn ramp_index(colour: usize, shade: usize) -> usize {
    colour * COMPANY_RAMP_SHADES + shade
}

/// Tono medio de la rampa OpenTTD, representativo en la UI.
const SWATCH_SHADE: usize = 4;

/// Color RGB para muestra en el selector de compañía (tono ~medio de la rampa).
#[must_use]
pub fn company_colour_swatch_color(colour: u8) -> Color {
    let c = CompanyColour::from_u8(colour);
    let idx = ramp_index(c.as_u8() as usize, SWATCH_SHADE);
    let rgb = COMPANY_RAMP_RGB[idx];
    Color::srgb_u8(rgb[0], rgb[1], rgb[2])
}

/// Color de texto de compañía usado por `ViewportDrawStrings` para una
/// estación transparente (`GetColourGradient(..., SHADE_LIGHTER)`).
///
/// La rampa generada conserva el orden de tonos de OpenTTD: el tono 4 es el
/// representativo de la muestra y el 5 corresponde a `SHADE_LIGHTER`.
#[must_use]
pub fn company_colour_label_text_color(colour: u8) -> Color {
    let c = CompanyColour::from_u8(colour);
    let idx = ramp_index(c.as_u8() as usize, SWATCH_SHADE + 1);
    let rgb = COMPANY_RAMP_RGB[idx];
    Color::srgb_u8(rgb[0], rgb[1], rgb[2])
}

/// Nombre legible del color de compañía (16 colores OpenTTD).
const COMPANY_COLOUR_NAMES: [&str; 16] = [
    "Azul oscuro",
    "Verde pálido",
    "Rosa",
    "Amarillo",
    "Rojo",
    "Celeste",
    "Verde",
    "Verde oscuro",
    "Azul",
    "Crema",
    "Malva",
    "Púrpura",
    "Naranja",
    "Marrón",
    "Gris",
    "Blanco",
];

const COMPANY_COLOUR_TOOLTIPS: [&str; 16] = [
    "Color compañía: Azul oscuro (0)",
    "Color compañía: Verde pálido (1)",
    "Color compañía: Rosa (2)",
    "Color compañía: Amarillo (3)",
    "Color compañía: Rojo (4)",
    "Color compañía: Celeste (5)",
    "Color compañía: Verde (6)",
    "Color compañía: Verde oscuro (7)",
    "Color compañía: Azul (8)",
    "Color compañía: Crema (9)",
    "Color compañía: Malva (10)",
    "Color compañía: Púrpura (11)",
    "Color compañía: Naranja (12)",
    "Color compañía: Marrón (13)",
    "Color compañía: Gris (14)",
    "Color compañía: Blanco (15)",
];

#[must_use]
pub fn company_colour_name(colour: u8) -> &'static str {
    COMPANY_COLOUR_NAMES[colour as usize % COMPANY_COLOUR_COUNT]
}

#[must_use]
pub fn company_colour_tooltip(colour: u8) -> &'static str {
    COMPANY_COLOUR_TOOLTIPS[colour as usize % COMPANY_COLOUR_COUNT]
}

/// Tabla RGB → RGB para remapear la rampa autora a `target`.
///
/// OpenGFX codifica los píxeles `PALETTE_MODIFIER_COLOUR` de estos PNG con
/// `COLOUR_DARK_BLUE` (índices DOS `0xC6..=0xCD`). Las demás rampas son
/// destinos posibles, no detectores de píxeles que deban recolorearse.
#[must_use]
pub fn build_remap_table(target: CompanyColour) -> HashMap<[u8; 3], [u8; 3]> {
    let mut map = HashMap::new();
    let dst = target.as_u8() as usize;
    for shade in 0..COMPANY_RAMP_SHADES {
        let out = COMPANY_RAMP_RGB[ramp_index(dst, shade)];
        let key = COMPANY_RAMP_RGB[ramp_index(CompanyColour::DarkBlue.as_u8() as usize, shade)];
        map.insert(key, out);
    }
    map
}

/// Recolorea un buffer RGBA8 in-place.
pub fn recolor_rgba8(buf: &mut [u8], target: CompanyColour) {
    let table = remap_table_cached(target);
    let (pixels, _) = buf.as_chunks_mut::<4>();
    for px in pixels {
        if px[3] == 0 {
            continue;
        }
        let key = [px[0], px[1], px[2]];
        if let Some(&rgb) = table.get(&key) {
            px[0] = rgb[0];
            px[1] = rgb[1];
            px[2] = rgb[2];
        }
    }
}

fn remap_table_cached(target: CompanyColour) -> &'static HashMap<[u8; 3], [u8; 3]> {
    static TABLES: [OnceLock<HashMap<[u8; 3], [u8; 3]>>; COMPANY_COLOUR_COUNT] =
        [const { OnceLock::new() }; COMPANY_COLOUR_COUNT];
    TABLES[target.as_u8() as usize].get_or_init(|| build_remap_table(target))
}

#[must_use]
/// Comparte la selección de recursos del cliente (override/paquete/cwd/dev).
/// No debe retener la ruta de compilación en un ejecutable trasladado.
pub fn tiles_assets_dir() -> PathBuf {
    crate::resolve_asset_root().join("assets/opengfx/tiles")
}

/// Carga `filename` desde `assets/opengfx/tiles/`, recolorea y registra en Bevy.
pub fn load_recolored_png(
    filename: &str,
    target: CompanyColour,
    images: &mut Assets<Image>,
) -> Option<Handle<Image>> {
    let path = tiles_assets_dir().join(filename);
    load_recolored_png_path(&path, target, images)
}

pub fn load_recolored_png_path(
    path: &Path,
    target: CompanyColour,
    images: &mut Assets<Image>,
) -> Option<Handle<Image>> {
    let mut img = image::open(path).ok()?.into_rgba8();
    recolor_rgba8(img.as_mut(), target);
    Some(images.add(rgba_to_bevy_image(native_zoom_image_for_capture(img))))
}

/// Carga un sprite de tesela y hornea `PALETTE_TO_BARE_LAND` (`791`).
///
/// La mayoría de las instalaciones distribuye los PNG individuales, pero el
/// atlas es el único recurso obligatorio del paquete. Mantener el mismo
/// fallback que las paletas de casas/árboles evita perder la variante cuando
/// sólo existe `tiles_atlas_*.png`.
pub(crate) fn load_bare_land_png(
    filename: &str,
    tiles: &Path,
    pages: &mut HashMap<u16, Option<RgbaImage>>,
    images: &mut Assets<Image>,
) -> Option<Handle<Image>> {
    let img = load_tile_rgba(filename, tiles, pages)?;
    let (width, height) = img.dimensions();
    let sprite = openttdrs_core::DecodedSprite {
        width: u16::try_from(width).ok()?,
        height: u16::try_from(height).ok()?,
        x_offs: 0,
        y_offs: 0,
        rgba: img.as_raw().clone(),
        mask: Vec::new(),
    };
    let rgba = openttdrs_core::newgrf_sprites::bake_sprite_bare_land(&sprite)?;
    let img = RgbaImage::from_raw(width, height, rgba)?;
    Some(images.add(rgba_to_bevy_image(native_zoom_image_for_capture(img))))
}

/// Lee un PNG individual o recorta su entrada correspondiente del atlas.
fn load_tile_rgba(
    filename: &str,
    tiles: &Path,
    pages: &mut HashMap<u16, Option<RgbaImage>>,
) -> Option<RgbaImage> {
    if let Ok(img) = image::open(tiles.join(filename)) {
        return Some(img.into_rgba8());
    }
    let entry = TILE_ATLAS_NAMES
        .binary_search_by(|(name, _)| name.cmp(&filename))
        .ok()?;
    let &(page, x, y, width, height) = TILE_ATLAS_RECTS.get(TILE_ATLAS_NAMES[entry].1 as usize)?;
    let atlas_dir = tiles.parent()?.join("atlas");
    let img = pages
        .entry(page)
        .or_insert_with(|| {
            image::open(atlas_dir.join(format!("tiles_atlas_{page}.png")))
                .ok()
                .map(image::DynamicImage::into_rgba8)
        })
        .as_ref()?;
    let (x, y, width, height) = (
        u32::from(x),
        u32::from(y),
        u32::from(width),
        u32::from(height),
    );
    if x + width > img.width() || y + height > img.height() {
        return None;
    }
    Some(image::imageops::crop_imm(img, x, y, width, height).to_image())
}

#[must_use]
pub fn rgba_to_bevy_image(img: RgbaImage) -> Image {
    let (width, height) = img.dimensions();
    Image::new(
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        img.into_raw(),
        TextureFormat::Rgba8UnormSrgb,
        default(),
    )
}

/// Factor de reducción nativa para una captura fija, si corresponde.
///
/// OpenTTD sólo cambia el muestreo en los niveles enteros `Out2x`, `Out4x` y
/// `Out8x`. Mantener la decisión en una función pura permite que atlas,
/// sprites recoloreados y vistas NewGRF compartan exactamente el mismo
/// contrato sin activar variantes durante una partida interactiva.
#[must_use]
pub(crate) fn native_zoom_factor_for(capture_requested: bool, scale: Option<f32>) -> Option<u32> {
    if !capture_requested {
        return None;
    }
    match scale {
        Some(scale) if (scale - 2.0).abs() < f32::EPSILON => Some(2),
        Some(scale) if (scale - 4.0).abs() < f32::EPSILON => Some(4),
        Some(scale) if (scale - 8.0).abs() < f32::EPSILON => Some(8),
        _ => None,
    }
}

#[must_use]
pub(crate) fn native_zoom_factor_for_capture_env() -> Option<u32> {
    native_zoom_factor_for(
        std::env::var_os("OPENTTDRS_MAP_SHOT").is_some(),
        std::env::var("OPENTTDRS_MAP_SHOT_SCALE")
            .ok()
            .and_then(|raw| raw.parse::<f32>().ok()),
    )
}

/// Replica el muestreo del blitter `8bpp-simple` sobre una textura RGBA.
///
/// La textura resultante conserva sus dimensiones para no cambiar el ancla
/// NFO. Cada bloque de `zoom × zoom` repite el primer píxel de la raíz; con el
/// sampler nearest de Bevy, la reducción ortográfica elige esa misma muestra.
#[must_use]
pub(crate) fn native_zoom_variant_rgba8(image: RgbaImage, zoom: u32) -> RgbaImage {
    if !matches!(zoom, 2 | 4 | 8) {
        return image;
    }

    let (width, height) = image.dimensions();
    let mut variant = RgbaImage::new(width, height);
    let factor = zoom as usize;
    for y in (0..height).step_by(factor) {
        for x in (0..width).step_by(factor) {
            let colour = *image.get_pixel(x, y);
            for yy in y..(y + zoom).min(height) {
                for xx in x..(x + zoom).min(width) {
                    variant.put_pixel(xx, yy, colour);
                }
            }
        }
    }
    variant
}

/// Aplica la variante nativa sólo durante una captura fija.
#[must_use]
pub(crate) fn native_zoom_image_for_capture(image: RgbaImage) -> RgbaImage {
    match native_zoom_factor_for_capture_env() {
        Some(zoom) => native_zoom_variant_rgba8(image, zoom),
        None => image,
    }
}

/// Versión equivalente para los buffers RGBA producidos por el cache NewGRF.
#[must_use]
pub(crate) fn native_zoom_bytes_for_capture(rgba: Vec<u8>, width: u32, height: u32) -> Vec<u8> {
    let Some(zoom) = native_zoom_factor_for_capture_env() else {
        return rgba;
    };
    let Some(image) = RgbaImage::from_raw(width, height, rgba) else {
        return Vec::new();
    };
    native_zoom_variant_rgba8(image, zoom).into_raw()
}

/// Extrae el nombre de archivo de una ruta de asset (`bus_stop_ne_build_a.png`).
#[must_use]
pub fn tile_filename(asset_path: &str) -> &str {
    asset_path.rsplit('/').next().unwrap_or(asset_path)
}

/// Sprites ferroviarios que usan `PALETTE_MODIFIER_COLOUR` (plataformas + waypoints).
#[must_use]
pub fn rail_sprite_ids_for_company_palette() -> Vec<u32> {
    use super::rail::rail_sprite_ids_for_preload;
    use super::station::{
        rail_station_draw_layers, rail_station_ground_track_sprite_for_type,
        rail_station_layer_for_type, rail_waypoint_draw_layers,
    };

    let mut ids: BTreeSet<u32> = rail_sprite_ids_for_preload().into_iter().collect();
    for rail_type in [
        openttdrs_core::RailType::Rail,
        openttdrs_core::RailType::Electric,
        openttdrs_core::RailType::Monorail,
        openttdrs_core::RailType::Maglev,
    ] {
        for gfx in 0..=7u8 {
            ids.insert(rail_station_ground_track_sprite_for_type(gfx, 0, rail_type));
            for layer in rail_station_draw_layers(gfx) {
                let layer = rail_station_layer_for_type(*layer, rail_type);
                // Cristal: PALETTE_TO_TRANSPARENT, no company colour.
                if !super::station::rail_station_roof_glass_sprite(layer.sprite_id) {
                    ids.insert(layer.sprite_id);
                }
            }
        }
    }
    for m5 in [0u8, 1] {
        for layer in rail_waypoint_draw_layers(m5) {
            ids.insert(layer.sprite_id);
        }
    }
    ids.into_iter().collect()
}

/// Lista de PNG estáticos + `rail_{id}.png` que participan en el remapeo.
#[must_use]
pub fn company_palette_tile_filenames() -> Vec<String> {
    let mut names: BTreeSet<String> = COMPANY_PALETTE_STATIC_PATHS
        .iter()
        .map(|s| (*s).to_string())
        .collect();
    // `OBJECT_STATUE` usa `PALETTE_MODIFIER_COLOUR` en OpenTTD, pero no forma
    // parte de las tablas de vehículos/depósitos que generan la lista base.
    names.insert("object_statue_company.png".into());
    for index in 0..8 {
        names.insert(format!("track_fence_{index}.png"));
    }
    for id in rail_sprite_ids_for_company_palette() {
        names.insert(format!("rail_{id}.png"));
    }
    names.into_iter().collect()
}

/// Sprites recoloreados para la compañía activa (fuera del atlas de teselas).
///
/// `tiles` = paleta de la compañía activa; `extra` = otras compañías vistas en mapa.
#[derive(Resource, Clone, Default)]
pub struct CompanyColoredSprites {
    pub colour: CompanyColour,
    /// Clave = nombre de archivo (`bus_stop_ne_build_a.png`).
    pub tiles: HashMap<String, Handle<Image>>,
    /// Paletas adicionales: `colour.as_u8()` → filename → handle.
    pub extra: HashMap<u8, HashMap<String, Handle<Image>>>,
}

impl CompanyColoredSprites {
    #[must_use]
    pub fn new(colour: CompanyColour) -> Self {
        Self {
            colour,
            tiles: HashMap::new(),
            extra: HashMap::new(),
        }
    }

    pub fn build_all(&mut self, images: &mut Assets<Image>) {
        self.tiles.clear();
        self.extra.clear();
        for filename in company_palette_tile_filenames() {
            if let Some(handle) = load_recolored_png(&filename, self.colour, images) {
                self.tiles.insert(filename, handle);
            }
        }
    }

    /// Asegura una paleta para `colour` (activa o en `extra`).
    pub fn ensure_palette(&mut self, colour: CompanyColour, images: &mut Assets<Image>) {
        if colour == self.colour {
            if self.tiles.is_empty() {
                self.build_all(images);
            }
            return;
        }
        let key = colour.as_u8();
        if self.extra.contains_key(&key) {
            return;
        }
        let mut tiles = HashMap::new();
        for filename in company_palette_tile_filenames() {
            if let Some(handle) = load_recolored_png(&filename, colour, images) {
                tiles.insert(filename, handle);
            }
        }
        self.extra.insert(key, tiles);
    }

    #[must_use]
    pub fn tile_handle(&self, filename: &str) -> Option<&Handle<Image>> {
        self.tiles.get(filename)
    }

    #[must_use]
    pub fn tile_handle_for_colour(
        &self,
        colour: CompanyColour,
        filename: &str,
    ) -> Option<&Handle<Image>> {
        if colour == self.colour {
            return self.tiles.get(filename);
        }
        self.extra
            .get(&colour.as_u8())
            .and_then(|m| m.get(filename))
    }

    #[must_use]
    pub fn tile_handle_path(&self, asset_path: &str) -> Option<&Handle<Image>> {
        self.tile_handle(tile_filename(asset_path))
    }

    #[must_use]
    pub fn tile_handle_path_for_colour(
        &self,
        colour: CompanyColour,
        asset_path: &str,
    ) -> Option<&Handle<Image>> {
        self.tile_handle_for_colour(colour, tile_filename(asset_path))
    }

    #[must_use]
    pub fn rail_handle(&self, sprite_id: u32) -> Option<&Handle<Image>> {
        self.tile_handle(&format!("rail_{sprite_id}.png"))
    }

    #[must_use]
    pub fn vehicle_handle(&self, path: &str) -> Option<&Handle<Image>> {
        self.tile_handle_path(path)
    }

    #[must_use]
    pub fn vehicle_handle_for_colour(
        &self,
        colour: CompanyColour,
        path: &str,
    ) -> Option<&Handle<Image>> {
        self.tile_handle_path_for_colour(colour, path)
    }

    /// Sprite de industria recoloreado (`PALETTE_MODIFIER_COLOUR` / `random_colour`).
    pub fn industry_sprite_handle(
        &mut self,
        sprite_id: u32,
        colour: CompanyColour,
        images: &mut Assets<Image>,
    ) -> Option<Handle<Image>> {
        let key = format!("industry_{sprite_id}_c{}", colour.as_u8());
        if let Some(h) = self.tiles.get(&key) {
            return Some(h.clone());
        }
        if !colour.needs_recolor() {
            return None;
        }
        let filename = format!("industry_{sprite_id}.png");
        let handle = load_recolored_png(&filename, colour, images)?;
        self.tiles.insert(key, handle.clone());
        Some(handle)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::implicit_clone)]
mod tests {
    use super::*;

    #[test]
    fn native_zoom_factor_is_capture_scoped_and_fixed() {
        assert_eq!(native_zoom_factor_for(false, Some(2.0)), None);
        assert_eq!(native_zoom_factor_for(true, Some(0.25)), None);
        assert_eq!(native_zoom_factor_for(true, Some(1.0)), None);
        assert_eq!(native_zoom_factor_for(true, Some(2.0)), Some(2));
        assert_eq!(native_zoom_factor_for(true, Some(4.0)), Some(4));
        assert_eq!(native_zoom_factor_for(true, Some(8.0)), Some(8));
        assert_eq!(native_zoom_factor_for(true, Some(3.0)), None);
    }

    #[test]
    fn native_zoom_variant_repeats_first_pixel_of_each_block() {
        let mut source = RgbaImage::new(5, 3);
        for y in 0..3 {
            for x in 0..5 {
                source.put_pixel(x, y, image::Rgba([x as u8, y as u8, 7, 255]));
            }
        }
        let variant = native_zoom_variant_rgba8(source, 2);
        for y in 0..3 {
            for x in 0..5 {
                assert_eq!(
                    *variant.get_pixel(x, y),
                    image::Rgba([(x / 2 * 2) as u8, (y / 2 * 2) as u8, 7, 255])
                );
            }
        }
    }

    #[test]
    fn bare_land_loader_matches_core_palette_bake() {
        let dir = tempfile::tempdir().expect("tempdir");
        let source = RgbaImage::from_raw(
            3,
            1,
            openttdrs_core::newgrf_sprites::indices_to_rgba(&[89, 83, 0], 3, 1)
                .expect("rgba source"),
        )
        .expect("source image");
        source.save(dir.path().join("rail_test.png")).expect("png");

        let mut images = Assets::<Image>::default();
        let mut pages = HashMap::new();
        let handle = load_bare_land_png("rail_test.png", dir.path(), &mut pages, &mut images)
            .expect("bare-land handle");
        let actual = images.get(&handle).expect("baked image");
        let expected =
            openttdrs_core::newgrf_sprites::bake_sprite_bare_land(&openttdrs_core::DecodedSprite {
                width: 3,
                height: 1,
                x_offs: 0,
                y_offs: 0,
                rgba: source.as_raw().to_vec(),
                mask: Vec::new(),
            })
            .expect("bare-land output");
        assert_eq!(actual.data.as_deref(), Some(expected.as_slice()));
        assert_eq!(actual.width(), 3);
        assert_eq!(actual.height(), 1);
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn recolor_uses_runtime_asset_root_in_an_isolated_process() {
        const PROBE: &str = "OPENTTDRS_TEST_PALETTE_ROOT";
        if let Some(root) = std::env::var_os(PROBE) {
            let expected = PathBuf::from(root).join("assets/opengfx/tiles");
            assert_eq!(tiles_assets_dir(), expected);
            let mut images = Assets::<Image>::default();
            let handle = load_recolored_png("palette_probe.png", CompanyColour::Red, &mut images)
                .expect("PNG in selected asset root");
            let image = images.get(&handle).expect("image");
            assert_eq!(image.data.as_deref(), Some([1, 2, 3, 255].as_slice()));
            return;
        }

        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path().join("package");
        let tiles = root.join("assets/opengfx/tiles");
        std::fs::create_dir_all(&tiles).expect("tiles");
        std::fs::create_dir_all(root.join("static/fonts")).expect("fonts");
        std::fs::write(root.join("static/fonts/DejaVuSansMono.ttf"), b"probe")
            .expect("font marker");
        image::RgbaImage::from_pixel(1, 1, image::Rgba([1, 2, 3, 255]))
            .save(tiles.join("palette_probe.png"))
            .expect("png");
        for mode in ["cwd", "override", "relocated"] {
            let original = std::env::current_exe().expect("test binary");
            let executable = if mode == "relocated" {
                let moved = root.join(format!("palette-tests{}", std::env::consts::EXE_SUFFIX));
                std::fs::copy(&original, &moved).expect("relocate test executable");
                moved
            } else {
                original
            };
            let mut command = std::process::Command::new(executable);
            command.args(["--exact", "sprites::company_palette::tests::recolor_uses_runtime_asset_root_in_an_isolated_process", "--nocapture"])
                .env(PROBE, &root).env_remove("OPENTTDRS_ASSET_ROOT");
            if mode == "override" {
                command
                    .env("OPENTTDRS_ASSET_ROOT", &root)
                    .current_dir(dir.path());
            } else if mode == "cwd" {
                command.current_dir(&root);
            } else {
                command.current_dir(dir.path());
            }
            let output = command.output().expect("isolated test");
            assert!(
                output.status.success(),
                "mode={mode}\n{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }

    #[test]
    fn foreign_company_ramp_is_not_inferred_from_rgb() {
        let src = COMPANY_RAMP_RGB[ramp_index(CompanyColour::Purple.as_u8() as usize, 2)];
        let mut px = [src[0], src[1], src[2], 255];
        recolor_rgba8(&mut px, CompanyColour::Green);
        assert_eq!(&px[..3], &src);
    }

    #[test]
    fn dark_blue_is_identity_for_baked_shades() {
        let mut px = [40u8, 92, 164, 255];
        recolor_rgba8(&mut px, CompanyColour::DarkBlue);
        assert_eq!(px, [40, 92, 164, 255]);
    }

    #[test]
    fn remaps_vehicle_blue_shade_to_green() {
        let src = COMPANY_RAMP_RGB[ramp_index(0, 4)];
        let mut px = [src[0], src[1], src[2], 255];
        recolor_rgba8(&mut px, CompanyColour::Green);
        let expected = COMPANY_RAMP_RGB[ramp_index(CompanyColour::Green.as_u8() as usize, 4)];
        assert_eq!(&px[..3], expected);
    }

    #[test]
    fn remap_table_maps_baked_dark_blue_to_target() {
        let table = build_remap_table(CompanyColour::Green);
        for shade in 0..COMPANY_RAMP_SHADES {
            let key = COMPANY_RAMP_RGB[ramp_index(CompanyColour::DarkBlue.as_u8() as usize, shade)];
            let expected =
                COMPANY_RAMP_RGB[ramp_index(CompanyColour::Green.as_u8() as usize, shade)];
            assert_eq!(table.get(&key), Some(&expected));
        }
    }

    #[test]
    fn ordinary_grey_that_looks_like_a_company_shade_is_not_recolored() {
        // Índice DOS 5: gris ordinario, también usado como un tono de la
        // rampa GREY. No es `COLOUR_DARK_BLUE` y debe permanecer intacto.
        let mut px = [82u8, 80, 82, 255];
        recolor_rgba8(&mut px, CompanyColour::Green);
        assert_eq!(px, [82, 80, 82, 255]);
    }

    #[test]
    fn from_u8_wraps() {
        assert_eq!(CompanyColour::from_u8(16), CompanyColour::DarkBlue);
        assert_eq!(CompanyColour::from_u8(6), CompanyColour::Green);
    }

    #[test]
    fn company_palette_paths_nonempty_and_unique() {
        let paths = company_palette_tile_filenames();
        assert!(
            paths.len() > 40,
            "expected many palette paths, got {}",
            paths.len()
        );
        let unique: BTreeSet<_> = paths.iter().collect();
        assert_eq!(unique.len(), paths.len());
        assert!(paths.iter().any(|p| p.starts_with("vehicle_")));
        assert!(paths.iter().any(|p| p.starts_with("bus_stop_")));
        assert!(paths.iter().any(|p| p.starts_with("truck_stop_")));
        assert!(paths.iter().any(|p| p.starts_with("road_depot_")));
        assert!(paths.iter().any(|p| p.starts_with("rail_depot_")));
        assert!(paths.iter().any(|p| p == "airport_jetway_3.png"));
        assert!(paths.iter().any(|p| p == "airport_passenger_tunnel.png"));
        assert!(paths.iter().any(|p| p == "ship_depot_ne.png"));
        assert!(paths.iter().any(|p| p == "ship_depot_sw_front.png"));
        assert!(paths.iter().any(|p| p == "object_statue_company.png"));
        for index in 0..8 {
            assert!(
                paths
                    .iter()
                    .any(|p| p == &format!("track_fence_{index}.png"))
            );
        }
    }
}
