//! Mapeo de sprites de industria (OpenGFX).

/// Metadatos de un sprite de tile de industria.
pub struct IndustryGfxSprite {
    /// Sprite ID en OpenGFX (0 = solo suelo, sin overlay de edificio).
    pub sprite_id: u32,
    pub w: f32,
    pub h: f32,
    /// Offset horizontal desde el vertice superior del rombo (pantalla).
    pub xrel: f32,
    /// Offset vertical hacia arriba desde el vertice (positivo = mas arriba en NFO = negativo yrel).
    pub yrel: f32,
}

/// Default generico para edificios cuyas dimensiones exactas no se han calibrado aun.
/// Centra un sprite 64x48 sobre el tile.
const fn gfx_building(sprite_id: u32) -> IndustryGfxSprite {
    IndustryGfxSprite {
        sprite_id,
        w: 64.0,
        h: 48.0,
        xrel: -32.0,
        yrel: -32.0,
    }
}

const fn gfx_ground() -> IndustryGfxSprite {
    IndustryGfxSprite {
        sprite_id: 0,
        w: 0.0,
        h: 0.0,
        xrel: 0.0,
        yrel: 0.0,
    }
}

/// Tabla gfx -> sprite para todos los climas de OpenTTD.
pub const INDUSTRY_GFX_DATA: [IndustryGfxSprite; 120] = [
    IndustryGfxSprite { sprite_id: 2013, w: 58.0, h: 50.0, xrel: -16.0, yrel: -33.0 },
    IndustryGfxSprite { sprite_id: 2015, w: 46.0, h: 53.0, xrel: -14.0, yrel: -38.0 },
    IndustryGfxSprite { sprite_id: 2018, w: 64.0, h: 39.0, xrel: -31.0, yrel: -8.0 },
    IndustryGfxSprite { sprite_id: 2021, w: 44.0, h: 38.0, xrel: -13.0, yrel: -21.0 },
    gfx_ground(),
    gfx_ground(),
    gfx_ground(),
    gfx_building(2047), gfx_building(2050), gfx_building(2053), gfx_building(2054),
    gfx_building(2063), gfx_building(2066), gfx_building(2069), gfx_building(2070), gfx_building(2071),
    gfx_building(2075), gfx_building(2076), gfx_building(2080), gfx_building(2083), gfx_building(2086),
    gfx_building(2089), gfx_building(2092), gfx_building(2095),
    gfx_ground(), gfx_building(2099), gfx_building(2100), gfx_building(2101), gfx_building(2102),
    gfx_building(2174), gfx_building(2178), gfx_building(2177), gfx_building(2174),
    gfx_building(2108), gfx_building(2109), gfx_building(2111), gfx_building(2113), gfx_building(2115), gfx_building(2117),
    gfx_building(2150), gfx_building(2151), gfx_building(2152), gfx_ground(),
    gfx_building(2169), gfx_building(2170), gfx_building(2171), gfx_building(2172),
    gfx_building(2028), gfx_building(2030), gfx_building(2033), gfx_building(2036), gfx_building(2039),
    gfx_building(2119), gfx_building(2121), gfx_building(2123), gfx_ground(), gfx_building(2126), gfx_building(2128),
    gfx_building(2180), gfx_building(2181),
    gfx_building(2190), gfx_building(2193), gfx_building(2196), gfx_building(2199), gfx_building(2202), gfx_building(2214),
    gfx_building(2205), gfx_building(2206), gfx_building(2208), gfx_building(2209), gfx_building(2212), gfx_building(2213),
    gfx_building(2247), gfx_ground(), gfx_building(2249), gfx_building(2250), gfx_ground(), gfx_ground(), gfx_ground(), gfx_building(2263),
    gfx_ground(), gfx_ground(), gfx_ground(), gfx_ground(), gfx_ground(), gfx_ground(), gfx_ground(), gfx_ground(), gfx_building(2265),
    gfx_building(2186), gfx_building(2187),
    gfx_building(2284), gfx_building(2285), gfx_building(2286), gfx_building(2287), gfx_ground(), gfx_ground(), gfx_building(2290), gfx_ground(), gfx_ground(),
    gfx_ground(), gfx_ground(), gfx_ground(), gfx_ground(), gfx_ground(), gfx_ground(), gfx_ground(), gfx_ground(),
    gfx_ground(), gfx_ground(), gfx_ground(), gfx_ground(), gfx_ground(), gfx_ground(), gfx_ground(), gfx_ground(),
    gfx_building(2342), gfx_building(2343), gfx_building(2349), gfx_building(2352),
];

/// Devuelve los metadatos del sprite de industria para el gfx dado (byte m5).
pub fn industry_sprite_for_gfx(gfx: u16) -> Option<&'static IndustryGfxSprite> {
    let entry = INDUSTRY_GFX_DATA.get(usize::from(gfx))?;
    if entry.sprite_id != 0 {
        Some(entry)
    } else {
        None
    }
}
