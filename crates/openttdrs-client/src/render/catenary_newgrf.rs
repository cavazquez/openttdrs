//! Caché de sprites NewGRF Action5 catenary in-world (`0x05`).

use std::collections::HashMap;

use bevy::prelude::*;
use openttdrs_core::{DecodedSprite, catenary_action5_local_slot};

use crate::iso::{iso, overlay_pos, remap_tile_offset};
use crate::render::newgrf_cache::{DecodedSpriteImagePolicy, decoded_sprite_image};
use crate::render::{AtlasSprite, WorldAssets};
use crate::sprites::catenary_sprite_gfx;

/// Rectángulo visible y ancla NFO de una pieza de catenaria.
///
/// A diferencia de un sprite de suelo, el origen que recibe
/// `AddSortableSpriteToDraw` no es el centro del PNG. El renderer debe sumar
/// los offsets NFO al origen de mundo antes de convertir la esquina superior
/// izquierda a centro Bevy.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct CatenarySpriteAnchor {
    width: f32,
    height: f32,
    x_offs: f32,
    y_offs: f32,
}

impl CatenarySpriteAnchor {
    pub(crate) fn from_decoded(sprite: &DecodedSprite) -> Self {
        Self {
            width: f32::from(sprite.width),
            height: f32::from(sprite.height),
            x_offs: f32::from(sprite.x_offs),
            y_offs: f32::from(sprite.y_offs),
        }
    }
}

/// Recorta una parte horizontal de un sprite de catenaria sin perder el ancla
/// NFO del PNG completo. `left`/`right` están expresados en coordenadas de
/// mundo relativas al origen del sprite (`None` = infinito).
pub(crate) fn catenary_sprite_horizontal_crop(
    mut sprite: Sprite,
    anchor: CatenarySpriteAnchor,
    left: Option<f32>,
    right: Option<f32>,
) -> Option<(Sprite, f32)> {
    let min_x = left.map_or(0.0, |x| (x - anchor.x_offs).clamp(0.0, anchor.width));
    let max_x = right.map_or(anchor.width, |x| {
        (x - anchor.x_offs + 1.0).clamp(0.0, anchor.width)
    });
    if max_x <= min_x {
        return None;
    }
    let rect = Rect::new(min_x, 0.0, max_x, anchor.height);
    let x_shift = rect.center().x - anchor.width / 2.0;
    sprite.rect = Some(rect);
    Some((sprite, x_shift))
}

/// Resuelve los metadatos de anclaje de Action5, incluidos reemplazos NewGRF.
#[must_use]
pub(crate) fn catenary_sprite_anchor(
    sprite_id: u32,
    catenary_newgrf: &[Option<DecodedSprite>],
) -> Option<CatenarySpriteAnchor> {
    if let Some(slot) = catenary_action5_local_slot(sprite_id)
        && let Some(decoded) = catenary_newgrf.get(slot).and_then(|sprite| sprite.as_ref())
    {
        return Some(CatenarySpriteAnchor::from_decoded(decoded));
    }
    catenary_sprite_gfx(sprite_id).map(|gfx| CatenarySpriteAnchor {
        width: gfx.width,
        height: gfx.height,
        x_offs: gfx.x_offs,
        y_offs: gfx.y_offs,
    })
}

/// Centro Bevy equivalente a un Action5 anclado por `AddSortableSpriteToDraw`.
///
/// `iso(tx, ty)` ya es el `RemapCoords` del origen de tesela a media escala.
/// Por eso el avance PCP/PPP y su altura local también pasan por
/// `remap_tile_offset(..) * 0.5`; los offsets NFO, en cambio, ya son píxeles
/// de pantalla y se aplican sin reescalarlos. Quien llama debe incorporar
/// previamente `SpriteBounds::origin` y `SpriteBounds::offset`: esos valores
/// desplazan el ancla visual de `AddSortableSpriteToDraw`, no sólo su caja de
/// ordenación.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub(crate) fn catenary_sprite_center(
    tx: i32,
    ty: i32,
    base_z: u8,
    layer: f32,
    tile_dx: f32,
    tile_dy: f32,
    local_z: f32,
    anchor: CatenarySpriteAnchor,
) -> Vec3 {
    let local = remap_tile_offset(tile_dx, tile_dy, local_z) * 0.5;
    overlay_pos(
        iso(tx, ty),
        local.x + anchor.x_offs,
        anchor.y_offs - local.y,
        anchor.width,
        anchor.height,
        base_z,
        layer,
        tx,
        ty,
    )
}

/// Slot local `0..35` → textura RGBA.
#[derive(Resource, Default)]
pub(crate) struct NewGrfCatenarySpriteCache {
    handles: HashMap<u8, Handle<Image>>,
}

impl NewGrfCatenarySpriteCache {
    pub(crate) fn clear(&mut self) {
        self.handles.clear();
    }

    pub(crate) fn handle_for(
        &mut self,
        slot: u8,
        sprite: &DecodedSprite,
        images: &mut Assets<Image>,
    ) -> Handle<Image> {
        self.handles
            .entry(slot)
            .or_insert_with(|| {
                images.add(decoded_sprite_image(sprite, DecodedSpriteImagePolicy::Raw))
            })
            .clone()
    }
}

/// Sprite de catenaria: NewGRF Action5 si hay slot; si no, OpenGFX.
pub(crate) fn catenary_sprite_colored(
    assets: &WorldAssets,
    sprite_id: u32,
    tint: Color,
    catenary_newgrf: &[Option<DecodedSprite>],
    cache: Option<&mut NewGrfCatenarySpriteCache>,
    images: Option<&mut Assets<Image>>,
) -> Option<Sprite> {
    if let Some(slot) = catenary_action5_local_slot(sprite_id)
        && let (Some(cache), Some(images), Some(decoded)) = (
            cache,
            images,
            catenary_newgrf.get(slot).and_then(|s| s.as_ref()),
        )
    {
        let handle = cache.handle_for(u8::try_from(slot).unwrap_or(0), decoded, images);
        return Some(Sprite {
            image: handle,
            color: tint,
            ..default()
        });
    }
    assets
        .rail
        .get(&sprite_id)
        .map(|img: &AtlasSprite| img.sprite_colored(tint))
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::sprites::{PYLON_SPRITE_BASE, WIRE_SPRITE_BASE};
    use openttdrs_core::apply_newgrf_action5_catenary;
    use openttdrs_core::newgrf_sprites::build_grf_v2_action5_with_sprite;
    use openttdrs_core::prelude::GameState;

    #[test]
    fn catenary_sprite_cache_builds_handle_from_action5() {
        let mut indices = vec![0u8; 8 * 8];
        for y in 2..6 {
            for x in 2..6 {
                indices[y * 8 + x] = 174;
            }
        }
        let bytes =
            build_grf_v2_action5_with_sprite(0x05, 1039, 8, 8, &indices, [b'E', b'L', 0, 3], "elw");
        let dir = tempfile::tempdir().expect("tmp");
        std::fs::write(dir.path().join("elw.grf"), &bytes).expect("write");
        let mut state = GameState::new(4, 4);
        state
            .newgrf_stack
            .push(openttdrs_core::NewGrfEntry::new("elw.grf", 2));
        apply_newgrf_action5_catenary(&mut state, &[dir.path()]);
        let spr = state.runtime.catenary_newgrf_sprites[0]
            .as_ref()
            .expect("slot 0");
        assert_eq!(
            catenary_sprite_anchor(WIRE_SPRITE_BASE, &state.runtime.catenary_newgrf_sprites),
            Some(CatenarySpriteAnchor {
                width: 8.0,
                height: 8.0,
                x_offs: -4.0,
                y_offs: -8.0,
            })
        );
        let mut images = Assets::<Image>::default();
        let mut cache = NewGrfCatenarySpriteCache::default();
        let handle = cache.handle_for(0, spr, &mut images);
        assert!(images.get(&handle).is_some());
    }

    #[test]
    fn vanilla_anchor_preserves_action5_nfo_offsets() {
        assert_eq!(
            catenary_sprite_anchor(WIRE_SPRITE_BASE, &[]),
            Some(CatenarySpriteAnchor {
                width: 32.0,
                height: 16.0,
                x_offs: -29.0,
                y_offs: -2.0,
            })
        );
        assert_eq!(
            catenary_sprite_anchor(PYLON_SPRITE_BASE, &[]),
            Some(CatenarySpriteAnchor {
                width: 8.0,
                height: 16.0,
                x_offs: -7.0,
                y_offs: -14.0,
            })
        );
    }

    #[test]
    fn center_applies_sortable_origin_before_nfo_anchor() {
        let anchor = catenary_sprite_anchor(WIRE_SPRITE_BASE, &[]).expect("wire anchor");
        // `_rail_catenary_sprite_data` X plano: SpriteBounds(0, 7, 10).
        // Sin esos tres valores el resultado sería (-13, -6), no el centro
        // real (1, -3) que recibe OpenTTD tras `RemapCoords` y el NFO.
        assert_eq!(
            catenary_sprite_center(0, 0, 0, 0.035, 0.0, 7.0, 10.0, anchor),
            Vec3::new(1.0, -3.0, crate::iso::sortable_draw_z(0, 0, 0, 0.035))
        );
    }
}
