//! Cimientos nivelados (`DrawFoundation` + `FOUNDATION_LEVELED`) para industrias.

/// `SPR_FOUNDATION_BASE` en OpenTTD; sprite concreto = base + `tileh` (1–14).
pub const FOUNDATION_SPRITE_BASE: u32 = 989;

/// Metadatos NFO OpenGFX por pendiente (bloque 0: muros NW+NE visibles).
pub struct FoundationGfx {
    pub w: f32,
    pub h: f32,
    pub xrel: f32,
    pub yrel: f32,
}

/// Filas 1..=14 (`tileh` OpenTTD).
pub const FOUNDATION_LEVELED_GFX: [FoundationGfx; 14] = [
    FoundationGfx {
        w: 64.0,
        h: 40.0,
        xrel: -31.0,
        yrel: -9.0,
    },
    FoundationGfx {
        w: 64.0,
        h: 32.0,
        xrel: -31.0,
        yrel: -9.0,
    },
    FoundationGfx {
        w: 64.0,
        h: 32.0,
        xrel: -31.0,
        yrel: -9.0,
    },
    FoundationGfx {
        w: 64.0,
        h: 40.0,
        xrel: -31.0,
        yrel: -9.0,
    },
    FoundationGfx {
        w: 64.0,
        h: 40.0,
        xrel: -31.0,
        yrel: -9.0,
    },
    FoundationGfx {
        w: 64.0,
        h: 32.0,
        xrel: -31.0,
        yrel: -9.0,
    },
    FoundationGfx {
        w: 64.0,
        h: 32.0,
        xrel: -31.0,
        yrel: -9.0,
    },
    FoundationGfx {
        w: 64.0,
        h: 40.0,
        xrel: -31.0,
        yrel: -9.0,
    },
    FoundationGfx {
        w: 64.0,
        h: 40.0,
        xrel: -31.0,
        yrel: -9.0,
    },
    FoundationGfx {
        w: 64.0,
        h: 32.0,
        xrel: -31.0,
        yrel: -9.0,
    },
    FoundationGfx {
        w: 64.0,
        h: 32.0,
        xrel: -31.0,
        yrel: -9.0,
    },
    FoundationGfx {
        w: 64.0,
        h: 40.0,
        xrel: -31.0,
        yrel: -9.0,
    },
    FoundationGfx {
        w: 64.0,
        h: 40.0,
        xrel: -31.0,
        yrel: -9.0,
    },
    FoundationGfx {
        w: 64.0,
        h: 32.0,
        xrel: -31.0,
        yrel: -9.0,
    },
];

#[must_use]
pub fn foundation_sprite_id(tileh: u8) -> Option<u32> {
    if (1..=14).contains(&tileh) {
        Some(FOUNDATION_SPRITE_BASE + u32::from(tileh))
    } else {
        None
    }
}

#[must_use]
pub fn foundation_gfx_for_tileh(tileh: u8) -> Option<&'static FoundationGfx> {
    if (1..=14).contains(&tileh) {
        Some(&FOUNDATION_LEVELED_GFX[(tileh - 1) as usize])
    } else {
        None
    }
}

#[must_use]
pub fn foundation_asset_path(tileh: u8) -> Option<String> {
    foundation_sprite_id(tileh).map(|_| format!("assets/opengfx/tiles/foundation_{tileh:02}.png"))
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::{
        FOUNDATION_SPRITE_BASE, foundation_asset_path, foundation_gfx_for_tileh,
        foundation_sprite_id,
    };

    #[test]
    fn foundation_sprite_ids_match_openttd_table() {
        assert_eq!(foundation_sprite_id(0), None);
        assert_eq!(foundation_sprite_id(1), Some(990));
        assert_eq!(foundation_sprite_id(14), Some(1003));
        assert_eq!(foundation_sprite_id(15), None);
    }

    #[test]
    fn gfx_and_paths_for_all_slopes() {
        for tileh in 1..=14 {
            assert!(foundation_gfx_for_tileh(tileh).is_some());
            assert_eq!(
                foundation_sprite_id(tileh),
                Some(FOUNDATION_SPRITE_BASE + u32::from(tileh))
            );
            let path = foundation_asset_path(tileh).expect("path 1..14");
            assert!(path.contains("foundation_"));
        }
    }
}
