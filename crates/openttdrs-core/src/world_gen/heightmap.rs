//! Heightmaps: parseo y aplicación de mapas de altura ASCII.

use crate::map::{Map, MapError, TileCoord, TileKind, WaterClass, set_water_class_m1};

use super::config::{Climate, clear_ground_m5, initial_clear_ground};
use super::landcover::grass_density;
use super::rivers::mark_water_coasts;

/// Heightmap ASCII: primera línea `OTDRHMAP1`, segunda `WIDTH HEIGHT`, luego `WIDTH*HEIGHT`
/// enteros 0..=15 (separados por whitespace).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeightmapData {
    pub width: u32,
    pub height: u32,
    pub heights: Vec<u8>,
}

/// Parsea un heightmap `.hmap` en texto.
///
/// # Errors
/// Cabecera inválida, dimensiones incoherentes o alturas fuera de rango.
pub fn parse_hmap(text: &str) -> Result<HeightmapData, String> {
    let mut tokens = text.split_whitespace();
    let magic = tokens.next().ok_or_else(|| "heightmap vacío".to_string())?;
    if magic != "OTDRHMAP1" {
        return Err(format!(
            "cabecera heightmap esperada OTDRHMAP1, got {magic}"
        ));
    }
    let width: u32 = tokens
        .next()
        .ok_or_else(|| "falta WIDTH".to_string())?
        .parse()
        .map_err(|_| "WIDTH inválido".to_string())?;
    let height: u32 = tokens
        .next()
        .ok_or_else(|| "falta HEIGHT".to_string())?
        .parse()
        .map_err(|_| "HEIGHT inválido".to_string())?;
    if width == 0 || height == 0 || width > 4096 || height > 4096 {
        return Err(format!(
            "dimensiones heightmap fuera de rango: {width}×{height}"
        ));
    }
    let expected = (width as usize).saturating_mul(height as usize);
    let mut heights = Vec::with_capacity(expected);
    for tok in tokens {
        let h: u8 = tok.parse().map_err(|_| format!("altura inválida: {tok}"))?;
        if h > 15 {
            return Err(format!("altura {h} > 15"));
        }
        heights.push(h);
    }
    if heights.len() != expected {
        return Err(format!(
            "se esperaban {expected} alturas, hay {}",
            heights.len()
        ));
    }
    Ok(HeightmapData {
        width,
        height,
        heights,
    })
}

/// Aplica un heightmap al mapa (redimensiona si hace falta) y marca agua ≤ `sea_level`.
///
/// # Errors
/// Fallos de mapa al setear altura/tipo.
pub fn apply_heightmap(
    map: &mut Map,
    data: &HeightmapData,
    sea_level: u8,
    climate: Climate,
    seed: u64,
) -> Result<(), MapError> {
    if map.dimensions() != (data.width, data.height) {
        *map = Map::new_flat(data.width, data.height, sea_level.saturating_add(2));
    }
    let mw = i32::try_from(data.width).expect("width fits i32");
    let mh = i32::try_from(data.height).expect("height fits i32");
    for y in 0..mh {
        for x in 0..mw {
            let idx = (y as u32 * data.width + x as u32) as usize;
            let h = data.heights.get(idx).copied().unwrap_or(sea_level);
            let c = TileCoord::new(x, y);
            map.set_height(c, h)?;
            if h <= sea_level {
                map.set_kind(c, TileKind::Water)?;
                map.set_mapt_m5(c, 0x60, 0)?;
                map.set_m1(c, set_water_class_m1(0, WaterClass::Sea))?;
            } else {
                let ground = initial_clear_ground(climate, x, y, h, seed);
                let m5 = clear_ground_m5(ground, grass_density(x, y, seed));
                map.set_kind(c, TileKind::Grass)?;
                map.set_mapt_m5(c, 0x40, m5)?;
            }
        }
    }
    mark_water_coasts(map, mw, mh, sea_level, &[]);
    Ok(())
}
