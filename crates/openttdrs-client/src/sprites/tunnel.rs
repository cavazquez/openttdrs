//! Sprites de boca de túnel por dirección diagonal (`DrawTunnelTile` en OpenTTD).
//!
//! Cada tipo de transporte usa cuatro sprites «rear» separados por `DiagDirection`
//! (`SPR_TUNNEL_ENTRY_REAR_* + direction * 2`).

use bevy::prelude::*;

use crate::iso::{HEIGHT_PX, iso};

/// Sprite rear de ferrocarril por `DiagDirection` (0=NE … 3=NW).
pub const RAIL_TUNNEL_REAR: [u32; 4] = [2365, 2367, 2369, 2371];
/// Sprite rear de carretera por dirección.
pub const ROAD_TUNNEL_REAR: [u32; 4] = [2389, 2391, 2393, 2395];

const DIR_SUFFIX: [&str; 4] = ["ne", "se", "sw", "nw"];

/// Id OpenGFX del portal rear según transporte y dirección (0–3).
#[must_use]
pub fn tunnel_rear_sprite_id(rail: bool, dir: u8) -> u32 {
    let d = dir as usize & 3;
    if rail {
        RAIL_TUNNEL_REAR[d]
    } else {
        ROAD_TUNNEL_REAR[d]
    }
}

/// Nombre en el atlas (`tunnel_rail_rear_sw.png`, …).
#[must_use]
pub fn tunnel_rear_atlas_name(rail: bool, dir: u8) -> String {
    let kind = if rail { "rail" } else { "road" };
    format!("tunnel_{kind}_rear_{}.png", DIR_SUFFIX[dir as usize & 3])
}

/// Alias histórico: portal NE (dir 0); fallback si faltan PNG direccionales.
#[must_use]
pub fn tunnel_rear_legacy_atlas_name(rail: bool) -> &'static str {
    if rail {
        "tunnel_rail_rear.png"
    } else {
        "tunnel_road_rear.png"
    }
}

/// Offsets NFO (w, h, xrel, yrel) por sprite id.
#[must_use]
pub fn tunnel_sprite_meta(sid: u32) -> (f32, f32, f32, f32) {
    match sid {
        2365 | 2371 | 2389 | 2395 => (64.0, 39.0, -31.0, -8.0),
        2367 | 2369 | 2391 | 2393 => (64.0, 23.0, -31.0, 0.0),
        2392 | 2394 => (64.0, 22.0, -31.0, -29.0),
        _ => (64.0, 39.0, -31.0, -8.0),
    }
}

/// Posición en pantalla del portal (anclaje NFO, como `spawn_layer` de puentes).
#[must_use]
pub fn tunnel_portal_translation(px: i32, py: i32, base_z: u8, sprite_id: u32, layer: f32) -> Vec3 {
    let (w, h, xrel, yrel) = tunnel_sprite_meta(sprite_id);
    let iso_pos = iso(px, py);
    let z_px = f32::from(base_z) * HEIGHT_PX;
    Vec3::new(
        iso_pos.x + xrel + w / 2.0,
        iso_pos.y - yrel - h / 2.0 + z_px,
        (px + py) as f32 * 0.01 + f32::from(base_z) * 0.001 + layer,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rail_tunnel_sprites_step_by_two_per_direction() {
        assert_eq!(tunnel_rear_sprite_id(true, 0), 2365);
        assert_eq!(tunnel_rear_sprite_id(true, 1), 2367);
        assert_eq!(tunnel_rear_sprite_id(true, 2), 2369);
        assert_eq!(tunnel_rear_sprite_id(true, 3), 2371);
    }

    #[test]
    fn atlas_names_use_diagonal_suffix() {
        assert_eq!(tunnel_rear_atlas_name(true, 2), "tunnel_rail_rear_sw.png");
    }
}
