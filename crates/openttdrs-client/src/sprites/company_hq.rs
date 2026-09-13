//! Geometría y selección de sprites de la sede de compañía vanilla.
//!
//! `object_land.h` indexa la sede por nivel y por posición dentro de su
//! huella 2×2. Las imágenes se extraen con el mismo orden de sprites que usa
//! OpenGFX (`2603..2631`), incluidos los tres niveles que tienen una pieza
//! BUILD adicional.

/// Primer sprite de `_object_hq` en `object_land.h`.
pub(crate) const COMPANY_HQ_SPRITE_BASE: u32 = 2603;
/// Cantidad de sprites consecutivos reservados por las cinco tablas HQ.
pub(crate) const COMPANY_HQ_SPRITE_COUNT: usize = 29;

/// Geometría NFO de un sprite HQ.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct CompanyHqSpriteMeta {
    pub(crate) width: f32,
    pub(crate) height: f32,
    pub(crate) x_offs: f32,
    pub(crate) y_offs: f32,
}

const HQ_GROUND_META: CompanyHqSpriteMeta = CompanyHqSpriteMeta {
    width: 64.0,
    height: 31.0,
    x_offs: -31.0,
    y_offs: 0.0,
};

/// Sprite de terreno de una tesela HQ según nivel y posición `(dx, dy)`.
#[must_use]
pub(crate) const fn company_hq_ground_sprite_id(level: u8, dx: u8, dy: u8) -> u32 {
    let dx = if dx > 1 { 1 } else { dx };
    let dy = if dy > 1 { 1 } else { dy };
    let level = if level > 4 { 4 } else { level };
    let index = dx as u32 + dy as u32 * 2;
    match level {
        0 => 2603 + index,
        1 => 2607 + index,
        2 => match index {
            0 => 2611,
            1 => 2613,
            2 => 2615,
            _ => 2617,
        },
        3 => match index {
            0 => 2618,
            1 => 2620,
            2 => 2622,
            _ => 2624,
        },
        _ => match index {
            0 => 2625,
            1 => 2627,
            2 => 2629,
            _ => 2631,
        },
    }
}

/// Sprite de estructura que `TILE_SEQ_LINE` añade a la posición HQ.
///
/// La tesela sur de cada huella no tiene una capa BUILD; el edificio se
/// completa con las piezas norte/este/oeste de las tablas nativas.
#[must_use]
pub(crate) const fn company_hq_build_sprite_id(level: u8, dx: u8, dy: u8) -> Option<u32> {
    let dx = if dx > 1 { 1 } else { dx };
    let dy = if dy > 1 { 1 } else { dy };
    let level = if level > 4 { 4 } else { level };
    let index = dx + dy * 2;
    match (level, index) {
        (2, 0) => Some(2612),
        (2, 1) => Some(2614),
        (2, 2) => Some(2616),
        (3, 0) => Some(2619),
        (3, 1) => Some(2621),
        (3, 2) => Some(2623),
        (4, 0) => Some(2626),
        (4, 1) => Some(2628),
        (4, 2) => Some(2630),
        _ => None,
    }
}

/// Altura de la caja `TILE_SEQ_LINE` de una pieza BUILD.
#[must_use]
pub(crate) const fn company_hq_build_height(sprite_id: u32) -> Option<i32> {
    match sprite_id {
        2612 | 2614 | 2616 => Some(20),
        2619 | 2621 | 2623 => Some(50),
        2626 | 2628 | 2630 => Some(60),
        _ => None,
    }
}

/// Metadata NFO exacta de los sprites HQ extraídos del baseset.
#[must_use]
pub(crate) const fn company_hq_sprite_meta(sprite_id: u32) -> Option<CompanyHqSpriteMeta> {
    match sprite_id {
        2603..=2611
        | 2613
        | 2615
        | 2617
        | 2618
        | 2620
        | 2622
        | 2624
        | 2625
        | 2627
        | 2629
        | 2631 => Some(HQ_GROUND_META),
        2612 => Some(CompanyHqSpriteMeta {
            width: 22.0,
            height: 13.0,
            x_offs: 11.0,
            y_offs: 2.0,
        }),
        2614 => Some(CompanyHqSpriteMeta {
            width: 32.0,
            height: 18.0,
            x_offs: 1.0,
            y_offs: -3.0,
        }),
        2616 => Some(CompanyHqSpriteMeta {
            width: 4.0,
            height: 5.0,
            x_offs: -31.0,
            y_offs: 12.0,
        }),
        2619 => Some(CompanyHqSpriteMeta {
            width: 64.0,
            height: 35.0,
            x_offs: -31.0,
            y_offs: -20.0,
        }),
        2621 => Some(CompanyHqSpriteMeta {
            width: 32.0,
            height: 38.0,
            x_offs: 1.0,
            y_offs: -23.0,
        }),
        2623 => Some(CompanyHqSpriteMeta {
            width: 32.0,
            height: 21.0,
            x_offs: -31.0,
            y_offs: -6.0,
        }),
        2626 => Some(CompanyHqSpriteMeta {
            width: 64.0,
            height: 57.0,
            x_offs: -31.0,
            y_offs: -42.0,
        }),
        2628 => Some(CompanyHqSpriteMeta {
            width: 32.0,
            height: 21.0,
            x_offs: 1.0,
            y_offs: -6.0,
        }),
        2630 => Some(CompanyHqSpriteMeta {
            width: 32.0,
            height: 61.0,
            x_offs: -31.0,
            y_offs: -46.0,
        }),
        _ => None,
    }
}

/// PNG del sprite HQ dentro de `assets/opengfx/tiles/`.
#[must_use]
pub(crate) fn company_hq_asset_filename(sprite_id: u32) -> String {
    let (prefix, base) = match sprite_id {
        2603..=2606 => ("hq_tiny", 2603),
        2607..=2610 => ("hq_small", 2607),
        2611..=2617 => ("hq_medium", 2611),
        2618..=2624 => ("hq_large", 2618),
        2625..=2631 => ("hq_huge", 2625),
        _ => return format!("hq_{sprite_id}.png"),
    };
    format!("assets/opengfx/tiles/{prefix}_{}.png", sprite_id - base)
}

#[cfg(test)]
mod tests {
    use super::{
        company_hq_build_height, company_hq_build_sprite_id, company_hq_ground_sprite_id,
        company_hq_sprite_meta,
    };

    #[test]
    fn hq_ground_table_matches_native_2x2_order() {
        assert_eq!(company_hq_ground_sprite_id(0, 0, 0), 2603);
        assert_eq!(company_hq_ground_sprite_id(2, 1, 0), 2613);
        assert_eq!(company_hq_ground_sprite_id(3, 0, 1), 2622);
        assert_eq!(company_hq_ground_sprite_id(4, 1, 1), 2631);
    }

    #[test]
    fn hq_build_table_excludes_south_tile_and_keeps_heights() {
        assert_eq!(company_hq_build_sprite_id(2, 1, 1), None);
        assert_eq!(company_hq_build_sprite_id(2, 0, 0), Some(2612));
        assert_eq!(company_hq_build_sprite_id(3, 1, 0), Some(2621));
        assert_eq!(company_hq_build_height(2626), Some(60));
        assert_eq!(company_hq_build_height(2603), None);
    }

    #[test]
    fn hq_metadata_covers_ground_and_build_assets() {
        assert_eq!(company_hq_sprite_meta(2603).map(|m| m.width), Some(64.0));
        assert_eq!(company_hq_sprite_meta(2616).map(|m| m.x_offs), Some(-31.0));
        assert_eq!(company_hq_sprite_meta(2630).map(|m| m.height), Some(61.0));
        assert_eq!(company_hq_sprite_meta(2632), None);
    }
}
