//! Mapeo de sprites de industria (OpenGFX).
//!
//! Los valores numéricos del array incluido salen de OpenTTD `src/table/industry_land.h`:
//! por cada `gfx` hay **4 filas** (estadios 0–3 o frames de animación), índice
//! `gfx * 4 + subíndice`. Con `anim_state`, el subíndice es `m4 & 3`; si no,
//! etapa de obra desde `m1` (`GetIndustryConstructionStage`).

use std::sync::{Mutex, OnceLock};

#[path = "industry_anim_state_generated.rs"]
mod anim_state_generated;
use anim_state_generated::INDUSTRY_TILE_ANIM_STATE;

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

/// Filas `gfx` en tabla (0..=174). Valores ≥175 no tienen entrada.
pub const INDUSTRY_GFX_TABLE_LEN: u16 = 175;

/// Estadios por `gfx` (OpenTTD `_industry_draw_tile_data`: 0–3).
pub const INDUSTRY_GFX_STAGES: usize = 4;

/// Máscara de frame de animación (`GetAnimationFrame & INDUSTRY_COMPLETED` en OpenTTD).
pub const INDUSTRY_ANIM_FRAME_MASK: u8 = 3;

/// Etapa de obra desde `m1` (`GetIndustryConstructionStage` / `IsIndustryCompleted`).
#[must_use]
pub fn industry_construction_stage_from_tile(m1: u8) -> usize {
    usize::from(openttdrs_core::industry_construction_stage(m1))
}

/// `IndustryTileSpec.anim_state` para este gfx.
#[must_use]
pub fn industry_tile_anim_state(gfx: u16) -> bool {
    INDUSTRY_TILE_ANIM_STATE
        .get(usize::from(gfx))
        .copied()
        .unwrap_or(false)
}

/// Frame de animación desde `m4`/`m3hi` del mapa OpenTTD.
#[must_use]
pub fn industry_animation_frame_from_m4(m4: u8) -> usize {
    usize::from(m4 & INDUSTRY_ANIM_FRAME_MASK)
}

/// Subíndice en `_industry_draw_tile_data` (`DrawTile_Industry` en `industry_cmd.cpp`).
#[must_use]
pub fn industry_gfx_table_subindex(gfx: u16, m1: u8, m4: u8) -> usize {
    if industry_tile_anim_state(gfx) {
        industry_animation_frame_from_m4(m4)
    } else {
        industry_construction_stage_from_tile(m1)
    }
}

/// Tesela terminada con capas que ciclan por `anim_state`.
#[must_use]
pub fn industry_building_needs_client_anim(gfx: u16, m1: u8) -> bool {
    industry_tile_anim_state(gfx) && m1 & 0x80 != 0
}

/// Torres de llama refinería (gfx 19–22): animación `oil_refinery` de paleta.
pub const REFINERY_FIRE_GFX_MIN: u16 = 19;
pub const REFINERY_FIRE_GFX_MAX: u16 = 22;

/// Sprites OpenGFX con llama (`_industry_draw_tile_data` gfx 19–22).
pub const REFINERY_FIRE_SPRITE_IDS: [u32; 12] = [
    2081, 2082, 2083, 2084, 2085, 2086, 2087, 2088, 2089, 2090, 2091, 2092,
];

/// Tesela terminada con fuego animado por ciclo de paleta.
#[must_use]
pub fn industry_gfx_uses_refinery_fire_anim(gfx: u16, m1: u8) -> bool {
    (REFINERY_FIRE_GFX_MIN..=REFINERY_FIRE_GFX_MAX).contains(&gfx) && m1 & 0x80 != 0
}

/// Fábrica de bebidas gaseosas Toyland (gfx 156–158).
pub const FIZZY_DRINK_GFX_MIN: u16 = 156;
pub const FIZZY_DRINK_GFX_MAX: u16 = 158;

/// Sprites con ciclo `fizzy_drink` (edificio + draw proc burbujas).
pub const FIZZY_DRINK_SPRITE_IDS: [u32; 5] = [4763, 4764, 4765, 4746, 4747];

/// Tesela terminada con burbujas/líquido animado por paleta.
#[must_use]
pub fn industry_gfx_uses_fizzy_drink_anim(gfx: u16, m1: u8) -> bool {
    (FIZZY_DRINK_GFX_MIN..=FIZZY_DRINK_GFX_MAX).contains(&gfx) && m1 & 0x80 != 0
}

#[must_use]
pub fn industry_sprite_uses_fizzy_drink_anim(sprite_id: u32) -> bool {
    FIZZY_DRINK_SPRITE_IDS.contains(&sprite_id)
}

/// Edificios con `PALETTE_MODIFIER_COLOUR` en tabla vanilla (gfx 29–174,
/// excl. pozos/torres animados 30–32, 48, 88).
#[must_use]
pub fn industry_gfx_uses_random_colour(gfx: u16) -> bool {
    (29..=174).contains(&gfx) && !matches!(gfx, 30 | 31 | 32 | 48 | 88)
}

/// Color de compañía OpenTTD para la instancia (`Industry.random_colour` vía `m2`).
#[must_use]
pub fn industry_palette_colour_for_instance(
    instance_id: u8,
    industries: &[openttdrs_core::Industry],
) -> crate::sprites::CompanyColour {
    if instance_id == 0 {
        return crate::sprites::CompanyColour::DarkBlue;
    }
    if let Some(ind) = industries.iter().find(|i| i.instance_id == instance_id) {
        return crate::sprites::CompanyColour::from_u8(ind.random_colour);
    }
    // Fallback: índice secuencial legacy o hash del id.
    let idx = usize::from(instance_id.saturating_sub(1));
    crate::sprites::CompanyColour::from_u8(
        industries
            .get(idx)
            .map(|i| i.random_colour)
            .unwrap_or_else(|| instance_id.wrapping_mul(5) % 16),
    )
}

/// Algún frame de animación dibuja esta capa (suelo o edificio).
#[must_use]
pub fn industry_anim_layer_used_in_any_frame(gfx: u16, ground: bool) -> bool {
    if !industry_tile_anim_state(gfx) {
        return false;
    }
    (0..INDUSTRY_GFX_STAGES).any(|frame| {
        industry_gfx_entry_staged(gfx, frame).is_some_and(|e| {
            if ground {
                e.ground_sprite_id != 0 && e.ground_w > 0.0 && e.ground_h > 0.0
            } else {
                e.sprite_id != 0 && e.w > 0.0 && e.h > 0.0
            }
        })
    })
}

/// `m4` efectivo al dibujar: frame en `m3hi` (P7 tile loop en sim).
#[must_use]
pub fn industry_effective_m4_for_draw(
    _gfx: u16,
    _m1: u8,
    m4: u8,
    _elapsed_secs: f32,
    _phase: u8,
) -> u8 {
    m4
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
        IndustryGfxStatus::OutOfRange => "gfx≥175",
    }
}

/// Fila para `gfx` y estadio (0–3). Preview/plantillas terminadas: estadio **3**.
#[must_use]
pub fn industry_gfx_entry_staged(gfx: u16, stage: usize) -> Option<&'static IndustryGfxSprite> {
    industry_gfx_draw_index(gfx, stage).and_then(|i| INDUSTRY_GFX_DATA.get(i))
}

/// Fila según bytes de tesela (`m1` etapa, `m4`/`m3hi` frame si `anim_state`).
#[must_use]
pub fn industry_gfx_entry_for_tile(gfx: u16, m1: u8, m4: u8) -> Option<&'static IndustryGfxSprite> {
    industry_gfx_entry_staged(gfx, industry_gfx_table_subindex(gfx, m1, m4))
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

/// Fila vacía en etapa de obra cuando upstream también omite overlay (`s2 = 0`).
#[must_use]
pub fn industry_gfx_empty_row_is_expected(gfx: u16, stage: usize) -> bool {
    if stage >= INDUSTRY_GFX_STAGES - 1 {
        return false;
    }
    let Some(under) = industry_gfx_entry_staged(gfx, stage) else {
        return false;
    };
    if under.sprite_id != 0 || under.ground_sprite_id != 0 {
        return false;
    }
    let Some(done) = industry_gfx_entry_staged(gfx, 3) else {
        return false;
    };
    done.sprite_id != 0 || done.ground_sprite_id != 0
}

/// Registra una vez por sesión los `gfx` problemáticos (debug + warn en release).
pub fn log_industry_gfx_once(gfx: u16, m1: u8, m4: u8, entry: Option<&IndustryGfxSprite>) {
    let stage = industry_gfx_table_subindex(gfx, m1, m4);
    let status = industry_gfx_status(gfx);
    if status == IndustryGfxStatus::Resolved {
        if cfg!(debug_assertions)
            && let Some(e) = entry
            && !industry_gfx_empty_row_is_expected(gfx, stage)
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
            "sin entrada en INDUSTRY_GFX_DATA (gfx≥175 o fuera de tabla)"
        }
        IndustryGfxStatus::EmptyRow => "fila vacía en INDUSTRY_GFX_DATA",
        IndustryGfxStatus::Resolved => return,
    };
    log_industry_problem_once(gfx, reason);
}

/// Compat: alias debug histórico (reexportado en `crate::sprites`).
#[allow(dead_code)]
pub fn debug_log_industry_gfx_once(gfx: u16, m1: u8, m4: u8, entry: Option<&IndustryGfxSprite>) {
    log_industry_gfx_once(gfx, m1, m4, entry);
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
        industry_animation_frame_from_m4, industry_building_needs_client_anim,
        industry_construction_stage_from_tile, industry_gfx_draw_index,
        industry_gfx_empty_row_is_expected, industry_gfx_entry, industry_gfx_entry_for_tile,
        industry_gfx_entry_staged, industry_gfx_status, industry_gfx_status_label,
        industry_gfx_table_subindex, industry_gfx_uses_generic_fallback, industry_sprite_for_gfx,
        industry_tile_anim_state,
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
        assert!((e.xrel - (-14.0)).abs() < 0.1);
        assert!((e.yrel - (-35.0)).abs() < 0.1);
        assert!((e.ground_w - 64.0).abs() < 0.1);
        assert!((e.ground_xrel - (-31.0)).abs() < 0.1);
        assert!(!industry_gfx_uses_generic_fallback(e));
    }

    #[test]
    fn power_station_gfx7_uses_nfo_offsets() {
        let e = industry_gfx_entry(7).expect("gfx 7");
        assert_eq!(e.sprite_id, 2047);
        assert!((e.xrel - (-21.0)).abs() < 0.1);
        assert!((e.yrel - (-35.0)).abs() < 0.1);
    }

    #[test]
    fn gfx_at_table_limit_is_resolved_or_empty_not_out_of_range() {
        assert_ne!(industry_gfx_status(119), IndustryGfxStatus::OutOfRange);
    }

    #[test]
    fn gfx_120_through_130_in_table() {
        for gfx in 120..=130 {
            assert_ne!(
                industry_gfx_status(gfx),
                IndustryGfxStatus::OutOfRange,
                "gfx {gfx} debe estar en tabla"
            );
        }
    }

    #[test]
    fn gfx_131_and_above_in_range_until_174() {
        assert_ne!(industry_gfx_status(131), IndustryGfxStatus::OutOfRange);
        assert_ne!(industry_gfx_status(174), IndustryGfxStatus::OutOfRange);
        assert_eq!(industry_gfx_status(175), IndustryGfxStatus::OutOfRange);
        assert!(industry_gfx_entry(131).is_some());
    }

    const SP3_VISUAL_CHECKLIST: &[u8] =
        include_bytes!("../../../openttdrs-core/tests/fixtures/sp3_visual_checklist.ottdmap");

    /// Regresión SP3 P1: y=10 del checklist (gfx 0…120 en tabla; 256 = NewGRF / OutOfRange).
    #[test]
    fn sp3_visual_checklist_industry_gfx_in_table() {
        use openttdrs_core::prelude::*;

        let map = Map::from_ottd_binary(SP3_VISUAL_CHECKLIST).expect("checklist MAP1");
        let cases: &[(i32, u16, IndustryGfxStatus)] = &[
            (1, 0, IndustryGfxStatus::Resolved),
            (3, 42, IndustryGfxStatus::Resolved),
            (5, 116, IndustryGfxStatus::Resolved),
            (7, 119, IndustryGfxStatus::Resolved),
            (9, 120, IndustryGfxStatus::Resolved),
            (11, 256, IndustryGfxStatus::OutOfRange),
        ];
        for &(x, expect_gfx, expect_status) in cases {
            let t = map.get(TileCoord::new(x, 10)).expect("industry tile");
            assert_eq!(t.kind, TileKind::Industry, "x={x}");
            let gfx9 = u16::from(t.m5) | (u16::from((t.m6 >> 2) & 1) << 8);
            assert_eq!(gfx9, expect_gfx, "x={x} gfx9");
            assert_eq!(industry_gfx_status(gfx9), expect_status, "x={x} gfx={gfx9}");
        }
        assert_eq!(
            industry_gfx_status_label(IndustryGfxStatus::OutOfRange),
            "gfx≥175"
        );
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
        let under = industry_gfx_entry_for_tile(0, 1, 0).expect("m1=1");
        let done = industry_gfx_entry_for_tile(0, 0x80, 0).expect("m1=0x80");
        assert_eq!(under.sprite_id, 2012);
        assert_eq!(done.ground_sprite_id, 2022);
    }

    #[test]
    fn anim_state_gfx_uses_m4_subindex() {
        assert!(industry_tile_anim_state(1));
        assert!(!industry_tile_anim_state(0));
        assert_eq!(industry_gfx_table_subindex(0, 0x80, 2), 3);
        assert_eq!(industry_gfx_table_subindex(1, 0x80, 2), 2);
        let e = industry_gfx_entry_for_tile(1, 0x80, 2).expect("gfx1 frame2");
        assert_eq!(e.sprite_id, 2015);
        let e0 = industry_gfx_entry_for_tile(1, 0x80, 0).expect("gfx1 frame0");
        assert_eq!(e0.sprite_id, 2013);
    }

    #[test]
    fn building_anim_frame_cycles() {
        assert_eq!(industry_animation_frame_from_m4(0xC2), 2);
    }

    #[test]
    fn coal_tower_needs_client_anim_when_complete() {
        assert!(industry_building_needs_client_anim(1, 0x80));
        assert!(!industry_building_needs_client_anim(1, 1));
        assert!(!industry_building_needs_client_anim(0, 0x80));
    }

    #[test]
    fn draw_index_roundtrip() {
        assert_eq!(
            industry_gfx_draw_index(7, 2),
            Some(7 * INDUSTRY_GFX_STAGES + 2)
        );
        assert_eq!(
            industry_gfx_draw_index(120, 0),
            Some(120 * INDUSTRY_GFX_STAGES)
        );
        assert_eq!(industry_gfx_draw_index(131, 0), Some(131 * 4));
        assert_eq!(industry_gfx_draw_index(174, 0), Some(174 * 4));
        assert_eq!(industry_gfx_draw_index(175, 0), None);
    }

    #[test]
    fn table_len_matches_openrtd_gfx_band() {
        assert_eq!(
            INDUSTRY_GFX_TABLE_LEN as usize * INDUSTRY_GFX_STAGES,
            INDUSTRY_GFX_DATA.len()
        );
    }

    #[test]
    fn sawmill_gfx_14_15_empty_construction_rows_are_expected() {
        for gfx in [14_u16, 15] {
            for stage in 0..3 {
                assert!(
                    industry_gfx_empty_row_is_expected(gfx, stage),
                    "gfx {gfx} stage {stage}"
                );
            }
            assert!(!industry_gfx_empty_row_is_expected(gfx, 3));
            let done = industry_gfx_entry_staged(gfx, 3).expect("terminada");
            assert_ne!(done.sprite_id, 0);
        }
    }

    #[test]
    fn toyland_gfx_uses_random_colour() {
        assert!(super::industry_gfx_uses_random_colour(143)); // toy factory band
        assert!(!super::industry_gfx_uses_random_colour(30)); // oil well anim
        assert!(!super::industry_gfx_uses_random_colour(10)); // below band
    }

    #[test]
    fn palette_colour_looks_up_by_instance_id_not_vector_index() {
        use openttdrs_core::prelude::*;
        use openttdrs_core::{Industry, IndustryKind, IndustrySpec};
        let industries = vec![
            Industry::with_tiles_spec(
                TileCoord::new(0, 0),
                IndustryKind::CoalMine,
                IndustrySpec::CoalMine,
                vec![TileCoord::new(0, 0)],
                7,
            )
            .with_instance_id(10),
        ];
        let colour = super::industry_palette_colour_for_instance(10, &industries);
        assert_eq!(colour, crate::sprites::CompanyColour::from_u8(7));
    }
}
