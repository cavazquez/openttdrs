//! Mapeo de sprites de industria (OpenGFX).
//!
//! Los valores numéricos del array incluido salen de OpenTTD `src/table/industry_land.h`:
//! por cada `gfx` se usa la fila del **estadio 3** (`GetIndustryGfx` construcción terminada),
//! índice `gfx * 4 + 3`. Macros `M(s1, …, s2, …)`: `ground_sprite_id` ≈ s1 (salvo hierba 0xF54),
//! `sprite_id` = s2.

use std::sync::{Mutex, OnceLock};

fn logged_gfx() -> &'static Mutex<Vec<u16>> {
    static LOGGED: OnceLock<Mutex<Vec<u16>>> = OnceLock::new();
    LOGGED.get_or_init(|| Mutex::new(Vec::new()))
}

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

const FALLBACK_WH: (f32, f32) = (64.0, 48.0);

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

/// `true` si la fila usa offsets genéricos (sin arte propio o calibración PNG).
#[must_use]
pub fn industry_gfx_uses_generic_fallback(entry: &IndustryGfxSprite) -> bool {
    entry.sprite_id == 0
        && entry.ground_sprite_id == 0
        && (entry.w, entry.h) == FALLBACK_WH
        && entry.xrel == -32.0
        && entry.yrel == -32.0
}

/// Registra una vez por sesión los `gfx` sin sprites o con fallback genérico.
pub fn debug_log_industry_gfx_once(gfx: u16, entry: Option<&IndustryGfxSprite>) {
    if !cfg!(debug_assertions) {
        return;
    }
    let Ok(mut seen) = logged_gfx().lock() else {
        return;
    };
    if seen.contains(&gfx) {
        return;
    }
    let msg = match entry {
        None => Some(format!(
            "industria gfx {gfx}: sin entrada en INDUSTRY_GFX_DATA"
        )),
        Some(e) if industry_gfx_uses_generic_fallback(e) => Some(format!(
            "industria gfx {gfx}: fallback genérico (sin PNG calibrado)"
        )),
        Some(e) if e.sprite_id == 0 && e.ground_sprite_id == 0 => Some(format!(
            "industria gfx {gfx}: sin sprite_id ni ground_sprite_id"
        )),
        _ => None,
    };
    if let Some(m) = msg {
        bevy::log::debug!("{m}");
        seen.push(gfx);
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod industry_coverage_tests {
    use super::{
        INDUSTRY_GFX_DATA, industry_gfx_entry, industry_gfx_uses_generic_fallback,
        industry_sprite_for_gfx,
    };

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

    #[test]
    fn coal_mine_gfx0_has_calibrated_dims() {
        let e = industry_gfx_entry(0).expect("gfx 0");
        assert_eq!(e.sprite_id, 2013);
        assert!(e.w > 40.0 && e.h > 30.0);
        assert!(!industry_gfx_uses_generic_fallback(e));
    }

    #[test]
    fn most_gfx_rows_are_png_calibrated_not_generic_fallback() {
        let generic = INDUSTRY_GFX_DATA
            .iter()
            .filter(|e| industry_gfx_uses_generic_fallback(e))
            .count();
        assert!(
            generic < 10,
            "demasiadas filas con fallback genérico: {generic}"
        );
    }
}
