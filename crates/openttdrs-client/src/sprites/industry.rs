//! Mapeo de sprites de industria (OpenGFX).
//!
//! Los valores numéricos del array incluido salen de OpenTTD `src/table/industry_land.h`:
//! por cada `gfx` hay **4 filas** (estadios 0–3), índice `gfx * 4 + GetIndustryConstructionStage()`.
//! `sprite_id` = s2.

use std::sync::{Mutex, OnceLock};

fn logged_gfx() -> &'static Mutex<Vec<u16>> {
    static LOGGED: OnceLock<Mutex<Vec<u16>>> = OnceLock::new();
    LOGGED.get_or_init(|| Mutex::new(Vec::new()))
}

/// Metadatos para dibujar un tile de industria (por estadio de construcción).
pub struct IndustryGfxSprite {
    /// Sprite del edificio / overlay (`s2` en `M()`). `0` = sin overlay estático.
    pub sprite_id: u32,
    /// Sprite de suelo propio de la industria (`s1`). `0` = no dibujar capa extra.
    pub ground_sprite_id: u32,
    /// Edificio: tamaño y anclaje NFO (`overlay_pos`).
    pub w: f32,
    pub h: f32,
    pub xrel: f32,
    pub yrel: f32,
    /// Suelo industrial: tamaño y anclaje NFO (capa debajo del edificio).
    pub ground_w: f32,
    pub ground_h: f32,
    pub ground_xrel: f32,
    pub ground_yrel: f32,
}

include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/src/sprites/industry_gfx_data_generated.rs"
));

const FALLBACK_WH: (f32, f32) = (64.0, 48.0);

/// Filas `gfx` en tabla (0..=119). Valores ≥120 no tienen entrada.
pub const INDUSTRY_GFX_TABLE_LEN: u16 = 120;

/// Estadios por `gfx` (OpenTTD `_industry_draw_tile_data`: 0–3).
pub const INDUSTRY_GFX_STAGES: usize = 4;

/// Etapa de obra desde `m1` (`GetIndustryConstructionStage` / `IsIndustryCompleted`).
#[must_use]
pub fn industry_construction_stage_from_tile(m1: u8) -> usize {
    if m1 & 0x80 != 0 {
        3
    } else {
        usize::from(m1 & 0x03).min(3)
    }
}

/// Índice en [`INDUSTRY_GFX_DATA`]: `gfx * 4 + stage`.
#[must_use]
pub fn industry_gfx_draw_index(gfx: u16, stage: usize) -> Option<usize> {
    if gfx >= INDUSTRY_GFX_TABLE_LEN {
        return None;
    }
    let stage = stage.min(INDUSTRY_GFX_STAGES - 1);
    Some(usize::from(gfx) * INDUSTRY_GFX_STAGES + stage)
}

/// Estado de resolución visual para un `gfx` de 9 bits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndustryGfxStatus {
    /// Fila en tabla con al menos un sprite (`sprite_id` o `ground_sprite_id`).
    Resolved,
    /// Fila en tabla pero sin PNGs (`sprite_id` y `ground_sprite_id` = 0).
    EmptyRow,
    /// `gfx` ≥ [`INDUSTRY_GFX_TABLE_LEN`] — fuera de `_industry_draw_tile_data` cargado.
    OutOfRange,
}

/// Clasifica el `gfx` para render/HUD (sin cargar NewGRF extra).
#[must_use]
pub fn industry_gfx_status(gfx: u16) -> IndustryGfxStatus {
    if gfx >= INDUSTRY_GFX_TABLE_LEN {
        return IndustryGfxStatus::OutOfRange;
    }
    match industry_gfx_entry_staged(gfx, 3) {
        None => IndustryGfxStatus::OutOfRange,
        Some(e) if e.sprite_id == 0 && e.ground_sprite_id == 0 => IndustryGfxStatus::EmptyRow,
        Some(_) => IndustryGfxStatus::Resolved,
    }
}

/// Texto corto para HUD / logs.
#[must_use]
pub fn industry_gfx_status_label(status: IndustryGfxStatus) -> &'static str {
    match status {
        IndustryGfxStatus::Resolved => "ok",
        IndustryGfxStatus::EmptyRow => "sin sprite",
        IndustryGfxStatus::OutOfRange => "gfx≥120",
    }
}

/// Fila para `gfx` y estadio (0–3). Preview/plantillas terminadas: estadio **3**.
#[must_use]
pub fn industry_gfx_entry_staged(gfx: u16, stage: usize) -> Option<&'static IndustryGfxSprite> {
    industry_gfx_draw_index(gfx, stage).and_then(|i| INDUSTRY_GFX_DATA.get(i))
}

/// Fila según bytes de tesela (`m1` = etapa + terminada).
#[must_use]
pub fn industry_gfx_entry_for_tile(gfx: u16, m1: u8) -> Option<&'static IndustryGfxSprite> {
    industry_gfx_entry_staged(gfx, industry_construction_stage_from_tile(m1))
}

/// Estadio **3** (industria terminada). Alias histórico.
#[must_use]
pub fn industry_gfx_entry(gfx: u16) -> Option<&'static IndustryGfxSprite> {
    industry_gfx_entry_staged(gfx, 3)
}

/// Solo overlay de edificio (si existe). Usado donde solo importa el sprite encima del suelo.
#[allow(dead_code)] // expuesto vía `crate::sprites` para API / documentación
#[must_use]
pub fn industry_sprite_for_gfx(gfx: u16) -> Option<&'static IndustryGfxSprite> {
    let entry = industry_gfx_entry_staged(gfx, 3)?;
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

/// Registra una vez por sesión los `gfx` problemáticos (debug + warn en release).
pub fn log_industry_gfx_once(gfx: u16, entry: Option<&IndustryGfxSprite>) {
    let status = industry_gfx_status(gfx);
    if status == IndustryGfxStatus::Resolved {
        if cfg!(debug_assertions)
            && let Some(e) = entry
        {
            if industry_gfx_uses_generic_fallback(e) {
                log_industry_problem_once(gfx, "fallback genérico (sin PNG calibrado)");
            } else if e.sprite_id == 0 && e.ground_sprite_id == 0 {
                log_industry_problem_once(gfx, "sin sprite_id ni ground_sprite_id");
            }
        }
        return;
    }
    let reason = match status {
        IndustryGfxStatus::OutOfRange => {
            "sin entrada en INDUSTRY_GFX_DATA (gfx≥120 o fuera de tabla)"
        }
        IndustryGfxStatus::EmptyRow => "fila vacía en INDUSTRY_GFX_DATA",
        IndustryGfxStatus::Resolved => return,
    };
    log_industry_problem_once(gfx, reason);
}

/// Compat: alias debug histórico (reexportado en `crate::sprites`).
#[allow(dead_code)]
pub fn debug_log_industry_gfx_once(gfx: u16, entry: Option<&IndustryGfxSprite>) {
    log_industry_gfx_once(gfx, entry);
}

fn log_industry_problem_once(gfx: u16, reason: &str) {
    let Ok(mut seen) = logged_gfx().lock() else {
        return;
    };
    if seen.contains(&gfx) {
        return;
    }
    bevy::log::warn!("industria gfx {gfx}: {reason}");
    if cfg!(debug_assertions) {
        bevy::log::debug!("industria gfx {gfx}: {reason}");
    }
    seen.push(gfx);
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod industry_coverage_tests {
    use super::{
        INDUSTRY_GFX_DATA, INDUSTRY_GFX_STAGES, INDUSTRY_GFX_TABLE_LEN, IndustryGfxStatus,
        industry_construction_stage_from_tile, industry_gfx_draw_index, industry_gfx_entry,
        industry_gfx_entry_for_tile, industry_gfx_entry_staged, industry_gfx_status,
        industry_gfx_uses_generic_fallback, industry_sprite_for_gfx,
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
        assert_eq!(e.ground_sprite_id, 2022);
        assert!((e.w - 58.0).abs() < 0.1);
        assert!((e.h - 50.0).abs() < 0.1);
        assert!((e.xrel - (-16.0)).abs() < 0.1);
        assert!((e.yrel - (-33.0)).abs() < 0.1);
        assert!((e.ground_w - 64.0).abs() < 0.1);
        assert!((e.ground_xrel - (-31.0)).abs() < 0.1);
        assert!(!industry_gfx_uses_generic_fallback(e));
    }

    #[test]
    fn power_station_gfx7_uses_nfo_offsets() {
        let e = industry_gfx_entry(7).expect("gfx 7");
        assert_eq!(e.sprite_id, 2047);
        assert!((e.xrel - (-21.0)).abs() < 0.1);
        assert!((e.yrel - (-34.0)).abs() < 0.1);
    }

    #[test]
    fn gfx_at_table_limit_is_resolved_or_empty_not_out_of_range() {
        assert_ne!(industry_gfx_status(119), IndustryGfxStatus::OutOfRange);
    }

    #[test]
    fn gfx_120_and_above_are_out_of_range() {
        assert_eq!(industry_gfx_status(120), IndustryGfxStatus::OutOfRange);
        assert_eq!(industry_gfx_status(256), IndustryGfxStatus::OutOfRange);
        assert!(industry_gfx_entry(120).is_none());
    }

    #[test]
    fn table_len_is_gfx_times_stages() {
        assert_eq!(
            INDUSTRY_GFX_DATA.len(),
            usize::from(INDUSTRY_GFX_TABLE_LEN) * INDUSTRY_GFX_STAGES
        );
    }

    #[test]
    fn construction_stage_from_m1_matches_openttd() {
        assert_eq!(industry_construction_stage_from_tile(0), 0);
        assert_eq!(industry_construction_stage_from_tile(2), 2);
        assert_eq!(industry_construction_stage_from_tile(0x80), 3);
        assert_eq!(industry_construction_stage_from_tile(0x82), 3);
    }

    #[test]
    fn gfx0_construction_stages_use_distinct_sprites() {
        let s0 = industry_gfx_entry_staged(0, 0).expect("stage 0");
        let s1 = industry_gfx_entry_staged(0, 1).expect("stage 1");
        let s2 = industry_gfx_entry_staged(0, 2).expect("stage 2");
        let done = industry_gfx_entry_staged(0, 3).expect("stage 3");
        assert_eq!(s0.sprite_id, 2011);
        assert_eq!(s1.sprite_id, 2012);
        assert_eq!(s2.sprite_id, 2013);
        assert_eq!(done.sprite_id, 2013);
        assert_eq!(done.ground_sprite_id, 2022);
        assert_ne!(s0.sprite_id, done.sprite_id);
    }

    #[test]
    fn entry_for_tile_uses_m1_stage() {
        let under = industry_gfx_entry_for_tile(0, 1).expect("m1=1");
        let done = industry_gfx_entry_for_tile(0, 0x80).expect("m1=0x80");
        assert_eq!(under.sprite_id, 2012);
        assert_eq!(done.ground_sprite_id, 2022);
    }

    #[test]
    fn draw_index_roundtrip() {
        assert_eq!(
            industry_gfx_draw_index(7, 2),
            Some(7 * INDUSTRY_GFX_STAGES + 2)
        );
        assert_eq!(industry_gfx_draw_index(120, 0), None);
    }

    #[test]
    fn table_len_matches_openrtd_gfx_band() {
        assert_eq!(
            INDUSTRY_GFX_TABLE_LEN as usize * INDUSTRY_GFX_STAGES,
            INDUSTRY_GFX_DATA.len()
        );
    }
}
