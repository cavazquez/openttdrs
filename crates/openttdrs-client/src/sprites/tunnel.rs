//! Sprites de boca de túnel por dirección diagonal (`DrawTunnelTile` en OpenTTD).
//!
//! Cada tipo de transporte usa un par por `DiagDirection`: `rear` se dibuja
//! como suelo y `front` (el ID siguiente) como techo/boca sortable.

use bevy::prelude::*;
use openttdrs_core::RailType;

use crate::iso::{HEIGHT_PX, iso, remap_tile_offset};
use crate::sprites::rail::CATENARY_ENTRANCE_SPRITE_BASE;

include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/src/sprites/tunnel_draw_data_generated.rs"
));

/// Sprite rear de ferrocarril por `DiagDirection` (0=NE … 3=NW).
pub const RAIL_TUNNEL_REAR: [u32; 4] = [2365, 2367, 2369, 2371];
/// Sprite rear de monorriel por `DiagDirection`.
pub const MONO_TUNNEL_REAR: [u32; 4] = [2373, 2375, 2377, 2379];
/// Sprite rear de maglev por `DiagDirection`.
pub const MAGLEV_TUNNEL_REAR: [u32; 4] = [2381, 2383, 2385, 2387];
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

/// Id OpenGFX del portal de túnel ferroviario para su tipo de vía y dirección.
#[must_use]
pub fn rail_tunnel_rear_sprite_id(rail_type: RailType, dir: u8) -> u32 {
    let d = dir as usize & 3;
    match rail_type {
        RailType::Monorail => MONO_TUNNEL_REAR[d],
        RailType::Maglev => MAGLEV_TUNNEL_REAR[d],
        RailType::Rail | RailType::Electric => RAIL_TUNNEL_REAR[d],
    }
}

/// Id OpenGFX de la capa frontal/techo de un túnel de carretera o ferrocarril.
/// OpenTTD la emite inmediatamente después de la capa `rear` de suelo.
#[must_use]
pub fn tunnel_front_sprite_id(rail: bool, dir: u8) -> u32 {
    tunnel_rear_sprite_id(rail, dir) + 1
}

/// Id OpenGFX de la capa frontal/techo de un túnel ferroviario tipado.
#[must_use]
pub fn rail_tunnel_front_sprite_id(rail_type: RailType, dir: u8) -> u32 {
    rail_tunnel_rear_sprite_id(rail_type, dir) + 1
}

/// Nombre en el atlas (`tunnel_rail_rear_sw.png`, …).
#[must_use]
pub fn tunnel_rear_atlas_name(rail: bool, dir: u8) -> String {
    let kind = if rail { "rail" } else { "road" };
    format!("tunnel_{kind}_rear_{}.png", DIR_SUFFIX[dir as usize & 3])
}

/// Nombre del atlas del portal ferroviario para el tipo de vía indicado.
#[must_use]
pub fn rail_tunnel_rear_atlas_name(rail_type: RailType, dir: u8) -> String {
    let kind = match rail_type {
        RailType::Monorail => "mono",
        RailType::Maglev => "mglv",
        RailType::Rail | RailType::Electric => "rail",
    };
    format!("tunnel_{kind}_rear_{}.png", DIR_SUFFIX[dir as usize & 3])
}

/// Nombre en el atlas de la capa frontal de carretera/ferrocarril.
#[must_use]
pub fn tunnel_front_atlas_name(rail: bool, dir: u8) -> String {
    let kind = if rail { "rail" } else { "road" };
    format!("tunnel_{kind}_front_{}.png", DIR_SUFFIX[dir as usize & 3])
}

/// Nombre en el atlas de la capa frontal de monorriel/maglev/riel.
#[must_use]
pub fn rail_tunnel_front_atlas_name(rail_type: RailType, dir: u8) -> String {
    let kind = match rail_type {
        RailType::Monorail => "mono",
        RailType::Maglev => "mglv",
        RailType::Rail | RailType::Electric => "rail",
    };
    format!("tunnel_{kind}_front_{}.png", DIR_SUFFIX[dir as usize & 3])
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
    if let Some((_, w, h, xrel, yrel)) = TUNNEL_SPRITE_META
        .iter()
        .find(|(sprite_id, ..)| *sprite_id == sid)
    {
        (*w, *h, *xrel, *yrel)
    } else {
        // Portal clásico NE: conserva un fallback visible si falta un asset.
        (64.0, 39.0, -31.0, -8.0)
    }
}

/// Metadatos NFO de los cuatro cables de entrada de túnel eléctrico.
///
/// No pertenecen a la hoja de portales `TUNNEL_SPRITE_META`: son sprites
/// virtuales de catenaria que se extraen de `openttd-rails`. Usar el ancla
/// del portal como fallback los desplazaba decenas de píxeles y los mezclaba
/// visualmente con el techo.
const TUNNEL_CATENARY_SPRITE_META: [(u32, f32, f32, f32, f32); 4] = [
    // WSO_ENTRANCE_SW .. WSO_ENTRANCE_NW, Action5 de OpenGFX.
    (CATENARY_ENTRANCE_SPRITE_BASE, 16.0, 8.0, -29.0, 6.0),
    (CATENARY_ENTRANCE_SPRITE_BASE + 1, 16.0, 8.0, -1.0, -2.0),
    (CATENARY_ENTRANCE_SPRITE_BASE + 2, 16.0, 8.0, -13.0, -2.0),
    (CATENARY_ENTRANCE_SPRITE_BASE + 3, 16.0, 8.0, 15.0, 6.0),
];

#[must_use]
fn tunnel_catenary_sprite_meta(sid: u32) -> (f32, f32, f32, f32) {
    TUNNEL_CATENARY_SPRITE_META
        .iter()
        .find(|(sprite_id, ..)| *sprite_id == sid)
        .map_or((16.0, 8.0, -29.0, 6.0), |(_, w, h, xrel, yrel)| {
            (*w, *h, *xrel, *yrel)
        })
}

fn tunnel_translation_from_meta(
    px: i32,
    py: i32,
    base_z: u8,
    meta: (f32, f32, f32, f32),
    layer: f32,
) -> Vec3 {
    let (w, h, xrel, yrel) = meta;
    let iso_pos = iso(px, py);
    let z_px = f32::from(base_z) * HEIGHT_PX;
    Vec3::new(
        iso_pos.x + xrel + w / 2.0,
        iso_pos.y - yrel - h / 2.0 + z_px,
        (px + py) as f32 * 0.01 + f32::from(base_z) * 0.001 + layer,
    )
}

/// Posición en pantalla del portal (anclaje NFO, como `spawn_layer` de puentes).
#[must_use]
pub fn tunnel_portal_translation(px: i32, py: i32, base_z: u8, sprite_id: u32, layer: f32) -> Vec3 {
    tunnel_translation_from_meta(px, py, base_z, tunnel_sprite_meta(sprite_id), layer)
}

/// Posición de la capa frontal/techo de una boca de túnel.
///
/// `DrawTile_TunnelBridge` no ancla el techo en el origen de la tesela. Antes
/// de convertirlo a pantalla, OpenTTD suma `SpriteBounds::origin` y
/// `SpriteBounds::offset`; para las cuatro direcciones eso da `(15, 15, 0)`.
/// Omitirlo separaba visualmente el techo de la rampa y hacía que pareciera
/// una tesela distinta de la vía que entra al túnel.
#[must_use]
pub fn tunnel_front_translation(px: i32, py: i32, base_z: u8, sprite_id: u32, layer: f32) -> Vec3 {
    let mut pos = tunnel_portal_translation(px, py, base_z, sprite_id, layer);
    // `TILE_SIZE - 1` en ambos ejes. `remap_tile_offset` usa el mismo
    // RemapCoords que el resto del renderer; media escala coincide con
    // `iso(tx, ty)`, que representa una tesela de 64 px.
    let offset = remap_tile_offset(15.0, 15.0, 0.0) * 0.5;
    pos.x += offset.x;
    pos.y += offset.y;
    pos
}

/// Posición de la entrada de catenaria de un túnel eléctrico.
///
/// A diferencia del techo, `DrawRailCatenaryOnTunnel` parte de `(0, 0)`.
/// Conserva el anclaje NFO del cable, no el del sprite frontal del túnel.
#[must_use]
pub fn tunnel_catenary_translation(
    px: i32,
    py: i32,
    base_z: u8,
    sprite_id: u32,
    layer: f32,
) -> Vec3 {
    tunnel_translation_from_meta(
        px,
        py,
        base_z,
        tunnel_catenary_sprite_meta(sprite_id),
        layer,
    )
}

/// Geometría lógica de la capa frontal del portal, tal como
/// `DrawTile_TunnelBridge` la pasa a `AddSortableSpriteToDraw`.
///
/// Los offsets de píxel del NFO determinan la posición visual (ver
/// [`tunnel_portal_translation`]); estos valores son la caja 3D usada por
/// OpenTTD para ordenar el techo contra trenes, árboles y la vía vecina.
#[must_use]
pub const fn tunnel_front_trace_geometry(
    dir: u8,
) -> ((i32, i32, i32), (i32, i32, i32, i32, i32, i32)) {
    match dir & 3 {
        // NE / SW: `roof_bounds` ocupa el borde Y de la tesela.
        0 | 2 => ((15, 14, -7), (0, 1, 7, 16, 15, 1)),
        // SE / NW: rota la caja al borde X.
        _ => ((14, 15, -7), (1, 0, 7, 15, 16, 1)),
    }
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
    fn tunnel_front_is_the_sprite_after_its_rear_layer() {
        assert_eq!(tunnel_front_sprite_id(false, 1), 2392);
        assert_eq!(rail_tunnel_front_sprite_id(RailType::Monorail, 2), 2378);
        assert_eq!(
            rail_tunnel_front_atlas_name(RailType::Maglev, 3),
            "tunnel_mglv_front_nw.png"
        );
    }

    #[test]
    fn atlas_names_use_diagonal_suffix() {
        assert_eq!(tunnel_rear_atlas_name(true, 2), "tunnel_rail_rear_sw.png");
    }

    #[test]
    fn rail_tunnel_sprites_preserve_type_and_direction() {
        assert_eq!(
            rail_tunnel_rear_sprite_id(RailType::Monorail, 2),
            MONO_TUNNEL_REAR[2]
        );
        assert_eq!(
            rail_tunnel_rear_atlas_name(RailType::Maglev, 3),
            "tunnel_mglv_rear_nw.png"
        );
    }

    #[test]
    fn generated_metadata_covers_every_tunnel_layer() {
        for id in RAIL_TUNNEL_REAR
            .into_iter()
            .chain(MONO_TUNNEL_REAR)
            .chain(MAGLEV_TUNNEL_REAR)
            .chain(ROAD_TUNNEL_REAR)
            .flat_map(|rear| [rear, rear + 1])
        {
            assert!(
                TUNNEL_SPRITE_META
                    .iter()
                    .any(|(sprite_id, ..)| *sprite_id == id)
            );
        }
    }

    #[test]
    fn front_geometry_matches_upstream_roof_bounds() {
        assert_eq!(
            tunnel_front_trace_geometry(2),
            ((15, 14, -7), (0, 1, 7, 16, 15, 1))
        );
        assert_eq!(
            tunnel_front_trace_geometry(1),
            ((14, 15, -7), (1, 0, 7, 15, 16, 1))
        );
    }

    #[test]
    fn front_translation_applies_the_upstream_roof_anchor_for_every_direction() {
        let base = tunnel_portal_translation(190, 125, 0, 2374, 0.08);
        let front = tunnel_front_translation(190, 125, 0, 2374, 0.08);
        assert_eq!(front.x - base.x, 0.0);
        assert_eq!(front.y - base.y, -30.0);
        assert_eq!(front.z, base.z);
    }

    #[test]
    fn catenary_keeps_its_own_nfo_anchor() {
        let wire = tunnel_catenary_translation(190, 127, 0, 910_063, 0.085);
        assert_eq!(wire.x, -2037.0);
        assert_eq!(wire.y, -5082.0);
        assert!((wire.z - 3.255).abs() < 0.000_01);
        assert_ne!(wire, tunnel_portal_translation(190, 127, 0, 910_063, 0.085));
        assert_eq!(
            tunnel_catenary_sprite_meta(CATENARY_ENTRANCE_SPRITE_BASE + 3),
            (16.0, 8.0, 15.0, 6.0)
        );
    }
}
