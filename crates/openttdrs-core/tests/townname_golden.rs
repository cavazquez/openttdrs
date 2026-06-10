//! Paridad del generador de nombres con `OpenTTD` real.
//!
//! `fixtures/townname_golden.txt` (`lang|seed|nombre`) fue generado compilando
//! los generadores reales de `OpenTTD/src/townname.cpp` + `table/townname.h`
//! con un harness mínimo (21 idiomas × 512 seeds).

#![allow(clippy::expect_used)]

use openttdrs_core::townname::generate_town_name;

#[test]
fn matches_openttd_reference_output() {
    let golden = include_str!("fixtures/townname_golden.txt");
    let mut checked = 0_u32;
    for line in golden.lines() {
        let mut parts = line.splitn(3, '|');
        let (Some(lang), Some(seed), Some(expected)) = (parts.next(), parts.next(), parts.next())
        else {
            panic!("línea malformada en golden: {line:?}");
        };
        let lang: u16 = lang.parse().expect("lang numérico");
        let seed: u32 = seed.parse().expect("seed numérico");
        let got =
            generate_town_name(lang, seed).unwrap_or_else(|| panic!("lang {lang} fuera de rango"));
        assert_eq!(
            got, expected,
            "lang {lang} seed {seed}: esperado {expected:?}, obtenido {got:?}"
        );
        checked += 1;
    }
    assert_eq!(checked, 21 * 512);
}
