//! Remapeo de paleta de compañía OpenTTD (`PALETTE_MODIFIER_COLOUR`).
//!
//! Los PNG 8bpp se hornean con la rampa `COLOUR_DARK_BLUE` (compañía 0).
//! Para otro color de compañía, cada píxel que coincide con el tono `shade`
//! de *cualquier* rampa de compañía se sustituye por el tono `shade` del color
//! destino — igual que las tablas `recolour_sprite` del baseset.

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

/// Tabla RGB → RGB para remapear al color de compañía `target`.
#[must_use]
pub fn build_remap_table(target: CompanyColour) -> HashMap<[u8; 3], [u8; 3]> {
    let mut map = HashMap::new();
    if !target.needs_recolor() {
        return map;
    }
    let dst = target.as_u8() as usize;
    for shade in 0..COMPANY_RAMP_SHADES {
        let out = COMPANY_RAMP_RGB[ramp_index(dst, shade)];
        for src_colour in 0..COMPANY_COLOUR_COUNT {
            let key = COMPANY_RAMP_RGB[ramp_index(src_colour, shade)];
            map.insert(key, out);
        }
    }
    map
}

/// Recolorea un buffer RGBA8 in-place.
pub fn recolor_rgba8(buf: &mut [u8], target: CompanyColour) {
    if !target.needs_recolor() {
        return;
    }
    let table = remap_table_cached(target);
    for px in buf.chunks_exact_mut(4) {
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
pub fn tiles_assets_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/opengfx/tiles")
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
    if target.needs_recolor() {
        recolor_rgba8(img.as_mut(), target);
    }
    Some(images.add(rgba_to_bevy_image(img)))
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

/// Extrae el nombre de archivo de una ruta de asset (`bus_stop_ne_build_a.png`).
#[must_use]
pub fn tile_filename(asset_path: &str) -> &str {
    asset_path.rsplit('/').next().unwrap_or(asset_path)
}

/// Sprites ferroviarios que usan `PALETTE_MODIFIER_COLOUR` (plataformas + waypoints).
#[must_use]
pub fn rail_sprite_ids_for_company_palette() -> Vec<u32> {
    use super::rail::rail_sprite_ids_for_preload;
    use super::station::{rail_station_draw_layers, rail_waypoint_draw_layers};

    let mut ids: BTreeSet<u32> = rail_sprite_ids_for_preload().into_iter().collect();
    for gfx in 0..=7u8 {
        ids.insert(super::station::rail_station_ground_track_sprite(gfx, 0));
        for layer in rail_station_draw_layers(gfx) {
            ids.insert(layer.sprite_id);
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
    for id in rail_sprite_ids_for_company_palette() {
        names.insert(format!("rail_{id}.png"));
    }
    names.into_iter().collect()
}

/// Sprites recoloreados para la compañía activa (fuera del atlas de teselas).
#[derive(Resource, Clone, Default)]
pub struct CompanyColoredSprites {
    pub colour: CompanyColour,
    /// Clave = nombre de archivo (`bus_stop_ne_build_a.png`).
    pub tiles: HashMap<String, Handle<Image>>,
}

impl CompanyColoredSprites {
    #[must_use]
    pub fn new(colour: CompanyColour) -> Self {
        Self {
            colour,
            tiles: HashMap::new(),
        }
    }

    pub fn build_all(&mut self, images: &mut Assets<Image>) {
        self.tiles.clear();
        if !self.colour.needs_recolor() {
            return;
        }
        for filename in company_palette_tile_filenames() {
            if let Some(handle) = load_recolored_png(&filename, self.colour, images) {
                self.tiles.insert(filename, handle);
            }
        }
    }

    #[must_use]
    pub fn tile_handle(&self, filename: &str) -> Option<&Handle<Image>> {
        self.tiles.get(filename)
    }

    #[must_use]
    pub fn tile_handle_path(&self, asset_path: &str) -> Option<&Handle<Image>> {
        self.tile_handle(tile_filename(asset_path))
    }

    #[must_use]
    pub fn rail_handle(&self, sprite_id: u32) -> Option<&Handle<Image>> {
        self.tile_handle(&format!("rail_{sprite_id}.png"))
    }

    #[must_use]
    pub fn vehicle_handle(&self, path: &str) -> Option<&Handle<Image>> {
        self.tile_handle_path(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dark_blue_is_identity() {
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
    }
}
