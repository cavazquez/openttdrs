//! Puerto fiel del generador de nombres de ciudades de `OpenTTD`
//! (`src/townname.cpp`): mismos algoritmos, mismas tablas y mismos nombres
//! para un mismo seed (`townnameparts`).
//!
//! Las tablas (`tables.rs`) se generan con `scripts/gen_townname_tables.py`
//! desde el checkout de referencia `OpenTTD/src/table/townname.h`.

mod tables;

use tables as t;

/// Cantidad de generadores incorporados (`BUILTIN_TOWNNAME_GENERATOR_COUNT`).
pub const TOWN_NAME_GENERATOR_COUNT: u16 = 21;

/// Primer `StringID` de nombres de ciudad (`SPECSTR_TOWNNAME_START`).
pub const SPECSTR_TOWNNAME_START: u16 = 0x20C0;

/// Género de sustantivos checos (`CzechGender`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CzechGender {
    SMasc,
    SFem,
    SNeut,
    PMasc,
    PFem,
    PNeut,
    /// El final elige el género.
    Free,
    /// Como `Free` pero sin neutros.
    NFree,
}

impl CzechGender {
    /// Índice en `NAME_CZECH_PATMOD` (solo géneros reales).
    fn patmod_index(self) -> usize {
        match self {
            Self::SMasc => 0,
            Self::SFem => 1,
            Self::SNeut => 2,
            Self::PMasc => 3,
            Self::PFem => 4,
            Self::PNeut | Self::Free | Self::NFree => 5,
        }
    }
}

/// Bits de `CzechChooseFlag` (Colour=1, Postfix=2, `NoPostfix`=4).
const CZC_POSTFIX: u8 = 2;
const CZC_NO_POSTFIX: u8 = 4;

pub(crate) struct CzechNameSubst {
    pub gender: CzechGender,
    /// Bits `CzechAllowFlag`: Short=1, Middle=2, Long=4.
    pub allow: u8,
    /// Bits `CzechChooseFlag`: Colour=1, Postfix=2, `NoPostfix`=4.
    pub choose: u8,
    pub name: &'static str,
}

pub(crate) struct CzechNameAdj {
    /// Índice de `CzechPattern` (0 jarní, 1 mladý, 2 přívl.).
    pub pattern: usize,
    pub choose: u8,
    pub name: &'static str,
}

/// `GB(seed, shift_by, 16)`: ventana de 16 bits del seed.
fn gb16(seed: u32, shift_by: u8) -> u32 {
    (seed >> shift_by) & 0xFFFF
}

/// `SeedChance`: número en `0..max` derivado del seed.
fn seed_chance(shift_by: u8, max: usize, seed: u32) -> usize {
    (gb16(seed, shift_by) as usize * max) >> 16
}

/// `SeedChanceBias`: como [`seed_chance`] pero con rango `-bias..max`;
/// `None` representa los valores negativos (segmento opcional omitido).
fn seed_chance_bias(shift_by: u8, max: usize, seed: u32, bias: usize) -> Option<usize> {
    seed_chance(shift_by, max + bias, seed).checked_sub(bias)
}

/// `SeedModChance`: distribución por módulo (usada por checo/turco/italiano/catalán).
fn seed_mod_chance(shift_by: u8, max: usize, seed: u32) -> usize {
    (seed >> shift_by) as usize % max
}

/// `ReplaceWords`: reemplaza el prefijo `org` por `rep` (mismo largo).
fn replace_words(org: &str, rep: &str, name: &mut String) {
    if name.starts_with(org) {
        name.replace_range(..org.len(), rep);
    }
}

/// `ReplaceEnglishWords`: arregla groserías y combinaciones feas.
fn replace_english_words(name: &mut String, original: bool) {
    if original {
        replace_words("Ce", "Ke", name);
        replace_words("Ci", "Ki", name);
    }
    replace_words("Cunt", "East", name);
    replace_words("Slag", "Pits", name);
    replace_words("Slut", "Edin", name);
    if !original {
        replace_words("Fart", "Boot", name);
    }
    replace_words("Drar", "Quar", name);
    replace_words("Dreh", "Bash", name);
    replace_words("Frar", "Shor", name);
    replace_words("Grar", "Aber", name);
    replace_words("Brar", "Over", name);
    replace_words("Wrar", if original { "Inve" } else { "Stan" }, name);
}

fn make_english_original(seed: u32, out: &mut String) {
    if let Some(i) = seed_chance_bias(0, t::NAME_ORIGINAL_ENGLISH_1.len(), seed, 50) {
        out.push_str(t::NAME_ORIGINAL_ENGLISH_1[i]);
    }
    out.push_str(
        t::NAME_ORIGINAL_ENGLISH_2[seed_chance(4, t::NAME_ORIGINAL_ENGLISH_2.len(), seed)],
    );
    out.push_str(
        t::NAME_ORIGINAL_ENGLISH_3[seed_chance(7, t::NAME_ORIGINAL_ENGLISH_3.len(), seed)],
    );
    out.push_str(
        t::NAME_ORIGINAL_ENGLISH_4[seed_chance(10, t::NAME_ORIGINAL_ENGLISH_4.len(), seed)],
    );
    out.push_str(
        t::NAME_ORIGINAL_ENGLISH_5[seed_chance(13, t::NAME_ORIGINAL_ENGLISH_5.len(), seed)],
    );
    if let Some(i) = seed_chance_bias(15, t::NAME_ORIGINAL_ENGLISH_6.len(), seed, 60) {
        out.push_str(t::NAME_ORIGINAL_ENGLISH_6[i]);
    }
    replace_english_words(out, true);
}

fn make_english_additional(seed: u32, out: &mut String) {
    if let Some(i) = seed_chance_bias(0, t::NAME_ADDITIONAL_ENGLISH_PREFIX.len(), seed, 50) {
        out.push_str(t::NAME_ADDITIONAL_ENGLISH_PREFIX[i]);
    }
    if seed_chance(3, 20, seed) >= 14 {
        out.push_str(
            t::NAME_ADDITIONAL_ENGLISH_1A
                [seed_chance(6, t::NAME_ADDITIONAL_ENGLISH_1A.len(), seed)],
        );
    } else {
        out.push_str(
            t::NAME_ADDITIONAL_ENGLISH_1B1
                [seed_chance(6, t::NAME_ADDITIONAL_ENGLISH_1B1.len(), seed)],
        );
        out.push_str(
            t::NAME_ADDITIONAL_ENGLISH_1B2
                [seed_chance(9, t::NAME_ADDITIONAL_ENGLISH_1B2.len(), seed)],
        );
        if seed_chance(11, 20, seed) >= 4 {
            out.push_str(
                t::NAME_ADDITIONAL_ENGLISH_1B3A
                    [seed_chance(12, t::NAME_ADDITIONAL_ENGLISH_1B3A.len(), seed)],
            );
        } else {
            out.push_str(
                t::NAME_ADDITIONAL_ENGLISH_1B3B
                    [seed_chance(12, t::NAME_ADDITIONAL_ENGLISH_1B3B.len(), seed)],
            );
        }
    }
    out.push_str(
        t::NAME_ADDITIONAL_ENGLISH_2[seed_chance(14, t::NAME_ADDITIONAL_ENGLISH_2.len(), seed)],
    );
    if let Some(i) = seed_chance_bias(15, t::NAME_ADDITIONAL_ENGLISH_3.len(), seed, 60) {
        out.push_str(t::NAME_ADDITIONAL_ENGLISH_3[i]);
    }
    replace_english_words(out, false);
}

fn make_austrian(seed: u32, out: &mut String) {
    // Bad, Maria, Gross, ...
    if let Some(i) = seed_chance_bias(0, t::NAME_AUSTRIAN_A1.len(), seed, 15) {
        out.push_str(t::NAME_AUSTRIAN_A1[i]);
    }

    let mut j = 0;
    let i = seed_chance(4, 6, seed);
    if i >= 4 {
        // Kaisers-kirchen
        out.push_str(t::NAME_AUSTRIAN_A2[seed_chance(7, t::NAME_AUSTRIAN_A2.len(), seed)]);
        out.push_str(t::NAME_AUSTRIAN_A3[seed_chance(13, t::NAME_AUSTRIAN_A3.len(), seed)]);
    } else if i >= 2 {
        // St. Johann
        out.push_str(t::NAME_AUSTRIAN_A5[seed_chance(7, t::NAME_AUSTRIAN_A5.len(), seed)]);
        out.push_str(t::NAME_AUSTRIAN_A6[seed_chance(9, t::NAME_AUSTRIAN_A6.len(), seed)]);
        j = 1; // más probable " an der " o " am "
    } else {
        // Zell
        out.push_str(t::NAME_AUSTRIAN_A4[seed_chance(7, t::NAME_AUSTRIAN_A4.len(), seed)]);
    }

    let i = seed_chance(1, 6, seed);
    if i >= 4 - j {
        // an der Donau (ríos)
        out.push_str(t::NAME_AUSTRIAN_F1[seed_chance(4, t::NAME_AUSTRIAN_F1.len(), seed)]);
        out.push_str(t::NAME_AUSTRIAN_F2[seed_chance(5, t::NAME_AUSTRIAN_F2.len(), seed)]);
    } else if i >= 2 - j {
        // am Dachstein (montañas)
        out.push_str(t::NAME_AUSTRIAN_B1[seed_chance(4, t::NAME_AUSTRIAN_B1.len(), seed)]);
        out.push_str(t::NAME_AUSTRIAN_B2[seed_chance(5, t::NAME_AUSTRIAN_B2.len(), seed)]);
    }
}

fn make_german(seed: u32, out: &mut String) {
    let seed_derivative = seed_chance(7, 28, seed);

    if seed_derivative == 12 || seed_derivative == 19 {
        out.push_str(t::NAME_GERMAN_PRE[seed_chance(2, t::NAME_GERMAN_PRE.len(), seed)]);
    }

    let i = seed_chance(3, t::NAME_GERMAN_REAL.len() + t::NAME_GERMAN_1.len(), seed);
    if i < t::NAME_GERMAN_REAL.len() {
        out.push_str(t::NAME_GERMAN_REAL[i]);
    } else {
        out.push_str(t::NAME_GERMAN_1[i - t::NAME_GERMAN_REAL.len()]);
        out.push_str(t::NAME_GERMAN_2[seed_chance(5, t::NAME_GERMAN_2.len(), seed)]);
    }

    if seed_derivative == 24 {
        let i = seed_chance(
            9,
            t::NAME_GERMAN_4_AN_DER.len() + t::NAME_GERMAN_4_AM.len(),
            seed,
        );
        if i < t::NAME_GERMAN_4_AN_DER.len() {
            out.push_str(t::NAME_GERMAN_3_AN_DER[0]);
            out.push_str(t::NAME_GERMAN_4_AN_DER[i]);
        } else {
            out.push_str(t::NAME_GERMAN_3_AM[0]);
            out.push_str(t::NAME_GERMAN_4_AM[i - t::NAME_GERMAN_4_AN_DER.len()]);
        }
    }
}

fn make_spanish(seed: u32, out: &mut String) {
    out.push_str(t::NAME_SPANISH_REAL[seed_chance(0, t::NAME_SPANISH_REAL.len(), seed)]);
}

fn make_french(seed: u32, out: &mut String) {
    out.push_str(t::NAME_FRENCH_REAL[seed_chance(0, t::NAME_FRENCH_REAL.len(), seed)]);
}

fn make_silly(seed: u32, out: &mut String) {
    out.push_str(t::NAME_SILLY_1[seed_chance(0, t::NAME_SILLY_1.len(), seed)]);
    out.push_str(t::NAME_SILLY_2[seed_chance(16, t::NAME_SILLY_2.len(), seed)]);
}

fn make_swedish(seed: u32, out: &mut String) {
    if let Some(i) = seed_chance_bias(0, t::NAME_SWEDISH_1.len(), seed, 50) {
        out.push_str(t::NAME_SWEDISH_1[i]);
    }
    if seed_chance(4, 5, seed) >= 3 {
        out.push_str(t::NAME_SWEDISH_2[seed_chance(7, t::NAME_SWEDISH_2.len(), seed)]);
    } else {
        out.push_str(t::NAME_SWEDISH_2A[seed_chance(7, t::NAME_SWEDISH_2A.len(), seed)]);
        out.push_str(t::NAME_SWEDISH_2B[seed_chance(10, t::NAME_SWEDISH_2B.len(), seed)]);
        out.push_str(t::NAME_SWEDISH_2C[seed_chance(13, t::NAME_SWEDISH_2C.len(), seed)]);
    }
    out.push_str(t::NAME_SWEDISH_3[seed_chance(16, t::NAME_SWEDISH_3.len(), seed)]);
}

fn make_dutch(seed: u32, out: &mut String) {
    if let Some(i) = seed_chance_bias(0, t::NAME_DUTCH_1.len(), seed, 50) {
        out.push_str(t::NAME_DUTCH_1[i]);
    }
    if seed_chance(6, 9, seed) > 4 {
        out.push_str(t::NAME_DUTCH_2[seed_chance(9, t::NAME_DUTCH_2.len(), seed)]);
    } else {
        out.push_str(t::NAME_DUTCH_3[seed_chance(9, t::NAME_DUTCH_3.len(), seed)]);
        out.push_str(t::NAME_DUTCH_4[seed_chance(12, t::NAME_DUTCH_4.len(), seed)]);
    }
    out.push_str(t::NAME_DUTCH_5[seed_chance(15, t::NAME_DUTCH_5.len(), seed)]);
}

fn make_finnish(seed: u32, out: &mut String) {
    // Nombre de una o dos partes según el seed.
    if seed_chance(0, 15, seed) >= 10 {
        out.push_str(t::NAME_FINNISH_REAL[seed_chance(2, t::NAME_FINNISH_REAL.len(), seed)]);
        return;
    }

    if seed_chance(0, 15, seed) >= 5 {
        // Dos partes: _name_finnish_1 + "la"/"lä".
        let sel = seed_chance(0, t::NAME_FINNISH_1.len(), seed);
        out.push_str(t::NAME_FINNISH_1[sel]);

        if out.ends_with('i') {
            out.pop();
            out.push('e');
        }
        if out
            .bytes()
            .any(|b| matches!(b, b'a' | b'o' | b'u' | b'A' | b'O' | b'U'))
        {
            out.push_str("la");
        } else {
            out.push_str("l\u{00e4}");
        }
        return;
    }

    // Dos partes: _name_finnish_{1,2} + _name_finnish_3.
    let sel = seed_chance(2, t::NAME_FINNISH_1.len() + t::NAME_FINNISH_2.len(), seed);
    if sel >= t::NAME_FINNISH_1.len() {
        out.push_str(t::NAME_FINNISH_2[sel - t::NAME_FINNISH_1.len()]);
    } else {
        out.push_str(t::NAME_FINNISH_1[sel]);
    }
    out.push_str(t::NAME_FINNISH_3[seed_chance(10, t::NAME_FINNISH_3.len(), seed)]);
}

fn make_polish(seed: u32, out: &mut String) {
    let i = seed_chance(
        0,
        t::NAME_POLISH_2_O.len()
            + t::NAME_POLISH_2_M.len()
            + t::NAME_POLISH_2_F.len()
            + t::NAME_POLISH_2_N.len(),
        seed,
    );
    let j = seed_chance(2, 20, seed);

    if i < t::NAME_POLISH_2_O.len() {
        out.push_str(t::NAME_POLISH_2_O[seed_chance(3, t::NAME_POLISH_2_O.len(), seed)]);
        return;
    }
    if i < t::NAME_POLISH_2_M.len() + t::NAME_POLISH_2_O.len() {
        if j < 4 {
            out.push_str(t::NAME_POLISH_1_M[seed_chance(5, t::NAME_POLISH_1_M.len(), seed)]);
        }
        out.push_str(t::NAME_POLISH_2_M[seed_chance(7, t::NAME_POLISH_2_M.len(), seed)]);
        if (4..16).contains(&j) {
            out.push_str(t::NAME_POLISH_3_M[seed_chance(10, t::NAME_POLISH_3_M.len(), seed)]);
        }
        return;
    }
    if i < t::NAME_POLISH_2_F.len() + t::NAME_POLISH_2_M.len() + t::NAME_POLISH_2_O.len() {
        if j < 4 {
            out.push_str(t::NAME_POLISH_1_F[seed_chance(5, t::NAME_POLISH_1_F.len(), seed)]);
        }
        out.push_str(t::NAME_POLISH_2_F[seed_chance(7, t::NAME_POLISH_2_F.len(), seed)]);
        if (4..16).contains(&j) {
            out.push_str(t::NAME_POLISH_3_F[seed_chance(10, t::NAME_POLISH_3_F.len(), seed)]);
        }
        return;
    }

    if j < 4 {
        out.push_str(t::NAME_POLISH_1_N[seed_chance(5, t::NAME_POLISH_1_N.len(), seed)]);
    }
    out.push_str(t::NAME_POLISH_2_N[seed_chance(7, t::NAME_POLISH_2_N.len(), seed)]);
    if (4..16).contains(&j) {
        out.push_str(t::NAME_POLISH_3_N[seed_chance(10, t::NAME_POLISH_3_N.len(), seed)]);
    }
}

#[allow(clippy::too_many_lines)]
fn make_czech(seed: u32, out: &mut String) {
    // 1:3 de probabilidad de usar un nombre real.
    if seed_mod_chance(0, 4, seed) == 0 {
        out.push_str(t::NAME_CZECH_REAL[seed_mod_chance(4, t::NAME_CZECH_REAL.len(), seed)]);
        return;
    }

    // Probabilidad de prefijos/sufijos:
    // 0..11 prefijo, 12..13 prefijo+sufijo, 14..16 sufijo, 17..31 nada.
    let prob_tails = seed_mod_chance(2, 32, seed);
    let mut do_prefix = prob_tails < 12;
    let do_suffix = prob_tails > 11 && prob_tails < 17;

    let prefix = if do_prefix {
        seed_mod_chance(5, t::NAME_CZECH_ADJ.len() * 12, seed) / 12
    } else {
        0
    };
    let suffix = if do_suffix {
        seed_mod_chance(7, t::NAME_CZECH_SUFFIX.len(), seed)
    } else {
        0
    };

    // 3:1 de probabilidad de usar un sustantivo dinámico.
    let mut stem = seed_mod_chance(
        9,
        t::NAME_CZECH_SUBST_FULL.len() + 3 * t::NAME_CZECH_SUBST_STEM.len(),
        seed,
    );

    let dynamic_subst = stem >= t::NAME_CZECH_SUBST_FULL.len();
    let mut gender;
    let mut choose;
    let mut postfix = 0_usize;
    let mut ending = 0_usize;

    if dynamic_subst {
        stem -= t::NAME_CZECH_SUBST_FULL.len();
        stem %= t::NAME_CZECH_SUBST_STEM.len();
        let s = &t::NAME_CZECH_SUBST_STEM[stem];
        gender = s.gender;
        choose = s.choose;
        let allow = s.allow;

        // Postfix opcional (1:1 de probabilidad de insertarlo).
        postfix = seed_mod_chance(14, t::NAME_CZECH_SUBST_POSTFIX.len() * 2, seed);
        if choose & CZC_POSTFIX != 0 {
            postfix %= t::NAME_CZECH_SUBST_POSTFIX.len();
        }
        if choose & CZC_NO_POSTFIX != 0 {
            postfix += t::NAME_CZECH_SUBST_POSTFIX.len();
        }
        if postfix < t::NAME_CZECH_SUBST_POSTFIX.len() {
            choose |= CZC_POSTFIX;
        } else {
            choose |= CZC_NO_POSTFIX;
        }

        // Segmento del array de finales con género compatible.
        let endings = t::NAME_CZECH_SUBST_ENDING;
        let mut ending_start: Option<usize> = None;
        let mut ending_stop: Option<usize> = None;
        let mut idx = 0;
        while idx < endings.len() {
            let e = &endings[idx];
            let matches_gender = gender == CzechGender::Free
                || (gender == CzechGender::NFree
                    && e.gender != CzechGender::SNeut
                    && e.gender != CzechGender::PNeut)
                || gender == e.gender;
            if matches_gender {
                if ending_start.is_none() {
                    ending_start = Some(idx);
                }
            } else if ending_start.is_some() {
                ending_stop = Some(idx - 1);
                break;
            }
            idx += 1;
        }
        let ending_start = ending_start.unwrap_or(0);
        let ending_stop = ending_stop.unwrap_or(idx - 1);

        // Mapa secuencial de finales compatibles con choose/allow.
        let mut map = Vec::with_capacity(ending_stop - ending_start + 1);
        for (offset, e) in endings[ending_start..=ending_stop].iter().enumerate() {
            if e.choose & choose == choose && e.allow & allow != 0 {
                map.push(ending_start + offset);
            }
        }
        debug_assert!(!map.is_empty());
        ending = map[seed_mod_chance(16, map.len(), seed)];
        // Género real definitivo (nunca Free/NFree) para ajustar el adjetivo.
        gender = endings[ending].gender;
    } else {
        let s = &t::NAME_CZECH_SUBST_FULL[stem];
        gender = s.gender;
        choose = s.choose;
    }

    if do_prefix && t::NAME_CZECH_ADJ[prefix].choose & choose != choose {
        // Descarta prefijos incompatibles.
        do_prefix = false;
    }

    if do_prefix {
        let adj = &t::NAME_CZECH_ADJ[prefix];
        out.push_str(adj.name);
        out.push_str(t::NAME_CZECH_PATMOD[gender.patmod_index()][adj.pattern]);
        out.push(' ');
    }

    if dynamic_subst {
        out.push_str(t::NAME_CZECH_SUBST_STEM[stem].name);
        if postfix < t::NAME_CZECH_SUBST_POSTFIX.len() {
            let poststr = t::NAME_CZECH_SUBST_POSTFIX[postfix];
            let endstr = t::NAME_CZECH_SUBST_ENDING[ending].name;
            // Evita los casos tipo "avava" y "Jananna" (comparación por bytes,
            // igual que el original).
            let p1 = poststr.as_bytes()[1];
            let e1 = endstr.as_bytes()[1];
            if p1 != b'v' || p1 != e1 {
                out.push_str(poststr);
            }
        }
        out.push_str(t::NAME_CZECH_SUBST_ENDING[ending].name);
    } else {
        out.push_str(t::NAME_CZECH_SUBST_FULL[stem].name);
    }

    if do_suffix {
        out.push(' ');
        out.push_str(t::NAME_CZECH_SUFFIX[suffix]);
    }
}

fn make_romanian(seed: u32, out: &mut String) {
    out.push_str(t::NAME_ROMANIAN_REAL[seed_chance(0, t::NAME_ROMANIAN_REAL.len(), seed)]);
}

fn make_slovak(seed: u32, out: &mut String) {
    out.push_str(t::NAME_SLOVAK_REAL[seed_chance(0, t::NAME_SLOVAK_REAL.len(), seed)]);
}

fn make_norwegian(seed: u32, out: &mut String) {
    // Bits 0-3: probabilidad 3/16 de nombre real.
    if seed_chance(0, 15, seed) < 3 {
        out.push_str(t::NAME_NORWEGIAN_REAL[seed_chance(4, t::NAME_NORWEGIAN_REAL.len(), seed)]);
        return;
    }
    out.push_str(t::NAME_NORWEGIAN_1[seed_chance(4, t::NAME_NORWEGIAN_1.len(), seed)]);
    out.push_str(t::NAME_NORWEGIAN_2[seed_chance(11, t::NAME_NORWEGIAN_2.len(), seed)]);
}

fn make_hungarian(seed: u32, out: &mut String) {
    if seed_chance(12, 15, seed) < 3 {
        out.push_str(t::NAME_HUNGARIAN_REAL[seed_chance(0, t::NAME_HUNGARIAN_REAL.len(), seed)]);
        return;
    }

    let i = seed_chance(3, t::NAME_HUNGARIAN_1.len() * 3, seed);
    if i < t::NAME_HUNGARIAN_1.len() {
        out.push_str(t::NAME_HUNGARIAN_1[i]);
    }

    out.push_str(t::NAME_HUNGARIAN_2[seed_chance(3, t::NAME_HUNGARIAN_2.len(), seed)]);
    out.push_str(t::NAME_HUNGARIAN_3[seed_chance(6, t::NAME_HUNGARIAN_3.len(), seed)]);

    let i = seed_chance(10, t::NAME_HUNGARIAN_4.len() * 3, seed);
    if i < t::NAME_HUNGARIAN_4.len() {
        out.push_str(t::NAME_HUNGARIAN_4[i]);
    }
}

fn make_swiss(seed: u32, out: &mut String) {
    out.push_str(t::NAME_SWISS_REAL[seed_chance(0, t::NAME_SWISS_REAL.len(), seed)]);
}

fn make_danish(seed: u32, out: &mut String) {
    if let Some(i) = seed_chance_bias(0, t::NAME_DANISH_1.len(), seed, 50) {
        out.push_str(t::NAME_DANISH_1[i]);
    }
    out.push_str(t::NAME_DANISH_2[seed_chance(7, t::NAME_DANISH_2.len(), seed)]);
    out.push_str(t::NAME_DANISH_3[seed_chance(16, t::NAME_DANISH_3.len(), seed)]);
}

fn make_turkish(seed: u32, out: &mut String) {
    match seed_mod_chance(0, 5, seed) {
        0 => {
            out.push_str(
                t::NAME_TURKISH_PREFIX[seed_mod_chance(2, t::NAME_TURKISH_PREFIX.len(), seed)],
            );
            out.push_str(
                t::NAME_TURKISH_MIDDLE[seed_mod_chance(4, t::NAME_TURKISH_MIDDLE.len(), seed)],
            );
            if seed_mod_chance(0, 7, seed) == 0 {
                out.push_str(
                    t::NAME_TURKISH_SUFFIX[seed_mod_chance(10, t::NAME_TURKISH_SUFFIX.len(), seed)],
                );
            }
        }
        1 | 2 => {
            out.push_str(
                t::NAME_TURKISH_PREFIX[seed_mod_chance(2, t::NAME_TURKISH_PREFIX.len(), seed)],
            );
            out.push_str(
                t::NAME_TURKISH_SUFFIX[seed_mod_chance(4, t::NAME_TURKISH_SUFFIX.len(), seed)],
            );
        }
        _ => {
            out.push_str(
                t::NAME_TURKISH_REAL[seed_mod_chance(4, t::NAME_TURKISH_REAL.len(), seed)],
            );
        }
    }
}

fn make_italian(seed: u32, out: &mut String) {
    if seed_mod_chance(0, 6, seed) == 0 {
        // Nombres reales.
        out.push_str(t::NAME_ITALIAN_REAL[seed_mod_chance(4, t::NAME_ITALIAN_REAL.len(), seed)]);
        return;
    }

    if seed_mod_chance(0, 8, seed) == 0 {
        out.push_str(t::NAME_ITALIAN_PREF[seed_mod_chance(11, t::NAME_ITALIAN_PREF.len(), seed)]);
    }

    let i = seed_chance(0, 2, seed);
    if i == 0 {
        out.push_str(t::NAME_ITALIAN_1M[seed_mod_chance(4, t::NAME_ITALIAN_1M.len(), seed)]);
    } else {
        out.push_str(t::NAME_ITALIAN_1F[seed_mod_chance(4, t::NAME_ITALIAN_1F.len(), seed)]);
    }

    if seed_mod_chance(3, 3, seed) == 0 {
        out.push_str(t::NAME_ITALIAN_2[seed_mod_chance(11, t::NAME_ITALIAN_2.len(), seed)]);
        out.push_str(if i == 0 { "o" } else { "a" });
    } else {
        out.push_str(t::NAME_ITALIAN_2I[seed_mod_chance(16, t::NAME_ITALIAN_2I.len(), seed)]);
    }

    if seed_mod_chance(15, 4, seed) == 0 {
        if seed_mod_chance(5, 2, seed) == 0 {
            // Sufijo genérico.
            out.push_str(t::NAME_ITALIAN_3[seed_mod_chance(4, t::NAME_ITALIAN_3.len(), seed)]);
        } else {
            // Sufijo de río.
            out.push_str(
                t::NAME_ITALIAN_RIVER1[seed_mod_chance(4, t::NAME_ITALIAN_RIVER1.len(), seed)],
            );
            out.push_str(
                t::NAME_ITALIAN_RIVER2[seed_mod_chance(16, t::NAME_ITALIAN_RIVER2.len(), seed)],
            );
        }
    }
}

fn make_catalan(seed: u32, out: &mut String) {
    if seed_mod_chance(0, 3, seed) == 0 {
        // Nombres reales.
        out.push_str(t::NAME_CATALAN_REAL[seed_mod_chance(4, t::NAME_CATALAN_REAL.len(), seed)]);
        return;
    }

    if seed_mod_chance(0, 2, seed) == 0 {
        out.push_str(t::NAME_CATALAN_PREF[seed_mod_chance(11, t::NAME_CATALAN_PREF.len(), seed)]);
    }

    let i = seed_chance(0, 2, seed);
    if i == 0 {
        out.push_str(t::NAME_CATALAN_1M[seed_mod_chance(4, t::NAME_CATALAN_1M.len(), seed)]);
        out.push_str(t::NAME_CATALAN_2M[seed_mod_chance(11, t::NAME_CATALAN_2M.len(), seed)]);
    } else {
        out.push_str(t::NAME_CATALAN_1F[seed_mod_chance(4, t::NAME_CATALAN_1F.len(), seed)]);
        out.push_str(t::NAME_CATALAN_2F[seed_mod_chance(11, t::NAME_CATALAN_2F.len(), seed)]);
    }

    if seed_mod_chance(15, 5, seed) == 0 {
        if seed_mod_chance(5, 2, seed) == 0 {
            // Sufijo genérico.
            out.push_str(t::NAME_CATALAN_3[seed_mod_chance(4, t::NAME_CATALAN_3.len(), seed)]);
        } else {
            // Sufijo de río.
            out.push_str(
                t::NAME_CATALAN_RIVER1[seed_mod_chance(4, t::NAME_CATALAN_RIVER1.len(), seed)],
            );
        }
    }
}

/// Genera el nombre de ciudad para un idioma (índice de `_town_name_generators`)
/// y un seed (`townnameparts`). `None` si el índice está fuera de rango.
#[must_use]
pub fn generate_town_name(lang: u16, seed: u32) -> Option<String> {
    let mut out = String::new();
    match lang {
        0 => make_english_original(seed, &mut out),
        1 => make_french(seed, &mut out),
        2 => make_german(seed, &mut out),
        3 => make_english_additional(seed, &mut out),
        4 => make_spanish(seed, &mut out),
        5 => make_silly(seed, &mut out),
        6 => make_swedish(seed, &mut out),
        7 => make_dutch(seed, &mut out),
        8 => make_finnish(seed, &mut out),
        9 => make_polish(seed, &mut out),
        10 => make_slovak(seed, &mut out),
        11 => make_norwegian(seed, &mut out),
        12 => make_hungarian(seed, &mut out),
        13 => make_austrian(seed, &mut out),
        14 => make_romanian(seed, &mut out),
        15 => make_czech(seed, &mut out),
        16 => make_swiss(seed, &mut out),
        17 => make_danish(seed, &mut out),
        18 => make_turkish(seed, &mut out),
        19 => make_italian(seed, &mut out),
        20 => make_catalan(seed, &mut out),
        _ => return None,
    }
    Some(out)
}

/// Nombre de ciudad según los campos del save (`townnamegrfid`,
/// `townnametype`, `townnameparts`). `None` si el nombre lo genera un `NewGRF`
/// o el tipo no es un generador incorporado.
#[must_use]
pub fn town_name_from_save(grfid: u32, name_type: u16, parts: u32) -> Option<String> {
    if grfid != 0 {
        return None;
    }
    let lang = name_type.checked_sub(SPECSTR_TOWNNAME_START)?;
    if lang >= TOWN_NAME_GENERATOR_COUNT {
        return None;
    }
    generate_town_name(lang, parts)
}

#[cfg(test)]
mod tests;
