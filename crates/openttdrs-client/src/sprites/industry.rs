//! Mapeo de sprites de industria (OpenGFX).
//!
//! Los valores numéricos del array incluido salen de OpenTTD `src/table/industry_land.h`:
//! por cada `gfx` se usa la fila del **estadio 3** (`GetIndustryGfx` construcción terminada),
//! índice `gfx * 4 + 3`. Macros `M(s1, …, s2, …)`: `ground_sprite_id` ≈ s1 (salvo hierba 0xF54),
//! `sprite_id` = s2.

/// Metadatos para dibujar un tile de industria (estadio 3).
pub struct IndustryGfxSprite {
    /// Sprite del edificio / overlay (`s2` en `M()`). `0` = sin overlay estático.
    pub sprite_id: u32,
    /// Sprite de suelo propio de la industria (`s1`). `0` = no dibujar capa extra.
    /// La hierba estándar temperate (`0xF54`) no se lista para no duplicar pasto.
    pub ground_sprite_id: u32,
    pub w: f32,
    pub h: f32,
    pub xrel: f32,
    pub yrel: f32,
}

include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/src/sprites/industry_gfx_data_generated.rs"
));

/// Fila completa del tile de industria para el `gfx` de 9 bits.
#[must_use]
pub fn industry_gfx_entry(gfx: u16) -> Option<&'static IndustryGfxSprite> {
    INDUSTRY_GFX_DATA.get(usize::from(gfx))
}

/// Solo overlay de edificio (si existe). Usado donde solo importa el sprite encima del suelo.
#[allow(dead_code)] // expuesto vía `crate::sprites` para API / documentación
#[must_use]
pub fn industry_sprite_for_gfx(gfx: u16) -> Option<&'static IndustryGfxSprite> {
    let entry = INDUSTRY_GFX_DATA.get(usize::from(gfx))?;
    if entry.sprite_id != 0 {
        Some(entry)
    } else {
        None
    }
}

#[cfg(test)]
mod industry_coverage_tests {
    use super::{industry_gfx_entry, industry_sprite_for_gfx};

    #[test]
    fn industry_gfx_entry_hits_table() {
        let _ = industry_gfx_entry(0);
        let _ = industry_gfx_entry(1);
        let _ = industry_gfx_entry(42);
    }

    #[test]
    fn industry_sprite_for_gfx_smoke() {
        let _ = industry_sprite_for_gfx(0);
        let _ = industry_sprite_for_gfx(1);
    }
}
