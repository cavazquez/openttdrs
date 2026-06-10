//! Tests unitarios del generador de nombres (la paridad completa contra
//! `OpenTTD` real está en `tests/townname_golden.rs`).

#![allow(clippy::expect_used)]

use super::*;

#[test]
fn english_original_seed_zero_applies_replacements() {
    // Con seed 0: "Wrar" + "n" + "ville" → ReplaceWords("Wrar", "Inve").
    assert_eq!(generate_town_name(0, 0).as_deref(), Some("Invenville"));
}

#[test]
fn out_of_range_lang_returns_none() {
    assert_eq!(generate_town_name(TOWN_NAME_GENERATOR_COUNT, 1234), None);
    assert_eq!(generate_town_name(u16::MAX, 0), None);
}

#[test]
fn deterministic_for_same_seed() {
    for lang in 0..TOWN_NAME_GENERATOR_COUNT {
        for seed in [0_u32, 1, 0xDEAD_BEEF, u32::MAX] {
            let a = generate_town_name(lang, seed);
            let b = generate_town_name(lang, seed);
            assert_eq!(a, b);
            assert!(!a.expect("idioma válido").is_empty());
        }
    }
}

#[test]
fn town_name_from_save_maps_specstr_range() {
    // SPECSTR_TOWNNAME_START + 0 = inglés original.
    assert_eq!(
        town_name_from_save(0, SPECSTR_TOWNNAME_START, 0).as_deref(),
        Some("Invenville")
    );
    // NewGRF (grfid != 0): no podemos replicarlo.
    assert_eq!(
        town_name_from_save(0x4A47_5246, SPECSTR_TOWNNAME_START, 0),
        None
    );
    // Fuera del rango de generadores incorporados.
    assert_eq!(town_name_from_save(0, SPECSTR_TOWNNAME_START - 1, 0), None);
    assert_eq!(
        town_name_from_save(0, SPECSTR_TOWNNAME_START + TOWN_NAME_GENERATOR_COUNT, 0),
        None
    );
}
